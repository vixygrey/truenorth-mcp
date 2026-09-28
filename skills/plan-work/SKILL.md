---
name: plan-work
description: "Planning spine step 3 of 3. Write a detailed specification and runnable task ledger for one declared work item. Use after slice-tasks and optional plan-tests."
kind: prose
---

# Plan Work

> **Spine position**: step 3. `scope-work`, then `slice-tasks`, optional
> `plan-tests`, then `plan-work`.

Produce the detailed specification and runnable task ledger for one work item
already declared in the active task manifest.

## Artifact contract

- **Planning writes**: the specification and task-ledger filenames reserved for
  the selected work item under `.agent/tasks/<capsule>/`.
- **Coordination write**: `.agent/tasks/state.yml` handoff only.
- **Reads**: `.agent/profile.yml`, `.agent/tasks/release-plan.yml`, product scope,
  the active task manifest, tech stack, glossary, and `test-plan.md` when present.
- **Readers**: execution, development, verification, and traceability skills.
- **Never writes**: the release index, the task manifest, `test-plan.md`, or
  `.agent/tasks/execution-status.yml`.

> **HARD GATE**: The selected work item must already exist in the active task
> manifest, with its specification and task filenames reserved. If either is
> missing, return to `slice-tasks`. Never synthesize or repair an upstream
> artifact.
>
> Do not proceed until the success criteria are clear. Every task must include a
> runnable verify command.

When the plan touches an existing module, run `assess-impact` first. For an
external API integration, verify the API signature from primary documentation.
When two or more valid interpretations remain, obtain a user decision before
writing steps. Every new abstraction needs a concrete reason for its depth.
External packages must be tagged `[OK]`, `[SUS]`, or `[SLOP]`; the latter two
require human approval.

## Profile rules

Read `.agent/profile.yml` before naming work. Use `task` and `work item` in shared
artifacts. Profile-specific grouping is optional except for `epic-based` and
`milestone-based`, where the declared group is required. Do not invent story or
epic ids for `issue-per-task`, `kanban`, or `generic` projects. BCP and WSJF are
optional project policies, not plan-work defaults.

## Process

1. **Explore**: understand affected modules, existing test patterns, prior art,
   dependencies, and the active profile.
2. **Draft steps**: each step leaves the codebase working, has one observable
   outcome, and ends with one runnable verify command.
3. **Write the reserved artifacts**: create only the specification and task
   ledger reserved by the selected work item. Follow [REFERENCE.md](./REFERENCE.md).
   Every task starts with `status: failing` and includes `risk: P0|P1|P2|P3`.
   Preserve scenario ids and risk classifications from `test-plan.md`. A task
   with `security: medium` or `security: high` must verify that affected paths
   have no new security findings.
4. **Record requirement deltas**: use `ADDED`, `MODIFIED`, `REMOVED`, or `RENAMED`.
   The latter three require explicit before and after content.
5. **Check consistency**: the work-item id must agree across the manifest, both
   reserved filenames, the specification, and the task ledger. Every scenario id
   must resolve. CRITICAL or HIGH mismatches block handoff. Report upstream
   mismatches to their owning skill instead of rewriting them.
6. **Review**: confirm order, granularity, and runnable verification with the user.

## Lifecycle gates

| Gate               | When                                               | Pass condition                                                                                 |
| ------------------ | -------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Pre-implementation | Before `kickoff-branch` or the first local RED run | Root cause is stated for a bug, impact is assessed for a module change, and consistency passes |
| Validation         | The work item is marked done                       | Every task is passing and has verification evidence                                            |
| Reopen             | A regression affects shipped work                  | Reopen the existing task or bug instead of creating a duplicate                                |

## Verify

Confirm that the selected work item has exactly one specification and one task
ledger, all ids and reserved filenames agree, every test scenario reference
resolves, every task has a runnable verify command, and no CRITICAL or HIGH
consistency finding remains.

## Handoff

Gate: READY. Write the canonical `.agent/tasks/state.yml` handoff with
`next_skill: kickoff-branch`.
