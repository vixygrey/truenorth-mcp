# The .agent workspace

The `.agent/` directory is the machine-facing layer. The runtime reads, watches, and writes
only under `.agent/`. A single write guard rejects any write outside it (ADR-0008). The
human-facing `specs/` layer holds narrative the runtime reads but never mutates.

## The layout contract

`.agent/layout.yml` is the layout contract. It lists the required areas as
language-agnostic data, so a reader in any language can parse it. The runtime validates the
contract at startup. A missing required entry logs a warning that names the absent path,
and the runtime keeps serving on the last valid state.

V1 supports `version: "1"` only. See [Compatibility](Compatibility) for the complete
workspace contract, legacy cockpit migration behavior, and upgrade steps.

## The required entries

The contract requires these entries under `.agent/`. Each area is a directory with its
named files.

| Area         | Required entries            | Purpose                                            |
| ------------ | --------------------------- | -------------------------------------------------- |
| `config/`    | `rules.yml`                 | Token caps, approval gates, protected paths, flags |
| `spec/`      | `requirements.md`           | The feature narrative                              |
| `tasks/`     | `state.yml`                 | Cockpit state, the phase, the TDD step             |
| `memories/`  | `lessons.md`, `glossary.md` | Durable lessons and the working glossary           |
| `telemetry/` | `runs.yml`                  | The agent cost audit, excluded from agent reads    |
| (root)       | `layout.yml`, `profile.yml` | The contract and the active profile name           |

The runtime also reads `tasks/release-plan.yml` for the recorded tasks and, when the
ontology feature is enabled, `ontology.yml`. Product docs live under `product/`.

## Who writes what

- `tasks/state.yml`, `tasks/release-plan.yml`, and `tasks/bugs.yml` are runtime-written.
- `layout.yml`, `profile.yml`, `config/*`, `spec/*`, and `product/*` are human-seeded.
- `memories/*` and `tasks/backlog.yml` are mixed: human-seeded and runtime-updatable.
- `telemetry/runs.yml` is runtime-written and excluded from agent reads.

## The single write guard

Every runtime write funnels through one guarded path. It normalizes the target, rejects any
path that escapes `.agent/`, and writes nothing on rejection. This is the load-bearing
invariant of the machine-facing layer. The scaffold is the one authorized exception: it
seeds files outside `.agent/` (root docs, git hooks, `.github/`) through an audited seed
path that the runtime tools never call.

## Version control

The `.agent/` tree tracks under version control. It is the durable project state, so it is
committed with the code rather than ignored.
