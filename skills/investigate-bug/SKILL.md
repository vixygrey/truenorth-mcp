---
name: investigate-bug
description: "Investigate an external bug issue, verify its root cause, and post an evidence-backed TDD fix plan to the same tracker issue. Use it when the user reports a bug, wants to investigate a problem, mentions triage, or wants to plan a fix."
kind: prose
---

# Investigate Bug

**Boundary**: End-to-end bug entry point — history check → RCA (via `diagnose-root`) → fix approach → TDD plan → bug record. Delegates the 4-phase RCA to `diagnose-root`; does not re-implement it.

Investigate a reported problem, find its root cause, and record a TDD fix plan in the external tracker. This is a mostly hands-off workflow — minimize questions to the user.

## Process

### 0. Read previous bug history

Before starting diagnosis:

1. Read the bug references under `.agent/tasks/bugs.yml` (if it exists). Check for prior bugs in the same `scope` or with similar symptoms.
2. If a relevant prior bug is found, read its detail in the external tracker to understand previous root cause analysis and fix approach.
3. Note in your investigation whether this is a recurrence, a related issue, or novel.

### 1. Capture the problem

Get a brief description of the issue from the user. If they haven't provided one, ask ONE question: "What's the problem you're seeing?"

Do NOT ask follow-up questions yet. Start investigating immediately.

> **Security-impact assessment** — After capturing the problem, assess and document: `Security impact: NONE / LOW / MEDIUM / HIGH / CRITICAL`. If HIGH or CRITICAL, assign bug severity HIGH and document the exploit path in the external issue. If MEDIUM+, include the exploit path in the findings. Document "no security exploit path identified" for NONE/LOW.

### 2. Explore and diagnose (4-phase RCA)

Run the 4-phase root-cause analysis via the `diagnose-root` skill (Reproduce → Isolate → Hypothesize → Verify). That skill is the canonical RCA engine — do not re-implement the phases here.

Also look at:

- Recent changes to affected files (`git log --oneline <file>`)
- Existing tests (what's tested, what's missing)
- Similar patterns elsewhere in the codebase that work correctly

> **HARD GATE** — Do NOT proceed to Step 3 (Fix Approach) until `diagnose-root` Phase 4 produces a verified root cause. "It probably is X" is not verified.

### 3. Identify the fix approach

Based on your investigation, determine:

- The minimal change needed to fix the root cause
- Which modules/interfaces are affected
- What behaviors need to be verified via tests
- Whether this is a regression, missing feature, or design flaw
- Risk level: Low / Medium / High

### 4. Design TDD fix plan

Create a concrete, ordered list of RED-GREEN cycles. Each cycle is one vertical slice:

- **RED**: Describe a specific test that captures the broken/missing behavior
- **GREEN**: Describe the minimal code change to make that test pass

Rules:

- Tests verify behavior through public interfaces, not implementation details
- One test at a time, vertical slices (NOT all tests first, then all code)
- Each test should survive internal refactors
- Include a final refactor step if needed
- **Durability**: Only suggest fixes that would survive radical codebase changes. Tests assert on observable outcomes (API responses, UI state, user-visible effects), not internal state.

### 5. Update the external issue and local reference

Post the investigation and fix plan to the same external issue. The external tracker owns all bug detail.

For a recurrence, update the original issue with the new evidence and note that the defect recurred. Do not create a second issue or local narrative for the same defect.

Use this as an external tracker comment or issue-body template:

<external-tracker-update>

## Root Cause Analysis

- Actual and expected behavior
- Reproduction evidence
- Verified root cause
- Contributing factors
- Security impact
- Risk level

## TDD Fix Plan

For each vertical slice:

1. **RED**: the public behavior the regression test must expose
2. **GREEN**: the minimal implementation change
3. **Verify**: the runnable command

Add the final refactor step and acceptance criteria. Avoid file paths and line numbers that will become stale.

</external-tracker-update>

Call `truenorth_record_bug` with the same canonical id and `external_link`, status `in-progress`, the linked task or group, and the current tags. The tool updates the existing lean reference. Never edit `.agent/tasks/bugs.yml` directly.

After updating the issue, print a one-line summary of the root cause and suggest running `kickoff-branch` next to create a fix branch.

## References

<!-- story: e35s10 -->

- Feathers' Seams and Characterization Tests: seam types, characterization tests, and the legacy-code change algorithm.
- Fowler's Code Smells: identifying a structural problem by its smell before investigating.
