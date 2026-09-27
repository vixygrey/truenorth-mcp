# Plan Tests — Reference

## Group test plan

Write this template to `.agent/tasks/<capsule>/test-plan.md`. Copy story ids from
`group.yml`. Never create a story id in the test plan.

```markdown
# Test Design: [group id and title]

## 1. Risk Matrix and Scenarios

| Scenario ID     | Behavior Description | Risk | Test Level  | Target File/Module |
| --------------- | -------------------- | ---- | ----------- | ------------------ |
| SC-e01s01-P0-01 | Primary checkout     | P0   | Integration | checkout.spec.ts   |

## 2. Fixture Architecture and Isolation

- Data factories: for example, `UserFactory`
- Network intercepts: for example, MSW handlers
- Database state: for example, in-memory SQLite

## 3. NFR Verification

| NFR Type | Requirement | Verification Command |
| -------- | ----------- | -------------------- |
| Perf     | < 200ms     | `npm run test:perf`  |

## 4. Out of Scope

- [Explicitly excluded testing areas]
```

## Fixture Planning Guidance

- **Data Factories**: Prefer factory functions over manual object construction.
- **Network Intercepts**: For frontend integration tests, use tools like Mock Service Worker (MSW) to intercept and mock HTTP requests.
- **Database State**: For backend tests, use a clean database state per test or an in-memory database to ensure isolation.
