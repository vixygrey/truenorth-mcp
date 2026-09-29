---
name: evolve-skill
description: "Benchmark-gated skill evolution. Invoke run-benchmark before and after a focused skill change, reject unavailable evidence or validation regression, and record the decision. Use it when a reviewed benchmark exposes a systemic skill gap."
kind: prose
---

# Evolve Skill

> **HARD GATE**: no skill change ships without a `run-benchmark` validation
> result at or above the recorded pre-change baseline. Missing evidence blocks.

## Loop

Run `run-benchmark` before the edit to establish a baseline and after the edit to
prove the validation delta did not regress.

1. **Mechanical gate**: run project verification through
   `truenorth_verify_gate`. Fix a failure before measuring capability.
2. **Resolve the definition**: locate the reviewed versioned JSON definition for
   the selected skill. Invoke `run-benchmark` with `--update-baseline` before
   editing. Save the definition, baseline, and report paths in
   `.agent/tasks/state.yml`.
3. **Handle unavailable evidence**: exit 2 or
   `TRUENORTH_BENCHMARK_UNAVAILABLE` means `UNAVAILABLE`. Stop until a reviewed
   definition, skill file, or baseline exists. Never invent a score, silently
   skip, or treat absence as a baseline.
4. **Identify the gap**: after `run-benchmark` establishes the baseline, inspect
   failed runs and low train or validation deltas. Validation is authoritative.
   Train results guide iteration only.
5. **Plan and edit**: use `plan-work`, then `craft-skill` or a direct `SKILL.md`
   edit. Target the measured gap without changing held-out validation fixtures.
6. **Re-run**: invoke `run-benchmark` with `--check-baseline`.
   - Exit 0 with stable or improved validation delta: continue.
   - Regression, malformed evidence, or failed required checks: revert the skill
     change and return to step 4.
   - Unavailable evidence: stop as described in step 3.
7. **Record the decision**: write an ADR with definition and skill digests,
   baseline and current report paths, and before-and-after validation deltas.
   Update `session-state`.

## Verify

Confirm both reports use schema version 1, the post-change report matches the
selected skill and definition, and `run-benchmark --check-baseline` exits 0.

See [REFERENCE.md](REFERENCE.md) for the ADR template.

<!-- story: e31s07 -->
