//! The Jev harness configuration, as data, and its resolution.
//!
//! The config carries the confidence thresholds, the aspect decision boundaries, the retry
//! and backoff limits, the call timeout, and an optional token price. A caller resolves it
//! once from `.agent/config/rules.yml` under a `jev` block. The default holds sensible
//! values, so a project with no config, no `config/` directory, and no `.agent/` directory
//! all resolve to the same default.
//!
//! The reader mirrors the feature-flag reader (`engine::features`). An absent file resolves
//! to the default. A present file that cannot be read or parsed returns a typed
//! [`super::JevError::Config`] error with no partial value. The reader deserializes only the
//! `jev` block through a private view struct, so an unrelated `rules.yml` key does not break
//! the parse.
//!
//! `serde_yaml` cannot read a `std::time::Duration` from a plain integer, so the private
//! view reads the two durations as millisecond counts (`max_backoff_ms`, `timeout_ms`) and
//! the builder converts them. [`JevConfig`] itself holds `Duration`.
//!
//! Requirements: 1.6, 1.7, 8.7, 12.5. Design: jev-integration-eval, the config aspect,
//! ADR-J1.

// The config type and its resolver have no non-test caller until the client and aspect
// modules land (later issues). Each public item carries a narrow non-test `allow` with this
// reason, per the repo dead-code policy (main.rs). The task that adds the first caller
// removes the attribute. The test build exercises resolution and validation through the
// sibling `config_tests.rs`.

use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

use crate::engine::agent_ws::AGENT_DIR;

/// The default confidence high threshold.
const DEFAULT_CONFIDENCE_HIGH: f64 = 0.85;
/// The default confidence low threshold.
const DEFAULT_CONFIDENCE_LOW: f64 = 0.60;
/// The default drift boundary (#334).
///
/// A calibration run showed the drift signal, with a task in the state (#331), puts an
/// out-of-scope change in a 0.19 to 0.76 noul band while an in-scope change stays at or below
/// 0.19. A boundary of 0.25 catches most drift with no false positive on that set, and leaves
/// a small margin above the in-scope ceiling. A project can override this in `rules.yml`.
const DEFAULT_DRIFT_BOUNDARY: f64 = 0.25;
/// The default rigor-failure boundary.
const DEFAULT_RIGOR_FAILURE_BOUNDARY: f64 = 0.70;
/// The default complexity boundary, on a 0 to 100 scale.
const DEFAULT_COMPLEXITY_BOUNDARY: f64 = 70.0;
/// The default pruning keep threshold.
const DEFAULT_PRUNING_KEEP_THRESHOLD: f64 = 0.50;
/// The default destructive-action threshold.
const DEFAULT_DESTRUCTIVE_THRESHOLD: f64 = 0.90;
/// The default retry limit.
const DEFAULT_RETRY_LIMIT: u32 = 3;
/// The default maximum backoff, in milliseconds.
const DEFAULT_MAX_BACKOFF_MS: u64 = 8_000;
/// The default call timeout, in milliseconds.
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// The rules.yml block path, for a config error message.
const JEV_BLOCK_PATH: &str = ".agent/config/rules.yml (jev block)";

/// The resolved Jev harness configuration (Requirement 1.6).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
pub struct JevConfig {
    /// The confidence high threshold, in 0 to 1. A confidence at or above it is High.
    pub confidence_high: f64,
    /// The confidence low threshold, in 0 to 1. It must be no more than `confidence_high`.
    pub confidence_low: f64,
    /// The drift boundary, in 0 to 1.
    pub drift_boundary: f64,
    /// The rigor-failure boundary, in 0 to 1.
    pub rigor_failure_boundary: f64,
    /// The complexity boundary, on a 0 to 100 scale.
    pub complexity_boundary: f64,
    /// The pruning keep threshold, in 0 to 1.
    pub pruning_keep_threshold: f64,
    /// The destructive-action threshold, in 0 to 1. It must be at or above `confidence_high`.
    pub destructive_threshold: f64,
    /// The retry limit for a rate-limited or overloaded call.
    pub retry_limit: u32,
    /// The maximum backoff between retries.
    pub max_backoff: Duration,
    /// The per-call wall-clock timeout.
    pub timeout: Duration,
    /// The optional token price. Absent means the harness reports no cost.
    pub price: Option<JevPrice>,
}

/// A token price, in currency per million tokens (Requirement 12.1).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy)]
pub struct JevPrice {
    /// The input token rate, per million input tokens. It must be finite and 0 or more.
    pub input_per_million: f64,
    /// The output token rate, per million output tokens. It must be finite and 0 or more.
    pub output_per_million: f64,
}

impl Default for JevConfig {
    /// The default config: sensible thresholds, a retry limit of 3, an 8-second maximum
    /// backoff, a 30-second timeout, and no price.
    fn default() -> Self {
        Self {
            confidence_high: DEFAULT_CONFIDENCE_HIGH,
            confidence_low: DEFAULT_CONFIDENCE_LOW,
            drift_boundary: DEFAULT_DRIFT_BOUNDARY,
            rigor_failure_boundary: DEFAULT_RIGOR_FAILURE_BOUNDARY,
            complexity_boundary: DEFAULT_COMPLEXITY_BOUNDARY,
            pruning_keep_threshold: DEFAULT_PRUNING_KEEP_THRESHOLD,
            destructive_threshold: DEFAULT_DESTRUCTIVE_THRESHOLD,
            retry_limit: DEFAULT_RETRY_LIMIT,
            max_backoff: Duration::from_millis(DEFAULT_MAX_BACKOFF_MS),
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
            price: None,
        }
    }
}

impl JevConfig {
    /// Reject an invalid config (Requirement 8.7, 12.5).
    ///
    /// A boundary in 0 to 1 that is NaN or out of range, a complexity boundary outside 0 to
    /// 100, or an out-of-range plain boundary returns [`super::JevError::Config`]. A low
    /// threshold above the high threshold, or a destructive threshold below the high
    /// threshold, returns [`super::JevError::InvalidThresholds`]. A non-finite or negative
    /// price rate returns [`super::JevError::InvalidPrice`].
    ///
    /// # Errors
    ///
    /// Returns a typed [`super::JevError`] naming the offending field and the expected range.
    fn validate(&self) -> Result<(), super::JevError> {
        check_unit("confidence_high", self.confidence_high)?;
        check_unit("confidence_low", self.confidence_low)?;
        check_unit("drift_boundary", self.drift_boundary)?;
        check_unit("rigor_failure_boundary", self.rigor_failure_boundary)?;
        check_unit("pruning_keep_threshold", self.pruning_keep_threshold)?;
        check_unit("destructive_threshold", self.destructive_threshold)?;

        if self.complexity_boundary.is_nan() || !(0.0..=100.0).contains(&self.complexity_boundary) {
            return Err(config_range_error("complexity_boundary", "0 to 100"));
        }

        if self.confidence_low > self.confidence_high {
            return Err(super::JevError::InvalidThresholds {
                low: self.confidence_low,
                high: self.confidence_high,
            });
        }

        if self.destructive_threshold < self.confidence_high {
            return Err(super::JevError::InvalidThresholds {
                low: self.confidence_high,
                high: self.destructive_threshold,
            });
        }

        if let Some(price) = self.price {
            check_price(price.input_per_million)?;
            check_price(price.output_per_million)?;
        }

        Ok(())
    }
}

/// Reject a boundary that is NaN or outside 0 to 1.
fn check_unit(field: &str, value: f64) -> Result<(), super::JevError> {
    if value.is_nan() || !(0.0..=1.0).contains(&value) {
        return Err(config_range_error(field, "0 to 1"));
    }
    Ok(())
}

/// Reject a price rate that is not finite or is negative (Requirement 12.5).
fn check_price(value: f64) -> Result<(), super::JevError> {
    if !value.is_finite() || value < 0.0 {
        return Err(super::JevError::InvalidPrice { value });
    }
    Ok(())
}

/// Build a config range error naming the field and the expected range.
fn config_range_error(field: &str, range: &str) -> super::JevError {
    super::JevError::Config {
        path: JEV_BLOCK_PATH.to_string(),
        detail: format!("{field} must be in {range}"),
    }
}

/// The default protected paths, matching the documented set in `.agent/config/rules.yml`.
///
/// The guard blocks a write to any of these when `rules.yml` names no `protected_paths` block
/// (jev-active-guardrail glossary, Protected_Path).
const DEFAULT_PROTECTED_PATHS: [&str; 4] =
    ["specs/", "specs/adr/", "LICENSE", ".github/workflows/"];

/// The subset of `.agent/config/rules.yml` this reader needs.
///
/// Only the `jev` block is deserialized. Every other key is ignored on read, so an
/// unrelated `rules.yml` key does not break the parse and is left untouched, because the
/// reader never writes.
#[derive(Debug, Default, Deserialize)]
struct RulesJevView {
    #[serde(default)]
    jev: JevBlock,
}

/// The subset of `.agent/config/rules.yml` the protected-paths reader needs.
///
/// Only the `protected_paths` list is deserialized. An absent block resolves to the default
/// set through [`resolve_protected_paths`], so the guard always has a protected list.
#[derive(Debug, Default, Deserialize)]
struct RulesProtectedView {
    #[serde(default)]
    protected_paths: Option<Vec<String>>,
}

/// The `jev` block, as read from disk. The two durations arrive as millisecond counts,
/// because `serde_yaml` cannot read a `Duration` from a plain integer.
#[derive(Debug, Deserialize)]
#[serde(default)]
struct JevBlock {
    confidence_high: f64,
    confidence_low: f64,
    drift_boundary: f64,
    rigor_failure_boundary: f64,
    complexity_boundary: f64,
    pruning_keep_threshold: f64,
    destructive_threshold: f64,
    retry_limit: u32,
    max_backoff_ms: u64,
    timeout_ms: u64,
    price: Option<JevPriceBlock>,
}

impl Default for JevBlock {
    /// An absent `jev` block resolves to the same values as [`JevConfig::default`].
    fn default() -> Self {
        Self {
            confidence_high: DEFAULT_CONFIDENCE_HIGH,
            confidence_low: DEFAULT_CONFIDENCE_LOW,
            drift_boundary: DEFAULT_DRIFT_BOUNDARY,
            rigor_failure_boundary: DEFAULT_RIGOR_FAILURE_BOUNDARY,
            complexity_boundary: DEFAULT_COMPLEXITY_BOUNDARY,
            pruning_keep_threshold: DEFAULT_PRUNING_KEEP_THRESHOLD,
            destructive_threshold: DEFAULT_DESTRUCTIVE_THRESHOLD,
            retry_limit: DEFAULT_RETRY_LIMIT,
            max_backoff_ms: DEFAULT_MAX_BACKOFF_MS,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            price: None,
        }
    }
}

/// The `price` sub-block, as read from disk.
#[derive(Debug, Clone, Copy, Deserialize)]
struct JevPriceBlock {
    input_per_million: f64,
    output_per_million: f64,
}

impl JevBlock {
    /// Convert the on-disk block into a [`JevConfig`], mapping the millisecond durations.
    fn into_config(self) -> JevConfig {
        JevConfig {
            confidence_high: self.confidence_high,
            confidence_low: self.confidence_low,
            drift_boundary: self.drift_boundary,
            rigor_failure_boundary: self.rigor_failure_boundary,
            complexity_boundary: self.complexity_boundary,
            pruning_keep_threshold: self.pruning_keep_threshold,
            destructive_threshold: self.destructive_threshold,
            retry_limit: self.retry_limit,
            max_backoff: Duration::from_millis(self.max_backoff_ms),
            timeout: Duration::from_millis(self.timeout_ms),
            price: self.price.map(|p| JevPrice {
                input_per_million: p.input_per_million,
                output_per_million: p.output_per_million,
            }),
        }
    }
}

/// Resolve the Jev config from `.agent/config/rules.yml` (Requirement 1.6, 1.7).
///
/// An absent `.agent/`, an absent `.agent/config/`, or an absent `rules.yml` all resolve to
/// [`JevConfig::default`]. A present file with no `jev` block resolves to the default. A
/// present file that cannot be read or parsed returns [`super::JevError::Config`] with no
/// partial value. The resolved config is validated before it returns.
///
/// # Errors
///
/// Returns [`super::JevError::Config`] when a present config cannot be read or parsed, and a
/// typed validation error ([`super::JevError::InvalidThresholds`],
/// [`super::JevError::InvalidPrice`], or [`super::JevError::Config`]) when the values are
/// out of range.
pub fn resolve(repo_root: &Path) -> Result<JevConfig, super::JevError> {
    let config_path = repo_root.join(AGENT_DIR).join("config").join("rules.yml");

    let text = match std::fs::read_to_string(&config_path) {
        Ok(text) => text,
        // An absent `.agent/`, `config/`, or `rules.yml` all surface as NotFound and resolve
        // to the default.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(JevConfig::default());
        }
        Err(source) => {
            return Err(super::JevError::Config {
                path: config_path.display().to_string(),
                detail: source.to_string(),
            });
        }
    };

    let view: RulesJevView =
        serde_yaml::from_str(&text).map_err(|source| super::JevError::Config {
            path: config_path.display().to_string(),
            detail: source.to_string(),
        })?;

    let config = view.jev.into_config();
    config.validate()?;
    Ok(config)
}

/// Resolve the protected paths from `.agent/config/rules.yml` (jev-active-guardrail R2.1).
///
/// The reader reads the `protected_paths` list. An absent `.agent/`, `config/`, or `rules.yml`,
/// a present file with no `protected_paths` block, or an empty list all resolve to the default
/// set [`DEFAULT_PROTECTED_PATHS`], so the guard always has a protected list. A present file
/// that cannot be read or parsed returns [`super::JevError::Config`] with no partial value.
///
/// The default set matches the documented list in `.agent/config/rules.yml`. This keeps the
/// guard safe when a project has not customized the block, rather than protecting nothing.
///
/// # Errors
///
/// Returns [`super::JevError::Config`] when a present config cannot be read or parsed.
pub fn resolve_protected_paths(repo_root: &Path) -> Result<Vec<String>, super::JevError> {
    let config_path = repo_root.join(AGENT_DIR).join("config").join("rules.yml");

    let text = match std::fs::read_to_string(&config_path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(default_protected_paths());
        }
        Err(source) => {
            return Err(super::JevError::Config {
                path: config_path.display().to_string(),
                detail: source.to_string(),
            });
        }
    };

    let view: RulesProtectedView =
        serde_yaml::from_str(&text).map_err(|source| super::JevError::Config {
            path: config_path.display().to_string(),
            detail: source.to_string(),
        })?;

    match view.protected_paths {
        // A named non-empty list wins. An empty list falls back to the default, so a project
        // does not accidentally protect nothing.
        Some(list) if !list.is_empty() => Ok(list),
        _ => Ok(default_protected_paths()),
    }
}

/// The default protected-path set as owned strings.
fn default_protected_paths() -> Vec<String> {
    DEFAULT_PROTECTED_PATHS
        .iter()
        .map(|s| s.to_string())
        .collect()
}

// Tests live in a sibling file to hold this module under the size guidance. The `#[path]`
// include keeps them a child module of `config`.
#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
