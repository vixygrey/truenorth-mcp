# Skills: Release

The Release phase integrates a finished branch: draft the commit message, verify the
gates, and land the change. Two skills run in order.

For the full arc across phases, see [The skill workflow](The-skill-workflow). For the
alphabetical list, see [Skill index](Skill-index).

---

## commit-message

Review the working-tree changes and draft a Conventional Commits message with the SemVer
bump it implies.

- **What it does**: reads the Git changes, inventories the changed paths, decides the
  commit shape, classifies the SemVer bump, writes a `type(scope): description` message,
  and notes which defensive-code categories were touched.
- **When to use it**: when the user wants to commit recent work, or to prepare a
  Conventional Commits message before a git commit.
- **Inputs**: the working-tree diff and the conversation intent.
- **Outputs**: a proposed commit message, the release bump it would drive, and the optional
  native commit command.
- **Modes**: default, or `--fix-type` (force `type=fix`).
- **Hard gate**: the message must follow `type(scope): description`. No vague message.
  Attribution follows `CONVENTIONS.md`; an allowed standard `Co-authored-by:` trailer
  does not change the human author's ownership.
- **Handoff**: gate READY, next `release-branch`.

## release-branch

Validate and integrate a finished Git branch through the configured workflow mode.

- **What it does**: validates the Git route, runs final verification, reads version 2
  coverage policy from `advisory.quality_gates.coverage` in project configuration,
  applies the security and traceability gates, integrates through `solo-git` or
  `team-pr`, verifies the landed change, and cleans up only after approval.
  Coverage policy is advisory to the runtime; release checks require real measurements.
- **When to use it**: when a feature is ready to ship, or on "release", "merge", or
  "open a PR".
- **Inputs**: the finished Git branch and `workflow_mode` from state.
- **Outputs**: a merged change or open PR, green checks, and approved cleanup.
- **Modes**: `solo-git` fast-forwards locally. `team-pr` pushes and creates a GitHub PR.
  The exact state value controls the route. Missing or invalid values block release.
- **Hard gates**: Git only. Do not merge when verification or configured coverage gates
  fail. Merge, tag, publish, and cleanup require separate explicit approval.
  `trace-requirement gate` must PASS or carry a complete explicit waiver.
- **Release boundary**: a merge does not publish. A `v*` tag can trigger publication,
  but tag creation and publication require their own approvals.
