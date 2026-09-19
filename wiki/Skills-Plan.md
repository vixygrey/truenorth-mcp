# Skills: Plan

The Plan phase turns a scoped idea into a sequenced, verifiable set of stories and tasks.
The core is the three-step planning spine: `scope-work`, then `slice-tasks`, then
`plan-work`. The other skills seed conventions, sequence a release, plan the tests, assess
impact, and handle mid-flight changes.

For the full arc across phases, see [The skill workflow](The-skill-workflow). For the
alphabetical list, see [Skill index](Skill-index).

---

## The planning spine

Three skills run in order. Each is explicit that it is not a substitute for the others.

### scope-work (spine step 1 of 3)

Define what is in and out of scope, and save the bounded PRD.

- **What it does**: reads the planning context and vision, interviews if needed, and writes
  `.agent/product/scope.yml` with the core value, summary, in-scope and out-of-scope items,
  constraints, success criteria, and references. It notes why each out-of-scope item is
  excluded.
- **When to use it**: before `slice-tasks` or `plan-release` on a new initiative.
- **Inputs**: the planning context from `elaborate-spec`, the vision, the tech-stack note.
- **Outputs**: `.agent/product/scope.yml`.
- **Hard gate**: every in-scope item must map to a future group or story, or an explicit
  deferral. Scope is what and why, not how; keep implementation detail out.
- **Handoff**: `slice-tasks`, or `plan-release`.

### slice-tasks (spine step 2 of 3)

Break the scoped PRD into vertical-slice stories, each independently deliverable.

- **What it does**: cuts tracer-bullet slices (the thinnest end-to-end path that delivers
  value), assigns Business Complexity Points (1 to 13), writes per-story
  `eNNsYY-tasks.yaml` files with runnable verify commands, updates the group manifest, and
  orders by WSJF.
- **When to use it**: after `scope-work`, before `plan-work`.
- **Inputs**: `.agent/product/scope.yml` and the release plan.
- **Outputs**: per-story task files and updated group manifests.
- **Hard gate**: no horizontal-only slices. Every story must be independently
  demonstrable. Every task verify must be a runnable command, not "manually check".
- **Handoff**: `plan-work`, or `plan-tests` first for a P0/P1 group.

### plan-work (spine step 3 of 3)

Write the detailed implementation plan into the active task group.

- **What it does**: explores the affected modules, drafts the smallest possible steps (each
  leaves the code working and has one verifiable command), and writes a countable-story
  spec plus a tasks file. It applies the zoom-out mandate, requirement delta tags, a
  slopcheck for external packages, and a cross-artifact consistency pass.
- **When to use it**: after `slice-tasks`.
- **Inputs**: the release plan, the scope, the active group, the tech stack, the glossary.
- **Outputs**: a story spec and a tasks file, every task starting `status: failing`.
- **Hard gate**: do not proceed until success criteria are clear. Every task ships a
  runnable verify. A CRITICAL or HIGH consistency finding blocks code generation.
- **Modes**: default (full), or `--fast` (skip the zoom-out and impact assessment for a
  small task).
- **Handoff**: gate READY, next `kickoff-branch`, then `build-epic`, `execute-plan`, or
  `develop-tdd`.

---

## Sequencing and intake

### plan-release

A release-index builder. Sequence elaborated task groups into the release plan with WSJF
ordering.

- **What it does**: synthesizes the conversation into `.agent/tasks/release-plan.yml`
  (the WSJF-ordered group index) and shards detail into each task group (group manifest,
  countable-story specs, decoupled task files, execution status). It boosts WSJF for a
  HIGH or CRITICAL security group.
- **When to use it**: after `elaborate-spec`, when the user wants a versioned release index
  of task groups. It is not a planning-spine substitute: it does not scope work or write
  story tasks.
- **Inputs**: the elaborated spec and `.agent/product/scope.yml` (required).
- **Outputs**: the release plan, group manifests, story specs, task files, execution
  status.
- **Hard gate**: do not run until `elaborate-spec` produced a clear spec and `scope.yml`
  exists. Every task must have a runnable verify. The version label is a non-authoritative
  mirror; the real version is tag-driven.
- **Handoff**: `assess-impact` then `plan-work` per story, or `change-request` for a new
  requirement.

### change-request

Add a new requirement or reorder task groups by WSJF mid-release.

- **What it does**: Mode A (add) intakes a new requirement, drafts the story and tasks with
  delta tags, places it in a group, and scores its WSJF. Mode B (reorder) re-scores and
  re-sorts the whole release and flags cut candidates below WSJF 1.5. A conversational mode
  extracts the parameters through dialogue.
- **When to use it**: when a new requirement arrives mid-release, or the plan needs
  re-prioritization.
- **Inputs**: the release plan (required) and the task groups.
- **Outputs**: an updated release plan, group manifests, and execution status.
- **Hard gate**: the release plan must exist first. Run `plan-release` if it does not.
- **Handoff**: `plan-work` or `build-epic` for the top-ranked unstarted story.

### run-planning

The discover-phase advancer. Drives the discover checklist and hands off to the spine.

- **What it does**: tracks progress through `survey-context`, `scope-work`,
  `research-first`, `elaborate-spec` (optional), `plan-release`, and `slice-tasks`,
  invoking each in order and recording status. It manages the planning-context capsule.
- **When to use it**: starting a brand-new initiative, resuming a stalled one, or after
  `orchestrate-project` hands off to Discover. It is not a duplicate of `plan-work`; it
  orchestrates the pre-coding discovery only.
- **Inputs**: the planning progress record and the state file.
- **Outputs**: an advanced discover checklist and a handoff to `plan-work`.
- **Hard gate**: confirm the task group exists and the active story is clear. Planning
  without a target is noise.

---

## Supporting analysis

### plan-tests

Design a risk-scaled test architecture for a task group before implementation.

- **What it does**: reads the sliced stories, maps each behavior to a P0 to P3 risk tier,
  classifies each scenario as unit, integration, or E2E, plans the fixtures, and publishes
  the group test plan with scenario ids.
- **When to use it**: between `slice-tasks` and `plan-work` for a P0 or P1 group. Optional
  for P2 or P3.
- **Inputs**: the story list for the active group.
- **Outputs**: the group test plan, with `SC-eNNsYY-P{0-3}-NN` scenario ids that
  `plan-work` references in its Gherkin.
- **Hard gates**: do not write any test or production code here. Default a test to the
  lowest possible level.
- **Handoff**: gate READY, next `plan-work`.

### assess-impact

Analyze the blast radius of a proposed change before any code is written.

- **What it does**: names the target, finds dependents by grep and git history, maps them
  to release-plan stories, lists the test coverage, classifies risk (low, medium, high),
  and writes an impact report with a recommended action.
- **When to use it**: before `plan-work` on a non-trivial change, when touching a shared
  module, or when the user asks "what does this break?".
- **Inputs**: the target symbol or file, the codebase, the release plan.
- **Outputs**: an impact report with a risk line.
- **Modes**: default (full), or `--lightweight` (fan-in/fan-out only, used by `build-epic`
  as a pre-plan gate). A lightweight risk score above 7 forces a `grill-me` session first.
- **Hard gate**: run it before `plan-work` when a change touches a module used by more than
  one caller. Skip only for net-new code with no dependents.

### seed-conventions

Generate the agent guide and conventions for a greenfield project, and confirm the
`.agent/` workspace.

- **What it does**: interviews for the project name, stack, commands, architecture,
  conventions, never-do list, and defensive-code categories, then generates `AGENTS.md` and
  `CONVENTIONS.md` and confirms the `.agent/` layout. It uses fenced HTML-comment markers so
  handwritten content is never clobbered.
- **When to use it**: the entry point for a greenfield project, or when there is no agent
  guide yet.
- **Inputs**: the interview answers.
- **Outputs**: `AGENTS.md`, `CONVENTIONS.md`, and the confirmed `.agent/` tree.
- **Hard gate**: before any new code lands, confirm the conventions are understood.
- **Related**: the `truenorth_scaffold_project` tool seeds the full `.agent/` tree; this
  skill confirms the workspace and authors the guide.

### plan-refactor

Create a detailed refactor plan of tiny commits through a user interview.

- **What it does**: interviews for the problem and options, verifies the current state in
  the code, checks test coverage, and breaks the refactor into the tiniest commits, each
  leaving the code working. It saves a refactor plan under `specs/` with a problem
  statement, the commits, a decision document, testing decisions, and out-of-scope notes.
- **When to use it**: to plan a refactor, create a refactoring RFC, or break a refactor into
  safe incremental steps.
- **Inputs**: the user's problem description and the codebase.
- **Outputs**: a refactor plan under `specs/`.
- **Hard gate**: before refactoring, document the current behavior, why it is wrong, and the
  one invariant that must be preserved.
- **Handoff**: `kickoff-branch` to create a refactor branch.
