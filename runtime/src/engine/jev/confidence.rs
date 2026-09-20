//! The confidence-band classifier: a pure, stateless mapping from a confidence value
//! and a threshold pair to one of three bands.
//!
//! A Jev Choice or Score answer carries a confidence from 0 to 1. The harness maps that
//! confidence to a band so a caller can act on the model's certainty. A confidence at or
//! above the high threshold is `High`. A confidence at or above the low threshold, but
//! below the high threshold, is `Medium`. A confidence below the low threshold is `Low`.
//!
//! The function is pure. It reads no config, no clock, and no disk, so the same arguments
//! always return the same band (Property 24). The thresholds arrive as arguments, resolved
//! once from [`super::config::JevConfig`] by the caller.
//!
//! Requirements: 8.6, 8.7. Design: jev-integration-eval, the confidence-band aspect,
//! Property 23 (totality), Property 24 (determinism).

// The classifier has no non-test caller until the confidence aspect module lands (a later
// issue). It carries a narrow non-test `allow` with this reason, per the repo dead-code
// policy (main.rs). The task that adds the first caller removes the attribute. The test
// build exercises every band and every error path through the sibling property tests.

/// One confidence band (Requirement 8.6).
///
/// The three bands are ordered by certainty: `High` is the most certain, `Low` the least.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfidenceBand {
    /// The confidence is at or above the high threshold.
    High,
    /// The confidence is at or above the low threshold, but below the high threshold.
    Medium,
    /// The confidence is below the low threshold.
    Low,
}

/// Classify a confidence value into a band against a threshold pair (Requirement 8.6).
///
/// The `low` and `high` thresholds bound the three bands. A `confidence` at or above
/// `high` is [`ConfidenceBand::High`]. A `confidence` at or above `low`, but below `high`,
/// is [`ConfidenceBand::Medium`]. A `confidence` below `low` is [`ConfidenceBand::Low`].
///
/// The function is pure and total over a valid threshold pair: every finite confidence
/// maps to exactly one band (Property 23). The same arguments always return the same
/// result (Property 24).
///
/// # Errors
///
/// Returns [`super::JevError::InvalidThresholds`] when `low` or `high` is NaN, when either
/// is outside 0 to 1, or when `low` is more than `high` (Requirement 8.7). The message
/// names both thresholds and the expected shape.
#[cfg_attr(not(test), allow(dead_code))]
pub fn confidence_band(
    confidence: f64,
    low: f64,
    high: f64,
) -> Result<ConfidenceBand, super::JevError> {
    if low.is_nan()
        || high.is_nan()
        || !(0.0..=1.0).contains(&low)
        || !(0.0..=1.0).contains(&high)
        || low > high
    {
        return Err(super::JevError::InvalidThresholds { low, high });
    }

    if confidence >= high {
        return Ok(ConfidenceBand::High);
    }
    if confidence >= low {
        return Ok(ConfidenceBand::Medium);
    }
    Ok(ConfidenceBand::Low)
}

// Example and property tests live in sibling files to hold this module under the size
// guidance. The `#[path]` include keeps them child modules of `confidence`.
#[cfg(test)]
#[path = "confidence_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "confidence_prop_tests.rs"]
mod prop_tests;
