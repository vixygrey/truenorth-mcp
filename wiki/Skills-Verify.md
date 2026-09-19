# Skills: Verify

The Verify phase proves the built thing works before it ships. The skills group into the
review chain (self-review, then independent review, then response), the UAT and evals
gates, the bug-fix chain, security, and traceability.

For the full arc across phases, see [The skill workflow](The-skill-workflow). For the
alphabetical list, see [Skill index](Skill-index).

A note on order: "review answers is the code good, verify answers does the built thing do
what was promised". `verify-work` runs first, then the review chain.

---

## The verification gate

### verify-work

The multi-phase UAT gate. Cold-start smoke, the mechanical gates, and step-by-step manual
verification.

- **What it does**: reads the story risk (P0 to P3) and scales the rigor, runs a cold-start
  smoke, the mechanical gates (build, typecheck, lint, tests) through
  `truenorth_verify_gate`, a security scan, blind-spot and completeness checks, an NFR gate
  for P0, and step-by-step UAT, then runs a gaps-closure loop. It persists structured
  verification evidence.
- **When to use it**: after `execute-plan` or `develop-tdd`, before `audit-code`.
- **Inputs**: the active story tasks and spec, the risk level.
- **Outputs**: a persisted verification-evidence file.
- **Hard gates**: not on `main`. No story is done until manual UAT is confirmed with
  evidence. At least one mechanical gate must be a real terminal-verdict command, its
  output captured from a single run.
- **Modes**: default, `--smoke` (hotfix), `--cli` (a CLI tool with no server).
- **Handoff**: gate READY, next `audit-code`.

### enforce-first

Apply the F.I.R.S.T test-quality rubric to a test suite.

- **What it does**: applies the canonical F.I.R.S.T rubric from CONVENTIONS.md (Fast,
  Independent, Repeatable, Self-Validating, Timely), identifies violating tests, and fixes
  them.
- **When to use it**: when `develop-tdd` is writing tests, or when test quality needs a
  check. It is typically invoked internally by `develop-tdd`, and can run standalone.
- **Modes**: default (all five criteria), or `--quick` (Fast, Independent, Self-Validating
  only, used by `build-epic` step 6).
- **Hard gate**: all enforcement checks (lint, typecheck, tests, coverage) must pass. Do
  not disable a check to reach green.

### run-evals

Eval-driven development. Define capability and regression evals before building.

- **What it does**: defines evals with a code grader (a verify command) or a model grader
  (an explicit rubric), and logs pass@k. (Phase: Verify.)
- **When to use it**: before `develop-tdd` on a new feature, or when measuring agent
  capability over runs.
- **Note**: this skill is one of the verify-arc steps; see the workflow-order note below.

---

## The review chain

### audit-code

A self-review checklist the coding agent runs on its own work before dispatching a
reviewer.

- **What it does**: ranks changed files by churn, then runs a checklist across supply chain
  and security, Law of Demeter, convention compliance, scope, the Boy Scout rule, types,
  test coverage, SOLID, and code style. It names any rationalization it caught.
- **When to use it**: before `request-review`, before committing, or on a code-quality
  request.
- **Modes**: default, `--quick` (supply-chain and coverage only), `--gate`
  (non-interactive CI gating, used by `build-epic`), `--parallel` (isolated worktrees).
- **Hard gate**: the audit must check correctness, security, performance, and clarity. Do
  not skip the security review when the code touches user data, auth, or an external API.
- **Handoff**: gate READY, next `commit-message`. When it passes, suggest `request-review`.

### request-review

Dispatch fresh reviewer agents with clean context to critique the code independently.

- **What it does**: writes a self-contained brief, dispatches two blind reviewers (A and B)
  in parallel, optionally fans out dimension-specific reviewers, collects both reports,
  computes a quality score each, and applies a dual-blind AND gate.
- **When to use it**: after `audit-code` passes, before committing.
- **Inputs**: the diff, the feature description, the conventions, the verify command.
- **Outputs**: two independent review reports and an AND-gate verdict.
- **Hard gate**: a single-reviewer pass is insufficient. Both reviewers must pass
  independently (zero must-fix, 94% or more each). Max five iterations, then stop for a
  human decision.
- **Handoff**: `respond-review` for the findings.

### respond-review

Act on reviewer feedback systematically.

- **What it does**: reads every finding, categorizes each as must-fix, should-fix, or
  consider, confirms the consider items with the user, applies the fixes in order, runs the
  full suite, and reports what was applied and skipped.
- **When to use it**: after `request-review` returns a report.
- **Hard gate**: every reviewer comment must be addressed: fix it, document a disagreement,
  or ask for clarification. Do not ignore feedback and merge.
- **Handoff**: `commit-message`.

---

## The bug-fix chain

### investigate-bug

The end-to-end bug entry point: history check, RCA, fix approach, TDD plan, bug record.

- **What it does**: reads prior bug history, captures the problem with a security-impact
  assessment, runs the 4-phase RCA by delegating to `diagnose-root`, identifies the fix
  approach, designs a TDD fix plan of RED-GREEN cycles, and records the bug in the external
  tracker plus a reference in `.agent/tasks/bugs.yml`.
- **When to use it**: when the user reports a bug, mentions triage, or wants to plan a fix.
- **Hard gate**: do not proceed to the fix approach until `diagnose-root` phase 4 produces
  a verified root cause.
- **Handoff**: `kickoff-branch` to create a fix branch.

### diagnose-root

The canonical four-phase root-cause analysis engine: reproduce, isolate, hypothesize,
verify.

- **What it does**: runs the four phases in order, recording findings in the external
  tracker. It is invoked by `investigate-bug` and by `fix-bug`. It does not write the bug
  record.
- **When to use it**: when a bug is confirmed but the root cause is unclear, or after
  `investigate-bug`.
- **Hard gate**: do not propose a fix until phase 4 confirms a single root cause with
  evidence.

### fix-bug

The bug-fix orchestrator. Sets the fix_bug flow and chains the bug-fix skills.

- **What it does**: sets `active_flow: fix_bug`, then runs the chain: `investigate-bug`
  (which itself runs `diagnose-root`), `develop-tdd`, `validate-fix`, and `release-branch`.
  It supports entry without a user-reported bug for a red baseline or CI failure.
- **When to use it**: when the user reports a defect.
- **Hard gate**: set the fix_bug flow and step before starting.
- **Handoff**: resumes from `bug_cycle.current_step`.

### validate-fix

Prove a fix works before declaring it done, and harden against recurrence.

- **What it does**: re-runs the originally failing test, the full suite, typecheck, and
  lint through `truenorth_verify_gate`, adds at least one hardening mechanism, generalizes
  the fix across the defect class, updates the bug file and registry, and proves the
  behavior.
- **When to use it**: after implementing a bug fix, when the user asks "is this fixed?", or
  before closing an investigation.
- **Hard gates**: the fix must not regress. Never use a type-ignore or lint-disable to fix
  a bug. Never mark done while any test fails. Loop until the behavior is proven in a single
  run.
- **Handoff**: `audit-code`, then `commit-message`.

---

## Security and quality

### security-review

Security analysis of code changes, tracing data flow across files.

- **What it does**: runs a five-phase scan (scope resolution, context research,
  vulnerability assessment, false-positive filtering, report generation), maps each rule to
  a CWE with positive and negative fixtures, and suppresses a finding below confidence 8.
- **When to use it**: when reviewing pending changes, before `release-branch`, during
  `verify-work`, during `build-epic` threat modeling, or on request.
- **Hard gate**: requires git context. A finding below confidence 8 is suppressed.
- **Integration**: it touches nine other skills (build-epic step 0, plan-work's `security:`
  field, plan-release's WSJF boost, audit-code, request-review, investigate-bug,
  validate-fix, verify-work phase 5, release-branch). See
  [The skill workflow](The-skill-workflow).

### inspect-quality

An interactive QA session that logs conversational bug reports to the registry.

- **What it does**: listens to the user's problem, clarifies lightly, explores the codebase
  in the background for context and domain language, decides single-issue versus breakdown,
  and appends a structured entry to `.agent/tasks/bugs.yml` with a durable, no-file-paths
  format.
- **When to use it**: to report bugs, do QA, or run a QA session.
- **Hard gate**: quality metrics must be monitored. Surface a degrading metric as a
  blocker; do not accept a regression.

### trace-requirement

Link story ids to the implementing code and tests, and surface gaps both ways.

- **What it does**: extracts story ids from the release plan, searches for `story:` tags in
  code and tests, builds a matrix (implemented, tested, dark, orphan), and writes a
  traceability report with a coverage summary.
- **When to use it**: to verify coverage of a release plan, audit which stories are
  implemented, or find a dark story with no code.
- **Hard gate**: the release plan and task groups must exist. Run `plan-release` first.
- **Handoff**: `plan-work` for each dark story.

### gate-trace

A deterministic traceability quality gate that emits a single verdict.

- **What it does**: reads the coverage matrix and blind-spot data, applies decision rules
  R1 to R6 (first match wins), applies an oracle-confidence downgrade for heuristic links,
  attempts to refute a PASS, and emits PASS, CONCERNS, FAIL, or WAIVED.
- **When to use it**: before `release-branch`, to gate a merge on traceability.
- **Inputs**: the traceability matrix and blind-spot data. Missing inputs yield WAIVED.
- **Outputs**: a structured verdict recorded in the project status.
- **Handoff**: gate READY, next `release-branch`.
