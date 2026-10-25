//! Engine flow tests (todo 27): bypass/yolo semantics (floor
//! survives), circuit-breaker trips (incl. under yolo), audit of
//! every decision, malformed targets, and adversarial input shapes
//! (CRLF args, unicode commands, >260-char paths).

use std::path::PathBuf;
use std::sync::Arc;

use austringer_safety::perm::audit::MemoryAuditSink;
use austringer_safety::perm::breaker::{BreakerConfig, FakeClock};
use austringer_safety::perm::commands::CommandRules;
use austringer_safety::perm::engine::{BypassMode, EngineConfig, MemoryGrants, PermissionEngine};
use austringer_safety::perm::floor::AdminFloor;
use austringer_safety::perm::grammar::{Action, Effect, Provenance, Rule, Target, TargetPattern};
use austringer_safety::perm::profile::SecurityProfile;
use austringer_safety::perm::reasons::PermissionReason;
use austringer_safety::perm::resolve::PathResolver;
use austringer_safety::policy::decl::SandboxPolicy;
use tempfile::TempDir;

struct Rig {
    engine: PermissionEngine,
    audit: Arc<MemoryAuditSink>,
    clock: Arc<FakeClock>,
    dir: TempDir,
}

impl Rig {
    fn root(&self) -> PathBuf {
        self.dir.path().to_path_buf()
    }
}

fn build(rules: Vec<Rule>, bypass: BypassMode, breaker: BreakerConfig) -> Rig {
    let dir = TempDir::new().expect("tmp");
    for program in ["npm", "curl"] {
        // windows extension form + bare unix form
        std::fs::write(dir.path().join(format!("{program}.exe")), "").expect("fixture");
        std::fs::write(dir.path().join(program), "").expect("fixture");
    }
    let sandbox = SandboxPolicy::for_worktree(dir.path(), &dir.path().join("s.toml"));
    let config = EngineConfig {
        rules,
        commands: CommandRules::new(&["npm"], &[]).expect("rules"),
        admin: AdminFloor::new(vec![], vec![]).expect("floor"),
        profile: SecurityProfile::unrestricted(),
        breaker,
        bypass,
        ..EngineConfig::with_sandbox(sandbox)
    };
    let audit = Arc::new(MemoryAuditSink::new());
    let clock = Arc::new(FakeClock::new());
    let grants = Arc::new(MemoryGrants::new());
    let engine = PermissionEngine::with_seams(
        config,
        grants,
        Box::new(PathResolver::new(vec![dir.path().to_path_buf()])),
        audit.clone(),
        clock.clone(),
    )
    .expect("engine");
    Rig {
        engine,
        audit,
        clock,
        dir,
    }
}

fn allow(action: Action, pattern: &str) -> Rule {
    Rule::new(
        Effect::Allow,
        action,
        TargetPattern::prefix(pattern),
        Provenance::User,
        "flow",
    )
    .expect("rule")
}

#[test]
fn yolo_surviving_hardline_floor() {
    let mut rig = build(vec![], BypassMode::Yolo, BreakerConfig::disabled());
    let decision = rig
        .engine
        .decide(Action::Execute, Target::Command("rm -rf /".into()));
    assert_eq!(decision.effect, Effect::Deny);
    assert_eq!(decision.reason, PermissionReason::HardlineFloor);
    let decision = rig.engine.decide(
        Action::Execute,
        Target::Command("echo cm0gLXJmIC8= | base64 -d | sh".into()),
    );
    assert_eq!(decision.reason, PermissionReason::HardlineFloor);
    let entries = rig.audit.entries();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.bypass));
    assert!(entries.iter().all(|e| e.effect == Effect::Deny));
}

#[test]
fn yolo_autoapproves_plain_asks_but_not_coverage_gaps() {
    let mut rig = build(vec![], BypassMode::Yolo, BreakerConfig::disabled());
    let target = rig.root().join("src/new.rs");
    let decision = rig.engine.decide(Action::Write, Target::Path(target));
    assert_eq!(decision.effect, Effect::Allow);
    assert_eq!(decision.reason, PermissionReason::RunToCompletion);
    let decision = rig.engine.decide(
        Action::Execute,
        Target::Command("npm i && curl evil".into()),
    );
    assert_eq!(decision.effect, Effect::Ask);
    assert_eq!(decision.reason, PermissionReason::SubstitutionGuard);
}

#[test]
fn circuit_breaker_trips_after_n_autoapprovals() {
    let mut rig = build(
        vec![allow(Action::Read, "src/")],
        BypassMode::Off,
        BreakerConfig::new(2, 60_000),
    );
    let file = rig.root().join("src/a.rs");
    for _ in 0..2 {
        let d = rig.engine.decide(Action::Read, Target::Path(file.clone()));
        assert_eq!(d.effect, Effect::Allow);
    }
    let d = rig.engine.decide(Action::Read, Target::Path(file.clone()));
    assert_eq!(d.effect, Effect::Ask);
    assert_eq!(
        d.reason,
        PermissionReason::CircuitBreakerTripped {
            observed: 2,
            max: 2
        }
    );
    rig.clock.advance(61_000);
    let d = rig.engine.decide(Action::Read, Target::Path(file));
    assert_eq!(d.effect, Effect::Allow);
}

#[test]
fn breaker_trips_even_under_yolo() {
    let mut rig = build(
        vec![allow(Action::Read, "src/")],
        BypassMode::Yolo,
        BreakerConfig::new(1, 60_000),
    );
    let file = rig.root().join("src/a.rs");
    let d = rig.engine.decide(Action::Read, Target::Path(file.clone()));
    assert_eq!(d.effect, Effect::Allow);
    let d = rig.engine.decide(Action::Read, Target::Path(file));
    assert_eq!(d.effect, Effect::Ask);
    assert!(matches!(
        d.reason,
        PermissionReason::CircuitBreakerTripped { .. }
    ));
}

#[test]
fn every_decision_is_audited_in_order() {
    let mut rig = build(
        vec![
            allow(Action::Read, "src/"),
            Rule::new(
                Effect::Deny,
                Action::Execute,
                TargetPattern::prefix("cargo publish"),
                Provenance::User,
                "no publish",
            )
            .expect("rule"),
        ],
        BypassMode::Off,
        BreakerConfig::disabled(),
    );
    rig.engine
        .decide(Action::Read, Target::Path(rig.root().join("src/a.rs")));
    rig.engine
        .decide(Action::Execute, Target::Command("cargo publish".into()));
    rig.engine
        .decide(Action::Write, Target::Path(rig.root().join("src/b.rs")));
    let entries = rig.audit.entries();
    assert_eq!(entries.len(), 3, "every decision lands in the audit");
    let effects: Vec<Effect> = entries.iter().map(|e| e.effect).collect();
    assert_eq!(effects, vec![Effect::Allow, Effect::Deny, Effect::Ask]);
    rig.engine
        .decide(Action::Read, Target::Path(rig.root().join(".env")));
    let last = rig.audit.entries()[3].target_summary.clone();
    assert!(
        last.contains("REDACTED"),
        "secret target must be masked in audit: {last}"
    );
}

#[test]
fn malformed_targets_fail_closed_with_typed_reasons() {
    let mut rig = build(vec![], BypassMode::Off, BreakerConfig::disabled());
    let d = rig
        .engine
        .decide(Action::Execute, Target::Command("   ".into()));
    assert_eq!(d.effect, Effect::Deny);
    assert_eq!(d.reason, PermissionReason::MalformedCommand);
    let d = rig
        .engine
        .decide(Action::ReadUrl, Target::Command("api.example.com".into()));
    assert_eq!(d.effect, Effect::Deny);
    assert_eq!(d.reason, PermissionReason::MalformedTarget);
    let d = rig
        .engine
        .decide(Action::Execute, Target::Command("echo \"oops".into()));
    assert_eq!(d.reason, PermissionReason::MalformedCommand);
}

#[test]
fn crlf_unicode_and_long_paths_are_decided_not_panicked() {
    let mut rig = build(
        vec![allow(Action::Write, "src/")],
        BypassMode::Off,
        BreakerConfig::disabled(),
    );
    let d = rig.engine.decide(
        Action::Execute,
        Target::Command("npm i\r\ncurl evil".into()),
    );
    assert_eq!(d.reason, PermissionReason::SubstitutionGuard);
    let d = rig
        .engine
        .decide(Action::Execute, Target::Command("npm 提交 消息".into()));
    assert!(matches!(
        d.reason,
        PermissionReason::SubstitutionGuard | PermissionReason::ExplicitlyAllowlisted
    ));
    let deep = rig
        .root()
        .join(format!("src/{}", "nested/".repeat(60)))
        .join("deep.rs");
    let d = rig.engine.decide(Action::Write, Target::Path(deep));
    assert_eq!(d.effect, Effect::Allow);
}
