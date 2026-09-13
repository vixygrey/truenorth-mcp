//! Unit tests for the git hook templates.
//!
//! Included from `hooks.rs` via `#[path]`, so `super` is the hooks module. These assert
//! the rendered template content. The behavior of the emitted scripts is covered by the
//! Property 11 and 12 tests, which run the scripts against temp git repos.
//!
//! Requirements: 6.1, 6.6, 6.8, 7.1, 7.7.

use super::*;
use crate::engine::profile::{EPIC_BASED, GENERIC, ISSUE_PER_TASK, KANBAN, MILESTONE_BASED};

#[test]
fn commit_msg_hook_starts_with_a_shebang() {
    assert!(commit_msg_hook(ISSUE_PER_TASK).starts_with("#!/bin/sh\n"));
}

#[test]
fn commit_msg_hook_requires_an_id_for_id_profiles() {
    // epic-based, issue-per-task, and milestone-based require an id (Requirement 6.6).
    for profile in [EPIC_BASED, ISSUE_PER_TASK, MILESTONE_BASED] {
        let hook = commit_msg_hook(profile);
        assert!(
            hook.contains(r#"REQUIRE_ISSUE_ID="yes""#),
            "`{}` must require an id",
            profile.name
        );
    }
}

#[test]
fn commit_msg_hook_does_not_require_an_id_for_flow_profiles() {
    // kanban and generic do not require an id (Requirement 6.8).
    for profile in [KANBAN, GENERIC] {
        let hook = commit_msg_hook(profile);
        assert!(
            hook.contains(r#"REQUIRE_ISSUE_ID="no""#),
            "`{}` must not require an id",
            profile.name
        );
    }
}

#[test]
fn commit_msg_hook_accepts_merge_and_revert_lines() {
    // The template short-circuits generated merge and revert subjects (Requirement 6.10).
    let hook = commit_msg_hook(ISSUE_PER_TASK);
    assert!(hook.contains(r#""Merge "*|"Revert "*"#));
}

#[test]
fn post_merge_hook_starts_with_a_shebang_and_pins_the_trunk() {
    let hook = post_merge_hook(ISSUE_PER_TASK);
    assert!(hook.starts_with("#!/bin/sh\n"));
    assert!(hook.contains(r#"TRUNK="main""#));
}

#[test]
fn post_merge_hook_embeds_the_profile_branch_pattern() {
    // The epic-based pattern differs from the milestone-based one (Requirement 7.7).
    assert!(post_merge_hook(EPIC_BASED).contains(EPIC_BASED.branch_pattern));
    assert!(post_merge_hook(MILESTONE_BASED).contains(MILESTONE_BASED.branch_pattern));
}

#[test]
fn post_merge_hook_guards_the_current_and_trunk_branches() {
    let hook = post_merge_hook(GENERIC);
    // The sweep skips the current branch and the trunk (Requirement 7.6).
    assert!(hook.contains(r#"[ "$b" = "$current" ] && continue"#));
    assert!(hook.contains(r#"[ "$b" = "$TRUNK" ] && continue"#));
}
