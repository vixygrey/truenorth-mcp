---
name: survey-context
description: "A per-task context bootstrap. Reads the existing narrative docs and the project tech-stack note to map the current lifecycle phase and suggest the next skill. Use it at the start of any task, when returning after a break, or when unsure what to do next. For deriving a tech-stack note from scratch, use map-codebase first."
kind: prose
---

# Survey Context

Read the project current state and give a phase map plus a next-skill
recommendation. This is the "where am I?" skill. Run it at the start of every task.

> Use this versus map-codebase: `survey-context` consumes the existing narrative docs (fast, it does not re-derive). `map-codebase` builds the tech-stack note from scratch by scanning the codebase. Run `map-codebase` when the tech-stack note does not exist yet. Run `survey-context` when it does.

> **HARD GATE**: read the cockpit and narrative files before you suggest a next step. When the state is stale or contradicts the codebase, request clarification rather than assuming intent.

The six phases: discover, design, plan, execute, review, integrate.

## Process

### 1. Read the conventions

When the project conventions doc exists at the root, read it first. It contains the
rules every agent must follow in this project. Read it through the
`truenorth://conventions` resource when available.

### 2. Read the cockpit and narrative

Scan the runtime state under `.agent/` and the narrative under `specs/`. Read the cockpit through the `truenorth://state` and
`truenorth://cockpit` resources when available. For each YAML file, note whether it
exists, whether the keys are populated, and the `handoff.next_skill`.

- `.agent/tasks/state.yml`: the session, the active flow, the active group, the git state, the
  handoff.
- `.agent/tasks/release-plan.yml`: the target version, the WSJF group index.
- `.agent/tasks/execution-status.yml`: the flat story and group status.
- `.agent/product/`, the task group under `.agent/tasks/`, the bug references under `.agent/tasks/bugs.yml`: scope, task groups, bug references.

### 3. Read the project agent guide

When the project agent guide exists at the root, read it for the stack, the
commands, the architecture, and the conventions.

### 4. Check the VCS state

Read the `vcs.kind` value from `state.yaml`. Use the native identity through the
git-context tool.

- Git: status, the last few commits, the current branch.
- Jujutsu: status and the recent change log.

For Jujutsu, compare the stable change ids and bookmarks with the `vcs` block. Never
interpret a colocated Git detached HEAD as branch state.

### 5. Map the lifecycle phase

Identify the current phase from what you found.

| Phase         | Signals                                               |
| ------------- | ----------------------------------------------------- |
| **Discover**  | No product scope yet, or only rough notes             |
| **Design**    | Scope exists but no release plan                      |
| **Plan**      | The release plan exists, on `main` or `master`        |
| **Initiate**  | On a feature branch, no code change yet               |
| **Execute**   | `active_flow: build_group`, a task group in progress  |
| **Verify**    | Implementation done, run `verify-work` or `run-evals` |
| **Bug**       | `active_flow: fix_bug`, or an open bug report         |
| **Review**    | All code written, no PR yet                           |
| **Integrate** | PR open, tests passing                                |
| **Sustain**   | Ongoing, no active task                               |

Prefer the `active_flow` and `handoff.next_skill` from `state.yaml` when present.

### 6. Suggest the next skill

Recommend the most useful next step for the phase and state.

- In the plan or bug phase and on `main`: suggest `kickoff-branch`.
- In the initiate phase: suggest `develop-tdd` or `execute-plan`.
- In the execute phase: suggest `build-epic` to resume, or `develop-tdd` for the
  active story.
- In the verify phase: suggest `verify-work` or `run-evals`.

Be specific. Name the exact skill and why. When several options exist, list them in
priority order.

### 7. Surface the blockers

Report a blocker before a recommendation: a broken baseline test, an open bug report
with no active fix branch, a group task with no verify command, or a git hash in
`state.yaml` that is stale versus the working tree.

### 8. Record the story-start timestamp

At story start, write `metrics.story_start` with the current ISO-8601 timestamp to
`.agent/tasks/state.yml` as an informational progress marker only, not a measurement input.

## Utility outputs

- **list-groups**: loop through the task groups and print a summary of the story
  counts per group.
- **check-gates**: print the active flow, validate the state YAML, then show the git
  or jj status through the git-context tool. Use it before a handoff.

## Handoff

Gate: READY. Next: plan-work.
Writes: `state.yaml` `handoff.next_skill = plan-work`.
