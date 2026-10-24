import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  Activity,
  ArrowDownToLine,
  ArrowRight,
  Check,
  ChevronRight,
  CircleHelp,
  Clock3,
  Code2,
  FolderGit2,
  FolderPlus,
  GitBranch,
  GitFork,
  Layers3,
  ListChecks,
  LoaderCircle,
  Monitor,
  RefreshCw,
  Search,
  Settings2,
  ShieldCheck,
  Terminal,
  X,
} from "lucide-react";
import { call, chooseDirectory } from "./bridge";
import {
  displayDate,
  groupTools,
  managerLabels,
  executableStep,
  relatedProjects,
  statusLabel,
} from "./logic";
import type { Snapshot, Plan, Update, Settings } from "./types";

type Page =
  "overview" | "environment" | "projects" | "plans" | "history" | "settings";
const pages: { id: Page; label: string; icon: typeof Monitor }[] = [
  { id: "overview", label: "全体状況", icon: Layers3 },
  { id: "environment", label: "開発環境", icon: Monitor },
  { id: "projects", label: "プロジェクト", icon: FolderGit2 },
  { id: "plans", label: "更新計画", icon: ListChecks },
  { id: "history", label: "実行履歴", icon: Clock3 },
];
const empty: Snapshot = {
  environments: [],
  inventories: [],
  projects: [],
  plans: [],
  history: [],
  settings: {
    roots: [],
    mcp_configs: [],
    manager_overrides: [],
    same_major: true,
  },
  updates: [],
  data_path: "",
  version: "0.1.0",
};
function Badge({ status, children }: { status: string; children?: ReactNode }) {
  return (
    <span className={`badge ${status}`}>
      {children ?? statusLabel[status] ?? status}
    </span>
  );
}
function Empty({
  icon: Icon,
  title,
  children,
  action,
}: {
  icon: typeof Monitor;
  title: string;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="empty">
      <Icon size={32} />
      <h3>{title}</h3>
      <p>{children}</p>
      {action}
    </div>
  );
}
function Findings({
  items,
}: {
  items: Snapshot["inventories"][number]["findings"];
}) {
  return (
    <div className="findings">
      {items.map((f, i) => (
        <div className="finding" key={`${f.scope}-${i}`}>
          <Badge status={f.status} />
          <div>
            <strong>{f.scope}</strong>
            <p>{f.message}</p>
          </div>
        </div>
      ))}
    </div>
  );
}

export function App() {
  const [data, setData] = useState<Snapshot>(empty);
  const [page, setPage] = useState<Page>("overview");
  const [env, setEnv] = useState("local");
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [pinTool, setPinTool] = useState("");
  const [dependencyQuery, setDependencyQuery] = useState("");
  const [search, setSearch] = useState("");
  const [projectId, setProjectId] = useState("");
  const [planId, setPlanId] = useState("");
  const [root, setRoot] = useState("");
  const [adding, setAdding] = useState(false);
  const [checks, setChecks] = useState<string[]>([]);
  const [selected, setSelected] = useState<number[]>([]);
  const [publish, setPublish] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [github, setGitFork] = useState<
    | {
        nameWithOwner: string;
        description: string;
        url: string;
        isArchived: boolean;
      }[]
    | null
  >(null);
  const [cloneParent, setCloneParent] = useState("");
  const [pr, setPr] = useState<unknown>(null);
  const [systemOutput, setSystemOutput] = useState("");
  const [systemPackage, setSystemPackage] = useState("");
  const operation = useRef(false);
  const refreshSequence = useRef(0);
  async function refresh() {
    const sequence = ++refreshSequence.current;
    const snapshot = await call<Snapshot>("snapshot");
    if (sequence === refreshSequence.current) setData(snapshot);
  }
  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
  }, []);
  async function act(
    label: string,
    method: string,
    payload: unknown = null,
    after?: (v: any) => void,
  ) {
    if (operation.current) return;
    operation.current = true;
    setBusy(label);
    setError("");
    setNotice("");
    try {
      const result = await call(method, payload);
      await refresh();
      after?.(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy("");
      operation.current = false;
    }
  }
  useEffect(() => {
    setSearch("");
    setSelected([]);
    setChecks([]);
    setConfirmed(false);
    setPinTool("");
    setDependencyQuery("");
  }, [env, projectId, planId]);
  useEffect(() => {
    if (!busy) return;
    const id = setInterval(() => refresh().catch(() => {}), 2000);
    return () => clearInterval(id);
  }, [busy]);
  useEffect(() => {
    setSystemOutput("");
    setPr(null);
  }, [env]);
  const environment = data.environments.find((e) => e.id === env);
  const inventory = data.inventories.find((i) => i.environment === env);
  const tools = inventory?.tools ?? [];
  const toolGroups = groupTools(tools, search, false);
  const wingetTools = tools.filter((t) => t.manager === "winget");
  const wingetRows = groupTools(wingetTools, search, true)[0]?.[1] ?? [];
  const projects = data.projects.filter((p) => p.environment === env);
  const project = projects.find((p) => p.id === projectId);
  const projectUpdates = data.updates.filter(
    (u) => u.project_id === project?.id,
  );
  const toolUpdates = data.updates.filter(
    (u) => u.tool_id && tools.some((t) => t.id === u.tool_id),
  );
  const plans = data.plans
    .filter((p) => p.environment === env)
    .sort((a, b) => b.created_at.localeCompare(a.created_at));
  const plan = plans.find((p) => p.id === planId) ?? plans[0];
  const next = plan ? executableStep(plan.status, plan.steps) : -1;
  const pending = plans.filter((p) => p.status === "ready").length;
  const security = data.updates.filter(
    (u) =>
      u.importance === "security" &&
      projects.some((p) => p.id === u.project_id),
  ).length;
  const toggle = (list: string[], value: string, set: (s: string[]) => void) =>
    set(
      list.includes(value) ? list.filter((v) => v !== value) : [...list, value],
    );
  const rootForm = (
    <form
      className="root-form"
      onSubmit={(e) => {
        e.preventDefault();
        act(
          "リポジトリを調査中",
          "scan_root",
          { environment: env, path: root },
          (r) => {
            setAdding(false);
            setRoot("");
            setNotice(
              `${r.count}件を登録しました${r.errors.length ? `。取得失敗: ${r.errors.join(" / ")}` : ""}`,
            );
            setPage("projects");
          },
        );
      }}
    >
      <label htmlFor="scan-root">
        {environment?.kind === "wsl" ? "WSL内の絶対パス" : "調査するフォルダー"}
      </label>
      <div className="input-row">
        <input
          id="scan-root"
          required
          value={root}
          onChange={(e) => setRoot(e.target.value)}
          placeholder={
            environment?.kind === "wsl"
              ? "/home/your-name/projects"
              : "フォルダーを選択、または絶対パスを入力"
          }
        />
        {environment?.kind !== "wsl" && (
          <button
            type="button"
            onClick={() =>
              chooseDirectory()
                .then((v) => {
                  if (v) setRoot(v);
                })
                .catch((e) => setError(String(e)))
            }
          >
            選択
          </button>
        )}
        <button className="primary" disabled={!!busy || !root.trim()}>
          調査して登録
        </button>
      </div>
      <small>
        Gitリポジトリを深さ7まで検出します。コードやインストール処理は実行しません。
      </small>
    </form>
  );
  function openPlan(p: Plan) {
    setPlanId(p.id);
    setPage("plans");
    setConfirmed(false);
  }
  function updatesTable(rows: Update[], selectable: boolean) {
    return (
      <div className="table-scroll">
        <table>
          <thead>
            <tr>
              {selectable && (
                <th>
                  <span className="sr-only">選択</span>
                </th>
              )}
              <th>パッケージ / ツール</th>
              <th>現在</th>
              <th>候補</th>
              <th>判断</th>
              <th>根拠・確認状況</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((u, i) => (
              <tr key={`${u.name}-${u.source}-${i}`}>
                {selectable && (
                  <td>
                    <input
                      type="checkbox"
                      aria-label={`${u.name}の更新を選択`}
                      disabled={!u.candidate}
                      checked={selected.includes(i)}
                      onChange={() =>
                        setSelected(
                          selected.includes(i)
                            ? selected.filter((n) => n !== i)
                            : [...selected, i],
                        )
                      }
                    />
                  </td>
                )}
                <td>
                  <strong>{u.name}</strong>
                  <small>
                    {u.manager} · {u.source}
                  </small>
                </td>
                <td>
                  <code>{u.current || "—"}</code>
                </td>
                <td>
                  <code>{u.candidate || "—"}</code>
                  {u.latest && u.latest !== u.candidate && (
                    <small>配布元の最新 {u.latest}</small>
                  )}
                </td>
                <td>
                  <Badge status={u.importance} />
                </td>
                <td>
                  <details>
                    <summary>詳細を確認</summary>
                    <p>{u.note}</p>
                    <p>{u.audit_status}</p>
                    {u.vulnerabilities.map((v) => (
                      <p key={v.id}>
                        <a href={v.url} target="_blank" rel="noreferrer">
                          {v.id}
                        </a>{" "}
                        — {v.summary}
                      </p>
                    ))}
                  </details>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    );
  }
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a
          className="brand"
          aria-label="Dev Steward 全体状況"
          href="#"
          onClick={(e) => {
            e.preventDefault();
            setPage("overview");
          }}
        >
          <span className="brand-icon">
            <Layers3 size={23} />
          </span>
          <span>
            dev steward<small>DEVELOPMENT, IN ORDER</small>
          </span>
        </a>
        <div className="workspace-label">WORKSPACE</div>
        <nav aria-label="メインナビゲーション">
          {pages.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              className={page === id ? "nav-item selected" : "nav-item"}
              aria-label={label}
              aria-current={page === id ? "page" : undefined}
              onClick={() => setPage(id)}
            >
              <Icon size={19} />
              <span>{label}</span>
              {id === "plans" && pending > 0 && (
                <span className="nav-count">{pending}</span>
              )}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <button
            className={`nav-item ${page === "settings" ? "selected" : ""}`}
            onClick={() => setPage("settings")}
          >
            <Settings2 size={19} />
            設定
          </button>
          <div className="local-note">
            <ShieldCheck size={16} />
            <span>
              ローカルで管理<small>v{data.version}</small>
            </span>
          </div>
        </div>
      </aside>
      <div className="workspace">
        <header className="topbar">
          <div className="breadcrumb">
            WORKSPACE <ChevronRight size={14} />
            <strong>
              {page === "settings"
                ? "設定"
                : pages.find((p) => p.id === page)?.label}
            </strong>
          </div>
          <div className="environment-select">
            <Monitor size={16} />
            <label className="sr-only" htmlFor="environment">
              対象の環境
            </label>
            <select
              id="environment"
              value={env}
              onChange={(e) => {
                setEnv(e.target.value);
                setProjectId("");
                setPlanId("");
              }}
            >
              {data.environments.length ? (
                data.environments.map((e) => (
                  <option key={e.id} value={e.id}>
                    {e.name} · {e.os}
                  </option>
                ))
              ) : (
                <option value="local">このデバイス</option>
              )}
            </select>
            <span className="status-dot" />
          </div>
        </header>
        <main id="main-content">
          <div className="feedback" aria-live="polite">
            {busy && (
              <div className="busy">
                <LoaderCircle className="spin" size={17} />
                {busy}…
                {busy === "工程を実行中" && (
                  <button
                    onClick={() =>
                      call("cancel").catch((e) => setError(String(e)))
                    }
                  >
                    中断する
                  </button>
                )}
              </div>
            )}
            {error && (
              <div role="alert" className="error">
                <strong>操作を完了できませんでした</strong>
                <span>{error}</span>
                <button
                  aria-label="エラーを閉じる"
                  onClick={() => setError("")}
                >
                  <X size={16} />
                </button>
              </div>
            )}
            {notice && (
              <div className="notice">
                <Check size={17} />
                {notice}
              </div>
            )}
          </div>
          {page === "overview" && (
            <>
              <div className="page-heading">
                <div>
                  <div className="eyebrow">YOUR DEVELOPMENT WORKSPACE</div>
                  <h1>開発環境を、見渡す。</h1>
                  <p>
                    ツールの配置からプロジェクトの依存まで。次の更新を、根拠とともに。
                  </p>
                </div>
                <button
                  className="primary"
                  disabled={!!busy}
                  onClick={() =>
                    act("環境を調査中", "scan_environment", {
                      environment: env,
                    })
                  }
                >
                  <RefreshCw size={16} />
                  環境を調査
                </button>
              </div>
              <div className="stats">
                <button onClick={() => setPage("environment")}>
                  <span>管理ツール・ランタイム</span>
                  <strong>
                    {inventory
                      ? tools.filter(
                          (t) => t.installed && t.manager !== "winget",
                        ).length
                      : "—"}
                    <small>件</small>
                  </strong>
                  <span>
                    {inventory
                      ? `最終調査 ${displayDate(inventory.captured_at)}`
                      : "まだ調査していません"}
                    <ArrowRight size={15} />
                  </span>
                </button>
                <button onClick={() => setPage("projects")}>
                  <span>登録プロジェクト</span>
                  <strong>
                    {projects.length}
                    <small>件</small>
                  </strong>
                  <span>
                    ローカルとGitHubから登録
                    <ArrowRight size={15} />
                  </span>
                </button>
                <button onClick={() => setPage("plans")}>
                  <span>実行待ちの計画</span>
                  <strong>
                    {pending}
                    <small>件</small>
                  </strong>
                  <span>
                    {security
                      ? `既知の問題 ${security}件の記録`
                      : "候補と検証結果を分けて管理"}
                    <ArrowRight size={15} />
                  </span>
                </button>
              </div>
              <div className="overview-columns">
                <section className="panel">
                  <div className="section-heading">
                    <div>
                      <h2>環境とプロジェクトをつなぐ</h2>
                      <p>最初の調査から、更新・検証へ進めます。</p>
                    </div>
                    <span className="index-label">GET STARTED</span>
                  </div>
                  <div className="start-item">
                    <span className="step-number">01</span>
                    <div>
                      <h3>ツールと保存先を確認</h3>
                      <p>mise・uv・rustupの管理版を取得します。</p>
                    </div>
                    <button
                      className="icon-button"
                      aria-label="開発環境を見る"
                      onClick={() => setPage("environment")}
                    >
                      <ArrowRight size={18} />
                    </button>
                  </div>
                  <div className="start-item">
                    <span className="step-number">02</span>
                    <div>
                      <h3>プロジェクトを登録</h3>
                      <p>固定バージョンと依存関係を静的に調査します。</p>
                    </div>
                    <button
                      className="icon-button"
                      aria-label="プロジェクトを登録する"
                      onClick={() => {
                        setPage("projects");
                        setAdding(true);
                      }}
                    >
                      <ArrowRight size={18} />
                    </button>
                  </div>
                  <div className="start-item">
                    <span className="step-number">03</span>
                    <div>
                      <h3>更新内容を確認して実行</h3>
                      <p>候補の理由とコマンドを確認し、工程ごとに実行。</p>
                    </div>
                    <button
                      className="icon-button"
                      aria-label="更新計画を見る"
                      onClick={() => setPage("plans")}
                    >
                      <ArrowRight size={18} />
                    </button>
                  </div>
                </section>
                <section className="principles">
                  <div className="principle-icon">
                    <ShieldCheck size={25} />
                  </div>
                  <h2>
                    新しくする。
                    <br />
                    必要なものは、残す。
                  </h2>
                  <p>
                    最新版が、すべてのプロジェクトの正解とは限りません。利用先と制約を確認しながら、環境を整えます。
                  </p>
                  <div>
                    <Check size={16} />
                    同一メジャーの更新を優先
                  </div>
                  <div>
                    <Check size={16} />
                    OS・アプリ同梱の環境を保護
                  </div>
                  <div>
                    <Check size={16} />
                    未確認は「未確認」のまま表示
                  </div>
                </section>
              </div>
              <section className="panel activity-panel">
                <div className="section-heading">
                  <h2>最近の操作</h2>
                  <button
                    className="text-button"
                    onClick={() => setPage("history")}
                  >
                    すべて見る
                    <ArrowRight size={14} />
                  </button>
                </div>
                {data.history.length ? (
                  data.history.slice(0, 4).map((h) => (
                    <div className="activity" key={h.id}>
                      <Activity size={17} />
                      <div>
                        <strong>{h.action}</strong>
                        <span>{h.target}</span>
                      </div>
                      <Badge status={h.status} />
                      <time>{displayDate(h.at)}</time>
                    </div>
                  ))
                ) : (
                  <p className="muted padded">
                    まだ操作の記録はありません。環境の調査から始めてください。
                  </p>
                )}
              </section>
            </>
          )}
          {page === "environment" && (
            <>
              <PageHeading
                eyebrow="ENVIRONMENTS"
                title="開発環境"
                description="管理元、配置、利用先を確認します。WindowsとWSLは別々の環境です。"
                action={
                  <button
                    className="primary"
                    disabled={!!busy}
                    onClick={() =>
                      act("環境を調査中", "scan_environment", {
                        environment: env,
                      })
                    }
                  >
                    <RefreshCw size={16} />
                    再調査
                  </button>
                }
              />
              <div className="toolbar">
                <div className="search">
                  <Search size={17} />
                  <input
                    aria-label="ツールを検索"
                    placeholder="ツール名・バージョン・管理元で検索"
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                  />
                </div>
                <button
                  disabled={!!busy || !inventory}
                  onClick={() =>
                    act("ツールの更新候補を照合中", "check_tool_updates", {
                      environment: env,
                    })
                  }
                >
                  <ArrowDownToLine size={16} />
                  更新候補を照合
                </button>
              </div>
              {!inventory ? (
                <Empty icon={Monitor} title="この環境は未調査です">
                  「再調査」で管理ツールとランタイムを取得します。
                </Empty>
              ) : (
                <div className="manager-groups">
                  {toolGroups.length ? (
                    toolGroups.map(([manager, rows]) => (
                      <section
                        className="panel manager-group"
                        key={manager}
                        aria-label={managerLabels[manager] ?? `${manager} 管理`}
                      >
                        <header className="manager-group-heading">
                          <h2>{managerLabels[manager] ?? `${manager} 管理`}</h2>
                          <span>{rows.length}件</span>
                        </header>
                        {manager === "python-launcher" && (
                          <p className="muted manager-group-note">
                            pyが認識するPythonです。旧ランチャーは導入・更新の管理元ではないため、登録情報として表示します。
                          </p>
                        )}
                        <div className="table-scroll">
                          <table
                            aria-label={
                              managerLabels[manager] ?? `${manager} 管理`
                            }
                          >
                            <thead>
                              <tr>
                                <th>ツール</th>
                                <th>バージョン</th>
                                <th>状態</th>
                                <th>配置と利用関係</th>
                                <th>更新</th>
                              </tr>
                            </thead>
                            <tbody>
                              {rows.map((t) => {
                                const related = relatedProjects(t, projects);
                                const update = toolUpdates.find(
                                  (u) => u.tool_id === t.id,
                                );
                                return (
                                  <tr key={t.id}>
                                    <td>
                                      <div className="tool-name">
                                        <Code2 size={18} />
                                        <strong>{t.name}</strong>
                                      </div>
                                    </td>
                                    <td>
                                      <code>{t.version || "未取得"}</code>
                                      {t.manager === "rustup" && (
                                        <small>{t.requested}</small>
                                      )}
                                    </td>
                                    <td>
                                      <Badge
                                        status={
                                          !t.installed
                                            ? "missing"
                                            : t.active
                                              ? "ok"
                                              : "partial"
                                        }
                                      >
                                        {!t.installed
                                          ? "未導入"
                                          : t.active
                                            ? "既定の環境で有効"
                                            : "インストール済み"}
                                      </Badge>
                                    </td>
                                    <td>
                                      <details>
                                        <summary>
                                          {related.length
                                            ? `${related.length}プロジェクトに関連する宣言`
                                            : "配置・参照情報"}
                                        </summary>
                                        <p className="path">
                                          {t.path || "配置先は未取得"}
                                        </p>
                                        {(t.source || t.requested) && (
                                          <p className="path">
                                            {t.source && (
                                              <>参照元: {t.source} / </>
                                            )}
                                            指定・登録タグ:{" "}
                                            {t.requested || "未取得"}
                                          </p>
                                        )}
                                        <p className="preserve">{t.note}</p>
                                        {related.map((p) => (
                                          <button
                                            className="text-button"
                                            key={p.id}
                                            onClick={() => {
                                              setProjectId(p.id);
                                              setPage("projects");
                                            }}
                                          >
                                            {p.name}
                                            <ArrowRight size={13} />
                                          </button>
                                        ))}
                                        <p className="muted">
                                          名前・宣言・仮想環境の版による関連候補です。実行確認や完全な未使用判定ではありません。
                                        </p>
                                        {t.manager === "mise" &&
                                          t.installed &&
                                          !t.active && (
                                            <button
                                              className="text-button danger"
                                              disabled={!!busy}
                                              onClick={() =>
                                                act(
                                                  "削除前の利用先を確認中",
                                                  "plan_cleanup",
                                                  {
                                                    environment: env,
                                                    tool: t.id,
                                                  },
                                                  openPlan,
                                                )
                                              }
                                            >
                                              旧版の削除可否を確認
                                            </button>
                                          )}
                                      </details>
                                    </td>
                                    <td>
                                      {![
                                        "mise",
                                        "uv",
                                        "rustup",
                                        "winget",
                                      ].includes(t.manager) ? (
                                        <span className="muted">
                                          一覧取得のみ
                                        </span>
                                      ) : update?.candidate ? (
                                        <button
                                          disabled={!!busy}
                                          onClick={() =>
                                            act(
                                              "導入計画を作成中",
                                              "plan_tool",
                                              {
                                                environment: env,
                                                tool: t.id,
                                              },
                                              openPlan,
                                            )
                                          }
                                        >
                                          {update.candidate} の導入計画
                                        </button>
                                      ) : (
                                        <span
                                          className="muted"
                                          title={update?.note}
                                        >
                                          {update
                                            ? update.importance === "unknown"
                                              ? "照合失敗 / 未対応"
                                              : "更新候補なし"
                                            : "未照合"}
                                        </span>
                                      )}
                                    </td>
                                  </tr>
                                );
                              })}
                            </tbody>
                          </table>
                        </div>
                      </section>
                    ))
                  ) : (
                    <Empty
                      icon={Terminal}
                      title={
                        tools.length
                          ? "表示条件に一致するツールはありません"
                          : "管理版を取得できませんでした"
                      }
                    >
                      {tools.length
                        ? "検索条件やアプリ表示の設定を変更してください。"
                        : "下の診断で、未検出や取得失敗の理由を確認してください。"}
                    </Empty>
                  )}
                </div>
              )}
              {environment?.os === "windows" && (
                <section
                  className="panel winget-panel"
                  aria-label="WinGet 管理"
                >
                  <header className="manager-group-heading">
                    <h2>WinGet 管理</h2>
                    <span>
                      {wingetRows.length} / {wingetTools.length}件
                    </span>
                  </header>
                  <p className="muted padded">
                    WinGetで照合できたアプリを表示します。アプリの導入版と配布元を確認し、個別の更新を計画できます。
                  </p>
                  {!inventory ? (
                    <p className="muted padded">
                      この環境は未調査です。「再調査」でアプリを取得します。
                    </p>
                  ) : wingetRows.length ? (
                    <div className="table-scroll capped">
                      <table aria-label="WinGet 管理">
                        <thead>
                          <tr>
                            <th>パッケージID</th>
                            <th>インストール版</th>
                            <th>ソース</th>
                            <th>更新</th>
                          </tr>
                        </thead>
                        <tbody>
                          {wingetRows.map((t) => (
                            <tr key={t.id}>
                              <td>
                                <strong>{t.name}</strong>
                              </td>
                              <td>
                                <code>{t.version || "不明"}</code>
                              </td>
                              <td>{t.source || "未取得"}</td>
                              <td>
                                <button
                                  disabled={!!busy}
                                  onClick={() =>
                                    act(
                                      "WinGetの配布情報を確認中",
                                      "plan_system_update",
                                      { environment: env, package: t.name },
                                      openPlan,
                                    )
                                  }
                                >
                                  更新を計画
                                </button>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  ) : (
                    <p className="muted padded">
                      {wingetTools.length
                        ? "検索条件に一致するアプリはありません。"
                        : "WinGetのアプリを取得できていません。下の診断で取得結果を確認してください。"}
                    </p>
                  )}
                </section>
              )}
              {inventory && (
                <>
                  <h2 className="subheading">
                    調査の範囲と診断{" "}
                    <small>{displayDate(inventory.captured_at)}</small>
                  </h2>
                  <Findings items={inventory.findings} />
                  <details className="panel details-panel">
                    <summary>管理ツールの実行ファイル</summary>
                    {inventory.manager_paths.map(([n, p]) => (
                      <p className="path" key={n}>
                        <strong>{n}</strong> {p}
                      </p>
                    ))}
                  </details>
                </>
              )}
              <section className="panel details-panel">
                <h2>OS・アプリ管理元の更新</h2>
                <p className="muted">
                  WinGetやAPTの更新一覧を取得します。OS用Pythonやアプリ同梱ランタイムを直接削除しません。
                </p>
                <button
                  disabled={!!busy}
                  onClick={() =>
                    act(
                      "システム管理元の更新を確認中",
                      "system_updates",
                      { environment: env },
                      (v) => setSystemOutput(v.output),
                    )
                  }
                >
                  更新一覧を取得
                </button>
                {systemOutput && <pre>{systemOutput}</pre>}
                {environment?.os === "windows" && (
                  <form
                    onSubmit={(e) => {
                      e.preventDefault();
                      act(
                        "WinGetの配布情報を確認中",
                        "plan_system_update",
                        { environment: env, package: systemPackage },
                        openPlan,
                      );
                    }}
                  >
                    <label htmlFor="system-package">
                      更新するWinGetパッケージID
                    </label>
                    <div className="input-row">
                      <input
                        id="system-package"
                        value={systemPackage}
                        onChange={(e) => setSystemPackage(e.target.value)}
                        placeholder="一覧にある正確なパッケージID"
                      />
                      <button disabled={!!busy || !systemPackage}>
                        更新計画を作成
                      </button>
                    </div>
                  </form>
                )}
                <h3>管理ツール本体の更新</h3>
                <div className="button-row">
                  {["mise", "uv", "rustup"].map((manager) => (
                    <button
                      key={manager}
                      disabled={!!busy}
                      onClick={() =>
                        act(
                          "導入元と更新方法を確認中",
                          "plan_manager_update",
                          { environment: env, manager },
                          openPlan,
                        )
                      }
                    >
                      {manager} の更新を計画
                    </button>
                  ))}
                </div>
              </section>
            </>
          )}
          {page === "projects" && (
            <>
              <PageHeading
                eyebrow="PROJECTS"
                title="プロジェクト"
                description="リポジトリと、そこで必要なツール・パッケージを管理します。"
                action={
                  <div className="button-row">
                    <button
                      disabled={!!busy}
                      onClick={() =>
                        act(
                          "GitHubのリポジトリを取得中",
                          "github_repos",
                          null,
                          setGitFork,
                        )
                      }
                    >
                      <GitFork size={16} />
                      GitHubから
                    </button>
                    <button
                      className="primary"
                      onClick={() => setAdding(!adding)}
                    >
                      <FolderPlus size={16} />
                      フォルダーを登録
                    </button>
                  </div>
                }
              />
              {adding && (
                <section className="panel details-panel">{rootForm}</section>
              )}
              {!projects.length ? (
                <Empty
                  icon={FolderGit2}
                  title="プロジェクトを登録しましょう"
                  action={
                    <button onClick={() => setAdding(true)}>
                      フォルダーを選ぶ
                    </button>
                  }
                >
                  クローン済みのリポジトリ、またはリポジトリをまとめたフォルダーを登録できます。
                </Empty>
              ) : (
                <div className="project-layout">
                  <aside className="project-list" aria-label="登録プロジェクト">
                    {projects.map((p) => (
                      <button
                        key={p.id}
                        className={
                          p.id === project?.id
                            ? "project-link active"
                            : "project-link"
                        }
                        onClick={() => setProjectId(p.id)}
                      >
                        <FolderGit2 size={19} />
                        <span>
                          <strong>{p.name}</strong>
                          <small>{p.package_manager}</small>
                        </span>
                        <ChevronRight size={14} />
                      </button>
                    ))}
                  </aside>
                  <div className="project-detail">
                    {!project ? (
                      <Empty icon={FolderGit2} title="プロジェクトを選択">
                        左の一覧から調査結果を確認してください。
                      </Empty>
                    ) : (
                      <>
                        <section className="panel details-panel">
                          <div className="section-heading">
                            <div>
                              <h2>{project.name}</h2>
                              <p className="path">{project.path}</p>
                            </div>
                            <button
                              disabled={!!busy}
                              onClick={() =>
                                act(
                                  "プロジェクトを再調査中",
                                  "rescan_project",
                                  { project: project.id },
                                )
                              }
                            >
                              <RefreshCw size={15} />
                              再調査
                            </button>
                          </div>
                          <div className="metadata">
                            <span>
                              <GitBranch size={14} />
                              {project.branch || "detached HEAD"}
                            </span>
                            <Badge
                              status={project.git_status ? "partial" : "ok"}
                            >
                              {project.git_status
                                ? "未コミット変更あり"
                                : "変更なし"}
                            </Badge>
                            <span>{project.package_manager}</span>
                            <span>{displayDate(project.scanned_at)}</span>
                          </div>
                          {project.git_status && (
                            <pre>{project.git_status}</pre>
                          )}
                          <div className="button-row">
                            <button
                              disabled={!!busy}
                              onClick={() =>
                                act(
                                  "公開レジストリ・OSVと照合中",
                                  "check_updates",
                                  { project: project.id },
                                  () => setSelected([]),
                                )
                              }
                            >
                              <ShieldCheck size={16} />
                              依存の更新・脆弱性を照合
                            </button>
                            <button
                              disabled={!!busy}
                              onClick={() =>
                                act(
                                  "PRとCIの状態を確認中",
                                  "pr_checks",
                                  { project: project.id },
                                  setPr,
                                )
                              }
                            >
                              PR・CIを確認
                            </button>
                            <button
                              disabled={!!busy}
                              onClick={() =>
                                act(
                                  "差分を取得中",
                                  "project_diff",
                                  { project: project.id },
                                  setPr,
                                )
                              }
                            >
                              変更差分を確認
                            </button>
                            <button
                              disabled={!!busy}
                              onClick={() =>
                                act(
                                  "同期計画を作成中",
                                  "plan_sync",
                                  { project: project.id },
                                  openPlan,
                                )
                              }
                            >
                              既定ブランチへの同期を計画
                            </button>
                            <button
                              disabled={!!busy}
                              onClick={() =>
                                act(
                                  "コミットとCIを確認中",
                                  "plan_merge",
                                  { project: project.id },
                                  openPlan,
                                )
                              }
                            >
                              CI成功後のマージを計画
                            </button>
                          </div>
                          <small>
                            照合ではパッケージ名と版を公開レジストリ・OSVへ送信します。ソースコードや認証情報は送信しません。
                          </small>
                        </section>
                        <section className="panel details-panel">
                          <h3>デバイスのツールと指定を揃える</h3>
                          <p className="muted">
                            導入済みの版へプロジェクトの固定を変更します。Pythonでは仮想環境の同期を伴います。互換性の検証は別途必要です。
                          </p>
                          <div className="input-row">
                            <label className="sr-only" htmlFor="pin-tool">
                              固定するツールとバージョン
                            </label>
                            <select
                              id="pin-tool"
                              value={pinTool}
                              onChange={(e) => setPinTool(e.target.value)}
                            >
                              <option value="">導入済みの版を選択</option>
                              {tools
                                .filter(
                                  (t) =>
                                    t.installed &&
                                    ["mise", "uv", "rustup"].includes(
                                      t.manager,
                                    ),
                                )
                                .map((t) => (
                                  <option key={t.id} value={t.id}>
                                    {t.name} {t.version} ({t.manager})
                                  </option>
                                ))}
                            </select>
                            <button
                              disabled={!!busy || !pinTool}
                              onClick={() =>
                                act(
                                  "固定版の変更計画を作成中",
                                  "plan_pin",
                                  { project: project.id, tool: pinTool },
                                  openPlan,
                                )
                              }
                            >
                              固定版の変更を計画
                            </button>
                          </div>
                          <button
                            disabled={!!busy}
                            onClick={() =>
                              act(
                                "プロジェクトの有効版を確認中",
                                "effective_tools",
                                { project: project.id },
                                setPr,
                              )
                            }
                          >
                            この場所で有効なツールを確認
                          </button>
                        </section>
                        <section className="panel details-panel">
                          <h3>検証スクリプト</h3>
                          <div className="check-row">
                            {project.scripts
                              .filter((s) =>
                                [
                                  "test",
                                  "build",
                                  "lint",
                                  "typecheck",
                                  "check",
                                  "uv-pytest",
                                  "uv-ruff",
                                  "cargo-check",
                                  "cargo-test",
                                ].includes(s.split("::").at(-1) ?? ""),
                              )
                              .map((s) => (
                                <label key={s}>
                                  <input
                                    type="checkbox"
                                    checked={checks.includes(s)}
                                    onChange={() =>
                                      toggle(checks, s, setChecks)
                                    }
                                  />
                                  {s}
                                </label>
                              ))}
                          </div>
                          {!project.scripts.some((s) =>
                            [
                              "test",
                              "build",
                              "lint",
                              "typecheck",
                              "check",
                              "uv-pytest",
                              "uv-ruff",
                              "cargo-check",
                              "cargo-test",
                            ].includes(s.split("::").at(-1) ?? ""),
                          ) && (
                            <p className="muted">
                              対応する検証コマンドはありません。検証手順をプロジェクト側へ追加して再調査してください。
                            </p>
                          )}
                          <button
                            disabled={!!busy || !checks.length}
                            onClick={() =>
                              act(
                                "検証計画を作成中",
                                "plan_validate",
                                { project: project.id, checks },
                                openPlan,
                              )
                            }
                          >
                            選択した検証の計画を作成
                          </button>
                        </section>
                        {!!projectUpdates.length && (
                          <section className="panel">
                            <div className="section-heading padded">
                              <h3>更新候補と既知の問題</h3>
                              <Badge status="partial">実行検証は別途必要</Badge>
                            </div>
                            {updatesTable(projectUpdates, true)}
                            <div className="plan-actions">
                              <label>
                                <input
                                  type="checkbox"
                                  checked={publish}
                                  onChange={(e) => setPublish(e.target.checked)}
                                />
                                検証後のコミット・プッシュ・PR作成も計画に含める
                              </label>
                              <button
                                className="primary"
                                disabled={!!busy || !selected.length}
                                onClick={() =>
                                  act(
                                    "更新計画を作成中",
                                    "plan_update",
                                    {
                                      project: project.id,
                                      selected,
                                      checks,
                                      publish,
                                    },
                                    openPlan,
                                  )
                                }
                              >
                                選択した更新の計画を作成
                                <ArrowRight size={15} />
                              </button>
                            </div>
                          </section>
                        )}
                        <section className="panel">
                          <div className="section-heading padded">
                            <h3>固定ツール・依存関係</h3>
                            <span className="muted">
                              {project.requirements.length}件（表示は最大200件）
                            </span>
                          </div>
                          <div className="padded">
                            <label htmlFor="dependency-search">
                              依存関係を検索
                            </label>
                            <input
                              id="dependency-search"
                              value={dependencyQuery}
                              onChange={(e) =>
                                setDependencyQuery(e.target.value)
                              }
                              placeholder="パッケージ名・版・宣言元"
                            />
                          </div>
                          <div className="table-scroll capped">
                            <table>
                              <thead>
                                <tr>
                                  <th>名前</th>
                                  <th>指定 / ロック版</th>
                                  <th>定義元</th>
                                </tr>
                              </thead>
                              <tbody>
                                {project.requirements
                                  .filter((r) =>
                                    `${r.name} ${r.requested} ${r.source}`
                                      .toLowerCase()
                                      .includes(dependencyQuery.toLowerCase()),
                                  )
                                  .slice(0, 200)
                                  .map((r, i) => (
                                    <tr key={i}>
                                      <td>
                                        <strong>{r.name}</strong>
                                        <small>
                                          {r.manager} · {r.section}
                                        </small>
                                      </td>
                                      <td>
                                        <code>{r.requested}</code>
                                      </td>
                                      <td className="path">{r.source}</td>
                                    </tr>
                                  ))}
                              </tbody>
                            </table>
                          </div>
                        </section>
                        {!!project.venvs.length && (
                          <section className="panel details-panel">
                            <h3>Python仮想環境と参照元</h3>
                            {project.venvs.map((v, index) => (
                              <div className="venv" key={v.path}>
                                <strong>{v.version || "版不明"}</strong>
                                <p className="path">{v.path}</p>
                                <p className="path">参照元 → {v.home}</p>
                                <small>{v.note}</small>
                                <button
                                  disabled={!!busy}
                                  onClick={() =>
                                    act(
                                      "Python参照の確認計画を作成中",
                                      "plan_verify_venv",
                                      { project: project.id, index },
                                      openPlan,
                                    )
                                  }
                                >
                                  Pythonの実体確認を計画
                                </button>
                              </div>
                            ))}
                          </section>
                        )}
                        <Findings items={project.findings} />
                        <button
                          className="text-button danger"
                          disabled={!!busy}
                          onClick={() =>
                            act(
                              "一覧から登録を解除中",
                              "forget_project",
                              { project: project.id },
                              () => setProjectId(""),
                            )
                          }
                        >
                          一覧から登録を解除（ファイルは残ります）
                        </button>
                      </>
                    )}
                  </div>
                </div>
              )}
            </>
          )}
          {page === "plans" && (
            <>
              <PageHeading
                eyebrow="UPDATE PLANS"
                title="更新計画"
                description="変更と実行するコマンドを確認し、一工程ずつ進めます。"
              />
              {!plans.length ? (
                <Empty icon={ListChecks} title="まだ更新計画はありません">
                  プロジェクトまたは開発環境で候補を照合し、計画を作成してください。
                </Empty>
              ) : (
                <>
                  <div className="plan-tabs">
                    {plans.map((p) => (
                      <button
                        className={p.id === plan?.id ? "active" : ""}
                        key={p.id}
                        onClick={() => setPlanId(p.id)}
                      >
                        {p.title}
                        <Badge status={p.status} />
                      </button>
                    ))}
                  </div>
                  {plan && (
                    <>
                      <section className="panel details-panel">
                        <div className="section-heading">
                          <div>
                            <h2>{plan.title}</h2>
                            <p>{plan.reason}</p>
                          </div>
                          <Badge status={plan.status} />
                        </div>
                        {plan.evidence.map((e, i) => (
                          <p className="evidence" key={i}>
                            <Check size={15} />
                            {e}
                          </p>
                        ))}
                        <div className="plan-warnings">
                          {plan.warnings.map((w, i) => (
                            <p key={i}>
                              <CircleHelp size={15} />
                              {w}
                            </p>
                          ))}
                        </div>
                      </section>
                      <div className="steps">
                        {plan.steps.map((s, i) => (
                          <section
                            key={s.id}
                            className={`panel execution-step ${s.status}`}
                          >
                            <div className="step-title">
                              <span className="step-number">
                                {s.status === "success" ? (
                                  <Check size={17} />
                                ) : (
                                  String(i + 1).padStart(2, "0")
                                )}
                              </span>
                              <h3>{s.label}</h3>
                              <Badge status={s.status} />
                            </div>
                            <p className="path">実行場所: {s.cwd}</p>
                            <pre>
                              {[
                                s.program,
                                ...s.args.map((a) => JSON.stringify(a)),
                              ].join(" ")}
                            </pre>
                            {s.output && (
                              <details open={s.status === "failed"}>
                                <summary>実行結果</summary>
                                <pre>{s.output}</pre>
                              </details>
                            )}
                          </section>
                        ))}
                      </div>
                      {next >= 0 && (
                        <section className="execution-confirm">
                          <label>
                            <input
                              type="checkbox"
                              checked={confirmed}
                              onChange={(e) => setConfirmed(e.target.checked)}
                            />
                            「{plan.steps[next].label}
                            」の対象・コマンドと影響を確認しました
                          </label>
                          <button
                            className="primary"
                            disabled={!!busy || !confirmed}
                            onClick={() =>
                              act(
                                "工程を実行中",
                                "execute_step",
                                { plan: plan.id, confirmed: true },
                                () => setConfirmed(false),
                              )
                            }
                          >
                            次の工程を実行
                            <ArrowRight size={16} />
                          </button>
                        </section>
                      )}
                      {["failed", "cancelled", "interrupted"].includes(
                        plan.status,
                      ) && (
                        <p className="warning-note">
                          結果を確認し、プロジェクトを再調査して再計画してください。失敗した操作を自動で繰り返しません。
                        </p>
                      )}
                    </>
                  )}
                </>
              )}
            </>
          )}
          {page === "history" && (
            <>
              <PageHeading
                eyebrow="ACTIVITY LOG"
                title="実行履歴"
                description="調査・計画・実行の経緯を、このデバイスに保存しています。"
              />
              {!data.history.length ? (
                <Empty icon={Clock3} title="まだ履歴はありません">
                  操作すると、結果と実行日時がここに記録されます。
                </Empty>
              ) : (
                <section className="panel">
                  {data.history.map((h) => (
                    <details className="history-row" key={h.id}>
                      <summary>
                        <Clock3 size={16} />
                        <strong>{h.action}</strong>
                        <span>{h.target}</span>
                        <Badge status={h.status} />
                        <time>{displayDate(h.at)}</time>
                      </summary>
                      <pre>{h.detail}</pre>
                    </details>
                  ))}
                </section>
              )}
            </>
          )}
          {page === "settings" && (
            <>
              <PageHeading
                eyebrow="PREFERENCES"
                title="設定"
                description="調査対象と管理ツールの場所を指定できます。標準外の配置にも対応します。"
              />
              <SettingsPanel
                settings={data.settings}
                env={env}
                busy={!!busy}
                save={(s) =>
                  act("設定を保存中", "save_settings", s, () =>
                    setNotice("設定を保存しました"),
                  )
                }
                detect={() =>
                  act("利用可能な環境を検出中", "detect_environments")
                }
                mcp={(path) =>
                  act("MCP設定を静的調査中", "scan_mcp_config", { path }, () =>
                    setNotice("MCP設定の起動先を取得しました"),
                  )
                }
              />
              <section className="panel details-panel">
                <h3>保存したMCPの参照情報</h3>
                {data.links?.length ? (
                  data.links.map((l, i) => (
                    <div className="venv" key={i}>
                      <strong>{l.name}</strong>
                      <p className="path">{l.command}</p>
                      <p className="path">
                        {l.home || "仮想環境の参照元は未検出"}
                      </p>
                      <small>{l.config}</small>
                    </div>
                  ))
                ) : (
                  <p className="muted">
                    MCP設定ファイルを指定すると、起動コマンドと仮想環境の静的情報を表示します。
                  </p>
                )}
              </section>
              <section className="panel details-panel">
                <h3>データの保存先</h3>
                <p className="path">
                  {data.data_path ||
                    "デスクトップアプリの初回起動時に作成します"}
                </p>
                <p className="muted">
                  OSのアプリデータディレクトリを利用します。管理するPythonやNodeをこの場所へ移動することはありません。
                </p>
                <h3>旧版の整理について</h3>
                <p className="muted">
                  未参照と完全な未使用は異なります。Windowsのmise管理版は、登録範囲・管理元・PATH・実行中プロセスを再確認して削除計画を作成できます。環境一覧の詳細から進めてください。WSL・Python・OS用・アプリ同梱ランタイムの削除実行は対象外です。
                </p>
              </section>
            </>
          )}
          <footer>
            DEV STEWARD <span>利用関係を確かめて、次の更新へ。</span>
          </footer>
        </main>
      </div>
      {github !== null && (
        <Modal
          title="GitHubからプロジェクトを登録"
          close={() => setGitFork(null)}
        >
          <p>
            GitHub
            CLIで認証したアカウントのリポジトリ（最大300件）です。まだクローンしていないものを取り込めます。
          </p>
          <label htmlFor="clone-parent">クローン先の親フォルダー</label>
          <div className="input-row">
            <input
              id="clone-parent"
              value={cloneParent}
              onChange={(e) => setCloneParent(e.target.value)}
            />
            <button
              onClick={() =>
                chooseDirectory().then((v) => {
                  if (v) setCloneParent(v);
                })
              }
            >
              選択
            </button>
          </div>
          <div className="repo-list">
            {github.length ? (
              github.map((r) => (
                <div className="repo-row" key={r.nameWithOwner}>
                  <FolderGit2 size={19} />
                  <div>
                    <strong>{r.nameWithOwner}</strong>
                    <p>{r.description || "説明なし"}</p>
                  </div>
                  <button
                    disabled={!!busy || !cloneParent || r.isArchived}
                    onClick={() =>
                      act(
                        "リポジトリをクローン中",
                        "clone_repo",
                        { repo: r.nameWithOwner, parent: cloneParent },
                        (p) => {
                          setGitFork(null);
                          setEnv("local");
                          setProjectId(p.id);
                        },
                      )
                    }
                  >
                    クローン
                  </button>
                </div>
              ))
            ) : (
              <p>
                リポジトリを取得できませんでした。GitHub
                CLIの認証とアクセス権を確認してください。
              </p>
            )}
          </div>
        </Modal>
      )}
      {pr !== null && (
        <Modal title="取得した情報" close={() => setPr(null)}>
          <p>
            取得した時点の情報です。PR・CIの結果では、対象コミット（headRefOid）も確認してください。
          </p>
          <pre>{typeof pr === "string" ? pr : JSON.stringify(pr, null, 2)}</pre>
        </Modal>
      )}
    </div>
  );
}
function PageHeading({
  eyebrow,
  title,
  description,
  action,
}: {
  eyebrow: string;
  title: string;
  description: string;
  action?: ReactNode;
}) {
  return (
    <div className="page-heading">
      <div>
        <div className="eyebrow">{eyebrow}</div>
        <h1>{title}</h1>
        <p>{description}</p>
      </div>
      {action}
    </div>
  );
}
function Modal({
  title,
  close,
  children,
}: {
  title: string;
  close: () => void;
  children: ReactNode;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const el = dialog.current;
    el?.showModal();
    return () => el?.close();
  }, []);
  return (
    <dialog ref={dialog} aria-label={title} onCancel={close}>
      <div className="modal-heading">
        <h2>{title}</h2>
        <button className="icon-button" aria-label="閉じる" onClick={close}>
          <X size={20} />
        </button>
      </div>
      {children}
    </dialog>
  );
}
function SettingsPanel({
  settings,
  env,
  busy,
  save,
  detect,
  mcp,
}: {
  settings: Settings;
  env: string;
  busy: boolean;
  save: (s: Settings) => void;
  detect: () => void;
  mcp: (path: string) => void;
}) {
  const [manager, setManager] = useState("mise");
  const [path, setPath] = useState("");
  const [config, setConfig] = useState("");
  return (
    <>
      <section className="panel details-panel">
        <h2>実行環境</h2>
        <p>
          WSLのディストリビューションを検出して環境一覧へ追加します。検出時に各ディストリビューションを起動することがあります。
        </p>
        <button disabled={busy} onClick={detect}>
          Windows・WSLの環境を検出
        </button>
      </section>
      <section className="panel details-panel">
        <h2>管理ツールの実行ファイル</h2>
        <p className="muted">
          自動検出できないときに、現在選択している環境の絶対パスを指定します。指定先は実行されるため、信頼できる管理ツールを選んでください。
        </p>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            save({
              ...settings,
              manager_overrides: [
                ...settings.manager_overrides.filter(
                  ([e, n]) => e !== env || n !== manager,
                ),
                [env, manager, path],
              ],
            });
            setPath("");
          }}
        >
          <div className="input-row">
            <label className="sr-only" htmlFor="manager">
              管理ツール
            </label>
            <select
              id="manager"
              value={manager}
              onChange={(e) => setManager(e.target.value)}
            >
              {[
                "mise",
                "uv",
                "rustup",
                "py",
                "pymanager",
                "pyenv",
                "conda",
                "fnm",
                "nvm",
                "python",
                "python3",
                "node",
                "java",
                "go",
                "dotnet",
                "bun",
                "deno",
                "git",
                "gh",
                "winget",
                "brew",
              ].map((n) => (
                <option key={n}>{n}</option>
              ))}
            </select>
            <label className="sr-only" htmlFor="manager-path">
              実行ファイルの絶対パス
            </label>
            <input
              id="manager-path"
              required
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="実行ファイルの絶対パス"
            />
            <button disabled={busy || !path}>保存</button>
          </div>
        </form>
        {settings.manager_overrides.map(([e, n, p]) => (
          <div className="override" key={`${e}-${n}`}>
            <span>
              <strong>
                {e} / {n}
              </strong>
              <small className="path">{p}</small>
            </span>
            <button
              disabled={busy}
              onClick={() =>
                save({
                  ...settings,
                  manager_overrides: settings.manager_overrides.filter(
                    ([ee, nn]) => ee !== e || nn !== n,
                  ),
                })
              }
            >
              自動検出に戻す
            </button>
          </div>
        ))}
      </section>
      <section className="panel details-panel">
        <h2>MCP起動設定の調査</h2>
        <p className="muted">
          ローカルのJSON・TOML設定から起動コマンドだけを取得します。環境変数・トークン・コマンド引数は保存しません。サーバーへの接続確認は行いません。
        </p>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            mcp(config);
          }}
        >
          <label htmlFor="mcp-config">設定ファイルの絶対パス</label>
          <div className="input-row">
            <input
              id="mcp-config"
              value={config}
              onChange={(e) => setConfig(e.target.value)}
              required
            />
            <button disabled={busy || !config}>静的に調査</button>
          </div>
        </form>
      </section>
      <section className="panel details-panel">
        <h2>更新方針</h2>
        <p>
          <ShieldCheck size={17} /> 同一メジャーの安定版を優先（0.xは同一minor）
        </p>
        <p className="muted">
          Python本体は同一minorの修正版を候補にします。メジャー更新、自動マージ、自動削除は有効になっていません。
        </p>
        <h3>登録した調査フォルダー</h3>
        {settings.roots.length ? (
          settings.roots.map(([e, p]) => (
            <p className="path" key={e + p}>
              {e} / {p}
            </p>
          ))
        ) : (
          <p className="muted">まだ登録されていません。</p>
        )}
      </section>
    </>
  );
}
