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
pub enum UpdatePlan {
    Ready(Box<Plan>),
    NoUpdate { message: String, installed: String },
}
pub fn installed_candidate(text: &str, id: &str) -> Result<(String, Option<String>), String> {
    for line in text.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        let Some(index) = fields.iter().position(|s| *s == id) else {
            continue;
        };
        let tail = &fields[index + 1..];
        // --source winget may omit the Source column entirely.
        let tail = tail.strip_suffix(&["winget"]).unwrap_or(tail);
        if ![1, 2].contains(&tail.len()) {
            continue;
        }
        let installed = tail[0].to_string();
        if ["Unknown", "不明"].contains(&installed.as_str()) {
            return Err("導入版を確認できません。更新不要とは判定していません".into());
        }
        let candidate = if tail.len() == 2 && tail[1] != tail[0] {
            Some(tail[1].to_string())
        } else {
            None
        };
        return Ok((installed, candidate));
    }
    Err("WinGetの導入版・更新候補の行を確認できません。再取得してください".into())
}
pub fn winget_check(
    env: &Environment,
    settings: &Settings,
    id: &str,
) -> Result<(String, Option<String>), String> {
    let exe = manager(env, settings, "winget").ok_or("WinGetが未検出です")?;
    let list = checked(
        env,
        &exe,
        &[
            "list",
            "--id",
            id,
            "--exact",
            "--source",
            "winget",
            "--disable-interactivity",
        ],
        &env.home,
    )?;
    installed_candidate(&list, id)
}
pub fn winget(
    env: &Environment,
    settings: &Settings,
    id: &str,
    expected: Option<&str>,
) -> Result<UpdatePlan, String> {
    if env.os != "windows"
        || id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-_+".contains(c))
    {
        return Err("Windows環境と正確なWinGetパッケージIDが必要です".into());
    }
    let (installed, candidate) = winget_check(env, settings, id)?;
    let Some(candidate) = candidate else {
        return Ok(UpdatePlan::NoUpdate {
            message: format!(
                "{id} はWinGetで更新候補がありません（導入済み: {installed}）。計画は作成していません。上流の最新版と管理元の配布版は異なる場合があります。"
            ),
            installed,
        });
    };
    if expected.is_some_and(|value| value != candidate) {
        return Err("WinGetの更新候補が変わりました。再取得してください".into());
    }
    if candidate.starts_with('-') || candidate.contains(['\n', '\r', '\0']) {
        return Err("候補版の形式が不正です".into());
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
            "--version",
            &candidate,
            "--disable-interactivity",
        ],
        &env.home,
    )?;
    let mut result = plan(
        env,
        format!("WinGet: {id} の更新"),
        "WinGetの更新候補を確認し、指定した版に更新します。".into(),
        exe,
        strings(&[
            "upgrade",
            "--id",
            id,
            "--exact",
            "--source",
            "winget",
            "--version",
            &candidate,
            "--disable-interactivity",
        ]),
    );
    result.evidence.extend([
        format!("管理元: WinGet / {id}"),
        format!("導入済み: {installed}"),
        format!("更新候補: {candidate}"),
        format!("確認日時: {}", now()),
        format!("配布情報:\n{}", crate::redact::output(&evidence)),
    ]);
    result.steps[0].kind = "system-update".into();
    Ok(UpdatePlan::Ready(Box::new(result)))
}
pub fn manager_update(
    env: &Environment,
    settings: &Settings,
    name: &str,
) -> Result<UpdatePlan, String> {
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
        return winget(env, settings, package, None);
    }
    let exe = manager(env, settings, name).ok_or("対象の管理ツールが未検出です")?;
    let args = match name {
        "mise" => strings(&["self-update", "--yes"]),
        "uv" => strings(&["self", "update"]),
        _ => strings(&["self", "update"]),
    };
    let mut result=plan(env,format!("{name} 本体の更新"),"WinGetでの登録が確認できない場合、管理ツール自身の公式更新機能を使用します。別のインストーラー管理の場合は管理ツール側が拒否することがあります。".into(),exe,args);
    result.warnings.push("処理が拒否された場合は導入元を確認してください。他の管理元に勝手に切り替えて再インストールしません。".into());
    Ok(UpdatePlan::Ready(Box::new(result)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_id_no_update_and_candidate_are_distinct() {
        assert_eq!(
            installed_candidate(
                "Name ID Version Source\nMise jdx.mise 2026.10.12 winget",
                "jdx.mise"
            )
            .unwrap(),
            ("2026.10.12".into(), None)
        );
        assert_eq!(
            installed_candidate(
                "名前 ID バージョン 利用可能 ソース\nExample App Example.App 1.0.0 1.1.0 winget",
                "Example.App"
            )
            .unwrap(),
            ("1.0.0".into(), Some("1.1.0".into()))
        );
        assert!(installed_candidate("Example Example.App2 1.0 1.1 winget", "Example.App").is_err());
        assert_eq!(
            installed_candidate(
                "名前 ID バージョン\nmise-en-place jdx.mise 2026.10.12",
                "jdx.mise"
            )
            .unwrap(),
            ("2026.10.12".into(), None)
        );
        assert_eq!(
            installed_candidate(
                "Name ID Version Available\nApp Example.App 1.0.0 1.1.0",
                "Example.App"
            )
            .unwrap(),
            ("1.0.0".into(), Some("1.1.0".into()))
        );
        assert!(installed_candidate("changed format", "jdx.mise").is_err());
    }
}
