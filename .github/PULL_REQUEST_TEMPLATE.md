<!--
Title: use a Conventional Commit subject under 70 characters, for example
"feat(tools): add truenorth_verify_ontology". The squash-merge uses this title as
the permanent commit on main. Follow the house writing rules: active voice, short
sentences, approved modals (can, will, must), American English, no em dashes.
-->

# Pull Request

## Summary

State what this PR changes, in one or two sentences. Write plain past facts.

## Linked issues

<!-- One PR closes or references at least one issue. This repo is issue-first. -->

Closes #

## Type

<!-- Mark the one type that matches the squash commit. One PR is one type. -->

- [ ] `feat` — a new capability (minor)
- [ ] `fix` — a bug fix (patch)
- [ ] `refactor` — no behavior change (none)
- [ ] `docs` — documentation only (none)
- [ ] `chore` / `ci` / `test` — maintenance or tests (none)
- [ ] Breaking change (major). Describe the migration below.

## Changes

<!-- List the concrete changes. One change per line. -->

-

## Testing

<!-- State what you ran and what you observed. A green gate that hides a real
problem is worse than a red one. -->

- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo test` passes
- [ ] `npm` `node --test` passes (when the wrapper changed)
- [ ] Added or updated a test for each new function and each bug fix

## Correctness properties

<!-- State the design property this change touches, or "none". -->

- [ ] P1 ontology gate
- [ ] P2 verify gate exit-0 semantics
- [ ] P3 state preservation
- [ ] P4 tier-transform invariant
- [ ] P5 no dangling references (required for any file removal)
- [ ] none

## Backward compatibility

<!-- Backward compatibility is mandatory. State the risk, or write "none". -->

## Accessibility

<!-- For CLI output, error messages, or docs. Write "not applicable" for
protocol internals and wire formats. -->

## Checklist

- [ ] The commits are atomic. Each one compiles and passes tests on its own.
- [ ] The subject and body follow the Conventional Commits format and the house
      writing rules. See `.github/COMMIT_TEMPLATE.md`.
- [ ] The branch is a feature branch. It is not a direct commit to `main`.
- [ ] No AI-attribution footers. The human author owns the commits.
- [ ] This PR is ready to squash-merge as one commit on `main`.
