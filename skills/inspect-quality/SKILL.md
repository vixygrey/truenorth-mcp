---
name: inspect-quality
description: "An interactive QA session. The user reports bugs conversationally, and the agent creates external issues plus lean local references. Explores the codebase in the background for context and domain language. Use it to report bugs, do QA, or when the user mentions a QA session."
kind: prose
---

# Inspect Quality

> **HARD GATE** — Quality metrics (coverage, lint, cyclomatic complexity, security scans) must be monitored. If a metric degrades, surface it as a blocker. Do NOT accept regressions.

Run an interactive QA session. The user describes problems they're encountering. You clarify, explore the codebase for context, create an issue in the configured external tracker, and record only its lean reference locally.

## For each issue the user raises

### 1. Listen and lightly clarify

Let the user describe the problem in their own words. Ask **at most 2–3 short clarifying questions** focused on:

- What they expected vs what actually happened
- Steps to reproduce (if not obvious)
- Whether it's consistent or intermittent

Do NOT over-interview. If the description is clear enough to log, move on.

### 2. Explore the codebase in the background

Kick off an Agent (subagent_type=Explore) to understand the relevant area. The goal is NOT to find a fix — it's to:

- Learn the domain language used in that area (check the project glossary if present)
- Understand what the feature is supposed to do
- Identify the user-facing behavior boundary

### 3. Assess scope: single issue or breakdown?

Break down when:

- The fix spans multiple independent areas
- There are clearly separable concerns that could be worked on in parallel
- The user describes something with multiple distinct failure modes

Keep as a single issue when:

- It's one behavior that's wrong in one place
- The symptoms are all caused by the same root behavior

### 4. Create or update the external issue

Create one issue in the configured external tracker for each independent bug. The issue body carries:

- Actual and expected behavior
- Reproduction steps
- Severity and priority
- Relevant domain context

The tracker-issued id and URL are canonical. Do not generate a second local bug id. Do not copy the issue body, reproduction steps, diagnosis, or implementation detail into `.agent/tasks/bugs.yml`.

When the report is a recurrence, update the original external issue with the new evidence instead of creating another narrative. Keep its canonical id and URL.

### 5. Record the lean local reference

Call `truenorth_record_bug` after the external tracker returns the canonical id and URL:

```json
{
  "id": "<tracker issue id>",
  "external_link": "<absolute issue URL>",
  "status": "open",
  "linked_ref": "<existing task or group id>",
  "tags": ["<scope>", "<priority>"]
}
```

Use an existing task or group as `linked_ref`. If none exists, create the corresponding planned task before recording the bug. Never edit `.agent/tasks/bugs.yml` directly.

### 6. Continue the session

After logging, ask: "Next issue, or are we done?" Keep going until the user says done. Each issue is independent — don't batch them.
