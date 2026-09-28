---
name: quick-fix
description: "A streamlined path for a trivial data-only fix on a small isolated Git branch. Skips TDD for a change that is purely data with no logic risk. Aborts with a fallback to investigate-bug when a guardrail triggers."
kind: prose
---

# Quick Fix

> **HARD GATE**: ALL entry criteria must pass before invoking quick-fix. Never
> mutate `main`, `master`, or another default branch directly. Run
> `kickoff-branch` first. If any guardrail triggers, abort and fall back to
> `investigate-bug`. Do not use quick-fix for logic changes, multi-file edits,
> or diffs greater than five lines.

Fast-track trivial data-only fixes without skipping branch isolation or release
safety.

## Discovered gate failures (e51s04)

A **Preflight**, **golden suite**, or **baseline red** failure discovered during unrelated work is valid quick-fix entry when the root cause is a data-only gap (missing key, typo, stale config) — **no user-reported bug required**. If guardrails abort (logic change, >1 file, >5 lines), fall back to `fix-bug` instead of narrating and continuing.

## Entry Criteria (ALL must be true)

Before invoking quick-fix, evaluate every item in this checklist:

1. **Purely data change** — adding a missing key, fixing a typo, updating a config value, correcting a constant
2. **No logic change** — no function signature, condition, loop, or control flow is modified
3. **No refactor risk** — the change does not reorganize or rename existing structures
4. **No API surface change** — no exported symbol, interface, or contract changes
5. **Verifiable with a single assertion** — one test, one curl, one grep can prove it works
6. **Affects ≤ 1 file**
7. **Affects ≤ 5 lines changed**

## Guardrails (HARD ABORT — all must pass)

If ANY guardrail triggers, **abort immediately** and suggest `investigate-bug` instead:

| Guardrail          | Check                                                     |
| ------------------ | --------------------------------------------------------- |
| **>1 file**        | The fix touches more than one file                        |
| **>5 lines**       | The diff exceeds 5 lines                                  |
| **Logic change**   | Any function signature, condition, or loop is modified    |
| **Complex verify** | The verify command is more than one pipeline              |
| **Test breakage**  | Running `npm test` or equivalent breaks any existing test |

> **Fallback:** If any guardrail triggers, tell the user: _"This fix exceeds quick-fix guardrails. Use `investigate-bug` for the full TDD bug-fix chain instead."_

## Fast-path workflow

Only three skills are needed for an eligible fix:

```text
kickoff-branch -> create a small isolated Git branch
quick-fix      -> apply the data change, verify, and commit
release-branch -> integrate through the configured workflow mode
```

**Skipped skills with justification:**

| Skipped skill     | Why skipped                                      |
| ----------------- | ------------------------------------------------ |
| `investigate-bug` | Root cause is an obvious data gap                |
| `develop-tdd`     | One focused assertion proves the data correction |

Include the justification in the `fix:` commit body for the audit trail.

## Process

### 1. Evaluate entry criteria

Run the entry criteria checklist above. If any criterion fails, abort and
suggest `investigate-bug`.

### 2. Create the branch

Run `kickoff-branch` and confirm that the current branch is not the default
branch. TrueNorth supports Git release workflows only. Stop with an actionable
error when the repository is not a Git working tree.

### 3. Apply the fix

Make the data change. Keep it to five changed lines or fewer in one file.

### 4. Verify

Run the single-assertion verify command. Example:

```bash
grep -q "Bosnia" src/flags.js
```

### 5. Commit

```bash
git add <file>
git commit -m "fix(<scope>): <description>" -m "Skipped skills:
- investigate-bug: root cause is an obvious data gap
- develop-tdd: one focused assertion proves correctness"
```

### 6. Release

Invoke `release-branch`. It reads `workflow_mode`, validates the Git route, and
asks for approval before merge or cleanup.

## Example

### Bosnia flag missing from FLAGS dictionary

```gherkin
Given a bug where FLAGS dictionary is missing entry "Bosnia"
And no logic depends on the missing entry (purely a data gap)
When the agent invokes quick-fix
Then the missing entry is added to the dictionary
And a one-line verify confirms the key exists: grep -q "Bosnia" src/flags.js
And a fix: commit is created with the skipped-skills rationale in the body
And the change is ready for release-branch
```
