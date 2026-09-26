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

## Supported runtimes

The npm wrapper supports macOS ARM64/x64 and Linux ARM64/x64. Native Windows is
unsupported in v1; Windows users must run the wrapper and native binary in WSL.

The wrapper requires Node.js 18 or newer. CI exercises Node.js 18, 20, 22, 24, and 26. Compatibility with an end-of-life Node.js release does not extend that release's
upstream security support. Production installations should use a Node.js release that
still receives upstream security fixes.

## Supported workspaces

A v1 `.agent/` workspace has this layout contract:

```yaml
version: "1"
areas:
  config: [rules.yml]
  spec: [requirements.md]
  tasks: [state.yml]
  memories: [lessons.md, glossary.md]
  telemetry: [runs.yml]
```

The runtime accepts only layout version `"1"`. It also requires these paths:

- `.agent/config/rules.yml`
- `.agent/spec/requirements.md`
- `.agent/tasks/state.yml`
- `.agent/memories/lessons.md`
- `.agent/memories/glossary.md`
- `.agent/telemetry/runs.yml`
- `.agent/layout.yml`
- `.agent/profile.yml`

The runtime logs an invalid layout as a warning and preserves its last valid layout
cache. A missing layout remains valid for an unscaffolded or legacy workspace, so
v1 can perform the legacy reads described below.

`profile.yml` has no independent version field. Its v1 contract is one of these
fixed profile names:

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

## Cockpit migration

V1 reads the `.agent/` cockpit first. When the corresponding file is absent, it
reads these legacy BigPowers paths:

| V1 path                         | Legacy fallback           | First runtime write                    |
| ------------------------------- | ------------------------- | -------------------------------------- |
| `.agent/tasks/state.yml`        | `specs/state.yaml`        | Writes `.agent/tasks/state.yml`        |
| `.agent/tasks/release-plan.yml` | `specs/release-plan.yaml` | Writes `.agent/tasks/release-plan.yml` |
| `.agent/ontology.yml`           | `specs/ontology.yaml`     | Writes `.agent/ontology.yml`           |

When both files exist, the `.agent/` file wins. V1 never mutates a legacy
`specs/` source. It does not automatically delete or rename legacy files.

State and release-plan mutations preserve every unmodified field, including unknown
top-level fields and legacy version markers. The runtime validates YAML before
writing. A malformed cockpit input is rejected and left unchanged. This preservation
promise does not cover ontology rewrites, which use a structured ontology model.

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

A deprecation must identify its replacement, removal version, and migration action.
A release that contains a breaking change must include the migration notes in its
release material and pull request.

## MCP client certifications

Certification covers only the exact client and extension versions recorded
below. Each pass requires initialization, tool and resource discovery, one read,
one `truenorth_record_task` mutation confined to a disposable workspace, and
clean shutdown against the packaged TrueNorth-MCP 1.0.1 stdio artifact.

| Client                           | Pinned version      | Date       | Result                       | Evidence                                                           |
| -------------------------------- | ------------------- | ---------- | ---------------------------- | ------------------------------------------------------------------ |
| MCP Inspector CLI                | 2.5.0               | 2026-09-25 | Certified                    | [`mcp-inspector.json`](../compatibility/mcp-inspector.json)        |
| Oh My Pi                         | 18.2.11             | 2026-09-25 | Certified                    | [`oh-my-pi.json`](../compatibility/oh-my-pi.json)                  |
| OpenCode                         | 2.0.16              | 2026-09-25 | Certified                    | [`opencode.json`](../compatibility/opencode.json)                  |
| VS Code with GitHub Copilot Chat | 1.139.1 with 0.67.0 | 2026-09-25 | Certified                    | [`vscode-copilot.json`](../compatibility/vscode-copilot.json)      |
| Kiro                             | Not pinned          | Not tested | Pending manual certification | [Issue #407](https://github.com/vixygrey/truenorth-mcp/issues/407) |
| Croft IDE                        | 0.1.700             | 2026-09-25 | Unsupported                  | [`croft.json`](../compatibility/croft.json)                        |

The machine-readable [client matrix](../compatibility/mcp-clients.json) is the
authoritative status index. The repository [README](../README.md#reproduce-the-vs-code-certification)
contains the repeatable VS Code disposable-profile procedure. The VS Code
certificate records one non-blocking Copilot JSON Schema warning tracked in
[issue #412](https://github.com/vixygrey/truenorth-mcp/issues/412).

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

## Upgrade to v1

1. Commit or back up the existing workspace.
2. Check `.agent/layout.yml` has `version: "1"` and all required paths. A legacy
   workspace can omit it temporarily while it uses fallback reads.
3. Check `profile.yml` uses one supported profile. Omit it only to use the
   `issue-per-task` default.
4. Start v1 and inspect `resources/list`, then read `truenorth://state` and
   `truenorth://cockpit` before any mutation.
5. Run the relevant workflow operation. The first write creates the v1 cockpit
   file under `.agent/`; the legacy source remains unchanged.
6. Compare the new file with the committed legacy source. Confirm required fields
   and unknown fields are present before considering the workspace migrated.
7. Keep legacy files until the migration is reviewed and committed. Roll back by
   restoring the prior commit rather than editing an active cockpit in place.

Use the [compatibility issue form](https://github.com/vixygrey/truenorth-mcp/issues/new?template=compat.yml)
for a dropped field, failed legacy read, invalid phase mapping, or unexpected
resource result.

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
- `engine::cockpit::tests` validates legacy reads, migration writes, source
  immutability, and state and release-plan field preservation.
- `resources::tests` and `resources::prop_tests` validate resource URI stability,
  legacy fallback, `.agent/` precedence, and arbitrary unknown state fields.
- `integration_tests` exercises the MCP client surface end to end.
