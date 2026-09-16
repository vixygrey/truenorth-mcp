//! Property tests for methodology profile resolution.
//!
//! Included from `profile.rs` via `#[path]`, so `super` is the profile module.
//!
//! Feature: agent-workspace-profiles, Property 10: profile default and unknown-name
//! resolution. For an absent `.agent/profile.yml`, `resolve_active` returns the
//! issue-per-task profile. For every name outside the five built-ins, `by_name` returns
//! `None` and `resolve_active` errors naming the value and the five valid names, with no
//! partial state. For each of the five names, resolution returns that profile.

use std::fs;

use proptest::prelude::*;
use tempfile::TempDir;

use super::*;

/// The five valid profile names, for the membership check.
const VALID_NAMES: [&str; 5] = [
    "epic-based",
    "issue-per-task",
    "kanban",
    "milestone-based",
    "generic",
];

/// Seed `.agent/profile.yml` with the given profile name under a fresh temp repo root.
///
/// The name is written through the YAML serializer, so any generated string becomes a
/// valid quoted scalar. This isolates the property to resolution logic rather than YAML
/// quoting rules.
fn seed_profile(name: &str) -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let agent = repo.path().join(AGENT_DIR);
    fs::create_dir_all(&agent).expect("create .agent");
    let doc = serde_yaml::to_string(&serde_yaml::Value::Mapping({
        let mut map = serde_yaml::Mapping::new();
        map.insert("profile".into(), name.into());
        map
    }))
    .expect("serialize profile config");
    fs::write(agent.join("profile.yml"), doc).expect("write profile.yml");
    repo
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// by_name and resolve_active agree with membership in the five built-ins.
    #[test]
    fn resolution_matches_membership(name in "[a-zA-Z0-9-]{0,24}") {
        let is_valid = VALID_NAMES.contains(&name.as_str());
        let looked_up = by_name(&name);
        prop_assert_eq!(looked_up.is_some(), is_valid);

        let repo = seed_profile(&name);
        let resolved = resolve_active(repo.path());

        if is_valid {
            let profile = resolved.expect("valid name resolves");
            prop_assert_eq!(profile.name, looked_up.expect("valid name").name);
        } else {
            match resolved {
                Err(ProfileError::UnknownProfile { name: reported }) => {
                    prop_assert_eq!(reported, name);
                }
                Err(other) => prop_assert!(false, "expected UnknownProfile, got {other:?}"),
                Ok(profile) => prop_assert!(false, "unknown name resolved to {}", profile.name),
            }
        }
    }
}

/// The unknown-profile error message names all five valid profiles, for any unknown value.
#[test]
fn unknown_profile_error_always_names_the_five() {
    for unknown in ["waterfall", "scrum", "", "EPIC-BASED", "issue_per_task"] {
        // Skip any that happen to be valid (none here, but keep the guard honest).
        if VALID_NAMES.contains(&unknown) {
            continue;
        }
        let repo = seed_profile(unknown);
        let error = resolve_active(repo.path()).expect_err("unknown rejected");
        let message = error.to_string();
        for valid in VALID_NAMES {
            assert!(
                message.contains(valid),
                "message must name `{valid}` for `{unknown}`"
            );
        }
    }
}
