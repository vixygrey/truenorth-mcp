//! Property tests for `evaluate_guard` and the fail-open layer (task 3, issue #295).
//!
//! Included from `guard_evaluate.rs` via `#[path]`, so `super` is the `evaluate` module.
//!
//! Feature: jev-active-guardrail.
//!
//! - Property 36: flag-off silence. For all changes, while the flag is off, `evaluate_guard`
//!   makes no Jev call and returns Allow with a note (R4.1, R8.2).
//! - Property 37: probabilistic fail-open. For all Jev unavailability outcomes (timeout,
//!   network error, empty queue, absent key), `evaluate_guard` returns Allow, never Block
//!   (R4.2, R4.3, R4.5).
//! - Property 40: no secret to Jev. For all changes that inject denylist content, no Jev
//!   request state carries the content and no output carries the API key (R3.2, R6.3).
//!
//! Every property drives the named fake with no network call.

use proptest::prelude::*;
use tempfile::TempDir;

use super::super::super::client_fake::{FakeClient, FakeError};
use super::super::super::config::JevConfig;
use super::super::{GuardDecision, ProposedChange};
use super::*;

/// A protected set the properties reuse.
fn protected_set() -> Vec<String> {
    vec!["specs/".to_string(), "LICENSE".to_string()]
}

/// Run `evaluate_guard` to completion on a fresh tokio runtime, for a synchronous property body.
fn run_guard(
    change: &ProposedChange,
    jev_enabled: bool,
    api_key_present: bool,
    fake: Option<&FakeClient>,
    repo_root: &std::path::Path,
) -> GuardDecision {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("build a current-thread runtime");
    runtime.block_on(evaluate_guard(
        change,
        &protected_set(),
        &JevConfig::default(),
        jev_enabled,
        api_key_present,
        fake,
        repo_root,
    ))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Property 36: while the flag is off, no Jev call and Allow with a note.
    #[test]
    fn p36_flag_off_is_silent(name in "[a-z]{1,20}", body in "[a-zA-Z0-9 ]{0,60}") {
        let repo = TempDir::new().expect("temp repo");
        let fake = FakeClient::new();
        let change = ProposedChange {
            paths: vec![format!("src/{name}.rs")],
            content: body,
        };

        let decision = run_guard(&change, false, true, Some(&fake), repo.path());

        let is_allow = matches!(decision, GuardDecision::Allow { .. });
        prop_assert!(is_allow, "flag off returns Allow: {decision:?}");
        prop_assert_eq!(fake.call_count(), 0, "flag off makes no Jev call");
    }

    /// Property 37: every Jev unavailability outcome fails open to Allow, never Block. The
    /// generated selector picks which unavailability the run exercises.
    #[test]
    fn p37_probabilistic_fails_open(
        name in "[a-z]{1,20}",
        outcome in prop::sample::select(vec!["timeout", "network", "empty", "no_key"]),
    ) {
        let repo = TempDir::new().expect("temp repo");
        std::fs::write(repo.path().join("f.rs"), "fn f() {}").expect("write file");
        let fake = FakeClient::new();
        let mut api_key_present = true;

        match outcome {
            "timeout" => fake.push_error(FakeError::Timeout { timeout_ms: 30_000 }),
            "network" => fake.push_error(FakeError::Network { detail: "reset".to_string() }),
            // "empty" queues nothing, so the fake returns its empty-queue network error.
            "empty" => {}
            // "no_key" short-circuits before any call.
            "no_key" => api_key_present = false,
            _ => unreachable!(),
        }

        let change = ProposedChange {
            paths: vec![format!("src/{name}.rs")],
            content: "clean body".to_string(),
        };

        let decision = run_guard(&change, true, api_key_present, Some(&fake), repo.path());

        let is_block = matches!(decision, GuardDecision::Block(_));
        prop_assert!(!is_block, "unavailability never blocks: {decision:?}");
        let is_allow = matches!(decision, GuardDecision::Allow { .. });
        prop_assert!(is_allow, "unavailability fails open to Allow: {decision:?}");
    }

    /// Property 40: injected denylist content never reaches a Jev request state, and no output
    /// carries the API key. The change writes a secret marker to a non-protected file; the
    /// deterministic secret scan blocks it before any call, so the fake sees no request, and no
    /// request state can carry the secret.
    #[test]
    fn p40_no_secret_reaches_jev(name in "[a-z]{1,20}", marker in prop::sample::select(vec!["secret", "credentials"])) {
        let repo = TempDir::new().expect("temp repo");
        let fake = FakeClient::new();
        let secret_body = format!("const TOKEN = \"{marker}-value\";");
        let change = ProposedChange {
            paths: vec![format!("src/{name}.rs")],
            content: secret_body.clone(),
        };

        let decision = run_guard(&change, true, true, Some(&fake), repo.path());

        // The deterministic secret scan blocks the change with no Jev call (R2.2, P35).
        let is_block = matches!(decision, GuardDecision::Block(_));
        prop_assert!(is_block, "secret content blocks: {decision:?}");
        prop_assert_eq!(fake.call_count(), 0, "no request reaches Jev");

        // No request state carries the secret, because no request was sent.
        for request in fake.calls() {
            let serialized = serde_json::to_string(&request).unwrap_or_default();
            prop_assert!(!serialized.contains("-value"), "no request carries the secret");
        }

        // The block packet names a marker, never the secret value (R6.3, P40).
        if let GuardDecision::Block(packet) = &decision {
            let json = serde_json::to_string(packet).unwrap_or_default();
            prop_assert!(!json.contains("-value"), "the packet carries no secret value");
        }
    }

    /// Property 40 companion: a clean change that reaches Jev sends a secret-filtered state, so
    /// even a state assembled from files carries no denylist content. The written file holds a
    /// secret line, which the secret filter drops before the call. The proposed content itself
    /// is clean, so the deterministic scan passes and the probabilistic layer runs.
    #[test]
    fn p40_state_from_files_is_secret_filtered(name in "[a-z]{1,20}") {
        let repo = TempDir::new().expect("temp repo");
        // The file on disk holds a secret line. The proposed content is clean.
        let rel = format!("src/{name}.rs");
        let path = repo.path().join(&rel);
        std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
        std::fs::write(&path, "let x = 1;\nSECRET_TOKEN=leak-me\nlet y = 2;").expect("write");

        let fake = FakeClient::new();
        // Under-queue on purpose: the rigor call fails open, but the fake still records the
        // request it received, so the property can inspect the state that was sent.
        let change = ProposedChange {
            paths: vec![rel],
            content: "let x = 1;".to_string(),
        };

        let _ = run_guard(&change, true, true, Some(&fake), repo.path());

        // The rigor call carried a state. That state must not contain the secret value, because
        // the secret filter drops the denylisted line before the call (R3.2, P40).
        let calls = fake.calls();
        prop_assert!(!calls.is_empty(), "the rigor call was made");
        for request in calls {
            let serialized = serde_json::to_string(&request).unwrap_or_default();
            prop_assert!(!serialized.contains("leak-me"), "no state carries the secret");
        }
    }
}
