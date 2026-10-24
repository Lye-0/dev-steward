//! Manager-specific, read-only candidate queries. Never executes project code.
use crate::{inventory::manager, models::*, process::*, updates};
use serde_json::Value;

pub fn outcome(tool: &Tool, status: &str, candidate: &str, note: &str) -> Update {
    Update {
        fingerprint: String::new(),
        name: tool.name.clone(),
        manager: tool.manager.clone(),
        current: tool.version.clone(),
        candidate: candidate.into(),
        latest: candidate.into(),
        source: tool.path.clone(),
        importance: status.into(),
        vulnerabilities: vec![],
        audit_status: "未監査".into(),
        note: note.into(),
        project_id: None,
        tool_id: Some(tool.id.clone()),
    }
}
pub fn executable(env: &Environment, settings: &Settings, name: &str) -> Result<String, String> {
    let program = manager(env, settings, name).ok_or_else(|| format!("{name}が未検出です"))?;
    if program.to_lowercase().ends_with(".bat") || program.to_lowercase().ends_with(".cmd") {
        return Err("バッチ経由は未対応です。管理元の実行ファイル本体を設定してください".into());
    }
    Ok(program)
}
pub fn version_lines(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|s| {
            let s = s.split_whitespace().next()?.trim_start_matches('v');
            let v = updates::numeric(s)?;
            (v.pre.is_empty() && s.split('.').count() == 3).then(|| s.to_string())
        })
        .collect()
}
pub fn select(tool: &Tool, versions: Vec<String>) -> Result<String, String> {
    let current = updates::numeric(&tool.version).ok_or("導入版を数値として確認できません")?;
    let compatible: Vec<_> = versions
        .into_iter()
        .filter(|s| {
            updates::numeric(s).is_some_and(|v| {
                v.pre.is_empty()
                    && v.major == current.major
                    && (tool.name != "python" || v.minor == current.minor)
            })
        })
        .collect();
    if compatible.is_empty() {
        return Err("同じ系列の配布版を取得できませんでした。候補なしとは判定していません".into());
    }
    Ok(updates::choose(&tool.version, compatible.into_iter())
        .unwrap_or_else(|| tool.version.clone()))
}
pub fn standard_python_tag(value: &str) -> bool {
    let tag = value
        .strip_prefix("PythonCore\\")
        .or_else(|| value.strip_prefix("PythonCore/"))
        .unwrap_or(value);
    regex::Regex::new(r"^3\.\d+(?:\.\d+)?$")
        .unwrap()
        .is_match(tag)
}
pub fn additional(env: &Environment, settings: &Settings, tool: &Tool) -> Result<String, String> {
    let package = if tool.manager == "conda" && tool.name == "node" {
        "nodejs"
    } else {
        tool.name.as_str()
    };
    let (name, args) = match tool.manager.as_str() {
        "fnm" => ("fnm", strings(&["list-remote"])),
        "pyenv" => ("pyenv", strings(&["install", "--list"])),
        "python-install-manager" => (
            "pymanager",
            strings(&["list", "--online", "--format=json", &tool.requested]),
        ),
        "conda" => ("conda", strings(&["search", "--json", package])),
        // nvm-windows only lists a short selection of releases. Use the official complete index.
        "nvm" => {
            let data: Value = updates::client()?
                .get("https://nodejs.org/dist/index.json")
                .send()
                .and_then(|r| r.error_for_status())
                .and_then(|r| r.json())
                .map_err(|e| e.to_string())?;
            let rows = data.as_array().ok_or("Node配布情報の形式に未対応です")?;
            return select(
                tool,
                rows.iter()
                    .filter_map(|v| v["version"].as_str())
                    .map(|s| s.trim_start_matches('v').to_string())
                    .collect(),
            );
        }
        _ => return Err("この管理元の候補取得には未対応です".into()),
    };
    let program = executable(env, settings, name)?;
    let out = checked(
        env,
        &program,
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
        &env.home,
    )?;
    let versions = match tool.manager.as_str() {
        "python-install-manager" => {
            let value: Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
            value["versions"]
                .as_array()
                .ok_or("versions配列がありません")?
                .iter()
                .filter_map(|v| v["sort-version"].as_str())
                .map(str::to_string)
                .collect()
        }
        "conda" => {
            let value: Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
            value[package]
                .as_array()
                .ok_or("Condaの検索結果を取得できません")?
                .iter()
                .filter_map(|v| v["version"].as_str())
                .map(str::to_string)
                .collect()
        }
        _ => version_lines(&out),
    };
    select(tool, versions)
}

/// A WinGet listing is not a universal proof of 'up to date': pins, sources and unknown versions may be excluded.
pub fn winget_rows(text: &str, inventory: &Inventory) -> Result<Vec<Update>, String> {
    let separator = text
        .lines()
        .any(|l| l.trim().len() > 10 && l.trim().chars().all(|c| c == '-'));
    if !separator {
        return Err("WinGet一覧の列形式を確認できません。取得ログを確認してください".into());
    }
    Ok(inventory.tools.iter().filter(|t|t.manager=="winget").map(|tool| {
        let found=text.lines().find_map(|line|{
            let fields:Vec<_>=line.split_whitespace().collect();
            let i=fields.iter().position(|s|*s==tool.name)?;
            if fields.len()!=i+4 {return None;}
            let (current,candidate,source)=(fields[i+1],fields[i+2],fields[i+3]);
            if candidate.contains('…') || candidate=="Unknown" {return None;}
            Some((current,candidate,source))
        });
        if let Some((current,candidate,source))=found {
            let status=if source!="winget" {"delegated"} else if updates::numeric(current).zip(updates::numeric(candidate)).is_some_and(|(a,b)| a.major!=b.major) {"major"} else {"maintenance"};
            let mut u=outcome(tool,status,candidate,"WinGetの更新一覧に掲載。互換性・メジャー変更は計画作成前に確認してください。");
            u.current=current.into();u.source=source.into();u
        } else {outcome(tool,if ["","不明","Unknown"].contains(&tool.version.as_str()) {"version-unknown"} else {"not-listed"},"","今回のWinGet更新一覧に一致するIDがありません。固定・不明な版・表示省略・対象外もあり、最新とは断定していません。")}
    }).collect())
}
pub fn apt_rows(text: &str, env: &Environment) -> Vec<Update> {
    text.lines().filter_map(|line|{
        let fields:Vec<_>=line.split_whitespace().collect();
        let (name,_suite)=fields.first()?.split_once('/')?;
        if fields.len()<4 || !name.chars().all(|c|c.is_ascii_alphanumeric() || "+.-:".contains(c)) {return None;}
        let old=line.rsplit_once(": ").and_then(|(_,s)|s.strip_suffix(']')).unwrap_or("");
        Some(Update{fingerprint:String::new(),name:name.into(),manager:"apt".into(),current:old.into(),candidate:fields[1].into(),latest:fields[1].into(),source:"APTローカルインデックス".into(),importance:"maintenance".into(),vulnerabilities:vec![],audit_status:"未監査".into(),note:"ローカルのAPTインデックスによる候補です。インデックス更新・更新実行はOSの管理元で行ってください。".into(),project_id:None,tool_id:Some(format!("{}:apt:{name}",env.id))})
    }).collect()
}
pub fn system(
    env: &Environment,
    settings: &Settings,
    inventory: &Inventory,
) -> Result<Value, String> {
    let (program, args) = if env.os == "windows" {
        (
            manager(env, settings, "winget").ok_or("WinGetが未検出です")?,
            strings(&["upgrade", "--disable-interactivity"]),
        )
    } else if env.os == "linux" {
        ("/usr/bin/apt".into(), strings(&["list", "--upgradable"]))
    } else {
        (
            manager(env, settings, "brew").ok_or("Homebrewが未検出です")?,
            strings(&["outdated", "--verbose"]),
        )
    };
    let mut inventory = inventory.clone();
    if env.os == "windows" && !inventory.tools.iter().any(|t| t.manager == "winget") {
        inventory.tools = crate::windows_apps::scan(env, &program)?;
    }
    let out = run(env, &program, &args, &env.home, 90)?;
    let no_updates = env.os == "windows" && out.code as u32 == 0x8a15002b;
    if out.code != 0 && !no_updates {
        return Err(format!(
            "更新一覧を取得できません ({}): {} {}",
            out.code, out.stdout, out.stderr
        ));
    }
    let output = format!("{}\n{}", out.stdout, out.stderr);
    let rows = if env.os == "windows" {
        if no_updates {
            inventory.tools.iter().filter(|t|t.manager=="winget").map(|t|outcome(t,"not-listed","","WinGetが適用可能な更新なしを返しました。すべてのアプリが最新という保証ではありません。" )).collect()
        } else {
            winget_rows(&output, &inventory)?
        }
    } else if env.os == "linux" {
        apt_rows(&output, env)
    } else {
        vec![]
    };
    Ok(
        serde_json::json!({"output":output,"updates":rows,"checked_at":now(),"structured":env.os=="windows" || env.os=="linux","program":program,"args":args}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn env() -> Environment {
        Environment {
            id: "unit".into(),
            name: "test".into(),
            kind: "local".into(),
            home: "/tmp".into(),
            os: "windows".into(),
        }
    }
    fn t(manager: &str, name: &str, version: &str) -> Tool {
        Tool {
            id: format!("unit:{manager}:{name}"),
            environment: "unit".into(),
            manager: manager.into(),
            name: name.into(),
            version: version.into(),
            path: "/tmp/runtime".into(),
            active: false,
            installed: true,
            source: String::new(),
            requested: String::new(),
            note: String::new(),
        }
    }
    #[test]
    fn python_tags_reject_variant_changes() {
        assert!(standard_python_tag("3.14"));
        assert!(standard_python_tag(r"PythonCore\3.14"));
        assert!(standard_python_tag("PythonCore/3.14.2"));
        assert!(!standard_python_tag("3.14-arm64"));
        assert!(!standard_python_tag(r"PythonTest\3.14"));
    }
    #[test]
    fn candidate_policy_and_failed_series_are_distinct() {
        assert_eq!(
            select(
                &t("fnm", "node", "22.1.0"),
                version_lines("v22.2.0\nv24.0.0\nv22.3.0-rc.1")
            )
            .unwrap(),
            "22.2.0"
        );
        assert_eq!(
            select(
                &t("pyenv", "python", "3.12.1"),
                version_lines("3.12.2\n3.13.1")
            )
            .unwrap(),
            "3.12.2"
        );
        assert!(select(&t("pyenv", "python", "3.12.1"), version_lines("3.13.1")).is_err());
    }
    #[test]
    fn winget_exact_ids_and_absent_are_not_latest() {
        let inv = Inventory {
            environment: "unit".into(),
            captured_at: now(),
            tools: vec![
                t("winget", "Example.App", "1.0"),
                t("winget", "Example.App2", "2.0"),
            ],
            findings: vec![],
            manager_paths: vec![],
        };
        let rows=winget_rows("名前 ID バージョン 利用可能 ソース\n-------------------------------\n日本語のアプリ  Example.App  1.0  1.1  winget\n",&inv).unwrap();
        assert_eq!(rows[0].candidate, "1.1");
        assert_eq!(rows[1].importance, "not-listed");
        assert!(winget_rows("changed format", &inv).is_err());
        let truncated = winget_rows(
            "--------------------\nName Example.Ap… 1.0 1.1 winget",
            &inv,
        )
        .unwrap();
        assert_eq!(truncated[0].importance, "not-listed");
    }
    #[test]
    fn apt_preserves_debian_versions() {
        let rows = apt_rows(
            "Listing...\nlibfoo/stable 1:2.3-4 amd64 [upgradable from: 1:2.3-3]",
            &env(),
        );
        assert_eq!(rows[0].candidate, "1:2.3-4");
        assert_eq!(rows[0].current, "1:2.3-3");
    }
}
