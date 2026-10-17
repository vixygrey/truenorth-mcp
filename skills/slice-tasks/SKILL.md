---
name: slice-tasks
description: "Planning spine step 2 of 3. Split scoped work into independently deliverable work items. Use it after scope-work and before plan-work. Group artifacts are conditional on the active methodology profile."
kind: prose
---

# Slice Tasks

> **Spine position:** `scope-work`, then `slice-tasks`, then `plan-work`.

Read `.agent/profile.yml` first. Every selected scope has a work-item manifest
named `group.yml`; the filename does not require a methodology group.
For grouped work, use the indexed `.agent/tasks/<capsule>/group.yml`.
For ungrouped work, use `.agent/tasks/<task-id>/group.yml` with one work item
and no `group_id` or `group_kind`.

## Artifact contract

- **Manifest write**: `group.yml` in the selected capsule, including boundaries
  and reserved specification and ledger filenames.
- **Task registration**: call `truenorth_record_task` for each work item.
- **Reads**: `.agent/product/scope.yml`, `.agent/tasks/release-plan.json`, and
  `.agent/tasks/planning-context.yml` when present.
- **Never writes**: test plans, detailed work-item specifications, task ledgers,
  or `.agent/tasks/execution-status.yml`.

For grouped work, the release index must already contain the selected `group_id`,
matching `group_kind`, and capsule path.

## Runtime-to-artifact mapping

`truenorth_record_task` accepts `task_name`, `verify_command`, and optional
`group_id` and `group_kind`. Do not send `task_id`, `capsule_dir`, `spec`, or
`tasks`; those belong to the planning artifacts, not the tool arguments.

For `issue-per-task`, use the actual issue number as the manifest's string
`task_id`. Other profiles use a stable declared task id. An ungrouped id must
be one safe path component, not `.` or `..`.
Copy the exact registered `task_name` and `verify_command` into its manifest
entry. Resolve the runtime record by that exact name and selected grouping,
not by parsing a title or treating `group_id` as the work-item id.

For grouped work, `plan-release` owns `groups[].capsule_dir`; resolve the unique
entry matching the selected `group_id` and `group_kind`.
For ungrouped work, the selected `task_id` fixes the capsule at
`.agent/tasks/<task-id>/`; omit grouping in the tool call, manifest, and state.
Reserve `<task-id>-spec.md` and `<task-id>-tasks.yml` in the manifest before
`plan-work` runs. Store filenames relative to that capsule.

`slice-tasks` owns missing boundaries, registrations, and reservations.
A missing group index or capsule path returns to `plan-release`.
Neither `plan-tests` nor `plan-work` may reconstruct either owner's artifact.

## Process

1. Read scope, profile, release-plan records, and optional planning context.
2. Cut the thinnest vertical work items that provide demonstrable user value.
3. Use `story` only when existing project artifacts explicitly select story
   decomposition.
4. Estimate BCP only when project policy or the user explicitly selects BCP.
   Otherwise omit it.
5. Register each work item through `truenorth_record_task` using only supported
   arguments. If one exact matching record already exists, reuse it; duplicate
   or conflicting matches block planning.
6. Write the manifest with exact runtime names and commands, stable task ids,
   and reserved specification and ledger filenames. Do not create those files.
7. Reject horizontal-only layers and a work item that depends on a later item
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
    task_name: Add login behavior
    verify_command: npm test -- login
    delta: ADDED
    spec: login-spec.md
    tasks: login-tasks.yml
  - task_id: token-management
    title: Token management
    status: todo
    task_name: Add token management
    verify_command: npm test -- token-management
    delta: ADDED
    spec: token-management-spec.md
    tasks: token-management-tasks.yml
```

For ungrouped work, the same `work_items` entry is stored in
`.agent/tasks/<task-id>/group.yml`, without the example's group fields.
`active_task` selects that task id; it does not select a title.

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

Confirm the selected manifest exists, each entry resolves to one exact runtime
task record, and all ids, commands, and reserved filenames agree.
For grouped work, confirm the index supplies the matching group kind and capsule.
For ungrouped work, confirm the capsule follows the task-id path and no grouping
was introduced. Confirm no detailed specification, task ledger, or execution
status changed.

## Handoff

For P0 or P1 risk, hand off to `plan-tests`. Otherwise hand off to `plan-work`.
