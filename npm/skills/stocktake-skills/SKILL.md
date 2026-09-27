---
name: stocktake-skills
description: "A batch audit of the skill catalog. A quick scan of changed skills, or a full audit of all skills. Use it during a sustain phase, before a major release, or when catalog drift is suspected."
kind: prose
---

# Stocktake Skills

> **HARD GATE**: the skill inventory MUST be current. A missing HARD GATE, a stale description, or a broken verify command is a defect, not cosmetic. Fix it in `evolve-skill`.

Audit the SKILL.md catalog for drift, a stale trigger, a missing HARD GATE, and a
frontmatter problem.

## Modes

| Mode           | Scope                                                       |
| -------------- | ----------------------------------------------------------- |
| **Quick scan** | Skills changed since the last tag or in the current diff    |
| **Full**       | Every skill, plus a catalog audit                           |
| **--verify**   | Run each scripted skill's command and append health results |

## Process

1. **Enumerate the catalog**: list every skill with the `index_skills` tool. This is
   the source of truth for what the runtime serves, so a drift between it and the
   `skills/` directory is a critical finding.
2. **Validate the structure**: run `validate_skill` for each in-scope skill. Record
   every failed check. The validator owns naming exceptions, required frontmatter,
   prose and scripted kinds, scripted `verify` metadata, the 150-line source cap,
   and skill-link resolution. Do NOT duplicate those checks.
3. **Audit semantics**: inspect description quality, HARD GATE suitability, writing
   rules, handoffs, lifecycle placement, and cockpit paths. These checks supplement
   `validate_skill`; they do not redefine its schema. Record repeated writing debt
   for `evolve-skill`. Do NOT rewrite the whole catalog in one pass.
4. **Write the report**: write a findings table (skill, issue, severity) to a dated
   stocktake file. Keep structural and semantic findings distinct.
5. **Effectiveness report** (full mode only): read the skill-usage metrics from
   `.agent/tasks/state.yml` and report the most-used skills, the skills with zero
   calls (potential dead weight), and the skills with a high average time
   (candidates for `evolve-skill`).
6. **Route the findings**: a critical finding becomes a `plan-work` story, a
   cosmetic one becomes an `evolve-skill` candidate.
7. **--verify mode**: for each `kind: scripted` skill, run its declared frontmatter
   `verify` command and append a verify-health section. Prose skills have no command
   to run. A command failure is a critical finding and goes straight to `plan-work`.

## Verify

Confirm the dated report covers every skill returned by `index_skills`, includes a
`validate_skill` result for each in-scope skill, and separates structural findings
from semantic and scripted-command findings.

See [REFERENCE.md](REFERENCE.md) for the checklist.
