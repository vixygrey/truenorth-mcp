# ADR-0003: Prescriptive Core Loop with Fast-Track Mode

**Status:** Accepted
**Date:** 2026-05-20

## Context

Without an enforced sequence, users skip validation gates (no `grill-me`, no `audit-code`) and
ship with untested assumptions. Free-form skill selection puts the orchestration burden entirely on
the user. GSD's fully prescriptive loop (no skips) is too rigid for brownfield projects.

## Decision

bigpowers enforces a prescriptive 6-phase core loop — discover → elaborate → plan → build →
verify → release — via `orchestrate-project`, with two opt-in modes:

- **Standard**: all gates enforced; no phase can be skipped without explicit confirmation.
- **Fast-track**: phases with measurable skip conditions (e.g., survey already done, coverage ≥ 95%) may be skipped with a logged rationale.
- **Ad-hoc**: legacy / experimental; no gate enforcement. User accepts full responsibility.

## Consequences

- Users can no longer silently skip `elaborate-spec` or `validate-fix`.
- Fast-track conditions are data-driven (file exists, metric threshold met) — not discretionary.
- Ad-hoc mode exists for experiments but is explicitly marked as lower-quality.
- Orchestration overhead: ~2% more tokens, ~15% more orchestrator complexity.

## Fork reconciliation (TrueNorth)

TrueNorth-MCP keeps the prescriptive loop and delivers it through two layers. The
`orchestrate-project` skill still carries the phase sequence for a harness that drives the
workflow through skills. The runtime adds an active enforcement layer: the
`truenorth_advance_phase` tool moves the phase and writes `.agent/tasks/state.yml`
(`truenorth-mcp/src/tools/lifecycle.rs`), and the `truenorth_verify_gate` tool runs a
project gate command in a sandbox and passes only on exit 0
(`truenorth-mcp/src/tools/gates.rs`).

The decision stands; the mechanism gained a protocol layer. The 6-phase lifecycle
(Discover, Design, Plan, Execute, Review and Harden, Integrate) is preserved from upstream,
as the refactor design states (`.kiro/specs/truenorth-mcp-refactor/design.md`). The gate is
now testable by the server, not only described in a skill, so an agent cannot claim a green
phase the runtime did not observe.

**Status today:** live, delivered by both the `orchestrate-project` skill and the runtime
lifecycle tools.
