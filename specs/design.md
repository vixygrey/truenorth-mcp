# Current design

This document describes the prepared 1.0.5 architecture. See [provenance and review status](./README.md).
The observable contract is [requirements.md](./requirements.md).

## Components and boundaries

The Rust crate in `runtime/` owns the MCP transport, typed inputs, resources, workspace validation, and state mutations.
The Node.js wrapper in `npm/` resolves the platform executable and distributes skills. It is not a second runtime implementation.
The skill catalog, tier renderer, and dependency graph describe the declared workflow surface.
The current bundle manifest binds distributed content to the release. Supported historical manifests remain immutable release records.

The machine workspace is `.agent/`.
Its guarded mutation paths stay under `.agent/`.
Its layout, profile, configuration, specification pointers, product definitions, and cockpit have distinct ownership.
`specs/` contains human-authored narratives and ADRs. It does not contain the mutable cockpit.
ADR-0019 authorizes only the enumerated repository-seed paths outside the normal machine-state guard.
Read-only repository resources can read outside the machine workspace without granting write authority.

## State transitions and preservation

Phase advancement and task or bug recording validate typed input before a guarded write.
State updates preserve fields they do not own.
A completed write can notify resource consumers. Invalid input does not partially mutate the workspace.
A malformed resource does not terminate service for unrelated resources.
The runtime treats disk as the live source rather than an agent's remembered snapshot.

`truenorth_tdd_cycle` stores an omitted `group_id` at the legacy `tdd.step` field.
A supplied group ID stores the step at `tdd.scopes.<group_id>.step`.
The runtime does not infer scope from `active_group`, because a completed group can remain in state.
Each scoped transition preserves the unscoped step and every other group scope.

The release plan records work and verify commands.
`execution-status.yml` owns current execution progress. Planning specifications and historical task checkboxes do not.
`backlog.yml` declares ownership independently from the profile.
An external pointer delegates current backlog detail to that tracker rather than cache its issues in the cockpit.

## Profiles and grouping

ADR-0009 makes the profile responsible for vocabulary, grouping requirements, branches, and commit issue requirements.
Neutral `group_id` and `group_kind` model that choice.
The documented legacy epic input maps into this model. It does not make epics mandatory across all profiles.
Scaffolding and workflow consumers use the same profile contract.

## Verification executor

The command is trusted operator configuration, not a tool argument.
The runtime launches it through `/bin/sh -c` with bounded execution and output handling.
The command allowlist checks the leading executable, not every operation in the shell expression.
Exact-name environment inheritance and value redaction limit accidental credential disclosure.
For a nonzero exit, the executor selects a bounded, line-aware stderr excerpt that retains the
last actionable diagnostic and final summary where they fit. It explicitly reports truncation
and byte counts. A long compiler invocation cannot consume the full failure response.
The executor does not persist full stderr or serve a diagnostic artifact.
They do not isolate files, networks, or hostile processes.
Only the observed subprocess result can produce a passing gate receipt.
A receipt proves that command invocation, not an unperformed manual review or a missing coverage measurement.

Version 2 configuration distinguishes runtime-enforced settings from advisory expectations.
Consumers must not describe an advisory threshold or writing rule as executor enforcement.
Release work performs its declared policy checks separately and retains their actual observations.

## Optional ontology

ADRs 0012 and 0013 define feature resolution and one guarded backing path.
Disabled ontology is absent from the served tool and resource surfaces and does not seed a file.
Enabled generation replaces only the shared empty-stub representation.
A substantive ontology survives a generation request that cannot safely replace it.
This repository keeps the explicit disabled setting unchanged.

## Scoped repository-seed authority

ADR-0019 reconciles ADR-0008's blanket wording with the audited repository-seed implementation.
Normal machine-state writes still use the `.agent/` guard.
The separate seed writer requires explicit opt-in and checks lexical and resolved containment under the repository root.

The MCP scaffold can create only its generated `.agent/` files and absent root workflow targets.
Its root targets are `AGENTS.md`, `CONVENTIONS.md`, the declared hooks, and the generated `.github/` and `.kiro/` templates.
The mutation coordinator holds the worktree writer permit across planning, receipt creation, and commit.
Existing targets are skipped without replacement.

CLI initialization rejects an existing `.agent/`, `specs/`, `skills/`, `AGENTS.md`, `CONVENTIONS.md`, `.githooks/`, or `.github/` target.
Per-path scaffold checks preserve an existing generated target outside that top-level set.
Initialization then creates the generated scaffold, selected package-manifest files, and the workspace ownership manifest.
The empty `specs/adr/.gitkeep` marker is the exclusive authorized `specs/` seed.
The fresh-target check prevents this marker from changing an existing narrative tree.

Managed upgrades can change generated or package-manifest paths that the ownership plan classifies for change.
They preserve unowned collisions and local edits.
Apply holds the writer lease, requires a clean worktree and index, and verifies operation preimages and postimages.
The journal supports resume and rollback before final validation and cleanup.
Managed upgrades do not manage or mutate `specs/**`.

The retained 1.0.4 scaffold observation proves only the behavior of that release.
Current 1.0.5 verification must exercise the affected scaffold, initialization, and upgrade paths.
Neither the historical observation nor scoped P6 verification certifies every possible runtime write.

## Correctness properties

The identities below come from the recovered designs. They are traceability, not a claim that historical wording remains authoritative.
Current requirements and accepted ADRs determine applicability. Executable evidence determines whether an implementation satisfies them.

| Property | Identity and current interpretation                                                                                   | Historical source                                      |
| -------- | --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| P1       | Ontology alias rejection applies when the ontology surface is enabled.                                                | [Refactor](archive/truenorth-mcp-refactor/design.md)   |
| P2       | A verification gate passes only on the observed zero exit result.                                                     | [Refactor](archive/truenorth-mcp-refactor/design.md)   |
| P3       | Mutations leave schema-valid state and preserve unowned fields.                                                       | [Refactor](archive/truenorth-mcp-refactor/design.md)   |
| P4       | Tier transforms preserve the declared invariants.                                                                     | [Refactor](archive/truenorth-mcp-refactor/design.md)   |
| P5       | Active references do not point to absent or retired targets.                                                          | [Refactor](archive/truenorth-mcp-refactor/design.md)   |
| P6       | Guarded machine-workspace writes stay under `.agent/`. ADR-0019 separately governs enumerated repository-seed writes. | [Profiles](archive/agent-workspace-profiles/design.md) |
| P7       | Cockpit relocation preserves unknown fields and version.                                                              | [Profiles](archive/agent-workspace-profiles/design.md) |
| P8       | Neutral grouping preserves the documented legacy mapping and fields.                                                  | [Profiles](archive/agent-workspace-profiles/design.md) |
| P9       | Invalid grouping input does not mutate state.                                                                         | [Profiles](archive/agent-workspace-profiles/design.md) |
| P10      | Profile default and unknown-name resolution follow the profile contract.                                              | [Profiles](archive/agent-workspace-profiles/design.md) |
| P11      | Post-merge sweep applies its safety conditions.                                                                       | [Profiles](archive/agent-workspace-profiles/design.md) |
| P12      | Commit issue decisions follow the selected profile.                                                                   | [Profiles](archive/agent-workspace-profiles/design.md) |
| P13      | Greenfield scaffolding does not overwrite user content.                                                               | [Profiles](archive/agent-workspace-profiles/design.md) |
| P14      | Bug references validate input and preserve tags.                                                                      | [Profiles](archive/agent-workspace-profiles/design.md) |
| P15      | Active skill references resolve after cleanup.                                                                        | [Profiles](archive/agent-workspace-profiles/design.md) |
| P16      | Missing ontology feature settings preserve the enabled default.                                                       | [Ontology](archive/optional-ontology/design.md)        |
| P17      | Disabled ontology tools and resource are absent.                                                                      | [Ontology](archive/optional-ontology/design.md)        |
| P18      | Disabled ontology does not seed its backing file.                                                                     | [Ontology](archive/optional-ontology/design.md)        |
| P19      | Enabled ontology has one guarded writable backing path.                                                               | [Ontology](archive/optional-ontology/design.md)        |
| P20      | Generation replaces the shared empty stub only.                                                                       | [Ontology](archive/optional-ontology/design.md)        |
| P21      | Ontology verification identifies the empty stub.                                                                      | [Ontology](archive/optional-ontology/design.md)        |

## Authority

[CONVENTIONS.md](../CONVENTIONS.md#document-precedence) defines precedence and the single writing policy.
[ADR-0019](adr/0019-scoped-repository-seed-write-authority.md) defines the scoped repository-seed authority.
The [#520 authority record](../.agent/tasks/compliance-515/520-authority-evidence.json) retains its approval and interpretation.
The archive retains exact historical text, including superseded paths and decisions.
It does not override the current contract, later accepted ADRs, or live execution status.
The #517 review applies to its recorded 1.0.4 scope and does not certify this prepared 1.0.5 change.
