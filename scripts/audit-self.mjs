import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const directory = mkdtempSync(join(tmpdir(), "steward-audit-"));
const binary = resolve(
  `target/debug/steward${process.platform === "win32" ? ".exe" : ""}`,
);
function call(method, payload = null) {
  return JSON.parse(
    execFileSync(
      binary,
      [join(directory, "state.db"), method, JSON.stringify(payload)],
      {
        windowsHide: true,
        encoding: "utf8",
        maxBuffer: 16 * 1024 * 1024,
        timeout: 300000,
      },
    ),
  );
}
call("scan_root", { environment: "local", path: resolve(".") });
const project = call("snapshot").projects[0];
const results = call("check_updates", { project: project.id });
const findings = results.filter((r) => r.vulnerabilities.length);
mkdirSync("test-results", { recursive: true });
writeFileSync(
  "test-results/dependency-audit.json",
  JSON.stringify(
    { at: new Date().toISOString(), project: project.name, results },
    null,
    2,
  ),
);
console.log(
  JSON.stringify(
    {
      project: project.name,
      requirements: project.requirements.length,
      findings: findings.map((f) => ({
        name: f.name,
        version: f.current,
        ids: f.vulnerabilities.map((v) => v.id),
      })),
      coverage: results[0]?.audit_status,
      lookupErrors: results
        .filter((r) => r.importance === "unknown")
        .map((r) => ({ name: r.name, note: r.note })),
    },
    null,
    2,
  ),
);
if (findings.length) process.exitCode = 1;
