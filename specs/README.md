# Specification authority and provenance

These documents describe the prepared 1.0.5 contract.
The user approved the 1.0.4 content and provenance for work item #517.
[The human review record](../.agent/tasks/compliance-515/517-human-review.json) identifies that historical review scope and its explicit exclusions.

- [Requirements](./requirements.md) define the current observable contract.
- [Design](./design.md) describes ownership, execution boundaries, and property identities.
- [Tasks](./tasks.md) identifies current work ownership and verification evidence.
- [Project conventions](../CONVENTIONS.md) define writing policy and document precedence.
- [Accepted decisions](adr/) record the architecture decisions that changed the historical baseline.

## Recovered originals

The following files are authentic Git originals, not reconstructed historical text.
They come from revision `ad34e41974a58952da2ce4a6bf17042fc14b7920`.
Commit `d6ebac734c3624efec775ce1ba27dc72b6ff2e62` removed their former `.kiro/specs/` paths from tracking.
Recovery verified every destination against its original Git blob before the write.
[The provenance record](../.agent/tasks/compliance-515/517-recovery-provenance.json) retains source paths, hashes, and review status.

| Historical feature | Requirements                                                 | Design                                                 | Tasks                                                 |
| ------------------ | ------------------------------------------------------------ | ------------------------------------------------------ | ----------------------------------------------------- |
| Runtime refactor   | [Original](archive/truenorth-mcp-refactor/requirements.md)   | [Original](archive/truenorth-mcp-refactor/design.md)   | [Original](archive/truenorth-mcp-refactor/tasks.md)   |
| Workspace profiles | [Original](archive/agent-workspace-profiles/requirements.md) | [Original](archive/agent-workspace-profiles/design.md) | [Original](archive/agent-workspace-profiles/tasks.md) |
| Optional ontology  | [Original](archive/optional-ontology/requirements.md)        | [Original](archive/optional-ontology/design.md)        | [Original](archive/optional-ontology/tasks.md)        |

The archive preserves historical wording, paths, and checked boxes without correction.
It is not current authority, current task status, or evidence that a present check passed.
Retired Jev material is not restored as active product scope.

## Current derivation

The current documents are newly authored syntheses of those originals and later accepted decisions.
They are not represented as recovered originals.
ADRs 0008 through 0019 establish the current architecture decisions.
ADR-0019 narrows ADR-0008's blanket write-confinement clause to match the audited repository-seed paths.
The version 2 rules and the current bounded executor replace older enforcement and containment claims.
The current documents state those differences explicitly.

[The #520 authority record](../.agent/tasks/compliance-515/520-authority-evidence.json) retains the approval and scoped interpretation for ADR-0019.
It does not certify runtime behavior, release compliance, or a whole-repository invariant.
The retained 1.0.4 observations remain historical release evidence, not verification of the prepared 1.0.5 contract.

The #517 human review covered the recovered provenance and its recorded 1.0.4 contract.
It does not replace retained execution and reader evidence or review of the prepared 1.0.5 change.
