# ADR-0006: Model Routing — Skill-Specific Model Assignment

**Status:** Accepted (shipped in v2.x — evolved from the original v2.4.0 estimate)
**Date:** 2026-05-20
**Amended:** 2026-07-03 (update status and clarify enforcement model)

## Context

Using the most capable model for every skill wastes cost on tasks where lighter models are
sufficient. Using the cheapest model everywhere degrades quality on complex reasoning
tasks (planning, architecture decisions, assumption surfacing) by 15–23%.

## Decision

Skills are assigned a model tier based on task complexity:

| Tier     | Model             | Tasks                                       | Token budget |
| -------- | ----------------- | ------------------------------------------- | ------------ |
| Light    | Haiku-equivalent  | Code review, verification, commit messages  | 100K         |
| Standard | Sonnet-equivalent | Code writing, research, planning            | 200K         |
| Deep     | Opus-equivalent   | Strategic planning, grill-me, ADR decisions | 250K         |

The assignment lives in `docs/references/model-profiles.md`. Skills declare their tier in SKILL.md
frontmatter via the `model:` field.

## Enforcement model (what shipped)

Routing is **declared in frontmatter but enforcement is harness-dependent.**

- **Declarative:** Every SKILL.md carries a `model:` field (see `model-profiles.md` for the
  canonical tier list). This is the single source of truth.
- **Harness-dependent:** The host agent harness (Claude Code, Cursor, Gemini CLI, pi) reads
  the `model:` field but each enforces routing differently — or not at all. Claude Code and pi
  respect tier hints natively; Cursor and Gemini CLI currently ignore them.
- **No central orchestrator:** The original ADR assumed `orchestrate-project` would enforce
  routing. Instead, each skill's `model:` declaration is read directly by the harness at launch
  time — simpler, no single point of failure, but with per-harness variance.

## Consequences

- ~35% cost reduction vs. all-premium baseline (harness-dependent — varies by tool).
- Quality maintained on complex tasks (premium models where it matters).
- The `model:` field in frontmatter is declarative and non-breaking — harnesses that don't
  support routing silently ignore it. This is intentional: bigpowers can't control host
  behavior, only declare intent.
- `model-profiles.md` must stay in sync with SKILL.md updates (same consequence as original).

## Fork reconciliation (TrueNorth)

TrueNorth-MCP keeps the premise of matching effort to task, but the mechanism changed and
one half of the original decision is gone.

The declarative half no longer exists. No skill in `skills/` carries a `model:` frontmatter
field, and there is no `docs/references/model-profiles.md` in this repo. The
per-skill tier declaration the ADR describes did not survive the fork.

The surviving mechanism is payload tiering in the runtime. The `get_skill` tool renders a
skill at a `full`, `reasoning`, or `lean` tier through `render_skill`
(`truenorth-mcp/src/engine/tier/mod.rs`, `truenorth-mcp/src/tools/skills.rs`). This serves
the same goal, spend less context where less is enough, but it tiers the emitted payload
rather than routing the skill to a named model, and the caller or harness picks the tier.
Model selection itself stays harness-dependent, as the original amendment already noted.

**Status today:** partially superseded. The `model:` frontmatter and `model-profiles.md`
are retired. The effort-matching premise lives on through the runtime tiered `get_skill`.
