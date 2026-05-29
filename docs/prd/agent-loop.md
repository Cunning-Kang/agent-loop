## Problem Statement

The user needs a structured, repeatable, and robust Phase 1 workflow for Claude Code that enables reliable Plan–Work–Verify execution loops. The goal is to delegate execution and review tasks from the Opus main session to subagents and an isolated external executor, while preserving high-quality, structured evidence, enabling post-execution verification, and ensuring safe local integration without context rot or workflow misalignment.

---

## Solution

The solution is a Claude Code Phase 1 workflow specification that includes:

* Opus main session orchestrating task slicing, contract approval, final decision, and integration.
* Custom subagents: `codebase-discovery` (static repo facts), `external-executor-adapter` (Haiku subagent coordinating backend execution), and `sonnet-reviewer` (artifact review).
* An isolated backend executor (provisional `omp` CLI) performing Plan–Work–Verify cycles.
* Lightweight Rust CLI (`agent-loop`) for init-run, evidence collection/validation, git guard, export, list-runs, and cleanup.
* Task-local artifacts for evidence, status, and verification.
* Automated worktree cleanup after commit, preserving evidence.
* Phase 1 frozen scope with deferred items (update-plan, RPC/daemon orchestration, parallel execution) for Phase 2.

---

## User Stories

1. As Opus main, I want to slice a high-level task into smaller contracts, so that execution can be delegated safely.
2. As Opus main, I want to approve contracts, so that only valid and scoped tasks are executed.
3. As Opus main, I want to perform the final gate decision, so that I can control what gets merged.
4. As a developer, I want the `codebase-discovery` subagent to identify relevant files, tests, and repo structure, so that task contracts are well-informed.
5. As a developer, I want `external-executor-adapter` to execute tasks in an isolated worktree, so that main session context is preserved.
6. As a developer, I want the external backend to perform Plan–Work–Verify cycles autonomously, so that execution quality is high.
7. As a reviewer, I want `sonnet-reviewer` to validate artifacts, diffs, and verification results, so that only high-quality outputs are passed to integration.
8. As Opus main, I want `/agent-integrate` to apply patches and commit locally only after post-apply verification passes, so that the main repo remains safe.
9. As Opus main, I want the CLI to provide `list-runs` showing active, blocked, and cleanup-eligible tasks, so that I can monitor progress.
10. As Opus main, I want automated worktree cleanup after successful commit, so that disk clutter is minimized.
11. As a developer, I want evidence artifacts to be preserved after cleanup, so that post-mortem analysis and auditing are possible.
12. As a developer, I want the system to prevent unsafe manual integration via `git-guard`, so that task integrity is maintained.
13. As a developer, I want Phase 1 workflow scope frozen, so that development efforts remain focused and reproducible.
14. As a developer, I want optional hooks like `Stop` for reminders, so that awareness of pending runs is enhanced without enforcing strict gating.
15. As Opus main, I want normalized evidence to follow a deterministic schema, so that LLM subagents can reliably interpret outputs.
16. As a developer, I want backend outputs and Sonnet reviews separated, so that independent verification is enforceable.
17. As Opus main, I want tasks blocked to record `blocked_reason` and `details`, so that I can understand why execution is halted.
18. As a developer, I want subagents to have strictly defined responsibilities, so that context rot and overreach are avoided.
19. As a developer, I want plan.json to be a static manifest without runtime state, so that task progress is fully determined by task-local artifacts.
20. As a developer, I want minimal four-state task status (active, committed, blocked, abandoned) to track runs, so that state management remains lightweight.
21. As a developer, I want evidence normalization to prevent semantic judgment or repair, so that artifacts remain auditable and reproducible.
22. As Opus main, I want integration only if machine gate passes, so that quality guarantees are preserved.
23. As a developer, I want a consistent snake_case naming scheme for all artifacts, so that CLI commands and scripts are predictable.
24. As a developer, I want deferred Phase 2 items documented (update-plan, RPC/daemon, parallel execution), so that roadmap clarity is preserved.

---

## Implementation Decisions

* **Modules built/modified:**

  * `codebase-discovery` subagent
  * `external-executor-adapter` subagent
  * `sonnet-reviewer` subagent
  * `agent-loop` Rust CLI (init-run, collect-evidence, validate, git-guard, export, list-runs, cleanup)
  * `/agent-plan`, `/agent-run`, `/agent-review`, `/agent-final-gate`, `/agent-integrate` slash commands
* **Artifact storage:**

  * `.agent-runs/tasks/{task_id}/` for evidence, status.json, verification
  * `.worktrees/{task_id}/` for isolated execution
* **Task status:**

  * Four minimal states: active, committed, blocked, abandoned
  * Blocked includes `blocked_reason` and `details`
* **Worktree cleanup:**

  * Auto-remove worktree after successful commit
  * Retain evidence artifacts
* **Plan.json:**

  * Static manifest only; no runtime state
  * Task lifecycle derived entirely from task-local artifacts
* **CLI responsibilities:**

  * Deterministic orchestration
  * No semantic judgment
  * Status updates only from Phase 1 commands
* **Naming conventions:**

  * snake_case for all JSON artifacts
* **Phase 1 frozen scope:**

  * update-plan and other complex orchestration deferred to Phase 2

---

## Testing Decisions

* Only test external observable behavior (evidence, status transitions, git commit)
* Modules to be tested:

  * `external-executor-adapter` (mock backend)
  * `sonnet-reviewer` (validation of normalized evidence)
  * `agent-loop` CLI (init-run, collect-evidence, git-guard, cleanup)
* Prior art:

  * Previous test cases in Claude Code for isolated worktree execution
  * Mocked backend outputs and validation pipelines

---

## Out of Scope

* RPC/daemon orchestration
* Parallel task execution
* Full task lifecycle engine
* Push/PR/deploy
* Automatic repair orchestration beyond backend
* Phase 2 extensions (update-plan, multi-backend support, task-scoped orchestration)

---

## Further Notes

* All Phase 1 decisions are locked; additional workflow extensions reserved for Phase 2.
* Evidence normalization enforces strict separation of execution, review, and integration duties.
* Optional hooks like `Stop` can enhance user awareness but do not affect execution flow.
* This PRD serves as source-of-truth for Phase 1 implementation of the Claude Code agent-loop workflow.
