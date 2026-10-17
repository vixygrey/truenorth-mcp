# Compatibility

This page defines the TrueNorth-MCP v1 compatibility contract. It applies to the
runtime, the npm wrapper, the `.agent/` workspace, MCP tools, and MCP resources.

## Compatibility scope

Within v1, a patch or minor release preserves existing supported workspace inputs,
advertised MCP tool names, resource URIs, required request fields, and documented
success and error semantics. A minor release can add an optional field, tool, or
resource. Clients must use MCP discovery and ignore additive fields they do not
need.

A tool or resource can be conditional on a feature flag. It is supported only when
it appears in `tools/list` or `resources/list` for that workspace. The ontology
tools and `truenorth://ontology` depend on `features.ontology`. The Jev guardrail
tool depends on `features.jev`.

A rename, removal, incompatible field-type change, altered required field, changed
success or error semantics, or removed resource URI is a breaking change. Breaking
changes require a new major version, release notes, a replacement when one exists,
and command-first migration steps.

## Operating and concurrency boundary

TrueNorth-MCP supports local development by individuals and small teams. The supported
deployment is one local stdio runtime governing one Git worktree, with one server holding
mutation rights. Additional servers may read, but their mutations return
`writer_lease_conflict`. Parallel mutating agents require separate Git worktrees, and Git
is the durable collaboration and history boundary.

Shared network-filesystem mutation, multi-user hosting, centralized policy enforcement,
authentication or authorization, and compliance-record guarantees are outside the v1
compatibility contract. Operation receipts are bounded call results, not retained audit
records. See the canonical [operating model](../README.md#operating-model).

## Supported runtimes

The npm wrapper supports macOS ARM64/x64 and Linux ARM64/x64. Native Windows is
unsupported in v1; Windows users must run the wrapper and native binary in WSL.

The wrapper requires Node.js 18 or newer. CI exercises Node.js 18, 20, 22, 24, and 26. Compatibility with an end-of-life Node.js release does not extend that release's
upstream security support. Production installations should use a Node.js release that
still receives upstream security fixes.

## Supported workspaces

A current `.agent/` workspace has this layout contract:

```yaml
version: "1"
areas:
  config: [rules.yml]
  spec: [requirements.md]
  tasks: [state.json] # truenorth-lint: allow-relative-layout-path
  memories: [lessons.md, glossary.md]
```

The runtime accepts layout version `"1"`. It requires these paths:

- `.agent/config/rules.yml`
- `.agent/spec/requirements.md`
- `.agent/tasks/state.json`
- `.agent/memories/lessons.md`
- `.agent/memories/glossary.md`
- `.agent/layout.yml`
- `.agent/profile.yml`

The runtime logs an invalid layout as a warning and preserves its last valid layout
cache. A missing layout remains valid for an unscaffolded workspace.

`.agent/telemetry/runs.yml` is no longer required or scaffolded. Existing telemetry
directories and files remain accepted as extra v1 content, but the runtime does not
write them. Before downgrading to a runtime that requires the placeholder, recreate it
if absent:

```sh
mkdir -p .agent/telemetry
test -e .agent/telemetry/runs.yml || printf 'runs: []\n' >.agent/telemetry/runs.yml
```

`profile.yml` has no independent version field. Its v1 contract is one of these fixed
profile names:

| Profile           | Grouping requirement        |
| ----------------- | --------------------------- |
| `issue-per-task`  | Optional ticket grouping    |
| `epic-based`      | Required epic grouping      |
| `milestone-based` | Required milestone grouping |
| `kanban`          | No grouping requirement     |
| `generic`         | No grouping requirement     |

An absent `profile.yml` resolves to `issue-per-task`. An unknown or malformed
profile is rejected without partial state.

## Backlog ownership

Backlog ownership is independent from the methodology profile. The optional
`.agent/tasks/backlog.yml` can hold an authoritative local list or point to an
external provider.

An explicit local backlog uses contract version `"1"` and a `backlog` list:

```yaml
version: "1"
ownership:
  mode: local
backlog: []
```

The list can contain project-defined issue or work-item records. A backlog file
without `ownership` retains the legacy local behavior.

An external backlog contains provider metadata only:

```yaml
version: "1"
ownership:
  mode: external
  provider: github
  url: https://github.com/example/project/issues
```

External ownership requires nonempty `provider` and `url` values and must not
contain a `backlog` field, including an empty list. Consumers obtain current work
from the declared provider. The runtime validates the ownership metadata but makes
no external tracker request.

## Cockpit documents

ADR-0021 defines the runtime-owned cockpit documents:

| Document       | Path                             | Resource MIME type                          |
| -------------- | -------------------------------- | ------------------------------------------- |
| State          | `.agent/tasks/state.json`        | `application/json` at `truenorth://state`   |
| Release plan   | `.agent/tasks/release-plan.json` | `application/json` at `truenorth://cockpit` |
| Bug references | `.agent/tasks/bugs.json`         | not served as an MCP resource               |

Each document root is a JSON object. Runtime mutations preserve unknown fields,
explicit `null`, arrays, and key insertion order. A malformed or non-object
document is rejected without a partial mutation.

The resource URIs remain stable. There is no YAML fallback for these three
documents. A workspace that still has legacy cockpit YAML must use
`truenorth-mcp migrate-cockpit-json --check` to identify its source and migration
constraints. That command is read-only; the runtime does not silently read or
convert legacy files.

The lifecycle tool continues to accept legacy phase names where the runtime maps
them to a current lifecycle phase. `truenorth_record_task` continues to accept
`epic_id`; it maps to `group_id` with `group_kind: epic`.

## MCP tools and resources

The following resource URIs are stable throughout v1:

- `truenorth://state`
- `truenorth://cockpit`
- `truenorth://conventions`
- `truenorth://adr`
- `truenorth://ontology`, when the ontology feature is enabled

These active tool names are stable throughout v1:

- `truenorth_advance_phase`
- `truenorth_record_task`
- `truenorth_verify_gate`
- `truenorth_tdd_cycle`
- `truenorth_scaffold_project`
- `truenorth_record_bug`
- `truenorth_generate_ontology` and `truenorth_verify_ontology`, when the ontology
  feature is enabled
- `truenorth_guard_change`, when the Jev feature is enabled

The catalog tools `index_skills`, `get_skill`, `read_skill`, `search_skills`,
`build_skill_graph`, `read_graph`, `search_nodes`, `open_nodes`,
`get_dependencies`, `get_git_context`, and `validate_skill` remain available for
the skill library. Use `tools/list` as the authoritative surface for a running
workspace, especially when feature flags are set.

### Operation receipts

Successful repository mutations retain their existing result fields and add a
`receipt` object:

```json
{
  "receipt": {
    "schema_version": 1,
    "operation": "truenorth_record_task",
    "runtime_version": "1.0.3",
    "correlation_id": "mcp-sha256:<digest>",
    "changes": [
      {
        "path": ".agent/tasks/release-plan.json",
        "sha256": "<digest of the exact resulting bytes>"
      }
    ]
  }
}
```

`changes` is sorted by repository-relative path and contains only files written by
that call. It never contains absolute paths, commands, environment values, or file
content. `correlation_id` is a one-way digest of the MCP request identifier, not the
identifier itself. Consumers must ignore unknown receipt fields so minor releases can
extend the schema.

Gate operations add `receipt.gate` with `status` (`pass`, `failure`, or `timeout`),
`expectation` (`zero` or `nonzero`), `duration_ms`, and nullable `exit_code`.
Gate failures carry the same receipt in MCP error data beside `remediation_hints`.
The configured command is never returned. Mutation failures remain MCP errors and do
not return a success receipt.

A deprecation must identify its replacement, removal version, and migration action.
A release that contains a breaking change must include the migration notes in its
release material and pull request.

## MCP client certifications

Certification covers only the exact client and extension versions recorded
below. Each pass requires lifecycle negotiation, tool and resource discovery, one
read, one `truenorth_record_task` mutation confined to a disposable workspace, and
clean shutdown against the packaged TrueNorth-MCP version in its evidence file.

| Client                           | Pinned version      | Date       | Result      | Evidence                                                      |
| -------------------------------- | ------------------- | ---------- | ----------- | ------------------------------------------------------------- |
| MCP Inspector CLI                | 2.8.0               | 2026-09-27 | Certified   | [`mcp-inspector.json`](../compatibility/mcp-inspector.json)   |
| Oh My Pi                         | 18.2.11             | 2026-09-25 | Certified   | [`oh-my-pi.json`](../compatibility/oh-my-pi.json)             |
| OpenCode                         | 2.0.16              | 2026-09-25 | Certified   | [`opencode.json`](../compatibility/opencode.json)             |
| VS Code with GitHub Copilot Chat | 1.139.1 with 0.67.0 | 2026-09-25 | Certified   | [`vscode-copilot.json`](../compatibility/vscode-copilot.json) |
| Kiro                             | 1.1.70              | 2026-09-26 | Certified   | [`kiro.json`](../compatibility/kiro.json)                     |
| Zed                              | 1.21.0              | 2026-09-26 | Unsupported | [`zed.json`](../compatibility/zed.json)                       |
| Croft IDE                        | 0.1.700             | 2026-09-25 | Unsupported | [`croft.json`](../compatibility/croft.json)                   |

The machine-readable [client matrix](../compatibility/mcp-clients.json) is the
authoritative status index. The repository [README](../README.md#reproduce-the-vs-code-certification)
contains the repeatable VS Code disposable-profile procedure. The VS Code
certificate records one non-blocking Copilot JSON Schema warning tracked in
[issue #412](https://github.com/vixygrey/truenorth-mcp/issues/412).

Zed 1.21.0 initializes the released TrueNorth-MCP 1.0.2 stdio
artifact, discovers all 19 tools, completes `get_skill` and
`truenorth_record_task`, and shuts the server down cleanly. Its native MCP
integration does not request `resources/list` or expose `resources/read`, so it
does not satisfy the full certification contract. The
[Zed assessment](../compatibility/zed.json) records the project-local
configuration, protocol trace, disposable mutation, and isolation evidence.
[Issue #427](https://github.com/vixygrey/truenorth-mcp/issues/427) tracks the
assessment.

Croft 0.1.700 can initialize a local stdio server, discover tools, and invoke a
predeclared zero-argument read tool. It cannot complete this certification
contract because its client exposes no MCP resources and its deterministic
command model can supply at most one string argument, while
`truenorth_record_task` requires both `task_name` and `verify_command`. The
[Croft assessment](../compatibility/croft.json) records the partial
interoperability. [Issue #406](https://github.com/vixygrey/truenorth-mcp/issues/406)
tracks the assessment, and
[croft#685](https://github.com/vitali87/croft/issues/685) tracks forced sidecar
shutdown.

## JSON cockpit migration

1. Commit or back up the existing workspace.
2. Check `.agent/layout.yml` has `version: "1"` and requires `tasks/state.json`.
3. Run `truenorth-mcp migrate-cockpit-json --check` for a workspace that has
   legacy cockpit YAML. It reports the selected source, unsupported YAML constructs,
   and potential comment loss without writing.
4. Do not expect the runtime to read, rename, or convert legacy cockpit YAML during
   normal operations. Preserve the original workspace until the explicit migration
   workflow is complete and reviewed.
5. Start the runtime and inspect `resources/list`, then read
   `truenorth://state` and `truenorth://cockpit` before mutation. They serve JSON
   with their stable URIs.

Use the [compatibility issue form](https://github.com/vixygrey/truenorth-mcp/issues/new?template=compat.yml)
for a dropped field, failed JSON migration analysis, invalid phase mapping, or
unexpected resource result.

## Contract evidence

The runtime tests enforce this policy:

- `engine::agent_ws::layout::tests` validates layout version and required paths.
- `engine::profile::tests` validates the default and five supported profiles.
- `engine::backlog::tests` validates missing, legacy, local, external, and invalid
  backlog ownership declarations.
- `cli_diagnostics::external_backlog_rejects_cached_issue_entries` validates that
  `--check-config` rejects cached entries under external ownership.
- `tools::scaffold::tests::scaffold_emits_the_agent_tree_and_root_docs` validates
  that a fresh project receives explicit local backlog ownership.
- `engine::cockpit::tests` validates JSON cockpit mutations, source preservation,
  and state and release-plan field preservation.
- `resources::tests` and `resources::prop_tests` validate resource URI stability,
  JSON MIME types, `.agent/` backing paths, and arbitrary unknown state fields.
- `integration_tests` exercises the MCP client surface end to end.
