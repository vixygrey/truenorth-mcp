---
name: execute-plan
description: "Batch-execute tasks from the active plan sequentially, with a human checkpoint after each step. Use it when the user has an approved plan and wants step-by-step oversight."
kind: prose
---

# Execute Plan

Execute the active task ledger one task at a time and show evidence before
advancing.

> **HARD GATE**: Do not proceed on `main` or `master`. Run `kickoff-branch`
> first.
>
> **HARD GATE**: Every selected task must have a runnable verify command. If the
> plan is incomplete, run `plan-release`, then `plan-work` or `build-group`.

## Process

### 1. Read the plan

Read `.agent/profile.yml`, `.agent/tasks/state.yml`, and
`.agent/tasks/release-plan.yml`. Select `active_task`. Read `active_group` and its
capsule only when the active profile groups work. Parse `depends-on` for execution
waves.

> **CONTEXT ISOLATION**: pass decisions through the canonical `handoff` in
> `.agent/tasks/state.yml`, never through assumed chat history.

Confirm the step count, skip or reorder choices, and stop point with the user.

### 2. Execute step by step

For each selected task:

1. Announce its description and verify command.
2. Implement directly or delegate a disjoint execution wave.
3. Run its verify command. Do not advance on failure.
4. Record non-obvious decisions in the canonical handoff.
5. Ask to proceed unless autonomous mode is active.

After the last task, run `verify-work`. Update
`.agent/tasks/execution-status.yml` for the completed task and optional group.

### 3. Blockers

Report the blocker, ask whether to skip, adapt, or stop, and update the owned task
artifact when the plan changes.

### 4. Final report

Suggest the verify arc, then the ship steps: `verify-work` →
`validate-contracts` → `smoke-test` → `run-evals` → `audit-code` →
`request-review` → `respond-review` → `commit-message` → `release-branch`.

## Rules

- If a verify command passes but observed behavior is wrong, return to the
  execution loop.
- Use the active profile's grouping vocabulary. Do not invent an epic, milestone,
  ticket, story, WSJF score, or BCP estimate.
