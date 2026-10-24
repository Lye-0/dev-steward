import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync, readdirSync } from "node:fs";
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
    },
    null,
    2,
  ),
);
