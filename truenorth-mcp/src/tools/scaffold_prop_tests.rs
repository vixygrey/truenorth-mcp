//! Property and golden tests for the greenfield scaffold.
//!
//! Included from `scaffold.rs` via `#[path]`, so `super` is the scaffold module.
//!
//! Feature: agent-workspace-profiles, Property 13: greenfield scaffold is non-destructive.
//! For any pre-existing subset of the scaffold's target paths, running the scaffold leaves
//! each pre-existing path's bytes unchanged and reports it as skipped. An unknown profile
//! name makes no file change.

use std::fs;

use proptest::prelude::*;
use rmcp::handler::server::wrapper::Parameters;
use tempfile::TempDir;

use super::*;
use crate::engine::profile::ALL_PROFILES;

fn run_scaffold(repo: &TempDir, profile: Option<&str>) {
    let srv = TrueNorthServer::new(repo.path().to_path_buf());
    let fut = srv.truenorth_scaffold_project(Parameters(ScaffoldArgs {
        profile: profile.map(str::to_string),
    }));
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(fut)
        .expect("scaffold");
}

/// A representative set of target paths the scaffold emits, used to pre-seed subsets.
const TARGET_PATHS: [&str; 8] = [
    ".agent/layout.yml",
    ".agent/tasks/state.yml",
    ".agent/memories/lessons.md",
    "AGENTS.md",
    "CONVENTIONS.md",
    ".githooks/commit-msg",
    ".github/commit-template.md",
    ".github/ISSUE_TEMPLATE/config.yml",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(120))]

    /// Property 13: for any pre-existing subset of targets, the scaffold leaves those
    /// bytes unchanged and writes the rest.
    #[test]
    fn scaffold_is_non_destructive_over_any_preexisting_subset(
        preexist in prop::collection::vec(any::<bool>(), TARGET_PATHS.len())
    ) {
        let repo = TempDir::new().expect("temp repo");

        // Pre-seed the chosen subset with sentinel content.
        let mut sentinels = Vec::new();
        for (i, &path) in TARGET_PATHS.iter().enumerate() {
            if preexist[i] {
                let full = repo.path().join(path);
                fs::create_dir_all(full.parent().unwrap()).expect("parent");
                let content = format!("SENTINEL {path}\n");
                fs::write(&full, &content).expect("seed sentinel");
                sentinels.push((full, content));
            }
        }

        run_scaffold(&repo, None);

        // Every pre-existing path keeps its sentinel bytes unchanged.
        for (full, content) in &sentinels {
            prop_assert_eq!(&fs::read_to_string(full).expect("read"), content);
        }
        // Every target now exists (either the sentinel or a freshly written file).
        for path in TARGET_PATHS {
            prop_assert!(repo.path().join(path).exists(), "{} should exist", path);
        }
    }

    /// Property 13: an unknown profile makes no file change.
    #[test]
    fn scaffold_unknown_profile_writes_nothing(name in "[a-z][a-z0-9-]{0,20}") {
        prop_assume!(crate::engine::profile::by_name(&name).is_none());
        let repo = TempDir::new().expect("temp repo");
        let srv = TrueNorthServer::new(repo.path().to_path_buf());
        let fut = srv.truenorth_scaffold_project(Parameters(ScaffoldArgs {
            profile: Some(name),
        }));
        let outcome = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(fut);
        prop_assert!(outcome.is_err());
        prop_assert!(!repo.path().join(".agent").exists());
    }
}

/// Golden: every profile emits a language-agnostic tree with no code manifests or source.
#[test]
fn scaffold_emits_no_code_manifests_for_any_profile() {
    for profile in ALL_PROFILES {
        let repo = TempDir::new().expect("temp repo");
        run_scaffold(&repo, Some(profile.name));

        for forbidden in [
            "Cargo.toml",
            "package.json",
            "src",
            "go.mod",
            "pyproject.toml",
        ] {
            assert!(
                !repo.path().join(forbidden).exists(),
                "`{}` profile emitted `{forbidden}`",
                profile.name
            );
        }
        // The two hooks and the root docs are present for every profile.
        assert!(repo.path().join(".githooks/commit-msg").is_file());
        assert!(repo.path().join(".githooks/post-merge").is_file());
        assert!(repo.path().join("AGENTS.md").is_file());
    }
}

/// The runtime tools never call the audited repo-seed path; only the scaffold does.
///
/// This is a source-level audit (ADR-6): `write_repo_seed` may appear only in the scaffold
/// tool among the `src/tools/` modules.
#[test]
fn only_the_scaffold_calls_write_repo_seed() {
    let tools_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tools");
    let mut offenders = Vec::new();
    for entry in fs::read_dir(&tools_dir).expect("read tools dir") {
        let path = entry.expect("entry").path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // Only inspect the tool implementation modules, not their test siblings.
        if !name.ends_with(".rs") || name.contains("_tests") || name.contains("_prop_tests") {
            continue;
        }
        if name == "scaffold.rs" {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read tool source");
        if text.contains("write_repo_seed") {
            offenders.push(name.to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "only the scaffold may call write_repo_seed, but these do: {offenders:?}"
    );
}
