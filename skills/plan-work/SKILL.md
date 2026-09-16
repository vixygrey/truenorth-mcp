---
name: plan-work
description: "Planning spine step 3 of 3. Plan the work: write detailed implementation tasks into the active task group. Produces a countable-story-format spec and a runnable tasks file. Use it after slice-tasks. Not a substitute for scope-work or slice-tasks."
---

# Plan Work

> **Spine position**: step 3. scope-work, then slice-tasks, then plan-work.

Produce a detailed, verifiable implementation plan in the active task group
directory. Output: a story-spec file (countable-story-format) and a tasks file with
runnable verify commands. "I think it works" is not a step.

> **HARD GATE**: Do NOT proceed with a plan until the task success criteria are clear. When success is ambiguous, convert the task into "step, then verify: `<cmd>`" pairs here before writing the tasks. Every task ships a runnable verify, or the plan is not done.
>
> **RECURSIVE DISCIPLINE**: this lifecycle applies to every task, including updating these skills. Never skip planning because a task is meta or documentation.

## Pre-flight

Read the release plan, the product scope, the active task group, the tech stack,
and the glossary.

> **ZOOM-OUT MANDATE**: when modifying an existing module, state the module purpose, name its callers, and list its contracts. When you cannot answer all three, stop. The scope is misunderstood.

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
3. **Write the capsule story spec and tasks**: output two files inside the active
   task group. See [REFERENCE.md](REFERENCE.md) for the file formats. Each task
   MUST include a `risk:` field (`P0` to `P3`). When a test plan exists, inherit its
   risk classifications and scenario ids. Each task optionally includes a
   `security:` field, and a `security: medium` or `high` task MUST include "no new
   security findings in affected paths" in its verify steps.

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
   4a. **Cross-artifact consistency pass** (HARD GATE): before handoff, check the
   capsule artifacts for consistency. Classify each finding as CRITICAL, HIGH, or
   MED. A CRITICAL or HIGH blocks code generation. Fix the capsule artifacts first.
   A MED requires explicit user acknowledgment.
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

Confirm the active task group has a story spec and a tasks file, and that the
cross-artifact consistency pass reports no CRITICAL or HIGH finding.

## Handoff

Gate: READY. Next: kickoff-branch.
Writes: `state.yaml` `handoff.next_skill = kickoff-branch`.
