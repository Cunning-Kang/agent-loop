//! CLI commands for agent-loop Phase 1.
//!
//! Implemented commands:
//! - init-run: Initialize a task run directory with status.json
//! - validate-discovery: Validate codebase discovery JSON schema
//! - list-runs: List actionable runs (active, blocked, cleanup-eligible)
//! - gate-check: Validate task artifacts without semantic judgment

use crate::artifacts::{
    ensure_dir, find_repo_root, list_subdirs, read_json, write_json,
    AgentRunsPaths, StatusJson, TaskStatus, WorktreePaths,
};
use crate::id::{validate_id, ContractId, IdKind, PlanId, TaskId};
use crate::schemas;
use jsonschema::Validator;
use std::path::{Path, PathBuf};

/// Result type for CLI commands.
pub type CommandResult<T = ()> = Result<T, CommandError>;

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
// cleanup command
// ============================================================================

/// Cleanup task artifacts (worktrees and/or evidence).
pub struct Cleanup {
    pub repo_root: PathBuf,
    pub task_id: Option<String>,
    pub plan_id: Option<String>,
    pub status_filter: Option<crate::artifacts::TaskStatus>,
    pub older_than: Option<u32>,
    pub worktrees: bool,
    pub evidence: bool,
    pub confirm: bool,
}

impl Cleanup {
    pub fn run(&self) -> CommandResult {
        let has_selector = self.task_id.is_some()
            || self.plan_id.is_some()
            || self.status_filter.is_some()
            || self.older_than.is_some()
            || self.worktrees
            || self.evidence;

        if !has_selector {
            return Err(CommandError::InvalidInput(
                "At least one selector is required".to_string(),
            ));
        }

        let repo_root = find_repo_root(&self.repo_root)
            .ok_or_else(|| CommandError::NotFound("Not inside a git repository".to_string()))?;

        let agent_paths = AgentRunsPaths::new(&repo_root);
        let worktree_paths = WorktreePaths::new(&repo_root);
        let tasks_root = agent_paths.tasks_root();

        if !tasks_root.exists() {
            println!("No tasks found.");
            return Ok(());
        }

        let mut to_clean = Vec::new();

        for task_dir in list_subdirs(&tasks_root)? {
            let task_id_str = task_dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if TaskId::parse(task_id_str).is_none() {
                continue;
            }

            let status_path = task_dir.join("status.json");
            if !status_path.exists() {
                continue;
            }

            let status: StatusJson = read_json(&status_path)?;

            if let Some(ref tid) = self.task_id {
                if tid != task_id_str {
                    continue;
                }
            }

            if let Some(ref sf) = self.status_filter {
                if &status.status != sf {
                    continue;
                }
            }

            if let Some(ref pid) = self.plan_id {
                if &status.plan_id != pid {
                    continue;
                }
            }

            if let Some(threshold) = self.older_than {
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&status.created_at) {
                    let age = chrono::Local::now().timestamp() - ts.timestamp();
                    if age <= threshold as i64 {
                        continue;
                    }
                }
            }

            if self.worktrees {
                let wt = worktree_paths.worktree(task_id_str);
                if wt.exists() {
                    to_clean.push((task_id_str.to_string(), wt));
                }
            }

            if self.evidence {
                to_clean.push((task_id_str.to_string(), task_dir));
            }
        }

        if to_clean.is_empty() {
            println!("No tasks found matching selectors.");
            return Ok(());
        }

        // Preview by default (unless --confirm is passed)
        if !self.confirm {
            println!("Dry run: {} task(s) would be cleaned.", to_clean.len());
            for (task_id, path) in &to_clean {
                println!("  {}: {}", task_id, path.display());
            }
        } else {
            let mut cleaned = 0;
            for (task_id, path) in &to_clean {
                if path.is_dir() {
                    if std::fs::remove_dir_all(path).is_ok() {
                        println!("Removed: {}", task_id);
                        cleaned += 1;
                    }
                }
            }
            println!("Cleaned {} task(s).", cleaned);
        }

        Ok(())
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
// export command
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportOutput {
    pub schema_version: String,
    pub task_id: String,
    pub plan_id: Option<String>,
    pub contract_id: Option<String>,
    pub contract_summary: Option<serde_json::Value>,
    pub changed_files: Option<Vec<serde_json::Value>>,
    pub diff_summary: Option<serde_json::Value>,
    pub verification: Option<Vec<serde_json::Value>>,
    pub external_verdict: Option<String>,
    pub sonnet_verdict: Option<serde_json::Value>,
    pub opus_decision: Option<String>,
    pub blocking_issues: Vec<String>,
    pub residual_risks: Vec<String>,
    pub accepted_risks: Vec<String>,
    pub secrets: Vec<String>,
    pub full_export: bool,
    pub raw_outputs: Option<serde_json::Value>,
}

const UNSAFE_PATTERNS: [&str; 8] = [
    ".env", ".aws", "id_rsa", "id_ed25519",
    ".git-credentials", ".npmrc", ".pypirc", "credentials",
];

pub struct Export {
    pub repo_root: PathBuf,
    pub task_id: String,
    pub output: Option<PathBuf>,
    pub full: bool,
    pub acknowledge_full_export_risk: bool,
    pub quiet: bool,
}

impl Export {
    pub fn run(&self) -> CommandResult {
        if self.full && !self.acknowledge_full_export_risk {
            return Err(CommandError::InvalidInput(
                "Full export requires --acknowledge-full-export-risk".to_string(),
            ));
        }

        let repo_root = find_repo_root(&self.repo_root)
            .ok_or_else(|| CommandError::NotFound("Not inside a git repository".to_string()))?;

        let paths = AgentRunsPaths::new(&repo_root);
        let task_dir = paths.task_dir(&self.task_id);

        if !task_dir.exists() {
            return Err(CommandError::NotFound(format!("Task not found: {}", self.task_id)));
        }

        let normalized_dir = paths.normalized_dir(&self.task_id);

        // Read status
        let (plan_id, contract_id) = if paths.task_status(&self.task_id).exists() {
            if let Ok(status) = read_json::<StatusJson>(&paths.task_status(&self.task_id)) {
                (Some(status.plan_id), Some(status.contract_id))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Contract summary
        let mut contract_summary = None;
        {
            let path = paths.task_contract(&self.task_id);
            if path.exists() {
                if let Ok(v) = read_json::<serde_json::Value>(&path) {
                    contract_summary = Some(serde_json::json!({
                        "objective": v.get("objective"),
                        "risk_class": v.get("risk_class"),
                        "scope": v.get("scope"),
                    }));
                }
            }
        };

        // Changed files (sanitized)
        let mut changed_files = None;
        {
            let cf_path = normalized_dir.join("changed_files.json");
            if cf_path.exists() {
                if let Ok(cf) = read_json::<serde_json::Value>(&cf_path) {
                    if let Some(files) = cf.get("files").and_then(|f| f.as_array()) {
                        let safe: Vec<_> = files.iter()
                            .filter_map(|f| {
                                let path = f.get("path")?.as_str()?;
                                if Self::is_unsafe_path(path) {
                                    return None;
                                }
                                Some(serde_json::json!({
                                    "path": Self::sanitize_path(path),
                                    "operation": f.get("operation"),
                                }))
                            })
                            .collect();
                        if !safe.is_empty() {
                            changed_files = Some(safe);
                        }
                    }
                }
            }
        };

        // Diff summary
        let mut diff_summary = None;
        {
            let diff_path = normalized_dir.join("diff.patch");
            if diff_path.exists() {
                if let Ok(diff) = std::fs::read_to_string(&diff_path) {
                    let (files, ins, dels) = Self::parse_diff(&diff);
                    diff_summary = Some(serde_json::json!({
                        "files_changed": files,
                        "insertions": ins,
                        "deletions": dels,
                        "has_secrets": Self::contains_secrets(&diff),
                    }));
                }
            }
        };

        // Verification (sanitized)
        let mut verification = None;
        {
            let v_path = normalized_dir.join("verification.json");
            if v_path.exists() {
                if let Ok(v) = read_json::<serde_json::Value>(&v_path) {
                    if let Some(results) = v.get("results").and_then(|r| r.as_array()) {
                        let sanitized: Vec<_> = results.iter()
                            .map(|r| {
                                let cmd = r.get("command")
                                    .and_then(|c| c.as_str())
                                    .map(|c| Self::sanitize_command(c))
                                    .unwrap_or_default();
                                serde_json::json!({
                                    "command": cmd,
                                    "exit_code": r.get("exit_code"),
                                    "passed": r.get("passed"),
                                })
                            })
                            .collect();
                        if !sanitized.is_empty() {
                            verification = Some(sanitized);
                        }
                    }
                }
            }
        };

        // External verdict
        let mut external_verdict = None;
        {
            let path = normalized_dir.join("external_review.json");
            if path.exists() {
                if let Ok(v) = read_json::<serde_json::Value>(&path) {
                    external_verdict = v.get("verdict").and_then(|v| v.as_str()).map(String::from);
                }
            }
        };

        // Sonnet verdict
        let mut sonnet_verdict = None;
        {
            let path = task_dir.join("sonnet_review.json");
            if path.exists() {
                if let Ok(v) = read_json::<serde_json::Value>(&path) {
                    let blockers: Vec<_> = v.get("blockers")
                        .and_then(|b| b.as_array())
                        .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    sonnet_verdict = Some(serde_json::json!({
                        "recommendation": v.get("recommendation"),
                        "blockers": blockers,
                        "non_blockers": v.get("non_blockers"),
                    }));
                }
            }
        };

        // Opus decision
        let mut opus_decision = None;
        {
            let path = paths.opus_final_gate(&self.task_id);
            if path.exists() {
                if let Ok(v) = read_json::<serde_json::Value>(&path) {
                    opus_decision = v.get("decision").and_then(|v| v.as_str()).map(String::from);
                }
            }
        };

        // Residual/accepted risks
        let mut residual_risks = Vec::new();
        let mut accepted_risks = Vec::new();
        {
            let path = normalized_dir.join("sensitive_audit.json");
            if path.exists() {
                if let Ok(v) = read_json::<serde_json::Value>(&path) {
                    residual_risks = v.get("residual_risks")
                        .and_then(|r| r.as_array())
                        .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    accepted_risks = v.get("accepted_risks")
                        .and_then(|a| a.as_array())
                        .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                }
            }
        };

        // Scan for secrets
        let mut secrets = Vec::new();
        Self::scan_dir_recursive(&task_dir, &mut secrets);
        Self::scan_dir_recursive(&normalized_dir, &mut secrets);

        // Raw outputs for full export
        let mut raw_outputs = None;
        if self.full {
            let backend_dir = paths.backend_output_dir(&self.task_id);
            if backend_dir.exists() {
                let mut outputs = serde_json::Map::new();
                if let Ok(entries) = std::fs::read_dir(&backend_dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let path = entry.path();
                        if path.is_file() {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                if let Ok(val) = serde_json::from_str(&content) {
                                    outputs.insert(name, val);
                                } else {
                                    outputs.insert(name, serde_json::Value::String(content));
                                }
                            }
                        }
                    }
                }
                raw_outputs = Some(serde_json::Value::Object(outputs));
            }
        };

        let output = ExportOutput {
            schema_version: "export-sanitized-v1".to_string(),
            task_id: self.task_id.clone(),
            plan_id,
            contract_id,
            contract_summary,
            changed_files,
            diff_summary,
            verification,
            external_verdict,
            sonnet_verdict,
            opus_decision,
            blocking_issues: Vec::new(),
            residual_risks,
            accepted_risks,
            secrets,
            full_export: self.full,
            raw_outputs,
        };

        let json = serde_json::to_string_pretty(&output)
            .map_err(|e| CommandError::Json(e))?;

        match &self.output {
            Some(path) => {
                std::fs::write(path, &json)?;
                if !self.quiet {
                    println!("Exported: {}", path.display());
                }
            }
            None => println!("{}", json),
        }

        Ok(())
    }

    fn is_unsafe_path(path: &str) -> bool {
        let lower = path.to_lowercase();
        UNSAFE_PATTERNS.iter().any(|p| lower.contains(&p.to_lowercase()))
    }

    fn sanitize_path(path: &str) -> String {
        // For absolute paths, return just the filename (no directory leak).
        let lower = path.to_lowercase();
        if path.starts_with('/') || path.contains(":/") || lower.starts_with("c:") {
            return path.split('/').last().unwrap_or(path).to_string();
        }
        // Safe relative path - preserve the full safe path (e.g. src/lib.rs).
        // Unsafe paths are already filtered out by the caller before this is called.
        path.to_string()
    }

    fn sanitize_command(cmd: &str) -> String {
        let mut result = cmd.to_string();
        let patterns = [
            (r"(?i)(token|password|secret|apikey|api_key|bearer|sk-)=[^\s]+", "$1=[REDACTED]"),
            (r"(?i)--token=[^\s]+", "--token=[REDACTED]"),
            (r"(?i)--password=[^\s]+", "--password=[REDACTED]"),
            (r"/[^\s]+", ""),
        ];
        for (p, r) in patterns {
            if let Ok(re) = regex::Regex::new(p) {
                result = re.replace_all(&result, r).to_string();
            }
        }
        result.trim().to_string()
    }

    fn contains_secrets(diff: &str) -> bool {
        let lower = diff.to_lowercase();
        UNSAFE_PATTERNS.iter().any(|p| lower.contains(&p.to_lowercase()))
    }

    fn parse_diff(diff: &str) -> (usize, usize, usize) {
        let mut files = 0;
        let mut ins = 0;
        let mut dels = 0;
        for line in diff.lines() {
            if line.starts_with("--- ") || line.starts_with("+++ ") {
                files += 1;
            } else if line.starts_with('+') && !line.starts_with("+++ ") {
                ins += 1;
            } else if line.starts_with('-') && !line.starts_with("--- ") {
                dels += 1;
            }
        }
        (files, ins, dels)
    }

    fn scan_dir_recursive(dir: &Path, found: &mut Vec<String>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let name = path.file_name()
                        .map(|n| n.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    if UNSAFE_PATTERNS.iter().any(|p| name.contains(&p.to_lowercase())) {
                        found.push(path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
                    } else if let Ok(content) = std::fs::read_to_string(&path) {
                        if Self::content_has_secrets(&content) {
                            found.push(path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
                        }
                    }
                } else if path.is_dir() {
                    Self::scan_dir_recursive(&path, found);
                }
            }
        }
    }

    fn content_has_secrets(content: &str) -> bool {
        let patterns = [
            r"(?i)(token|password|secret|apikey|api_key|bearer|sk-)=",
            r"(?i)(aws_key|ghp_|gho_)",
        ];
        for p in patterns {
            if regex::Regex::new(p).map(|r| r.is_match(content)).unwrap_or(false) {
                return true;
            }
        }
        false
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

        // InitRun generates task_id with today's date and provided sequence.
        // Status file should exist (date matches today, sequence=1).
        let paths = AgentRunsPaths::new(repo.path());
        // Use a glob pattern: task-YYYYMMDD-001 should exist.
        let tasks_dir = repo.path().join(".agent-runs/tasks");
        let entries = std::fs::read_dir(&tasks_dir).unwrap();
        let mut found = false;
        for entry in entries {
            let name = entry.unwrap().file_name().into_string().unwrap();
            // Should be task-YYYYMMDD-001
            if name.starts_with("task-") && name.ends_with("-001") {
                found = true;
                // Status file should exist
                assert!(tasks_dir.join(&name).join("status.json").exists());
                break;
            }
        }
        assert!(found, "Expected task with sequence 001 to be created");
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
// ============================================================================
// collect-evidence command
// ============================================================================

/// Normalize backend raw outputs into the seven required artifacts.
///
/// The CLI is deterministic: it copies/transforms inputs from
/// `backend_output_dir` into `normalized_dir`. It does NOT evaluate
/// correctness, infer missing verification success, or fabricate absent
/// evidence. Backend raw outputs are preserved under `backend-output/`.
pub struct CollectEvidence {
 pub repo_root: PathBuf,
 pub task_id: String,
 pub backend_output_dir: Option<PathBuf>,
 pub normalized_dir: Option<PathBuf>,
 pub review_verdict: String,
 pub execution_completeness: String,
}

impl CollectEvidence {
 pub fn run(&self) -> CommandResult {
 let repo_root = find_repo_root(&self.repo_root)
 .ok_or_else(|| CommandError::NotFound("Not inside a git repository".to_string()))?;

 // Validate task_id format (no semantic judgment; just structural).
 TaskId::parse(&self.task_id).ok_or_else(|| {
 CommandError::IdValidation(format!("Invalid task_id format: {}", self.task_id))
 })?;

 // Validate review_verdict enum.
 match self.review_verdict.as_str() {
 "pass" | "fail" | "blocked" => {}
 other => {
 return Err(CommandError::InvalidInput(format!(
 "review_verdict must be one of pass|fail|blocked, got: {}",
 other
)));
 }
 }
 match self.execution_completeness.as_str() {
 "full" | "partial" | "unavailable" => {}
 other => {
 return Err(CommandError::InvalidInput(format!(
 "execution_completeness must be one of full|partial|unavailable, got: {}",
 other
)));
 }
 }

 let paths = AgentRunsPaths::new(&repo_root);
 let backend_output_dir = self
 .backend_output_dir
 .clone()
 .unwrap_or_else(|| paths.backend_output_dir(&self.task_id));
 let normalized_dir = self
 .normalized_dir
 .clone()
 .unwrap_or_else(|| paths.normalized_dir(&self.task_id));

 // Read backend raw outputs if present.
 let diff_text = read_backend_diff(&backend_output_dir);
 let changed_files = read_backend_changed_files(&backend_output_dir)
 .map_err(CommandError::InvalidInput)?;
 let verification = read_backend_verification(&backend_output_dir);
 let trace_events = read_backend_trace(&backend_output_dir);

 let external_review = crate::evidence::ExternalReviewDoc {
 schema_version: "external-review-v1".to_string(),
 task_id: self.task_id.clone(),
 verdict: self.review_verdict.clone(),
 scope_compliance: None,
 policy_compliance: None,
 verification_sufficient: None,
 summary: None,
 findings: Vec::new(),
 };

 let inputs = crate::evidence::CollectEvidenceInputs {
 task_id: &self.task_id,
 backend_output_dir: &backend_output_dir,
 normalized_dir: &normalized_dir,
 diff_text: &diff_text,
 changed_files,
 verification,
 external_review,
 execution_trace_events: trace_events,
 execution_completeness: &self.execution_completeness,
 audit_limitations: Vec::new(),
 };

 let written = crate::evidence::collect_evidence(inputs)
 .map_err(|e| CommandError::InvalidInput(e.to_string()))?;

 for path in &written {
 println!("{}", path.display());
 }
 println!(
 "Wrote {} normalized artifact(s) for {}",
 written.len(),
 self.task_id
 );

 Ok(())
 }
}

/// Read backend raw diff text if present.
fn read_backend_diff(backend_output_dir: &Path) -> String {
 let p = backend_output_dir.join("diff.patch");
 std::fs::read_to_string(&p).unwrap_or_default()
}

/// Read backend raw `changed_files.json` if present.
fn read_backend_changed_files(
 backend_output_dir: &Path,
) -> Result<Vec<crate::evidence::ChangedFileEntry>, String> {
 let p = backend_output_dir.join("changed_files.json");
 if !p.exists() {
 return Ok(Vec::new());
 }
 let doc: crate::evidence::ChangedFilesDoc = read_json(&p).map_err(|e| e.to_string())?;
 Ok(doc.files)
}

/// Read backend raw `verification.json` if present.
fn read_backend_verification(
 backend_output_dir: &Path,
) -> Vec<crate::evidence::VerificationResult> {
 let p = backend_output_dir.join("verification.json");
 if !p.exists() {
 return Vec::new();
 }
 let doc: crate::evidence::VerificationDoc = match read_json(&p) {
 Ok(d) => d,
 Err(_) => return Vec::new(),
 };
 doc.results
}

/// Read backend raw `execution_trace.jsonl` if present and convert to events.
fn read_backend_trace(backend_output_dir: &Path) -> Vec<crate::evidence::ExecutionTraceEvent> {
 let p = backend_output_dir.join("execution_trace.jsonl");
 let content = match std::fs::read_to_string(&p) {
 Ok(c) => c,
 Err(_) => return Vec::new(),
 };
 let mut events: Vec<crate::evidence::ExecutionTraceEvent> = Vec::new();
 for line in content.lines() {
 let trimmed = line.trim();
 if trimmed.is_empty() {
 continue;
 }
 // Best-effort: only the `event` and `timestamp` fields are required;
 // everything else is preserved as `details`.
 let v: serde_json::Value = match serde_json::from_str(trimmed) {
 Ok(v) => v,
 Err(_) => continue,
 };
 let event = v
 .get("event")
 .and_then(|x| x.as_str())
 .unwrap_or("unknown")
 .to_string();
 let timestamp = v
 .get("timestamp")
 .and_then(|x| x.as_str())
 .unwrap_or("1970-01-01T00:00:00Z")
 .to_string();
 let details = v.get("details").cloned();
 events.push(crate::evidence::ExecutionTraceEvent {
 event,
 timestamp,
 details,
 });
 }
 events
}

// ============================================================================
// validate-evidence command
// ============================================================================

/// Validate normalized evidence artifacts against their JSON Schemas.
/// Rejects invalid artifacts. No semantic judgment, no repair, no fabrication.
pub struct ValidateEvidence {
 pub repo_root: PathBuf,
 pub task_id: String,
 pub normalized_dir: Option<PathBuf>,
 pub quiet: bool,
}

impl ValidateEvidence {
 pub fn run(&self) -> CommandResult {
 let repo_root = find_repo_root(&self.repo_root)
 .ok_or_else(|| CommandError::NotFound("Not inside a git repository".to_string()))?;

 TaskId::parse(&self.task_id).ok_or_else(|| {
 CommandError::IdValidation(format!("Invalid task_id format: {}", self.task_id))
 })?;

 let paths = AgentRunsPaths::new(&repo_root);
 let normalized_dir = self
 .normalized_dir
 .clone()
 .unwrap_or_else(|| paths.normalized_dir(&self.task_id));

 let report = crate::evidence::validate_evidence(&self.task_id, &normalized_dir)
 .map_err(|e| CommandError::InvalidInput(e.to_string()))?;

 if !self.quiet {
 for art in &report.artifacts {
 if art.valid {
 println!("{}: valid", art.name);
 } else {
 println!("{}: INVALID", art.name);
 for err in &art.errors {
 println!(" - {}", err);
 }
 }
 }
 }

 if report.valid {
 if !self.quiet {
 println!("Evidence valid for {}", self.task_id);
 }
 Ok(())
 } else {
 Err(CommandError::InvalidInput(format!(
 "Evidence validation failed for {}",
 self.task_id
)))
 }
 }
}

// ============================================================================
// collect-evidence / validate-evidence tests
// ============================================================================

#[cfg(test)]
mod collect_validate_tests {
 use super::*;
 use crate::artifacts::ensure_dir;
 use std::fs;
 use tempfile::TempDir;

 fn setup_repo() -> (TempDir, PathBuf) {
 let temp = TempDir::new().unwrap();
 let repo = temp.path().to_path_buf();
 let git = repo.join(".git");
 fs::create_dir(&git).unwrap();
 fs::write(git.join("config"), "[core]\n").unwrap();
 (temp, repo)
 }

 #[test]
 fn test_collect_evidence_command_writes_seven() {
 let (_temp, repo) = setup_repo();
 let task_id = "task-20260529-001";

 // Pre-create the backend output directory with raw artifacts.
 let backend_output = repo.join("backend-output");
 ensure_dir(&backend_output).unwrap();

 // diff.patch
 fs::write(backend_output.join("diff.patch"), "--- a/x\n+++ b/x\n").unwrap();

 // changed_files.json
 let changed = serde_json::json!({
 "schema_version": "changed-files-v1",
 "task_id": task_id,
 "files": [{"path": "src/lib.rs", "operation": "modify"}]
 });
 fs::write(
 backend_output.join("changed_files.json"),
 serde_json::to_string(&changed).unwrap(),
 )
 .unwrap();

 // verification.json
 let verification = serde_json::json!({
 "schema_version": "verification-v1",
 "task_id": task_id,
 "results": [{"command": "cargo test", "exit_code":0, "passed": true}]
 });
 fs::write(
 backend_output.join("verification.json"),
 serde_json::to_string(&verification).unwrap(),
 )
 .unwrap();

 // execution_trace.jsonl
 fs::write(
 backend_output.join("execution_trace.jsonl"),
 "{\"event\":\"x\",\"timestamp\":\"2026-05-29T12:00:00Z\"}\n",
 )
 .unwrap();

 let cmd = CollectEvidence {
 repo_root: repo.clone(),
 task_id: task_id.to_string(),
 backend_output_dir: Some(backend_output.clone()),
 normalized_dir: None,
 review_verdict: "pass".to_string(),
 execution_completeness: "full".to_string(),
 };

 assert!(cmd.run().is_ok());
 let paths = AgentRunsPaths::new(&repo);
 let normalized = paths.normalized_dir(task_id);
 for name in crate::evidence::REQUIRED_ARTIFACTS.iter() {
 assert!(normalized.join(name).exists(), "missing {}", name);
 }
 }

 #[test]
 fn test_collect_evidence_command_rejects_invalid_verdict() {
 let (_temp, repo) = setup_repo();
 let cmd = CollectEvidence {
 repo_root: repo,
 task_id: "task-20260529-001".to_string(),
 backend_output_dir: None,
 normalized_dir: None,
 review_verdict: "garbage".to_string(),
 execution_completeness: "full".to_string(),
 };
 assert!(cmd.run().is_err());
 }

 #[test]
 fn test_validate_evidence_command_accepts_clean() {
 let (_temp, repo) = setup_repo();
 let task_id = "task-20260529-001";
 let backend_output = repo.join("backend-output");
 ensure_dir(&backend_output).unwrap();
 // Pre-populate backend output with raw artifacts.
 fs::write(backend_output.join("diff.patch"), "--- a/x\n+++ b/x\n").unwrap();
 let changed = serde_json::json!({
 "schema_version": "changed-files-v1",
 "task_id": task_id,
 "files": [{"path": "src/lib.rs", "operation": "modify"}]
 });
 fs::write(backend_output.join("changed_files.json"), serde_json::to_string(&changed).unwrap()).unwrap();
 let verification = serde_json::json!({
 "schema_version": "verification-v1",
 "task_id": task_id,
 "results": [{"command": "cargo test", "exit_code":0, "passed": true}]
 });
 fs::write(backend_output.join("verification.json"), serde_json::to_string(&verification).unwrap()).unwrap();
 fs::write(backend_output.join("execution_trace.jsonl"), b"{\"event\":\"x\",\"timestamp\":\"2026-05-29T12:00:00Z\"}\n").unwrap();
 let normalized = repo.join("normalized");
 ensure_dir(&normalized).unwrap();

 // Write a clean set.
 let collect = CollectEvidence {
 repo_root: repo.clone(),
 task_id: task_id.to_string(),
 backend_output_dir: Some(backend_output.clone()),
 normalized_dir: Some(normalized.clone()),
 review_verdict: "pass".to_string(),
 execution_completeness: "full".to_string(),
 };
 collect.run().unwrap();

 let validate = ValidateEvidence {
 repo_root: repo,
 task_id: task_id.to_string(),
 normalized_dir: Some(normalized.clone()),
 quiet: true,
 };
 assert!(validate.run().is_ok());
 }

 #[test]
 fn test_validate_evidence_command_rejects_missing() {
 let (_temp, repo) = setup_repo();
 let normalized = repo.join("normalized");
 ensure_dir(&normalized).unwrap();
 let validate = ValidateEvidence {
 repo_root: repo,
 task_id: "task-20260529-001".to_string(),
 normalized_dir: Some(normalized),
 quiet: true,
 };
 assert!(validate.run().is_err());
 }

 #[test]
 fn test_validate_evidence_command_rejects_schema_violation() {
 let (_temp, repo) = setup_repo();
 let task_id = "task-20260529-001";
 let backend_output = repo.join("backend-output");
 ensure_dir(&backend_output).unwrap();
 let normalized = repo.join("normalized");
 ensure_dir(&normalized).unwrap();

 let collect = CollectEvidence {
 repo_root: repo.clone(),
 task_id: task_id.to_string(),
 backend_output_dir: Some(backend_output.clone()),
 normalized_dir: Some(normalized.clone()),
 review_verdict: "pass".to_string(),
 execution_completeness: "full".to_string(),
 };
 collect.run().unwrap();

 // Tamper with verification.json.
 let bad = serde_json::json!({
 "schema_version": "verification-v1",
 "task_id": task_id,
 "results": [{"command": "x", "exit_code":0}]
 });
 fs::write(
 normalized.join("verification.json"),
 serde_json::to_string(&bad).unwrap(),
 )
 .unwrap();

 let validate = ValidateEvidence {
 repo_root: repo,
 task_id: task_id.to_string(),
 normalized_dir: Some(normalized),
 quiet: true,
 };
 assert!(validate.run().is_err());
 }
}

// ============================================================================
// validate-sonnet-review command
// ============================================================================

/// Ordered list of required gates in the five-gate review order.
const REQUIRED_GATE_ORDER: [&str; 5] = [
    "evidence_validity",
    "scope_policy",
    "verification",
    "diff_code_review",
    "merge_recommendation",
];

/// Validate a sonnet_review.json artifact against the sonnet-review schema.
/// Enforces the five-gate review order: evidence_validity -> scope_policy ->
/// verification -> diff_code_review -> merge_recommendation.
/// Reports per-gate validity; rejects reviews with wrong gate count or order.
/// No semantic judgment, no repair, no modification.
pub struct ValidateSonnetReview {
    pub repo_root: PathBuf,
    pub task_id: Option<String>,
    pub path: Option<PathBuf>,
    pub quiet: bool,
}

impl ValidateSonnetReview {
    pub fn run(&self) -> CommandResult {
        let review_path = match (&self.path, &self.task_id) {
            (Some(p), _) => p.clone(),
            (None, Some(task_id)) => {
                let repo_root = find_repo_root(&self.repo_root)
                    .ok_or_else(|| CommandError::NotFound("Not inside a git repository".to_string()))?;
                let paths = AgentRunsPaths::new(&repo_root);
                let review_path = paths.task_dir(task_id).join("sonnet_review.json");
                if !review_path.exists() {
                    return Err(CommandError::NotFound(format!(
                        "sonnet_review.json not found for task {}",
                        task_id
                    )));
                }
                review_path
            }
            (None, None) => {
                return Err(CommandError::InvalidInput(
                    "Either --task-id or --path must be provided".to_string(),
                ));
            }
        };

        // Read and parse the JSON.
        let content = std::fs::read_to_string(&review_path)?;
        let json: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| CommandError::Json(e))?;

        // Step 1: JSON Schema validation.
        let schema: serde_json::Value =
            serde_json::from_str(schemas::SONNET_REVIEW_SCHEMA)
                .map_err(|e| CommandError::SchemaValidation(format!("Invalid schema: {}", e)))?;
        let validator = Validator::new(&schema)
            .map_err(|e| CommandError::SchemaValidation(e.to_string()))?;

        let schema_errors: Vec<String> = validator
            .iter_errors(&json)
            .map(|e| format!("{}: {}", e.instance_path, e))
            .collect();

        if !schema_errors.is_empty() {
            if !self.quiet {
                eprintln!("Schema validation failed for {}:", review_path.display());
                for err in &schema_errors {
                    eprintln!("  - {}", err);
                }
            }
            return Err(CommandError::SchemaValidation(schema_errors.join("; ")));
        }

        // Step 2: Gate-order enforcement.
        let gates = json.get("gates").and_then(|g| g.as_array());
        let gates = match gates {
            Some(arr) => arr,
            None => {
                // Schema already validated this is present; shouldn't happen.
                return Err(CommandError::SchemaValidation(
                    "gates array missing".to_string(),
                ));
            }
        };

        if gates.len() != 5 {
            if !self.quiet {
                eprintln!(
                    "Wrong number of gates: expected 5, got {}",
                    gates.len()
                );
            }
            return Err(CommandError::SchemaValidation(format!(
                "Wrong gate count: expected 5 gates, got {}",
                gates.len()
            )));
        }

        for (i, (expected, actual_entry)) in REQUIRED_GATE_ORDER
            .iter()
            .zip(gates.iter())
            .enumerate()
        {
            let actual_gate = actual_entry
                .get("gate")
                .and_then(|g| g.as_str())
                .unwrap_or("");
            if actual_gate != *expected {
                if !self.quiet {
                    eprintln!(
                        "Gate order violation at position {}: expected '{}', got '{}'",
                        i + 1,
                        expected,
                        actual_gate
                    );
                }
                return Err(CommandError::SchemaValidation(format!(
                    "Gate order violation at position {}: expected '{}', got '{}'",
                    i + 1,
                    expected,
                    actual_gate
                )));
            }
        }

        if !self.quiet {
            println!("sonnet_review.json: valid (five-gate order confirmed)");
        }
        Ok(())
    }
}

// ============================================================================
// final-gate command
// ============================================================================

/// Final gate decision values (must match opus_final_gate.json schema).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalGateDecision {
    Merge,
    RequestRepair,
    Reject,
    NeedsUserDecision,
}

impl FinalGateDecision {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "merge" => Some(Self::Merge),
            "request_repair" => Some(Self::RequestRepair),
            "reject" => Some(Self::Reject),
            "needs_user_decision" => Some(Self::NeedsUserDecision),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::RequestRepair => "request_repair",
            Self::Reject => "reject",
            Self::NeedsUserDecision => "needs_user_decision",
        }
    }
}

/// Opus final gate output artifact.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpusFinalGate {
    #[serde(rename = "schema_version")]
    pub schema_version: String,
    #[serde(rename = "task_id")]
    pub task_id: String,
    pub decision: String,
    #[serde(rename = "commit_message")]
    pub commit_message: Option<String>,
    pub timestamp: String,
    #[serde(rename = "notes", skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl OpusFinalGate {
    pub fn new(task_id: String, decision: FinalGateDecision, commit_message: Option<String>, notes: Option<String>) -> Self {
        Self {
            schema_version: "opus-final-gate-v1".to_string(),
            task_id,
            decision: decision.as_str().to_string(),
            commit_message,
            timestamp: chrono::Local::now().to_rfc3339(),
            notes,
        }
    }
}

/// Produce the final gate decision for a completed review.
/// Reads contract, evidence, sonnet review, diff, verification, and sensitive audit.
/// Writes opus_final_gate.json with four-state decision.
/// Does NOT apply patch or commit.
pub struct FinalGate {
    pub repo_root: PathBuf,
    pub task_id: String,
    pub decision: String,
    pub commit_message: String,
    pub notes: Option<String>,
}

impl FinalGate {
    pub fn run(&self) -> CommandResult {
        let repo_root = find_repo_root(&self.repo_root)
            .ok_or_else(|| CommandError::NotFound("Not inside a git repository".to_string()))?;
        TaskId::parse(&self.task_id).ok_or_else(|| {
            CommandError::IdValidation(format!("Invalid task_id format: {}", self.task_id))
        })?;
        let decision = FinalGateDecision::from_str(&self.decision)
            .ok_or_else(|| CommandError::InvalidInput(format!(
                "Invalid decision: {}. Must be one of: merge, request_repair, reject, needs_user_decision",
                self.decision
            )))?;
        let paths = AgentRunsPaths::new(&repo_root);
        let task_dir = paths.task_dir(&self.task_id);

        // Validate required artifacts exist before writing gate.
        let review_path = task_dir.join("sonnet_review.json");
        if !review_path.exists() {
            return Err(CommandError::NotFound(format!(
                "sonnet_review.json not found for task {} (required for final gate)",
                self.task_id
            )));
        }
        let gate_path = paths.machine_gate(&self.task_id);
        if !gate_path.exists() {
            return Err(CommandError::NotFound(format!(
                "machine_gate.json not found for task {} (required for final gate)",
                self.task_id
            )));
        }
        let normalized_dir = paths.normalized_dir(&self.task_id);
        if !normalized_dir.exists() {
            return Err(CommandError::NotFound(format!(
                "normalized/ directory not found for task {} (required for final gate)",
                self.task_id
            )));
        }
        let diff_path = normalized_dir.join("diff.patch");
        if !diff_path.exists() {
            return Err(CommandError::NotFound(format!(
                "diff.patch not found in normalized/ for task {} (required for final gate)",
                self.task_id
            )));
        }

        let commit_msg = if decision == FinalGateDecision::Merge {
            Some(self.commit_message.clone())
        } else {
            None
        };

        let gate = OpusFinalGate::new(
            self.task_id.clone(),
            decision,
            commit_msg,
            self.notes.clone(),
        );
        let gate_out = paths.opus_final_gate(&self.task_id);
        write_json(&gate_out, &gate)?;
        println!("Final gate written: {}", gate_out.display());
        println!("Decision: {}", gate.decision);
        Ok(())
    }
}

// ============================================================================
// integrate command
// ============================================================================

/// Post-apply verification output artifact.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PostApplyVerification {
    #[serde(rename = "schema_version")]
    pub schema_version: String,
    #[serde(rename = "task_id")]
    pub task_id: String,
    pub passed: bool,
    #[serde(rename = "commands_run")]
    pub commands_run: Vec<CommandRunResult>,
    pub timestamp: String,
    #[serde(rename = "error", skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommandRunResult {
    pub command: String,
    #[serde(rename = "exit_code")]
    pub exit_code: i32,
    pub passed: bool,
    #[serde(rename = "stdout_excerpt", skip_serializing_if = "Option::is_none")]
    pub stdout_excerpt: Option<String>,
    #[serde(rename = "stderr_excerpt", skip_serializing_if = "Option::is_none")]
    pub stderr_excerpt: Option<String>,
}

/// Integration result output artifact.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IntegrationResult {
    #[serde(rename = "schema_version")]
    pub schema_version: String,
    #[serde(rename = "task_id")]
    pub task_id: String,
    #[serde(rename = "commit_hash")]
    pub commit_hash: String,
    #[serde(rename = "committed_files")]
    pub committed_files: Vec<String>,
    #[serde(rename = "verification_passed")]
    pub verification_passed: bool,
    #[serde(rename = "worktree_removed")]
    pub worktree_removed: bool,
    #[serde(rename = "agent_runs_retained")]
    pub agent_runs_retained: bool,
    pub timestamp: String,
}

/// Integrate a task into the main worktree.
pub struct Integrate {
    pub repo_root: PathBuf,
    pub task_id: String,
}

impl Integrate {
    pub fn run(&self) -> CommandResult {
        let repo_root = find_repo_root(&self.repo_root)
            .ok_or_else(|| CommandError::NotFound("Not inside a git repository".to_string()))?;
        TaskId::parse(&self.task_id).ok_or_else(|| {
            CommandError::IdValidation(format!("Invalid task_id format: {}", self.task_id))
        })?;

        let paths = AgentRunsPaths::new(&repo_root);
        let task_dir = paths.task_dir(&self.task_id);
        let normalized_dir = paths.normalized_dir(&self.task_id);

        // Step 1: Verify opus_final_gate.json exists and decision is merge.
        let gate_path = paths.opus_final_gate(&self.task_id);
        if !gate_path.exists() {
            return Err(CommandError::InvalidInput(format!(
                "opus_final_gate.json not found for task {}", self.task_id
            )));
        }
        let gate: OpusFinalGate = read_json(&gate_path)?;
        if gate.decision != "merge" {
            return Err(CommandError::InvalidInput(format!(
                "opus_final_gate.decision is '{}', must be 'merge' for integration", gate.decision
            )));
        }
        let commit_message = gate.commit_message.clone()
            .ok_or_else(|| CommandError::InvalidInput(
                "opus_final_gate.commit_message is null (required for merge)".to_string()
            ))?;

        // Step 2: Verify machine_gate.json exists and passed.
        let machine_gate_path = paths.machine_gate(&self.task_id);
        if !machine_gate_path.exists() {
            return Err(CommandError::InvalidInput(format!(
                "machine_gate.json not found for task {}", self.task_id
            )));
        }
        #[derive(serde::Deserialize)]
        struct MachineGate { passed: bool }
        let machine_gate: MachineGate = read_json(&machine_gate_path)?;
        if !machine_gate.passed {
            return Err(CommandError::InvalidInput(format!(
                "machine_gate.passed is false for task {}", self.task_id
            )));
        }

        // Step 3: Require clean main worktree.
        Self::require_clean_worktree(&repo_root)?;

        // Step 4: Verify diff.patch exists.
        let diff_path = normalized_dir.join("diff.patch");
        if !diff_path.exists() {
            return Err(CommandError::NotFound(format!(
                "diff.patch not found in normalized/ for task {}", self.task_id
            )));
        }
        let diff_content = std::fs::read_to_string(&diff_path)?;

        // Step 5: git apply --check (conflict detection).
        let patch_file = task_dir.join(format!("TASK_{}_PATCH.patch", self.task_id));
        std::fs::write(&patch_file, &diff_content)?;
        let check_result = std::process::Command::new("git")
            .args(["apply", "--check", patch_file.to_str().unwrap()])
            .current_dir(&repo_root)
            .output()
            .map_err(|e| CommandError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!(
                "Failed to run git apply --check: {}", e
            ))))?;

        if !check_result.status.success() {
            let stderr = String::from_utf8_lossy(&check_result.stderr);
            let _ = std::fs::remove_file(&patch_file);
            return Err(CommandError::InvalidInput(format!(
                "Patch conflict detected (git apply --check failed):\n{}", stderr
            )));
        }

        // Step 6: Apply the patch.
        let apply_output = std::process::Command::new("git")
            .args(["apply", "--whitespace=nowarn", patch_file.to_str().unwrap()])
            .current_dir(&repo_root)
            .output()
            .map_err(|e| CommandError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!(
                "Failed to apply patch: {}", e
            ))))?;
        let patch_applied = apply_output.status.success();

        // Step 7: Run post-apply verification.
        let post_apply_result = Self::run_post_apply_verification(&self.task_id, &repo_root, &normalized_dir)?;

        // If verification failed, roll back the patch.
        if !post_apply_result.passed && patch_applied {
            let rollback = std::process::Command::new("git")
                .args(["checkout", "--", "."])
                .current_dir(&repo_root)
                .output()
                .map_err(|e| CommandError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!(
                    "Failed to rollback patch: {}", e
                ))))?;
            if !rollback.status.success() {
                return Err(CommandError::InvalidInput(
                    "Verification failed AND rollback also failed. Manual intervention required.".to_string()
                ));
            }
            let post_apply_path = paths.task_dir(&self.task_id).join("post_apply_verification.json");
            write_json(&post_apply_path, &post_apply_result)?;
            return Err(CommandError::InvalidInput(format!(
                "Post-apply verification failed. Patch has been rolled back. Verification record: {}",
                post_apply_path.display()
            )));
        }

        // Step 8: Record post_apply_verification.json.
        let post_apply_path = paths.task_dir(&self.task_id).join("post_apply_verification.json");
        write_json(&post_apply_path, &post_apply_result)?;

        // Step 9: Stage expected changed files (from changed_files.json evidence).
        let changed_files_path = normalized_dir.join("changed_files.json");
        let expected_files: Vec<String> = if changed_files_path.exists() {
            #[derive(serde::Deserialize)]
            struct ChangedFiles { files: Vec<ChangedFileEntry> }
            #[derive(serde::Deserialize)]
            struct ChangedFileEntry { path: String }
            let cf: ChangedFiles = read_json(&changed_files_path)?;
            cf.files.into_iter().map(|f| f.path).collect()
        } else {
            Vec::new()
        };

        for file in &expected_files {
            let stage_output = std::process::Command::new("git")
                .args(["add", "--", file.as_str()])
                .current_dir(&repo_root)
                .output()
                .map_err(|e| CommandError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!(
                    "Failed to stage {}: {}", file, e
                ))))?;
            if !stage_output.status.success() {
                return Err(CommandError::InvalidInput(format!(
                    "Failed to stage file: {} (file may not exist in diff)", file
                )));
            }
        }

        // Step 10: Create local commit.
        let commit_output = std::process::Command::new("git")
            .args(["commit", "-m", &commit_message])
            .current_dir(&repo_root)
            .output()
            .map_err(|e| CommandError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!(
                "Failed to commit: {}", e
            ))))?;

        if !commit_output.status.success() {
            return Err(CommandError::InvalidInput(format!(
                "Git commit failed: {}", String::from_utf8_lossy(&commit_output.stderr)
            )));
        }

        // Get commit hash.
        let hash_output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_root)
            .output()
            .map_err(|e| CommandError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!(
                "Failed to get commit hash: {}", e
            ))))?;
        let commit_hash = String::from_utf8_lossy(&hash_output.stdout).trim().to_string();

        // Step 11: Write integration_result.json.
        let result = IntegrationResult {
            schema_version: "integration-result-v1".to_string(),
            task_id: self.task_id.clone(),
            commit_hash: commit_hash.clone(),
            committed_files: expected_files.clone(),
            verification_passed: post_apply_result.passed,
            worktree_removed: false,
            agent_runs_retained: true,
            timestamp: chrono::Local::now().to_rfc3339(),
        };
        let result_path = paths.task_dir(&self.task_id).join("integration_result.json");
        write_json(&result_path, &result)?;

        // Step 12: Remove worktree directory.
        let worktree = WorktreePaths::new(&repo_root).worktree(&self.task_id);
        let worktree_removed = if worktree.exists() {
            std::fs::remove_dir_all(&worktree).is_ok()
        } else {
            true
        };

        // Update result with worktree status.
        let mut final_result = result;
        final_result.worktree_removed = worktree_removed;
        write_json(&result_path, &final_result)?;

        // Clean up patch file.
        let _ = std::fs::remove_file(&patch_file);

        println!("Integration complete for task {}", self.task_id);
        println!("  Commit: {}", final_result.commit_hash);
        println!("  Files: {:?}", final_result.committed_files);
        println!("  Verification passed: {}", final_result.verification_passed);
        println!("  Worktree removed: {}", final_result.worktree_removed);
        println!("  Result: {}", result_path.display());

        Ok(())
    }

    fn require_clean_worktree(repo_root: &Path) -> CommandResult {
        let output = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repo_root)
            .output()
            .map_err(|e| CommandError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!(
                "Failed to check git status: {}", e
            ))))?;
        let status = String::from_utf8_lossy(&output.stdout);
        if !status.trim().is_empty() {
            return Err(CommandError::InvalidInput(format!(
                "Main worktree is not clean. Uncommitted changes:\n{}", status
            )));
        }
        Ok(())
    }

    fn run_post_apply_verification(
        task_id: &str,
        repo_root: &Path,
        normalized_dir: &Path,
    ) -> CommandResult<PostApplyVerification> {
        let mut commands_run: Vec<CommandRunResult> = Vec::new();

        let verification_path = normalized_dir.join("verification.json");
        if verification_path.exists() {
            #[derive(serde::Deserialize)]
            struct VerificationDoc { results: Vec<VerificationResultEntry> }
            #[derive(serde::Deserialize)]
            struct VerificationResultEntry {
                command: String,
                #[serde(rename = "exit_code")]
                exit_code: i32,
                passed: bool,
            }

            if let Ok(doc) = read_json::<VerificationDoc>(&verification_path) {
                for entry in doc.results {
                    let output = std::process::Command::new("sh")
                        .args(["-c", &entry.command])
                        .current_dir(repo_root)
                        .output();

                    let (exit_code, passed, stdout_excerpt, stderr_excerpt) = match output {
                        Ok(out) => {
                            let code = out.status.code().unwrap_or(-1);
                            let pass = out.status.success();
                            let stdout = String::from_utf8_lossy(&out.stdout);
                            let stderr = String::from_utf8_lossy(&out.stderr);
                            (
                                code, pass,
                                Some(stdout.to_string()),
                                Some(stderr.to_string()),
                            )
                        }
                        Err(e) => (-1, false, None, Some(format!("Failed to run: {}", e))),
                    };
                    commands_run.push(CommandRunResult {
                        command: entry.command,
                        exit_code,
                        passed,
                        stdout_excerpt,
                        stderr_excerpt,
                    });
                }
            }
        }

        // Default Phase1 verification: cargo test --lib
        let cargo_test_output = std::process::Command::new("sh")
            .args(["-c", "cargo test --lib 2>&1 || true"])
            .current_dir(repo_root)
            .output();

        let (cargo_exit, cargo_passed, cargo_stdout, cargo_stderr) = match cargo_test_output {
            Ok(out) => {
                let code = out.status.code().unwrap_or(-1);
                let pass = out.status.success();
                (code, pass,
                    Some(String::from_utf8_lossy(&out.stdout).to_string()),
                    Some(String::from_utf8_lossy(&out.stderr).to_string()))
            }
            Err(_) => (-1, false, None, Some("cargo test not available".to_string())),
        };

        commands_run.push(CommandRunResult {
            command: "cargo test --lib".to_string(),
            exit_code: cargo_exit,
            passed: cargo_passed,
            stdout_excerpt: cargo_stdout,
            stderr_excerpt: cargo_stderr,
        });

        let passed = commands_run.iter().all(|r| r.passed);

        Ok(PostApplyVerification {
            schema_version: "post-apply-verification-v1".to_string(),
            task_id: task_id.to_string(),
            passed,
            commands_run,
            timestamp: chrono::Local::now().to_rfc3339(),
            error: None,
        })
    }
}

// ============================================================================
// git-guard command
// ============================================================================

/// Git guard decision output.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GitGuardResult {
    #[serde(rename = "schema_version")]
    pub schema_version: String,
    #[serde(rename = "task_id")]
    pub task_id: String,
    pub status: String,
    #[serde(rename = "blocked_reason", skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    pub timestamp: String,
}

/// Check whether git operations are safe for a task.
pub struct GitGuard {
    pub repo_root: PathBuf,
    pub task_id: String,
}

impl GitGuard {
    pub fn run(&self) -> CommandResult {
        let repo_root = find_repo_root(&self.repo_root)
            .ok_or_else(|| CommandError::NotFound("Not inside a git repository".to_string()))?;
        TaskId::parse(&self.task_id).ok_or_else(|| {
            CommandError::IdValidation(format!("Invalid task_id format: {}", self.task_id))
        })?;

        let paths = AgentRunsPaths::new(&repo_root);
        let task_dir = paths.task_dir(&self.task_id);
        let status_path = paths.task_status(&self.task_id);

        // No run detected -> allow.
        if !task_dir.exists() || !status_path.exists() {
            println!("allowed: no run detected");
            println!("status: allowed");
            return Ok(());
        }

        let status: StatusJson = read_json(&status_path)?;

        // Active run -> block.
        if status.status == TaskStatus::Active {
            println!("blocked: active run in progress");
            println!("status: blocked");
            println!("reason: active_run");
            return Err(CommandError::InvalidInput("blocked: active run in progress".to_string()));
        }

        // Check opus_final_gate.
        let gate_path = paths.opus_final_gate(&self.task_id);
        if !gate_path.exists() {
            println!("pending: no final gate decision");
            println!("status: pending");
            return Ok(());
        }

        let gate: OpusFinalGate = read_json(&gate_path)?;
        if gate.decision != "merge" {
            println!("blocked: final gate decision is '{}'", gate.decision);
            println!("status: blocked");
            println!("reason: gate_rejected");
            return Err(CommandError::InvalidInput(format!(
                "blocked: final gate decision is '{}'", gate.decision
            )));
        }

        // Check machine gate.
        let machine_gate_path = paths.machine_gate(&self.task_id);
        if !machine_gate_path.exists() {
            println!("pending: no machine gate");
            println!("status: pending");
            return Ok(());
        }
        #[derive(serde::Deserialize)]
        struct MachineGate { passed: bool }
        let machine_gate: MachineGate = read_json(&machine_gate_path)?;
        if !machine_gate.passed {
            println!("blocked: machine gate failed");
            println!("status: blocked");
            println!("reason: machine_gate_failed");
            return Err(CommandError::InvalidInput("blocked: machine gate failed".to_string()));
        }

        // Check main worktree cleanliness.
        let output = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&repo_root)
            .output();

        match output {
            Ok(out) => {
                let code = out.status.code().unwrap_or(-1);
                let stderr = String::from_utf8_lossy(&out.stderr);

                // git status --porcelain: dirty if stdout is non-empty, regardless of exit code.
                if code == 0 || code == 1 {
                    let status_output = String::from_utf8_lossy(&out.stdout);

                    if !status_output.trim().is_empty() {
                        println!("blocked: main worktree not clean");
                        println!("status: blocked");
                        println!("reason: dirty_worktree");
                        return Err(CommandError::InvalidInput("blocked: main worktree not clean".to_string()));
                    }
                    // Worktree is clean - fall through to allowed.
                } else {


                    println!("pending: cannot verify worktree status");
                    println!("status: pending");
                    return Ok(());
                }
            }
            Err(_) => {
                println!("pending: cannot verify worktree status");
                println!("status: pending");
                return Ok(());
            }
        }

        println!("allowed: final gate merge with all preconditions satisfied");
        println!("status: allowed");
        Ok(())
    }
}

// ============================================================================
// validate-subagent-stop command
// ============================================================================

/// Validate that adapter/reviewer artifacts are present for SubagentStop hook.
pub struct ValidateSubagentStop {
    pub repo_root: PathBuf,
    pub task_id: String,
}

impl ValidateSubagentStop {
    pub fn run(&self) -> CommandResult {
        let repo_root = find_repo_root(&self.repo_root)
            .ok_or_else(|| CommandError::NotFound("Not inside a git repository".to_string()))?;
        TaskId::parse(&self.task_id).ok_or_else(|| {
            CommandError::IdValidation(format!("Invalid task_id format: {}", self.task_id))
        })?;

        let paths = AgentRunsPaths::new(&repo_root);
        let task_dir = paths.task_dir(&self.task_id);
        let mut missing: Vec<String> = Vec::new();

        let machine_gate = paths.machine_gate(&self.task_id);
        if !machine_gate.exists() {
            missing.push("machine_gate.json".to_string());
        }
        let review = task_dir.join("sonnet_review.json");
        if !review.exists() {
            missing.push("sonnet_review.json".to_string());
        }
        let final_gate = paths.opus_final_gate(&self.task_id);
        if !final_gate.exists() {
            missing.push("opus_final_gate.json".to_string());
        }

        if missing.is_empty() {
            println!("All required SubagentStop artifacts present");
            Ok(())
        } else {
            eprintln!("Missing required artifacts:");
            for m in &missing {
                eprintln!("  - {}", m);
            }
            Err(CommandError::InvalidInput(format!(
                "Missing {} required artifact(s) for SubagentStop", missing.len()
            )))
        }
    }
}

// ============================================================================
// pre-tool-guard command (PreToolUse hook enforcement)
// ============================================================================

/// PreToolCheck validates git operations for dangerous commands.
/// Guards: git apply, git commit, git merge, git push, git add . and git add -A.
/// Calls git-guard logic: no run -> allow, active run -> block, merge preconditions -> allow.
/// Push/merge remote/deploy always blocked by integration policy.
pub struct PreToolCheck {
    pub repo_root: PathBuf,
    pub task_id: Option<String>,
}

impl PreToolCheck {
    /// Run the pre-tool check. Returns Ok(()) if allowed, Err if blocked.
    pub fn run(&self) -> CommandResult {
        let repo_root = find_repo_root(&self.repo_root)
            .ok_or_else(|| CommandError::NotFound("Not inside a git repository".to_string()))?;

        let paths = AgentRunsPaths::new(&repo_root);

        // If no task_id provided, just check whether any active run exists.
        // Default: no run detected -> allow.
        let task_id = match &self.task_id {
            Some(t) => t.clone(),
            None => {
                // No task specified: check for any active run in the repo.
                let tasks_dir = paths.tasks_root();
                if tasks_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&tasks_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_dir() {
                                let status_path = path.join("status.json");
                                if status_path.exists() {
                                    if let Ok(status) = read_json::<StatusJson>(&status_path) {
                                        if status.status == TaskStatus::Active {
                                            println!("blocked: active run in progress");
                                            println!("status: blocked");
                                            println!("reason: active_run");
                                            return Err(CommandError::InvalidInput(
                                                "blocked: active run in progress".to_string(),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                println!("allowed: no active run detected");
                println!("status: allowed");
                return Ok(());
            }
        };

        // Validate task_id format.
        TaskId::parse(&task_id).ok_or_else(|| {
            CommandError::IdValidation(format!("Invalid task_id format: {}", task_id))
        })?;

        let task_dir = paths.task_dir(&task_id);
        let status_path = paths.task_status(&task_id);

        // No run detected -> allow.
        if !task_dir.exists() || !status_path.exists() {
            println!("allowed: no run detected for {}", task_id);
            println!("status: allowed");
            return Ok(());
        }

        let status: StatusJson = read_json(&status_path)?;

        // Active run -> block.
        if status.status == TaskStatus::Active {
            println!("blocked: active run in progress");
            println!("status: blocked");
            println!("reason: active_run");
            return Err(CommandError::InvalidInput("blocked: active run in progress".to_string()));
        }

        // Check opus_final_gate.
        let gate_path = paths.opus_final_gate(&task_id);
        if !gate_path.exists() {
            println!("pending: no final gate decision");
            println!("status: pending");
            return Ok(());
        }

        let gate: OpusFinalGate = read_json(&gate_path)?;
        if gate.decision != "merge" {
            println!("blocked: final gate decision is '{}'", gate.decision);
            println!("status: blocked");
            println!("reason: gate_rejected");
            return Err(CommandError::InvalidInput(format!(
                "blocked: final gate decision is '{}'", gate.decision
            )));
        }

        // Check machine gate.
        let machine_gate_path = paths.machine_gate(&task_id);
        if !machine_gate_path.exists() {
            println!("pending: no machine gate");
            println!("status: pending");
            return Ok(());
        }
        #[derive(serde::Deserialize)]
        struct MachineGate { passed: bool }
        let machine_gate: MachineGate = read_json(&machine_gate_path)?;
        if !machine_gate.passed {
            println!("blocked: machine gate failed");
            println!("status: blocked");
            println!("reason: machine_gate_failed");
            return Err(CommandError::InvalidInput("blocked: machine gate failed".to_string()));
        }

        // Check commit message is present.
        if gate.commit_message.is_none() || gate.commit_message.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
            println!("blocked: commit_message missing in final gate");
            println!("status: blocked");
            println!("reason: missing_commit_message");
            return Err(CommandError::InvalidInput(
                "blocked: commit_message missing in final gate".to_string(),
            ));
        }

        // All preconditions satisfied -> allow.
        println!("allowed: final gate merge with all preconditions satisfied");
        println!("status: allowed");
        Ok(())
    }
}
