# migrate-spec Reference — GSD

Full artifact transformation rules for migrating GSD projects to the TrueNorth
layout. Runtime state goes under `.agent/`. Human-authored narrative goes under
`specs/`.

See [REFERENCE.md](./REFERENCE.md) for spec-kit, BMAD, learnings, and the ADR and
decision-log formats.

---

## Artifact Locations

GSD stores everything under `.planning/` at the project root.

```
.planning/
├── ROADMAP.md
├── STATE.md
├── REQUIREMENTS.md
├── METHODOLOGY.md
├── HANDOFF.json
├── .continue-here.md
└── phases/
    └── XX-name/
        ├── XX-CONTEXT.md
        ├── XX-YY-PLAN.md
        ├── XX-YY-SUMMARY.md
        └── XX-DISCUSSION-LOG.md
    spikes/
        └── SPIKE-NNN/README.md
```

---

## Transformation Rules

### `.planning/ROADMAP.md` → `.agent/tasks/release-plan.yml` + `.agent/tasks/<group>/`

GSD ROADMAP has a milestone name, phases, success criteria per phase, and a plan
count.

Transform:

- Each GSD phase becomes one task-group entry in `release-plan.yml` (`group_id`,
  `group_kind: epic`, `title`, `wsjf`, `file`). A GSD phase is genuinely
  epic-shaped, so it maps to `group_kind: epic` under the neutral grouping model.
- Phase detail becomes the matching task group under `.agent/tasks/<group>/`
  (stories, tasks, `verify:`).
- Completed phases become `done` in `.agent/tasks/execution-status.yml`. An active
  phase becomes `in_progress`.

---

### `.planning/REQUIREMENTS.md` → `.agent/product/scope.yml`

GSD REQUIREMENTS has REQ-XX IDs and the Validated, Active, and Out-of-Scope
categories, plus traceability.

Transform:

- Preserve REQ-XX IDs as first-class `id:` fields in `in_scope` entries (see
  [REFERENCE.md — ID tracking format](./REFERENCE.md#in_scope-format-with-id-tracking)).
- Validated requirements become `in_scope` entries with `id:`, `description:`, and
  `source:` fields.
- Out-of-Scope becomes `out_of_scope` entries (preserve IDs when present).
- Active (in-progress) becomes `in_scope` with a status note.

---

### `.planning/phases/XX-name/XX-CONTEXT.md` → `.agent/spec/architecture.md` + `specs/adr/`

GSD CONTEXT.md has six sections: domain, decisions, canonical_refs, code_context,
specifics, deferred.

Transform:

- `domain` becomes the Domain section of `.agent/spec/architecture.md`.
- `decisions`: scan each. A hard-to-reverse and surprising decision becomes
  `specs/adr/NNNN-{slug}.md`. A lightweight decision goes to
  `.agent/spec/decision-log.md`.
- `canonical_refs` becomes reference links in `architecture.md`.
- `code_context` becomes the Architecture section.
- `deferred` becomes `out_of_scope` in `.agent/product/scope.yml`, with a
  "(deferred from GSD)" note.

---

### `.planning/phases/XX-name/XX-YY-PLAN.md` → `.agent/tasks/<group>/` tasks

GSD PLAN has frontmatter (depends-on, verify), an objective, typed tasks, success
criteria, and an output spec.

Transform:

- Preserve the task structure as `tasks[]` in the task group.
- Keep `verify: <command>` lines.
- Map GSD `depends-on` onto task `depends-on` notes.
- SUMMARY.md (an execution record) is skipped (see the skip list).

---

### `.planning/METHODOLOGY.md` → `.agent/spec/methodology.md`

GSD METHODOLOGY.md is a standing reference for analytical lenses (Bayesian
updating, STRIDE, cost of delay).

Transform:

- Copy each lens as a section in `.agent/spec/methodology.md`.
- Note: "These lenses inform `plan-work` and `audit-code` sessions."

---

### `.planning/HANDOFF.json` + `.continue-here.md` → `.agent/tasks/state.yml` `handoff`

GSD HANDOFF has the current phase, last plan, blocking reason, and required-reading
list.

Transform — populate `handoff` in `.agent/tasks/state.yml`:

```yaml
handoff:
  last_step_completed: "<phase/plan from HANDOFF>"
  open_decisions:
    - "<blocking reason if any>"
  required_reading:
    - "<required_reading list>"
  next_skill: survey-context
```

---

### `.planning/spikes/SPIKE-NNN/README.md` → `.agent/spec/spikes/SPIKE-{name}.md`

GSD spike README has YAML frontmatter (verdict, validates, related), a methodology,
findings, and a recommendation.

Transform:

- Flatten the directory into `.agent/spec/spikes/SPIKE-{name}.md`.
- Preserve the frontmatter as a YAML block at the top.
- Keep the verdict prominent: `**Verdict:** ADOPTED / REJECTED / DEFERRED`.

---

## Skip List

These GSD artifacts are not migrated. They are execution records, not planning
inputs.

| Artifact                                   | Reason                                             |
| ------------------------------------------ | -------------------------------------------------- |
| `.planning/phases/XX/XX-YY-SUMMARY.md`     | Execution log. No TrueNorth equivalent.            |
| `.planning/phases/XX/XX-DISCUSSION-LOG.md` | Audit trail only. Not consumed by agents.          |
| `.planning/USER-PROFILE.md`                | User calibration. TrueNorth has no profile system. |
| `.planning/sketches/`                      | Visual exploration. Not spec artifacts.            |
