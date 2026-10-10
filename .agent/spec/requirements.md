# Requirements

This is the machine-facing entry point for the feature narrative. The runtime
reads it as part of the `.agent/` layout contract. Human-authored current
requirements, design, and task narratives live under `specs/`.

## Current specifications

- [Authority, provenance, and review status](../../specs/README.md).
- [Current requirements](../../specs/requirements.md).
- [Current design and correctness properties P1 to P21](../../specs/design.md).
- [Current task and verification narrative](../../specs/tasks.md).
- [Writing policy and document precedence](../../CONVENTIONS.md).

The user approved the current content and recovery provenance for work item #517.
[The human review record](../tasks/compliance-515/517-human-review.json) retains that decision.
Formatting remains supporting evidence, not content approval or overall compliance.

## Historical provenance

The specification index links nine authentic recovered originals for the runtime
refactor, workspace profiles, and optional ontology. They come from Git revision
`ad34e41974a58952da2ce4a6bf17042fc14b7920`, not a historical reconstruction.
The archive preserves superseded paths and checked boxes without promoting them
to current behavior or verification. Later accepted ADRs and the reviewed current
contract determine applicability.

## Product scope

The product scope, vision, and glossary live under `.agent/product/`:

- `.agent/product/scope.yml`: the core value, in-scope and out-of-scope items, constraints,
  and success criteria.
- `.agent/product/vision.yml`: the product vision.
- `.agent/product/glossary.yml`: the domain vocabulary.

## Architecture decisions

The Architecture Decision Records live under `specs/adr/` and are served read-only through
the `truenorth://adr` resource. ADR-0008 through ADR-0013 cover the `.agent/` split, the
methodology profiles, the tracker-owned bug model, the cockpit relocation, and the optional
ontology feature flag.
