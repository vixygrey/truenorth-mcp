---
name: plan-tests
description: "Design a risk-scaled test architecture for a declared work-item scope without writing implementation tests. Use after slice-tasks when planned behavior needs explicit scenario coverage."
kind: prose
---

# Plan Tests

Design the test architecture for an existing work-item scope, grouped or
ungrouped. This skill plans scenarios and ownership. It does not write test code.

## Artifact contract

- **Writes**: `.agent/tasks/<capsule>/test-plan.md` only.
- **Reads**: `.agent/profile.yml`, the release index, active task manifest,
  architecture, product scope, conventions, and ontology when present.
- **Readers**: `plan-work`, `verify-work`, and lifecycle gate orchestration.
- **Never writes**: the release index, task manifest, work-item specifications,
  task ledgers, test code, production code, or execution status.

The active task manifest must exist and contain the work-item boundaries. If it is
missing or the requested task is unknown, return to `slice-tasks`. Do not
synthesize tasks or profile-specific ids.

Resolve the manifest by the `slice-tasks` contract: the indexed group capsule,
or `.agent/tasks/<task-id>/group.yml` with grouping fields omitted. Match each
selected task's exact registered name and command before copying its task id.
A missing group index returns to `plan-release`; missing or ambiguous work-item
mapping or filename reservations return to `slice-tasks`, without creating them.

## Profile rules

Use neutral task ids in shared scenario keys. A project may retain an existing
profile-specific id convention, such as epic work-item ids, but plan-tests must
not require that convention in `issue-per-task`, `kanban`, or `generic` projects.

## Process

1. Read the active task manifest and copy its exact task or work-item ids.
2. Map each observable behavior to P0 through P3 risk.
3. Place each scenario at the lowest effective unit, integration, or end-to-end
   level.
4. Define fixtures, boundaries, failure cases, and ownership.
5. Write `.agent/tasks/<capsule>/test-plan.md` using
   [REFERENCE.md](./REFERENCE.md).
6. Verify that every P0 and P1 behavior has an owner and runnable evidence path.

## Verify

Confirm that every scenario references a declared task id, every P0 and P1
behavior has coverage, and the plan does not create implementation files or
modify upstream planning artifacts.

## Handoff

Write the canonical `.agent/tasks/state.json` handoff with
`next_skill: plan-work`.
