//! Runtime capability-override semantics (plan todo M7): capability
//! flags are catalog-owned, source-cited data. An override may only
//! LOWER a flag; any false->true transition — including restoring a
//! previously lowered flag — is refused with a typed error. The
//! per-model (never family-inferred) invariant is pinned here too: the
//! divergence test restricts ONE model of a family and proves the two
//! same-family entries stay independent.

use harness_intel::catalog::{Capabilities, Catalog, OverrideError};

#[test]
fn override_attempting_to_raise_a_capability_is_refused() {
    // Given: catalog-owned, source-cited flags. When: a runtime override
    // attempts to RAISE a flag (audio on the text-only reasoner).
    // Then: typed refusal and the catalog data is left untouched.
    let mut catalog = Catalog::seeded();
    let raise = Capabilities {
        vision: false,
        audio_input: true,
        video_input: false,
    };
    let err = catalog
        .override_capabilities("deepseek-reasoner", raise)
        .expect_err("raising above the cited seed must be refused");
    let OverrideError::Raise(refused) = err else {
        panic!("expected a raise refusal, got: {err}");
    };
    assert_eq!(refused.model, "deepseek-reasoner");
    assert_eq!(refused.requested, raise);
    assert_eq!(refused.current, Capabilities::default());
    assert_eq!(
        catalog.capabilities("deepseek-reasoner"),
        Ok(Capabilities::default()),
        "the refused override must not mutate the entry"
    );
}

#[test]
fn override_lowering_is_allowed_and_never_reversible_by_override() {
    // Given: a seeded vision model. When: restricted to text-only.
    // Then: the restriction applies, and re-raising to the ORIGINAL
    // seed value is refused — no override ever turns false into true.
    let mut catalog = Catalog::seeded();
    let seed = catalog.capabilities("claude-opus-4-6").expect("seeded");
    let restricted = Capabilities {
        vision: false,
        ..seed
    };
    catalog
        .override_capabilities("claude-opus-4-6", restricted)
        .expect("lowering a flag is legal");
    assert_eq!(catalog.capabilities("claude-opus-4-6"), Ok(restricted));
    // Restoring a lowered flag would be a false->true raise: refused.
    assert!(
        matches!(
            catalog.override_capabilities("claude-opus-4-6", seed),
            Err(OverrideError::Raise(_))
        ),
        "re-raising to the original seed value must be refused"
    );
    // Unknown models are typed errors on the override path too.
    assert!(matches!(
        catalog.override_capabilities("mystery-model", Capabilities::default()),
        Err(OverrideError::UnknownModel(_))
    ));
}

#[test]
fn capabilities_are_per_model_not_family_inferred() {
    // The M7 invariant: two models in ONE family can carry different
    // flags and stay different — there is no family->capabilities path
    // to collapse them. Given: two same-family Anthropic models.
    let mut catalog = Catalog::seeded();
    let before = catalog.capabilities("claude-opus-4-6").expect("seeded");
    assert_eq!(
        before,
        catalog.capabilities("claude-opus-4-7").expect("seeded")
    );

    // When: only one of them is restricted at runtime. Then: the two
    // SAME-FAMILY models diverge — lookup serves per-entry data.
    catalog
        .override_capabilities("claude-opus-4-6", Capabilities::default())
        .expect("lowering is legal");
    assert_ne!(
        catalog.capabilities("claude-opus-4-6").expect("seeded"),
        catalog.capabilities("claude-opus-4-7").expect("seeded"),
        "same-family models with different flags must stay different"
    );
    assert_eq!(
        catalog.capabilities("claude-opus-4-6"),
        Ok(Capabilities::default())
    );
    assert_eq!(catalog.capabilities("claude-opus-4-7"), Ok(before));
}
