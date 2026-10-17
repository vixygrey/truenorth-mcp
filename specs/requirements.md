# Current requirements

This document describes the prepared 1.0.5 contract. See [provenance and review status](./README.md).
Writing policy and document precedence are defined in [CONVENTIONS.md](../CONVENTIONS.md).

## R1. Protocol and delivery

The Rust runtime serves typed MCP tools and resources over stdio.
The thin Node.js wrapper selects the platform binary and delivers the declared skill bundle.
A supported bundle retains its historical manifest. A current manifest matches the delivered files and aligned package versions.
Tool input validation identifies the offending field and rejects invalid input before mutation.
Errors provide an actionable correction rather than terminate unrelated resource service.

## R2. Workspace ownership

The machine workspace is `.agent/`. The layout contract defines its supported structure.
Normal machine-state mutations use the shared guard and stay inside that workspace.
ADR-0019 separately authorizes the enumerated scaffold, fresh-init, and managed-upgrade repository-seed paths.
Existing human requirements, designs, task narratives, and ADRs remain read-only to the runtime.
Fresh initialization can create only the empty `specs/adr/.gitkeep` seed after it rejects an existing `specs/` target.
Read-only repository resources can expose human narratives and root conventions.
Runtime and skill operations do not recreate the former mutable cockpit under `specs/`.
Unknown supported workspace fields survive updates that do not own them.

## R3. State and lifecycle

The cockpit records lifecycle phase, handoff, release tasks, story status, and bug references in their declared machine files.
Phase advancement uses `truenorth_advance_phase`.
The TDD tool enforces the declared Red, Green, and Refactor order and observes the red test result.
An optional `group_id` scopes a cycle to one task group. An omitted value retains the legacy unscoped cycle.
A failed validation or write does not leave a partially changed state file.
Legacy phase mappings follow the project convention rather than create another active lifecycle.

## R4. Methodology and grouping

The profile selects epic-based, issue-per-task, kanban, milestone-based, or generic methodology.
The default profile is issue-per-task.
The profile controls grouping vocabulary, grouping requirements, branch pattern, and commit issue requirements.
Tasks use `group_id` and optional `group_kind`. A legacy `epic_id` maps to an epic group.
Do not require an epic for a profile that does not require grouping.
Invalid grouping input fails without changing recorded state.

## R5. Work and bug ownership

Record work with `truenorth_record_task` and its verify command.
Backlog ownership is independent of the methodology profile.
A local backlog owns its declared items. An external backlog file is a provider pointer, not a live issue cache.
This repository uses GitHub Issues as its backlog authority.
Bug records retain external references, status, linked work, and tags, not a duplicated tracker narrative.
The default runtime request path does not call the external tracker.

## R6. Verification boundary

`truenorth_verify_gate` runs the trusted operator command from `TRUENORTH_VERIFY_CMD`.
The caller does not supply the shell command or replace execution with a claimed result.
A gate passes only when its executed command returns the expected zero status within the configured bounds.
Timeout, disallowed command, invalid configuration, and nonzero exit are failure outcomes.
For a nonzero exit, the server returns a bounded, line-aware stderr diagnostic excerpt and its
captured, returned, and truncation metadata. A truncated excerpt does not claim to be a full
command log and directs the caller to rerun the configured gate locally.
The executor does not persist full stderr or provide a diagnostic artifact retrieval surface.
The leading command allowlist is not containment for the remaining shell expression.
The executor is not a filesystem, network, or hostile command sandbox.
Environment inheritance and stderr redaction follow the exact-name policy in the conventions.
A successful supporting command does not replace required observable smoke or human approval.

## R7. Enforced and advisory rules

Version 2 `.agent/config/rules.yml` separates runtime-enforced settings from `advisory` policy.
Schema validation and runtime enforcement do not claim that advisory coverage or prose rules were measured.
Coverage policy names overall and business-logic thresholds. Release work requires actual measurements and its declared comparison.
The current compliance plan assigns the coverage evaluator and client gate remediation to #518.
A configuration check alone does not establish those outcomes.

## R8. Optional ontology

An absent feature setting preserves the documented enabled default.
An explicit false setting removes the ontology tools and resource and avoids ontology seeding.
This repository explicitly disables ontology. This restoration does not enable it.
Enabled ontology uses `.agent/ontology.yml` as its writable backing path.
Legacy read compatibility does not authorize a write outside the machine workspace.
Generation replaces only the documented empty stub, not substantive user ontology content.
Verification identifies an empty stub without treating it as a populated domain model.

## R9. Skill contracts

A skill has validated frontmatter, a supported kind, and the applicable size contract.
A scripted skill declares its runnable verify command. A prose skill does not claim scripted execution.
Tier transforms preserve the declared reasoning invariants.
Graph references resolve to declared skills rather than retired placeholders.
Root and distributed skill copies agree with the current bundle manifest.

## R10. Scaffolding and integration

Scaffolding creates the declared machine layout and absent workflow files without overwriting existing user content.
Fresh initialization rejects its enumerated existing top-level targets and preserves other generated path collisions.
Managed upgrades preserve unowned collisions and local edits and mutate only ownership-planned paths.
Upgrade apply requires its lease, clean-worktree check, transaction checks, validation, and rollback behavior.
Post-merge cleanup checks its safety conditions before a destructive operation.
Issue and attribution checks follow the active profile and project conventions.
A recorded release task or a local passing gate does not authorize a commit, merge, tag, or publication.
Those actions require their separate approvals.

## Historical differences

The recovered refactor predates the machine workspace and neutral grouping contract.
Its mutable `specs/` cockpit, compulsory epic grouping, caller-evidence verification, and stronger sandbox descriptions are not current requirements.
The current write guard, profile rules, and executor boundary above replace those statements.
The [property index](./design.md#correctness-properties) preserves historical identities without treating old text as present proof.

## Repository-seed authority and verification

[ADR-0019](adr/0019-scoped-repository-seed-write-authority.md) resolves the prior wording inconsistency.
It supersedes only ADR-0008's blanket confinement clauses.
The `.agent/` guard, layer ownership, and protection of existing human narrative remain requirements.

The separate repository-seed writer requires explicit opt-in and repository containment.
The MCP scaffold can seed absent generated root workflow paths under the mutation permit.
Fresh initialization can seed generated and selected package-manifest paths only after its full fresh-target check.
Managed upgrade can change only generated and package-manifest paths selected by its ownership plan.
It must preserve unowned collisions and local edits and must not manage `specs/**`.

The retained native observation records the 1.0.4 scaffold behavior.
It is historical evidence, not certification of the prepared 1.0.5 contract.
Current verification must exercise scaffold collision handling, fresh initialization, and managed-upgrade transaction behavior.
It must not report a blanket every-runtime-write confinement result.
