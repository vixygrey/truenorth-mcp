# Work item 520: prove compliant workflow and GitHub controls

**task_id:** 520
**type:** feat
**risk:** P0
**context:** infra
**issue:** https://github.com/vixygrey/truenorth-mcp/issues/520
**group_id:** 515
**status:** planned; implementation has not started

## Context

This work item implements step 6 of the approved compliance plan. The source manifest is group.yml, which reserves this specification and 520-tasks.yml. The prior impact assessment is impact.md. Dependencies: #519.

## Requirements

#### ADDED: MC-16

The real remediation workflow demonstrates issue-first planning, green baseline, applicable TDD, observable verification, review, and current-head traceability.

#### ADDED: MC-17

Protected-main rules, required checks, direct-push restrictions, squash merging, and immutable release settings are observed remotely.

#### ADDED: MC-18

Merge, tag, publication, dependency additions, and destructive cleanup require their separate approvals.

#### ADDED: MC-19

No published 1.0.4 asset is replaced; distributed skill changes are prepared for a new version before publication.

## Steps

1. Observe the real remediation issue/branch/baseline/TDD/verification/review/traceability workflow and all required checks.
   - Verify: `npm run preflight && node scripts/run-methodology-evals.js --mode deterministic --check-baseline`
   - Required observable smoke: Retain actual operator and MCP evidence for the changed path; deterministic fixture scenarios do not certify prose-skill execution.
   - Scenarios: SC-520-P0-01

2. Inspect actual GitHub protection/ruleset settings, required checks, direct-push restrictions, squash policy, and immutable-release controls.
   - Verify: `gh api repos/vixygrey/truenorth-mcp/rulesets && gh api repos/vixygrey/truenorth-mcp/branches/main/protection`
   - Required observable smoke: Record interpreted remote control settings and required run identities. Missing or inaccessible controls remain a blocker, not an inferred pass.
   - Scenarios: SC-520-P0-02

3. After separate explicit approval, integrate the passing branch and observe the landed squash commit/post-merge checks; preserve published 1.0.4 assets.
   - Verify: `bash skills/release-branch/scripts/check-route.sh --mode team-pr --action inspect`
   - Required observable smoke: Record the particular integration approval, then actual landed commit and required workflow outcomes. A route inspection never authorizes merge, tag, publication, or cleanup.
   - Scenarios: SC-520-P0-03

## Verification script

1. Observe issue-first planning, applicable TDD, real verification, audits/review, current-head traceability, and required CI checks.
   - Expected: Every transition has evidence; no implementation starts with unresolved readiness gates or failed baseline.
   - Record: Record actual issue/branch/task identities, outputs, receipts, and the evaluated commit. Deterministic fixtures are supporting runtime checks only.
   - Scenario: SC-520-P0-01

2. Query remote controls and inspect required checks and the configured squash/immutable-release policies without changing settings.
   - Expected: Required controls are observed as enabled; missing or inaccessible settings block an unqualified compliance verdict.
   - Record: Retain the relevant redacted configuration and run identities; a 404/403 is not PASS and rulesets/protection must be interpreted together.
   - Scenario: SC-520-P0-02

3. Verify route inspection does not authorize merge/tag/publish/cleanup. Only after separate approval observe the actual squash merge and post-merge checks; prepare any distribution change under a new version.
   - Expected: No irreversible action without its approval; published 1.0.4 assets remain unchanged; landed commit and required checks are observed.
   - Record: Retain approval scope, observed guard outcomes, landed commit, and workflow run links; inspect output alone never authorizes integration.
   - Scenario: SC-520-P0-03

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

Related open #320 investigates additional git-side guards; this issue verifies current controls and does not duplicate that feature work. Optional model evaluations are not required by the deterministic CI contract. Publication and cleanup remain separately approved actions.

Shared consumer blast radius is documented in impact.md. Retain explicit gaps and failure outcomes; neither catalog validation, formatting, diagnostics, nor deterministic fixtures alone certify a complete prose workflow.
