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
- [ ] Coverage gates meet the minimums in `.agent/config/rules.yml`
- [ ] CONVENTIONS.md compliance verified
- [ ] PR Title follows Conventional Commits (for automated release)

## Narrative artifacts
- [List any human-narrative files under specs/ produced or updated]
EOF
)"
```

## Worktree cleanup details

```bash
# Run only after explicit cleanup approval.
bash skills/release-branch/scripts/check-route.sh \
  --mode "$WORKFLOW_MODE" --action cleanup --approved
git worktree prune
git worktree remove ../<branch-name>
git branch -d <branch-name>
```

If cleanup finds uncommitted changes, stop. Force removal requires a new,
separate approval that names the dirty worktree.

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

For `team-pr`, wait for every required GitHub check before asking for merge
approval. For `solo-git`, run the configured local gate before asking for
integration approval. A missing `gh` command blocks only `team-pr`.

The expected outcomes are:

- all required checks green: request merge approval;
- any check failed: set `handoff.next_skill = fix-bug`;
- timeout: retry or investigate without merging.

Set `release.ci_verified: true` only after the configured checks are green.

---

## Solo Git integration

Use this route only when `workflow_mode: solo-git`. Confirm a fast-forward is
possible, ask for explicit merge approval, and run the route guard immediately
before changing the default branch. Do not silently fall back to this route
from `team-pr`.

## Handoff

Gate: READY -> next: survey-context
Writes: `.agent/tasks/state.yml` `handoff.next_skill = survey-context`

---

## Solo Git command sequence

```bash
FEATURE_BRANCH=<task-slug>
DEFAULT_BRANCH=<configured-default-branch>

git fetch origin "$DEFAULT_BRANCH"
git merge-base --is-ancestor "origin/$DEFAULT_BRANCH" "$FEATURE_BRANCH"

# Continue only after explicit merge approval.
bash skills/release-branch/scripts/check-route.sh \
  --mode solo-git --action merge --approved
git switch "$DEFAULT_BRANCH"
git merge --ff-only "$FEATURE_BRANCH"
git push origin "$DEFAULT_BRANCH"
```

Cleanup is a separate action with separate approval. Follow
[Worktree cleanup details](#worktree-cleanup-details).
