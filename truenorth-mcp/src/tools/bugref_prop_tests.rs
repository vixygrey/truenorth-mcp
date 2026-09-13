//! Property tests for the bug-reference tool.
//!
//! Included from `bugref.rs` via `#[path]`, so `super` is the bugref module.
//!
//! Feature: agent-workspace-profiles, Property 14: bug-reference validation and tag
//! preservation. A call is accepted if and only if the id is 1 to 200 characters, the
//! external link is an absolute URL, the status is one of the enum, and the linked
//! reference resolves to an existing id. On acceptance the stored tags equal the supplied
//! tags. On rejection bugs.yml is unchanged.

use std::fs;

use proptest::prelude::*;
use rmcp::handler::server::wrapper::Parameters;
use tempfile::TempDir;

use super::*;

/// The one resolvable reference seeded in the release plan.
const KNOWN_REF: &str = "e80";

/// A repo whose release plan carries a task with group_id `e80`.
fn repo_with_task() -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let plan = release_plan_path(repo.path());
    fs::create_dir_all(plan.parent().unwrap()).expect("tasks dir");
    fs::write(
        &plan,
        "tasks:\n- group_id: e80\n  task_name: A task\n  verify_command: cargo test\n",
    )
    .expect("seed plan");
    repo
}

fn bugs_path(repo: &TempDir) -> std::path::PathBuf {
    repo.path().join(".agent").join("tasks").join("bugs.yml")
}

/// Map an index to a status, so the generator covers the whole enum.
fn status_of(i: usize) -> BugStatus {
    match i % 5 {
        0 => BugStatus::Open,
        1 => BugStatus::Triaged,
        2 => BugStatus::InProgress,
        3 => BugStatus::Resolved,
        _ => BugStatus::Closed,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// A bug reference is accepted iff every field rule holds, tags round-trip on accept,
    /// and bugs.yml is unchanged on reject.
    #[test]
    fn record_bug_accepts_iff_valid_and_preserves_state(
        id in "[A-Za-z0-9-]{0,205}",
        // A scheme-with-authority link or a relative one, to exercise both branches.
        link in prop_oneof!["https://[a-z]{1,8}/[0-9]{1,4}", "[a-z/]{1,12}", Just(String::new())],
        status_idx in 0usize..5,
        // The linked_ref is either the known ref or a random unresolved value.
        use_known in any::<bool>(),
        other_ref in "[a-z0-9-]{1,10}",
        tags in prop::collection::vec("[a-z][a-z0-9-]{0,10}", 0..4),
    ) {
        let repo = repo_with_task();
        let srv = TrueNorthServer::new(repo.path().to_path_buf());

        let linked_ref = if use_known { KNOWN_REF.to_string() } else { other_ref.clone() };

        // Independent oracle for the four field rules.
        let id_ok = (1..=MAX_BUG_ID).contains(&id.chars().count());
        let link_ok = is_absolute_url(&link);
        // The status is always valid (the enum is typed). The linked_ref resolves only for
        // the known ref, since no bug exists yet in a fresh repo.
        let ref_ok = linked_ref == KNOWN_REF;
        let expected_ok = id_ok && link_ok && ref_ok;

        let args = RecordBugArgs {
            id: id.clone(),
            external_link: link.clone(),
            status: status_of(status_idx),
            linked_ref: linked_ref.clone(),
            tags: tags.clone(),
        };

        let outcome = tokio_test_block(srv.truenorth_record_bug(Parameters(args)));
        prop_assert_eq!(outcome.is_ok(), expected_ok,
            "id_ok={} link_ok={} ref_ok={} id_len={}", id_ok, link_ok, ref_ok, id.chars().count());

        if expected_ok {
            // Tags round-trip on acceptance (Requirement 8.3).
            let text = fs::read_to_string(bugs_path(&repo)).expect("read bugs");
            let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("parse");
            let entry = &doc.get("bugs").and_then(|v| v.as_sequence()).expect("bugs")[0];
            let stored: Vec<String> = entry
                .get("tags")
                .and_then(|v| v.as_sequence())
                .expect("tags")
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            prop_assert_eq!(stored, tags);
        } else {
            // On reject, no bugs file was written (Requirement 8.7).
            prop_assert!(!bugs_path(&repo).exists());
        }
    }
}

/// Block on a future in a synchronous proptest body.
fn tokio_test_block<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(fut)
}
