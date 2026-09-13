//! Honest isolation reporting (plan todo 26, harness-rs pattern): every
//! sandboxed run reports REQUESTED versus ENFORCED, with every gap named.
//! A run never claims isolation the backend did not actually provide.

use crate::policy::decl::SandboxPolicy;

/// harness-rs `Isolation` vocabulary for one capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Isolation {
    /// Nothing enforced.
    None,
    /// Filesystem changes confined (write confinement).
    Changes,
    /// Whole-process isolation (separate container/token principal).
    Process,
}

/// How network isolation is actually enforced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkIsolation {
    /// Kernel/netfilter-level enforcement (AppContainer no-capability,
    /// firewall, seccomp).
    Kernel,
    /// Userland HTTP/SOCKS allowlist proxy only (filter-proxy fallback).
    UserlandProxy,
    /// No network enforcement at all.
    None,
}

/// What the backend actually established, capability by capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EnforcedSummary {
    /// Filesystem write confinement level.
    pub fs_write: Isolation,
    /// Filesystem read confinement level.
    pub fs_read: Isolation,
    /// Network enforcement class.
    pub network: NetworkIsolation,
    /// Whether the child env was scrubbed.
    pub env_scrubbed: bool,
}

/// One named gap between requested and enforced.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsolationGap {
    /// The capability short name (`fs_write`, `network`, `env_scrub`…).
    pub capability: String,
    /// Human-auditable explanation of what is missing.
    pub detail: String,
}

/// The requested-vs-enforced diff report for one sandboxed run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IsolationReport {
    /// The policy that was asked for.
    pub requested: SandboxPolicy,
    /// What the backend enforced.
    pub enforced: EnforcedSummary,
    /// Every named divergence between the two.
    pub gaps: Vec<IsolationGap>,
}

impl IsolationReport {
    /// Diff a requested policy against what a backend enforced. Any
    /// capability the policy implies but the backend did not deliver
    /// becomes a named gap.
    #[must_use]
    pub fn diff(requested: &SandboxPolicy, enforced: EnforcedSummary) -> Self {
        let mut gaps = Vec::new();
        if enforced.fs_write == Isolation::None {
            gaps.push(IsolationGap {
                capability: "fs_write".into(),
                detail: "filesystem write confinement not enforced".into(),
            });
        }
        if enforced.fs_read == Isolation::None {
            gaps.push(IsolationGap {
                capability: "fs_read".into(),
                detail: "filesystem read confinement not enforced".into(),
            });
        }
        match enforced.network {
            NetworkIsolation::None => gaps.push(IsolationGap {
                capability: "network".into(),
                detail: "no network isolation enforced".into(),
            }),
            NetworkIsolation::UserlandProxy => {
                if !requested.egress.is_empty() {
                    gaps.push(IsolationGap {
                        capability: "network".into(),
                        detail:
                            "kernel network isolation unavailable; userland allowlist proxy only"
                                .into(),
                    });
                }
            }
            NetworkIsolation::Kernel => {}
        }
        if !enforced.env_scrubbed {
            gaps.push(IsolationGap {
                capability: "env_scrub".into(),
                detail: "child environment was not scrubbed".into(),
            });
        }
        Self {
            requested: requested.clone(),
            enforced,
            gaps,
        }
    }

    /// True only when every capability runs at whole-process isolation.
    #[must_use]
    pub fn claims_process_isolation(&self) -> bool {
        self.enforced.fs_write == Isolation::Process
            && self.enforced.fs_read == Isolation::Process
            && self.enforced.network == NetworkIsolation::Kernel
    }
}
