# ADR-0001: Verb-Noun Skill Naming

**Status:** Accepted
**Date:** 2026-05-18

## Context

Skills need names that are memorable, discoverable by agents scanning a directory, and unambiguous
when grep'd. Early naming (single nouns like `tdd`, `review`) caused collisions and unclear intent.

## Decision

All skill directories use a two-word `verb-noun` kebab-case pair (e.g., `develop-tdd`,
`audit-code`, `plan-work`). Exceptions (meta-skills, mode toggles) must be documented in
`CONVENTIONS.md` — they are not silent deviations.

## Consequences

- A global `grep` for any skill name returns < 5 results (grep-ability mandate, Akita #3).
- Skills self-document their intent: `investigate-bug` is unambiguous; `debug` is not.
- Two-word constraint prevents sprawl — a skill that needs three words is probably two skills.
- `terse-mode` and `visual-dashboard` are named exceptions (adjective-noun); noted in CONVENTIONS.md.

## Fork reconciliation (TrueNorth)

TrueNorth-MCP inherited this decision from bigpowers and keeps it. The skill sources
under `skills/` are named verb-noun kebab-case, and the documented exceptions still hold:
`terse-mode` and `visual-dashboard` are the adjective-noun cases named above. TrueNorth
renamed the guide skill from `using-bigpowers` to `using-truenorth`, so the convention
survived the rename.

The runtime does not enforce the naming; it is a source convention on the `skills/`
directory. The runtime reads a skill by its directory name through the `get_skill`,
`index_skills`, `read_skill`, and `search_skills` tools, so a clear verb-noun name stays
the discoverability contract the ADR intended.

**Status today:** live, as a `skills/` source convention.
