---
name: kickoff-branch
description: Create an isolated Git worktree or branch, then verify a clean test baseline before code. Use it when starting a feature or task.
kind: prose
---

# Kickoff Branch

> **HARD GATE**: direct work on `main`, `master`, or another default branch is
> prohibited. Create a Git feature branch or worktree.
>
> **HARD GATE**: Do NOT proceed with development until preflight passes on the
> default branch. A red preflight blocks branch creation and all forward work.
> Invoke `quick-fix` or `fix-bug`.

Create an isolated Git worktree or branch before code. Preflight must be green
first. TrueNorth does not support Jujutsu workflows.

## Process

### 1. Confirm the task name

Ask when it is not known: "What is the name of this feature or task?". Use it as
the branch-name slug (kebab-case, at most 40 characters).

### 2. Anchor Git on the default branch

> **HARD GATE**: Git kickoff MUST start from an updated, clean default branch in the primary repository root, not a linked worktree.

```bash
DEFAULT=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@^refs/remotes/origin/@@' || echo main)
git checkout "$DEFAULT"
git pull --ff-only origin "$DEFAULT"   # skip if no remote
git status                             # the working tree MUST be clean
git log --oneline -5
```

**Spec-only pre-kickoff**: before you enforce the clean-tree gate, check whether
the dirty files are spec artifacts. A dirty tree of only `specs/` files can be
committed as a checkpoint after you confirm. A dirty tree with a non-spec file
(under `src/`, a SKILL.md, and so on) still enforces the full clean-tree gate.
When it is not clean and not spec-only, stash or commit before you proceed.

### 3. Git pre-flight and conflict resolution

Before you create the worktree, verify the target is clean: check for an existing
directory, an existing branch, and a ghost worktree (metadata present but the
directory is gone).

- Directory exists: ask the user to use it or delete it.
- Branch exists with no worktree: ask to use the existing branch
  (`git worktree add ../<task-slug> <task-slug>`) or delete it.
- Ghost worktree: run `git worktree prune` to clear the stale metadata.

### 4. Create the Git worktree and branch

```bash
git worktree add ../<task-slug> -b <task-slug>
cd ../<task-slug>
```

When the user prefers a branch without a worktree:

```bash
git checkout -b <task-slug>
```

### 4a. Verify a clean baseline

There is no in-repo lock file. Before you run the tests, confirm no other agent works the same story. When another agent holds it, abort.

Run preflight, the project's full local verification stack, through the
`truenorth_verify_gate` tool, and confirm green before you write any code.

- [ ] Preflight passes, every chained gate green.
- [ ] No type error.
- [ ] No lint error.

When preflight is red, stop. Route to `quick-fix` or `fix-bug`. Fix it before
kickoff continues.

### 5. Confirm readiness

Report the green preflight, the branch, and the worktree. Suggest the next skill:
`develop-tdd` or `execute-group`.

## Handoff

Gate: READY. Next: develop-tdd.
Writes: `.agent/tasks/state.yml` `handoff.next_skill = develop-tdd`.
