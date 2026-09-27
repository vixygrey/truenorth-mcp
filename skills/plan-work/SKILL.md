---
name: plan-work
description: "Planning spine step 3 of 3. Plan the work: write detailed implementation tasks into the active task group. Produces a countable-story-format spec and a runnable tasks file. Use it after slice-tasks. Not a substitute for scope-work or slice-tasks."
kind: prose
---

# Plan Work

> **Spine position**: step 3. `scope-work`, then `slice-tasks`, optional
> `plan-tests`, then `plan-work`.

Produce the detailed specification and runnable task ledger for one story already
declared in the active group manifest.

## Artifact contract

- **Planning writes**: `.agent/tasks/<capsule>/eNNsYY-<slug>.md` and
  `.agent/tasks/<capsule>/eNNsYY-tasks.yaml` for the selected story.
- **Coordination write**: `.agent/tasks/state.yml` handoff only.
- **Reads**: the release index, product scope, active `group.yml`, tech stack,
  glossary, and `test-plan.md` when present.
- **Readers**: execution, development, verification, and traceability skills.
- **Never writes**: the release index, `group.yml`, `test-plan.md`, or
  `execution-status.yml`.

> **HARD GATE**: The selected story must already exist in `group.yml`, with its
> specification and task filenames reserved. If the group or story is missing,
> return to `slice-tasks`. Never synthesize or repair an upstream artifact.
>
> Do not proceed until the success criteria are clear. Every task must include a
> runnable verify command.

When the plan touches an existing module, run `assess-impact` first to understand
the blast radius.

> **DISCOVERY MANDATE**: for an external API integration, verify the API signature via local docs or a search, and quote at least one technical detail in the step context.

> **MULTIPLE INTERPRETATIONS** (HARD GATE): when the task admits two or more valid interpretations, list them and get a user decision before drafting any step.

> **COMPLEXITY PUSHBACK** (HARD GATE): every new abstraction MUST include a one-sentence reason for depth. When it cannot be filled non-trivially, the abstraction is premature. Use inline code instead.

> **SLOPCHECK** (HARD GATE): for every external package, tag it `[OK]`, `[SUS]`, or `[SLOP]`. A `[SUS]` or `[SLOP]` requires human approval before execution.

## Invocation modes

- Default: the full plan with the zoom-out mandate, the impact assessment, and the
  slopcheck.
- `--fast`: skip the zoom-out and the impact assessment. Use it for a small task
  with no module-interface change.

## Process

1. **Explore**: understand the affected modules, the existing test patterns,
   similar prior art, and the dependencies.
2. **Draft the steps**: break the implementation into the smallest possible steps.
   Each step leaves the codebase working, has one observable outcome, and is
   verifiable with a single command. Name any rationalization you caught before you
   move on.
3. **Write the selected story specification and task ledger**: create only the two
   filenames reserved for the story in `group.yml`. See
   [REFERENCE.md](REFERENCE.md) for their formats. Each task MUST include a `risk:`
   field from P0 to P3. When `test-plan.md` exists, preserve its risk
   classifications and scenario ids. Each task can include a `security:` field. A
   `security: medium` or `high` task MUST include "no new security findings in
   affected paths" in its verify steps.

   Requirement delta tags: when a story modifies existing behavior, the story-spec
   requirements MUST use delta tags with mandatory before-and-after content.

   | Tag        | When                       | Required content                                         |
   | ---------- | -------------------------- | -------------------------------------------------------- |
   | `ADDED`    | A new requirement          | The full requirement text                                |
   | `MODIFIED` | Changed behavior           | Before: the prior behavior. After: the new behavior      |
   | `REMOVED`  | A retired requirement      | Before: what existed. After: removed, plus the rationale |
   | `RENAMED`  | An id or title change only | Before: the old id or title. After: the new one          |

   A greenfield story uses `ADDED` only. A `MODIFIED`, `REMOVED`, or `RENAMED`
   without before-and-after blocks fails the plan-work gate.

4. **Verify the step format**: every step MUST follow "N. <what to do>, then
   verify: <runnable command>". See [REFERENCE.md](REFERENCE.md) for examples.
   4a. **Cross-artifact consistency pass** (HARD GATE): before handoff, verify
   that the story id agrees across `group.yml`, both reserved filenames, the story
   specification, and the task ledger. Every referenced scenario id must exist in
   `test-plan.md`. Classify each mismatch as CRITICAL, HIGH, or MED. A CRITICAL or
   HIGH finding blocks handoff. Report an upstream mismatch to its owning skill;
   never rewrite that artifact. A MED finding requires explicit user acknowledgment.
   4b. **Tasks failing ledger**: every new task entry starts with `status: failing`.
   Flip it to `status: passing` only after its verify command exits 0 during
   develop-tdd or verify-work. Never pre-mark a task passing at plan time.
5. **Review with the user**: confirm the step order, the granularity, and that the
   verify commands are runnable in this project.

## Lifecycle gates

| Gate                      | When                                          | Pass condition                                                                                              |
| ------------------------- | --------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| **Pre-implementation**    | Before kickoff-branch or the first RED commit | A root cause is stated for a bug, `assess-impact` is done for a module change, the consistency check passes |
| **Validation**            | The story is marked done                      | Every task is `status: passing`, with verify evidence recorded                                              |
| **Reopen, do not refile** | A regression on a shipped story               | Reopen the existing story or bug, do not create a duplicate capsule entry                                   |

After writing the capsule tasks, suggest `kickoff-branch` (when not already on a
feature branch), then `build-epic`, `execute-plan`, or `develop-tdd`.

## Verify

Confirm the selected story has exactly one specification and one task ledger, all
story ids and reserved filenames agree with `group.yml`, every test scenario
reference resolves, and no CRITICAL or HIGH consistency finding remains.

## Handoff

Gate: READY. Next: `kickoff-branch`.
Writes: `.agent/tasks/state.yml` `handoff.next_skill = kickoff-branch`.
