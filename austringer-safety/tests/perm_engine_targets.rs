//! Unified-engine target-class tests (todo 27): URL rows (SSRF,
//! egress round trip, read_url vs execute_url), grant rows, the
//! wrapper-peel blocklist catch, and profile clamping.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use austringer_safety::egress::HostPattern;
use austringer_safety::perm::audit::MemoryAuditSink;
use austringer_safety::perm::breaker::{BreakerConfig, FakeClock};
use austringer_safety::perm::commands::CommandRules;
use austringer_safety::perm::engine::{
    BypassMode, EngineConfig, GrantOrigin, MemoryGrants, PermissionEngine,
};
use austringer_safety::perm::floor::AdminFloor;
use austringer_safety::perm::grammar::{
    Action, Effect, Provenance, Rule, Target, TargetPattern, UrlTarget,
};
use austringer_safety::perm::profile::SecurityProfile;
use austringer_safety::perm::reasons::{PermissionReason, SsrfKind};
use austringer_safety::perm::resolve::PathResolver;
use austringer_safety::policy::decl::SandboxPolicy;

fn engine_with(
    rules: Vec<Rule>,
    grants: MemoryGrants,
    profile: SecurityProfile,
    dir: &std::path::Path,
) -> PermissionEngine {
    let sandbox = SandboxPolicy::for_worktree(dir, &dir.join("s.toml"));
    let config = EngineConfig {
        rules,
        commands: CommandRules::default(),
        admin: AdminFloor::new(vec![], vec![]).expect("floor"),
        profile,
        breaker: BreakerConfig::disabled(),
        bypass: BypassMode::Off,
        ..EngineConfig::with_sandbox(sandbox)
    };
    PermissionEngine::with_seams(
        config,
        Arc::new(grants),
        Box::new(PathResolver::new(vec![dir.to_path_buf()])),
        Arc::new(MemoryAuditSink::new()),
        Arc::new(FakeClock::new()),
    )
    .expect("engine")
}

fn allow(action: Action, pattern: &str) -> Rule {
    Rule::new(
        Effect::Allow,
        action,
        TargetPattern::prefix(pattern),
        Provenance::User,
        "target",
    )
    .expect("rule")
}

fn expect(
    engine: &mut PermissionEngine,
    action: Action,
    target: Target,
    effect: Effect,
    reason: PermissionReason,
) {
    let decision = engine.decide(action, target);
    assert_eq!(decision.effect, effect, "reason was {:?}", decision.reason);
    assert_eq!(decision.reason, reason, "effect was {:?}", decision.effect);
    assert!(
        !decision.layers.is_empty(),
        "decision must carry its resolution path"
    );
}

#[test]
fn matrix_url_rows() {
    let dir = tempfile::tempdir().expect("tmp");
    let mut engine = engine_with(
        vec![allow(Action::ReadUrl, "api.example.com")],
        MemoryGrants::new(),
        SecurityProfile::unrestricted(),
        dir.path(),
    );
    let fetch = |raw: &str| Target::Url(UrlTarget::parse(raw).expect("url"));
    expect(
        &mut engine,
        Action::ReadUrl,
        fetch("https://api.example.com/v1"),
        Effect::Allow,
        PermissionReason::ExplicitlyAllowlisted,
    );
    expect(
        &mut engine,
        Action::ReadUrl,
        fetch("https://evil.io/x"),
        Effect::Deny,
        PermissionReason::NotInEgressAllowlist,
    );
    expect(
        &mut engine,
        Action::ReadUrl,
        fetch("http://169.254.169.254/latest/meta-data"),
        Effect::Deny,
        PermissionReason::SsrfGuard {
            kind: SsrfKind::LinkLocalMetadata,
        },
    );
    expect(
        &mut engine,
        Action::ReadUrl,
        fetch("http://localhost:8080/"),
        Effect::Deny,
        PermissionReason::SsrfGuard {
            kind: SsrfKind::Localhost,
        },
    );
    expect(
        &mut engine,
        Action::ExecuteUrl,
        fetch("https://api.example.com/v1"),
        Effect::Ask,
        PermissionReason::FailClosed,
    );
}

#[test]
fn matrix_grant_rows() {
    let outside = PathBuf::from("C:/other/notes.txt");
    let grants = MemoryGrants::new();
    grants.grant(
        Action::Read,
        &Target::Path(outside.clone()),
        GrantOrigin::User,
    );
    let agent_target = PathBuf::from("C:/other/report.md");
    grants.grant(
        Action::Read,
        &Target::Path(agent_target.clone()),
        GrantOrigin::Agent,
    );
    let dir = tempfile::tempdir().expect("tmp");
    let mut engine = engine_with(vec![], grants, SecurityProfile::unrestricted(), dir.path());
    expect(
        &mut engine,
        Action::Read,
        Target::Path(outside),
        Effect::Allow,
        PermissionReason::StoredGrant,
    );
    expect(
        &mut engine,
        Action::Read,
        Target::Path(agent_target),
        Effect::Allow,
        PermissionReason::AgentDecided,
    );
}

#[test]
fn program_resolution_blocklist_catches_wrapper() {
    let dir = tempfile::tempdir().expect("tmp");
    fs::write(
        dir.path().join("shim.cmd"),
        "@echo off\r\n\"C:\\Windows\\System32\\format.com\" %*\r\n",
    )
    .expect("fixture");
    // unix twin of the .cmd wrapper: shebang peel to `format`
    fs::write(dir.path().join("shim"), "#!/bin/sh\nexec format \"$@\"\n").expect("fixture");
    let sandbox = SandboxPolicy::for_worktree(dir.path(), &dir.path().join("s.toml"));
    let config = EngineConfig {
        rules: vec![allow(Action::Execute, "shim")],
        commands: CommandRules::new(&["shim"], &[]).expect("rules"),
        admin: AdminFloor::new(vec![], vec![]).expect("floor"),
        profile: SecurityProfile::unrestricted(),
        breaker: BreakerConfig::disabled(),
        bypass: BypassMode::Off,
        ..EngineConfig::with_sandbox(sandbox)
    };
    let mut engine = PermissionEngine::with_seams(
        config,
        Arc::new(MemoryGrants::new()),
        Box::new(PathResolver::new(vec![dir.path().to_path_buf()])),
        Arc::new(MemoryAuditSink::new()),
        Arc::new(FakeClock::new()),
    )
    .expect("engine");
    expect(
        &mut engine,
        Action::Execute,
        Target::Command("shim wipe".into()),
        Effect::Deny,
        PermissionReason::BlocklistResolved,
    );
}

#[test]
fn grant_clamped_to_profile_surfaces_notice() {
    // A read_url grant compiled into egress lets the FETCH layer
    // pass; the execute_url grant above it is wider than the profile
    // → clamped + typed notice (Devin).
    let dir = tempfile::tempdir().expect("tmp");
    let grants = MemoryGrants::new();
    let url = UrlTarget::parse("https://api.example.com/x").expect("url");
    grants.grant(
        Action::ExecuteUrl,
        &Target::Url(url.clone()),
        GrantOrigin::User,
    );
    let profile = SecurityProfile {
        read_url_hosts: Some(vec![HostPattern::parse("*.internal.corp").expect("p")]),
        denied_programs: vec![],
        require_ask: vec![],
    };
    let mut engine = engine_with(
        vec![allow(Action::ReadUrl, "*.example.com")],
        grants,
        profile,
        dir.path(),
    );
    let decision = engine.decide(Action::ExecuteUrl, Target::Url(url));
    assert_eq!(decision.effect, Effect::Ask);
    assert_eq!(decision.reason, PermissionReason::ProfileClamp);
    assert!(decision.notice.is_some(), "clamped grants carry a notice");
}

#[test]
fn permissions_derived_egress_round_trip_through_engine() {
    // One config source drives both layers: the engine's compiled
    // egress allowlist IS the permission layer's read_url grants.
    let dir = tempfile::tempdir().expect("tmp");
    let rules = vec![
        allow(Action::ReadUrl, "api.example.com"),
        allow(Action::ReadUrl, "*.cdn.example.com"),
    ];
    let engine = engine_with(
        rules.clone(),
        MemoryGrants::new(),
        SecurityProfile::unrestricted(),
        dir.path(),
    );
    let compiled = austringer_safety::perm::compile::compile_egress(&rules).expect("compile");
    for host in ["api.example.com", "assets.cdn.example.com"] {
        assert_eq!(
            engine.egress().verdict(host, 443),
            compiled.verdict(host, 443)
        );
    }
    for host in ["evil.example.com", "api.example.com.evil.io"] {
        assert_eq!(
            engine.egress().verdict(host, 443),
            compiled.verdict(host, 443)
        );
        assert!(engine.egress().verdict(host, 443).is_denied());
    }
}
