# Release Branch — Reference

## Navigation

| Lines   | Section                              |
| ------- | ------------------------------------ |
| 1       | Title                                |
| 3–22    | Navigation                           |
| 23–31   | PR body template (team-pr mode)      |
| 32–35   | Summary                              |
| 36–41   | Verify                               |
| 42–47   | Narrative artifacts                  |
| 48–58   | Worktree cleanup details             |
| 59–81   | Cycle-time recording                 |
| 82–100  | Why not story_start minus story_end? |
| 101–121 | CI verification                      |
| 122–127 | Solo-local fallback detail           |
| 128–134 | Handoff                              |
| 135–159 | Reference block 1                    |

# Release Branch — Reference

## PR body template (team-pr mode)

```bash
PR_TITLE="<type>(<scope>): <description>"
echo "$PR_TITLE" | grep -vE "^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\(.+\))?!?: .+$" && echo "❌ ERROR: PR Title must follow Conventional Commits"

gh pr create \
  --title "$PR_TITLE" \
  --body "$(cat <<'EOF'
## Summary
- [What this PR does]
- [Key decisions made]

## Verify
- [ ] All tests pass
- [ ] Coverage gates met (≥80% overall, ≥95% business logic)
- [ ] CONVENTIONS.md compliance verified
- [ ] PR Title follows Conventional Commits (for automated release)

## Narrative artifacts
- [List any human-narrative files under specs/ produced or updated]
EOF
)"
```

## Worktree cleanup details

```bash
# From the main repo root
git worktree prune
git worktree remove ../<branch-name> 2>/dev/null || true
git branch -d <branch-name>
```

If `git worktree remove` fails due to uncommitted changes, ask: "There are uncommitted changes in the worktree. Force remove? (y/n)". If yes: `git worktree remove -f ../<branch-name>`.

## Cycle-time recording

Cycle-time metrics are out of scope. The cycle-time ledger is removed. This
section records the retired design for reference only.

The old approach derived delivery metrics from git history (replaced
hand-arithmetic), using the commit range `$(git merge-base main HEAD)..HEAD`.

The row recorded two separated metrics:

- **effort_hours** — ADDITIVE. Idle-stripped estimated effort from git commit
  history (git-hours model: 120-min session threshold, 120-min first-commit pad).
  Sums exactly to whole-repo effort. NO hand-arithmetic, NO wall-clock includes.
- **lead_time_minutes** — calendar latency from first commit to merge.
  Median-aggregated across stories; NEVER summed.

The script also runs an additivity self-check: Σ(story effort) == whole-repo effort
within rounding tolerance.

### Why not story_start minus story_end?

The previous hand-arithmetic approach (survey-context writes `story_start`,
release-branch writes `story_end`, agent hand-computes `cycle_minutes`) was
retired because:

1. It was **agent-self-reported** — trivially fabricated or mis-subtracted.
2. Wall-clock included **overnight/weekend/UAT gaps** — calendar latency,
   not coding effort.
3. The `bcp_per_hour` metric was **computationally meaningless** (velocity
   derived from a latency measurement).

The new approach derives effort from commit history (objective, reproducible)
and lead time from first commit to merge (honest calendar latency). The effort
model uses a 120-min session threshold with a 120-min first-commit pad, and an
additivity self-check.

---

## CI verification

After pushing, wait for CI to complete. Poll every 30 seconds with a 600-second
timeout. Confirm the branch/commit workflows before you land.

**Exit codes:**

- **0** — all workflows green. Set `release.ci_verified: true` in state.yaml.
- **1** — at least one workflow failed. Prints failure URLs. Set `handoff.next_skill = fix-bug`.
- **2** — timeout. CI did not complete. Retry or investigate.
- **0 with warning** — `gh` CLI not available, git-only fallback confirmed push landed but CI status unverified.

The CI-verification step covers: auto-discovery of all workflows for the current
branch/commit, polling until completion, success/failure/timeout exit codes,
and git-only fallback when `gh` CLI is unavailable.

---

## Solo-local fallback detail

The fallback sequence (Path B above) handles the "remote has moved" case with `git pull --rebase`. Use when no automated branch-landing path is available.

**Acceptance:** When fallback runs, main is updated, feature branch is deleted locally, and output states that the fallback merge was used.

## Handoff

Gate: READY -> next: survey-context
Writes: state.yaml handoff.next_skill = survey-context

---

## Reference block 1

```bash
# Fallback: manual squash-merge when land-branch.sh is absent
FEATURE_BRANCH=<task-slug>
DEFAULT_BRANCH=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@^refs/remotes/origin/@@' || echo main)

# Ensure we're on the feature branch
if [ "$(git branch --show-current)" != "$FEATURE_BRANCH" ]; then
  git checkout "$FEATURE_BRANCH"
fi

# Checkout default branch and update
git checkout "$DEFAULT_BRANCH"
git pull --rebase origin "$DEFAULT_BRANCH" 2>/dev/null || git pull origin "$DEFAULT_BRANCH"

# Squash-merge the feature branch
git merge --no-ff "$FEATURE_BRANCH" -m "<conventional-commit-message>"

# Push
git push origin "$DEFAULT_BRANCH"

# Clean up local feature branch
git branch -d "$FEATURE_BRANCH"
```
