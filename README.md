# truenorth-mcp

An active, protocol-first MCP execution runtime for spec-driven engineering
discipline. Written in Rust, distributed as a thin Node.js wrapper. Token-lean and
model and harness agnostic.

## What it is

truenorth-mcp is a Model Context Protocol server. It delivers engineering discipline
through typed MCP tools and resources, not through a large instruction file dumped
into a context window. An agent connects over stdio, calls a tool to advance a
lifecycle phase, verify a gate, or check an ontology, and reads the project cockpit
through a resource.

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

Install the npm wrapper. It resolves the platform binary and runs the server. Scaffold
a new project by calling the `truenorth_scaffold_project` tool from your MCP client,
which seeds the `.agent/` tree for a methodology profile.

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

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow, and
[CONVENTIONS.md](CONVENTIONS.md) for the engineering conventions.

## License

MIT. See [LICENSE](LICENSE). Third-party attributions are in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
