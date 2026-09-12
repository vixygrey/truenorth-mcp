---
name: kickoff-branch
description: Create an isolated Git worktree or branch, or a Jujutsu workspace, then verify a clean test baseline before code. Use it when starting a feature or task.
---

# Kickoff Branch

> **HARD GATE**: direct Git work on `main` or `master`, or reuse of an unrelated Jujutsu change, is prohibited. Create a feature branch or worktree, or a Jujutsu workspace or change.
>
> **HARD GATE**: Do NOT proceed with development until preflight passes on the default branch. A red preflight blocks branch creation and all forward work. Invoke `quick-fix` or `fix-bug`.

Create an isolated Git worktree or Jujutsu workspace before code. Preflight must be
green first.

## Process

### 1. Confirm the task name

Ask when it is not known: "What is the name of this feature or task?". Use it as
the branch-name slug (kebab-case, at most 40 characters).

### 2. Select the VCS procedure

Read the `vcs.kind` value from `state.yaml`. For `jj`, do not run the Git blocks
below. Verify with `jj status` and `jj log -r '::@' -n 5`, create isolation with
`jj workspace add ../<task-slug> -r @`, then run
`jj -R ../<task-slug> describe -m "feat: <task>"`. Record the new stable change id.
For `git`, continue below.

### 2a. Anchor Git on the default branch

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

> **HARD GATE**: acquire the story lock in `specs/agent-locks.yaml` before you run the tests. When the story is already locked, abort. When it is unlocked, add the entry and proceed.

Run preflight, the project's full local verification stack, through the
`truenorth_verify_gate` tool, and confirm green before you write any code.

- [ ] Preflight passes, every chained gate green.
- [ ] No type error.
- [ ] No lint error.

When preflight is red, stop. Route to `quick-fix` or `fix-bug`. Fix it before
kickoff continues.

### 5. Confirm readiness

Report the green preflight, the branch, and the worktree. Suggest the next skill:
`develop-tdd` or `execute-plan`.

## Handoff

Gate: READY. Next: develop-tdd.
Writes: `state.yaml` `handoff.next_skill = develop-tdd`.
