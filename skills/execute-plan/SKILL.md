---
name: execute-plan
description: "Batch-execute the tasks from the active task group sequentially, with a human checkpoint after each step. Use it when the user has an approved plan and wants step-by-step oversight."
---

# Execute Plan

Execute tasks from the **active group** (the task group story `tasks[]`) one at a time, showing evidence after each step before proceeding.

> **HARD GATE** — Do NOT proceed if on `main` or `master`. Run `kickoff-branch` first.
>
> **HARD GATE** — The active group must exist with runnable `verify` on each task. If missing, run `plan-release` then `plan-work` or `build-epic`.

## Process

### 1. Read the plan

Read `.agent/tasks/state.yml` (`active_group`, `active_story`) and the matching task group under `.agent/tasks/`. Parse `depends-on` in task descriptions for execution waves.

> **CONTEXT ISOLATION** — Spawn each skill with a **fresh context window**. Pass decisions only through `.agent/tasks/state.yml` `handoff` — never rely on prior chat history.

Confirm with the user: step count, skip/reorder, stop-after step.

### 2. Execute step by step

For each task in the active story:

**a. Announce** — task `desc` and `verify` command.

**b. Execute** — code or `delegate-task` / `dispatch-agents` for waves.

**c. Run verify** — must be green before advancing.

**d. Log** — non-obvious decisions in `.agent/tasks/state.yml` under `decisions[]` or `handoff` block.

**e. Checkpoint** — ask to proceed unless autonomous mode requested.

**f. Story UAT** — after last task, run manual verification script from story notes or `verify-work`.

On verify failure: fix and re-run; never advance on red.

Update `.agent/tasks/execution-status.yml` when a story or group completes.

### 3. Blockers

Report blocker; ask skip/adapt/stop; update the task group if plan changes.

### 4. Final report

Suggest: `verify-work` → `run-evals` → `audit-code` → `simulate-agents` → `commit-message` → `release-branch`

## Rules

- **Loop until behavioral correctness is verified**: if a verify command passes but the observed behavior is still wrong, return to step 1 and run the execution cycle again.

<!-- story: e02s03 -->
<!-- story: e05s02 -->
<!-- story: e08s01 -->
