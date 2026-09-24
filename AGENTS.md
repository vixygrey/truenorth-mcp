# AGENTS.md

This is the agent guide for TrueNorth-MCP. TrueNorth-MCP is an active,
protocol-first MCP execution runtime for spec-driven engineering discipline. The
runtime crate is Rust. A thin Node.js wrapper distributes the binary.

## The two layers

The repository has two layers. The machine-facing layer is `.agent/`. The
runtime reads, watches, and writes only under `.agent/`. The human-facing layer is
`specs/`, which holds human-authored narrative. The runtime can read a file under
`specs/`, but it never mutates a path under `specs/`.

A single write guard funnels every runtime write and rejects any target outside
`.agent/` (ADR-0008).

## The `.agent/` layout

- `.agent/layout.yml`: the layout contract the runtime validates on read.
- `.agent/profile.yml`: the active methodology profile name.
- `.agent/config/`: token caps, human-approval gates, protected paths, and lint
  standards.
- `.agent/spec/`: the feature requirements, architecture, and the definitions of
  ready and done.
- `.agent/tasks/`: the cockpit. `state.yml` carries the phase and the TDD loop,
  `release-plan.yml` carries the recorded tasks, `execution-status.yml` carries
  the story status, `bugs.yml` carries external-tracker bug references, and
  `backlog.yml` declares whether backlog ownership is local or external.
- `.agent/ontology.yml`: the domain ontology.
- `.agent/product/`: the product scope, vision, and glossary.
- `.agent/memories/`: the lessons and the glossary.
- `.agent/telemetry/`: the agent cost audit. This area is excluded from agent
  reads.

## The `specs/` layer

- `specs/adr/`: the Architecture Decision Records, served read-only through the
  `truenorth://adr` resource.
- `specs/contracts/`: the human-authored data-shape contracts, when present.

## Methodology profiles

The project declares one of five methodology profiles in `.agent/profile.yml`:
epic-based, issue-per-task, kanban, milestone-based, or generic. The default is
issue-per-task. A profile sets the grouping vocabulary, whether grouping is
required, the branch pattern, and whether a commit needs an issue id (ADR-0009).

## Working rules

- GitHub Issues is this repository's backlog source of truth. The external
  ownership declaration in `.agent/tasks/backlog.yml` is a pointer, not an issue
  cache. Never treat it as live backlog data.
- Record a task with the `truenorth_record_task` tool. The grouping key is
  `group_id` with an optional `group_kind`. A legacy `epic_id` still maps to a
  group with `group_kind` set to epic.
- Advance the lifecycle phase with the `truenorth_advance_phase` tool.
- Run a quality gate with the `truenorth_verify_gate` tool.
- Record a bug reference with the `truenorth_record_bug` tool. The external
  tracker owns the bug detail (ADR-0010).
- Commits are atomic and follow Conventional Commits. Work on a short-lived
  branch off `main` and merge back through a squash-merge pull request.

## Conventions

The engineering standards live in `CONVENTIONS.md` at the repository root, served
through the `truenorth://conventions` resource. The house writing rules apply to
all prose this project produces.
