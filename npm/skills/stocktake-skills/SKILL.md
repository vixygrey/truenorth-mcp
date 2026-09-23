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

| Mode           | Scope                                                         |
| -------------- | ------------------------------------------------------------- |
| **Quick scan** | Skills changed since the last tag or in the current diff      |
| **Full**       | Every skill, plus a catalog audit                             |
| **--verify**   | Run each skill's verify command and append the health results |

## Process

1. **Enumerate the catalog**: list every skill with the `index_skills` tool. This is
   the source of truth for what the runtime serves, so a drift between it and the
   `skills/` directory is a critical finding.
2. **Run the mode**: for each in-scope skill, check that it exists, has a verb-noun
   name, is under 300 lines, has a HARD GATE where needed, has a `name` and
   `description` frontmatter only, and has a description within 1024 characters.
3. **Body-writing audit**: flag a hedge word and a sentence over 20 words, per the
   house writing rules. Record the writing debt in the report. Do NOT rewrite the
   whole catalog in one pass. Route a repeat offender to `evolve-skill`.
4. **Write the report**: write a findings table (skill, issue, severity) to a dated
   stocktake file.
5. **Effectiveness report** (full mode only): read the skill-usage metrics from
   `.agent/tasks/state.yml` and report the most-used skills, the skills with zero calls
   (potential dead weight), and the skills with a high average time (candidates for
   `evolve-skill`).
6. **Route the findings**: a critical finding becomes a `plan-work` story, a
   cosmetic one becomes an `evolve-skill` candidate.
7. **--verify mode**: run each skill's verify command through the
   `truenorth_verify_gate` tool and append a verify-health section. A FAIL is a
   critical finding and goes straight to `plan-work`.

## Verify

Confirm a dated stocktake report exists and the catalog enumerated by `index_skills`
matches the `skills/` directory.

See [REFERENCE.md](REFERENCE.md) for the checklist.
