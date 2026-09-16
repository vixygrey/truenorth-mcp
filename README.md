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
- **The cockpit**: the project state under `specs/`, served through the
  `truenorth://state`, `truenorth://cockpit`, `truenorth://conventions`, and
  `truenorth://ontology` resources.

## Install

Install the npm wrapper. It resolves the platform binary and runs the server. The
`init` subcommand scaffolds the `specs/` directory when it is absent.

## Build from source

```bash
cd runtime
cargo build --release
```

Run the tests with `cargo test`. Run the wrapper tests with `node --test` in `npm/`.

## The tools

- `truenorth_advance_phase`, `truenorth_record_task`: drive the lifecycle.
- `truenorth_verify_gate`: run a project gate command in a sandbox, pass only on exit 0.
- `truenorth_generate_ontology`, `truenorth_verify_ontology`: seed and enforce a
  domain ontology.
- `truenorth_tdd_cycle`: enforce the red-green-refactor order.
- `get_skill`, `index_skills`, `read_skill`, `search_skills`: serve the skill
  catalog.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow, and
[CONVENTIONS.md](CONVENTIONS.md) for the engineering conventions.

## License

MIT. See [LICENSE](LICENSE). Third-party attributions are in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
