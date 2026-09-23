//! Unit tests for the per-project feature flags (Property 16).
//!
//! Included from `features.rs` via `#[path]`, so `super` is the features module.
//!
//! These cover the three onboarding shapes that resolve to the default (an absent
//! `.agent/`, an absent `config/`, and an absent `rules.yml`), the present-file cases
//! (no `features` block, no `ontology` key, explicit `true`/`false`), the unrelated-key
//! tolerance, and the read and parse errors. The reader resolves without a layout contract.

use std::fs;

use tempfile::TempDir;

use super::*;

/// The `.agent/config/rules.yml` path under a repo root.
fn rules_path(repo: &TempDir) -> std::path::PathBuf {
    repo.path().join(AGENT_DIR).join("config").join("rules.yml")
}

/// Seed `.agent/config/rules.yml` with the given body under a fresh temp repo root.
fn seed_rules(body: &str) -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let config = repo.path().join(AGENT_DIR).join("config");
    fs::create_dir_all(&config).expect("create .agent/config");
    fs::write(config.join("rules.yml"), body).expect("write rules.yml");
    repo
}

// ── The three onboarding shapes: all resolve to enabled (Requirement 1.3) ────────────

#[test]
fn absent_agent_dir_resolves_enabled() {
    // An existing project with no TrueNorth scaffolding: no `.agent/` at all.
    let repo = TempDir::new().expect("temp repo");
    let resolved = resolve(repo.path()).expect("absent .agent resolves");
    assert_eq!(
        resolved,
        Features {
            ontology: true,
            jev: false
        }
    );
}

#[test]
fn absent_config_dir_resolves_enabled() {
    // A bigpowers convert: `.agent/` exists (tasks, product) but no `config/`.
    let repo = TempDir::new().expect("temp repo");
    fs::create_dir_all(repo.path().join(AGENT_DIR).join("tasks")).expect("create .agent/tasks");
    let resolved = resolve(repo.path()).expect("absent config resolves");
    assert_eq!(
        resolved,
        Features {
            ontology: true,
            jev: false
        }
    );
}

#[test]
fn absent_rules_file_resolves_enabled() {
    // A greenfield project mid-scaffold: `config/` exists but `rules.yml` not yet written.
    let repo = TempDir::new().expect("temp repo");
    fs::create_dir_all(repo.path().join(AGENT_DIR).join("config")).expect("create .agent/config");
    let resolved = resolve(repo.path()).expect("absent rules.yml resolves");
    assert_eq!(
        resolved,
        Features {
            ontology: true,
            jev: false
        }
    );
}

// ── Present-file cases ───────────────────────────────────────────────────────────────

#[test]
fn no_features_block_resolves_enabled() {
    let repo = seed_rules("token_caps:\n  max: 1000\n");
    let resolved = resolve(repo.path()).expect("no features block resolves");
    assert_eq!(
        resolved,
        Features {
            ontology: true,
            jev: false
        }
    );
}

#[test]
fn features_block_without_ontology_key_resolves_enabled() {
    let repo = seed_rules("features:\n  other: true\n");
    let resolved = resolve(repo.path()).expect("no ontology key resolves");
    assert_eq!(
        resolved,
        Features {
            ontology: true,
            jev: false
        }
    );
}

#[test]
fn ontology_true_resolves_enabled() {
    let repo = seed_rules("features:\n  ontology: true\n");
    let resolved = resolve(repo.path()).expect("ontology true resolves");
    assert_eq!(
        resolved,
        Features {
            ontology: true,
            jev: false
        }
    );
}

#[test]
fn ontology_false_resolves_disabled() {
    let repo = seed_rules("features:\n  ontology: false\n");
    let resolved = resolve(repo.path()).expect("ontology false resolves");
    assert_eq!(
        resolved,
        Features {
            ontology: false,
            jev: false
        }
    );
}

// ── Tolerance and errors ───────────────────────────────────────────────────────────────

#[test]
fn unrelated_key_alongside_ontology_still_resolves_and_is_left_on_disk() {
    let body = "token_caps:\n  max: 1000\nfeatures:\n  ontology: false\n";
    let repo = seed_rules(body);
    let resolved = resolve(repo.path()).expect("unrelated key tolerated");
    assert_eq!(
        resolved,
        Features {
            ontology: false,
            jev: false
        }
    );
    // The reader never writes, so the file is byte-for-byte unchanged (Requirement 1.9).
    let on_disk = fs::read_to_string(rules_path(&repo)).expect("read back");
    assert_eq!(on_disk, body);
}

#[test]
fn unreadable_config_returns_io_error() {
    // A directory at the rules.yml path makes read_to_string fail with a non-NotFound kind.
    let repo = TempDir::new().expect("temp repo");
    let config = repo.path().join(AGENT_DIR).join("config");
    fs::create_dir_all(&config).expect("create .agent/config");
    fs::create_dir_all(config.join("rules.yml")).expect("create dir at rules.yml path");

    let error = resolve(repo.path()).expect_err("unreadable config rejected");
    match error {
        FeaturesError::Io { path, .. } => {
            assert!(path.contains("rules.yml"), "message names the config path");
        }
        other => panic!("expected Io, got {other:?}"),
    }
}

#[test]
fn malformed_config_returns_parse_error() {
    // A scalar where a mapping is expected fails to deserialize into RulesFeatureView.
    let repo = seed_rules("just a bare string\n");
    let error = resolve(repo.path()).expect_err("malformed config rejected");
    match error {
        FeaturesError::Parse { path, .. } => {
            assert!(path.contains("rules.yml"), "message names the config path");
        }
        other => panic!("expected Parse, got {other:?}"),
    }
}

#[test]
fn resolve_needs_no_layout_contract() {
    // No layout.yml, no profile.yml: the reader still resolves (Requirement 1.10).
    let repo = seed_rules("features:\n  ontology: false\n");
    let resolved = resolve(repo.path()).expect("resolves without a layout contract");
    assert_eq!(
        resolved,
        Features {
            ontology: false,
            jev: false
        }
    );
}

#[test]
fn default_features_are_enabled() {
    assert_eq!(
        Features::default(),
        Features {
            ontology: true,
            jev: false
        }
    );
}

// ── The Jev feature flag: default off (jev-integration-eval Requirement 1.2, 1.3) ──────

#[test]
fn jev_defaults_off_when_agent_dir_absent() {
    // An existing project with no scaffolding resolves the Jev feature to off.
    let repo = TempDir::new().expect("temp repo");
    let resolved = resolve(repo.path()).expect("absent .agent resolves");
    assert!(
        !resolved.jev,
        "an absent .agent/ resolves the Jev feature to off"
    );
}

#[test]
fn jev_defaults_off_when_features_block_omits_the_key() {
    // A present features block with no `jev` key resolves the Jev feature to off.
    let repo = seed_rules("features:\n  ontology: true\n");
    let resolved = resolve(repo.path()).expect("no jev key resolves");
    assert!(
        !resolved.jev,
        "an absent `jev` key resolves the Jev feature to off"
    );
}

#[test]
fn jev_true_resolves_on() {
    let repo = seed_rules("features:\n  jev: true\n");
    let resolved = resolve(repo.path()).expect("jev true resolves");
    assert!(resolved.jev, "`jev: true` resolves the Jev feature to on");
}

#[test]
fn jev_false_resolves_off() {
    let repo = seed_rules("features:\n  jev: false\n");
    let resolved = resolve(repo.path()).expect("jev false resolves");
    assert!(
        !resolved.jev,
        "`jev: false` resolves the Jev feature to off"
    );
}

#[test]
fn ontology_and_jev_resolve_independently() {
    // The two flags read from the same block without cross-talk: ontology off, jev on.
    let repo = seed_rules("features:\n  ontology: false\n  jev: true\n");
    let resolved = resolve(repo.path()).expect("both keys resolve");
    assert_eq!(
        resolved,
        Features {
            ontology: false,
            jev: true
        }
    );
}

#[test]
fn token_caps_resolve_defaults_and_overrides() {
    assert_eq!(
        resolve_token_caps(TempDir::new().expect("temporary repository").path())
            .expect("missing rules resolve defaults"),
        TokenCaps::default()
    );

    let repo = seed_rules("token_caps:\n  skill_lean_tokens: 1200\n  tool_payload_tokens: 4000\n");
    assert_eq!(
        resolve_token_caps(repo.path()).expect("token caps resolve"),
        TokenCaps {
            skill_lean_tokens: 1200,
            tool_payload_tokens: 4000,
        }
    );
}

#[test]
fn token_caps_reject_zero_and_preserve_unrelated_keys() {
    let repo = seed_rules(
        "features:\n  ontology: false\ntoken_caps:\n  skill_lean_tokens: 0\n  tool_payload_tokens: 4000\nother: retained\n",
    );
    assert!(matches!(
        resolve_token_caps(repo.path()),
        Err(FeaturesError::InvalidTokenCap {
            key: "token_caps.skill_lean_tokens"
        })
    ));
    assert_eq!(
        resolve(repo.path()).expect("features still resolve"),
        Features {
            ontology: false,
            jev: false
        }
    );
}

#[test]
fn token_caps_reject_invalid_type_and_negative_values() {
    for body in [
        "token_caps:\n  skill_lean_tokens: text\n",
        "token_caps:\n  tool_payload_tokens: -5\n",
        "token_caps:\n  skill_lean_tokens: 1.5\n",
    ] {
        let repo = seed_rules(body);
        assert!(matches!(
            resolve_token_caps(repo.path()),
            Err(FeaturesError::Parse { .. })
        ));
    }
}

#[test]
fn token_caps_default_independently_and_preserve_unrelated_rules() {
    let body = "features:\n  ontology: false\ntoken_caps:\n  tool_payload_tokens: 2500\nmethodology:\n  model: generic\n";
    let repo = seed_rules(body);
    let caps = resolve_token_caps(repo.path()).expect("resolve partial caps");
    assert_eq!(caps.skill_lean_tokens, 1500);
    assert_eq!(caps.tool_payload_tokens, 2500);
    assert_eq!(
        fs::read_to_string(rules_path(&repo)).expect("read rules"),
        body
    );
}
