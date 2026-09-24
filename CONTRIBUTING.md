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

Install the root development dependencies once, then run the full local
verification:

```bash
npm install --no-audit --no-fund
npm run preflight
```

`npm run preflight` runs the same five ownership groups as the required CI gate:
`check`, `wrapper`, `artifact-smoke`, `format`, and `shell`. Run one or more
groups directly for a focused rerun:

```bash
bash scripts/preflight.sh check
bash scripts/preflight.sh wrapper artifact-smoke
```

The command checks the default and `tree-sitter` Rust builds, wrapper tests,
native and packed artifacts, formatting, documentation, shell scripts, skill
paths, handoffs, and script-bearing skills. It reports missing prerequisites and
unsupported artifact platforms with remediation. Browser-bound skill checks are
reported as skips when their optional dependencies are unavailable.

The preflight never publishes, tags, or uses production credentials. Release
rehearsal and cross-platform publication remain CI release responsibilities.

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

Before pushing `v<version>`, update those files and add
`release-notes/v<version>.md`. The note needs non-empty `## Highlights` and
`## Migration` sections. A breaking release must state command-first migration and
rollback steps.

Run:

```bash
node --test npm/test/release-version.test.js
node --test scripts/release-record.test.js
node scripts/check-release-version.js <version>
```

The release workflow verifies committed metadata and release notes, builds each native target,
and smoke tests both the staged executable and a clean installation of the packed wrapper. It
then stages the npm packages after `release-publication` Environment approval, and publishes a
GitHub Release with native archives, `SHA256SUMS`, and `RELEASE-VERIFICATION.json`.

Staged npm packages become public only after a maintainer's 2FA approval. Run the protected
`Finalize release record` workflow after that approval. It verifies every package's registry
integrity metadata and attaches `NPM-VERIFICATION.json` to the GitHub Release. A protected
`v*` tag must name the current `main` commit.
