# /agent-review

Run Sonnet reviewer on a completed task run to produce structured `sonnet_review.json`.

## Preconditions

`/agent-review` MUST refuse to dispatch unless all of the following are true:

1. `contract.json` exists under the task directory.
2. `status.json` exists under the task directory.
3. Normalized evidence directory exists at
   `.agent-runs/tasks/{task_id}/normalized/`.
4. All seven normalized artifacts exist:
   - `changed_files.json`
   - `diff.patch`
   - `execution_trace.jsonl`
   - `verification.json`
   - `external_review.json` (produced by independent worker review, not worker self-summary)
   - `sensitive_audit.json`
   - `final_evidence.json`
5. `external_review.json` exists — **must not be written by the Haiku adapter**
   (Haiku adapter cannot write external review per ADR-001 constraints).
6. `agent-loop validate-evidence --task-id <task_id>` succeeds
   (normalized artifacts validate against schemas).

If any precondition fails, `/agent-review` reports `blocked` with the verbatim
reason and stops.

## External Review Requirement

External review MUST be written by the external backend's independent reviewer
component, NOT by the worker agent (worker self-summary is forbidden per ADR-001
§External backend). The `external_review.json` artifact proves this separation.
If `external_review.json` appears to be worker self-summary (e.g., suspiciously
generic findings, no independent verification), the review MUST be flagged.

## Behavior

1. Run preconditions check above.
2. Call `sonnet-reviewer` subagent with:
   - `task_id`
   - paths to all normalized evidence artifacts
   - `contract.json`
3. Sonnet reviewer produces `sonnet_review.json` following the five-gate order:
   `evidence_validity` → `scope_policy` → `verification` →
   `diff_code_review` → `merge_recommendation`.
4. Validate the produced review:
   ```
   agent-loop validate-sonnet-review --task-id <task_id> --repo-root <repo>
   ```
5. If validation fails, report failure and stop.
6. Forward review status to Opus main:
   - `ready` — review valid; proceed to `/agent-final-gate`.
   - `failed` — review invalid or review process failed.

## Public CLI Path (Testable)

```sh
# Preconditions
agent-loop validate-evidence --task-id <task_id> --repo-root <repo>
# Must succeed before /agent-review proceeds

# Review validation (after sonnet-reviewer writes sonnet_review.json)
agent-loop validate-sonnet-review --task-id <task_id> --repo-root <repo>
# Must succeed for /agent-review to report ready
```

## Forbidden

- `/agent-review` MUST NOT modify code.
- `/agent-review` MUST NOT repair code.
- `/agent-review` MUST NOT approve final merge.
- `/agent-review` MUST NOT proceed without valid `external_review.json`.
- `/agent-review` MUST NOT accept worker self-summary as external review.
- `/agent-review` MUST NOT use Haiku adapter to write external review.

## Output

- `.agent-runs/tasks/{task_id}/sonnet_review.json` — structured review with five gates.
- Status: `ready` (review valid) or `failed` (review invalid or preconditions unmet).

## Reference

- ADR-001 §`/agent-review`
- ADR-001 §`sonnet-reviewer`
- PRD §Implementation Decisions
