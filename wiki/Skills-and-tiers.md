# Skills and tiers

The runtime serves a skill library. A skill is a canonical `SKILL.md` source, rendered per
call at one of three tiers. One source directory, rendered per call, replaces the
per-harness static mirrors of the upstream design.

This page explains the tier mechanism and the catalog tools. For the full per-skill
reference, see the [Skill index](Skill-index) and [The skill workflow](The-skill-workflow).

## Read a skill

`get_skill` reads a skill at a tier. It takes the skill `name` and an optional `tier`.

```json
{
  "name": "develop-tdd",
  "tier": "lean"
}
```

An absent `tier` uses the `TRUENORTH_TIER` environment default. The `tier` argument
overrides it for the one call.

## The three tiers

- `full`: the verbatim `SKILL.md`. Use it when the model has ample context and wants the
  complete guidance, including rationale and examples.
- `reasoning`: the directive content with the meta scaffolding stripped. Use it for a
  native reasoning model that does not need hand-held chain-of-thought.
- `lean`: the directives compressed to a tight token budget. Use it for a local model with
  a small context window.

A tier transform never alters the directive content that encodes an invariant or an
acceptance criterion. It strips or compresses only meta and guardrail scaffolding. `full`
is the identity.

## The catalog tools

The catalog tools read and search the library:

- `index_skills`: list every skill and its phase.
- `read_skill`: parse a `SKILL.md` into its frontmatter, headings, and sections.
- `search_skills`: search skill metadata.
- `validate_skill`: enforce the two-tier structural contract. Every skill requires `name`,
  `description`, and `kind`, where `kind` is `prose` or `scripted`. Scripted skills also
  require a nonempty frontmatter `verify`; naming exceptions use `name_exception`. The
  complete SKILL.md is limited to 150 lines. The tool reports command metadata but does not
  execute it.

## The skill graph

The graph tools build and query the entity-relation graph over the skills:

- `build_skill_graph`: build and persist the graph.
- `read_graph`, `search_nodes`, `open_nodes`: query it.
- `get_dependencies`: report the forward and reverse dependencies and the handoff chain for
  one skill.

`build_skill_graph` classifies skill mentions instead of treating every mention as a
dependency:

- `depends_on`: `A` requires `B` before `A` runs. `A after B` produces `A → B`;
  `A before B` produces `B → A`.
- `invokes`: `run`, `invoke`, `invokes`, and `route to` name an execution target.
- `handoff_to`: `next`, `next_skill`, and `hand off to` name the next skill.
- `references`: any other exact mention or skill link.
- `enforces`: a skill cites a conventions section.

The build receipt includes unresolved or unclassified mentions, isolated skills,
orchestrator-like skills without outgoing control flow, and dependency cycles.
Unresolved or unclassified targets fail the build before the graph cache is written.

## Git context

`get_git_context` reports the git status, log, or diff scoped to the tracked areas. It is a
read-only report the runtime uses to ground its work.
