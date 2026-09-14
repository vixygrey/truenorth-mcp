# ADR-0008: The `.agent/` and `specs/` Split

**Status:** Accepted
**Date:** 2026-09-13
**Supersedes:** ADR-0002 (local-first specs)

## Context

The bigpowers layout mixed machine-written cockpit state and human-authored
narrative under one `specs/` directory. An agent read and wrote the same tree a
human edited. The boundary between what the runtime owns and what the human owns
was unclear, so a runtime write could touch a human document.

## Decision

Split the repository into two layers. The `.agent/` directory holds the
machine-facing workspace that the runtime reads, watches, and writes. The
`specs/` directory holds human-authored narrative, for example ADRs. The runtime
writes only under `.agent/`. The runtime can read a file under `specs/`, but it
never mutates a path under `specs/`.

A single write guard, `engine::agent_ws::write_under_agent`, funnels every
runtime write and rejects any target outside `.agent/`. This is the load-bearing
invariant of the split.

## Consequences

The runtime and the human own separate layers, so a runtime write cannot corrupt
a human document. The write guard makes the invariant testable once and hard to
bypass. ADR-0002 assumed a single local-first `specs/` cockpit; this decision
replaces that assumption. Existing bigpowers cockpits under `specs/` still read
through a backward-compatible fallback.
