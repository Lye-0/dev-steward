use crate::{inventory::manager, models::*, process::*};
use serde_json::{Value, json};

fn command(label: &str, program: &str, args: &[&str], cwd: &str, kind: &str) -> Step {
    Step {
        id: id(),
        label: label.into(),
        program: program.into(),
        args: strings(args),
        cwd: cwd.into(),
        kind: kind.into(),
        risk: "write".into(),
        status: "pending".into(),
        output: String::new(),
    }
}
pub fn plan(
    env: &Environment,
    settings: &Settings,
    project: &Project,
    tool: &Tool,
) -> Result<Plan, String> {
    if !project.git_status.is_empty() {
        return Err("固定版の変更前に、既存の変更をコミットまたは退避してください".into());
    }
    if !tool.installed || tool.environment != project.environment {
        return Err("同じ環境にインストール済みの版を選択してください".into());
    }
    if crate::updates::numeric(&tool.version).is_none() {
        return Err("数値バージョンのツールを選択してください".into());
    }
    let git = manager(env, settings, "git").ok_or("Gitがありません")?;
    let plan_id = id();
    let branch = format!("steward/pin-{}", &plan_id[..8]);
    let mut steps = vec![command(
        "固定版変更用のブランチを作成",
        &git,
        &["switch", "-c", &branch],
        &project.path,
        "branch",
    )];
    match tool.manager.as_str() {
        "mise" => {
            let mise = manager(env, settings, "mise").ok_or("miseがありません")?;
            steps.push(command(
                "プロジェクトのmise指定を更新",
                &mise,
                &["use", &format!("{}@{}", tool.name, tool.version)],
                &project.path,
                "pin",
            ));
            if tool.name == "pnpm" && project.manifest_files.contains(&"package.json".into()) {
                steps.push(command(
                    "packageManagerとdevEnginesを同期",
                    "package.json の構造化編集",
                    &[&tool.version],
                    &project.path,
                    "package-manager-pin",
                ));
            }
        }
        "uv" if tool.name == "python" => {
            if !project.manifest_files.contains(&"pyproject.toml".into()) {
                return Err("Pythonの自動切り替えはルートにpyproject.tomlのあるuvプロジェクトが対象です。専用MCP環境の移行は別途検証してください。".into());
            }
            let uv = manager(env, settings, "uv").ok_or("uvがありません")?;
            steps.push(command(
                "uvプロジェクトのPythonを指定",
                &uv,
                &["python", "pin", &tool.version],
                &project.path,
                "pin",
            ));
            steps.push(command(
                "指定Pythonで仮想環境を同期",
                &uv,
                &["sync", "--locked", "--python", &tool.version],
                &project.path,
                "sync",
            ));
        }
        "rustup" => steps.push(command(
            "プロジェクトのRustを固定",
            "rust-toolchain.toml の構造化編集",
            &[&tool.version],
            &project.path,
            "rust-pin",
        )),
        _ => return Err("この管理元のプロジェクト固定は未対応です".into()),
    }
    steps.push(command(
        "差分の形式を検査",
        &git,
        &["diff", "--check"],
        &project.path,
        "verify",
    ));
    let mut plan = Plan{id:plan_id,environment:env.id.clone(),project_id:Some(project.id.clone()),title:format!("{}: {} を {} に固定",project.name,tool.name,tool.version),reason:"デバイスに導入済みの版へプロジェクトの指定を揃えます。変更後の実行検証はプロジェクト画面から別途計画してください。".into(),evidence:vec![tool.path.clone(),format!("{} / {}",tool.manager,tool.version)],warnings:vec!["この操作だけでは互換性・起動・接続を確認したことになりません。検証後にコミットしてください。".into(),"Pythonの同期は仮想環境の再作成やビルド処理を伴う場合があります。旧Python本体は削除しません。".into(),"Rustはrustupが固定版を別ツールチェーンとして保持する場合があります。mise管理には追加しません。".into()],steps,created_at:now(),status:"ready".into(),fingerprint:project.fingerprint.clone()};
    plan.warnings.extend(crate::ci::plan_warnings(project));
    Ok(plan)
}
pub fn apply(env: &Environment, step: &Step) -> Result<Output, String> {
    let version = step.args.first().ok_or("固定する版がありません")?;
    crate::updates::numeric(version).ok_or("無効なバージョンです")?;
    let (filename, text) = if step.kind == "package-manager-pin" {
        let path = join(&step.cwd, "package.json", env);
        let mut v: Value = serde_json::from_str(&read(env, &path)?).map_err(|e| e.to_string())?;
        v["packageManager"] = json!(format!("pnpm@{version}"));
        if v.get("devEngines")
            .and_then(|v| v.get("packageManager"))
            .is_some()
        {
            v["devEngines"]["packageManager"] =
                json!({"name":"pnpm","version":version,"onFail":"error"});
        }
        (
            "package.json",
            serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n",
        )
    } else if step.kind == "rust-pin" {
        let path = join(&step.cwd, "rust-toolchain.toml", env);
        let mut v: toml::Value = match read(env, &path) {
            Ok(s) => toml::from_str(&s).map_err(|e| e.to_string())?,
            Err(_) => toml::Value::Table(toml::map::Map::new()),
        };
        if v.get("toolchain").is_none() {
            v["toolchain"] = toml::Value::Table(toml::map::Map::new());
        }
        v["toolchain"]["channel"] = toml::Value::String(version.clone());
        (
            "rust-toolchain.toml",
            toml::to_string_pretty(&v).map_err(|e| e.to_string())?,
        )
    } else {
        return Err("未対応の構造化編集です".into());
    };
    write_atomic(env, &join(&step.cwd, filename, env), &text)?;
    Ok(Output {
        code: 0,
        stdout: format!("{filename} の固定版を {version} に変更しました。"),
        stderr: String::new(),
    })
}

pub fn verify_venv(env: &Environment, p: &Project, index: usize) -> Result<Plan, String> {
    let v = p.venvs.get(index).ok_or("仮想環境がありません")?;
    let executable = join(
        &v.path,
        if env.os == "windows" {
            "Scripts/python.exe"
        } else {
            "bin/python"
        },
        env,
    );
    let code = "import sys,json; print(json.dumps({'version':sys.version.split()[0],'executable':sys.executable,'base_prefix':sys.base_prefix},ensure_ascii=False))";
    let mut step = command(
        "仮想環境のPython実体と参照元を確認",
        &executable,
        &["-I", "-B", "-S", "-c", code],
        &p.path,
        "verify-venv",
    );
    step.risk = "executes-project-code".into();
    Ok(Plan{id:id(),environment:env.id.clone(),project_id:Some(p.id.clone()),title:format!("{} のPython参照確認",p.name),reason:"登録した仮想環境のPythonを起動し、版・実行ファイル・base_prefixを取得します。".into(),evidence:vec![v.path.clone(),v.home.clone()],warnings:vec!["選択した仮想環境の実行ファイルを起動します。信頼できる環境で実行してください。site初期化とユーザーsiteは無効にします。".into()],steps:vec![step],created_at:now(),status:"ready".into(),fingerprint:p.fingerprint.clone()})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pnpm_pin_synchronizes_auto_download_setting() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().to_str().unwrap();
        std::fs::write(root.path().join("package.json"),r#"{"packageManager":"pnpm@12.0.0","devEngines":{"packageManager":{"name":"pnpm","version":"12.0.0","onFail":"download"}}}"#).unwrap();
        let e = Environment {
            id: "local".into(),
            name: String::new(),
            kind: "local".into(),
            home: path.into(),
            os: std::env::consts::OS.into(),
        };
        apply(
            &e,
            &command("pin", "", &["11.28.8"], path, "package-manager-pin"),
        )
        .unwrap();
        let result: Value = serde_json::from_str(
            &std::fs::read_to_string(root.path().join("package.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(result["packageManager"], "pnpm@11.28.8");
        assert_eq!(result["devEngines"]["packageManager"]["onFail"], "error");
    }
}
