//! Versioned, declarative contracts adjacent to skill instructions.
//!
//! A contract describes a skill's observable effects, risks, prerequisites, declared
//! handoffs, and outputs. `SKILL.md` remains the procedural source of truth.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::engine::agent_ws::fs_boundary::RepositoryFs;
use crate::engine::skill::{SkillError, discover_skills, resolve_skill_path};

const SCHEMA_VERSION: u8 = 1;
const CONTRACT_FILE: &str = "contract.yml";
/// Maximum bytes read from one contract before rejecting it.
pub const MAX_READ_SKILL_CONTRACT_BYTES: usize = 64 * 1024;

/// The bounded scope of a skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillScale {
    Atomic,
    Task,
    Workflow,
    System,
}

/// An observable side-effect class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectCategory {
    FilesystemRead,
    FilesystemWrite,
    Network,
    ProcessExecution,
    GitRead,
    GitWrite,
    GithubRead,
    GithubWrite,
    ExternalServiceWrite,
    PackagePublish,
    Deployment,
    BrowserAutomation,
}

/// A user-relevant risk class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskCategory {
    DestructiveOperation,
    IrreversibleMerge,
    ReleasePublication,
    BranchDeletion,
    ProductionChange,
    CredentialAccess,
    DataLoss,
}

/// A declared successor skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillHandoff {
    pub skill: String,
    pub when: String,
    pub required: bool,
}

/// The version-one declarative contract for a skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillContract {
    pub schema_version: u8,
    pub name: String,
    pub description: String,
    pub scale: SkillScale,
    pub side_effects: bool,
    pub risks: bool,
    pub effect_categories: Vec<EffectCategory>,
    pub risk_categories: Vec<RiskCategory>,
    pub preconditions: Vec<String>,
    pub handoffs: Vec<SkillHandoff>,
    pub outputs: Vec<String>,
}

/// A contract parse or read failure.
#[derive(Debug, Error)]
pub enum SkillContractError {
    #[error("could not resolve contract for skill `{name}`: {source}")]
    Resolve {
        name: String,
        #[source]
        source: SkillError,
    },
    #[error("could not read contract for skill `{name}`: {source}")]
    Read {
        name: String,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "contract for skill `{name}` exceeds {max_bytes} bytes ({actual_bytes} bytes read); \
         reduce contract.yml to the supported size"
    )]
    TooLarge {
        name: String,
        max_bytes: usize,
        actual_bytes: usize,
    },
    #[error("could not parse contract for skill `{name}`: {source}")]
    Parse {
        name: String,
        #[source]
        source: saneyaml::Error,
    },
}

/// One contract validation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContractDiagnostic {
    pub id: String,
    pub pass: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

/// Validation result for one skill contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillContractReport {
    pub skill: String,
    pub pass: bool,
    pub missing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<SkillContract>,
    pub diagnostics: Vec<ContractDiagnostic>,
}

/// Migration-mode batch validation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillContractCatalogReport {
    pub pass: bool,
    pub missing: Vec<String>,
    pub invalid: Vec<String>,
    pub reports: Vec<SkillContractReport>,
}

/// Return a guarded contract path beside a skill's `SKILL.md`.
pub fn resolve_skill_contract_path(
    repo_root: &Path,
    name: &str,
) -> Result<PathBuf, SkillContractError> {
    let skill_path =
        resolve_skill_path(repo_root, name).map_err(|source| SkillContractError::Resolve {
            name: name.to_string(),
            source,
        })?;
    Ok(skill_path.with_file_name(CONTRACT_FILE))
}

/// Read and deserialize one skill contract.
pub fn read_skill_contract(
    repo_root: &Path,
    name: &str,
) -> Result<SkillContract, SkillContractError> {
    let path = resolve_skill_contract_path(repo_root, name)?;
    let relative = path
        .strip_prefix(repo_root)
        .map_err(|source| SkillContractError::Read {
            name: name.to_string(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, source),
        })?;
    let fs = RepositoryFs::open(repo_root).map_err(|source| SkillContractError::Read {
        name: name.to_string(),
        source,
    })?;
    let bytes = fs
        .read(relative)
        .map_err(|source| SkillContractError::Read {
            name: name.to_string(),
            source,
        })?;
    if bytes.len() > MAX_READ_SKILL_CONTRACT_BYTES {
        return Err(SkillContractError::TooLarge {
            name: name.to_string(),
            max_bytes: MAX_READ_SKILL_CONTRACT_BYTES,
            actual_bytes: bytes.len(),
        });
    }
    crate::engine::yaml::from_slice(&bytes).map_err(|source| SkillContractError::Parse {
        name: name.to_string(),
        source,
    })
}

/// Validate one contract. Missing contracts fail validation.
pub fn validate_skill_contract(repo_root: &Path, name: &str) -> SkillContractReport {
    let path = match resolve_skill_contract_path(repo_root, name) {
        Ok(path) => path,
        Err(error) => return invalid_report(name, "contract-path", error.to_string()),
    };
    if !path.is_file() {
        return SkillContractReport {
            skill: name.to_string(),
            pass: false,
            missing: true,
            contract: None,
            diagnostics: vec![diagnostic(
                "contract-missing",
                false,
                format!("No contract at {}", path.display()),
                Some("Add a valid contract.yml".to_string()),
            )],
        };
    }

    match read_skill_contract(repo_root, name) {
        Ok(contract) => validate_present_contract(repo_root, name, contract),
        Err(error) => invalid_report(name, "contract-parse", error.to_string()),
    }
}

/// Validate all discovered contracts and reject absent or invalid contracts.
pub fn validate_skill_contracts(repo_root: &Path) -> SkillContractCatalogReport {
    let reports: Vec<_> = discover_skills(repo_root)
        .into_iter()
        .map(|entry| validate_skill_contract(repo_root, &entry.name))
        .collect();
    let missing: Vec<String> = reports
        .iter()
        .filter(|report| report.missing)
        .map(|report| report.skill.clone())
        .collect();
    let invalid: Vec<String> = reports
        .iter()
        .filter(|report| !report.missing && !report.pass)
        .map(|report| report.skill.clone())
        .collect();
    SkillContractCatalogReport {
        pass: missing.is_empty() && invalid.is_empty(),
        missing,
        invalid,
        reports,
    }
}

fn validate_present_contract(
    repo_root: &Path,
    name: &str,
    contract: SkillContract,
) -> SkillContractReport {
    let mut diagnostics = Vec::new();
    diagnostics.push(check(
        "schema-version",
        contract.schema_version == SCHEMA_VERSION,
        format!("Uses supported schema version {SCHEMA_VERSION}"),
        format!("Set schema_version to {SCHEMA_VERSION}"),
    ));
    diagnostics.push(check(
        "contract-name",
        contract.name == name,
        format!("Contract name matches skill `{name}`"),
        format!("Set name to `{name}`"),
    ));
    diagnostics.push(check(
        "contract-description",
        !contract.description.trim().is_empty(),
        "Contract has a nonempty description".to_string(),
        "Set description to a concise capability statement".to_string(),
    ));
    diagnostics.push(check(
        "side-effects-consistency",
        contract.side_effects != contract.effect_categories.is_empty(),
        "side_effects agrees with effect_categories".to_string(),
        "Set side_effects to whether effect_categories is nonempty".to_string(),
    ));
    diagnostics.push(check(
        "risks-consistency",
        contract.risks != contract.risk_categories.is_empty(),
        "risks agrees with risk_categories".to_string(),
        "Set risks to whether risk_categories is nonempty".to_string(),
    ));
    diagnostics.push(check(
        "outputs",
        !contract.outputs.is_empty()
            && all_nonempty(&contract.outputs)
            && is_unique(&contract.outputs),
        "outputs contains unique nonempty values".to_string(),
        "Declare at least one unique nonempty output".to_string(),
    ));
    diagnostics.push(check(
        "preconditions",
        all_nonempty(&contract.preconditions) && is_unique(&contract.preconditions),
        "preconditions contains unique nonempty values".to_string(),
        "Use unique nonempty precondition labels".to_string(),
    ));
    diagnostics.push(check(
        "effect-categories",
        is_unique(&contract.effect_categories),
        "effect_categories contains unique values".to_string(),
        "Remove duplicate effect categories".to_string(),
    ));
    diagnostics.push(check(
        "risk-categories",
        is_unique(&contract.risk_categories),
        "risk_categories contains unique values".to_string(),
        "Remove duplicate risk categories".to_string(),
    ));

    let known: BTreeSet<_> = discover_skills(repo_root)
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    let mut handoff_keys = BTreeSet::new();
    for handoff in &contract.handoffs {
        diagnostics.push(check(
            "handoff-when",
            is_kebab_label(&handoff.when),
            format!("Handoff to `{}` has a valid when label", handoff.skill),
            "Use a nonempty kebab-case when label".to_string(),
        ));
        diagnostics.push(check(
            "required-handoff-target",
            !handoff.required || known.contains(&handoff.skill),
            format!("Required handoff target `{}` resolves", handoff.skill),
            "Point a required handoff at an existing skill".to_string(),
        ));
        let unique = handoff_keys.insert((handoff.skill.as_str(), handoff.when.as_str()));
        diagnostics.push(check(
            "handoff-uniqueness",
            unique,
            format!("Handoff to `{}` is unique", handoff.skill),
            "Remove duplicate skill and when handoffs".to_string(),
        ));
    }

    let pass = diagnostics.iter().all(|diagnostic| diagnostic.pass);
    SkillContractReport {
        skill: name.to_string(),
        pass,
        missing: false,
        contract: Some(contract),
        diagnostics,
    }
}

fn invalid_report(name: &str, id: &str, message: String) -> SkillContractReport {
    SkillContractReport {
        skill: name.to_string(),
        pass: false,
        missing: false,
        contract: None,
        diagnostics: vec![diagnostic(
            id,
            false,
            message,
            Some("Fix contract.yml and rerun contract validation".to_string()),
        )],
    }
}

fn diagnostic(
    id: &str,
    pass: bool,
    message: String,
    remediation: Option<String>,
) -> ContractDiagnostic {
    ContractDiagnostic {
        id: id.to_string(),
        pass,
        message,
        remediation,
    }
}

fn check(id: &str, pass: bool, message: String, remediation: String) -> ContractDiagnostic {
    diagnostic(id, pass, message, (!pass).then_some(remediation))
}

fn all_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
}

fn is_unique<T: Ord>(values: &[T]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn is_kebab_label(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        })
}

#[cfg(test)]
#[path = "skill_contract_tests.rs"]
mod tests;
