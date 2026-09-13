---
name: audit-code
description: 'A self-review checklist for the coding agent to run before dispatching a reviewer. Checks convention compliance, the Boy Scout rule, test coverage, types, and SOLID. Produces a pass or fail checklist. Use it before request-review, before committing, or when the user asks for a code-quality check.'
---

# Audit Code

> **HARD GATE**: the audit MUST check for bugs (correctness), security, performance, and clarity. Do NOT skip the security review when the code touches user data, auth, or an external API.

Run this self-review before you ask anyone else to look at the code. The goal is to
catch everything clearly wrong or missing, so the reviewer can focus on design and
architecture, not hygiene.

Distinct from `request-review`. This is the coding agent checking its own work. No
second agent is involved. Run this first. Run `request-review` after this passes.

## Look here first (a churn heuristic)

Before the checklist, rank the changed files by git churn and review the high-churn
hotspots first. They carry the most latent risk regardless of diff size. Use the
git-context tool or `git log` to rank the recently changed files. A file with zero
recent commits but a large diff still gets reviewed. Churn sets the priority, not the
scope.

## Modes

- Default: the full checklist.
- `--quick`: run only the supply-chain and test-coverage sections. Use it for a
  change under 50 lines.
- `--gate`: non-interactive mode for automated CI gating (used by build-epic).
  Exit non-zero on any checklist failure, exit 0 only when every item passes.
  Produce a compact pass or fail summary. On failure, list every failed item with a
  reason.
- `--parallel`: run the checklist sections in isolated git worktrees, so concurrent
  checks cannot corrupt each other's working tree.

## Checklist

### Supply chain and security

- [ ] A slopcheck ran for a new dependency. Packages tagged `[OK]`, `[SUS]`, or `[SLOP]`.
- [ ] No `[SLOP]` package without a documented human approval.
- [ ] No secret in the diff (`sk-`, `ghp_`, `AKIA`, an `.env` value). See the `guard-git` patterns.
- [ ] OWASP Top 10 spot-check: injection, broken auth, sensitive-data exposure, misconfiguration.
- [ ] The diff is scanned. No unaddressed HIGH finding, or a documented deviation.

### Provenance and metadata

- [ ] A new plan artifact includes its `type:` and `context:` metadata.
- [ ] An implementation step references the ADR or commit SHA where the decision was made.

### Law of Demeter

- [ ] No method chain through an unrelated object (`a.getB().getC().doX()`).
- [ ] A collaborator talks to its immediate neighbor only. A violation needs an explicit justification.

### Convention compliance

- [ ] Every output file is in `specs/`, no doc written to the project root.
- [ ] No `gh issue create` call in a new or modified skill or script.
- [ ] `gh` used only for a PR or a repo clone.
- [ ] No direct GitHub REST API call.

### Scope

- [ ] The changes are limited to what was asked. Nothing extra refactored.
- [ ] No speculative feature added.
- [ ] No file touched outside the stated scope.
- [ ] Discovered defect: a reproducible gate failure needs fix-or-log (`quick-fix` or `fix-bug`), even when outside the story scope. Scope minimization does not waive Always Green.

### Boy Scout rule

- [ ] Every file you touched is cleaner than you found it.
- [ ] No dead code left behind.
- [ ] No commented-out code block.

### Types and safety

- [ ] No `any` type introduced, and no untyped public function.
- [ ] No type-ignore or lint-disable added.
- [ ] No cast that bypasses type safety.

### Test coverage

- [ ] Every new function has at least one test.
- [ ] Every bug fix has a regression test.
- [ ] The tests verify behavior through the public interface, not an implementation detail.
- [ ] The tests are F.I.R.S.T compliant. Use `enforce-first` when unsure.

### SOLID and heuristics

- [ ] Single responsibility: no function or module doing two unrelated things.
- [ ] Open/closed: extended through an interface, not by modifying stable code.
- [ ] Dependency inversion: a dependency is injected, not imported globally where avoidable.
- [ ] The code is free of the smells documented in [HEURISTICS.md](HEURISTICS.md).

### Refactoring smells

Explicitly name any detected smell: a mysterious name, duplicated code, feature
envy, a data clump, primitive obsession, a message chain, a middle man.

### Code style

- [ ] Functions are 4 to 20 lines. Split a longer one.
- [ ] A function descends exactly one level of abstraction.
- [ ] Files are under 300 lines.
- [ ] Names are specific and unique.
- [ ] No duplication. Shared logic is extracted.
- [ ] Early returns over nested ifs. At most two levels of indentation.
- [ ] A conditional is expressed as a positive.
- [ ] A comment explains why, not what.

### Red flags

Before you report, name any rationalization you caught yourself making for skipping
a checklist item. Silence is not acceptable. When you skip an item, state the reason.

## Output

Report the checklist with a pass or fail mark per item. For each fail, describe what
needs to change. When every item passes, suggest running `request-review`. When any
item fails, fix it before proceeding.

In `--gate` mode, print one summary line per section, exit 0 only when all pass, and
write the full report for the story.

## Verify

Confirm the project conventions are present and the sibling skills (enforce-first,
request-review) exist.

## Handoff

Gate: READY. Next: commit-message.
Writes: `state.yaml` `handoff.next_skill = commit-message`.
