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
fn unsupported_or_combined_arguments_return_usage() {
    for args in [
        vec!["guard".to_string()],
        vec!["--version".to_string(), "--check-config".to_string()],
    ] {
        let error = parse_args(args).expect_err("arguments must be rejected");
        assert_eq!(error, USAGE);
    }
}
