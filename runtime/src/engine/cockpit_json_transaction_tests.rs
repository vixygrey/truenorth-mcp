use std::fs;
use std::path::Path;

use tempfile::tempdir;

use super::*;

const STATE: &str = ".agent/tasks/state.yml";
const PLAN: &str = ".agent/tasks/release-plan.json";

fn fixture(root: &Path) {
    fs::create_dir_all(root.join(".agent/tasks")).expect("tasks");
    fs::create_dir_all(root.join(JOURNAL_PARENT)).expect("journal directory");
}

fn entry(path: &str, before: Option<&[u8]>, after: Option<&[u8]>) -> Entry {
    Entry {
        path: path.to_string(),
        before: before.map(Vec::from),
        after: after.map(Vec::from),
        before_hash: before.map(sha256),
        after_hash: after.map(sha256),
    }
}

fn journal(root: &Path, entries: Vec<Entry>) {
    fs::write(
        root.join(JOURNAL),
        serde_json::to_vec(&Journal { entries }).expect("serialize journal"),
    )
    .expect("write journal");
}

fn recover(root: &Path, rollback: bool) -> Result<(), TransactionError> {
    if rollback {
        super::rollback(root)
    } else {
        super::resume(root)
    }
}

#[test]
fn rejects_escape_and_unsupported_paths_before_any_revision_is_applied() {
    let container = tempdir().expect("container");
    let root = container.path().join("repo");
    fs::create_dir(&root).expect("repo");
    fixture(&root);
    let outside = container.path().join("escape");
    fs::write(&outside, b"outside").expect("outside");
    fs::create_dir(root.join("specs")).expect("specs");
    fs::write(root.join("specs/state.yaml"), b"specification").expect("spec");
    let state = root.join(STATE);
    fs::write(&state, b"original").expect("state");

    for path in [
        "../escape".to_string(),
        outside.display().to_string(),
        ".agent/tasks/../../../escape".to_string(),
        ".agent/tasks/../state.yml".to_string(),
        ".agent\\tasks\\state.yml".to_string(),
        "specs/state.yaml".to_string(),
        ".agent/tasks/unknown.json".to_string(),
    ] {
        for rollback in [false, true] {
            // The valid entry is last: an old reverse-order replay would mutate it first.
            journal(
                &root,
                vec![
                    entry(&path, None, Some(b"outside write")),
                    entry(STATE, Some(b"original"), Some(b"changed")),
                ],
            );
            assert!(
                matches!(recover(&root, rollback), Err(TransactionError::Corrupt(_))),
                "path: {path}"
            );
            assert_eq!(fs::read(&state).expect("state"), b"original");
            assert_eq!(fs::read(&outside).expect("outside"), b"outside");
            assert_eq!(
                fs::read(root.join("specs/state.yaml")).expect("spec"),
                b"specification"
            );
            assert!(root.join(JOURNAL).exists());
        }
    }
}

#[test]
fn rejects_corrupt_hash_pairs_duplicates_and_changed_later_entries_before_writes() {
    let root = tempdir().expect("repo");
    fixture(root.path());
    fs::write(root.path().join(STATE), b"original").expect("state");
    fs::write(root.path().join(PLAN), b"unexpected").expect("plan");
    for rollback in [false, true] {
        let invalid = [
            entry(PLAN, None, Some(b"new")),
            Entry {
                before_hash: Some("wrong".to_string()),
                ..entry(PLAN, Some(b"unexpected"), Some(b"new"))
            },
            Entry {
                after_hash: None,
                ..entry(PLAN, Some(b"unexpected"), Some(b"new"))
            },
            Entry {
                before_hash: Some(sha256(b"nonexistent")),
                ..entry(PLAN, None, Some(b"new"))
            },
        ];
        for (index, bad) in invalid.into_iter().enumerate() {
            journal(
                root.path(),
                vec![bad, entry(STATE, Some(b"original"), Some(b"changed"))],
            );
            let result = recover(root.path(), rollback);
            if index == 0 {
                assert!(matches!(result, Err(TransactionError::Changed { .. })));
            } else {
                assert!(matches!(result, Err(TransactionError::Corrupt(_))));
            }
            assert_eq!(
                fs::read(root.path().join(STATE)).expect("state"),
                b"original"
            );
            assert_eq!(
                fs::read(root.path().join(PLAN)).expect("plan"),
                b"unexpected"
            );
        }
        journal(
            root.path(),
            vec![
                entry(STATE, Some(b"original"), Some(b"changed")),
                entry(STATE, Some(b"original"), Some(b"changed")),
            ],
        );
        assert!(matches!(
            recover(root.path(), rollback),
            Err(TransactionError::Corrupt(_))
        ));
        assert_eq!(
            fs::read(root.path().join(STATE)).expect("state"),
            b"original"
        );
    }
}

#[test]
fn unreadable_revision_is_an_io_error_before_any_target_changes() {
    let root = tempdir().expect("repo");
    fixture(root.path());
    fs::create_dir(root.path().join(PLAN)).expect("directory at revision path");
    fs::write(root.path().join(STATE), b"original").expect("state");
    for rollback in [false, true] {
        journal(
            root.path(),
            vec![
                entry(PLAN, None, Some(b"replacement")),
                entry(STATE, Some(b"original"), Some(b"changed")),
            ],
        );
        assert!(matches!(
            recover(root.path(), rollback),
            Err(TransactionError::Io { .. })
        ));
        assert_eq!(
            fs::read(root.path().join(STATE)).expect("state"),
            b"original"
        );
        assert!(root.path().join(PLAN).is_dir());
    }
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_journal_journal_parent_target_parent_and_target() {
    use std::os::unix::fs::symlink;

    for rollback in [false, true] {
        for kind in ["journal", "journal-parent", "target-parent", "target"] {
            let root = tempdir().expect("repo");
            let outside = tempdir().expect("outside");
            fixture(root.path());
            fs::write(root.path().join(STATE), b"original").expect("state");
            journal(
                root.path(),
                vec![entry(STATE, Some(b"original"), Some(b"changed"))],
            );
            match kind {
                "journal" => {
                    fs::rename(
                        root.path().join(JOURNAL),
                        outside.path().join("transaction.json"),
                    )
                    .expect("move journal");
                    symlink(
                        outside.path().join("transaction.json"),
                        root.path().join(JOURNAL),
                    )
                    .expect("link journal");
                }
                "journal-parent" => {
                    fs::rename(
                        root.path().join(JOURNAL_PARENT),
                        outside.path().join("journal"),
                    )
                    .expect("move journal directory");
                    symlink(
                        outside.path().join("journal"),
                        root.path().join(JOURNAL_PARENT),
                    )
                    .expect("link journal directory");
                }
                "target-parent" => {
                    fs::rename(
                        root.path().join(".agent/tasks"),
                        outside.path().join("tasks"),
                    )
                    .expect("move tasks");
                    symlink(
                        outside.path().join("tasks"),
                        root.path().join(".agent/tasks"),
                    )
                    .expect("link tasks");
                }
                "target" => {
                    fs::rename(root.path().join(STATE), outside.path().join("state.yml"))
                        .expect("move target");
                    symlink(outside.path().join("state.yml"), root.path().join(STATE))
                        .expect("link target");
                }
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    recover(root.path(), rollback),
                    Err(TransactionError::Io { .. })
                ),
                "{kind}"
            );
            let observed = match kind {
                "target-parent" => outside.path().join("tasks/state.yml"),
                "target" => outside.path().join("state.yml"),
                _ => root.path().join(STATE),
            };
            assert_eq!(fs::read(observed).expect("untouched target"), b"original");
        }
    }
}

#[test]
fn partial_resume_and_rollback_are_idempotent_across_revision_states() {
    let root = tempdir().expect("repo");
    fixture(root.path());
    fs::write(root.path().join(STATE), b"old").expect("old state");
    prepare(
        root.path(),
        vec![
            Change {
                path: STATE.into(),
                after: Some(b"new".to_vec()),
            },
            Change {
                path: PLAN.into(),
                after: Some(b"plan".to_vec()),
            },
        ],
    )
    .expect("prepare");
    fs::write(root.path().join(STATE), b"new").expect("partial resume");
    resume(root.path()).expect("complete resume");
    resume(root.path()).expect("repeat resume");
    assert_eq!(fs::read(root.path().join(STATE)).expect("state"), b"new");
    assert_eq!(fs::read(root.path().join(PLAN)).expect("plan"), b"plan");

    fs::write(root.path().join(STATE), b"old").expect("partial rollback");
    rollback(root.path()).expect("complete rollback");
    assert_eq!(fs::read(root.path().join(STATE)).expect("state"), b"old");
    assert!(!root.path().join(PLAN).exists());
    assert!(!root.path().join(JOURNAL).exists());
}

#[test]
fn partial_deletion_can_resume_then_rollback() {
    let root = tempdir().expect("repo");
    fixture(root.path());
    fs::write(root.path().join(STATE), b"old").expect("old state");
    prepare(
        root.path(),
        vec![Change {
            path: STATE.into(),
            after: None,
        }],
    )
    .expect("prepare");
    fs::remove_file(root.path().join(STATE)).expect("partial deletion");
    resume(root.path()).expect("already deleted");
    rollback(root.path()).expect("restore deleted file");
    assert_eq!(fs::read(root.path().join(STATE)).expect("restored"), b"old");
}

#[test]
fn prepare_rejects_unapproved_paths_duplicates_and_symlinked_targets_without_journal() {
    let root = tempdir().expect("repo");
    fixture(root.path());
    for path in ["../escape", "specs/state.yaml", ".agent/tasks/unknown.json"] {
        assert!(matches!(
            prepare(
                root.path(),
                vec![Change {
                    path: path.into(),
                    after: Some(vec![1])
                }]
            ),
            Err(TransactionError::Corrupt(_))
        ));
        assert!(!root.path().join(JOURNAL).exists());
    }
    assert!(matches!(
        prepare(
            root.path(),
            vec![
                Change {
                    path: STATE.into(),
                    after: None
                },
                Change {
                    path: STATE.into(),
                    after: None
                },
            ]
        ),
        Err(TransactionError::Corrupt(_))
    ));
    assert!(!root.path().join(JOURNAL).exists());

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let outside = tempdir().expect("outside");
        fs::write(outside.path().join("state.yml"), b"outside").expect("outside target");
        symlink(outside.path().join("state.yml"), root.path().join(STATE)).expect("link target");
        assert!(matches!(
            prepare(
                root.path(),
                vec![Change {
                    path: STATE.into(),
                    after: None
                }]
            ),
            Err(TransactionError::Io { .. })
        ));
        assert_eq!(
            fs::read(outside.path().join("state.yml")).expect("outside"),
            b"outside"
        );
        assert!(!root.path().join(JOURNAL).exists());
        fs::remove_file(root.path().join(STATE)).expect("remove target link");
        fs::remove_dir(root.path().join(".agent/tasks")).expect("remove empty tasks");
        symlink(outside.path(), root.path().join(".agent/tasks")).expect("link tasks");
        assert!(matches!(
            prepare(
                root.path(),
                vec![Change {
                    path: STATE.into(),
                    after: Some(b"new".to_vec()),
                }],
            ),
            Err(TransactionError::Io { .. })
        ));
        assert_eq!(
            fs::read(outside.path().join("state.yml")).expect("outside"),
            b"outside"
        );
        assert!(!root.path().join(JOURNAL).exists());
    }
}
