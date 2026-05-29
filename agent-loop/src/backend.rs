//! External execution backend trait and Phase1 mock implementation.
//!
//! The Rust CLI does NOT orchestrate backends directly. It owns:
//! - the trait (`Backend`) that describes the contract between adapter and
//! backend;
//! - the mock backend (`MockBackend`) that produces deterministic raw
//! outputs for tests and `/agent-run` demonstrations;
//! - worktree path conventions.
//!
//! A concrete `omp` backend is OPTIONAL and only added if an existing
//! config supports it. Phase1 frozen scope: provisional backend is `omp`,
//! but adapter is responsible for invoking it; the CLI does not depend on
//! `omp` being installed.

use crate::artifacts::AgentRunsPaths;
use crate::evidence::ChangedFileEntry;
use crate::id::TaskId;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Outcome returned by a backend after it executes inside the worktree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendOutcome {
 pub backend: String,
 pub worktree: PathBuf,
 pub changed_files: Vec<ChangedFileEntry>,
 pub verification: Vec<VerificationEntry>,
 pub review_verdict: String, // pass | fail | blocked
 pub execution_completeness: String, // full | partial | unavailable
 pub execution_trace_lines: Vec<String>,
 pub raw_diff: String,
 pub raw_changed_files_path: Option<PathBuf>,
 pub audit_limitations: Vec<String>,
}

/// One verification entry reported by the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationEntry {
 pub command: String,
 pub exit_code: i64,
 pub passed: bool,
}

/// Backend trait. Implementations MUST:
/// - write outputs under `output_dir` only (the worktree or `backend-output`);
/// - never modify the main worktree;
/// - never commit, push, or merge;
/// - never fabricate outputs when execution failed — they should return
/// `BackendOutcome` with `review_verdict="blocked"` or `execution_completeness="unavailable"`.
pub trait Backend {
 fn name(&self) -> &str;
 fn run(&self, task_id: &TaskId, output_dir: &Path) -> BackendOutcome;
}

/// Mock backend for Phase1. Deterministic: produces a fixed diff, one
/// passing verification, and a configurable review verdict. Used by tests
/// and by the `/agent-run` demonstration path.
#[derive(Debug, Clone)]
pub struct MockBackend {
 pub verdict: String,
 pub execution_completeness: String,
 pub changed_files: Vec<ChangedFileEntry>,
 pub verification: Vec<VerificationEntry>,
 pub diff_text: String,
 pub trace_lines: Vec<String>,
 pub audit_limitations: Vec<String>,
}

impl Default for MockBackend {
 fn default() -> Self {
 Self {
 verdict: "pass".to_string(),
 execution_completeness: "full".to_string(),
 changed_files: vec![ChangedFileEntry {
 path: "src/lib.rs".to_string(),
 operation: "modify".to_string(),
 }],
 verification: vec![VerificationEntry {
 command: "cargo test".to_string(),
 exit_code:0,
 passed: true,
 }],
 diff_text: "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n"
 .to_string(),
 trace_lines: vec![],
 audit_limitations: vec![],
 }
 }
}

impl MockBackend {
 pub fn new() -> Self {
 Self::default()
 }

 pub fn with_verdict(mut self, verdict: &str) -> Self {
 self.verdict = verdict.to_string();
 self
 }

 pub fn with_changed(mut self, files: Vec<ChangedFileEntry>) -> Self {
 self.changed_files = files;
 self
 }

 pub fn with_diff(mut self, diff: &str) -> Self {
 self.diff_text = diff.to_string();
 self
 }

 pub fn with_trace(mut self, lines: Vec<String>) -> Self {
 self.trace_lines = lines;
 self
 }
}

impl Backend for MockBackend {
 fn name(&self) -> &str {
 "mock"
 }

 fn run(&self, task_id: &TaskId, output_dir: &Path) -> BackendOutcome {
 fs::create_dir_all(output_dir).ok();

 // Write raw changed_files.json for the audit detector to read.
 let raw_changed = serde_json::json!({
 "schema_version": "changed-files-v1",
 "task_id": task_id.to_string(),
 "files": self.changed_files,
 });
 let raw_changed_path = output_dir.join("changed_files.json");
 let _ = fs::write(
 &raw_changed_path,
 serde_json::to_string_pretty(&raw_changed).unwrap(),
 );

 // Write raw execution_trace.jsonl if provided.
 if !self.trace_lines.is_empty() {
 let _ = fs::write(
 output_dir.join("execution_trace.jsonl"),
 self.trace_lines.join("\n") + "\n",
 );
 }

 // Write raw diff.patch if provided.
 if !self.diff_text.is_empty() {
 let _ = fs::write(output_dir.join("diff.patch"), &self.diff_text);
 }

 BackendOutcome {
 backend: self.name().to_string(),
 worktree: output_dir.to_path_buf(),
 changed_files: self.changed_files.clone(),
 verification: self.verification.clone(),
 review_verdict: self.verdict.clone(),
 execution_completeness: self.execution_completeness.clone(),
 execution_trace_lines: self.trace_lines.clone(),
 raw_diff: self.diff_text.clone(),
 raw_changed_files_path: Some(raw_changed_path),
 audit_limitations: self.audit_limitations.clone(),
 }
 }
}

/// Stub for an optional `omp` backend. Phase1 ADR mentions `omp` as
/// provisional; the actual invocation is performed by the adapter (a
/// separate process). This struct exists so that the CLI can reference
/// `omp` by name without depending on it being installed.
#[derive(Debug, Clone, Default)]
pub struct OmpBackend;

impl Backend for OmpBackend {
 fn name(&self) -> &str {
 "omp"
 }

 fn run(&self, _task_id: &TaskId, _output_dir: &Path) -> BackendOutcome {
 // Phase1 invariant: the Rust CLI does NOT shell out to `omp`.
 // The adapter (a separate Claude Code subagent) is responsible for
 // invoking `omp` inside the worktree. If the adapter did not run
 // `omp`, this branch is unreachable; if it did, the raw outputs are
 // passed back to `collect_evidence` via `BackendOutcome`.
 BackendOutcome {
 backend: self.name().to_string(),
 worktree: PathBuf::new(),
 changed_files: Vec::new(),
 verification: Vec::new(),
 review_verdict: "blocked".to_string(),
 execution_completeness: "unavailable".to_string(),
 execution_trace_lines: Vec::new(),
 raw_diff: String::new(),
 raw_changed_files_path: None,
 audit_limitations: vec![
 "omp backend is a stub; invocation is owned by the adapter".to_string(),
 ],
 }
 }
}

/// Convenience: resolve the backend-output dir for a task.
pub fn backend_output_dir_for(repo_root: &Path, task_id: &str) -> PathBuf {
 AgentRunsPaths::new(repo_root).backend_output_dir(task_id)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
 use super::*;
 use tempfile::TempDir;

 fn setup() -> (TempDir, PathBuf) {
 let temp = TempDir::new().unwrap();
 let repo = temp.path().to_path_buf();
 let backend_output = repo.join("backend-output");
 fs::create_dir_all(&backend_output).unwrap();
 (temp, backend_output)
 }

 #[test]
 fn test_mock_backend_writes_raw_changed_files() {
 let (temp, output) = setup();
 let task = TaskId::parse("task-20260529-001").unwrap();
 let backend = MockBackend::new();
 let outcome = backend.run(&task, &output);
 assert_eq!(outcome.backend, "mock");
 assert_eq!(outcome.review_verdict, "pass");
 let raw = output.join("changed_files.json");
 assert!(raw.exists());
 let content = fs::read_to_string(&raw).unwrap();
 let v: serde_json::Value = serde_json::from_str(&content).unwrap();
 assert_eq!(v["task_id"], "task-20260529-001");
 assert_eq!(v["files"][0]["path"], "src/lib.rs");
 assert_eq!(v["files"][0]["operation"], "modify");
 }

 #[test]
 fn test_mock_backend_never_writes_outside_output_dir() {
 let (temp, output) = setup();
 let outside = temp.path().join("outside.txt");
 let task = TaskId::parse("task-20260529-001").unwrap();
 let backend = MockBackend::new();
 let _ = backend.run(&task, &output);
 assert!(!outside.exists());
 }

 #[test]
 fn test_omp_backend_is_a_stub() {
 let (temp, output) = setup();
 let task = TaskId::parse("task-20260529-001").unwrap();
 let backend = OmpBackend;
 let outcome = backend.run(&task, &output);
 assert_eq!(outcome.backend, "omp");
 assert_eq!(outcome.review_verdict, "blocked");
 assert_eq!(outcome.execution_completeness, "unavailable");
 assert!(outcome.audit_limitations.iter().any(|l| l.contains("stub")));
 }

 #[test]
 fn test_mock_backend_configurable_verdict() {
 let (temp, output) = setup();
 let task = TaskId::parse("task-20260529-001").unwrap();
 let backend = MockBackend::new().with_verdict("fail");
 let outcome = backend.run(&task, &output);
 assert_eq!(outcome.review_verdict, "fail");
 }

 #[test]
 fn test_backend_output_dir_for() {
 let temp = TempDir::new().unwrap();
 let p = backend_output_dir_for(temp.path(), "task-20260529-001");
 assert!(p.ends_with(".agent-runs/tasks/task-20260529-001/backend-output"));
 }
}
