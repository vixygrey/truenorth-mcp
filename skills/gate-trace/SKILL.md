---
name: gate-trace
description: "Deterministic traceability quality gate. Reads the coverage matrix and blind-spot data, applies decision rules with an oracle-confidence downgrade, and emits a PASS, CONCERNS, FAIL, or WAIVED verdict. Use it before release-branch to gate a merge on traceability."
kind: prose
---

# Gate Trace

A deterministic quality gate. It combines traceability coverage and blind-spot
data into a single PASS, FAIL, CONCERNS, or WAIVED decision before a release.

## Decision rules

| Rule | Condition                                                                            | Verdict  |
| ---- | ------------------------------------------------------------------------------------ | -------- |
| R1   | Any undone story with 0 code tags                                                    | FAIL     |
| R2   | Any story done but no verify evidence                                                | CONCERNS |
| R3   | A P0 story (top WSJF quartile) with 0% coverage                                      | FAIL     |
| R4   | Overall coverage less than 60%                                                       | CONCERNS |
| R6   | A P0 story with a test plan present and zero `SC-eNNsYY-P0-*` tags in the test files | CONCERNS |
| R5   | Overall coverage 80% or more, no critical gaps, all verify passed                    | PASS     |

## Oracle confidence downgrade

TEA-inspired: when trace links rely on heuristics rather than explicit tags, the
confidence drops.

| Heuristic ratio                                                      | Downgrade                                      |
| -------------------------------------------------------------------- | ---------------------------------------------- |
| More than 50% of links from heuristics (file name or task reference) | One level (PASS to CONCERNS, CONCERNS to FAIL) |
| More than 80% of links from heuristics                               | Two levels (PASS to FAIL, CONCERNS to FAIL)    |

## Process

The gate reads two inputs: a traceability matrix and blind-spot data. Both are
project data the caller provides or maintains. This skill does not run a build
pipeline. When an input is absent, the gate emits WAIVED.

1. Read the traceability matrix and the blind-spot data for the project.
2. When either input is absent, emit WAIVED and stop. The gate cannot evaluate
   without its inputs.
3. Apply the decision rules R1 to R6 in order. The first match wins.
4. Apply the oracle-confidence downgrade from the matrix `oracle_stats`
   heuristic ratio.
5. When drift data is present and shows suspect links, set the verdict to
   CONCERNS. Add the note "Drift detected: some implementing files are newer than
   their specs."
6. Attempt to refute a PASS before you emit it. State at least one concrete
   traceability gap that would block a merge if it were real. When the gap is
   real, downgrade the verdict. Every PASS must survive one refutation attempt.
7. Run an adversarial gap-finding pass. Classify each finding as BLOCKER,
   WARNING, or FILLED. A BLOCKER overrides any PASS and forces FAIL. Add the
   summary to the rationale.
8. Record the gate-trace result in the project status.

To verify the outcome, run the project verify command through the
`truenorth_verify_gate` tool. Do not shell out to a repository script.

## Verdict semantics

| Verdict  | Meaning                          | Action                                              |
| -------- | -------------------------------- | --------------------------------------------------- |
| PASS     | Every gate is satisfied          | Proceed with the merge                              |
| CONCERNS | Non-critical issues found        | Requires an explicit human override in `state.yaml` |
| FAIL     | A critical traceability gap      | Block the merge. Fix the gap first                  |
| WAIVED   | Cannot evaluate (missing inputs) | Skip the gate. The data is not available            |

## Output format

```yaml
gate_trace:
  verdict: PASS|CONCERNS|FAIL|WAIVED
  generated_at: "<ISO 8601>"
  rationale: "<human-readable explanation>"
  heuristic_ratio: <0.0-1.0>
  downgrade_applied: <true|false>
```

## Integration points

- `release-branch`: the pre-PR gate. A FAIL blocks the merge.
- `verify-work`: runs the blind-spot checks. Gate-trace consumes the result.

## Handoff

Gate: READY. Next: release-branch, the final step before a merge.
Writes: `state.yaml` `handoff.next_skill = release-branch`.

## Verify

Run the project verify command through the `truenorth_verify_gate` tool. A pass
returns exit 0.
