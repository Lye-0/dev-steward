import { useState } from "react";
import { displayDate } from "./logic";
import { projectGroups, type ProjectTarget } from "./workspace";
import type { Snapshot } from "./types";
export function ProjectCatalog({
  data,
  selected,
  onSelect,
  busy,
  onAudit,
}: {
  data: Snapshot;
  selected: ProjectTarget;
  onSelect: (target: ProjectTarget) => void;
  busy: boolean;
  onAudit: (ids: string[]) => void;
}) {
  const [checked, setChecked] = useState<string[]>([]);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const groups = projectGroups(data.projects, data.github_projects);
  const visible = groups.filter(
    (g) =>
      (filter !== "local" || g.locals.length > 0) &&
      (filter !== "github" || g.reports.length > 0) &&
      `${g.name} ${g.locals.map((p) => `${p.path} ${p.branch}`).join(" ")} ${g.reports.map((r) => `${r.repository} ${r.branch}`).join(" ")}`
        .toLowerCase()
        .includes(query.toLowerCase()),
  );
  return (
    <section
      className="panel details-panel project-catalog"
      id="project-catalog"
      tabIndex={-1}
      aria-label="プロジェクト一覧"
    >
      <div className="section-heading">
        <h2>
          登録プロジェクト{" "}
          <small>
            {groups.length}件 / 作業場所 {data.projects.length}件・GitHub調査{" "}
            {data.github_projects.length}件
          </small>
        </h2>
      </div>
      <div className="toolbar">
        <div className="search">
          <input
            aria-label="プロジェクトを検索"
            placeholder="名前・ブランチ・配置先で検索"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <select
          aria-label="プロジェクトの登録元で絞り込み"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        >
          <option value="all">すべて</option>
          <option value="local">ローカル登録あり</option>
          <option value="github">GitHub調査あり</option>
        </select>
      </div>
      <div className="button-row">
        <button
          disabled={busy || !checked.length || checked.length > 50}
          onClick={() => onAudit(checked)}
        >
          選択したGitHub結果を照合（{checked.length}件）
        </button>
      </div>
      <p className="muted">
        GitHubの一括照合は最大50件です。パッケージ名・版を公開レジストリとOSVへ送信します。
      </p>
      {data.github_errors.length > 0 && (
        <details className="plan-warnings">
          <summary>
            直近のGitHub取得・照合の未確認事項（{data.github_errors.length}件）
          </summary>
          {data.github_errors.map((e, i) => (
            <p key={i}>
              {e.scope}: {e.message}
            </p>
          ))}
          <p>失敗した対象の以前の結果は更新していません。</p>
        </details>
      )}
      <div className="catalog-groups">
        {visible.map((g) => (
          <section className="catalog-group" key={g.key}>
            <h3>{g.name}</h3>
            {g.locals.map((p) => {
              const updates = data.updates.filter((u) => u.project_id === p.id);
              return (
                <button
                  key={p.id}
                  className={`catalog-entry ${selected.kind === "local" && selected.id === p.id ? "active" : ""}`}
                  aria-pressed={
                    selected.kind === "local" && selected.id === p.id
                  }
                  onClick={() => onSelect({ kind: "local", id: p.id })}
                  aria-label={`${p.name}のローカル調査・${data.environments.find((e) => e.id === p.environment)?.name ?? p.environment}を開く`}
                >
                  <span className="source-tag">ローカル登録</span>
                  <span>
                    <strong>
                      {data.environments.find((e) => e.id === p.environment)
                        ?.name ?? p.environment}{" "}
                      · {p.branch || "detached HEAD"}
                    </strong>
                    <small className="path">{p.path}</small>
                    <small>
                      {displayDate(p.scanned_at)} ·{" "}
                      {p.git_status ? "未コミット変更あり" : "静的調査済み"}
                    </small>
                  </span>
                  <small>
                    {updates.length
                      ? `更新候補 ${updates.filter((u) => u.candidate).length}件`
                      : "未照合"}
                  </small>
                </button>
              );
            })}
            {g.reports.map((r) => {
              const updates = data.github_updates.filter(
                (u) => u.project_id === r.id,
              );
              return (
                <div className="catalog-choice" key={r.id}>
                  <input
                    type="checkbox"
                    aria-label={`${r.repository} (${r.branch})を照合対象に選択`}
                    checked={checked.includes(r.id)}
                    onChange={(e) =>
                      setChecked((current) =>
                        e.target.checked
                          ? [...current, r.id]
                          : current.filter((id) => id !== r.id),
                      )
                    }
                  />
                  <button
                    key={r.id}
                    className={`catalog-entry ${selected.kind === "github" && selected.id === r.id ? "active" : ""}`}
                    aria-pressed={
                      selected.kind === "github" && selected.id === r.id
                    }
                    onClick={() => onSelect({ kind: "github", id: r.id })}
                    aria-label={`${r.repository}のGitHub調査・${r.branch}を開く`}
                  >
                    <span className="source-tag github">GitHub調査</span>
                    <span>
                      <strong>
                        {r.branch} · {r.commit.slice(0, 12)}
                      </strong>
                      <small>
                        {r.host} · {displayDate(r.project.scanned_at)}
                      </small>
                      <small>
                        {r.complete
                          ? "静的取得済み・実行検証は未実施"
                          : "取得・解析に未完了あり"}
                      </small>
                    </span>
                    <small>
                      {updates.length
                        ? `更新候補 ${updates.filter((u) => u.candidate).length}件`
                        : "未照合"}
                    </small>
                  </button>
                </div>
              );
            })}
          </section>
        ))}
        {!visible.length && (
          <p className="muted padded">
            {groups.length
              ? "表示条件に一致するプロジェクトはありません。"
              : "フォルダーまたはGitHubからプロジェクトを登録してください。"}
          </p>
        )}
      </div>
    </section>
  );
}
