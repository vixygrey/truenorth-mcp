# ADR-0002: Local-First Specs

**Status:** Superseded by ADR-0008 (the `.agent/` vs `specs/` split)
**Date:** 2026-05-19

## Context

External trackers (Jira, Linear, GitHub Issues) require API calls, authentication, and introduce
latency. Agents relying on remote state cannot operate offline and incur token cost fetching
context that could be present in the local working tree.

## Decision

All planning and spec output goes to `specs/` at the project root, as Markdown files tracked by
Git. No bigpowers skill creates or reads from an external issue tracker. Skills that bridge to
external systems (e.g., future `to-issues`) are opt-in utilities, not core workflow steps.

## Consequences

- Full agent context is available without network calls.
- `specs/` becomes the project's long-term memory, accumulating across every phase.
- Losing external tracker sync means no sprint boards, no Gantt charts — deliberate trade-off.
- `git blame` on `specs/` files gives a full audit trail of decisions.

## Fork reconciliation (TrueNorth)

TrueNorth-MCP keeps the local-first premise: the project state lives on disk, tracked by
git, and the runtime reads it live rather than calling an external tracker. ADR-0008
refined where that state lives by splitting the tree into a machine-facing `.agent/` layer
and a human-facing `specs/` layer, so this ADR is superseded by that split, not by a
return to remote state.

Two later decisions carry the premise forward. ADR-0011 relocated the cockpit into
`.agent/`, so the runtime reads `.agent/tasks/state.yml` and `.agent/tasks/release-plan.yml`
live. ADR-0010 kept bug detail in an external tracker while storing only a lean reference
on disk, which is the one deliberate, bounded exception to pure local-first.

**Status today:** superseded by ADR-0008; the local-first premise remains in force through
ADR-0011.
