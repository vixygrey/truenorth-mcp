---
name: using-truenorth
description: 'A one-time bootstrap that introduces the skills system, the lifecycle arc, and tells you which skill to call first for your situation. Use it when starting for the first time, when the user asks "where do I start?", or when the skills system needs to be explained.'
---

# Using truenorth

> **HARD GATE**: this skill is the entry point. Do NOT skip it when onboarding a new user or starting a new session. It establishes the methodology, the lifecycle phases, and the conventions.

Welcome. This is a lifecycle of agent skills for production-ready, TDD-driven
software. Find the live catalog with the `search_skills` and `index_skills` tools.

## Install

Install the npm wrapper for the project, which resolves the platform binary and
runs the MCP server. Scaffold a new project by calling the
`truenorth_scaffold_project` tool, which seeds the `.agent/` tree for a
methodology profile. Runtime state lands under `.agent/`. Narrative lands under
`specs/`.

## What this is

A curated set of skills organized around the developer lifecycle. Each skill does
one thing. A skill references another by name only, so coupling stays low and
cohesion stays high. Runtime state lands under `.agent/`. Narrative lands under
`specs/`. The server serves each skill through the `get_skill` tool at a full,
reasoning, or lean tier.

## The lifecycle at a glance

See orchestrate-project for the canonical six-phase lifecycle.

```text
BOOTSTRAP   using-truenorth (this skill, first time only)
DISCOVER    survey-context, research-first, elaborate-spec
DESIGN      model-domain, define-language, grill-me, deepen-architecture, design-interface
PLAN        scope-work, slice-tasks, plan-work, plan-refactor
INITIATE    kickoff-branch, guard-git, hook-commits, seed-conventions
SPIKE?      spike-prototype (feeds back to plan-work)
EXECUTE     develop-tdd + enforce-first, delegate-task, dispatch-agents, execute-plan
VERIFY      run-evals, verify-work
HARDEN      wire-observability (any phase)
BUG?        investigate-bug, diagnose-root, validate-fix
REVIEW      audit-code, request-review, respond-review
INTEGRATE   commit-message, release-branch
SUSTAIN     inspect-quality, organize-workspace (ongoing)
UTILITY     terse-mode, craft-skill, edit-document (any phase)
```

## Where to start

| Your situation                       | First skill to call                     |
| ------------------------------------ | --------------------------------------- |
| A greenfield project, nothing set up | `seed-conventions`                      |
| An existing project, a new task      | `survey-context`                        |
| A vague idea that needs shaping      | `elaborate-spec`                        |
| A plan exists, ready to implement    | `kickoff-branch`, then `develop-tdd`    |
| A bug to fix                         | `investigate-bug`                       |
| Code ready for review                | `audit-code`                            |
| Shipping a feature                   | `commit-message`, then `release-branch` |

## The cockpit

The operational source of truth, served through the `truenorth://` resources:

- `.agent/tasks/state.yml`: the session, the active group and story, the handoff.
- `.agent/tasks/release-plan.yml`: the release index and the group list.
- The task groups: the stories and tasks, each with a verify command.
- `.agent/tasks/execution-status.yml`: done or pending per story.

## Key conventions

- **The workspace is your memory.** Runtime state lands under `.agent/`. Every
  domain doc, plan, and investigation narrative lands under `specs/`.
- **Integrate through the tools.** Use the git tooling and the tag-driven release.
  Never create a tracker issue from a skill. Use a local file instead.
- **One skill, one thing.** When unsure which skill to call, call `survey-context`.
  It reads the current state and recommends the next step.
- **A verify for every step.** Every group task has a runnable verify command.
  Evidence over claims.
- **Find a skill with `search_skills`.** The catalog is served live.

## After this

Call `survey-context` to read the project current state and get a recommendation for
where to go next.
