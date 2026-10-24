use crate::{inventory::manager, models::*, process::*, projects};

fn step(label: &str, program: &str, args: Vec<String>, cwd: &str, kind: &str, risk: &str) -> Step {
    Step {
        id: id(),
        label: label.into(),
        program: program.into(),
        args,
        cwd: cwd.into(),
        kind: kind.into(),
        risk: risk.into(),
        status: "pending".into(),
        output: String::new(),
    }
}
fn base(env: &Environment, project: Option<&Project>, title: String, reason: &str) -> Plan {
    Plan {
        id: id(),
        environment: env.id.clone(),
        project_id: project.map(|p| p.id.clone()),
        title,
        reason: reason.into(),
        evidence: vec![],
        warnings: vec![],
        steps: vec![],
        created_at: now(),
        status: "ready".into(),
        fingerprint: project.map(|p| p.fingerprint.clone()).unwrap_or_default(),
    }
}
pub fn update(
    env: &Environment,
    settings: &Settings,
    project: &Project,
    updates: &[Update],
    selected: &[usize],
    checks: &[String],
    publish: bool,
) -> Result<Plan, String> {
    if project.findings.iter().any(|f| f.status == "error") {
        return Err(
            "設定の解析・読み取りに失敗した箇所があります。診断を確認して再調査してください。"
                .into(),
        );
    }
    if !project.git_status.is_empty() {
        return Err("未コミット変更があります。先にコミットまたは退避してください。アプリは既存の変更を上書きしません。".into());
    }
    if selected.is_empty() {
        return Err("更新候補を選択してください".into());
    }
    let git = manager(env, settings, "git").ok_or("Gitが見つかりません")?;
    let mut plan = base(
        env,
        Some(project),
        format!("{} の依存更新", project.name),
        "同一メジャー内の安定版候補を、選択した依存に限定して適用します。",
    );
    let branch = format!("steward/update-{}", &plan.id[..8]);
    plan.steps.push(step(
        "更新用ブランチを作成",
        &git,
        strings(&["switch", "-c", &branch]),
        &project.path,
        "branch",
        "write",
    ));
    for index in selected {
        let u = updates.get(*index).ok_or("更新候補の選択が不正です")?;
        if u.fingerprint != project.fingerprint {
            return Err(
                "候補の照合後にプロジェクトが変わっています。更新候補を再照合してください。".into(),
            );
        }
        if u.project_id.as_deref() != Some(&project.id)
            || u.candidate.is_empty()
            || crate::updates::numeric(&u.candidate).is_none()
        {
            return Err("実行可能なバージョン候補ではありません".into());
        }
        if !crate::updates::numeric(&u.current)
            .zip(crate::updates::numeric(&u.candidate))
            .is_some_and(|(a, b)| crate::updates::compatible(&a, &b))
        {
            return Err("メジャー更新はこの操作では実行しません".into());
        }
        let requirement = project
            .requirements
            .iter()
            .find(|r| r.name == u.name && r.source == u.source && r.section != "locked")
            .ok_or("直接依存ではありません")?;
        let parent = u
            .source
            .rsplit_once('/')
            .map(|(p, _)| join(&project.path, p, env))
            .unwrap_or_else(|| project.path.clone());
        match u.manager.as_str() {
            "npm" => {
                let mise=manager(env,settings,"mise").ok_or("Node系パッケージの実行にはmiseが必要です")?;
                let pm=node_manager(env,project,&parent)?;
                let mut args=strings(&["exec","--",pm,if pm=="pnpm"{"add"}else{"install"},&format!("{}@{}",u.name,u.candidate),"--ignore-scripts"]);
                if pm=="pnpm" && parent==project.path && project.manifest_files.contains(&"pnpm-workspace.yaml".into()){args.push("--workspace-root".into());}
                match requirement.section.as_str(){"devDependencies"=>args.push("--save-dev".into()),"optionalDependencies"=>args.push("--save-optional".into()),_=>{}}
                plan.steps.push(step(&format!("{} → {}",u.name,u.candidate),&mise,args,&parent,"update","scripts-disabled"));
            },
            "cargo" => {
                let cargo=manager(env,settings,"cargo").ok_or("Cargoが未検出です")?;
                plan.steps.push(step(&format!("{} → {} のCargoロック更新",u.name,u.candidate),&cargo,strings(&["update","--package",&u.name,"--precise",&u.candidate]),&parent,"update","write"));
            },
            "pypi" if project.manifest_files.iter().any(|p|p.ends_with("uv.lock")) => {
                let uv=manager(env,settings,"uv").ok_or("uvが未検出です")?;
                plan.steps.push(step(&format!("{} → {} のロック更新",u.name,u.candidate),&uv,strings(&["lock","--upgrade-package",&format!("{}=={}",u.name,u.candidate)]),&parent,"update","write"));
                plan.steps.push(step("uvロックに合わせて仮想環境を同期",&uv,strings(&["sync","--locked"]),&parent,"sync","executes-project-code"));
            },
            _=>return Err("この管理形式の更新実行は未対応です。npm・pnpm・uvの対応した直接依存から選択してください。".into()),
        }
        plan.evidence.push(format!(
            "{} {} → {} / {}",
            u.name, u.current, u.candidate, u.source
        ));
    }
    add_checks(env, settings, project, checks, &mut plan)?;
    plan.steps.push(step(
        "差分の形式を検査",
        &git,
        strings(&["diff", "--check"]),
        &project.path,
        "verify",
        "read",
    ));
    if publish {
        if checks.is_empty() {
            return Err("PR作成には少なくとも1つの検証を選択してください".into());
        }
        plan.steps.push(step(
            "更新差分を表示して確認",
            &git,
            strings(&["diff", "--no-ext-diff", "--no-textconv", "HEAD"]),
            &project.path,
            "review-diff",
            "read",
        ));
        let mut files: Vec<String> = project
            .manifest_files
            .iter()
            .filter(|s| {
                s.ends_with("Cargo.lock")
                    || s.ends_with("package.json")
                    || s.ends_with("package-lock.json")
                    || s.ends_with("pnpm-lock.yaml")
                    || s.ends_with("uv.lock")
            })
            .cloned()
            .collect();
        files.sort();
        files.dedup();
        let mut args = strings(&["add", "--"]);
        args.extend(files);
        plan.steps.push(step(
            "依存ファイルのみをステージ",
            &git,
            args,
            &project.path,
            "stage",
            "write",
        ));
        plan.steps.push(step(
            "検証した変更をコミット",
            &git,
            strings(&[
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "-m",
                "chore: パッケージを更新",
            ]),
            &project.path,
            "commit",
            "write",
        ));
        plan.steps.push(step(
            "更新ブランチをプッシュ",
            &git,
            strings(&["push", "--set-upstream", "origin", &branch]),
            &project.path,
            "push",
            "external",
        ));
        let gh = manager(env, settings, "gh").ok_or("PR作成にはGitHub CLIと認証が必要です")?;
        let body = format!(
            "- 同一メジャー内で選択した依存パッケージを更新\n- 検証: {}\n- Dev Stewardで計画・実行",
            checks.join(", ")
        );
        plan.steps.push(step(
            "PRを作成",
            &gh,
            strings(&[
                "pr",
                "create",
                "--title",
                "chore: パッケージを更新",
                "--body",
                &body,
            ]),
            &project.path,
            "pr",
            "external",
        ));
    }
    plan.warnings=vec!["更新候補の互換性・脆弱性修正は未保証です。各検証の成功と差分を確認してください。".into(),"同期や検証はプロジェクトのコード・ビルド処理を実行します。信頼できるプロジェクトで実行してください。".into(),"中断や失敗後に環境・ブランチを自動復元しません。履歴と差分を確認してください。".into(),"PRは自動マージしません。CIを確認し、別途マージしてください。".into()];
    Ok(plan)
}

pub fn node_manager(env: &Environment, p: &Project, cwd: &str) -> Result<&'static str, String> {
    let path = join(cwd, "package.json", env);
    let value: serde_json::Value =
        serde_json::from_str(&read(env, &path)?).map_err(|e| e.to_string())?;
    let declared = value["packageManager"]
        .as_str()
        .unwrap_or(&p.package_manager);
    if declared.starts_with("pnpm@") {
        Ok("pnpm")
    } else if declared.starts_with("yarn@") || declared.starts_with("bun@") {
        Err("Yarn・Bunの更新実行は未対応です".into())
    } else {
        Ok("npm")
    }
}
fn add_checks(
    env: &Environment,
    settings: &Settings,
    p: &Project,
    checks: &[String],
    plan: &mut Plan,
) -> Result<(), String> {
    for check in checks {
        if !p.scripts.contains(check) {
            return Err("登録済みの検証を選択してください".into());
        }
        let (scope, name) = check.rsplit_once("::").unwrap_or(("", check));
        let cwd = if scope.is_empty() {
            p.path.clone()
        } else {
            join(&p.path, scope, env)
        };
        if ["cargo-check", "cargo-test"].contains(&name) {
            let cargo = manager(env, settings, "cargo").ok_or("Cargoが未検出です")?;
            plan.steps.push(step(
                &format!("検証: {check}"),
                &cargo,
                strings(&[
                    if name == "cargo-check" {
                        "check"
                    } else {
                        "test"
                    },
                    "--locked",
                ]),
                &cwd,
                "test",
                "executes-project-code",
            ));
        } else if ["uv-pytest", "uv-ruff"].contains(&name) {
            let uv = manager(env, settings, "uv").ok_or("uvが未検出です")?;
            let args = if name == "uv-pytest" {
                strings(&["run", "--locked", "--no-sync", "python", "-m", "pytest"])
            } else {
                strings(&["run", "--locked", "--no-sync", "ruff", "check", "."])
            };
            plan.steps.push(step(
                &format!("検証: {check}"),
                &uv,
                args,
                &cwd,
                "test",
                "executes-project-code",
            ));
        } else {
            if !["test", "build", "lint", "typecheck", "check"].contains(&name) {
                return Err("未対応の検証です".into());
            }
            let mise = manager(env, settings, "mise").ok_or("miseが未検出です")?;
            let pm = node_manager(env, p, &cwd)?;
            plan.steps.push(step(
                &format!("検証: {check}"),
                &mise,
                strings(&["exec", "--", pm, "run", name]),
                &cwd,
                "test",
                "executes-project-code",
            ));
        }
    }
    Ok(())
}
pub fn validate(
    env: &Environment,
    settings: &Settings,
    p: &Project,
    checks: &[String],
) -> Result<Plan, String> {
    if checks.is_empty() {
        return Err("検証スクリプトを選択してください".into());
    }
    let mut plan = base(
        env,
        Some(p),
        format!("{} の実行検証", p.name),
        "登録済みスクリプトを実行します。静的調査とは異なり、プロジェクトのコードが実行されます。",
    );
    add_checks(env, settings, p, checks, &mut plan)?;
    plan.warnings.push(
        "プロジェクトのスクリプトは任意のコードを実行できます。対象を確認してください。".into(),
    );
    Ok(plan)
}

pub fn tool(
    env: &Environment,
    settings: &Settings,
    tool: &Tool,
    update: &Update,
) -> Result<Plan, String> {
    if update.candidate.is_empty() || update.tool_id.as_deref() != Some(&tool.id) {
        return Err("有効な更新候補がありません".into());
    }
    let mut plan = base(
        env,
        None,
        format!("{} {} の導入", tool.name, update.candidate),
        "新しい版を導入します。既存のプロジェクト固定・仮想環境・旧版は変更しません。",
    );
    let (program, args) = match tool.manager.as_str() {
        "mise" => (
            manager(env, settings, "mise").ok_or("miseがありません")?,
            strings(&["install", &format!("{}@{}", tool.name, update.candidate)]),
        ),
        "uv" => (
            manager(env, settings, "uv").ok_or("uvがありません")?,
            strings(&["python", "install", &update.candidate]),
        ),
        "rustup" => (
            manager(env, settings, "rustup").ok_or("rustupがありません")?,
            strings(&[
                "toolchain",
                "install",
                &update.candidate,
                "--profile",
                "minimal",
                "--component",
                "rustfmt",
                "--component",
                "clippy",
            ]),
        ),
        _ => return Err("この管理元の更新実行は未対応です".into()),
    };
    plan.steps.push(step(
        "新しい版を管理元から導入",
        &program,
        args,
        &env.home,
        "tool-install",
        "installer",
    ));
    plan.evidence.push(format!(
        "{} {} → {}",
        tool.name, tool.version, update.candidate
    ));
    plan.warnings.push(
        "プロジェクトの固定版は引き続き旧版を使います。移行検証前に旧版を削除しないでください。"
            .into(),
    );
    Ok(plan)
}

pub fn tool_updates(env: &Environment, settings: &Settings, inventory: &Inventory) -> Vec<Update> {
    let mut result = vec![];
    for tool in inventory.tools.iter().filter(|t| t.installed) {
        let Some(current) = tool_numeric(&tool.version) else {
            continue;
        };
        let latest: Result<String, String> = (|| match tool.manager.as_str() {
            "mise" => {
                let mise = manager(env, settings, "mise").ok_or("miseが未検出です")?;
                let channel = if tool.version.starts_with("temurin-") {
                    format!("temurin-{}", current.major)
                } else {
                    current.major.to_string()
                };
                checked(
                    env,
                    &mise,
                    &["latest", &format!("{}@{channel}", tool.name)],
                    &env.home,
                )
            }
            "uv" => {
                if !tool.requested.starts_with("cpython-") {
                    return Err("CPython以外のランタイムの更新候補は未対応です".into());
                }
                let uv = manager(env, settings, "uv").ok_or("uvが未検出です")?;
                let text = checked(
                    env,
                    &uv,
                    &[
                        "python",
                        "list",
                        "--all-versions",
                        "--output-format",
                        "json",
                    ],
                    &env.home,
                )?;
                let entries: Vec<serde_json::Value> =
                    serde_json::from_str(&text).map_err(|e| e.to_string())?;
                Ok(crate::updates::choose(
                    &tool.version,
                    entries
                        .iter()
                        .filter(|v| v["implementation"] == "cpython" && v["variant"] == "default")
                        .filter_map(|v| v["version"].as_str())
                        .filter(|s| {
                            crate::updates::numeric(s).is_some_and(|v| v.minor == current.minor)
                        })
                        .map(str::to_string),
                )
                .unwrap_or_else(|| tool.version.clone()))
            }
            "rustup" => {
                let text = crate::updates::client()?
                    .get("https://static.rust-lang.org/dist/channel-rust-stable.toml")
                    .send()
                    .and_then(|r| r.error_for_status())
                    .and_then(|r| r.text())
                    .map_err(|e| e.to_string())?;
                let v: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
                v.get("pkg")
                    .and_then(|v| v.get("rust"))
                    .and_then(|v| v.get("version"))
                    .and_then(toml::Value::as_str)
                    .and_then(|s| s.split_whitespace().next())
                    .map(str::to_string)
                    .ok_or_else(|| "Rust配布情報の形式に未対応です".into())
            }
            _ => Err("OS・別の配布元の管理です。この経路では更新しません。".into()),
        })();
        let (candidate, latest, importance, note) = match latest {
            Ok(latest) => {
                let Some(version) = tool_numeric(&latest) else {
                    result.push(Update {
                        fingerprint: String::new(),
                        name: tool.name.clone(),
                        manager: tool.manager.clone(),
                        current: tool.version.clone(),
                        candidate: String::new(),
                        latest,
                        source: tool.path.clone(),
                        importance: "unknown".into(),
                        vulnerabilities: vec![],
                        audit_status: "未監査".into(),
                        note: "配布元の版を解析できませんでした".into(),
                        project_id: None,
                        tool_id: Some(tool.id.clone()),
                    });
                    continue;
                };
                let candidate =
                    if version > current && crate::updates::compatible(&current, &version) {
                        latest.clone()
                    } else {
                        String::new()
                    };
                let importance = if candidate.is_empty() {
                    "current"
                } else {
                    "maintenance"
                };
                (candidate,latest,importance.to_string(),"管理元へ問い合わせ済み。同一メジャー、Pythonは同一minorの候補です。互換性の検証は別途必要です。".to_string())
            }
            Err(e) => (
                String::new(),
                String::new(),
                "unknown".into(),
                format!("照合失敗または未対応：{e}"),
            ),
        };
        result.push(Update {
            fingerprint: String::new(),
            name: tool.name.clone(),
            manager: tool.manager.clone(),
            current: tool.version.clone(),
            candidate,
            latest,
            source: tool.path.clone(),
            importance,
            vulnerabilities: vec![],
            audit_status:
                "ランタイムのセキュリティ監査は未実施。配布元のリリース情報を確認してください。"
                    .into(),
            note,
            project_id: None,
            tool_id: Some(tool.id.clone()),
        });
    }
    result
}

pub fn tool_numeric(value: &str) -> Option<semver::Version> {
    crate::updates::numeric(value.strip_prefix("temurin-").unwrap_or(value))
}

pub fn assert_fresh(
    env: &Environment,
    settings: &Settings,
    p: &Project,
    plan: &Plan,
) -> Result<Project, String> {
    let fresh = projects::scan(env, settings, &p.path)?;
    if fresh.fingerprint != plan.fingerprint {
        return Err(
            "計画作成後にリポジトリが変更されました。再調査して計画を作り直してください。".into(),
        );
    }
    Ok(fresh)
}
