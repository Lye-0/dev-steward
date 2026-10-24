import { invoke, isTauri } from "@tauri-apps/api/core";
export async function call<T = unknown>(
  method: string,
  payload: unknown = null,
): Promise<T> {
  if (isTauri()) return invoke<T>("steward", { method, payload });
  throw new Error(
    "環境の調査にはデスクトップアプリが必要です。pnpm tauri dev で起動してください。",
  );
}
export async function chooseDirectory(): Promise<string | null> {
  if (!isTauri()) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const result = await open({
    directory: true,
    multiple: false,
    title: "調査するフォルダーを選択",
  });
  return typeof result === "string" ? result : null;
}
