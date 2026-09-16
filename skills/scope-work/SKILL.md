---
name: scope-work
description: "Planning spine step 1 of 3. Scope the work: define what is in and out of scope, and save the product scope. Use it before slice-tasks or plan-release on a new initiative. Not a substitute for slice-tasks or plan-work."
---

# story: e03s01

# story: e24s02

# Scope Work

> **Spine position:** Step 1 — scope-work → slice-tasks → plan-work.

Turn the current conversation into a bounded PRD at `.agent/product/scope.yml`. Without a scope boundary, implementation drifts — stories expand, estimates blow up, and "done" becomes undefined.

## Pre-flight

- [ ] Do you have a clear user need or problem statement? If not, run `elaborate-spec` first.
- [ ] Does `.agent/product/vision.yml` exist? If yes, read it for north-star alignment.
- [ ] Is there an existing `.agent/product/scope.yml`? If yes, you're refining, not creating from scratch.

## Process

0. **Read planning-context.yaml** — If `.agent/tasks/planning-context.yml` exists, read it before doing anything else:

   ```bash
   test -f .agent/tasks/planning-context.yml && echo "Context found" || echo "No context — starting fresh"
   ```

   Pre-populate `feature_name`, `constraints`, and `out_of_scope` from the file. Skip re-asking questions already answered by elaborate-spec. If the file is absent, proceed normally.

1. **Gather context** — Read the existing cockpit artifacts (`.agent/tasks/release-plan.yml`, the project tech-stack note, `.agent/product/vision.yml` if any). Understand what the project is building and why.

2. **Interview (if needed)** — Clarify: What is the goal? Who are the users? What is definitely in scope? What is explicitly out of scope? What constraints exist (time, budget, tech)? How will success be measured?

3. **Write `.agent/product/scope.yml`** with these fields:
   - `core_value` — one-sentence value proposition
   - `summary` — 2-3 paragraph scope overview
   - `in_scope[]` — list of what this initiative covers (each maps to a group or story)
   - `out_of_scope[]` — explicit exclusions (prevents scope creep)
   - `constraints` — tech, time, resource boundaries
   - `success_criteria` — observable outcomes that prove the scope is delivered
   - `references` — links to related specs, ADRs, or documents

4. **Lightweight trade-off analysis** — For each `out_of_scope` item, note _why_ it's excluded (deferred, not valuable, too risky, depends on external factor). This protects against "what about X?" questions later.

5. **Run `research-first`** if external dependencies are proposed — verify the dependency exists, is maintained, and fits the scope before committing to it.

> **HARD GATE** — Every `in_scope` item must map to a future group or story ID or explicit deferred note in `out_of_scope`. If an item can't be mapped, the scope is too vague — refine before proceeding.

> **HARD GATE** — Do NOT include implementation details in SCOPE*LATEST.yaml. Scope is \_what* and _why_, not _how_. Implementation detail belongs in task groups and slice-tasks.

## Common Anti-Patterns

- **"Everything is in scope"** — If nothing is out of scope, you haven't defined a scope. You've described a universe. Cut aggressively.
- **"We'll figure it out later"** — Ambiguity in scope propagates to every downstream decision. Resolve now or explicitly defer in writing.
- **Scope as architecture** — Saying "we need a PostgreSQL database" is architecture, not scope. Scope says "we need to store user profiles and transaction history."

## Output

`.agent/product/scope.yml` — the bounded PRD. Subsequent skills (`slice-tasks`, `plan-work`) reference this as the source of truth for what to build.

## Verify

→ verify: `test -f .agent/product/scope.yml && grep -q out_of_scope .agent/product/scope.yml`
