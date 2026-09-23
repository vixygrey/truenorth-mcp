# truenorth-mcp

An active, protocol-first MCP execution runtime for spec-driven engineering
discipline. Written in Rust, distributed as a thin Node.js wrapper. Token-lean and
model and harness agnostic.

> **Credit.** TrueNorth-MCP began as a fork of
> [bigpowers](https://github.com/danielvm-git/bigpowers) by danielvm-git, under the MIT
> license. The inherited work is the skill set: the engineering discipline in the
> `skills/` directory is the upstream contribution this project builds on. The Rust
> runtime is a clean-room reimplementation and shares no code with the upstream
> TypeScript server. Thank you to the bigpowers author.

## Relationship to upstream

TrueNorth-MCP is a hard fork. Upstream is a historical ancestor, not a live
dependency. The project does not track or merge upstream commits.

- The runtime shares no code with upstream, so there is no cherry-pick surface. A
  new upstream commit is, at most, an idea. The project reimplements a good idea in
  its own architecture and style, through its normal issue-first, spec, and pull-request
  flow. It never merges an upstream diff.
- The `skills/` directory is the one inherited surface. The runtime reads it by
  format, not by content, so a skill is data the runtime parses, not code it depends on.
- The MIT license and this credit are permanent obligations the project keeps. The
  git ancestry is honest history and stays. Tightening the fork framing corrects the
  forward expectation (no merges), not the backward fact (it started here).

Do not run `git merge upstream/main`. With the divergence between the projects, a
blind merge would reintroduce the removed `bigpowers` process and conflict across
rewritten files.

## What it is

truenorth-mcp is a Model Context Protocol server. It delivers engineering discipline
through typed MCP tools and resources, not through a large instruction file dumped
into a context window. An agent connects over stdio, calls a tool to advance a
lifecycle phase, verify a gate, or check an ontology, and reads the project cockpit
through a resource.

## Why

An agent left to its own devices drifts. It skips the plan, writes code before a test,
declares a green build it never ran, and renames a concept halfway through. A large
markdown instruction file does not stop this, because the model can read past it, and
it burns context.

truenorth-mcp moves the discipline into the protocol. A gate passes only when the
server ran the command and saw exit 0, so the model cannot fake it. A phase advances
only through a tool that records the work. The cockpit is live state the agent reads,
not prose it interprets. The result is a workflow the runtime enforces, not one the
agent is asked to remember.

## The lifecycle

The runtime drives a six-phase lifecycle. `truenorth_advance_phase` accepts only the
recorded current phase and its immediate successor, then records the artifacts each phase
produced. Integrate advances to Discover, starting the next lifecycle loop.

1. **Discover**: understand the problem and the current state.
2. **Design**: model the domain and design the solution.
3. **Plan**: break the work into tasks, each with a verify command.
4. **Execute**: build the change under a Red-Green-Refactor loop.
5. **Review**: review and harden the change against the quality bar.
6. **Integrate**: integrate the change and advance the release.

## Architecture

- **The runtime**: a Rust crate at `runtime/`. It serves the MCP tools and the
  `truenorth://` resources over stdio.
- **The wrapper**: a thin Node.js package at `npm/`. It resolves the platform binary
  and runs it. There is no business logic in the wrapper.
- **The skills**: a library under `skills/`, served through the `get_skill`,
  `index_skills`, `read_skill`, and `search_skills` tools. Each skill is
  model-agnostic and rendered at a full, reasoning, or lean tier.
- **The cockpit**: the machine-facing project state under `.agent/`, served through the
  `truenorth://state`, `truenorth://cockpit`, `truenorth://conventions`, and
  `truenorth://ontology` resources.

## The two layers

The repository has two layers. The machine-facing layer is `.agent/`. The runtime
reads, watches, and writes only under `.agent/`. A single write guard rejects any
write outside `.agent/`. The human-facing layer is `specs/`, which holds
human-authored narrative such as Architecture Decision Records. The runtime reads a
file under `specs/`, but it never writes there.

The `.agent/` tree holds the cockpit (`tasks/state.yml`, `tasks/release-plan.yml`),
the ontology (`ontology.yml`), the config (`config/rules.yml`), the product scope, and
the memories. The `truenorth://adr` resource serves `specs/adr/` read-only.

## Install

Bootstrap an empty project with the npm wrapper. It resolves the platform binary, copies its
versioned skill bundle, and creates the language-agnostic TrueNorth workspace:

```bash
npx -y truenorth-mcp init --profile generic
```

`init` refuses to overwrite `.agent/`, `specs/`, `skills/`, or its workflow files. It does
not create a language manifest, source tree, CI workflow, or verify command.

Then register the wrapper as an MCP server in your client:

```json
{
  "mcpServers": {
    "truenorth": {
      "command": "npx",
      "args": ["-y", "truenorth-mcp"],
      "env": {
        "TRUENORTH_ROOT": "/absolute/path/to/repo",
        "TRUENORTH_VERIFY_CMD": "<your-project-verify-command>"
      }
    }
  }
}
```

Run the client from the repository root instead of setting `TRUENORTH_ROOT` when that is more
convenient. Configure `TRUENORTH_VERIFY_CMD` for the project's own language and tooling.
The [install and connect guide](https://github.com/vixygrey/truenorth-mcp/wiki/Install-and-connect)
has per-client steps.

## Diagnose a setup

```bash
npx -y truenorth-mcp --version
npx -y truenorth-mcp --check-config
```

`--check-config` reports the repository root, workspace layout, enabled features,
verify-gate readiness, package version, and platform. It reads configuration only. It
does not start the MCP server, run the verify command, or print configured command values.

## Methodology profiles

A project declares one methodology profile in `.agent/profile.yml`. The profile sets the
grouping vocabulary, whether grouping is required, and the branch pattern. There are five
built-in profiles:

- **issue-per-task** (the default): optional ticket grouping.
- **epic-based**: required epic grouping.
- **milestone-based**: required milestone grouping.
- **kanban**: no grouping, continuous flow.
- **generic**: no grouping, minimal structure.

## Build from source

```bash
cd runtime
cargo build --release
```

Run the tests with `cargo test`. Run the wrapper tests with `node --test` in `npm/`.

## The tools

Active tools drive the workflow:

- `truenorth_advance_phase`, `truenorth_record_task`: drive the lifecycle.
- `truenorth_verify_gate`: run a project gate command in a sandbox, pass only on exit 0.
- `truenorth_tdd_cycle`: enforce the red-green-refactor order.
- `truenorth_scaffold_project`: seed a new project's `.agent/` tree for a methodology profile.
- `truenorth_record_bug`: record an external-tracker bug reference in `.agent/tasks/bugs.yml`.
- `truenorth_generate_ontology`, `truenorth_verify_ontology`: seed and enforce a
  domain ontology. These two are present only when the ontology feature is enabled (see below).

Catalog tools serve the skill library:

- `get_skill`, `index_skills`, `read_skill`, `search_skills`: read and search skills.
- `build_skill_graph`, `read_graph`, `search_nodes`, `open_nodes`, `get_dependencies`:
  build and query the skill graph.
- `get_git_context`, `validate_skill`: report scoped git context and lint a skill.

## The resources

- `truenorth://state`: the cockpit state (`.agent/tasks/state.yml`).
- `truenorth://cockpit`: the release plan (`.agent/tasks/release-plan.yml`).
- `truenorth://conventions`: the engineering conventions (`CONVENTIONS.md`).
- `truenorth://ontology`: the domain ontology (`.agent/ontology.yml`), present only when
  the ontology feature is enabled.
- `truenorth://adr`: the Architecture Decision Records under `specs/adr/`, read-only.

A resource reflects the current on-disk content. A disk edit emits a
`notifications/resources/updated` for the affected resource.

## The ontology feature

The ontology surface is a per-project choice. Set `features.ontology` in
`.agent/config/rules.yml`. The default is enabled. When it is disabled, the two ontology
tools and the `truenorth://ontology` resource are absent.

## Support

TrueNorth-MCP v1 supports the npm wrapper on macOS ARM64/x64 and Linux ARM64/x64.
Windows is unsupported. The wrapper requires Node.js 18 or newer. Source builds require
Rust 1.88 or newer. The server supports MCP over stdio; vendor-specific MCP clients are
not separately certified.

Use [GitHub Discussions](https://github.com/vixygrey/truenorth-mcp/discussions) for
questions and the issue forms for reproducible non-security defects. Report vulnerabilities
privately through the [security policy](SECURITY.md).

## Documentation

- The [landing page](https://vixygrey.github.io/truenorth-mcp/) presents the project.
- The [Wiki](https://github.com/vixygrey/truenorth-mcp/wiki/Home) is the full usage
  manual: install and connect, the `.agent/` workspace, the lifecycle, the resources,
  the ontology feature, and troubleshooting.
- The [compatibility policy](https://github.com/vixygrey/truenorth-mcp/wiki/Compatibility)
  defines the v1 workspace, MCP, migration, and upgrade contract.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow, and
[CONVENTIONS.md](CONVENTIONS.md) for the engineering conventions.

## License

MIT. See [LICENSE](LICENSE). Third-party attributions are in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
