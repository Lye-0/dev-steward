use crate::{inventory, models::*, process::*, projects};

fn in_use(env: &Environment, path: &str) -> Result<(), String> {
    if env.kind != "local" {
        return Err(
            "WSLのプロセス・PATH調査を伴う削除実行は未対応です。管理元で手動確認してください。"
                .into(),
        );
    }
    if std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .any(|p| p.starts_with(path))
    {
        return Err("対象の版がPATHに直接含まれています".into());
    }
    #[cfg(windows)]
    {
        // Escape data as a PowerShell single-quoted literal. No command line is returned or persisted.
        let target = path.replace('\'', "''");
        let script = format!(
            "$ErrorActionPreference='Stop'; $target='{target}'; foreach($scope in @('User','Machine')) {{ $p=[Environment]::GetEnvironmentVariable('Path',$scope); if($p -and $p.IndexOf($target,[StringComparison]::OrdinalIgnoreCase) -ge 0) {{ throw 'Target is in PATH' }} }}; $hits=@(Get-CimInstance Win32_Process | Where-Object {{ $_.ProcessId -ne $PID -and (($_.ExecutablePath -and $_.ExecutablePath.StartsWith($target+'\\',[StringComparison]::OrdinalIgnoreCase)) -or ($_.CommandLine -and $_.CommandLine.IndexOf($target,[StringComparison]::OrdinalIgnoreCase) -ge 0)) }}); if($hits.Count) {{ throw 'Target has running process references' }}; 'No live references found'"
        );
        checked(
            env,
            "powershell.exe",
            &["-NoProfile", "-NonInteractive", "-Command", &script],
            &env.home,
        )?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err(
            "このOSでの実行中プロセスを伴う削除確認は未対応です。管理元で手動確認してください。"
                .into(),
        )
    }
}

pub fn verify(
    env: &Environment,
    settings: &Settings,
    tool: &Tool,
    registered: &[Project],
) -> Result<(), String> {
    if tool.manager != "mise" || !tool.installed || tool.active || tool.path.is_empty() {
        return Err("削除は未使用判定されたmise管理版に限定しています。Python・OS・アプリ同梱環境は対象外です。".into());
    }
    if settings.roots.iter().all(|(e, _)| e != &env.id) {
        return Err("先に調査対象のフォルダーを登録してください".into());
    }
    let exe = inventory::manager(env, settings, "mise").ok_or("miseが未検出です")?;
    let current = inventory::parse_mise(
        &checked(env, &exe, &["ls", "--prunable", "--json"], &env.home)?,
        env,
    )?;
    if !current
        .iter()
        .any(|t| t.name == tool.name && t.version == tool.version && t.path == tool.path)
    {
        return Err("管理元が対象を未使用と判定していません".into());
    }
    let canonical = std::fs::canonicalize(&tool.path).map_err(|e| e.to_string())?;
    let allowed = std::path::Path::new(&tool.path);
    if canonical.file_name() != allowed.file_name()
        || canonical.parent().and_then(|p| p.file_name()) != Some(std::ffi::OsStr::new(&tool.name))
    {
        return Err("想定外のインストール先です。削除しません。".into());
    }
    let mut checked_paths = std::collections::BTreeSet::new();
    for (_, root) in settings.roots.iter().filter(|(e, _)| e == &env.id) {
        for path in projects::discover(env, settings, root)? {
            checked_paths.insert(path);
        }
    }
    for p in registered.iter().filter(|p| p.environment == env.id) {
        checked_paths.insert(p.path.clone());
    }
    for path in checked_paths {
        let p = projects::scan(env, settings, &path)?;
        if p.findings.iter().any(|f| f.status == "error") {
            return Err(format!("{}の調査に失敗しています", p.name));
        }
        if p.requirements.iter().any(|r| {
            if r.name != tool.name {
                return false;
            }
            let version = r
                .requested
                .strip_prefix(&format!("{}@", tool.name))
                .unwrap_or(&r.requested);
            // Ranges, aliases and unrecognized declarations are unresolved references, not proof of non-use.
            crate::updates::numeric(version).is_none() || version == tool.version
        }) {
            return Err(format!("{}に対象版の関連宣言があります", p.name));
        }
        if p.venvs.iter().any(|v| v.home.starts_with(&tool.path)) {
            return Err(format!("{}の仮想環境が参照しています", p.name));
        }
    }
    for config in &settings.mcp_configs {
        for link in crate::integrations::mcp(config)? {
            if link["command"]
                .as_str()
                .is_some_and(|p| p.starts_with(&tool.path))
                || link["home"]
                    .as_str()
                    .is_some_and(|p| p.starts_with(&tool.path))
            {
                return Err("MCP設定から参照されています".into());
            }
        }
    }
    in_use(env, &tool.path)
}

pub fn plan(
    env: &Environment,
    settings: &Settings,
    tool: &Tool,
    registered: &[Project],
) -> Result<Plan, String> {
    verify(env, settings, tool, registered)?;
    let exe = inventory::manager(env, settings, "mise").ok_or("miseがありません")?;
    let target = format!("{}@{}", tool.name, tool.version);
    let step = Step {
        id: id(),
        label: format!("{target} を削除"),
        program: exe,
        args: strings(&["uninstall", &target, "--yes"]),
        cwd: env.home.clone(),
        kind: "tool-remove".into(),
        risk: "delete".into(),
        status: "pending".into(),
        output: String::new(),
    };
    Ok(Plan{id:id(),environment:env.id.clone(),project_id:None,title:format!("{target} の整理"),reason:"登録した調査範囲とmiseの未使用判定に基づく削除計画です。範囲外の利用がないこともご自身で確認してください。".into(),evidence:vec![tool.id.clone(),tool.path.clone(),"管理元・登録フォルダー・登録MCP・PATH・実行中プロセスを確認。実行直前にも再確認します。".into()],warnings:vec!["未登録のプロジェクトや設定の利用は網羅できません。対象と調査範囲を確認した場合だけ実行してください。".into(),"削除した版の復元には、配布元からの再取得が必要です。".into()],steps:vec![step],created_at:now(),status:"ready".into(),fingerprint:String::new()})
}
