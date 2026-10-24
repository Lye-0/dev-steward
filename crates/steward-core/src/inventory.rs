use crate::{models::*, process::*};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

pub fn environments() -> Vec<Environment> {
    #[allow(unused_mut)]
    let mut result = vec![Environment {
        id: "local".into(),
        name: "このデバイス".into(),
        kind: "local".into(),
        home: local_home(),
        os: std::env::consts::OS.into(),
    }];
    #[cfg(windows)]
    if let Ok(out) = run_raw("wsl.exe", &strings(&["--list", "--quiet"]), None, 15) {
        // WSL outputs UTF-16 on some Windows versions. Null bytes are removed by run_raw.
        for name in out
            .stdout
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.starts_with("docker-desktop"))
        {
            let args = strings(&[
                "--distribution",
                name,
                "--exec",
                "/bin/sh",
                "-c",
                "printf '%s' \"$HOME\"",
            ]);
            if let Ok(home) = run_raw("wsl.exe", &args, None, 25)
                && home.code == 0
                && home.stdout.starts_with('/')
            {
                result.push(Environment {
                    id: format!("wsl:{name}"),
                    name: name.into(),
                    kind: "wsl".into(),
                    home: home.stdout.trim().into(),
                    os: "linux".into(),
                });
            }
        }
    }
    result
}

pub fn manager(env: &Environment, settings: &Settings, name: &str) -> Option<String> {
    if let Some((_, _, path)) = settings
        .manager_overrides
        .iter()
        .find(|(e, n, _)| e == &env.id && n == name)
    {
        return Some(path.clone());
    }
    if env.kind == "local" {
        let variable = match name {
            "conda" => Some("CONDA_EXE"),
            "pyenv" => Some("PYENV_EXECUTABLE"),
            _ => None,
        };
        if let Some(path) = variable
            .and_then(std::env::var_os)
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_absolute() && p.is_file())
        {
            return Some(path.to_string_lossy().into());
        }
        if name == "py" && env.os == "windows" {
            let candidates = [
                std::env::var_os("WINDIR").map(|p| std::path::PathBuf::from(p).join("py.exe")),
                dirs::data_local_dir().map(|p| p.join("Programs/Python/Launcher/py.exe")),
            ];
            // PATH takes precedence; these are discoverable standard-location hints only.
            if let Some(path) = resolve(env, name).or_else(|| {
                candidates
                    .into_iter()
                    .flatten()
                    .find(|p| p.is_file())
                    .map(|p| p.to_string_lossy().into())
            }) {
                return Some(path);
            }
        }
    }
    resolve(env, name).or_else(|| {
        if env.kind != "local" {
            return None;
        }
        // Official rustup/uv executable directories are discovery hints, never storage destinations.
        let candidates = [
            Path::new(&env.home).join(".cargo/bin"),
            Path::new(&env.home).join(".local/bin"),
        ];
        candidates
            .into_iter()
            .map(|d| {
                d.join(if cfg!(windows) {
                    format!("{name}.exe")
                } else {
                    name.to_string()
                })
            })
            .find(|p| p.is_file())
            .map(|p| p.to_string_lossy().into())
    })
}

pub fn parse_mise(data: &str, env: &Environment) -> Result<Vec<Tool>, String> {
    let value: Value = serde_json::from_str(data).map_err(|e| format!("mise JSON: {e}"))?;
    let object = value.as_object().ok_or("mise JSONの形式に未対応")?;
    let mut tools = vec![];
    for (name, versions) in object {
        for version in versions.as_array().ok_or("mise version配列がありません")? {
            let ver = version["version"]
                .as_str()
                .ok_or("mise versionがありません")?
                .to_string();
            tools.push(Tool {
                id: format!("{}:mise:{name}:{ver}", env.id),
                environment: env.id.clone(),
                manager: "mise".into(),
                name: name.clone(),
                version: ver,
                path: version["install_path"].as_str().unwrap_or_default().into(),
                active: version["active"].as_bool().unwrap_or(false),
                installed: version["installed"].as_bool().unwrap_or(false),
                source: version["source"]["path"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                requested: version["requested_version"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                note: String::new(),
            });
        }
    }
    Ok(tools)
}

pub fn parse_uv(data: &str, env: &Environment) -> Result<Vec<Tool>, String> {
    parse_uv_with_root(data, env, None)
}
fn parse_uv_with_root(
    data: &str,
    env: &Environment,
    root: Option<&str>,
) -> Result<Vec<Tool>, String> {
    let value: Vec<Value> = serde_json::from_str(data).map_err(|e| format!("uv JSON: {e}"))?;
    let mut grouped: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for item in value {
        let key = item["key"]
            .as_str()
            .ok_or("uv keyがありません")?
            .to_string();
        grouped.entry(key).or_default().push(item);
    }
    Ok(grouped.into_iter().map(|(key, items)| {
        let item = items.iter().find(|i| root.is_some_and(|root| within(&inventory_path(env,i["path"].as_str().unwrap_or_default()),root,env.os=="windows")))
            .or_else(|| items.iter().find(|i| i["path"].as_str().unwrap_or_default().contains(&key))).unwrap_or(&items[0]);
        let path = inventory_path(env, item["path"].as_str().unwrap_or_default());
        let canonical = if env.kind == "local" { std::fs::canonicalize(&path).ok().map(|p| p.to_string_lossy().to_string()) } else { None };
        Tool { id: format!("{}:uv:{key}", env.id), environment: env.id.clone(), manager: "uv".into(), name: "python".into(), version: item["version"].as_str().unwrap_or_default().into(), path: canonical.unwrap_or(path), active: false, installed: true, source: String::new(), requested: key, note: format!("{}件のパスを同一配布キーで集約。ランチャーと実体の参照先は詳細で確認してください。\n{}", items.len(), items.iter().filter_map(|i| i["path"].as_str()).collect::<Vec<_>>().join("\n")) }
    }).collect())
}

fn inventory_path(env: &Environment, path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    if (env.os == "windows" && !Path::new(path).is_absolute())
        || (env.os != "windows" && !path.starts_with('/'))
    {
        join(&env.home, path, env)
    } else {
        path.into()
    }
}

pub fn scan(env: &Environment, settings: &Settings) -> Inventory {
    let mut report = Inventory {
        environment: env.id.clone(),
        captured_at: now(),
        ..Default::default()
    };
    for name in [
        "mise",
        "uv",
        "rustup",
        "git",
        "gh",
        "winget",
        "dpkg-query",
        "brew",
    ] {
        let Some(program) = manager(env, settings, name) else {
            if ["mise", "uv", "rustup"].contains(&name) {
                report.findings.push(finding(name, "missing", "コマンドを検出できません。未導入またはPATH外です。設定で実行ファイルを指定できます。"));
            }
            continue;
        };
        report.manager_paths.push((name.into(), program.clone()));
        if name == "winget" && env.os == "windows" {
            match crate::windows_apps::scan(env, &program) {
                Ok(tools) => {
                    report.tools.extend(tools);
                    report.findings.push(finding(
                        "WinGet",
                        "ok",
                        "照合・エクスポートできたインストール済みアプリを取得しました。",
                    ));
                }
                Err(e) => report.findings.push(finding("WinGet", "error", e)),
            }
        }
        let args = match name {
            "mise" => vec!["ls", "--json"],
            "uv" => vec![
                "python",
                "list",
                "--only-installed",
                "--output-format",
                "json",
            ],
            "rustup" => vec!["toolchain", "list"],
            _ => continue,
        };
        match checked(env, &program, &args, &env.home) {
            Ok(output) => {
                let parsed = match name {
                    "mise" => parse_mise(&output, env), "uv" => {
                        let root = checked(env,&program,&["python","dir"],&env.home).ok();
                        parse_uv_with_root(&output,env,root.as_deref())
                    },
                    _ => Ok(output.lines().filter(|s| !s.is_empty()).map(|line| {
                        let name = line.split_whitespace().next().unwrap_or_default();
                        let path=checked(env,&program,&["which","--toolchain",name,"rustc"],&env.home).unwrap_or_default();
                        let version=checked(env,&program,&["run",name,"rustc","--version"],&env.home).ok().and_then(|s|s.split_whitespace().nth(1).map(str::to_string)).unwrap_or_else(||name.into());
                        Tool { id: format!("{}:rustup:{name}", env.id), environment: env.id.clone(), manager: "rustup".into(), name: "rust".into(), version, path, active: line.contains("active") || line.contains("default"), installed: true, source: String::new(), requested: name.into(), note: "rustupで指定したツールチェーンのrustcを実行して版を確認。プロジェクトのコードは実行していません。".into() }
                    }).collect()),
                };
                match parsed {
                    Ok(tools) => {
                        report.tools.extend(tools);
                        report
                            .findings
                            .push(finding(name, "ok", "管理元の一覧を取得しました。"));
                    }
                    Err(e) => report.findings.push(finding(name, "error", e)),
                }
            }
            Err(e) => report.findings.push(finding(name, "error", e)),
        }
    }
    if let Some((_, uv)) = report.manager_paths.iter().find(|(n, _)| n == "uv") {
        match checked(env, uv, &["python", "dir"], &env.home) {
            Ok(dir) => {
                for tool in report.tools.iter_mut().filter(|t| t.manager == "uv") {
                    let absolute = inventory_path(env, &tool.path);
                    tool.path = if env.kind == "wsl" {
                        checked(
                            env,
                            "/usr/bin/readlink",
                            &["-f", "--", &absolute],
                            &env.home,
                        )
                        .unwrap_or(absolute)
                    } else {
                        std::fs::canonicalize(&absolute)
                            .ok()
                            .map(|p| p.to_string_lossy().into())
                            .unwrap_or(absolute)
                    };
                    if !within(&tool.path, &dir, env.os == "windows") {
                        tool.manager = if env.os == "linux" && tool.path.starts_with("/usr/") {
                            "system"
                        } else {
                            "external"
                        }
                        .into();
                        tool.note = format!(
                            "uvが検出しましたがuvの管理先外です。OS・別の配布元の管理を維持します。\n{}",
                            tool.note
                        );
                    }
                }
                report.findings.push(finding("uv配置", "ok", dir));
            }
            Err(e) => {
                for tool in report.tools.iter_mut().filter(|t| t.manager == "uv") {
                    tool.manager = "external".into();
                    tool.note
                        .push_str("\nuv管理先を確認できないため、uv管理とは確定していません。");
                }
                report.findings.push(finding("uv配置", "error", e));
            }
        }
    }
    crate::discovery::scan(env, settings, &mut report);
    if env.os == "linux" {
        report.findings.push(finding(
            "OS",
            "protected",
            "OS管理のPython・システムツールは削除しません。APTなどOS管理元を維持します。",
        ));
    }
    report.findings.push(finding("調査範囲", "partial", "mise・uv・rustupに加え、py／Python Install Manager・pyenv・Conda・fnm・nvm-windowsとPATH上の主要ランタイムを調査。シェル関数のみの管理元、全インストーラー、同梱ランタイムを網羅するものではありません。未参照表示だけでは削除しません。"));
    report
}

pub fn within(path: &str, root: &str, windows: bool) -> bool {
    let clean = |s: &str| {
        s.trim_start_matches("\\\\?\\")
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string()
    };
    let (mut path, mut root) = (clean(path), clean(root));
    if windows {
        path = path.to_lowercase();
        root = root.to_lowercase();
    }
    path == root || path.starts_with(&(root + "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn env() -> Environment {
        Environment {
            id: "different-user".into(),
            name: "test".into(),
            kind: "local".into(),
            home: "/tmp/person".into(),
            os: "linux".into(),
        }
    }
    #[test]
    fn mise_missing_is_not_installed() {
        let rows = parse_mise(r#"{"java":[{"version":"21","installed":false,"source":{"path":"/mounted/config.toml"}}]}"#, &env()).unwrap();
        assert!(!rows[0].installed);
        assert_eq!(rows[0].source, "/mounted/config.toml");
    }
    #[test]
    fn python_launchers_are_grouped() {
        let rows=parse_uv(r#"[{"key":"cpython-3.13.1","version":"3.13.1","path":"/x/python"},{"key":"cpython-3.13.1","version":"3.13.1","path":"/y/python"}]"#,&env()).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].note.contains("2件"));
    }
    #[test]
    fn uv_minor_alias_in_managed_root_beats_launcher_even_without_full_distribution_key() {
        let mut target = env();
        target.kind = "wsl".into();
        let rows = parse_uv_with_root(r#"[{"key":"cpython-3.13.17-linux-x86_64-gnu","version":"3.13.17","path":".local/bin/python3.13"},{"key":"cpython-3.13.17-linux-x86_64-gnu","version":"3.13.17","path":"custom/uv/python/cpython-3.13-linux-x86_64-gnu/bin/python"}]"#,&target,Some("/tmp/person/custom/uv/python")).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].path,
            "/tmp/person/custom/uv/python/cpython-3.13-linux-x86_64-gnu/bin/python"
        );
    }
    #[test]
    fn uv_relative_paths_use_the_target_home_not_the_app_working_directory() {
        let mut target = env();
        target.kind = "wsl".into();
        let rows = parse_uv(r#"[{"key":"cpython-3.13.1","version":"3.13.1","path":".local/share/uv/python/cpython-3.13.1/bin/python"}]"#, &target).unwrap();
        assert_eq!(
            rows[0].path,
            "/tmp/person/.local/share/uv/python/cpython-3.13.1/bin/python"
        );
        assert!(within(
            &rows[0].path,
            "/tmp/person/.local/share/uv/python",
            false
        ));
    }
    #[cfg(windows)]
    #[test]
    fn windows_uv_relative_paths_are_resolved_from_the_environment_home() {
        let mut target = env();
        target.os = "windows".into();
        target.home = "Q:\\Different User".into();
        let rows = parse_uv(r#"[{"key":"cpython-3.13.1","version":"3.13.1","path":"AppData\\Roaming\\uv\\python\\cpython-3.13.1\\python.exe"}]"#, &target).unwrap();
        assert!(within(
            &rows[0].path,
            "Q:\\Different User\\AppData\\Roaming\\uv\\python",
            true
        ));
    }
    #[test]
    fn unsupported_json_is_error() {
        assert!(parse_mise("[]", &env()).is_err());
        assert!(parse_uv("{}", &env()).is_err());
    }
    #[test]
    fn system_python_is_not_uv_managed() {
        assert!(!within(
            "/usr/bin/python3.14",
            "/home/person/.local/share/uv/python",
            false
        ));
        assert!(within(
            "/custom/uv/python/version/bin/python",
            "/custom/uv/python",
            false
        ));
        assert!(!within(
            "/custom/uv/python-other/bin/python",
            "/custom/uv/python",
            false
        ));
    }
}
