---
name: plan-tests
description: 'Design a risk-scaled test architecture for an epic before implementation begins. Produces prioritized scenarios, a test-level distribution, and fixture plans.'
---

# Plan Tests

> **Spine position**: between `slice-tasks` and `plan-work` for an epic with `risk: P0` or `P1`. Optional for P2 or P3, and can be waived in `state.yaml`.

Bridge the gap between slicing and planning by designing the test suite as a
first-class system. Produces the epic test plan.

## Pre-flight

- Read the story list for the active epic.

## Core workflow

1. **Analyze the epic**: read the sliced stories in the active epic capsule.
2. **Risk assessment**: map each behavior to a P0 to P3 risk tier.
3. **Level strategy**: classify each scenario as unit, integration, or E2E.
4. **Fixture design**: plan the factories, network intercepts, and mocks. See
   REFERENCE.md.
5. **NFR plan**: define verifiable commands for the non-functional requirements.
   Skip when `--lite`.
6. **Publish**: generate the epic test plan.

## Hard gates and guardrails

- Do NOT write test code or production code during this skill.
- The scenario id format MUST be `SC-eNNsYY-P{0|1|2|3}-NN`.
- `plan-work` MUST reference these scenario ids in its Gherkin acceptance criteria.
- Default to pushing a test to the lowest possible level.

## Execution modes

- Standard: the full plan.
- `--lite`: skip the NFRs and the complex fixture planning.

## Verify

Confirm the epic test plan exists for the active epic.

## Handoff

Gate: READY. Next: plan-work.
Writes: `state.yaml` `handoff.next_skill = plan-work`.
