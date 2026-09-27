---
name: request-review
description: "Dispatch fresh reviewer agents with clean contexts to critique code after audit-code passes. Reviewer count and focus follow change risk. Use it before committing or for an independent auditor, code review, or pre-review critique."
kind: prose
---

# Request Review

Dispatch independent reviewer agents with clean contexts. A reviewer has no shared
state, so it can find what the coding agent missed.

Distinct from `audit-code`. `audit-code` is self-review. Run it first so
independent reviewers can focus on correctness, risk, and design rather than
routine hygiene.

## Risk-based reviewer policy

Read the active story `risk:` field, defaulting to P1 when absent:

| Risk | Reviewer policy                                                                                                  |
| ---- | ---------------------------------------------------------------------------------------------------------------- |
| P0   | Two independent reviewers. Add a security or domain specialist when the affected boundary requires it.           |
| P1   | One independent reviewer. Add a specialist for security-sensitive or cross-boundary changes.                     |
| P2   | One focused reviewer for the changed behavior and its evidence.                                                  |
| P3   | One lightweight reviewer when this skill is invoked. Independent review may remain optional in the orchestrator. |

When multiple reviewers run, keep their contexts independent until all reports
arrive. Reviewer count controls depth, not the pass rule.

> **HARD GATE**: review passes only when no blocking finding remains unresolved. Reviewer count and percentage scores do not determine the verdict.

## Finding classes

| Class            | Meaning                                                                                                | Effect                            |
| ---------------- | ------------------------------------------------------------------------------------------------------ | --------------------------------- |
| **blocking**     | A correctness defect, security vulnerability, failing required check, or explicit convention violation | Must be resolved before pass      |
| **non-blocking** | A concrete improvement whose current impact does not block delivery                                    | Record and address or disposition |
| **advisory**     | A preference, alternative, or question without a demonstrated defect                                   | Record for consideration          |

## Process

### 1. Prepare the review brief

Write a self-contained brief. Include the feature behavior, changed files,
relevant planning artifacts, active project conventions, risk tier, verify
command, behavior-smoke evidence, and the uncertain areas.

When the task group has a threat model, include the relevant vulnerability
categories and false-positive exclusions. Tag the review as security-sensitive
when the threat-model risk is HIGH or greater.

### 2. Dispatch the required reviewers

Dispatch the risk-required reviewer set in one parallel batch when more than one
reviewer is needed. Give each the same core brief and a fresh context. Add
specialist focus without narrowing the core correctness review.

```text
You are an independent code reviewer.

Context: [the feature behavior]
Conventions: [the active project rules]
Risk: [P0|P1|P2|P3]
Diff: [the changed files]
Verify command: [a runnable command]
Behavior evidence: [the observed smoke result]

Review correctness, edge cases, convention compliance, test quality, design,
security, and the supplied evidence. For each finding, provide its location,
consequence, evidence, remediation, and class: blocking, non-blocking, or
advisory. Run the verify command and report the result.
```

### 3. Collect and decide

Read every report before acting. Merge duplicate findings without losing
independent evidence. The round passes when:

- every required reviewer completed;
- every required verify command passed; and
- no blocking finding remains unresolved.

Non-blocking and advisory findings never become blockers through a score or
count. Record their disposition.

### 4. Respond and iterate

Pass all findings to `respond-review`. Fix or explicitly resolve every blocking
finding, then re-dispatch only the reviewers needed to verify affected areas.
Preserve independent contexts for any multi-reviewer rerun.

Stop after five unsuccessful rounds and report `Review cap exhausted. Human
decision required.` Do not merge with an unresolved blocker. The cap limits
operation cost; it is not a quality score.

## Verify

Confirm the risk-required reviewers completed, required verification passed, all
findings have a disposition, and no blocking finding remains unresolved.
