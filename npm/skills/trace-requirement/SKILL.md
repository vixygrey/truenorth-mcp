---
name: trace-requirement
description: "Link task ids from the release plan and optional group artifacts to implementing code and tests. Use it to verify plan coverage or find planned work with no implementation."
kind: prose
---

# Trace Requirement

Build a traceability matrix from `.agent/tasks/release-plan.yml` and any profile-
selected group artifacts. Surface planned tasks with no code and tagged code with
no matching task.

## Pre-flight

> **HARD GATE**: `.agent/tasks/release-plan.yml` must exist. Run
> `plan-release` first when it does not.

Read `.agent/profile.yml` and the release plan before proceeding. Read group
directories only when the profile and plan use groups.

## Process

### 1. Extract task ids

Collect stable task ids from `tasks[]`. When an existing project uses a separate
work-item or story id convention, collect those ids as additional trace keys. Do
not synthesize story-shaped ids.

### 2. Search implementation tags

Search source and tests for the project's trace tag, using `task:` by default:

```text
// task: 452
# task: 452
```

Preserve an existing `story:` tag convention, but treat it as project-selected
vocabulary rather than the universal schema.

### 3. Build the matrix

For each task id:

- **Implemented**: implementation files carry the trace tag.
- **Tested**: test files carry the trace tag.
- **Dark**: no implementation tag exists.

For each tagged file with no matching id in `.agent/tasks/release-plan.yml`:

- **Orphan**: the implementation has no current plan entry.

### 4. Write the report

```markdown
## Task coverage

| Task | Title     | Files | Tests | Status  |
| ---- | --------- | ----- | ----- | ------- |
| 452  | Normalize | 2     | 1     | Covered |
| 453  | Bug links | 0     | 0     | Dark    |

## Orphan code

- path: contains an unmatched task tag

## Coverage summary

Tasks: X covered / Y dark / Z total
```

Suggest `plan-work` for each dark task.

## Verify

The report must count every release-plan task exactly once and use the active
project's selected trace vocabulary.
