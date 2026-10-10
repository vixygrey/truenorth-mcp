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
3. **Correctness is executable.** Pin load-bearing behavior with the
   [current design's correctness properties](specs/design.md#correctness-properties)
   (P1 to P21) and a test. Prefer a property or golden test.
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
- Each wrapper release regenerates `npm/bundle/current.json`, retains every supported historical
  manifest under `npm/bundle/history/`, and keeps the bundle, wrapper, native package, Cargo
  manifest, and lockfile versions aligned. `node scripts/build-bundle-manifest.js --check` is a
  release gate.
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
- The verify gate command is trusted operator configuration (`TRUENORTH_VERIFY_CMD`), never
  a verify-tool argument. It runs through `/bin/sh -c`, and the command allowlist gates the
  leading binary only. The bounded gate executor is not a filesystem, network, or hostile
  command containment boundary.
- A gate subprocess inherits only exact environment names from the built-in allowlist and
  `TRUENORTH_GATE_ENV_ALLOWLIST`. Reject credential-like and `TRUENORTH_*` additions. Never
  persist or return inherited values, and redact exact inherited values from stderr.
- The secret denylist is load-bearing for paths and produced content. Never read, echo, or
  write a value matching it (`.env`, `*.pem`, `secret`, `credentials`).

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

This section is the single authoritative house writing policy. It adapts
ASD-STE100 Issue 9 principles. It does not claim certification or replace technical
review. The historical source is `.kiro/steering/writing-rules.md` at Git revision
`ad34e41974a58952da2ce4a6bf17042fc14b7920`, not a required current file.

### Scope and tiers

- **Strict:** commits, pull requests, specifications, technical documentation,
  release notes, incident reports, error messages, CLI and UI text, and agent
  instructions.
- **Loose:** issues, comments, wiki pages, and conversation. Apply the mechanical
  rules and house voice below. Sentence limits and the contraction restriction do
  not apply.
- Preserve literal code, identifiers, paths, flags, protocol keywords, quotations,
  and proper names exactly. Do not change a contract to satisfy a prose rule.
- Keep recovered historical originals verbatim. Archive status prevents their old
  wording from becoming current policy.
- Safety instructions override tier relaxation.

### House voice and mechanical rules

Use calm, technical, direct language. Use American English and inclusive terms.
State observed facts, decisions, risks, and uncertainty plainly.
Remove praise, sales language, filler, canned encouragement, and unsupported
certainty. Do not use an em dash in user-facing prose.

Use one word for one meaning and one name for one concept. Define a technical noun
or verb before an unusual use. Keep its part of speech consistent.
Keep noun chains to three words. Define a shorter term for a longer chain.
Do not turn a noun into a verb merely to shorten an instruction.

Use `can`, `will`, and `must` as the house modals. Preserve literal normative
keywords from an external standard or protocol.
Do not use a semicolon in prose. Expand Latin abbreviations such as `e.g.` and
`i.e.` outside literal quotations.

Use plain replacements: `use` for `utilize`, `to` for `in order to`, `before` for
`prior to`, and `make sure that` for `ensure`. Prefer the precise function or
feature name to marketing terminology.

Use `test`, `check`, and `work` as nouns in controlled prose, not verbs.
Use `help` as a verb. Use `follow` for "come after," not "obey."
Use `above` and `below` for physical position. Cite a section or step for document
location. Literal identifiers and established technical names remain unchanged.

Markdown tables use space-padded cells and matching delimiter widths
(`| --- |`, not `|---|`). Tables inside fenced code blocks remain verbatim.

### Strict prose

- Use an imperative for a procedure. Give one instruction per sentence, with at
  most 20 words.
- Use simple present, past, or future for a description. Keep a sentence to at most
  25 words and a paragraph to one topic and six sentences.
- Prefer active voice. A descriptive passive is acceptable when the actor is
  unknown. Do not hide responsibility.
- Avoid perfect and progressive verb forms. Use an `-ing` form only as an
  established technical noun.
- Do not use contractions. Include the articles, relative pronouns, and other
  grammar needed to make the relationship explicit.
- Put a command or condition before its explanation. Separate actions that have
  different conditions.

### Safety and review

State the required action or condition before the risk.
Use `WARNING` for a risk of injury and `CAUTION` for a risk of damage.
Name the consequence and the prevention. Do not rely on color or tone alone.
The human writer reviews technical accuracy, terminology, and any justified
exception. Formatting and automated lint are not content approval.

## Output files

The repository has two layers. `.agent/` is the machine workspace.
Its guarded mutation paths stay under `.agent/`.
Human-authored narrative lives under `specs/` and remains separate from machine state.

[ADR-0019](specs/adr/0019-scoped-repository-seed-write-authority.md) defines the narrow repository-seed authority.
The MCP scaffold can create absent generated workflow files under its mutation permit.
Fresh initialization rejects its enumerated top-level set and preserves other generated path collisions.
Its empty `specs/adr/.gitkeep` marker is the only authorized `specs/` seed.
Managed upgrades can change ownership-planned generated and package-manifest paths.
They preserve local edits and unowned collisions and do not manage `specs/**`.
These paths do not authorize mutation of existing human narrative or prove blanket runtime write confinement.

The cockpit lives under `.agent/tasks/`: `state.yml` carries the phase and the TDD
loop, `release-plan.yml` carries the recorded tasks, `execution-status.yml` carries
story status, and `bugs.yml` carries external-tracker bug references. `backlog.yml`
declares backlog ownership independently from the methodology profile. In `local`
mode its `backlog` list is authoritative. In `external` mode it contains only the
provider pointer, and consumers must read current work from that tracker. The
runtime never calls an external tracker in its default request path. The empty
`execution-status.yml` seed has `stories` and `development_status` maps. The domain
ontology is `.agent/ontology.yml`.

## Accessibility

Most of the runtime is a stdio MCP server with no human interface. Accessibility
applies to the CLI, the terminal output, an error message, and any docs. Do not
encode meaning in color alone. State the problem and the fix in words. Respect the
`NO_COLOR` variable and disable color when the output is not a TTY.

## Document precedence

This file is the authoritative project engineering convention and writing policy.
It applies to every contributor and agent.

1. The reviewed current specifications under `specs/` and accepted ADRs determine
   the product contract. An explicit later accepted decision supersedes the
   historical baseline.
2. `CONVENTIONS.md` determines engineering and writing policy.
3. Formatter and linter defaults determine unspecified layout.

The [specification index](specs/README.md) records provenance and review status.
A draft awaiting human review is not an accepted override.
Recovered originals under `specs/archive/` are historical evidence, not current
authority or present verification.
Live execution status belongs to `.agent/tasks/execution-status.yml`, not a
narrative or an archived checkbox.

Contributor guides and client adapters point here. They do not define competing
writing rules or another precedence order. Kiro inclusion is an adapter, not a
required source of policy. Do not import a `bigpowers` convention by default.
