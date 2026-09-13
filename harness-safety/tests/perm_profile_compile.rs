//! Security-profile + permissions-derived-egress compile tests
//! (todo 27): profile intersection where lower levels only tighten,
//! grant clamping, restart-stable serialization, and the
//! read_url→egress compiler round trip.

use harness_safety::egress::{EgressPolicy, EgressVerdict, HostPattern};
use harness_safety::perm::compile::compile_egress;
use harness_safety::perm::grammar::{Action, Effect, Provenance, Rule, TargetPattern};
use harness_safety::perm::profile::{ClampOutcome, SecurityProfile, intersect};

fn hosts(list: &[&str]) -> Vec<HostPattern> {
    list.iter()
        .map(|h| HostPattern::parse(h).expect("pattern"))
        .collect()
}

fn profile(read_url_hosts: Option<Vec<&str>>, denied: &[&str]) -> SecurityProfile {
    SecurityProfile {
        read_url_hosts: read_url_hosts.map(|h| hosts(&h)),
        denied_programs: denied.iter().map(ToString::to_string).collect(),
        require_ask: Vec::new(),
    }
}

#[test]
fn profile_intersection_only_tightens() {
    let org = profile(Some(vec!["api.example.com", "cdn.example.com"]), &[]);
    let automation = profile(None, &[]);
    let session = profile(Some(vec!["cdn.example.com"]), &[]);
    let effective = intersect(&org, &automation, &session);
    assert_eq!(effective.read_url_hosts, Some(hosts(&["cdn.example.com"])));
    let loose = profile(None, &[]);
    assert_eq!(
        intersect(&org, &loose, &loose).read_url_hosts,
        Some(hosts(&["api.example.com", "cdn.example.com"]))
    );
}

#[test]
fn profile_denied_programs_and_ask_union() {
    let org = profile(None, &["format"]);
    let automation = profile(None, &["diskpart"]);
    let session = SecurityProfile {
        read_url_hosts: None,
        denied_programs: vec![],
        require_ask: vec![Action::ExecuteUrl],
    };
    let effective = intersect(&org, &automation, &session);
    assert!(effective.denied_programs.iter().any(|p| p == "format"));
    assert!(effective.denied_programs.iter().any(|p| p == "diskpart"));
    assert!(effective.require_ask.contains(&Action::ExecuteUrl));
}

#[test]
fn mid_session_grants_clamp_to_the_intersection() {
    let org = profile(Some(vec!["*.internal.corp"]), &[]);
    let effective = intersect(&org, &profile(None, &[]), &profile(None, &[]));
    let ok = effective.check_grant(&Action::ReadUrl, "api.internal.corp");
    assert!(matches!(ok, ClampOutcome::WithinProfile));
    let wide = effective.check_grant(&Action::ReadUrl, "evil.example.com");
    assert!(
        matches!(wide, ClampOutcome::Clamped { .. }),
        "granting wider than the profile must clamp: {wide:?}"
    );
}

#[test]
fn profile_survives_serialization_roundtrip() {
    let effective = intersect(
        &profile(Some(vec!["*.internal.corp"]), &["format"]),
        &profile(None, &[]),
        &profile(None, &[]),
    );
    let json = serde_json::to_string(&effective).expect("serialize");
    let back: SecurityProfile = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, effective);
}

fn read_url_allow(pattern: &str) -> Rule {
    Rule::new(
        Effect::Allow,
        Action::ReadUrl,
        TargetPattern::prefix(pattern),
        Provenance::User,
        "fetch",
    )
    .expect("valid rule")
}

#[test]
fn read_url_grants_compile_into_egress_allowlist() {
    let rules = vec![
        read_url_allow("api.example.com"),
        read_url_allow("*.cdn.example.com"),
    ];
    let egress = compile_egress(&rules).expect("compile");
    assert_eq!(
        egress.verdict("api.example.com", 443),
        EgressVerdict::Allowed
    );
    assert_eq!(
        egress.verdict("assets.cdn.example.com", 443),
        EgressVerdict::Allowed
    );
    assert!(egress.verdict("evil.example.com", 443).is_denied());
    assert!(egress.verdict("api.example.com.evil.io", 443).is_denied());
}

#[test]
fn non_url_and_deny_rules_do_not_compile_grants() {
    let rules = vec![
        Rule::new(
            Effect::Allow,
            Action::Execute,
            TargetPattern::prefix("npm"),
            Provenance::User,
            "cmd",
        )
        .expect("valid rule"),
        Rule::new(
            Effect::Deny,
            Action::ReadUrl,
            TargetPattern::prefix("evil.com"),
            Provenance::User,
            "no",
        )
        .expect("valid rule"),
    ];
    let egress = compile_egress(&rules).expect("compile");
    assert!(egress.is_empty());
}

#[test]
fn regex_url_rules_fail_compile_closed() {
    let rules = vec![
        Rule::new(
            Effect::Allow,
            Action::ReadUrl,
            TargetPattern::regex(".*").expect("regex"),
            Provenance::User,
            "sloppy",
        )
        .expect("valid rule"),
    ];
    assert!(compile_egress(&rules).is_err());
}

#[test]
fn empty_compile_yields_the_empty_allowlist() {
    let egress = compile_egress(&[]).expect("compile");
    assert_eq!(egress, EgressPolicy::none());
}
