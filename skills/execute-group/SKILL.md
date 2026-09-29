---
name: execute-group
description: "Execute the active task or profile-defined group through planning, implementation, verification, review, and release gates. Use it when approved work is ready to build in checkpoint or autonomous mode."
kind: prose
---

# Execute Group

Own execution progress for the selected scope. Read `.agent/profile.yml` before
interpreting grouping fields or naming the scope in user-facing output.

| Profile           | Selected scope                                     | Grouping word |
| ----------------- | -------------------------------------------------- | ------------- |
| `epic-based`      | required `active_group` and its ordered work items | epic          |
| `milestone-based` | required `active_group` and its ordered work items | milestone     |
| `issue-per-task`  | optional ticket group, otherwise `active_task`     | ticket        |
| `kanban`          | `active_task`; omit group fields                   | none          |
| `generic`         | `active_task`; omit group fields                   | none          |

Shared state and artifact instructions use `group`, `task`, and `work item`.
Do not invent profile-specific grouping for ungrouped work.

> **HARD GATE**: set `active_flow: execute_group`, the `active_task`, and
> `group_cycle.mode` in `.agent/tasks/state.yml`. Set `active_group` and
> `handoff.group_id` only when the profile groups work.
>
> **HARD GATE**: do not implement on `main` or `master`. Run `kickoff-branch`
> and require a green baseline before changing code.
>
> **HARD GATE**: every selected task must have a runnable verify command. Route
> incomplete planning to its owning planning skill before execution.

## Modes

- **checkpoint**: the default. Confirm scope and order before execution. Pause
  after each verified work item and before any skip, reorder, or adaptation.
- **autonomous**: continue after successful work-item verification without
  routine progress prompts.

Both modes stop for a failed verify or behavior smoke, an unresolved security,
audit, traceability, or review blocker, inconsistent planning artifacts, an
external dependency, a requested scope change, or a destructive safety gate.
Autonomous mode never auto-approves `release-branch`.

Persist the selected mode and `group_cycle.current_step`. Resume from that
cursor. Never restart the cycle or skip a previously failed gate.

## Artifact ownership

- Read `.agent/tasks/release-plan.yml` for ordering and verify commands.
- Read the selected group manifest only when the profile groups work.
- Read the selected work-item specification and task ledger.
- Write execution progress and timestamps to
  `.agent/tasks/execution-status.yml`.
- Write only the cursor and canonical handoff to `.agent/tasks/state.yml`.
- Do not copy status into state or rewrite planning artifacts to record progress.

Preserve unknown fields in every artifact.

## Process

### 1. Resolve the scope

Read the profile, state, execution status, release plan, and selected task
ledger. Reject a missing required group, a `group_kind` that conflicts with the
profile, or an ambiguous active task. Parse declared dependencies and preserve
the plan's order when no dependency changes it.

Run `survey-context` to confirm the active task and optional group. Run
`security-review` against the selected scope. Run `assess-impact` before work on
a shared module; risk above 7 requires `grill-me`. Run `plan-work` only when the
selected work item lacks runnable detail.

### 2. Establish the branch

Run `kickoff-branch` when the branch gate has not passed. A red baseline blocks
execution and routes to the baseline owner. Record the branch context in the
canonical handoff.

### 3. Execute the task ledger

For each selected task in dependency order:

1. Record its description and verify command.
2. Run `develop-tdd` through RED, GREEN, and REFACTOR for each behavior.
3. Delegate only a disjoint wave whose dependencies and touched files do not
   conflict. The conductor retains ordering, state, and verification ownership.
4. Run the task verify command. Do not advance on failure or when observed
   behavior remains wrong.
5. Update execution status and the canonical handoff.
6. In checkpoint mode, pause after the verified work item. In autonomous mode,
   continue unless a stop condition applies.

### 4. Verify and audit

Run `verify-work` on the completed work-item scope. Require an actual-surface
behavior smoke and every risk-required scenario.

Run `trace-requirement report`. Missing, stale, or incomplete required evidence
blocks release. Route each gap to its planning, implementation, or verification
owner.

Run `audit-code --gate` on the complete diff. A blocker returns execution to
`develop-tdd`. When project policy adopts F.I.R.S.T, run `enforce-first` on
changed tests.

### 5. Review and release

Run `request-review` when the risk policy requires independent review. Route
blocking findings through `respond-review`, then repeat affected verification
and audit gates.

Run `commit-message` after all required evidence is green. Run
`release-branch` only when the active branch's declared scope is complete. Its
safety confirmation remains mandatory in both modes.

### 6. Complete or advance

Mark the completed work item in `.agent/tasks/execution-status.yml` and record
`started_at` and `completed_at`. Advance to the next selected work item, or clear
the cycle cursor and hand off to the profile-appropriate integration step.

## Blockers

Record the blocked task, failed gate, concrete evidence, proposed next action,
and artifact owner. Update `handoff.next_skill`, `handoff.context`,
`handoff.open_decisions`, and `handoff.required_reading`. A plan change must go
to the skill that owns the affected artifact.

## Verify

Confirm the selected scope matches the profile, every task has a verify command,
the branch baseline is green, execution status matches the cycle cursor, and the
security, TDD, verification, traceability, audit, review, and release gates all
remain reachable.
