# ADR-0014: Skill Artifact-Writing Convention

**Status:** Accepted
**Date:** 2026-09-19

## Context

A skill runs as a Node.js or shell script that the model spawns as a separate
process. A skill talks to the runtime over MCP stdio. It does not link the Rust
crate, so it cannot call the single write guard `engine::agent_ws::write_under_agent`
(ADR-0008). No general artifact-write MCP tool exists. The write-capable tools
(`scaffold`, `generate_ontology`, `record_task`, `record_bug`, `tdd_cycle`) are all
fixed-shape, so none accepts an arbitrary output path.

Two skills write artifacts today, and each rolls its own path handling:

- `extract-design` writes the committed design note to `.agent/product/design.md`
  through the `OUTPUT_PATH` constant, and writes a handoff to `.agent/tasks/state.yml`
  through the `STATE_PATH` constant. Both paths are under `.agent/`.
- `visual-dashboard` writes a transient render tree to an out-of-repo session dir
  (`/tmp/truenorth-dashboard`), or to `${PROJECT_DIR}/.truenorth/dashboard/${SESSION_ID}`
  when a project dir is set. It reads the cockpit under `.agent/tasks/`, and it never
  writes into the repo `.agent/` tree.

Both skills respect the write model, but they respect it by convention in each
script, not by construction. The runtime cannot enforce a skill write, because the
skill write never passes through the Rust guard. The open question was whether the
runtime must own a shared artifact-writing helper, so skills stop hand-rolling their
output paths.

Three options were considered.

1. A documented convention, checked by a lint. Publish the rule and add a lint over
   skill scripts. The invariant holds by convention, checked, not by construction.
2. A shared JS helper in the skills toolkit. One Node module resolves and validates
   artifact paths for every skill. This is a second implementation of the guard rule,
   not the Rust one.
3. A new artifact-write MCP tool. The only way to route a skill write through the
   actual Rust guard. It widens the protocol surface and duplicates the guard's path
   validation at the boundary.

## Decision

Adopt option 1. Publish the artifact-writing convention here, and add a lint over
skill scripts that checks it.

The convention has two rules, one per artifact class:

- **Machine artifacts go under `.agent/`.** A skill that writes a committed,
  machine-owned artifact writes it under `.agent/`, matching the write guard's
  invariant. The design note at `.agent/product/design.md` and the handoff at
  `.agent/tasks/state.yml` are the current examples.
- **Transient render output goes to an out-of-repo session dir, or a gitignored
  in-repo session dir.** A skill that writes a throwaway render (a served dashboard,
  a scratch tree) writes it outside the repo, for example under a temp dir. When a
  skill must write a session dir inside the repo, the path stays gitignored, so a
  transient artifact never lands in version control.

A skill never writes into `specs/`, because that layer is human-authored and
read-only to the runtime and its skills (ADR-0008).

Option 2 stays open as a later step. A shared JS helper is worth adding when a third
artifact-writing skill lands and the duplicated path handling starts to cost more
than the helper would. Option 3 is rejected for the current two skills, because a
new general-purpose write tool is over-engineering against the narrow-interface
principle, and both skills are already compliant.

## Consequences

The convention is written down, so a new skill has one rule to obey per artifact
class, and the disposition split is explicit rather than implicit in two scripts. A
lint over skill scripts (`scripts/lint-skill-artifacts.sh`, wired into the CI `shell`
job) checks the rule, so a regression fails the gate rather than drifting silently.

The invariant holds by convention and by the lint, not by construction. A skill
script can still write an arbitrary path at runtime, because it does not pass through
the Rust guard. The lint narrows this gap by rejecting a skill write that names a
path outside `.agent/` and outside an allowed transient location. It cannot prove the
absence of every escaping write, so a skill author still carries final responsibility
for the write target.

If a third artifact-writing skill arrives, revisit option 2, so the path handling
lives in one shared helper rather than three hand-rolled copies.
