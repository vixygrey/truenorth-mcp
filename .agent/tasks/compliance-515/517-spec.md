# Work item 517: restore specification and writing authority

**task_id:** 517
**type:** refactor
**risk:** P3
**context:** infra
**issue:** https://github.com/vixygrey/truenorth-mcp/issues/517
**group_id:** 515
**status:** planned; implementation has not started

## Context

This work item implements step 3 of the approved compliance plan. The source manifest is group.yml, which reserves this specification and 517-tasks.yml. The prior impact assessment is impact.md. Dependencies: #516.

## Requirements

#### MODIFIED: MC-05

**Before:** The requirements entry point names three absent .kiro/specs directories.

**After:** Every active requirements/design/task reference resolves to reviewed authoritative content with explicit provenance.

#### MODIFIED: MC-06

**Before:** CONVENTIONS.md, CONTRIBUTING.md, and Kiro authority refer to absent writing-rules/styleguide documents.

**After:** Writing policy and document precedence have one authoritative definition and no dangling normative references.

#### MODIFIED: MC-07

**Before:** Historical source narratives are not present in the current checkout; any recovered or reconstructed replacement needs explicit provenance.

**After:** Unavailable historical specifications are reconstructed as current reviewed specifications, not represented as recovered originals.

## Steps

1. Recover normative requirement/design/task narratives from reachable history or prepare explicitly labeled current reconstruction for human review.
   - Verify: `bash scripts/preflight.sh format`
   - Required observable smoke: Open recovered/reconstructed content through the intended reader; retain provenance and actual human review rather than claiming formatting establishes content accuracy.
   - Scenarios: SC-517-P3-01

2. Consolidate writing policy into the authoritative project convention and update precedence and all affected normative reference consumers.
   - Verify: `bash scripts/preflight.sh format`
   - Required observable smoke: Open every active local normative reference and demonstrate one consistent writing/document-precedence rule with the human owner.
   - Scenarios: SC-517-P3-01

## Verification script

1. Open every active normative reference through the intended reader and review content/provenance with the human owner.
   - Expected: No absent target or authority contradiction; reconstructed content is never represented as an original.
   - Record: Record each resolved target and the human content/provenance review; formatting is supporting evidence only.
   - Scenario: SC-517-P3-01

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

Search reachable history before reconstruction. Human-authored narrative remains in specs/; runtime and skill writes remain under .agent/. Do not create empty files or aliases merely to satisfy stale references.

Shared consumer blast radius is documented in impact.md. Retain explicit gaps and failure outcomes; neither catalog validation, formatting, diagnostics, nor deterministic fixtures alone certify a complete prose workflow.
