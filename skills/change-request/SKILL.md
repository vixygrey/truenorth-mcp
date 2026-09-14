---
name: change-request
description: 'Add a new requirement or reorder task groups by WSJF against the release plan and the task groups. Modes: add and reorder. Use it when a new requirement arrives mid-release or the plan needs re-prioritization.'
---

# story: e45s29

# story: e20s01

# Change Request

> **HARD GATE** — `.agent/tasks/release-plan.yml` must exist before running either mode. If it doesn't, run `plan-release` first.
>
> → verify: `test -f .agent/tasks/release-plan.yml`

Two modes. State which one you want or the skill will ask.

## Mode A — Add

Intake a new requirement mid-flight without disrupting work in progress.

1. **Capture**: What is the change? What problem does it solve?
2. **Locate**: Which existing stories in the task group does it affect or replace?
3. **Draft**: Add story + `tasks[]` with Gherkin-style AC in group YAML (each task has `verify`). Tag requirement deltas: `ADDED` / `MODIFIED` / `REMOVED` / `RENAMED` with before/after for non-`ADDED` changes (e45s29).
4. **Place**: Append story under an existing task group, or create a new group and register it in `.agent/tasks/release-plan.yml` `groups[]`.
5. **Score**: Compute WSJF; note if it outranks in-progress work.

→ verify: `grep -ci 'stor' .agent/tasks/release-plan.yml`

## Mode B — Reorder

Value-engineering pass over the full release using WSJF.

See [REFERENCE.md](REFERENCE.md) for the full WSJF scoring rubric.

1. **Score** each group or story: BV + TC + RR / Job Size.
2. **Re-sort** `.agent/tasks/release-plan.yml` `groups[]` and per-group `wsjf` fields.
3. **Flag cut candidates**: WSJF < 1.5.
4. **Update** `.agent/tasks/release-plan.yml` and group `wsjf` keys with rationale.
5. **Report** the delta.

→ verify: `grep -c 'wsjf' .agent/tasks/release-plan.yml`

## Conversational Mode

If the user's request is in natural language and does not match the structured format of Mode A or Mode B, enter Conversational Mode to extract the change parameters through interactive dialogue.

### 5-Step Flow

1. **Capture**: Parse the natural-language request for what, why, and where. Ask at most 3 clarifying questions before drafting.
2. **Locate**: Identify which group or capability in the task group the request affects or replaces.
3. **Draft**: Present a structured draft of the proposed story and tasks for user confirmation.
4. **Score**: Estimate the WSJF score and explain the calculation so the user understands the priority.
5. **Place**: Confirm the final group placement with the user before writing files.

## After either mode

Update `.agent/tasks/execution-status.yml` from the group manifests. Suggest `plan-work` or
`build-epic` for the top-ranked unstarted story.
