---
name: session-state
description: 'Track implementation decisions and progress in .agent/tasks/state.yml to prevent context rot. Use it at the start of a session to load context, and whenever a significant decision is made or a milestone is reached.'
---

# Session State

> **HARD GATE**: the session state MUST stay synchronized with the git state. When `state.yaml` conflicts with the working tree, halt and ask for clarification. Do NOT assume the state is correct.

Track the current state of implementation, the decisions made, the pending tasks,
and the open questions, so continuity holds across a session boundary and context
rot does not set in.

Session-state implements the isolate strategy: each agent gets exactly the context
it needs, no more, by recording decisions so the next agent can cold-start without
replaying history. The strategies work together: session-state handles isolation,
terse-mode handles compression, survey-context handles selection, and the
conventions ensure token-efficient writing.

## Goal

Maintain a single source of truth for the current session in `.agent/tasks/state.yml`.
Read and write it through the `truenorth://state` resource when available. This
complements the long-term docs and the delivery detail in the task groups and the
release plan.

## Handoff block (cold start)

When ending a session or before a context-heavy spawn, update the `handoff` block in
`state.yaml`.

```yaml
handoff:
  last_step_completed: 'e02s01 verify-work passed'
  open_decisions:
    - 'Use folder mode for e07 (more than 5 stories)'
  required_reading:
    - .agent/tasks/e02-verification/group.yml
  next_skill: develop-tdd
```

## Strategic compaction

| Trigger                       | Action                                                      |
| ----------------------------- | ----------------------------------------------------------- |
| A phase transition            | Compact the handoff, archive a verbose decision to an ADR   |
| Context over 70% estimated    | Run terse-mode for status only, move the detail to `specs/` |
| Before a dispatch-agents wave | `state.yaml` is the only channel between spawns             |

## Workflow

### 1. Initialize (session start)

When `.agent/tasks/state.yml` does not exist, or you are starting a new major phase:

- [ ] Read the release plan and the product scope.
- [ ] Get the git metadata through the git-context tool: the current branch and the
      short hash.
- [ ] Create `.agent/tasks/state.yml` with the active flow, the git block, the handoff,
      and the group cycle when in a build.

### 2. Load (context refresh)

When starting a new session or after a context flush:

- [ ] Read `.agent/tasks/state.yml` to understand where the previous agent left off.
- [ ] Read `.agent/tasks/execution-status.yml` for the story progress.
- [ ] Verify the git branch and hash match `state.yaml`.

### 3. Update (a decision point or milestone)

When a significant decision is made or a milestone is reached:

- [ ] Patch `state.yaml` through the lifecycle tools or a direct edit.
- [ ] Update `handoff.open_decisions` with the rationale.
- [ ] Advance the cycle counter when advancing the group steps.
- [ ] Record an open question under `handoff.open_decisions` or in an ADR.

## Universal checkpoint pattern

Every multi-step flow uses a cycle counter in `state.yaml`.

| Flow                | Cycle key       | Step field      |
| ------------------- | --------------- | --------------- |
| build-epic          | `group_cycle`   | `current_step`  |
| fix-bug             | `bug_cycle`     | `current_step`  |
| orchestrate-project | `project_cycle` | `current_phase` |

Checkpoint: after each step completes, advance the counter and update
`handoff.next_skill`. Resume: on session start, read the current step from the cycle
key and continue from there, not from step 1. Track the completed steps for an audit
trail.

### reset-state (absorbed)

Clear the ephemeral session state. Set the active group, the active story, and the
group-cycle step to null. Use it when ending a phase or starting a new project
context.

### compact-state (absorbed)

Archive a verbose decision before a context transition. Move a system-wide decision
to a global ADR, and a group-scoped decision to a group-local ADR. After archiving,
reset `handoff.open_decisions` to an empty list.

## File format: .agent/tasks/state.yml

```yaml
active_flow: build_group # planning | build_group | fix_bug
active_group_id: e02
active_story_id: e02s01
active_bug_id: null
release:
  target_version: null
  last_tag: v2.28.0
  last_publish: null
group_cycle:
  current_step: develop-tdd
  next_skill: develop-tdd
  completed_steps: [kickoff-branch]
bug_cycle:
  current_step: null
  completed_steps: []
git:
  branch: feat/e02-verify
  hash: abc1234
handoff:
  last_step_completed: null
  open_decisions: []
  next_skill: survey-context
```

## Anti-patterns

- **Duplicate plan**: do not copy the release plan or a task group into
  `state.yaml`.
- **Stale state**: forgetting to update `state.yaml` after a major refactor.
- **Status in the release plan**: story and group status live only in the execution
  status.

## Verify

Confirm `state.yaml` parses and its git block matches the working tree.
