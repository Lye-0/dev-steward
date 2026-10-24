use crate::{inventory::manager, models::*, process::*};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};
use walkdir::WalkDir;

pub fn git(
    env: &Environment,
    settings: &Settings,
    path: &str,
    args: &[&str],
) -> Result<String, String> {
    let exe = manager(env, settings, "git").ok_or("Gitが未検出です")?;
    let mut all = vec![
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.hooksPath=/dev/null",
    ];
    all.extend_from_slice(args);
    checked(env, &exe, &all, path)
}

pub fn safe_path(env: &Environment, path: &str) -> Result<String, String> {
    if path.contains('\0') || path.contains('\n') || path.contains('\r') {
        return Err("パスに制御文字は使用できません".into());
    }
    if env.kind == "wsl" {
        if !path.starts_with('/') {
            return Err("WSLの絶対パスを指定してください".into());
        }
        checked(env, "/usr/bin/realpath", &["--", path], &env.home)
    } else {
        let p = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        if !p.is_dir() {
            return Err("フォルダーを指定してください".into());
        }
        Ok(p.to_string_lossy().to_string())
    }
}

fn ignored(name: &str) -> bool {
    [
        "node_modules",
        ".git",
        "target",
        "dist",
        "build",
        "site-packages",
        ".next",
        ".kilo",
        "__pycache__",
        "models",
        "datasets",
    ]
    .contains(&name)
}

pub fn discover(env: &Environment, settings: &Settings, root: &str) -> Result<Vec<String>, String> {
    let root = safe_path(env, root)?;
    if git(env, settings, &root, &["rev-parse", "--show-toplevel"])
        .is_ok_and(|s| normalize(&s, env) == normalize(&root, env))
    {
        return Ok(vec![root]);
    }
    let mut found = BTreeSet::new();
    if env.kind == "wsl" {
        let out = checked(
            env,
            "/usr/bin/find",
            &[
                &root,
                "-maxdepth",
                "7",
                "(",
                "-name",
                "node_modules",
                "-o",
                "-name",
                "target",
                "-o",
                "-name",
                ".kilo",
                "-o",
                "-name",
                ".venv",
                ")",
                "-prune",
                "-o",
                "-name",
                ".git",
                "-print",
                "-prune",
            ],
            &env.home,
        )?;
        for p in out.lines() {
            if let Some(parent) = p.strip_suffix("/.git") {
                found.insert(parent.to_string());
            }
        }
    } else {
        for entry in WalkDir::new(&root)
            .max_depth(7)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !ignored(&e.file_name().to_string_lossy()))
        {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().is_dir() && entry.path().join(".git").exists() {
                found.insert(entry.path().to_string_lossy().to_string());
            }
            if found.len() > 300 {
                return Err("300件を超えました。調査フォルダーを絞ってください。".into());
            }
        }
    }
    Ok(found.into_iter().collect())
}

fn normalize(s: &str, env: &Environment) -> String {
    let value = s
        .trim_start_matches("\\\\?\\")
        .trim_end_matches(['/', '\\'])
        .replace('\\', "/");
    if env.os == "windows" {
        value.to_lowercase()
    } else {
        value
    }
}

pub fn manifest(name: &str) -> bool {
    [
        "package.json",
        "pnpm-lock.yaml",
        "package-lock.json",
        "yarn.lock",
        "pnpm-workspace.yaml",
        "pyproject.toml",
        "uv.lock",
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "mise.toml",
        ".mise.toml",
        ".tool-versions",
        ".python-version",
        ".node-version",
        ".nvmrc",
        "requirements.txt",
        "pyvenv.cfg",
        ".npmrc",
    ]
    .contains(&name)
}

fn req(
    manager: &str,
    name: &str,
    requested: &str,
    source: &str,
    section: &str,
    resolved: &str,
) -> Requirement {
    Requirement {
        manager: manager.into(),
        name: name.into(),
        requested: requested.into(),
        source: source.into(),
        section: section.into(),
        resolved: resolved.into(),
    }
}

pub fn parse_manifest(
    source: &str,
    text: &str,
    requirements: &mut Vec<Requirement>,
    scripts: &mut Vec<String>,
    package_manager: &mut String,
) -> Result<(), String> {
    let name = source.rsplit(['/', '\\']).next().unwrap_or(source);
    let text = text.trim_start_matches('\u{feff}');
    match name {
        ".npmrc" => {
            for line in text
                .lines()
                .map(str::trim)
                .filter(|l| !l.starts_with('#') && !l.starts_with(';'))
            {
                if let Some((key, value)) = line.split_once('=')
                    && (key.trim() == "registry" || key.trim().ends_with(":registry"))
                {
                    requirements.push(req(
                        "registry",
                        key.trim(),
                        &redact_remote(value.trim()),
                        source,
                        "registry",
                        "",
                    ));
                }
            }
        }
        "package.json" => {
            let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
            if source == "package.json" {
                *package_manager = v["packageManager"].as_str().unwrap_or_default().into();
            }
            if let Some(o) = v["scripts"].as_object() {
                for key in o.keys().filter(|k| {
                    ["test", "build", "lint", "typecheck", "check"].contains(&k.as_str())
                }) {
                    let prefix = source
                        .rsplit_once('/')
                        .map(|(p, _)| format!("{p}::"))
                        .unwrap_or_default();
                    scripts.push(format!("{prefix}{key}"));
                }
            }
            for section in [
                "dependencies",
                "devDependencies",
                "optionalDependencies",
                "peerDependencies",
                "engines",
            ] {
                if let Some(o) = v[section].as_object() {
                    for (n, ver) in o {
                        if let Some(ver) = ver.as_str() {
                            requirements.push(req(
                                if section == "engines" {
                                    "engine"
                                } else {
                                    "npm"
                                },
                                n,
                                ver,
                                source,
                                section,
                                "",
                            ));
                        }
                    }
                }
            }
            if let Some(pm) = v["packageManager"].as_str() {
                requirements.push(req(
                    "packageManager",
                    pm.split('@').next().unwrap_or(pm),
                    pm,
                    source,
                    "packageManager",
                    "",
                ));
            }
        }
        "mise.toml" | ".mise.toml" => {
            let v: toml::Value = toml::from_str::<toml::Value>(text).map_err(|e| e.to_string())?;
            if let Some(tools) = v.get("tools").and_then(toml::Value::as_table) {
                for (n, ver) in tools {
                    let value = ver
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| ver.to_string());
                    requirements.push(req("mise", n, &value, source, "tools", ""));
                }
            }
        }
        "pyproject.toml" => {
            let v: toml::Value = toml::from_str::<toml::Value>(text).map_err(|e| e.to_string())?;
            if let Some(p) = v.get("project") {
                if let Some(version) = p.get("requires-python").and_then(toml::Value::as_str) {
                    requirements.push(req(
                        "engine",
                        "python",
                        version,
                        source,
                        "requires-python",
                        "",
                    ));
                }
                if let Some(deps) = p.get("dependencies").and_then(toml::Value::as_array) {
                    parse_python_deps(deps, source, "dependencies", requirements);
                }
                if let Some(groups) = p
                    .get("optional-dependencies")
                    .and_then(toml::Value::as_table)
                {
                    for (group, deps) in groups {
                        if let Some(deps) = deps.as_array() {
                            parse_python_deps(deps, source, group, requirements);
                        }
                    }
                }
            }
            if let Some(groups) = v.get("dependency-groups").and_then(toml::Value::as_table) {
                for (group, deps) in groups {
                    if let Some(deps) = deps.as_array() {
                        parse_python_deps(deps, source, group, requirements);
                    }
                }
            }
        }
        "Cargo.toml" => {
            let v: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
            for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
                if let Some(items) = v.get(section).and_then(toml::Value::as_table) {
                    for (name, value) in items {
                        let version = value
                            .as_str()
                            .or_else(|| value.get("version").and_then(toml::Value::as_str));
                        if value.get("git").is_none()
                            && value.get("path").is_none()
                            && value.get("registry").is_none()
                            && let Some(version) = version
                        {
                            let package = value
                                .get("package")
                                .and_then(toml::Value::as_str)
                                .unwrap_or(name);
                            requirements.push(req("cargo", package, version, source, section, ""));
                        }
                    }
                }
            }
            if source == "Cargo.toml" {
                for name in ["cargo-check", "cargo-test"] {
                    scripts.push(name.into());
                }
            }
        }
        "Cargo.lock" => {
            let v: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
            if let Some(packages) = v.get("package").and_then(toml::Value::as_array) {
                for package in packages {
                    if let (Some(name), Some(version), Some(registry)) = (
                        package.get("name").and_then(toml::Value::as_str),
                        package.get("version").and_then(toml::Value::as_str),
                        package.get("source").and_then(toml::Value::as_str),
                    ) && (registry == "registry+https://github.com/rust-lang/crates.io-index"
                        || registry == "sparse+https://index.crates.io/")
                    {
                        requirements.push(req("cargo", name, version, source, "locked", version));
                    }
                }
            }
        }
        "uv.lock" => {
            let v: toml::Value = toml::from_str::<toml::Value>(text).map_err(|e| e.to_string())?;
            if let Some(packages) = v.get("package").and_then(toml::Value::as_array) {
                for p in packages {
                    if let (Some(n), Some(ver)) = (
                        p.get("name").and_then(toml::Value::as_str),
                        p.get("version").and_then(toml::Value::as_str),
                    ) {
                        // Local/path/VCS packages must not be treated as PyPI releases.
                        if p.get("source")
                            .and_then(|s| s.get("registry"))
                            .and_then(toml::Value::as_str)
                            .is_some_and(|s| s.trim_end_matches('/') == "https://pypi.org/simple")
                        {
                            requirements.push(req("pypi", n, ver, source, "locked", ver));
                        }
                    }
                }
            }
        }
        "package-lock.json" => {
            let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
            if let Some(packages) = v["packages"].as_object() {
                for (path, p) in packages {
                    if let (Some((_, n)), Some(ver)) =
                        (path.rsplit_once("node_modules/"), p["version"].as_str())
                    {
                        requirements.push(req("npm", n, ver, source, "locked", ver));
                    }
                }
            }
        }
        "pnpm-lock.yaml" => {
            let documents: Vec<Value> =
                serde_saphyr::from_multiple(text).map_err(|e| e.to_string())?;
            let lock_prefix = source
                .rsplit_once('/')
                .map(|(p, _)| format!("{p}/"))
                .unwrap_or_default();
            for v in documents {
                if let Some(packages) = v["packages"].as_object() {
                    for key in packages.keys() {
                        if let Some((name, ver)) = key.rsplit_once('@')
                            && !name.is_empty()
                        {
                            let version = ver.split('(').next().unwrap_or(ver);
                            requirements.push(req("npm", name, version, source, "locked", version));
                        }
                    }
                }
                if let Some(importers) = v["importers"].as_object() {
                    for (importer, entry) in importers {
                        let manifest = if importer == "." {
                            format!("{lock_prefix}package.json")
                        } else {
                            format!("{lock_prefix}{importer}/package.json")
                        };
                        for section in ["dependencies", "devDependencies", "optionalDependencies"] {
                            if let Some(items) = entry[section].as_object() {
                                for (name, value) in items {
                                    if let Some(version) = value["version"].as_str() {
                                        let version = version.split('(').next().unwrap_or(version);
                                        requirements.push(req(
                                            "npm",
                                            name,
                                            version,
                                            &manifest,
                                            &format!("direct-lock:{section}"),
                                            version,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        "rust-toolchain.toml" => {
            let v: toml::Value = toml::from_str::<toml::Value>(text).map_err(|e| e.to_string())?;
            if let Some(channel) = v
                .get("toolchain")
                .and_then(|v| v.get("channel"))
                .and_then(toml::Value::as_str)
            {
                requirements.push(req("rustup", "rust", channel, source, "channel", ""));
            }
        }
        ".python-version" => requirements.push(req("uv", "python", text.trim(), source, "pin", "")),
        ".node-version" | ".nvmrc" => {
            requirements.push(req("engine", "node", text.trim(), source, "pin", ""))
        }
        ".tool-versions" => {
            for line in text.lines().filter(|l| !l.trim().starts_with('#')) {
                if let Some((name, version)) = line.split_once(char::is_whitespace) {
                    requirements.push(req("mise", name, version.trim(), source, "tools", ""));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn parse_python_deps(
    deps: &[toml::Value],
    source: &str,
    section: &str,
    requirements: &mut Vec<Requirement>,
) {
    for dep in deps.iter().filter_map(toml::Value::as_str) {
        let name = dep
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'))
            .next()
            .unwrap_or_default();
        if !name.is_empty() {
            requirements.push(req("pypi", name, dep, source, section, ""));
        }
    }
}

pub fn scan(env: &Environment, settings: &Settings, path: &str) -> Result<Project, String> {
    let path = safe_path(env, path)?;
    let top = git(env, settings, &path, &["rev-parse", "--show-toplevel"])?;
    if normalize(&top, env) != normalize(&path, env) {
        return Err("リポジトリのルートを指定してください".into());
    }
    let files = git(
        env,
        settings,
        &path,
        &["ls-files", "--cached", "--others", "--exclude-standard"],
    )?;
    let mut project = Project {
        id: format!("{}:{}", env.id, path),
        environment: env.id.clone(),
        name: path.rsplit(['/', '\\']).next().unwrap_or(&path).into(),
        path: path.clone(),
        remote: git(env, settings, &path, &["remote", "get-url", "origin"])
            .map(|s| redact_remote(&s))
            .unwrap_or_default(),
        branch: git(env, settings, &path, &["branch", "--show-current"]).unwrap_or_default(),
        git_status: git(env, settings, &path, &["status", "--short"])
            .unwrap_or_else(|e| format!("ERROR: {e}")),
        package_manager: String::new(),
        requirements: vec![],
        venvs: vec![],
        scripts: vec![],
        findings: vec![],
        fingerprint: String::new(),
        scanned_at: now(),
        manifest_files: vec![],
    };
    let user_npmrc = if env.kind == "local" {
        std::env::var("NPM_CONFIG_USERCONFIG").unwrap_or_else(|_| join(&env.home, ".npmrc", env))
    } else {
        join(&env.home, ".npmrc", env)
    };
    if let Ok(text) = read(env, &user_npmrc) {
        let _ = parse_manifest(
            &user_npmrc,
            &text,
            &mut project.requirements,
            &mut project.scripts,
            &mut project.package_manager,
        );
    }
    let mut hash = Sha256::new();
    // HEAD is included: CI/check results cannot be attached to a different revision silently.
    hash.update(git(env, settings, &path, &["rev-parse", "HEAD"])?);
    hash.update(&project.git_status);
    hash.update(&project.branch);
    hash.update(git(
        env,
        settings,
        &path,
        &["diff", "--no-ext-diff", "--no-textconv", "HEAD"],
    )?);
    let mut candidates: BTreeSet<String> = files
        .lines()
        .filter(|f| manifest(f.rsplit('/').next().unwrap_or(f)))
        .filter(|f| !f.split('/').any(ignored))
        .map(str::to_string)
        .collect();
    for p in [
        "mise.toml",
        ".mise.toml",
        ".python-version",
        "rust-toolchain.toml",
    ] {
        if read(env, &join(&path, p, env)).is_ok() {
            candidates.insert(p.into());
        }
    }
    if candidates.len() > 1500 {
        return Err("設定ファイルが1500件を超えています。対象範囲を絞ってください。".into());
    }
    for relative in candidates {
        if relative.starts_with('/') || relative.split('/').any(|s| s == "..") {
            continue;
        }
        let full = join(&path, &relative, env);
        let real = if env.kind == "wsl" {
            checked(env, "/usr/bin/readlink", &["-f", "--", &full], &env.home).ok()
        } else {
            std::fs::canonicalize(&full)
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        };
        if real
            .as_ref()
            .is_some_and(|real| !crate::inventory::within(real, &path, env.os == "windows"))
        {
            project.findings.push(finding(
                &relative,
                "partial",
                "リポジトリ外を参照するファイルは調査しません",
            ));
            continue;
        }
        if env.kind == "local"
            && std::fs::symlink_metadata(&full).is_ok_and(|m| m.file_type().is_symlink())
        {
            project.findings.push(finding(
                &relative,
                "partial",
                "シンボリックリンクは調査対象外です",
            ));
            continue;
        }
        match read(env, &full) {
            Ok(text) => {
                hash.update(relative.as_bytes());
                hash.update(text.as_bytes());
                project.manifest_files.push(relative.clone());
                if let Err(e) = parse_manifest(
                    &relative,
                    &text,
                    &mut project.requirements,
                    &mut project.scripts,
                    &mut project.package_manager,
                ) {
                    project.findings.push(finding(&relative, "error", e));
                }
            }
            Err(e) => project.findings.push(finding(&relative, "error", e)),
        }
    }
    if env.kind == "local" {
        for entry in WalkDir::new(&path)
            .max_depth(6)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !ignored(&e.file_name().to_string_lossy()))
            .take(12000)
        {
            let e = match entry {
                Ok(e) => e,
                Err(error) => {
                    project
                        .findings
                        .push(finding("仮想環境の走査", "error", error.to_string()));
                    continue;
                }
            };
            if e.file_type().is_file()
                && e.file_name() == "pyvenv.cfg"
                && let Ok(text) = std::fs::read_to_string(e.path())
            {
                let values: BTreeMapCompat = text
                    .lines()
                    .filter_map(|s| s.split_once('='))
                    .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                    .collect();
                project.venvs.push(VirtualEnvironment {
                    path: e
                        .path()
                        .parent()
                        .unwrap_or(Path::new(""))
                        .to_string_lossy()
                        .to_string(),
                    home: values.get("home").cloned().unwrap_or_default(),
                    version: values
                        .get("version_info")
                        .or_else(|| values.get("version"))
                        .cloned()
                        .unwrap_or_default(),
                    note: "pyvenv.cfgの静的情報。Pythonの実起動は未確認。".into(),
                });
            }
        }
    } else {
        match checked(
            env,
            "/usr/bin/find",
            &[
                &path,
                "-maxdepth",
                "6",
                "(",
                "-name",
                "node_modules",
                "-o",
                "-name",
                ".git",
                "-o",
                "-name",
                "target",
                "-o",
                "-name",
                "site-packages",
                "-o",
                "-name",
                ".kilo",
                ")",
                "-prune",
                "-o",
                "-name",
                "pyvenv.cfg",
                "-type",
                "f",
                "-print",
            ],
            &env.home,
        ) {
            Ok(paths) => {
                for config in paths.lines().take(200) {
                    match read(env, config) {
                        Ok(text) => {
                            let values: BTreeMapCompat = text
                                .lines()
                                .filter_map(|l| l.split_once('='))
                                .map(|(k, v)| (k.trim().into(), v.trim().into()))
                                .collect();
                            project.venvs.push(VirtualEnvironment{path:config.trim_end_matches("/pyvenv.cfg").into(),home:values.get("home").cloned().unwrap_or_default(),version:values.get("version_info").or_else(||values.get("version")).cloned().unwrap_or_default(),note:"pyvenv.cfgの静的情報。実行結果は検証計画の履歴から確認してください。".into()});
                        }
                        Err(e) => project.findings.push(finding(config, "error", e)),
                    }
                }
                if paths.lines().count() > 200 {
                    project.findings.push(finding(
                        "仮想環境",
                        "partial",
                        "仮想環境の走査上限200件を超えました",
                    ));
                }
            }
            Err(e) => project.findings.push(finding("仮想環境", "error", e)),
        }
    }
    for requirement in &project.requirements {
        if requirement.manager == "pypi" && requirement.section != "locked" {
            let name = match requirement.name.as_str() {
                "pytest" => "uv-pytest",
                "ruff" => "uv-ruff",
                _ => continue,
            };
            let prefix = requirement
                .source
                .rsplit_once('/')
                .map(|(p, _)| format!("{p}::"))
                .unwrap_or_default();
            let check = format!("{prefix}{name}");
            if !project.scripts.contains(&check) {
                project.scripts.push(check);
            }
        }
    }
    hash.update(serde_json::to_vec(&project.requirements).map_err(|e| e.to_string())?);
    project.fingerprint = format!("{:x}", hash.finalize());
    if project.package_manager.is_empty() {
        project.package_manager = if project.manifest_files.contains(&"uv.lock".into()) {
            "uv"
        } else if project.manifest_files.contains(&"package-lock.json".into()) {
            "npm"
        } else {
            "未指定"
        }
        .into();
    }
    if project
        .manifest_files
        .iter()
        .any(|s| s.ends_with("yarn.lock"))
    {
        project.findings.push(finding(
            "yarn.lock",
            "partial",
            "Yarnロックの推移的依存解析・更新実行は未対応です。直接依存は表示します。",
        ));
    }
    project.findings.push(finding("実行検証","partial","この静的調査ではコード・テストを実行しません。実施した検証結果は、対象コミットとともに計画・履歴から確認してください。"));
    Ok(project)
}
type BTreeMapCompat = std::collections::BTreeMap<String, String>;

pub fn redact_remote(input: &str) -> String {
    if let Ok(mut url) = reqwest::Url::parse(input) {
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.set_query(None);
        return url.to_string();
    }
    input.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monorepo_manifests_preserve_scope() {
        let mut r = vec![];
        let mut s = vec![];
        let mut pm = String::new();
        parse_manifest(
            "apps/a/package.json",
            r#"{"dependencies":{"react":"^19"},"scripts":{"test":"do-not-run"}}"#,
            &mut r,
            &mut s,
            &mut pm,
        )
        .unwrap();
        assert_eq!(r[0].source, "apps/a/package.json");
        assert_eq!(s, vec!["apps/a::test"]);
    }
    #[test]
    fn python_local_packages_not_audited_as_pypi() {
        let mut r = vec![];
        parse_manifest("uv.lock","[[package]]\nname='local'\nversion='1.0.0'\nsource={editable='.'}\n[[package]]\nname='remote'\nversion='2.0.0'\nsource={registry='https://pypi.org/simple'}",&mut r,&mut vec![],&mut String::new()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "remote");
    }
    #[test]
    fn credentials_are_not_persisted() {
        assert_eq!(
            redact_remote("https://person:secret@github.com/owner/repo.git"),
            "https://github.com/owner/repo.git"
        );
    }
    #[test]
    fn invalid_manifest_is_error() {
        assert!(
            parse_manifest(
                "package.json",
                "oops",
                &mut vec![],
                &mut vec![],
                &mut String::new()
            )
            .is_err()
        );
    }
    #[test]
    fn multiple_pnpm_documents_preserve_importer_versions() {
        let text = "packages:\n  manager@12.0.0: {}\n---\nimporters:\n  apps/a:\n    dependencies:\n      example:\n        version: 1.2.3\npackages:\n  example@1.2.3: {}\n  example@2.0.0: {}\n";
        let mut rows = vec![];
        parse_manifest(
            "pnpm-lock.yaml",
            text,
            &mut rows,
            &mut vec![],
            &mut String::new(),
        )
        .unwrap();
        assert_eq!(rows.iter().filter(|r| r.section == "locked").count(), 3);
        let direct = rows
            .iter()
            .find(|r| r.section == "direct-lock:dependencies")
            .unwrap();
        assert_eq!(direct.source, "apps/a/package.json");
        assert_eq!(direct.resolved, "1.2.3");
    }
}
