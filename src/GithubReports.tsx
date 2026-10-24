import { useState } from "react";
import { displayDate } from "./logic";
import type { Snapshot, Tool } from "./types";

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
  open,
}: {
  data: Snapshot;
  tools: Tool[];
  scanned: boolean;
  environment: string;
  busy: boolean;
  act: Act;
  open: () => void;
}) {
  const [selected, setSelected] = useState("");
  const [query, setQuery] = useState("");
  const [checked, setChecked] = useState<string[]>([]);
  const reports = data.github_projects;
  const report = reports.find((r) => r.id === selected);
  const updates = data.github_updates.filter(
    (u) => u.project_id === report?.id,
  );
  const plans = data.github_plans
    .filter((p) => p.report_id === report?.id)
    .sort((a, b) => b.created_at.localeCompare(a.created_at));
  const [dependencyQuery, setDependencyQuery] = useState("");
  const requirements =
    report?.project.requirements.filter((r) =>
      `${r.name} ${r.source} ${r.manager}`
        .toLowerCase()
        .includes(dependencyQuery.toLowerCase()),
    ) ?? [];
  return (
    <>
      <div className="page-heading">
        <div>
          <div className="eyebrow">GITHUB · READ ONLY</div>
          <h1>GitHub上で調査</h1>
          <p>
            必要な設定ファイルだけを取得します。クローン・コード実行・GitHubへの書き込みは行いません。
          </p>
        </div>
        <button className="primary" disabled={busy} onClick={open}>
          リポジトリを選択
        </button>
      </div>
      <p className="muted">
        保存した調査結果です。対象コミットと取得日時を確認してください。最大50件ずつ取得・照合できます。
      </p>
      {data.github_errors.length > 0 && (
        <section className="panel details-panel">
          <h2>直近の取得・照合で確認できなかったもの</h2>
          {data.github_errors.map((e, i) => (
            <p key={i}>
              <strong>{e.scope}</strong> {e.message}
            </p>
          ))}
          <p className="muted">
            失敗したリポジトリの以前の結果は更新していません。
          </p>
        </section>
      )}
      <div className="toolbar github-toolbar">
        <div className="search">
          <input
            aria-label="GitHub調査結果を検索"
            placeholder="リポジトリ名で検索"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <button
          disabled={busy || !checked.length || checked.length > 50}
          onClick={() =>
            act("選択したGitHub依存を照合中", "check_github_updates_batch", {
              reports: checked,
            })
          }
        >
          選択した結果を照合（{checked.length}件）
        </button>
      </div>
      <p className="muted">
        更新・脆弱性の照合では、パッケージ名・版を公開レジストリとOSVへ送信します。
      </p>
      {reports.length ? (
        <section className="panel">
          <div className="table-scroll">
            <table aria-label="GitHubの調査結果">
              <thead>
                <tr>
                  <th>選択</th>
                  <th>リポジトリ</th>
                  <th>ファイル</th>
                  <th>更新候補 / 注意情報</th>
                  <th>調査時点</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {reports
                  .filter((r) =>
                    `${r.repository} ${r.branch}`
                      .toLowerCase()
                      .includes(query.toLowerCase()),
                  )
                  .map((r) => {
                    const candidates = data.github_updates.filter(
                      (u) => u.project_id === r.id,
                    );
                    const candidateCount = candidates.filter(
                      (u) => u.candidate,
                    ).length;
                    const notices = new Set(
                      candidates.flatMap((u) =>
                        u.vulnerabilities.map((v) => v.id),
                      ),
                    ).size;
                    return (
                      <tr key={r.id}>
                        <td>
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
                        </td>
                        <td>
                          <button
                            className="text-button"
                            aria-label={`${r.repository} (${r.branch})`}
                            onClick={() => {
                              setSelected(r.id);
                              setDependencyQuery("");
                            }}
                          >
                            {r.repository}
                          </button>
                          <small>
                            {!r.files.length
                              ? "対応設定なし"
                              : r.complete
                                ? "静的取得済み"
                                : "取得・解析に未完了あり"}{" "}
                            · {r.branch} · {r.project.package_manager}
                          </small>
                        </td>
                        <td>
                          {r.files.filter((f) => f.status === "ok").length}/
                          {r.files.length}件
                        </td>
                        <td>
                          {candidates.length
                            ? `${candidateCount}件 / ${notices}件`
                            : "未照合"}
                          {candidates.some(
                            (u) =>
                              u.importance === "unknown" ||
                              u.audit_status.startsWith("照合失敗") ||
                              u.audit_status.startsWith("未照合"),
                          ) && <small>未確認項目あり</small>}
                        </td>
                        <td>
                          <code>{r.commit.slice(0, 12)}</code>
                          <small>{displayDate(r.project.scanned_at)}</small>
                        </td>
                        <td>
                          <button
                            disabled={busy}
                            onClick={() =>
                              act(
                                "GitHub上の設定を再取得中",
                                "scan_github_repos",
                                {
                                  repositories: [r.repository],
                                  reference: r.branch,
                                },
                              )
                            }
                          >
                            再調査
                          </button>
                        </td>
                      </tr>
                    );
                  })}
              </tbody>
            </table>
          </div>
        </section>
      ) : (
        <div className="empty">
          <h2>リポジトリを選択して調査</h2>
          <p>クローン先を指定せずにGitHub上の設定・依存を取得できます。</p>
          <button onClick={open}>GitHubから選ぶ</button>
        </div>
      )}
      {report && (
        <>
          <section className="panel details-panel">
            <div className="section-heading">
              <div>
                <h2>{report.repository}</h2>
                <p className="path">{report.url}</p>
              </div>
              <button className="text-button" onClick={() => setSelected("")}>
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
                  act("GitHub更新方針を作成中", "plan_github_updates", {
                    report: report.id,
                  })
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
          {plans.map((plan) => (
            <section className="panel details-panel" key={plan.id}>
              <h2>
                更新方針{" "}
                {plan.status === "outdated"
                  ? "（再調査が必要）"
                  : "（静的な計画）"}
              </h2>
              <p>
                <code>{plan.commit}</code> · {displayDate(plan.created_at)}
              </p>
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
                <p className="muted">
                  今回の照合で確定したパッケージ更新候補はありません。
                </p>
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
                  "自動検出できませんでした。プロジェクト固有の検証を設定してください。"}
              </p>
              <div className="plan-warnings">
                {plan.warnings.map((warning, i) => (
                  <p key={i}>{warning}</p>
                ))}
              </div>
            </section>
          ))}
        </>
      )}
    </>
  );
}
