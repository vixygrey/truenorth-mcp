---
name: fix-bug
description: "A bug-fix orchestrator. Sets the fix_bug flow and chains external issue investigation, TDD implementation, behavioral validation, and release. Use it when the user reports a defect."
kind: prose
---

# Fix Bug

**Boundary**: Orchestrator flow. Chains `investigate-bug` → `develop-tdd` → `validate-fix` → `release-branch`. It does not perform RCA, implement the fix, create issue content, or write bug references directly.

Orchestrates **fix_bug** flow without mixing group build state.

> **HARD GATE** — Set `.agent/tasks/state.yml` `active_flow: fix_bug` and `bug_cycle.current_step: 1` before starting.

## Discovered gate failures (e51s04)

Valid entry **without a user-reported bug** when:

- **Preflight** or **CI** is red at kickoff or verify-work
- The project baseline is red, per the `truenorth_verify_gate` tool
- A reproducible gate failure during unrelated group work exceeds quick-fix guardrails

Record the gate failure in the configured external tracker through `investigate-bug`, then run the standard fix_bug chain.

## Four steps (`bug_cycle` in `.agent/tasks/state.yml`)

| Step | Skill / action                                                               |
| ---- | ---------------------------------------------------------------------------- |
| 1    | `investigate-bug` — verify RCA and update the external issue                 |
| 2    | `develop-tdd` — run RED-GREEN against the external issue's TDD plan          |
| 3    | `validate-fix` — prove behavior and update the same issue and lean reference |
| 4    | `release-branch` — PR or solo land the fix                                   |

The 4-phase root-cause analysis is not a separate step. `investigate-bug` owns
and runs it directly before producing the TDD plan.

### Checkpoint / resume

Track progress via `.agent/tasks/state.yml` `bug_cycle`:

- `bug_cycle.current_step`: current step (1–4)
- `bug_cycle.completed_steps`: completed step numbers
- `handoff.next_skill`: skill for the current step
- On resume, read `bug_cycle.current_step` and continue from there

## Process

1. **Step 1 — investigate-bug:** Run `investigate-bug`. It checks history, completes the four-phase RCA, posts the fix approach and TDD plan to the external issue, and records or updates the lean reference through `truenorth_record_bug`. Increment `bug_cycle.current_step` to 2 on completion.
2. **Step 2 — develop-tdd:** Run `develop-tdd` against the external issue's TDD plan and verify commands. Increment to step 3 on all-green.
3. **Step 3 — validate-fix:** Run `validate-fix` to re-run the failing test, full suite, typecheck, lint, and behavioral proof. Update the same external issue and lean reference. Increment to step 4.
4. **Step 4 — release-branch:** Land the fix via `release-branch`. Clear `bug_cycle` and `active_flow` when done.

## Verify

Confirm the same external issue carries intake, RCA, the TDD plan, and resolution evidence. Confirm `.agent/tasks/bugs.yml` contains only one lean reference for its canonical id. The project verification must pass through `truenorth_verify_gate`.
