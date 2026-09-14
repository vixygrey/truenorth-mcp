---
name: release-branch
description: Make the merge, PR, keep, or discard decision for a feature branch, verify the coverage gates, create the PR, and clean up the worktree. Use it when a feature is done and ready to ship, or when the user says "release", "merge", or "open a PR".
---

# Release Branch

> **HARD GATE**: Do NOT merge or release when tests fail or a coverage gate is not met. When the branch is red, return to `develop-tdd` to fix a regression or add a missing test before you proceed.

Finalize a completed feature branch: verify the coverage gates, integrate onto
`main`, and clean up the worktree.

## Additional modes

- `--hotfix`: cherry-pick to main and tag. Skip the PR in solo mode.
- `--squash-state`: squash the `chore(state):` commits before the merge.

## Integrate mode

Read the `workflow_mode` key from `.agent/tasks/state.yml` (`team-pr` or `solo-git`).

| Mode           | When                               | Ship path                                   |
| -------------- | ---------------------------------- | ------------------------------------------- |
| **solo-local** | `workflow_mode: solo-git`          | Fast-forward the default branch locally     |
| **team-pr**    | `workflow_mode: team-pr` (default) | `gh pr create`, then `gh pr merge --squash` |

When unsure, prefer solo-local. Also read the `vcs.kind` value. Git follows the
procedures below. Jujutsu uses workspaces and bookmarks, and must not call a
Git-only landing step.

## Process

### 1. Final verification

Run the full test, typecheck, and lint through the `truenorth_verify_gate` tool.
Then confirm the commits.

- [ ] All tests pass, no type error, no lint violation.
- [ ] Every commit follows Conventional Commits.
- [ ] No `Co-authored-by` footer in any commit body. The human author owns the commit.

Check the commit range against the Conventional Commits format and reject an
AI-attribution footer:

```bash
git log main...HEAD --oneline | grep -vE "^[a-f0-9]+ (feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\(.+\))?!?: .+$" && echo "Non-conventional commit found" || echo "Commits verified"
git log main...HEAD --format="%B" | grep -qiE 'co[- ]authored[- ]by' && echo "Co-authored-by footer found, blocked" || echo "No AI attribution"
```

### 2. Coverage check

- [ ] Overall coverage 80% or more. Business-logic coverage 95% or more.

### 2a. Security gate

- [ ] A security review exists and is fresh, matching the current branch diff.
- [ ] No unresolved HIGH finding with confidence 8 or more, or every one is documented with a sign-off rationale.

When the review is missing or stale, run `security-review` inline. A finding
blocks the merge unless it is documented.

### 2b. Traceability gate

Run `gate-trace` before the merge. A FAIL blocks the merge. A CONCERNS requires an
explicit override in `.agent/tasks/state.yml`. A WAIVED applies when no matrix is
available.

> **Adversarial refute framing**: the final pre-merge check refutes, it does not rubber-stamp. Before you declare ready, actively try to disprove traceability completeness: a missing story tag, absent verify evidence, a stale security review. Proceed only when the refutation fails.

### 3. Diff review

- [ ] Every commit is intentional, no secret, convention-compliant.

### 4. Decision

Options: release (solo-local), open PR, keep the branch, or discard.

### 5. Integrate

Run `commit-message` first. Git solo-local fast-forwards the default branch.
Jujutsu team mode advances and pushes a bookmark. The `-m` flag is mandatory for a
commit or describe operation.

```bash
# Jujutsu team PR
jj describe -m "feat(scope): description"
jj bookmark set <task-slug> -r @
jj git push -b <task-slug>
```

### 6. Create the PR (team-pr only)

Create the pull request, then squash-merge it, so each PR becomes one commit on
`main`.

```bash
gh pr create --title "..." --body "$(cat <<'EOF'
## Summary

- ...

## Test plan
- [ ] ...
EOF
)"
gh pr merge --squash --delete-branch
```

The release itself is tag-driven. A `v*` tag triggers the release workflow, which
builds and publishes. The merge does not publish on its own.

### 7a. Archive the completed epic capsule

> **HARD GATE**: when every epic story is done, archive the capsule.

Move the completed capsule to the archive under the task group directory.

### 7b. CI verification

> **HARD GATE**: Do NOT declare success until CI completes. Confirm three independent facts: the commit landed, the workflow is green, and the release is visible. See [REFERENCE.md](REFERENCE.md#three-independent-facts-release).

- [ ] CI passes. Set `release.ci_verified: true` in `state.yaml`.
- On failure: set `handoff.next_skill = fix-bug`.

### 8. Clean up and return

Git: prune the worktree, delete the branch, return to main. Jujutsu:
`jj workspace forget <workspace>` only after integration. Do not delete its
bookmark implicitly.

Report: "Branch released.".

## Verify

Confirm `gh` is available, `.agent/tasks/state.yml` exists, and the verify-work skill is
present. Run the final verification through the `truenorth_verify_gate` tool.
