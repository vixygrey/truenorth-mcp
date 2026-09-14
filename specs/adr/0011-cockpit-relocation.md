# ADR-0011: Cockpit Relocation into `.agent/`

**Status:** Accepted
**Date:** 2026-09-13

## Context

The cockpit state files (`state.yaml`, `release-plan.yaml`, `ontology.yaml`,
`execution-status.yaml`) and the product concept lived under `specs/`. Under the
`.agent/` and `specs/` split (ADR-0008), machine-written state must live in the
machine-facing layer, not in the human-authored `specs/` layer.

## Decision

Relocate the cockpit into `.agent/`. The state file resolves at
`.agent/tasks/state.yml`, the release plan at `.agent/tasks/release-plan.yml`, and
the ontology at `.agent/ontology.yml`. The product concept moves to
`.agent/product/`. The resource backing paths, the cockpit engine paths, and the
watcher path mapping all re-point to `.agent/`.

A read prefers the `.agent/` path and falls back to a legacy `specs/` file when
the `.agent/` file is absent. The fallback preserves every unknown field and the
`bigpowers_version` value. A runtime write always targets `.agent/`, so a legacy
`specs/` file is never mutated.

## Consequences

Machine-written state lives in the controlled workspace, consistent with the
split. An existing bigpowers cockpit under `specs/` keeps working through the
backward-compatible read. The `truenorth://ontology` resource creates its backing
file under `.agent/` on first read.
