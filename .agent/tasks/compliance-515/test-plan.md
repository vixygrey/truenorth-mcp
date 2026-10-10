# Test design: ticket 515, TrueNorth compliance remediation

## Scope

- Profile: issue-per-task.
- Declared work items: 516, 517, 518, 519, 520.
- Source manifest: group.yml; scenario owners below are copied from its actual issue ids.
- Exclusions: implementation during bootstrap, fabricated history, optional ontology, retired telemetry, automatic integration/publication, and runtime redesign.
- All scenarios below are planned, not executed. Supporting suite commands do not substitute for the required observable smoke. No implementation test code is created by this test plan.

## Scenarios

### SC-516-P1-01: Version 2 release policy and valid commit message resolve

- Owner: 516
- Risk: P1
- Requirements: MC-01, MC-02
- Level: operator end-to-end
- Setup: Use the current version 2 rules and a Conventional Commit with a standard Co-authored-by trailer allowed by CONVENTIONS.md.
- Action: Follow the corrected release preflight, resolve both coverage thresholds and evaluate the message without attempting integration.
- Expected: Resolve 80/95 from advisory.quality_gates.coverage; explain advisory versus runtime enforcement; do not reject the permitted trailer.
- Verify: `TRUENORTH_VERIFY_CMD='npm run preflight' cargo run --locked --quiet --manifest-path runtime/Cargo.toml --bin truenorth-mcp -- --check-config --strict && bash skills/release-branch/scripts/check-route.sh --mode team-pr --action inspect`
- Evidence: Record the actual operator walkthrough plus diagnostic/route outputs; commands alone do not prove the corrected prose was followed.

### SC-516-P1-02: Recorded issue resolves to its reserved planning artifacts

- Owner: 516
- Risk: P1
- Requirements: MC-03
- Level: operator end-to-end
- Setup: Use a real task recorded with the supported schema and its reserved manifest filenames; use a disposable copy for a missing-artifact case.
- Action: Read the recorded task, group manifest, specification, and ledger through the corrected planning guidance. Then remove a prerequisite in the disposable copy.
- Expected: Resolve exactly one work item without unsupported arguments; missing prerequisites route to their owning skill rather than being silently synthesized.
- Verify: `node scripts/run-methodology-evals.js --mode deterministic --check-baseline`
- Evidence: Retain resolved issue/path identities and the observed missing-artifact stop; deterministic scenarios are supporting runtime evidence, not proof of this prose walkthrough.

### SC-516-P1-03: Consumers receive coherent packaged skill changes

- Owner: 516
- Risk: P1
- Requirements: MC-04
- Level: integration
- Setup: Regenerate current bundle metadata after updating the affected source and packaged skills.
- Action: Run bundle validation, wrapper verification, and scripted-skill verification against the actual package inputs.
- Expected: Package consumers resolve the changed skill catalog consistently; no stale source/package digest remains.
- Verify: `node --test scripts/build-bundle-manifest.test.js && node scripts/build-bundle-manifest.js --check && bash scripts/preflight.sh wrapper shell`
- Evidence: Retain command outcomes and an actual packaged-skill read for the affected skill.

### SC-517-P3-01: Normative references resolve with reviewed provenance

- Owner: 517
- Risk: P3
- Requirements: MC-05, MC-06, MC-07
- Level: document/resource walkthrough
- Setup: Recovered history or explicitly labeled current reconstruction; reviewed writing policy and precedence.
- Action: Open every active normative reference through the intended reader and review content/provenance with the human owner.
- Expected: No absent target or authority contradiction; reconstructed content is never represented as an original.
- Verify: `bash scripts/preflight.sh format`
- Evidence: Record each resolved target and the human content/provenance review; formatting is supporting evidence only.

### SC-518-P1-01: Mounted server executes the actual verification command

- Owner: 518
- Risk: P1
- Requirements: MC-08
- Level: MCP end-to-end
- Setup: Configure the intended verify command in the actual client environment and restart the client.
- Action: Run strict diagnostics, then invoke mounted truenorth_verify_gate for the actual lifecycle phase and retain its result.
- Expected: The mounted server runs the intended command and returns an observed successful gate receipt, or honestly reports its failure.
- Verify: `TRUENORTH_VERIFY_CMD='npm run preflight' cargo run --locked --quiet --manifest-path runtime/Cargo.toml --bin truenorth-mcp -- --check-config --strict`
- Evidence: A process-local diagnostic setting is not client configuration or gate proof; retain the actual mounted-tool receipt.

### SC-518-P1-02: Coverage uses declared metric and source sets

- Owner: 518
- Risk: P1
- Requirements: MC-09, MC-11
- Level: coverage integration
- Setup: First record the approved pinned tool, metric, exact command, overall source set, and business-logic source set in the work item.
- Action: Run the chosen coverage command on actual tests and compare measured values with the configured policy through the adopted workflow.
- Expected: Measured overall coverage is at least 80 percent and business-logic coverage at least 95 percent; no threshold lowering or unapproved dependency.
- Verify: `npm run preflight`
- Evidence: The coverage-specific command is a pre-implementation planning gate owned by #518; retain the raw report, metric definitions, percentages, and evaluator result.

### SC-518-P1-03: Invalid or insufficient coverage cannot pass

- Owner: 518
- Risk: P1
- Requirements: MC-10
- Level: coverage evaluator boundary
- Setup: Isolated reports with a missing file, malformed data, wrong source/head, and values below each threshold.
- Action: Exercise the actual selected coverage evaluator after its command is finalized; do not mock an echoed pass.
- Expected: Each invalid or insufficient case fails with a field/report-specific explanation; valid threshold boundaries behave as documented.
- Verify: `npm run preflight`
- Evidence: Retain evaluator exit codes and diagnostics from disposable fixtures; this scenario is not completed by preflight alone.

### SC-519-P1-01: Current execution evidence records only observed work

- Owner: 519
- Risk: P1
- Requirements: MC-12, MC-14
- Level: artifact/MCP integration
- Setup: Actual execution results, real timestamps, scoped behavior-smoke outputs, and gate receipts from the remediation work.
- Action: Update execution-owned status only after required verification; retain evidence in the owning capsules.
- Expected: Status and timestamps match observed execution; planned checks and local RED evidence are not invented as historical facts.
- Verify: `npm run preflight && node scripts/run-methodology-evals.js --mode deterministic --check-baseline`
- Evidence: Inspect the actual status/evidence through the runtime resources and retained receipts.

### SC-519-P1-02: Historical traceability accounts for every entry honestly

- Owner: 519
- Risk: P1
- Requirements: MC-13, MC-14
- Level: historical evidence walkthrough
- Setup: Every existing release-plan entry plus reachable issue, PR, commit, CI, and release records.
- Action: Build report rows once per entry, link recoverable evidence, and record real gaps and legitimate owner/rationale dispositions.
- Expected: No duplicate or omitted entry; missing receipts and timestamps stay missing; no blanket exclusion to force PASS.
- Verify: `npm run preflight && node scripts/run-methodology-evals.js --mode deterministic --check-baseline`
- Evidence: Retain a complete entry-to-row inventory and source URLs or explicit unavailable-evidence notes.

### SC-519-P1-03: Traceability gates reject stale or incomplete reports

- Owner: 519
- Risk: P1
- Requirements: MC-15
- Level: release gate boundary
- Setup: Current report plus disposable stale-head, unsupported-schema, and missing-required-evidence variants.
- Action: Run trace-requirement gate against the exact report generated by report mode; never mutate the real report for negative cases.
- Expected: Only supported current complete passing evidence can pass; failure and missing evidence cannot become an automatic waiver.
- Verify: `npm run preflight && node scripts/run-methodology-evals.js --mode deterministic --check-baseline`
- Evidence: Retain report identity, evaluated head, verdict, and observed negative-case decisions.

### SC-520-P0-01: The real remediation follows the complete governed workflow

- Owner: 520
- Risk: P0
- Requirements: MC-16
- Level: operator/MCP end-to-end
- Setup: Actual issues, ledgers, approved scope, implementation branches, green baselines, and applicable behavioral changes.
- Action: Observe issue-first planning, applicable TDD, real verification, audits/review, current-head traceability, and required CI checks.
- Expected: Every transition has evidence; no implementation starts with unresolved readiness gates or failed baseline.
- Verify: `npm run preflight && node scripts/run-methodology-evals.js --mode deterministic --check-baseline`
- Evidence: Record actual issue/branch/task identities, outputs, receipts, and the evaluated commit. Deterministic fixtures are supporting runtime checks only.

### SC-520-P0-02: Remote integration enforcement is observed

- Owner: 520
- Risk: P0
- Requirements: MC-17
- Level: GitHub integration
- Setup: Authenticated read access to the actual repository settings, rulesets, branch protection, and workflow runs.
- Action: Query remote controls and inspect required checks and the configured squash/immutable-release policies without changing settings.
- Expected: Required controls are observed as enabled; missing or inaccessible settings block an unqualified compliance verdict.
- Verify: `gh api repos/vixygrey/truenorth-mcp/rulesets && gh api repos/vixygrey/truenorth-mcp/branches/main/protection`
- Evidence: Retain the relevant redacted configuration and run identities; a 404/403 is not PASS and rulesets/protection must be interpreted together.

### SC-520-P0-03: Irreversible integration requires separate approval

- Owner: 520
- Risk: P0
- Requirements: MC-18, MC-19
- Level: integration boundary
- Setup: Current-head passing evidence and explicit approval for the particular action; disposable fixture for rejected-action cases.
- Action: Verify route inspection does not authorize merge/tag/publish/cleanup. Only after separate approval observe the actual squash merge and post-merge checks; prepare any distribution change under a new version.
- Expected: No irreversible action without its approval; published 1.0.4 assets remain unchanged; landed commit and required checks are observed.
- Verify: `bash skills/release-branch/scripts/check-route.sh --mode team-pr --action inspect`
- Evidence: Retain approval scope, observed guard outcomes, landed commit, and workflow run links; inspect output alone never authorizes integration.

## Coverage matrix

| Requirements | Task id | Scenario id | Risk |
| --- | --- | --- | --- |
| MC-01, MC-02 | 516 | SC-516-P1-01 | P1 |
| MC-03 | 516 | SC-516-P1-02 | P1 |
| MC-04 | 516 | SC-516-P1-03 | P1 |
| MC-05, MC-06, MC-07 | 517 | SC-517-P3-01 | P3 |
| MC-08 | 518 | SC-518-P1-01 | P1 |
| MC-09, MC-11 | 518 | SC-518-P1-02 | P1 |
| MC-10 | 518 | SC-518-P1-03 | P1 |
| MC-12, MC-14 | 519 | SC-519-P1-01 | P1 |
| MC-13, MC-14 | 519 | SC-519-P1-02 | P1 |
| MC-15 | 519 | SC-519-P1-03 | P1 |
| MC-16 | 520 | SC-520-P0-01 | P0 |
| MC-17 | 520 | SC-520-P0-02 | P0 |
| MC-18, MC-19 | 520 | SC-520-P0-03 | P0 |

## Fixtures and boundaries

Use disposable directories/worktrees for negative cases and separate mutating MCP writers. Do not corrupt current cockpit state to demonstrate a failure. Preserve inherited secret values and never put credentials in evidence. Finalize the coverage tool/metric/source sets/exact command in #518 before implementing coverage behavior. Risk P0 external-control scenarios require independent review and explicit irreversible-action approval.

## Deferred coverage

No required P0 or P1 behavior is silently deferred. Model-mode methodology evaluations are optional and non-gating under evals/methodology/README.md. They do not replace actual operator execution of the prose workflow. Historical evidence that cannot be recovered remains an explicit gap owned by #519.
