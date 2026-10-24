// Windows WebView2 integration test: real Tauri IPC and Rust backend, isolated data/repository.
import { chromium } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";
const root = resolve(".");
const temp = mkdtempSync(join(tmpdir(), "steward-native-"));
const fixture = join(temp, "repository with spaces");
mkdirSync(fixture);
writeFileSync(
  join(fixture, "package.json"),
  JSON.stringify(
    {
      name: "portable-fixture",
      packageManager: "pnpm@11.28.8",
      scripts: { test: "node -e \"console.log('fixture passed')\"" },
      dependencies: { react: "^19.0.0" },
    },
    null,
    2,
  ),
);
writeFileSync(
  join(fixture, "mise.toml"),
  '[tools]\nnode="24.22.1"\npnpm="11.28.8"\n',
);
writeFileSync(
  join(fixture, "pnpm-lock.yaml"),
  "lockfileVersion: '9.0'\npackages:\n  react@19.0.0: {}\n",
);
for (const args of [
  ["init", "-b", "main"],
  ["config", "user.email", "fixture@example.invalid"],
  ["config", "user.name", "Native Fixture"],
  ["add", "."],
  ["commit", "-m", "fixture"],
])
  execFileSync("git", args, { cwd: fixture, windowsHide: true, stdio: "pipe" });
execFileSync("mise", ["trust", fixture], {
  cwd: fixture,
  windowsHide: true,
  stdio: "pipe",
});
const vite = spawn(
  process.execPath,
  [
    "node_modules/vite/bin/vite.js",
    "--host",
    "127.0.0.1",
    "--port",
    "1420",
    "--strictPort",
  ],
  { cwd: root, windowsHide: true, stdio: "pipe" },
);
let app, browser;
const errors = [];
let logs = "";
vite.stderr.on("data", (b) => (logs += b));
vite.stdout.on("data", (b) => (logs += b));
async function waitFor(fn, ms = 60000) {
  const start = Date.now();
  while (Date.now() - start < ms) {
    try {
      const x = await fn();
      if (x) return x;
    } catch {}
    await new Promise((r) => setTimeout(r, 300));
  }
  throw new Error(`Timed out. ${logs}`);
}
try {
  await waitFor(async () => (await fetch("http://127.0.0.1:1420")).ok);
  app = spawn(
    resolve("target/debug/dev-steward.exe"),
    ["--data-dir", join(temp, "state")],
    {
      cwd: root,
      windowsHide: true,
      env: {
        ...process.env,
        DEV_STEWARD_TEST_DATA_DIR: join(temp, "state"),
        WEBVIEW2_USER_DATA_FOLDER: join(temp, "webview"),
        WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: "--remote-debugging-port=9224",
      },
      stdio: "pipe",
    },
  );
  app.stderr.on("data", (b) => (logs += b));
  app.stdout.on("data", (b) => (logs += b));
  await waitFor(
    async () => (await fetch("http://127.0.0.1:9224/json/version")).ok,
  );
  browser = await chromium.connectOverCDP("http://127.0.0.1:9224");
  const page = await waitFor(() =>
    browser
      .contexts()
      .flatMap((c) => c.pages())
      .find((p) => p.url().includes("1420")),
  );
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.getByRole("heading", { name: "開発環境を、見渡す。" }).waitFor();
  const native = (method, payload = null) =>
    page.evaluate(
      async ({ method, payload }) =>
        window.__TAURI_INTERNALS__.invoke("steward", { method, payload }),
      { method, payload },
    );
  let snapshot = await native("snapshot");
  assert.equal(snapshot.projects.length, 0);
  assert(snapshot.data_path.startsWith(temp));
  await page.getByRole("button", { name: "環境を調査", exact: true }).click();
  await page
    .getByText("環境を調査中…", { exact: false })
    .waitFor({ state: "hidden", timeout: 120000 });
  snapshot = await native("snapshot");
  assert(
    snapshot.inventories[0].tools.length > 0,
    "Native inventory should contain real installed runtimes",
  );
  await page.getByRole("button", { name: "プロジェクト", exact: true }).click();
  await page
    .getByRole("button", { name: "フォルダーを登録", exact: true })
    .click();
  await page.getByLabel("調査するフォルダー", { exact: true }).fill(fixture);
  await page.getByRole("button", { name: "調査して登録", exact: true }).click();
  await page
    .getByRole("button", { name: /portable-fixture|repository with spaces/ })
    .first()
    .waitFor({ timeout: 60000 });
  snapshot = await native("snapshot");
  assert.equal(snapshot.projects.length, 1);
  const project = snapshot.projects[0];
  assert.equal(
    project.requirements.filter((r) => r.section === "locked").length,
    1,
  );
  await page.getByRole("button", { name: /repository with spaces/ }).click();
  await page.getByRole("checkbox", { name: "test", exact: true }).check();
  await page
    .getByRole("button", { name: "選択した検証の計画を作成", exact: true })
    .click();
  await page
    .getByRole("heading", { name: /repository with spaces の実行検証/ })
    .waitFor();
  await page
    .getByRole("checkbox", { name: /対象・コマンドと影響を確認/ })
    .check();
  await page
    .getByRole("button", { name: "次の工程を実行", exact: true })
    .click();
  await page
    .getByText("工程を実行中…", { exact: false })
    .waitFor({ state: "hidden", timeout: 120000 });
  snapshot = await native("snapshot");
  const plan = snapshot.plans[0];
  assert.equal(plan.status, "complete", JSON.stringify(plan));
  assert(plan.steps[0].output.includes("fixture passed"));
  await page.getByRole("button", { name: "全体状況", exact: true }).click();
  mkdirSync("test-results", { recursive: true });
  await page.screenshot({
    path: "test-results/native-overview.png",
    fullPage: true,
  });
  await page.getByRole("button", { name: "開発環境", exact: true }).click();
  await page.screenshot({
    path: "test-results/native-environment.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 780, height: 800 });
  await page.screenshot({
    path: "test-results/native-narrow.png",
    fullPage: true,
  });
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth > window.innerWidth,
  );
  assert.equal(overflow, false, "Page should not overflow horizontally");
  await page.getByRole("button", { name: "設定", exact: true }).click();
  assert.equal(
    await page.getByRole("heading", { name: "設定", exact: true }).count(),
    1,
  );
  assert.equal(errors.length, 0, errors.join("\n"));
  console.log(
    JSON.stringify(
      {
        ok: true,
        tools: snapshot.inventories[0].tools.length,
        project: project.name,
        plan: plan.status,
        testOutput: plan.steps[0].output,
        fixture,
        data: snapshot.data_path,
        screenshots: [
          "native-overview.png",
          "native-environment.png",
          "native-narrow.png",
        ],
      },
      null,
      2,
    ),
  );
} finally {
  await browser?.close().catch(() => {});
  for (const child of [app, vite])
    if (child?.pid) {
      try {
        execFileSync("taskkill.exe", ["/PID", String(child.pid), "/T", "/F"], {
          windowsHide: true,
          stdio: "pipe",
        });
      } catch {}
    }
}
