//! Identity-contract tests: model identity is explicit (never inferred from
//! endpoints), request identity is stable across re-issues, and a model
//! fallback can ONLY happen through the acknowledged `FallbackRequest` flow.

use austringer_providers::identity::{
    AcknowledgedFallback, CompletionRequest, FallbackRequest, ModelIdentity,
};

fn primary() -> ModelIdentity {
    ModelIdentity::new("anthropic", "claude-4-5")
}

fn backup() -> ModelIdentity {
    ModelIdentity::new("anthropic", "claude-4-sonnet")
}

#[test]
fn request_identity_is_stable_across_identical_reissues() {
    let a = CompletionRequest::new(primary(), "same prompt");
    let b = CompletionRequest::new(primary(), "same prompt");
    assert_eq!(
        a.turn_id(),
        b.turn_id(),
        "identical content => identical id"
    );

    let c = CompletionRequest::new(primary(), "different prompt");
    assert_ne!(a.turn_id(), c.turn_id());

    let d = CompletionRequest::new(backup(), "same prompt");
    assert_ne!(
        a.turn_id(),
        d.turn_id(),
        "model identity is part of the request identity"
    );
}

#[test]
fn headers_ride_on_the_request_for_transport_use() {
    let request =
        CompletionRequest::new(primary(), "p").with_header("authorization", "Bearer test");
    assert_eq!(
        request.headers.get("authorization"),
        Some(&"Bearer test".to_owned())
    );
    assert_eq!(request.identity, primary());
}

#[test]
fn fallback_requires_explicit_acknowledgement_and_retargets_the_request() {
    // The confirmation flow: build a FallbackRequest naming from/to, then
    // ACKNOWLEDGE it. `AcknowledgedFallback` has no other constructor, so no
    // code path can swap models without this explicit step.
    let pending = FallbackRequest::new(
        primary(),
        backup(),
        "primary() unavailable after retry budget",
    );
    let acknowledged: AcknowledgedFallback = pending.acknowledge();

    let request = CompletionRequest::new(primary(), "p").with_fallback(acknowledged);
    // The acknowledged fallback RETARGETS the request: identity becomes the
    // acknowledged backup() and the audit trail records the swap origin.
    assert_eq!(request.identity, backup());
    let trail = request.fallback.as_ref().expect("audit trail recorded");
    assert_eq!(trail.from(), &primary());
    assert_eq!(trail.to(), &backup());
    assert_eq!(trail.reason(), "primary() unavailable after retry budget");
}

#[test]
fn an_unacknowledged_request_keeps_targeting_the_original_model() {
    // A plain request has no fallback attached: there is no way for the
    // transport layer to move it to another model without constructing a
    // fresh, acknowledged request.
    let request = CompletionRequest::new(primary(), "p");
    assert!(request.fallback.is_none());
    assert_eq!(request.identity, primary());
}
