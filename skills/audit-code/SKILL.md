---
name: audit-code
description: "A self-review checklist for the coding agent to run before dispatching a reviewer. Checks convention compliance, the Boy Scout rule, test coverage, types, and SOLID. Produces a pass or fail checklist. Use it before request-review, before committing, or when the user asks for a code-quality check."
kind: prose
---

# Audit Code

> **HARD GATE**: the audit MUST check for bugs (correctness), security, performance, and clarity. Do NOT skip the security review when the code touches user data, auth, or an external API.

Run this self-review before you ask anyone else to look at the code. The goal is to
catch everything clearly wrong or missing, so the reviewer can focus on design and
architecture, not hygiene.

Distinct from `request-review`. This is the coding agent checking its own work. No
second agent is involved. Run this first. Run `request-review` after this passes.

## Look here first

Read the active project conventions. They define the enforceable style, structure,
and test rules. Then rank changed files by git churn and review high-churn
hotspots first. Churn sets priority, not scope or verdict. A file with zero recent
commits but a large diff still gets reviewed.

## Modes

- Default: the full risk-scaled checklist.
- `--quick`: use for P2 or P3 changes that do not cross a security, persistence,
  compatibility, or public API boundary. Run focused correctness, convention,
  and test-evidence checks.
- `--gate`: non-interactive mode for automated CI gating (used by build-epic).
  Exit non-zero when any concrete blocking defect or explicit convention
  violation remains unresolved. Produce a compact pass or fail summary and list
  every blocker with evidence.
- `--parallel`: run independent checklist sections in isolated git worktrees, so
  concurrent checks cannot corrupt each other's working tree.

## Checklist

### Supply chain and security

- [ ] A slopcheck ran for a new dependency. Packages tagged `[OK]`, `[SUS]`, or `[SLOP]`.
- [ ] No `[SLOP]` package without a documented human approval.
- [ ] No secret in the diff (`sk-`, `ghp_`, `AKIA`, an `.env` value). See the `guard-git` patterns.
- [ ] OWASP Top 10 spot-check: injection, broken auth, sensitive-data exposure, misconfiguration.
- [ ] The diff is scanned. No confirmed HIGH/CRITICAL blocker or undispositioned high-impact investigation item remains.

### Provenance and metadata

- [ ] A new plan artifact includes its `type:` and `context:` metadata.
- [ ] An implementation step references the ADR or commit SHA where the decision was made.

### Law of Demeter

- [ ] No method chain through an unrelated object (`a.getB().getC().doX()`).
- [ ] A collaborator talks to its immediate neighbor only. A violation needs an explicit justification.

### Convention compliance

- [ ] Runtime state is under `.agent/`. Narrative is under `specs/`. No doc written to the project root.
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

- [ ] Types and public boundaries follow the active project conventions.
- [ ] No suppression, cast, or unchecked value hides a concrete safety defect.

### Test evidence

- [ ] The changed behavior has the tests or smoke evidence required by the active
      project conventions and risk tier.
- [ ] Every bug fix has regression evidence.
- [ ] Tests verify behavior through the public interface, not an implementation
      detail.
- [ ] Tests follow the project's quality rules. Use `enforce-first` when the
      project adopts the F.I.R.S.T rubric.

### Design and maintainability

- [ ] No concrete correctness, performance, security, or maintainability defect
      is hidden by unnecessary complexity.
- [ ] Public boundaries and dependency direction follow the active project
      conventions.
- [ ] The code is free of applicable smells documented in
      [HEURISTICS.md](HEURISTICS.md).

### Refactoring smells

Explicitly name any detected smell: a mysterious name, duplicated code, feature
envy, a data clump, primitive obsession, a message chain, a middle man.

### Code style

- [ ] Formatting, naming, file organization, abstraction, control flow, and
      comments comply with the active project conventions.
- [ ] A size or complexity concern is reported only when it violates an explicit
      convention or causes a concrete defect. A heuristic alone does not fail the
      audit.
- [ ] Duplicated logic is reported when it creates a demonstrated consistency or
      maintenance risk, not merely because similar text exists.

### Red flags

Before you report, name any rationalization you caught yourself making for skipping
a checklist item. Silence is not acceptable. When you skip an item, state the reason.

## Output

Report pass or fail for each applicable section. Every failure must include the
file and location, the concrete consequence or violated project convention, the
supporting evidence, and a remediation. A preference without a defect or
convention violation is advisory and cannot block.

When no blocking defect remains, suggest running `request-review`. When a blocker
exists, fix it before proceeding.

In `--gate` mode, print one summary line per section, exit non-zero only while a
blocking defect remains, and write the full report for the story.

## Verify

Confirm the project conventions are present and the sibling skills (enforce-first,
request-review) exist.

## Handoff

Gate: READY. Next: commit-message.
Writes: `state.yaml` `handoff.next_skill = commit-message`.
