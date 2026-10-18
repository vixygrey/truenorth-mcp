//! Example tests for the secret-filtered state builder.
//!
//! Included from `secret_filter.rs` via `#[path]`, so `super` is the secret_filter module.
//! These pin unsafe-path rejection, content redaction, and missing proposed files.

use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use super::*;

/// Write a file under the repo root, creating parent directories.
fn write_file(repo: &TempDir, rel: &str, content: &str) {
    let path = repo.path().join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, content).expect("write file");
}

#[test]
fn a_secret_path_is_rejected_before_clean_content_is_read() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, ".env", "API_TOKEN=abcdef");
    write_file(&repo, "readme.txt", "hello world");
    let paths = vec![PathBuf::from("readme.txt"), PathBuf::from(".env")];

    let error = build_state_with_secret_filter(repo.path(), &paths).expect_err("secret path");
    assert!(matches!(
        error,
        super::super::JevError::InvalidGuardPath { .. }
    ));
}

#[test]
fn a_secret_content_line_is_dropped() {
    let repo = TempDir::new().expect("temp repo");
    write_file(
        &repo,
        "notes.txt",
        "keep me\nthis is a secret line\nkeep me too",
    );
    let paths = vec![PathBuf::from("notes.txt")];

    let state = build_state_with_secret_filter(repo.path(), &paths).expect("builds");
    let serialized = state.to_string();
    assert!(!serialized.contains("secret"), "secret line dropped");
    assert!(serialized.contains("keep me"), "clean lines kept");
}

#[test]
fn an_unreadable_file_is_skipped_not_failed() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "present.txt", "content here");
    // `missing.txt` is never created, so its read fails and the builder skips it.
    let paths = vec![PathBuf::from("missing.txt"), PathBuf::from("present.txt")];

    let state = build_state_with_secret_filter(repo.path(), &paths).expect("builds");
    assert_eq!(state.as_array().map(Vec::len), Some(1));
    assert!(state.to_string().contains("content here"));
}

#[test]
fn a_secret_parent_path_is_rejected() {
    let repo = TempDir::new().expect("temp repo");
    write_file(&repo, "app/credentials/readme.txt", "safe text");
    let paths = vec![PathBuf::from("app/credentials/readme.txt")];
    let error = build_state_with_secret_filter(repo.path(), &paths).expect_err("secret parent");
    assert!(matches!(
        error,
        super::super::JevError::InvalidGuardPath { .. }
    ));
}

#[test]
fn drift_scope_state_carries_paths_and_protected_no_content() {
    // The lightweight drift state carries the changed paths, the protected list, and a scope
    // hint, and no file content (#328).
    let changed = vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")];
    let protected = vec!["specs/".to_string(), "LICENSE".to_string()];

    let state = build_drift_scope_state(&changed, &protected, None).expect("builds");

    // The changed paths are present.
    let paths = state["changed_paths"]
        .as_array()
        .expect("changed_paths is an array");
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0], "src/main.rs");
    // The protected list is present.
    let prot = state["protected_paths"]
        .as_array()
        .expect("protected_paths is an array");
    assert_eq!(prot.len(), 2);
    assert_eq!(prot[1], "LICENSE");
    // A scope hint is present.
    assert!(state["scope_note"].is_string(), "the scope note is present");
    // With no task, the state carries no `task` key.
    assert!(
        state.get("task").is_none(),
        "no task key when task is absent"
    );
    // There is no `content` key anywhere: the state carries no file content.
    assert!(
        !state.to_string().contains("\"content\""),
        "the drift scope state carries no file content"
    );
}

#[test]
fn drift_scope_state_carries_the_task_when_present() {
    // With a task, the state carries it and the scope hint references the task (#331).
    let changed = vec![PathBuf::from("src/parser.rs")];
    let protected = vec!["specs/".to_string()];

    let state = build_drift_scope_state(&changed, &protected, Some("fix the tokenizer off-by-one"))
        .expect("builds");

    assert_eq!(state["task"], "fix the tokenizer off-by-one");
    // The scope hint tells the model to judge against the task.
    let note = state["scope_note"]
        .as_str()
        .expect("scope note is a string");
    assert!(note.contains("task"), "the scope note references the task");
    // Still no file content.
    assert!(
        !state.to_string().contains("\"content\""),
        "the drift scope state carries no file content"
    );
}

#[test]
fn drift_scope_state_excludes_a_secret_path() {
    // A changed path that matches the secret denylist is excluded, so no secret path reaches
    // the model (Requirement 9.1).
    let changed = vec![PathBuf::from("src/main.rs"), PathBuf::from("config/.env")];
    let protected = vec!["specs/".to_string()];

    let state = build_drift_scope_state(&changed, &protected, None).expect("builds");

    let paths = state["changed_paths"]
        .as_array()
        .expect("changed_paths is an array");
    assert_eq!(paths.len(), 1, "the secret path is excluded");
    assert_eq!(paths[0], "src/main.rs");
    assert!(
        !state.to_string().contains(".env"),
        "no secret path in the state"
    );
}

#[test]
fn guard_state_distinguishes_one_proposal_from_filtered_disk_and_declared_context() {
    let repo = TempDir::new().unwrap();
    write_file(
        &repo,
        "src/main.rs",
        "old code\nSECRET_TOKEN=do-not-send\nsafe code",
    );
    write_file(
        &repo,
        "CONVENTIONS.md",
        "Use explicit errors.\nprivate secret rule\n",
    );
    write_file(&repo, "Cargo.toml", "[dependencies]\nserde = \"1\"");
    let paths = vec![
        PathBuf::from("src/main.rs"),
        PathBuf::from("new/missing.rs"),
    ];
    let mut files = open_state_files(repo.path(), &paths).unwrap();
    let (state, missing) =
        build_guard_rigor_state(repo.path(), &paths, &mut files, "new code").unwrap();
    assert!(missing.is_empty());
    assert_eq!(state["proposal"]["content"], "new code");
    assert_eq!(
        state["proposal"]["paths"],
        json!(["src/main.rs", "new/missing.rs"])
    );
    assert_eq!(
        state["existing_context"][0]["content"],
        "old code\nsafe code"
    );
    assert_eq!(
        state["project_context"][0]["content"],
        "Use explicit errors."
    );
    assert_eq!(state["project_context"][1]["path"], "Cargo.toml");
    assert!(!state.to_string().contains("do-not-send"));
}

#[test]
fn guard_state_reports_missing_or_oversized_context_without_reading_other_files() {
    let repo = TempDir::new().unwrap();
    write_file(&repo, "unrelated.rs", "not selected");
    write_file(&repo, "CONVENTIONS.md", &"rule ".repeat(10_000));
    let paths = vec![PathBuf::from("new/mod.rs")];
    let mut files = open_state_files(repo.path(), &paths).unwrap();
    let (state, missing) =
        build_guard_rigor_state(repo.path(), &paths, &mut files, "new source").unwrap();
    assert!(
        missing
            .iter()
            .any(|note| note.contains("CONVENTIONS.md unavailable"))
    );
    assert!(
        missing
            .iter()
            .any(|note| note.contains("dependency manifest unavailable"))
    );
    assert!(!state.to_string().contains("not selected"));
    assert!(!state.to_string().contains("rule rule"));
}

#[cfg(unix)]
#[test]
fn guard_context_does_not_follow_a_manifest_symlink() {
    use std::os::unix::fs::symlink;
    let repo = TempDir::new().unwrap();
    write_file(&repo, "outside.txt", "private material");
    symlink("outside.txt", repo.path().join("Cargo.toml")).unwrap();
    let paths = vec![PathBuf::from("new/mod.rs")];
    let mut files = open_state_files(repo.path(), &paths).unwrap();
    let (state, missing) =
        build_guard_rigor_state(repo.path(), &paths, &mut files, "new source").unwrap();
    assert!(
        missing
            .iter()
            .any(|note| note.contains("dependency manifest unavailable"))
    );
    assert!(!state.to_string().contains("private material"));
}

#[test]
fn guard_context_selects_nearest_existing_manifest_without_tree_scan() {
    let repo = TempDir::new().unwrap();
    write_file(&repo, "requirements.txt", "rootdep==1");
    write_file(
        &repo,
        "packages/widget/pyproject.toml",
        "[project]\ndependencies = [\"localdep\"]",
    );
    write_file(
        &repo,
        "packages/other/pyproject.toml",
        "[project]\ndependencies = [\"unrelated\"]",
    );
    let paths = vec![PathBuf::from("packages/widget/src/new.py")];
    let mut files = open_state_files(repo.path(), &paths).unwrap();
    let (state, missing) =
        build_guard_rigor_state(repo.path(), &paths, &mut files, "import localdep").unwrap();
    assert_eq!(
        state["project_context"][0]["path"],
        "packages/widget/pyproject.toml"
    );
    assert!(!state.to_string().contains("rootdep"));
    assert!(!state.to_string().contains("unrelated"));
    assert!(
        missing
            .iter()
            .any(|note| note.contains("CONVENTIONS.md unavailable"))
    );
}

#[test]
fn unreadable_existing_bytes_are_reported_without_discarding_the_proposal() {
    let repo = TempDir::new().unwrap();
    write_file(
        &repo,
        "src/large.rs",
        &"x".repeat(MAX_CONTEXT_BYTES as usize + 1),
    );
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(repo.path().join("src/binary.rs"), b"safe\xffprivate").unwrap();
    let paths = vec![
        PathBuf::from("src/large.rs"),
        PathBuf::from("src/binary.rs"),
    ];
    let mut files = open_state_files(repo.path(), &paths).unwrap();

    let (state, missing) =
        build_guard_rigor_state(repo.path(), &paths, &mut files, "fn replacement() {}").unwrap();

    assert_eq!(state["proposal"]["content"], "fn replacement() {}");
    assert_eq!(state["existing_context"], json!([]));
    assert!(
        missing
            .iter()
            .any(|note| note.contains("src/large.rs") && note.contains("exceeds"))
    );
    assert!(
        missing
            .iter()
            .any(|note| note.contains("src/binary.rs") && note.contains("non-UTF-8"))
    );
    assert!(!state.to_string().contains("private"));
    assert!(!state.to_string().contains(&"x".repeat(256)));
}

#[test]
fn nearest_manifest_is_used_for_javascript_python_and_go_sources() {
    for (source, manifest, root_content, near_content) in [
        (
            "js",
            "package.json",
            "root-js-dependency",
            "near-js-dependency",
        ),
        (
            "py",
            "requirements.txt",
            "root-py-dependency",
            "near-py-dependency",
        ),
        ("go", "go.mod", "root-go-dependency", "near-go-dependency"),
    ] {
        let repo = TempDir::new().unwrap();
        write_file(&repo, manifest, root_content);
        write_file(&repo, &format!("packages/widget/{manifest}"), near_content);
        let paths = vec![PathBuf::from(format!("packages/widget/src/main.{source}"))];
        let mut files = open_state_files(repo.path(), &paths).unwrap();

        let (state, missing) =
            build_guard_rigor_state(repo.path(), &paths, &mut files, "new source").unwrap();

        assert_eq!(
            state["project_context"],
            json!([{
                "path": format!("packages/widget/{manifest}"),
                "content": near_content
            }]),
            "nearest manifest for {source}"
        );
        assert!(!state.to_string().contains(root_content));
        assert!(
            !missing
                .iter()
                .any(|note| note.contains("dependency manifest"))
        );
    }
}

#[test]
fn unsafe_or_oversized_project_context_is_reported_without_leaking_it() {
    let repo = TempDir::new().unwrap();
    fs::write(repo.path().join("CONVENTIONS.md"), b"rule\xffprivate").unwrap();
    write_file(&repo, "Cargo.toml", &"untrusted ".repeat(4_000));
    let paths = vec![PathBuf::from("src/new.rs")];
    let mut files = open_state_files(repo.path(), &paths).unwrap();

    let (state, missing) =
        build_guard_rigor_state(repo.path(), &paths, &mut files, "new source").unwrap();

    assert_eq!(state["project_context"], json!([]));
    assert!(
        missing
            .iter()
            .any(|note| note.contains("CONVENTIONS.md unavailable: non-UTF-8"))
    );
    assert!(
        missing
            .iter()
            .any(|note| note.contains("Cargo.toml unavailable: exceeds"))
    );
    assert!(
        missing
            .iter()
            .any(|note| note == "dependency manifest unavailable")
    );
    assert!(!state.to_string().contains("private"));
    assert!(!state.to_string().contains("untrusted"));
}

#[cfg(unix)]
#[test]
fn optional_symlinked_context_never_reads_an_outside_canary() {
    use std::os::unix::fs::symlink;

    let repo = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::write(outside.path().join("rules.md"), "OUTSIDE_RULE_CANARY").unwrap();
    fs::write(
        outside.path().join("package.json"),
        "OUTSIDE_MANIFEST_CANARY",
    )
    .unwrap();
    fs::create_dir_all(repo.path().join("packages/widget")).unwrap();
    symlink(
        outside.path().join("rules.md"),
        repo.path().join("CONVENTIONS.md"),
    )
    .unwrap();
    symlink(
        outside.path().join("package.json"),
        repo.path().join("packages/widget/package.json"),
    )
    .unwrap();
    let paths = vec![PathBuf::from("packages/widget/src/new.tsx")];
    let mut files = open_state_files(repo.path(), &paths).unwrap();

    let (state, missing) =
        build_guard_rigor_state(repo.path(), &paths, &mut files, "new source").unwrap();

    assert_eq!(state["project_context"], json!([]));
    assert!(
        missing
            .iter()
            .any(|note| note.contains("CONVENTIONS.md unavailable: unsafe"))
    );
    assert!(
        missing
            .iter()
            .any(|note| note.contains("packages/widget/package.json unavailable: unsafe"))
    );
    assert!(!state.to_string().contains("OUTSIDE_RULE_CANARY"));
    assert!(!state.to_string().contains("OUTSIDE_MANIFEST_CANARY"));
}

#[test]
fn drift_scope_refuses_residual_markers_in_protected_metadata() {
    let result = build_drift_scope_state(
        &[PathBuf::from("src/main.rs")],
        &["specs/secret-canary".to_string()],
        None,
    );
    assert!(matches!(
        &result,
        Err(super::super::JevError::SecretResidual)
    ));
    assert!(!result.unwrap_err().to_string().contains("canary"));
}
