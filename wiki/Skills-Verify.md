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

The risk-scaled behavior and evidence gate.

- **What it does**: reads the story risk (P0 to P3), exercises at least one
  story-specific observable behavior at every tier, runs the relevant mechanical
  gates through `truenorth_verify_gate`, and records expected versus actual
  results. Higher risk adds broader tests, UAT, security review, and P0 NFR
  evidence. Its `--simulate-user` mode follows the Verification Script from a
  fresh user perspective.
- **When to use it**: after `execute-plan` or `develop-tdd`, before `audit-code`,
  or for a mock-user or user-observable simulation.
- **Inputs**: active story tasks and spec, risk level, and project gate commands.
- **Outputs**: persisted verification evidence with a mandatory behavior smoke.
- **Hard gates**: not on `main`; every tier exercises changed behavior; at least
  one gate provides a contiguous terminal verdict. Manual confirmation is
  required only when acceptance criteria need human judgment.
- **Modes**: default risk-scaled verification, `--smoke` for a hotfix, `--cli`
  for a CLI tool, and `--simulate-user` for fresh-context gap discovery.
- **Handoff**: gate READY, next `audit-code`.

### enforce-first

Apply the F.I.R.S.T test-quality rubric to a test suite.

- **What it does**: applies the canonical F.I.R.S.T rubric from CONVENTIONS.md (Fast,
  Independent, Repeatable, Self-Validating, Timely), identifies violating tests, and fixes
  them.
- **When to use it**: when `develop-tdd` is writing tests, or when test quality needs a
  check. It is typically invoked internally by `develop-tdd`, and can run standalone.
- **Modes**: default (all five criteria), or `--quick` (Fast, Independent, Self-Validating
  only, used by `build-group` step 6).
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

- **What it does**: reads active project conventions, ranks changed files by
  churn, and reports concrete defects across correctness, security, performance,
  clarity, scope, dependencies, and test evidence. A style or size preference
  cannot block without a convention violation or demonstrated consequence.
- **When to use it**: before `request-review`, before committing, or on a
  code-quality request.
- **Modes**: default risk-scaled audit, `--quick` for eligible P2/P3 changes,
  `--gate` for a blocker-only CI verdict, and `--parallel` for isolated checks.
- **Hard gate**: each failure names its location, consequence or violated
  convention, evidence, and remediation. Pass means no concrete blocker remains.
- **Handoff**: gate READY, next `commit-message`. When it passes, suggest
  `request-review`.

### request-review

Dispatch fresh reviewer agents with clean contexts. Reviewer count and focus
follow story risk.

- **What it does**: prepares a self-contained brief and dispatches two
  independent reviewers for P0, one reviewer for P1 through P3, and an additional
  security or domain specialist when the affected boundary requires it.
- **When to use it**: after `audit-code` passes, before committing, or for an
  independent audit.
- **Inputs**: diff, feature behavior, active conventions, risk, verify command,
  and behavior evidence.
- **Outputs**: independent reports whose findings are blocking, non-blocking, or
  advisory.
- **Hard gate**: every risk-required reviewer completes, required verification
  passes, and no blocking finding remains unresolved. No percentage score is
  used. Five unsuccessful rounds require a human decision.
- **Handoff**: `respond-review` for all findings.

### respond-review

Act on reviewer feedback systematically.

- **What it does**: reads every finding, preserves or assigns its blocking,
  non-blocking, or advisory class, resolves blockers first, records every
  disposition, and runs the required suite.
- **When to use it**: after `request-review` returns a report.
- **Hard gate**: every reviewer comment must be addressed and no blocker may
  remain open. Fix it, document evidence-backed disagreement, or ask for
  clarification.
- **Handoff**: `commit-message`.

---

## The bug-fix chain

### investigate-bug

The end-to-end bug entry point: history check, RCA, fix approach, TDD plan, and tracker update.

- **What it does**: reads lean local references and prior external issue history, captures
  the problem with a security-impact assessment, and runs four ordered RCA phases:
  reproduce the failure with evidence, isolate the responsible boundary, rank hypotheses
  with falsification tests, and verify one root cause. It posts the detailed RCA and TDD
  plan to the same external issue and records or updates only the lean local reference
  through `truenorth_record_bug`.
- **When to use it**: when the user reports a bug, mentions triage, or wants to plan a fix.
- **Hard gate**: do not propose a fix, implementation change, or TDD plan until the Verify
  phase confirms one root cause with reproducible evidence.
- **Handoff**: `kickoff-branch` to create a fix branch.

### fix-bug

The bug-fix orchestrator. Sets the fix_bug flow and chains the bug-fix skills.

- **What it does**: sets `active_flow: fix_bug`, then runs the chain: `investigate-bug`
  (which owns the complete four-phase RCA), `develop-tdd`, `validate-fix`, and
  `release-branch`. It supports entry without a user-reported bug for a red baseline or CI failure.
- **When to use it**: when the user reports a defect.
- **Hard gate**: set the fix_bug flow and step before starting.
- **Handoff**: resumes from `bug_cycle.current_step`.

### validate-fix

Prove a fix works before declaring it done, and harden against recurrence.

- **What it does**: re-runs the originally failing test, the full suite, typecheck, and
  lint through `truenorth_verify_gate`, adds at least one hardening mechanism, generalizes
  the fix across the defect class, posts resolution and behavioral evidence to the same
  external issue, and updates its lean local reference through `truenorth_record_bug`.
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

- **What it does**: resolves scope, researches trust boundaries, assesses
  vulnerabilities, applies proven exclusions, and reports three sections:
  `Confirmed findings`, `Needs investigation`, and `Excluded`.
- **When to use it**: when reviewing pending changes, before `release-branch`,
  during `verify-work`, during `build-group` threat modeling, or on request.
- **Hard gate**: requires git context. Confirmed unresolved HIGH/CRITICAL
  findings block. An uncertain potentially HIGH/CRITICAL path remains visible
  under `Needs investigation` and requires explicit disposition before release.
  Confidence alone never suppresses either class.
- **Integration**: it touches `build-group`, `plan-work`, `plan-release`,
  `audit-code`, `request-review`, `investigate-bug`, `validate-fix`,
  `verify-work`, and `release-branch`.

### inspect-quality

An interactive QA session that creates external issues and lean local references.

- **What it does**: listens to the user's problem, clarifies lightly, explores the codebase
  in the background for context and domain language, decides single-issue versus breakdown,
  creates or updates the external issue, and records only its canonical id, URL, status,
  linked task or group, and tags through `truenorth_record_bug`.
- **When to use it**: to report bugs, do QA, or run a QA session.
- **Hard gate**: quality metrics must be monitored. Surface a degrading metric as a
  blocker; do not accept a regression.

### trace-requirement

Build and gate one versioned traceability report from durable project evidence.

- **Modes**: `report` writes `.agent/tasks/traceability.yml`; `gate` consumes that
  exact artifact and records PASS, CONCERNS, FAIL, or WAIVED.
- **Evidence**: release tasks, work-item ledgers, Git changes and commits, test
  scenarios and commands, and real verification receipts. Source planning-ID
  comments are optional legacy hints, never required proof.
- **When to use it**: report mode after `verify-work`; gate mode before
  `release-branch`.
- **Hard gates**: a missing, unreadable, unsupported, stale, or materially
  incomplete report fails. A failed receipt remains authoritative.
- **Waivers**: WAIVED requires an explicit approval with owner, rationale, scope,
  and timestamp. Missing evidence is not a waiver.
- **Handoff**: route evidence gaps to their planning, implementation, or
  verification owner. A passing or validly waived gate hands off to
  `release-branch`.
