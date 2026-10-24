import { describe, it, expect } from "vitest";
import { relatedProjects, executableStep, groupTools } from "../src/logic";
import type { Project, Tool } from "../src/types";
describe("manager grouping", () => {
  const rows = [
    { id: "a", manager: "uv", name: "python", version: "3.12.8" },
    { id: "b", manager: "mise", name: "node", version: "24.0.0" },
    { id: "c", manager: "uv", name: "python", version: "3.11.9" },
    { id: "d", manager: "winget", name: "Application", version: "1" },
    { id: "e", manager: "python-launcher", name: "python", version: "" },
    { id: "f", manager: "custom-manager", name: "other", version: "2" },
  ] as Tool[];
  it("keeps each manager in a separate group without losing versions", () => {
    const groups = groupTools(rows, "", false);
    expect(groups.map(([manager]) => manager)).toEqual([
      "mise",
      "uv",
      "python-launcher",
      "custom-manager",
    ]);
    expect(groups[1][1].map((t) => t.id)).toEqual(["a", "c"]);
    expect(
      groupTools(rows, "", true).flatMap(([, tools]) => tools),
    ).toHaveLength(6);
  });
  it("searches across groups and Japanese labels and handles no matches", () => {
    expect(groupTools(rows, "uv 管理", false).map(([key]) => key)).toEqual([
      "uv",
    ]);
    expect(groupTools(rows, "3.11", false)[0][1].map((t) => t.id)).toEqual([
      "c",
    ]);
    expect(groupTools(rows, "not-found", false)).toEqual([]);
  });
});
describe("execution state", () => {
  it("only exposes next sequential step", () => {
    expect(
      executableStep("ready", [{ status: "success" }, { status: "pending" }]),
    ).toBe(1);
  });
  it("never replays failed or interrupted operations", () => {
    for (const status of ["failed", "cancelled", "interrupted", "running"])
      expect(executableStep(status, [{ status: "pending" }])).toBe(-1);
  });
  it("blocks execution after failure", () => {
    expect(
      executableStep("ready", [{ status: "failed" }, { status: "pending" }]),
    ).toBe(-1);
  });
});
describe("relationships", () => {
  const tool = {
    environment: "local",
    name: "python",
    version: "3.13.17",
  } as Tool;
  it("does not mix Windows and WSL references", () => {
    const p = {
      environment: "wsl:custom",
      requirements: [{ name: "python" }],
      venvs: [],
    } as unknown as Project;
    expect(relatedProjects(tool, [p])).toHaveLength(0);
  });
  it("links declared runtimes without claiming actual execution", () => {
    const p = {
      environment: "local",
      requirements: [{ name: "python" }],
      venvs: [],
    } as unknown as Project;
    expect(relatedProjects(tool, [p])).toEqual([p]);
  });
});
