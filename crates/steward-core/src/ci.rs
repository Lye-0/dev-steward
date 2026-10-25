//! Static CI/CD inspection. Commands are evidence, never executable project checks.
use crate::models::{Project, Requirement, finding};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CiFile {
    pub path: String,
    pub provider: String,
    pub runtimes: Vec<CiRuntime>,
    pub steps: Vec<CiStep>,
    pub references: Vec<String>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiRuntime {
    pub name: String,
    pub requested: String,
    pub scope: String,
    pub resolved: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiStep {
    pub scope: String,
    pub command: String,
    pub directory: String,
    pub shell: String,
}

pub fn provider(path: &str) -> Option<&'static str> {
    let name = path.rsplit('/').next()?;
    if path.starts_with(".github/workflows/") && (name.ends_with(".yml") || name.ends_with(".yaml"))
    {
        Some("GitHub Actions")
    } else if name == "Dockerfile"
        || name.starts_with("Dockerfile.")
        || name.ends_with(".Dockerfile")
    {
        Some("Docker")
    } else if name == ".gitlab-ci.yml" || name == ".gitlab-ci.yaml" {
        Some("GitLab CI")
    } else if path == ".circleci/config.yml" || path == ".circleci/config.yaml" {
        Some("CircleCI")
    } else if name.starts_with("azure-pipelines")
        && (name.ends_with(".yml") || name.ends_with(".yaml"))
    {
        Some("Azure Pipelines")
    } else if name == "Jenkinsfile" {
        Some("Jenkins")
    } else if name == "vercel.json" {
        Some("Vercel")
    } else if name == "netlify.toml" {
        Some("Netlify")
    } else if path.starts_with(".buildkite/") && (name.ends_with(".yml") || name.ends_with(".yaml"))
    {
        Some("Buildkite")
    } else {
        None
    }
}
fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}
fn safe(s: &str) -> String {
    crate::redact::output(s).chars().take(2000).collect()
}
fn warn(file: &mut CiFile, message: impl Into<String>) {
    let message = message.into();
    if !file.warnings.contains(&message) && file.warnings.len() < 100 {
        file.warnings.push(message);
    }
}
fn runtime(file: &mut CiFile, name: &str, value: &str, scope: &str, resolved: Vec<String>) {
    if value.is_empty() {
        return;
    }
    if file.runtimes.len() >= 200 {
        warn(file, "実行環境指定は先頭200件まで解析しています");
        return;
    }
    file.runtimes.push(CiRuntime {
        name: name.into(),
        requested: safe(value),
        scope: safe(scope),
        resolved: resolved.iter().map(|v| safe(v)).collect(),
    });
    if name == "container" {
        for image in resolved {
            if let Some((base, tag)) = image.split_once(':') {
                let kind = match base {
                    "node" => Some("node"),
                    "python" => Some("python"),
                    "golang" => Some("go"),
                    "rust" => Some("rust"),
                    "eclipse-temurin" => Some("java"),
                    "mcr.microsoft.com/dotnet/sdk" => Some("dotnet"),
                    _ => None,
                };
                if let Some(kind) = kind {
                    runtime(file, kind, &image, scope, vec![tag.into()]);
                }
            }
        }
    }
}
fn command(file: &mut CiFile, scope: &str, value: &str, directory: &str, shell: &str) {
    if value.is_empty() {
        return;
    }
    if file.steps.len() >= 200 {
        warn(file, "実行手順は先頭200件まで解析しています");
        return;
    }
    file.steps.push(CiStep {
        scope: safe(scope),
        command: value
            .lines()
            .map(|line| {
                let lowered = line.to_ascii_lowercase();
                if [
                    "token",
                    "password",
                    "secret",
                    "authorization",
                    "api_key",
                    "api-key",
                ]
                .iter()
                .any(|key| lowered.contains(key))
                {
                    "[機密情報を含む可能性がある行を非表示]".to_string()
                } else {
                    safe(line)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            .chars()
            .take(2000)
            .collect(),
        directory: safe(directory),
        shell: safe(shell),
    });
    if value.chars().count() > 2000 {
        warn(
            file,
            "長いコマンドは2000文字で省略しています。原文の確認が必要です",
        );
    }
    if value.contains("grep")
        || value.contains("--test-reporter")
        || value.contains("tee ")
        || value.contains("junit")
        || value.contains("test-results")
    {
        warn(
            file,
            "テスト出力・レポートを扱う手順があります。ランタイム更新時はテストだけでなく集計・成果物作成まで検証してください",
        );
    }
}
fn resolve(value: &str, root: &Value, job: &Value, step: &Value) -> Vec<String> {
    let pattern = regex::Regex::new(r"^\$\{\{\s*(matrix|env)\.([A-Za-z0-9_-]+)\s*\}\}$").unwrap();
    if let Some(c) = pattern.captures(value.trim()) {
        let key = &c[2];
        let mut values = vec![];
        if &c[1] == "matrix" {
            if let Some(axis) = job["strategy"]["matrix"][key].as_array() {
                values.extend(
                    axis.iter()
                        .filter(|v| !v.is_f64())
                        .map(scalar)
                        .filter(|s| !s.is_empty()),
                );
            }
            if let Some(includes) = job["strategy"]["matrix"]["include"].as_array() {
                values.extend(
                    includes
                        .iter()
                        .filter(|v| !v[key].is_f64())
                        .map(|v| scalar(&v[key]))
                        .filter(|s| !s.is_empty()),
                );
            }
        } else {
            let env = [&step["env"][key], &job["env"][key], &root["env"][key]]
                .into_iter()
                .find(|v| !v.is_null())
                .map(|v| if v.is_f64() { String::new() } else { scalar(v) })
                .unwrap_or_default();
            if !env.is_empty() {
                values.push(env);
            }
        }
        values.retain(|v| !v.contains("${"));
        values.sort();
        values.dedup();
        values.into_iter().map(|v| safe(&v)).collect()
    } else if value.contains("${")
        || value.contains("$[")
        || value.starts_with('$')
        || value.contains("$(")
    {
        vec![]
    } else {
        vec![safe(value)]
    }
}
fn action(
    file: &mut CiFile,
    requirements: &mut Vec<Requirement>,
    scope: &str,
    item: &Value,
    root: &Value,
    job: &Value,
) {
    let uses = scalar(&item["uses"]);
    if !uses.is_empty() {
        if let Some((name, version)) = uses.rsplit_once('@') {
            requirements.push(Requirement {
                manager: "github-actions".into(),
                name: safe(name),
                requested: safe(version),
                source: file.path.clone(),
                section: format!("workflow:{scope}"),
                resolved: String::new(),
            });
        }
        if file.references.len() >= 200 {
            warn(file, "Actionの参照は先頭200件まで解析しています");
            return;
        }
        file.references.push(safe(&uses));
        if uses.starts_with("./") || uses.contains("/.github/workflows/") {
            warn(
                file,
                "ローカルAction・再利用ワークフローは参照を記録します。呼び出し先の入力・実行環境を含む評価は未対応です",
            );
        }
        let action_name = uses.split('@').next().unwrap_or("");
        let setup = match action_name {
            "actions/setup-node" => Some(("node", "node-version", "node-version-file")),
            "actions/setup-python" => Some(("python", "python-version", "python-version-file")),
            "actions/setup-java" => Some(("java", "java-version", "java-version-file")),
            "actions/setup-go" => Some(("go", "go-version", "go-version-file")),
            "actions/setup-dotnet" => Some(("dotnet", "dotnet-version", "global-json-file")),
            "pnpm/action-setup" => Some(("pnpm", "version", "")),
            "oven-sh/setup-bun" => Some(("bun", "bun-version", "bun-version-file")),
            "astral-sh/setup-uv" => Some(("uv", "version", "version-file")),
            _ => None,
        };
        if let Some((name, key, reference_key)) = setup {
            let value = scalar(&item["with"][key]);
            if item["with"][key].is_f64() {
                warn(
                    file,
                    format!(
                        "{scope}: 小数のバージョン指定はYAMLで桁が失われる場合があります。引用符付きの文字列を使用してください"
                    ),
                );
            }
            runtime(
                file,
                name,
                &value,
                scope,
                if item["with"][key].is_f64() {
                    vec![]
                } else {
                    resolve(&value, root, job, item)
                },
            );
            let reference = scalar(&item["with"][reference_key]);
            if !reference.is_empty() {
                if value.is_empty() {
                    file.references
                        .push(format!("{name} version-file:{reference}"));
                } else {
                    file.references.push(format!(
                        "参照のみ（版の直接指定を優先）: {name} {reference}"
                    ));
                }
            }
            if value.is_empty() && reference.is_empty() {
                warn(
                    file,
                    format!(
                        "{scope}: {name}の版は明示されていません。既定値・参照ファイルの確認が必要です"
                    ),
                );
            }
        }
        if action_name == "jdx/mise-action"
            && item["with"]["install"] != false
            && scalar(&item["with"]["install"]) != "false"
        {
            let directory = scalar(&item["with"]["working_directory"]);
            let inline = [
                ("mise.toml", scalar(&item["with"]["mise_toml"])),
                (".tool-versions", scalar(&item["with"]["tool_versions"])),
            ];
            if inline.iter().any(|(_, value)| !value.is_empty()) {
                for (filename, text) in inline {
                    if text.is_empty() {
                        continue;
                    }
                    if text.contains("${{") {
                        warn(
                            file,
                            "mise Actionのインライン設定に動的な式があります。実行時の設定を確認してください",
                        );
                        continue;
                    }
                    let mut rows = vec![];
                    if crate::projects::parse_manifest(
                        filename,
                        &text,
                        &mut rows,
                        &mut vec![],
                        &mut String::new(),
                    )
                    .is_err()
                    {
                        warn(file, "mise Actionのインライン設定を解析できません");
                        continue;
                    }
                    for row in rows {
                        runtime(
                            file,
                            &row.name,
                            &row.requested,
                            scope,
                            resolve(&row.requested, root, job, item),
                        );
                    }
                }
            } else {
                file.references
                    .push(format!("mise-config:{}", directory.trim_end_matches('/')));
            }
            warn(
                file,
                "mise Actionは設定候補の静的参照です。インストール対象の絞り込み・キャッシュ・グローバル設定・monorepoの展開は実行環境で確認してください",
            );
        }
        if action_name == "dtolnay/rust-toolchain"
            || action_name == "actions-rust-lang/setup-rust-toolchain"
        {
            let with = scalar(&item["with"]["toolchain"]);
            let value = if with.is_empty() && action_name == "dtolnay/rust-toolchain" {
                uses.rsplit('@').next().unwrap_or("").into()
            } else {
                with
            };
            runtime(
                file,
                "rust",
                &value,
                scope,
                resolve(&value, root, job, item),
            );
        }
        if !matches!(
            action_name,
            "actions/checkout" | "actions/upload-artifact" | "actions/download-artifact"
        ) && setup.is_none()
        {
            warn(
                file,
                format!(
                    "{scope}: Action内部のランタイムや導入処理は参照先で確認してください: {}",
                    safe(action_name)
                ),
            );
        }
    }
}
fn github(file: &mut CiFile, root: &Value, requirements: &mut Vec<Requirement>) {
    let Some(jobs) = root["jobs"].as_object() else {
        warn(
            file,
            "jobsを解析できません。外部テンプレート等を確認してください",
        );
        return;
    };
    for (name, job) in jobs {
        if !job["strategy"]["matrix"].is_null() {
            warn(
                file,
                "matrixの小数値はバージョン文字列として確定しません。Python等は引用符付きで指定してください",
            );
            warn(
                file,
                format!(
                    "{name}: matrixは宣言候補を列挙します。include/exclude・条件式による実際の実行組み合わせは未検証です"
                ),
            );
        }
        runtime(
            file,
            "runner",
            &scalar(&job["runs-on"]),
            name,
            resolve(&scalar(&job["runs-on"]), root, job, &Value::Null),
        );
        let image = if job["container"].is_string() {
            scalar(&job["container"])
        } else {
            scalar(&job["container"]["image"])
        };
        runtime(
            file,
            "container",
            &image,
            name,
            resolve(&image, root, job, &Value::Null),
        );
        action(file, requirements, name, job, root, job);
        if let Some(steps) = job["steps"].as_array() {
            for (i, step) in steps.iter().enumerate() {
                let scope = format!("{name} / {}", i + 1);
                action(file, requirements, &scope, step, root, job);
                let directory = [
                    &step["working-directory"],
                    &job["defaults"]["run"]["working-directory"],
                    &root["defaults"]["run"]["working-directory"],
                ]
                .into_iter()
                .map(scalar)
                .find(|s| !s.is_empty())
                .unwrap_or_default();
                let shell = [
                    &step["shell"],
                    &job["defaults"]["run"]["shell"],
                    &root["defaults"]["run"]["shell"],
                ]
                .into_iter()
                .map(scalar)
                .find(|s| !s.is_empty())
                .unwrap_or_default();
                command(file, &scope, &scalar(&step["run"]), &directory, &shell);
                if !step["if"].is_null() {
                    warn(file, format!("{scope}: 実行条件の真偽は未評価です"));
                }
            }
        }
    }
}
fn runtime_key(key: &str) -> Option<&'static str> {
    match key {
        "NODE_VERSION" | "node-version" => Some("node"),
        "PYTHON_VERSION" | "python-version" => Some("python"),
        "GO_VERSION" | "go-version" => Some("go"),
        "JAVA_VERSION" | "java-version" => Some("java"),
        "PNPM_VERSION" => Some("pnpm"),
        "RUST_VERSION" => Some("rust"),
        _ => None,
    }
}
// Generic providers: preserve scoped literals and ordered arrays without pretending
// to evaluate their template languages, shells, or inherited configuration.
fn walk(file: &mut CiFile, value: &Value, scope: &str, depth: usize) {
    if depth > 24 {
        warn(file, "設定の深さ上限に達しました。残りは要確認です");
        return;
    }
    if let Some(object) = value.as_object() {
        if let Some(task) = object.get("task").and_then(Value::as_str) {
            let setup = if task.starts_with("NodeTool@") || task.starts_with("UseNode@") {
                Some(("node", "versionSpec"))
            } else if task.starts_with("UsePythonVersion@") {
                Some(("python", "versionSpec"))
            } else {
                None
            };
            if let Some((name, key)) = setup {
                let version = scalar(&value["inputs"][key]);
                let version = if version.is_empty() {
                    scalar(&value["inputs"]["version"])
                } else {
                    version
                };
                runtime(
                    file,
                    name,
                    &version,
                    scope,
                    resolve(&version, &Value::Null, &Value::Null, &Value::Null),
                );
            }
        }
        for (key, v) in object {
            let location = format!("{scope}/{key}");
            if let Some(name) = runtime_key(key) {
                let version = scalar(v);
                runtime(
                    file,
                    name,
                    &version,
                    &location,
                    resolve(&version, &Value::Null, &Value::Null, &Value::Null),
                );
            }
            if key == "image" {
                let image = if v.is_object() {
                    scalar(&v["name"])
                } else {
                    scalar(v)
                };
                runtime(
                    file,
                    "container",
                    &image,
                    &location,
                    resolve(&image, &Value::Null, &Value::Null, &Value::Null),
                );
            }
            if [
                "script",
                "before_script",
                "after_script",
                "command",
                "run",
                "buildCommand",
                "installCommand",
                "testCommand",
                "bash",
                "powershell",
                "pwsh",
            ]
            .contains(&key.as_str())
            {
                if let Some(items) = v.as_array() {
                    for (i, item) in items.iter().enumerate() {
                        command(
                            file,
                            &format!("{location}/{}", i + 1),
                            &scalar(item),
                            "",
                            "",
                        );
                    }
                } else {
                    command(file, &location, &scalar(v), "", "");
                }
            }
            if ["include", "extends", "template", "orbs", "executors"].contains(&key.as_str()) {
                warn(
                    file,
                    format!("{location}: 継承・外部設定の完全な展開は未対応です"),
                );
            }
            walk(file, v, &location, depth + 1);
        }
    } else if let Some(items) = value.as_array() {
        for (i, item) in items.iter().enumerate() {
            walk(file, item, &format!("{scope}/{}", i + 1), depth + 1);
        }
    }
}
fn docker(file: &mut CiFile, text: &str) {
    let arg_pattern =
        regex::Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let mut args = std::collections::BTreeMap::new();
    for (i, line) in text.lines().enumerate() {
        let Some((instruction, rest)) = line.trim().split_once(char::is_whitespace) else {
            continue;
        };
        let scope = format!("line {}", i + 1);
        let rest = rest.trim();
        match instruction.to_ascii_uppercase().as_str() {
            "ARG" => {
                if let Some((name, value)) = rest.split_once('=') {
                    args.insert(name.to_string(), value.to_string());
                }
            }
            "FROM" => {
                let image = rest
                    .split_whitespace()
                    .find(|v| !v.starts_with("--"))
                    .unwrap_or("");
                let resolved = arg_pattern
                    .replace_all(image, |c: &regex::Captures<'_>| {
                        args.get(c.get(1).or_else(|| c.get(2)).unwrap().as_str())
                            .cloned()
                            .unwrap_or_else(|| c[0].into())
                    })
                    .into_owned();
                runtime(
                    file,
                    "container",
                    image,
                    &scope,
                    if resolved.contains('$') {
                        vec![]
                    } else {
                        vec![resolved]
                    },
                );
            }
            "RUN" => command(file, &scope, rest, "", "Docker RUN"),
            _ => {}
        }
    }
    warn(
        file,
        "DockerのARGはファイル内の既定値だけを参照します。ビルド引数・複数行・ステージ間の継承は原文で確認してください",
    );
}
pub fn parse(
    path: &str,
    text: &str,
    requirements: &mut Vec<Requirement>,
) -> Result<CiFile, String> {
    let provider = provider(path).ok_or("未対応のCI/CDファイルです")?;
    let mut file = CiFile {
        path: path.into(),
        provider: provider.into(),
        ..Default::default()
    };
    match provider {
        "Docker" => docker(&mut file, text),
        "Jenkins" => {
            let tools =
                regex::Regex::new(r#"(?m)\b(nodejs|jdk|python|golang)\s+['\"]([^'\"]+)['\"]"#)
                    .unwrap();
            for c in tools.captures_iter(text) {
                runtime(
                    &mut file,
                    if &c[1] == "nodejs" { "node" } else { &c[1] },
                    &c[2],
                    "tools",
                    vec![],
                );
            }
            warn(
                &mut file,
                "JenkinsのGroovyは評価しません。toolsのラベルは実際のバージョンとは限りません。ステージ・共有ライブラリ・実行手順は原文で確認してください",
            );
        }
        _ => {
            let value: Value = if provider == "Netlify" {
                let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
                serde_json::to_value(value).map_err(|e| e.to_string())?
            } else if provider == "Vercel" {
                serde_json::from_str(text).map_err(|e| e.to_string())?
            } else {
                serde_saphyr::from_str(text).map_err(|e| e.to_string())?
            };
            if provider == "GitHub Actions" {
                github(&mut file, &value, requirements)
            } else {
                walk(&mut file, &value, "", 0);
                warn(
                    &mut file,
                    "宣言された実行環境・コマンドの静的抽出です。継承・実行順・条件・クラウド側の設定は完全には評価していません",
                );
            }
        }
    }
    for r in file.runtimes.clone() {
        if r.resolved.is_empty() {
            warn(
                &mut file,
                format!(
                    "{}: {} {}を確定できません。式・外部値・参照先を確認してください",
                    r.scope, r.name, r.requested
                ),
            );
        }
    }
    Ok(file)
}

pub fn finish(project: &mut Project) {
    let series_pattern = regex::Regex::new(
        r"^[v~^]?(\d+)(?:\.(\d+|[xX*]))?(?:\.(?:\d+|[xX*]))?(?:[-+][0-9A-Za-z.+-]+)?$",
    )
    .unwrap();
    for file in &mut project.ci {
        for reference in file.references.clone() {
            if let Some(directory) = reference.strip_prefix("mise-config:") {
                let prefix = if directory.is_empty() || directory == "." {
                    String::new()
                } else {
                    format!("{}/", directory.trim_start_matches("./"))
                };
                let sources = [
                    format!("{prefix}mise.toml"),
                    format!("{prefix}.mise.toml"),
                    format!("{prefix}.tool-versions"),
                ];
                let found: Vec<_> = project
                    .requirements
                    .iter()
                    .filter(|r| r.manager == "mise" && sources.contains(&r.source))
                    .cloned()
                    .collect();
                if found.is_empty() {
                    warn(
                        file,
                        "mise Actionが参照する設定を解決できません。作業場所・動的な指定・設定ファイルの取得範囲を確認してください",
                    );
                }
                for row in found {
                    runtime(
                        file,
                        &row.name,
                        &format!("{} ({})", row.requested, row.source),
                        "mise action",
                        resolve(&row.requested, &Value::Null, &Value::Null, &Value::Null),
                    );
                }
            }
            if let Some((name, path)) = reference.split_once(" version-file:") {
                let path = path.trim_start_matches("./");
                let found: Vec<_> = project
                    .requirements
                    .iter()
                    .filter(|r| r.name == name && r.source == path && r.section != "locked")
                    .map(|r| r.requested.clone())
                    .collect();
                if found.is_empty() {
                    warn(
                        file,
                        format!(
                            "参照ファイルを解決できません: {path}。未取得・未対応形式・動的な指定を確認してください"
                        ),
                    );
                } else {
                    runtime(file, name, &reference, "version-file", found);
                }
            }
        }
        for r in &file.runtimes {
            project.requirements.push(Requirement {
                manager: "ci-runtime".into(),
                name: r.name.clone(),
                requested: r.requested.clone(),
                source: file.path.clone(),
                section: format!("ci:{}", r.scope),
                resolved: r.resolved.join(" / "),
            });
        }
        for r in file.runtimes.clone() {
            let local: Vec<_> = project
                .requirements
                .iter()
                .filter(|q| {
                    q.name == r.name
                        && ["pin", "tools", "engines", "packageManager"]
                            .contains(&q.section.as_str())
                })
                .collect();
            for q in local {
                if series_pattern.captures(q.requested.trim()).is_none() && !r.resolved.is_empty() {
                    warn(
                        file,
                        format!(
                            "版の比較は要確認: {} の {} 指定 {} は範囲・別形式です。CI候補 {} が許容範囲か確認してください",
                            q.source,
                            q.name,
                            q.requested,
                            r.resolved.join(" / ")
                        ),
                    );
                }
                for version in &r.resolved {
                    let series = |s: &str| {
                        series_pattern.captures(s.trim()).map(|c| {
                            (
                                c[1].to_string(),
                                c.get(2).and_then(|v| v.as_str().parse::<u64>().ok()),
                            )
                        })
                    };
                    if let (Some(a), Some(b)) = (series(version), series(&q.requested))
                        && (a.0 != b.0
                            || (r.name == "python" && a.1.is_some() && b.1.is_some() && a.1 != b.1))
                    {
                        warn(
                            file,
                            format!(
                                "実行環境の相違: CIの{} {}（{}）と {} の指定 {} が異なります。意図したテスト対象か確認し、単純に統一しないでください",
                                r.name, version, r.scope, q.source, q.requested
                            ),
                        );
                    }
                }
            }
        }
        if !file.steps.is_empty() {
            warn(
                file,
                "表示された手順は実行していません。CIと同じ環境・作業場所でインストール→検証→集計→成果物作成まで確認してください",
            );
        }
        for warning in &file.warnings {
            project
                .findings
                .push(finding(&file.path, "partial", warning));
        }
    }
}
pub fn plan_warnings(project: &Project) -> Vec<String> {
    let mut notes = vec![];
    for file in &project.ci {
        notes.push(format!("CI/CD確認: {} ({})。ランタイム・コンテナーと検証から集計までの手順を変更後のコミットで検証してください",file.path,file.provider));
        notes.extend(file.runtimes.iter().map(|r| {
            format!(
                "{} / {}: {} {} → {}",
                file.path,
                r.scope,
                r.name,
                r.requested,
                if r.resolved.is_empty() {
                    "要確認".into()
                } else {
                    r.resolved.join(" / ")
                }
            )
        }));
        notes.extend(
            file.warnings
                .iter()
                .map(|w| format!("{}: {}", file.path, w)),
        );
    }
    if notes.len() > 40 {
        let remaining = notes.len() - 40;
        notes.truncate(40);
        notes.push(format!("CI/CD確認事項はほか{remaining}件あります。プロジェクトの解析結果で全件を確認してください"));
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inspect(path: &str, text: &str) -> CiFile {
        parse(path, text, &mut vec![]).unwrap()
    }
    #[test]
    fn github_matrix_env_and_step_scope_are_preserved() {
        let f = inspect(
            ".github/workflows/check.yml",
            r#"
env:
  PY: '3.12'
jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        node: [22, 24]
        include:
          - node: 20
    defaults:
      run:
        working-directory: apps/web
        shell: bash
    steps:
      - uses: actions/setup-node@v6
        with:
          node-version: ${{ matrix.node }}
      - uses: actions/setup-python@v6
        with:
          python-version: ${{ env.PY }}
      - run: npm test | tee results.txt
      - run: grep '^# tests' results.txt
"#,
        );
        assert_eq!(
            f.runtimes
                .iter()
                .find(|r| r.name == "node")
                .unwrap()
                .resolved,
            vec!["20", "22", "24"]
        );
        assert_eq!(
            f.runtimes
                .iter()
                .find(|r| r.name == "python")
                .unwrap()
                .resolved,
            vec!["3.12"]
        );
        assert_eq!(f.steps[0].directory, "apps/web");
        assert_eq!(f.steps[0].shell, "bash");
        assert!(f.steps[1].command.contains("grep"));
        assert!(f.warnings.iter().any(|w| w.contains("集計")));
    }
    #[test]
    fn dynamic_expressions_and_unquoted_float_versions_are_unresolved() {
        let f = inspect(
            ".github/workflows/check.yml",
            r#"
jobs:
  test:
    steps:
      - uses: actions/setup-node@v6
        with:
          node-version: ${{ inputs.node }}
      - uses: actions/setup-python@v6
        with:
          python-version: 3.10
"#,
        );
        assert!(f.runtimes.iter().all(|r| r.resolved.is_empty()));
        assert!(f.warnings.iter().any(|w| w.contains("引用符")));
    }
    #[test]
    fn docker_arguments_images_and_run_commands_are_separate() {
        let f = inspect(
            "apps/web/Dockerfile.release",
            "ARG NODE_VERSION=22-alpine\nFROM node:${NODE_VERSION} AS build\nRUN npm ci\nFROM custom:${BUILD_VERSION}\n",
        );
        assert!(
            f.runtimes
                .iter()
                .any(|r| r.name == "node" && r.resolved == vec!["22-alpine"])
        );
        assert!(
            f.runtimes
                .iter()
                .any(|r| r.requested == "custom:${BUILD_VERSION}" && r.resolved.is_empty())
        );
        assert_eq!(f.steps[0].command, "npm ci");
        let plain = inspect("Dockerfile", "ARG NODE=24\nFROM node:$NODE\n");
        assert!(
            plain
                .runtimes
                .iter()
                .any(|r| r.name == "node" && r.resolved == vec!["24"])
        );
        let unsupported = inspect("Dockerfile", "ARG NODE=24\nFROM node:${NODE:-22}\n");
        assert!(unsupported.runtimes[0].resolved.is_empty());
    }
    #[test]
    fn generic_ci_providers_collect_runtime_and_checks_without_execution() {
        for (path, text, provider) in [
            (
                ".gitlab-ci.yml",
                "image: node:22\ninclude: remote.yml\ntest:\n  script: ['npm ci','npm test']\n",
                "GitLab CI",
            ),
            (
                ".circleci/config.yml",
                "version: 2.1\njobs:\n  test:\n    docker: [{image: 'node:22'}]\n    steps: [{run: 'npm test'}]\n",
                "CircleCI",
            ),
            (
                "azure-pipelines.yml",
                "steps:\n  - task: NodeTool@0\n    inputs: {versionSpec: '22.x'}\n  - script: npm test\n",
                "Azure Pipelines",
            ),
            (
                "netlify.toml",
                "[build]\ncommand='npm run build'\n[build.environment]\nNODE_VERSION='22'\n",
                "Netlify",
            ),
            (
                "vercel.json",
                r#"{"buildCommand":"npm run build"}"#,
                "Vercel",
            ),
            (
                ".buildkite/pipeline.yml",
                "steps:\n  - command: npm test\n",
                "Buildkite",
            ),
        ] {
            let file = inspect(path, text);
            assert_eq!(file.provider, provider);
            assert!(!file.steps.is_empty(), "{path}");
            assert!(!file.warnings.is_empty());
        }
    }
    #[test]
    fn jenkins_labels_are_not_versions_and_invalid_yaml_is_not_success() {
        let file = inspect("Jenkinsfile", "pipeline { tools { nodejs 'node-22' } }");
        assert!(file.runtimes[0].resolved.is_empty());
        assert!(parse(".github/workflows/broken.yml", "jobs: [", &mut vec![]).is_err());
        assert!(provider("README.md").is_none());
    }
    #[test]
    fn secrets_in_commands_and_env_are_not_recorded_wholesale() {
        let file = inspect(
            ".github/workflows/check.yml",
            r#"
env:
  TOKEN: never-store-env-secret
jobs:
  test:
    steps:
      - run: |
          curl -H 'Authorization: Bearer never-store-command-secret' https://example.invalid
      - run: npm test
"#,
        );
        let output = serde_json::to_string(&file).unwrap();
        assert!(!output.contains("never-store"));
        assert_eq!(file.steps[1].command, "npm test");
    }
    #[test]
    fn mise_inline_configuration_is_inspected_and_install_false_is_respected() {
        let file = inspect(
            ".github/workflows/check.yml",
            r#"
jobs:
  test:
    steps:
      - uses: jdx/mise-action@v5
        with:
          mise_toml: |
            [tools]
            node = '24.22.1'
            pnpm = '11.28.8'
      - uses: jdx/mise-action@v5
        with:
          install: false
          tool_versions: node 18
"#,
        );
        assert!(
            file.runtimes
                .iter()
                .any(|r| r.name == "node" && r.resolved == vec!["24.22.1"])
        );
        assert!(!file.runtimes.iter().any(|r| r.resolved == vec!["18"]));
    }
    #[test]
    fn explicit_version_takes_precedence_and_env_shadowing_is_not_ignored() {
        let file = inspect(
            ".github/workflows/check.yml",
            r#"
env:
  PY: '3.12'
jobs:
  test:
    steps:
      - uses: actions/setup-node@v6
        with:
          node-version: '24'
          node-version-file: .node-version
      - uses: actions/setup-python@v6
        env:
          PY: ''
        with:
          python-version: ${{ env.PY }}
"#,
        );
        assert_eq!(file.runtimes[0].resolved, vec!["24"]);
        assert!(file.runtimes[1].resolved.is_empty());
        assert!(
            !file
                .references
                .iter()
                .any(|r| r.starts_with("node version-file:"))
        );
    }
}
