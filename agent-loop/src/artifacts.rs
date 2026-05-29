//! Artifact directory management per ADR-001 conventions.
//!
//! Directory structure:
//! - `.agent-runs/plans/{plan_id}/`      for plan artifacts
//! - `.agent-runs/plans/{plan_id}/discovery/` for codebase discovery
//! - `.agent-runs/plans/{plan_id}/contracts/` for contracts
//! - `.agent-runs/tasks/{task_id}/`      for task run artifacts
//! - `.worktrees/{task_id}/`             for isolated worktrees

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Base directories for agent-loop artifacts.
#[derive(Debug, Clone)]
pub struct AgentRunsPaths {
    root: PathBuf,
}

impl AgentRunsPaths {
    /// Create from a repo root (detected or given).
    pub fn new(repo_root: &Path) -> Self {
        Self {
            root: repo_root.join(".agent-runs"),
        }
    }

    /// Plans directory: .agent-runs/plans/
    pub fn plans_root(&self) -> PathBuf {
        self.root.join("plans")
    }

    /// Single plan directory: .agent-runs/plans/{plan_id}/
    pub fn plan_dir(&self, plan_id: &str) -> PathBuf {
        self.plans_root().join(plan_id)
    }

    /// Discovery directory: .agent-runs/plans/{plan_id}/discovery/
    pub fn discovery_dir(&self, plan_id: &str) -> PathBuf {
        self.plan_dir(plan_id).join("discovery")
    }

    /// Contracts directory: .agent-runs/plans/{plan_id}/contracts/
    pub fn contracts_dir(&self, plan_id: &str) -> PathBuf {
        self.plan_dir(plan_id).join("contracts")
    }

    /// Tasks directory: .agent-runs/tasks/
    pub fn tasks_root(&self) -> PathBuf {
        self.root.join("tasks")
    }

    /// Single task directory: .agent-runs/tasks/{task_id}/
    pub fn task_dir(&self, task_id: &str) -> PathBuf {
        self.tasks_root().join(task_id)
    }

    /// Status file for a task: .agent-runs/tasks/{task_id}/status.json
    pub fn task_status(&self, task_id: &str) -> PathBuf {
        self.task_dir(task_id).join("status.json")
    }

    /// Contract copy for a task: .agent-runs/tasks/{task_id}/contract.json
    pub fn task_contract(&self, task_id: &str) -> PathBuf {
        self.task_dir(task_id).join("contract.json")
    }

    /// Normalized evidence directory: .agent-runs/tasks/{task_id}/normalized/
    pub fn normalized_dir(&self, task_id: &str) -> PathBuf {
        self.task_dir(task_id).join("normalized")
    }

    /// Backend output directory: .agent-runs/tasks/{task_id}/backend-output/
    pub fn backend_output_dir(&self, task_id: &str) -> PathBuf {
        self.task_dir(task_id).join("backend-output")
    }

    /// Machine gate file: .agent-runs/tasks/{task_id}/machine_gate.json
    pub fn machine_gate(&self, task_id: &str) -> PathBuf {
        self.task_dir(task_id).join("machine_gate.json")
    }

    /// Sonnet review file: .agent-runs/tasks/{task_id}/sonnet_review.json
    pub fn sonnet_review(&self, task_id: &str) -> PathBuf {
        self.task_dir(task_id).join("sonnet_review.json")
    }

    /// Opus final gate file: .agent-runs/tasks/{task_id}/opus_final_gate.json
    pub fn opus_final_gate(&self, task_id: &str) -> PathBuf {
        self.task_dir(task_id).join("opus_final_gate.json")
    }
}

/// Worktree directories.
#[derive(Debug, Clone)]
pub struct WorktreePaths {
    root: PathBuf,
}

impl WorktreePaths {
    /// Create from a repo root.
    pub fn new(repo_root: &Path) -> Self {
        Self {
            root: repo_root.join(".worktrees"),
        }
    }

    /// Worktrees root: .worktrees/
    pub fn root(&self) -> PathBuf {
        self.root.clone()
    }

    /// Single worktree: .worktrees/{task_id}/
    pub fn worktree(&self, task_id: &str) -> PathBuf {
        self.root.join(task_id)
    }
}

/// Task status values per ADR-001.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Active,
    Committed,
    Blocked,
    Abandoned,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Active => write!(f, "active"),
            TaskStatus::Committed => write!(f, "committed"),
            TaskStatus::Blocked => write!(f, "blocked"),
            TaskStatus::Abandoned => write!(f, "abandoned"),
        }
    }
}

/// Status.json structure per ADR-001.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusJson {
    #[serde(rename = "schema_version")]
    pub schema_version: String,
    #[serde(rename = "task_id")]
    pub task_id: String,
    #[serde(rename = "plan_id")]
    pub plan_id: String,
    #[serde(rename = "contract_id")]
    pub contract_id: String,
    pub status: TaskStatus,
    #[serde(rename = "blocked_reason", skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    #[serde(rename = "details", skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "updated_at")]
    pub updated_at: String,
}

impl StatusJson {
    /// Create a new active status.
    pub fn new(task_id: String, plan_id: String, contract_id: String) -> Self {
        let now = chrono::Local::now().to_rfc3339();
        Self {
            schema_version: "status-v1".to_string(),
            task_id,
            plan_id,
            contract_id,
            status: TaskStatus::Active,
            blocked_reason: None,
            details: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Validate that blocked status has required fields.
    pub fn validate_blocked(&self) -> Result<(), String> {
        if self.status == TaskStatus::Blocked {
            if self.blocked_reason.is_none() {
                return Err("blocked_reason required when status is blocked".to_string());
            }
            if self.details.is_none() {
                return Err("details required when status is blocked".to_string());
            }
        }
        Ok(())
    }

    /// Transition to blocked status with reason.
    pub fn set_blocked(&mut self, blocked_reason: String, details: String) {
        self.status = TaskStatus::Blocked;
        self.blocked_reason = Some(blocked_reason);
        self.details = Some(details);
        self.updated_at = chrono::Local::now().to_rfc3339();
    }

    /// Transition to committed status.
    pub fn set_committed(&mut self) {
        self.status = TaskStatus::Committed;
        self.blocked_reason = None;
        self.details = None;
        self.updated_at = chrono::Local::now().to_rfc3339();
    }

    /// Transition to abandoned status.
    pub fn set_abandoned(&mut self) {
        self.status = TaskStatus::Abandoned;
        self.blocked_reason = None;
        self.details = None;
        self.updated_at = chrono::Local::now().to_rfc3339();
    }

    /// Transition to active status.
    pub fn set_active(&mut self) {
        self.status = TaskStatus::Active;
        self.blocked_reason = None;
        self.details = None;
        self.updated_at = chrono::Local::now().to_rfc3339();
    }
}

/// Ensure a directory exists, creating it if needed.
pub fn ensure_dir(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)
}

/// Read and parse a JSON file.
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> std::io::Result<T> {
    let content = fs::read_to_string(path)?;
    serde_json::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Write a JSON file with pretty formatting.
pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let content = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, content)
}

/// Check if a path exists.
pub fn exists(path: &Path) -> bool {
    path.exists()
}

/// List all subdirectories in a directory matching a pattern.
pub fn list_subdirs(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let entries = fs::read_dir(path)?;
    let mut dirs = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            dirs.push(entry.path());
        }
    }
    dirs.sort();
    Ok(dirs)
}

/// Check if directory is a git repo root.
pub fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

/// Find the git repo root from a starting path.
pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if is_git_repo(&current) {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}
