use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub home: String,
    pub os: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    pub environment: String,
    pub manager: String,
    pub name: String,
    pub version: String,
    pub path: String,
    pub active: bool,
    pub installed: bool,
    pub source: String,
    pub requested: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub scope: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Inventory {
    pub environment: String,
    pub captured_at: String,
    pub tools: Vec<Tool>,
    pub findings: Vec<Finding>,
    pub manager_paths: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    pub manager: String,
    pub name: String,
    pub requested: String,
    pub source: String,
    pub section: String,
    pub resolved: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualEnvironment {
    pub path: String,
    pub home: String,
    pub version: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub environment: String,
    pub name: String,
    pub path: String,
    pub remote: String,
    pub branch: String,
    pub git_status: String,
    pub package_manager: String,
    pub requirements: Vec<Requirement>,
    pub venvs: Vec<VirtualEnvironment>,
    pub scripts: Vec<String>,
    pub findings: Vec<Finding>,
    pub fingerprint: String,
    pub scanned_at: String,
    pub manifest_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub kind: String,
    pub risk: String,
    pub status: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub environment: String,
    pub project_id: Option<String>,
    pub title: String,
    pub reason: String,
    pub evidence: Vec<String>,
    pub warnings: Vec<String>,
    pub steps: Vec<Step>,
    pub created_at: String,
    pub status: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub id: String,
    pub summary: String,
    pub url: String,
    #[serde(default)]
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update {
    #[serde(default)]
    pub fingerprint: String,
    pub name: String,
    pub manager: String,
    pub current: String,
    pub candidate: String,
    pub latest: String,
    pub source: String,
    pub importance: String,
    pub vulnerabilities: Vec<Vulnerability>,
    pub audit_status: String,
    pub note: String,
    pub project_id: Option<String>,
    pub tool_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct History {
    pub id: String,
    pub at: String,
    pub action: String,
    pub target: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub roots: Vec<(String, String)>,
    pub mcp_configs: Vec<String>,
    pub manager_overrides: Vec<(String, String, String)>,
    pub same_major: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            roots: vec![],
            mcp_configs: vec![],
            manager_overrides: vec![],
            same_major: true,
        }
    }
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn finding(scope: &str, status: &str, message: impl Into<String>) -> Finding {
    Finding {
        scope: scope.into(),
        status: status.into(),
        message: message.into(),
    }
}
