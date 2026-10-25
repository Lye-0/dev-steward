import type { Project } from "./types";

export function CiPanel({ project }: { project: Project }) {
  const files = project.ci ?? [];
  const warningCount = files.reduce(
    (total, file) => total + file.warnings.length,
    0,
  );
  return (
    <section
      className="panel details-panel ci-panel"
      aria-label="CI/CDの解析結果"
    >
      <div className="section-heading">
        <h3>CI/CD・ビルド・デプロイ</h3>
        <span className="muted">
          {files.length}ファイル · 確認事項 {warningCount}件
        </span>
      </div>
      <p className="muted">
        実行環境と検証・集計の手順を静的に取得しています。浮動指定は実際のパッチ版を保証しません。表示したコマンドは実行しません。
      </p>
      {!files.length && (
        <p>
          対応する設定は未検出です。外部で管理するCI/CDや未対応形式もあります。以前の調査結果は再調査してください。
        </p>
      )}
      {files.map((file) => (
        <details key={file.path} className="ci-file">
          <summary>
            <strong>{file.provider}</strong>{" "}
            <span className="path">{file.path}</span>{" "}
            <span className="muted">確認事項 {file.warnings.length}件</span>
          </summary>
          {file.runtimes.length > 0 && (
            <div className="table-scroll">
              <table aria-label={`${file.path}の実行環境`}>
                <thead>
                  <tr>
                    <th>実行環境</th>
                    <th>指定</th>
                    <th>静的に解決した候補</th>
                    <th>ジョブ・宣言位置</th>
                  </tr>
                </thead>
                <tbody>
                  {file.runtimes.map((runtime, index) => (
                    <tr key={index}>
                      <td>{runtime.name}</td>
                      <td>
                        <code>{runtime.requested}</code>
                      </td>
                      <td>
                        {runtime.resolved.length
                          ? runtime.resolved.join(" / ")
                          : "要確認"}
                      </td>
                      <td className="path">{runtime.scope}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {file.warnings.length > 0 && (
            <div className="ci-warnings">
              <h4>更新前後に確認すること</h4>
              <ul>
                {file.warnings.map((warning, i) => (
                  <li key={i}>{warning}</li>
                ))}
              </ul>
            </div>
          )}
          {file.steps.length > 0 && (
            <details>
              <summary>
                検証・集計・デプロイの手順（{file.steps.length}件）
              </summary>
              <p className="muted">
                宣言上の順序です。ジョブ間の並行実行・条件・継承は評価していません。
              </p>
              {file.steps.map((step, i) => (
                <div className="ci-step" key={i}>
                  <strong>{step.scope}</strong>
                  <p className="muted">
                    作業場所：{step.directory || "設定元の既定値"} · シェル：
                    {step.shell || "設定元の既定値"}
                  </p>
                  <pre>{step.command}</pre>
                </div>
              ))}
            </details>
          )}
          {file.references.length > 0 && (
            <details>
              <summary>
                Action・参照ファイル（{file.references.length}件）
              </summary>
              <ul>
                {file.references.map((reference, i) => (
                  <li className="path" key={i}>
                    {reference}
                  </li>
                ))}
              </ul>
              <p className="muted">
                Actionの指定は抽出のみです。Action自身の更新候補・内部ランタイムは照合していません。
              </p>
            </details>
          )}
        </details>
      ))}
    </section>
  );
}
