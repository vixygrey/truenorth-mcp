---
name: build-group
description: "The profile-aware build cycle. Reads state, execution status, and the active task or group, then advances one verified step per invocation."
kind: prose
---

# Build Group

Read `.agent/profile.yml` first. For a grouped profile, scope the cycle to
`active_group`. For `kanban`, `generic`, or ungrouped `issue-per-task`, scope it
to `active_task` and omit group fields.

> **HARD GATE**: set `active_flow: build_group` and `active_task` in
> `.agent/tasks/state.yml`. Set `active_group` only when the profile groups work.
>
> **HARD GATE**: do not start implementation on `main` or `master`.

## Steps

| Step | Skill or action                                              |
| ---- | ------------------------------------------------------------ |
| 0    | `security-review`: threat-model the selected scope           |
| 1    | `survey-context`: confirm the active task and optional group |
| 2    | `plan-work`: complete the runnable task plan                 |
| 3    | `kickoff-branch`: create a feature branch and clean baseline |
| 4    | `develop-tdd`: run red-green per task                        |
| 5    | `verify-work`: exercise behavior and mechanical gates        |
| 6    | `audit-code`: non-optional gate; failure loops to step 4     |
| 7    | `commit-message`: draft the Conventional Commit              |
| 8    | `release-branch`: open or land the pull request              |

## Process

1. Read `.agent/profile.yml`, `.agent/tasks/state.yml`,
   `.agent/tasks/execution-status.yml`, and `.agent/tasks/release-plan.yml`.
   Read a group capsule only when the profile groups work.
2. Record `started_at` under the task key at step 1. Record task progress through
   `truenorth_record_task`.
3. Run `security-review` against the selected scope.
4. Before writing tasks, run `assess-impact`. Risk above 7 requires `grill-me`.
5. Set a missing current step to 1. Run only the current step in resume mode
   unless the user requests a full automatic run.
6. After verification, advance the lifecycle with
   `truenorth_advance_phase` and record the completed step.
7. At step 8, mark the task done and record `completed_at`.

```yaml
tasks:
  "452":
    status: done
    started_at: "2026-07-12T16:00:00-03:00"
    completed_at: "2026-07-12T18:45:00-03:00"
```

Refresh traceability before step 8. Surface dark, orphan, or stale findings. A
missing refresh remains visible as `trace skipped`; `gate-trace` decides whether
it blocks.

## Audit gate

After `verify-work`, run `audit-code --gate` on the complete diff for the active
task. A blocker resets the current step to 4. Record every blocker and
non-blocking disposition. When the project adopts F.I.R.S.T, run `enforce-first`
on changed tests.

## Fast mode

`build-group --fast` combines survey with planning and audit with commit-message.
It does not weaken evidence, skip blockers, or combine steps that need user or
branch state.

## Handoff

Write the canonical handoff in `.agent/tasks/state.yml`. Set
`handoff.group_id` only for grouped work.

## Verify

Confirm the cockpit files exist, the selected group kind matches the profile, and
the gate skills `assess-impact`, `audit-code`, and `security-review` are present.
