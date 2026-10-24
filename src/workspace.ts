import type { Project, GithubReport, Snapshot } from "./types";
export type ProjectTarget = { kind: "local" | "github"; id: string };
export function repositoryKey(
  remote: string,
  githubHosts: string[] = ["github.com"],
): string | null {
  let url: URL;
  try {
    url = new URL(remote);
  } catch {
    const scp = /^(?:[^@/]+@)?([^:/]+):([^\\]+)$/.exec(remote);
    if (!scp) return null;
    try {
      url = new URL(`ssh://${scp[1]}/${scp[2]}`);
    } catch {
      return null;
    }
  }
  if (!["https:", "http:", "ssh:", "git:"].includes(url.protocol)) return null;
  const host = (
    url.protocol === "ssh:" && url.port === "22" ? url.hostname : url.host
  ).toLowerCase();
  let path = url.pathname.replace(/^\/+|\/+$/g, "").replace(/\.git$/i, "");
  if (!host || !path) return null;
  if (githubHosts.some((h) => h.toLowerCase() === host))
    path = path.toLowerCase();
  return `${host}/${path}`;
}
export interface ProjectGroup {
  key: string;
  name: string;
  locals: Project[];
  reports: GithubReport[];
}
export function projectGroups(
  projects: Project[],
  reports: GithubReport[],
): ProjectGroup[] {
  const hosts = ["github.com", ...reports.map((r) => r.host)];
  const groups = new Map<string, ProjectGroup>();
  for (const report of reports) {
    const key = repositoryKey(report.url, hosts) ?? `github:${report.id}`;
    const group = groups.get(key) ?? {
      key,
      name: report.repository,
      locals: [],
      reports: [],
    };
    group.reports.push(report);
    groups.set(key, group);
  }
  for (const project of projects) {
    const key = repositoryKey(project.remote, hosts) ?? `local:${project.id}`;
    const group = groups.get(key) ?? {
      key,
      name: project.name,
      locals: [],
      reports: [],
    };
    group.locals.push(project);
    groups.set(key, group);
  }
  return [...groups.values()].sort((a, b) => a.name.localeCompare(b.name));
}
export function proposalLocals(data: Snapshot, reportId: string): Project[] {
  return (
    projectGroups(data.projects, data.github_projects).find((g) =>
      g.reports.some((r) => r.id === reportId),
    )?.locals ?? []
  );
}
