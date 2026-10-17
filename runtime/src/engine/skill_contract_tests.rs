//! Tests for the versioned per-skill contract.

use std::fs;

use super::*;
use tempfile::tempdir;

fn write_skill(root: &std::path::Path, name: &str) {
    let skill_dir = root.join("skills").join(name);
    fs::create_dir_all(&skill_dir).expect("create skill directory");
    fs::write(skill_dir.join("SKILL.md"), "# Skill\n").expect("write skill");
}

fn valid_contract(name: &str) -> String {
    format!(
        "schema_version: 1\nname: {name}\ndescription: Validate a contract.\nscale: task\nside_effects: true\nrisks: true\neffect_categories:\n  - filesystem_write\nrisk_categories:\n  - data_loss\npreconditions:\n  - clean-worktree\nhandoffs:\n  - skill: verify-work\n    when: after-success\n    required: true\noutputs:\n  - validation-report\n"
    )
}

#[test]
fn reads_and_validates_a_v1_contract() {
    let dir = tempdir().expect("temp directory");
    write_skill(dir.path(), "validate-work");
    write_skill(dir.path(), "verify-work");
    fs::write(
        dir.path().join("skills/validate-work/contract.yml"),
        valid_contract("validate-work"),
    )
    .expect("write contract");

    let report = validate_skill_contract(dir.path(), "validate-work");

    assert!(report.pass, "{:#?}", report.diagnostics);
    assert!(report.contract.is_some());
}

#[test]
fn rejects_a_contract_with_a_name_mismatch() {
    let dir = tempdir().expect("temp directory");
    write_skill(dir.path(), "validate-work");
    write_skill(dir.path(), "verify-work");
    fs::write(
        dir.path().join("skills/validate-work/contract.yml"),
        valid_contract("other-work"),
    )
    .expect("write contract");

    let report = validate_skill_contract(dir.path(), "validate-work");

    assert!(!report.pass);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == "contract-name")
    );
}

#[test]
fn rejects_category_boolean_mismatches_and_unknown_required_handoffs() {
    let dir = tempdir().expect("temp directory");
    write_skill(dir.path(), "validate-work");
    let contract = valid_contract("validate-work")
        .replace("side_effects: true", "side_effects: false")
        .replace("risks: true", "risks: false")
        .replace("verify-work", "missing-work");
    fs::write(
        dir.path().join("skills/validate-work/contract.yml"),
        contract,
    )
    .expect("write contract");

    let report = validate_skill_contract(dir.path(), "validate-work");

    assert!(!report.pass);
    let ids: Vec<_> = report
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.id.as_str())
        .collect();
    assert!(ids.contains(&"side-effects-consistency"));
    assert!(ids.contains(&"risks-consistency"));
    assert!(ids.contains(&"required-handoff-target"));
}

#[test]
fn batch_rejects_missing_contracts() {
    let dir = tempdir().expect("temp directory");
    write_skill(dir.path(), "validate-work");

    let report = validate_skill_contracts(dir.path());

    assert!(!report.pass);
    assert_eq!(report.missing, vec!["validate-work"]);
    assert!(report.invalid.is_empty());
    assert!(!report.reports[0].pass);
}

#[test]
fn rejects_unknown_schema_version_and_unknown_fields() {
    let dir = tempdir().expect("temp directory");
    write_skill(dir.path(), "validate-work");
    let contract = valid_contract("validate-work")
        .replace("schema_version: 1", "schema_version: 2")
        + "unexpected: value\n";
    fs::write(
        dir.path().join("skills/validate-work/contract.yml"),
        contract,
    )
    .expect("write contract");

    let report = validate_skill_contract(dir.path(), "validate-work");

    assert!(!report.pass);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == "contract-parse")
    );
}

#[test]
fn rejects_duplicate_values_and_invalid_handoff_labels() {
    let dir = tempdir().expect("temp directory");
    write_skill(dir.path(), "validate-work");
    let contract = valid_contract("validate-work")
        .replace(
            "effect_categories:\n  - filesystem_write",
            "effect_categories:\n  - filesystem_write\n  - filesystem_write",
        )
        .replace(
            "outputs:\n  - validation-report",
            "outputs:\n  - validation-report\n  - validation-report",
        )
        .replace("when: after-success", "when: after success");
    fs::write(
        dir.path().join("skills/validate-work/contract.yml"),
        contract,
    )
    .expect("write contract");

    let report = validate_skill_contract(dir.path(), "validate-work");

    assert!(!report.pass);
    let ids: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|diagnostic| !diagnostic.pass)
        .map(|diagnostic| diagnostic.id.as_str())
        .collect();
    assert!(ids.contains(&"effect-categories"));
    assert!(ids.contains(&"outputs"));
    assert!(ids.contains(&"handoff-when"));
}

#[test]
fn rejects_contracts_larger_than_the_read_limit() {
    let dir = tempdir().expect("temp directory");
    write_skill(dir.path(), "validate-work");
    fs::write(
        dir.path().join("skills/validate-work/contract.yml"),
        "x".repeat(MAX_READ_SKILL_CONTRACT_BYTES + 1),
    )
    .expect("write contract");

    let report = validate_skill_contract(dir.path(), "validate-work");

    assert!(!report.pass);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == "contract-parse")
    );
}

#[test]
fn checked_in_skill_catalog_has_complete_valid_contracts() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .to_path_buf();

    let report = validate_skill_contracts(&root);

    assert!(report.pass, "{report:#?}");
    assert!(report.missing.is_empty(), "{report:#?}");
    assert!(report.invalid.is_empty(), "{report:#?}");
    assert_eq!(report.reports.len(), discover_skills(&root).len());
    assert!(report.reports.iter().all(|item| !item.missing && item.pass));
}
