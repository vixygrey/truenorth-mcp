---
name: verify-work
description: "Verify a completed task against its specification, risk level, and observable behavior. Record evidence without changing planning artifacts."
kind: prose
---

# Verify Work

> **HARD GATE**: No task is done until at least one task-specific observable
> behavior smoke passes and its evidence is recorded.
>
> Do not run on `main` or `master`. Use the feature branch from `kickoff-branch`.

## Artifact contract

- **Reads**: `.agent/profile.yml`, the selected work-item specification and task
  ledger, optional `test-plan.md`, architecture, conventions, and affected code.
- **Writes**: verification evidence in the task capsule and coordination state in
  `.agent/tasks/state.yml`.
- **Never writes**: `.agent/tasks/release-plan.yml`, the task manifest, work-item
  specification, task ledger promises, or product scope.

Use task and work-item ids in shared evidence. Preserve a profile-specific id only
when the active project already selected it.

## Process

1. Read the active task ledger and work-item specification. Note risk P0 through
   P3 and all promised observable outcomes.
2. Validate every `verify:` command before UAT. If a command does not exercise the
   intended behavior, route the mismatch back to `plan-work`; do not weaken the
   expected outcome.
3. Run the project's configured quality gate.
4. Exercise at least one changed behavior through the actual surface. Include a
   cold start when stale state is plausible. For a CLI, invoke the CLI. For a
   server or UI, use the running application. For documentation or configuration,
   parse, render, or load it with the real consumer.
5. Run every required P0 and P1 scenario from `test-plan.md`. Apply P2 and P3 per
   project policy.
6. Compare expected and actual results. A mismatch fails verification even when
   tests pass.
7. Record evidence using [REFERENCE.md](./REFERENCE.md), including command or
   action, expected result, actual result, and pass or fail.
8. Mark execution status done only after every required task is passing and no
   blocking mismatch remains.

## Evidence rules

Evidence must identify the task, timestamp, risk, verifier, quality-gate result,
behavior smoke, and any scenario results. Source-text inspection, non-empty output,
mock echoes, and bare not-throw checks are not observable behavior proof.

## Failure routing

- Implementation defect: route to `develop-tdd` or `fix-bug`.
- Planning mismatch: route to `plan-work` without rewriting the promise here.
- Upstream scope or architecture mismatch: route to the owning planning skill.
- Security finding: route to `security-review` and block completion when policy
  requires it.

## Verify

The work item passes only when the configured quality gate succeeds, at least one
actual-surface behavior smoke matches its promise, every required risk scenario
passes, and durable evidence is recorded.

## Handoff

Write the canonical `.agent/tasks/state.yml` handoff. Use `next_skill: build-group`
when more tasks remain in the group, otherwise route to the profile-appropriate
integration or release step.
