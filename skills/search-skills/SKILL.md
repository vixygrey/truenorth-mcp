---
name: search-skills
description: 'Find the right skill from a natural-language intent using the catalog search. Use it when unsure which skill to invoke, or at the start of research-first.'
---

# Search Skills

> **HARD GATE**: rank the search results by relevance. Do NOT return every match without prioritization. Use the skill metadata to rank.
>
> **HARD GATE**: Do NOT use an external embedding API or AI-based semantic search. The catalog search is lexical over the skill frontmatter, with zero external dependency.

Search the skill catalog lexically, with no embedding service. The
`search_skills` tool reads every skill's `name` and `description` directly from the
`skills/` directory. There is no vector DB, no API call, and no network dependency.

## When to use

- You are unsure which skill to invoke for a request.
- At the start of `research-first`, to find a pre-existing skill that solves the
  problem.
- When a user asks "is there a skill for X?".
- Before calling a skill by name, to confirm it is the right one.

## Process

1. **Search the catalog**: call the `search_skills` tool with the keywords from the
   intent. It matches the skill name, phase, and description. Use `index_skills`
   first when you want the full list.
2. **Rank the results**: read the top 3 matches. Evaluate each by exactness (does
   the description literally match the intent?), phase fit (is the skill for the
   current lifecycle phase?), and trigger phrases (does the "Use it ..." text match
   the situation?).
3. **Recommend one skill**: select the single best match. Give the skill name, why
   it is the best match (citing the description or a trigger phrase), and what it
   produces (an artifact, a dialogue, or a state change).
4. **Invoke**: call the skill directly or through the orchestrator. When no match is
   found, suggest the closest phase-appropriate skill or the general entry-point
   skill for the project.

## Why not semantic search

- Zero network dependency. It works fully offline.
- Zero cost. No API key, no usage limit.
- Instant. A lexical match over the served catalog is sub-second.
- Deterministic. The same query always returns the same result.
- Auditable. You can read the full skill set.

## Verify

Confirm the `search_skills` tool returns a ranked result for a representative query.
