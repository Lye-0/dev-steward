use crate::{inventory, models::*, process::*};
use serde_json::{Value, json};

pub fn mcp(path: &str) -> Result<Vec<Value>, String> {
    let env = Environment {
        id: "local".into(),
        name: String::new(),
        kind: "local".into(),
        home: local_home(),
        os: std::env::consts::OS.into(),
    };
    let text = read(&env, path)?;
    let value: Value = if path.to_lowercase().ends_with(".toml") {
        let t: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
        serde_json::to_value(t).map_err(|e| e.to_string())?
    } else {
        serde_json::from_str(&text).map_err(|e| e.to_string())?
    };
    let servers = value
        .get("mcp_servers")
        .or_else(|| value.get("mcpServers"))
        .and_then(Value::as_object)
        .ok_or("対応するmcp_servers / mcpServersがありません")?;
    let mut result = vec![];
    for (name, server) in servers {
        let Some(command) = server["command"].as_str() else {
            continue;
        };
        let mut entry = json!({"config":path,"name":name,"command":command});
        let exe = std::path::Path::new(command);
        // Never execute a configured MCP command or persist its args/env.
        if let Some(parent) = exe.parent().and_then(|p| p.parent()) {
            let cfg = parent.join("pyvenv.cfg");
            if cfg.is_file() {
                let text = std::fs::read_to_string(&cfg).map_err(|e| e.to_string())?;
                entry["venv"] = json!(parent);
                entry["home"] = json!(
                    text.lines()
                        .filter_map(|l| l.split_once('='))
                        .find(|(k, _)| k.trim() == "home")
                        .map(|(_, v)| v.trim())
                        .unwrap_or("")
                );
            }
        }
        result.push(entry);
    }
    Ok(result)
}
pub fn system_updates(env: &Environment, settings: &Settings) -> Result<Value, String> {
    let (program, args) = if env.os == "windows" {
        (
            inventory::manager(env, settings, "winget").ok_or("WinGetが未検出です")?,
            strings(&["upgrade", "--disable-interactivity"]),
        )
    } else if env.os == "linux" {
        ("/usr/bin/apt".into(), strings(&["list", "--upgradable"]))
    } else {
        (
            inventory::manager(env, settings, "brew").ok_or("Homebrewが未検出です")?,
            strings(&["outdated", "--verbose"]),
        )
    };
    let out = run(env, &program, &args, &env.home, 90)?;
    if out.code != 0 {
        return Err(format!(
            "更新一覧の取得に失敗 ({}): {}\n{}",
            out.code, out.stdout, out.stderr
        ));
    }
    Ok(
        json!({"output":format!("{}\n{}",out.stdout,out.stderr),"program":program,"args":args,"checked_at":now()}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mcp_does_not_save_secret_arguments() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("config.json");
        std::fs::write(&p,r#"{"mcpServers":{"sample":{"command":"node","args":["secret-value"],"env":{"TOKEN":"private"}}}}"#).unwrap();
        let data = mcp(p.to_str().unwrap()).unwrap();
        let s = serde_json::to_string(&data).unwrap();
        assert!(!s.contains("secret-value"));
        assert!(!s.contains("private"));
        assert_eq!(data[0]["command"], "node");
    }
}
