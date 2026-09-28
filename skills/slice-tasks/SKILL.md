---
name: slice-tasks
description: "Planning spine step 2 of 3. Split scoped work into independently deliverable work items. Use it after scope-work and before plan-work. Group artifacts are conditional on the active methodology profile."
kind: prose
---

# Slice Tasks

> **Spine position:** `scope-work`, then `slice-tasks`, then `plan-work`.

Read `.agent/profile.yml` first. Under a grouped profile, create the selected
group's manifest at `.agent/tasks/<capsule>/group.yml`. Under `kanban`, `generic`,
or an ungrouped `issue-per-task` workflow, update task boundaries directly in
`.agent/tasks/release-plan.yml` through `truenorth_record_task`.

## Artifact contract

- **Grouped write**: `.agent/tasks/<capsule>/group.yml`.
- **Ungrouped write**: task records through `truenorth_record_task`.
- **Reads**: `.agent/product/scope.yml`, `.agent/tasks/release-plan.yml`, and
  `.agent/tasks/planning-context.yml` when present.
- **Never writes**: test plans, detailed work-item specifications, task ledgers,
  or `.agent/tasks/execution-status.yml`.

For grouped work, the release index must already contain the selected `group_id`,
matching `group_kind`, and capsule path.

## Process

1. Read scope, profile, release-plan records, and optional planning context.
2. Cut the thinnest vertical work items that provide demonstrable user value.
3. Use `story` only when existing project artifacts explicitly select story
   decomposition.
4. Estimate BCP only when project policy or the user explicitly selects BCP.
   Otherwise omit it.
5. Reserve detailed specification and task filenames without creating them.
6. Reject horizontal-only layers and a work item that depends on a later item
   before it provides value.

Grouped example:

```yaml
group_id: m01
group_kind: milestone
title: Authentication
status: todo
work_items:
  - task_id: login
    title: Login
    status: todo
    delta: ADDED
    spec: login.md
    tasks: login-tasks.yaml
  - task_id: token-management
    title: Token management
    status: todo
    delta: ADDED
    spec: token-management.md
    tasks: token-management-tasks.yaml
```

An `epic-based` project can call `work_items` stories in prose and preserve an
existing `stories` key. Shared tooling must consume both as selected project
conventions, not assume stories universally.

## Hard gates

- Every task id is unique in its scope.
- Every reserved filename stays within the selected capsule.
- Every work item is independently demonstrable.
- `MODIFIED`, `REMOVED`, and `RENAMED` deltas name prior behavior.
- A grouped manifest's `group_kind` matches `.agent/profile.yml`.
- WSJF and BCP fields name the policy that selected them.

## Verify

For grouped work, confirm `group.yml` exists, uses the profile-derived grouping,
contains unique work-item ids, and references the release-plan group. For
ungrouped work, confirm each task was recorded through `truenorth_record_task`.
Confirm no detailed specification, task ledger, or execution status changed.

## Handoff

For P0 or P1 risk, hand off to `plan-tests`. Otherwise hand off to `plan-work`.
