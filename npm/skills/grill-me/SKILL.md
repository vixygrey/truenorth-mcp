---
name: grill-me
description: "Stress-test a plan through relentless questioning until every decision is resolved. Use context-only mode for conversation and repository evidence, or doc-grounded mode when external library, framework, or API behavior requires official-source validation."
kind: prose
---

# Grill Me

Use one of two modes. Default to **context-only mode**. Use **docs mode** when the
user asks to “grill me with docs” or when correctness depends on an external library,
framework, or API. State the selected mode before questioning begins.

> **HARD GATE**: Do NOT accept a design until every hard decision has been
> stress-tested. “Seems right” is not a decision. Grilling must identify and resolve
> tensions before build begins.

## Common process

Interview relentlessly about every aspect of the plan until reaching shared
understanding. Walk each branch of the design tree, resolving dependencies between
decisions one by one. Ask one question at a time and provide a recommended answer.

### Facts vs. decisions boundary

- **Facts** are discoverable by exploring the repository, reading official
  documentation, or checking an API. Find them without asking the user to confirm them.
- **Decisions** require user judgment about tradeoffs, preferences, or priorities.
  Present concrete options and ask the user to choose.

Never “grill yourself.” Research an answerable fact. Ask only when user judgment is
required.

## Context-only mode

Use the conversation, plan, repository, and existing local artifacts. Explore the
codebase for answerable facts. Do not fetch external documentation. Surface only the
unresolved choices that require user judgment.

## Docs mode

List every external library, API, and framework behavior that the plan relies on.
Validate each assumption against an official API reference, versioned documentation,
changelog, or migration guide.

> **HARD GATE**: Every external factual challenge must cite the official source URL
> and quote the relevant API detail, such as a method signature, parameter, supported
> version, deprecation, quota, timeout, or documented error. A blog post or search-result
> summary is insufficient when an official source exists.

For each assumption, record the plan claim, source URL, quoted detail, and a verdict:
confirmed, corrected, or unresolved. Correct the plan when official documentation
contradicts it. Route unresolved empirical uncertainty to `spike-prototype`; do not ask
the user to decide a fact.

See [REFERENCE.md](REFERENCE.md) for the full docs-mode process and result format.

## Confirmation gate

> **HARD GATE**: Do NOT enact the plan or generate specifications until the user
> explicitly confirms shared understanding. Wait for explicit approval before any
> implementation, specification-writing, or task-slicing step.
