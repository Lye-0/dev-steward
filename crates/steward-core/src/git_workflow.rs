use crate::{inventory::manager, models::*, process::*, projects};
fn command(label: &str, program: &str, args: &[&str], p: &Project, kind: &str) -> Step {
    Step {
        id: id(),
        label: label.into(),
        program: program.into(),
        args: strings(args),
        cwd: p.path.clone(),
        kind: kind.into(),
        risk: "external".into(),
        status: "pending".into(),
        output: String::new(),
    }
}
fn base(p: &Project, title: String, steps: Vec<Step>) -> Plan {
    Plan {
        id: id(),
        environment: p.environment.clone(),
        project_id: Some(p.id.clone()),
        title,
        reason: "リポジトリの状態を確認し、一工程ずつ実行します。".into(),
        evidence: vec![p.branch.clone(), p.remote.clone()],
        warnings: vec![
            "ネットワーク操作を含みます。失敗時に自動リセットや強制プッシュは行いません。".into(),
        ],
        steps,
        created_at: now(),
        status: "ready".into(),
        fingerprint: p.fingerprint.clone(),
    }
}
fn default_branch(env: &Environment, settings: &Settings, p: &Project) -> Result<String, String> {
    let output = projects::git(
        env,
        settings,
        &p.path,
        &["ls-remote", "--symref", "origin", "HEAD"],
    )?;
    let branch = output
        .lines()
        .find_map(|l| {
            l.strip_prefix("ref: refs/heads/")
                .and_then(|s| s.strip_suffix("\tHEAD"))
        })
        .ok_or("originの現在の既定ブランチを確認できません")?;
    if branch.is_empty() || branch.starts_with('-') {
        return Err("既定ブランチが不正です".into());
    }
    Ok(branch.into())
}
pub fn sync(env: &Environment, settings: &Settings, p: &Project) -> Result<Plan, String> {
    if !p.git_status.is_empty() {
        return Err("同期前に変更をコミットまたは退避してください".into());
    }
    let git = manager(env, settings, "git").ok_or("Gitがありません")?;
    let branch = default_branch(env, settings, p)?;
    Ok(base(
        p,
        format!("{} を既定ブランチへ同期", p.name),
        vec![
            command(
                "originから最新情報を取得",
                &git,
                &["fetch", "origin"],
                p,
                "fetch",
            ),
            command(
                "既定ブランチへ切り替え",
                &git,
                &["switch", &branch],
                p,
                "switch",
            ),
            command(
                "fast-forwardで同期",
                &git,
                &["merge", "--ff-only", &format!("origin/{branch}")],
                p,
                "sync",
            ),
        ],
    ))
}
pub fn pr_ready(env: &Environment, settings: &Settings, p: &Project) -> Result<String, String> {
    let gh = manager(env, settings, "gh").ok_or("GitHub CLIがありません")?;
    let out = checked(
        env,
        &gh,
        &[
            "pr",
            "view",
            "--json",
            "headRefOid,state,isDraft,statusCheckRollup",
        ],
        &p.path,
    )?;
    let v: serde_json::Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
    let local = projects::git(env, settings, &p.path, &["rev-parse", "HEAD"])?;
    validate_checks(&v, &local)
}
pub fn validate_checks(v: &serde_json::Value, local: &str) -> Result<String, String> {
    if v["state"] != "OPEN" || v["isDraft"] != false {
        return Err("マージ対象はOpenかつDraftではないPRです".into());
    }
    let head = v["headRefOid"]
        .as_str()
        .ok_or("PRのコミットを取得できません")?;
    if local != head {
        return Err("ローカルHEADとPRのコミットが一致しません。再調査してください。".into());
    }
    let checks = v["statusCheckRollup"]
        .as_array()
        .ok_or("CIの結果を取得できません")?;
    if checks.is_empty()
        || checks
            .iter()
            .any(|c| c["conclusion"] != "SUCCESS" && c["state"] != "SUCCESS")
    {
        return Err("CIがない、未完了、失敗、スキップのチェックがあります。全チェックの成功後に再確認してください。".into());
    }
    Ok(head.into())
}
pub fn merge(env: &Environment, settings: &Settings, p: &Project) -> Result<Plan, String> {
    if !p.git_status.is_empty() {
        return Err("ローカルの変更を先に整理してください".into());
    }
    let head = pr_ready(env, settings, p)?;
    let gh = manager(env, settings, "gh").ok_or("GitHub CLIがありません")?;
    let git = manager(env, settings, "git").ok_or("Gitがありません")?;
    let branch = default_branch(env, settings, p)?;
    let mut plan = base(
        p,
        format!("{} のPRをマージして同期", p.name),
        vec![
            command(
                "成功したコミットのPRをマージ",
                &gh,
                &["pr", "merge", "--merge", "--match-head-commit", &head],
                p,
                "merge-pr",
            ),
            command(
                "既定ブランチへ切り替え",
                &git,
                &["switch", &branch],
                p,
                "switch",
            ),
            command(
                "マージ結果をfast-forwardで取得",
                &git,
                &["pull", "--ff-only", "origin", &branch],
                p,
                "sync",
            ),
        ],
    );
    plan.evidence.push(format!("CI確認済みHEAD: {head}"));
    plan.warnings.push("マージはGitHub上の既定ブランチへ反映されます。実行直前にもCIとコミットの一致を確認します。".into());
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn response() -> serde_json::Value {
        json!({"state":"OPEN","isDraft":false,"headRefOid":"abc","statusCheckRollup":[{"conclusion":"SUCCESS"}]})
    }
    #[test]
    fn merge_requires_exact_head() {
        assert!(validate_checks(&response(), "other").is_err());
    }
    #[test]
    fn no_checks_are_not_success() {
        let mut v = response();
        v["statusCheckRollup"] = json!([]);
        assert!(validate_checks(&v, "abc").is_err());
    }
    #[test]
    fn draft_or_failed_or_skipped_blocks_merge() {
        for (key, value) in [
            ("isDraft", json!(true)),
            ("statusCheckRollup", json!([{"conclusion":"FAILURE"}])),
            ("statusCheckRollup", json!([{"conclusion":"SKIPPED"}])),
        ] {
            let mut v = response();
            v[key] = value;
            assert!(validate_checks(&v, "abc").is_err());
        }
    }
    #[test]
    fn exact_successful_head_passes() {
        assert_eq!(validate_checks(&response(), "abc").unwrap(), "abc");
    }
}
