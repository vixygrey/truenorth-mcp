# Contributing to truenorth-mcp

Thank you for contributing. This guide covers the workflow. The engineering
conventions are in [CONVENTIONS.md](CONVENTIONS.md), and the commit format is in
[.github/COMMIT_TEMPLATE.md](.github/COMMIT_TEMPLATE.md).

## Workflow

This project uses an issue-first, trunk-based workflow.

1. Open or claim an issue. One PR references at least one issue.
2. Create a feature branch. Do NOT commit to `main`.
3. Make atomic commits, each one a single logical change that leaves the tree
   working.
4. Open a PR. The template guides the summary, the testing, and the correctness
   properties.
5. Squash-merge, so the PR becomes one commit on `main`.

External pull requests must also pass the required `External contribution review` check.
See [GOVERNANCE.md](GOVERNANCE.md) for merge, emergency, and release authority.

## Before you push

Run the full local verification.

```bash
cd runtime
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

For a wrapper change, also run `node --test` in `npm/`.

For a change to a shell script under `skills/`, lint it with the pinned
shellcheck the CI gate uses:

```bash
bash scripts/lint-shell.sh
```

The script reads the pinned version from `.github/workflows/ci.yml`
(`SHELLCHECK_VERSION`), so a local run matches CI. It uses a matching local
shellcheck when present, and the `koalaman/shellcheck` container otherwise. A
version bump is one line in the workflow.

## Commits

- Use atomic, Conventional Commits: `type(scope): description`.
- The scope is a runtime domain: `engine`, `tools`, `resources`, `server`, `cli`,
  `wrapper`, `distribution`, `ci`, or `spec`.
- No AI-attribution footer. The human author owns the commit.

## Tests

- Every new function gets a test. Every bug fix gets a regression test.
- A property or golden test is required for anything a correctness property (P1 to
  P5) covers.

## Writing

All prose follows the house writing rules in
`.kiro/steering/writing-rules.md`: short sentences, active voice, approved modals
(`can`, `will`, `must`), American English, and no em dash.

Markdown tables use the space-padded style: pad each cell to its column width
and pad the delimiter row to match (`| --- |`, not `|---|`). A table inside a
fenced code block stays verbatim. The repo checks in no editor config for this,
because `.vscode/` is gitignored, so keep the style by hand or through your own
formatter.

## Releases

A release is tag-driven. `runtime/Cargo.toml` is the release version source of truth. The
root npm wrapper, its optional dependencies, the four platform packages, and `Cargo.lock`
must mirror that version in the same release-preparation commit.

Before pushing `v<version>`, update those files, then run:

```bash
node --test npm/test/release-version.test.js
node scripts/check-release-version.js <version>
```

The release workflow verifies the committed metadata, builds each native target, and smoke
tests both the staged executable and a clean installation of the packed wrapper before a
`release-publication` Environment approval authorizes npm staging. It does not rewrite
package versions. A protected `v*` tag must name the current `main` commit.
