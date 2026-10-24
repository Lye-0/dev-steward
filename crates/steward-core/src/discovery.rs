//! Read-only adapters for common installations, independent of the developer's machine.
use crate::{
    inventory::{manager, within},
    models::*,
    process::*,
};
use regex::Regex;
use serde_json::Value;

pub const EXECUTABLES: &[&str] = &[
    "mise",
    "uv",
    "rustup",
    "cargo",
    "git",
    "gh",
    "winget",
    "dpkg-query",
    "brew",
    "py",
    "pymanager",
    "pyenv",
    "conda",
    "fnm",
    "nvm",
    "python",
    "python3",
    "node",
    "npm",
    "pnpm",
    "java",
    "go",
    "dotnet",
    "bun",
    "deno",
];

fn tool(env: &Environment, owner: &str, name: &str, version: &str, path: &str, key: &str) -> Tool {
    Tool {
        id: format!("{}:{owner}:{name}:{key}", env.id),
        environment: env.id.clone(),
        manager: owner.into(),
        name: name.into(),
        version: version.into(),
        path: path.into(),
        active: false,
        installed: true,
        source: String::new(),
        requested: key.into(),
        note: "検出結果です。候補照合と更新計画の対応状況は「更新」で確認できます。".into(),
    }
}

fn absolute(path: &str, windows: bool) -> bool {
    if windows {
        path.starts_with("\\\\")
            || (path.as_bytes().get(1) == Some(&b':')
                && matches!(path.as_bytes().get(2), Some(b'\\' | b'/')))
    } else {
        path.starts_with('/')
    }
}

pub fn parse_py(data: &str, env: &Environment) -> Result<Vec<Tool>, String> {
    // Both legacy '-3.11-64' and PEP 514 '-V:Company/Tag' preserve spaced paths.
    let pattern =
        Regex::new(r"^\s*(?P<tag>-(?:V:)?\S+)\s+(?P<default>\*\s+)?(?P<path>.+?\.exe)\s*$")
            .unwrap();
    let version_line = Regex::new(r"^\s*-\d").unwrap();
    let mut rows = vec![];
    for line in data.lines() {
        if let Some(c) = pattern.captures(line) {
            let path = c["path"].trim().trim_matches('"');
            if !absolute(path, true) {
                return Err("pyの一覧に絶対パスではない参照があります".into());
            }
            let mut row = tool(env, "python-launcher", "python", "", path, &c["tag"]);
            row.active = c.name("default").is_some();
            row.note = "pyの登録情報です。タグは完全なPythonバージョンとは限りません。旧ランチャーをインストール元と断定しません。更新・削除は未対応です。".into();
            rows.push(row);
        } else if line.trim_start().starts_with("-V:") || version_line.is_match(line) {
            return Err("pyの登録一覧に解析できない行があります".into());
        }
    }
    if rows.is_empty()
        && !data.trim().is_empty()
        && !data.to_lowercase().contains("no installed python")
    {
        return Err("pyの登録一覧を解析できませんでした".into());
    }
    Ok(rows)
}

pub fn parse_pymanager(data: &str, env: &Environment) -> Result<Vec<Tool>, String> {
    let v: Value =
        serde_json::from_str(data).map_err(|e| format!("Python Install Manager JSON: {e}"))?;
    let versions = v["versions"]
        .as_array()
        .ok_or("Python Install Managerのversions配列がありません")?;
    versions
        .iter()
        .map(|v| {
            let path = v["executable"]
                .as_str()
                .ok_or("Python実行ファイルの参照がありません")?;
            if !absolute(path, true) {
                return Err("Python実行ファイルの参照が絶対パスではありません".into());
            }
            let owner = if v["unmanaged"] == true
                || v["id"].as_str().is_some_and(|s| s.starts_with("__"))
            {
                "python-launcher"
            } else {
                "python-install-manager"
            };
            let mut row = tool(
                env,
                owner,
                "python",
                v["sort-version"].as_str().unwrap_or_default(),
                path,
                v["tag"].as_str().unwrap_or(path),
            );
            row.active = v["default"] == true;
            row.source = v["prefix"].as_str().unwrap_or_default().into();
            Ok(row)
        })
        .collect()
}

pub fn parse_node_versions(
    data: &str,
    env: &Environment,
    owner: &str,
) -> Result<Vec<Tool>, String> {
    let pattern = Regex::new(r"^\s*(\*\s*)?v?(\d+\.\d+\.\d+)(?:\s.*)?$").unwrap();
    let mut rows = vec![];
    for line in data.lines() {
        if let Some(c) = pattern.captures(line) {
            let mut row = tool(env, owner, "node", &c[2], "", &c[2]);
            row.active = c.get(1).is_some() && (owner != "fnm" || line.contains("default"));
            row.note
                .push_str(" 一覧から配置先を得られない場合、配置は未取得と表示します。");
            rows.push(row);
        } else if line.trim_start().starts_with("* v")
            || line.trim_start().starts_with("v") && line.contains('.')
        {
            return Err(format!("{owner}のバージョン行を解析できませんでした"));
        }
    }
    if rows.is_empty()
        && !data.trim().is_empty()
        && !data.to_lowercase().contains("no installations")
        && !data.contains("system")
    {
        return Err(format!("{owner}のバージョン一覧を解析できませんでした"));
    }
    Ok(rows)
}

pub fn parse_conda_packages(
    data: &str,
    prefix: &str,
    env: &Environment,
) -> Result<Vec<Tool>, String> {
    let packages: Vec<Value> =
        serde_json::from_str(data).map_err(|e| format!("Conda JSON: {e}"))?;
    let mut result = vec![];
    for p in packages {
        let name = p["name"]
            .as_str()
            .ok_or("Condaのパッケージ名がありません")?;
        let (name, executable) = match name {
            "python" => (
                "python",
                if env.os == "windows" {
                    "python.exe"
                } else {
                    "bin/python"
                },
            ),
            "nodejs" => (
                "node",
                if env.os == "windows" {
                    "node.exe"
                } else {
                    "bin/node"
                },
            ),
            "go" => (
                "go",
                if env.os == "windows" {
                    "bin/go.exe"
                } else {
                    "bin/go"
                },
            ),
            _ => continue,
        };
        let version = p["version"]
            .as_str()
            .ok_or("Condaのバージョンがありません")?;
        let mut row = tool(
            env,
            "conda",
            name,
            version,
            &join(prefix, executable, env),
            prefix,
        );
        row.source = prefix.into();
        row.note.push_str(
            " Conda環境のパッケージメタデータで確認。チャンネルの認証情報は保存しません。",
        );
        result.push(row);
    }
    Ok(result)
}

fn canonical(env: &Environment, path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    if env.kind == "wsl" {
        checked(env, "/usr/bin/readlink", &["-f", "--", path], &env.home)
            .unwrap_or_else(|_| path.into())
    } else {
        std::fs::canonicalize(path)
            .ok()
            .map(|p| p.to_string_lossy().into())
            .unwrap_or_else(|| path.into())
    }
}

/// Attach discovery evidence to an existing managed body rather than inventing a second owner.
pub fn merge_tool(report: &mut Inventory, row: Tool, windows: bool) {
    if !row.path.is_empty()
        && let Some(old) = report.tools.iter_mut().find(|t| {
            t.name == row.name
                && !t.path.is_empty()
                && t.manager != "winget"
                && (within(&row.path, &t.path, windows) || within(&t.path, &row.path, windows))
        })
    {
        let evidence = format!("\n{}でも検出: {}", row.manager, row.path);
        let old_strong = !["external", "python-launcher", "system"].contains(&old.manager.as_str());
        let new_strong = !["external", "python-launcher", "system"].contains(&row.manager.as_str());
        if !old_strong && new_strong {
            let previous = format!("\n{}でも検出: {}", old.manager, old.path);
            let mut replacement = row;
            replacement.note.push_str(&previous);
            *old = replacement;
        } else {
            old.note.push_str(&evidence);
        }
        return;
    }
    report.tools.push(row);
}

fn collect(env: &Environment, program: &str, name: &str) -> Result<Vec<Tool>, String> {
    if env.os == "windows"
        && ["bat", "cmd"]
            .iter()
            .any(|ext| program.to_lowercase().ends_with(&format!(".{ext}")))
    {
        return Err("バッチファイルやシェル関数経由の管理元は未対応です。CondaはCONDA_EXEなど実行ファイル本体を指定してください。".into());
    }
    match name {
        "py" => {
            // --list-paths is shared by old/new launchers; never launch 'py list' on the legacy executable.
            let out = run(env, program, &strings(&["--list-paths"]), &env.home, 15)?;
            if out.code != 0 {
                return Err(format!("pyの一覧取得に失敗: {}", out.stderr));
            }
            parse_py(&format!("{}\n{}", out.stdout, out.stderr), env)
        }
        "pymanager" => parse_pymanager(
            &checked(env, program, &["list", "--format=json"], &env.home)?,
            env,
        ),
        "pyenv" => {
            let versions = checked(env, program, &["versions", "--bare"], &env.home)?;
            if versions.lines().count() > 100 {
                return Err("pyenvの版が100件を超えます。全件取得を完了していません".into());
            }
            let mut rows = vec![];
            for version in versions
                .lines()
                .filter(|s| !s.is_empty() && *s != "system")
                .take(100)
            {
                let path = checked(env, program, &["prefix", version], &env.home)?;
                if !absolute(&path, env.os == "windows") {
                    return Err("pyenv prefixが絶対パスではありません".into());
                }
                let mut row = tool(env, "pyenv", "python", version, &path, version);
                row.source = path;
                rows.push(row);
            }
            Ok(rows)
        }
        "conda" => {
            let data = checked(env, program, &["env", "list", "--json"], &env.home)?;
            let value: Value = serde_json::from_str(&data).map_err(|e| e.to_string())?;
            let envs = value["envs"]
                .as_array()
                .ok_or("Condaのenvs配列がありません")?;
            if envs.len() > 32 {
                return Err("Conda環境が32件を超えます。全件取得を完了していません".into());
            }
            let mut rows = vec![];
            for prefix in envs {
                let prefix = prefix.as_str().ok_or("Conda環境のパスが不正です")?;
                if !absolute(prefix, env.os == "windows") {
                    return Err("Conda環境が絶対パスではありません".into());
                }
                let data = checked(
                    env,
                    program,
                    &["list", "--prefix", prefix, "--json"],
                    &env.home,
                )?;
                rows.extend(parse_conda_packages(&data, prefix, env)?);
            }
            Ok(rows)
        }
        "fnm" | "nvm" => {
            let mut rows =
                parse_node_versions(&checked(env, program, &["list"], &env.home)?, env, name)?;
            if rows.len() > 100 {
                return Err("Nodeの版が100件を超えます。全件取得を完了していません".into());
            }
            for row in &mut rows {
                if name == "fnm" {
                    match checked(
                        env,
                        program,
                        &[
                            "exec",
                            &format!("--using={}", row.version),
                            "node",
                            "-p",
                            "process.execPath",
                        ],
                        &env.home,
                    ) {
                        Ok(path) if absolute(&path, env.os == "windows") => {
                            row.path = path;
                        }
                        _ => row
                            .note
                            .push_str(" 指定版のNode実行先を確認できませんでした。"),
                    }
                } else if let Ok(root) = checked(env, program, &["root"], &env.home) {
                    // nvm-windows reports its configurable root; never assume an install drive.
                    if let Some(root) = root
                        .lines()
                        .filter_map(|line| line.trim().strip_prefix("Current Root:"))
                        .map(str::trim)
                        .find(|s| absolute(s, true))
                    {
                        let candidate = join(root, &format!("v{}/node.exe", row.version), env);
                        if std::path::Path::new(&candidate).is_file() {
                            row.path = candidate;
                        }
                    }
                }
            }
            Ok(rows)
        }
        _ => Err("未対応の検出アダプターです".into()),
    }
}

pub fn scan(env: &Environment, settings: &Settings, report: &mut Inventory) {
    for name in ["pymanager", "py", "pyenv", "conda", "fnm", "nvm"] {
        if (name == "py" || name == "pymanager" || name == "nvm") && env.os != "windows" {
            continue;
        }
        let Some(program) = manager(env, settings, name) else {
            report.findings.push(finding(name, "missing", "コマンド未検出。未導入・PATH外・シェル関数の場合があります。実行ファイルを設定できます。"));
            continue;
        };
        report.manager_paths.push((name.into(), program.clone()));
        match collect(env, &program, name) {
            Ok(rows) => {
                let count = rows.len();
                for mut row in rows {
                    row.path = canonical(env, &row.path);
                    merge_tool(report, row, env.os == "windows");
                }
                report.findings.push(finding(
                    name,
                    "ok",
                    format!("{count}件のランタイム参照を取得。更新・削除の自動実行は未対応です。"),
                ));
            }
            Err(e) => report.findings.push(finding(name, "error", e)),
        }
    }
    scan_path(env, settings, report);
}

fn scan_path(env: &Environment, settings: &Settings, report: &mut Inventory) {
    let version_pattern = Regex::new(r"\d+\.\d+(?:\.\d+)?(?:[a-zA-Z0-9.+_-]*)?").unwrap();
    for name in [
        "python", "python3", "node", "java", "go", "dotnet", "bun", "deno",
    ] {
        let Some(program) = manager(env, settings, name) else {
            continue;
        };
        // Windows Store execution aliases can launch an installer. Never probe them as runtimes.
        if env.os == "windows"
            && program
                .replace('\\', "/")
                .to_lowercase()
                .contains("/microsoft/windowsapps/")
        {
            report.findings.push(finding(name, "partial", "Windowsアプリ実行エイリアスを検出。インストーラー起動を避けるため実行していません。py／pymanagerの登録情報を確認してください。"));
            continue;
        }
        let args = match name {
            "python" | "python3" => vec![
                "-I",
                "-S",
                "-c",
                "import sys,json;print(json.dumps({'version':sys.version.split()[0],'path':sys.executable}))",
            ],
            "node" => vec![
                "-p",
                "JSON.stringify({version:process.versions.node,path:process.execPath})",
            ],
            "java" => vec!["-version"],
            "go" => vec!["version"],
            _ => vec!["--version"],
        };
        match run(env, &program, &strings(&args), &env.home, 12) {
            Ok(out) if out.code == 0 => {
                let output = format!("{}\n{}", out.stdout, out.stderr);
                let info = serde_json::from_str::<Value>(out.stdout.trim()).ok();
                let version = info
                    .as_ref()
                    .and_then(|v| v["version"].as_str())
                    .map(str::to_string)
                    .or_else(|| {
                        version_pattern
                            .find(&output)
                            .map(|m| m.as_str().to_string())
                    });
                if let Some(version) = version {
                    let path = canonical(
                        env,
                        info.as_ref()
                            .and_then(|v| v["path"].as_str())
                            .filter(|s| absolute(s, env.os == "windows"))
                            .unwrap_or(&program),
                    );
                    let owner = if env.os != "windows"
                        && (path.starts_with("/usr/bin/") || path.starts_with("/bin/"))
                    {
                        "system"
                    } else {
                        "external"
                    };
                    let mut row = tool(
                        env,
                        owner,
                        if name == "python3" { "python" } else { name },
                        &version,
                        &path,
                        &path,
                    );
                    row.note = format!(
                        "PATHまたは明示設定の実行ファイル: {program}\n版を問い合わせて確認。管理元は別途確認が必要です。既存の管理版と参照先が一致した場合は集約します。"
                    );
                    merge_tool(report, row, env.os == "windows");
                } else {
                    report.findings.push(finding(
                        name,
                        "error",
                        "版の問い合わせ結果を解析できませんでした",
                    ));
                }
            }
            Ok(out) => report.findings.push(finding(
                name,
                "error",
                format!(
                    "実行ファイルを検出しましたが版の問い合わせに失敗 (終了コード{})",
                    out.code
                ),
            )),
            Err(e) => report.findings.push(finding(name, "error", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn env() -> Environment {
        Environment {
            id: "other".into(),
            name: "test".into(),
            kind: "local".into(),
            home: "Q:\\Other User".into(),
            os: "windows".into(),
        }
    }
    #[test]
    fn legacy_launcher_preserves_spaced_paths_and_does_not_invent_patch_versions() {
        let rows = parse_py(" -3.11-64 * Q:\\Other User\\Python311\\python.exe\n -V:Other/PyPy Q:\\Portable Python\\pypy.exe", &env()).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows[0].active);
        assert_eq!(rows[0].path, "Q:\\Other User\\Python311\\python.exe");
        assert_eq!(rows[0].manager, "python-launcher");
        assert!(rows[0].version.is_empty());
        assert!(parse_py(" -V:3.12 relative/python.exe", &env()).is_err());
    }
    #[test]
    fn install_manager_distinguishes_unmanaged_registrations() {
        let rows = parse_pymanager(r#"{"versions":[{"tag":"3.14","sort-version":"3.14.2","executable":"R:\\Portable\\python.exe","default":true},{"id":"__unmanaged-311","unmanaged":true,"tag":"3.11","executable":"S:\\Old Python\\python.exe"}]}"#, &env()).unwrap();
        assert_eq!(rows[0].manager, "python-install-manager");
        assert_eq!(rows[1].manager, "python-launcher");
        assert!(parse_pymanager("[]", &env()).is_err());
    }
    #[test]
    fn conda_uses_environment_identity_not_version_as_key() {
        let data = r#"[{"name":"python","version":"3.12.8","channel":"https://secret@private.example"},{"name":"openssl","version":"3.0.0"}]"#;
        let a = parse_conda_packages(data, "Q:\\Env A", &env()).unwrap();
        let b = parse_conda_packages(data, "Q:\\Env B", &env()).unwrap();
        assert_eq!(a.len(), 1);
        assert_ne!(a[0].id, b[0].id);
        assert!(!a[0].note.contains("secret"));
    }
    #[test]
    fn node_managers_keep_installed_versions_not_aliases() {
        let rows =
            parse_node_versions("* v22.2.0 default\n* v20.4.0\n* system", &env(), "fnm").unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows[0].active);
        assert!(!rows[1].active);
        assert!(parse_node_versions("Unexpected output", &env(), "fnm").is_err());
    }
    #[test]
    fn launcher_discovery_does_not_override_uv_owner() {
        let mut report = Inventory::default();
        report.tools.push(tool(
            &env(),
            "uv",
            "python",
            "3.12.8",
            "Q:\\Managed\\Python\\python.exe",
            "3.12.8",
        ));
        merge_tool(
            &mut report,
            tool(
                &env(),
                "python-launcher",
                "python",
                "",
                "q:/managed/python/python.exe",
                "3.12",
            ),
            true,
        );
        assert_eq!(report.tools.len(), 1);
        assert_eq!(report.tools[0].manager, "uv");
        assert!(report.tools[0].note.contains("python-launcher"));
        merge_tool(
            &mut report,
            tool(
                &env(),
                "python-launcher",
                "python",
                "",
                "Q:\\Managed\\Python-other\\python.exe",
                "3.11",
            ),
            true,
        );
        assert_eq!(report.tools.len(), 2);
    }
}
