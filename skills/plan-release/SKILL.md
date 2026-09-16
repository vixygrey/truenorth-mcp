---
name: plan-release
description: "A release-index builder. Sequence elaborated task groups into .agent/tasks/release-plan.yml with WSJF ordering and BCP baselines. Not a planning-spine substitute: it does not scope work or write story tasks. Use it after elaborate-spec when the user wants a versioned release index of task groups."
---

# Plan Release

> **HARD GATE** — Do NOT run this skill unless `elaborate-spec` has produced a clear spec or the user has already defined the feature in detail. If the problem is still fuzzy, run `elaborate-spec` first.
> **HARD GATE** — `.agent/product/scope.yml` must exist. If missing, run `scope-work` first.

Synthesize the conversation context into `.agent/tasks/release-plan.yml` (index) and shard detail into the task group under `.agent/tasks/`. No new interview — only clarify if something is genuinely ambiguous.

## Outputs

| File                                       | Content                                                                                                                     |
| ------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------- |
| `.agent/tasks/release-plan.yml`            | `release.version`, semver bump hint, WSJF-ordered group list with `id`, `capsule_dir`, `wsjf`, `bcps` — **no story status** |
| `.agent/tasks/<capsule>/group.yml`         | Group manifest: `id`, `title`, `wsjf`, `total_bcps`, `status`, `stories[]` list                                             |
| `.agent/tasks/<capsule>/eNNsYY-<slug>.md`  | Story spec in the countable-story-format with 20 sections and Gherkin acceptance criteria                                   |
| `.agent/tasks/<capsule>/eNNsYY-tasks.yaml` | Decoupled task checklist with `verify:` commands per task                                                                   |
| `.agent/tasks/execution-status.yml`        | Flat key-value store for story status (`eNNsYY: todo`)                                                                      |

## Task Group Structure

All task groups use capsule directories (no flat/folder distinction):

```
.agent/tasks/e01-auth-system/
├── group.yml              # Group manifest
├── adr/                   # Group-local ADRs (created lazily)
├── e01s01-login.md        # Story spec (countable-story-format)
├── e01s01-tasks.yaml      # Decoupled task checklist
├── e01s02-jwt.md          # Story spec
└── e01s02-tasks.yaml      # Decoupled task checklist
```

**Rationale:** Capsule dirs achieve change isolation (C9), enable archive pruning (C2/C6), and enforce SRP by decoupling spec `.md` from execution `-tasks.yaml` (C1).

## Process

### 1. Draft task groups and stories

From the conversation context, define:

- **Task groups** — `e01`, `e02`, … (stable IDs; WSJF order in `release-plan.yaml` only)
- **Stories** — `e01s01`, `e01s02`, … with Gherkin acceptance criteria

WSJF-sort the groups: score = (Business Value + Time Criticality + Risk Reduction) / Job Size. Highest score first.

> **Security risk boost:** If the security review report for a task group identifies HIGH or CRITICAL risk, add +2 to the WSJF numerator (BV + TC + RR + 2) to reflect the urgency of addressing security concerns before they ship. Document the boost in the group note field in release-plan.yaml.

### 2. Write acceptance criteria (Gherkin)

For each story, write at least one happy-path and one edge-case scenario (countable format §17 if maturity ≥ 3).

### 3. Write tasks with verify commands

Every task must have a `verify:` command. No verify command = not a task.

### 4. Save .agent/tasks/release-plan.yml

> **Do NOT hand-track the real version.** The release is tag-driven. A `v*` tag
> triggers the release. The `version` here is a non-authoritative label. Read the
> real number from the published release. Set the `bump_hint`, the expectation.

```yaml
release:
  version: "2.29.0" # a mirror of the next expected tag, not authoritative
  codename: "Feature Name"
  status: planning # planning | in_progress | released
  bump_hint: minor # patch | minor | major
groups:
  - id: e01
    title: Auth System
    wsjf: 4.5
    capsule_dir: epics/e01-auth-system
  - id: e02
    title: User Profile
    wsjf: 3.8
    capsule_dir: epics/e02-user-profile
```

### 5. Save group manifest (`group.yml`)

Each task group directory contains a `group.yml` manifest:

```yaml
id: e01
title: Auth System
wsjf: 4.5
total_bcps: 8
status: in_progress
stories:
  - id: e01s01
    title: Login
    bcps: 3
    status: todo
    spec: e01s01-login.md
    tasks: e01s01-tasks.yaml
  - id: e01s02
    title: JWT Token Management
    bcps: 5
    status: todo
    spec: e01s02-jwt.md
    tasks: e01s02-tasks.yaml
```

### 6. Save story specs (countable-story-format .md)

Each story becomes a standalone `.md` file following the countable-story-format. Minimum: maturity 3 (Countable) with all 20 sections present. Acceptance criteria in §17 use Gherkin scenarios.

### 7. Save decoupled task files (`-tasks.yaml`)

Each story has a decoupled `-tasks.yaml` with implementation steps:

```yaml
story_id: e01s01
title: Login
status: todo
bcps: 3
tasks:
  - id: 1
    description: "Add login form component tests"
    verify: "npm test -- login-form.test.tsx"
    status: todo
  - id: 2
    description: "Implement login form with validation"
    verify: "npm test -- login-form.test.tsx"
    status: todo
```

> **HARD GATE** — Every task MUST have a runnable `verify:` command. No `verify:` = not a task.

Confirm the release plan and the task groups parse as valid YAML.

### 7b. Generate bug summary

Read the bug references under `.agent/tasks/bugs.yml` and add a `bugs:` section to `release-plan.yaml` with totals by status (`fixed`, `deferred`, `wontfix`, `open`): `bugs: { total: N, fixed: N, deferred: N, wontfix: N, ref: .agent/tasks/bugs.yml }`.

### 8. Sync execution status

Update `.agent/tasks/execution-status.yml` so each story key reflects its status from the
group manifests.

### 9. Snapshot on planning close (optional)

Copy to `.agent/product/snapshots/release-<version>/` when the user approves the plan.

### 10. Suggest next steps

- Run `assess-impact` before `plan-work` for any story touching existing modules.
- Run `plan-work` per story for detailed steps inside the group shard.
- Run `change-request` if a new requirement arrives mid-flight.
