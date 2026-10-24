import { execFileSync } from "node:child_process";
import {
  mkdtempSync,
  writeFileSync,
  readdirSync,
  mkdirSync,
  unlinkSync,
} from "node:fs";
import { tmpdir, homedir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";
assert.equal(process.platform, "win32");
const temp = mkdtempSync(join(tmpdir(), "steward github "));
const gh = join(temp, "mock-gh.exe");
execFileSync(
  join(homedir(), ".cargo/bin/rustc.exe"),
  ["--edition=2024", "scripts/fixtures/github-cli.rs", "-o", gh],
  { windowsHide: true, stdio: "pipe" },
);
const database = join(temp, "state.sqlite3");
function invoke(method, payload = null) {
  return JSON.parse(
    execFileSync(
      resolve("target/debug/steward.exe"),
      [database, method, JSON.stringify(payload)],
      { encoding: "utf8", windowsHide: true, timeout: 180000 },
    ),
  );
}
function rejected(method, payload) {
  assert.throws(() => invoke(method, payload));
}
const settings = invoke("snapshot").settings;
settings.manager_overrides = [["local", "gh", gh]];
invoke("save_settings", settings);
const repos = invoke("github_repos");
assert.equal(repos[0].nameWithOwner, "Example/monorepo");
const scan = invoke("scan_github_repos", {
  repositories: ["Example/denied", "Example/monorepo"],
});
assert.equal(scan.errors.length, 1);
assert.equal(scan.reports.length, 1);
const report = scan.reports[0];
assert.equal(report.complete, true);
assert.equal(report.commit, "a".repeat(40));
assert.equal(report.files.length, 7);
assert(
  report.project.requirements.some(
    (r) => r.source === "apps/web/package.json" && r.name === "kleur",
  ),
);
assert(
  report.project.requirements.some(
    (r) => r.manager === "github-actions" && r.name === "actions/checkout",
  ),
);
assert.equal(invoke("snapshot").projects.length, 0);
assert(!JSON.stringify(report).includes("never-persist-this-secret"));
rejected("plan_github_updates", { report: report.id });
invoke("check_github_updates", { report: report.id });
const proposal = invoke("plan_github_updates", { report: report.id });
assert(
  proposal.changes.some((c) => c.name === "kleur" && c.candidate === "4.1.5"),
);
assert(proposal.checks.includes("apps/web::build"));
assert.equal(proposal.commit, report.commit);
rejected("execute_step", { plan: proposal.id, confirmed: true });
writeFileSync(join(temp, "head-moved"), "");
rejected("plan_github_updates", { report: report.id });
invoke("scan_github_repos", { repositories: ["Example/monorepo"] });
const next = invoke("snapshot");
assert(next.github_plans.every((p) => p.status === "outdated"));
assert.equal(next.github_updates.length, 0);
assert(
  !readdirSync(temp, { recursive: true }).some(
    (p) =>
      p === ".git" ||
      p.endsWith("/.git") ||
      p.endsWith("\\.git") ||
      p.endsWith("package.json"),
  ),
);
// Only this generated workspace is modified; no clone or user repository is used.
unlinkSync(join(temp, "head-moved"));
const workspace = join(temp, "local workspace");
mkdirSync(workspace);
const git = (...args) =>
  execFileSync("git", args, {
    cwd: workspace,
    windowsHide: true,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
git("init", "-b", "main");
git("config", "user.email", "fixture@example.invalid");
git("config", "user.name", "Fixture");
writeFileSync(
  join(workspace, "package.json"),
  '{"name":"fixture","version":"1.0.0"}',
);
git("add", ".");
git("commit", "-m", "fixture");
const origin = "https://github.example.invalid/Example/monorepo.git";
git("remote", "add", "origin", origin);
const head = git("rev-parse", "HEAD");
writeFileSync(join(temp, "fixed-head"), head);
const freshReport = invoke("scan_github_repos", {
  repositories: ["Example/monorepo"],
}).reports[0];
invoke("check_github_updates", { report: freshReport.id });
const freshProposal = invoke("plan_github_updates", { report: freshReport.id });
invoke("scan_root", { environment: "local", path: workspace });
const local = invoke("snapshot").projects[0];
const preparation = { proposal: freshProposal.id, project: local.id };
invoke("prepare_github_proposal", preparation);
const prepared = invoke("snapshot").github_plans.find(
  (p) => p.id === freshProposal.id,
);
assert.equal(prepared.workspace, local.id);
assert.equal(prepared.status, "prepared");
assert.equal(git("rev-parse", "HEAD"), head);
assert.equal(git("status", "--porcelain"), "");
writeFileSync(join(workspace, "uncommitted.txt"), "fixture");
rejected("prepare_github_proposal", preparation);
unlinkSync(join(workspace, "uncommitted.txt"));
git(
  "remote",
  "set-url",
  "origin",
  "https://github.example.invalid/Example/other.git",
);
rejected("prepare_github_proposal", preparation);
git("remote", "set-url", "origin", origin);
git("commit", "--allow-empty", "-m", "different commit");
rejected("prepare_github_proposal", preparation);
console.log(
  JSON.stringify(
    {
      ok: true,
      files: report.files.length,
      cloneCreated: false,
      partialBatchFailure: true,
      sharedMonorepoParser: true,
      proposalSaved: true,
      staleCommitRejected: true,
      workspaceGuards: true,
    },
    null,
    2,
  ),
);
