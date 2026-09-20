//! The active guardrail: a pre-write check that returns one decision (jev-active-guardrail).
//!
//! The guardrail wires the Jev evaluation harness into the live request path. It runs a
//! pre-write check over a proposed change and returns one [`GuardDecision`]: allow, block,
//! or annotate. It composes the existing harness modules whole and adds no new Jev client,
//! wire type, or aspect (ADR-G1).
//!
//! The guardrail runs two layers in a fixed order. This module lands the first layer, the
//! deterministic layer. The deterministic layer runs first, with no Jev call, regardless of
//! the flag. It blocks a write to a protected path and a write whose content matches the
//! secret denylist. It never fails open (R2, P34, P35). The probabilistic layer, the
//! `evaluate_guard` entry point, the tool, the CLI subcommand, and the emitted hook land in
//! later tasks (issues #294 onward).
//!
//! The deterministic layer is genuinely deterministic: it is a literal match in code, not a
//! model judgment. A protected-path hit and a secret hit each block with no client call, so
//! a Jev outage cannot weaken the hard rule (ADR-G3).
//!
//! The types and the layer have no non-test caller until the combine step and the
//! `evaluate_guard` entry point land (issue #294) and the tool lands (issue #296). Each
//! staged public item carries a narrow non-test `allow` with this reason, per the repo
//! dead-code policy (main.rs). The task that adds each item's first caller removes its
//! attribute. The test build exercises every item through the sibling `guard_tests.rs` and
//! `guard_prop_tests.rs`.
//!
//! Requirements: 1.2, 1.4, 2.1, 2.2, 2.3, 2.5, 2.6, 6.1, 6.3, 6.4.
//! Design: jev-active-guardrail, ADR-G1, ADR-G3, the deterministic layer, P34, P35.

use crate::config::secret_denylist;

use super::drift::path_is_protected;

/// The violated-check name a protected-path block reports (R6.4).
const CHECK_PROTECTED_PATH: &str = "protected-path";

/// The violated-check name a secret block reports (R6.4).
const CHECK_SECRET: &str = "secret";

/// The remediation hint a protected-path block returns (R6.1).
const REMEDIATION_PROTECTED_PATH: &str =
    "Do not write a protected path. Route the change to a path outside the protected set.";

/// The remediation hint a secret block returns (R6.1).
const REMEDIATION_SECRET: &str = "Remove the secret before the write. Move the value to an environment variable or a \
     secret store, then reference it by name.";

/// One proposed change the guard evaluates (R1.2).
///
/// The change names the target paths it writes and the new content it writes. The guard
/// reads it and decides; it performs no write itself.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct ProposedChange {
    /// The target paths the change writes.
    pub paths: Vec<String>,
    /// The new content the change writes.
    pub content: String,
}

/// The guard decision (R1.4).
///
/// One evaluation returns exactly one decision. An `Allow` and an `Annotate` both let the
/// change proceed and carry advisory notes; a `Block` stops the change and carries the
/// structured [`NeutralizationPacket`].
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub enum GuardDecision {
    /// The change may proceed. Carries advisory notes.
    Allow {
        /// The advisory notes attached to the decision.
        notes: Vec<String>,
    },
    /// The change is blocked. Carries the neutralization packet.
    Block(NeutralizationPacket),
    /// The change may proceed, but the guard attaches advisory findings.
    Annotate {
        /// The advisory findings attached to the decision.
        notes: Vec<String>,
    },
}

/// The structured block reason (R6).
///
/// A `Block` returns this packet. It names the violated check, the offending value for a
/// deterministic block, a remediation hint the agent can act on, and an optional suggested
/// self-heal instruction. It carries no secret content and no API key: a secret block names
/// the matched pattern name, never the secret value (R6.3, P40).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct NeutralizationPacket {
    /// The violated check, for example `protected-path` or `secret` or `drift`.
    pub violated_check: String,
    /// The offending value for a deterministic block, such as the protected path or the
    /// matched pattern name. Never a secret value.
    pub offending_value: Option<String>,
    /// A remediation hint the agent can act on.
    pub remediation: String,
    /// A suggested self-heal instruction, present when a rigor failure warranted one.
    pub suggested_fix: Option<String>,
}

impl NeutralizationPacket {
    /// Build a deterministic-block packet naming the violated check and the offending value.
    ///
    /// A deterministic block carries no suggested fix; the self-heal instruction attaches
    /// only to a rigor block in the probabilistic layer (issue #294).
    fn deterministic(violated_check: &str, offending_value: String, remediation: &str) -> Self {
        Self {
            violated_check: violated_check.to_string(),
            offending_value: Some(offending_value),
            remediation: remediation.to_string(),
            suggested_fix: None,
        }
    }
}

/// Run the deterministic layer over a proposed change (R2, P34, P35).
///
/// The layer runs two model-free checks in order and returns the first block it finds. It
/// makes no Jev call and runs regardless of the flag (R2.3). It never fails open (ADR-G3).
///
/// First, the protected-path match: when any written path is protected, the layer returns a
/// `protected-path` block naming the path, with no Jev call (R2.1, R2.6). The match reuses
/// [`path_is_protected`], the same literal match the drift aspect runs (R2.1).
///
/// Then, the secret scan: when the content matches the secret denylist, the layer returns a
/// `secret` block naming the matched pattern marker, with no Jev call (R2.2, R6.3). It names
/// the marker, never the secret value (R6.3, P40).
///
/// When neither check hits, the layer returns `None`, so the caller runs the probabilistic
/// layer next (issue #294). The layer returns the same result on the same input (R2.5).
#[cfg_attr(not(test), allow(dead_code))]
pub fn deterministic_layer(
    change: &ProposedChange,
    protected_paths: &[String],
) -> Option<GuardDecision> {
    // The protected-path match. A written path that is protected blocks with no Jev call.
    for path in &change.paths {
        if path_is_protected(path, protected_paths) {
            let packet = NeutralizationPacket::deterministic(
                CHECK_PROTECTED_PATH,
                path.clone(),
                REMEDIATION_PROTECTED_PATH,
            );
            return Some(GuardDecision::Block(packet));
        }
    }

    // The secret scan. Content that matches the denylist blocks with no Jev call. The
    // packet names the matched pattern marker, never the secret value (R6.3, P40).
    if let Some(marker) = secret_marker(&change.content) {
        let packet = NeutralizationPacket::deterministic(
            CHECK_SECRET,
            marker.to_string(),
            REMEDIATION_SECRET,
        );
        return Some(GuardDecision::Block(packet));
    }

    None
}

/// Report the marker name of the first secret-denylist pattern the content matches (R6.3).
///
/// The scan runs the content against every compiled denylist regex from
/// [`secret_denylist`]. On a match, it returns a short, fixed marker name for the matched
/// pattern, never a slice of the content, so no secret value leaves the guard (R6.3, P40).
/// The markers are stable identifiers, so a caller and a test can name the matched pattern
/// without echoing the offending text.
///
/// The `secret` and `credentials` patterns match anywhere in the content. The env-file and
/// pem-file patterns are path-anchored (they end in `$`), so they match a `.env` or `.pem`
/// path form that ends the content, not the marker embedded mid-sentence. This reuses the
/// crate denylist whole rather than a second content-only pattern set (ADR-G1).
fn secret_marker(content: &str) -> Option<&'static str> {
    secret_denylist()
        .iter()
        .position(|pattern| pattern.is_match(content))
        .map(marker_for_index)
}

/// Map a denylist pattern index to its stable marker name (R6.3).
///
/// The order matches [`secret_denylist`]: an environment file, a PEM file, a `secret`
/// marker, and a `credentials` marker. An out-of-range index maps to a generic marker, so
/// the function is total and names no secret value. The generic arm also keeps the function
/// correct when the denylist grows.
fn marker_for_index(index: usize) -> &'static str {
    match index {
        0 => "env-file",
        1 => "pem-file",
        2 => "secret-marker",
        3 => "credentials-marker",
        _ => "secret-denylist",
    }
}

// Example and property tests live in sibling files to hold this module under the size
// guidance. The `#[path]` include keeps them child modules of `guard`.
#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "guard_prop_tests.rs"]
mod prop_tests;
