---
name: trace-requirement
description: "Link the story ids from the release plan and the task groups to the implementing code and tests. Produces a traceability report. Use it to verify coverage of a release plan, audit which stories are implemented, or find a dark story with no code."
kind: prose
---

# Trace Requirement

Build a traceability matrix from `.agent/tasks/release-plan.yml` and the task group directories to implementing code and tests. Surfaces gaps in both directions: stories with no code, and code with no story.

## Pre-flight

> **HARD GATE** — `.agent/tasks/release-plan.yml` and the task group directories must exist. If it doesn't, run `plan-release` first.

→ verify: `test -f .agent/tasks/release-plan.yml`

Read `.agent/tasks/release-plan.yml` and the task group directories fully before proceeding.

## Process

### 1. Extract story IDs

From release-plan.yaml, collect all story IDs (for example `e01s01`, `e01s02`, `e02s01`).

→ verify: `grep -rho 'e[0-9]\+s[0-9]\+' .agent/tasks/release-plan.yml 2>/dev/null | sort -u | head -1 | grep -q .`

### 2. Search for story tags in code

Look for `// story: eNNsYY` or `# story: eNNsYY` comments in source files and tests:

```
grep -rn "story: " . --include="*.ts" --include="*.js" --include="*.py" --include="*.sh" | grep -v node_modules
```

→ verify: `[ "$(grep -rl "story: " . --include="*.ts" --include="*.sh" --include="*.py" 2>/dev/null | wc -l | tr -d " ")" -gt 0 ]`

### 3. Build the matrix

For each story ID:

- **Implemented**: list files that contain `// story: eNNsYY`
- **Tested**: list test files that contain `// story: eNNsYY`
- **Dark**: story has no tag in any file — flag as unimplemented

For each tagged file with no matching story ID in release-plan.yaml:

- **Orphan**: code exists but story was removed or never planned — flag for cleanup

### 4. Write the traceability report

```
## Story Coverage

| Story  | Title              | Files | Tests | Status    |
|--------|--------------------|-------|-------|-----------|
| e01s01 | [title]            | 2     | 1     | Covered   |
| e01s02 | [title]            | 0     | 0     | Dark      |

## Orphan Code (no story tag)
- [file]: contains untagged implementation

## Gaps (dark stories)
- Story e01s02: no implementation found → run plan-work

## Coverage summary
Stories: [X] covered / [Y] dark / [Z] total
```

→ verify: the traceability report counts `Covered` and `Dark` stories.

Suggest `plan-work` for each dark story found.
