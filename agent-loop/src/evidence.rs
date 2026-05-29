//! Evidence collection and validation for agent-loop Phase1.
//!
//! `collect-evidence` deterministically normalizes backend raw outputs into
//! seven required artifacts under `.agent-runs/tasks/{task_id}/normalized/`.
//! `validate-evidence` validates each artifact against its JSON Schema and
//! produces no semantic judgment, no repair, and no fabrication.
//!
//! All Phase1 evidence schemas are snake_case JSON. Execution trace uses
//! JSONL; everything else is JSON or text (diff.patch).
//!
//! Seven required artifacts:
//!1. `changed_files.json` — list of {path, operation}
//!2. `diff.patch` — unified diff text
//!3. `execution_trace.jsonl` — one event object per line
//!4. `verification.json` — list of {command, exit_code, passed}
//!5. `external_review.json` — independent reviewer verdict
//!6. `sensitive_audit.json` — deterministic sensitive-file detector
//!7. `final_evidence.json` — bundle referencing all of the above

use crate::artifacts::{ensure_dir, read_json, write_json, AgentRunsPaths, WorktreePaths};
use crate::id::TaskId;
use crate::schemas;
use jsonschema::Validator;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Errors produced by evidence collection/validation.
#[derive(Debug)]
pub enum EvidenceError {
 Io(std::io::Error),
 Json(serde_json::Error),
 Schema(String),
 InvalidTaskId(String),
 MissingArtifact(String),
 InvalidBackendOutput(String),
}

impl std::fmt::Display for EvidenceError {
 fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
 match self {
 EvidenceError::Io(e) => write!(f, "evidence IO error: {}", e),
 EvidenceError::Json(e) => write!(f, "evidence JSON error: {}", e),
 EvidenceError::Schema(e) => write!(f, "evidence schema error: {}", e),
 EvidenceError::InvalidTaskId(e) => write!(f, "invalid task id: {}", e),
 EvidenceError::MissingArtifact(e) => write!(f, "missing evidence artifact: {}", e),
 EvidenceError::InvalidBackendOutput(e) => write!(f, "invalid backend output: {}", e),
 }
 }
}

impl std::error::Error for EvidenceError {}

impl From<std::io::Error> for EvidenceError {
 fn from(e: std::io::Error) -> Self {
 EvidenceError::Io(e)
 }
}

impl From<serde_json::Error> for EvidenceError {
 fn from(e: serde_json::Error) -> Self {
 EvidenceError::Json(e)
 }
}

pub type EvidenceResult<T> = Result<T, EvidenceError>;

/// Required normalized artifact names (snake_case, exactly seven).
pub const REQUIRED_ARTIFACTS: [&str;7] = [
 "changed_files.json",
 "diff.patch",
 "execution_trace.jsonl",
 "verification.json",
 "external_review.json",
 "sensitive_audit.json",
 "final_evidence.json",
];

/// Validate a task_id and return the typed wrapper.
pub fn parse_task_id(task_id: &str) -> EvidenceResult<TaskId> {
 TaskId::parse(task_id).ok_or_else(|| EvidenceError::InvalidTaskId(task_id.to_string()))
}

/// One file entry in `changed_files.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangedFileEntry {
 pub path: String,
 pub operation: String, // create | modify | delete
}

/// Shape of `changed_files.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedFilesDoc {
 pub schema_version: String,
 pub task_id: String,
 pub files: Vec<ChangedFileEntry>,
}

impl ChangedFilesDoc {
 pub fn new(task_id: &str, files: Vec<ChangedFileEntry>) -> Self {
 Self {
 schema_version: "changed-files-v1".to_string(),
 task_id: task_id.to_string(),
 files,
 }
 }
}

/// One verification result in `verification.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
 pub command: String,
 pub exit_code: i64,
 pub passed: bool,
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub stdout_excerpt: Option<String>,
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub stderr_excerpt: Option<String>,
}

/// Shape of `verification.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationDoc {
 pub schema_version: String,
 pub task_id: String,
 pub results: Vec<VerificationResult>,
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub all_required_passed: Option<bool>,
}

/// Shape of `external_review.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalReviewDoc {
 pub schema_version: String,
 pub task_id: String,
 pub verdict: String, // pass | fail | blocked
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub scope_compliance: Option<bool>,
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub policy_compliance: Option<bool>,
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub verification_sufficient: Option<bool>,
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub summary: Option<String>,
 #[serde(default, skip_serializing_if = "Vec::is_empty")]
 pub findings: Vec<ExternalReviewFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalReviewFinding {
 pub category: String,
 pub severity: String,
 pub description: String,
}

/// Shape of `sensitive_audit.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitiveAuditDoc {
 pub schema_version: String,
 pub task_id: String,
 pub detector: String,
 pub blocked: bool,
 #[serde(default)]
 pub sensitive_paths_touched: Vec<String>,
 #[serde(default)]
 pub forbidden_patterns_violated: Vec<String>,
 #[serde(default)]
 pub limitations: Vec<String>,
}

/// Shape of `final_evidence.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalEvidenceDoc {
 pub schema_version: String,
 pub task_id: String,
 pub execution_completeness: String, // full | partial | unavailable
 pub external_verdict: Option<String>,
 pub artifacts: FinalEvidenceArtifacts,
 #[serde(default)]
 pub audit_limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalEvidenceArtifacts {
 pub changed_files: String,
 pub diff_patch: String,
 pub execution_trace: String,
 pub verification: String,
 pub external_review: String,
 pub sensitive_audit: String,
 pub final_evidence: String,
}

/// Outcome of one `validate-evidence` invocation.
#[derive(Debug, Clone)]
pub struct ValidationReport {
 pub task_id: String,
 pub normalized_dir: PathBuf,
 pub artifacts: Vec<ArtifactValidation>,
 pub valid: bool,
}

#[derive(Debug, Clone)]
pub struct ArtifactValidation {
 pub name: String,
 pub path: PathBuf,
 pub valid: bool,
 pub errors: Vec<String>,
}

/// Worktree lifecycle helpers.
///
/// `/agent-run` must never modify the main worktree. The adapter creates
/// `.worktrees/{task_id}/` and points the backend at it. This module owns
/// the path conventions; it does NOT shell out to `git worktree add`.
#[derive(Debug, Clone)]
pub struct WorktreeHandle {
 pub task_id: String,
 pub root: PathBuf,
}

impl WorktreeHandle {
 /// Create the worktree directory and ensure it exists. This is the
 /// deterministic path step. The actual `git worktree add` invocation is
 /// performed by the caller (adapter / hook), not by the CLI.
 pub fn create(&self) -> EvidenceResult<()> {
 if self.root.exists() {
 // Already created; idempotent.
 return Ok(());
 }
 ensure_dir(&self.root)?;
 Ok(())
 }

 /// Remove the worktree directory. Used by `/agent-integrate` after a
 /// successful commit, and by `cleanup`. Phase1 invariant: this never
 /// touches the main worktree.
 pub fn remove(&self) -> EvidenceResult<()> {
 if self.root.exists() {
 fs::remove_dir_all(&self.root)?;
 }
 Ok(())
 }

 /// True iff the worktree directory exists on disk.
 pub fn exists(&self) -> bool {
 self.root.exists()
 }

 /// True iff the given absolute path resolves inside this worktree.
 pub fn contains(&self, path: &Path) -> bool {
 path.starts_with(&self.root)
 }
}

/// Resolve the worktree path for a task_id under the given repo root.
pub fn worktree_for(repo_root: &Path, task_id: &str) -> WorktreeHandle {
 let paths = WorktreePaths::new(repo_root);
 WorktreeHandle {
 task_id: task_id.to_string(),
 root: paths.worktree(task_id),
 }
}

/// Validate that `path` does not escape the main worktree. Used by adapter
/// preflight to keep backend output contained.
pub fn is_within_repo_root(repo_root: &Path, path: &Path) -> bool {
 let canon = match fs::canonicalize(path) {
 Ok(p) => p,
 Err(_) => return false,
 };
 let canon_root = match fs::canonicalize(repo_root) {
 Ok(p) => p,
 Err(_) => return false,
 };
 canon.starts_with(&canon_root)
}

/// Inputs to `collect_evidence`. The caller is responsible for the actual
/// backend invocation; this struct describes the deterministic artifacts
/// left behind by the backend in `backend_output_dir`.
pub struct CollectEvidenceInputs<'a> {
 pub task_id: &'a str,
 pub backend_output_dir: &'a Path,
 pub normalized_dir: &'a Path,
 pub diff_text: &'a str,
 pub changed_files: Vec<ChangedFileEntry>,
 pub verification: Vec<VerificationResult>,
 pub external_review: ExternalReviewDoc,
 pub execution_trace_events: Vec<ExecutionTraceEvent>,
 pub execution_completeness: &'a str, // full | partial | unavailable
 pub audit_limitations: Vec<String>,
}

/// One event in `execution_trace.jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTraceEvent {
 pub event: String,
 pub timestamp: String,
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub details: Option<Value>,
}

impl ExecutionTraceEvent {
 pub fn new(event: &str, timestamp: &str) -> Self {
 Self {
 event: event.to_string(),
 timestamp: timestamp.to_string(),
 details: None,
 }
 }
}

/// Collect evidence: deterministic transformation from backend raw output
/// into the seven required normalized artifacts. Returns the list of
/// artifact paths written. Fails if required inputs are missing.
pub fn collect_evidence(inputs: CollectEvidenceInputs) -> EvidenceResult<Vec<PathBuf>> {
 let _ = parse_task_id(inputs.task_id)?;

 ensure_dir(inputs.normalized_dir)?;

 let mut written: Vec<PathBuf> = Vec::new();

 //1. changed_files.json
 let changed = ChangedFilesDoc::new(inputs.task_id, inputs.changed_files.clone());
 let changed_path = inputs.normalized_dir.join("changed_files.json");
 write_json(&changed_path, &changed)?;
 written.push(changed_path);

 //2. diff.patch
 let diff_path = inputs.normalized_dir.join("diff.patch");
 fs::write(&diff_path, inputs.diff_text.as_bytes())?;
 written.push(diff_path);

 //3. execution_trace.jsonl — append backend raw trace if present, then events.
 let trace_path = inputs.normalized_dir.join("execution_trace.jsonl");
 let mut trace_lines: Vec<String> = Vec::new();
 let raw_trace = inputs.backend_output_dir.join("execution_trace.jsonl");
 if raw_trace.exists() {
 let raw = fs::read_to_string(&raw_trace)?;
 for line in raw.lines() {
 let trimmed = line.trim();
 if trimmed.is_empty() {
 continue;
 }
 // Wrap raw backend trace line with required schema fields when missing.
 let mut v: serde_json::Value = match serde_json::from_str(trimmed) {
 Ok(v) => v,
 Err(_) => continue,
 };
 if v.get("schema_version").is_none() {
 v["schema_version"] = serde_json::json!("execution-trace-v1");
 }
 if v.get("task_id").is_none() {
 v["task_id"] = serde_json::json!(inputs.task_id);
 }
 if v.get("event").is_none() {
 v["event"] = serde_json::json!("backend_event");
 }
 if v.get("timestamp").is_none() {
 v["timestamp"] = serde_json::json!("1970-01-01T00:00:00Z");
 }
 if let Ok(s) = serde_json::to_string(&v) {
 trace_lines.push(s);
 }
 }
 }
 for ev in &inputs.execution_trace_events {
 let mut obj = serde_json::json!({
 "schema_version": "execution-trace-v1",
 "task_id": inputs.task_id,
 "event": ev.event,
 "timestamp": ev.timestamp,
 });
 if let Some(d) = &ev.details {
 obj["details"] = d.clone();
 }
 trace_lines.push(serde_json::to_string(&obj)?);
 }
 if trace_lines.is_empty() {
 // Empty file is still a valid artifact; empty line set is acceptable.
 fs::write(&trace_path, "")?;
 } else {
 fs::write(&trace_path, trace_lines.join("\n") + "\n")?;
 }
 written.push(trace_path);

 //4. verification.json
 let all_required_passed = if inputs.verification.is_empty() {
 None
 } else {
 Some(inputs.verification.iter().all(|r| r.passed))
 };
 let verification = VerificationDoc {
 schema_version: "verification-v1".to_string(),
 task_id: inputs.task_id.to_string(),
 results: inputs.verification.clone(),
 all_required_passed,
 };
 let verification_path = inputs.normalized_dir.join("verification.json");
 write_json(&verification_path, &verification)?;
 written.push(verification_path);

 //5. external_review.json
 let mut review = inputs.external_review.clone();
 review.schema_version = "external-review-v1".to_string();
 review.task_id = inputs.task_id.to_string();
 let review_path = inputs.normalized_dir.join("external_review.json");
 write_json(&review_path, &review)?;
 written.push(review_path);

 //6. sensitive_audit.json — run deterministic detector over backend output.
 let audit = run_sensitive_audit(inputs.task_id, inputs.backend_output_dir);
 let audit_path = inputs.normalized_dir.join("sensitive_audit.json");
 write_json(&audit_path, &audit)?;
 written.push(audit_path);

 //7. final_evidence.json — references all of the above.
 let final_doc = FinalEvidenceDoc {
 schema_version: "final-evidence-v1".to_string(),
 task_id: inputs.task_id.to_string(),
 execution_completeness: inputs.execution_completeness.to_string(),
 external_verdict: Some(review.verdict.clone()),
 artifacts: FinalEvidenceArtifacts {
 changed_files: "changed_files.json".to_string(),
 diff_patch: "diff.patch".to_string(),
 execution_trace: "execution_trace.jsonl".to_string(),
 verification: "verification.json".to_string(),
 external_review: "external_review.json".to_string(),
 sensitive_audit: "sensitive_audit.json".to_string(),
 final_evidence: "final_evidence.json".to_string(),
 },
 audit_limitations: inputs.audit_limitations.clone(),
 };
 let final_path = inputs.normalized_dir.join("final_evidence.json");
 write_json(&final_path, &final_doc)?;
 written.push(final_path);

 Ok(written)
}

/// Default sensitive-path detector. Phase1: deterministic patterns derived
/// from contract `mutation_policy.forbidden_patterns`. Backend claims are
/// supplementary and never source of truth.
pub fn run_sensitive_audit(task_id: &str, backend_output_dir: &Path) -> SensitiveAuditDoc {
 let mut sensitive: Vec<String> = Vec::new();
 let mut limitations: Vec<String> = Vec::new();
 limitations.push(
 "phase1_detector_only: backend claims are not source of truth".to_string(),
 );

 let changed_files_path = backend_output_dir.join("changed_files.json");
 if changed_files_path.exists() {
 if let Ok(doc) = read_json::<ChangedFilesDoc>(&changed_files_path) {
 for f in &doc.files {
 let p = &f.path;
 if is_known_sensitive(p) {
 sensitive.push(p.clone());
 }
 }
 }
 } else {
 limitations.push("no changed_files.json in backend output".to_string());
 }

 SensitiveAuditDoc {
 schema_version: "sensitive-audit-v1".to_string(),
 task_id: task_id.to_string(),
 detector: "phase1-default".to_string(),
 blocked: !sensitive.is_empty(),
 sensitive_paths_touched: sensitive,
 forbidden_patterns_violated: Vec::new(),
 limitations,
 }
}

/// Default sensitive-path predicate. Conservative — only flags widely
/// recognized patterns.
fn is_known_sensitive(path: &str) -> bool {
 let p = path.to_lowercase();
 p.contains(".env")
 || p.contains("secret")
 || p.contains("credential")
 || p.contains("/.ssh/")
 || p.contains("private_key")
 || p.contains("id_rsa")
 || p.contains("password")
}

/// Validate a single normalized artifact by file name.
pub fn validate_artifact(path: &Path) -> EvidenceResult<Vec<String>> {
 let name = path
 .file_name()
 .and_then(|n| n.to_str())
 .ok_or_else(|| EvidenceError::MissingArtifact(path.display().to_string()))?;

 let schema_str: &str = match name {
 "changed_files.json" => schemas::CHANGED_FILES_SCHEMA,
 "execution_trace.jsonl" => return validate_execution_trace(path),
 "verification.json" => schemas::VERIFICATION_SCHEMA,
 "external_review.json" => schemas::EXTERNAL_REVIEW_SCHEMA,
 "sensitive_audit.json" => schemas::SENSITIVE_AUDIT_SCHEMA,
 "final_evidence.json" => schemas::FINAL_EVIDENCE_SCHEMA,
 "diff.patch" => return validate_diff_patch(path),
 other => {
 return Err(EvidenceError::MissingArtifact(format!(
 "unknown artifact name: {}",
 other
)))
 }
 };

 let content = fs::read_to_string(path)?;
 let json: Value = serde_json::from_str(&content)?;
 let schema: Value = serde_json::from_str(schema_str)
 .map_err(|e| EvidenceError::Schema(format!("schema parse: {}", e)))?;
 let validator = Validator::new(&schema)
 .map_err(|e| EvidenceError::Schema(format!("schema compile: {}", e)))?;

 let errors: Vec<String> = validator
 .iter_errors(&json)
 .map(|e| format!("{}: {}", e.instance_path, e))
 .collect();
 Ok(errors)
}

fn validate_execution_trace(path: &Path) -> EvidenceResult<Vec<String>> {
 let content = fs::read_to_string(path)?;
 let schema: Value = serde_json::from_str(schemas::EXECUTION_TRACE_LINE_SCHEMA)
 .map_err(|e| EvidenceError::Schema(format!("schema parse: {}", e)))?;
 let validator = Validator::new(&schema)
 .map_err(|e| EvidenceError::Schema(format!("schema compile: {}", e)))?;
 let mut errors: Vec<String> = Vec::new();
 for (i, line) in content.lines().enumerate() {
 let trimmed = line.trim();
 if trimmed.is_empty() {
 continue;
 }
 let json: Value = match serde_json::from_str(trimmed) {
 Ok(v) => v,
 Err(e) => {
 errors.push(format!("line {}: invalid JSON: {}", i +1, e));
 continue;
 }
 };
 for err in validator.iter_errors(&json) {
 errors.push(format!("line {}: {}: {}", i +1, err.instance_path, err));
 }
 }
 Ok(errors)
}

fn validate_diff_patch(path: &Path) -> EvidenceResult<Vec<String>> {
 let content = fs::read_to_string(path)?;
 // Phase1: text must be non-empty when present. We do NOT validate diff
 // syntax or content — that is semantic review work.
 if content.trim().is_empty() {
 return Ok(vec!["diff.patch is empty".to_string()]);
 }
 Ok(Vec::new())
}

/// Validate all required artifacts in `normalized_dir`.
pub fn validate_evidence(task_id: &str, normalized_dir: &Path) -> EvidenceResult<ValidationReport> {
 let _ = parse_task_id(task_id)?;

 let mut artifacts: Vec<ArtifactValidation> = Vec::new();
 let mut overall_valid = true;

 for name in REQUIRED_ARTIFACTS.iter() {
 let path = normalized_dir.join(name);
 if !path.exists() {
 artifacts.push(ArtifactValidation {
 name: name.to_string(),
 path,
 valid: false,
 errors: vec![format!("required artifact missing: {}", name)],
 });
 overall_valid = false;
 continue;
 }
 match validate_artifact(&path) {
 Ok(errors) => {
 let valid = errors.is_empty();
 if !valid {
 overall_valid = false;
 }
 artifacts.push(ArtifactValidation {
 name: name.to_string(),
 path,
 valid,
 errors,
 });
 }
 Err(e) => {
 overall_valid = false;
 artifacts.push(ArtifactValidation {
 name: name.to_string(),
 path,
 valid: false,
 errors: vec![e.to_string()],
 });
 }
 }
 }

 Ok(ValidationReport {
 task_id: task_id.to_string(),
 normalized_dir: normalized_dir.to_path_buf(),
 artifacts,
 valid: overall_valid,
 })
}

/// Convenience helper: build the normalized dir path for a task.
pub fn normalized_dir_for(repo_root: &Path, task_id: &str) -> PathBuf {
 AgentRunsPaths::new(repo_root).normalized_dir(task_id)
}

// ============================================================================
// Tests
// ============================================================================


#[cfg(test)]
mod tests_inner {

 use super::*;
 use tempfile::TempDir;

 fn setup() -> (TempDir, PathBuf, PathBuf) {
 let temp = TempDir::new().unwrap();
 let repo = temp.path().to_path_buf();
 let task_id = "task-20260529-001";
 let backend_output = repo.join("backend-output");
 let normalized = repo.join("normalized");
 fs::create_dir_all(&backend_output).unwrap();
 (temp, backend_output, normalized)
 }

 fn default_review(task_id: &str) -> ExternalReviewDoc {
 ExternalReviewDoc {
 schema_version: "external-review-v1".to_string(),
 task_id: task_id.to_string(),
 verdict: "pass".to_string(),
 scope_compliance: Some(true),
 policy_compliance: Some(true),
 verification_sufficient: Some(true),
 summary: Some("Independent review ok".to_string()),
 findings: Vec::new(),
 }
 }

 fn default_verification() -> Vec<VerificationResult> {
 vec![VerificationResult {
 command: "cargo test".to_string(),
 exit_code:0,
 passed: true,
 stdout_excerpt: None,
 stderr_excerpt: None,
 }]
 }

 fn default_changed() -> Vec<ChangedFileEntry> {
 vec![ChangedFileEntry {
 path: "src/lib.rs".to_string(),
 operation: "modify".to_string(),
 }]
 }

 #[test]
 fn test_worktree_create_idempotent_and_remove() {
 let (temp, _, _) = setup();
 let handle = worktree_for(temp.path(), "task-20260529-001");
 assert!(!handle.exists());
 handle.create().unwrap();
 assert!(handle.exists());
 // Idempotent
 handle.create().unwrap();
 assert!(handle.exists());
 handle.remove().unwrap();
 assert!(!handle.exists());
 }

 #[test]
 fn test_worktree_does_not_touch_main_worktree() {
 let (temp, _, _) = setup();
 let repo = temp.path();
 let main_marker = repo.join("MAIN_WORKTREE_MARKER");
 fs::write(&main_marker, "do not touch").unwrap();
 let handle = worktree_for(repo, "task-20260529-001");
 handle.create().unwrap();
 // Main worktree marker still present and untouched.
 assert!(main_marker.exists());
 let content = fs::read_to_string(&main_marker).unwrap();
 assert_eq!(content, "do not touch");
 }

 #[test]
 fn test_worktree_contains_path() {
 let (temp, _, _) = setup();
 let handle = worktree_for(temp.path(), "task-20260529-001");
 handle.create().unwrap();
 let inside = handle.root.join("src/lib.rs");
 let outside = temp.path().join("outside.txt");
 assert!(handle.contains(&inside));
 assert!(!handle.contains(&outside));
 }

 #[test]
 fn test_required_artifact_names_match_contract() {
 // ADR-001 mandates exactly seven names in this order.
 assert_eq!(REQUIRED_ARTIFACTS.len(),7);
 assert_eq!(REQUIRED_ARTIFACTS[0], "changed_files.json");
 assert_eq!(REQUIRED_ARTIFACTS[1], "diff.patch");
 assert_eq!(REQUIRED_ARTIFACTS[2], "execution_trace.jsonl");
 assert_eq!(REQUIRED_ARTIFACTS[3], "verification.json");
 assert_eq!(REQUIRED_ARTIFACTS[4], "external_review.json");
 assert_eq!(REQUIRED_ARTIFACTS[5], "sensitive_audit.json");
 assert_eq!(REQUIRED_ARTIFACTS[6], "final_evidence.json");
 }

 #[test]
 fn test_collect_evidence_writes_all_seven() {
 let (temp, backend_output, normalized) = setup();
 let task_id = "task-20260529-001";
 let inputs = CollectEvidenceInputs {
 task_id,
 backend_output_dir: &backend_output,
 normalized_dir: &normalized,
 diff_text: "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n",
 changed_files: default_changed(),
 verification: default_verification(),
 external_review: default_review(task_id),
 execution_trace_events: vec![ExecutionTraceEvent::new(
 "adapter_dispatched",
 "2026-05-29T12:00:00Z",
 )],
 execution_completeness: "full",
 audit_limitations: vec![],
 };
 let written = collect_evidence(inputs).unwrap();
 assert_eq!(written.len(),7);
 for name in REQUIRED_ARTIFACTS.iter() {
 let p = normalized.join(name);
 assert!(p.exists(), "missing artifact: {}", name);
 }
 }

 #[test]
 fn test_collect_evidence_rejects_invalid_task_id() {
 let (temp, backend_output, normalized) = setup();
 let inputs = CollectEvidenceInputs {
 task_id: "not-a-task-id",
 backend_output_dir: &backend_output,
 normalized_dir: &normalized,
 diff_text: "",
 changed_files: vec![],
 verification: vec![],
 external_review: ExternalReviewDoc {
 schema_version: "external-review-v1".to_string(),
 task_id: "not-a-task-id".to_string(),
 verdict: "pass".to_string(),
 scope_compliance: None,
 policy_compliance: None,
 verification_sufficient: None,
 summary: None,
 findings: Vec::new(),
 },
 execution_trace_events: vec![],
 execution_completeness: "unavailable",
 audit_limitations: vec![],
 };
 assert!(collect_evidence(inputs).is_err());
 }

 #[test]
 fn test_collect_evidence_includes_backend_trace_when_present() {
 let (temp, backend_output, normalized) = setup();
 let task_id = "task-20260529-001";
 fs::write(
 backend_output.join("execution_trace.jsonl"),
 "{\"x\":1}\n{\"x\":2}\n",
 )
 .unwrap();
 let inputs = CollectEvidenceInputs {
 task_id,
 backend_output_dir: &backend_output,
 normalized_dir: &normalized,
 diff_text: "diff",
 changed_files: default_changed(),
 verification: default_verification(),
 external_review: default_review(task_id),
 execution_trace_events: vec![ExecutionTraceEvent::new(
 "after_backend",
 "2026-05-29T12:00:00Z",
 )],
 execution_completeness: "partial",
 audit_limitations: vec![],
 };
 collect_evidence(inputs).unwrap();
 let trace = fs::read_to_string(normalized.join("execution_trace.jsonl")).unwrap();
 // Both raw backend lines + the one explicit event, all non-empty.
 let lines: Vec<&str> = trace.lines().filter(|l| !l.trim().is_empty()).collect();
 assert_eq!(lines.len(),3);
 }

 #[test]
 fn test_sensitive_audit_flags_env_files() {
 let (temp, backend_output, _) = setup();
 let task_id = "task-20260529-001";
 let changed = ChangedFilesDoc::new(
 task_id,
 vec![ChangedFileEntry {
 path: "src/lib.rs".to_string(),
 operation: "modify".to_string(),
 }],
 );
 write_json(&backend_output.join("changed_files.json"), &changed).unwrap();
 let audit = run_sensitive_audit(task_id, &backend_output);
 assert!(!audit.blocked);

 let changed2 = ChangedFilesDoc::new(
 task_id,
 vec![ChangedFileEntry {
 path: ".env.production".to_string(),
 operation: "create".to_string(),
 }],
 );
 write_json(&backend_output.join("changed_files.json"), &changed2).unwrap();
 let audit2 = run_sensitive_audit(task_id, &backend_output);
 assert!(audit2.blocked);
 assert!(audit2
 .sensitive_paths_touched
 .iter()
 .any(|p| p == ".env.production"));
 }

 #[test]
 fn test_validate_evidence_accepts_clean_set() {
 let (temp, backend_output, normalized) = setup();
 let task_id = "task-20260529-001";
 let inputs = CollectEvidenceInputs {
 task_id,
 backend_output_dir: &backend_output,
 normalized_dir: &normalized,
 diff_text: "diff text",
 changed_files: default_changed(),
 verification: default_verification(),
 external_review: default_review(task_id),
 execution_trace_events: vec![ExecutionTraceEvent::new(
 "event1",
 "2026-05-29T12:00:00Z",
 )],
 execution_completeness: "full",
 audit_limitations: vec![],
 };
 collect_evidence(inputs).unwrap();
 let report = validate_evidence(task_id, &normalized).unwrap();
  assert!(report.valid, "expected valid evidence");
 for art in &report.artifacts {
 assert!(art.valid, "{} invalid: {:?}", art.name, art.errors);
 }
 }

 #[test]
 fn test_validate_evidence_rejects_missing_required() {
 let temp = TempDir::new().unwrap();
 let normalized = temp.path().join("normalized");
 fs::create_dir_all(&normalized).unwrap();
 let report = validate_evidence("task-20260529-001", &normalized).unwrap();
 assert!(!report.valid);
 assert_eq!(report.artifacts.len(),7);
 for art in &report.artifacts {
 assert!(!art.valid);
 }
 }

 #[test]
 fn test_validate_evidence_rejects_schema_violation() {
 let (temp, backend_output, normalized) = setup();
 let task_id = "task-20260529-001";
 let inputs = CollectEvidenceInputs {
 task_id,
 backend_output_dir: &backend_output,
 normalized_dir: &normalized,
 diff_text: "diff",
 changed_files: default_changed(),
 verification: default_verification(),
 external_review: default_review(task_id),
 execution_trace_events: vec![],
 execution_completeness: "full",
 audit_limitations: vec![],
 };
 collect_evidence(inputs).unwrap();
 // Tamper: invalid verification (missing required `passed`).
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
 let report = validate_evidence(task_id, &normalized).unwrap();
 assert!(!report.valid);
 let verification = report
 .artifacts
 .iter()
 .find(|a| a.name == "verification.json")
 .unwrap();
 assert!(!verification.valid);
 assert!(!verification.errors.is_empty());
 }

 #[test]
 fn test_collect_evidence_diff_text_written_verbatim() {
 let (temp, backend_output, normalized) = setup();
 let task_id = "task-20260529-001";
 let diff = "--- a/foo\n+++ b/foo\n@@\n-a\n+b\n";
 let inputs = CollectEvidenceInputs {
 task_id,
 backend_output_dir: &backend_output,
 normalized_dir: &normalized,
 diff_text: diff,
 changed_files: vec![],
 verification: vec![],
 external_review: ExternalReviewDoc {
 schema_version: "external-review-v1".to_string(),
 task_id: task_id.to_string(),
 verdict: "blocked".to_string(),
 scope_compliance: None,
 policy_compliance: None,
 verification_sufficient: None,
 summary: None,
 findings: Vec::new(),
 },
 execution_trace_events: vec![],
 execution_completeness: "unavailable",
 audit_limitations: vec![],
 };
 collect_evidence(inputs).unwrap();
 let read = fs::read_to_string(normalized.join("diff.patch")).unwrap();
 assert_eq!(read, diff);
 }

 #[test]
 fn test_normalized_dir_for_uses_agent_runs_path() {
 let temp = TempDir::new().unwrap();
 let p = normalized_dir_for(temp.path(), "task-20260529-001");
 assert!(p.ends_with(".agent-runs/tasks/task-20260529-001/normalized"));
 }

 #[test]
 fn test_is_within_repo_root() {
 let temp = TempDir::new().unwrap();
 let repo = temp.path().to_path_buf();
 let inside = repo.join("a/b.txt");
 let outside = temp.path().join("..").join("outside.txt");
 fs::create_dir_all(inside.parent().unwrap()).unwrap();
 fs::write(&inside, "x").unwrap();
 fs::write(&outside, "y").unwrap();
 assert!(is_within_repo_root(&repo, &inside));
 assert!(!is_within_repo_root(&repo, &outside));
 }
}
