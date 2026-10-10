# Work item 516: reconcile release and planning contracts

**task_id:** 516
**type:** fix
**risk:** P1
**context:** infra
**issue:** https://github.com/vixygrey/truenorth-mcp/issues/516
**group_id:** 515
**status:** planned; implementation has not started

## Context

This work item implements step 2 of the approved compliance plan. The source manifest is group.yml, which reserves this specification and 516-tasks.yml. The prior impact assessment is impact.md. Dependencies: completed planning bootstrap and a green issue-linked implementation baseline.

## Requirements

#### MODIFIED: MC-01

**Before:** release-branch reads quality_gates.coverage at the top level, while the active version 2 configuration places these thresholds under advisory.quality_gates.coverage.

**After:** Release guidance resolves advisory.quality_gates.coverage from the active version 2 configuration without claiming runtime enforcement.

#### MODIFIED: MC-02

**Before:** release-branch rejects an AI-attribution footer, while CONVENTIONS.md permits a standard Co-authored-by trailer.

**After:** Commit guidance respects the authoritative CONVENTIONS.md policy on Co-authored-by trailers.

#### MODIFIED: MC-03

**Before:** The task tool accepts group_id/group_kind/task_name/verify_command, while downstream planning instructions require declared work-item identity and reserved capsule filenames without fully explaining their supported mapping.

**After:** An actual issue recorded through the supported task schema resolves to exactly one reserved specification and ledger, without adding unsupported tool arguments.

#### MODIFIED: MC-04

**Before:** Affected distributed skill changes require synchronized source/package contents and regenerated current metadata; published history must remain intact.

**After:** Changed source skills, packaged skills, catalog ownership, and bundle metadata remain coherent.

## Steps

1. Reconcile the v2 advisory coverage path and permitted commit trailers with authoritative project conventions.
   - Verify: `TRUENORTH_VERIFY_CMD='npm run preflight' cargo run --locked --quiet --manifest-path runtime/Cargo.toml --bin truenorth-mcp -- --check-config --strict && bash skills/release-branch/scripts/check-route.sh --mode team-pr --action inspect`
   - Required observable smoke: Follow corrected release preflight with current v2 policy and a permitted Co-authored-by trailer; record threshold resolution and the operator verdict.
   - Scenarios: SC-516-P1-01

2. Document and exercise supported issue identity, ticket-group indexing, reserved filenames, and missing-artifact owner routing.
   - Verify: `node scripts/run-methodology-evals.js --mode deterministic --check-baseline`
   - Required observable smoke: Resolve a real recorded task through group/spec/ledger readers; demonstrate owner routing for a missing prerequisite in a disposable capsule.
   - Scenarios: SC-516-P1-02

3. Update affected packaged skills and consumers, regenerate bundle metadata with the existing generator, and verify package delivery.
   - Verify: `node --test scripts/build-bundle-manifest.test.js && node scripts/build-bundle-manifest.js --check && bash scripts/preflight.sh wrapper shell`
   - Required observable smoke: Read the changed affected skill through the packaged consumer and retain the coherent bundle/catalog and wrapper outcomes.
   - Scenarios: SC-516-P1-03

## Verification script

1. Follow the corrected release preflight, resolve both coverage thresholds and evaluate the message without attempting integration.
   - Expected: Resolve 80/95 from advisory.quality_gates.coverage; explain advisory versus runtime enforcement; do not reject the permitted trailer.
   - Record: Record the actual operator walkthrough plus diagnostic/route outputs; commands alone do not prove the corrected prose was followed.
   - Scenario: SC-516-P1-01

2. Read the recorded task, group manifest, specification, and ledger through the corrected planning guidance. Then remove a prerequisite in the disposable copy.
   - Expected: Resolve exactly one work item without unsupported arguments; missing prerequisites route to their owning skill rather than being silently synthesized.
   - Record: Retain resolved issue/path identities and the observed missing-artifact stop; deterministic scenarios are supporting runtime evidence, not proof of this prose walkthrough.
   - Scenario: SC-516-P1-02

3. Run bundle validation, wrapper verification, and scripted-skill verification against the actual package inputs.
   - Expected: Package consumers resolve the changed skill catalog consistently; no stale source/package digest remains.
   - Record: Retain command outcomes and an actual packaged-skill read for the affected skill.
   - Scenario: SC-516-P1-03

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

Related completed work: #432 (v2 policy namespace) and #450 (planning artifact ownership). This work addresses their current consumer-contract gaps, not a runtime schema redesign.

Shared consumer blast radius is documented in impact.md. Retain explicit gaps and failure outcomes; neither catalog validation, formatting, diagnostics, nor deterministic fixtures alone certify a complete prose workflow.
