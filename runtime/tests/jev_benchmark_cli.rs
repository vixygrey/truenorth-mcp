//! Process-level evidence that benchmark input failures preserve reports and pending policies reject.

#![cfg(feature = "jev-http")]

use std::path::Path;
use std::process::Command;

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("repository");
    for marker in [".agent", "specs", "skills", "benchmarks/jev"] {
        std::fs::create_dir_all(dir.path().join(marker)).expect("root marker");
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace");
    for name in ["fixtures-v1.yml", "policy-v1.yml", "source-v1.yml"] {
        std::fs::copy(
            root.join("benchmarks/jev").join(name),
            dir.path().join("benchmarks/jev").join(name),
        )
        .expect("versioned input");
    }
    // Authorization scenarios use a small pending suite, not the production pilot coverage limits.
    let policy_path = dir.path().join("benchmarks/jev/policy-v1.yml");
    let mut policy: serde_json::Value = truenorth_mcp::engine::yaml::from_str(
        &std::fs::read_to_string(&policy_path).expect("policy"),
    )
    .expect("policy data");
    policy["limits"] = serde_json::Value::Null;
    std::fs::write(policy_path, policy.to_string()).expect("small pending test policy");
    dir
}

fn run(root: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_jev-bench"))
        .current_dir(root)
        .env_clear()
        .env("TRUENORTH_ROOT", root)
        .output()
        .expect("benchmark process")
}

#[test]
fn default_fake_never_passes_promotion_and_writes_only_aggregate_telemetry() {
    let repo = repo();
    let result = run(repo.path());
    assert!(
        !result.status.success(),
        "pending policy and erroring fake cannot pass"
    );
    let report_path = repo.path().join(".agent/telemetry/jev-benchmark.yml");
    let report = std::fs::read_to_string(report_path).expect("rejected aggregate report");
    assert!(report.contains("accepted: false"));
    assert!(report.contains("unknown_usage_calls:"));
    assert!(!report.contains("synthetic-placeholder-not-a-real-key"));
    assert!(!report.contains("src/example.rs"));
    assert!(
        !repo
            .path()
            .join(".agent/telemetry/jev-fixtures.yml")
            .exists()
    );
}

#[test]
fn invalid_suite_and_policy_leave_previous_report_intact() {
    let repo = repo();
    let report_path = repo.path().join(".agent/telemetry/jev-benchmark.yml");
    std::fs::create_dir_all(report_path.parent().unwrap()).unwrap();
    std::fs::write(&report_path, "prior aggregate only\n").unwrap();
    let fixture = repo.path().join("benchmarks/jev/fixtures-v1.yml");
    std::fs::write(&fixture, "schema_version: 1\ncases: []\n").unwrap();
    assert!(!run(repo.path()).status.success());
    assert_eq!(
        std::fs::read_to_string(&report_path).unwrap(),
        "prior aggregate only\n"
    );
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    std::fs::copy(workspace.join("benchmarks/jev/fixtures-v1.yml"), &fixture).unwrap();
    std::fs::write(
        repo.path().join("benchmarks/jev/policy-v1.yml"),
        "approval: approved\n",
    )
    .unwrap();
    assert!(!run(repo.path()).status.success());
    assert_eq!(
        std::fs::read_to_string(&report_path).unwrap(),
        "prior aggregate only\n"
    );
}

#[test]
fn explicit_http_without_approved_policy_refuses_to_call_provider() {
    let repo = repo();
    let output = Command::new(env!("CARGO_BIN_EXE_jev-bench"))
        .current_dir(repo.path())
        .env_clear()
        .env("TRUENORTH_ROOT", repo.path())
        .env("JEV_BENCH_CLIENT", "http")
        .env(
            "TRUENORTH_JEV_ENDPOINT",
            "https://example.invalid/v1/systemone",
        )
        .env("TRUENORTH_JEV_API_KEY", "test-only-value")
        .output()
        .expect("benchmark process");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("JEV_BENCH_CALIBRATE=1"));
    assert!(
        !repo
            .path()
            .join(".agent/telemetry/jev-benchmark.yml")
            .exists()
    );
}

#[test]
fn calibration_requires_both_reviewed_estimate_and_provider_spending_cap() {
    let repo = repo();
    for (budget, provider_cap, diagnostic) in [
        (None, None, "JEV_BENCH_MAX_ESTIMATED_USD"),
        (Some("0.01"), None, "JEV_BENCH_PROVIDER_SPEND_CAP_CONFIRMED"),
        (Some("NaN"), Some("true"), "JEV_BENCH_MAX_ESTIMATED_USD"),
        (
            Some("0.01"),
            Some("false"),
            "JEV_BENCH_PROVIDER_SPEND_CAP_CONFIRMED",
        ),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_jev-bench"));
        command
            .current_dir(repo.path())
            .env_clear()
            .env("TRUENORTH_ROOT", repo.path())
            .env("JEV_BENCH_CLIENT", "http")
            .env("JEV_BENCH_CALIBRATE", "1");
        if let Some(budget) = budget {
            command.env("JEV_BENCH_MAX_ESTIMATED_USD", budget);
        }
        if let Some(cap) = provider_cap {
            command.env("JEV_BENCH_PROVIDER_SPEND_CAP_CONFIRMED", cap);
        }
        let output = command.output().expect("benchmark process");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
        assert!(
            !repo
                .path()
                .join(".agent/telemetry/jev-benchmark.yml")
                .exists()
        );
    }
}

#[test]
fn symlinked_and_outside_inputs_are_rejected_before_report_write() {
    let repo = repo();
    let outside = tempfile::tempdir().unwrap();
    let source = repo.path().join("benchmarks/jev/fixtures-v1.yml");
    let linked = repo.path().join("benchmarks/jev/link.yml");
    std::os::unix::fs::symlink(&source, &linked).unwrap();
    for path in [
        linked,
        outside.path().join("fixture.yml"),
        repo.path().join(".agent/telemetry/no.yml"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_jev-bench"))
            .current_dir(repo.path())
            .env_clear()
            .env("TRUENORTH_ROOT", repo.path())
            .env("JEV_BENCH_FIXTURES", path)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            !repo
                .path()
                .join(".agent/telemetry/jev-benchmark.yml")
                .exists()
        );
    }
}

#[test]
fn nested_inputs_run_but_symlinked_parent_and_traversal_are_rejected() {
    let repo = repo();
    let base = repo.path().join("benchmarks/jev");
    let nested = base.join("holdout");
    std::fs::create_dir(&nested).unwrap();
    for name in ["fixtures-v1.yml", "source-v1.yml"] {
        std::fs::copy(base.join(name), nested.join(name)).unwrap();
    }
    let invoke = |path: &Path| {
        Command::new(env!("CARGO_BIN_EXE_jev-bench"))
            .current_dir(repo.path())
            .env_clear()
            .env("TRUENORTH_ROOT", repo.path())
            .env("JEV_BENCH_FIXTURES", path)
            .output()
            .unwrap()
    };
    let report = repo.path().join(".agent/telemetry/jev-benchmark.yml");
    let output = invoke(&nested.join("fixtures-v1.yml"));
    assert!(!output.status.success());
    assert!(
        report.is_file(),
        "valid nested inputs reached offline evaluation"
    );
    std::fs::remove_file(&report).unwrap();
    let linked = base.join("redirect");
    std::os::unix::fs::symlink(&nested, &linked).unwrap();
    for rejected in [
        linked.join("fixtures-v1.yml"),
        nested.join("../fixtures-v1.yml"),
    ] {
        assert!(!invoke(&rejected).status.success());
        assert!(
            !report.exists(),
            "unsafe inputs cannot produce evaluation evidence"
        );
    }
}
