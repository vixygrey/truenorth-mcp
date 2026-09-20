//! Property test for the flag-off silence property.
//!
//! Included from `client_fake.rs` via `#[path]`, so `super` is the client_fake module.
//!
//! Feature: jev-integration-eval, Property 22: flag-off silence. While the Jev feature
//! flag is off, the harness makes no network call and constructs no client call path. The
//! aspect modules will gate every call on the flag. This property pins the contract at the
//! client layer: a caller that respects the flag makes zero calls while the flag is off,
//! so the fake records none (Requirement 1.4, 1.5, 2.4).

use std::collections::BTreeMap;

use proptest::prelude::*;

use super::super::{Answer, JevRequest, JevResponse, Usage};
use super::*;

/// The flag-gated call contract every aspect module must obey: call the client only when
/// the Jev feature is on. This mirrors the guard the aspects apply, so the property tests
/// the contract rather than a single call site.
fn evaluate_when_enabled(jev_enabled: bool, fake: &FakeClient, state: serde_json::Value) {
    if !jev_enabled {
        // Flag off: build no request and make no call (Requirement 1.4, 1.5).
        return;
    }
    fake.push_response(JevResponse {
        model: "jev-latest".to_string(),
        answers: BTreeMap::from([("q".to_string(), Answer::Noul { noul: 0.0 })]),
        usage: Usage {
            input_tokens: 1,
            output_tokens: 0,
        },
    });
    // A synchronous drive is enough: the fake resolves immediately.
    let request = JevRequest::new(state, BTreeMap::new());
    let _ = futures_lite_block_on(fake.evaluate(request));
}

/// Drive a ready future to completion without a runtime.
///
/// The fake never yields, so the first poll resolves. This keeps the property test free of
/// an async runtime while still exercising the async trait method.
fn futures_lite_block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    fn noop_raw_waker() -> RawWaker {
        fn no_op(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            noop_raw_waker()
        }
        let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
        RawWaker::new(std::ptr::null(), vtable)
    }

    let waker = unsafe { Waker::from_raw(noop_raw_waker()) };
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the fake future must resolve on the first poll"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// While the flag is off, a flag-respecting caller makes no call, so the fake records
    /// none. While the flag is on, the caller makes exactly one call.
    #[test]
    fn flag_off_makes_no_call(jev_enabled in any::<bool>(), state in "[a-z ]{0,40}") {
        let fake = FakeClient::new();
        evaluate_when_enabled(jev_enabled, &fake, serde_json::json!(state));

        if jev_enabled {
            prop_assert_eq!(fake.call_count(), 1, "an enabled flag makes exactly one call");
        } else {
            prop_assert_eq!(fake.call_count(), 0, "a disabled flag makes no call");
        }
    }
}
