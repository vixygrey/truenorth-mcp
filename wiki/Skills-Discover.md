# Skills: Discover

The Discover phase answers "where am I, and what already exists?" before any design or
code. The skills here bootstrap context, find prior art, shape a rough idea into a spec,
and route you to the right next skill.

For the full arc across phases, see [The skill workflow](The-skill-workflow). For the
alphabetical list, see [Skill index](Skill-index).

---

## using-truenorth

The one-time bootstrap. It introduces the skills system, the lifecycle arc, and tells you
which skill to call first for your situation.

- **What it does**: explains the methodology, the six-phase lifecycle, the cockpit under
  `.agent/`, and the key conventions. It is the entry point for a new user or a new
  session.
- **When to use it**: the first time, when a user asks "where do I start?", or when the
  skills system needs explaining.
- **Inputs**: none. It reads no project state.
- **Outputs**: orientation, plus a routing table from your situation to a first skill.
- **Hard gate**: do not skip it when onboarding a new user or starting a new session.
- **Handoff**: call `survey-context` next, to read the project state and get a
  recommendation.

## survey-context

The per-task context bootstrap. It reads the current state and gives a phase map plus a
next-skill recommendation. This is the "where am I?" skill.

- **What it does**: reads the conventions, the cockpit under `.agent/` (state, release
  plan, execution status), the product docs, the agent guide, and the VCS state. It maps
  the current lifecycle phase and recommends the next skill. It records a story-start
  timestamp as an informational marker.
- **When to use it**: at the start of any task, when returning after a break, or when
  unsure what to do next.
- **Inputs**: the existing narrative docs and the project tech-stack note. It does not
  re-derive them.
- **Outputs**: a phase map, a named next-skill recommendation, and a blocker report.
- **Hard gate**: read the cockpit and narrative files before suggesting a step. When the
  state is stale or contradicts the code, ask rather than assume.
- **Handoff**: gate READY, next `plan-work`, writing `handoff.next_skill`. In practice it
  routes to the phase-appropriate skill (for example `kickoff-branch` on `main`,
  `build-epic` mid-build, `verify-work` after implementation).
- **Related**: for deriving a tech-stack note from scratch, run `map-codebase` first.

## map-codebase

Derive the tech-stack note from scratch by scanning the codebase. Where `survey-context`
identifies "where we are", `map-codebase` identifies "what we are dealing with".

- **What it does**: scans the dependency manifests, maps the architecture and data flow,
  analyzes the gray areas (error handling, API shapes, type safety, observability,
  testing), identifies planning signals (consistency gaps, debt hotspots, integration
  points), and persists the findings to the tech-stack note.
- **When to use it**: when first joining a project, before a major refactor, or when
  `survey-context` reveals a lack of domain knowledge and the tech-stack note does not
  exist yet.
- **Inputs**: the codebase itself.
- **Outputs**: the project tech-stack note (the project long-term memory).
- **Hard gate**: cold analysis only. Do not assume a pattern without reading the code.
- **Handoff**: once the tech-stack note exists, `survey-context` consumes it.

## research-first

Look before you build. Search for prior art before implementing.

- **What it does**: reads the scope and current task, then searches in order (this repo,
  the skill catalog, the package registries, the web docs), reads any locally cached
  source for API shapes, and appends a prior-art section to the scope or the active story.
- **When to use it**: after `survey-context` and before `elaborate-spec`, when adding a
  dependency, or when the task may already be solved.
- **Inputs**: the product scope, the release plan and task groups, the current task
  statement.
- **Outputs**: a prior-art section with a verdict per candidate.
- **The verdict matrix**: adopt (use as-is), extend (wrap or configure), compose (chain
  existing skills or modules), or build (new code, justified).
- **Hard gate**: do not implement until you search. The minimum outcome is one of adopt,
  extend, compose, or build, with evidence.
- **Handoff**: feeds the prior-art into `elaborate-spec`.

## search-skills

Find the right skill from a natural-language intent using the catalog search.

- **What it does**: calls the `search_skills` tool (a lexical match over each skill name
  and description), ranks the top matches by exactness, phase fit, and trigger phrase, and
  recommends the single best skill.
- **When to use it**: when unsure which skill to invoke, at the start of `research-first`,
  or when a user asks "is there a skill for X?".
- **Inputs**: a natural-language intent.
- **Outputs**: one recommended skill with the reason and what it produces.
- **Hard gate**: rank the results by relevance. Do not use an external embedding API or
  AI-based semantic search. The catalog search is lexical, with zero external dependency.
- **Why lexical, not semantic**: zero network dependency, zero cost, instant,
  deterministic, and auditable.

## elaborate-spec

Refine a rough idea into a clear specification through dialogue. No code is written.

- **What it does**: listens to the idea, asks clarifying questions one at a time (problem
  clarity, solution boundaries, success criteria, constraints), surfaces hidden
  assumptions, synthesizes a summary aligned with the countable-story format, and writes
  `.agent/tasks/planning-context.yml`.
- **When to use it**: when the user has a vague idea, wants to think through a feature
  before planning, or needs to turn "I want X" into a concrete spec.
- **Inputs**: the user's rough idea, and the prior-art from `research-first` when present.
- **Outputs**: `.agent/tasks/planning-context.yml`, consumed by `scope-work` and
  `slice-tasks`.
- **Hard gate**: do not proceed to planning or code until the problem space is clear.
  When the request admits two or more valid interpretations, list them and ask the user to
  choose rather than guess.
- **Handoff**: to `model-domain` when the domain needs work, `plan-release` then
  `plan-work` when ready to plan, `spike-prototype` when a spike is needed first, or
  `deepen-architecture` / `grill-me` for architecture decisions.
