---
name: context7-mcp
description: 'Fetch current library docs through the Context7 MCP server instead of training data. Use it when the user asks about a framework, an API, setup, or a code example for a specific library.'
---

# Context7 MCP

> **HARD GATE**: at most 3 Context7 tool calls per user question (`resolve-library-id` plus `query-docs` count toward the cap). On a quota or rate-limit error, emit an explicit CONTEXT7_UNAVAILABLE block. Do NOT silently answer from training data.
>
> **HARD GATE**: before an HTTP fetch, check the doc-fetch cache for the library and query. A cache hit within the TTL uses the cached body with no round-trip. On an ETag mismatch after a conditional refresh, replace the cache entry.

## When to use

- A setup or configuration question.
- Code involving a library.
- An API reference.
- The user names a specific framework.

## Bounded retry (at most 3)

| Attempt | Action                                                 |
| ------- | ------------------------------------------------------ |
| 1       | `resolve-library-id`, pick the best match              |
| 2       | `query-docs` with the selected library id              |
| 3       | Retry `query-docs` once with a refined, narrower query |

After 3 failures, stop and print:

```text
CONTEXT7_UNAVAILABLE
Reason: <quota|rate-limit|no-match|timeout>
Action: Ask the user to retry later, or paste the official docs URL.
Do NOT substitute a training-data answer without labeling it UNVERIFIED.
```

## Fetch cache

1. The cache key is the library id and the normalized query (lowercase, trimmed).
2. Read the cache. A hit uses the cached body.
3. On a miss or a stale entry, call `query-docs` and store the result.
4. The default TTL is 300 seconds. A stale entry refreshes on the next fetch. Honor
   the `ETag` when the server returns one.

## Process

1. `resolve-library-id` with the library name and the full user query.
2. Select the match by name similarity, reputation, and version. Prefer a
   version-specific id when the user names a version.
3. `query-docs` with the library id and a specific question, one concept per call.
4. Answer using the fetched docs. Cite the library and version when relevant.

## Verify

Confirm the answer cites fetched docs, or that a CONTEXT7_UNAVAILABLE block was
emitted when the server was unreachable.
