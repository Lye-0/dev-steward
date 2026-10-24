import type { Project, Tool } from "./types";
export const managerLabels: Record<string, string> = {
  mise: "mise 管理",
  uv: "uv 管理",
  rustup: "rustup 管理",
  "python-install-manager": "Python Install Manager 管理",
  "python-launcher": "Python 登録情報（py）",
  pyenv: "pyenv 管理",
  conda: "Conda 管理",
  fnm: "fnm 管理",
  nvm: "nvm-windows 管理",
  system: "OS 管理",
  external: "その他・管理元未確定",
  winget: "WinGet 管理",
};
export function groupTools(
  tools: Tool[],
  search: string,
  includeApps: boolean,
) {
  const groups = new Map<string, Tool[]>();
  const query = search.trim().toLowerCase();
  for (const tool of tools) {
    if (!includeApps && tool.manager === "winget") continue;
    if (
      !`${tool.name} ${tool.version} ${tool.manager} ${managerLabels[tool.manager] ?? ""}`
        .toLowerCase()
        .includes(query)
    )
      continue;
    const rows = groups.get(tool.manager) ?? [];
    rows.push(tool);
    groups.set(tool.manager, rows);
  }
  const order = Object.keys(managerLabels);
  return [...groups].sort(([a], [b]) => {
    const rank = (name: string) =>
      order.includes(name) ? order.indexOf(name) : order.length;
    return rank(a) - rank(b) || a.localeCompare(b);
  });
}
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
  prepared: "作業環境確認済み",
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
