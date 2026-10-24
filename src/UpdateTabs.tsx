import { useRef } from "react";
export type UpdateView = "environment" | "projects" | "plans";
export type EnvironmentUpdateCategory = "tools" | "system" | "managers";
const tabs = [
  { id: "environment" as const, label: "環境の更新" },
  { id: "projects" as const, label: "プロジェクトの更新" },
  { id: "plans" as const, label: "計画・進行状況" },
];
export function UpdateTabs({
  value,
  onChange,
}: {
  value: UpdateView;
  onChange: (value: UpdateView) => void;
}) {
  return (
    <SelectionTabs
      items={tabs}
      value={value}
      onChange={onChange}
      label="更新の表示"
      prefix="update"
    />
  );
}
export function EnvironmentUpdateTabs({
  value,
  onChange,
}: {
  value: EnvironmentUpdateCategory;
  onChange: (value: EnvironmentUpdateCategory) => void;
}) {
  return (
    <SelectionTabs
      items={[
        { id: "tools", label: "開発ツール" },
        { id: "system", label: "OS・アプリ" },
        { id: "managers", label: "管理ツール本体" },
      ]}
      value={value}
      onChange={onChange}
      label="環境更新の分類"
      prefix="environment-update"
      secondary
    />
  );
}
function SelectionTabs<T extends string>({
  items,
  value,
  onChange,
  label,
  prefix,
  secondary = false,
}: {
  items: readonly { id: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
  label: string;
  prefix: string;
  secondary?: boolean;
}) {
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  return (
    <div
      className={`workspace-tabs ${secondary ? "update-categories" : ""}`}
      role="tablist"
      aria-label={label}
      aria-orientation="horizontal"
    >
      {items.map((tab, index) => (
        <button
          key={tab.id}
          ref={(el) => {
            buttons.current[index] = el;
          }}
          type="button"
          role="tab"
          id={`${prefix}-tab-${tab.id}`}
          aria-controls={`${prefix}-panel-${tab.id}`}
          aria-selected={value === tab.id}
          tabIndex={value === tab.id ? 0 : -1}
          onClick={() => onChange(tab.id)}
          onKeyDown={(e) => {
            let next: number;
            if (e.key === "ArrowRight") next = (index + 1) % items.length;
            else if (e.key === "ArrowLeft")
              next = (index + items.length - 1) % items.length;
            else if (e.key === "Home") next = 0;
            else if (e.key === "End") next = items.length - 1;
            else return;
            e.preventDefault();
            onChange(items[next].id);
            buttons.current[next]?.focus();
          }}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
