# Docs Mode: Full Process

Use docs mode when the user asks to “grill me with docs” or when a plan depends on a
specific external library, framework, or API.

The purpose is to prevent invented API methods, signatures, versions, and behavior.
Validate every external factual assumption before code is written.

## Step 1: Identify dependencies

From the plan and repository, list:

- Every external library being used
- Every third-party API being called
- Every framework behavior being relied upon
- The installed or targeted version of each dependency

Do not ask the user to confirm facts that the repository or official documentation can
establish.

## Step 2: Fetch official documentation

For each dependency, use official, version-matched sources. Prioritize:

1. The official API reference for the exact method
2. Versioned documentation and changelogs
3. Official migration guides
4. Official limits, error, and operational guidance

A blog post, generated summary, or search-result excerpt is insufficient when an
official source exists.

## Step 3: Challenge each assumption

For every external factual assumption, capture:

- The plan claim
- The official source URL
- A quoted API detail: signature, parameter, version, deprecation, quota, timeout, or
  documented error
- Whether the source confirms or contradicts the claim

Ask one decision question at a time only when the evidence exposes a genuine tradeoff.
Include a recommended answer.

## Step 4: Surface discrepancies

When a plan assumption does not match the documentation, state the mismatch precisely:

> The plan uses `library.doThing(a, b)`. The
> [official API reference](https://example.com/api-reference) says
> “`doThing(config: {a, b})`,” so the positional call will fail.

Do not soften a documented contradiction into an optional suggestion.

## Step 5: Correct the plan

For each confirmed discrepancy, recommend and apply the correction after the user
confirms the resulting decision. Corrections can include:

- Method signature or argument order
- Supported version or replacement for a deprecated API
- Rate-limit, timeout, retry, or documented error handling
- An alternative approach supported by the official API

Route a fact that official sources cannot settle to `spike-prototype`.

## Step 6: Report and confirm

Use this result format:

| Assumption     | Official source | Quoted detail             | Verdict                             | Plan correction    |
| -------------- | --------------- | ------------------------- | ----------------------------------- | ------------------ |
| `<plan claim>` | `<URL>`         | `<exact relevant detail>` | confirmed, corrected, or unresolved | `<change or none>` |

Report which assumptions were confirmed, corrected, or remain unresolved. The
confirmation gate in `SKILL.md` still applies before implementation, specification
writing, or task slicing.

## Question templates

- “The official docs at [URL] show signature `foo(bar?: Baz)`, while the plan calls
  `foo(bar, baz)`. Which supported design should replace the plan call?”
- “The official changelog at [URL] says ‘X is deprecated in v3.’ Should the plan migrate
  to the documented replacement or pin the supported older version?”
- “The official error reference at [URL] says ‘throws `NetworkError`.’ Which documented
  recovery behavior should the plan adopt?”
