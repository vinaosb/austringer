//! Egress control tests (todo 26): typed HostPattern matching, empty
//! allowlist, no vendor escapes, verdict reasons.

#![cfg(test)]

use austringer_safety::egress::{
    CommandEgress, DenyReason, EgressPolicy, EgressVerdict, HostPattern, guard_command,
};

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

#[test]
fn guard_blocks_curl_under_empty_allowlist_naming_the_host() {
    match guard_command(
        "curl -s https://egress-blocked.invalid/lease",
        &EgressPolicy::none(),
    ) {
        CommandEgress::Denied {
            host: Some(host),
            reason: DenyReason::NotInAllowlist,
            ..
        } => assert_eq!(host, "egress-blocked.invalid"),
        other => panic!("expected NotInAllowlist denial naming the host, got {other:?}"),
    }
}

#[test]
fn guard_blocks_unqualified_and_path_qualified_egress_programs() {
    let policy = EgressPolicy::none();
    for command in [
        "curl https://x.invalid/",
        "curl.exe https://x.invalid/",
        "C:\\Windows\\System32\\curl.exe -s https://x.invalid/",
        "/usr/bin/curl https://x.invalid/",
        "wget http://x.invalid/f",
        "ping x.invalid",
        "ssh git@x.invalid",
        "nc x.invalid 443",
    ] {
        assert!(
            matches!(
                guard_command(command, &policy),
                CommandEgress::Denied { .. }
            ),
            "{command} must be denied under the empty allowlist"
        );
    }
}

#[test]
fn guard_blocks_egress_program_with_no_parsable_destination() {
    // flags only: network intent, hidden destination — fail closed.
    assert!(matches!(
        guard_command("curl --version", &EgressPolicy::none()),
        CommandEgress::Denied {
            reason: DenyReason::MalformedHost,
            ..
        }
    ));
}

#[test]
fn guard_blocks_unsegmentable_commands_fail_closed() {
    // unbalanced quoting: the guard cannot judge what it cannot segment.
    assert!(matches!(
        guard_command(
            "echo \"unclosed && curl https://x.invalid/",
            &EgressPolicy::none()
        ),
        CommandEgress::Denied { .. }
    ));
}

#[test]
fn guard_judges_every_compound_segment_independently() {
    // An allowed-looking prefix must never cover an appended egress
    // attempt (the `npm i && curl evil` lesson).
    let policy = EgressPolicy::none();
    for command in [
        "echo ok && curl -s https://x.invalid/",
        "echo ok ; wget http://x.invalid/f",
        "echo ok | curl https://x.invalid/",
        "curl $(echo https://x.invalid/)",
    ] {
        assert!(
            matches!(
                guard_command(command, &policy),
                CommandEgress::Denied { .. }
            ),
            "{command} must be denied: the egress segment stands alone"
        );
    }
}

#[test]
fn guard_passes_non_egress_commands_without_verdicts() {
    let policy = EgressPolicy::none();
    for command in [
        "echo hello",
        "cargo nextest run -p austringer-safety",
        "git status --short",
        "dir /b",
    ] {
        assert_eq!(
            guard_command(command, &policy),
            CommandEgress::NoEgress,
            "{command} attempts no egress"
        );
    }
}

#[test]
fn guard_allows_egress_to_explicitly_granted_hosts() {
    let mut policy = EgressPolicy::none();
    policy
        .allow(HostPattern::Exact("deps.invalid".into()))
        .expect("allow");
    assert_eq!(
        guard_command("curl -s https://deps.invalid/index", &policy),
        CommandEgress::Allowed
    );
    assert!(
        matches!(
            guard_command("curl -s https://other.invalid/index", &policy),
            CommandEgress::Denied { .. }
        ),
        "the grant is exact: a sibling host stays denied"
    );
}

#[test]
fn guard_strips_userinfo_scheme_and_port_to_reach_the_host() {
    let mut policy = EgressPolicy::none();
    policy
        .allow(HostPattern::parse("mirror.invalid:8443").expect("parse"))
        .expect("allow");
    for command in [
        "curl https://mirror.invalid:8443/pkg",
        "wget ftp://user:pass@mirror.invalid:8443/pkg",
    ] {
        assert_eq!(
            guard_command(command, &policy),
            CommandEgress::Allowed,
            "{command} must reduce to the granted host:port"
        );
    }
}
