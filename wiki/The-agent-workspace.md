# The .agent workspace

The `.agent/` directory is the machine-facing layer. Normal cockpit mutations use
the guarded `.agent/` path. Scaffold, fresh initialization, and managed upgrade
have separate, enumerated repository-seed authority (ADR-0019); they do not grant
general write access to human-facing `specs/` narrative.

## The layout contract

`.agent/layout.yml` is the layout contract. It lists the required areas as
language-agnostic data, so a reader in any language can parse it. The runtime validates the
contract at startup. A missing required entry logs a warning that names the absent path,
and the runtime keeps serving on the last valid state.

The runtime supports layout `version: "1"` only. See [Compatibility](Compatibility) for the complete
workspace contract, legacy cockpit migration behavior, and upgrade steps.

## The required entries

The contract requires these entries under `.agent/`. Each area is a directory with its
named files.

| Area        | Required entries            | Purpose                                                                                    |
| ----------- | --------------------------- | ------------------------------------------------------------------------------------------ |
| `config/`   | `rules.yml`                 | Token caps and runtime feature settings                                                    |
| `spec/`     | `requirements.md`           | The feature narrative                                                                      |
| `tasks/`    | `state.json`                | Cockpit state, the phase, the TDD step <!-- truenorth-lint: allow-relative-layout-path --> |
| `memories/` | `lessons.md`, `glossary.md` | Durable lessons and the working glossary                                                   |
| (root)      | `layout.yml`, `profile.yml` | The contract and the active profile name                                                   |

The runtime also reads `tasks/release-plan.json` for recorded tasks.
`tasks/backlog.yml` independently declares whether backlog ownership is local or
external. A local file owns its `backlog` list. An external declaration contains
only a provider pointer, and consumers read current work from that tracker. The
runtime does not contact an external tracker by default. When the ontology feature
is enabled, the runtime also reads `ontology.yml`. Product docs live under
`product/`.

## Who writes what

- `tasks/state.json`, `tasks/release-plan.json`, and `tasks/bugs.json` are runtime-written.
- `tasks/backlog.yml` is human-seeded. Local owners may update its backlog; external
  owners keep only provider metadata in it.
- `layout.yml`, `profile.yml`, `config/*`, `spec/*`, `product/*`, and `memories/*`
  are human-seeded.
- `.agent/telemetry/` remains an optional local diagnostics location. It is excluded
  from agent reads, and the runtime does not create or append a telemetry journal.

TrueNorth does not collect or retain aggregate skill invocation counts or timings.
Operation receipts are returned for individual calls and are not a retained usage
source. Stocktake usage analysis stores no raw prompts, repository file contents,
secrets, or user identifiers. This follows the receipt and no-journal decision in
[issue #436](https://github.com/vixygrey/truenorth-mcp/issues/436).

## Filesystem write boundaries

Normal cockpit writes stay under `.agent/`. Repository-seed writes are limited
to enumerated scaffold, fresh-init, and managed-upgrade targets, with containment,
ownership, and transaction checks. They do not edit existing human narrative
under `specs/`; see [ADR-0019](../specs/adr/0019-scoped-repository-seed-write-authority.md).

## One writer per worktree

One TrueNorth server owns mutation rights for a Git worktree. It holds an operating-system
lock on `.agent/runtime/writer.lock` and serializes mutating requests inside the process. A
second server can read existing resources, but its mutating tools return a typed
`writer_lease_conflict`.

The lock file can remain after the owner exits or crashes. Its presence does not mean the
worktree is locked. The operating-system lock on the open file descriptor is authoritative,
and another running server retries it on the next mutation.

Use separate Git worktrees for parallel agents that both need to mutate TrueNorth state.
Network-filesystem coordination is unsupported.

## Version control

The `.agent/` tree tracks under version control. It is the durable project state, so it is
committed with the code rather than ignored.
