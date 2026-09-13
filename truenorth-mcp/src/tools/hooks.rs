//! The two emitted git hook templates: `commit-msg` and `post-merge`.
//!
//! The scaffold emits these shell scripts into `.githooks/`, templated per methodology
//! profile (Requirements 6, 7). Git invokes them outside the MCP session, so they are
//! plain POSIX shell, not runtime behavior. This module renders the script text from a
//! [`Profile`]; the scaffold (task 9) writes the text through the audited repo-seed path.
//!
//! The `commit-msg` hook validates Conventional Commits format and a profile-driven id
//! reference (Requirement 6). The `post-merge` hook sweeps only local topic branches that
//! are provably present on the trunk (Requirement 7).
//!
//! Requirements: 6.1 to 6.10, 7.1 to 7.8. Design: agent-workspace-profiles §6, §7.

// The hook renderers are consumed by the scaffold (task 9). They are unused until the
// scaffold wires them, so the module-scoped allow prevents a premature dead-code error
// under `clippy -D warnings`. Remove this allow once task 9 wires the consumer.
#![allow(dead_code)]

use crate::engine::profile::Profile;

/// The trunk branch the post-merge sweep runs on and never deletes.
const TRUNK_BRANCH: &str = "main";

/// Render the `commit-msg` hook for a profile (Requirement 6).
///
/// The hook accepts a generated merge, revert, fixup, or squash message with exit 0
/// (Requirement 6.10). It accepts a subject that uses an approved type and matches the
/// `type(scope): description` format, and rejects otherwise with a non-zero exit
/// (Requirements 6.3, 6.4, 6.5). When the profile requires an id, the hook requires an
/// issue or ticket reference and rejects its absence (Requirements 6.6, 6.7). When the
/// profile does not, a message with no id passes (Requirement 6.8).
pub fn commit_msg_hook(profile: Profile) -> String {
    let require_issue_id = if profile.require_issue_id {
        "yes"
    } else {
        "no"
    };
    format!(
        r##"#!/bin/sh
# commit-msg hook, emitted by truenorth-mcp for the `{profile_name}` profile.
# git passes the commit-message file as $1 (Requirement 6.2).
set -eu

msg_file="$1"
subject=$(head -n1 "$msg_file")

# Merge, revert, fixup, and squash messages pass unchanged (Requirement 6.10).
case "$subject" in
  "Merge "*|"Revert "*|"fixup! "*|"squash! "*) exit 0 ;;
esac

# Conventional Commits type set and format (Requirements 6.3, 6.4, 6.5).
types='feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert'
if ! printf '%s' "$subject" | grep -Eq "^($types)(\([a-z0-9._-]+\))?!?: .+"; then
  echo "commit-msg: subject must be 'type(scope): description'." >&2
  echo "  allowed types: $types" >&2
  exit 1
fi

# Profile-driven id reference (Requirements 6.6, 6.7, 6.8).
REQUIRE_ISSUE_ID="{require_issue_id}"
if [ "$REQUIRE_ISSUE_ID" = "yes" ]; then
  if ! grep -Eq '(#[0-9]+|[A-Z]+-[0-9]+)' "$msg_file"; then
    echo "commit-msg: an issue or ticket id reference is required (for example #12 or ABC-12)." >&2
    exit 1
  fi
fi

exit 0
"##,
        profile_name = profile.name,
        require_issue_id = require_issue_id,
    )
}

/// Render the `post-merge` hook for a profile (Requirement 7).
///
/// The hook sweeps only while on the trunk (Requirement 7.2). On a detached HEAD or an
/// undeterminable branch it exits 0 and deletes nothing (Requirement 7.3). It deletes a
/// local topic branch only when the branch matches the profile pattern and is provably
/// present on the trunk, where provably present means the tip is an ancestor of the trunk
/// tip or `git diff trunk..branch` reports no differences (Requirements 7.4, 7.5). It
/// never deletes the current branch or the trunk (Requirement 7.6), and retains any
/// branch not provably present (Requirement 7.8).
pub fn post_merge_hook(profile: Profile) -> String {
    format!(
        r##"#!/bin/sh
# post-merge hook, emitted by truenorth-mcp for the `{profile_name}` profile.
# Deletes only local topic branches provably present on the trunk (Requirement 7).
set -eu

TRUNK="{trunk}"
BRANCH_PATTERN='{branch_pattern}'   # from the profile (Requirement 7.7)

current=$(git symbolic-ref --quiet --short HEAD 2>/dev/null || true)
# Detached HEAD or an undeterminable branch: exit 0, delete nothing (Requirement 7.3).
[ -z "$current" ] && exit 0
# Sweep only while on the trunk (Requirement 7.2).
[ "$current" != "$TRUNK" ] && exit 0

git for-each-ref --format='%(refname:short)' refs/heads | while read -r b; do
  # Never the current branch or the trunk (Requirement 7.6).
  [ "$b" = "$current" ] && continue
  [ "$b" = "$TRUNK" ] && continue
  # Match the profile branch pattern (Requirement 7.7).
  printf '%s' "$b" | grep -Eq "$BRANCH_PATTERN" || continue
  # Provably present: ancestor of trunk OR empty diff trunk..branch (Requirement 7.4).
  if git merge-base --is-ancestor "$b" "$TRUNK" 2>/dev/null \
     || [ -z "$(git diff "$TRUNK".."$b")" ]; then
    git branch -D "$b"          # delete only the provably-merged branch (Requirement 7.5)
  fi
  # Otherwise retain the branch (Requirement 7.8).
done

exit 0
"##,
        profile_name = profile.name,
        trunk = TRUNK_BRANCH,
        branch_pattern = profile.branch_pattern,
    )
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `hooks`.
#[cfg(test)]
#[path = "hooks_tests.rs"]
mod tests;
