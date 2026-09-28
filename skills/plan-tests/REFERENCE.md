# Plan Tests Reference

Write this template to `.agent/tasks/<capsule>/test-plan.md`. Copy task ids from
the active manifest. Never create an id in the test plan.

```markdown
# Test design: <task group id and title>

## Scope

- Profile: <issue-per-task | epic-based | milestone-based | kanban | generic>
- Work items: <declared task ids>
- Exclusions: <explicit exclusions>

## Scenarios

### SC-<task-id>-P0-01: <observable behavior>

- Risk: P0
- Owner: <task id>
- Level: unit | integration | end-to-end
- Setup: <fixture or initial state>
- Action: <user or system action>
- Expected: <observable result>
- Verify: `<runnable command or smoke procedure>`
- Failure evidence: <log, response, state, or screenshot>

## Coverage matrix

| Requirement or behavior | Task id   | Scenario ids       | Risk |
| ----------------------- | --------- | ------------------ | ---- |
| <behavior>              | <task-id> | SC-<task-id>-P0-01 | P0   |

## Fixtures and boundaries

- <fixture ownership, isolation, and cleanup>

## Deferred coverage

- <P2 or P3 scenario, owner, and rationale>
```

## Rules

- Every scenario references a task already declared in the active manifest.
- Every P0 and P1 behavior has an owner and verification path.
- Prefer the lowest test level that proves the behavior.
- Include failure and boundary behavior where risk warrants it.
- Preserve an existing profile-specific id format only when the project already
  selected it. Shared schema remains task-based.
