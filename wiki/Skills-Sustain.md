# Skills: Sustain

The Sustain phase keeps the project and its own tooling healthy across sessions. The skills
group into session and context management, agent delegation, document authoring, catalog
maintenance, and communication mode.

For the full arc across phases, see [The skill workflow](The-skill-workflow). For the
alphabetical list, see [Skill index](Skill-index).

---

## Session and context

### session-state

Track decisions and progress in `.agent/tasks/state.yml` to prevent context rot.

- **What it does**: maintains a single source of truth for the session (the active flow,
  the git block, the handoff, the cycle counters), reads and writes it through the
  `truenorth://state` resource, and provides strategic compaction and a universal checkpoint
  pattern. It absorbs the reset-state and compact-state operations.
- **When to use it**: at the start of a session to load context, and whenever a significant
  decision or milestone is reached.
- **Hard gate**: the session state must stay synchronized with the git state. On a
  conflict, halt and ask.
- **Context strategies**: it implements isolation (each agent gets exactly the context it
  needs). It works with `terse-mode` (compression), `survey-context` (selection), and the
  conventions (token-efficient writing).

### organize-workspace

Scan the workspace for disposable artifacts and propose consolidating scattered assets.

- **What it does**: inventories read-only first, classifies candidates (logs and temp,
  build and cache, package caches, stray drafts, duplicate dirs), presents a numbered plan,
  and executes only after explicit approval. It optionally revises the gitignore.
- **When to use it**: on "clean my room", "organize workspace", or a safe tidy pass.
- **Hard gate**: never delete or move without a numbered list and explicit approval. Never
  touch `.git/`, `node_modules/`, `venv/`, `.env*`, or SSH keys.

### reset-baseline

Restore the project to a known clean state between runs or experiments.

- **What it does**: lists uncommitted and untracked files, asks stash/discard/keep per
  category with a safe stash default, re-runs `setup-environment`, and re-runs the test
  baseline.
- **When to use it**: between benchmark runs, after a failed spike, or for a clean working
  tree.
- **Hard gate**: confirm with the user before any destructive git operation. Never
  `reset --hard` without explicit approval.

### terse-mode

A fallback ultra-compressed communication mode that cuts token usage while keeping full
technical accuracy.

- **What it does**: drops articles, filler, and pleasantries, uses fragments and short
  synonyms, and stays active every response once triggered until the user says stop. It
  keeps technical terms, code blocks, and errors exact.
- **When to use it**: only when context is critically long, or on "terse mode" / "be
  brief".
- **Hard gate**: do not use it when clarity is critical (complex design, bug
  investigations). Enable it only on explicit user request.
- **Auto-clarity exception**: it drops terseness temporarily for a security warning, an
  irreversible-action confirmation, or a multi-step sequence, then resumes.

---

## Agent delegation

These two are a deliberate pair. The distinction is stated in both.

### delegate-task

Delegate one complex task to a single subagent, with a two-stage review before merging.

- **What it does**: writes a minimal self-contained brief (goal, in scope, out of bounds,
  constraints, verify, prior decisions), spawns the subagent with a fresh context
  (iterative, max three cycles), reviews the output report (stage 1) then the diff (stage
  2), and decides accept, revise, or reject.
- **When to use it**: when a single complex task needs careful oversight before the result
  is accepted. It is sequential, one agent at a time.
- **Hard gate**: delegated work must have clear success criteria and a verify command the
  delegate can run independently.
- **Related**: `dispatch-agents` for parallel, decoupled tasks with no inter-task review.

### dispatch-agents

Dispatch multiple subagents in parallel on independent tasks.

- **What it does**: confirms task independence, writes typed task briefs (an Orca message
  protocol with task_brief, checkpoint, result, circuit_open envelopes), dispatches all
  agents in one message, applies a circuit breaker (stop a task after three consecutive
  failures), and integrates the accepted results.
- **When to use it**: when the tasks are truly decoupled and speed matters.
- **When not to use it**: when one task depends on another, or the tasks share a file.
- **Hard gate**: agent work must be parallelizable with explicit synchronization points. Do
  not dispatch work with hidden dependencies.
- **Related**: `delegate-task` for sequential single-task oversight. On a silent stall,
  invoke `diagnose-stall`.

---

## Document authoring

These two are a pair: create versus improve.

### write-document

Write a high-integrity technical document using the BMAD methodology (Bold, Minimal,
Actionable, Durable).

- **What it does**: identifies the artifact type (ADR, context map, technical guide,
  behavioral feature, README), drafts with instructions over descriptions and provenance
  links, applies a quality gate against filler and ambiguity, and organizes it by tier with
  nested indexing.
- **When to use it**: to create a document that does not yet exist.
- **Hard gate**: every document must have a clear reason to exist. No speculative doc.
- **Related**: `edit-document` when the document already exists.

### edit-document

Edit and improve an existing document by restructuring, clarifying, and tightening.

- **What it does**: divides the document into sections, orders them to respect information
  dependencies (treating information as a DAG), confirms the sections with the user, then
  rewrites each for clarity within a paragraph length limit.
- **When to use it**: to revise, restructure, or improve any existing document.
- **Hard gate**: preserve intent and accuracy. Do not remove or contradict content without
  understanding why it was written; check git history.
- **Related**: `write-document` to create from scratch.

---

## Catalog maintenance

These skills maintain the skill library itself. This is the runtime governing its own
tooling with the same discipline it applies to a target project.

### stocktake-skills

A batch audit of the skill catalog for drift, stale triggers, missing gates, and
frontmatter problems.

- **What it does**: enumerates the catalog with `index_skills` (the source of truth), checks
  each in-scope skill against the structure rules (verb-noun name, under 300 lines, a gate
  where needed, name and description frontmatter only, description under 1024 characters),
  audits the body against the writing rules, and routes findings to `plan-work` (critical)
  or `evolve-skill` (cosmetic).
- **When to use it**: during a sustain phase, before a major release, or when catalog drift
  is suspected.
- **Modes**: quick scan (changed skills), full (every skill plus a catalog audit),
  `--verify` (run each skill's verify command).
- **Hard gate**: a missing HARD GATE, a stale description, or a broken verify command is a
  defect, not cosmetic.

### evolve-skill

Benchmark-gated skill evolution.

- **What it does**: runs a regression gate, establishes a benchmark baseline, identifies the
  failing scenarios, plans a minimal change with `plan-work`, edits via `craft-skill`,
  re-runs the benchmark, and records an ADR with before-and-after scores. A regression
  reverts and loops.
- **When to use it**: when a skill underperforms on a benchmark, or a stocktake finds a
  systemic gap.
- **Hard gate**: no skill change ships without a benchmark score at or above the pre-change
  baseline. Learning is measured and versioned, never implicit.

### compose-workflow

Chain multiple skills into a custom workflow recipe.

- **What it does**: interviews for the goal and phases, writes a YAML workflow recipe (name,
  command, description, skills, verify), registers it in the state, and maps it to a stack
  command. It defines a terminal-state taxonomy (success, no-op, blocked, exhausted) and
  ships a Standard Recipe Library (for example `/ship` = audit-code, commit-message,
  release-branch).
- **When to use it**: when a project repeats a non-standard skill sequence, or wants a
  documented playbook beyond the `orchestrate-project` modes.
- **Hard gate**: a workflow is orchestration, not automation. Do not create one for a task
  that should be a single skill.

### simulate-agents

Run a mock-user agent and an auditor agent against a feature in fresh contexts before human
review.

- **What it does**: spawns a Mock User (steps through the verification script, reports UX
  gaps) and an Auditor (runs the `audit-code` checklist cold), then writes a simulation
  report and routes failures to `respond-review` or `plan-work`.
- **When to use it**: after `verify-work` and before `request-review`, for a pre-review
  simulation.
- **Hard gate**: simulations are hypothetical. Do not use sim results for a production
  decision without validation on real agents.

### migrate-spec

Detect a foreign spec artifact and transform it into the project YAML layout.

- **What it does**: auto-detects the source framework (GSD, spec-kit, or BMAD) by
  fingerprint, inventories the artifacts, transforms them one at a time with diffs and
  confirmation into `.agent/` and `specs/`, tracks source IDs as first-class YAML fields,
  and regenerates the state file. No code is written.
- **When to use it**: when migrating foreign spec docs.
- **Hard gate**: never overwrite an existing `specs/` file without explicit confirmation;
  merge, do not clobber. It also stops and asks on a partial artifact set, a wrong trigger,
  a stale source, or active divergence.
