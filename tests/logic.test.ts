import { describe, it, expect } from "vitest";
import { relatedProjects, executableStep } from "../src/logic";
import type { Project, Tool } from "../src/types";
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
