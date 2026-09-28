---
name: plan-release
description: "Build the release index in .agent/tasks/release-plan.yml without assuming a grouping or prioritization method. Use it after elaborate-spec when the user wants release metadata, optional profile-derived groups, and an ordered task ledger."
kind: prose
---

# Plan Release

> **HARD GATE**: Run this skill only after `elaborate-spec` has produced a clear
> specification and `.agent/product/scope.yml` exists.

Create or update `.agent/tasks/release-plan.yml`. Preserve the runtime-owned
`tasks[]` ledger written by `truenorth_record_task`.

## Methodology gate

Read `.agent/profile.yml` before creating groups:

| Profile           | Release index behavior                                      |
| ----------------- | ----------------------------------------------------------- |
| `epic-based`      | require `group_id` and `group_kind: epic`                   |
| `milestone-based` | require `group_id` and `group_kind: milestone`              |
| `issue-per-task`  | use optional ticket groups only when the project needs them |
| `kanban`          | omit `groups[]`; order `tasks[]` directly                   |
| `generic`         | omit `groups[]`; order `tasks[]` directly                   |

Use the active profile's grouping word in user-facing prose. Shared schema and
instructions use `group`.

## Artifact contract

- **Writes**: release metadata and optional `groups[]` in
  `.agent/tasks/release-plan.yml`.
- **Preserves**: every existing `tasks[]` entry and unknown field.
- **Readers**: planning analysis, execution, traceability, and status views.
- **Never writes**: group manifests, test plans, work-item specifications, task
  ledgers, or `.agent/tasks/execution-status.yml`.

## Process

### 1. Read policy

Read the active profile and project conventions. WSJF and BCP are not profile
properties. Use either only when project policy, an existing artifact, or the user
explicitly selects it. Otherwise preserve declared order and omit those fields.

### 2. Define optional groups

For a grouped profile, give each group a stable `group_id`, the profile-derived
`group_kind`, a title, and a capsule path. Do not define implementation tasks here.
For an ungrouped profile, omit `groups[]`.

### 3. Order work

Preserve user or dependency order by default. When the project explicitly selects
WSJF, calculate it from the documented project inputs and record the rationale.
When it explicitly selects BCP, retain the estimate without making it a universal
gate.

### 4. Save the release index

```yaml
release:
  version: "2.29.0"
  status: planning
groups:
  - group_id: m01
    group_kind: milestone
    title: Authentication
    capsule_dir: .agent/tasks/m01-authentication
tasks:
  - group_id: m01
    group_kind: milestone
    task_name: Add login behavior
    verify_command: npm test -- login
```

For `kanban`, `generic`, or an ungrouped `issue-per-task` project, omit
`groups[]`, `group_id`, and `group_kind`:

```yaml
release:
  version: "2.29.0"
  status: planning
tasks:
  - task_name: Add login behavior
    verify_command: npm test -- login
```

Each `capsule_dir` stays under `.agent/tasks/`. `slice-tasks` owns any
`group.yml` within it.

## Verify

Confirm the file parses, existing `tasks[]` records are unchanged, group kinds
match `.agent/profile.yml`, ids are unique, and every capsule path stays under
`.agent/tasks/`. When WSJF or BCP appears, name the project policy that selected
it.

## Handoff

For a grouped profile, gate INDEXED and hand off to `slice-tasks` for each group.
For an ungrouped profile, hand off directly to `plan-work`.
