# Work item 518: establish real verification and coverage gates

**task_id:** 518
**type:** feat
**risk:** P1
**context:** infra
**issue:** https://github.com/vixygrey/truenorth-mcp/issues/518
**group_id:** 515
**status:** planned; implementation has not started

## Context

This work item implements step 4 of the approved compliance plan. The source manifest is group.yml, which reserves this specification and 518-tasks.yml. The prior impact assessment is impact.md. Dependencies: #516.

## Requirements

#### ADDED: MC-08

The actual mounted server runs npm run preflight through truenorth_verify_gate and returns a real observed receipt.

#### ADDED: MC-09

A reproducible coverage command measures explicitly defined overall and business-logic source sets against 80% and 95% thresholds.

#### ADDED: MC-10

Missing, stale, malformed, or below-threshold coverage reports fail the adopted release check.

#### ADDED: MC-11

Coverage execution is part of the existing verification workflow; any new dependency requires explicit approval.

## Steps

1. Resolve and record the coverage metric, overall/business-logic source sets, pinned tool, exact measurement command, and any dependency approval before evaluator implementation.
   - Verify: `TRUENORTH_VERIFY_CMD='npm run preflight' cargo run --locked --quiet --manifest-path runtime/Cargo.toml --bin truenorth-mcp -- --check-config --strict`
   - Required observable smoke: Review the declared coverage contract and approval; run the chosen command on actual tests to establish a reproducible report before adopting the evaluator.
   - Scenarios: SC-518-P1-02

2. Configure the actual MCP client verify command, restart it, and run the mounted gate on the real lifecycle phase.
   - Verify: `npm run preflight`
   - Required observable smoke: Retain the mounted truenorth_verify_gate result and show that it ran the actual command; process-local diagnostics alone are not acceptance.
   - Scenarios: SC-518-P1-01

3. Integrate the approved coverage measurement/evaluator into verification and prove threshold and report-error behavior.
   - Verify: `npm run preflight`
   - Required observable smoke: Run the finalized coverage-specific command and actual evaluator on the real report plus isolated missing/malformed/stale/below-threshold cases. Retain percentages and failure exit codes.
   - Scenarios: SC-518-P1-02, SC-518-P1-03

## Verification script

1. Run strict diagnostics, then invoke mounted truenorth_verify_gate for the actual lifecycle phase and retain its result.
   - Expected: The mounted server runs the intended command and returns an observed successful gate receipt, or honestly reports its failure.
   - Record: A process-local diagnostic setting is not client configuration or gate proof; retain the actual mounted-tool receipt.
   - Scenario: SC-518-P1-01

2. Run the chosen coverage command on actual tests and compare measured values with the configured policy through the adopted workflow.
   - Expected: Measured overall coverage is at least 80 percent and business-logic coverage at least 95 percent; no threshold lowering or unapproved dependency.
   - Record: The coverage-specific command is a pre-implementation planning gate owned by #518; retain the raw report, metric definitions, percentages, and evaluator result.
   - Scenario: SC-518-P1-02

3. Exercise the actual selected coverage evaluator after its command is finalized; do not mock an echoed pass.
   - Expected: Each invalid or insufficient case fails with a field/report-specific explanation; valid threshold boundaries behave as documented.
   - Record: Retain evaluator exit codes and diagnostics from disposable fixtures; this scenario is not completed by preflight alone.
   - Scenario: SC-518-P1-03

A supporting command cannot mark a task passing without its required observable smoke and durable evidence. All verification in this work item is planned, not completed.

## Readiness and approval gates

- Dependencies must pass their required verification before this work item executes.
- kickoff-branch must establish the issue-linked implementation branch and green baseline before code changes.
- Resolve scenario-specific planning decisions before the affected implementation step; #518 owns its coverage command/source sets/metric and dependency approval.
- No implementation on main. No commit unless requested. No merge/tag/publication/cleanup without its separate approval.
- Risk-required security/audit/independent review must match the final diff and evaluated head.

## Out of scope

- Runtime redesign, new telemetry, ontology enablement, or restoring retired placeholders.
- Modifying another planning owner's artifact instead of routing a correction.
- Fabricating historical evidence, automatic waivers for missing evidence, or replacement of published 1.0.4 assets.

## Risks and early detection

The exact coverage tool, metric, source mapping, and runnable coverage command must be resolved and recorded before implementing the evaluator. Do not lower thresholds, substitute a mock echo, or claim a process-local diagnostic setting configures the MCP client.

Shared consumer blast radius is documented in impact.md. Retain explicit gaps and failure outcomes; neither catalog validation, formatting, diagnostics, nor deterministic fixtures alone certify a complete prose workflow.
