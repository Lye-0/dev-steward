// Real registry -> plan -> isolated branch -> dependency update -> test. No GitHub writes.
import { execFileSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  realpathSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";
const root = resolve(".");
// Windows TEMP may use an 8.3 alias (RUNNER~1). Match the physical path
// used by Rust project discovery before pnpm records its virtual-store location.
const state = realpathSync.native(
  mkdtempSync(join(tmpdir(), "steward-update-")),
);
const repo = join(state, "project");
mkdirSync(repo);
const pins = readFileSync("mise.toml", "utf8");
const packageManager = JSON.parse(
  readFileSync("package.json", "utf8"),
).packageManager;
writeFileSync(
  join(repo, "package.json"),
  JSON.stringify(
    {
      name: "update-fixture",
      private: true,
      packageManager,
      scripts: { test: "node -e \"console.log('update fixture passed')\"" },
      dependencies: { kleur: "4.1.4" },
    },
    null,
    2,
  ),
);
writeFileSync(join(repo, "mise.toml"), pins);
writeFileSync(join(repo, ".gitignore"), "node_modules/\n");
const command = (program, args) =>
  execFileSync(program, args, {
    cwd: repo,
    windowsHide: true,
    encoding: "utf8",
    timeout: 180000,
    stdio: ["ignore", "pipe", "pipe"],
  });
command("mise", ["trust", repo]);
command("mise", ["exec", "--", "pnpm", "install", "--ignore-scripts"]);
for (const args of [
  ["init", "-b", "main"],
  ["config", "user.email", "fixture@example.invalid"],
  ["config", "user.name", "Update Fixture"],
  ["add", "."],
  ["commit", "-m", "fixture"],
])
  command("git", args);
const bin = resolve(
  `target/debug/steward${process.platform === "win32" ? ".exe" : ""}`,
);
const database = join(state, "state.db");
const call = (method, payload = null) =>
  JSON.parse(
    execFileSync(bin, [database, method, JSON.stringify(payload)], {
      cwd: root,
      windowsHide: true,
      encoding: "utf8",
      timeout: 180000,
    }),
  );
call("scan_root", { environment: "local", path: repo });
let snapshot = call("snapshot");
const project = snapshot.projects[0];
assert.equal(project.git_status, "");
assert.equal(realpathSync.native(project.path), realpathSync.native(repo));
const updates = call("check_updates", { project: project.id });
const selected = updates.findIndex((u) => u.name === "kleur" && u.candidate);
assert(selected >= 0, JSON.stringify(updates));
let plan = call("plan_update", {
  project: project.id,
  selected: [selected],
  checks: ["test"],
  publish: false,
});
assert(!plan.steps.some((s) => ["push", "pr", "merge-pr"].includes(s.kind)));
for (const step of plan.steps) {
  plan = call("execute_step", { plan: plan.id, confirmed: true });
  assert.notEqual(plan.status, "failed", JSON.stringify(plan));
  console.log(`${step.label}: success`);
}
assert.equal(plan.status, "complete");
const result = JSON.parse(readFileSync(join(repo, "package.json"), "utf8"));
assert.equal(result.dependencies.kleur, updates[selected].candidate);
assert(
  command("git", ["branch", "--show-current"]).startsWith("steward/update-"),
);
assert(
  plan.steps.some(
    (s) => s.kind === "test" && s.output.includes("update fixture passed"),
  ),
);
// Re-run the audit against the new lock, then independently exercise failure handling.
call("rescan_project", { project: project.id });
const audit = call("check_updates", { project: project.id });
assert(audit.some((u) => u.audit_status.startsWith("OSV照合済み")));
result.scripts.test = 'node -e "process.exit(3)"';
writeFileSync(join(repo, "package.json"), JSON.stringify(result, null, 2));
call("rescan_project", { project: project.id });
let failed = call("plan_validate", { project: project.id, checks: ["test"] });
failed = call("execute_step", { plan: failed.id, confirmed: true });
assert.equal(failed.status, "failed");
let replayRefused = false;
try {
  call("execute_step", { plan: failed.id, confirmed: true });
} catch {
  replayRefused = true;
}
assert(replayRefused);
console.log(
  JSON.stringify(
    {
      ok: true,
      update: `4.1.4 -> ${updates[selected].candidate}`,
      audit: audit[0].audit_status,
      failureAndReplayGuard: true,
      repository: repo,
    },
    null,
    2,
  ),
);
