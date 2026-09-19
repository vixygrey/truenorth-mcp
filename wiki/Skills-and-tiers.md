# Skills and tiers

The runtime serves a skill library. A skill is a canonical `SKILL.md` source, rendered per
call at one of three tiers. One source directory, rendered per call, replaces the
per-harness static mirrors of the upstream design.

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
- `validate_skill`: lint a `SKILL.md` against the conventions.

## The skill graph

The graph tools build and query the entity-relation graph over the skills:

- `build_skill_graph`: build and persist the graph.
- `read_graph`, `search_nodes`, `open_nodes`: query it.
- `get_dependencies`: report the forward and reverse dependencies and the handoff chain for
  one skill.

## Git context

`get_git_context` reports the git status, log, or diff scoped to the tracked areas. It is a
read-only report the runtime uses to ground its work.
