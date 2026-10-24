import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, unlinkSync } from "node:fs";
import { tmpdir, homedir } from "node:os";
import { join, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";
import assert from "node:assert/strict";
const temp = mkdtempSync(join(tmpdir(), "steward-manager-updates-"));
const fixture = join(temp, "manager fixture.exe");
execFileSync(
  join(homedir(), ".cargo/bin/rustc.exe"),
  ["--edition=2024", "scripts/fixtures/discovery-cli.rs", "-o", fixture],
  { windowsHide: true, stdio: "pipe" },
);
const database = join(temp, "state.sqlite3");
const invoke = (method, payload = null) =>
  JSON.parse(
    execFileSync(
      resolve("target/debug/steward.exe"),
      [database, method, JSON.stringify(payload)],
      { encoding: "utf8", windowsHide: true, timeout: 180000 },
    ),
  );
const state = invoke("snapshot");
const settings = state.settings;
settings.manager_overrides = [
  "fnm",
  "nvm",
  "pyenv",
  "pymanager",
  "conda",
  "winget",
].map((m) => ["local", m, fixture]);
invoke("save_settings", settings);
const prefix = join(temp, "Conda Env");
mkdirSync(prefix);
const tool = (manager, name, version, requested = "") => ({
  id: `local:${manager}:${name}`,
  environment: "local",
  manager,
  name,
  version,
  requested,
  path: join(temp, manager),
  source: manager === "conda" ? prefix : "winget",
  active: false,
  installed: true,
  note: "Fixture",
});
const tools = [
  tool("fnm", "node", "22.2.0"),
  tool("nvm", "node", "22.2.0"),
  tool("pyenv", "python", "3.13.7"),
  tool("python-install-manager", "python", "3.14.2", "3.14"),
  tool("conda", "python", "3.12.8"),
  tool("python-launcher", "python", "3.10.0"),
  tool("winget", "Example.App", "1.0.0"),
  tool("winget", "Example.Other", "2.0.0"),
];
const db = new DatabaseSync(database);
db.prepare("INSERT OR REPLACE INTO records VALUES (?,?,?)").run(
  "inventories",
  "local",
  JSON.stringify({
    environment: "local",
    captured_at: new Date().toISOString(),
    tools,
    findings: [],
    manager_paths: [],
  }),
);
db.close();
const updates = invoke("check_tool_updates", { environment: "local" });
assert(!updates.some((u) => u.manager === "winget"));
assert.equal(
  updates.find((u) => u.manager === "python-launcher").importance,
  "delegated",
);
for (const [manager, candidate] of [
  ["fnm", "22.3.0"],
  ["pyenv", "3.13.8"],
  ["python-install-manager", "3.14.3"],
  ["conda", "3.12.9"],
]) {
  const update = updates.find((u) => u.manager === manager);
  assert.equal(update.candidate, candidate, JSON.stringify(update));
  const plan = invoke("plan_tool", {
    environment: "local",
    tool: update.tool_id,
  });
  assert.equal(plan.status, "ready");
  assert(
    plan.steps[0].args.includes(
      manager === "conda" ? `python=${candidate}` : candidate,
    ),
  );
  assert(!plan.steps[0].args.includes("use"));
}
const nvm = updates.find((u) => u.manager === "nvm");
// A registry outage must be a visible error, never a successful 'no candidates'.
assert(["maintenance", "error"].includes(nvm.importance));
if (nvm.candidate)
  assert(
    invoke("plan_tool", {
      environment: "local",
      tool: nvm.tool_id,
    }).steps[0].args.includes(nvm.candidate),
  );
const report = invoke("system_updates", { environment: "local" });
assert.equal(
  report.updates.find((u) => u.name === "Example.App").candidate,
  "1.1.0",
);
assert.equal(
  report.updates.find((u) => u.name === "Example.Other").importance,
  "not-listed",
);
const plan = invoke("plan_system_update", {
  environment: "local",
  package: "Example.App",
  candidate: "1.1.0",
});
assert(plan.steps[0].args.includes("--version"));
assert(plan.steps[0].args.includes("1.1.0"));
assert.throws(() =>
  invoke("plan_system_update", {
    environment: "local",
    package: "Example.App",
    candidate: "9.0.0",
  }),
);
const before = invoke("snapshot").plans.length;
const noUpdate = invoke("plan_manager_update", {
  environment: "local",
  manager: "mise",
});
assert.equal(noUpdate.status, "no-update");
assert.equal(noUpdate.installed, "2026.10.12");
assert.equal(invoke("snapshot").plans.length, before);
// A candidate disappearing after planning must stop before the update command.
writeFileSync(join(temp, "manager-current"), "");
assert.throws(() => invoke("execute_step", { plan: plan.id, confirmed: true }));
assert.equal(
  invoke("snapshot").plans.find((p) => p.id === plan.id).status,
  "outdated",
);
unlinkSync(join(temp, "manager-current"));
// Old unpinned plans are retained, but disabled for re-planning.
const old = {
  ...plan,
  id: "old-unpinned-plan",
  status: "ready",
  steps: plan.steps.map((s) => ({
    ...s,
    args: s.args.filter(
      (a, i, list) => a !== "--version" && list[i - 1] !== "--version",
    ),
  })),
};
const oldDb = new DatabaseSync(database);
oldDb
  .prepare("INSERT INTO records VALUES (?,?,?)")
  .run("plans", old.id, JSON.stringify(old));
oldDb.close();
assert.equal(
  invoke("snapshot").plans.find((p) => p.id === old.id).status,
  "outdated",
);
// No install commands are supported by the fixture: all tests stop at plan creation.
console.log(
  JSON.stringify(
    {
      ok: true,
      additionalManagers: 5,
      legacyLauncherDelegated: true,
      wingetCandidates: true,
      unlistedIsNotLatest: true,
      selectedVersionPinned: true,
      noOpPlanSuppressed: true,
      staleExecutionBlocked: true,
      legacyPlansInvalidated: true,
      installExecuted: false,
    },
    null,
    2,
  ),
);
