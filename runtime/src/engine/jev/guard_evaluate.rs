//! Shared pre-write guard evaluation over one proposed payload and bounded disk context.
//!
//! Unsafe target paths are input errors; protected paths and lexical denylist matches
//! block before any provider call. Model findings and failures are visible but advisory.
//! Only safely opened, filtered context and the filtered proposal reach the client.

use std::path::PathBuf;

use super::super::JevClient;
use super::super::config::JevConfig;
use super::super::drift::{DriftOutcome, evaluate_drift};
use super::super::rigor::{RigorReport, evaluate_rigor};
use super::super::secret_filter::{
    build_drift_scope_state, build_guard_rigor_state, open_state_files,
};
use super::combine::{combine, rigor_advice};
use super::{GuardDecision, ProposedChange, deterministic_layer};
use crate::engine::cockpit::read_active_task;

/// The note a flag-off evaluation returns (R4.1).
const NOTE_FLAG_OFF: &str = "probabilistic layer did not run: the jev feature is off";

/// The note an absent-client evaluation returns (R4.1).
const NOTE_NO_CLIENT: &str = "probabilistic layer did not run: no jev client is configured";

/// The note an absent-key evaluation returns (R4.2).
const NOTE_NO_KEY: &str = "probabilistic layer did not run: no jev API key is set";

/// Actual aspect outcomes, or the reason evaluation was unavailable. The production
/// decision remains advisory for every model result.
#[derive(Debug, Clone, Default)]
pub struct GuardObservation {
    pub rigor: Option<RigorReport>,
    pub drift: Option<DriftOutcome>,
    pub unavailable: Option<String>,
    pub unavailable_context: Vec<String>,
}

/// Evaluate a proposed change without exposing raw aspect observations.
///
/// Paths are validated and existing targets opened before reading any content.
/// Missing proposed files and missing parents are allowed without filesystem writes.
/// The proposed body is one logical payload, not a replacement for every target.
/// Deterministic blocks take precedence; Jev failures fail open with visible notes.
/// `api_key_present` is supplied by the caller to avoid process-global reads.
pub async fn evaluate_guard<C: JevClient>(
    change: &ProposedChange,
    protected_paths: &[String],
    config: &JevConfig,
    jev_enabled: bool,
    api_key_present: bool,
    client: Option<&C>,
    repo_root: &std::path::Path,
) -> Result<GuardDecision, super::super::JevError> {
    evaluate_guard_observed(
        change,
        protected_paths,
        config,
        jev_enabled,
        api_key_present,
        client,
        repo_root,
    )
    .await
    .map(|(decision, _)| decision)
}

/// Evaluate through the production guard while retaining raw model observations for
/// qualification. A blocked deterministic change never calls the provider.
pub async fn evaluate_guard_observed<C: JevClient>(
    change: &ProposedChange,
    protected_paths: &[String],
    config: &JevConfig,
    jev_enabled: bool,
    api_key_present: bool,
    client: Option<&C>,
    repo_root: &std::path::Path,
) -> Result<(GuardDecision, GuardObservation), super::super::JevError> {
    let mut observation = GuardObservation::default();
    if change.paths.is_empty() {
        return Err(super::super::JevError::InvalidGuardPath {
            path: "paths".to_string(),
            detail: "at least one repository-relative path is required".to_string(),
        });
    }
    let paths: Vec<PathBuf> = change.paths.iter().map(PathBuf::from).collect();
    let mut files = open_state_files(repo_root, &paths)?;
    if let Some(decision) = deterministic_layer(change, protected_paths) {
        return Ok((decision, observation));
    }

    if !jev_enabled {
        observation.unavailable = Some(NOTE_FLAG_OFF.to_string());
        return Ok((allow_with_note(NOTE_FLAG_OFF), observation));
    }
    let Some(client) = client else {
        observation.unavailable = Some(NOTE_NO_CLIENT.to_string());
        return Ok((allow_with_note(NOTE_NO_CLIENT), observation));
    };
    if !api_key_present {
        observation.unavailable = Some(NOTE_NO_KEY.to_string());
        return Ok((allow_with_note(NOTE_NO_KEY), observation));
    }

    let (rigor_state, unavailable_context) =
        match build_guard_rigor_state(repo_root, &paths, &mut files, &change.content) {
            Ok(state) => state,
            Err(cause @ super::super::JevError::InvalidGuardPath { .. }) => return Err(cause),
            Err(cause) => {
                let note = format!("probabilistic layer did not run: {cause}");
                observation.unavailable = Some(note.clone());
                return Ok((allow_with_note(&note), observation));
            }
        };
    observation.unavailable_context = unavailable_context;
    let rigor = match evaluate_rigor(client, rigor_state).await {
        Ok(report) => report,
        Err(cause) => {
            let note = format!("rigor scoring did not complete: {cause}");
            observation.unavailable = Some(note.clone());
            let mut decision = allow_with_note(&note);
            if let GuardDecision::Allow { notes } = &mut decision {
                notes.extend(observation.unavailable_context.iter().cloned());
            }
            return Ok((decision, observation));
        }
    };
    observation.rigor = Some(rigor);

    // The caller task wins over cockpit fallback. Drift remains a bounded
    // path/protected/task scope question with no proposal or disk file content.
    let task = resolve_scope_task(change, repo_root);
    if task.is_none() {
        observation
            .unavailable_context
            .push("scope task unavailable: judging paths against protected paths only".to_string());
    }
    let drift_state = match build_drift_scope_state(&paths, protected_paths, task.as_deref()) {
        Ok(state) => state,
        Err(cause) => {
            let note = format!("probabilistic layer did not run: {cause}");
            observation.unavailable = Some(note.clone());
            let decision =
                partial_with_note(&rigor, config, &note, &observation.unavailable_context);
            return Ok((decision, observation));
        }
    };
    let drift = match evaluate_drift(
        client,
        drift_state,
        &change.paths,
        protected_paths,
        config.drift_boundary,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(cause) => {
            let note = format!("drift scoring did not complete: {cause}");
            observation.unavailable = Some(note.clone());
            let decision =
                partial_with_note(&rigor, config, &note, &observation.unavailable_context);
            return Ok((decision, observation));
        }
    };
    observation.drift = Some(drift);

    let mut decision = combine(&rigor, &drift, None, config);
    if let GuardDecision::Allow { notes } | GuardDecision::Annotate { notes } = &mut decision {
        notes.extend(observation.unavailable_context.iter().cloned());
    }
    Ok((decision, observation))
}

/// Build an `Allow` decision carrying one advisory note.
fn allow_with_note(note: &str) -> GuardDecision {
    GuardDecision::Allow {
        notes: vec![note.to_string()],
    }
}

fn partial_with_note(
    rigor: &RigorReport,
    config: &JevConfig,
    note: &str,
    context: &[String],
) -> GuardDecision {
    let mut notes = rigor_advice(rigor, None, config);
    let finding = !notes.is_empty();
    notes.push(note.to_string());
    notes.extend(context.iter().cloned());
    if finding {
        GuardDecision::Annotate { notes }
    } else {
        GuardDecision::Allow { notes }
    }
}

/// Resolve the scope task for the drift judgment (#331).
///
/// The order is: the caller-supplied task on the change, then the cockpit active task, then
/// none. A non-empty caller task wins outright. When the caller omits the task, the cockpit
/// `active_task` fills in when present. A cockpit read error or an empty value resolves to
/// `None`, so a missing cockpit never fails the evaluation; the drift guard degrades to a
/// protected-path-only scope judgment.
fn resolve_scope_task(change: &ProposedChange, repo_root: &std::path::Path) -> Option<String> {
    if let Some(task) = change
        .task
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return Some(task.to_string());
    }
    read_active_task(repo_root).ok().flatten()
}

// Example and property tests live in sibling files to hold this module under the size
// guidance. The `#[path]` include keeps them child modules of `evaluate`.
#[cfg(test)]
#[path = "guard_evaluate_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "guard_evaluate_prop_tests.rs"]
mod prop_tests;
