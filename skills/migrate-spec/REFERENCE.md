# Migrate Spec — Reference

Transformation rules for spec-kit and BMAD projects, plus learnings to adopt and
output formats. Runtime state goes under `.agent/`. Human-authored narrative goes
under `specs/`.

See [REFERENCE-GSD.md](./REFERENCE-GSD.md) for the full GSD mapping.

## Navigation

| Section                                             |
| --------------------------------------------------- |
| spec-kit mapping                                    |
| BMAD mapping                                        |
| Learnings to adopt                                  |
| Output formats (ADR, decision log, migration audit) |
| `in_scope` format with ID tracking                  |
| `REQUIREMENTS_TRACE.yaml` format                    |
| `.agent/tasks/state.yml` template format            |
| Step 5 — Surface learnings (optional)               |
| Step 6 — Adversarial review (optional)              |
| Step 7 — Two-pass spec writing gate (optional)      |
| Step 8 — Methodology doc template (optional)        |
| Artifact mapping summary                            |
| Rules                                               |

---

## spec-kit → TrueNorth Mapping

### Artifact Locations

```
project-root/
├── spec.md         ← user journeys, success criteria, scope
├── plan.md         ← technology, architecture, constraints
├── tasks.md        ← atomic task list
└── .specify/
    ├── workflow-catalogs.yml
    └── workflows/runs/<id>/
        ├── state.json
        └── log.jsonl
```

### `spec.md` → `.agent/product/scope.yml` + `.agent/spec/architecture.md`

spec-kit `spec.md` covers who uses the product, the user journeys, the success
criteria, and what is in and out of scope.

Transform:

- User journeys become `scope.yml` success criteria and `in_scope` entries.
- In and out of scope become the `in_scope` and `out_of_scope` sections.
- Domain terms and the glossary become `.agent/product/glossary.yml`.
- The problem statement and vision become `.agent/product/vision.yml`.

### `plan.md` → `.agent/spec/architecture.md` + `.agent/tasks/release-plan.yml` + `.agent/tasks/<group>/`

spec-kit `plan.md` covers the technology stack, the architectural patterns, and the
implementation constraints.

Transform:

- Technology decisions become the Technology section of `.agent/spec/architecture.md`.
- Architecture patterns become the Architecture section.
- A hard decision with a trade-off becomes `specs/adr/NNNN-{slug}.md`.
- A phased approach or milestones become task-group entries in `release-plan.yml`.
- Implementation steps become the task list under `.agent/tasks/<group>/` with
  `verify:`.

### `tasks.md` → `.agent/tasks/<group>/` (via slice-tasks)

spec-kit tasks are atomic and verifiable in isolation, the same principle as the
TrueNorth `verify:` mandate.

Transform:

- Copy the tasks into the task group's `tasks[]`. Preserve the task numbers.
- Add a `verify:` line when a spec-kit task has an acceptance criterion.
- Group the tasks under the task groups that match the `release-plan.yml` entries.

### `.specify/` state

Discard. This is workflow-engine state, not meaningful in the TrueNorth skill model.

---

## BMAD → TrueNorth Mapping

### Artifact Locations

```
project-root/
├── _bmad/bmm/config.yaml
├── _bmad-output/
│   ├── product-brief.md
│   ├── prfaq-{project}.md
│   ├── prd.md
│   ├── addendum.md
│   ├── decision-log.md
│   ├── ux-spec.md
│   └── architecture.md
├── project-context.md
└── docs/
    ├── epic-{slug}.md
    └── story-{slug}.md
```

### `product-brief.md` / `prfaq-{project}.md` → `.agent/product/vision.yml`

Transform:

- The vision and core value become the `north_star` and `success_criteria` of
  `vision.yml`.
- Target users become notes in `vision.yml` or `scope.yml`.
- A prfaq customer FAQ can inform the success criteria in `scope.yml`.

### `prd.md` → `.agent/product/scope.yml` + `.agent/product/glossary.yml`

BMAD `prd.md` has a glossary, FR-XX functional requirements, UJ-XX user journeys,
NFRs, and assumptions.

Transform:

- The glossary becomes `.agent/product/glossary.yml`.
- FR-XX items become `in_scope` entries with the IDs preserved.
- UJ-XX user journeys become success criteria.
- NFRs become the `constraints` section.
- Inline `[ASSUMPTION: ...]` tags are collected in the scope YAML.
- Out-of-scope features become `out_of_scope`.

### `addendum.md` + `decision-log.md` → `specs/adr/` + `.agent/spec/decision-log.md`

Transform:

- A hard, irreversible, surprising decision becomes an individual
  `specs/adr/NNNN-{slug}.md`.
- A lightweight decision goes to `.agent/spec/decision-log.md` (date, decision,
  rationale).
- An `addendum.md` change signal becomes a note in the `scope.yml` metadata.

### `architecture.md` → `.agent/spec/architecture.md` + `specs/adr/`

Transform:

- ADR sections become individual `specs/adr/NNNN-{slug}.md` files.
- The system overview and data models become the Architecture section of
  `.agent/spec/architecture.md`.
- API contracts stay at `docs/api.md` or similar and are linked from
  `architecture.md`.

### `epic-*.md` → `.agent/tasks/release-plan.yml` + `.agent/tasks/<group>/`

Each epic becomes one release-plan entry with `group_kind: epic` plus one task
group. Acceptance criteria become story tasks with `verify:`.

### `story-*.md` → `.agent/tasks/<group>/` stories

Each story becomes one story entry in the task group. Acceptance criteria become
`verify:` lines.

### `project-context.md` → the project agent guide

Add a "## Project Context" section to the project agent guide. Copy the tech stack,
coding rules, and preferences verbatim.

---

## Learnings to Adopt

Optional enhancements to offer the user after migration. Present them as checkboxes.

### From GSD

- [x] **`.agent/spec/methodology.md`** — standing analytical lenses. Agents read it
      before planning. (adopted: optional Step 8 template scaffold)
- [x] **`handoff` block in `state.yml`** — last skill, last step, and required
      reading for the next session. (adopted: mandatory in Step 4 output)
- [x] **ID tracking in `scope.yml`** — FR and UJ IDs for spec-to-plan-to-verification
      traceability. (adopted in the Step 3 transform)

### From spec-kit

- [x] **Two-pass spec writing** — a user-journey pass first, then a
      technical-decisions pass. (adopted: optional post-migration gate)
- [ ] **Explicit inter-phase gate** — an "Approve to proceed?" at the end of
      `elaborate-spec`.
- [ ] **Task isolation** — each task completable in isolation, with `depends-on`
      explicit in the task group.

### From BMAD

- [x] **FR-XX and UJ-XX in `scope.yml`** — rigorous traceability. (adopted:
      `REQUIREMENTS_TRACE.yaml` emitted on migration)
- [ ] **`.agent/spec/decision-log.md`** — lightweight decisions below the ADR
      threshold.
- [x] **Adversarial review pass** — critique the task group before `develop-tdd`.
      (adopted: optional Step 6 in migration)

---

## Output Formats

### ADR format

Use `model-domain/ADR-FORMAT.md`. Create an ADR only when all three apply: it is
hard to reverse, surprising without context, and the result of a real trade-off.

```markdown
# ADR-NNNN: {Title}

**Status:** Accepted
**Date:** YYYY-MM-DD

## Context

[What situation forced this decision?]

## Decision

[What was decided?]

## Consequences

[What becomes easier or harder?]
```

### Decision-log format

For a lightweight decision that does not warrant a full ADR, append to
`.agent/spec/decision-log.md`:

```markdown
# Decision Log

| Date       | Decision     | Rationale              | Alternatives                              |
| ---------- | ------------ | ---------------------- | ----------------------------------------- |
| 2026-05-19 | Use Postgres | Existing ops expertise | SQLite (limited), DynamoDB (no local dev) |
```

### Migration-audit format

The post-migration adversarial review report. Written to
`.agent/spec/migration-audit.md` when Step 6 runs:

```markdown
# Migration Audit — <project-name>

**Source Framework:** <GSD|spec-kit|BMAD>
**Date:** <ISO 8601>
**Status:** <Pass|Findings|Critical>

## Summary

- TODO markers: N
- FIXME markers: N
- MISSING markers: N
- Task groups without verify: N

## High Priority Findings

- **Artifact:** .agent/tasks/e02-auth-ui/story.yml
  **Issue:** Story e02s01 has no verify: commands in tasks
  **Recommendation:** Add a runnable verify command before develop-tdd

- **Artifact:** .agent/tasks/state.yml
  **Issue:** open_decisions list empty without a comment
  **Recommendation:** Add a # comment when all decisions were resolved during migration

## Information

- Artifact .agent/tasks/e01-auth/story.yml contains TODO: "Define Neon Auth client URL injection" (normal for a fresh migration)

## Next Steps

1. Address the high-priority findings before plan-work.
2. Run the project verification through the `truenorth_verify_gate` tool to enforce the code-quality gates.
3. Begin develop-tdd on the highest-WSJF task group.
```

### in_scope format with ID tracking

Source IDs (REQ-XX, FR-XX, UJ-XX) are emitted as first-class YAML fields:

```yaml
in_scope:
  - id: REQ-001
    description: 'User can register with email and password'
    source: 'REQUIREMENTS.md'
  - id: FR-015
    description: 'Auth service must support OAuth2 token flow'
    source: 'prd.md'
  - id: REQ-AUTO-002 # auto-generated when source had no ID
    description: 'Dashboard displays user profile'
    # auto-generated: true  (optional comment for tracking)
```

**When the source has no IDs:** if the user opts in, auto-generated IDs follow the
`REQ-{NNN}` format with an optional `# auto-generated` comment.

**When the source has mixed IDs:** an entry with a source ID gets an `id:` field.
An entry without an ID receives an auto-generated ID. A comment block at the top of
`in_scope` documents which IDs were auto-generated.

### REQUIREMENTS_TRACE.yaml format

Emitted at `.agent/product/REQUIREMENTS_TRACE.yaml` when the source has FR-XX
(functional requirement) or UJ-XX (user journey) IDs. It maps source requirements
onto the task-group structure and the verification commands:

```yaml
trace:
  # Functional Requirements
  - id: FR-001
    type: functional_requirement
    description: 'User can register with email/password'
    source_artifact: 'prd.md'
    group_id: 'e02-auth-ui'
    group_kind: epic
    story: 'e02s01'
    verify: "grep -q 'FR-001' .agent/product/scope.yml && echo OK"

  # User Journeys
  - id: UJ-001
    type: user_journey
    description: 'New user completes registration flow'
    source_artifact: 'epic-auth-ui.md'
    group_id: 'e02-auth-ui'
    group_kind: epic
    story: 'e02s01'
    verify: "grep -q 'UJ-001' .agent/tasks/e02-auth-ui/story.yml && echo OK"

metadata:
  source_framework: 'BMAD'
  migrated_at: '2026-06-26T12:00:00Z'
  total_requirements: 2
  coverage: 'All FR-XX and UJ-XX IDs from source mapped'
```

**When the source has no FR-XX/UJ-XX:** skip `REQUIREMENTS_TRACE.yaml`. Add a note
to the `state.yml` handoff: "No FR-XX/UJ-XX IDs found, traceability file skipped".

**Existing trace file:** when `REQUIREMENTS_TRACE.yaml` exists, prompt the user:
"Overwrite? [yes / merge / skip]". Merge appends new entries. Skip leaves the
existing file intact.

### `.agent/tasks/state.yml` template format

Generated during Step 4 of migration. Regenerate it from scratch in the TrueNorth
YAML format. The **handoff block is mandatory**:

```yaml
active_flow: null
active_group_id: null
active_story_id: null
completed_group: false

group_cycle:
  current_step: null
  next_skill: null
  story_bcps: null
  completed_steps: []
  audit_result: null

bug_cycle:
  current_step: null
  completed_steps: []

release:
  target_version: null
  last_tag: null
  last_publish: null
  ci_verified: false

metrics:
  story_start: null
  story_end: null
  cycle_minutes: null
  bcp_per_hour: null

git:
  branch: <current branch>
  hash: <git rev-parse HEAD>
  pushed: false

handoff:
  last_step_completed: 'Migrated from <framework> on <date>'
  open_decisions: [] # Empty when all decisions were resolved during migration
  required_reading:
    - .agent/product/vision.yml
    - .agent/product/scope.yml
    - .agent/spec/architecture.md
    - .agent/tasks/release-plan.yml
  next_skill: survey-context

two_pass_spec: # Optional: only when the user activates the two-pass spec-writing gate
  journey_pass: pending
  technical_pass: pending
  approved_at: null
```

---

## Reference block: detection output

```text
Detected: GSD
Found:
  ✓ .planning/ROADMAP.md
  ✓ .planning/REQUIREMENTS.md  (12 REQ-XX items)
  ✓ .planning/state.yaml
  ✓ .planning/phases/01-auth/01-CONTEXT.md
  ✗ .planning/METHODOLOGY.md  (not present)

Skipping:
  .planning/phases/01-auth/01-01-SUMMARY.md  (execution record; skipped)

Proceed with migration? [yes / skip <artifact> / abort]
```

---

## Reference block: ID field form

```yaml
# CORRECT — first-class id: field
in_scope:
  - id: REQ-001
    description: "User can register with email/password"
    source: "REQUIREMENTS.md"

# DEPRECATED — comment-only
in_scope:
  - "User can register with email/password"  # REQ-001
```

---

### Step 5 — Surface learnings (optional)

After migration, offer the user a brief analysis of what the source framework did
that TrueNorth does not have yet.

Use the learnings table above. Present it as checkboxes so the user can decide which
to adopt.

→ verify: `grep -c "\- \[ \]" .agent/tasks/state.yml 2>/dev/null && echo "pending items recorded" || echo "no pending items in state.yml"`

---

### Step 6 — Adversarial review (optional)

Before the user runs `plan-work`, offer an optional lightweight audit of the migrated
artifacts. This catches common migration errors early: incomplete specs, missing
verification commands, and unresolved decisions.

Prompt: "Run adversarial review of the migrated artifacts? [yes / skip]"

If yes, perform these checks:

1. Scan for incomplete markers. Find TODO, FIXME, and MISSING under `.agent/`.
2. Verify every task group has `verify:` commands. Parse the task-group files.
3. Check the `state.yml` handoff. Make sure that `open_decisions` is documented,
   even when empty.

Collect the findings and write them to `.agent/spec/migration-audit.md`:

```markdown
# Migration Audit — <project-name> from <framework>

**Date:** <ISO 8601 timestamp>
**Status:** Pass / Fail with findings

## Findings

### High Priority

- Artifact: .agent/tasks/e02-auth-ui/story.yml
  Finding: No verify: commands in the story tasks
  Recommendation: Add a `verify:` to each task before develop-tdd

### Information

- Count of TODO markers: 3 (normal for a fresh migration)
```

When findings exist, the handoff block notes: "Adversarial review: N findings, see
`.agent/spec/migration-audit.md`".

When skip is chosen, add to the handoff: "Adversarial review: skipped, review
manually before plan-work".

→ verify: `test -f .agent/spec/migration-audit.md && echo "audit completed" || echo "audit skipped or not performed"`

---

### Step 7 — Two-pass spec writing gate (optional)

After Steps 1 through 6, offer the user an optional two-pass spec-writing workflow
(a spec-kit learning):

Prompt: "Use two-pass spec writing (user journeys first, then technical)? [yes / no]"

If **yes**, initialize the gate in `.agent/tasks/state.yml`:

```yaml
two_pass_spec:
  journey_pass: pending
  technical_pass: pending
  approved_at: null
```

The journey pass must be marked complete by the user, after stakeholder approval of
the user-journey specs, before the technical pass begins:

```yaml
two_pass_spec:
  journey_pass: complete
  approved_at: '2026-06-26T12:00:00Z'
  technical_pass: pending
```

Inform the user: "The journey pass is pending. Run `elaborate-spec` for the user
journeys, get stakeholder approval, then update `two_pass_spec.journey_pass` to
`complete` in `state.yml` before you proceed to the technical specs."

If **no**, skip the two-pass gate. Proceed directly to plan-work.

→ verify: `grep -q 'two_pass_spec:' .agent/tasks/state.yml && echo "two-pass gate initialized" || echo "two-pass gate not activated"`

---

### Step 8 — Methodology doc template (optional)

After Steps 1 through 7, offer the user an optional analytical-framework scaffold (a
GSD learning):

Prompt: "Create a methodology doc? [yes / no]"

If **yes**, present a checklist of analytical lenses:

```
Which lenses to include in .agent/spec/methodology.md?

[x] Cost of Delay (CD3)           — Priority and trade-off assessment
[ ] STRIDE                        — Security threat modeling
[ ] F.I.R.S.T                     — Test-quality principles
[ ] Bayesian Updating             — Probabilistic decision-making
[ ] OWASP Top 10                  — Web security framework
```

Copy the template from `migrate-spec/templates/methodology.md` to
`.agent/spec/methodology.md`.

- Active lenses stay uncommented.
- Unselected lenses stay commented out.
- Populate `{{project_name}}` with the migrated project's name.

If **no**, skip. Add a note to the handoff: "Methodology doc: skipped, can be added
later by copying `migrate-spec/templates/methodology.md` to `.agent/spec/`".

→ verify: `test -f .agent/spec/methodology.md && echo "methodology doc created" || echo "methodology doc skipped"`

---

## Artifact Mapping Summary

Full mapping tables: [REFERENCE-GSD.md](./REFERENCE-GSD.md) (GSD), this file
(spec-kit, BMAD, learnings).

| Source                    | Target                                                                        |
| ------------------------- | ----------------------------------------------------------------------------- |
| GSD `ROADMAP.md`          | `.agent/tasks/release-plan.yml` + task groups                                 |
| GSD `REQUIREMENTS.md`     | `.agent/product/scope.yml`                                                    |
| GSD `CONTEXT.md` (phases) | `.agent/spec/architecture.md` + `specs/adr/`                                  |
| GSD `PLAN.md`             | `.agent/tasks/<group>/` (tasks with verify)                                   |
| GSD `METHODOLOGY.md`      | `.agent/spec/methodology.md`                                                  |
| spec-kit `spec.md`        | `.agent/product/scope.yml` + `.agent/spec/architecture.md`                    |
| spec-kit `plan.md`        | `.agent/spec/architecture.md` + `.agent/tasks/release-plan.yml` + task groups |
| spec-kit `tasks.md`       | `.agent/tasks/<group>/` (see slice-tasks)                                     |
| BMAD `prd.md`             | `.agent/product/scope.yml`                                                    |
| BMAD `architecture.md`    | `.agent/spec/architecture.md` + `specs/adr/`                                  |
| BMAD `epic-*.md`          | `.agent/tasks/release-plan.yml` + task groups                                 |
| BMAD `story-*.md`         | `.agent/tasks/<group>/` (see slice-tasks)                                     |
| BMAD `project-context.md` | the project agent guide (append a project-specific section)                   |
| BMAD `decision-log.md`    | `specs/adr/` (one ADR per logged decision) or `.agent/spec/decision-log.md`   |

---

## Rules

- **Preserve source IDs.** REQ-XX, FR-XX, and UJ-XX are emitted as first-class `id:`
  fields in the TrueNorth YAML targets (for example, `in_scope` entries). Never
  silently renumber. See the Step 3 ID Tracking subsection in the SKILL.md.
- **Never merge contradictory docs.** When the source has both `CONTEXT.md` and
  `architecture.md`, create sections in `.agent/spec/architecture.md`. Do not
  collapse them.
- **ADRs are opt-in.** Create an ADR only when the decision is hard to reverse,
  surprising without context, and the result of a real trade-off. A lightweight
  decision goes to `.agent/spec/decision-log.md`.
- **state.yml is always regenerated.** Never migrate the source STATE verbatim. The
  TrueNorth `state.yml` needs its own format.
- **`.agent/` and `specs/adr/` are the only outputs.** Runtime state and the
  product concept go under `.agent/`. An ADR goes to `specs/adr/`. No file is
  created elsewhere, except the project agent guide.
