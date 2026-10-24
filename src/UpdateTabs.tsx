import { useRef } from "react";
export type UpdateView = "candidates" | "plans";
const tabs = [
  { id: "candidates" as const, label: "更新候補" },
  { id: "plans" as const, label: "計画・進行状況" },
];
export function UpdateTabs({
  value,
  onChange,
}: {
  value: UpdateView;
  onChange: (value: UpdateView) => void;
}) {
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  return (
    <div
      className="workspace-tabs"
      role="tablist"
      aria-label="更新の表示"
      aria-orientation="horizontal"
    >
      {tabs.map((tab, index) => (
        <button
          key={tab.id}
          ref={(el) => {
            buttons.current[index] = el;
          }}
          type="button"
          role="tab"
          id={`update-tab-${tab.id}`}
          aria-controls={`update-panel-${tab.id}`}
          aria-selected={value === tab.id}
          tabIndex={value === tab.id ? 0 : -1}
          onClick={() => onChange(tab.id)}
          onKeyDown={(e) => {
            let next: number;
            if (e.key === "ArrowRight") next = (index + 1) % tabs.length;
            else if (e.key === "ArrowLeft")
              next = (index + tabs.length - 1) % tabs.length;
            else if (e.key === "Home") next = 0;
            else if (e.key === "End") next = tabs.length - 1;
            else return;
            e.preventDefault();
            onChange(tabs[next].id);
            buttons.current[next]?.focus();
          }}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
