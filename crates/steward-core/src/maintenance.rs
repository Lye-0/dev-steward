use crate::{inventory::manager, models::*, process::*};
fn plan(
    env: &Environment,
    title: String,
    reason: String,
    program: String,
    args: Vec<String>,
) -> Plan {
    Plan{id:id(),environment:env.id.clone(),project_id:None,title,reason,evidence:vec![program.clone()],warnings:vec!["管理元のインストーラーが実行されます。管理者権限や再起動が必要な場合は、その結果を確認してください。".into(),"管理ツール本体の更新はプロジェクトの固定版や仮想環境の移行とは別です。".into()],steps:vec![Step{id:id(),label:"管理元から更新を実行".into(),program,args,cwd:env.home.clone(),kind:"tool-manager-update".into(),risk:"installer".into(),status:"pending".into(),output:String::new()}],created_at:now(),status:"ready".into(),fingerprint:String::new()}
}
pub fn winget(env: &Environment, settings: &Settings, id: &str) -> Result<Plan, String> {
    if env.os != "windows"
        || id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-_+".contains(c))
    {
        return Err("Windows環境と正確なWinGetパッケージIDが必要です".into());
    }
    let exe = manager(env, settings, "winget").ok_or("WinGetが未検出です")?;
    let evidence = checked(
        env,
        &exe,
        &[
            "show",
            "--id",
            id,
            "--exact",
            "--source",
            "winget",
            "--disable-interactivity",
        ],
        &env.home,
    )?;
    let mut result = plan(
        env,
        format!("WinGet: {id} の更新"),
        "WinGetの配布情報を確認し、指定した1パッケージだけを更新します。".into(),
        exe,
        strings(&[
            "upgrade",
            "--id",
            id,
            "--exact",
            "--source",
            "winget",
            "--disable-interactivity",
        ]),
    );
    result.evidence.push(evidence);
    result.steps[0].kind = "system-update".into();
    Ok(result)
}
pub fn manager_update(env: &Environment, settings: &Settings, name: &str) -> Result<Plan, String> {
    let package = match name {
        "mise" => "jdx.mise",
        "uv" => "astral-sh.uv",
        "rustup" => "Rustlang.Rustup",
        _ => return Err("mise・uv・rustupから選択してください".into()),
    };
    if env.os == "windows"
        && let Some(winget_path) = manager(env, settings, "winget")
        && checked(
            env,
            &winget_path,
            &[
                "list",
                "--id",
                package,
                "--exact",
                "--disable-interactivity",
            ],
            &env.home,
        )
        .is_ok()
    {
        return winget(env, settings, package);
    }
    let exe = manager(env, settings, name).ok_or("対象の管理ツールが未検出です")?;
    let args = match name {
        "mise" => strings(&["self-update", "--yes"]),
        "uv" => strings(&["self", "update"]),
        _ => strings(&["self", "update"]),
    };
    let mut result=plan(env,format!("{name} 本体の更新"),"WinGetでの登録が確認できない場合、管理ツール自身の公式更新機能を使用します。別のインストーラー管理の場合は管理ツール側が拒否することがあります。".into(),exe,args);
    result.warnings.push("処理が拒否された場合は導入元を確認してください。他の管理元に勝手に切り替えて再インストールしません。".into());
    Ok(result)
}
