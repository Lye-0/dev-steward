use crate::{inventory, models::*, plans, process, projects, store::Store, updates};
use serde_json::{Value, json};
use std::sync::{Mutex, atomic::Ordering};

pub struct Service {
    pub store: Store,
    execution: Mutex<()>,
    _lock: std::fs::File,
}
fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .ok_or_else(|| format!("{key}が必要です"))
}
fn array(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
impl Service {
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let store = Store::new(path)?;
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(store.path.with_extension("lock"))
            .map_err(|e| e.to_string())?;
        lock.try_lock().map_err(|_| {
            "同じデータベースを使用するDev Stewardが既に起動しています。".to_string()
        })?;
        for mut plan in store.list::<Plan>("plans")? {
            if plan.steps.iter().any(|s| s.status == "running") {
                for s in &mut plan.steps {
                    if s.status == "running" {
                        s.status = "interrupted".into();
                        s.output.push_str(
                            "\nアプリの終了で結果が不明です。変更状態を調べて再計画してください。",
                        );
                    }
                }
                plan.status = "interrupted".into();
                store.put("plans", &plan.id, &plan)?;
            }
        }
        let service = Self {
            store,
            execution: Mutex::new(()),
            _lock: lock,
        };
        let local = Environment {
            id: "local".into(),
            name: "このデバイス".into(),
            kind: "local".into(),
            home: process::local_home(),
            os: std::env::consts::OS.into(),
        };
        service.store.put("environments", "local", &local)?;
        Ok(service)
    }
    pub fn settings(&self) -> Settings {
        self.store.get("settings", "default").unwrap_or_default()
    }
    fn env(&self, id: &str) -> Result<Environment, String> {
        self.store.get("environments", id)
    }
    pub fn snapshot(&self) -> Result<Value, String> {
        Ok(
            json!({"environments":self.store.list::<Environment>("environments")?,"inventories":self.store.list::<Inventory>("inventories")?,"projects":self.store.list::<Project>("projects")?,"plans":self.store.list::<Plan>("plans")?,"history":self.store.history()?,"settings":self.settings(),"updates":self.store.list::<Vec<Update>>("updates")?.into_iter().flatten().collect::<Vec<_>>(),"links":self.store.list::<Vec<Value>>("links")?.into_iter().flatten().collect::<Vec<_>>(),"data_path":self.store.path,"version":env!("CARGO_PKG_VERSION")}),
        )
    }
    pub fn call(&self, method: &str, payload: Value) -> Result<Value, String> {
        let settings = self.settings();
        match method {
            "snapshot" => self.snapshot(),
            "system_updates" => crate::integrations::system_updates(
                &self.env(field(&payload, "environment")?)?,
                &settings,
            ),
            "plan_system_update" => self.save_plan(crate::maintenance::winget(
                &self.env(field(&payload, "environment")?)?,
                &settings,
                field(&payload, "package")?,
            )?),
            "plan_manager_update" => self.save_plan(crate::maintenance::manager_update(
                &self.env(field(&payload, "environment")?)?,
                &settings,
                field(&payload, "manager")?,
            )?),
            "scan_mcp_config" => {
                let path = field(&payload, "path")?;
                let links = crate::integrations::mcp(path)?;
                self.store.put("links", path, &links)?;
                let mut settings = settings;
                if !settings.mcp_configs.contains(&path.to_string()) {
                    settings.mcp_configs.push(path.into());
                    self.store.put("settings", "default", &settings)?;
                }
                Ok(json!(links))
            }
            "detect_environments" => {
                for env in inventory::environments() {
                    self.store.put("environments", &env.id, &env)?;
                }
                self.snapshot()
            }
            "scan_environment" => {
                let env = self.env(field(&payload, "environment")?)?;
                let inv = inventory::scan(&env, &settings);
                self.store.put("inventories", &env.id, &inv)?;
                self.store.log(
                    "環境の調査",
                    &env.name,
                    "complete",
                    "未対応・調査失敗の範囲は環境の診断を参照",
                )?;
                Ok(json!(inv))
            }
            "scan_root" => {
                let env = self.env(field(&payload, "environment")?)?;
                let root = projects::safe_path(&env, field(&payload, "path")?)?;
                let found = projects::discover(&env, &settings, &root)?;
                let mut errors = vec![];
                let mut count = 0;
                for path in found {
                    match projects::scan(&env, &settings, &path) {
                        Ok(p) => {
                            self.store.put("projects", &p.id, &p)?;
                            count += 1;
                        }
                        Err(e) => errors.push(format!("{path}: {e}")),
                    }
                }
                let mut settings = settings;
                if !settings.roots.contains(&(env.id.clone(), root.clone())) {
                    settings.roots.push((env.id.clone(), root.clone()));
                    self.store.put("settings", "default", &settings)?;
                }
                self.store.log(
                    "プロジェクトの検出",
                    &root,
                    if errors.is_empty() {
                        "complete"
                    } else {
                        "partial"
                    },
                    &format!("{count}件登録。{}", errors.join("\n")),
                )?;
                Ok(json!({"count":count,"errors":errors}))
            }
            "rescan_project" => {
                let p: Project = self.store.get("projects", field(&payload, "project")?)?;
                let fresh = projects::scan(&self.env(&p.environment)?, &settings, &p.path)?;
                self.store.put("projects", &fresh.id, &fresh)?;
                self.store.delete("updates", &fresh.id)?;
                Ok(json!(fresh))
            }
            "effective_tools" => {
                let p: Project = self.store.get("projects", field(&payload, "project")?)?;
                let env = self.env(&p.environment)?;
                let mise = inventory::manager(&env, &settings, "mise").ok_or("miseが未検出です")?;
                Ok(json!(inventory::parse_mise(
                    &process::checked(&env, &mise, &["ls", "--json"], &p.path)?,
                    &env
                )?))
            }
            "project_diff" => {
                let p: Project = self.store.get("projects", field(&payload, "project")?)?;
                let env = self.env(&p.environment)?;
                let diff = projects::git(
                    &env,
                    &settings,
                    &p.path,
                    &["diff", "--no-ext-diff", "--no-textconv", "HEAD"],
                )?;
                Ok(json!(crate::redact::output(&diff)))
            }
            "plan_pin" => {
                let p: Project = self.store.get("projects", field(&payload, "project")?)?;
                let env = self.env(&p.environment)?;
                let inv: Inventory = self.store.get("inventories", &env.id)?;
                let tool = inv
                    .tools
                    .iter()
                    .find(|t| Some(t.id.as_str()) == payload["tool"].as_str())
                    .ok_or("環境を調査し、インストール済みのツールを選択してください")?;
                self.save_plan(crate::pins::plan(&env, &settings, &p, tool)?)
            }
            "plan_sync" => {
                let p: Project = self.store.get("projects", field(&payload, "project")?)?;
                self.save_plan(crate::git_workflow::sync(
                    &self.env(&p.environment)?,
                    &settings,
                    &p,
                )?)
            }
            "plan_verify_venv" => {
                let p: Project = self.store.get("projects", field(&payload, "project")?)?;
                let index = payload["index"]
                    .as_u64()
                    .ok_or("仮想環境を選択してください")? as usize;
                self.save_plan(crate::pins::verify_venv(
                    &self.env(&p.environment)?,
                    &p,
                    index,
                )?)
            }
            "plan_merge" => {
                let p: Project = self.store.get("projects", field(&payload, "project")?)?;
                self.save_plan(crate::git_workflow::merge(
                    &self.env(&p.environment)?,
                    &settings,
                    &p,
                )?)
            }
            "forget_project" => {
                self.store.delete("projects", field(&payload, "project")?)?;
                Ok(json!(true))
            }
            "save_settings" => {
                let updated: Settings =
                    serde_json::from_value(payload).map_err(|e| e.to_string())?;
                if !updated.same_major {
                    return Err("現在の版ではメジャー更新を有効にできません".into());
                }
                for (env, name, path) in &updated.manager_overrides {
                    self.env(env)?;
                    if !crate::discovery::EXECUTABLES.contains(&name.as_str()) || path.is_empty() {
                        return Err("管理ツール指定が不正です".into());
                    }
                }
                self.store.put("settings", "default", &updated)?;
                Ok(json!(updated))
            }
            "check_updates" => {
                let p: Project = self.store.get("projects", field(&payload, "project")?)?;
                let result = updates::project(&p)?;
                self.store.put("updates", &p.id, &result)?;
                self.store.log(
                    "依存の照合",
                    &p.name,
                    "complete",
                    "候補・未照合・照合失敗の詳細は更新一覧を参照",
                )?;
                Ok(json!(result))
            }
            "check_tool_updates" => {
                let env = self.env(field(&payload, "environment")?)?;
                let inv: Inventory = self.store.get("inventories", &env.id)?;
                let result = plans::tool_updates(&env, &settings, &inv);
                self.store.put("updates", &env.id, &result)?;
                Ok(json!(result))
            }
            "plan_update" => {
                let p: Project = self.store.get("projects", field(&payload, "project")?)?;
                let updates: Vec<Update> = self.store.get("updates", &p.id)?;
                let selected = payload["selected"]
                    .as_array()
                    .ok_or("候補を選択してください")?
                    .iter()
                    .filter_map(Value::as_u64)
                    .map(|v| v as usize)
                    .collect::<Vec<_>>();
                let plan = plans::update(
                    &self.env(&p.environment)?,
                    &settings,
                    &p,
                    &updates,
                    &selected,
                    &array(&payload, "checks"),
                    payload["publish"].as_bool().unwrap_or(false),
                )?;
                self.save_plan(plan)
            }
            "plan_validate" => {
                let p: Project = self.store.get("projects", field(&payload, "project")?)?;
                self.save_plan(plans::validate(
                    &self.env(&p.environment)?,
                    &settings,
                    &p,
                    &array(&payload, "checks"),
                )?)
            }
            "plan_tool" => {
                let env = self.env(field(&payload, "environment")?)?;
                let inv: Inventory = self.store.get("inventories", &env.id)?;
                let id = field(&payload, "tool")?;
                let tool = inv
                    .tools
                    .iter()
                    .find(|t| t.id == id)
                    .ok_or("ツールが見つかりません")?;
                let updates: Vec<Update> = self.store.get("updates", &env.id)?;
                let update = updates
                    .iter()
                    .find(|u| u.tool_id.as_deref() == Some(id))
                    .ok_or("更新照合を先に実行してください")?;
                self.save_plan(plans::tool(&env, &settings, tool, update)?)
            }
            "plan_cleanup" => {
                let env = self.env(field(&payload, "environment")?)?;
                let inv: Inventory = self.store.get("inventories", &env.id)?;
                let tool = inv
                    .tools
                    .iter()
                    .find(|t| Some(t.id.as_str()) == payload["tool"].as_str())
                    .ok_or("対象のツールがありません")?;
                self.save_plan(crate::cleanup::plan(
                    &env,
                    &settings,
                    tool,
                    &self.store.list::<Project>("projects")?,
                )?)
            }
            "execute_step" => self.execute(
                field(&payload, "plan")?,
                payload["confirmed"].as_bool().unwrap_or(false),
            ),
            "cancel" => {
                process::CANCEL.store(true, Ordering::Relaxed);
                Ok(json!(true))
            }
            "github_repos" => {
                let env = self.env("local")?;
                let gh = inventory::manager(&env, &settings, "gh")
                    .ok_or("GitHub CLIが未検出です。gh auth loginで認証してください。")?;
                let out = process::checked(
                    &env,
                    &gh,
                    &[
                        "repo",
                        "list",
                        "--limit",
                        "300",
                        "--json",
                        "nameWithOwner,url,isArchived,defaultBranchRef,description",
                    ],
                    &env.home,
                )?;
                let result: Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
                Ok(result)
            }
            "clone_repo" => {
                let env = self.env("local")?;
                let repo = field(&payload, "repo")?;
                let parts: Vec<_> = repo.split('/').collect();
                if parts.len() != 2
                    || parts.iter().any(|p| {
                        p.is_empty()
                            || p == &".."
                            || !p
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
                    })
                {
                    return Err("owner/repositoryの形式が必要です".into());
                }
                let parent = projects::safe_path(&env, field(&payload, "parent")?)?;
                let dest = process::join(&parent, parts[1], &env);
                if std::path::Path::new(&dest).exists() {
                    return Err(
                        "同名フォルダーが存在します。既存フォルダーを登録してください。".into(),
                    );
                }
                let gh =
                    inventory::manager(&env, &settings, "gh").ok_or("GitHub CLIがありません")?;
                process::checked(&env, &gh, &["repo", "clone", repo, &dest], &parent)?;
                let project = projects::scan(&env, &settings, &dest)?;
                self.store.put("projects", &project.id, &project)?;
                self.store.log("クローン", repo, "complete", &dest)?;
                Ok(json!(project))
            }
            "pr_checks" => {
                let p: Project = self.store.get("projects", field(&payload, "project")?)?;
                let env = self.env(&p.environment)?;
                let gh =
                    inventory::manager(&env, &settings, "gh").ok_or("GitHub CLIがありません")?;
                let out = process::checked(
                    &env,
                    &gh,
                    &[
                        "pr",
                        "view",
                        "--json",
                        "url,headRefOid,statusCheckRollup,state,isDraft",
                    ],
                    &p.path,
                )?;
                let value: Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
                Ok(value)
            }
            _ => Err("未対応の操作です".into()),
        }
    }
    fn save_plan(&self, plan: Plan) -> Result<Value, String> {
        self.store.put("plans", &plan.id, &plan)?;
        self.store
            .log("計画の作成", &plan.title, "ready", &plan.id)?;
        Ok(json!(plan))
    }
    fn execute(&self, id: &str, confirmed: bool) -> Result<Value, String> {
        if !confirmed {
            return Err("実行内容の確認が必要です".into());
        }
        let _guard = self
            .execution
            .try_lock()
            .map_err(|_| "別の更新工程が実行中です")?;
        process::CANCEL.store(false, Ordering::Relaxed);
        let mut plan: Plan = self.store.get("plans", id)?;
        if ["interrupted", "failed", "cancelled"].contains(&plan.status.as_str()) {
            return Err(
                "中断・失敗した計画は再実行しません。差分を確認し再調査・再計画してください。"
                    .into(),
            );
        }
        let index = plan
            .steps
            .iter()
            .position(|s| s.status == "pending")
            .ok_or("すべての工程が完了しています")?;
        if plan.steps[..index].iter().any(|s| s.status != "success") {
            return Err("前の工程が完了していません".into());
        }
        let env = self.env(&plan.environment)?;
        let settings = self.settings();
        let project: Option<Project> = plan
            .project_id
            .as_ref()
            .map(|id| self.store.get("projects", id))
            .transpose()?;
        if let Some(p) = &project {
            plans::assert_fresh(&env, &settings, p, &plan)?;
        }
        let step = plan.steps[index].clone();
        if step.kind == "merge-pr" {
            let p = project.as_ref().ok_or("対象のプロジェクトがありません")?;
            crate::git_workflow::pr_ready(&env, &settings, p)?;
        }
        if step.kind == "tool-remove" {
            let inv: Inventory = self.store.get("inventories", &env.id)?;
            let target = plan.evidence.first().ok_or("削除対象の証拠がありません")?;
            let tool = inv
                .tools
                .iter()
                .find(|t| &t.id == target)
                .ok_or("再調査が必要です")?;
            crate::cleanup::verify(
                &env,
                &settings,
                tool,
                &self.store.list::<Project>("projects")?,
            )?;
        }
        plan.steps[index].status = "running".into();
        plan.status = "running".into();
        self.store.put("plans", id, &plan)?;
        let output = if ["package-manager-pin", "rust-pin"].contains(&step.kind.as_str()) {
            crate::pins::apply(&env, &step)
        } else {
            process::run(&env, &step.program, &step.args, &step.cwd, 900)
        };
        let success = output.as_ref().is_ok_and(|o| o.code == 0);
        let detail = match output {
            Ok(o) => format!("exit={}\n{}\n{}", o.code, o.stdout, o.stderr),
            Err(e) => e,
        };
        let detail = crate::redact::output(&detail);
        plan.steps[index].output = detail.clone();
        plan.steps[index].status = if success { "success" } else { "failed" }.into();
        plan.status = if success {
            if index + 1 == plan.steps.len() {
                "complete"
            } else {
                "ready"
            }
        } else if process::CANCEL.load(Ordering::Relaxed) {
            "cancelled"
        } else {
            "failed"
        }
        .into();
        process::CANCEL.store(false, Ordering::Relaxed);
        if step.kind.starts_with("tool-") {
            self.store
                .put("inventories", &env.id, &inventory::scan(&env, &settings))?;
        }
        if let Some(p) = project {
            match projects::scan(&env, &settings, &p.path) {
                Ok(fresh) => {
                    plan.fingerprint = fresh.fingerprint.clone();
                    self.store.put("projects", &fresh.id, &fresh)?;
                    self.store.delete("updates", &fresh.id)?;
                }
                Err(e) => {
                    plan.status = "interrupted".into();
                    plan.warnings.push(format!("工程後の再調査に失敗：{e}"));
                }
            }
        }
        self.store.put("plans", id, &plan)?;
        self.store
            .log(&step.label, &plan.title, &plan.status, &detail)?;
        Ok(json!(plan))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn app_works_without_managers() {
        let temp = tempfile::tempdir().unwrap();
        let app = Service::new(temp.path().join("state.db")).unwrap();
        let s = app.snapshot().unwrap();
        assert_eq!(s["environments"][0]["id"], "local");
        assert_eq!(s["projects"].as_array().unwrap().len(), 0);
    }
    #[test]
    fn rejects_arbitrary_commands() {
        let temp = tempfile::tempdir().unwrap();
        let app = Service::new(temp.path().join("state.db")).unwrap();
        assert!(app.call("shell", json!({"program":"anything"})).is_err());
    }
    #[test]
    fn confirmation_is_required() {
        let temp = tempfile::tempdir().unwrap();
        let app = Service::new(temp.path().join("state.db")).unwrap();
        assert!(
            app.call("execute_step", json!({"plan":"x","confirmed":false}))
                .unwrap_err()
                .contains("確認")
        );
    }
    #[test]
    fn interrupted_steps_are_never_replayed() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.db");
        let app = Service::new(&path).unwrap();
        let plan = Plan {
            id: "a".into(),
            environment: "local".into(),
            project_id: None,
            title: "test".into(),
            reason: String::new(),
            evidence: vec![],
            warnings: vec![],
            steps: vec![Step {
                id: "b".into(),
                label: "test".into(),
                program: "none".into(),
                args: vec![],
                cwd: String::new(),
                kind: "test".into(),
                risk: "write".into(),
                status: "running".into(),
                output: String::new(),
            }],
            created_at: now(),
            status: "running".into(),
            fingerprint: String::new(),
        };
        app.store.put("plans", "a", &plan).unwrap();
        drop(app);
        let reopened = Service::new(&path).unwrap();
        let plan: Plan = reopened.store.get("plans", "a").unwrap();
        assert_eq!(plan.status, "interrupted");
        assert!(
            reopened
                .call("execute_step", json!({"plan":"a","confirmed":true}))
                .is_err()
        );
    }
}
