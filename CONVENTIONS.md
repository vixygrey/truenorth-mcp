# truenorth-mcp Conventions

The engineering conventions for this project. The MCP server serves this file as the
`truenorth://conventions` resource, so an agent working in the repo can read it live.

This project is a hard fork of `bigpowers`. It does not track or merge upstream commits.
See the "Relationship to upstream" section in `README.md`.

## First principles

1. **Do it right the first time.** Follow the best practice for the language, the
   framework, and the protocol. No hack, no workaround, no shortcut that trades
   correctness for speed. Fix the root cause, not the symptom.
2. **Protocol over prose.** Deliver behavior through a typed MCP tool or a resource,
   not a longer instruction file. A new capability is a schema-backed tool, not more
   markdown.
3. **Correctness is executable.** Pin load-bearing behavior with the design's
   correctness properties (P1 to P5) and a test. Prefer a property or golden test.
4. **Deep modules, narrow interfaces.** Hide complexity behind a small public
   surface.
5. **Token economy.** Every emitted payload costs context. Be terse in output, in an
   error, and in generated content.
6. **Model and harness agnostic.** No vendor-specific scaffolding, no vendor-directed
   meta-instruction, no assumption about the calling client. Identical content across
   clients.
7. **Disk is the source of truth.** The server is a syncing peer. Read live, write
   through, notify.

## Language and tooling

- The runtime is a Rust crate (`runtime/`), MCP over stdio.
- The distribution is a thin Node.js wrapper (`npm/`) that resolves the platform
  binary and runs it.
- `cargo fmt` and `cargo clippy` settle Rust style. CI denies warnings. Run both
  before every commit.

## Rust standards

- Follow the official Rust Style Guide and the Rust API Guidelines. `cargo fmt` with
  default settings enforces the layout. Do not hand-format against it or add a custom
  `rustfmt` option.
- Types are explicit at a public boundary. Model states and enums. Avoid a
  stringly-typed API.
- `unwrap`, `expect`, and `panic!` are banned in library code. Return a typed
  `Result` and propagate with `?`.
- An error message must include the offending value, the expected shape, and an
  actionable remediation hint. Cite the constraint id where one applies (for example
  `[Ontology Gate C-02]`).
- A function name describes its side effect. A function that writes disk or emits a
  notification says so (`write_state`, `emit_resource_updated`), not `handle` or
  `process`.
- One responsibility per function. Early returns over nesting. No magic literal, no
  dead code. An `#[allow(dead_code)]` or other suppression must carry an inline reason.
- Files under about 300 lines, split by concern, not arbitrarily. A single cohesive
  concern may sit modestly over the guideline rather than be fragmented across files.
  A few modules do (for example `tools/catalog.rs`, `engine/ontology_scan.rs`,
  `engine/graph.rs`): each is one concern whose logic reads better whole.
- Use dependency injection over a global. Wrap a third-party crate behind a thin
  project-owned trait so a test can fake it.

## MCP tools and resources

- Every tool input is a `schemars`-derived struct with bounded constraints (length
  caps, enums, regex patterns). Reject schema-violating input and identify the
  offending field. Never perform a partial mutation on invalid input.
- Active runtime tools carry the `truenorth_` prefix (`truenorth_advance_phase`,
  `truenorth_verify_gate`). Ported legacy catalog tools keep their bare names for
  compatibility (`index_skills`, `get_skill`).
- Resource URIs use the `truenorth://` scheme (`truenorth://state`, `://cockpit`,
  `://conventions`, `://ontology`). A resource read reflects the current on-disk
  content.
- A write is transactional in spirit: on a write failure, leave the target file in
  its pre-invocation state, then emit a resource-updated notification after a
  successful write.
- A parse or validation failure is not a crash. Return a resource read error, retain
  the last good content, and keep serving the other resources.
- Preserve an unknown field on read and write, for backward compatibility. Map a
  legacy phase name (Build to Execute, Verify to Review, Release or Sustain to
  Integrate).
- The gate command is trusted operator configuration (`TRUENORTH_VERIFY_CMD`), never a
  tool argument. It runs through `/bin/sh -c`, and the allowlist gates the leading
  binary only, as a guardrail, not a sandbox against a hostile command. Never wire a
  caller-supplied string into the gate command.
- The secret denylist is load-bearing. Never read, echo, or write a value matching it
  (`.env`, `*.pem`, `secret`, `credentials`). Sanitize the environment before a gate
  subprocess runs.

## Tests

- Tests are Fast, Independent, Repeatable, Self-validating, and Timely.
- Every new function gets a test. Every bug fix gets a regression test.
- A property or golden test is required for anything a correctness property covers.
- Test through the public interface, not private state.

## Git and commits

- Use atomic, Conventional Commits. One commit is one logical change that leaves the
  tree working. One commit is one type. When a change needs two types, it needs two
  commits.
- `main` is the trunk and is always releasable. Work on a short-lived branch off
  `main` and merge it back fast. Do not run a long-lived divergent branch.
- No direct commit to `main`. Only commit when the user asks. Stage specific files,
  not `git add .`.
- Squash-merge every PR, so each PR becomes one commit on `main`. Do not use a merge
  commit or a rebase-merge.
- Every unit of work starts as an issue. Open the issue before you branch. Name the
  branch and PR after the issue. Put `Closes #NN` in the PR body, or `Refs #NN` when
  the PR resolves part of the issue.
- A destructive git operation (force push, `reset --hard`, `clean -f`, `branch -D`)
  needs explicit user confirmation. Preserve the hooks.
- An AI co-author footer is allowed. Use a standard `Co-authored-by:` trailer when an
  AI agent helped. The human author still owns the commit and is accountable for it.
- See `.github/COMMIT_TEMPLATE.md` for the full format.

## Writing

All prose this project produces follows the house writing rules in
`.kiro/steering/writing-rules.md` (Simplified Technical English). One word, one
meaning. Active voice. Short sentences. Approved modals: `can`, `will`, `must`.
American English. No em dash in user-facing prose.

## Output files

The repository has two layers. The machine-facing layer is `.agent/`. The runtime
reads, watches, and writes only under `.agent/`. The human-facing layer is `specs/`,
which holds human-authored narrative. The runtime can read a file under `specs/`, but
it never mutates a path under `specs/`. A single write guard rejects any target
outside `.agent/`.

The cockpit lives under `.agent/tasks/`: `state.yml` carries the phase and the TDD
loop, `release-plan.yml` carries the recorded tasks, `execution-status.yml` carries
story status, and `bugs.yml` carries external-tracker bug references. The empty
`execution-status.yml` seed has `stories` and `development_status` maps. The domain
ontology is `.agent/ontology.yml`.

## Accessibility

Most of the runtime is a stdio MCP server with no human interface. Accessibility
applies to the CLI, the terminal output, an error message, and any docs. Do not
encode meaning in color alone. State the problem and the fix in words. Respect the
`NO_COLOR` variable and disable color when the output is not a TTY.

## When in doubt

This file is the authoritative engineering convention for the project. It is
project-owned and applies to every contributor and every agent. The Kiro styleguide
at `.kiro/steering/styleguide.md` is a Kiro-only mirror. When this file conflicts
with the styleguide, this file wins.

Ground a decision in the active spec first, then this file, then the formatter and
linter defaults. Do not import a `bigpowers` convention by default.
