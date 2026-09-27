---
name: change-request
description: "Orchestrate a new requirement or release reprioritization through the skills that own the affected planning artifacts. Use it when requirements change mid-release."
kind: prose
---

# Change Request

> **HARD GATE**: `.agent/tasks/release-plan.yml` must exist. Run `plan-release`
> first when it does not.

Capture and route a mid-release change without taking ownership of planning
artifacts. This skill reads the release index and capsule artifacts, presents the
impact, and invokes the current owner for each required mutation.

## Artifact contract

- **Writes**: no planning artifact directly.
- **Reads**: the release index, affected group manifests, story specifications,
  task ledgers, and execution status.
- **Routes**: release ordering to `plan-release`, story boundaries to
  `slice-tasks`, test architecture to `plan-tests`, and story detail to
  `plan-work`.

## Mode A: Add or change a requirement

1. **Capture**: state the requested change and the problem it solves.
2. **Locate**: identify affected groups and stories without editing them.
3. **Classify**: record `ADDED`, `MODIFIED`, `REMOVED`, or `RENAMED`, including
   before and after behavior when applicable.
4. **Route the index**: invoke `plan-release` when the change adds a group or
   changes group priority.
5. **Route boundaries**: invoke `slice-tasks` to add or change story boundaries in
   `group.yml`.
6. **Route detail**: invoke `plan-tests` when risk architecture changes, then
   `plan-work` for each affected story specification and task ledger.

## Mode B: Reorder groups

1. Gather the changed Business Value, Time Criticality, Risk Reduction, and Job
   Size inputs.
2. Present the proposed WSJF delta and identify in-progress work that must not be
   interrupted.
3. Invoke `plan-release` to update group scores and ordering.

See [REFERENCE.md](REFERENCE.md) for the scoring rubric.

## Conversational mode

When the request is incomplete, ask only for the missing change, reason, affected
capability, and priority inputs. Present the resulting route before invoking an
owner.

## Verify

Confirm each changed artifact was written by its catalog owner. This orchestration
must not directly modify the release index, group manifest, test plan, story files,
task ledgers, or execution status.

## Handoff

Resume with `build-epic` only after every required owner has completed its update
and cross-artifact consistency passes.
