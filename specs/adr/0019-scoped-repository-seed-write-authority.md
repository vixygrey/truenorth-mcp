# ADR-0019: Scoped Repository-Seed Write Authority

**Status:** Accepted
**Date:** 2026-10-10
**Supersedes in part:** ADR-0008 blanket write-confinement clauses
**Related:** ADR-0011, ADR-0016
**Forward clarification:** In 2.0.0 the Jev hook seed in item 1 is retired;
the remaining scoped repository-seed authority is unchanged.

## Context

ADR-0008 separates mutable machine state under `.agent/` from human narrative under `specs/`.
Its blanket wording says that every runtime write stays under `.agent/`.

The implemented scaffold, fresh initialization, and managed upgrade paths also use an audited repository-seed writer.
Those paths create or maintain declared workflow files outside `.agent/` without granting general repository write authority.
The current documents must describe that narrow authority without certifying broader confinement.

## Decision

Normal machine-state mutations use `engine::agent_ws::write_under_agent` and stay under `.agent/`.
`write_repo_seed` and `write_repo_seed_bytes` are separate, explicit repository-seed paths.
They require caller opt-in and reject lexical traversal, absolute paths, and resolved symlink escapes from the repository root.
Their final file replacement is atomic.

The repository-seed authority applies only to these operations and targets:

1. `truenorth_scaffold_project` can seed its generated root workflow files when each target is absent.
   These files are `AGENTS.md`, `CONVENTIONS.md`, `.githooks/commit-msg`, `.githooks/post-merge`, and `.kiro/hooks/jev-guard.json`.
   The generated `.github/` paths are:
   - `.github/commit-template.md`
   - `.github/pull-request-template.md`
   - `.github/ISSUE_TEMPLATE/bug.md`
   - `.github/ISSUE_TEMPLATE/feature.md`
   - `.github/ISSUE_TEMPLATE/config.yml`
     The mutation coordinator holds the worktree writer permit for the complete MCP operation.
     Existing targets are skipped and retain their content.
2. CLI `init` requires its enumerated fresh-target set to be absent before it writes.
   The checked targets are `.agent/`, `specs/`, `skills/`, `AGENTS.md`, `CONVENTIONS.md`, `.githooks/`, and `.github/`.
   Per-path scaffold checks preserve an existing generated target outside that top-level set.
   Initialization can write the generated scaffold, selected package-manifest files, and `.agent/workspace-manifest.yml`.
   Its only authorized `specs/` write is the empty `specs/adr/.gitkeep` seed after the fresh-target check.
   The current package manifest contains selected `skills/**` files and no `specs/**` path.
3. CLI managed upgrade can write only generated scaffold paths and selected package-manifest paths that its ownership plan classifies for change.
   Upgrade planning preserves unowned path collisions and locally modified content.
   Apply holds the advisory worktree writer lease, requires a clean worktree and index, and uses the journal under `.agent/runtime/upgrade`.
   Each operation verifies its expected preimage or postimage.
   Failed validation restores recorded preimages.
   Managed upgrade does not manage or mutate `specs/**`.

This authority does not permit mutation of existing human narrative.
It does not turn the repository-seed helper into a general runtime write API.
ADR-0011 remains authoritative for cockpit relocation and legacy read-only fallback behavior.
ADR-0016 remains authoritative for ownership manifests, collision preservation, transactions, and managed upgrades.

This decision supersedes only ADR-0008 statements that every runtime write uses one `.agent/` guard and that no runtime path can write under `specs/`.
The layer ownership, guarded machine-state path, and prohibition on mutation of existing human narrative remain in force.

## Consequences

P6 means that guarded machine-workspace writes stay under `.agent/`.
Repository-seed behavior has its separate, enumerated authority and repository-containment checks.
A P6 result does not prove that every runtime write is confined to `.agent/`.

Historical 1.0.4 observations remain evidence of that release.
They do not certify the prepared 1.0.5 contract or a whole-repository invariant.
Current verification must exercise the affected scaffold, fresh-init, and managed-upgrade paths at the evaluated revision.

## Forward clarification

The `.kiro/hooks/jev-guard.json` seed listed above records the original
decision, not a current scaffold target. Version 2.0.0 retires that seed and
uses ADR-0016's existing ownership and transaction rules to remove verified
unchanged old hooks during managed upgrade. Modified and unowned hooks are
preserved for their owners to resolve. All other repository-seed write
boundaries remain in force.
