# Design note: strengthen the lean tier (#62)

## Goal and current gap

The lean tier is the token-efficient render for local agents and lean harnesses. Today
it barely compresses a tight skill. Measured on `gate-trace` through `render_skill`:

- Full: 618 words
- Reasoning: 618 words
- Lean: 617 words

The three tiers converge. `compress_for_local_context` (in
`truenorth-mcp/src/engine/tier.rs`) only bites on five exact-title sections
(`rationale`, `background`, `philosophy`, `examples (verbose)`, `red flags`) or on the
1500-token budget. A well-written skill hits neither. This design makes lean meaningfully
smaller while Property 4 holds.

## Invariants that do not move

- `render_skill(md, Full) == md`, byte-for-byte. All new work is `Tier::Lean` only.
- Reasoning tier behavior is unchanged. Its tests pin it.
- The transforms stay pure functions of the input markdown. No config. No I/O.
- Property 4: no pass drops or rewrites a line that encodes an invariant or an acceptance
  criterion.

## Evidence

A heading survey across the 80 skills informs the design:

- Markdown tables appear in 45 of 80 skills. This is the largest untapped lever.
- Handoff sections appear in 13 skills. Their bodies carry `Next:`, `Writes:`, and `Gate:`
  wiring lines that a chaining agent needs.
- Integration points (3), References (2), Output format (2), Notes, and Out of scope
  appear in smaller numbers with mixed content.

## Change 1: module split (pure move, ships first)

Split `tier.rs` into a `tier/` module before any behavior change, as its own atomic
commit:

- `tier/mod.rs`: the public surface (`Tier`, `render_skill`, `TIER_LEAN_TOKEN_BUDGET`,
  `strip_meta_steps`, `compress_for_local_context`) and the reasoning-tier internals.
- `tier/compress.rs`: the lean passes (existing plus new) and their helpers.
- Tests move to sibling files via the existing `#[path]` include pattern.

Each file stays under the 300-line guidance once the new passes land. The move commit
changes no behavior, so the suite stays green across it.

## Change 2: extend the invariant check (the Property-4 seam)

`encodes_invariant_or_ac` gains three leading-anchored line shapes, matched on the trimmed
line start: `Next:`, `Writes:`, `Gate:`. These are the handoff wiring lines. The anchor is
the line start, not a substring, so prose that merely contains the word is not preserved.
This runs first, because the widened section drop and the table pass both consult it per
line.

## Change 3: widen the low-value section set (drop body, keep heading)

Exact-title, case-insensitive match, as today. Final drop-body set:

- Existing: `rationale`, `background`, `philosophy`, `examples (verbose)`, `red flags`.
- Added: `references`, `out of scope`, `handoff`.
- Left intact: `integration points`, `notes`. They carry action-relevant prose with no
  reliable line shape to protect.

Handoff is safe to drop because Change 2 protects its `Next:`, `Writes:`, and `Gate:`
lines globally. The heading stays as a landmark, the wiring survives, only true prose
drops.

## Change 4: compact tables (new pass)

Detection: a run of consecutive lines that each trim to start and end with `|`, and the run
contains a separator row (`^\|[\s:|-]+\|$`). The run breaks at the first non-table line. No
separator row means the run is not treated as a table.

- All tables: drop the separator row, strip cell padding to single spaces, rejoin as
  `| a | b |`.
- Two-column tables only: reflow each body row to `- left -> right`, and drop the
  two-column header, which is usually `Condition | Verdict` scaffolding.
- Three or more columns: stay tabular, padding stripped, separator dropped. The `->` form
  cannot represent them without loss.
- One column: tabular, padding strip only.
- Per-row Property-4 override: before a row is reflowed or padding-stripped, check the raw
  row against `encodes_invariant_or_ac`. A match is emitted verbatim.
- Row and header cell-count mismatch: padding-strip that row, skip the `->` reflow, so no
  mapping is fabricated.
- Escaped pipes (`\|`): split on unescaped `|` only, so `\|` stays inside the cell.

## Change 5: strip decorative formatting (new pass)

Collapse standalone horizontal rules (`---`, `***`, `___`) and repeated separator lines,
and trim trailing alignment whitespace. Content is never touched. This is low risk.

## Lean pipeline order

`drop_low_value_sections` (widened), then `compact_tables`, then `strip_decoration`, then
`headings_to_bullets`, then `dedupe_directives`, then `truncate_to_budget` (unchanged final
guard).

Section drop runs first because it removes the most. The table and decoration passes act on
what remains. Truncation stays last.

## Tests

- Compression golden test: a fixture with a table, a References section, and a Handoff
  section. Assert `lean_words <= 0.60 * full_words`. Add a real-skill ratio check against a
  table-heavy skill, skipped when absent, matching the existing `repo_root_file` pattern.
- Property 4, extended fixture: invariant lines placed inside a table row, inside a
  References section, and as `Next:`, `Writes:`, and `Gate:` lines inside a dropped Handoff
  section. All survive lean.
- Table behavior: a two-column table reflows to `a -> b`. A three-column table stays
  tabular with padding stripped and the separator gone. An invariant-bearing row survives
  verbatim.
- Regression guards: `full_tier_is_identity` and the reasoning-tier tests stay unchanged and
  green.

## Commit plan (atomic, conventional)

1. `refactor(tier): split tier.rs into a tier/ module`. Pure move, suite green.
2. `feat(tier): treat handoff wiring lines as load-bearing`. Change 2, with tests.
3. `feat(tier): widen the lean low-value section set`. Change 3.
4. `feat(tier): compact tables in the lean tier`. Change 4, the main lever.
5. `feat(tier): strip decorative formatting in the lean tier`. Change 5.

Each commit compiles and passes `cargo fmt`, `cargo clippy --all-targets -- -D warnings`,
and the tests. Each is individually revertable. One PR, `Refs #62`, hold for merge.

## Residual risk

A prose-only skill with no table and no structural sections can still render lean near
full. This is correct. There is nothing inert to remove. The acceptance test uses a skill
that has structural sections, matching the issue scope.
