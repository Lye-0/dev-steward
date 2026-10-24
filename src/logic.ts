import type { Project, Tool } from "./types";
export function relatedProjects(tool: Tool, projects: Project[]) {
  return projects.filter(
    (p) =>
      p.environment === tool.environment &&
      (p.requirements.some((r) => r.name === tool.name) ||
        p.venvs.some((v) => v.version === tool.version)),
  );
}
export function executableStep(status: string, steps: { status: string }[]) {
  if (["failed", "cancelled", "interrupted", "running"].includes(status))
    return -1;
  const index = steps.findIndex((s) => s.status === "pending");
  return index >= 0 &&
    steps.slice(0, index).every((s) => s.status === "success")
    ? index
    : -1;
}
export function displayDate(value: string) {
  return value
    ? new Date(value).toLocaleString(undefined, {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      })
    : "未調査";
}
export const statusLabel: Record<string, string> = {
  ok: "取得済み",
  missing: "未検出",
  error: "取得失敗",
  partial: "一部未確認",
  protected: "保護対象",
  ready: "実行待ち",
  pending: "未実行",
  running: "実行中",
  success: "成功",
  complete: "完了",
  failed: "失敗",
  cancelled: "中断",
  interrupted: "結果不明",
  security: "セキュリティ",
  current: "候補なし",
  support: "サポート確認",
  maintenance: "通常更新",
  unknown: "未確認",
};
