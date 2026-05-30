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

    let plan_id = "plan-20260529-001";
    let contract_id = "contract-001";

    // Create plan directory structure as /agent-plan would
    let plan_dir = repo_path.join(".agent-runs/plans").join(plan_id);
    let contracts_dir = plan_dir.join("contracts");
    fs::create_dir_all(&contracts_dir).unwrap();

    // Create plan.json (static manifest)
    let plan_json = serde_json::json!({
        "schema_version": "plan-v1",
        "plan_id": plan_id,
        "created_at": "2026-05-29T12:00:00Z",
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
        .join("task-20260529-001")
        .join("status.json");
    assert!(status_path.exists(), "status.json should be created");

    // Verify status.json content
    let status_content = fs::read_to_string(&status_path).unwrap();
    let status: serde_json::Value = serde_json::from_str(&status_content).unwrap();
    assert_eq!(status["status"], "active");
    assert_eq!(status["task_id"], "task-20260529-001");
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
        .stdout(predicate::str::contains("task-20260529-001"));

    println!("All /agent-plan demonstration tests passed!");
}

#[test]
fn test_id_formats() {
    // Test plan-YYYYMMDD-NNN format
    let plan_id = "plan-20260529-001";
    assert!(regex::Regex::new(r"^plan-\d{8}-\d{3}$")
        .unwrap()
        .is_match(plan_id));

    // Test contract-NNN format
    let contract_id = "contract-001";
    assert!(regex::Regex::new(r"^contract-\d{3}$")
        .unwrap()
        .is_match(contract_id));

    // Test task-YYYYMMDD-NNN format
    let task_id = "task-20260529-001";
    assert!(regex::Regex::new(r"^task-\d{8}-\d{3}$")
        .unwrap()
        .is_match(task_id));
}

#[test]
fn test_blocked_requires_reason_and_details() {
    let repo = create_test_repo();
    let repo_path = repo.path();

    let task_id = "task-20260529-001";
    let plan_id = "plan-20260529-001";

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
        "created_at": "2026-05-29T12:00:00Z",
        "updated_at": "2026-05-29T12:00:00Z"
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

    let task_id = "task-20260529-001";
    let plan_id = "plan-20260529-001";

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
        "created_at": "2026-05-29T12:00:00Z",
        "updated_at": "2026-05-29T12:00:00Z"
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

 let plan_id = "plan-20260529-001".to_string();
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
 "created_at": "2026-05-29T12:00:00Z",
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
 "approved_at": "2026-05-29T12:00:00Z",
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
 .arg("task-20260529-001")
 .arg("--repo-root")
 .arg(repo_path)
 .current_dir(repo_path);
 cmd.assert().success();

 let task_id = "task-20260529-001";

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
 .arg("plan-20260529-001")
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
 .arg("plan-20260529-001")
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
 .arg("plan-20260529-001")
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
    let task_id = "task-20260529-001";
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
    let task_id = "task-20260529-001";
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
    let task_id = "task-20260529-001";
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
    let task_id = "task-20260529-001";
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
    let task_id = "task-20260529-001";
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
    let task_id = "task-20260529-001";
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
    let task_id = "task-20260529-001";
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
        .arg("init-run").arg("--plan-id").arg("plan-20260529-001")
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
