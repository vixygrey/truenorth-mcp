# ADR-0016: Versioned Workspace Ownership and Upgrades

**Status:** Accepted
**Date:** 2026-09-27

## Context

`init` installs a versioned set of skills and generated workflow files, but earlier workspaces
have no machine-readable ownership record. Re-running `init` is intentionally rejected. A
package update therefore cannot distinguish an unchanged installed file from an operator edit,
or remove a retired file without risking user data.

The runtime must remain local and deterministic. Upgrade behavior cannot depend on a network
registry, an unpinned source tree, or mutation under the human-authored `specs/` tree.

## Decision

The npm package publishes `bundle/current.json`, immutable historical manifests under
`bundle/history/`, and the files named by those manifests. `init` writes
`.agent/workspace-manifest.yml` last. The workspace manifest records the installed bundle and
workspace schema versions, active profile, and the source digest and mode of each managed file.

`truenorth-mcp upgrade --check` computes and prints a stable JSON plan without writing. The plan
classifies paths as add, update, preserve, conflict, or remove. Upgrade application uses these
rules:

- add an absent new managed path;
- update a managed path only when its content and mode still match the recorded source;
- preserve local edits and advance their recorded source baseline when the bundle also changed;
- preserve an unowned path collision without claiming ownership;
- remove a retired managed path only when it still matches its recorded source;
- never manage or mutate `specs/**`.

A workspace without a manifest is eligible only when the package contains a supported historical
manifest. Migration adopts historical bundle files and profile-generated files only when their
content and mode match exactly. TrueNorth-MCP 1.0.2 is the first migration baseline.

Application requires a clean Git worktree and index and the same advisory writer lease used by MCP
mutations. It stages source files, backups, and a journal under `.agent/runtime/upgrade`. Each
operation verifies its expected preimage or postimage, making interrupted application resumable.
Validation runs before the journal is removed. A failed application or validation restores every
preimage in reverse order.

## Consequences

Operators can inspect upgrades before mutation. Local edits win conflicts, so an upgrade can
complete without replacing user work. Files that predate the ownership manifest are managed only
when exact historical evidence supports adoption.

Release packaging must keep the bundle version aligned with the runtime and npm package versions.
The generated current manifest is checked in preflight, and historical manifests are retained for
every supported migration source.

The clean-worktree requirement makes rollback auditable and prevents upgrade recovery from
interacting with unrelated changes. Upgrade is unavailable when Git is unavailable, the writer
lease is held elsewhere, the source version is unsupported, or packaged bytes do not match their
manifest.
