import { managerLabels } from "./logic";
import type { Tool, Update } from "./types";
export function ToolUpdates({
  tools,
  updates,
  selected,
  busy,
  scanned,
  windows,
  onSelect,
  onCheck,
  onPlan,
  onCleanup,
}: {
  tools: Tool[];
  updates: Update[];
  selected: string;
  busy: boolean;
  scanned: boolean;
  windows: boolean;
  onSelect: (id: string) => void;
  onCheck: () => void;
  onPlan: (id: string) => void;
  onCleanup: (id: string) => void;
}) {
  const choices = tools.filter((t) => t.manager !== "winget");
  const tool = choices.find((t) => t.id === selected);
  const update = updates.find((u) => u.tool_id === tool?.id);
  const supported =
    tool &&
    [
      "mise",
      "uv",
      "rustup",
      "fnm",
      "nvm",
      "pyenv",
      "python-install-manager",
      "conda",
    ].includes(tool.manager);
  const ordinaryPython = /^(?:PythonCore[\\/])?3\.\d+(?:\.\d+)?$/.test(
    tool?.requested ?? "",
  );
  const canPlan =
    supported && (tool?.manager !== "python-install-manager" || ordinaryPython);
  return (
    <section
      aria-label="開発ツールの更新と整理"
      className="tool-update-controls"
    >
      <h3>開発ツールの更新と整理</h3>
      <div className="tool-update-toolbar">
        <div className="maintenance-target">
          <label htmlFor="update-tool">更新・整理するツール</label>
          <select
            id="update-tool"
            value={tool?.id ?? ""}
            disabled={busy || !choices.length}
            onChange={(e) => onSelect(e.target.value)}
          >
            <option value="">対象を選択</option>
            {choices.map((t) => (
              <option key={t.id} value={t.id}>
                {managerLabels[t.manager] ?? t.manager} · {t.name} {t.version}
                {t.manager === "conda" ? ` · ${t.source}` : ""}
              </option>
            ))}
          </select>
        </div>
        <button disabled={busy || !scanned} onClick={onCheck}>
          ツールの更新候補を照合
        </button>
      </div>
      {!scanned && (
        <p className="muted">
          この環境は未調査です。「環境・ツール」で調査してください。
        </p>
      )}
      {tool && (
        <div className="tool-update-selection">
          <strong>
            {tool.name} {tool.version}
          </strong>
          <p className="muted">
            {!supported
              ? "この管理元の自動更新には未対応です。"
              : update?.candidate
                ? `${tool.version} → ${update.candidate}`
                : !update
                  ? "候補は未照合です。"
                  : ["unknown", "error"].includes(update.importance)
                    ? "候補を確認できていません。詳細を確認してください。"
                    : "更新候補なし"}
          </p>
          <p className="muted">
            候補取得：{supported ? "対応" : "導入元の確認が必要"} ／ 更新計画：
            {canPlan ? "対応" : "管理元で確認"}
          </p>
          <div className="button-row">
            {canPlan && (
              <button
                disabled={busy || !update?.candidate}
                onClick={() => onPlan(tool.id)}
              >
                選択したツールの更新計画を作成
              </button>
            )}
            {windows &&
              tool.manager === "mise" &&
              tool.installed &&
              !tool.active && (
                <button disabled={busy} onClick={() => onCleanup(tool.id)}>
                  旧版の削除可否を確認
                </button>
              )}
          </div>
          <details className="update-output">
            <summary>配置・候補の詳細</summary>
            <p className="path">{tool.path || "配置先は未取得"}</p>
            <p>
              {!supported
                ? "この管理元の自動更新には未対応です。"
                : update
                  ? update.candidate
                    ? `更新候補: ${update.candidate}`
                    : update.note || "更新候補なし"
                  : "更新候補は未照合です。"}
            </p>
            {update?.candidate && <p className="muted">{update.note}</p>}
          </details>
        </div>
      )}
    </section>
  );
}
