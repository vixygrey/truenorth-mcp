//! Usage recording for complete guard evaluations.

use super::*;
#[derive(Default, Clone, Copy)]
pub(super) struct Totals {
    pub calls: u64,
    pub input: u64,
    pub output: u64,
    pub responses: u64,
}

pub(super) struct Recorder<'a, C: JevClient, F: Fn() -> Option<u64>> {
    client: &'a C,
    totals: Mutex<Totals>,
    models: Mutex<BTreeSet<String>>,
    transport_attempts: &'a F,
    budget: Option<f64>,
    price: JevPrice,
}
impl<'a, C: JevClient, F: Fn() -> Option<u64>> Recorder<'a, C, F> {
    pub(super) fn new(
        client: &'a C,
        transport_attempts: &'a F,
        budget: Option<f64>,
        price: JevPrice,
    ) -> Self {
        Self {
            client,
            totals: Mutex::new(Totals::default()),
            models: Mutex::new(BTreeSet::new()),
            transport_attempts,
            budget,
            price,
        }
    }
    pub(super) fn totals(&self) -> Totals {
        *self.totals.lock().unwrap_or_else(|p| p.into_inner())
    }
    pub(super) fn models(&self) -> Vec<String> {
        self.models
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .cloned()
            .collect()
    }
}
impl<C: JevClient, F: Fn() -> Option<u64>> JevClient for Recorder<'_, C, F> {
    async fn evaluate(&self, request: JevRequest) -> Result<JevResponse, JevError> {
        let before = self.totals();
        if before.calls != before.responses
            || (self.transport_attempts)().is_some_and(|attempts| attempts > before.responses)
        {
            return Err(JevError::Network {
                detail: "provider usage is unknown; no further evaluation was sent".into(),
            });
        }
        if self.budget.is_some_and(|budget| {
            cost_estimate(before.input, before.output, Some(self.price))
                .is_none_or(|cost| cost.amount >= budget)
        }) {
            return Err(JevError::Network {
                detail: "reviewed provider budget reached; no further evaluation was sent".into(),
            });
        }
        self.totals.lock().unwrap_or_else(|p| p.into_inner()).calls += 1;
        let response = self.client.evaluate(request).await?;
        let mut totals = self.totals.lock().unwrap_or_else(|p| p.into_inner());
        totals.input = totals.input.saturating_add(response.usage.input_tokens);
        totals.output = totals.output.saturating_add(response.usage.output_tokens);
        totals.responses += 1;
        drop(totals);
        let model = &response.model;
        self.models
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                if !model.is_empty()
                    && model.len() <= 64
                    && model
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
                {
                    model.clone()
                } else {
                    "<invalid responder identity>".into()
                },
            );
        Ok(response)
    }
}
