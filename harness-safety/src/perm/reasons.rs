//! Typed permission reasons (Warp pattern, plan todo 27): every
//! allow/ask/deny carries a machine-readable reason surfaced in the
//! decision, so the user sees WHY an action ran — not just that it did.

use serde::{Deserialize, Serialize};

/// Why a rule matched the URL's SSRF guard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SsrfKind {
    /// `localhost` / `*.localhost` / `127.0.0.0/8` (v4-mapped included).
    Localhost,
    /// RFC 1918 private ranges (10/8, 172.16/12, 192.168/16).
    Rfc1918,
    /// 169.254.0.0/16 incl. the 169.254.169.254 metadata endpoint.
    LinkLocalMetadata,
    /// IPv6 unique-local (fc00::/7).
    Ipv6Ula,
    /// IPv6 link-local (fe80::/10).
    Ipv6LinkLocal,
    /// The unspecified address (0.0.0.0 / ::).
    Unspecified,
}

/// The machine-readable reason attached to every permission decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionReason {
    /// An explicit allow rule matched.
    ExplicitlyAllowlisted,
    /// An explicit deny rule matched.
    ExplicitlyDenied,
    /// An ask rule matched.
    AskRule,
    /// A stored user grant (T8 `a` answer) covered the request.
    StoredGrant,
    /// A stored grant decided by the agent classifier (T8 seam).
    AgentDecided,
    /// Read inside granted roots needs no approval (read-only stance).
    ReadOnly,
    /// Bypass/yolo mode converted an ask into an auto-approval; the
    /// floor still denies (run-to-completion).
    RunToCompletion,
    /// The target is secret-looking; approval redacts the transcript.
    ContainsRedaction,
    /// Program resolution matched the hard blocklist (Droid).
    BlocklistResolved,
    /// Not every segment of a compound command was independently
    /// allowed (Roo substitution guard).
    SubstitutionGuard,
    /// The admin floor refused; user/project config cannot weaken it.
    AdminFloor,
    /// A command deny-prefix matched.
    CommandPrefixDenied,
    /// A grant was clamped down to the security-profile intersection.
    ProfileClamp,
    /// The profile pins this action to ask-always.
    ProfileCeiling,
    /// The deobfuscated dangerous-command floor fired (survives yolo).
    HardlineFloor,
    /// Auto-approvals hit the max-requests circuit breaker.
    CircuitBreakerTripped {
        /// Approvals observed inside the window.
        observed: usize,
        /// Configured maximum per window.
        max: usize,
    },
    /// Program resolution failed; fail closed.
    ResolutionFailed,
    /// The egress destination is not in the permissions-derived
    /// allowlist.
    NotInEgressAllowlist,
    /// The URL's host hits the SSRF guard.
    SsrfGuard {
        /// Which blocked category the host fell into.
        kind: SsrfKind,
    },
    /// Write to a sandbox-protected path (`.git`, settings, ...).
    ProtectedPath,
    /// Write outside every writable root.
    WriteOutsideRoot,
    /// The target could not be parsed (typed; fail closed).
    MalformedTarget,
    /// The command could not be segmented (typed; fail closed).
    MalformedCommand,
    /// No rule or grant covered the request; headless default asks.
    FailClosed,
}

impl std::fmt::Display for PermissionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = serde_json::to_string(self).unwrap_or_else(|_| "\"unknown\"".into());
        let trimmed = name.trim_matches('"');
        write!(f, "{trimmed}")
    }
}
