---
name: run-evals
description: 'Eval-driven development. Define capability and regression evals before building. A code grader uses a verify command, a model grader uses an explicit rubric. Log pass@k. Use it before develop-tdd on a new feature, or when measuring agent capability over runs.'
---

# Run Evals

> **HARD GATE**: define the evals before implementation. A code grader is a runnable verify command. A model grader is an explicit rubric with pass and fail criteria.

## Process

1. Name the capability under test in one sentence.
2. Write an evals document with:
   - **Capability evals** (does it do the job?).
   - **Regression evals** (did anything break?).
3. Assign a grader type per eval: `code` (a shell verify) or `model` (a rubric).
4. Assign a strictness tier per eval, with graduated promotion:

   | Tier             | Meaning                                        | Promotion rule                                              |
   | ---------------- | ---------------------------------------------- | ----------------------------------------------------------- |
   | `EXPERIMENTAL`   | A new eval, it can flake                       | Not gating                                                  |
   | `USUALLY_PASSES` | Stable in dev, 2 of 3 recent runs pass or more | Blocks the build only combined with the ALWAYS_PASSES suite |
   | `ALWAYS_PASSES`  | Zero tolerance, required for release           | Any single failure blocks the build and the merge           |

   Promote `EXPERIMENTAL` to `USUALLY_PASSES` after 3 consecutive passes. Promote
   `USUALLY_PASSES` to `ALWAYS_PASSES` after 5 consecutive passes with zero flakes
   documented in `.agent/tasks/state.yml`.

5. Run the evals. Log the results table with pass@k (for example 3 of 3 runs) and
   the tier per eval. Run a code grader through the `truenorth_verify_gate` tool.
6. Block the build until every `ALWAYS_PASSES` eval passes at the agreed k. A
   `USUALLY_PASSES` failure warns. An `EXPERIMENTAL` failure logs only.

## Artifact

Write the eval report alongside the verification evidence, keyed by story id for
traceability. See [REFERENCE.md](REFERENCE.md) for the template.

## Verify

Confirm the eval report exists for the story. Run each code grader through the
`truenorth_verify_gate` tool. A pass returns exit 0.
