//! The per-command safety gate for `execute_command` (todo 4,
//! ecosystem alignment): the generalization of the
//! austringer-media/src/tool executor pattern onto the command lane.
//!
//! Where each guarantee is enforced (honesty rule, media parity):
//!
//! * **Sandbox policy** — the per-command profile is declared in
//!   austringer-safety's [`SandboxPolicy`] vocabulary and fail-closed
//!   validated; an unsanctionable policy is a typed block carrying a
//!   [`ViolationEvent`] audit record and nothing spawns. The OS
//!   backend is resolved through
//!   [`platform_backend`](austringer_safety::backends::platform_backend)
//!   (the same seam doctor probes) and named in every result.
//! * **Egress guard** — the command text is verdicted by
//!   [`guard_command`](austringer_safety::egress::guard_command) against
//!   the policy's egress allowlist (empty by default: deny-all). A
//!   network-shaped segment is blocked BEFORE any spawn; the guard is
//!   a string-pattern verdict layer and never opens a socket.
//! * **Env scrub** — the policy's [`EnvScrub`] rides the
//!   [`ExecSpec`](austringer_exec::spec::ExecSpec) and is applied at the
//!   exec spawn seam (KEY/SECRET/TOKEN never reach the child).
//! * **Requested-vs-enforced** — every result carries the
//!   [`IsolationReport`] diff. The PTY spawn lane enforces the
//!   userland layers (egress guard + env scrub), NOT kernel fs
//!   confinement — those gaps are NAMED, never papered over
//!   (harness-rs honesty: isolation is never claimed where it was not
//!   established). The kernel lane (Windows restricted-token
//!   AppContainer et al.) stays exercised by austringer-safety's own
//!   runtime escape suites.

use austringer_safety::backends::platform_backend;
use austringer_safety::egress::{self, CommandEgress, DenyReason, EgressPolicy};
use austringer_safety::policy::decl::{SandboxPolicy, SandboxTier, ViolationEvent, ViolationKind};
use austringer_safety::report::{EnforcedSummary, Isolation, IsolationReport, NetworkIsolation};
use austringer_safety::scrub::EnvScrub;

/// A passed gate: the policy that will ride the spec plus the honest
/// requested-vs-enforced report for the PTY lane.
pub(crate) struct SafetyVerdict {
    pub(crate) policy: SandboxPolicy,
    pub(crate) report: IsolationReport,
    pub(crate) backend: &'static str,
}

/// The gate's decision: run with the verdict, or the typed block. The
/// full policy rides behind a Box so the Block path stays small.
pub(crate) enum SafetyDecision {
    Pass(Box<SafetyVerdict>),
    Block {
        violation: ViolationEvent,
        backend: &'static str,
    },
}

/// Judge one command line. Ordering is cheapest-and-most-decisive
/// first, and every failure is a denial (fail-closed: an error or
/// unresolvable case in the safety layer must DENY, never allow).
pub(crate) fn gate(command: &str) -> SafetyDecision {
    let backend = platform_backend().name();
    let policy = command_policy();
    // Media `sandbox_gate` parity: an unsanctionable policy blocks the
    // run before anything spawns.
    if let Err(error) = policy.validate() {
        return SafetyDecision::Block {
            violation: ViolationEvent::new(
                ViolationKind::WriteOutsideRoot,
                &format!("sandbox policy rejected: {error}"),
            ),
            backend,
        };
    }
    match egress::guard_command(command, &policy.egress) {
        CommandEgress::NoEgress | CommandEgress::Allowed => {}
        CommandEgress::Denied {
            segment,
            host,
            reason,
        } => {
            return SafetyDecision::Block {
                violation: ViolationEvent::new(
                    ViolationKind::EgressDenied,
                    &format!(
                        "egress to {} denied ({}) — the egress allowlist starts empty, \
                         so no network-shaped command runs; segment: `{segment}`",
                        host.as_deref().unwrap_or("<unresolved destination>"),
                        deny_text(reason),
                    ),
                ),
                backend,
            };
        }
    }
    let report = IsolationReport::diff(
        &policy,
        EnforcedSummary {
            fs_write: Isolation::None,
            fs_read: Isolation::None,
            network: NetworkIsolation::UserlandProxy,
            env_scrubbed: true,
        },
    );
    SafetyDecision::Pass(Box::new(SafetyVerdict {
        policy,
        report,
        backend,
    }))
}

/// The per-command policy: read-only tier (writes belong to
/// `edit_lines`), deny-all egress (the allowlist starts empty — zero
/// vendor-escape domains), default env scrub.
fn command_policy() -> SandboxPolicy {
    SandboxPolicy {
        tier: SandboxTier::Readonly,
        writable_roots: Vec::new(),
        readable_roots: Vec::new(),
        protected_paths: Vec::new(),
        deny_globs: Vec::new(),
        egress: EgressPolicy::none(),
        env_scrub: EnvScrub::default(),
    }
}

fn deny_text(reason: DenyReason) -> &'static str {
    match reason {
        DenyReason::NotInAllowlist => "not in allowlist",
        DenyReason::MalformedHost => "destination unresolvable — fail closed",
    }
}

/// The result suffix for an allowed run: the requested-vs-enforced
/// statement, with every gap named.
pub(crate) fn render_report(verdict: &SafetyVerdict) -> String {
    let egress = if verdict.policy.egress.is_empty() {
        "deny-all".to_owned()
    } else {
        format!("allowlist({} hosts)", verdict.policy.egress.allowed.len())
    };
    let gaps = if verdict.report.gaps.is_empty() {
        "none".to_owned()
    } else {
        verdict
            .report
            .gaps
            .iter()
            .map(|gap| gap.capability.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "[safety] requested: tier=readonly egress={egress} env_scrub=on | enforced: \
         egress=userland-guard env_scrub=at-spawn fs=unconfined | backend={} | gaps: {gaps}",
        verdict.backend,
    )
}

/// The result for a blocked run: the typed reason plus the
/// requested-vs-enforced statement (the block IS the enforcement).
pub(crate) fn render_block(violation: &ViolationEvent, backend: &str) -> String {
    format!(
        "blocked by sandbox policy: {}\n[safety] requested: tier=readonly egress=deny-all | \
         enforced: blocked-before-spawn (deny honored) | backend={backend}",
        violation.detail,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_blocks_curl_shaped_command_with_typed_violation() {
        match gate("curl -s https://egress-blocked.invalid/lease") {
            SafetyDecision::Block { violation, .. } => {
                assert_eq!(violation.kind, ViolationKind::EgressDenied);
                assert!(
                    violation.detail.contains("egress-blocked.invalid"),
                    "the violation names the denied host: {}",
                    violation.detail
                );
            }
            SafetyDecision::Pass(_) => panic!("curl under deny-all egress must block"),
        }
    }

    #[test]
    fn gate_passes_plain_commands_with_honest_gaps() {
        let SafetyDecision::Pass(verdict) = gate("echo hello") else {
            panic!("echo must pass the gate");
        };
        let gaps: Vec<&str> = verdict
            .report
            .gaps
            .iter()
            .map(|gap| gap.capability.as_str())
            .collect();
        assert!(
            gaps.contains(&"fs_write") && gaps.contains(&"fs_read"),
            "the PTY lane must name its fs gaps honestly: {gaps:?}"
        );
        assert_eq!(
            verdict.report.enforced.network,
            NetworkIsolation::UserlandProxy
        );
        assert!(verdict.report.enforced.env_scrubbed);
    }

    #[test]
    fn renders_carry_requested_vs_enforced_and_backend() {
        let SafetyDecision::Pass(verdict) = gate("echo hello") else {
            panic!("echo must pass the gate");
        };
        let report = render_report(&verdict);
        assert!(report.contains("requested:"));
        assert!(report.contains("enforced:"));
        assert!(report.contains("backend="));
        assert!(report.contains("gaps: fs_write, fs_read"));

        let violation = ViolationEvent::new(ViolationKind::EgressDenied, "test");
        let block = render_block(&violation, "windows-appcontainer");
        assert!(block.contains("blocked by sandbox policy"));
        assert!(block.contains("requested:"));
        assert!(block.contains("enforced: blocked-before-spawn"));
        assert!(block.contains("backend=windows-appcontainer"));
    }
}
