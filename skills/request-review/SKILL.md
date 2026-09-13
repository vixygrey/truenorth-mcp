---
name: request-review
description: 'Dispatch a fresh reviewer agent with a clean context to critique the code after audit-code passes. The reviewer has no shared state with the coding agent and gives a genuine second opinion. Use it after audit-code passes, before committing, or when the user wants an independent code review.'
---

# Request Review

Dispatch a fresh reviewer agent with a clean context. A reviewer has no shared
state, so it finds what the coding agent missed.

Distinct from `audit-code`. `audit-code` is self-review. This skill dispatches an
external agent.

Run `audit-code` first. Do not waste reviewer attention on a hygiene issue you could
have caught yourself.

## Dual-blind AND gate

Use two independent reviewers, A and B, with no shared context between them or the
coding agent.

| Parameter             | Value                                                        |
| --------------------- | ------------------------------------------------------------ |
| Reviewers             | 2 (mandatory)                                                |
| Max review iterations | 5 (a hard cap, iteration 6 is forbidden)                     |
| Pass rule             | An AND gate. Both reviewers must pass independently          |
| Blindness             | Neither reviewer sees the other's report until both complete |

Iteration loop (at most 5):

1. Dispatch reviewer A and reviewer B in parallel with identical briefs but separate
   contexts.
2. Collect both reports. Each categorizes findings: must-fix, should-fix, consider.
3. AND gate: when either reviewer has a must-fix finding, the round fails. Run
   `respond-review`, fix, and re-dispatch both reviewers.
4. When both pass (zero must-fix, a score of 94% or more each), the review is
   complete.
5. After 5 iterations without a dual pass, stop and report "Review cap exhausted.
   Human decision required.". Do not merge.

> **HARD GATE**: a single-reviewer pass is insufficient. Partial agreement does not satisfy the AND gate.

## Process

### 1. Prepare the review brief

Write a self-contained brief for each reviewer. Include what was built (the feature,
not the implementation), which files changed, the relevant `specs/` artifacts, what
the conventions require, the verify command, and what you are most uncertain about.

Security focus: when the epic has a threat model, include the relevant vulnerability
categories as reviewer focal points, plus the false-positive exclusion rules. Tag
the review as security-sensitive when the threat-model risk is HIGH or more.

### 2. Fan out parallel reviewers

Beyond the mandatory dual-blind pair, optionally dispatch several
dimension-specific subagents in one message, one check per agent, for broader
coverage.

| Agent       | Focus                                              |
| ----------- | -------------------------------------------------- |
| Correctness | Logic, edge cases, the verify-command result       |
| Conventions | The project conventions, test quality (F.I.R.S.T)  |
| Security    | Injection, auth, secrets (when security-sensitive) |
| Design      | A simpler alternative, the API shape               |

The dual-blind method still applies. Each agent is blind. The AND gate uses the A
and B scores. A fan-out agent feeds findings into `respond-review`, but does not
replace the dual-blind pair.

### 2b. Dispatch both reviewer agents

Dispatch two agents with completely fresh contexts. Each prompt is self-contained,
with no reference to the current conversation.

```text
You are code reviewer [A|B]. Review the following changes independently.

Context: [the feature description]
Conventions: [the relevant rules]
Active epic: [the relevant capsule]
Diff: [the changed files]
Verify command: [a runnable command]

Review for correctness, convention compliance, test quality, design, edge cases,
security, and refactoring smells. For each finding, categorize it as must-fix,
should-fix, or consider. Run the verify command and report the result.
```

### 3. Collect both reports

When the reviewers return, read every finding from both reports before acting.
Note each verify result. Compute a quality score per reviewer:
`100 * (total - must_fix - should_fix) / total`. Then check the AND gate: both
scores 94% or more and zero must-fix from both.

> **HARD GATE**: when either score is below 94% or either has a must-fix, the round fails. Run `respond-review` first.

### 4. Hand off to respond-review

Pass the combined findings to `respond-review` to categorize and apply the fixes.
Increment the iteration counter. Re-dispatch both reviewers until the AND gate passes
or the cap is exhausted. Report the round number and the two scores.

## Verify

Confirm both reviewers passed the AND gate for the current round, or that the
iteration cap was reached.
