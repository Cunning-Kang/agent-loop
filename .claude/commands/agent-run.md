# /agent-run

Execute one approved task contract end-to-end via the
`external-executor-adapter` subagent.

## Preconditions

`/agent-run` MUST refuse to dispatch unless all of the following are true:

1. The referenced `contract.json` validates against `task-contract-v1`
 (use `agent-loop gate-check --plan-id <plan_id> --check-contracts`).
2. `contract.status == "approved"`.
3. `contract.execution_eligibility.allowed == true`.
4. `contract.risk_class` is one of `low | normal | high`.
5. For code/behavior changes, `contract.required_verification` is non-empty.
6. The main worktree is clean (`git status --porcelain` is empty).

If any precondition fails, `/agent-run` reports `blocked` and stops.

## Behavior

1. Run the preflight checks above.
2. Call `external-executor-adapter` with the approved contract and the
 generated `task_id`.
3. Adapter performs the deterministic Phase1 path:
 - `agent-loop init-run`
 - isolated worktree at `.worktrees/{task_id}/`
 - backend invocation inside the worktree
 - raw outputs under `.agent-runs/tasks/{task_id}/backend-output/`
 - `agent-loop collect-evidence`
 - `agent-loop validate-evidence`
4. `/agent-run` forwards the adapter status to Opus main:
 - `ready` — evidence valid; proceed to `/agent-review`.
 - `blocked | failed | invalid` — stop and surface the verbatim reason.

## Public CLI path (testable)

This command is implemented by chaining these public CLI calls. Tests use
the same path to prove end-to-end behavior:

```sh
agent-loop init-run --plan-id <plan_id> --contract-id <contract_id> \
 --task-id <task_id> --repo-root <repo>
mkdir -p .worktrees/<task_id>
# backend invocation writes raw outputs under
# .agent-runs/tasks/<task_id>/backend-output/
agent-loop collect-evidence --task-id <task_id> \
 --review-verdict <pass|fail|blocked> \
 --execution-completeness <full|partial|unavailable> \
 --repo-root <repo>
agent-loop validate-evidence --task-id <task_id> --repo-root <repo>
```

## Forbidden

- `/agent-run` MUST NOT modify the contract.
- `/agent-run` MUST NOT broaden scope.
- `/agent-run` MUST NOT stage, commit, push, or merge.
- `/agent-run` MUST NOT perform final review.
- `/agent-run` MUST NOT hide backend failures.

## Out of scope (Phase1)

- task-scoped RPC / daemon orchestration;
- parallel execution;
- multi-backend selection;
- automatic repair beyond what the backend does internally.

## Reference

- ADR-001 §`/agent-run`
- ADR-001 §`external-executor-adapter`
- PRD §Implementation Decisions
