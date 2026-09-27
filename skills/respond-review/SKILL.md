---
name: respond-review
description: "Act on a reviewer agent feedback systematically. Categorize the findings, apply the fixes, and verify the tests still pass. Use it after request-review returns a report, or when the user wants to work through code-review findings."
kind: prose
---

# Respond Review

> **HARD GATE**: every reviewer comment MUST be addressed. Fix it, disagree and document the reason, or ask for clarification. Do NOT ignore feedback and merge.

Work through reviewer findings systematically. Don't apply changes blindly — categorize first, then decide, then fix, then verify.

## Process

### 1. Read the full review report

Read every finding before acting on any of them. Get the full picture first.

### 2. Categorize findings

For each finding, preserve or assign a class:

| Class            | Meaning                                                                                                | Action                             |
| ---------------- | ------------------------------------------------------------------------------------------------------ | ---------------------------------- |
| **blocking**     | A correctness defect, security vulnerability, failing required check, or explicit convention violation | Resolve before proceeding          |
| **non-blocking** | A concrete improvement whose current impact does not block delivery                                    | Apply or record a disposition      |
| **advisory**     | A preference, alternative, or question without a demonstrated defect                                   | Consider with the user when needed |

Create a numbered list of all findings with their classes.

### 3. Confirm advisory decisions

For an advisory item that requires a product or architecture choice, briefly
describe the tradeoff and ask: `Apply, skip, or discuss?`

### 4. Resolve blocking items first

Resolve every blocking item. For each one:

- Describe what is changing and why.
- Make the change or document evidence-backed disagreement.
- Run the verify command for the affected area.
- Record the resolution. An undocumented disagreement leaves the blocker open.

### 5. Address non-blocking items

Apply each non-blocking item or record why it is deferred. If an item is large
enough to warrant separate work, link that work in the disposition.

### 6. Run the full suite

After all changes are applied:

```bash
<full test command>
<typecheck command>
<lint command>
```

- [ ] All tests pass
- [ ] No type errors
- [ ] No lint violations

### 7. Report

Summarize every disposition:

```text
Resolved (blocking): #1, #2, #3
Applied (non-blocking): #4
Deferred (non-blocking): #5, linked to #123
Skipped (advisory): #6, agreed with user
Open blockers: none
All required checks pass.
```

Suggest next skill: `commit-message`.
