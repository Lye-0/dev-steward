// Exercise the real backend with isolated manager executables, even when none are installed.
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir, homedir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";

assert.equal(process.platform, "win32", "Windows discovery fixture");
const temporary = mkdtempSync(join(tmpdir(), "steward discovery "));
const fixture = join(temporary, "manager fixture.exe");
execFileSync(
  join(homedir(), ".cargo", "bin", "rustc.exe"),
  ["--edition=2024", "scripts/fixtures/discovery-cli.rs", "-o", fixture],
  { windowsHide: true, stdio: "pipe" },
);
mkdirSync(join(temporary, "nvm root", "v22.2.0"), { recursive: true });
writeFileSync(join(temporary, "nvm root", "v22.2.0", "node.exe"), "");
const db = join(temporary, "state.sqlite3");
function invoke(method, payload = null) {
  return JSON.parse(
    execFileSync(
      resolve("target/debug/steward.exe"),
      [db, method, JSON.stringify(payload)],
      { windowsHide: true, encoding: "utf8", timeout: 180000 },
    ),
  );
}
const settings = invoke("snapshot").settings;
settings.manager_overrides = [
  "py",
  "pymanager",
  "pyenv",
  "conda",
  "fnm",
  "nvm",
].map((name) => ["local", name, fixture]);
invoke("save_settings", settings);
invoke("scan_environment", { environment: "local" });
const inventory = invoke("snapshot").inventories.find(
  (i) => i.environment === "local",
);
for (const name of ["py", "pymanager", "pyenv", "conda", "fnm", "nvm"]) {
  assert(
    inventory.findings.some((f) => f.scope === name && f.status === "ok"),
    name,
  );
}
for (const manager of [
  "python-launcher",
  "python-install-manager",
  "pyenv",
  "conda",
  "fnm",
  "nvm",
]) {
  assert(
    inventory.tools.some(
      (t) =>
        t.manager === manager &&
        t.path.replace(/^\\\\\?\\/, "").startsWith(temporary),
    ),
    manager,
  );
}
assert(!JSON.stringify(inventory).includes("token@private.invalid"));
// A missing override must be reported as a failure, never a successful empty list.
settings.manager_overrides = [
  ["local", "py", join(temporary, "does-not-exist.exe")],
];
invoke("save_settings", settings);
invoke("scan_environment", { environment: "local" });
const next = invoke("snapshot").inventories.find(
  (i) => i.environment === "local",
);
assert(next.findings.some((f) => f.scope === "py" && f.status === "error"));
console.log(
  JSON.stringify(
    {
      ok: true,
      adapters: 6,
      independentOfInstalledManagers: true,
      missingOverrideRejected: true,
    },
    null,
    2,
  ),
);
