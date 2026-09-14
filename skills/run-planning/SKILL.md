---
name: run-planning
description: 'The discover-phase advancer. Drives the discover-phase checklist through survey-context, scope-work, research-first, elaborate-spec, plan-release, and slice-tasks. Not a duplicate of plan-work or the planning spine. It orchestrates the pre-coding discover phase only.'
---

# story: e24s03

# Run Planning

> **HARD GATE** — Before running planning skills, confirm the epic capsule exists and the active story is clear. Planning without a target is noise.
>
> **Role:** DISCOVER-PHASE ADVANCER — orchestrates the discover-phase sequence; hands off to the scope-work → slice-tasks → plan-work spine for implementation planning.

Tracks the planning progress as discover-phase skills complete. This is NOT a duplicate of plan-work — it orchestrates the _pre-coding_ discovery phase only (Discover phase in the 6-phase PMBOK lifecycle), handing off to the planning spine for implementation detail.

## When to use

- Starting a brand-new feature or initiative with no prior planning artifacts
- Returning to a stalled initiative and needing to resume the discovery workflow
- After `orchestrate-project` hands off to the Discover phase
- When a new epic emerges from `change-request` and needs to go through full discovery

## Pre-flight

- [ ] Does the planning progress record exist? If not, create it with the default workflow keys.
- [ ] Does `.agent/tasks/state.yml` have `active_flow: planning`? Set it if not already.
- [ ] Is the epic identified in `release-plan.yaml`? The epic must exist before discovery begins.

## Workflows (default keys)

- `survey-context` → `scope-work` → `research-first` → `elaborate-spec` (optional) → `plan-release` → `slice-tasks`

Each key maps to a skill invocation. Optional keys can be skipped; required keys must complete before the phase advances.

## Process

1. **Read state** — Read the planning progress record and `.agent/tasks/state.yml`. Understand where discovery stands: which workflow keys are `done`, which are `pending`, and which are `optional` (can be skipped).

2. **Find next step** — Find the first workflow key with `status: pending`. If the key is `optional`, check if the user wants to run it. If not, mark it `skipped`.

2a. **Context capsule check** — Before invoking `elaborate-spec`, check whether a fresh `.agent/tasks/planning-context.yml` exists:

```bash
test -f .agent/tasks/planning-context.yml && python3 -c "
import yaml, datetime
d = yaml.safe_load(open('.agent/tasks/planning-context.yml'))
written = d.get('written_at','')
if written:
 age = (datetime.datetime.now(datetime.timezone.utc) - datetime.datetime.fromisoformat(written)).total_seconds() / 3600
 print(f'Context age: {age:.1f}h')
" 2>/dev/null || echo "No context or no written_at"
```

- If context is **< 24h old**, ask: `"Planning context from Xh ago exists for '<feature_name>'. Re-run elaborate-spec? [y/N]"`. Skip elaborate-spec on N.
- If context is **≥ 24h old** or absent, run elaborate-spec normally.
- On planning cycle completion (all required keys done), clear the capsule: delete `.agent/tasks/planning-context.yml` and set the planning progress `context_capsule: null`.

3. **Invoke the matching skill** — Run the skill that matches the workflow key:
   - `survey-context` — where are we?
   - `scope-work` — what's in and out?
   - `research-first` — what already exists?
   - `elaborate-spec` — refine the idea (optional)
   - `plan-release` — sequence epics by WSJF
   - `slice-tasks` — cut vertical slices

4. **Update status** — On successful completion, set `status: done` for that workflow key in the planning progress.

5. **Advance** — Set `state.yaml` `active_flow: planning` while in this chain. When all required keys are done, set `handoff.next_skill` to `plan-work`.

## Workflow Keys Schema

The planning progress record uses this schema:

```yaml
context_capsule: # written by elaborate-spec; cleared on cycle completion
  written_at: '2026-06-22T03:00:00Z'
  written_by: elaborate-spec
  feature_name: 'add dark mode'
workflows:
  survey-context:
    required: true
    status: done
  scope-work:
    required: true
    status: pending
  research-first:
    required: false
    status: optional
    note: 'Skip if no external dependencies'
  elaborate-spec:
    required: false
    status: optional
  plan-release:
    required: true
    status: pending
  slice-tasks:
    required: true
    status: pending
```

## Verify

→ verify: the planning progress record exists and marks at least 3 workflow keys `status: done`.
