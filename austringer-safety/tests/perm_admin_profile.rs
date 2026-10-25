//! Admin-floor + SSRF-guard tests (todo 27): org rules that user
//! config cannot weaken, and SSRF categories blocked by default in
//! egress rule authoring (admin opt-out only).

use austringer_safety::egress::HostPattern;
use austringer_safety::perm::floor::{
    AdminFloor, EgressAuthoringError, author_egress, ssrf_violation,
};
use austringer_safety::perm::grammar::{
    Action, Effect, GrammarVerdict, Provenance, Rule, Target, TargetPattern,
};
use austringer_safety::perm::reasons::SsrfKind;

fn admin_rule(pattern: &str) -> Rule {
    Rule::new(
        Effect::Deny,
        Action::Execute,
        TargetPattern::prefix(pattern),
        Provenance::Admin,
        "org floor",
    )
    .expect("valid rule")
}

fn user_allow(pattern: &str) -> Rule {
    Rule::new(
        Effect::Allow,
        Action::Execute,
        TargetPattern::prefix(pattern),
        Provenance::User,
        "user",
    )
    .expect("valid rule")
}

#[test]
fn admin_floor_only_accepts_admin_deny_rules() {
    assert!(AdminFloor::new(vec![admin_rule("npm publish")], vec![]).is_ok());
    let wrong_effect = Rule::new(
        Effect::Allow,
        Action::Execute,
        TargetPattern::prefix("x"),
        Provenance::Admin,
        "bad",
    )
    .expect("valid rule");
    assert!(AdminFloor::new(vec![wrong_effect], vec![]).is_err());
    let wrong_provenance = Rule::new(
        Effect::Deny,
        Action::Execute,
        TargetPattern::prefix("x"),
        Provenance::User,
        "bad",
    )
    .expect("valid rule");
    assert!(AdminFloor::new(vec![wrong_provenance], vec![]).is_err());
}

#[test]
fn admin_floor_denies_even_when_user_rules_allow() {
    // The floor check runs before and above user rules; the user's
    // broad allow can never weaken the admin deny.
    let floor = AdminFloor::new(vec![admin_rule("npm publish")], vec![]).expect("floor");
    let user_rules = vec![user_allow("npm")];
    let target = Target::Command("npm publish".into());
    assert_eq!(
        floor.enforce(&user_rules, &Action::Execute, &target),
        GrammarVerdict::Denied {
            origin: "org floor".into(),
            admin: true
        }
    );
    let ok = Target::Command("npm i".into());
    assert_eq!(
        floor.enforce(&user_rules, &Action::Execute, &ok),
        GrammarVerdict::Allowed {
            origin: "user".into()
        }
    );
}

#[test]
fn ssrf_guard_blocks_private_categories_by_default() {
    assert_eq!(ssrf_violation("localhost", None), Some(SsrfKind::Localhost));
    assert_eq!(ssrf_violation("127.0.0.1", None), Some(SsrfKind::Localhost));
    assert_eq!(
        ssrf_violation("sub.localhost", None),
        Some(SsrfKind::Localhost)
    );
    assert_eq!(ssrf_violation("10.0.0.5", None), Some(SsrfKind::Rfc1918));
    assert_eq!(ssrf_violation("172.16.1.1", None), Some(SsrfKind::Rfc1918));
    assert_eq!(ssrf_violation("192.168.1.1", None), Some(SsrfKind::Rfc1918));
    assert_eq!(
        ssrf_violation("169.254.169.254", None),
        Some(SsrfKind::LinkLocalMetadata)
    );
    assert_eq!(ssrf_violation("fd00::1", None), Some(SsrfKind::Ipv6Ula));
    assert_eq!(
        ssrf_violation("fe80::1", None),
        Some(SsrfKind::Ipv6LinkLocal)
    );
    assert_eq!(ssrf_violation("::1", None), Some(SsrfKind::Localhost));
    assert_eq!(ssrf_violation("0.0.0.0", None), Some(SsrfKind::Unspecified));
    assert_eq!(ssrf_violation("::", None), Some(SsrfKind::Unspecified));
    assert_eq!(
        ssrf_violation("::ffff:10.0.0.1", None),
        Some(SsrfKind::Rfc1918)
    );
    // Decimal-integer IPv4 host form.
    assert_eq!(
        ssrf_violation("2130706433", None),
        Some(SsrfKind::Localhost)
    );
    assert_eq!(ssrf_violation("api.example.com", None), None);
    assert_eq!(ssrf_violation("8.8.8.8", None), None);
}

#[test]
fn ssrf_opt_out_requires_admin_grant() {
    let opt = AdminFloor::new(
        vec![],
        vec![HostPattern::parse("10.1.2.3").expect("pattern")],
    )
    .expect("floor");
    assert_eq!(ssrf_violation("10.1.2.3", Some(&opt)), None);
    assert_eq!(
        ssrf_violation("10.1.2.4", Some(&opt)),
        Some(SsrfKind::Rfc1918)
    );
}

#[test]
fn egress_authoring_blocks_ssrf_by_default() {
    let floor = AdminFloor::new(vec![], vec![]).expect("floor");
    assert!(author_egress("api.example.com", Some(&floor)).is_ok());
    assert!(matches!(
        author_egress("169.254.169.254", Some(&floor)),
        Err(EgressAuthoringError::SsrfBlocked {
            kind: SsrfKind::LinkLocalMetadata
        })
    ));
    assert!(matches!(
        author_egress("localhost", Some(&floor)),
        Err(EgressAuthoringError::SsrfBlocked { .. })
    ));
    // No floor at all (unmanaged local config): the guard is still
    // the default stance.
    assert!(matches!(
        author_egress("10.0.0.5", None),
        Err(EgressAuthoringError::SsrfBlocked { .. })
    ));
}

#[test]
fn egress_authoring_admin_optout_lets_private_hosts_in() {
    let floor = AdminFloor::new(
        vec![],
        vec![HostPattern::parse("proxy.internal").expect("p")],
    )
    .expect("floor");
    assert!(author_egress("proxy.internal", Some(&floor)).is_ok());
}
