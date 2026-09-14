---
name: develop-tdd
description: Test-driven development with a red-green-refactor loop using vertical slices. Use it for a feature (a group task) or a bug (a BUG report).
---

# Develop TDD

> **HARD GATE**: Do NOT proceed on `main` or `master`. Run `kickoff-branch` first to create a feature branch or worktree.
>
> **HARD GATE**: Do NOT write code before you have a plan. New feature: `plan-work` to task group tasks. Bug: `investigate-bug` to a BUG report, or use the `fix-bug` orchestrator.
>
> **RECURSIVE DISCIPLINE**: this lifecycle applies to every task, including updating these skills. Never skip planning because a task is meta or documentation.

## Philosophy

A test verifies behavior through the public interface, not an implementation
detail. A good test reads like a specification. See [REFERENCE.md](REFERENCE.md)
for the horizontal-slice anti-pattern and the TDD phase detail.

## Red flags

When you catch yourself thinking one of these, stop and reconsider. You are likely
deviating from production-grade craft.

| Red flag                                 | Reality                                                                                        |
| :--------------------------------------- | :--------------------------------------------------------------------------------------------- |
| "This is too simple to need a test."     | Simple code is where a bug hides. When it is simple, the test is cheap.                        |
| "I will refactor this later."            | Later is when technical debt becomes bankruptcy. Refactor while green.                         |
| "The tests are already comprehensive."   | When you add behavior, you need a new test. Coverage is not correctness.                       |
| "I am just fixing a small bug."          | A small bug often indicates a deep interface flaw. Investigate the root cause.                 |
| "I need to mock this internal class."    | Mocking an internal couples the test to the implementation. Mock only I/O.                     |
| "This refactor is out of scope."         | Leave the code cleaner than you found it.                                                      |
| "Preflight failed but it is unrelated."  | Always green: any reproducible gate failure routes quick-fix then fix-bug before forward work. |
| "I will note the red gate and continue." | Narrating a failure is banned. Fix-or-log is mandatory.                                        |

## Workflow

### 1. Planning

- [ ] Read the active group story tasks or the BUG report. Understand the verify steps.
- [ ] When a test plan exists for the active group, read it before the first test. Implement the P0 scenarios before P1. P2 and P3 are optional per the time budget.
- [ ] Confirm the interface changes and the behaviors to test. Prioritize them.
- [ ] Design the interfaces for testability. Identify deep-module opportunities.
- [ ] Get user approval on the plan.

Apply the enforce-first F.I.R.S.T rubric: Fast, Independent, Repeatable,
Self-Validating, Timely.

### 2. Tracer bullet

Write ONE test that confirms ONE thing about the system. Drive the cycle through
the `truenorth_tdd_cycle` tool, which enforces the red-green-refactor order and the
red-stage exit-code semantics.

```text
RED:      write a test for the first behavior, the test fails, commit test(<scope>): ...  (test-only, red in CI)
GREEN:    write the minimal code to pass, the test passes, commit feat(<scope>): ...      (fix commit, green)
REFACTOR: (optional) clean up, commit refactor(<scope>): ...
```

> **Two-commit red/green policy** (HARD GATE): each behavior cycle requires two separate commits. First a test-only commit that fails in CI (RED), then an implementation commit that makes it pass (GREEN). Never combine a test and a fix in one commit.

Run the RED stage through `truenorth_tdd_cycle`. It reports RED passed only when
the failing test command exits non-zero. When the test-only commit passes in
isolation, the RED gate is violated. Stop and fix it before GREEN. Show
`git log -2 --oneline` and the tool result as evidence.

> **tasks ledger**: after each task's `verify:` exits 0, set that task's `status: passing`. The story-level status is `passing` only when every task passes.

### 3. Incremental loop

Before each RED-to-GREEN or GREEN-to-REFACTOR transition, create a checkpoint, so
a failed transition rolls back cleanly. Never refactor while RED.

For each remaining behavior: RED, then GREEN, then REFACTOR (optional). One test at
a time. Two commits per behavior, test-only RED then fix GREEN. Commit after every
GREEN phase.

### 4. Visual slices (UI alternate workflow)

For a UI component where behavioral unit testing is brittle, extract the logic into
a controller, view model, or hook (pure TDD), then use visual slices for the view
layer. See [REFERENCE.md](REFERENCE.md) for the full procedure.

### 5. Refactor

After every test passes, extract duplication, deepen modules, and apply SOLID
principles. Never refactor while RED.

### 6. Verify

After every behavior cycle, run the verify command from the active group task
through the `truenorth_verify_gate` tool. Show the evidence before you declare the
step done.

### 6a. CI dry-run sub-step

When this cycle modified a file under `.github/workflows/`, run the CI dry-run
procedure in [REFERENCE.md](REFERENCE.md#ci-dry-run).

### 7. Manual verification handover

Once every test passes, locate the verification script in the active task group,
present it to the user step by step, and wait for confirmation of behavioral
correctness.

## Checklist per cycle

```text
[ ] The test describes behavior, not implementation
[ ] No test is ignored without an explicit ambiguity note
[ ] Boundary conditions tested: empty, max, min, off-by-one
[ ] The test verifies behavior through the public interface only, no private methods
[ ] The test would survive an internal refactor
[ ] The code is minimal for this test
[ ] No speculative feature added
[ ] Every new abstraction has an explicit reason-for-depth justification
[ ] Progress committed (Conventional Commits)
[ ] The verify command passes
```

## Verify

Run the verify command from the active group task through the `truenorth_verify_gate`
tool. A pass returns exit 0.

## Handoff

Gate: READY. Next: verify-work.
Writes: `state.yaml` `handoff.next_skill = verify-work`.
