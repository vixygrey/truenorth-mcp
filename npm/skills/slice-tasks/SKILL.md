---
name: slice-tasks
description: "Planning spine step 2 of 3. Slice the work: break a scoped PRD into vertical-slice stories in the task groups. Use it after scope-work, before plan-work. Not a substitute for scope-work or plan-work."
kind: prose
---

# Slice Tasks

> **Spine position:** Step 2: `scope-work`, then `slice-tasks`, then
> `plan-work`.

Create the group manifest and define independently deliverable story boundaries.
This skill owns `.agent/tasks/<capsule>/group.yml`. It does not write detailed
story specifications or runnable task ledgers.

## Artifact contract

- **Writes**: `.agent/tasks/<capsule>/group.yml`.
- **Reads**: `.agent/product/scope.yml`, `.agent/tasks/release-plan.yml`, and
  `.agent/tasks/planning-context.yml` when present.
- **Readers**: `plan-tests`, `plan-work`, execution, verification, traceability,
  and status views.
- **Never writes**: `release-plan.yml`, `test-plan.md`, story specifications,
  task ledgers, or `execution-status.yml`.

The release index must already contain the selected group and capsule path. If it
does not, return to `plan-release`. Do not create or reorder release-index entries.

## Process

1. **Read context**: read the product scope, selected release-index entry, and
   optional planning context. Use its constraints, exclusions, and decisions when
   defining boundaries.
2. **Cut tracer-bullet stories**: each story must be the thinnest vertical path
   that provides demonstrable user value.
3. **Assign BCPs**: estimate each story from 1 to 13. Split a story above 8 BCPs
   unless its cohesion makes the larger boundary necessary.
4. **Record boundaries**: write each story id, title, BCPs, status, requirement
   delta, and reserved spec and task filenames to `group.yml`.
5. **Validate slices**: reject horizontal-only layers and stories that depend on a
   later story before they provide user value.

Example:

```yaml
id: e01
title: Auth System
total_bcps: 8
status: todo
stories:
  - id: e01s01
    title: Login
    bcps: 3
    status: todo
    delta: ADDED
    spec: e01s01-login.md
    tasks: e01s01-tasks.yaml
  - id: e01s02
    title: JWT Token Management
    bcps: 5
    status: todo
    delta: ADDED
    spec: e01s02-jwt.md
    tasks: e01s02-tasks.yaml
```

The `spec` and `tasks` values reserve filenames. They do not authorize this skill
to create those files. `plan-work` owns both.

## Hard gates

- Every story id is unique within the group.
- Every reserved filename stays within the active capsule.
- Every story is independently demonstrable.
- `MODIFIED`, `REMOVED`, and `RENAMED` deltas name the prior behavior that
  `plan-work` must expand.
- The selected group already exists in `.agent/tasks/release-plan.yml`.

## Verify

Confirm `.agent/tasks/<capsule>/group.yml` exists, contains only unique story
boundaries, and references the group id and capsule selected by the release index.
Confirm no release index, story file, task ledger, or execution status changed.

## Handoff

For a P0 or P1 group, hand off to `plan-tests`. Otherwise hand off to
`plan-work`.
