# Plan Work Reference

## Output file formats

Use the filenames reserved by the selected work item in its manifest. A neutral
convention is:

- `.agent/tasks/<capsule>/<task-id>-spec.md`
- `.agent/tasks/<capsule>/<task-id>-tasks.yml`

A profile may use its grouping vocabulary in the id. Shared instructions and
schema fields remain task-based.

## Task ledger

```yaml
task_id: task-17
title: Login
status: failing
tasks:
  - id: 1
    description: "Add login form component tests"
    verify: "npm test -- login-form.test.tsx"
    risk: P1
    status: failing
```

Add a BCP estimate only when project policy or the user explicitly selects BCP:

```yaml
bcp: 3
estimation_policy: bcp
```

The work item must already be listed in its manifest. Do not update the manifest
or `.agent/tasks/execution-status.yml`; report a mismatch to the owning skill.

## Specification template

```markdown
# Work item <id>: <title>

**type:** feat | fix | refactor
**risk:** P0 | P1 | P2 | P3
**context:** domain | infra

## Context

<What this work item implements and why.>

## Requirements

#### ADDED: <requirement>

<Full requirement text.>

## Steps

1. <Step description> (ref: ADR-NNNN or commit SHA)
   verify: `<runnable command>`

## Verification script

1. <Action>
2. <Observation proving the outcome>

## Out of scope

- <Explicit exclusion>

## Risks

- <Risk and early detection method>
```

## Verify step rules

Every implementation step must include a command that proves its observable
outcome. Commands that only inspect source text, assert non-empty output, or
prove that a mock echoed its input are not acceptable.

Good:

```text
1. Add the POST /users endpoint
   verify: npm test -- users-api.test.ts
```

Bad:

```text
1. Implement the user creation flow
2. Write tests for the API
```

## Risk assignment

Preserve the scenario risk mapping from
`.agent/tasks/<capsule>/test-plan.md` when present. Otherwise use behavior and
impact, not effort estimates:

- **P0**: critical path, data-loss risk, authentication or security boundary, or
  external integration.
- **P1**: core behavior, state mutation, or standard business value.
- **P2**: utility behavior, layout change, or display-only data.
- **P3**: documentation, cosmetic changes, or no behavioral change.

If a selected estimation policy includes BCP, it may provide additional context.
It never overrides a higher behavior-based risk.

## Requirement delta tags

For existing behavior, use one of:

```markdown
#### MODIFIED: User can reset a password through an email link

**Before:** Password reset required administrator approval.
**After:** A signed email link enables self-service reset and expires after one hour.
```

Allowed tags: `ADDED`, `MODIFIED`, `REMOVED`, and `RENAMED`. `MODIFIED`,
`REMOVED`, and `RENAMED` require explicit before and after content.

## Package check

For every external dependency, record one tag:

- `[OK]`: established package with clear maintenance and provenance.
- `[SUS]`: uncertain maintenance, provenance, or fit.
- `[SLOP]`: likely unnecessary, abandoned, or unsafe for the proposed use.

`[SUS]` and `[SLOP]` require explicit human approval before execution.
