//! Validate checked-in skill contracts.

use std::path::PathBuf;

use truenorth_mcp::engine::skill_contract::validate_skill_contracts;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent().ok_or_else(|| {
        std::io::Error::other("runtime manifest directory has no repository parent")
    })?;
    let report = validate_skill_contracts(repo_root);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "pass": report.pass,
            "missing": report.missing,
            "invalid": report.invalid,
        }))?
    );

    if report.pass {
        Ok(())
    } else {
        Err(format!(
            "missing skill contracts: {}; invalid skill contracts: {}",
            report.missing.join(", "),
            report.invalid.join(", ")
        )
        .into())
    }
}
