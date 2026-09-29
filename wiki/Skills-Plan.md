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

Create the group manifest and define independently deliverable story boundaries.

- **What it does**: cuts tracer-bullet work items and records ids, titles,
  deltas, optional policy-selected estimates, and reserved filenames in the task manifest.
- **When to use it**: after `plan-release`, before `plan-work`.
- **Inputs**: `.agent/product/scope.yml`, the release-index entry, and optional
  planning context.
- **Outputs**: the group manifest only. Reserved specification and task filenames
  are not created here.
- **Hard gate**: the group must already exist in the release index. Every story
  must be independently demonstrable.
- **Handoff**: `plan-tests` for P0 or P1 risk, otherwise `plan-work`.

### plan-work (spine step 3 of 3)

Write the detailed implementation plan into the active task group.

- **What it does**: explores the affected modules, drafts the smallest possible steps (each
  leaves the code working and has one verifiable command), and writes a countable-story
  spec plus a tasks file. It applies the zoom-out mandate, requirement delta tags, a
  slopcheck for external packages, and a cross-artifact consistency pass.
- **When to use it**: after `slice-tasks` and optional `plan-tests`.
- **Inputs**: the release index, scope, active `group.yml`, tech stack, glossary,
  and `test-plan.md` when present.
- **Outputs**: one story specification and its task ledger, every task starting
  `status: failing`.
- **Hard gate**: do not proceed until success criteria are clear. Every task ships a
  runnable verify. A CRITICAL or HIGH consistency finding blocks code generation.
- **Modes**: default (full), or `--fast` (skip the zoom-out and impact assessment for a
  small task).
- **Handoff**: gate READY, next `kickoff-branch`, then `execute-group` or
  `develop-tdd`.

---

## Sequencing and intake

### plan-release

Create and order the release index.

- **What it does**: writes release metadata plus tasks and optional groups to
  `.agent/tasks/release-plan.yml`, ordered by dependencies and selected project policy.
- **When to use it**: after `elaborate-spec` and `scope-work`.
- **Inputs**: the elaborated specification, product scope, and relevant risk reports.
- **Outputs**: the release index only.
- **Hard gate**: do not create capsule contents or execution status. Missing
  downstream artifacts return to their catalog owner.
- **Handoff**: `slice-tasks` for each indexed group.

### change-request

Route a mid-release change through the skills that own the affected artifacts.

- **What it does**: captures and classifies the change, assesses affected groups
  and stories, and invokes the required owner.
- **When to use it**: when a requirement or group priority changes mid-release.
- **Inputs**: the release index and affected capsule artifacts.
- **Outputs**: no planning artifact directly.
- **Routing**: group ordering to `plan-release`, story boundaries to
  `slice-tasks`, test architecture to `plan-tests`, and story details to
  `plan-work`.
- **Hard gate**: every changed artifact must be written by its catalog owner.
- **Handoff**: resume execution after all owner updates and consistency checks pass.

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

Write the risk-scaled test architecture for one group.

- **What it does**: reads story boundaries from `group.yml`, maps behavior to P0
  through P3, chooses test levels, plans fixtures, and assigns stable scenario ids.
- **When to use it**: between `slice-tasks` and `plan-work` for P0 or P1 risk.
- **Inputs**: `.agent/tasks/<capsule>/group.yml`.
- **Outputs**: `.agent/tasks/<capsule>/test-plan.md`.
- **Hard gates**: every scenario must reference an existing group story. The skill
  cannot change boundaries or write implementation artifacts.
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
- **Modes**: default (full), or `--lightweight` (fan-in/fan-out only, used by
  `execute-group` as a pre-plan gate). A lightweight risk score above 7 forces a `grill-me` session first.
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
