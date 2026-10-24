import { useState } from "react";
import { displayDate, statusLabel } from "./logic";
import type { GithubProposal, Snapshot } from "./types";
import { proposalLocals, type ProjectTarget } from "./workspace";
export function UpdateCandidates({
  data,
  onProject,
  onEnvironment,
}: {
  data: Snapshot;
  onProject: (target: ProjectTarget) => void;
  onEnvironment: (id: string) => void;
}) {
  const [filter, setFilter] = useState("all");
  const rows = [
    ...data.updates.map((u) => ({
      update: u,
      kind: u.tool_id ? "tool" : "local",
    })),
    ...data.github_updates.map((u) => ({ update: u, kind: "github" })),
  ].filter(
    ({ update: u, kind }) =>
      (filter === "all" || filter === kind) &&
      !!(u.candidate || u.vulnerabilities.length || u.importance === "unknown"),
  );
  return (
    <section className="panel details-panel">
      <div className="section-heading">
        <h2>更新候補と未確認事項</h2>
        <select
          aria-label="更新候補の対象"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        >
          <option value="all">すべての対象</option>
          <option value="tool">環境・ツール</option>
          <option value="local">ローカルプロジェクト</option>
          <option value="github">GitHubの調査結果</option>
        </select>
      </div>
      <p className="muted">
        調査した対象環境・コミットごとの照合結果です。計画前に、対象の最新状態と検証方法を確認します。
      </p>
      {rows.length ? (
        <div className="table-scroll">
          <table aria-label="更新対象の一覧">
            <thead>
              <tr>
                <th>対象</th>
                <th>名前</th>
                <th>現在 → 候補</th>
                <th>重要性・未確認事項</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {rows.map(({ update: u, kind }, i) => {
                const local = data.projects.find((p) => p.id === u.project_id);
                const remote = data.github_projects.find(
                  (p) => p.id === u.project_id,
                );
                const inventory = data.inventories.find((inv) =>
                  inv.tools.some((t) => t.id === u.tool_id),
                );
                const label =
                  kind === "github"
                    ? `${remote?.repository ?? "GitHub"} · ${remote?.branch ?? ""} · ${remote?.commit.slice(0, 12) ?? ""}`
                    : kind === "tool"
                      ? (data.environments.find(
                          (e) => e.id === inventory?.environment,
                        )?.name ?? "環境未確認")
                      : `${local?.name ?? "プロジェクト"} · ${data.environments.find((e) => e.id === local?.environment)?.name ?? local?.environment ?? ""}`;
                return (
                  <tr key={`${kind}:${i}`}>
                    <td>
                      {label}
                      <small>
                        {kind === "github"
                          ? "GitHubの静的調査"
                          : kind === "tool"
                            ? "導入済みツール"
                            : "ローカル調査"}
                      </small>
                    </td>
                    <td>
                      {u.name}
                      <small className="path">{u.source}</small>
                    </td>
                    <td>
                      {u.current || "未確定"} → {u.candidate || "未確定"}
                    </td>
                    <td>
                      {statusLabel[u.importance] ?? u.importance}
                      <small>{u.note}</small>
                      {u.vulnerabilities.map((v) => (
                        <small key={v.id}>{v.id}</small>
                      ))}
                    </td>
                    <td>
                      <button
                        disabled={
                          kind === "tool"
                            ? !inventory
                            : kind === "github"
                              ? !remote
                              : !local
                        }
                        onClick={() =>
                          kind === "tool"
                            ? onEnvironment(inventory!.environment)
                            : onProject({
                                kind: kind === "github" ? "github" : "local",
                                id: u.project_id!,
                              })
                        }
                      >
                        対象を確認して計画
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      ) : (
        <p className="muted padded">
          保存済みの更新候補はありません。環境・ツール、またはプロジェクトで調査・照合してください。未調査の対象が最新であるという判定ではありません。
        </p>
      )}
    </section>
  );
}
export function PlanCatalog({
  data,
  selected,
  onSelect,
}: {
  data: Snapshot;
  selected: { kind: "local" | "github"; id: string };
  onSelect: (target: { kind: "local" | "github"; id: string }) => void;
}) {
  const [filter, setFilter] = useState("all");
  const entries = [
    ...data.plans.map((p) => ({
      id: p.id,
      kind: "local" as const,
      title: p.title,
      date: p.created_at,
      status: p.status,
      target:
        data.environments.find((e) => e.id === p.environment)?.name ??
        p.environment,
    })),
    ...data.github_plans.map((p) => ({
      id: p.id,
      kind: "github" as const,
      title:
        data.github_projects.find((r) => r.id === p.report_id)?.repository ??
        p.report_id,
      date: p.created_at,
      status: p.status,
      target: `コミット ${p.commit.slice(0, 12)}`,
    })),
  ]
    .sort((a, b) => b.date.localeCompare(a.date))
    .filter((p) => filter === "all" || p.kind === filter);
  return (
    <section className="panel details-panel">
      <div className="section-heading">
        <h2>計画・進行状況</h2>
        <select
          aria-label="計画の種類"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        >
          <option value="all">すべての計画</option>
          <option value="local">実行計画</option>
          <option value="github">静的な更新方針</option>
        </select>
      </div>
      {entries.length ? (
        <div className="plan-catalog">
          {entries.map((p) => (
            <button
              key={`${p.kind}:${p.id}`}
              className={`catalog-entry ${selected.kind === p.kind && selected.id === p.id ? "active" : ""}`}
              aria-pressed={selected.kind === p.kind && selected.id === p.id}
              onClick={() => onSelect({ kind: p.kind, id: p.id })}
            >
              <span className="source-tag">
                {p.kind === "github" ? "静的な更新方針" : "実行計画"}
              </span>
              <span>
                <strong>{p.title}</strong>
                <small>
                  {p.target} · {displayDate(p.date)}
                </small>
              </span>
              <span className={`badge ${p.status}`}>
                {p.status === "draft"
                  ? "実行環境の確認前"
                  : p.status === "outdated"
                    ? "再調査が必要"
                    : (statusLabel[p.status] ?? p.status)}
              </span>
            </button>
          ))}
        </div>
      ) : (
        <p className="muted padded">
          対象を調査・照合してから計画を作成してください。
        </p>
      )}
    </section>
  );
}
export function StaticProposal({
  data,
  plan,
  busy,
  onProject,
  onPrepare,
  onClone,
}: {
  data: Snapshot;
  plan: GithubProposal;
  busy: boolean;
  onProject: (target: ProjectTarget) => void;
  onPrepare: (proposal: string, project: string) => void;
  onClone: () => void;
}) {
  const report = data.github_projects.find((r) => r.id === plan.report_id);
  const locals = proposalLocals(data, plan.report_id);
  return (
    <section className="panel details-panel" aria-label="静的な更新方針の詳細">
      <h2>
        更新方針{" "}
        {plan.status === "outdated" ? "（再調査が必要）" : "（静的な計画）"}
      </h2>
      <p>
        {report?.repository} · <code>{plan.commit}</code> ·{" "}
        {displayDate(plan.created_at)}
      </p>
      <p className="muted">
        GitHubの静的調査に基づく方針です。コマンド実行や動作検証の成功を示すものではありません。
      </p>
      <div className="button-row">
        {report && (
          <button onClick={() => onProject({ kind: "github", id: report.id })}>
            対象の調査結果を開く
          </button>
        )}
        <button disabled={busy} onClick={onClone}>
          GitHubから作業環境を用意
        </button>
      </div>
      <h3>実行へ進むための作業環境</h3>
      {plan.status === "prepared" && (
        <p className="muted">
          作業環境の一致を確認済みです。実行前には再確認し、プロジェクトで依存の照合と検証の選択を行ってください。
        </p>
      )}
      {locals.length ? (
        locals.map((p) => (
          <div className="venv" key={p.id}>
            <strong>
              {data.environments.find((e) => e.id === p.environment)?.name ??
                p.environment}{" "}
              · {p.branch}
            </strong>
            <p className="path">{p.path}</p>
            <button
              disabled={busy || plan.status === "outdated"}
              onClick={() => onPrepare(plan.id, p.id)}
            >
              コミットと作業状態を再確認
            </button>
          </div>
        ))
      ) : (
        <p className="muted">
          同じリポジトリのローカル登録がありません。クローンまたは既存フォルダーの登録後、ここから再確認できます。
        </p>
      )}
      <ol>
        {plan.steps.map((step) => (
          <li key={step}>{step}</li>
        ))}
      </ol>
      <h3>変更候補</h3>
      {plan.changes.length ? (
        plan.changes.map((c, i) => (
          <p key={i}>
            <strong>{c.name}</strong> {c.current} →{" "}
            {c.candidate || "アドバイザリの修正条件を確認"}
            <small>
              {c.source} · {c.note}
            </small>
          </p>
        ))
      ) : (
        <p className="muted">確定したパッケージ更新候補はありません。</p>
      )}
      <h3>固定ツールの確認</h3>
      {plan.tools.map((r, i) => (
        <p key={i}>
          {r.manager === "packageManager"
            ? r.requested
            : `${r.name} ${r.requested}`}
          <small>{r.source}</small>
        </p>
      ))}
      <h3>宣言から取得した検証</h3>
      <p>
        {plan.checks.join(" / ") ||
          "プロジェクト固有の検証を設定してください。"}
      </p>
      <div className="plan-warnings">
        {plan.warnings.map((w, i) => (
          <p key={i}>{w}</p>
        ))}
      </div>
    </section>
  );
}
