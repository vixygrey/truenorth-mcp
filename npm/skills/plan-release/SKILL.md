---
name: plan-release
description: "A release-index builder. Sequence elaborated task groups into .agent/tasks/release-plan.yml with WSJF ordering and BCP baselines. Not a planning-spine substitute: it does not scope work or write story tasks. Use it after elaborate-spec when the user wants a versioned release index of task groups."
kind: prose
---

# Plan Release

> **HARD GATE**: Run this skill only after `elaborate-spec` has produced a clear
> specification and `.agent/product/scope.yml` exists. Run `elaborate-spec` or
> `scope-work` first when either input is missing.

Create or update the release index at `.agent/tasks/release-plan.yml`. This skill
owns release metadata, group identity, capsule paths, prioritization, and group
ordering. It does not create capsule contents.

## Artifact contract

- **Writes**: `.agent/tasks/release-plan.yml`.
- **Reads**: the elaborated specification, product scope, and relevant risk reports.
- **Readers**: `slice-tasks`, planning analysis, execution, traceability, and status
  views.
- **Never writes**: `group.yml`, `test-plan.md`, story specifications, task ledgers,
  or `execution-status.yml`.

If a required group manifest, story specification, or task ledger is missing, hand
off to its owner. Do not regenerate it.

## Process

### 1. Define release groups

Identify the task groups needed to deliver the scoped release. Give each group a
stable id, title, capsule path, BCP baseline, and WSJF inputs. Do not define story
boundaries or implementation tasks here.

### 2. Order groups

Calculate WSJF as:

```text
(Business Value + Time Criticality + Risk Reduction) / Job Size
```

Sort groups from highest to lowest score. If a security report identifies HIGH or
CRITICAL risk for a group, add 2 to the numerator and record the reason in that
group's release-index note.

### 3. Save the release index

The version is a non-authoritative label. The published tag remains authoritative.

```yaml
release:
  version: "2.29.0"
  codename: "Feature Name"
  status: planning
  bump_hint: minor
groups:
  - id: e01
    title: Auth System
    wsjf: 4.5
    bcps: 8
    capsule_dir: .agent/tasks/e01-auth-system
  - id: e02
    title: User Profile
    wsjf: 3.8
    bcps: 5
    capsule_dir: .agent/tasks/e02-user-profile
```

Each `capsule_dir` uses `.agent/tasks/<capsule>/`. `slice-tasks` owns the
`group.yml` within that directory.

### 4. Validate ownership and ordering

- Every group has one stable id and one unique capsule path.
- Group order matches descending WSJF.
- Every referenced capsule path is under `.agent/tasks/`.
- The change set contains no capsule artifact or execution-status mutation.

## Verify

Confirm `.agent/tasks/release-plan.yml` exists, parses as YAML through the project's
configured YAML tooling, and contains a unique ordered group list. Confirm no file
owned by another planning skill changed.

## Handoff

Gate: INDEXED. Next: `slice-tasks` for each indexed group.
