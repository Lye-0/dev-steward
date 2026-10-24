import { useState } from "react";
import { displayDate, statusLabel } from "./logic";
import type { History } from "./types";
export function HistoryPanel({ items }: { items: History[] }) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const rows = items.filter(
    (h) =>
      `${h.action} ${h.target} ${h.detail}`
        .toLowerCase()
        .includes(query.toLowerCase()) &&
      (filter === "all" ||
        (filter === "attention"
          ? ["error", "failed", "cancelled", "interrupted"].includes(h.status)
          : ["complete", "success", "ok"].includes(h.status))),
  );
  return (
    <section className="panel details-panel">
      <div className="toolbar">
        <div className="search">
          <input
            aria-label="履歴を検索"
            placeholder="操作・対象・記録で検索"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <select
          aria-label="履歴の状態で絞り込み"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        >
          <option value="all">すべての状態</option>
          <option value="attention">失敗・中断</option>
          <option value="complete">完了した操作</option>
        </select>
      </div>
      <p className="muted">
        {rows.length}件／全{items.length}件
      </p>
      {!rows.length && (
        <p className="padded">条件に一致する履歴はありません。</p>
      )}
      {rows.map((h) => (
        <details className="history-row" key={h.id}>
          <summary>
            <strong>{h.action}</strong>
            <span className="history-target">{h.target}</span>
            <span className={`badge ${h.status}`}>
              {statusLabel[h.status] ?? h.status}
            </span>
            <time>{displayDate(h.at)}</time>
          </summary>
          <pre>{h.detail}</pre>
        </details>
      ))}
    </section>
  );
}
