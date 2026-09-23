---
name: fix-bug
description: "A bug-fix orchestrator. Sets the fix_bug flow, reads the BUG report, and chains investigate-bug, develop-tdd, and validate-fix. Use it when the user reports a defect."
kind: prose
---

# Fix Bug

**Boundary**: Orchestrator flow — chains `investigate-bug` (entry point + RCA via `diagnose-root`) → `develop-tdd` → `validate-fix`. Does not implement RCA or write bug files directly.

Orchestrates **fix_bug** flow without mixing group build state.

> **HARD GATE** — Set `.agent/tasks/state.yml` `active_flow: fix_bug` and `bug_cycle.current_step: 1` before starting.

## Discovered gate failures (e51s04)

Valid entry **without a user-reported bug** when:

- **Preflight** or **CI** is red at kickoff or verify-work
- The project baseline is red, per the `truenorth_verify_gate` tool
- A reproducible gate failure during unrelated group work exceeds quick-fix guardrails

Record the gate failure via `investigate-bug` (or inline in fix-bug step 1) in the external tracker, then run the standard fix_bug chain.

## Four steps (`bug_cycle` in state.yaml)

| Step | Skill / action                                                     |
| ---- | ------------------------------------------------------------------ |
| 1    | `investigate-bug` — create BUG-\*.md with RCA (runs diagnose-root) |
| 2    | `develop-tdd` — red-green against bug file verify steps            |
| 3    | `validate-fix` — re-run failing test, full suite, lint             |
| 4    | `release-branch` — PR or solo land the fix                         |

The 4-phase root-cause analysis is not a separate step. `investigate-bug` runs
`diagnose-root` internally, so the chain does not invoke it twice.

### Checkpoint / resume

Track progress via `.agent/tasks/state.yml` `bug_cycle`:

- `bug_cycle.current_step`: current step (1–4)
- `bug_cycle.completed_steps`: completed step numbers
- `handoff.next_skill`: skill for the current step
- On resume, read `bug_cycle.current_step` and continue from there

## Process

1. **Step 1 — investigate-bug:** If no bug record exists, run `investigate-bug` first. It handles the history check, the 4-phase RCA (via `diagnose-root`), the fix approach, and records the bug in the external tracker. Increment `bug_cycle.current_step` to 2 on completion.
2. **Step 2 — develop-tdd:** `develop-tdd` against the bug file's verify steps. Increment to step 3 on all-green.
3. **Step 3 — validate-fix:** `validate-fix` — re-run failing test, full suite, typecheck, lint. Increment to step 4.
4. **Step 4 — release-branch:** Land the fix via `release-branch`. Clear `bug_cycle` and `active_flow` when done.

## Bug file SoT

One markdown file per bug with frontmatter:

```yaml
---
bug_id: BUG-001
status: open
severity: high
scope: api
title: Short title
---
```

## Verify

Confirm the BUG report exists and the fix passes the project verification through
the `truenorth_verify_gate` tool.
