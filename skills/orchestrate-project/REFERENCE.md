# Orchestrate Reference: Phases, Modes, and Workflows

Detailed documentation for the `orchestrate-project` meta-skill.

## The 6-Phase Core Loop

### PHASE 1: DISCOVER

- **Goal**: Understand the problem completely, research existing art, and establish shared boundaries.
- **Deliverables**: `.agent/product/vision.yml`, the project tech-stack note, and transient `.agent/tasks/planning-context.yml`.
- **Sequence**:
  1. `survey-context` — map current lifecycle phase, active work, and repository state.
  2. `research-first` — verify dependencies, investigate prior art, and prevent duplicate work.
  3. `elaborate-spec` — refine user needs into a specification and write transient `.agent/tasks/planning-context.yml`.
  4. `grill-me` — stress-test assumptions and resolve ambiguities before design.
- **Gate**: Confirm ("Is the problem clear?").

### PHASE 2: ELABORATE

- **Goal**: Research solutions and lock architectural design.
- **Deliverables**: Prior art in scope YAML, ADRs in `specs/adr/`.
- **Skills**: `grill-me`, `model-domain`, `define-language`, `deepen-architecture`, `design-interface`.
- **Gate**: Risk-required review has no unresolved blocker + Confirm ("Are decisions locked?").

### PHASE 3: PLAN

- **Goal**: Write a verifiable implementation plan with success criteria.
- **Deliverables**: `.agent/product/scope.yml`, `.agent/tasks/release-plan.yml`, each
  `.agent/tasks/<capsule>/group.yml`, optional
  `.agent/tasks/<capsule>/test-plan.md`, and per-work-item specification and task
  ledger files.
- **Sequence**:
  1. `scope-work` — consume `.agent/tasks/planning-context.yml` and produce the bounded `.agent/product/scope.yml`.
  2. `plan-release` — build the release index and define profile-appropriate group boundaries.
  3. `slice-tasks` — define vertical slices and optional group manifests.
  4. `plan-tests` — design test architecture when risk requires explicit scenario coverage.
  5. `plan-work` — write runnable task specifications and ledgers.
- **Context capsule cleanup**: `.agent/tasks/planning-context.yml` is consumed during scoping; clear it when transitioning out of planning into execution.
- **Gate**: Risk-required review has no unresolved blocker plus slopcheck [SUS]/[SLOP].

### PHASE 4: EXECUTE

- **Goal**: Execute the plan through the profile-aware `execute-group` conductor
  with TDD and vertical slices.
- **Deliverables**: Code and `.agent/tasks/execution-status.yml` updates per task.
- **Skills**: `execute-group` runs `survey-context`, `plan-work`,
  `kickoff-branch`, `develop-tdd`, `verify-work`, `audit-code`,
  `commit-message`, and `release-branch`.
- **Estimation**: `slice-tasks` records BCP only when project policy selects it.
- **next_skill**: Each critical-path skill writes `handoff.next_skill` to
  `.agent/tasks/state.yml`. Agents resume by reading that file.
- **Dashboard**: `npm run dashboard` (TUI) or `npm run dashboard:web` (browser, port 7742)
  shows live task and optional group status.
- **Gate**: Integration tests PASS; all required execute-group gates completed for each work item.

### PHASE 5: VERIFY

- **Goal**: Validate success criteria and ensure production readiness.
- **Deliverables**: Risk-scaled behavior evidence, eval results, and review findings.
- **Skills**: `run-evals`, `verify-work`, `audit-code`, `request-review` when required by risk.
- **Gate**: Every risk tier has an observable behavior smoke; no blocking finding remains unresolved; `verify-work` not on `main`/`master`.

### PHASE 6: RELEASE (Integrate)

- **Goal**: Ship to `main` with full traceability.
- **Deliverables**: Release tag (vX.Y.Z), release notes via the tag-driven release (a `v*` tag triggers the release workflow).
- **Skills**: `commit-message`, `release-branch`.
- **Git arc**:
  1. Plan on `main` (Discover / Plan)
  2. `kickoff-branch` → worktree + feature branch + clean baseline
  3. Build / Verify / Review on feature branch
  4. Integrate: **solo-local** (automated branch-landing path) or **team-pr** (`gh pr create` → squash merge)
  5. Cleanup worktree; **end on `main`** in primary repo root
- **Gate**: Safety ("About to land on main. Confirm?").

---

## Orchestration Modes

### Mode 1: Standard (Enforce All Gates)

**Use Case**: New features, major refactors, architectural changes.
**Behavior**:

- All Confirm gates require explicit user approval.
- All Quality gates are hard stops if threshold is not met.
- No shortcuts or phase skipping.

### Mode 2: Fast-Track (Skip Negotiable Gates)

**Use Case**: Hotfixes, minor improvements, refactors on well-tested code.
**Behavior**:

- Skip Discover if `.agent/product/scope.yml` exists.
- Skip Elaborate if design decisions are already locked.
- Skip Verify if coverage ≥95% + all tests PASS.
- Soft gates auto-approve if baseline conditions are met.

### Mode 3: Ad-Hoc (Legacy, Warnings Only)

**Use Case**: Exploration, prototyping, spikes (NOT for production).
**Behavior**:

- Gates emit warnings but do not block execution.
- User can manually skip any phase.
- No enforced quality thresholds.

---

## Gate & Checkpoint Types

_The gate types and the checkpoint keys are defined in this reference._

- **Confirm**: Requires human "yes/no" decision.
- **Quality**: Automated threshold check (e.g., coverage, audit score).
- **Safety**: Destructive actions require risk acknowledgment.
- **Transition**: Mandatory artifact presence check.
- **slopcheck**: Identification of [SUS] (Suspicious) or [SLOP] (High-risk) packages.

---

## Error Recovery & State

Orchestrate maintains `.agent/tasks/state.yml` to track:

- **Current flow / task group**: `active_flow`, `active_group_id`, `group_cycle`.
- **Handoff**: `last_step_completed`, `open_decisions`, `required_reading`, `next_skill`.
- **Git**: `branch`, `hash` for session continuity.
- **Progress**: Story status lives in `.agent/tasks/execution-status.yml` only.

In the event of a crash or exit, resume the orchestrate skill to pick up exactly where the session left off.
