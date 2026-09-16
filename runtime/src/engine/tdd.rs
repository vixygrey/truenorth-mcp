//! The TDD cycle step model and the Red-Green-Refactor transition rule.
//!
//! `TddStep` is the recorded cycle step. `next_step` enforces the strict cyclic order:
//! the only valid step after the current one is the next in Red then Green then Refactor,
//! and Refactor returns to Red for the next cycle (Requirement 2.7). An out-of-order step
//! is rejected, so the caller leaves the recorded state unchanged (Requirement 2.8).
//!
//! The logic is pure, so it is unit-testable without disk. The cockpit module persists
//! the step in `state.yaml`.
//!
//! Requirements: 2.7, 2.8. Design: Part II §2.

use thiserror::Error;

/// A TDD cycle step (design §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TddStep {
    /// Write a failing test.
    Red,
    /// Write the minimal code to pass.
    Green,
    /// Clean up while green.
    Refactor,
}

/// An invalid TDD step transition (Requirement 2.8).
#[derive(Debug, Error, PartialEq, Eq)]
#[error(
    "invalid TDD transition: {requested} does not follow {current}. \
     The order is red, then green, then refactor, then red for the next cycle. \
     The recorded step is unchanged."
)]
pub struct InvalidTransition {
    /// The current recorded step, or `none` when no cycle has started.
    pub current: String,
    /// The requested step.
    pub requested: String,
}

impl TddStep {
    /// Parse a step name.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "red" => Some(TddStep::Red),
            "green" => Some(TddStep::Green),
            "refactor" => Some(TddStep::Refactor),
            _ => None,
        }
    }

    /// The step name in lowercase.
    pub fn as_str(self) -> &'static str {
        match self {
            TddStep::Red => "red",
            TddStep::Green => "green",
            TddStep::Refactor => "refactor",
        }
    }
}

/// Validate a requested step against the current recorded step (Requirements 2.7, 2.8).
///
/// The valid next step is the one that follows the current in the strict cycle: no
/// current or Refactor allows Red, Red allows Green, Green allows Refactor. Any other
/// request is an invalid transition.
///
/// # Errors
///
/// Returns [`InvalidTransition`] when the requested step does not follow the current one.
pub fn next_step(
    current: Option<TddStep>,
    requested: TddStep,
) -> Result<TddStep, InvalidTransition> {
    let allowed = match current {
        None | Some(TddStep::Refactor) => TddStep::Red,
        Some(TddStep::Red) => TddStep::Green,
        Some(TddStep::Green) => TddStep::Refactor,
    };
    if requested == allowed {
        Ok(requested)
    } else {
        Err(InvalidTransition {
            current: current.map(TddStep::as_str).unwrap_or("none").to_string(),
            requested: requested.as_str().to_string(),
        })
    }
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `tdd`.
#[cfg(test)]
#[path = "tdd_tests.rs"]
mod tests;
