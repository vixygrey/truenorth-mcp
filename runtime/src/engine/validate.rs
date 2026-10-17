//! Validation for the JSON cockpit documents and legacy phase mapping.
//!
//! This module validates `state.json` and `release-plan.json` against the observed
//! shapes. Reads reject invalid data without mutation, and write validators serialize
//! through the duplicate-key-rejecting ordered cockpit JSON codec before disk writes.
//!
//! The module also maps legacy bigpowers phase names onto the six-phase model
//! (Requirement 9.5) and rejects an unrecognized legacy name (Requirement 9.6).
//!
//! Requirements: 9.1, 9.2, 9.5, 9.6. Design: Part II §8.

use serde_json::{Map, Value};
use thiserror::Error;

use crate::engine::cockpit_json;
use crate::engine::spec::{Phase, ReleasePlanFile, StateFile};

/// The cockpit file a validation error refers to. The value names the file in the
/// error message, so an operator can locate it (Requirement 9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CockpitFile {
    /// `state.json`.
    State,
    /// `release-plan.json`.
    ReleasePlan,
}

impl CockpitFile {
    /// The file name for the error message.
    fn name(self) -> &'static str {
        match self {
            CockpitFile::State => "state.json",
            CockpitFile::ReleasePlan => "release-plan.json",
        }
    }
}

/// A validation or phase-mapping error.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// A cockpit file failed to parse against its observed schema (Requirement 9.2).
    #[error(
        "{file} failed schema validation: {detail}. The file was left unchanged. Fix the JSON and retry."
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

/// Validate and parse `state.json` content (Requirements 9.1, 9.2).
pub fn validate_state(json: &str) -> Result<StateFile, ValidationError> {
    let state = StateFile {
        root: parse(json, CockpitFile::State)?,
    };
    check_mapping_field(&state.root, "git", CockpitFile::State)?;
    check_mapping_field(&state.root, "handoff", CockpitFile::State)?;
    if let Some(metrics) = state.root.get("metrics") {
        let metrics = expect_mapping(metrics, "metrics", CockpitFile::State)?;
        check_mapping_field(metrics, "skill_timings", CockpitFile::State)?;
    }
    Ok(state)
}

/// Validate and parse `release-plan.json` content (Requirements 9.1, 9.2).
pub fn validate_release_plan(json: &str) -> Result<ReleasePlanFile, ValidationError> {
    let plan = ReleasePlanFile {
        root: parse(json, CockpitFile::ReleasePlan)?,
    };
    if let Some(build_order) = plan.root.get("build_order")
        && !build_order.is_array()
    {
        return Err(ValidationError::Schema {
            file: CockpitFile::ReleasePlan.name(),
            detail: "`build_order` must be an array of epic ids".to_string(),
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

/// Parse cockpit JSON into an ordered object, wrapping failures in a schema error.
fn parse(json: &str, file: CockpitFile) -> Result<cockpit_json::CockpitObject, ValidationError> {
    cockpit_json::parse_object(json).map_err(|error| ValidationError::Schema {
        file: file.name(),
        detail: error.to_string(),
    })
}

/// Reject a present field that is not an object. An absent field passes.
fn check_mapping_field(
    root: &Map<String, Value>,
    field: &str,
    file: CockpitFile,
) -> Result<(), ValidationError> {
    match root.get(field) {
        Some(value) => expect_mapping(value, field, file).map(|_| ()),
        None => Ok(()),
    }
}

/// Reject a value that is not an object, naming the field and the file.
fn expect_mapping<'a>(
    value: &'a Value,
    field: &str,
    file: CockpitFile,
) -> Result<&'a Map<String, Value>, ValidationError> {
    value.as_object().ok_or_else(|| ValidationError::Schema {
        file: file.name(),
        detail: format!("`{field}` must be an object"),
    })
}

/// Serialize a model through the ordered JSON codec and re-parse it.
fn serialize_and_recheck<T, F, R>(
    value: &T,
    file: CockpitFile,
    recheck: F,
) -> Result<String, ValidationError>
where
    T: serde::Serialize,
    F: Fn(&str) -> Result<R, ValidationError>,
{
    let object = serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .ok_or_else(|| ValidationError::Schema {
            file: file.name(),
            detail: "cockpit model did not serialize as an object".to_string(),
        })?;
    let json = cockpit_json::to_string(&object).map_err(|error| ValidationError::Schema {
        file: file.name(),
        detail: error.to_string(),
    })?;
    recheck(&json)?;
    Ok(json)
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `validate`.
#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
