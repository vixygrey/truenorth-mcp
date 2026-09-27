//! Unit tests for command-line parsing.

use super::*;

#[test]
fn no_arguments_starts_the_mcp_server() {
    assert_eq!(parse_args([]).expect("parse arguments"), Mode::Serve);
}

#[test]
fn version_argument_selects_version_mode() {
    assert_eq!(
        parse_args(["--version".to_string()]).expect("parse arguments"),
        Mode::Version
    );
}

#[test]
fn check_config_argument_selects_diagnostics_mode() {
    assert_eq!(
        parse_args(["--check-config".to_string()]).expect("parse arguments"),
        Mode::CheckConfig
    );
}

#[test]
fn init_requires_a_package_bundle_and_accepts_profile_in_any_option_order() {
    assert_eq!(
        parse_args([
            "init".to_string(),
            "--profile".to_string(),
            "generic".to_string(),
            "--bundle-dir".to_string(),
            "/tmp/package".to_string(),
        ])
        .expect("parse init"),
        Mode::Init {
            profile: Some("generic".to_string()),
            bundle_dir: PathBuf::from("/tmp/package"),
            skill_sets: Vec::new(),
        }
    );
    assert_eq!(
        parse_args([
            "init".to_string(),
            "--bundle-dir".to_string(),
            "/tmp/package".to_string(),
        ])
        .expect("parse init"),
        Mode::Init {
            profile: None,
            bundle_dir: PathBuf::from("/tmp/package"),
            skill_sets: Vec::new(),
        }
    );
}

#[test]
fn init_rejects_missing_duplicate_and_unknown_options() {
    for args in [
        vec!["init".to_string()],
        vec!["init".to_string(), "--bundle-dir".to_string()],
        vec![
            "init".to_string(),
            "--bundle-dir".to_string(),
            "one".to_string(),
            "--bundle-dir".to_string(),
            "two".to_string(),
        ],
        vec![
            "init".to_string(),
            "--unknown".to_string(),
            "value".to_string(),
        ],
    ] {
        assert_eq!(
            parse_args(args).expect_err("must reject invalid init"),
            USAGE
        );
    }
}

#[test]
fn upgrade_accepts_check_and_requires_the_package_bundle() {
    assert_eq!(
        parse_args([
            "upgrade".to_string(),
            "--check".to_string(),
            "--bundle-dir".to_string(),
            "/tmp/package".to_string(),
        ])
        .expect("parse upgrade"),
        Mode::Upgrade {
            check: true,
            bundle_dir: PathBuf::from("/tmp/package"),
            add_skill_sets: Vec::new(),
            remove_skill_sets: Vec::new(),
        }
    );
    assert_eq!(
        parse_args([
            "upgrade".to_string(),
            "--bundle-dir".to_string(),
            "/tmp/package".to_string(),
        ])
        .expect("parse upgrade"),
        Mode::Upgrade {
            check: false,
            bundle_dir: PathBuf::from("/tmp/package"),
            add_skill_sets: Vec::new(),
            remove_skill_sets: Vec::new(),
        }
    );
    assert!(parse_args(["upgrade".to_string(), "--check".to_string()]).is_err());
}

#[test]
fn skill_set_options_are_repeatable_and_catalog_is_read_only() {
    assert_eq!(
        parse_args([
            "init".to_string(),
            "--bundle-dir".to_string(),
            "/tmp/package".to_string(),
            "--skill-set".to_string(),
            "visual".to_string(),
            "--skill-set".to_string(),
            "integrations".to_string(),
        ])
        .expect("parse init skill sets"),
        Mode::Init {
            profile: None,
            bundle_dir: PathBuf::from("/tmp/package"),
            skill_sets: vec!["visual".to_string(), "integrations".to_string()],
        }
    );
    assert_eq!(
        parse_args([
            "upgrade".to_string(),
            "--bundle-dir".to_string(),
            "/tmp/package".to_string(),
            "--add-skill-set".to_string(),
            "visual".to_string(),
            "--remove-skill-set".to_string(),
            "integrations".to_string(),
        ])
        .expect("parse upgrade skill sets"),
        Mode::Upgrade {
            check: false,
            bundle_dir: PathBuf::from("/tmp/package"),
            add_skill_sets: vec!["visual".to_string()],
            remove_skill_sets: vec!["integrations".to_string()],
        }
    );
    assert_eq!(
        parse_args([
            "skills".to_string(),
            "list".to_string(),
            "--bundle-dir".to_string(),
            "/tmp/package".to_string(),
        ])
        .expect("parse skills list"),
        Mode::SkillsList {
            bundle_dir: PathBuf::from("/tmp/package"),
        }
    );
}

#[test]
fn unsupported_or_combined_arguments_return_usage() {
    for args in [
        vec!["guard".to_string()],
        vec!["--version".to_string(), "--check-config".to_string()],
    ] {
        let error = parse_args(args).expect_err("arguments must be rejected");
        assert_eq!(error, USAGE);
    }
}
