//! Egress control tests (todo 26): typed HostPattern matching, empty
//! allowlist, no vendor escapes, verdict reasons.

#![cfg(test)]

use harness_safety::egress::{DenyReason, EgressPolicy, EgressVerdict, HostPattern};

#[test]
fn empty_allowlist_denies_every_host() {
    let policy = EgressPolicy::none();
    for host in ["api.anthropic.com", "factory.workos", "example.com"] {
        match policy.verdict(host, 443) {
            EgressVerdict::Denied {
                reason: DenyReason::NotInAllowlist,
                ..
            } => {}
            other => panic!("expected NotInAllowlist denial for {host}, got {other:?}"),
        }
    }
}

#[test]
fn allowlist_has_no_builtin_domains() {
    let fresh = EgressPolicy::none();
    assert!(
        fresh.iter().next().is_none(),
        "the list starts EMPTY - no always-allowed hosts, ever"
    );
}

#[test]
fn exact_pattern_matches_only_that_host() {
    let mut policy = EgressPolicy::none();
    policy
        .allow(HostPattern::Exact("api.example.com".into()))
        .expect("allow");
    assert!(matches!(
        policy.verdict("api.example.com", 443),
        EgressVerdict::Allowed
    ));
    assert!(policy.verdict("evil.example.com", 443).is_denied());
    assert!(policy.verdict("api.example.com.evil.io", 443).is_denied());
}

#[test]
fn wildcard_matches_subdomains_and_apex() {
    let mut policy = EgressPolicy::none();
    policy
        .allow(HostPattern::parse("*.deps.dev").expect("parse"))
        .expect("allow");
    assert!(matches!(
        policy.verdict("a.deps.dev", 443),
        EgressVerdict::Allowed
    ));
    assert!(matches!(
        policy.verdict("deps.dev", 443),
        EgressVerdict::Allowed
    ));
    assert!(policy.verdict("x.deps.dev.evil.io", 443).is_denied());
}

#[test]
fn port_scoped_pattern_only_matches_its_port() {
    let mut policy = EgressPolicy::none();
    policy
        .allow(HostPattern::parse("proxy.internal:8080").expect("parse"))
        .expect("allow");
    assert!(matches!(
        policy.verdict("proxy.internal", 8080),
        EgressVerdict::Allowed
    ));
    assert!(policy.verdict("proxy.internal", 443).is_denied());
}

#[test]
fn malformed_hosts_are_typed_rejections_not_panics() {
    for bad in ["", "has space", "a/b", "..", "https://x", "a:b:c"] {
        let parsed = HostPattern::parse(bad);
        assert!(parsed.is_err(), "{bad:?} must be a typed parse error");
    }
}

#[test]
fn verdict_on_malformed_host_is_denied_with_reason() {
    let policy = EgressPolicy::none();
    match policy.verdict("", 443) {
        EgressVerdict::Denied {
            reason: DenyReason::MalformedHost,
            ..
        } => {}
        other => panic!("expected MalformedHost, got {other:?}"),
    }
}

#[test]
fn host_matching_is_pure_text_no_eval() {
    // untrusted input in the host position must never be interpreted
    let mut policy = EgressPolicy::none();
    policy
        .allow(HostPattern::Exact("a.com".into()))
        .expect("allow");
    let hostile = "a.com'; DROP TABLE; $(rm -rf); `id`";
    assert!(policy.verdict(hostile, 443).is_denied());
}

#[test]
fn unicode_host_matches_by_exact_text() {
    let mut policy = EgressPolicy::none();
    policy
        .allow(HostPattern::Exact("exämple.de".into()))
        .expect("allow");
    assert!(matches!(
        policy.verdict("exämple.de", 443),
        EgressVerdict::Allowed
    ));
    assert!(policy.verdict("xn--exmple-cua.de", 443).is_denied());
}

#[test]
fn parse_round_trips_exact_wildcard_and_port() {
    assert!(matches!(
        HostPattern::parse("a.com"),
        Ok(HostPattern::Exact(_))
    ));
    assert!(matches!(
        HostPattern::parse("*.a.com"),
        Ok(HostPattern::Wildcard(_))
    ));
    assert!(matches!(
        HostPattern::parse("a.com:8443"),
        Ok(HostPattern::WithPort(_, 8443))
    ));
}
