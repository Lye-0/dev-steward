export interface Environment {
  id: string;
  name: string;
  kind: string;
  home: string;
  os: string;
}
export interface Finding {
  scope: string;
  status: string;
  message: string;
}
export interface Tool {
  id: string;
  environment: string;
  manager: string;
  name: string;
  version: string;
  path: string;
  active: boolean;
  installed: boolean;
  source: string;
  requested: string;
  note: string;
}
export interface Inventory {
  environment: string;
  captured_at: string;
  tools: Tool[];
  findings: Finding[];
  manager_paths: [string, string][];
}
export interface Requirement {
  manager: string;
  name: string;
  requested: string;
  source: string;
  section: string;
  resolved: string;
}
export interface Project {
  id: string;
  environment: string;
  name: string;
  path: string;
  remote: string;
  branch: string;
  git_status: string;
  package_manager: string;
  requirements: Requirement[];
  venvs: { path: string; home: string; version: string; note: string }[];
  scripts: string[];
  findings: Finding[];
  fingerprint: string;
  scanned_at: string;
  manifest_files: string[];
}
export interface Step {
  id: string;
  label: string;
  program: string;
  args: string[];
  cwd: string;
  kind: string;
  risk: string;
  status: string;
  output: string;
}
export interface Plan {
  id: string;
  environment: string;
  project_id: string | null;
  title: string;
  reason: string;
  evidence: string[];
  warnings: string[];
  steps: Step[];
  created_at: string;
  status: string;
  fingerprint: string;
}
export interface Update {
  name: string;
  manager: string;
  current: string;
  candidate: string;
  latest: string;
  source: string;
  importance: string;
  vulnerabilities: { id: string; summary: string; url: string }[];
  audit_status: string;
  note: string;
  project_id: string | null;
  tool_id: string | null;
}
export interface Settings {
  roots: [string, string][];
  mcp_configs: string[];
  manager_overrides: [string, string, string][];
  same_major: boolean;
}
export interface History {
  id: string;
  at: string;
  action: string;
  target: string;
  status: string;
  detail: string;
}
export interface Snapshot {
  environments: Environment[];
  inventories: Inventory[];
  projects: Project[];
  plans: Plan[];
  history: History[];
  settings: Settings;
  updates: Update[];
  data_path: string;
  version: string;
  links?: {
    config: string;
    name: string;
    command: string;
    venv?: string;
    home?: string;
  }[];
}
