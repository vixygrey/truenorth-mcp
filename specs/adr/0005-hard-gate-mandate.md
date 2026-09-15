# ADR-0005: Hard Gate Mandate

**Status:** Accepted
**Date:** 2026-05-20

## Context

Agents rationalise skipping quality checks ("tests are already comprehensive", "this is too simple
to need design"). Without a visible, named stop condition, agents proceed through gates silently.
Prose instructions like "make sure to review" are ignored under time pressure.

## Decision

Every skill that has a critical transition point must include a `> **HARD GATE**` blockquote
immediately before that transition. The blockquote names the condition that must be true before
proceeding and uses a `→ verify:` shell command to make it testable.

```markdown
> **HARD GATE** — Do not proceed until all tests pass.
>
> → verify: `npm test 2>&1 | tail -5`
```

Skipping a HARD GATE requires the agent to name the rationalisation explicitly in `STATE.md`.

## Consequences

- Agents cannot silently skip gates — the skip must be logged.
- Skills are slightly longer (gate blockquotes add 3–5 lines each).
- Hard gates are scannable at a glance — reviewers can audit gate coverage in `audit-code`.
- The `superpowers` benchmark feature passes on this criterion.

## Fork reconciliation (TrueNorth)

TrueNorth-MCP keeps the HARD GATE mandate unchanged. Skills across the catalog carry the
`> **HARD GATE**` blockquote at their critical transitions, for example `build-epic`,
`plan-release`, `audit-code`, `grill-me`, and `security-review`. The convention is also
documented in the domain glossary (`.agent/product/glossary.yml`).

One detail moved with ADR-0011. The original ADR said a skipped gate must be logged in
`STATE.md`; the cockpit state file is now `.agent/tasks/state.yml`, so a documented skip is
recorded there. The runtime reinforces the same intent at the protocol layer: the
`truenorth_verify_gate` tool passes only on exit 0, so a gate backed by a runnable command
is enforced by the server, not only by a blockquote a model can read past.

**Status today:** live, as a `skills/` convention, with the skip record in
`.agent/tasks/state.yml`.
