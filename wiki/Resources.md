# Resources

The runtime serves the project cockpit as live MCP resources under the `truenorth://`
scheme. A `resources/read` reflects the current on-disk content. A disk edit emits a
`notifications/resources/updated` for the affected resource.

## The resources

| URI                       | Backing file                    | Contents                          |
| ------------------------- | ------------------------------- | --------------------------------- |
| `truenorth://state`       | `.agent/tasks/state.yml`        | The phase, the TDD step, handoff  |
| `truenorth://cockpit`     | `.agent/tasks/release-plan.yml` | The recorded tasks                |
| `truenorth://conventions` | `CONVENTIONS.md`                | The engineering conventions       |
| `truenorth://adr`         | `specs/adr/`                    | The Architecture Decision Records |
| `truenorth://ontology`    | `.agent/ontology.yml`           | The domain ontology               |

The `truenorth://adr` resource is read-only. It serves the human-authored ADRs, which the
runtime never mutates.

The `truenorth://ontology` resource is present only when the ontology feature is enabled.
When the feature is disabled, the resource is absent and a read of its URI is an unknown
resource. See [The ontology feature](The-ontology-feature).

## Write-through-notify

Disk is the source of truth. The runtime is a syncing peer, not the sole writer (ADR-0004).
It reads live from disk, writes through to disk, then emits a resource update. A human or
another tool can edit a backing file directly. The watcher maps the debounced change to a
resource URI and notifies, so a client re-reads fresh content.

## Read errors do not crash

A parse or validation failure on a backing file returns a resource read error that names
the file and the cause. The runtime retains the last good content and keeps serving the
other resources. It does not crash on a malformed file.
