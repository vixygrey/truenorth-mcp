---
name: orchestrate-project
description: 'A meta-skill that enforces the six-phase core loop (discover, elaborate, plan, build, verify, release) with hard gates. Use it to coordinate a multi-phase project with quality checkpoints across the lifecycle.'
---

# Orchestrate

> **HARD GATE**: Do NOT invoke orchestrate-project without a clear multi-phase workflow. A single-skill task uses a dedicated skill instead. Orchestrate is for complex, multi-stage work that needs coordination across phases.

The orchestrate skill coordinates projects through a prescriptive 6-phase core loop with hard gates, ensuring consistent quality and preventing scope creep.

## Quick start

- Start a new project in standard mode. This initializes the `specs/` cockpit and
  begins the discover phase.
- Resume an existing project at the current phase.
- For a low-risk scenario (a hotfix, a refactor on well-tested code), use
  fast-track mode.

## The 6-Phase Core Loop

1. **DISCOVER** (3-6 hours): Understand problem. Deliverables: `.agent/product/vision.yml`, `.agent/product/scope.yml`, the project tech-stack note.
2. **ELABORATE** (3-6 hours): Research solutions. Deliverables: Prior art in scope YAML, ADRs in `specs/adr/`.
3. **PLAN** (2-4 hours): Write verifiable plan. Deliverables: `release-plan.yaml`, `epics/eNN-*.yaml` with `verify:` per task.
4. **BUILD** (1-8 hours): Execute plan. Runs build-epic once per story in WSJF order. Deliverables: Code; update `execution-status.yaml`.
5. **VERIFY** (1-3 hours): Validate success criteria. Deliverables: UAT evidence, the review report if used.
6. **RELEASE** (30 min - 2 hours): Ship to production. Deliverables: Release tag (vX.Y.Z), `state.yaml` `release.last_tag`.

### Checkpoint / resume

Track progress via `.agent/tasks/state.yml` `project_cycle`:

- `project_cycle.current_phase`: current phase (1–6)
- `project_cycle.completed_phases`: completed phase numbers
- `handoff.next_skill`: skill for the current phase
- On resume, read `project_cycle.current_phase` and continue from there

See [REFERENCE.md](REFERENCE.md) for detailed phase specifications and gate types.

## How Orchestrate Works

1. **Maintains the state**: tracks the current phase, `active_group`, `active_flow`,
   decisions, and risks in `.agent/tasks/state.yml`.
2. **Routes to the phase skill**: selects the skill for the current phase. A
   decision passes only through the `handoff` block in `.agent/tasks/state.yml` between
   steps.
3. **Applies methodology lenses**: when a test plan or an ADR exists, apply it at
   the phase gates.
4. **Enforces the gates**: hard stops when a success criterion is not met.
5. **The gatekeeper**: between stories in the build phase, read the execution
   status. The previous story must be `done` before the next starts. Use
   `build-epic` for the group cycle.
6. **Pauses for confirmation**: after each phase, ask "ready to proceed?".
7. **Snapshots**: take a cockpit snapshot before a major release cut.

## Orchestration Modes

- **Standard**: Enforce all gates. Use for new features and major refactors.
- **Fast-Track**: Skip negotiable gates. Use for hotfixes and minor improvements.
- **Ad-Hoc**: Warnings only. Use for prototyping and spikes (non-production).

See [REFERENCE.md](REFERENCE.md) for full mode behaviors.

## Verification

Confirm every phase completed with its artifacts: the state, the release plan, the
product scope, and the task groups all exist.

<!-- story: e05s03 -->
