//! The guard CLI mapping: turn a guard decision into an exit code and a stderr message.
//!
//! The `guard` subcommand of the `jev-bench` bin runs the guardrail over a proposed change on
//! standard input and reports the decision through an exit code. This module holds the pure
//! mapping from a [`GuardDecision`] to an exit code and to the stderr block reason. The bin
//! reads standard input, resolves the inputs, builds the client, calls `evaluate_guard`, then
//! uses these mappers.
//!
//! The mapping is pure and always compiled, not behind `jev-http`, so its property test runs
//! offline (P41). The bin that builds the HTTP client is behind `jev-http` (ADR-G4), but the
//! decision-to-exit-code mapping needs no client and no feature.
//!
//! The stderr message names the violated check and the remediation, never a secret value or
//! the API key (R6.3, R7.6, P40). The neutralization packet already excludes secrets, so the
//! message reuses its fields.
//!
//! Requirements: 7.2, 7.3, 7.6. Design: jev-active-guardrail, the guard CLI subcommand, P41.

use super::guard::GuardDecision;

/// The exit code for a guard decision that allows the write.
pub const EXIT_ALLOW: u8 = 0;

/// The exit code for a guard decision that blocks the write.
pub const EXIT_BLOCK: u8 = 1;

/// Map a guard decision to a process exit code (R7.2, R7.3, P41).
///
/// A `Block` returns [`EXIT_BLOCK`], a non-zero code, so a `PreToolUse` hook stops the write.
/// An `Allow` and an `Annotate` return [`EXIT_ALLOW`], zero, so the write proceeds. The
/// mapping is total and pure, so the property test drives it with no client and no runtime.
pub fn exit_code_for(decision: &GuardDecision) -> u8 {
    match decision {
        GuardDecision::Block(_) => EXIT_BLOCK,
        GuardDecision::Allow { .. } | GuardDecision::Annotate { .. } => EXIT_ALLOW,
    }
}

/// Build the stderr message for a guard decision, or `None` when nothing is printed (R7.2).
///
/// A `Block` returns a message naming the violated check and the remediation, and the
/// suggested fix when one is present. An `Allow` and an `Annotate` return `None`, so the CLI
/// prints nothing on a pass and the write proceeds silently. The message names no secret
/// value and no API key, because it reuses the neutralization packet fields, which already
/// exclude both (R6.3, R7.6, P40).
pub fn block_message(decision: &GuardDecision) -> Option<String> {
    match decision {
        GuardDecision::Block(packet) => {
            let mut message = format!(
                "guard blocked the change: {} ({})",
                packet.remediation, packet.violated_check
            );
            if let Some(value) = &packet.offending_value {
                message.push_str(&format!(" [offending: {value}]"));
            }
            if let Some(fix) = &packet.suggested_fix {
                message.push_str(&format!(" [suggested fix: {fix}]"));
            }
            Some(message)
        }
        GuardDecision::Allow { .. } | GuardDecision::Annotate { .. } => None,
    }
}

// Property tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `guard_cli`.
#[cfg(test)]
#[path = "guard_cli_prop_tests.rs"]
mod prop_tests;

#[cfg(test)]
#[path = "guard_cli_tests.rs"]
mod tests;
