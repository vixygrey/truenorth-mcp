//! Backward-compatible validation for the bigpowers cockpit and legacy phase mapping.
//!
//! This module validates `state.yaml` and `release-plan.yaml` content against the
//! observed bigpowers shapes (Requirement 9.1). A read that fails validation is rejected
//! with an error that names the file and the failed constraint, and the caller leaves
//! the file unmodified (Requirement 9.2). The write validators let a tool confirm its
//! serialized output still parses before it touches disk, so a write never corrupts a
//! human-authored file (Requirement 9.3).
//!
//! The module also maps legacy bigpowers phase names onto the six-phase model
//! (Requirement 9.5) and rejects an unrecognized legacy name (Requirement 9.6).
//!
//! Requirements: 9.1, 9.2, 9.5, 9.6. Design: Part II §8.

// These validators and the phase mapping are consumed by later tasks: the lifecycle and
// ontology tools (tasks 9, 12) and the resources layer (task 14). They are unused until
// those tasks land, so the module-scoped allow prevents a premature dead-code error
// under `clippy -D warnings`. Remove this allow once task 14 wires the last consumer.
#![allow(dead_code)]

use thiserror::Error;

use crate::engine::spec::{Phase, ReleasePlanFile, StateFile};

/// The cockpit file a validation error refers to. The value names the file in the
/// error message, so an operator can locate it (Requirement 9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CockpitFile {
    /// `state.yaml`.
    State,
    /// `release-plan.yaml`.
    ReleasePlan,
}

impl CockpitFile {
    /// The file name for the error message.
    fn name(self) -> &'static str {
        match self {
            CockpitFile::State => "state.yaml",
            CockpitFile::ReleasePlan => "release-plan.yaml",
        }
    }
}

/// A validation or phase-mapping error.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// A cockpit file failed to parse against its observed schema (Requirement 9.2).
    #[error(
        "{file} failed schema validation: {detail}. The file was left unchanged. Fix the YAML and retry."
    )]
    Schema {
        /// The offending file.
        file: &'static str,
        /// The parser detail, including the location of the failed constraint.
        detail: String,
    },

    /// A legacy phase name was not one of the recognized names (Requirement 9.6).
    #[error(
        "unrecognized legacy phase name `{name}`. \
         The recognized legacy names are Build, Verify, Release, and Sustain, \
         which map to the six-phase model (Execute, Review, Integrate). \
         Use one of the six-phase names directly: \
         discover, design, plan, execute, review, integrate."
    )]
    UnknownPhase {
        /// The offending phase name.
        name: String,
    },
}

/// Validate and parse `state.yaml` content (Requirements 9.1, 9.2).
///
/// The content must be a mapping. When the observed fields are present, they must match
/// their observed types: `git` and `handoff` and `metrics` are mappings, and
/// `metrics.skill_timings` is a mapping. Unknown fields pass, so a human edit is never
/// rejected for adding a field.
///
/// # Errors
///
/// Returns [`ValidationError::Schema`] naming `state.yaml` when the content does not
/// parse or a present field has the wrong type. The caller leaves the file unchanged.
pub fn validate_state(yaml: &str) -> Result<StateFile, ValidationError> {
    let state: StateFile = parse(yaml, CockpitFile::State)?;
    check_mapping_field(&state.root, "git", CockpitFile::State)?;
    check_mapping_field(&state.root, "handoff", CockpitFile::State)?;
    if let Some(metrics) = state.root.get("metrics") {
        expect_mapping(metrics, "metrics", CockpitFile::State)?;
        check_mapping_field(
            metrics.as_mapping().expect("metrics is a mapping"),
            "skill_timings",
            CockpitFile::State,
        )?;
    }
    Ok(state)
}

/// Validate and parse `release-plan.yaml` content (Requirements 9.1, 9.2).
///
/// The content must be a mapping. When `build_order` is present, it must be a sequence.
/// Unknown fields pass.
///
/// # Errors
///
/// Returns [`ValidationError::Schema`] naming `release-plan.yaml` when the content does
/// not parse or `build_order` is not a sequence. The caller leaves the file unchanged.
pub fn validate_release_plan(yaml: &str) -> Result<ReleasePlanFile, ValidationError> {
    let plan: ReleasePlanFile = parse(yaml, CockpitFile::ReleasePlan)?;
    if let Some(build_order) = plan.root.get("build_order")
        && !build_order.is_sequence()
    {
        return Err(ValidationError::Schema {
            file: CockpitFile::ReleasePlan.name(),
            detail: "`build_order` must be a sequence of epic ids".to_string(),
        });
    }
    Ok(plan)
}

/// Validate that a `StateFile` serializes and re-parses cleanly before a write
/// (Requirement 9.3).
///
/// This confirms a tool's mutated state still validates against the observed schema. The
/// caller writes only after this returns the serialized YAML.
///
/// # Errors
///
/// Returns [`ValidationError::Schema`] when serialization or the re-parse fails. The
/// caller leaves the target file in its pre-invocation state.
pub fn validate_state_for_write(state: &StateFile) -> Result<String, ValidationError> {
    serialize_and_recheck(state, CockpitFile::State, validate_state)
}

/// Validate that a `ReleasePlanFile` serializes and re-parses cleanly before a write
/// (Requirement 9.3).
///
/// # Errors
///
/// Returns [`ValidationError::Schema`] when serialization or the re-parse fails. The
/// caller leaves the target file in its pre-invocation state.
pub fn validate_release_plan_for_write(plan: &ReleasePlanFile) -> Result<String, ValidationError> {
    serialize_and_recheck(plan, CockpitFile::ReleasePlan, validate_release_plan)
}

/// Map a legacy bigpowers phase name onto the six-phase model (Requirement 9.5).
///
/// The mapping is `Build → Execute`, `Verify → Review`, and `Release`/`Sustain →
/// Integrate`. A six-phase name (for example `execute`) also resolves to its own phase,
/// so the mapping is idempotent. The match is case-insensitive.
///
/// # Errors
///
/// Returns [`ValidationError::UnknownPhase`] when the name is neither a recognized legacy
/// name nor a six-phase name (Requirement 9.6).
pub fn map_legacy_phase(name: &str) -> Result<Phase, ValidationError> {
    match name.trim().to_ascii_lowercase().as_str() {
        // Legacy names.
        "build" => Ok(Phase::Execute),
        "verify" => Ok(Phase::Review),
        "release" | "sustain" => Ok(Phase::Integrate),
        // Six-phase names resolve to themselves, so the mapping is idempotent.
        "discover" => Ok(Phase::Discover),
        "design" => Ok(Phase::Design),
        "plan" => Ok(Phase::Plan),
        "execute" => Ok(Phase::Execute),
        "review" => Ok(Phase::Review),
        "integrate" => Ok(Phase::Integrate),
        _ => Err(ValidationError::UnknownPhase {
            name: name.to_string(),
        }),
    }
}

/// Parse cockpit YAML into a typed model, wrapping a parse failure in a schema error
/// that names the file.
fn parse<T>(yaml: &str, file: CockpitFile) -> Result<T, ValidationError>
where
    T: serde::de::DeserializeOwned,
{
    serde_yaml::from_str(yaml).map_err(|error| ValidationError::Schema {
        file: file.name(),
        detail: error.to_string(),
    })
}

/// Reject a present field that is not a mapping. An absent field passes.
fn check_mapping_field(
    root: &serde_yaml::Mapping,
    field: &str,
    file: CockpitFile,
) -> Result<(), ValidationError> {
    match root.get(field) {
        Some(value) => expect_mapping(value, field, file),
        None => Ok(()),
    }
}

/// Reject a value that is not a mapping, naming the field and the file.
fn expect_mapping(
    value: &serde_yaml::Value,
    field: &str,
    file: CockpitFile,
) -> Result<(), ValidationError> {
    if value.is_mapping() {
        return Ok(());
    }
    Err(ValidationError::Schema {
        file: file.name(),
        detail: format!("`{field}` must be a mapping"),
    })
}

/// Serialize a model to YAML and re-parse it to confirm the output still validates.
fn serialize_and_recheck<T, F, R>(
    value: &T,
    file: CockpitFile,
    recheck: F,
) -> Result<String, ValidationError>
where
    T: serde::Serialize,
    F: Fn(&str) -> Result<R, ValidationError>,
{
    let yaml = serde_yaml::to_string(value).map_err(|error| ValidationError::Schema {
        file: file.name(),
        detail: error.to_string(),
    })?;
    recheck(&yaml)?;
    Ok(yaml)
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `validate`.
#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
