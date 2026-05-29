# ADR-001: Claude Code Agent Loop Workflow

## Status

Accepted.

## Context

The primary development environment is Claude Code. The desired operating model is to use the Opus main session as the human-facing controller, task slicer, final judge, and integration owner, while delegating implementation and review work to bounded agents and external execution systems.

The workflow is inspired by Plan–Work–Verify / Plan–Generate–Evaluate agent loops. The goal is not to replace the Opus main session, but to make delegated execution more reliable by forcing task contracts, isolated execution, independent verification, structured evidence, second-pass review, final gate, and local integration.

The workflow is a reusable personal Claude Code workflow, not a one-off project configuration. Therefore, Phase 1 assets live primarily in a dedicated workflow repository and are installed into the user-level Claude Code environment. Project-local repositories only store run artifacts and temporary worktrees.

## Goals

- Keep Opus main focused on user intent, task slicing, contract synthesis, final judgment, and integration.
- Avoid Opus main context rot by delegating codebase discovery, execution adaptation, and review to subagents.
- Execute candidate code changes inside isolated git worktrees.
- Use an external execution backend to perform implementation plus internal review/verify/repair loop.
- Normalize backend output into structured LLM-oriented evidence artifacts.
- Require Sonnet reviewer to review artifacts and diffs, not human-oriented summaries.
- Require Opus final gate before integration.
- Apply and commit approved changes locally through a standard `/agent-integrate` flow.
- Keep Phase 1 small enough to implement.

## Non-goals

- Build a full workflow engine.
- Build a full agent runtime.
- Build a full security sandbox or network ACL system.
- Support parallel external execution in Phase 1.
- Support multiple external backends in Phase 1.
- Support task-scoped RPC/daemon backend orchestration in Phase 1.
- Push, create PRs, deploy, or perform irreversible external side effects in Phase 1.
- Make discovery output a machine-trusted execution artifact.
- Let external agents directly modify the main worktree.

## Decision

Implement a user-level Claude Code agent-loop workflow with:

- custom `codebase-discovery` subagent;
- custom `external-executor-adapter` subagent;
- custom `sonnet-reviewer` subagent;
- Claude Code slash commands;
- Claude Code hooks;
- a lightweight Rust `agent-loop` CLI;
- project-local `.agent-runs/` and `.worktrees/` artifacts;
- provisional `omp` one-shot CLI backend for Phase 1;
- normalized evidence and final-gate artifacts;
- `/agent-integrate` for local apply, verification, commit, and successful worktree cleanup.

## Phase 1 Frozen Scope

### Custom subagents

- `codebase-discovery`
- `external-executor-adapter`
- `sonnet-reviewer`

### Slash commands

- `/agent-plan`
- `/agent-run`
- `/agent-review`
- `/agent-final-gate`
- `/agent-integrate`

### Core CLI commands

- `agent-loop init-run`
- `agent-loop collect-evidence`
- `agent-loop validate-discovery`
- `agent-loop validate-evidence`
- `agent-loop validate-sonnet-review`
- `agent-loop gate-check`
- `agent-loop git-guard`
- `agent-loop export`

### Maintenance CLI commands

- `agent-loop list-runs`
- `agent-loop cleanup`

### Phase 1 backend

- provisional backend: `omp` / `oh-my-pi`
- invocation mode: one-shot CLI
- backend status: replaceable, not strategic lock-in

### Deferred to Phase 2

- task-scoped RPC/daemon backend mode;
- multiple backend implementations;
- backend orchestration inside Rust CLI;
- parallel task execution;
- automatic backend selection;
- full execution trace from RPC events;
- advanced cleanup policies;
- stronger sandbox/network enforcement;
- `agent-loop update-plan` or richer plan/task lifecycle synchronization;
- cloud sync or team sharing.

## Architecture

```text
Opus main session
  -> /agent-plan
      -> codebase-discovery subagent
      -> plan.json
      -> proposed/approved contract.json

  -> /agent-run
      -> external-executor-adapter subagent
          -> agent-loop init-run
          -> isolated git worktree
          -> provisional omp backend
          -> external worker + independent external review/verify
          -> repair loop within repair_budget
          -> raw backend output
          -> normalized evidence

  -> /agent-review
      -> sonnet-reviewer subagent
      -> sonnet_review.json

  -> /agent-final-gate
      -> machine gate
      -> opus_final_gate.json

  -> /agent-integrate
      -> verify final gate
      -> apply patch to clean main worktree
      -> run post-apply verification
      -> stage expected files only
      -> create local commit
      -> clean successful worktree
```

## Agent Roles

### Opus main session

Owns:

- user-facing interaction;
- requirement clarification;
- task slicing;
- deciding whether discovery is needed;
- synthesizing `contract.json`;
- approving contracts;
- deciding `risk_class`;
- deciding `execution_eligibility`;
- final gate decision;
- commit message generation;
- final local integration orchestration.

Does not own:

- bulk codebase scanning;
- implementation;
- external backend operation details;
- second-pass code review.

### `codebase-discovery`

Default model tier: low-cost.

Role: read-only structured fact collector for Opus main.

Allowed:

- inspect repository structure;
- identify package manager, scripts, languages, frameworks, tests, and CI conventions;
- identify relevant source files and tests;
- recommend verification candidates;
- identify scope candidates;
- identify risk hints and unknowns;
- output LLM-oriented JSON for Opus main.

Forbidden:

- modify files;
- generate final contracts;
- approve contracts;
- run external executor;
- implement code;
- decide product behavior;
- decide final scope or acceptance criteria.

The built-in Claude Code `Explore` agent is fallback only, for ad hoc exploration, custom agent unavailability, or quality comparison.

### `external-executor-adapter`

Default model tier: Haiku / low-cost.

Role: adapter and evidence handoff only.

Allowed:

- read approved contract;
- call `agent-loop init-run`;
- invoke configured backend in isolated worktree;
- collect backend raw outputs;
- call `agent-loop collect-evidence`;
- call `agent-loop validate-evidence`;
- return artifact paths;
- report blocked/failed/invalid states.

Forbidden:

- modify task contract;
- broaden scope;
- change acceptance criteria;
- directly implement code without backend;
- perform final review;
- claim success without valid evidence;
- stage, commit, push, or merge;
- hide backend failures;
- replace artifacts with natural-language summary.

Adapter may repair only packaging/path/evidence collection issues. It must not repair code, tests, contract scope, or backend implementation failures.

### External backend

Phase 1 backend: provisional `omp` one-shot CLI.

Role: external execution system.

Required behavior:

- operate only inside assigned worktree;
- use `contract.json` as source of truth;
- perform implementation;
- perform independent external review/verify;
- run repair loop within `repair_budget`;
- produce raw outputs for normalization;
- never write main worktree;
- never commit, push, or merge.

External backend may use the same provider/model internally, but worker/verifier contexts must be isolated. Verifier reviews artifacts, not worker self-summary.

### `sonnet-reviewer`

Role: second-pass artifact and diff reviewer.

Review order:

1. evidence validity;
2. scope and policy compliance;
3. verification sufficiency;
4. diff/code review;
5. merge recommendation.

Inputs:

- normalized evidence;
- `contract.json`;
- `diff.patch`;
- verification artifacts;
- sensitive audit;
- external review.

Forbidden:

- modify code;
- repair code;
- approve final merge;
- rely only on adapter/backend summary.

Output: structured `sonnet_review.json`.

## Command Flow

### `/agent-plan`

Purpose: produce one or more proposed task contracts.

Behavior:

- decide whether codebase discovery is needed;
- call `codebase-discovery` when repository facts are required;
- create static `plan.json`;
- create plan-local contracts under `contracts/`;
- contracts start as `proposed` unless Opus main explicitly approves;
- `/agent-plan` never executes tasks.

### `/agent-run`

Purpose: execute one approved contract.

Preconditions:

- `contract.json` schema valid;
- `status = approved`;
- `execution_eligibility.allowed = true`;
- valid `risk_class`;
- required verification present for code/behavior changes;
- clean main working tree.

Behavior:

- calls `external-executor-adapter`;
- adapter calls `agent-loop init-run`;
- creates isolated worktree;
- invokes backend;
- collects and validates normalized evidence.

### `/agent-review`

Purpose: run Sonnet reviewer on a completed task run.

Preconditions:

- normalized evidence exists;
- `diff.patch` exists;
- `final_evidence.json` exists;
- `external_review.json` exists.

Behavior:

- calls `sonnet-reviewer`;
- writes `sonnet_review.json`;
- validates review schema.

### `/agent-final-gate`

Purpose: Opus main creates final four-state decision.

Decision enum:

- `merge`
- `request_repair`
- `reject`
- `needs_user_decision`

Behavior:

- runs or consumes machine gate;
- reads contract, evidence, Sonnet review, diff, verification, sensitive audit;
- writes `opus_final_gate.json`;
- generates commit message when decision is `merge`;
- does not apply patch;
- does not commit.

### `/agent-integrate`

Purpose: local integration after final gate merge.

Preconditions:

- `opus_final_gate.decision = merge`;
- machine gate passed;
- main worktree clean;
- diff exists;
- commit message present in `opus_final_gate.json`.

Behavior:

- run final gate check;
- run `git apply --check`;
- apply normalized diff patch;
- run post-apply verification in main worktree;
- record `post_apply_verification.json`;
- stage expected changed files only;
- create local commit using Opus-provided message;
- write `integration_result.json`;
- remove `.worktrees/{task_id}/` after successful commit;
- never push, create PR, merge remote, deploy, or auto-resolve conflicts.

## CLI Scope

The Rust `agent-loop` CLI is deterministic infrastructure, not an agent runtime.

Owns:

- repo root detection;
- task ID generation;
- run directory initialization;
- clean worktree preflight;
- isolated worktree creation;
- artifact path conventions;
- schema validation;
- evidence collection helpers;
- sensitive audit mechanical detection;
- machine gate checks;
- hook entrypoints;
- export;
- list/cleanup maintenance.

Does not own in Phase 1:

- LLM reasoning;
- backend orchestration;
- direct `omp` RPC/daemon control;
- semantic code review;
- final decision;
- plan/task lifecycle synchronization beyond task-local artifacts;
- automatic repair orchestration beyond external backend loop.

## Artifact Layout

Project-local artifacts are stored under:

```text
.agent-runs/
  plans/
    {plan_id}/
      plan.json
      discovery/
        codebase_discovery.json
      contracts/
        contract-001.json
        contract-002.json

  tasks/
    {task_id}/
      status.json
      contract.json
      run_metadata.json
      runtime_preflight.json
      backend-output/
      normalized/
        changed_files.json
        diff.patch
        execution_trace.jsonl
        verification.json
        external_review.json
        sensitive_audit.json
        final_evidence.json
      repairs/
      machine_gate.json
      sonnet_review.json
      opus_final_gate.json
      post_apply_verification.json
      integration_result.json

.worktrees/
  {task_id}/
```

Recommended `.gitignore` entries:

```gitignore
.agent-runs/
.worktrees/
```

## ID Policy

### `plan_id`

Generated by `/agent-plan`.

Format:

```text
plan-YYYYMMDD-NNN
```

Example:

```text
plan-20260526-001
```

### `contract_id`

Generated by `/agent-plan`.

Format: plan-local sequence.

Examples:

```text
contract-001
contract-002
```

### `task_id`

Generated by `agent-loop init-run`.

Format:

```text
task-YYYYMMDD-NNN
```

Example:

```text
task-20260526-001
```

Worktree path:

```text
.worktrees/{task_id}/
```

## Contract Policy

`contract.json` is the first execution source of truth.

`codebase_discovery.json` is advisory only. External executor never reads discovery output directly.

### `task-contract-v1` fields

Phase 1 contract schema includes:

- `schema_version`
- `plan_id`
- `contract_id`
- `task_id` only in run-local copy
- `status`
- `objective`
- `non_goals`
- `risk_class`
- `risk_basis`
- `execution_eligibility`
- `scope`
- `acceptance_criteria`
- `required_verification`
- `optional_verification`
- `mutation_policy`
- `test_policy`
- `repair_budget`
- `discovery_usage`
- `approval`

### Contract status

Allowed values:

- `proposed`
- `approved`
- `superseded`
- `rejected`

`/agent-run` only accepts `approved` contracts.

### Risk class

Allowed values:

- `low`
- `normal`
- `high`

There is no `prohibited` risk class. Execution blocking is handled by `execution_eligibility`.

### Execution eligibility

Required field:

```json
{
  "allowed": true,
  "blocked_reason": null,
  "details": null
}
```

If blocked:

```json
{
  "allowed": false,
  "blocked_reason": "needs_user_decision",
  "details": "..."
}
```

`execution_eligibility` is written by Opus main and expresses semantic/policy eligibility. Runtime readiness is checked by `agent-loop init-run` preflight.

### Discovery usage

Required field. If discovery was used:

- `used = true`
- `source_path`
- `validation_status`
- `adopted`
- `modified`
- `rejected`
- `unresolved_unknowns`

If discovery was skipped:

- `used = false`
- `reason`

### Verification policy

For code/behavior changes, Opus main must define `required_verification` in contract.

External/backend-inferred verification is supplemental only and cannot replace contract-required verification.

After each external repair, required repair-scope verification must rerun. Before external `pass`, all final required verification must pass or be explicitly justified.

### Test policy

Allowed by default:

- add task-scoped tests;
- add regression tests;
- add missing assertions for acceptance criteria;
- update tests to match explicitly authorized behavior changes.

Forbidden unless explicitly authorized:

- delete tests;
- skip/xfail tests;
- weaken assertions;
- broaden mocks to hide failures;
- update snapshots;
- change test runner config;
- remove integration/e2e coverage.

Snapshot updates are forbidden by default.

## Discovery Policy

Primary Phase 1 discovery agent: custom `codebase-discovery`.

Fallback: built-in Claude Code `Explore`.

`codebase-discovery` output:

- primary format: JSON;
- audience: Opus main LLM;
- human Markdown summary: not required;
- authority: advisory only.

Required top-level fields:

- `repo_facts`
- `available_scripts`
- `relevant_files`
- `relevant_tests`
- `verification_candidates`
- `scope_candidates`
- `risk_hints`
- `unknowns`
- `discovery_limits`

`agent-loop validate-discovery` performs soft validation only. Discovery validation failure does not directly block the workflow. It requires retry, Opus normalization, or fallback manual contract synthesis.

## Evidence Policy

`agent-loop collect-evidence` only collects deterministic artifacts and normalizes known backend outputs. It must not evaluate semantic correctness, repair code, infer missing verification success, or fabricate absent evidence.

Backend raw output is preserved under `backend-output/`.

Formal review inputs live under `normalized/`.

External backend does not need to natively emit the workflow schema. The adapter plus `agent-loop collect-evidence` normalize raw output into standard artifacts.

If normalization fails, run status becomes invalid/blocked and must not proceed to Sonnet code review.

### Required normalized artifacts

- `changed_files.json`
- `diff.patch`
- `execution_trace.jsonl`
- `verification.json`
- `external_review.json`
- `sensitive_audit.json`
- `final_evidence.json`

### Execution trace

Phase 1 does not require full tool-event trace.

Allowed completeness:

- `full`
- `partial`
- `unavailable`

Completeness and limitations must be declared.

### Sensitive audit

Phase 1 sensitive audit uses deterministic detection from:

- `git diff --name-only`
- `git status --porcelain`
- contract mutation policy
- execution trace when available
- backend command log when available

Backend claims are supplementary only and never the source of truth.

Audit limitations must be recorded.

## External Review and Repair Policy

Phase 1 requires `external_review.json`.

Source preference:

1. backend-native review/verify;
2. second context-isolated backend invocation as independent review.

Forbidden:

- worker self-summary as external review;
- Haiku adapter writing external review;
- Sonnet reviewer replacing external review.

Internal external repair may happen only within:

- original contract;
- original scope;
- existing sensitive authorizations;
- `repair_budget`.

Repair must stop and return blocked if it needs:

- scope expansion;
- new sensitive authorization;
- contract clarification;
- public API/product/security/data tradeoff;
- budget beyond `repair_budget`.

Default `repair_budget = 2`.

Only narrow cases may explicitly raise it to 3.

## Review Policy

Sonnet reviewer must use gate-based review:

1. evidence validity;
2. scope/policy compliance;
3. verification sufficiency;
4. diff/code review;
5. final recommendation.

Output: `sonnet_review.json`.

Sonnet reviewer cannot modify code or trigger repair directly.

If Sonnet recommends repair, Opus main must translate blocking findings into a repair contract or reject/ask user.

Non-blocking findings do not enter automatic repair.

## Final Gate and Integration Policy

### Final gate

`/agent-final-gate` writes `opus_final_gate.json`.

Decision enum:

- `merge`
- `request_repair`
- `reject`
- `needs_user_decision`

Merge is allowed only when:

- external final verdict is pass;
- Sonnet reviewer recommends ship;
- machine gate passes;
- evidence is valid;
- scope/policy checks pass;
- verification is sufficient;
- sensitive audit is clean or authorized;
- residual risks are acceptable.

`/agent-final-gate` does not apply patches or commit.

It must write commit message into `opus_final_gate.json` when decision is `merge`.

### Integration

`/agent-integrate` applies and commits locally.

Required steps:

1. verify final gate merge;
2. verify machine gate pass;
3. require clean main worktree;
4. run `git apply --check`;
5. apply normalized patch;
6. run post-apply verification in main worktree;
7. record `post_apply_verification.json`;
8. stage expected files only;
9. create local commit using Opus final-gate message;
10. write `integration_result.json`;
11. remove successful worktree.

Forbidden:

- push;
- create PR;
- merge remote;
- deploy;
- auto-resolve conflicts;
- commit if post-apply verification fails;
- stage all files;
- commit unrelated dirty files.

Staging must use expected changed files only. `git add .` and `git add -A` are forbidden.

Patch conflict policy:

- always run `git apply --check` first;
- if check/apply fails, stop as blocked;
- do not auto-resolve;
- do not continue to post-apply verification;
- do not commit partial changes.

## Hook Policy

Use hooks plus scripts/CLI, not hooks alone.

Hooks provide lifecycle enforcement. Rust CLI provides deterministic logic.

Primary location: user-level Claude Code settings, because this is a reusable personal workflow.

Project-level hardening is optional for important or team projects.

Required hook categories:

- `SubagentStop` for validating adapter/reviewer artifacts;
- `PreToolUse` for guarding git apply/commit/merge/push-style actions.

Optional hook category:

- `Stop` for reminders about pending runs.

`git-guard` blocks unsafe manual integration. For `/agent-integrate`, it checks the integration context files produced during the command before allowing commit.

Default guard behavior:

- no agent run detected: allow;
- pending agent run detected: block unsafe integration actions;
- final gate merge and integration preconditions satisfied: allow.

## Plan and Status Policy

### `plan.json`

Each plan directory contains lightweight `plan.json`.

Purpose:

- organize contracts;
- record dependencies;
- record suggested execution order;
- reference discovery.

It does not track task-run lifecycle in Phase 1.

`plan.json` is not execution source of truth and is not a Phase 1 runtime status file.

Execution source of truth is individual approved `contract.json`.

Task progress is tracked by task-local artifacts under `.agent-runs/tasks/{task_id}/`.

### `status.json`

Each task has lightweight `status.json`.

Allowed status values:

- `active`
- `committed`
- `blocked`
- `abandoned`

If `blocked`, include:

- `blocked_reason`
- `details`

No detailed workflow state machine in Phase 1. Detailed progress is derived from artifacts.

No separate manual `mark` command in Phase 1.

## Cleanup and Retention Policy

### Worktrees

After successful `/agent-integrate` local commit:

- automatically remove `.worktrees/{task_id}/`;
- retain `.agent-runs/tasks/{task_id}/` evidence.

If external run, final gate, or integration is failed/blocked/invalid:

- keep worktree;
- keep evidence.

### Evidence

`.agent-runs/tasks/{task_id}/` is retained by default.

Cleanup is explicit only.

`agent-loop cleanup`:

- defaults to dry-run;
- destructive cleanup requires `--confirm`;
- destructive cleanup requires explicit selector.

Supported selectors:

- `--task-id`
- `--plan-id`
- `--status`
- `--older-than`
- `--worktrees`
- `--evidence`

`agent-loop list-runs` defaults to showing actionable runs:

- active;
- blocked;
- cleanup-eligible.

Failure details are derived from artifacts such as `final_evidence.json`, `machine_gate.json`, and `integration_result.json`. Use `--all` for full history.

## Export Policy

`agent-loop export` produces sanitized audit summaries by default.

Default includes:

- task ID;
- plan ID and contract ID when available;
- contract summary;
- changed files;
- diff summary;
- verification commands and exit codes;
- external verdict;
- Sonnet verdict;
- Opus final decision;
- blocking issues;
- residual risks;
- accepted risks.

Default excludes:

- full command output;
- full execution trace;
- absolute paths;
- environment variables;
- raw model outputs;
- secrets/tokens;
- local usernames.

Full export requires explicit risk acknowledgement.

## Phase 2 Deferred

- Replace one-shot CLI backend mode with task-scoped RPC/daemon.
- Add separate worker/verifier sessions with full event capture.
- Capture full tool-event `execution_trace.jsonl`.
- Support backend orchestration inside Rust CLI.
- Evaluate `zeroshot`, `goose`, `mini-swe-agent`, `opencode`, or other backends.
- Add parallel execution only for disjoint file scopes.
- Add richer metrics and benchmarking.
- Add project-level hardening for selected repos.
- Add stronger sandboxing if worth the operational cost.
- Add `agent-loop update-plan` or richer plan/task lifecycle synchronization only if Phase 1 evidence shows it is needed.

## Invariants

- Opus main owns contract synthesis and final decision.
- `contract.json` is the first execution source of truth.
- Discovery output is advisory and never read directly by external executor.
- External executor runs only in isolated worktree.
- External executor never modifies main worktree.
- External executor never commits, pushes, or merges.
- Haiku adapter is adapter/evidence handoff only, not implementer or reviewer.
- External review is required before Sonnet review.
- Sonnet reviewer reviews normalized evidence and diff, not natural-language summaries.
- Sonnet reviewer cannot modify code.
- Final gate must produce `opus_final_gate.json` before integration.
- `/agent-final-gate` does not apply patch or commit.
- `/agent-integrate` may create local commit only after final gate, machine gate, clean worktree, patch apply, and post-apply verification pass.
- `/agent-integrate` never pushes, creates PRs, deploys, or merges remote branches.
- Stage expected files only; never `git add .` or `git add -A`.
- Successful commit removes worktree but retains evidence.
- Failed/blocked/invalid runs retain worktree and evidence.
- `plan.json` remains a static planning manifest in Phase 1; task lifecycle is derived from task-local artifacts.
- Phase 1 does not implement a full workflow engine.
- Phase 1 scope is frozen; new core agents, commands, required artifacts, backends, lifecycle states, or permission models are deferred.

