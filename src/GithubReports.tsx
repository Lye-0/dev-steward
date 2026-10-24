import { useState } from "react";
import type { Snapshot, Tool, GithubProposal } from "./types";
type Act = (
  label: string,
  method: string,
  payload: unknown,
  after?: (value: any) => void,
) => Promise<void>;
export function GithubReports({
  data,
  tools,
  scanned,
  environment,
  busy,
  act,
  selected,
  onClose,
  onProposal,
}: {
  data: Snapshot;
  tools: Tool[];
  scanned: boolean;
  environment: string;
  busy: boolean;
  act: Act;
  selected: string;
  onClose: () => void;
  onProposal: (plan: GithubProposal) => void;
}) {
  const report = data.github_projects.find((r) => r.id === selected);
  const updates = data.github_updates.filter(
    (u) => u.project_id === report?.id,
  );
  const [dependencyQuery, setDependencyQuery] = useState("");
  const requirements =
    report?.project.requirements.filter((r) =>
      `${r.name} ${r.source} ${r.manager}`
        .toLowerCase()
        .includes(dependencyQuery.toLowerCase()),
    ) ?? [];
  return (
    <>
      {report && (
        <>
          <section className="panel details-panel">
            <div className="section-heading">
              <div>
                <h2>{report.repository}</h2>
                <p className="path">{report.url}</p>
              </div>
              <button className="text-button" onClick={onClose}>
                詳細を閉じる
              </button>
            </div>
            <p>
              ブランチ: {report.branch} / コミット: <code>{report.commit}</code>
            </p>
            <p className="muted">
              同一コミットから取得した静的な情報です。デバイスの実行先やテスト成功は確認していません。
            </p>
            <div className="button-row">
              <button
                disabled={busy}
                onClick={() =>
                  act("GitHub依存の更新候補を照合中", "check_github_updates", {
                    report: report.id,
                  })
                }
              >
                更新候補・脆弱性を照合
              </button>
              <button
                disabled={busy || !report.complete || !updates.length}
                onClick={() =>
                  act(
                    "GitHub更新方針を作成中",
                    "plan_github_updates",
                    { report: report.id },
                    onProposal,
                  )
                }
              >
                更新方針を作成
              </button>
            </div>
          </section>
          <details className="panel details-panel">
            <summary>取得ファイルと調査範囲（{report.files.length}件）</summary>
            {report.files.map((f) => (
              <p className="path" key={f.path}>
                {f.path} · {f.status} · {f.size} bytes · {f.sha.slice(0, 12)}
              </p>
            ))}
            {report.project.findings.map((f, i) => (
              <p key={i}>
                <strong>{f.scope}</strong> {f.message}
              </p>
            ))}
          </details>
          <section className="panel details-panel">
            <h2>依存・固定ツールの宣言</h2>
            <div className="search">
              <input
                aria-label="GitHubの依存を検索"
                value={dependencyQuery}
                onChange={(e) => setDependencyQuery(e.target.value)}
                placeholder="名前・ファイル・管理元で検索"
              />
            </div>
            <p className="muted">
              {requirements.length}件。先頭200件を表示します。
            </p>
            <div className="table-scroll capped">
              <table aria-label="GitHubの依存宣言">
                <thead>
                  <tr>
                    <th>名前</th>
                    <th>指定 / 解決版</th>
                    <th>宣言元</th>
                  </tr>
                </thead>
                <tbody>
                  {requirements.slice(0, 200).map((r, i) => (
                    <tr key={i}>
                      <td>
                        {r.name}
                        <small>
                          {r.manager} · {r.section}
                        </small>
                      </td>
                      <td>
                        <code>{r.requested}</code>
                        {r.resolved && <small>{r.resolved}</small>}
                      </td>
                      <td className="path">{r.source}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>
          <section className="panel details-panel">
            <h2>デバイスの導入版との比較</h2>
            <p className="muted">
              比較対象: {environment}
              。宣言と導入版の表示であり、制約充足・実行確認とは別です。
            </p>
            {report.project.requirements
              .filter((r) =>
                ["mise", "uv", "rustup", "engine", "packageManager"].includes(
                  r.manager,
                ),
              )
              .map((r, i) => (
                <p key={i}>
                  <strong>
                    {r.manager === "packageManager"
                      ? r.requested
                      : `${r.name} ${r.requested}`}
                  </strong>{" "}
                  / 導入版:{" "}
                  {tools
                    .filter((t) => t.installed && t.name === r.name)
                    .map((t) => `${t.version} (${t.manager})`)
                    .join(", ") || (scanned ? "未検出" : "未調査")}
                  <small>{r.source}</small>
                </p>
              ))}
          </section>
          {updates.length > 0 && (
            <section className="panel details-panel">
              <h2>更新候補と既知の注意情報</h2>
              <div className="table-scroll capped">
                <table aria-label="GitHubの更新候補">
                  <thead>
                    <tr>
                      <th>名前</th>
                      <th>現在 → 候補</th>
                      <th>出典・照合状態</th>
                    </tr>
                  </thead>
                  <tbody>
                    {updates.map((u, i) => (
                      <tr key={i}>
                        <td>
                          {u.name}
                          <small>{u.source}</small>
                        </td>
                        <td>
                          {u.current || "未確定"} →{" "}
                          {u.candidate ||
                            (u.importance === "unknown"
                              ? "未確認"
                              : "候補なし")}
                          <small>{u.note}</small>
                        </td>
                        <td>
                          {u.audit_status}
                          {u.vulnerabilities.map((v) => (
                            <p key={v.id}>
                              <a href={v.url} target="_blank" rel="noreferrer">
                                {v.id}
                              </a>{" "}
                              {v.summary}
                            </p>
                          ))}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          )}
        </>
      )}
    </>
  );
}
