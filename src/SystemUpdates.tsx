import { useState } from "react";
import type { Update } from "./types";
import { statusLabel, displayDate } from "./logic";
export function SystemUpdates({
  updates,
  busy,
  windows,
  onPlan,
  checkedAt,
  structured = true,
}: {
  updates: Update[];
  checkedAt?: string;
  structured?: boolean;
  busy: boolean;
  windows: boolean;
  onPlan: (item: Update) => void;
}) {
  const [all, setAll] = useState(false);
  const available = updates.filter(
    (u) => u.candidate && u.importance !== "delegated",
  );
  const rows = all ? updates : available;
  return (
    <section className="system-candidates" aria-label="OS・アプリの照合結果">
      <div className="section-heading">
        <h3>取得した候補</h3>
        <label>
          <input
            type="checkbox"
            checked={all}
            onChange={(e) => setAll(e.target.checked)}
          />
          掲載なし・別管理元も表示
        </label>
      </div>
      {checkedAt && <p className="muted">取得日時：{displayDate(checkedAt)}</p>}
      <p className="muted">
        候補 {available.length}件 ／ 一覧に掲載なし{" "}
        {updates.filter((u) => u.importance === "not-listed").length}件 ／
        別管理元 {updates.filter((u) => u.importance === "delegated").length}件
        ／ 導入版不明{" "}
        {updates.filter((u) => u.importance === "version-unknown").length}件
      </p>
      {checkedAt && !structured ? (
        <p>
          この管理元は取得ログで確認してください。一覧への変換は未対応です。
        </p>
      ) : !checkedAt ? (
        <p>
          「更新一覧を取得」で確認してください。未取得は最新という判定ではありません。
        </p>
      ) : !rows.length ? (
        <p>
          今回の一覧に更新候補はありません。固定・不明な版・対象外を含むため、すべて最新とは断定できません。
        </p>
      ) : (
        <div className="table-scroll">
          <table aria-label="OS・アプリの更新候補">
            <thead>
              <tr>
                <th>パッケージ</th>
                <th>現在 → 候補</th>
                <th>取得状況</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((u) => (
                <tr key={u.tool_id ?? u.name}>
                  <td>
                    {u.name}
                    <small>{u.manager}</small>
                  </td>
                  <td>
                    {u.current || "版不明"} → {u.candidate || "候補なし"}
                  </td>
                  <td>
                    {statusLabel[u.importance] ?? u.importance}
                    <small>{u.note}</small>
                  </td>
                  <td>
                    {windows &&
                    u.manager === "winget" &&
                    u.source === "winget" &&
                    u.candidate ? (
                      <button disabled={busy} onClick={() => onPlan(u)}>
                        この候補の計画を作成
                      </button>
                    ) : (
                      <span className="muted">
                        {u.manager === "apt"
                          ? "OSの管理元で更新"
                          : "計画対象外"}
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
