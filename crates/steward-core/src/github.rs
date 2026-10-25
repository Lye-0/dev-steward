//! Immutable, read-only GitHub snapshots. No checkout, Git command or repository code execution.
use crate::{inventory::manager, models::*, process::*, projects};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub path: String,
    pub sha: String,
    pub size: u64,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub id: String,
    pub repository: String,
    pub host: String,
    pub url: String,
    pub branch: String,
    pub commit: String,
    pub tree: String,
    pub complete: bool,
    pub files: Vec<File>,
    pub project: Project,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub report_id: String,
    pub commit: String,
    pub created_at: String,
    pub changes: Vec<Update>,
    pub tools: Vec<Requirement>,
    pub checks: Vec<String>,
    pub steps: Vec<String>,
    pub warnings: Vec<String>,
    pub status: String,
    #[serde(default)]
    pub workspace: Option<String>,
}

pub fn validate_repo(repo: &str) -> Result<(), String> {
    let parts: Vec<_> = repo.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|p| {
            p.is_empty()
                || [".", ".."].contains(p)
                || !p
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        })
    {
        return Err("owner/repositoryの形式で指定してください".into());
    }
    Ok(())
}
fn sha(value: &Value) -> Result<String, String> {
    let value = value
        .as_str()
        .ok_or("コミット／ファイルのSHAがありません")?;
    if ![40, 64].contains(&value.len()) || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("SHAの形式が不正です".into());
    }
    Ok(value.into())
}
fn api(
    env: &Environment,
    settings: &Settings,
    host: Option<&str>,
    endpoint: &str,
    fields: &[(&str, &str)],
    raw: bool,
) -> Result<String, String> {
    let gh = manager(env, settings, "gh")
        .ok_or("GitHub CLIが未検出です。gh auth loginで認証してください")?;
    let mut args = strings(&[
        "api",
        "--method",
        "GET",
        "--header",
        if raw {
            "Accept: application/vnd.github.raw+json"
        } else {
            "Accept: application/vnd.github+json"
        },
    ]);
    if let Some(host) = host {
        args.extend(strings(&["--hostname", host]));
    }
    args.push(endpoint.into());
    for (key, value) in fields {
        args.extend(["--raw-field".into(), format!("{key}={value}")]);
    }
    let out = run(env, &gh, &args, &env.home, 40)?;
    if out.code != 0 {
        return Err(crate::redact::output(&format!(
            "GitHub API ({}): {}",
            out.code,
            out.stderr.trim()
        )));
    }
    Ok(out.stdout)
}
fn json(text: String) -> Result<Value, String> {
    serde_json::from_str(&text).map_err(|e| format!("GitHub JSON: {e}"))
}
pub fn repositories(env: &Environment, settings: &Settings) -> Result<Vec<Value>, String> {
    let mut result = vec![];
    for page in 1..=3 {
        let data = json(api(
            env,
            settings,
            None,
            "user/repos",
            &[
                ("per_page", "100"),
                ("page", &page.to_string()),
                ("sort", "updated"),
            ],
            false,
        )?)?;
        let rows = data
            .as_array()
            .ok_or("GitHubのリポジトリ一覧が配列ではありません")?;
        for row in rows {
            result.push(serde_json::json!({"nameWithOwner":row["full_name"],"url":row["html_url"],"isArchived":row["archived"],"description":row["description"],"defaultBranchRef":{"name":row["default_branch"]}}));
        }
        if rows.len() < 100 {
            break;
        }
    }
    Ok(result)
}
fn commit(
    env: &Environment,
    settings: &Settings,
    host: &str,
    repo: &str,
    branch: &str,
) -> Result<Value, String> {
    let data = json(api(
        env,
        settings,
        Some(host),
        &format!("repos/{repo}/commits"),
        &[("sha", branch), ("per_page", "1")],
        false,
    )?)?;
    data.as_array()
        .and_then(|a| a.first())
        .cloned()
        .ok_or_else(|| "対象ブランチのコミットがありません".into())
}
pub fn assert_current(
    env: &Environment,
    settings: &Settings,
    report: &Report,
) -> Result<(), String> {
    if sha(&commit(
        env,
        settings,
        &report.host,
        &report.repository,
        &report.branch,
    )?["sha"])?
        != report.commit
    {
        return Err("GitHubの対象ブランチが更新されました。再調査・再照合してください".into());
    }
    Ok(())
}

pub fn same_repository(remote: &str, report: &Report) -> bool {
    let parsed = reqwest::Url::parse(remote).ok().or_else(|| {
        let (host, path) = remote.split_once(':')?;
        if host.contains('/') || path.starts_with('\\') {
            return None;
        }
        reqwest::Url::parse(&format!("ssh://{}/{path}", host.rsplit('@').next()?)).ok()
    });
    let Some(url) = parsed else {
        return false;
    };
    if !["https", "http", "ssh", "git"].contains(&url.scheme()) {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = match url.port() {
        Some(22) if url.scheme() == "ssh" => host.into(),
        Some(port) => format!("{host}:{port}"),
        None => host.into(),
    };
    host.eq_ignore_ascii_case(&report.host)
        && url
            .path()
            .trim_matches('/')
            .trim_end_matches(".git")
            .eq_ignore_ascii_case(&report.repository)
}
pub fn wanted(path: &str) -> bool {
    if path.starts_with('/')
        || path.contains(['\\', '\0', '\n', '\r'])
        || path.split('/').any(|p| p == ".." || p == ".")
    {
        return false;
    }
    if path
        .split('/')
        .any(|p| ["node_modules", ".git", "target", "dist", ".venv", "vendor"].contains(&p))
    {
        return false;
    }
    let name = path.rsplit('/').next().unwrap_or_default();
    (projects::manifest(name) && name != "pyvenv.cfg") || crate::ci::provider(path).is_some()
}

pub fn select_files(tree: &Value) -> Result<Vec<File>, String> {
    if tree["truncated"] != false {
        return Err(
            "GitHubのファイル一覧が省略されています。完全な調査結果として登録できません".into(),
        );
    }
    let entries = tree["tree"]
        .as_array()
        .ok_or("GitHubのtree配列がありません")?;
    let mut files = vec![];
    for entry in entries {
        let path = entry["path"].as_str().ok_or("ファイル名がありません")?;
        if !wanted(path) {
            continue;
        }
        let regular = entry["type"] == "blob"
            && ["100644", "100755"].contains(&entry["mode"].as_str().unwrap_or_default());
        let size = entry["size"].as_u64().unwrap_or(0);
        files.push(File {
            path: path.into(),
            sha: sha(&entry["sha"])?,
            size,
            status: if !regular {
                "skipped-link"
            } else if size > 1_500_000 {
                "too-large"
            } else {
                "pending"
            }
            .into(),
        });
    }
    if files.len() > 200 {
        return Err("対象ファイルが200件を超えます。この版では全件調査できません".into());
    }
    if files.iter().map(|f| f.size).sum::<u64>() > 16_000_000 {
        return Err("対象ファイルが合計16MBを超えます".into());
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}
fn parse_file(project: &mut Project, path: &str, text: &str) -> Result<(), String> {
    if crate::ci::provider(path).is_some() {
        let file = crate::ci::parse(path, text, &mut project.requirements)?;
        project.ci.push(file);
        return Ok(());
    }
    projects::parse_manifest(
        path,
        text,
        &mut project.requirements,
        &mut project.scripts,
        &mut project.package_manager,
    )
}
pub fn scan(
    env: &Environment,
    settings: &Settings,
    repo: &str,
    reference: Option<&str>,
) -> Result<Report, String> {
    validate_repo(repo)?;
    let meta = json(api(
        env,
        settings,
        None,
        &format!("repos/{repo}"),
        &[],
        false,
    )?)?;
    let repository = meta["full_name"]
        .as_str()
        .ok_or("リポジトリ名がありません")?;
    if !repository.eq_ignore_ascii_case(repo) {
        return Err("取得したリポジトリ名が一致しません".into());
    }
    let branch = reference
        .or_else(|| meta["default_branch"].as_str())
        .filter(|s| !s.is_empty())
        .ok_or("既定ブランチがありません")?;
    let url = meta["html_url"]
        .as_str()
        .ok_or("リポジトリURLがありません")?;
    let parsed = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
    if parsed.scheme() != "https" || !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("GitHub URLの形式が不正です".into());
    }
    let hostname = parsed.host_str().ok_or("GitHubのホストがありません")?;
    let host = parsed
        .port()
        .map(|port| format!("{hostname}:{port}"))
        .unwrap_or_else(|| hostname.to_string());
    let head = commit(env, settings, &host, repository, branch)?;
    let commit = sha(&head["sha"])?;
    let tree = sha(&head["commit"]["tree"]["sha"])?;
    let listing = json(api(
        env,
        settings,
        Some(&host),
        &format!("repos/{repository}/git/trees/{tree}"),
        &[("recursive", "1")],
        false,
    )?)?;
    if sha(&listing["sha"])? != tree {
        return Err("取得したツリーのSHAが一致しません".into());
    }
    let mut files = select_files(&listing)?;
    let report_id = format!(
        "github:{}:{}:{}",
        host.to_lowercase(),
        repository.to_lowercase(),
        branch
    );
    let mut project = Project {
        id: report_id.clone(),
        environment: "github".into(),
        name: repository.into(),
        path: url.into(),
        remote: url.into(),
        branch: branch.into(),
        git_status: String::new(),
        package_manager: String::new(),
        requirements: vec![],
        venvs: vec![],
        scripts: vec![],
        findings: vec![],
        fingerprint: String::new(),
        scanned_at: now(),
        manifest_files: vec![],
        ci: vec![],
    };
    let mut fingerprint = Sha256::new();
    fingerprint.update(commit.as_bytes());
    for file in &mut files {
        if file.status != "pending" {
            project.findings.push(finding(
                &file.path,
                "partial",
                "リンク・サブモジュールまたはサイズ上限のため未取得です",
            ));
            continue;
        }
        let content = api(
            env,
            settings,
            Some(&host),
            &format!("repos/{repository}/git/blobs/{}", file.sha),
            &[],
            true,
        );
        match content {
            Ok(text) if text.len() <= 1_500_000 && !text.contains('\0') => {
                fingerprint.update(file.path.as_bytes());
                fingerprint.update(text.as_bytes());
                match parse_file(&mut project, &file.path, &text) {
                    Ok(()) => {
                        file.status = "ok".into();
                        project.manifest_files.push(file.path.clone());
                    }
                    Err(_) => {
                        file.status = "parse-error".into();
                        project.findings.push(finding(
                            &file.path,
                            "error",
                            "設定ファイルを解析できませんでした。形式・記法を確認してください。",
                        ));
                    }
                }
            }
            Ok(_) => {
                file.status = "invalid-content".into();
                project.findings.push(finding(
                    &file.path,
                    "partial",
                    "ファイルがサイズ上限を超えるか、テキストとして扱えません",
                ));
            }
            Err(e) => {
                file.status = "error".into();
                project.findings.push(finding(&file.path, "error", e));
            }
        }
    }
    crate::ci::finish(&mut project);
    projects::add_python_checks(&project.requirements, &mut project.scripts);
    project.scripts.sort();
    project.scripts.dedup();
    if project.package_manager.is_empty() {
        project.package_manager = if project.manifest_files.contains(&"uv.lock".into()) {
            "uv"
        } else if project.manifest_files.contains(&"package-lock.json".into()) {
            "npm"
        } else {
            "未指定"
        }
        .into();
    }
    let complete = files.iter().all(|f| f.status == "ok");
    if files.is_empty() {
        project.findings.push(finding("対象ファイル","partial","対応する設定・ロックファイルが見つかりませんでした。依存が存在しないという判定ではありません。"));
    }
    for file in files
        .iter()
        .filter(|f| f.path.ends_with("yarn.lock") || f.path.ends_with("requirements.txt"))
    {
        project.findings.push(finding(
            &file.path,
            "partial",
            "この形式の完全な依存解析・更新実行は未対応です",
        ));
    }
    project.findings.push(finding("実行検証","partial","GitHub上の同一コミットの静的調査です。依存解決・テスト・ビルド・実機接続は未実施です。Actionsのusesは指定の抽出のみで、更新候補の照合は未対応です。"));
    project.fingerprint = format!("{:x}", fingerprint.finalize());
    Ok(Report {
        id: report_id,
        repository: repository.into(),
        host,
        url: url.into(),
        branch: branch.into(),
        commit,
        tree,
        complete,
        files,
        project,
    })
}
pub fn proposal(report: &Report, updates: &[Update]) -> Result<Proposal, String> {
    if !report.complete {
        return Err(
            "未取得・解析失敗のファイルがあります。完全に取得してから計画してください".into(),
        );
    }
    if updates.is_empty()
        || updates
            .iter()
            .any(|u| u.fingerprint != report.project.fingerprint)
    {
        return Err("対象コミットの更新候補を先に照合してください".into());
    }
    let changes = updates
        .iter()
        .filter(|u| !u.candidate.is_empty() || !u.vulnerabilities.is_empty())
        .cloned()
        .collect();
    let tools = report
        .project
        .requirements
        .iter()
        .filter(|r| !["npm", "pypi", "cargo", "registry"].contains(&r.manager.as_str()))
        .cloned()
        .collect();
    let mut warnings=vec!["静的な更新方針です。実行コマンド・更新後のロック差分・動作互換性は確定していません。ローカルまたはCIの作業環境で検証してください。".into()];
    warnings.extend(
        updates
            .iter()
            .filter(|u| {
                u.importance == "unknown"
                    || u.audit_status.starts_with("未照合")
                    || u.audit_status.starts_with("照合失敗")
            })
            .map(|u| format!("{}: {} / {}", u.name, u.note, u.audit_status)),
    );
    warnings.extend(
        report
            .project
            .findings
            .iter()
            .filter(|f| f.status != "ok")
            .map(|f| format!("{}: {}", f.scope, f.message)),
    );
    warnings.extend(crate::ci::plan_warnings(&report.project));
    let mut seen = std::collections::BTreeSet::new();
    warnings.retain(|warning| seen.insert(warning.clone()));
    Ok(Proposal {
        id: id(),
        report_id: report.id.clone(),
        commit: report.commit.clone(),
        created_at: now(),
        changes,
        tools,
        checks: report.project.scripts.clone(),
        steps: vec![
            "対象コミットを再確認し、作業用ブランチと実行環境を用意する".into(),
            "更新対象の宣言と開発ツールの固定を見直し、管理元でロックファイルを再生成する".into(),
            "差分を確認し、宣言済みの検証とプロジェクト固有の接続試験を行う".into(),
            "検証結果を添えてコミット・PRを作成し、対象コミットのCI成功を確認する".into(),
        ],
        warnings,
        status: "draft".into(),
        workspace: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn files_are_nested_and_exclude_secrets_and_checkouts() {
        assert!(wanted("apps/web/package.json"));
        assert!(wanted(".github/workflows/ci.yml"));
        for path in [
            ".env",
            "README.md",
            "node_modules/a/package.json",
            "../package.json",
            "/package.json",
            ".venv/pyvenv.cfg",
            "packages/a/pyvenv.cfg",
        ] {
            assert!(!wanted(path), "{path}");
        }
    }
    #[test]
    fn incomplete_tree_is_not_a_complete_scan() {
        assert!(select_files(&serde_json::json!({"truncated":true,"tree":[]})).is_err());
        let rows=select_files(&serde_json::json!({"truncated":false,"tree":[{"path":"package.json","sha":"a".repeat(40),"size":10,"type":"blob","mode":"120000"}]})).unwrap();
        assert_eq!(rows[0].status, "skipped-link");
        assert!(validate_repo("owner/name --other").is_err());
        assert!(validate_repo("../name").is_err());
    }
    #[test]
    fn same_parser_preserves_monorepo_and_ignores_npm_credentials() {
        let mut requirements = vec![];
        let mut scripts = vec![];
        let mut pm = String::new();
        projects::parse_manifest(
            "apps/web/package.json",
            r#"{"dependencies":{"kleur":"4.1.4"},"scripts":{"test":"never execute"}}"#,
            &mut requirements,
            &mut scripts,
            &mut pm,
        )
        .unwrap();
        projects::parse_manifest(
            ".npmrc",
            "//example.invalid/:_authToken=secret\nregistry=https://registry.npmjs.org",
            &mut requirements,
            &mut scripts,
            &mut pm,
        )
        .unwrap();
        assert_eq!(scripts, vec!["apps/web::test"]);
        assert_eq!(requirements[0].source, "apps/web/package.json");
        assert!(
            !serde_json::to_string(&requirements)
                .unwrap()
                .contains("secret")
        );
    }
    #[test]
    fn oversized_settings_are_not_marked_as_fetched() {
        let rows=select_files(&serde_json::json!({"truncated":false,"tree":[{"path":"apps/web/package.json","sha":"b".repeat(40),"size":1_500_001,"type":"blob","mode":"100644"}]})).unwrap();
        assert_eq!(rows[0].status, "too-large");
        let bad = serde_json::json!({"truncated":false,"tree":[{"path":"package.json","sha":"../other","size":1,"type":"blob","mode":"100644"}]});
        assert!(select_files(&bad).is_err());
    }
}
