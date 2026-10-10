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

Create the selected work-item manifest and define independently deliverable boundaries.

- **What it does**: cuts tracer-bullet work items, registers them through
  `truenorth_record_task`, and records ids, exact registered names and commands,
  deltas, optional policy-selected estimates, and reserved filenames in `group.yml`.
- **When to use it**: after `scope-work` and any required `plan-release` indexing,
  before `plan-work`.
- **Inputs**: `.agent/product/scope.yml`, the runtime task records, an indexed group
  when selected, and optional planning context.
- **Outputs**: the manifest and task registration. Reserved specification and ledger
  files are not created here.
- **Hard gate**: a selected group must already have an indexed capsule. Ungrouped
  work needs no group; it uses `.agent/tasks/<task-id>/group.yml` with grouping fields
  omitted. Every work item must be independently demonstrable.
- **Handoff**: `plan-tests` for P0 or P1 risk, otherwise `plan-work`.

### plan-work (spine step 3 of 3)

Write the detailed implementation plan for one declared work item.

- **What it does**: explores the affected modules, drafts the smallest possible steps
  (each leaves the code working and has observable verification), and writes one
  reserved specification and ledger. It applies requirement delta tags, external
  package checks, and cross-artifact consistency.
- **When to use it**: after `slice-tasks` and optional `plan-tests`.
- **Inputs**: the release index, scope, active `group.yml`, tech stack, glossary,
  and `test-plan.md` when present.
- **Outputs**: one work-item specification and its task ledger, every task starting
  `status: failing`.
- **Hard gate**: the selected manifest entry must match one exact runtime task
  record and reserve both filenames. Missing mapping or reservations return to
  `slice-tasks`; a missing group index returns to `plan-release`. Clear success
  criteria and runnable verification are required; CRITICAL or HIGH consistency
  findings block implementation.
- **Handoff**: gate READY, next `kickoff-branch`, then `execute-group` or
  `develop-tdd`.

### Runtime task records and planning files

The task tool accepts `task_name`, `verify_command`, and optional `group_id` and
`group_kind`. Planning fields such as `task_id`, `capsule_dir`, `spec`, and `tasks`
are not tool arguments. They belong to the index or selected manifest.

For `issue-per-task`, the manifest's string `task_id` is the actual issue number.
Match its exact registered name and command, plus selected grouping, to the runtime
record. A ticket group can contain work items with different issue ids; the group
id is not the work-item id.

For grouped work, `groups[].capsule_dir` locates `group.yml`. For ungrouped work,
the capsule is `.agent/tasks/<task-id>/`; its `group.yml` has one work item and no
grouping fields. The filename is shared across profiles, not a synthetic group.
`slice-tasks` reserves `<task-id>-spec.md` and `<task-id>-tasks.yml` there.
Readers select `active_task` by id and consume those exact reservations, not
filenames guessed from a title.

---

## Sequencing and intake

### plan-release

Create and order the release index.

- **What it does**: writes release metadata and optional group indexing, preserving
  runtime-owned task records and dependency order unless project policy selects otherwise.
- **When to use it**: after `elaborate-spec` and `scope-work`.
- **Inputs**: the elaborated specification, product scope, and relevant risk reports.
- **Outputs**: the release index only.
- **Hard gate**: do not create capsule contents or execution status. Missing
  downstream artifacts return to their catalog owner.
- **Handoff**: `slice-tasks` for every selected scope; ungrouped work must not bypass
  registration and filename reservations.

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

## Supporting analysis

### plan-tests

Write the risk-scaled test architecture for one declared work-item scope.

- **What it does**: reads boundaries from the selected `group.yml`, grouped or
  ungrouped, maps behavior to P0 through P3, chooses test levels, plans fixtures,
  and assigns stable scenario ids.
- **When to use it**: between `slice-tasks` and `plan-work` for P0 or P1 risk.
- **Inputs**: `.agent/tasks/<capsule>/group.yml`.
- **Outputs**: `.agent/tasks/<capsule>/test-plan.md`.
- **Hard gates**: every scenario must reference an existing manifest work item.
  Missing mapping returns to `slice-tasks`; the skill cannot change boundaries,
  fabricate grouping, or write implementation artifacts.
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
