//! Unit tests for the config module (task 2.2).
//!
//! Included from `config.rs` via `#[path]`, so `super` is the config module and the
//! tests reach its private helpers. The tests drive the pure `select_repo_root` and
//! `is_valid_repo_root` with fixed inputs, so they read no process-global state and
//! stay independent under parallel execution.
//!
//! Requirements: 1.4, 1.6, 1.7, 1.13, 2.7, 2.8.

use super::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn gitignore_does_not_ignore_the_agent_workspace() {
    // Requirement 1.13: the .agent/ tree tracks under version control, so the repo-root
    // .gitignore must not carry a bare `.agent` or `.agents` ignore entry.
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root is the crate's parent");
    let gitignore = repo_root.join(".gitignore");
    let text = fs::read_to_string(&gitignore).expect("read .gitignore");

    for line in text.lines() {
        let entry = line.trim();
        assert_ne!(
            entry, ".agent",
            "`.agent` must not be gitignored (Requirement 1.13)"
        );
        assert_ne!(
            entry, ".agent/",
            "`.agent/` must not be gitignored (Requirement 1.13)"
        );
        assert_ne!(
            entry, ".agents",
            "`.agents` must not be gitignored (Requirement 1.13)"
        );
        assert_ne!(
            entry, ".agents/",
            "`.agents/` must not be gitignored (Requirement 1.13)"
        );
    }
}

/// Create a directory that contains all three marker directories, so it is a valid root.
fn make_valid_root(base: &Path, name: &str) -> PathBuf {
    let root = base.join(name);
    fs::create_dir_all(root.join(".agent")).expect("create .agent marker");
    fs::create_dir_all(root.join("specs")).expect("create specs marker");
    fs::create_dir_all(root.join("skills")).expect("create skills marker");
    root
}

#[test]
fn valid_root_needs_all_three_markers() {
    let dir = tempdir().expect("temp dir");
    let root = make_valid_root(dir.path(), "repo");
    assert!(is_valid_repo_root(&root));
}

#[test]
fn root_missing_agent_marker_is_invalid() {
    // A root with only the legacy two markers now fails, because `.agent/` is required.
    let dir = tempdir().expect("temp dir");
    let root = dir.path().join("repo");
    fs::create_dir_all(root.join("specs")).expect("create specs marker");
    fs::create_dir_all(root.join("skills")).expect("create skills marker");
    assert!(!is_valid_repo_root(&root));
}

#[test]
fn root_with_only_skills_is_invalid() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path().join("repo");
    fs::create_dir_all(root.join("skills")).expect("create skills marker");
    assert!(!is_valid_repo_root(&root));
}

#[test]
fn root_with_only_specs_is_invalid() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path().join("repo");
    fs::create_dir_all(root.join("specs")).expect("create specs marker");
    assert!(!is_valid_repo_root(&root));
}

#[test]
fn marker_as_file_does_not_satisfy_root() {
    // A `.agent` file, not a directory, must not count as a marker.
    let dir = tempdir().expect("temp dir");
    let root = dir.path().join("repo");
    fs::create_dir_all(root.join("specs")).expect("create specs marker");
    fs::create_dir_all(root.join("skills")).expect("create skills marker");
    fs::write(root.join(".agent"), b"not a dir").expect("write .agent file");
    assert!(!is_valid_repo_root(&root));
}

#[test]
fn git_scope_covers_the_three_marker_dirs() {
    // Git status, log, and diff scope to .agent/, specs/, and skills/ (Requirement 2.8).
    assert!(GIT_SCOPE_DIRS.contains(&".agent"));
    assert!(GIT_SCOPE_DIRS.contains(&"specs"));
    assert!(GIT_SCOPE_DIRS.contains(&"skills"));
    assert_eq!(GIT_SCOPE_DIRS.len(), 3);
}

#[test]
fn selection_precedence_prefers_earlier_candidate() {
    // Both candidates are valid roots. The earlier one (env over cwd) wins.
    let dir = tempdir().expect("temp dir");
    let env_root = make_valid_root(dir.path(), "env");
    let cwd_root = make_valid_root(dir.path(), "cwd");

    let selected = select_repo_root(&[env_root.clone(), cwd_root]).expect("a valid root exists");
    assert_eq!(selected, env_root);
}

#[test]
fn selection_skips_invalid_candidate() {
    // The first candidate is not a valid root, so the second valid one is selected.
    let dir = tempdir().expect("temp dir");
    let invalid = dir.path().join("invalid");
    fs::create_dir_all(&invalid).expect("create invalid candidate");
    let valid = make_valid_root(dir.path(), "valid");

    let selected = select_repo_root(&[invalid, valid.clone()]).expect("a valid root exists");
    assert_eq!(selected, valid);
}

#[test]
fn no_valid_root_returns_error() {
    let dir = tempdir().expect("temp dir");
    let invalid = dir.path().join("invalid");
    fs::create_dir_all(&invalid).expect("create invalid candidate");

    let outcome = select_repo_root(&[invalid]);
    assert!(matches!(outcome, Err(ConfigError::NoValidRoot { .. })));
}

#[test]
fn no_valid_root_error_names_candidates() {
    let outcome = select_repo_root(&[PathBuf::from("/tmp/one"), PathBuf::from("/tmp/two")]);
    let message = outcome.expect_err("expected no valid root").to_string();
    assert!(message.contains("/tmp/one"));
    assert!(message.contains("/tmp/two"));
}

#[test]
fn empty_candidate_list_returns_error() {
    let outcome = select_repo_root(&[]);
    assert!(matches!(outcome, Err(ConfigError::NoValidRoot { .. })));
}

#[test]
fn denylist_matches_env_file() {
    assert!(is_secret_path(Path::new("config/.env")));
    assert!(is_secret_path(Path::new(".env")));
    assert!(is_secret_path(Path::new("service/.env.local")));
}

#[test]
fn denylist_matches_pem_file() {
    assert!(is_secret_path(Path::new("keys/server.pem")));
    assert!(is_secret_path(Path::new("cert.PEM")));
}

#[test]
fn denylist_matches_secret_marker() {
    assert!(is_secret_path(Path::new("config/secret.yaml")));
    assert!(is_secret_path(Path::new("app/my_secrets/token.txt")));
}

#[test]
fn denylist_matches_credentials_marker() {
    assert!(is_secret_path(Path::new("aws/credentials")));
    assert!(is_secret_path(Path::new("home/.aws/credentials.json")));
}

#[test]
fn denylist_ignores_ordinary_paths() {
    assert!(!is_secret_path(Path::new("skills/deploy/SKILL.md")));
    assert!(!is_secret_path(Path::new("specs/state.yaml")));
    assert!(!is_secret_path(Path::new("src/config.rs")));
}

#[test]
fn denylist_does_not_match_environment_word() {
    // `.env` matches only as a full segment, not inside an unrelated word.
    assert!(!is_secret_path(Path::new("docs/environment.md")));
}

#[test]
fn sandbox_config_defaults() {
    let cfg = SandboxConfig::new(PathBuf::from("/repo"), vec!["cargo".to_string()]);
    assert_eq!(cfg.timeout, DEFAULT_GATE_TIMEOUT);
    assert_eq!(cfg.working_dir, PathBuf::from("/repo"));
    assert_eq!(cfg.allowlist, vec!["cargo".to_string()]);
    assert!(cfg.execution_enabled);
}
