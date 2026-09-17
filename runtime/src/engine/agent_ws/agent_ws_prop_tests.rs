//! Property tests for the agent workspace write guard.
//!
//! Included from `agent_ws.rs` via `#[path]`, so `super` is the agent_ws module.
//!
//! Feature: agent-workspace-profiles, Property 6: runtime writes only under `.agent/`.
//! For every write target path `p`, `write_under_agent` succeeds if and only if the
//! normalized `p` stays under `repo_root/.agent/`. Every path that escapes `.agent/` via
//! `..` or an absolute path is rejected, and on rejection no file is written, so every
//! target is left unchanged. For every read target under `.agent/telemetry/`,
//! `is_excluded_read` is true; for every other path it is false.

use std::fs;
use std::path::{Component, Path, PathBuf};

use proptest::prelude::*;
use tempfile::TempDir;

use super::*;

/// A path segment the generator can emit. The mix produces both safe descents and
/// escaping components, so the property covers accept and reject inputs.
#[derive(Debug, Clone)]
enum Seg {
    Normal(String),
    ParentDir,
    CurDir,
}

/// Generate one path segment. Normal names are drawn from a small safe alphabet so the
/// path is a valid relative filename on every platform.
fn seg_strategy() -> impl Strategy<Value = Seg> {
    prop_oneof![
        3 => "[a-z][a-z0-9_-]{0,7}".prop_map(Seg::Normal),
        1 => Just(Seg::ParentDir),
        1 => Just(Seg::CurDir),
    ]
}

/// Build a relative path from generated segments.
fn path_strategy() -> impl Strategy<Value = PathBuf> {
    prop::collection::vec(seg_strategy(), 1..6).prop_map(|segs| {
        let mut path = PathBuf::new();
        for seg in segs {
            match seg {
                Seg::Normal(name) => path.push(name),
                Seg::ParentDir => path.push(".."),
                Seg::CurDir => path.push("."),
            }
        }
        path
    })
}

/// The independent oracle: normalize `rel` under `base`, or `None` when it escapes.
///
/// This mirrors the guard's rule without reusing its code, so the property checks the
/// guard against a separate implementation rather than against itself. It returns the
/// resolved absolute path, so the test can assert existence against the normalized
/// target rather than the raw `..`-carrying join.
fn resolve_oracle(base: &Path, rel: &Path) -> Option<PathBuf> {
    let mut resolved = base.to_path_buf();
    let mut depth: i64 = 0;
    for component in rel.components() {
        match component {
            Component::RootDir | Component::Prefix(_) => return None,
            Component::CurDir => {}
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
                resolved.pop();
            }
            Component::Normal(part) => {
                depth += 1;
                resolved.push(part);
            }
        }
    }
    // A target must name a file under the base. A path that resolves back to the base
    // itself (depth 0, for example `a/..`) is not a writable file target.
    if depth == 0 {
        return None;
    }
    Some(resolved)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// write_under_agent succeeds exactly when the target stays under .agent/, and on a
    /// reject it writes nothing outside .agent/.
    #[test]
    fn write_under_agent_succeeds_iff_under_agent(rel in path_strategy()) {
        let repo = TempDir::new().expect("temp repo");
        // A sentinel sibling of .agent/ that a leaking write could clobber.
        let sentinel = repo.path().join("specs").join("guarded.txt");
        fs::create_dir_all(sentinel.parent().unwrap()).expect("seed sentinel dir");
        fs::write(&sentinel, "original\n").expect("seed sentinel");

        let agent_root = repo.path().join(AGENT_DIR);
        let expected_target = resolve_oracle(&agent_root, &rel);
        let result = write_under_agent(repo.path(), &rel, "payload\n");

        prop_assert_eq!(result.is_ok(), expected_target.is_some());

        if let Some(target) = expected_target {
            // The write landed at the normalized target, which resolves under .agent/.
            prop_assert!(target.is_file(), "expected a file at {target:?}");
            let agent = fs::canonicalize(&agent_root).expect("canon agent");
            let resolved = fs::canonicalize(&target).expect("canon target");
            prop_assert!(resolved.starts_with(&agent), "target {resolved:?} not under {agent:?}");
        }

        // On every input the sentinel outside .agent/ is untouched.
        prop_assert_eq!(fs::read_to_string(&sentinel).expect("read sentinel"), "original\n");
    }

    /// is_excluded_read is true exactly for the telemetry area.
    #[test]
    fn is_excluded_read_matches_telemetry(rel in path_strategy()) {
        let agent_rel = Path::new(AGENT_DIR).join(&rel);
        let text = agent_rel.to_string_lossy().replace('\\', "/");
        let telemetry_root = format!("{AGENT_DIR}/{TELEMETRY_AREA}");
        let expected = text == telemetry_root || text.starts_with(&format!("{telemetry_root}/"));

        prop_assert_eq!(is_excluded_read(&agent_rel), expected);
    }
}
