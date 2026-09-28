# Skills: Sustain

The Sustain phase keeps the project and its own tooling healthy across sessions. The skills
group into session and context management, agent delegation, document authoring, and
catalog maintenance.

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
- **Context strategies**: it implements isolation and state compaction. Compaction archives
  durable decisions and keeps only current handoff state. It does not change prose style.
  `survey-context` handles selection, and the conventions ensure token-efficient writing.

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

- **What it does**: enumerates the catalog with `index_skills`, then delegates naming,
  frontmatter, kind, the 150-line source cap, and link checks to `validate_skill`. It
  separately audits description quality, gates, handoffs, lifecycle placement, cockpit
  paths, and writing rules.
- **When to use it**: during a sustain phase, before a major release, or when catalog drift
  is suspected.
- **Modes**: quick scan (changed skills), full (every skill plus a catalog audit), `--verify`
  (run commands declared by scripted skills only).
- **Hard gate**: a failed structural report, stale description, missing necessary gate, or
  failed scripted verification command is a defect, not cosmetic.

A full report keeps structural, semantic, usage, and optional verify-health findings
separate. TrueNorth does not collect skill invocation counts or timings, so the usage
section reports evidence as unavailable, with no source, observation window, or
retention. Unavailable evidence is not zero usage. This follows the no-journal decision
in [issue #436](https://github.com/vixygrey/truenorth-mcp/issues/436).

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
