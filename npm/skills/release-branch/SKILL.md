---
name: release-branch
description: Validate and integrate a finished Git branch through the configured solo or pull-request workflow. Invokes trace-requirement gate before integration. Use it when a feature is ready to ship, or when the user says "release", "merge", or "open a PR".
kind: scripted
verify: bash skills/release-branch/scripts/tests/run.sh
---

# Release Branch

> **HARD GATE**: Do NOT merge or release when tests fail or a coverage gate is not met. When the branch is red, return to `develop-tdd` to fix a regression or add a missing test before you proceed.

Finalize a completed Git branch through the configured mode. Clean up only after approval.

## Integration mode

Read `workflow_mode` from `.agent/tasks/state.yml`. The only valid values are
`solo-git` and `team-pr`. A missing or unknown value blocks release. Never infer
or silently change the mode.

| Mode       | Ship path                                   |
| ---------- | ------------------------------------------- |
| `solo-git` | Fast-forward the default branch locally     |
| `team-pr`  | Push, create a PR, then squash-merge the PR |

TrueNorth release workflows support Git only. Run the side-effect-free route
check before any integration command:

```bash
bash skills/release-branch/scripts/check-route.sh \
  --mode "$WORKFLOW_MODE" --action inspect
```

## Process

### 1. Final verification

Run the full test, typecheck, and lint through `truenorth_verify_gate`. Confirm
that the current Git branch is not the default branch, the worktree is clean,
the commit range is nonempty, and every commit follows Conventional Commits.
Reject an AI-attribution footer.

```bash
git status --short --branch
git log <default-branch>..HEAD --oneline
git log <default-branch>..HEAD --format="%B"
```

Stop when verification fails. Do not land a branch with an empty commit range.

### 2. Coverage check

Read `quality_gates.coverage.{overall_minimum_percent,business_logic_minimum_percent}`
from `.agent/config/rules.yml`. Missing thresholds block release. Run the project
coverage command and compare observed values. Never use skill-level defaults.

### 2a. Security gate

- [ ] A security review exists and is fresh, matching the current branch diff.
- [ ] No confirmed unresolved HIGH or CRITICAL finding remains.
- [ ] Every `Needs investigation` item with potentially HIGH or CRITICAL impact
      has an explicit disposition.

When the review is missing or stale, run `security-review` inline. A confirmed
blocker or undispositioned high-impact investigation item blocks the merge unless
an explicit sign-off documents the decision.

### 2b. Traceability gate

Run `trace-requirement gate`; it consumes the current report artifact. `FAIL`
blocks. `CONCERNS` requires the configured override. `WAIVED` requires approval
with owner, rationale, scope, and timestamp. Missing or stale evidence fails.
Before merging, try to disprove completeness with missing task, test, change, or
verification evidence.

### 3. Diff review

- [ ] Every commit is intentional, no secret, convention-compliant.

### 4. Select the configured route

Do not offer or select a different integration mode. `workflow_mode` is the
decision. If the user wants another route, update the state only after explicit
confirmation, then rerun every gate.

### 5. Integrate

Run `commit-message` first.

For `solo-git`, fetch the default branch and confirm that it can fast-forward to
the feature branch. Ask for explicit merge approval. Immediately before the
fast-forward or push, validate that approval:

```bash
bash skills/release-branch/scripts/check-route.sh \
  --mode solo-git --action merge --approved
```

For `team-pr`, push the feature branch and create the pull request with `gh`.
Wait for every required check to pass. Ask for explicit merge approval, then
validate it immediately before squash merge:

```bash
bash skills/release-branch/scripts/check-route.sh \
  --mode team-pr --action merge --approved
gh pr merge --squash
```

Approval to create a PR does not approve its merge.

### 6. Hotfix tag and publication

`--hotfix` does not imply approval to tag or publish. Ask separately before
each action, then run the corresponding guard:

```bash
bash skills/release-branch/scripts/check-route.sh \
  --mode "$WORKFLOW_MODE" --action tag --approved
bash skills/release-branch/scripts/check-route.sh \
  --mode "$WORKFLOW_MODE" --action publish --approved
```

A `v*` tag triggers the release workflow. Confirm the exact tag before creation.

### 7. Archive the completed task group

When every group story is done, archive the capsule under the task group
directory.

### 8. Verify the landed change

Confirm the expected commit landed and all required workflows are green. Set
`release.ci_verified: true` only after both facts are observed. On failure, set
`handoff.next_skill = fix-bug`.

### 9. Clean up and return

List every branch and worktree path that cleanup will remove. Ask for explicit
approval, then validate it immediately before deletion:

```bash
bash skills/release-branch/scripts/check-route.sh \
  --mode "$WORKFLOW_MODE" --action cleanup --approved
```

Prune the worktree, delete the feature branch, and return to the default branch.
Do not use force deletion without separate explicit approval.

## Verify

Verify with `bash skills/release-branch/scripts/tests/run.sh`. Then run `truenorth_verify_gate`.
