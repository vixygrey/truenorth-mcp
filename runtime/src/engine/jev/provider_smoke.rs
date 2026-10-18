//! Protected Jev provider contract checks. No provider payload or error text enters evidence.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;

use super::config::{JevConfig, JevPrice};
use super::{Answer, JEV_MODEL, JevClient, JevError, JevRequest, JevResponse, Question};

/// The policy fields that bind smoke evidence to benchmark acceptance provenance.
#[derive(Debug)]
pub struct SmokePolicy {
    pub schema_version: u32,
    pub approval: String,
    pub requested_model: String,
    pub concrete_model: String,
    pub pricing: SmokePricing,
}

#[derive(Debug)]
pub struct SmokePricing {
    pub version: String,
    pub source: String,
    pub currency: String,
    pub input_per_million: f64,
    pub output_per_million: f64,
}

impl From<super::bench::AcceptancePolicy> for SmokePolicy {
    fn from(policy: super::bench::AcceptancePolicy) -> Self {
        Self {
            schema_version: policy.schema_version,
            approval: policy.approval,
            requested_model: policy.requested_model,
            concrete_model: policy.concrete_model,
            pricing: SmokePricing {
                version: policy.pricing.version,
                source: policy.pricing.source,
                currency: policy.pricing.currency,
                input_per_million: policy.pricing.input_per_million,
                output_per_million: policy.pricing.output_per_million,
            },
        }
    }
}

impl SmokePolicy {
    /// Validate all external policy fields before constructing a network client.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != 1 || self.requested_model != JEV_MODEL {
            return Err("unsupported policy version or requested model");
        }
        let model = self
            .concrete_model
            .strip_prefix("jev-")
            .ok_or("invalid concrete model")?;
        if model.is_empty()
            || !model.bytes().all(|c| c.is_ascii_digit() || c == b'.')
            || model.split('.').count() != 3
            || model.split('.').any(str::is_empty)
        {
            return Err("invalid concrete model");
        }
        if !matches!(self.approval.as_str(), "approved" | "pending") {
            return Err("invalid approval state");
        }
        if self.pricing.version.is_empty()
            || self.pricing.version.len() > 80
            || !self
                .pricing
                .version
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            || self.pricing.source != "https://docs.typesafe.ai/models"
            || self.pricing.currency != "USD"
            || !self.pricing.input_per_million.is_finite()
            || self.pricing.input_per_million <= 0.0
            || !self.pricing.output_per_million.is_finite()
            || self.pricing.output_per_million < 0.0
        {
            return Err("invalid pricing provenance");
        }
        Ok(())
    }

    pub fn price(&self) -> JevPrice {
        JevPrice {
            input_per_million: self.pricing.input_per_million,
            output_per_million: self.pricing.output_per_million,
        }
    }
}

/// Do not forward bearer credentials to a controlled fault endpoint on another origin.
pub fn same_origin(approved: &str, controlled: &str) -> bool {
    match (
        reqwest::Url::parse(approved),
        reqwest::Url::parse(controlled),
    ) {
        (Ok(approved), Ok(controlled)) => {
            approved.origin() == controlled.origin()
                && controlled.username().is_empty()
                && controlled.password().is_none()
                && controlled.query().is_none()
                && controlled.fragment().is_none()
        }
        _ => false,
    }
}

/// Fixed small synthetic request for auth and controlled-fault probes only.
pub fn request() -> JevRequest {
    JevRequest::new(
        serde_json::json!("A documentation-only change."),
        BTreeMap::from([(
            "drift".to_string(),
            Question::noul("Is this out of scope?", None),
        )]),
    )
}

/// Deliberately omit provider details even if a fake or a future HTTP client leaks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Passed,
    Failed,
    NotProven,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Counters {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub estimated_cost_usd: f64,
}

#[derive(Clone, Copy)]
pub enum Aspect {
    Rigor,
    Drift,
}

/// Typed production-wire and accounting check, with redacted errors.
pub fn inspect(
    response: &JevResponse,
    policy: &SmokePolicy,
    aspect: Aspect,
) -> Result<Counters, &'static str> {
    if response.model != policy.concrete_model {
        return Err("unexpected concrete responder model");
    }
    if response.usage.input_tokens == 0 {
        return Err("missing token usage");
    }
    let noul_ids: &[&str] = match aspect {
        Aspect::Rigor => &[
            "hallucinated_import",
            "violates_conventions",
            "contains_secrets",
        ],
        Aspect::Drift => &["drift"],
    };
    if response.answers.len() != noul_ids.len() + usize::from(matches!(aspect, Aspect::Rigor)) {
        return Err("missing required answers");
    }
    for id in noul_ids {
        match response.answers.get(*id) {
            Some(Answer::Noul { noul }) if noul.is_finite() && (0.0..=1.0).contains(noul) => {}
            _ => return Err("invalid Noul answer"),
        }
    }
    if matches!(aspect, Aspect::Rigor) {
        let levels = ["trivial", "simple", "moderate", "complex", "very complex"];
        match response.answers.get("complexity") {
            Some(Answer::Score {
                score,
                legend,
                probabilities,
                confidence,
            }) if score.is_finite()
                && (0.0..=4.0).contains(score)
                && confidence.is_finite()
                && (0.0..=1.0).contains(confidence)
                && legend.len() == levels.len()
                && probabilities.len() == levels.len()
                && ["0", "1", "2", "3", "4"]
                    .into_iter()
                    .zip(levels)
                    .all(|(key, level)| {
                        legend.get(key).is_some_and(|value| value.as_str() == level)
                            && probabilities
                                .get(key)
                                .is_some_and(|v| v.is_finite() && (0.0..=1.0).contains(v))
                    })
                && (probabilities.values().sum::<f64>() - 1.0).abs() <= 0.02 => {}
            _ => return Err("invalid Score answer"),
        }
    }
    let price = policy.price();
    let estimated_cost_usd = (response.usage.input_tokens as f64 * price.input_per_million
        + response.usage.output_tokens as f64 * price.output_per_million)
        / 1_000_000.0;
    if !estimated_cost_usd.is_finite() {
        return Err("cost estimate overflow");
    }
    Ok(Counters {
        input_tokens: response.usage.input_tokens,
        output_tokens: response.usage.output_tokens,
        estimated_cost_usd,
    })
}

struct CapturedClient<'a, C> {
    inner: &'a C,
    policy: &'a SmokePolicy,
    aspect: Aspect,
    usage: Cell<Option<Counters>>,
}

impl<C: JevClient> JevClient for CapturedClient<'_, C> {
    async fn evaluate(&self, request: JevRequest) -> Result<JevResponse, JevError> {
        let response = self.inner.evaluate(request).await?;
        let counts =
            inspect(&response, self.policy, self.aspect).map_err(|_| JevError::Network {
                detail: "provider contract failed (details redacted)".to_string(),
            })?;
        self.usage.set(Some(counts));
        Ok(response)
    }
}

/// Exercise real rigor and drift state builders and evaluators on synthetic, transient inputs.
pub async fn compatibility<C: JevClient>(
    client: &C,
    policy: &SmokePolicy,
    config: &JevConfig,
    max_estimated_usd: f64,
) -> Result<Counters, &'static str> {
    let repo = tempfile::tempdir().map_err(|_| "cannot prepare synthetic state")?;
    std::fs::write(
        repo.path().join("sample.rs"),
        "fn add_one(value: u32) -> u32 { value + 1 }",
    )
    .map_err(|_| "cannot prepare synthetic state")?;
    let paths = [PathBuf::from("sample.rs")];
    let rigor_state = super::secret_filter::build_state_with_secret_filter(repo.path(), &paths)
        .map_err(|_| "synthetic rigor state rejected")?;
    let rigor = CapturedClient {
        inner: client,
        policy,
        aspect: Aspect::Rigor,
        usage: Cell::new(None),
    };
    super::rigor::evaluate_rigor(&rigor, rigor_state)
        .await
        .map_err(|_| "production rigor evaluation failed")?;
    let first = rigor.usage.get().ok_or("missing rigor usage")?;
    if first.estimated_cost_usd > max_estimated_usd {
        return Err("reviewed spending ceiling reached");
    }

    let changed = [PathBuf::from("sample.rs")];
    let protected = ["specs/".to_string()];
    let drift_state = super::secret_filter::build_drift_scope_state(
        &changed,
        &protected,
        Some("Update sample function only"),
    )
    .map_err(|_| "synthetic drift state rejected")?;
    let drift = CapturedClient {
        inner: client,
        policy,
        aspect: Aspect::Drift,
        usage: Cell::new(None),
    };
    let written = ["sample.rs".to_string()];
    let outcome = super::drift::evaluate_drift(
        &drift,
        drift_state,
        &written,
        &protected,
        config.drift_boundary,
    )
    .await
    .map_err(|_| "production drift evaluation failed")?;
    if outcome.forced_by_path_match {
        return Err("drift did not reach provider");
    }
    let second = drift.usage.get().ok_or("missing drift usage")?;
    let input_tokens = first
        .input_tokens
        .checked_add(second.input_tokens)
        .ok_or("token count overflow")?;
    let output_tokens = first
        .output_tokens
        .checked_add(second.output_tokens)
        .ok_or("token count overflow")?;
    let estimated_cost_usd = first.estimated_cost_usd + second.estimated_cost_usd;
    if !estimated_cost_usd.is_finite() || estimated_cost_usd > max_estimated_usd {
        return Err("reviewed spending ceiling reached");
    }
    Ok(Counters {
        input_tokens,
        output_tokens,
        estimated_cost_usd,
    })
}

/// Require the documented unauthenticated rejection. Caller supplies a client reading an invalid key.
pub async fn unauthorized<C: JevClient>(client: &C) -> Result<(), &'static str> {
    match client.evaluate(request()).await {
        Err(JevError::Unauthorized { .. }) => Ok(()),
        _ => Err("unauthorized rejection not demonstrated"),
    }
}

/// Only a provider-controlled delayed endpoint can establish live timeout evidence.
pub async fn timeout<C: JevClient>(client: &C) -> Result<(), &'static str> {
    match client.evaluate(request()).await {
        Err(JevError::Timeout { .. }) => Ok(()),
        _ => Err("provider timeout not demonstrated"),
    }
}

/// Only a provider-controlled 429/529 endpoint can establish live retry exhaustion evidence.
pub async fn retry<C: JevClient>(client: &C) -> Result<u16, &'static str> {
    match client.evaluate(request()).await {
        Err(JevError::RateLimitedOrOverloaded {
            last_status: status @ (429 | 529),
            attempts: 2,
        }) => Ok(status),
        _ => Err("provider retriable status not demonstrated"),
    }
}

/// A pending policy or a missing controlled vendor scenario cannot establish acceptance.
pub fn accepted(policy: &SmokePolicy, outcomes: [Outcome; 4], elapsed: Duration) -> bool {
    policy.approval == "approved"
        && outcomes
            .into_iter()
            .all(|outcome| outcome == Outcome::Passed)
        && elapsed < MAX_ELAPSED
}

/// Keep every individual scenario below the process-wide wall clock budget.
pub const MAX_ELAPSED: Duration = Duration::from_secs(90);

#[cfg(test)]
#[path = "provider_smoke_tests.rs"]
mod tests;
