# ADR-0020: Prospective Traceability Evidence Cutover at v1.0.4

**Status:** Accepted
**Date:** 2026-10-10
**Approved by:** vixygrey (repository owner)
**Related:** ADR-0019

## Context

`trace-requirement` enforces that every active or done release-plan entry carries
real implementation, test, and verification evidence, including a native
`truenorth_verify_gate` receipt. Versioned, bounded operation receipts did not
exist before v1.0.4 (commit `692edb9880866c7fb02e73fc66fbbea959a85544`): no runtime
build predating that release could emit one.

Track 2 of the historical-evidence remediation plan independently re-verified the
already-recovered issue, pull-request, commit, and CI evidence for all 77
pre-v1.0.4 release-plan entries (`release-plan:001` through `release-plan:077`)
against live GitHub data and found zero discrepancies with the original recovery
performed in #519. The remaining `FAIL` gate verdict for those entries is
therefore a structural property of when the evidence-capture tooling shipped, not
a missed or incomplete recovery effort. No further mechanical work can produce a
native receipt for work completed before the tooling existed.

## Decision

The traceability gate's evidence requirement applies prospectively, from v1.0.4
(commit `692edb9880866c7fb02e73fc66fbbea959a85544`) forward.

Every release-plan entry at or before that commit (`release-plan:001` through
`release-plan:077`) is disposed `out_of_scope` in `.agent/tasks/traceability.yml`,
with:

- `owner: vixygrey`
- `rationale`: a reference to this decision and the structural cutover reason
- scope: exactly `release-plan:001` through `release-plan:077`, no other rows

This is a single batch decision with one shared rationale, not 77 individual
judgment calls. It does not retroactively certify that pre-cutover work was
correct, tested, or free of defects; it states only that the traceability gate no
longer requires evidence that could never have existed for that work.

The already-recovered association evidence (issue state, merged PR, commit SHA,
CI check-run conclusions, release records) for those 77 entries is preserved in
the traceability report and is not deleted. The disposition changes only the gate
verdict's treatment of missing native-receipt and behavior-smoke requirements for
rows at or before the cutover; it does not alter or invalidate the underlying
external evidence already on file.

Entries `release-plan:078` through `release-plan:082` (#516–#520, all at or after
v1.0.4) remain `active` disposition and are unaffected: they already carry real
native receipts and are not covered by this cutover.

## Consequences

The traceability gate can reach a legitimate non-`FAIL` verdict without
fabricating historical evidence or silently excluding work the owner did not
actually review. The cutover boundary is visible, dated, and attributed, so a
future reader can see exactly when and why pre-receipt history stopped being
held to a standard it could not meet.

A new historical entry discovered after this decision that genuinely predates
v1.0.4 inherits the same disposition automatically by falling before the cutover
commit; no new entry can be added at or before that commit going forward, since
development has moved past it.

This decision does not waive, defer, or exempt any future entry. Every
release-plan entry recorded from v1.0.4 forward remains subject to the full
evidence requirement with no exception.
