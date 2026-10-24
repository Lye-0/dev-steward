import { useEffect, useRef } from "react";
import { Check, LoaderCircle, X } from "lucide-react";
export function Feedback({
  busy,
  error,
  notice,
  onDismissError,
  onDismissNotice,
  onCancel,
}: {
  busy: string;
  error: string;
  notice: string;
  onDismissError: () => void;
  onDismissNotice: () => void;
  onCancel: () => void;
}) {
  const region = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (error) {
      const alert =
        region.current?.querySelector<HTMLElement>('[role="alert"]');
      alert?.scrollIntoView({ block: "nearest" });
      alert?.focus({ preventScroll: true });
    }
  }, [error]);
  return (
    <div ref={region} className="feedback" aria-live="polite">
      {busy && (
        <div className="busy" role="status">
          <LoaderCircle className="spin" size={17} />
          <span>{busy}…</span>
          {busy === "工程を実行中" && (
            <button onClick={onCancel}>中断する</button>
          )}
        </div>
      )}
      {error && (
        <div role="alert" tabIndex={-1} className="error">
          <strong>操作を完了できませんでした</strong>
          <div className="feedback-message">
            <span>
              {error.length > 280 ? error.split("\n")[0].slice(0, 240) : error}
            </span>
            {error.length > 280 && (
              <details>
                <summary>エラーの詳細を表示</summary>
                <pre>{error}</pre>
              </details>
            )}
          </div>
          <button aria-label="エラーを閉じる" onClick={onDismissError}>
            <X size={16} />
          </button>
        </div>
      )}
      {notice && (
        <div className="notice" role="status">
          <Check size={17} />
          <span className="feedback-message">{notice}</span>
          <button
            className="feedback-dismiss"
            aria-label="通知を閉じる"
            onClick={onDismissNotice}
          >
            <X size={16} />
          </button>
        </div>
      )}
    </div>
  );
}
