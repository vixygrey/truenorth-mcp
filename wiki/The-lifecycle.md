# The lifecycle

The runtime drives a six-phase lifecycle: Discover, Design, Plan, Execute, Review, and
Integrate. This page walks one feature through it with the active tools. Each tool writes
cockpit state under `.agent/` and returns a typed result.

## Discover

Read the current state before you act. Read the `truenorth://state` resource. It reports
the active flow, the active group, the active task, the phase, and the handoff. See
[Resources](Resources) for the resource surface.

## Design

Design the change against the feature narrative in `.agent/spec/` and the ADRs under
`specs/adr/`. Design is human and agent narrative work, not a tool call.

## Plan

Record a task with `truenorth_record_task`. It takes `task_name` and `verify_command`, and
an optional grouping key.

```json
{
  "task_name": "add pagination to the users endpoint",
  "verify_command": "cargo test users::pagination"
}
```

The `verify_command` is the command that proves the task done. Record a real command, so
the verify gate can run it later. The tool appends the task to
`.agent/tasks/release-plan.yml`.

## Execute

Drive the change with the Red-Green-Refactor loop. `truenorth_tdd_cycle` enforces the step
order. It takes `step` (`red`, `green`, or `refactor`), a `failing_test_cmd`, and the
`files_to_modify`.

```json
{
  "step": "red",
  "failing_test_cmd": "cargo test users::pagination",
  "files_to_modify": ["runtime/src/tools/users.rs"]
}
```

Run the `red` step first. The runtime checks that the test fails before you write the code,
so a green result at the red step is itself a failure. Then run `green`, then `refactor`.

## Review

Run the quality gate with `truenorth_verify_gate`. It takes the `phase` and runs the
configured phase verify command in a sandbox. It passes only on a real exit code of `0`
that the server observed. A timeout, a non-zero exit, or an allowlist rejection returns
an error with a remediation hint.

```json
{
  "phase": "review"
}
```

The gate is trustworthy because the server, not the model, observed the exit code. The
sandbox bounds the run with a wall-clock timeout, a working directory pinned under the
repository root, a command allowlist, and an environment sanitized against the secret
denylist.

## Integrate

Advance the phase with `truenorth_advance_phase`. It takes `from_phase`, `to_phase`, and an
`artifacts_summary`. The source must match the recorded phase, and the target must be its
immediate successor. An absent or `null` recorded phase bootstraps as Discover. Integrate
advances to Discover to begin the next loop.

```json
{
  "from_phase": "review",
  "to_phase": "integrate",
  "artifacts_summary": "Pagination added to the users endpoint, tests green."
}
```

The tool writes `.agent/tasks/state.yml` and records the git context, then the watcher
emits a resource update for `truenorth://state`.

## Record a bug

Record an external-tracker bug reference with `truenorth_record_bug`. The external tracker
owns the bug detail. The runtime stores the reference in `.agent/tasks/bugs.yml` (ADR-0010).
