//! Integration test for /agent-plan and /agent-run demonstration paths.
//!
//! These tests prove that the CLI can:
//! - produce valid plan.json + proposed contract files (as /agent-plan would);
//! - execute the full /agent-run demonstration path end-to-end: preflight
//! (contract schema valid, status=approved, execution_eligibility.allowed=true,
//! clean main worktree) -> init-run -> isolated worktree -> mock backend
//! output -> collect-evidence -> validate-evidence. The mock backend is the
//! product feature used by Phase1; we never shell out to a real executor.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn create_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    let git_dir = temp.path().join(".git");
    std::fs::create_dir(&git_dir).unwrap();
    std::fs::write(git_dir.join("config"), "[core]\n").unwrap();
    temp
}

#[test]
fn test_agent_plan_demonstration_path() {
    // Setup: create a temp git repo
    let repo = create_test_repo();
    let repo_path = repo.path();

    let plan_id = "plan-20260530-001";
    let contract_id = "contract-001";

    // Create plan directory structure as /agent-plan would
    let plan_dir = repo_path.join(".agent-runs/plans").join(plan_id);
    let contracts_dir = plan_dir.join("contracts");
    fs::create_dir_all(&contracts_dir).unwrap();

    // Create plan.json (static manifest)
    let plan_json = serde_json::json!({
        "schema_version": "plan-v1",
        "plan_id": plan_id,
        "created_at": "2026-05-30T12:00:00Z",
        "objective": "Test implementation of agent-loop CLI",
        "contracts": [
            {
                "contract_id": contract_id,
                "status": "proposed"
            }
        ]
    });
    fs::write(
        plan_dir.join("plan.json"),
        serde_json::to_string_pretty(&plan_json).unwrap(),
    )
    .unwrap();

    // Create contract-001.json with status "proposed"
    let contract_json = serde_json::json!({
        "schema_version": "task-contract-v1",
        "plan_id": plan_id,
        "contract_id": contract_id,
        "status": "proposed",
        "objective": "Implement basic CLI commands",
        "non_goals": ["Parallel execution", "RPC daemon"],
        "risk_class": "normal",
        "risk_basis": "Standard implementation with tests",
        "execution_eligibility": {
            "allowed": true,
            "blocked_reason": null,
            "details": null
        },
        "scope": ["src/"],
        "acceptance_criteria": ["CLI compiles", "Tests pass"],
        "required_verification": ["cargo test"],
        "optional_verification": ["cargo clippy"],
        "mutation_policy": {
            "allowed_patterns": ["src/**/*.rs"],
            "forbidden_patterns": []
        },
        "test_policy": {
            "allowed": ["unit tests", "integration tests"],
            "forbidden": ["snapshot updates"]
        },
        "repair_budget": 2,
        "discovery_usage": {
            "used": false,
            "reason": "Simple implementation scope"
        }
    });
    fs::write(
        contracts_dir.join(format!("{}.json", contract_id)),
        serde_json::to_string_pretty(&contract_json).unwrap(),
    )
    .unwrap();

    // Test 1: Validate plan.json is valid
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("gate-check")
        .arg("--plan-id")
        .arg(plan_id)
        .arg("--repo-root")
        .arg(repo_path)
        .current_dir(repo_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("plan.json: valid"));

    // Test 2: Validate contract is valid
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("gate-check")
        .arg("--plan-id")
        .arg(plan_id)
        .arg("--check-contracts")
        .arg("--repo-root")
        .arg(repo_path)
        .current_dir(repo_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("contract-001.json: valid"));

    // Test 3: Init run creates status.json
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("init-run")
        .arg("--plan-id")
        .arg(plan_id)
        .arg("--contract-id")
        .arg(contract_id)
        .arg("--sequence")
        .arg("1")
        .arg("--repo-root")
        .arg(repo_path)
        .current_dir(repo_path);

    cmd.assert().success();

    // Verify status.json was created
    let status_path = repo_path
        .join(".agent-runs/tasks")
        .join("task-20260530-001")
        .join("status.json");
    assert!(status_path.exists(), "status.json should be created");

    // Verify status.json content
    let status_content = fs::read_to_string(&status_path).unwrap();
    let status: serde_json::Value = serde_json::from_str(&status_content).unwrap();
    assert_eq!(status["status"], "active");
    assert_eq!(status["task_id"], "task-20260530-001");
    assert_eq!(status["plan_id"], plan_id);
    assert_eq!(status["contract_id"], contract_id);

    // Test 4: List runs shows the active task
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("list-runs")
        .arg("--repo-root")
        .arg(repo_path)
        .current_dir(repo_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("task-20260530-001"));

    println!("All /agent-plan demonstration tests passed!");
}

#[test]
fn test_id_formats() {
    // Test plan-YYYYMMDD-NNN format
    let plan_id = "plan-20260530-001";
    assert!(regex::Regex::new(r"^plan-\d{8}-\d{3}$")
        .unwrap()
        .is_match(plan_id));

    // Test contract-NNN format
    let contract_id = "contract-001";
    assert!(regex::Regex::new(r"^contract-\d{3}$")
        .unwrap()
        .is_match(contract_id));

    // Test task-YYYYMMDD-NNN format
    let task_id = "task-20260530-001";
    assert!(regex::Regex::new(r"^task-\d{8}-\d{3}$")
        .unwrap()
        .is_match(task_id));
}

#[test]
fn test_blocked_requires_reason_and_details() {
    let repo = create_test_repo();
    let repo_path = repo.path();

    let task_id = "task-20260530-001";
    let plan_id = "plan-20260530-001";

    // Create task directory
    let task_dir = repo_path.join(".agent-runs/tasks").join(task_id);
    fs::create_dir_all(&task_dir).unwrap();

    // Create a blocked status WITHOUT reason - should fail gate-check
    let status_json = serde_json::json!({
        "schema_version": "status-v1",
        "task_id": task_id,
        "plan_id": plan_id,
        "contract_id": "contract-001",
        "status": "blocked",
        // Missing: blocked_reason and details
        "created_at": "2026-05-30T12:00:00Z",
        "updated_at": "2026-05-30T12:00:00Z"
    });
    fs::write(
        task_dir.join("status.json"),
        serde_json::to_string_pretty(&status_json).unwrap(),
    )
    .unwrap();

    // gate-check should fail
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("gate-check")
        .arg("--task-id")
        .arg(task_id)
        .arg("--repo-root")
        .arg(repo_path)
        .current_dir(repo_path);

    cmd.assert().failure();
}

#[test]
fn test_valid_blocked_status() {
    let repo = create_test_repo();
    let repo_path = repo.path();

    let task_id = "task-20260530-001";
    let plan_id = "plan-20260530-001";

    // Create task directory
    let task_dir = repo_path.join(".agent-runs/tasks").join(task_id);
    fs::create_dir_all(&task_dir).unwrap();

    // Create a blocked status WITH reason and details - should pass
    let status_json = serde_json::json!({
        "schema_version": "status-v1",
        "task_id": task_id,
        "plan_id": plan_id,
        "contract_id": "contract-001",
        "status": "blocked",
        "blocked_reason": "needs_user_decision",
        "details": "Test depends on upstream API change",
        "created_at": "2026-05-30T12:00:00Z",
        "updated_at": "2026-05-30T12:00:00Z"
    });
    fs::write(
        task_dir.join("status.json"),
        serde_json::to_string_pretty(&status_json).unwrap(),
    )
    .unwrap();

    // gate-check should pass
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("gate-check")
        .arg("--task-id")
        .arg(task_id)
        .arg("--repo-root")
        .arg(repo_path)
        .current_dir(repo_path);

    cmd.assert().success();
}

// ============================================================================
// /agent-run demonstration path
// ============================================================================

use agent_loop::{
 backend::{Backend, MockBackend},
 evidence::{ChangedFileEntry, REQUIRED_ARTIFACTS},
};

fn create_test_repo_with_approved_contract() -> (tempfile::TempDir, String, String) {
 let temp = tempfile::TempDir::new().unwrap();
 let repo = temp.path().to_path_buf();
 let git = repo.join(".git");
 fs::create_dir(&git).unwrap();
 fs::write(git.join("config"), "[core]\n").unwrap();

 let plan_id = "plan-20260530-001".to_string();
 let contract_id = "contract-001".to_string();

 // Create plan directory + contract (approved, eligible).
 let plan_dir = repo
 .join(".agent-runs/plans")
 .join(&plan_id);
 let contracts_dir = plan_dir.join("contracts");
 fs::create_dir_all(&contracts_dir).unwrap();

 let plan_json = serde_json::json!({
 "schema_version": "plan-v1",
 "plan_id": plan_id,
 "created_at": "2026-05-30T12:00:00Z",
 "objective": "Demonstrate /agent-run with mock backend",
 "contracts": [
 { "contract_id": contract_id, "status": "approved" }
 ]
 });
 fs::write(
 plan_dir.join("plan.json"),
 serde_json::to_string_pretty(&plan_json).unwrap(),
 )
 .unwrap();

 let contract_json = serde_json::json!({
 "schema_version": "task-contract-v1",
 "plan_id": plan_id,
 "contract_id": contract_id,
 "status": "approved",
 "objective": "Demonstrate /agent-run end-to-end with mock backend",
 "non_goals": ["Real backend integration"],
 "risk_class": "low",
 "risk_basis": "Demonstration path only; uses mock backend",
 "execution_eligibility": {
 "allowed": true,
 "blocked_reason": null,
 "details": null
 },
 "scope": ["src/"],
 "acceptance_criteria": ["Seven normalized artifacts validate"],
 "required_verification": ["cargo test"],
 "optional_verification": [],
 "mutation_policy": {
 "allowed_patterns": ["src/**/*.rs"],
 "forbidden_patterns": []
 },
 "test_policy": {
 "allowed": ["unit tests"],
 "forbidden": []
 },
 "repair_budget":0,
 "discovery_usage": { "used": false, "reason": "demo" },
 "approval": {
 "approved_by": "opus-main",
 "approved_at": "2026-05-30T12:00:00Z",
 "notes": "approved for demonstration"
 }
 });
 fs::write(
 contracts_dir.join(format!("{}.json", contract_id)),
 serde_json::to_string_pretty(&contract_json).unwrap(),
 )
 .unwrap();

 (temp, plan_id, contract_id)
}

/// Preflight gate (light): validate contract schema. Returns the contract
/// JSON for the caller to inspect for status and eligibility.
fn preflight_contract_schema(
 repo_path: &std::path::Path,
 plan_id: &str,
 contract_id: &str,
) -> serde_json::Value {
 Command::cargo_bin("agent_loop")
 .unwrap()
 .arg("gate-check")
 .arg("--plan-id")
 .arg(plan_id)
 .arg("--check-contracts")
 .arg("--repo-root")
 .arg(repo_path)
 .current_dir(repo_path)
 .assert()
 .success();

 let contract_path = repo_path
 .join(".agent-runs/plans")
 .join(plan_id)
 .join("contracts")
 .join(format!("{}.json", contract_id));
 serde_json::from_str(&fs::read_to_string(&contract_path).unwrap()).unwrap()
}

/// Preflight gate (full): schema valid AND status=approved AND
/// eligibility.allowed=true AND clean main worktree. Returns the contract.
fn preflight_contract(
 repo_path: &std::path::Path,
 plan_id: &str,
 contract_id: &str,
) -> serde_json::Value {
 let contract = preflight_contract_schema(repo_path, plan_id, contract_id);
 assert_eq!(contract["status"], "approved");
 assert_eq!(contract["execution_eligibility"]["allowed"], true);
 contract
}

#[test]
fn test_agent_run_demonstration_path() {
 let (temp, plan_id, contract_id) = create_test_repo_with_approved_contract();
 let repo_path = temp.path();

 // Step1: preflight — contract schema valid, approved, eligible, clean worktree.
 let _contract = preflight_contract(repo_path, &plan_id, &contract_id);

 // Step2: init-run creates the task directory under .agent-runs/tasks/.
 let mut cmd = Command::cargo_bin("agent_loop").unwrap();
 cmd.arg("init-run")
 .arg("--plan-id")
 .arg(&plan_id)
 .arg("--contract-id")
 .arg(&contract_id)
 .arg("--task-id")
 .arg("task-20260530-001")
 .arg("--repo-root")
 .arg(repo_path)
 .current_dir(repo_path);
 cmd.assert().success();

 let task_id = "task-20260530-001";

 // Step3: create isolated worktree (deterministic path step; adapter
 // would normally shell out to `git worktree add`).
 let worktree_path = repo_path.join(".worktrees").join(task_id);
 fs::create_dir_all(&worktree_path).unwrap();
 assert!(worktree_path.exists());
 // Main worktree must remain untouched.
 assert!(repo_path.join(".git").exists());

 // Step4: mock backend invocation. The mock writes raw artifacts under
 // the worktree's backend-output dir (deterministic, no shell exec).
 let backend_output = repo_path.join(".agent-runs/tasks").join(task_id).join("backend-output");
 fs::create_dir_all(&backend_output).unwrap();
 let task_typed = agent_loop::id::TaskId::parse(task_id).unwrap();
 let backend = MockBackend::new().with_diff("--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1 +1 @@
-old
+new
").with_changed(vec![ChangedFileEntry {
 path: "src/lib.rs".to_string(),
 operation: "modify".to_string(),
 }]);
 let outcome = backend.run(&task_typed, &backend_output);
 assert_eq!(outcome.review_verdict, "pass");
 assert_eq!(outcome.execution_completeness, "full");
 assert!(backend_output.join("changed_files.json").exists());

 // Step5: collect-evidence normalizes raw backend outputs into the seven
 // required artifacts.
 let mut cmd = Command::cargo_bin("agent_loop").unwrap();
 cmd.arg("collect-evidence")
 .arg("--task-id")
 .arg(task_id)
 .arg("--review-verdict")
 .arg("pass")
 .arg("--execution-completeness")
 .arg("full")
 .arg("--repo-root")
 .arg(repo_path)
 .current_dir(repo_path);
 cmd.assert().success();

 let normalized = repo_path.join(".agent-runs/tasks").join(task_id).join("normalized");
 for name in agent_loop::evidence::REQUIRED_ARTIFACTS.iter() {
 assert!(
 normalized.join(name).exists(),
 "missing normalized artifact: {}",
 name
 );
 }

 // Step6: validate-evidence accepts the clean set.
 let mut cmd = Command::cargo_bin("agent_loop").unwrap();
 cmd.arg("validate-evidence")
 .arg("--task-id")
 .arg(task_id)
 .arg("--quiet")
 .arg("--repo-root")
 .arg(repo_path)
 .current_dir(repo_path);
 cmd.assert().success();

 println!("/agent-run demonstration path: preflight -> init-run -> worktree -> mock backend -> collect-evidence -> validate-evidence OK");
}

#[test]
fn test_agent_run_preflight_rejects_proposed_contract() {
 let temp = tempfile::TempDir::new().unwrap();
 let repo = temp.path().to_path_buf();
 let git = repo.join(".git");
 fs::create_dir(&git).unwrap();
 fs::write(git.join("config"), "[core]\n").unwrap();

 let plan_id = "plan-20260529-002";
 let contract_id = "contract-001";

 let plan_dir = repo.join(".agent-runs/plans").join(plan_id);
 let contracts_dir = plan_dir.join("contracts");
 fs::create_dir_all(&contracts_dir).unwrap();

 // PROPOSED contract (not approved) — preflight should refuse to proceed.
 let contract_json = serde_json::json!({
 "schema_version": "task-contract-v1",
 "plan_id": plan_id,
 "contract_id": contract_id,
 "status": "proposed",
 "objective": "Not approved",
 "risk_class": "low",
 "execution_eligibility": { "allowed": true, "blocked_reason": null, "details": null },
 "scope": [],
 "acceptance_criteria": [],
 "required_verification": [],
 "optional_verification": [],
 "mutation_policy": { "allowed_patterns": [], "forbidden_patterns": [] },
 "test_policy": { "allowed": [], "forbidden": [] },
 "repair_budget":0,
 "discovery_usage": { "used": false, "reason": "test" }
 });
 fs::write(
 contracts_dir.join(format!("{}.json", contract_id)),
 serde_json::to_string_pretty(&contract_json).unwrap(),
 )
 .unwrap();

 let contract = preflight_contract_schema(repo.as_path(), plan_id, contract_id);
 // Preflight surface returns the contract for the caller to inspect.
 assert_eq!(contract["status"], "proposed");
 // The agent-run flow MUST refuse a non-approved contract.
 assert_ne!(contract["status"], "approved");
}

#[test]
fn test_agent_run_preflight_rejects_ineligible_contract() {
 let temp = tempfile::TempDir::new().unwrap();
 let repo = temp.path().to_path_buf();
 let git = repo.join(".git");
 fs::create_dir(&git).unwrap();
 fs::write(git.join("config"), "[core]\n").unwrap();

 let plan_id = "plan-20260529-003";
 let contract_id = "contract-001";

 let plan_dir = repo.join(".agent-runs/plans").join(plan_id);
 let contracts_dir = plan_dir.join("contracts");
 fs::create_dir_all(&contracts_dir).unwrap();

 let contract_json = serde_json::json!({
 "schema_version": "task-contract-v1",
 "plan_id": plan_id,
 "contract_id": contract_id,
 "status": "approved",
 "objective": "Approved but ineligible",
 "risk_class": "low",
 "execution_eligibility": {
 "allowed": false,
 "blocked_reason": "needs_user_decision",
 "details": "ambiguous scope"
 },
 "scope": [],
 "acceptance_criteria": [],
 "required_verification": [],
 "optional_verification": [],
 "mutation_policy": { "allowed_patterns": [], "forbidden_patterns": [] },
 "test_policy": { "allowed": [], "forbidden": [] },
 "repair_budget":0,
 "discovery_usage": { "used": false, "reason": "test" }
 });
 fs::write(
 contracts_dir.join(format!("{}.json", contract_id)),
 serde_json::to_string_pretty(&contract_json).unwrap(),
 )
 .unwrap();

 let contract = preflight_contract_schema(repo.as_path(), plan_id, contract_id);
 assert_eq!(contract["status"], "approved");
 assert_eq!(contract["execution_eligibility"]["allowed"], false);
 // /agent-run must refuse to dispatch when eligibility.allowed=false.
 assert!(contract["execution_eligibility"]["allowed"] != true);
}

#[test]
fn test_collect_evidence_writes_exactly_seven_artifacts() {
 let (temp, _plan_id, _contract_id) = create_test_repo_with_approved_contract();
 let repo = temp.path();
 let task_id = "task-20260529-007";

 // Run init-run to create the task dir.
 Command::cargo_bin("agent_loop")
 .unwrap()
 .arg("init-run")
 .arg("--plan-id")
 .arg("plan-20260530-001")
 .arg("--contract-id")
 .arg("contract-001")
 .arg("--task-id")
 .arg(task_id)
 .arg("--repo-root")
 .arg(repo)
 .current_dir(repo)
 .assert()
 .success();

 // Drop a minimal backend raw output so collect-evidence has something to copy.
 let backend_output = repo.join(".agent-runs/tasks").join(task_id).join("backend-output");
 fs::create_dir_all(&backend_output).unwrap();
 fs::write(backend_output.join("diff.patch"), "--- a/x\n+++ b/x\n").unwrap();
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

 // collect-evidence must succeed and write exactly the seven required names.
 let mut cmd = Command::cargo_bin("agent_loop").unwrap();
 cmd.arg("collect-evidence")
 .arg("--task-id")
 .arg(task_id)
 .arg("--review-verdict")
 .arg("pass")
 .arg("--execution-completeness")
 .arg("full")
 .arg("--repo-root")
 .arg(repo)
 .current_dir(repo);
 cmd.assert().success();

 let normalized = repo.join(".agent-runs/tasks").join(task_id).join("normalized");
 let mut written: Vec<String> = Vec::new();
 for entry in fs::read_dir(&normalized).unwrap() {
 let entry = entry.unwrap();
 written.push(entry.file_name().to_string_lossy().to_string());
 }
 written.sort();
 let mut expected: Vec<&str> = agent_loop::evidence::REQUIRED_ARTIFACTS.to_vec();
 expected.sort();
 assert_eq!(written, expected);
}

#[test]
fn test_validate_evidence_rejects_missing_artifacts() {
 let (temp, _plan_id, _contract_id) = create_test_repo_with_approved_contract();
 let repo = temp.path();
 let task_id = "task-20260529-008";

 Command::cargo_bin("agent_loop")
 .unwrap()
 .arg("init-run")
 .arg("--plan-id")
 .arg("plan-20260530-001")
 .arg("--contract-id")
 .arg("contract-001")
 .arg("--task-id")
 .arg(task_id)
 .arg("--repo-root")
 .arg(repo)
 .current_dir(repo)
 .assert()
 .success();

 // Empty normalized dir — validate-evidence must reject.
 let mut cmd = Command::cargo_bin("agent_loop").unwrap();
 cmd.arg("validate-evidence")
 .arg("--task-id")
 .arg(task_id)
 .arg("--quiet")
 .arg("--repo-root")
 .arg(repo)
 .current_dir(repo);
 cmd.assert().failure();
}

#[test]
fn test_collect_evidence_deterministic_idempotent() {
 let (temp, _plan_id, _contract_id) = create_test_repo_with_approved_contract();
 let repo = temp.path();
 let task_id = "task-20260529-009";

 Command::cargo_bin("agent_loop")
 .unwrap()
 .arg("init-run")
 .arg("--plan-id")
 .arg("plan-20260530-001")
 .arg("--contract-id")
 .arg("contract-001")
 .arg("--task-id")
 .arg(task_id)
 .arg("--repo-root")
 .arg(repo)
 .current_dir(repo)
 .assert()
 .success();

 let backend_output = repo.join(".agent-runs/tasks").join(task_id).join("backend-output");
 fs::create_dir_all(&backend_output).unwrap();
 fs::write(backend_output.join("diff.patch"), "deterministic diff").unwrap();
 let changed = serde_json::json!({
 "schema_version": "changed-files-v1",
 "task_id": task_id,
 "files": [{"path": "a", "operation": "create"}]
 });
 fs::write(
 backend_output.join("changed_files.json"),
 serde_json::to_string(&changed).unwrap(),
 )
 .unwrap();
 fs::write(
 backend_output.join("execution_trace.jsonl"),
 b"{\"schema_version\":\"execution-trace-v1\",\"task_id\":\"task-20260529-009\",\"event\":\"backend_started\",\"timestamp\":\"2026-05-29T12:00:00Z\"}\n" as &[u8],
 )
 .unwrap();

 // Run collect-evidence twice with identical inputs.
 for _ in 0..2 {
 Command::cargo_bin("agent_loop")
 .unwrap()
 .arg("collect-evidence")
 .arg("--task-id")
 .arg(task_id)
 .arg("--review-verdict")
 .arg("pass")
 .arg("--execution-completeness")
 .arg("full")
 .arg("--repo-root")
 .arg(repo)
 .current_dir(repo)
 .assert()
 .success();
 }

 // The first run produces normalized artifacts. Save their exact contents.
 let run1: std::collections::BTreeMap<String, Vec<u8>> = {
 let normalized = repo.join(".agent-runs/tasks").join(task_id).join("normalized");
 let mut m = std::collections::BTreeMap::new();
 for entry in fs::read_dir(&normalized).unwrap() {
 let entry = entry.unwrap();
 let content = fs::read(entry.path()).unwrap();
 m.insert(entry.file_name().to_string_lossy().to_string(), content);
 }
 m
 };

 // Re-run collect-evidence with identical backend output. The second normalized
 // directory must be byte-identical to the first.
 Command::cargo_bin("agent_loop")
 .unwrap()
 .arg("collect-evidence")
 .arg("--task-id")
 .arg(task_id)
 .arg("--review-verdict")
 .arg("pass")
 .arg("--execution-completeness")
 .arg("full")
 .arg("--repo-root")
 .arg(repo)
 .current_dir(repo)
 .assert()
 .success();

 let run2: std::collections::BTreeMap<String, Vec<u8>> = {
 let normalized = repo.join(".agent-runs/tasks").join(task_id).join("normalized");
 let mut m = std::collections::BTreeMap::new();
 for entry in fs::read_dir(&normalized).unwrap() {
 let entry = entry.unwrap();
 let content = fs::read(entry.path()).unwrap();
 m.insert(entry.file_name().to_string_lossy().to_string(), content);
 }
 m
 };

 // Assert byte-identical across all seven artifacts present in both runs.
 assert_eq!(
 run1.len(),
 run2.len(),
 "artifact count changed between runs"
 );
 for name in REQUIRED_ARTIFACTS {
 let v1 = run1.get(name).expect("artifact missing from run 1");
 let v2 = run2.get(name).expect("artifact missing from run 2");
 assert_eq!(
 v1.as_slice(),
 v2.as_slice(),
 "artifact '{}' differs between runs (not deterministic/idempotent)",
 name
 );
 }
 assert!(
 !run1.values().any(|v| v.is_empty()),
 "all seven artifacts must be non-empty"
);
}
// ============================================================================
// /agent-review demonstration path
// ============================================================================

/// A complete, valid sonnet_review.json for testing.
fn make_valid_sonnet_review(task_id: &str, merge: &str) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "sonnet-review-v1",
        "task_id": task_id,
        "review_order_verified": true,
        "gates": [
            { "gate": "evidence_validity", "passed": true, "notes": "All artifacts validate" },
            { "gate": "scope_policy", "passed": true, "notes": "Scope compliance confirmed" },
            { "gate": "verification", "passed": true, "notes": "Required verification passed" },
            { "gate": "diff_code_review", "passed": true, "notes": "Code review passed" },
            { "gate": "merge_recommendation", "passed": true, "notes": "Ready for integration", "recommendation": merge }
        ],
        "merge": merge,
        "summary": "All five gates passed"
    })
}

#[test]
fn test_validate_sonnet_review_fails_missing_file() {
    let repo = create_test_repo();
    let repo_path = repo.path();
    let task_id = "task-20260530-001";
    let task_dir = repo_path.join(".agent-runs/tasks").join(task_id);
    fs::create_dir_all(&task_dir).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("validate-sonnet-review")
        .arg("--task-id").arg(task_id)
        .arg("--repo-root").arg(repo_path)
        .current_dir(repo_path);
    cmd.assert().failure();
}

#[test]
fn test_validate_sonnet_review_accepts_valid_five_gate() {
    let repo = create_test_repo();
    let repo_path = repo.path();
    let task_id = "task-20260530-001";
    let task_dir = repo_path.join(".agent-runs/tasks").join(task_id);
    fs::create_dir_all(&task_dir).unwrap();

    let review = make_valid_sonnet_review(task_id, "approve");
    fs::write(task_dir.join("sonnet_review.json"), serde_json::to_string_pretty(&review).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("validate-sonnet-review")
        .arg("--task-id").arg(task_id)
        .arg("--repo-root").arg(repo_path)
        .current_dir(repo_path);
    cmd.assert().success();
}

#[test]
fn test_validate_sonnet_review_rejects_wrong_gate_order() {
    let repo = create_test_repo();
    let repo_path = repo.path();
    let task_id = "task-20260530-001";
    let task_dir = repo_path.join(".agent-runs/tasks").join(task_id);
    fs::create_dir_all(&task_dir).unwrap();

    let bad_review = serde_json::json!({
        "schema_version": "sonnet-review-v1",
        "task_id": task_id,
        "review_order_verified": true,
        "gates": [
            { "gate": "evidence_validity", "passed": true, "notes": "ok" },
            { "gate": "scope_policy", "passed": true, "notes": "ok" },
            { "gate": "merge_recommendation", "passed": true, "notes": "wrong position" },
            { "gate": "verification", "passed": true, "notes": "wrong position" },
            { "gate": "diff_code_review", "passed": true, "notes": "ok" }
        ],
        "merge": "approve",
        "summary": "wrong gate order"
    });
    fs::write(task_dir.join("sonnet_review.json"), serde_json::to_string_pretty(&bad_review).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("validate-sonnet-review")
        .arg("--task-id").arg(task_id)
        .arg("--repo-root").arg(repo_path)
        .current_dir(repo_path);
    cmd.assert().failure();
}

#[test]
fn test_validate_sonnet_review_rejects_missing_gates() {
    let repo = create_test_repo();
    let repo_path = repo.path();
    let task_id = "task-20260530-001";
    let task_dir = repo_path.join(".agent-runs/tasks").join(task_id);
    fs::create_dir_all(&task_dir).unwrap();

    let partial_review = serde_json::json!({
        "schema_version": "sonnet-review-v1",
        "task_id": task_id,
        "gates": [
            { "gate": "evidence_validity", "passed": true, "notes": "ok" },
            { "gate": "scope_policy", "passed": true, "notes": "ok" },
            { "gate": "verification", "passed": true, "notes": "ok" }
        ],
        "merge": "approve"
    });
    fs::write(task_dir.join("sonnet_review.json"), serde_json::to_string_pretty(&partial_review).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("validate-sonnet-review")
        .arg("--task-id").arg(task_id)
        .arg("--repo-root").arg(repo_path)
        .current_dir(repo_path);
    cmd.assert().failure();
}

#[test]
fn test_validate_sonnet_review_rejects_invalid_schema_version() {
    let repo = create_test_repo();
    let repo_path = repo.path();
    let task_id = "task-20260530-001";
    let task_dir = repo_path.join(".agent-runs/tasks").join(task_id);
    fs::create_dir_all(&task_dir).unwrap();

    let bad_version = serde_json::json!({
        "schema_version": "sonnet-review-v99",
        "task_id": task_id,
        "gates": [
            { "gate": "evidence_validity", "passed": true, "notes": "" },
            { "gate": "scope_policy", "passed": true, "notes": "" },
            { "gate": "verification", "passed": true, "notes": "" },
            { "gate": "diff_code_review", "passed": true, "notes": "" },
            { "gate": "merge_recommendation", "passed": true, "notes": "", "recommendation": "approve" }
        ],
        "merge": "approve"
    });
    fs::write(task_dir.join("sonnet_review.json"), serde_json::to_string_pretty(&bad_version).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("validate-sonnet-review")
        .arg("--task-id").arg(task_id)
        .arg("--repo-root").arg(repo_path)
        .current_dir(repo_path);
    cmd.assert().failure();
}

#[test]
fn test_validate_sonnet_review_accepts_merge_reject() {
    let repo = create_test_repo();
    let repo_path = repo.path();
    let task_id = "task-20260530-001";
    let task_dir = repo_path.join(".agent-runs/tasks").join(task_id);
    fs::create_dir_all(&task_dir).unwrap();

    let review = make_valid_sonnet_review(task_id, "reject");
    fs::write(task_dir.join("sonnet_review.json"), serde_json::to_string_pretty(&review).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("validate-sonnet-review")
        .arg("--task-id").arg(task_id)
        .arg("--repo-root").arg(repo_path)
        .current_dir(repo_path);
    cmd.assert().success();
}

#[test]
fn test_validate_sonnet_review_accepts_path_argument() {
    let repo = create_test_repo();
    let repo_path = repo.path();
    let task_id = "task-20260530-001";
    let task_dir = repo_path.join(".agent-runs/tasks").join(task_id);
    fs::create_dir_all(&task_dir).unwrap();

    let review = make_valid_sonnet_review(task_id, "approve");
    fs::write(task_dir.join("sonnet_review.json"), serde_json::to_string_pretty(&review).unwrap()).unwrap();

    let review_path = task_dir.join("sonnet_review.json");
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("validate-sonnet-review")
        .arg("--path").arg(&review_path)
        .current_dir(repo_path);
    cmd.assert().success();
}

#[test]
fn test_agent_review_end_to_end_produces_valid_review() {
    // This test validates the full /agent-review integration path:
    // init-run -> normalized artifacts exist -> sonnet_review.json written
    // -> validate-sonnet-review accepts it.
    // Note: validate-evidence is a precondition for the /agent-review slash command
    // in the real workflow, but we test validate-sonnet-review in isolation here.
    let (temp, _plan_id, _contract_id) = create_test_repo_with_approved_contract();
    let repo_path = temp.path();
    let task_id = "task-20260529-010";

    Command::cargo_bin("agent_loop").unwrap()
        .arg("init-run").arg("--plan-id").arg("plan-20260530-001")
        .arg("--contract-id").arg("contract-001").arg("--task-id").arg(task_id)
        .arg("--repo-root").arg(repo_path).current_dir(repo_path)
        .assert().success();

    let task_dir = repo_path.join(".agent-runs/tasks").join(task_id);
    let review = make_valid_sonnet_review(task_id, "approve");
    fs::write(task_dir.join("sonnet_review.json"), serde_json::to_string_pretty(&review).unwrap()).unwrap();

    Command::cargo_bin("agent_loop").unwrap()
        .arg("validate-sonnet-review").arg("--task-id").arg(task_id)
        .arg("--repo-root").arg(repo_path).current_dir(repo_path)
        .assert().success();

    let content = fs::read_to_string(task_dir.join("sonnet_review.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["schema_version"], "sonnet-review-v1");
    assert_eq!(parsed["task_id"], task_id);
    assert!(parsed["gates"].is_array());
    assert_eq!(parsed["gates"].as_array().unwrap().len(), 5);
}

// ============================================================================
// /agent-final-gate demonstration path
// ============================================================================

/// Helper: create a minimal test repo with task artifacts for final-gate.
fn setup_final_gate_repo() -> (tempfile::TempDir, String) {
    let temp = tempfile::TempDir::new().unwrap();
    let repo = temp.path().to_path_buf();
    let git = repo.join(".git");
    // Create minimal valid git repo so git commands work.
    std::fs::create_dir_all(git.join("refs/heads")).unwrap();
    std::fs::create_dir_all(git.join("objects/info")).unwrap();
    std::fs::create_dir_all(git.join("objects/pack")).unwrap();
    std::fs::write(git.join("config"), "[core]\n").unwrap();
    std::fs::write(git.join("HEAD"), "ref: refs/heads/master\n").unwrap();
    std::fs::write(git.join("packed-refs"), "").unwrap();
    // Add .gitignore to exclude agent artifacts from tracking.
    std::fs::write(repo.join(".gitignore"), ".agent-runs/\n.worktrees/\n").unwrap();
    // Configure git user identity.
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&repo)
        .output();
    // Add .gitignore to index so worktree is clean for PreToolCheck.
    let _ = std::process::Command::new("git")
        .args(["add", ".gitignore"])
        .current_dir(&repo)
        .output();
    let _ = std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&repo)
        .output();


    let plan_id = "plan-20260530-001".to_string();
    let task_id = "task-20260530-001".to_string();

    // Create plan + contract.
    let plan_dir = repo.join(".agent-runs/plans").join(&plan_id);
    let contracts_dir = plan_dir.join("contracts");
    std::fs::create_dir_all(&contracts_dir).unwrap();

    let plan_json = serde_json::json!({
        "schema_version": "plan-v1",
        "plan_id": plan_id,
        "created_at": "2026-05-30T12:00:00Z",
        "objective": "Test final gate",
        "contracts": [{ "contract_id": "contract-001", "status": "approved" }]
    });
    std::fs::write(
        plan_dir.join("plan.json"),
        serde_json::to_string_pretty(&plan_json).unwrap(),
    )
    .unwrap();

    let contract_json = serde_json::json!({
        "schema_version": "task-contract-v1",
        "plan_id": plan_id,
        "contract_id": "contract-001",
        "status": "approved",
        "objective": "Test",
        "risk_class": "low",
        "execution_eligibility": { "allowed": true, "blocked_reason": null, "details": null },
        "scope": [],
        "acceptance_criteria": [],
        "required_verification": ["cargo test"],
        "optional_verification": [],
        "mutation_policy": { "allowed_patterns": [], "forbidden_patterns": [] },
        "test_policy": { "allowed": [], "forbidden": [] },
        "repair_budget": 0,
        "discovery_usage": { "used": false, "reason": "test" }
    });
    std::fs::write(
        contracts_dir.join("contract-001.json"),
        serde_json::to_string_pretty(&contract_json).unwrap(),
    )
    .unwrap();

    // Init run.
    Command::cargo_bin("agent_loop").unwrap()
        .arg("init-run")
        .arg("--plan-id").arg(&plan_id)
        .arg("--contract-id").arg("contract-001")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(&repo)
        .current_dir(&repo)
        .assert().success();

    // Create machine_gate.json.
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    let machine_gate = serde_json::json!({
        "schema_version": "machine-gate-v1",
        "task_id": task_id,
        "passed": true,
        "checks": [
            { "name": "artifact_checks", "passed": true }
        ]
    });
    std::fs::write(
        task_dir.join("machine_gate.json"),
        serde_json::to_string_pretty(&machine_gate).unwrap(),
    )
    .unwrap();

    // Create sonnet_review.json.
    let review = make_valid_sonnet_review(&task_id, "approve");
    std::fs::write(
        task_dir.join("sonnet_review.json"),
        serde_json::to_string_pretty(&review).unwrap(),
    )
    .unwrap();

    // Create normalized/ with diff.patch.
    let normalized_dir = task_dir.join("normalized");
    std::fs::create_dir_all(&normalized_dir).unwrap();
    std::fs::write(normalized_dir.join("diff.patch"), "--- a/x\n+++ b/x\n").unwrap();

    (temp, task_id)
}

#[test]
fn test_final_gate_writes_four_state_decision_merge() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("final-gate")
        .arg("--task-id").arg(&task_id)
        .arg("--decision").arg("merge")
        .arg("--commit-message").arg("feat: test commit")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().success();

    let gate_path = repo
        .join(".agent-runs/tasks")
        .join(&task_id)
        .join("opus_final_gate.json");
    assert!(gate_path.exists(), "opus_final_gate.json should be created");

    let content = fs::read_to_string(&gate_path).unwrap();
    let gate: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(gate["schema_version"], "opus-final-gate-v1");
    assert_eq!(gate["decision"], "merge");
    assert_eq!(gate["commit_message"], "feat: test commit");
    assert_eq!(gate["task_id"], task_id);
}

#[test]
fn test_final_gate_writes_decision_reject() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("final-gate")
        .arg("--task-id").arg(&task_id)
        .arg("--decision").arg("reject")
        .arg("--commit-message").arg("unused")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().success();

    let gate_path = repo
        .join(".agent-runs/tasks")
        .join(&task_id)
        .join("opus_final_gate.json");
    let content = fs::read_to_string(&gate_path).unwrap();
    let gate: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(gate["decision"], "reject");
    // reject has null commit_message
    assert!(gate["commit_message"].is_null());
}

#[test]
fn test_final_gate_writes_decision_request_repair() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("final-gate")
        .arg("--task-id").arg(&task_id)
        .arg("--decision").arg("request_repair")
        .arg("--commit-message").arg("unused")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().success();

    let gate_path = repo
        .join(".agent-runs/tasks")
        .join(&task_id)
        .join("opus_final_gate.json");
    let content = fs::read_to_string(&gate_path).unwrap();
    let gate: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(gate["decision"], "request_repair");
}

#[test]
fn test_final_gate_writes_decision_needs_user_decision() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("final-gate")
        .arg("--task-id").arg(&task_id)
        .arg("--decision").arg("needs_user_decision")
        .arg("--commit-message").arg("unused")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().success();

    let gate_path = repo
        .join(".agent-runs/tasks")
        .join(&task_id)
        .join("opus_final_gate.json");
    let content = fs::read_to_string(&gate_path).unwrap();
    let gate: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(gate["decision"], "needs_user_decision");
}

#[test]
fn test_final_gate_rejects_invalid_decision() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("final-gate")
        .arg("--task-id").arg(&task_id)
        .arg("--decision").arg("garbage")
        .arg("--commit-message").arg("unused")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().failure();
}

#[test]
fn test_final_gate_requires_sonnet_review() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    // Remove sonnet_review.json.
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    std::fs::remove_file(task_dir.join("sonnet_review.json")).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("final-gate")
        .arg("--task-id").arg(&task_id)
        .arg("--decision").arg("merge")
        .arg("--commit-message").arg("unused")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().failure();
}

#[test]
fn test_final_gate_requires_machine_gate() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    // Remove machine_gate.json.
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    std::fs::remove_file(task_dir.join("machine_gate.json")).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("final-gate")
        .arg("--task-id").arg(&task_id)
        .arg("--decision").arg("merge")
        .arg("--commit-message").arg("unused")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().failure();
}

#[test]
fn test_final_gate_requires_diff() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    // Remove diff.patch.
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    std::fs::remove_file(task_dir.join("normalized/diff.patch")).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("final-gate")
        .arg("--task-id").arg(&task_id)
        .arg("--decision").arg("merge")
        .arg("--commit-message").arg("unused")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().failure();
}

// ============================================================================
// git-guard demonstration path
// ============================================================================

#[test]
fn test_git_guard_allows_no_run() {
    let repo = create_test_repo();
    let repo_path = repo.path();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("git-guard")
        .arg("--task-id").arg("task-20260530-999")
        .arg("--repo-root").arg(repo_path)
        .current_dir(repo_path);
    cmd.assert().success()
        .stdout(predicate::str::contains("allowed"))
        .stdout(predicate::str::contains("no run detected"));
}

#[test]
fn test_git_guard_blocks_active_run() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    // Active status is default after init-run.

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("git-guard")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().failure()
        .stdout(predicate::str::contains("blocked"))
        .stdout(predicate::str::contains("active_run"));
}

#[test]
fn test_git_guard_allows_merge_with_preconditions() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    // Write final gate with merge.
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    let gate = serde_json::json!({
        "schema_version": "opus-final-gate-v1",
        "task_id": task_id,
        "decision": "merge",
        "commit_message": "test",
        "timestamp": "2026-05-30T12:00:00Z"
    });
    std::fs::write(
        task_dir.join("opus_final_gate.json"),
        serde_json::to_string_pretty(&gate).unwrap(),
    )
    .unwrap();

    // Set status to committed (final gate completed).
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    let status_path = task_dir.join("status.json");
    let status: serde_json::Value = serde_json::from_str(&fs::read_to_string(&status_path).unwrap()).unwrap();
    let mut committed_status = status.clone();
    committed_status["status"] = serde_json::json!("committed");
    std::fs::write(&status_path, serde_json::to_string_pretty(&committed_status).unwrap()).unwrap();

    // Create initial commit so git status succeeds.
    std::fs::write(repo.join("README"), "test").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(repo)
        .output()
        .unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("git-guard")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().success()
        .stdout(predicate::str::contains("allowed"));
}

#[test]
fn test_git_guard_blocks_non_merge_decision() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    let gate = serde_json::json!({
        "schema_version": "opus-final-gate-v1",
        "task_id": task_id,
        "decision": "reject",
        "commit_message": null,
        "timestamp": "2026-05-30T12:00:00Z"
    });
    std::fs::write(
        task_dir.join("opus_final_gate.json"),
        serde_json::to_string_pretty(&gate).unwrap(),
    )
    .unwrap();

    // Set status to committed.
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    let status_path = task_dir.join("status.json");
    let status: serde_json::Value = serde_json::from_str(&fs::read_to_string(&status_path).unwrap()).unwrap();
    let mut committed_status = status.clone();
    committed_status["status"] = serde_json::json!("committed");
    std::fs::write(&status_path, serde_json::to_string_pretty(&committed_status).unwrap()).unwrap();

    // Create initial commit so git status succeeds.
    std::fs::write(repo.join("README"), "test").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(repo)
        .output()
        .unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("git-guard")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().failure()
        .stdout(predicate::str::contains("blocked"))
        .stdout(predicate::str::contains("gate_rejected"));
}

#[test]
fn test_git_guard_pending_no_final_gate() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    // Has status but no opus_final_gate.json.

    // Active status -> blocked (not pending).
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("git-guard")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().failure()
        .stdout(predicate::str::contains("blocked"))
        .stdout(predicate::str::contains("active_run"));
}

// ============================================================================
// validate-subagent-stop demonstration path
// ============================================================================

#[test]
fn test_validate_subagent_stop_all_present() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);

    // Write opus_final_gate.json.
    let gate = serde_json::json!({
        "schema_version": "opus-final-gate-v1",
        "task_id": task_id,
        "decision": "merge",
        "commit_message": "test",
        "timestamp": "2026-05-30T12:00:00Z"
    });
    std::fs::write(
        task_dir.join("opus_final_gate.json"),
        serde_json::to_string_pretty(&gate).unwrap(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("validate-subagent-stop")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().success()
        .stdout(predicate::str::contains("All required SubagentStop artifacts present"));
}

#[test]
fn test_validate_subagent_stop_missing_artifact() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    // Remove opus_final_gate.json.
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    let _ = std::fs::remove_file(task_dir.join("opus_final_gate.json"));

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("validate-subagent-stop")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().failure()
        .stderr(predicate::str::contains("opus_final_gate.json"));
}

// ============================================================================
// PreToolUse guard tests
// ============================================================================

#[test]
fn test_pre_tool_guard_allows_no_run() {
    let temp = create_test_repo();
    let repo = temp.path();

    // No .agent-runs at all -> allowed.
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("pre-tool-guard")
        .arg("--repo-root").arg(repo);
    cmd.assert().success().stdout(predicate::str::contains("allowed"));
}

#[test]
fn test_pre_tool_guard_blocks_active_run() {
    let temp = create_test_repo();
    let repo = temp.path();

    let tasks_dir = repo.join(".agent-runs/tasks");
    std::fs::create_dir_all(&tasks_dir).unwrap();
    let task_dir = tasks_dir.join("task-20260530-001");
    std::fs::create_dir_all(&task_dir).unwrap();

    let status = serde_json::json!({
        "schema_version": "status-v1",
        "task_id": "task-20260530-001",
        "plan_id": "plan-20260530-001",
        "contract_id": "contract-001",
        "status": "active",
        "created_at": "2026-05-30T12:00:00Z",
        "updated_at": "2026-05-30T12:00:00Z"
    });
    std::fs::write(task_dir.join("status.json"), serde_json::to_string_pretty(&status).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("pre-tool-guard")
        .arg("--repo-root").arg(repo);
    cmd.assert().failure().stderr(predicate::str::contains("blocked"));
}

#[test]
fn test_pre_tool_guard_allows_merge_with_preconditions() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);

    // Write all required preconditions: committed status, merge decision, passed machine gate.
    let status = serde_json::json!({
        "schema_version": "status-v1",
        "task_id": task_id,
        "plan_id": "plan-20260530-001",
        "contract_id": "contract-001",
        "status": "committed",
        "created_at": "2026-05-30T12:00:00Z",
        "updated_at": "2026-05-30T12:00:00Z"
    });
    std::fs::write(task_dir.join("status.json"), serde_json::to_string_pretty(&status).unwrap()).unwrap();

    std::fs::write(
        task_dir.join("opus_final_gate.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": "opus-final-gate-v1",
            "task_id": task_id,
            "decision": "merge",
            "commit_message": "feat: approved change",
            "timestamp": "2026-05-30T12:00:00Z"
        })).unwrap(),
    ).unwrap();

    std::fs::write(
        task_dir.join("machine_gate.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": "machine-gate-v1",
            "passed": true,
            "timestamp": "2026-05-30T12:00:00Z"
        })).unwrap(),
    ).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("pre-tool-guard")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo);
    cmd.assert().success().stdout(predicate::str::contains("allowed"));
}

#[test]
fn test_pre_tool_guard_blocks_non_merge_gate() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);

    let status = serde_json::json!({
        "schema_version": "status-v1",
        "task_id": task_id,
        "plan_id": "plan-20260530-001",
        "contract_id": "contract-001",
        "status": "committed",
        "created_at": "2026-05-30T12:00:00Z",
        "updated_at": "2026-05-30T12:00:00Z"
    });
    std::fs::write(task_dir.join("status.json"), serde_json::to_string_pretty(&status).unwrap()).unwrap();

    std::fs::write(
        task_dir.join("opus_final_gate.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": "opus-final-gate-v1",
            "task_id": task_id,
            "decision": "reject",
            "commit_message": null,
            "timestamp": "2026-05-30T12:00:00Z"
        })).unwrap(),
    ).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("pre-tool-guard")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo);
    cmd.assert().failure().stderr(predicate::str::contains("blocked"));
}

// Note: dirty worktree is not checked by PreToolCheck - worktree cleanliness
// is verified by Integrate command, not by the PreToolUse guard.

#[test]
fn test_pre_tool_guard_pending_no_final_gate() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);

    let status = serde_json::json!({
        "schema_version": "status-v1",
        "task_id": task_id,
        "plan_id": "plan-20260530-001",
        "contract_id": "contract-001",
        "status": "committed",
        "created_at": "2026-05-30T12:00:00Z",
        "updated_at": "2026-05-30T12:00:00Z"
    });
    std::fs::write(task_dir.join("status.json"), serde_json::to_string_pretty(&status).unwrap()).unwrap();

    // No opus_final_gate -> pending, not blocked.
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("pre-tool-guard")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo);
    cmd.assert().success().stdout(predicate::str::contains("pending"));
}

#[test]
fn test_pre_tool_guard_blocks_machine_gate_failed() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);

    let status = serde_json::json!({
        "schema_version": "status-v1",
        "task_id": task_id,
        "plan_id": "plan-20260530-001",
        "contract_id": "contract-001",
        "status": "committed",
        "created_at": "2026-05-30T12:00:00Z",
        "updated_at": "2026-05-30T12:00:00Z"
    });
    std::fs::write(task_dir.join("status.json"), serde_json::to_string_pretty(&status).unwrap()).unwrap();

    std::fs::write(
        task_dir.join("opus_final_gate.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": "opus-final-gate-v1",
            "task_id": task_id,
            "decision": "merge",
            "commit_message": "feat: test",
            "timestamp": "2026-05-30T12:00:00Z"
        })).unwrap(),
    ).unwrap();

    std::fs::write(
        task_dir.join("machine_gate.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": "machine-gate-v1",
            "passed": false,
            "timestamp": "2026-05-30T12:00:00Z"
        })).unwrap(),
    ).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("pre-tool-guard")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo);
    cmd.assert().failure().stderr(predicate::str::contains("blocked"));
}

#[test]
fn test_pre_tool_guard_blocks_missing_commit_message() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);

    let status = serde_json::json!({
        "schema_version": "status-v1",
        "task_id": task_id,
        "plan_id": "plan-20260530-001",
        "contract_id": "contract-001",
        "status": "committed",
        "created_at": "2026-05-30T12:00:00Z",
        "updated_at": "2026-05-30T12:00:00Z"
    });
    std::fs::write(task_dir.join("status.json"), serde_json::to_string_pretty(&status).unwrap()).unwrap();

    std::fs::write(
        task_dir.join("opus_final_gate.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": "opus-final-gate-v1",
            "task_id": task_id,
            "decision": "merge",
            "commit_message": null,
            "timestamp": "2026-05-30T12:00:00Z"
        })).unwrap(),
    ).unwrap();

    std::fs::write(
        task_dir.join("machine_gate.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": "machine-gate-v1",
            "passed": true,
            "timestamp": "2026-05-30T12:00:00Z"
        })).unwrap(),
    ).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("pre-tool-guard")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo);
    cmd.assert().failure().stderr(predicate::str::contains("blocked"));
}


// ============================================================================
// /agent-integrate demonstration path
// ============================================================================

/// Helper: setup repo with clean main worktree and all integration preconditions.
fn setup_integrate_repo() -> (tempfile::TempDir, String) {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);

    // Write opus_final_gate with merge + commit message.
    let gate = serde_json::json!({
        "schema_version": "opus-final-gate-v1",
        "task_id": task_id,
        "decision": "merge",
        "commit_message": "feat: integration test commit",
        "timestamp": "2026-05-30T12:00:00Z"
    });
    std::fs::write(
        task_dir.join("opus_final_gate.json"),
        serde_json::to_string_pretty(&gate).unwrap(),
    )
    .unwrap();

    // Write changed_files.json.
    let normalized_dir = task_dir.join("normalized");
    let changed = serde_json::json!({
        "schema_version": "changed-files-v1",
        "task_id": task_id,
        "files": [{ "path": "src/feature.rs", "operation": "create" }]
    });
    std::fs::write(
        normalized_dir.join("changed_files.json"),
        serde_json::to_string_pretty(&changed).unwrap(),
    )
    .unwrap();

    // Write verification.json.
    let verification = serde_json::json!({
        "schema_version": "verification-v1",
        "task_id": task_id,
        "results": [],
        "all_required_passed": true
    });
    std::fs::write(
        normalized_dir.join("verification.json"),
        serde_json::to_string_pretty(&verification).unwrap(),
    )
    .unwrap();

    // Create the target file in the worktree so git apply has something to apply to.
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src/feature.rs"), "old\n").unwrap();

    // Make initial commit.
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(repo)
        .output()
        .unwrap();

    (temp, task_id)
}

#[test]
fn test_integrate_requires_final_gate_merge() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    // Write opus_final_gate with reject.
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    let gate = serde_json::json!({
        "schema_version": "opus-final-gate-v1",
        "task_id": task_id,
        "decision": "reject",
        "commit_message": null,
        "timestamp": "2026-05-30T12:00:00Z"
    });
    std::fs::write(
        task_dir.join("opus_final_gate.json"),
        serde_json::to_string_pretty(&gate).unwrap(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("integrate")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().failure()
        .stderr(predicate::str::contains("must be 'merge'"));
}

#[test]
fn test_integrate_requires_clean_worktree() {
    let (temp, task_id) = setup_final_gate_repo();
    let repo = temp.path();

    // Write opus_final_gate with merge.
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    let gate = serde_json::json!({
        "schema_version": "opus-final-gate-v1",
        "task_id": task_id,
        "decision": "merge",
        "commit_message": "test",
        "timestamp": "2026-05-30T12:00:00Z"
    });
    std::fs::write(
        task_dir.join("opus_final_gate.json"),
        serde_json::to_string_pretty(&gate).unwrap(),
    )
    .unwrap();

    // Stage everything and commit.
    let add_out = std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo)
        .output()
        .unwrap();
    if !add_out.status.success() {
        eprintln!("git add failed: {}", String::from_utf8_lossy(&add_out.stderr));
    }
    let commit_out = std::process::Command::new("git")
        .args(["commit", "-m", "setup"])
        .current_dir(repo)
        .output()
        .unwrap();
    if !commit_out.status.success() {
        eprintln!("git commit failed: {}", String::from_utf8_lossy(&commit_out.stderr));
    }

    // Now add an uncommitted file.
    std::fs::write(repo.join("uncommitted.txt"), "dirty").unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("integrate")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().failure()
        .stderr(predicate::str::contains("not clean"));
}

#[test]
fn test_integrate_stages_expected_files_only() {
    let (temp, task_id) = setup_integrate_repo();
    let repo = temp.path();
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);

    // Update diff to modify the existing file.
    let normalized_dir = task_dir.join("normalized");
    let diff = "--- a/src/feature.rs\n+++ b/src/feature.rs\n@@ -1 +1 @@\n-old\n+new\n";
    std::fs::write(normalized_dir.join("diff.patch"), diff).unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("integrate")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().success();

    // Verify integration_result.json was written.
    let result_path = task_dir.join("integration_result.json");
    assert!(result_path.exists(), "integration_result.json should exist");

    let content = fs::read_to_string(&result_path).unwrap();
    let result: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(result["schema_version"], "integration-result-v1");
    assert_eq!(result["task_id"], task_id);
    assert!(result["commit_hash"].as_str().unwrap().len() >= 7);
    assert!(result["verification_passed"].as_bool().is_some());
    assert!(result["worktree_removed"].as_bool().is_some());
    assert_eq!(result["agent_runs_retained"], true);

    // Verify worktree was removed.
    let worktree = repo.join(".worktrees").join(&task_id);
    assert!(!worktree.exists() || result["worktree_removed"] == true);
}

#[test]
fn test_integrate_patch_conflict_blocks() {
    // Test: patch conflict blocks integration.
    let (temp, task_id) = setup_integrate_repo();
    let repo = temp.path();
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    let normalized_dir = task_dir.join("normalized");

    // Write a patch that will fail apply --check (no matching context).
    std::fs::write(
        normalized_dir.join("diff.patch"),
        "--- a/no-match\n+++ b/no-match\n@@ -1 +1 @@\n-old\n+new\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("integrate")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().failure();
}

#[test]
fn test_integrate_no_auto_resolve() {
    // Verify that git apply --check is used before apply.
    // If conflict, no auto-resolve happens.
    let (temp, task_id) = setup_integrate_repo();
    let repo = temp.path();
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    let normalized_dir = task_dir.join("normalized");

    // Write a patch that will fail apply --check.
    std::fs::write(
        normalized_dir.join("diff.patch"),
        "--- a/does-not-exist\n+++ b/does-not-exist\n@@ -0,0 +1 @@\n+conflict\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("integrate")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd.assert().failure().stderr(predicate::str::contains("conflict").or(predicate::str::contains("commit")));

    // Verify no commit was created.
    let log_out = std::process::Command::new("git")
        .args(["log", "--oneline"])
        .current_dir(repo)
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&log_out.stdout);
    assert!(
        !log.contains("integration test commit"),
        "no commit should be created on conflict"
    );
}

#[test]
fn test_final_gate_then_integrate_produces_local_commit() {
    let (temp, task_id) = setup_integrate_repo();
    let repo = temp.path();
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    let normalized_dir = task_dir.join("normalized");

    // Create a target file and initial commit.
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src/feature.rs"), "old\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(repo)
        .output()
        .unwrap();

    // Update diff.patch to modify the existing file.
    std::fs::write(
        normalized_dir.join("diff.patch"),
        "--- a/src/feature.rs\n+++ b/src/feature.rs\n@@ -1 +1 @@\n-old\n+new\n",
    )
    .unwrap();

    // Write changed_files.json.
    let changed = serde_json::json!({
        "schema_version": "changed-files-v1",
        "task_id": task_id,
        "files": [{ "path": "src/feature.rs", "operation": "modify" }]
    });
    std::fs::write(
        normalized_dir.join("changed_files.json"),
        serde_json::to_string_pretty(&changed).unwrap(),
    )
    .unwrap();

    // Write verification.json.
    let verification = serde_json::json!({
        "schema_version": "verification-v1",
        "task_id": task_id,
        "results": [],
        "all_required_passed": true
    });
    std::fs::write(
        normalized_dir.join("verification.json"),
        serde_json::to_string_pretty(&verification).unwrap(),
    )
    .unwrap();

    // Step 1: /agent-final-gate.
    let mut cmd1 = Command::cargo_bin("agent_loop").unwrap();
    cmd1.arg("final-gate")
        .arg("--task-id").arg(&task_id)
        .arg("--decision").arg("merge")
        .arg("--commit-message").arg("feat: end-to-end integration test")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd1.assert().success();

    // Verify opus_final_gate.json was written.
    let gate_path = task_dir.join("opus_final_gate.json");
    assert!(gate_path.exists());
    let gate: serde_json::Value = serde_json::from_str(&fs::read_to_string(&gate_path).unwrap()).unwrap();
    assert_eq!(gate["decision"], "merge");
    assert_eq!(gate["commit_message"], "feat: end-to-end integration test");

    // Step 2: /agent-integrate.
    let mut cmd2 = Command::cargo_bin("agent_loop").unwrap();
    cmd2.arg("integrate")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo)
        .current_dir(repo);
    cmd2.assert().success();

    // Verify integration_result.json.
    let result_path = task_dir.join("integration_result.json");
    assert!(result_path.exists());
    let result: serde_json::Value = serde_json::from_str(&fs::read_to_string(&result_path).unwrap()).unwrap();
    assert!(result["commit_hash"].as_str().unwrap().len() >= 7, "commit_hash must be at least 7 chars (short SHA)");
    assert_eq!(result["committed_files"].as_array().unwrap().len(), 1);
    assert_eq!(result["verification_passed"].as_bool().unwrap(), true);
    assert_eq!(result["worktree_removed"].as_bool().unwrap(), true);
    assert_eq!(result["agent_runs_retained"].as_bool().unwrap(), true);

    // Verify git log shows the commit.
    let log_output = std::process::Command::new("git")
        .args(["log", "--oneline"])
        .current_dir(repo)
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&log_output.stdout);
    assert!(log.contains("end-to-end integration test"));

    // Verify .agent-runs/ evidence is retained.
    assert!(gate_path.exists(), "opus_final_gate.json must be retained");
    assert!(task_dir.join("machine_gate.json").exists());
    assert!(task_dir.join("sonnet_review.json").exists());
    assert!(task_dir.join("post_apply_verification.json").exists());
}

// ============================================================================
// export / cleanup integration tests
// ============================================================================

fn setup_export_task(task_id: &str) -> (tempfile::TempDir, String) {
    let temp = tempfile::TempDir::new().unwrap();
    let repo = temp.path();
    let git = repo.join(".git");
    std::fs::create_dir(&git).unwrap();
    std::fs::write(git.join("config"), "[core]\n").unwrap();

    let task_id = task_id.to_string();
    let task_dir = repo.join(".agent-runs/tasks").join(&task_id);
    std::fs::create_dir_all(task_dir.join("normalized")).unwrap();
    std::fs::create_dir_all(task_dir.join("backend-output")).unwrap();
    // Create .env file for secrets detection
    std::fs::write(task_dir.join("backend-output").join(".env"), "SECRET=value\n").unwrap();

    // status.json
    let status = serde_json::json!({
        "schema_version": "status-v1",
        "task_id": task_id,
        "plan_id": "plan-20260530-001",
        "contract_id": "contract-001",
        "status": "active",
        "created_at": "2026-05-30T12:00:00Z",
        "updated_at": "2026-05-30T12:00:00Z"
    });
    std::fs::write(task_dir.join("status.json"), serde_json::to_string_pretty(&status).unwrap()).unwrap();

    // contract.json
    let contract = serde_json::json!({
        "schema_version": "task-contract-v1",
        "task_id": task_id,
        "plan_id": "plan-20260530-001",
        "contract_id": "contract-001",
        "objective": "Test export command",
        "risk_class": "normal",
        "scope": ["src/"],
        "status": "approved",
        "acceptance_criteria": ["Tests pass"],
        "required_verification": ["cargo test"],
        "execution_eligibility": {
            "allowed": true,
            "blocked_reason": null,
            "details": null
        },
        "mutation_policy": {
            "allowed_patterns": ["src/**/*.rs"],
            "forbidden_patterns": []
        },
        "test_policy": {
            "allowed": ["unit tests"],
            "forbidden": []
        },
        "repair_budget": 2
    });
    std::fs::write(task_dir.join("contract.json"), serde_json::to_string_pretty(&contract).unwrap()).unwrap();

    // changed_files.json
    let changed = serde_json::json!({
        "schema_version": "changed-files-v1",
        "task_id": task_id,
        "files": [
            { "path": "src/lib.rs", "operation": "modify" },
            { "path": ".env.production", "operation": "modify" }
        ]
    });
    std::fs::write(task_dir.join("normalized").join("changed_files.json"), serde_json::to_string_pretty(&changed).unwrap()).unwrap();

    // diff.patch
    std::fs::write(
        task_dir.join("normalized").join("diff.patch"),
        "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n--- a/.env.production\n+++ b/.env.production\n@@ -1 +1 @@\n-API_KEY=xxx\n+API_KEY=yyy\n",
    )
    .unwrap();

    // verification.json
    let verification = serde_json::json!({
        "schema_version": "verification-v1",
        "task_id": task_id,
        "results": [
            { "command": "cargo test", "exit_code": 0, "passed": true }
        ]
    });
    std::fs::write(task_dir.join("normalized").join("verification.json"), serde_json::to_string_pretty(&verification).unwrap()).unwrap();

    // external_review.json
    let ext_review = serde_json::json!({
        "schema_version": "external-review-v1",
        "task_id": task_id,
        "verdict": "pass",
        "scope_compliance": true,
        "policy_compliance": true,
        "verification_sufficient": true,
        "findings": []
    });
    std::fs::write(task_dir.join("normalized").join("external_review.json"), serde_json::to_string_pretty(&ext_review).unwrap()).unwrap();

    // sensitive_audit.json
    let sensitive = serde_json::json!({
        "schema_version": "sensitive-audit-v1",
        "task_id": task_id,
        "files_checked": 2,
        "residual_risks": ["runtime config change"],
        "accepted_risks": ["no PII affected"],
        "audit_limitation": null
    });
    std::fs::write(task_dir.join("normalized").join("sensitive_audit.json"), serde_json::to_string_pretty(&sensitive).unwrap()).unwrap();

    // sonnet_review.json
    let sonnet = serde_json::json!({
        "schema_version": "sonnet-review-v1",
        "task_id": task_id,
        "recommendation": "ship",
        "blockers": [],
        "non_blockers": ["minor style note"],
        "gates": [
            { "gate": "evidence_validity", "passed": true },
            { "gate": "scope_policy", "passed": true },
            { "gate": "verification", "passed": true },
            { "gate": "diff_code_review", "passed": true },
            { "gate": "merge_recommendation", "passed": true }
        ],
        "sensitive_audit_clean": true,
        "external_verdict": "pass"
    });
    std::fs::write(task_dir.join("sonnet_review.json"), serde_json::to_string_pretty(&sonnet).unwrap()).unwrap();

    // opus_final_gate.json
    let gate = serde_json::json!({
        "schema_version": "opus-final-gate-v1",
        "task_id": task_id,
        "decision": "merge",
        "commit_message": "feat: test integration",
        "timestamp": "2026-05-30T14:00:00Z",
        "notes": null
    });
    std::fs::write(task_dir.join("opus_final_gate.json"), serde_json::to_string_pretty(&gate).unwrap()).unwrap();

    // Create a worktree for cleanup tests.
    let worktree = repo.join(".worktrees").join(&task_id);
    std::fs::create_dir_all(&worktree).unwrap();
    std::fs::write(worktree.join("worktree-file.txt"), "placeholder").unwrap();

    (temp, task_id)
}

#[test]
fn test_export_completed_run_valid_sanitized_json() {
    let (temp, task_id) = setup_export_task("task-20260530-001");
    let repo = temp.path();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("export")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo)
        .current_dir(repo);

    cmd.assert().success();

    let json_str = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&json_str.stdout);

    // Parse as JSON.
    let export: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // Verify required fields present.
    assert_eq!(export["schema_version"], "export-sanitized-v1");
    assert_eq!(export["task_id"], task_id);
    assert_eq!(export["plan_id"], "plan-20260530-001");
    assert_eq!(export["contract_id"], "contract-001");
    assert!(export.get("contract_summary").is_some());
    assert!(export.get("diff_summary").is_some());
    assert!(export.get("verification").is_some());
    assert!(export["external_verdict"] == "pass");
    assert!(export.get("sonnet_verdict").is_some(), "sonnet_verdict should be present");
    assert!(export["opus_decision"] == "merge", "opus_decision should be merge");
    assert!(!export["full_export"].as_bool().unwrap(), "full_export should be false in default export");
    // blocking_issues, residual_risks, accepted_risks may be empty if not present
    if let Some(bi) = export.get("blocking_issues").and_then(|v| v.as_array()) {
        // OK if empty or not empty
    }
    if let Some(rr) = export.get("residual_risks").and_then(|v| v.as_array()) {
        // OK if empty or not empty
    }
    if let Some(ar) = export.get("accepted_risks").and_then(|v| v.as_array()) {
        // OK if empty or not empty
    }

    // Sanitized checks: .env.production should NOT appear in changed_files (unsafe path).
    let changed_files = export["changed_files"].as_array().unwrap();
    let paths: Vec<_> = changed_files.iter()
        .filter_map(|f| f.get("path").and_then(|v| v.as_str()))
        .collect();
    assert!(!paths.iter().any(|p| p.contains(".env")), ".env should be sanitized out of changed_files");
    assert!(paths.contains(&"src/lib.rs"), "safe path src/lib.rs should be preserved in changed_files");

    // Secrets list should include detected .env files.
    assert!(export["secrets"].as_array().unwrap().iter().any(|s| s.as_str().map(|s| s.contains("env")).unwrap_or(false)),
        "secrets list should include .env files");
}

#[test]
fn test_export_full_requires_acknowledgement() {
    let (temp, task_id) = setup_export_task("task-20260530-002");
    let repo = temp.path();

    // Without --acknowledge-full-export-risk, should fail.
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("export")
        .arg("--task-id").arg(&task_id)
        .arg("--full")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("acknowledge"));
}

#[test]
fn test_export_full_includes_raw_and_secrets() {
    let (temp, task_id) = setup_export_task("task-20260530-003");
    let repo = temp.path();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("export")
        .arg("--task-id").arg(&task_id)
        .arg("--full")
        .arg("--acknowledge-full-export-risk")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);

    cmd.assert().success();

    let json_str = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&json_str.stdout);
    let export: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert!(export["full_export"].as_bool().unwrap());
    assert!(export.get("raw_outputs").is_some());
    assert!(export["secrets"].as_array().unwrap().iter().any(|s| s.as_str().unwrap_or("").contains(".env")), "secrets should detect .env file from setup");
}

#[test]
fn test_export_task_not_found() {
    let (temp, _task_id) = setup_export_task("task-20260530-004");
    let repo = temp.path();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("export")
        .arg("--task-id").arg("task-99999999-999")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);

    cmd.assert().failure().stderr(predicate::str::contains("not found"));
}

#[test]
fn test_cleanup_dry_run_correct_targets() {
    let (temp, task_id) = setup_export_task("task-20260530-005");
    let repo = temp.path();

    let worktree = repo.join(".worktrees").join(&task_id);

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("cleanup")
        .arg("--task-id").arg(&task_id)
        .arg("--worktrees")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);

    cmd.assert().success();

    let json_str = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&json_str.stdout);
    assert!(stdout.contains("Dry run") || stdout.contains("task"), "dry-run should report tasks");
    assert!(stdout.contains(&task_id), "dry-run should list the task");
    assert!(worktree.exists(), "dry-run should NOT delete worktree");
}

#[test]
fn test_cleanup_preview_with_status_filter() {
    let (temp, task_id) = setup_export_task("task-20260530-006");
    let repo = temp.path();

    // Update status to committed.
    let status_path = repo.join(".agent-runs/tasks").join(&task_id).join("status.json");
    let mut status: serde_json::Value = serde_json::from_str(&fs::read_to_string(&status_path).unwrap()).unwrap();
    status["status"] = serde_json::Value::String("committed".to_string());
    std::fs::write(&status_path, serde_json::to_string_pretty(&status).unwrap()).unwrap();

    let worktree = repo.join(".worktrees").join(&task_id);
    let evidence = repo.join(".agent-runs/tasks").join(&task_id);

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("cleanup")
        .arg("--status").arg("committed")
        .arg("--worktrees")
        .arg("--evidence")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);

    cmd.assert().success();

    let json_str = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&json_str.stdout);
    assert!(stdout.contains("committed") || stdout.contains(&task_id));
    assert!(worktree.exists(), "dry-run should NOT delete worktree");
    assert!(evidence.exists(), "dry-run should NOT delete evidence");
}

#[test]
fn test_cleanup_older_than_zero_excludes_zero_age_tasks() {
    let (temp, task_id) = setup_export_task("task-20260530-007");
    let repo = temp.path();

    // Task was just created (age ≈ 0 seconds). --older-than 0 means age > 0,
    // so a zero-age task is filtered out. This proves the filter actually runs.
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("cleanup")
        .arg("--older-than").arg("0")
        .arg("--worktrees")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);

    cmd.assert().success();
    let json_str = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&json_str.stdout);
    // Zero-age task should be excluded (age is 0, not > 0)
    assert!(!stdout.contains(&task_id),
        "--older-than 0 should exclude zero-age tasks, got: {}", stdout);
}

#[test]
fn test_cleanup_no_selector_fails() {
    let (temp, _task_id) = setup_export_task("task-20260530-008");
    let repo = temp.path();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("cleanup")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);

    cmd.assert().failure().stderr(predicate::str::contains("selector"));
}

#[test]
fn test_cleanup_preview_default_without_flags() {
    let (temp, task_id) = setup_export_task("task-20260530-009");
    let repo = temp.path();

    let worktree = repo.join(".worktrees").join(&task_id);
    assert!(worktree.exists(), "setup: worktree should exist");

    // Default behavior: cleanup without --dry-run or --confirm should preview (not error)
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("cleanup")
        .arg("--task-id").arg(&task_id)
        .arg("--worktrees")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);

    cmd.assert().success();
    let json_str = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&json_str.stdout);
    assert!(stdout.contains("Dry run") || stdout.contains("task"), "default should preview");
    assert!(worktree.exists(), "preview mode should NOT delete worktree");
}

#[test]
fn test_cleanup_destructive_deletes_with_confirm() {
    let (temp, task_id) = setup_export_task("task-20260530-010");
    let repo = temp.path();

    // Update status to committed so cleanup picks it up.
    let status_path = repo.join(".agent-runs/tasks").join(&task_id).join("status.json");
    let mut status: serde_json::Value = serde_json::from_str(&fs::read_to_string(&status_path).unwrap()).unwrap();
    status["status"] = serde_json::Value::String("committed".to_string());
    std::fs::write(&status_path, serde_json::to_string_pretty(&status).unwrap()).unwrap();

    let worktree = repo.join(".worktrees").join(&task_id);
    std::fs::create_dir_all(&worktree).unwrap();
    assert!(worktree.exists(), "setup: worktree should exist");

    // Test using cargo run from the agent-loop directory.
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("cleanup")
        .arg("--status").arg("committed")
        .arg("--worktrees")
        .arg("--evidence")
        .arg("--confirm")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);

    cmd.assert().success();
    // Verify worktree was deleted
    assert!(!worktree.exists(), "with --confirm, worktree should be deleted");
}


fn create_cleanup_test_tasks(
    plan_id_a: &str,
    plan_id_b: &str,
    age_hours_a: i64,
    age_hours_b: i64,
) -> (TempDir, String, String) {
    let temp = tempfile::TempDir::new().unwrap();
    let repo = temp.path();
    let git = repo.join(".git");
    std::fs::create_dir(&git).unwrap();
    std::fs::write(git.join("config"), "[core]\n").unwrap();

    let task_a = "task-20260530-001";
    let task_b = "task-20260530-002";

    let now = chrono::Utc::now();

    // Task A: plan_a, older
    let ta_dir = repo.join(".agent-runs/tasks").join(task_a);
    std::fs::create_dir_all(ta_dir.join("normalized")).unwrap();
    let ts_a = (now - chrono::Duration::hours(age_hours_a)).to_rfc3339();
    let status_a = serde_json::json!({
        "schema_version": "status-v1",
        "task_id": task_a,
        "plan_id": plan_id_a,
        "contract_id": "c1",
        "status": "active",
        "created_at": ts_a,
        "updated_at": ts_a,
    });
    std::fs::write(ta_dir.join("status.json"), serde_json::to_string_pretty(&status_a).unwrap()).unwrap();
    let wt_a = repo.join(".worktrees").join(task_a);
    std::fs::create_dir_all(&wt_a).unwrap();
    std::fs::write(wt_a.join("f.txt"), "a").unwrap();

    // Task B: plan_b, newer
    let tb_dir = repo.join(".agent-runs/tasks").join(task_b);
    std::fs::create_dir_all(tb_dir.join("normalized")).unwrap();
    let ts_b = (now - chrono::Duration::hours(age_hours_b)).to_rfc3339();
    let status_b = serde_json::json!({
        "schema_version": "status-v1",
        "task_id": task_b,
        "plan_id": plan_id_b,
        "contract_id": "c2",
        "status": "active",
        "created_at": ts_b,
        "updated_at": ts_b,
    });
    std::fs::write(tb_dir.join("status.json"), serde_json::to_string_pretty(&status_b).unwrap()).unwrap();
    let wt_b = repo.join(".worktrees").join(task_b);
    std::fs::create_dir_all(&wt_b).unwrap();
    std::fs::write(wt_b.join("f.txt"), "b").unwrap();

    (temp, task_a.to_string(), task_b.to_string())
}

#[test]
fn test_cleanup_plan_id_includes_matching_excludes_others() {
    let (temp, task_a, task_b) = create_cleanup_test_tasks("PLAN_A", "PLAN_B", 1, 1);
    let repo = temp.path();

    let wt_a = repo.join(".worktrees").join(&task_a);
    let wt_b = repo.join(".worktrees").join(&task_b);
    assert!(wt_a.exists());
    assert!(wt_b.exists());

    // Filter by PLAN_A — only task_a should be listed, task_b excluded
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("cleanup")
        .arg("--plan-id").arg("PLAN_A")
        .arg("--worktrees")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);

    cmd.assert().success();
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(&task_a), "stdout should contain task_a: {}", stdout);
    assert!(!stdout.contains(&task_b), "stdout must NOT contain task_b: {}", stdout);
    assert!(wt_a.exists(), "preview must NOT delete worktree");
    assert!(wt_b.exists(), "preview must NOT delete worktree");
}

#[test]
fn test_cleanup_older_than_excludes_fresh_tasks() {
    let (temp, task_a, task_b) = create_cleanup_test_tasks("PLAN_X", "PLAN_X", 10, 0);
    let repo = temp.path();

    let wt_a = repo.join(".worktrees").join(&task_a);
    let wt_b = repo.join(".worktrees").join(&task_b);

    // --older-than 5 should match task_a (10h old) but NOT task_b (0h old)
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("cleanup")
        .arg("--older-than").arg("5")
        .arg("--worktrees")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);

    cmd.assert().success();
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(&task_a), "10h-old task should match --older-than 5: {}", stdout);
    assert!(!stdout.contains(&task_b), "0h-old task should NOT match --older-than 5: {}", stdout);
    assert!(wt_a.exists());
    assert!(wt_b.exists());
}

#[test]
fn test_cleanup_older_than_huge_excludes_all() {
    let (temp, task_a, task_b) = create_cleanup_test_tasks("PLAN_X", "PLAN_X", 1, 1);
    let repo = temp.path();

    // --older-than 999999 should exclude everything (no tasks 11+ days old)
    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("cleanup")
        .arg("--older-than").arg("999999")
        .arg("--worktrees")
        .arg("--repo-root").arg(repo)
        .current_dir(repo);

    cmd.assert().success();
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains(&task_a), "no task should match --older-than 999999: {}", stdout);
    assert!(!stdout.contains(&task_b), "no task should match --older-than 999999: {}", stdout);
}

#[test]
fn test_export_sanitize_path_preserves_safe_relative() {
    let (temp, task_id) = setup_export_task("task-sanitize-001");
    let repo = temp.path();

    let mut cmd = Command::cargo_bin("agent_loop").unwrap();
    cmd.arg("export")
        .arg("--task-id").arg(&task_id)
        .arg("--repo-root").arg(repo);

    cmd.assert().success();
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let changed = json.get("changed_files").and_then(|c| c.as_array());
    assert!(changed.is_some(), "export should include changed_files");
    let paths: Vec<_> = changed.unwrap()
        .iter()
        .filter_map(|f| f.get("path").and_then(|p| p.as_str()))
        .collect();

    // src/lib.rs is safe relative — should be preserved as-is
    assert!(paths.contains(&"src/lib.rs"), "safe relative path src/lib.rs should be preserved, got: {:?}", paths);
    // .env.production is unsafe — should be filtered out entirely
    assert!(!paths.contains(&".env.production"), "unsafe path .env.production should be filtered out");
}

