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

## Before you push

Run the full local verification.

```bash
cd truenorth-mcp
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

For a wrapper change, also run `node --test` in `npm/`.

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

## Releases

A release is tag-driven. A `v*` tag triggers the release workflow, which
cross-compiles the native targets, publishes the per-platform packages, then the
root wrapper.
