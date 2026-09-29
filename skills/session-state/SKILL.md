---
name: session-state
description: "Track implementation decisions and progress in .agent/tasks/state.yml to prevent context rot. Use it at the start of a session to load context, and whenever a significant decision is made or a milestone is reached."
kind: prose
---

# Session State

> **HARD GATE**: session state must stay synchronized with Git. When
> `.agent/tasks/state.yml` conflicts with the working tree, halt and ask for
> clarification. Do not assume either source is correct.

Track the current work item, decisions, pending tasks, and open questions so a new
session can resume without replaying chat history.

## Methodology gate

Read `.agent/profile.yml` before setting grouping fields:

| Profile           | Grouping field                                         |
| ----------------- | ------------------------------------------------------ |
| `epic-based`      | required `group_id`, `group_kind: epic`                |
| `milestone-based` | required `group_id`, `group_kind: milestone`           |
| `issue-per-task`  | optional `group_id`, `group_kind: ticket` when grouped |
| `kanban`          | omit `group_id` and `group_kind`                       |
| `generic`         | omit `group_id` and `group_kind`                       |

Use `task`, `work item`, and `group` in shared state. Use `epic` only under the
`epic-based` profile. Use `story` only when the project's existing planning
artifacts explicitly adopt story decomposition.

## Canonical handoff

Every current skill uses this field set:

```yaml
handoff:
  next_skill: null
  last_step_completed: null
  context: null
  open_decisions: []
  required_reading: []
  group_id: null
  artifacts_summary: null
  git_context: null
```

`context` is concise free-form resume context. Put unresolved choices, including
design uncertainty, in `open_decisions`. The lifecycle runtime owns
`artifacts_summary` and `git_context`. Omit `group_id` for an ungrouped profile.
Legacy fields remain readable, but current skills do not write `group`, `epic`,
`uncertain_decisions`, or a cycle-local second `next_skill`.

## Workflow

### Initialize

When `.agent/tasks/state.yml` does not exist, or a new major phase starts:

- Read `.agent/profile.yml`, `.agent/tasks/release-plan.yml`, and product scope.
- Get the branch and short hash through `get_git_context`.
- Create the state with `active_flow`, `active_task`, the optional active group,
  the Git block, the canonical handoff, and the relevant cycle.

### Load

- Read `.agent/tasks/state.yml`.
- Read `.agent/tasks/execution-status.yml` for task or work-item progress.
- Verify that the recorded branch and hash match the working tree.

### Update

- Patch `.agent/tasks/state.yml` through lifecycle tools when a tool owns the
  mutation; otherwise use one direct transactional edit.
- Update `handoff.last_step_completed`, `handoff.context`, and
  `handoff.open_decisions`.
- Set `handoff.group_id` only when the active profile groups work.
- Advance the cycle counter at a completed step.

## Universal checkpoint pattern

| Flow                | Cycle key       | Step field      |
| ------------------- | --------------- | --------------- |
| execute-group       | `group_cycle`   | `current_step`  |
| fix-bug             | `bug_cycle`     | `current_step`  |
| orchestrate-project | `project_cycle` | `current_phase` |

After each step, advance the counter and update `handoff.next_skill`. On resume,
continue from the recorded step. Do not restart at step 1.

### Reset state

Set `active_task`, optional group fields, and the current cycle step to null when a
phase ends or the work context changes.

### Compact state

Move a system-wide decision to a global ADR and a group-scoped decision to a
group-local ADR. Remove resolved `handoff.open_decisions`. Retain only information
needed for the next step.

## File format

```yaml
active_flow: execute_group # planning | execute_group | fix_bug
active_task: "Implement profile-neutral planning"
active_group: null
phase: execute
group_cycle:
  mode: checkpoint
  current_step: develop-tdd
  completed_steps: [kickoff-branch]
bug_cycle:
  current_step: null
  completed_steps: []
git:
  branch: feat/452-neutral-skills
  hash: abc1234
handoff:
  next_skill: develop-tdd
  last_step_completed: kickoff-branch
  context: "Implement the next failing task."
  open_decisions: []
  required_reading:
    - .agent/tasks/release-plan.yml
  group_id: null
  artifacts_summary: null
  git_context: null
```

## Anti-patterns

- Do not copy the release plan or task details into state.
- Do not write profile-specific group fields under an ungrouped profile.
- Do not record task status in the release plan. Status belongs in
  `.agent/tasks/execution-status.yml`.
- Do not leave state stale after a significant decision or completed step.

## Verify

Confirm `.agent/tasks/state.yml` parses, uses the canonical handoff fields, and
matches the working tree's Git context.
