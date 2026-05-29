//! Integration test for /agent-plan demonstration path.
//!
//! This test proves that the CLI can produce valid plan.json + proposed contract files
//! as would be created by the /agent-plan slash command.

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
