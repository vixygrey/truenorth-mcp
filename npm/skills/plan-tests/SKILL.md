---
name: plan-tests
description: "Design a risk-scaled test architecture for a task group before implementation begins. Produces prioritized scenarios, a test-level distribution, and fixture plans."
kind: prose
---

# Plan Tests

> **Spine position**: after `slice-tasks` and before `plan-work` for a task group
> with P0 or P1 risk. Optional for P2 or P3.

Design the risk-scaled test architecture for one group and write it to
`.agent/tasks/<capsule>/test-plan.md`.

## Artifact contract

- **Writes**: `.agent/tasks/<capsule>/test-plan.md`.
- **Reads**: `.agent/tasks/<capsule>/group.yml` and relevant product and
  architecture context.
- **Readers**: `plan-work`, `verify-work`, and lifecycle gate orchestration.
- **Never writes**: the release index, group manifest, story specifications, task
  ledgers, test code, production code, or execution status.

`group.yml` must exist and contain the story boundaries. If it is missing or a
requested story is unknown, return to `slice-tasks`. Do not synthesize stories.

## Core workflow

1. **Read the group**: use the exact story ids and boundaries from `group.yml`.
2. **Assess risk**: map each behavior to P0 through P3.
3. **Choose levels**: place each scenario at the lowest effective unit,
   integration, or end-to-end level.
4. **Design fixtures**: specify factories, network intercepts, and database state.
5. **Plan NFR checks**: define runnable commands for applicable non-functional
   requirements. Skip this step in `--lite` mode.
6. **Publish**: write `.agent/tasks/<capsule>/test-plan.md` using the template in
   [REFERENCE.md](REFERENCE.md).

## Hard gates

- Scenario ids use `SC-eNNsYY-P{0|1|2|3}-NN`.
- Every scenario names a story present in `group.yml`.
- Every P0 and P1 behavior has a verification level and runnable command.
- No scenario changes a story boundary.
- `plan-work` must preserve scenario ids and risk classifications.

## Verify

Confirm `.agent/tasks/<capsule>/test-plan.md` exists, every scenario references a
story in `group.yml`, and no upstream or implementation artifact changed.

## Handoff

Gate: READY. Next: `plan-work`.
