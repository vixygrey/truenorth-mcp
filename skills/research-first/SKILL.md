---
name: research-first
description: 'Look before you build. Search the registries, the repo, the existing skills, and the web for prior art before implementing. Appends prior art to the spec. Use it after survey-context and before elaborate-spec, when adding a dependency, or when the task may already be solved.'
---

# Research First

> **HARD GATE**: Do NOT implement until you search for prior art. The minimum outcome is one of adopt, extend, compose, or build, with evidence.

## Process

1. Read the product scope, the release plan and epic capsules, and the current task
   statement.
2. Search in order: this repo, then the skill catalog with the `search_skills`
   tool, then the package registries, then the web docs.
3. Check for local source: when the task integrates an external library, find any
   locally-cached source and read it for the API shapes before writing integration
   code.
4. For each candidate, note the name, the URL or path, and the fit (adopt, extend,
   compose, or build).
5. Append a prior-art section to the product-scope notes or the active epic story.

## Outcome matrix

| Verdict     | Action                                         |
| ----------- | ---------------------------------------------- |
| **adopt**   | Use as-is; link in plan; no new code           |
| **extend**  | Wrap or configure existing solution            |
| **compose** | Chain existing skills/modules                  |
| **build**   | New implementation — justify why others failed |

## Verify

→ verify: `grep -rq 'Prior Art' specs/product specs/release-plan.yaml specs/epics 2>/dev/null`

See [REFERENCE.md](REFERENCE.md) for search commands and registry checklist.
