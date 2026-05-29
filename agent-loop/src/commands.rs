//! CLI commands for agent-loop Phase 1.
//!
//! Implemented commands:
//! - init-run: Initialize a task run directory with status.json
//! - validate-discovery: Validate codebase discovery JSON schema
//! - list-runs: List actionable runs (active, blocked, cleanup-eligible)
//! - gate-check: Validate task artifacts without semantic judgment

use crate::artifacts::{
    ensure_dir, find_repo_root, list_subdirs, read_json, write_json,
    AgentRunsPaths, StatusJson, TaskStatus,
};
use crate::id::{validate_id, ContractId, IdKind, PlanId, TaskId};
use crate::schemas;
use jsonschema::Validator;
use std::path::{Path, PathBuf};

/// Result type for CLI commands.
pub type CommandResult = Result<(), CommandError>;

#[derive(Debug)]
pub enum CommandError {
    Io(std::io::Error),
    Json(serde_json::Error),
    SchemaValidation(String),
    IdValidation(String),
    InvalidInput(String),
    NotFound(String),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::Io(e) => write!(f, "IO error: {}", e),
            CommandError::Json(e) => write!(f, "JSON error: {}", e),
            CommandError::SchemaValidation(e) => write!(f, "Schema validation error: {}", e),
            CommandError::IdValidation(e) => write!(f, "ID validation error: {}", e),
            CommandError::InvalidInput(e) => write!(f, "Invalid input: {}", e),
            CommandError::NotFound(e) => write!(f, "Not found: {}", e),
        }
    }
}

impl std::error::Error for CommandError {}

impl From<std::io::Error> for CommandError {
    fn from(e: std::io::Error) -> Self {
        CommandError::Io(e)
    }
}

impl From<serde_json::Error> for CommandError {
    fn from(e: serde_json::Error) -> Self {
        CommandError::Json(e)
    }
}

// ============================================================================
// init-run command
// ============================================================================

/// Initialize a new task run directory.
pub struct InitRun {
    pub repo_root: PathBuf,
    pub plan_id: String,
    pub contract_id: String,
    pub task_id: Option<String>,
    pub sequence: Option<u16>,
}

impl InitRun {
    pub fn run(&self) -> CommandResult {
        // Validate plan_id format
        let plan_id = PlanId::parse(&self.plan_id)
            .ok_or_else(|| CommandError::IdValidation(format!("Invalid plan_id format: {}", self.plan_id)))?;

        // Validate contract_id format
        let contract_id = ContractId::parse(&self.contract_id)
            .ok_or_else(|| CommandError::IdValidation(format!("Invalid contract_id format: {}", self.contract_id)))?;

        // Generate or validate task_id
        let task_id = match &self.task_id {
            Some(id) => {
                TaskId::parse(id)
                    .ok_or_else(|| CommandError::IdValidation(format!("Invalid task_id format: {}", id)))?;
                id.clone()
            }
            None => {
                let seq = self.sequence.unwrap_or(1);
                TaskId::generate(seq).to_string()
            }
        };

        // Find repo root
        let repo_root = find_repo_root(&self.repo_root)
            .ok_or_else(|| CommandError::NotFound("Not inside a git repository".to_string()))?;

        let paths = AgentRunsPaths::new(&repo_root);
        let task_dir = paths.task_dir(&task_id);

        // Create task directory
        ensure_dir(&task_dir)?;

        // Create status.json
        let status = StatusJson::new(task_id.clone(), plan_id.to_string(), contract_id.to_string());
        let status_path = paths.task_status(&task_id);
        write_json(&status_path, &status)?;

        // Copy contract from plan to task directory
        let plan_contract = paths.contracts_dir(&plan_id.to_string()).join(format!("{}.json", contract_id));
        if plan_contract.exists() {
            let task_contract = paths.task_contract(&task_id);
            let content = std::fs::read_to_string(&plan_contract)?;
            std::fs::write(&task_contract, content)?;
        }

        println!("Initialized task run: {}", task_id);
        println!("  Status: {}", status_path.display());
        println!("  Task dir: {}", task_dir.display());

        Ok(())
    }
}

// ============================================================================
// validate-discovery command
// ============================================================================

/// Validate a codebase discovery JSON file against the schema.
pub struct ValidateDiscovery {
    pub path: PathBuf,
    pub quiet: bool,
}

impl ValidateDiscovery {
    pub fn run(&self) -> CommandResult {
        let content = std::fs::read_to_string(&self.path)?;
        let json: serde_json::Value = serde_json::from_str(&content)?;

        // Validate against schema
        let schema: serde_json::Value = serde_json::from_str(schemas::CODEBASE_DISCOVERY_SCHEMA)?;
        let validator = Validator::new(&schema)
            .map_err(|e| CommandError::SchemaValidation(e.to_string()))?;

        let mut errors: Vec<String> = Vec::new();
        for error in validator.iter_errors(&json) {
            errors.push(format!("{}: {}", error.instance_path, error));
        }

        if errors.is_empty() {
            if !self.quiet {
                println!("Discovery file is valid: {}", self.path.display());
            }
            Ok(())
        } else {
            eprintln!("Schema validation failed for {}:", self.path.display());
            for error in &errors {
                eprintln!("  - {}", error);
            }
            Err(CommandError::SchemaValidation(errors.join("; ")))
        }
    }
}

// ============================================================================
// list-runs command
// ============================================================================

/// List actionable task runs.
pub struct ListRuns {
    pub repo_root: PathBuf,
    pub all: bool,
    pub status_filter: Option<TaskStatus>,
    pub plan_filter: Option<String>,
    pub quiet: bool,
}

impl ListRuns {
    pub fn run(&self) -> CommandResult {
        let repo_root = find_repo_root(&self.repo_root)
            .ok_or_else(|| CommandError::NotFound("Not inside a git repository".to_string()))?;

        let paths = AgentRunsPaths::new(&repo_root);
        let tasks_root = paths.tasks_root();

        if !tasks_root.exists() {
            if !self.quiet {
                println!("No task runs found.");
            }
            return Ok(());
        }

        let mut runs: Vec<RunSummary> = Vec::new();

        for task_dir in list_subdirs(&tasks_root)? {
            let task_id = task_dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Validate task_id format
            if TaskId::parse(task_id).is_none() {
                continue;
            }

            let status_path = paths.task_status(task_id);
            if !status_path.exists() {
                continue;
            }

            let status: StatusJson = read_json(&status_path)?;

            // Apply filters
            if let Some(ref filter_status) = self.status_filter {
                if &status.status != filter_status {
                    continue;
                }
            }

            if let Some(ref plan_id) = self.plan_filter {
                if &status.plan_id != plan_id {
                    continue;
                }
            }

            // Default: show actionable runs (active, blocked, committed for cleanup)
            let is_actionable = self.all
                || matches!(
                    status.status,
                    TaskStatus::Active | TaskStatus::Blocked | TaskStatus::Committed
                );

            if !is_actionable {
                continue;
            }

            runs.push(RunSummary {
                task_id: status.task_id,
                plan_id: status.plan_id,
                contract_id: status.contract_id,
                status: status.status,
                blocked_reason: status.blocked_reason,
                updated_at: status.updated_at,
            });
        }

        // Sort by updated_at descending
        runs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        if runs.is_empty() {
            if !self.quiet {
                println!("No matching task runs found.");
            }
            return Ok(());
        }

        // Print runs
        for run in &runs {
            println!("{}  {}  {}", run.task_id, run.status, run.plan_id);
            if let Some(ref reason) = run.blocked_reason {
                println!("    blocked: {}", reason);
            }
        }

        if !self.quiet {
            println!("\n{} run(s) shown", runs.len());
        }

        Ok(())
    }
}

struct RunSummary {
    task_id: String,
    plan_id: String,
    contract_id: String,
    status: TaskStatus,
    blocked_reason: Option<String>,
    updated_at: String,
}

// ============================================================================
// gate-check command
// ============================================================================

/// Validate task artifacts without semantic judgment.
pub struct GateCheck {
    pub repo_root: PathBuf,
    pub task_id: Option<String>,
    pub plan_id: Option<String>,
    pub check_contracts: bool,
    pub check_status: bool,
}

impl GateCheck {
    pub fn run(&self) -> CommandResult {
        let repo_root = find_repo_root(&self.repo_root)
            .ok_or_else(|| CommandError::NotFound("Not inside a git repository".to_string()))?;

        let paths = AgentRunsPaths::new(&repo_root);
        let mut all_passed = true;

        // Check plan-level artifacts
        if let Some(ref plan_id) = self.plan_id {
            if !validate_id(plan_id, IdKind::Plan) {
                eprintln!("Invalid plan_id format: {}", plan_id);
                all_passed = false;
            }

            let plan_dir = paths.plan_dir(plan_id);
            if plan_dir.exists() {
                // Check plan.json if it exists
                let plan_json = plan_dir.join("plan.json");
                if plan_json.exists() {
                    if let Err(e) = Self::validate_json_schema(&plan_json, schemas::PLAN_SCHEMA) {
                        eprintln!("plan.json: {}", e);
                        all_passed = false;
                    } else {
                        println!("plan.json: valid");
                    }
                }

                // Check contracts
                let contracts_dir = paths.contracts_dir(plan_id);
                if contracts_dir.exists() {
                    for entry in std::fs::read_dir(&contracts_dir)? {
                        let entry = entry?;
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("json") {
                            if let Err(e) = Self::validate_json_schema(&path, schemas::CONTRACT_SCHEMA) {
                                eprintln!("{}: {}", path.file_name().unwrap().to_string_lossy(), e);
                                all_passed = false;
                            } else {
                                println!("{}: valid", path.file_name().unwrap().to_string_lossy());
                            }
                        }
                    }
                }
            } else {
                eprintln!("Plan directory not found: {}", plan_id);
                all_passed = false;
            }
        }

        // Check task-level artifacts
        if let Some(ref task_id) = self.task_id {
            if !validate_id(task_id, IdKind::Task) {
                eprintln!("Invalid task_id format: {}", task_id);
                all_passed = false;
            }

            let task_dir = paths.task_dir(task_id);
            if task_dir.exists() {
                // Check status.json
                let status_path = paths.task_status(task_id);
                if status_path.exists() {
                    if let Err(e) = Self::validate_json_schema(&status_path, schemas::STATUS_SCHEMA) {
                        eprintln!("status.json: {}", e);
                        all_passed = false;
                    } else {
                        // Additional validation: blocked must have reason and details
                        let status: StatusJson = read_json(&status_path)?;
                        if let Err(e) = status.validate_blocked() {
                            eprintln!("status.json: {}", e);
                            all_passed = false;
                        } else {
                            println!("status.json: valid");
                        }
                    }
                }

                // Check machine_gate.json if exists
                let gate_path = paths.machine_gate(task_id);
                if gate_path.exists() {
                    if let Err(e) = Self::validate_json_schema(&gate_path, schemas::MACHINE_GATE_SCHEMA) {
                        eprintln!("machine_gate.json: {}", e);
                        all_passed = false;
                    } else {
                        println!("machine_gate.json: valid");
                    }
                }
            } else {
                eprintln!("Task directory not found: {}", task_id);
                all_passed = false;
            }
        }

        if all_passed {
            println!("All checks passed.");
            Ok(())
        } else {
            Err(CommandError::InvalidInput("One or more checks failed".to_string()))
        }
    }

    fn validate_json_schema(path: &Path, schema_str: &str) -> Result<(), CommandError> {
        let content = std::fs::read_to_string(path)?;
        let json: serde_json::Value = serde_json::from_str(&content)?;
        let schema: serde_json::Value = serde_json::from_str(schema_str)?;
        let validator = Validator::new(&schema)
            .map_err(|e| CommandError::SchemaValidation(e.to_string()))?;

        let errors: Vec<String> = validator
            .iter_errors(&json)
            .map(|e| format!("{}: {}", e.instance_path, e))
            .collect();

        if errors.is_empty() {
            Ok(())
        } else {
            Err(CommandError::SchemaValidation(errors.join("; ")))
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo() -> TempDir {
        let temp = TempDir::new().unwrap();
        let git_dir = temp.path().join(".git");
        std::fs::create_dir(&git_dir).unwrap();
        std::fs::write(git_dir.join("config"), "[core]\n").unwrap();
        temp
    }

    #[test]
    fn test_init_run_creates_directories() {
        let repo = create_test_repo();
        let cmd = InitRun {
            repo_root: repo.path().to_path_buf(),
            plan_id: "plan-20260529-001".to_string(),
            contract_id: "contract-001".to_string(),
            task_id: None,
            sequence: Some(1),
        };

        let result = cmd.run();
        assert!(result.is_ok());

        let paths = AgentRunsPaths::new(repo.path());
        assert!(paths.task_status("task-20260529-001").exists());
    }

    #[test]
    fn test_init_run_with_custom_task_id() {
        let repo = create_test_repo();
        let cmd = InitRun {
            repo_root: repo.path().to_path_buf(),
            plan_id: "plan-20260529-001".to_string(),
            contract_id: "contract-001".to_string(),
            task_id: Some("task-20260529-042".to_string()),
            sequence: None,
        };

        let result = cmd.run();
        assert!(result.is_ok());

        let paths = AgentRunsPaths::new(repo.path());
        assert!(paths.task_status("task-20260529-042").exists());
    }

    #[test]
    fn test_init_run_invalid_plan_id() {
        let repo = create_test_repo();
        let cmd = InitRun {
            repo_root: repo.path().to_path_buf(),
            plan_id: "invalid-plan".to_string(),
            contract_id: "contract-001".to_string(),
            task_id: None,
            sequence: Some(1),
        };

        let result = cmd.run();
        assert!(result.is_err());
    }

    #[test]
    fn test_init_run_invalid_contract_id() {
        let repo = create_test_repo();
        let cmd = InitRun {
            repo_root: repo.path().to_path_buf(),
            plan_id: "plan-20260529-001".to_string(),
            contract_id: "invalid-contract".to_string(),
            task_id: None,
            sequence: Some(1),
        };

        let result = cmd.run();
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_discovery_valid() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("discovery.json");

        let discovery = serde_json::json!({
            "schema_version": "codebase-discovery-v1",
            "repo_facts": {
                "root_path": "/test",
                "languages": ["rust"],
                "package_managers": ["cargo"],
                "frameworks": [],
                "has_tests": true
            },
            "available_scripts": {},
            "relevant_files": [],
            "relevant_tests": [],
            "verification_candidates": [],
            "scope_candidates": [],
            "risk_hints": [],
            "unknowns": [],
            "discovery_limits": []
        });

        write_json(&path, &discovery).unwrap();

        let cmd = ValidateDiscovery {
            path,
            quiet: true,
        };

        assert!(cmd.run().is_ok());
    }

    #[test]
    fn test_validate_discovery_invalid_missing_field() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("discovery.json");

        let discovery = serde_json::json!({
            "schema_version": "codebase-discovery-v1",
            "repo_facts": {
                "root_path": "/test",
                "languages": [],
                "package_managers": [],
                "frameworks": [],
                "has_tests": false
            }
        });

        write_json(&path, &discovery).unwrap();

        let cmd = ValidateDiscovery {
            path,
            quiet: true,
        };

        assert!(cmd.run().is_err());
    }

    #[test]
    fn test_list_runs_empty() {
        let repo = create_test_repo();
        let cmd = ListRuns {
            repo_root: repo.path().to_path_buf(),
            all: false,
            status_filter: None,
            plan_filter: None,
            quiet: false,
        };

        assert!(cmd.run().is_ok());
    }

    #[test]
    fn test_status_validation_blocked_requires_reason() {
        let mut status = StatusJson::new(
            "task-20260529-001".to_string(),
            "plan-20260529-001".to_string(),
            "contract-001".to_string(),
        );

        status.set_blocked("test_reason".to_string(), "test_details".to_string());
        assert!(status.validate_blocked().is_ok());

        let mut bad_status = StatusJson::new(
            "task-20260529-001".to_string(),
            "plan-20260529-001".to_string(),
            "contract-001".to_string(),
        );
        bad_status.status = TaskStatus::Blocked;
        assert!(bad_status.validate_blocked().is_err());
    }

    #[test]
    fn test_status_transitions() {
        let mut status = StatusJson::new(
            "task-20260529-001".to_string(),
            "plan-20260529-001".to_string(),
            "contract-001".to_string(),
        );

        assert_eq!(status.status, TaskStatus::Active);

        status.set_blocked("reason".to_string(), "details".to_string());
        assert_eq!(status.status, TaskStatus::Blocked);
        assert!(status.blocked_reason.is_some());
        assert!(status.details.is_some());

        status.set_committed();
        assert_eq!(status.status, TaskStatus::Committed);
        assert!(status.blocked_reason.is_none());

        status.set_abandoned();
        assert_eq!(status.status, TaskStatus::Abandoned);
    }

    #[test]
    fn test_gate_check_valid_task() {
        let repo = create_test_repo();
        let paths = AgentRunsPaths::new(repo.path());

        let task_id = "task-20260529-001";
        ensure_dir(&paths.task_dir(task_id)).unwrap();

        let status = StatusJson::new(
            task_id.to_string(),
            "plan-20260529-001".to_string(),
            "contract-001".to_string(),
        );
        write_json(&paths.task_status(task_id), &status).unwrap();

        let cmd = GateCheck {
            repo_root: repo.path().to_path_buf(),
            task_id: Some(task_id.to_string()),
            plan_id: None,
            check_contracts: false,
            check_status: true,
        };

        assert!(cmd.run().is_ok());
    }

    #[test]
    fn test_gate_check_blocked_without_reason_fails() {
        let repo = create_test_repo();
        let paths = AgentRunsPaths::new(repo.path());

        let task_id = "task-20260529-001";
        ensure_dir(&paths.task_dir(task_id)).unwrap();

        // Create a blocked status without reason
        let mut status = StatusJson::new(
            task_id.to_string(),
            "plan-20260529-001".to_string(),
            "contract-001".to_string(),
        );
        status.status = TaskStatus::Blocked;
        // Missing blocked_reason and details
        write_json(&paths.task_status(task_id), &status).unwrap();

        let cmd = GateCheck {
            repo_root: repo.path().to_path_buf(),
            task_id: Some(task_id.to_string()),
            plan_id: None,
            check_contracts: false,
            check_status: true,
        };

        assert!(cmd.run().is_err());
    }
}
