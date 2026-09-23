---
name: slice-tasks
description: "Planning spine step 2 of 3. Slice the work: break a scoped PRD into vertical-slice stories in the task groups. Use it after scope-work, before plan-work. Not a substitute for scope-work or plan-work."
kind: prose
---

# Slice Tasks

> **Spine position:** Step 2 — scope-work → slice-tasks → plan-work.

Produce **task group story tasks** in the task group under `.agent/tasks/` — vertical slices, each independently deliverable and testable. Output: decoupled `eNNsYY-tasks.yaml` files with runnable verify commands. Use capsule dirs + `execution-status.yaml`.

## Pre-flight

- [ ] Does `.agent/product/scope.yml` exist? If not, run `scope-work` first — you can't slice what you haven't bounded.
- [ ] Is the `release-plan.yaml` populated with the task groups you're slicing? Group IDs (e01, e02…) should exist before you create stories.
- [ ] Do you understand the difference between a horizontal layer and a vertical slice? (See anti-patterns below.)

## Process

0. **Read planning-context.yaml** — If `.agent/tasks/planning-context.yml` exists, read it first:

   ```bash
   test -f .agent/tasks/planning-context.yml && echo "Context found" || echo "No context — starting fresh"
   ```

   Use `feature_name`, `constraints`, and `out_of_scope` to inform slice boundaries. `key_decisions` in the file may constrain how stories are cut (e.g., "no external deps" constrains slice 2). If absent, proceed normally.

1. **Read context** — Read `.agent/product/scope.yml` and/or `.agent/tasks/release-plan.yml`. Understand what the task group delivers end-to-end.

2. **Cut tracer-bullet slices** — Identify the thinnest possible vertical path through the stack that delivers user value. Start with this slice; it will catch integration issues first. For example:
   - A search feature: first slice is "user types query → API returns results" (no filters, no pagination, no ranking — just the plumbing working end-to-end).
   - A checkout flow: first slice is "user clicks buy → order created" (no payment, no inventory, no email).

3. **Assign BCPs** — For each story, estimate Business Complexity Points (1–13). A 1-BCP story is a trivial change (one file, one concept). A 13-BCP story is a major feature across multiple modules. If a story exceeds 8 BCPs, consider splitting it.

4. **Each story** writes:
   - `eNNsYY-tasks.yaml` with `story_id`, `title`, `status`, `bcps`, `tasks[]` (each with `id`, `description`, `verify`, `status`)
   - Story spec `.md` files are written by `plan-work` and follow countable-story-format.md
   - The task group manifest (`group.yml`) is updated to list the story ID and BCPs
   - **Requirement deltas (e45s29):** Stories that alter existing behavior MUST carry `delta:` in `group.yml` (`ADDED` | `MODIFIED` | `REMOVED` | `RENAMED`). `plan-work` expands deltas into full before/after requirement text.

5. **Order by WSJF** in `release-plan.yaml` group list — highest WSJF first. Weight-shortest-job-first ensures the highest value arrives earliest.

6. **Validate slices** — Every slice must answer: "If this story ships, does a user get new value?" If the answer is "no, they need a later story too", the slice is too horizontal — cut vertically deeper.

> **HARD GATE** — No horizontal-only slices ("add all models") without a vertical path that proves integration. Every story must be independently demonstrable, even if it only handles the happy path.

> **HARD GATE** — Each task's `verify:` field must contain a runnable command (not "manually check" or "review visually"). If verification requires manual steps, prefix with `verify-script:` and write the steps in the story file.

## Anti-Patterns

- **Layer cakes** — "Week 1: all models. Week 2: all controllers. Week 3: all views." This hides integration risk until the end. Every story must cut through all layers.
- **Too-small slices** — If a slice takes < 30 minutes to implement, it's probably noise. Combine with adjacent slices.
- **Too-large slices** — If a slice takes > 3 days, it's a task group, not a story. Split further.

## Output

- `.agent/tasks/<capsule>/eNNsYY-tasks.yaml` — per-story task breakdown with verify commands
- `.agent/tasks/<capsule>/group.yml` — updated with story list and BCPs
- `.agent/tasks/release-plan.yml` — updated WSJF ordering (if needed)

## Verify

→ verify: `[ "$(find .agent/tasks -name '*-tasks.yaml' 2>/dev/null | wc -l | tr -d ' ')" -gt 0 ]`

<!-- story: e03s01 -->
