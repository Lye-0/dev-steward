use serde_json::json;
use std::{path::Path, process::Command};
use steward_core::{models::*, projects, service::Service};
fn git(path: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn fixture() -> tempfile::TempDir {
    let root = tempfile::Builder::new()
        .prefix("another-user repo 日本語 ")
        .tempdir()
        .unwrap();
    git(root.path(), &["init", "-b", "main"]);
    git(
        root.path(),
        &["config", "user.email", "fixture@example.invalid"],
    );
    git(root.path(), &["config", "user.name", "Fixture"]);
    std::fs::write(root.path().join("package.json"),r#"{"name":"fixture","packageManager":"pnpm@11.28.8","scripts":{"test":"node -e 0"},"dependencies":{"react":"^19.0.0"}}"#).unwrap();
    std::fs::write(
        root.path().join("pnpm-lock.yaml"),
        "lockfileVersion: '9.0'\npackages:\n  react@19.0.0: {}\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "fixture"]);
    root
}
fn env(root: &Path) -> Environment {
    Environment {
        id: "local".into(),
        name: "fixture".into(),
        kind: "local".into(),
        home: root.to_string_lossy().into(),
        os: std::env::consts::OS.into(),
    }
}
#[test]
fn discovers_and_fingerprints_real_repositories() {
    let root = fixture();
    let e = env(root.path());
    let settings = Settings::default();
    let p = projects::scan(&e, &settings, root.path().to_str().unwrap()).unwrap();
    assert!(p.git_status.is_empty());
    assert_eq!(
        p.requirements
            .iter()
            .filter(|r| r.section == "locked")
            .count(),
        1
    );
    std::fs::write(root.path().join("package.json"), "{}").unwrap();
    let changed = projects::scan(&e, &settings, root.path().to_str().unwrap()).unwrap();
    assert_ne!(p.fingerprint, changed.fingerprint);
}
#[test]
fn stale_plan_is_rejected_before_side_effects() {
    let root = fixture();
    let database = tempfile::tempdir().unwrap();
    let app = Service::new(database.path().join("state.db")).unwrap();
    let e = env(root.path());
    app.store.put("environments", "local", &e).unwrap();
    let p = projects::scan(&e, &Settings::default(), root.path().to_str().unwrap()).unwrap();
    app.store.put("projects", &p.id, &p).unwrap();
    let plan = Plan {
        id: "stale".into(),
        environment: "local".into(),
        project_id: Some(p.id.clone()),
        title: "stale".into(),
        reason: String::new(),
        evidence: vec![],
        warnings: vec![],
        steps: vec![Step {
            id: "one".into(),
            label: "must not run".into(),
            program: "git".into(),
            args: vec!["switch".into(), "-c".into(), "must-not-exist".into()],
            cwd: p.path.clone(),
            kind: "branch".into(),
            risk: "write".into(),
            status: "pending".into(),
            output: String::new(),
        }],
        created_at: now(),
        status: "ready".into(),
        fingerprint: p.fingerprint,
    };
    app.store.put("plans", "stale", &plan).unwrap();
    std::fs::write(root.path().join("package.json"), "{}").unwrap();
    assert!(
        app.call("execute_step", json!({"plan":"stale","confirmed":true}))
            .unwrap_err()
            .contains("変更")
    );
    git(root.path(), &["symbolic-ref", "--short", "HEAD"]);
    let branch = Command::new("git")
        .args(["branch", "--list", "must-not-exist"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(branch.stdout.is_empty());
}
#[test]
fn code_change_invalidates_plan_even_when_manifests_do_not_change() {
    let root = fixture();
    std::fs::write(root.path().join("app.js"), "before").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "code"]);
    let e = env(root.path());
    let first = projects::scan(&e, &Settings::default(), root.path().to_str().unwrap()).unwrap();
    std::fs::write(root.path().join("app.js"), "after").unwrap();
    let changed = projects::scan(&e, &Settings::default(), root.path().to_str().unwrap()).unwrap();
    assert_ne!(first.fingerprint, changed.fingerprint);
}
