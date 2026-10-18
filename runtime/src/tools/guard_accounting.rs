//! Per-invocation accounting, without retaining requests, responses, or credentials.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct GuardAccounting {
    pub schema_version: u8,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub transport_attempts: u64,
    pub unknown_usage_calls: u64,
    pub observed_models: Vec<String>,
    pub elapsed_ms: u64,
}

impl Default for GuardAccounting {
    fn default() -> Self {
        Self {
            schema_version: 1,
            input_tokens: 0,
            output_tokens: 0,
            transport_attempts: 0,
            unknown_usage_calls: 0,
            observed_models: Vec::new(),
            elapsed_ms: 0,
        }
    }
}

#[cfg(feature = "jev-http")]
pub(crate) struct AccountedClient {
    inner: crate::engine::jev::client_http::HttpClient,
    totals: std::sync::Mutex<UsageTotals>,
}

#[cfg(feature = "jev-http")]
#[derive(Default)]
struct UsageTotals {
    input: u64,
    output: u64,
    responses: u64,
    models: Vec<String>,
}

#[cfg(feature = "jev-http")]
impl AccountedClient {
    pub fn new(inner: crate::engine::jev::client_http::HttpClient) -> Self {
        Self {
            inner,
            totals: std::sync::Mutex::new(UsageTotals::default()),
        }
    }

    pub fn into_accounting(self, elapsed_ms: u64) -> GuardAccounting {
        let transport_attempts = self.inner.attempt_count();
        let totals = self
            .totals
            .into_inner()
            .unwrap_or_else(|poison| poison.into_inner());
        GuardAccounting {
            schema_version: 1,
            input_tokens: totals.input,
            output_tokens: totals.output,
            transport_attempts,
            // Retried sends have no parsed usage, even if the final response succeeds.
            unknown_usage_calls: transport_attempts.saturating_sub(totals.responses),
            observed_models: totals.models,
            elapsed_ms,
        }
    }
}

#[cfg(feature = "jev-http")]
impl crate::engine::jev::JevClient for AccountedClient {
    async fn evaluate(
        &self,
        request: crate::engine::jev::JevRequest,
    ) -> Result<crate::engine::jev::JevResponse, crate::engine::jev::JevError> {
        let unknown_usage = {
            let totals = self
                .totals
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            self.inner.attempt_count() > totals.responses
        };
        if unknown_usage {
            return Err(crate::engine::jev::JevError::Network {
                detail: "provider usage is unknown; no further evaluation was sent".to_string(),
            });
        }
        let response = crate::engine::jev::JevClient::evaluate(&self.inner, request).await?;
        let mut totals = self
            .totals
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        totals.input = totals.input.saturating_add(response.usage.input_tokens);
        totals.output = totals.output.saturating_add(response.usage.output_tokens);
        totals.responses = totals.responses.saturating_add(1);
        let model = if !response.model.is_empty()
            && response.model.len() <= 64
            && response
                .model
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            response.model.as_str()
        } else {
            "<invalid responder identity>"
        };
        if !totals.models.iter().any(|observed| observed == model) {
            totals.models.push(model.to_string());
        }
        Ok(response)
    }
}

pub(super) fn findings(
    observation: &crate::engine::jev::guard::evaluate::GuardObservation,
) -> serde_json::Value {
    serde_json::json!({
        "rigor": observation.rigor.as_ref().map(|rigor| serde_json::json!({
            "hallucinated_import": rigor.hallucinated_import,
            "violates_conventions": rigor.violates_conventions,
            "contains_secrets": rigor.contains_secrets,
            "complexity": rigor.complexity,
        })),
        "drift": observation.drift.as_ref().map(|drift| serde_json::json!({
            "noul": drift.noul,
            "out_of_scope": drift.out_of_scope,
            "forced_by_path_match": drift.forced_by_path_match,
        })),
        "unavailable": observation.unavailable.as_ref().map(|_| "evaluation_unavailable"),
        "unavailable_context": observation.unavailable_context,
    })
}
