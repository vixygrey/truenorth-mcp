# Commit and PR message guide

This project uses atomic, Conventional Commits. This guide gives the format, the
types, and the scopes for truenorth-mcp. It also states how a commit type maps to
the SemVer bump.

All commit and PR prose follows the house writing rules in
`.kiro/steering/writing-rules.md`: one word one meaning, active voice, short
sentences, approved modals (`can`, `will`, `must`), American English, and no em
dashes.

## Format

```text
<type>(<scope>): <description>

[body]

[footer]
```

- The subject is imperative and describes the change. Keep it under 72 characters.
- The body states plain past facts and explains why, not what. Wrap at 72 characters.
- The footer carries `Refs #<n>`, `Closes #<n>`, or a `BREAKING CHANGE:` note.

## Atomic commits

- One commit is one logical, self-contained change that leaves the tree working:
  it compiles and the tests pass.
- Do not mix unrelated changes. Split a large change into a sequence of atomic
  commits. Do not bundle a refactor with a behavior change.
- One commit is one type. When a change needs two types, it needs two commits.

## Types and SemVer

| Type       | Meaning                                      | SemVer |
| ---------- | -------------------------------------------- | ------ |
| `feat`     | A new capability                             | minor  |
| `fix`      | A bug fix                                    | patch  |
| `docs`     | Documentation only                           | none   |
| `style`    | Formatting only, no logic change             | none   |
| `refactor` | A code change that is not a feature or a fix | none   |
| `perf`     | A performance change                         | patch  |
| `test`     | Tests only                                   | none   |
| `chore`    | Maintenance, build, or tooling               | none   |

A `!` after the scope or a `BREAKING CHANGE:` footer forces a major bump, whatever
the type.

Note: SemVer maps from the commit type, but the release itself is tag-driven. The
`release.yml` workflow runs on a `v*` tag. It does not parse commits to
compute the version.

## Scopes

Use a runtime domain as the scope:

- `engine` — the analysis and transform core (gate runner, ontology, tier, git).
- `tools` — the MCP tools (`truenorth_*` and the ported catalog tools).
- `resources` — the `truenorth://` resource layer.
- `server` — the server wiring, the handler, and the watcher.
- `cli` — the runtime binary entry point.
- `wrapper` — the npm distribution wrapper under `npm/`.
- `distribution` — packaging and the release workflow.
- `ci` — the CI workflows.
- `spec` — the spec and steering files under `.kiro/`.

Be specific. Write `fix(engine)`, not `fix(core)`.

## Examples

A feature (0.1.0 to 0.2.0):

```text
feat(tools): add truenorth_verify_ontology

Scan the changed files against the ontology baseline. Return the first violation
with its constraint id and a remediation hint.

Closes #22
```

A fix (0.1.0 to 0.1.1):

```text
fix(resources): retain the last good cache on a parse failure

A malformed cockpit file returned a read error and dropped the cache. Keep the
last good content and keep serving the other resources.

Refs #24
```

A breaking change (0.1.0 to 1.0.0):

```text
feat(tools)!: rename the phase-advance tool

BREAKING CHANGE: `truenorth_advance` is now `truenorth_advance_phase`. Update the
client tool name.
```

## Pull requests

- One PR is one squash-merge commit on `main`. The squash subject and body follow
  this guide, because that message is the permanent history entry.
- Keep the PR title under 70 characters. Use the body for detail.
- Structure the body as a summary of the change, what you tested, and any risk.
- Do not add AI-attribution footers. The human author owns the commit.
