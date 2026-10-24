import { describe, it, expect } from "vitest";
import { repositoryKey, projectGroups, proposalLocals } from "../src/workspace";
import type { Project, GithubReport, Snapshot } from "../src/types";
describe("repository identity", () => {
  it("links SSH and HTTPS GitHub remotes, while retaining host and repo boundaries", () => {
    expect(repositoryKey("git@github.com:Example/Demo.git")).toBe(
      repositoryKey("https://github.com/example/demo"),
    );
    expect(repositoryKey("ssh://git@github.com:22/Example/Demo.git")).toBe(
      "github.com/example/demo",
    );
    expect(repositoryKey("https://github.com:22/example/demo")).not.toBe(
      repositoryKey("https://github.com/example/demo"),
    );
    expect(repositoryKey("https://other.example/example/demo")).not.toBe(
      repositoryKey("https://github.com/example/demo"),
    );
    expect(repositoryKey("https://gitlab.example/Team/Demo")).not.toBe(
      repositoryKey("https://gitlab.example/team/demo"),
    );
    expect(repositoryKey("file:///tmp/project")).toBeNull();
    expect(repositoryKey("Q:\\projects\\demo")).toBeNull();
  });
  it("groups a repository without collapsing environments, checkouts or revisions", () => {
    const locals = [
      {
        id: "windows-copy",
        name: "Demo",
        remote: "git@github.com:Example/Demo.git",
        environment: "local",
      },
      {
        id: "wsl-copy",
        name: "Demo",
        remote: "https://github.com/example/demo.git",
        environment: "wsl:test",
      },
    ] as Project[];
    const reports = [
      {
        id: "main",
        repository: "Example/Demo",
        url: "https://github.com/Example/Demo",
        host: "github.com",
        branch: "main",
      },
      {
        id: "release",
        repository: "Example/Demo",
        url: "https://github.com/Example/Demo",
        host: "github.com",
        branch: "release",
      },
    ] as GithubReport[];
    const groups = projectGroups(locals, reports);
    expect(groups).toHaveLength(1);
    expect(groups[0].locals).toHaveLength(2);
    expect(groups[0].reports).toHaveLength(2);
    expect(
      proposalLocals(
        { projects: locals, github_projects: reports } as Snapshot,
        "main",
      ),
    ).toHaveLength(2);
  });
  it("never associates unrelated local folders by display name", () => {
    const locals = [
      { id: "a", name: "demo", remote: "" },
      { id: "b", name: "demo", remote: "" },
    ] as Project[];
    expect(projectGroups(locals, [])).toHaveLength(2);
  });
});
