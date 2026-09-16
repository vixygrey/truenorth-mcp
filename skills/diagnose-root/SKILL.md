---
name: diagnose-root
description: "Run a four-phase root-cause analysis: reproduce, isolate, hypothesize, verify. Use it when a bug is confirmed but the root cause is unclear, after investigate-bug, or when the user mentions root-cause analysis."
---

# Diagnose Root

**Boundary**: Canonical, reusable 4-phase RCA engine. Invoked by `investigate-bug` (as step 2 of the end-to-end flow) and by `fix-bug` (when no bug record exists). Does not write the bug record. The external tracker owns bug detail.

Four phases, do not skip. Record the findings for each phase in the external tracker.

## Phases

1. **Reproduce** — minimal steps; record environment; capture logs.
2. **Isolate** — narrow to module/function; binary-search commits or config.
3. **Hypothesize** — list ranked hypotheses with falsification test each.
4. **Verify** — run falsification; confirm single root cause; link to fix plan.

> **HARD GATE** — Do not propose a fix until phase 4 confirms one root cause with evidence.

## Verify

→ verify: phase 4 confirms one root cause with evidence, recorded in the external tracker.
