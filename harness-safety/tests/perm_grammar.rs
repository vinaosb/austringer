//! Permission grammar tests (todo 27): Action(target) model, Deny >
//! Ask > Allow precedence, prefix-default matching, opt-in regex,
//! read_url/execute_url split, typed malformed targets.

use harness_safety::perm::grammar::{
    Action, Effect, GrammarVerdict, MalformedTarget, Provenance, Rule, Target, TargetPattern,
    UrlTarget,
};
use std::path::Path;

fn rule(effect: Effect, action: Action, pattern: &str) -> Rule {
    Rule::new(
        effect,
        action,
        TargetPattern::prefix(pattern),
        Provenance::User,
        "test",
    )
    .unwrap_or_else(|e| panic!("valid rule rejected: {e}"))
}

fn url(raw: &str) -> UrlTarget {
    UrlTarget::parse(raw).unwrap_or_else(|e| panic!("valid url rejected: {e}"))
}

#[test]
fn deny_beats_ask_beats_allow_on_same_target() {
    let rules = vec![
        rule(Effect::Allow, Action::Read, "src/"),
        rule(Effect::Ask, Action::Read, "src/"),
        rule(Effect::Deny, Action::Read, "src/secret"),
    ];
    // Strictest match wins: deny over ask.
    let v = harness_safety::perm::grammar::verdict(
        &rules,
        &Action::Read,
        &Target::Path(Path::new("src/secret/x.pem").to_path_buf()),
    );
    assert!(matches!(v, GrammarVerdict::Denied { .. }), "got {v:?}");
    // Ask beats allow.
    let ask_rules = vec![
        rule(Effect::Allow, Action::Read, "src/"),
        rule(Effect::Ask, Action::Read, "src/"),
    ];
    let v = harness_safety::perm::grammar::verdict(
        &ask_rules,
        &Action::Read,
        &Target::Path(Path::new("src/a.rs").to_path_buf()),
    );
    assert!(matches!(v, GrammarVerdict::Asked { .. }), "got {v:?}");
}

#[test]
fn allow_short_circuits_when_no_deny_or_ask_matches() {
    let rules = vec![rule(Effect::Allow, Action::Read, "src/")];
    let v = harness_safety::perm::grammar::verdict(
        &rules,
        &Action::Read,
        &Target::Path(Path::new("src/a.rs").to_path_buf()),
    );
    assert!(matches!(v, GrammarVerdict::Allowed { .. }), "got {v:?}");
    // Outside the prefix: no opinion (engine default takes over).
    let v = harness_safety::perm::grammar::verdict(
        &rules,
        &Action::Read,
        &Target::Path(Path::new("vendor/a.rs").to_path_buf()),
    );
    assert!(matches!(v, GrammarVerdict::NoOpinion), "got {v:?}");
}

#[test]
fn command_prefix_matches_on_token_boundary() {
    let rules = vec![rule(Effect::Allow, Action::Execute, "npm")];
    let ok = harness_safety::perm::grammar::verdict(
        &rules,
        &Action::Execute,
        &Target::Command("npm i".into()),
    );
    assert!(matches!(ok, GrammarVerdict::Allowed { .. }), "got {ok:?}");
    let sneaky = harness_safety::perm::grammar::verdict(
        &rules,
        &Action::Execute,
        &Target::Command("npmx --evil".into()),
    );
    assert!(
        matches!(sneaky, GrammarVerdict::NoOpinion),
        "prefix must not bleed into the next token: {sneaky:?}"
    );
}

#[test]
fn path_prefix_matches_on_separator_boundary() {
    let rules = vec![rule(Effect::Allow, Action::Write, "src/")];
    let ok = harness_safety::perm::grammar::verdict(
        &rules,
        &Action::Write,
        &Target::Path(Path::new("src\\nested\\a.rs").to_path_buf()),
    );
    assert!(matches!(ok, GrammarVerdict::Allowed { .. }), "got {ok:?}");
    let sneaky = harness_safety::perm::grammar::verdict(
        &rules,
        &Action::Write,
        &Target::Path(Path::new("src_backdoor/a.rs").to_path_buf()),
    );
    assert!(
        matches!(sneaky, GrammarVerdict::NoOpinion),
        "got {sneaky:?}"
    );
}

#[test]
fn regex_is_opt_in_and_prefix_is_never_regex() {
    // A dot in a prefix is a literal dot, not a regex metachar.
    let rules = vec![rule(Effect::Allow, Action::Read, "docs.")];
    let literal = harness_safety::perm::grammar::verdict(
        &rules,
        &Action::Read,
        &Target::Path(Path::new("docs.a/x.md").to_path_buf()),
    );
    assert!(matches!(literal, GrammarVerdict::Allowed { .. }));
    let wildcard = harness_safety::perm::grammar::verdict(
        &rules,
        &Action::Read,
        &Target::Path(Path::new("docsX/x.md").to_path_buf()),
    );
    assert!(
        matches!(wildcard, GrammarVerdict::NoOpinion),
        "prefix '.' must not act as regex: {wildcard:?}"
    );
    // Explicit regex matches.
    let re = Rule::new(
        Effect::Allow,
        Action::Read,
        TargetPattern::regex("docs-[0-9]+/").expect("valid regex"),
        Provenance::User,
        "test",
    )
    .unwrap_or_else(|e| panic!("valid rule rejected: {e}"));
    let hit = harness_safety::perm::grammar::verdict(
        &[re],
        &Action::Read,
        &Target::Path(Path::new("docs-42/x.md").to_path_buf()),
    );
    assert!(matches!(hit, GrammarVerdict::Allowed { .. }), "got {hit:?}");
}

#[test]
fn invalid_regex_is_rejected_at_authoring() {
    let bad = TargetPattern::regex("(unclosed");
    assert!(bad.is_err(), "invalid regex must fail authoring");
}

#[test]
fn read_url_grant_never_covers_execute_url() {
    let rules = vec![rule(Effect::Allow, Action::ReadUrl, "api.example.com")];
    let fetch = harness_safety::perm::grammar::verdict(
        &rules,
        &Action::ReadUrl,
        &Target::Url(url("https://api.example.com/v1")),
    );
    assert!(
        matches!(fetch, GrammarVerdict::Allowed { .. }),
        "got {fetch:?}"
    );
    // fetch != actuate: the same URL under execute_url is NOT allowed.
    let actuate = harness_safety::perm::grammar::verdict(
        &rules,
        &Action::ExecuteUrl,
        &Target::Url(url("https://api.example.com/v1")),
    );
    assert!(
        matches!(actuate, GrammarVerdict::NoOpinion),
        "got {actuate:?}"
    );
}

#[test]
fn url_host_subdomain_prefix_is_explicit() {
    // Plain host prefix = exact host only.
    let exact = vec![rule(Effect::Allow, Action::ReadUrl, "example.com")];
    let apex = harness_safety::perm::grammar::verdict(
        &exact,
        &Action::ReadUrl,
        &Target::Url(url("https://example.com/")),
    );
    assert!(
        matches!(apex, GrammarVerdict::Allowed { .. }),
        "got {apex:?}"
    );
    let sub = harness_safety::perm::grammar::verdict(
        &exact,
        &Action::ReadUrl,
        &Target::Url(url("https://evil.example.com.attacker.io/")),
    );
    assert!(matches!(sub, GrammarVerdict::NoOpinion), "got {sub:?}");
    // "*." prefix opts into label-boundary subdomains.
    let wild = vec![rule(Effect::Allow, Action::ReadUrl, "*.example.com")];
    let kid = harness_safety::perm::grammar::verdict(
        &wild,
        &Action::ReadUrl,
        &Target::Url(url("https://api.example.com/")),
    );
    assert!(matches!(kid, GrammarVerdict::Allowed { .. }), "got {kid:?}");
    let evil_suffix = harness_safety::perm::grammar::verdict(
        &wild,
        &Action::ReadUrl,
        &Target::Url(url("https://example.com.evil.io/")),
    );
    assert!(
        matches!(evil_suffix, GrammarVerdict::NoOpinion),
        "got {evil_suffix:?}"
    );
}

#[test]
fn malformed_urls_are_typed_failures() {
    assert!(matches!(
        UrlTarget::parse("not a url"),
        Err(MalformedTarget::NotAUrl { .. })
    ));
    assert!(matches!(
        UrlTarget::parse("ftp://files.example.com/x"),
        Err(MalformedTarget::UnsupportedScheme { .. })
    ));
    assert!(matches!(
        UrlTarget::parse("http://"),
        Err(MalformedTarget::MissingHost { .. })
    ));
}

#[test]
fn url_parse_normalizes_host_case_and_defaults_ports() {
    let t = url("HTTP://API.Example.COM:8443/x");
    assert_eq!(t.host, "api.example.com");
    assert_eq!(t.port, 8443);
    assert_eq!(url("https://api.example.com/x").port, 443);
    assert_eq!(url("http://api.example.com/x").port, 80);
}

#[test]
fn tool_targets_match_tool_rules() {
    let rules = vec![rule(Effect::Allow, Action::Read, "view_")];
    // Tool-scoped rules evaluate against Tool targets with the same
    // prefix semantics (Antigravity named targets).
    let v = harness_safety::perm::grammar::verdict(
        &rules,
        &Action::Read,
        &Target::Tool("view_window".into()),
    );
    assert!(matches!(v, GrammarVerdict::Allowed { .. }), "got {v:?}");
}
