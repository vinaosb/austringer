//! Honest isolation reporting tests (todo 26): requested-vs-enforced diffs
//! with named gaps - never claim isolation the backend did not provide.

#![cfg(test)]

use austringer_safety::policy::decl::SandboxPolicy;
use austringer_safety::report::{EnforcedSummary, Isolation, IsolationReport, NetworkIsolation};
use std::path::Path;

#[test]
fn full_enforcement_report_shows_no_gaps() {
    let root = Path::new("/repo");
    let policy = SandboxPolicy::for_worktree(root, &root.join("settings.toml"));
    let enforced = EnforcedSummary {
        fs_write: Isolation::Changes,
        fs_read: Isolation::Changes,
        network: NetworkIsolation::UserlandProxy,
        env_scrubbed: true,
    };
    let report = IsolationReport::diff(&policy, enforced);
    assert_eq!(report.gaps.len(), 0, "expected enforced == requested");
    assert!(
        !report.claims_process_isolation(),
        "Changes-level fs isolation must never read as Process isolation"
    );
}

#[test]
fn proxy_only_network_is_a_named_gap() {
    let root = Path::new("/repo");
    let mut policy = SandboxPolicy::for_worktree(root, &root.join("settings.toml"));
    policy
        .egress
        .allow(austringer_safety::egress::HostPattern::Exact(
            "api.example.com".into(),
        ))
        .expect("allow");
    let enforced = EnforcedSummary {
        fs_write: Isolation::Changes,
        fs_read: Isolation::Changes,
        network: NetworkIsolation::UserlandProxy,
        env_scrubbed: true,
    };
    let report = IsolationReport::diff(&policy, enforced);
    assert!(
        report
            .gaps
            .iter()
            .any(|gap| gap.capability.contains("network")),
        "userland-proxy-only enforcement must name the network gap, got {:?}",
        report.gaps
    );
}

#[test]
fn unenforced_fs_write_is_a_named_gap_never_silence() {
    let root = Path::new("/repo");
    let policy = SandboxPolicy::for_worktree(root, &root.join("settings.toml"));
    let enforced = EnforcedSummary {
        fs_write: Isolation::None,
        fs_read: Isolation::Changes,
        network: NetworkIsolation::None,
        env_scrubbed: true,
    };
    let report = IsolationReport::diff(&policy, enforced);
    let caps: Vec<&str> = report.gaps.iter().map(|g| g.capability.as_str()).collect();
    assert!(caps.contains(&"fs_write"));
    assert!(caps.contains(&"network"));
}

#[test]
fn report_never_claims_process_isolation_for_changes_only() {
    let enforced = EnforcedSummary {
        fs_write: Isolation::Changes,
        fs_read: Isolation::Changes,
        network: NetworkIsolation::UserlandProxy,
        env_scrubbed: true,
    };
    let root = Path::new("/repo");
    let policy = SandboxPolicy::for_worktree(root, &root.join("settings.toml"));
    let report = IsolationReport::diff(&policy, enforced);
    assert!(
        !report.claims_process_isolation(),
        "Changes-level fs isolation must never read as Process isolation"
    );
}

#[test]
fn env_scrub_gap_is_named_when_unenforced() {
    let root = Path::new("/repo");
    let policy = SandboxPolicy::for_worktree(root, &root.join("settings.toml"));
    let enforced = EnforcedSummary {
        fs_write: Isolation::Changes,
        fs_read: Isolation::Changes,
        network: NetworkIsolation::None,
        env_scrubbed: false,
    };
    let report = IsolationReport::diff(&policy, enforced);
    assert!(report.gaps.iter().any(|gap| gap.capability == "env_scrub"));
}

#[test]
fn report_serializes_stably_for_audit() {
    let root = Path::new("/repo");
    let policy = SandboxPolicy::for_worktree(root, &root.join("settings.toml"));
    let enforced = EnforcedSummary {
        fs_write: Isolation::Changes,
        fs_read: Isolation::Changes,
        network: NetworkIsolation::None,
        env_scrubbed: true,
    };
    let report = IsolationReport::diff(&policy, enforced);
    let json = serde_json::to_string(&report).expect("serde");
    let reparsed: IsolationReport = serde_json::from_str(&json).expect("round trip");
    assert_eq!(reparsed.gaps.len(), report.gaps.len());
}
