---
name: build-epic
description: "The task-group build cycle. Reads the state, the execution status, and one task group, then advances the build flow one step per invocation in resume mode. Use it instead of an ad-hoc execute-plan for release work."
kind: prose
---

# Build Epic

Scope: one story. Called by orchestrate-project in its build phase. Not a
replacement for orchestrate-project.

Orchestrate the build flow for a single task group: survey, plan tasks, kickoff, TDD,
verify, audit, commit, release.

> **HARD GATE**: set `active_flow: build_group` and `active_group: eNN` in `.agent/tasks/state.yml` before starting.
>
> **HARD GATE**: not on `main` or `master` before step 3 (kickoff-branch).

## Steps

| Step | Skill or action                                                |
| ---- | -------------------------------------------------------------- |
| 0    | `security-review`: threat-model the group scope                |
| 1    | `survey-context`: confirm the group and the story              |
| 2    | `plan-work`: flesh out the story tasks in the task group       |
| 3    | `kickoff-branch`: a feature branch and a clean baseline        |
| 4    | `develop-tdd`: red-green per task                              |
| 5    | `verify-work`: UAT and the mechanical gates                    |
| 6    | `audit-code`: a non-optional gate. A fail loops back to step 4 |
| 7    | `commit-message`: the Conventional Commits draft               |
| 8    | `release-branch`: PR or solo land                              |

## Process

1. Read `.agent/tasks/state.yml`, `.agent/tasks/execution-status.yml`,
   `.agent/tasks/release-plan.yml`, and the active task group.
   - On story start (step 1): record the `started_at` ISO-8601 timestamp under the
     story key in the execution status. Record a task's progress through the
     `truenorth_record_task` tool.
2. **Step 0, threat model**: run `security-review` against the group scope. Write the
   threat model for the task group.
3. **Assess impact (step 2)**: before writing tasks, run `assess-impact` on the
   proposed change. When the risk score exceeds 7, gate and require a `grill-me`
   session. Write the impact report. For net-new code with no existing dependents,
   skip.
4. When the current step is missing, set it to 1.
5. Run only the current step in resume mode, unless the user asked for a full
   auto-run.
6. After the step verify passes, advance the phase through the
   `truenorth_advance_phase` tool and record the step in the state.
7. On story complete, set the story key to `done` in the execution status.
   - On story complete (step 8): record the `completed_at` timestamp under the same
     story key.

     ```yaml
     stories:
       e01s01:
         status: done
         started_at: "2026-07-12T16:00:00-03:00"
         completed_at: "2026-07-12T18:45:00-03:00"
     ```

8. **Traceability refresh**: before step 8, refresh the traceability data and
   surface any dark, orphan, or stale finding for the just-built group in the verify
   summary. When the refresh is unavailable, note "trace skipped" and continue. A
   trace failure must be visible, not silent. Blocking is the job of `gate-trace`.

### Step 6, the audit-code gate (non-optional)

After step 5 (verify-work) completes, step 6 runs `audit-code` in gate mode.

1. **Run the audit**: invoke `audit-code` in gate mode on the complete diff for the
   story.
2. **Pass**: every checklist section passes. Advance to step 7. Record the audit
   result as pass in the state.
3. **Fail**: one or more sections fail. Reset the current step to 4 (develop-tdd)
   and record the failing section ids. Record the audit result as fail. Do NOT
   advance past step 6 until the audit passes.
4. **Audit artifact**: save the full audit report for the story regardless of the
   result, for reviewer traceability.
5. **Enforce F.I.R.S.T**: after the audit passes, run `enforce-first` on the new or
   modified tests. Append any F.I.R.S.T violation to the audit report. A failing
   criterion triggers the same loop-back to step 4.

## --fast mode

Coalesce the read-and-report steps to reduce token overhead. Activate with
`build-epic --fast`. It combines survey and plan into one invocation, and audit and
commit-message into one. It does NOT skip any checklist item. The kickoff, develop,
verify, and release steps still run sequentially, since they need user interaction
or branch state.

## Handoff

Write `handoff.next_skill` and `handoff.context` in `state.yaml` when pausing
mid-group.

## Verify

Confirm the cockpit files exist (`.agent/tasks/state.yml`, `.agent/tasks/execution-status.yml`,
`.agent/tasks/release-plan.yml`) and the gate skills are present (assess-impact,
audit-code, security-review).
