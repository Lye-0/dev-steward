use crate::models::*;
use reqwest::blocking::Client;
use semver::Version;
use serde_json::{Value, json};
use std::{
    collections::{BTreeSet, HashMap},
    time::Duration,
};

pub fn client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("dev-steward/0.1.0")
        .build()
        .map_err(|e| e.to_string())
}
pub fn numeric(s: &str) -> Option<Version> {
    let s = s.trim().trim_start_matches('v');
    Version::parse(s).ok().or_else(|| {
        if s.split('.').count() == 2 {
            Version::parse(&format!("{s}.0")).ok()
        } else {
            None
        }
    })
}
pub fn compatible(current: &Version, next: &Version) -> bool {
    next.pre.is_empty()
        && next.major == current.major
        && (current.major != 0 || current.minor == next.minor)
}
pub fn choose(current: &str, versions: impl Iterator<Item = String>) -> Option<String> {
    let current = numeric(current)?;
    versions
        .filter_map(|s| numeric(&s).map(|v| (s, v)))
        .filter(|(_, v)| v > &current && compatible(&current, v))
        .max_by(|a, b| a.1.cmp(&b.1))
        .map(|(s, _)| s)
}

pub fn project(project: &Project) -> Result<Vec<Update>, String> {
    let http = client()?;
    let locked: Vec<_> = project
        .requirements
        .iter()
        .filter(|r| r.section == "locked")
        .collect();
    if project.requirements.iter().any(|r| {
        r.manager == "registry" && r.requested.trim_end_matches('/') != "https://registry.npmjs.org"
    }) {
        return Err("カスタムnpmレジストリが指定されています。この版は公開npmレジストリとの照合に限定しています。設定を変更せず、管理元で確認してください。".into());
    }
    let mut details: HashMap<String, Vulnerability> = HashMap::new();
    let mut audit: HashMap<(String, String, String), Vec<Vulnerability>> = HashMap::new();
    let mut queries = vec![];
    let mut keys = vec![];
    for r in &locked {
        let ecosystem = match r.manager.as_str() {
            "npm" => "npm",
            "pypi" => "PyPI",
            "cargo" => "crates.io",
            _ => continue,
        };
        let key = (r.manager.clone(), r.name.clone(), r.resolved.clone());
        if keys.contains(&key) {
            continue;
        }
        keys.push(key);
        queries.push(json!({"package":{"name":r.name,"ecosystem":ecosystem},"version":r.resolved}));
    }
    let audit_note = if queries.is_empty() {
        "未照合：対応するロックファイルがありません".to_string()
    } else if queries.len() > 2000 {
        "未照合：2000件を超えています".into()
    } else {
        match http
            .post("https://api.osv.dev/v1/querybatch")
            .json(&json!({"queries":queries}))
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.json::<Value>())
        {
            Ok(v) => {
                if let Some(results) = v["results"].as_array().filter(|r| r.len() == keys.len()) {
                    for (key, result) in keys.iter().zip(results) {
                        let mut vulns = vec![];
                        for id in result["vulns"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|v| v["id"].as_str())
                        {
                            let entry = details.entry(id.to_string()).or_insert_with(|| {
                                let metadata = http
                                    .get(format!("https://api.osv.dev/v1/vulns/{id}"))
                                    .send()
                                    .and_then(|r| r.error_for_status())
                                    .and_then(|r| r.json::<Value>())
                                    .unwrap_or(Value::Null);
                                Vulnerability {
                                    id: id.into(),
                                    summary: metadata["summary"]
                                        .as_str()
                                        .unwrap_or("影響と修正条件はアドバイザリで確認してください")
                                        .into(),
                                    url: format!("https://osv.dev/vulnerability/{id}"),
                                    kind: metadata["database_specific"]["informational"]
                                        .as_str()
                                        .unwrap_or("vulnerability")
                                        .into(),
                                }
                            });
                            vulns.push(entry.clone());
                        }
                        audit.insert(key.clone(), vulns);
                    }
                    format!(
                        "OSV照合済み：{}版。公開データにない問題や実行環境全体の安全性を保証しません。",
                        keys.len()
                    )
                } else {
                    "照合失敗：OSVの応答形式が不正です".into()
                }
            }
            Err(e) => format!("照合失敗：{e}"),
        }
    };
    let mut result = vec![];
    let mut visited = BTreeSet::new();
    // Report transitive vulnerabilities even if no supported upgrade can be proposed.
    for r in &locked {
        if let Some(vulns) = audit
            .get(&(r.manager.clone(), r.name.clone(), r.resolved.clone()))
            .filter(|v| !v.is_empty())
        {
            result.push(Update { fingerprint: String::new(),name:r.name.clone(),manager:r.manager.clone(),current:r.resolved.clone(),candidate:String::new(),latest:String::new(),source:r.source.clone(),importance:"security".into(),vulnerabilities:vulns.clone(),audit_status:audit_note.clone(),note:"ロック済み依存の既知の問題。直接依存の更新計画と、アドバイザリの修正条件を照合してください。".into(),project_id:Some(project.id.clone()),tool_id:None});
        }
    }
    for r in project.requirements.iter().filter(|r| {
        ["npm", "pypi", "cargo"].contains(&r.manager.as_str())
            && r.section != "locked"
            && !r.section.starts_with("direct-lock:")
            && r.section != "peerDependencies"
    }) {
        if !visited.insert((r.manager.clone(), r.name.clone(), r.source.clone())) {
            continue;
        }
        if visited.len() > 100 {
            result.push(Update {
                fingerprint: String::new(),
                name: "調査上限".into(),
                manager: "info".into(),
                current: String::new(),
                candidate: String::new(),
                latest: String::new(),
                source: String::new(),
                importance: "unknown".into(),
                vulnerabilities: vec![],
                audit_status: audit_note.clone(),
                note: "直接依存100件まで照合しました。残りは未確認です。".into(),
                project_id: Some(project.id.clone()),
                tool_id: None,
            });
            break;
        }
        let matching: Vec<_> = locked
            .iter()
            .filter(|l| l.manager == r.manager && l.name == r.name)
            .collect();
        let current = resolve_current(r, &project.requirements);
        let Some(current) = current else {
            result.push(Update {
                fingerprint: String::new(),
                name: r.name.clone(),
                manager: r.manager.clone(),
                current: r.requested.clone(),
                candidate: String::new(),
                latest: String::new(),
                source: r.source.clone(),
                importance: "unknown".into(),
                vulnerabilities: vec![],
                audit_status: audit_note.clone(),
                note: "現在版を確定できません。対応するロックまたは厳密なバージョンが必要です。"
                    .into(),
                project_id: Some(project.id.clone()),
                tool_id: None,
            });
            continue;
        };
        let endpoint = if r.manager == "npm" {
            format!("https://registry.npmjs.org/{}", r.name.replace('/', "%2f"))
        } else if r.manager == "pypi" {
            format!("https://pypi.org/pypi/{}/json", r.name)
        } else {
            format!("https://crates.io/api/v1/crates/{}", r.name)
        };
        match http
            .get(&endpoint)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.json::<Value>())
        {
            Ok(v) => {
                let versions: Vec<String> = match r.manager.as_str() {
                    "npm" => v["versions"]
                        .as_object()
                        .map(|o| o.keys().cloned().collect())
                        .unwrap_or_default(),
                    "pypi" => v["releases"]
                        .as_object()
                        .map(|o| o.keys().cloned().collect())
                        .unwrap_or_default(),
                    _ => v["versions"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter(|v| v["yanked"] != true)
                                .filter_map(|v| v["num"].as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default(),
                };
                let candidate = choose(&current, versions.into_iter()).unwrap_or_default();
                let latest = match r.manager.as_str() {
                    "npm" => v["dist-tags"]["latest"].as_str(),
                    "pypi" => v["info"]["version"].as_str(),
                    _ => v["crate"]["default_version"].as_str(),
                }
                .unwrap_or_default()
                .into();
                let vulnerabilities = matching
                    .iter()
                    .flat_map(|l| {
                        audit
                            .get(&(r.manager.clone(), r.name.clone(), l.resolved.clone()))
                            .cloned()
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>();
                result.push(Update { fingerprint: String::new(),name:r.name.clone(),manager:r.manager.clone(),current,candidate,latest,source:r.source.clone(),importance:advisory_level(&vulnerabilities).into(),vulnerabilities,audit_status:audit_note.clone(),note:format!("{}。同一メジャー内（0.xは同一minor）の安定版候補。制約解決・実行検証は未確認。宣言元に対応するロック版を基準にしています。",endpoint),project_id:Some(project.id.clone()),tool_id:None});
            }
            Err(e) => result.push(Update {
                fingerprint: String::new(),
                name: r.name.clone(),
                manager: r.manager.clone(),
                current,
                candidate: String::new(),
                latest: String::new(),
                source: r.source.clone(),
                importance: "unknown".into(),
                vulnerabilities: vec![],
                audit_status: audit_note.clone(),
                note: format!("レジストリ照合失敗：{e}"),
                project_id: Some(project.id.clone()),
                tool_id: None,
            }),
        }
    }
    if result.is_empty() {
        result.push(Update {
            fingerprint: String::new(),
            name: "依存監査".into(),
            manager: "info".into(),
            current: String::new(),
            candidate: String::new(),
            latest: String::new(),
            source: String::new(),
            importance: "unknown".into(),
            vulnerabilities: vec![],
            audit_status: audit_note,
            note:
                "対応する直接依存の更新候補はありません。全エコシステムの監査完了を意味しません。"
                    .into(),
            project_id: Some(project.id.clone()),
            tool_id: None,
        });
    }
    for update in &mut result {
        update.fingerprint = project.fingerprint.clone();
        if update.importance == "maintenance"
            && update.candidate.is_empty()
            && update.vulnerabilities.is_empty()
        {
            update.importance = "current".into();
        }
    }
    Ok(result)
}

fn advisory_level(items: &[Vulnerability]) -> &'static str {
    if items.iter().any(|v| v.kind != "unmaintained") {
        "security"
    } else {
        "maintenance"
    }
}

pub fn resolve_current(requirement: &Requirement, requirements: &[Requirement]) -> Option<String> {
    if let Some(exact) = requirements.iter().find(|r| {
        r.manager == requirement.manager
            && r.name == requirement.name
            && r.source == requirement.source
            && r.section == format!("direct-lock:{}", requirement.section)
    }) {
        return numeric(&exact.resolved).map(|v| v.to_string());
    }
    let constraint = semver::VersionReq::parse(&requirement.requested).ok();
    let versions: BTreeSet<_> = requirements
        .iter()
        .filter(|r| {
            r.section == "locked" && r.manager == requirement.manager && r.name == requirement.name
        })
        .filter_map(|r| numeric(&r.resolved))
        .filter(|v| constraint.as_ref().is_none_or(|c| c.matches(v)))
        .collect();
    if versions.len() == 1 {
        return versions.first().map(ToString::to_string);
    }
    numeric(&requirement.requested).map(|v| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_major_only() {
        assert_eq!(
            choose(
                "1.2.0",
                ["1.3.0", "2.0.0", "1.9.0-beta.1"]
                    .into_iter()
                    .map(str::to_string)
            ),
            Some("1.3.0".into())
        );
    }
    #[test]
    fn zero_minor_is_preserved() {
        assert_eq!(
            choose("0.4.1", ["0.5.0", "0.4.2"].into_iter().map(str::to_string)),
            Some("0.4.2".into())
        );
    }
    #[test]
    fn no_update_on_non_semver() {
        assert!(choose("local+cuda", ["3.0.0".into()].into_iter()).is_none());
    }
    #[test]
    fn unrelated_workspace_major_is_not_current() {
        let requirement = Requirement {
            manager: "npm".into(),
            name: "example".into(),
            requested: "^1.0.0".into(),
            source: "apps/a/package.json".into(),
            section: "dependencies".into(),
            resolved: String::new(),
        };
        let mut locked = requirement.clone();
        locked.section = "locked".into();
        locked.resolved = "2.0.0".into();
        assert!(resolve_current(&requirement, &[locked]).is_none());
    }
    #[test]
    fn importer_wins_over_other_locked_versions() {
        let requirement = Requirement {
            manager: "npm".into(),
            name: "example".into(),
            requested: "*".into(),
            source: "apps/a/package.json".into(),
            section: "dependencies".into(),
            resolved: String::new(),
        };
        let mut importer = requirement.clone();
        importer.section = "direct-lock:dependencies".into();
        importer.resolved = "1.2.3".into();
        let mut other = importer.clone();
        other.source = "apps/b/package.json".into();
        other.resolved = "2.0.0".into();
        assert_eq!(
            resolve_current(&requirement, &[importer, other]).unwrap(),
            "1.2.3"
        );
    }
}
