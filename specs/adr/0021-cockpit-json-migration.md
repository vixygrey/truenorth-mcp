# ADR-0021: Cockpit Documents Move to JSON

**Status:** Accepted
**Date:** 2026-10-17
**Supersedes:** The state and release-plan path and format portion of [ADR-0011](0011-cockpit-relocation.md).

## Context

Issue #527 found that the runtime depended on `serde_yaml`, an archived crate with a
reported unsafe finding in its `libyaml` bridge. Issue #571 investigated whether the
three runtime-owned mutable cockpit documents, `state.yml`, `release-plan.yml`, and
`bugs.yml`, should move from YAML to JSON as part of that remediation.

YAML's dynamic-value surface, non-string mapping keys, tags, aliases, merge keys, and
multi-document streams, is wider than these three documents need. The runtime only
ever mutates them as ordered maps that preserve unknown fields and explicit `null`.
`serde_json::Value` with the `preserve_order` feature already gives that contract
with no YAML-specific construct to define or defend at this boundary.

## Decision

The three runtime-owned mutable cockpit documents are JSON, not YAML:

- `.agent/tasks/state.json` (was `state.yml`)
- `.agent/tasks/release-plan.json` (was `release-plan.yml`)
- `.agent/tasks/bugs.json` (was `bugs.yml`)

Each document root is a JSON object. The runtime parses it through an ordered,
duplicate-key-rejecting codec (`engine::cockpit_json`) and serializes it back through
the same codec, so unknown nested fields, explicit `null`, arrays, and key insertion
order survive a read-mutate-write cycle. A non-object root is rejected.

The `truenorth://state` and `truenorth://cockpit` resource URIs are unchanged. Their
served MIME type is `application/json`, not `application/yaml`.

The legacy `specs/state.yaml` and `specs/release-plan.yaml` fallback read that
ADR-0011 established is removed for these two documents. An absent
`.agent/tasks/state.json` or `.agent/tasks/release-plan.json` now resolves to a fresh
empty document, the same way a brand-new workspace already behaved. A workspace that
still has only the legacy `.yml` files needs the explicit migration command below; the
runtime does not silently read or convert them.

The workspace and bundle schema version moves to `"3"` (`MANIFEST_VERSION`,
`BUNDLE_SCHEMA_VERSION`) to mark this as a breaking change to the managed workspace
shape. `.agent/workspace-manifest.yml` itself stays YAML; only the three mutable
cockpit documents change format. `.agent/layout.yml` requires `tasks/state.json`.

An explicit, read-only analysis command ships ahead of any write path:

```text
truenorth-mcp migrate-cockpit-json --check
```

It reports, for each of the three documents, the selected source (preferring an
existing `.agent/tasks/*.yml`, falling back to a compatible `specs/*.yaml` only when
the `.agent/` source is absent), the source digest, any unsupported YAML construct
(non-string keys, tags, aliases, a non-object root, a multi-document stream), and
whether a comment would be lost. It never writes. A dedicated migration transaction
(`engine::cockpit_json_transaction`) backs the eventual `--apply`, `--resume`, and
`--rollback` commands with its own journal, separate from the workspace-upgrade
transaction's managed-file ownership model. `--apply` is not yet exposed; shipping it
is tracked as follow-up work, not part of this decision's initial cutover.

Two bundled consumer scripts read and write these documents directly: the
`extract-design` skill's handoff writer and the `visual-dashboard` skill's status
reader. Both read and write JSON now, through a full-object merge that preserves
unknown fields, not a string-append.

## Consequences

The runtime, its scaffold seeds, its profile starter files, its watcher path mapping,
and its bundled consumer scripts speak one format, JSON, for the cockpit documents
they mutate. The dynamic-YAML surface at this boundary, and the associated parser
risk, is gone.

A workspace scaffolded before this change needs an explicit migration before the
runtime reads its cockpit correctly; there is no silent YAML fallback for state or
the release plan. The ontology resource, `execution-status.yml`, `backlog.yml`, and
every human-authored configuration file keep their existing YAML format; this
decision is scoped to the three mutable cockpit documents named above.

The safe-YAML-parser remediation from issue #527 remains independently required: the
runtime still reads and writes YAML for configuration, frontmatter, the backlog,
the ontology, and workspace manifests. The Jev-report entry in the original
decision describes the earlier release; Jev reports are retired in 2.0.0.
