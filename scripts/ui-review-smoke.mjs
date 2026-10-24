// All operations use the parent test's isolated data and generated repositories.
import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
export async function reviewUi({ page, native, cloneParent }) {
  // Modal failures must be visible in the modal, not in the inert page behind it.
  await page
    .getByRole("button", { name: "GitHubから作業環境を用意", exact: true })
    .click();
  let dialog = page.getByRole("dialog");
  await dialog
    .getByLabel("クローン先の親フォルダー", { exact: true })
    .fill(cloneParent);
  await dialog.getByRole("button", { name: "クローン", exact: true }).click();
  await dialog.getByRole("alert").waitFor();
  assert(
    (await dialog.getByRole("alert").innerText()).includes("同名フォルダー"),
  );
  await page.screenshot({
    path: "test-results/ui-review-modal-error.png",
    fullPage: true,
  });
  await page.keyboard.press("Escape");
  await dialog.waitFor({ state: "hidden" });
  // Settings drafts survive manager switches and failed saves; no real settings are saved here.
  await page.getByRole("button", { name: "設定", exact: true }).click();
  const picker = page.getByRole("combobox", {
    name: "管理ツール",
    exact: true,
  });
  await picker.selectOption("go");
  const input = page.getByLabel("実行ファイルの絶対パス", { exact: true });
  const draft = "Z:/ui-review/unsaved-tool.exe";
  await input.fill(draft);
  await picker.selectOption("deno");
  await picker.selectOption("go");
  assert.equal(await input.inputValue(), draft);
  const isolatedState = await native("snapshot");
  const lock = new DatabaseSync(isolatedState.data_path);
  lock.exec("BEGIN IMMEDIATE");
  try {
    await page.getByRole("button", { name: "保存", exact: true }).click();
    await page.getByRole("alert").waitFor();
  } finally {
    lock.exec("ROLLBACK");
    lock.close();
  }
  assert.equal(await input.inputValue(), draft);
  assert(
    (
      await page.getByRole("status").filter({ hasText: "未保存" }).innerText()
    ).includes("未保存"),
  );
  await page.screenshot({
    path: "test-results/ui-review-settings-error.png",
    fullPage: true,
  });
  await page
    .getByRole("button", { name: "エラーを閉じる", exact: true })
    .click();
  await picker.selectOption("mise");
  // All six pages at a normal size and the application's minimum size.
  for (const size of [
    { width: 1400, height: 900 },
    { width: 760, height: 600 },
  ]) {
    await page.setViewportSize(size);
    for (const [name, slug] of [
      ["概要", "overview"],
      ["環境・ツール", "environment"],
      ["プロジェクト", "projects"],
      ["更新", "updates"],
      ["履歴", "history"],
      ["設定", "settings"],
    ]) {
      const nav = page
        .getByRole("navigation", {
          name:
            name === "履歴" || name === "設定"
              ? "補助ナビゲーション"
              : "メインナビゲーション",
        })
        .getByRole("button", { name, exact: true });
      await nav.click();
      assert.equal(await nav.getAttribute("aria-current"), "page");
      assert.equal(
        await page.evaluate(
          () => document.documentElement.scrollWidth > innerWidth,
        ),
        false,
        `${name} must not overflow at ${size.width}px`,
      );
      assert.equal(
        await page
          .getByRole("navigation", { name: "補助ナビゲーション" })
          .getByRole("button")
          .count(),
        2,
      );
      await page.screenshot({
        path: `test-results/ui-review-${slug}-${size.width}.png`,
        fullPage: true,
      });
    }
  }
  // Search tools stay usable with history and saved plans already present.
  await page.getByRole("button", { name: "履歴", exact: true }).click();
  await page
    .getByLabel("履歴を検索", { exact: true })
    .fill("no-matching-ui-review-history");
  await page
    .getByText("条件に一致する履歴はありません。", { exact: true })
    .waitFor();
  await page.getByLabel("履歴を検索", { exact: true }).fill("");
  assert((await page.locator(".history-row").count()) > 0);
  await page.getByRole("button", { name: "更新", exact: true }).click();
  await page.getByRole("tab", { name: "計画・進行状況", exact: true }).click();
  await page
    .getByLabel("計画を検索", { exact: true })
    .fill("no-matching-ui-review-plan");
  await page
    .getByText(
      "条件に一致する計画はありません。検索・絞り込みを変更してください。",
      { exact: true },
    )
    .waitFor();
  await page.getByLabel("計画を検索", { exact: true }).fill("");
  await page
    .getByLabel("計画の状態で絞り込み", { exact: true })
    .selectOption("complete");
  assert((await page.locator(".plan-catalog button").count()) > 0);
  await page
    .getByLabel("計画の状態で絞り込み", { exact: true })
    .selectOption("all");
  await page
    .locator(".plan-catalog button")
    .filter({ hasText: "Example/monorepo" })
    .click();
  await page
    .getByRole("button", { name: "計画一覧へ戻る", exact: true })
    .click();
  await page.waitForFunction(
    () => document.activeElement.id === "plan-catalog",
  );
  await page
    .locator(".plan-catalog button")
    .filter({ hasText: "Example/monorepo" })
    .click();
  await page.waitForFunction(
    () => document.activeElement.id === "update-plan-detail",
  );
  await page.getByRole("button", { name: "プロジェクト", exact: true }).click();
  await page
    .getByRole("button", {
      name: "Example/monorepoのGitHub調査・mainを開く",
      exact: true,
    })
    .click();
  await page.getByRole("button", { name: "詳細を閉じる", exact: true }).click();
  await page.waitForFunction(
    () => document.activeElement.id === "project-catalog",
  );
  const snapshot = await native("snapshot");
  assert(
    snapshot.settings.manager_overrides.every(
      ([, name, path]) => name !== "go" || path !== draft,
    ),
    "Failed save must not modify settings",
  );
  console.log(
    JSON.stringify({
      uiReview: true,
      pages: 6,
      widths: [1400, 760],
      modalErrorsVisible: true,
      draftPreservedOnFailure: true,
      searchAndReturnPaths: true,
    }),
  );
}
