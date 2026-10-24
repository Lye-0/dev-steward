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
        let item = items.iter().find(|i| i["path"].as_str().unwrap_or_default().contains(&key)).unwrap_or(&items[0]);
        let path = item["path"].as_str().unwrap_or_default();
        let canonical = if env.kind == "local" { std::fs::canonicalize(path).ok().map(|p| p.to_string_lossy().to_string()) } else { None };
        Tool { id: format!("{}:uv:{key}", env.id), environment: env.id.clone(), manager: "uv".into(), name: "python".into(), version: item["version"].as_str().unwrap_or_default().into(), path: canonical.unwrap_or_else(|| path.into()), active: false, installed: true, source: String::new(), requested: key, note: format!("{}件のパスを同一配布キーで集約。ランチャーと実体の参照先は詳細で確認してください。\n{}", items.len(), items.iter().filter_map(|i| i["path"].as_str()).collect::<Vec<_>>().join("\n")) }
    }).collect())
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
                    "mise" => parse_mise(&output, env), "uv" => parse_uv(&output, env),
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
                    let absolute = if env.kind == "wsl" && !tool.path.starts_with('/') {
                        join(&env.home, &tool.path, env)
                    } else {
                        tool.path.clone()
                    };
                    tool.path = if env.kind == "wsl" {
                        checked(
                            env,
                            "/usr/bin/readlink",
                            &["-f", "--", &absolute],
                            &env.home,
                        )
                        .unwrap_or(absolute)
                    } else {
                        absolute
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
            Err(e) => report.findings.push(finding("uv配置", "error", e)),
        }
    }
    if env.os == "linux" {
        report.findings.push(finding(
            "OS",
            "protected",
            "OS管理のPython・システムツールは削除しません。APTなどOS管理元を維持します。",
        ));
    }
    report.findings.push(finding("調査範囲", "partial", "mise・uv・rustupの管理版を調査。アプリ同梱ランタイム、全インストーラー、全実行中プロセスを網羅するものではありません。未参照表示だけでは削除しません。"));
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
