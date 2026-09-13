# truenorth-mcp Conventions

The engineering conventions for this project. The MCP server serves this file as the
`truenorth://conventions` resource, so an agent working in the repo can read it live.

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

- The runtime is a Rust crate (`truenorth-mcp/`), MCP over stdio.
- The distribution is a thin Node.js wrapper (`npm/`) that resolves the platform
  binary and runs it.
- `cargo fmt` and `cargo clippy` settle Rust style. CI denies warnings. Run both
  before every commit.

## Rust standards

- Follow the official Rust Style Guide and the Rust API Guidelines.
- `unwrap`, `expect`, and `panic!` are banned in library code. Return a typed
  `Result` and propagate with `?`.
- An error message must include the offending value, the expected shape, and an
  actionable remediation hint.
- One responsibility per function. Early returns over nesting. No magic literal, no
  dead code.
- Files under about 300 lines, split by concern.

## MCP tools and resources

- Every tool input is a `schemars`-derived struct with bounded constraints. Reject
  schema-violating input, and never perform a partial mutation on invalid input.
- A write is transactional in spirit: on a write failure, leave the target file in
  its pre-invocation state, then emit a resource-updated notification after a
  successful write.
- A parse or validation failure is not a crash. Return a resource read error, retain
  the last good content, and keep serving the other resources.
- Preserve an unknown field on read and write, for backward compatibility.

## Tests

- Tests are Fast, Independent, Repeatable, Self-validating, and Timely.
- Every new function gets a test. Every bug fix gets a regression test.
- A property or golden test is required for anything a correctness property covers.
- Test through the public interface, not private state.

## Git and commits

- Use atomic, Conventional Commits. One commit is one logical change that leaves the
  tree working.
- Squash-merge every PR, so each PR becomes one commit on `main`.
- No direct commit to `main`. Work on a feature branch.
- No AI-attribution footer. The human author owns the commit.
- See `.github/COMMIT_TEMPLATE.md` for the full format.

## Writing

All prose this project produces follows the house writing rules in
`.kiro/steering/writing-rules.md` (Simplified Technical English). One word, one
meaning. Active voice. Short sentences. Approved modals: `can`, `will`, `must`.
American English. No em dash in user-facing prose.

## Output files

All planning output goes to `specs/` at the repository root. The cockpit files are
`specs/state.yaml`, `specs/release-plan.yaml`, `specs/execution-status.yaml`, and
`specs/ontology.yaml`.
