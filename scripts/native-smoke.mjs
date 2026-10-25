// Windows WebView2 integration test: real Tauri IPC and Rust backend, isolated data/repository.
import { chromium } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  existsSync,
  realpathSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";
import assert from "node:assert/strict";
import { reviewUi } from "./ui-review-smoke.mjs";
const root = resolve(".");
const temp = realpathSync.native(
  mkdtempSync(join(tmpdir(), "steward-native-")),
);
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
mkdirSync(join(fixture, ".github", "workflows"), { recursive: true });
writeFileSync(
  join(fixture, ".github", "workflows", "ci.yml"),
  "jobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/setup-node@v6\n        with:\n          node-version: '22.x'\n      - run: npm test | tee results.txt\n      - run: grep tests results.txt\n",
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
  let lastError;
  while (Date.now() - start < ms) {
    if (app?.exitCode !== null && app?.exitCode !== undefined) {
      throw new Error(
        `Application exited before startup: ${app.exitCode}. ${logs}`,
      );
    }
    try {
      const x = await fn();
      if (x) return x;
    } catch (error) {
      lastError = error;
    }
    await new Promise((r) => setTimeout(r, 300));
  }
  throw new Error(
    `Timed out waiting for startup. Last error: ${lastError}. ${logs}`,
  );
}
try {
  await waitFor(async () => (await fetch("http://127.0.0.1:1420")).ok);
  app = spawn(
    resolve("target/debug/dev-steward.exe"),
    ["--data-dir", join(temp, "state"), "--automation-port", "9224"],
    {
      cwd: root,
      windowsHide: true,
      env: {
        ...process.env,
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
  const mainNav = page.getByRole("navigation", {
    name: "メインナビゲーション",
  });
  assert.deepEqual(await mainNav.getByRole("button").allTextContents(), [
    "概要",
    "環境・ツール",
    "プロジェクト",
    "更新",
  ]);
  assert.equal(
    await page
      .getByRole("navigation", { name: "補助ナビゲーション" })
      .getByRole("button")
      .count(),
    2,
  );
  assert.equal(
    await page.getByRole("combobox").count(),
    0,
    "Overview has no implicit environment filter",
  );
  await page.getByRole("button", { name: "環境・ツール", exact: true }).click();
  // Native customizable pickers must retain the dark theme while open.
  mkdirSync("test-results", { recursive: true });
  assert(await page.evaluate(() => CSS.supports("appearance", "base-select")));
  const environmentPicker = page.getByRole("combobox", {
    name: "操作する環境",
  });
  await environmentPicker.click();
  assert(await environmentPicker.evaluate((el) => el.matches(":open")));
  const pickerColors = await environmentPicker.evaluate((el) => ({
    background: getComputedStyle(el, "::picker(select)").backgroundColor,
    color: getComputedStyle(el, "::picker(select)").color,
  }));
  assert.equal(pickerColors.background, "rgb(23, 33, 44)");
  assert.equal(pickerColors.color, "rgb(227, 237, 245)");
  await page.screenshot({ path: "test-results/native-environment-picker.png" });
  await page.keyboard.press("Escape");
  assert(!(await environmentPicker.evaluate((el) => el.matches(":open"))));
  const native = (method, payload = null) =>
    page.evaluate(
      async ({ method, payload }) =>
        window.__TAURI_INTERNALS__.invoke("steward", { method, payload }),
      { method, payload },
    );
  let snapshot = await native("snapshot");
  assert.equal(snapshot.projects.length, 0);
  assert(snapshot.data_path.startsWith(temp));
  await page.getByRole("button", { name: "再調査", exact: true }).click();
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
  await page.getByRole("button", { name: "概要", exact: true }).click();
  mkdirSync("test-results", { recursive: true });
  await page.screenshot({
    path: "test-results/native-overview.png",
    fullPage: true,
  });
  await page.getByRole("button", { name: "環境・ツール", exact: true }).click();
  const inventory = snapshot.inventories[0];
  const visibleManagers = [
    ...new Set(
      inventory.tools
        .filter((t) => t.manager !== "winget")
        .map((t) => t.manager),
    ),
  ];
  assert.equal(
    await page.locator(".manager-group table").count(),
    visibleManagers.length,
  );
  for (const section of await page.locator(".manager-group").all()) {
    assert.equal(await section.getByRole("heading", { level: 2 }).count(), 1);
    assert.equal(await section.getByRole("table").count(), 1);
  }
  const search = page.getByRole("textbox", { name: "ツールを検索" });
  await search.fill("uv 管理");
  assert.equal(
    await page.getByRole("table", { name: "uv 管理", exact: true }).count(),
    inventory.tools.some((t) => t.manager === "uv") ? 1 : 0,
  );
  await search.fill("no matching manager 1234");
  await page
    .getByText("表示条件に一致するツールはありません", { exact: true })
    .waitFor();
  assert.equal(await page.locator(".manager-group table").count(), 0);
  await search.fill("");
  assert.equal(
    await page
      .getByRole("region", { name: "WinGet 管理", exact: true })
      .count(),
    1,
  );
  assert.equal(
    await page.getByRole("table", { name: "WinGet 管理", exact: true }).count(),
    inventory.tools.some((t) => t.manager === "winget") ? 1 : 0,
  );
  assert.equal(
    await page
      .locator(".manager-groups")
      .getByRole("table", { name: "WinGet 管理", exact: true })
      .count(),
    0,
  );
  // The same five-column schema must align across managers and survive disclosure.
  await page.setViewportSize({ width: 1600, height: 1000 });
  async function geometry() {
    return page.locator(".runtime-table").evaluateAll((tables) =>
      tables.map((table) => ({
        columns: [...table.querySelectorAll("thead th")].map((th) => ({
          x: th.getBoundingClientRect().x,
          width: th.getBoundingClientRect().width,
        })),
        first: [...table.querySelector("tbody tr").children].map((td) => ({
          x: td.getBoundingClientRect().x,
          width: td.getBoundingClientRect().width,
        })),
      })),
    );
  }
  function aligned(actual, expected) {
    assert.equal(actual.length, expected.length);
    actual.forEach((column, i) => {
      assert(Math.abs(column.x - expected[i].x) < 1, "Column start moved");
      assert(
        Math.abs(column.width - expected[i].width) < 1,
        "Column width moved",
      );
    });
  }
  const before = await geometry();
  for (const group of before) {
    aligned(group.columns, before[0].columns);
    aligned(group.first, group.columns);
  }
  const toggles = page.locator(".runtime-table .tool-details-toggle");
  const toggle = page
    .getByRole("table", { name: "uv 管理", exact: true })
    .locator(".tool-details-toggle")
    .first();
  const selectedToggle = (await toggle.count()) ? toggle : toggles.first();
  await selectedToggle.click();
  assert.equal(await selectedToggle.getAttribute("aria-expanded"), "true");
  const after = await geometry();
  after.forEach((group, i) => {
    aligned(group.columns, before[i].columns);
    aligned(group.first, before[i].first);
  });
  const panelId = await selectedToggle.getAttribute("aria-controls");
  const expanded = page.locator(`[id="${panelId}"]`);
  assert(await expanded.isVisible());
  const panel = await expanded.boundingBox();
  const table = await selectedToggle
    .locator("xpath=ancestor::table")
    .boundingBox();
  assert(
    panel.width > table.width - 6,
    "Detail content must use the full row width",
  );
  await page.screenshot({
    path: "test-results/native-details-expanded.png",
    fullPage: true,
  });
  await selectedToggle.press("Enter");
  assert.equal(await selectedToggle.getAttribute("aria-expanded"), "false");
  assert(!(await expanded.isVisible()));
  (await geometry()).forEach((group, i) =>
    aligned(group.columns, before[i].columns),
  );
  await page.screenshot({
    path: "test-results/native-environment.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 780, height: 800 });
  await page.screenshot({
    path: "test-results/native-narrow.png",
    fullPage: true,
  });
  // Bring the control into the horizontal viewport before comparing disclosure geometry.
  // Scrolling to an initially off-screen button is distinct from a column reflow.
  await selectedToggle.scrollIntoViewIfNeeded();
  const narrowBefore = await geometry();
  await selectedToggle.click();
  const narrowAfter = await geometry();
  narrowAfter.forEach((group, i) =>
    aligned(group.columns, narrowBefore[i].columns),
  );
  await selectedToggle.click();
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth > window.innerWidth,
  );
  assert.equal(overflow, false, "Page should not overflow horizontally");
  await page.getByRole("button", { name: "設定", exact: true }).click();
  assert.equal(
    await page.getByRole("heading", { name: "設定", exact: true }).count(),
    1,
  );
  const managerPicker = page.getByRole("combobox", {
    name: "管理ツール",
    exact: true,
  });
  const initialManager = await managerPicker.inputValue();
  await managerPicker.click();
  assert(await managerPicker.evaluate((el) => el.matches(":open")));
  await page.screenshot({ path: "test-results/native-manager-picker.png" });
  await page.getByRole("option", { name: "uv", exact: true }).click();
  assert.equal(await managerPicker.inputValue(), "uv");
  await managerPicker.click();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Escape");
  assert.equal(
    await managerPicker.inputValue(),
    "uv",
    "Escape should cancel the pending selection",
  );
  await managerPicker.click();
  await page.keyboard.press("Home");
  await page.keyboard.press("Enter");
  assert.equal(await managerPicker.inputValue(), initialManager);
  // Exercise clone-free GitHub import through the real window and service.
  const fakeGithub = join(temp, "github fixture.exe");
  execFileSync(
    join(process.env.USERPROFILE, ".cargo/bin/rustc.exe"),
    ["--edition=2024", "scripts/fixtures/github-cli.rs", "-o", fakeGithub],
    { windowsHide: true, stdio: "pipe" },
  );
  const githubSettings = (await native("snapshot")).settings;
  githubSettings.manager_overrides = githubSettings.manager_overrides.filter(
    ([, name]) => name !== "gh",
  );
  githubSettings.manager_overrides.push(["local", "gh", fakeGithub]);
  execFileSync(
    "git",
    [
      "remote",
      "add",
      "origin",
      "https://github.example.invalid/Example/monorepo.git",
    ],
    { cwd: fixture, windowsHide: true, stdio: "pipe" },
  );
  writeFileSync(join(fixture, ".gitignore"), "node_modules/\n");
  execFileSync("git", ["add", "."], {
    cwd: fixture,
    windowsHide: true,
    stdio: "pipe",
  });
  execFileSync("git", ["commit", "-m", "verified fixture"], {
    cwd: fixture,
    windowsHide: true,
    stdio: "pipe",
  });
  writeFileSync(
    join(temp, "fixed-head"),
    execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: fixture,
      windowsHide: true,
      encoding: "utf8",
    }),
  );
  await native("rescan_project", { project: project.id });
  await native("save_settings", githubSettings);
  await page.setViewportSize({ width: 1400, height: 1000 });
  await page.getByRole("button", { name: "プロジェクト", exact: true }).click();
  await page.getByRole("button", { name: "GitHubから", exact: true }).click();
  await page
    .getByRole("checkbox", { name: "Example/monorepoを調査対象に選択" })
    .check();
  await page
    .getByRole("checkbox", { name: "Example/deniedを調査対象に選択" })
    .check();
  await page
    .getByRole("button", {
      name: "選択したリポジトリを調査（2件）",
      exact: true,
    })
    .click();
  await page
    .getByRole("heading", { name: "プロジェクト", exact: true })
    .waitFor();
  await page
    .getByRole("region", { name: "プロジェクト一覧", exact: true })
    .waitFor();
  await page
    .getByRole("checkbox", { name: "Example/monorepo (main)を照合対象に選択" })
    .check();
  await page
    .getByRole("button", {
      name: "選択したGitHub結果を照合（1件）",
      exact: true,
    })
    .click();
  await page
    .getByText("選択したGitHub依存を照合中…", { exact: false })
    .waitFor({ state: "hidden", timeout: 120000 });
  await page
    .getByRole("button", {
      name: "Example/monorepoのGitHub調査・mainを開く",
      exact: true,
    })
    .click();
  await page
    .getByRole("table", { name: "GitHubの更新候補", exact: true })
    .waitFor();
  await page
    .getByRole("button", { name: "更新方針を作成", exact: true })
    .click();
  await page
    .getByRole("heading", { name: "更新方針 （静的な計画）", exact: false })
    .waitFor();
  const githubSnapshot = await native("snapshot");
  assert.equal(
    githubSnapshot.projects.length,
    1,
    "Remote analysis must not register a checkout",
  );
  assert.equal(githubSnapshot.github_projects.length, 1);
  assert(
    githubSnapshot.github_plans[0].changes.some(
      (c) => c.name === "kleur" && c.candidate === "4.1.5",
    ),
  );
  assert.equal(
    await page
      .getByRole("navigation", { name: "メインナビゲーション" })
      .locator('[aria-current="page"]')
      .getAttribute("aria-label"),
    "更新",
  );
  await page.getByRole("button", { name: "プロジェクト", exact: true }).click();
  assert.equal(
    await page.locator(".catalog-group").count(),
    1,
    "Remote/local snapshots must share one logical repository",
  );
  await page
    .getByRole("button", {
      name: "Example/monorepoのGitHub調査・mainを開く",
      exact: true,
    })
    .click();
  const isolatedDb = new DatabaseSync(githubSnapshot.data_path);
  const simulated = {
    id: "wsl:display-fixture",
    name: "WSL表示試験",
    kind: "wsl",
    home: "/tmp/fixture",
    os: "linux",
  };
  isolatedDb
    .prepare("INSERT INTO records VALUES (?,?,?)")
    .run("environments", simulated.id, JSON.stringify(simulated));
  const managerFixture = join(temp, "manager updates fixture.exe");
  execFileSync(
    join(process.env.USERPROFILE, ".cargo/bin/rustc.exe"),
    [
      "--edition=2024",
      "scripts/fixtures/discovery-cli.rs",
      "-o",
      managerFixture,
    ],
    { windowsHide: true, stdio: "pipe" },
  );
  const systemFixtureState = await native("snapshot");
  const localInv = systemFixtureState.inventories.find(
    (i) => i.environment === "local",
  );
  localInv.tools.push({
    id: "local:winget:Example.App",
    environment: "local",
    manager: "winget",
    name: "Example.App",
    version: "1.0.0",
    path: "",
    source: "winget",
    requested: "Example.App",
    installed: true,
    active: false,
    note: "Fixture",
  });
  isolatedDb
    .prepare("INSERT OR REPLACE INTO records VALUES (?,?,?)")
    .run("inventories", "local", JSON.stringify(localInv));
  systemFixtureState.settings.manager_overrides.push([
    "local",
    "winget",
    managerFixture,
  ]);
  await native("save_settings", systemFixtureState.settings);
  isolatedDb.close();
  // Refresh through a read-only operation, then change comparison scope without hiding the project.
  await page
    .getByRole("button", { name: "更新候補・脆弱性を照合", exact: true })
    .click();
  await page
    .getByText("GitHub依存の更新候補を照合中…", { exact: false })
    .waitFor({ state: "hidden", timeout: 120000 });
  await page
    .getByRole("combobox", { name: "比較・登録する環境" })
    .selectOption(simulated.id);
  assert(
    await page.getByRole("table", { name: "GitHubの依存宣言" }).isVisible(),
  );
  assert.equal(await page.locator(".catalog-group").count(), 1);
  await page.screenshot({
    path: "test-results/native-projects-unified.png",
    fullPage: true,
  });
  await page.getByRole("button", { name: "更新", exact: true }).click();
  await page.getByRole("tab", { name: "計画・進行状況", exact: true }).click();
  await page
    .locator(".plan-catalog button")
    .filter({ hasText: "Example/monorepo" })
    .click();
  await page
    .getByRole("button", { name: "コミットと作業状態を再確認", exact: true })
    .click();
  await page
    .getByText("対象コミットと作業状態を確認しました。", { exact: false })
    .waitFor();
  assert.equal(
    await page
      .getByRole("combobox", { name: "比較・登録する環境" })
      .inputValue(),
    simulated.id,
  );
  assert((await native("snapshot")).github_plans[0].workspace === project.id);
  await page.getByRole("button", { name: "更新", exact: true }).click();
  assert.equal(
    await page.getByRole("combobox", { name: "比較・登録する環境" }).count(),
    0,
    "Update targets belong to each plan, not a global filter",
  );
  await page.getByRole("tab", { name: "計画・進行状況", exact: true }).click();
  await page
    .locator(".plan-catalog button")
    .filter({ hasText: "Example/monorepo" })
    .click();
  await page.screenshot({
    path: "test-results/native-github-analysis.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 780, height: 800 });
  assert.equal(
    await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    ),
    false,
    "GitHub page should not overflow horizontally",
  );
  // Update actions belong to Updates, with their own explicit environment target.
  await page.getByRole("button", { name: "環境・ツール", exact: true }).click();
  assert.equal(
    await page
      .getByRole("heading", { name: "OS・アプリ管理元の更新", exact: true })
      .count(),
    0,
  );
  // Each tool carries its exact environment and installed version into Updates.
  const installed = (await native("snapshot")).inventories
    .find((i) => i.environment === "local")
    .tools.find((t) => t.manager === "mise" && t.installed);
  assert(installed);
  await page
    .getByRole("combobox", { name: "操作する環境", exact: true })
    .selectOption("local");
  assert.equal(
    await page
      .getByRole("button", { name: "更新候補を照合", exact: true })
      .count(),
    0,
  );
  await page
    .getByRole("button", {
      name: `${installed.name} ${installed.version}の更新・整理へ`,
      exact: true,
    })
    .click();
  assert.equal(
    await page.getByLabel("更新操作の対象環境", { exact: true }).inputValue(),
    "local",
  );
  assert.equal(
    await page.getByLabel("更新・整理するツール", { exact: true }).inputValue(),
    installed.id,
  );
  assert.equal(
    await page
      .getByRole("navigation", { name: "メインナビゲーション" })
      .locator('[aria-current="page"]')
      .getAttribute("aria-label"),
    "更新",
  );
  assert.equal(
    await page
      .getByRole("button", { name: "ツールの更新候補を照合", exact: true })
      .count(),
    1,
  );
  // Changing the operation environment must drop a tool chosen in another environment.
  await page
    .getByLabel("更新操作の対象環境", { exact: true })
    .selectOption(simulated.id);
  assert.equal(
    await page.getByLabel("更新・整理するツール", { exact: true }).inputValue(),
    "",
  );
  // WinGet paths carry the package ID, and only open the review screen.
  const winget = (await native("snapshot")).inventories
    .find((i) => i.environment === "local")
    .tools.find((t) => t.manager === "winget");
  if (winget) {
    await page
      .getByRole("button", { name: "環境・ツール", exact: true })
      .click();
    await page
      .getByRole("combobox", { name: "操作する環境", exact: true })
      .selectOption("local");
    await page
      .getByRole("row")
      .filter({ hasText: winget.name })
      .getByRole("button", { name: "更新へ", exact: true })
      .click();
    assert.equal(
      await page
        .getByLabel("更新するWinGetパッケージID", { exact: true })
        .inputValue(),
      winget.name,
    );
  }
  await page.getByRole("button", { name: "環境・ツール", exact: true }).click();
  await page
    .getByRole("combobox", { name: "操作する環境", exact: true })
    .selectOption(simulated.id);
  await page
    .getByRole("button", { name: "この環境の更新へ", exact: true })
    .click();
  await page
    .getByRole("heading", { name: "環境の更新操作", exact: true })
    .waitFor();
  assert.equal(
    await page.getByLabel("更新操作の対象環境", { exact: true }).inputValue(),
    simulated.id,
  );
  await page
    .getByLabel("更新操作の対象環境", { exact: true })
    .selectOption("local");
  await page.getByRole("tab", { name: "OS・アプリ", exact: true }).click();
  await page
    .getByLabel("更新するWinGetパッケージID", { exact: true })
    .waitFor();
  await page
    .getByRole("button", { name: "更新一覧を取得", exact: true })
    .click();
  const systemTable = page.getByRole("table", {
    name: "OS・アプリの更新候補",
    exact: true,
  });
  await systemTable.waitFor();
  assert.equal(
    await systemTable.getByRole("cell", { name: /^Example.App/ }).count(),
    1,
  );
  await systemTable
    .getByRole("button", { name: "この候補の計画を作成", exact: true })
    .click();
  await page
    .getByRole("heading", { name: "WinGet: Example.App の更新", exact: true })
    .waitFor();
  const evidenceDetails = page.locator(".evidence-detail");
  assert.equal(await evidenceDetails.count(), 1);
  assert.equal(await evidenceDetails.getAttribute("open"), null);
  await evidenceDetails
    .getByText("配布情報・取得結果の詳細", { exact: true })
    .click();
  assert(await evidenceDetails.locator("pre").isVisible());
  await evidenceDetails
    .getByText("配布情報・取得結果の詳細", { exact: true })
    .click();
  const savedSystemPlan = (await native("snapshot")).plans.find(
    (p) => p.title === "WinGet: Example.App の更新",
  );
  assert(savedSystemPlan.steps[0].args.includes("1.1.0"));
  assert(savedSystemPlan.steps[0].args.includes("--version"));
  await page.getByRole("tab", { name: "環境の更新", exact: true }).click();
  await page.getByRole("tab", { name: "管理ツール本体", exact: true }).click();
  const previousPlanCount = (await native("snapshot")).plans.length;
  await page
    .getByRole("button", { name: "mise の更新を計画", exact: true })
    .click();
  await page.getByText(/jdx\.mise はWinGetで更新候補がありません/).waitFor();
  assert.equal((await native("snapshot")).plans.length, previousPlanCount);
  assert.equal(
    await page
      .getByRole("tab", { name: "環境の更新", exact: true })
      .getAttribute("aria-selected"),
    "true",
  );
  await page.getByRole("tab", { name: "開発ツール", exact: true }).click();
  const candidateTab = page.getByRole("tab", {
    name: "環境の更新",
    exact: true,
  });
  const planTab = page.getByRole("tab", {
    name: "計画・進行状況",
    exact: true,
  });
  for (const width of [1400, 780]) {
    await page.setViewportSize({ width, height: 900 });
    const first = await candidateTab.boundingBox(),
      second = await planTab.boundingBox();
    assert(first && second);
    assert(
      Math.abs(first.y - second.y) < 1,
      "Update tabs must share a horizontal row",
    );
    assert(second.x >= first.x + first.width, "Tabs must not overlap");
    assert(
      first.width + second.width < width - 70,
      "Tabs should not become full-width stacked buttons",
    );
  }
  await candidateTab.focus();
  await candidateTab.press("ArrowRight");
  const projectTab = page.getByRole("tab", {
    name: "プロジェクトの更新",
    exact: true,
  });
  assert.equal(await projectTab.getAttribute("aria-selected"), "true");
  assert.equal(
    await page
      .getByRole("table", { name: "更新対象の一覧", exact: true })
      .getByRole("cell", { name: /^kleur/ })
      .count(),
    2,
  );
  assert.equal(
    await page.getByLabel("更新操作の対象環境", { exact: true }).count(),
    0,
  );
  await projectTab.press("ArrowRight");
  assert.equal(await planTab.getAttribute("aria-selected"), "true");
  assert(await planTab.evaluate((el) => el === document.activeElement));
  await planTab.press("ArrowRight");
  assert.equal(await candidateTab.getAttribute("aria-selected"), "true");
  await candidateTab.press("End");
  assert.equal(await planTab.getAttribute("aria-selected"), "true");
  await planTab.press("Home");
  assert.equal(await candidateTab.getAttribute("aria-selected"), "true");
  assert.equal(
    await page
      .getByRole("tabpanel", { name: "環境の更新", exact: true })
      .count(),
    1,
  );
  assert(
    await page.evaluate(() => {
      const controls = document.querySelector("#environment-update-controls");
      const candidates = [...document.querySelectorAll("h2")].find(
        (h) => h.textContent === "開発ツールの更新候補",
      );
      return (
        !!controls &&
        !!candidates &&
        !!(
          controls.compareDocumentPosition(candidates) &
          Node.DOCUMENT_POSITION_FOLLOWING
        )
      );
    }),
    "Operations must precede candidate results regardless of result count",
  );
  const visibleAction = async (locator) => {
    const box = await locator.boundingBox();
    assert(
      box && box.y >= 0 && box.y + box.height <= 900,
      "Primary update action must be above the fold",
    );
  };
  for (const width of [1400, 780]) {
    await page.setViewportSize({ width, height: 900 });
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.getByRole("tab", { name: "開発ツール", exact: true }).click();
    await visibleAction(page.getByLabel("更新操作の対象環境", { exact: true }));
    await visibleAction(
      page.getByRole("button", { name: "ツールの更新候補を照合", exact: true }),
    );
    assert.equal(
      await page
        .getByRole("button", { name: "更新一覧を取得", exact: true })
        .count(),
      0,
    );
    await page.getByRole("tab", { name: "OS・アプリ", exact: true }).click();
    await visibleAction(
      page.getByRole("button", { name: "更新一覧を取得", exact: true }),
    );
    assert.equal(
      await page.getByLabel("更新・整理するツール", { exact: true }).count(),
      0,
    );
    await page
      .getByRole("tab", { name: "管理ツール本体", exact: true })
      .click();
    await visibleAction(
      page.getByRole("button", { name: "mise の更新を計画", exact: true }),
    );
    await page.getByRole("tab", { name: "開発ツール", exact: true }).click();
    assert.equal(
      await page.evaluate(
        () => document.documentElement.scrollWidth > innerWidth,
      ),
      false,
    );
  }
  // Negative child margins must not create a one-pixel vertical scroll range.
  for (const width of [1400, 780, 480]) {
    await page.setViewportSize({ width, height: 900 });
    for (const label of ["更新の表示", "環境更新の分類"]) {
      const bar = page.getByRole("tablist", { name: label, exact: true });
      const tabs = bar.getByRole("tab");
      const assertNoVerticalScroll = async () => {
        const geometry = await bar.evaluate((el) => ({
          height: el.clientHeight,
          scroll: el.scrollHeight,
          overflow: getComputedStyle(el).overflowY,
        }));
        assert(
          geometry.scroll <= geometry.height,
          `${label} must not have a vertical scroll range at ${width}px`,
        );
        assert.equal(geometry.overflow, "hidden");
      };
      await assertNoVerticalScroll();
      for (let i = 0; i < (await tabs.count()); i++) {
        await tabs.nth(i).hover();
        await tabs.nth(i).focus();
        await assertNoVerticalScroll();
      }
      const last = tabs.last();
      await last.focus();
      const box = await last.boundingBox(),
        container = await bar.boundingBox();
      assert(
        box &&
          container &&
          box.x >= container.x - 1 &&
          box.x + box.width <= container.x + container.width + 1,
        "Focused tabs must remain reachable on narrow screens",
      );
    }
    assert.equal(
      await page.evaluate(
        () => document.documentElement.scrollWidth > innerWidth,
      ),
      false,
    );
  }
  await page.setViewportSize({ width: 780, height: 900 });
  await page.getByRole("tab", { name: "開発ツール", exact: true }).focus();
  await page.screenshot({
    path: "test-results/native-update-tabs-narrow.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 1400, height: 1000 });
  await page.screenshot({
    path: "test-results/native-update-tabs.png",
    fullPage: true,
  });
  // Start on candidates to prove the overview shortcut chooses plans explicitly.
  await page.getByRole("tab", { name: "環境の更新", exact: true }).click();
  await page.getByRole("button", { name: "概要", exact: true }).click();
  await page
    .getByRole("button", { name: "更新計画を見る", exact: true })
    .click();
  assert.equal(
    await page
      .getByRole("tab", { name: "計画・進行状況", exact: true })
      .getAttribute("aria-selected"),
    "true",
  );
  await page.setViewportSize({ width: 1400, height: 1000 });
  await page
    .locator(".plan-catalog button")
    .filter({ hasText: "Example/monorepo" })
    .click();
  // Move the generated source beyond the audited revision. The clone must open the audited commit.
  const auditedHead = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: fixture,
    encoding: "utf8",
    windowsHide: true,
  }).trim();
  execFileSync("git", ["commit", "--allow-empty", "-m", "newer source head"], {
    cwd: fixture,
    windowsHide: true,
    stdio: "pipe",
  });
  writeFileSync(join(temp, "clone-source"), fixture);
  const cloneParent = join(temp, "clones");
  mkdirSync(cloneParent);
  await page
    .getByRole("button", { name: "GitHubから作業環境を用意", exact: true })
    .click();
  const dialog = page.getByRole("dialog");
  await dialog.locator(".repo-row").first().waitFor();
  assert.equal(await dialog.locator(".repo-row").count(), 1);
  assert.equal(await dialog.locator("#github-reference").inputValue(), "main");
  assert(
    await dialog.locator("#github-reference").evaluate((el) => el.readOnly),
  );
  await dialog
    .getByLabel("クローン先の親フォルダー", { exact: true })
    .fill(cloneParent);
  await dialog.getByRole("button", { name: "クローン", exact: true }).click();
  await dialog.waitFor({ state: "hidden", timeout: 120000 });
  await page
    .getByRole("button", { name: "元の更新方針で作業環境を確認", exact: true })
    .waitFor();
  assert.equal(
    await page
      .getByRole("navigation", { name: "メインナビゲーション" })
      .locator('[aria-current="page"]')
      .getAttribute("aria-label"),
    "プロジェクト",
  );
  assert.equal(
    execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: join(cloneParent, "monorepo"),
      encoding: "utf8",
      windowsHide: true,
    }).trim(),
    auditedHead,
  );
  await page
    .getByRole("button", { name: "元の更新方針で作業環境を確認", exact: true })
    .click();
  await page
    .getByRole("heading", { name: "更新方針 （静的な計画）", exact: false })
    .waitFor();
  await reviewUi({ page, native, cloneParent });
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
  mkdirSync("test-results", { recursive: true });
  writeFileSync("test-results/native-smoke-startup.log", logs);
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
