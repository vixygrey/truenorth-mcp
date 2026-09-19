# Skills: Release

The Release phase integrates a finished branch: draft the commit message, verify the
gates, and land the change. Two skills run in order.

For the full arc across phases, see [The skill workflow](The-skill-workflow). For the
alphabetical list, see [Skill index](Skill-index).

---

## commit-message

Review the working-tree changes and draft a Conventional Commits message with the SemVer
bump it implies.

- **What it does**: reads the changes through the VCS (git or Jujutsu), inventories the
  changed paths, decides the commit shape (one atomic commit, or several when concerns
  mix), classifies the SemVer bump, writes a `type(scope): description` message, and notes
  which defensive-code categories were touched.
- **When to use it**: when the user wants to commit recent work, or to prepare a
  Conventional Commits message before a git commit.
- **Inputs**: the working-tree diff and the conversation intent.
- **Outputs**: a proposed commit message, the release bump it would drive, and the optional
  native commit command.
- **Modes**: default, or `--fix-type` (force `type=fix`).
- **Hard gate**: the message must follow `type(scope): description`. No vague message. No
  `Co-authored-by` footer; the human author owns the commit.
- **Handoff**: gate READY, next `release-branch`.

## release-branch

Make the merge, PR, keep, or discard decision for a finished branch, verify the gates, and
clean up.

- **What it does**: runs final verification (tests, typecheck, lint through
  `truenorth_verify_gate`, plus a Conventional Commits and AI-attribution check), the
  coverage gate, the security gate (running `security-review` inline if stale), and the
  traceability gate (`gate-trace`), then integrates per the workflow mode, archives the
  completed capsule, and verifies CI.
- **When to use it**: when a feature is done and ready to ship, or on "release", "merge", or
  "open a PR".
- **Inputs**: the finished branch, `workflow_mode` and `vcs.kind` from the state.
- **Outputs**: a merged change (or an open PR), a pruned worktree, CI confirmation.
- **Modes**: default, `--hotfix` (cherry-pick and tag), `--squash-state` (squash the
  `chore(state):` commits). Integrate mode is solo (fast-forward locally) or team-pr
  (`gh pr create` then `gh pr merge --squash`), read from `workflow_mode`.
- **Hard gates**: do not merge when tests fail or a coverage gate is not met. A `gate-trace`
  FAIL blocks the merge. Do not declare success until CI confirms three independent facts:
  the commit landed, the workflow is green, and the release is visible.
- **Note on release**: the release itself is tag-driven. A `v*` tag triggers the release
  workflow, which builds and publishes. The merge does not publish on its own.
