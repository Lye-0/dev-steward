use crate::{models::*, process::*};
use serde_json::Value;

pub fn parse(text: &str, env: &Environment) -> Result<Vec<Tool>, String> {
    let value: Value =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    let sources = value["Sources"]
        .as_array()
        .ok_or("WinGet exportの形式に未対応です")?;
    let mut tools = vec![];
    for source in sources {
        if let Some(packages) = source["Packages"].as_array() {
            for p in packages {
                let name = p["PackageIdentifier"]
                    .as_str()
                    .ok_or("WinGetパッケージIDがありません")?;
                tools.push(Tool{id:format!("{}:winget:{name}",env.id),environment:env.id.clone(),manager:"winget".into(),name:name.into(),version:p["Version"].as_str().unwrap_or("不明").into(),path:String::new(),active:false,installed:true,source:source["SourceDetails"]["Name"].as_str().unwrap_or("winget").into(),requested:name.into(),note:"WinGetで照合・エクスポートできたアプリです。物理的な配置先・同梱ランタイム・すべてのWindowsアプリを網羅するものではありません。".into()});
            }
        }
    }
    Ok(tools)
}
pub fn scan(env: &Environment, program: &str) -> Result<Vec<Tool>, String> {
    let path = std::env::temp_dir().join(format!("dev-steward-inventory-{}.json", id()));
    let path_string = path.to_string_lossy().to_string();
    let result = (|| {
        checked(
            env,
            program,
            &[
                "export",
                "--output",
                &path_string,
                "--include-versions",
                "--disable-interactivity",
            ],
            &env.home,
        )?;
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        parse(&text, env)
    })();
    // Remove only this exact generated temporary file. No recursive deletion.
    let _ = std::fs::remove_file(path);
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_version_is_not_a_fake_number() {
        let e = Environment {
            id: "local".into(),
            name: String::new(),
            kind: "local".into(),
            home: String::new(),
            os: "windows".into(),
        };
        let result = parse(
            r#"{"Sources":[{"Packages":[{"PackageIdentifier":"Example.App"}]}]}"#,
            &e,
        )
        .unwrap();
        assert_eq!(result[0].version, "不明");
        assert_eq!(result[0].manager, "winget");
    }
}
