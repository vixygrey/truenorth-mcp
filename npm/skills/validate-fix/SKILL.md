---
name: validate-fix
description: Prove a fix works before you declare it done. Re-run the failing test, run the full suite, typecheck, lint, and harden against recurrence. Use it after implementing a bug fix, when the user asks "is this fixed?", or before closing an investigation.
kind: prose
---

# Validate Fix

> **HARD GATE**: the fix must not regress. Run the full test suite and manual UAT before you declare success.

Prove the fix works. "I think it works" is not evidence. Run the suite, show the
output, then harden against recurrence.

> **Two-commit red/green policy**: a bug fix follows the same two-commit discipline as `develop-tdd`. The first commit adds or adjusts the failing test (`test(<scope>): ...`). The second commit applies the fix (`fix(<scope>): ...`). Do not squash RED and GREEN before review.

## Checklist

### 1. Re-run the originally failing test

Run the specific test that captured the bug.

- [ ] The previously failing test now passes.

### 2. Run the full test suite

Run every test with no filtering, through the `truenorth_verify_gate` tool.

- [ ] All tests pass, zero regressions.

### 3. Typecheck

- [ ] No type error introduced.

### 4. Lint

- [ ] No lint violation introduced.

### 5. Harden against recurrence

For every bug fixed, add at least one prevention layer.

| Mechanism                 | When to use                                 |
| ------------------------- | ------------------------------------------- |
| Type guard                | The input could be the wrong shape          |
| Schema validation         | External data crosses a boundary            |
| Invariant assertion       | Internal state that must always hold        |
| Lint rule                 | A pattern that is easy to repeat by mistake |
| Startup environment check | A missing config causes a silent failure    |

- [ ] At least one hardening mechanism added.
- [ ] The hardening mechanism is tested.

### 5b. Generalize the fix (HARD GATE)

Sweep the defect class across the codebase after the local hardening. See
[REFERENCE-generalize-fix.md](REFERENCE-generalize-fix.md).

- [ ] The defect class is documented, not just the one-line root cause.
- [ ] A codebase sweep is run and the match count is recorded.
- [ ] The generalize sweep passes on the artifact.

### 6. Update the external issue and lean reference

Post the resolution to the same external issue used for investigation and implementation:

```markdown
## Resolution

**Fixed:** [date]
**Root cause confirmed:** [one sentence]
**Fix applied:** [behavioral change]
**Hardening added:** [type guard / schema / assertion / lint rule]
**Evidence:** [specific test and manual proof]
**Commit:** `fix(<scope>): <description>`
```

Call `truenorth_record_bug` with the issue's existing canonical id and `external_link`, status `resolved`, its linked task or group, and current tags. The tool updates the lean reference without creating another row. Close the external issue only after its behavioral proof and release policy allow closure.

- [ ] The same external issue contains the resolution and evidence.
- [ ] The lean local reference is `resolved` or `closed`.

### 7. Behavioral proof (HARD GATE)

Mechanical verification (the tests passing) is only half the fix. You must prove
behavioral correctness.

- [ ] Manually demonstrate the fixed behavior.
- [ ] Compare the output or state against the expected behavior in the external issue.
- [ ] Show the user evidence of the behavior, not just the test logs.

## Rules

- **Loop until behavioral correctness is verified**: when any checklist item
  fails, or the behavior is still incorrect despite passing tests, return to step
  1 and run every check again from the top. Do not declare done until every item
  is green and the behavior is proven correct in a single run.
- **Never use a type-ignore, an `as any`, or a lint-disable to fix a bug.** These
  suppress the symptom without fixing the root cause.
- **Never mark the task done while any test is failing.**
- **The verify command from the external issue or active group task must pass.**

Suggest the next skill: `audit-code`, then `commit-message`.

## Verify

Run the full test suite through the `truenorth_verify_gate` tool. A pass returns
exit 0.
