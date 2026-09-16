---
name: evolve-skill
description: "Benchmark-gated skill evolution. Consume a benchmark report, propose a plan-work change, edit the skill via craft-skill, re-run the benchmark, and record an ADR. Use it when a skill underperforms on a benchmark or stocktake finds a systemic gap."
---

# Evolve Skill

> **HARD GATE**: no skill change ships without a benchmark score at or above the pre-change baseline. Learning is measured and versioned, never implicit.

## Loop

1. **Regression gate**: run the project verification through the
   `truenorth_verify_gate` tool to catch a mechanical regression before you spend
   time on the benchmark evals. When it fails, fix the regression first. It is a
   prerequisite for any capability improvement.
2. **Establish the baseline**: run the benchmark for the skill in baseline mode.
   When no definition exists, create one first. Save the report path in
   `state.yaml`. When a baseline report already exists, skip this step.
3. **Identify the gap**: read the baseline report. Find the scenarios with a FAIL
   result or a low pass-at-k. This is the measurable gap.
4. **Plan the change**: use `plan-work` to write a minimal change proposal that
   targets the failing scenarios. Include the verify commands.
5. **Edit**: use `craft-skill` or a direct SKILL.md edit.
6. **Re-run the benchmark**: compare the new pass-at-k against the baseline.
   - Improved or stable: advance to step 7.
   - Regression (the new pass-at-k is below the baseline): revert the change and
     loop back to step 3.
7. **Record the decision**: write an ADR with the before-and-after pass-at-k scores.
   Update `session-state`.

## Verify

Confirm the benchmark definition exists and the post-change score is at or above the
baseline.

See [REFERENCE.md](REFERENCE.md) for the ADR template.

<!-- story: e31s07 -->
