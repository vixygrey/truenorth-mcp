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
fn init_requires_a_skill_bundle_and_accepts_profile_in_any_option_order() {
    assert_eq!(
        parse_args([
            "init".to_string(),
            "--profile".to_string(),
            "generic".to_string(),
            "--skills-dir".to_string(),
            "/tmp/skills".to_string(),
        ])
        .expect("parse init"),
        Mode::Init {
            profile: Some("generic".to_string()),
            skills_dir: PathBuf::from("/tmp/skills"),
        }
    );
    assert_eq!(
        parse_args([
            "init".to_string(),
            "--skills-dir".to_string(),
            "/tmp/skills".to_string(),
        ])
        .expect("parse init"),
        Mode::Init {
            profile: None,
            skills_dir: PathBuf::from("/tmp/skills"),
        }
    );
}

#[test]
fn init_rejects_missing_duplicate_and_unknown_options() {
    for args in [
        vec!["init".to_string()],
        vec!["init".to_string(), "--skills-dir".to_string()],
        vec![
            "init".to_string(),
            "--skills-dir".to_string(),
            "one".to_string(),
            "--skills-dir".to_string(),
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
fn unsupported_or_combined_arguments_return_usage() {
    for args in [
        vec!["guard".to_string()],
        vec!["--version".to_string(), "--check-config".to_string()],
    ] {
        let error = parse_args(args).expect_err("arguments must be rejected");
        assert_eq!(error, USAGE);
    }
}
