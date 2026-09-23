---
name: grill-me
description: "Interactive assumption-surfacing Q&A that stress-tests a plan through relentless questioning until every decision is resolved. Use it to challenge a plan or validate decisions from the conversation. For the doc-grounded variant, use grill-with-docs."
kind: prose
---

# Grill Me

> **Use this vs grill-with-docs:** `grill-me` surfaces assumptions from the conversation and context alone — no documentation fetching. Use `grill-with-docs` (the doc-grounded variant) when the plan relies on a specific library or external API and every challenge must cite a real doc URL.

This skill surfaces assumptions from the conversation and the codebase, with no
documentation fetching. When the plan relies on a specific library or external API, use
`grill-with-docs` instead, so every challenge cites a real documentation URL.

> **HARD GATE** — Do NOT accept a design until every hard decision has been stress-tested. "Seems right" is not a decision. Grilling must identify and resolve tensions before build begins.

## Design

Interview relentlessly about every aspect of this plan until reaching shared understanding. Walk each branch of the design tree, resolving dependencies between decisions one-by-one. For each question, provide your recommended answer. Ask one question at a time.

If a question can be answered by exploring the codebase, explore it instead.

### Facts vs. Decisions Boundary

Distinguish between **facts** and **decisions**:

- **Facts** — things discoverable by exploring the codebase, reading docs, or checking APIs. Do not ask the user to confirm facts; find them yourself.
- **Decisions** — choices that require user input (trade-offs, preferences, priorities). Always present options and ask the user to choose.

Never "grill yourself" — if the answer is in the code, go find it. Only ask questions where the user's judgment is needed.

## The doc-grounded variant

When the plan depends on a specific library or external API, run `grill-with-docs`. It
grounds every challenge in a fetched, cited documentation URL. Do not duplicate that
process here.

## Confirmation Gate

> **HARD GATE** — Do NOT enact the plan or generate specifications until the user explicitly confirms shared understanding. Wait for explicit approval (e.g., "looks good", "confirmed", "proceed") before transitioning to any implementation, spec-writing, or task-slicing step.
