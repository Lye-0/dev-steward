import { chromium } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";
const profile = mkdtempSync(join(tmpdir(), "steward-release-"));
const app = spawn(
  resolve("target/release/dev-steward.exe"),
  ["--data-dir", join(profile, "state")],
  {
    windowsHide: true,
    stdio: "pipe",
    env: {
      ...process.env,
      PATH: `${process.env.SystemRoot}\\System32;${process.env.SystemRoot}`,
      WEBVIEW2_USER_DATA_FOLDER: join(profile, "webview"),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: "--remote-debugging-port=9225",
    },
  },
);
let browser;
let logs = "";
app.stderr.on("data", (b) => (logs += b));
app.stdout.on("data", (b) => (logs += b));
async function waitFor(fn) {
  const start = Date.now();
  while (Date.now() - start < 60000) {
    try {
      const result = await fn();
      if (result) return result;
    } catch {}
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`Timeout: ${logs}`);
}
try {
  await waitFor(
    async () => (await fetch("http://127.0.0.1:9225/json/version")).ok,
  );
  browser = await chromium.connectOverCDP("http://127.0.0.1:9225");
  const page = await waitFor(() =>
    browser
      .contexts()
      .flatMap((c) => c.pages())
      .find(
        (p) =>
          p.url().includes("tauri.localhost") || p.url().startsWith("tauri:"),
      ),
  );
  await page.getByRole("heading", { name: "開発環境を、見渡す。" }).waitFor();
  const state = await page.evaluate(() =>
    window.__TAURI_INTERNALS__.invoke("steward", {
      method: "snapshot",
      payload: null,
    }),
  );
  assert(state.data_path.startsWith(profile));
  assert.equal(state.projects.length, 0);
  assert(!page.url().includes("1420"));
  mkdirSync("test-results", { recursive: true });
  await page.screenshot({
    path: "test-results/release-overview.png",
    fullPage: true,
  });
  // Installed and missing managers are optional: app navigation and SQLite work without their PATH entries.
  await page.getByRole("button", { name: "設定", exact: true }).click();
  await page.getByRole("heading", { name: "設定", exact: true }).waitFor();
  console.log(
    JSON.stringify(
      {
        ok: true,
        assetUrl: page.url(),
        managersRemovedFromPath: true,
        externalDevServer: false,
        data: state.data_path,
      },
      null,
      2,
    ),
  );
} finally {
  await browser?.close().catch(() => {});
  try {
    execFileSync("taskkill.exe", ["/PID", String(app.pid), "/T", "/F"], {
      windowsHide: true,
      stdio: "pipe",
    });
  } catch {}
}
