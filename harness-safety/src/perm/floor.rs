//! Admin floor + SSRF guard (Cursor pattern, plan todo 27): an
//! org/admin layer of deny rules and egress-SSRF policy that
//! user/project/session configuration CANNOT weaken — the floor's
//! denies always win (the provenance extension of todo-26's
//! deny-always-wins policy merge), and private-network egress targets
//! are blocked BY DEFAULT at rule-authoring time. Opting out of the
//! SSRF guard requires an explicit admin grant.

use std::net::{IpAddr, Ipv4Addr};

use crate::egress::HostPattern;
use crate::perm::grammar::{Action, Effect, GrammarVerdict, Provenance, Rule, Target};
use crate::perm::reasons::SsrfKind;

/// The un-weakenable admin layer.
#[derive(Debug, Clone)]
pub struct AdminFloor {
    deny_rules: Vec<Rule>,
    ssrf_opt_out: Vec<HostPattern>,
}

impl AdminFloor {
    /// Builds the floor; every rule must be an admin-provenance deny.
    ///
    /// # Errors
    /// [`FloorError`] naming the first rule that is not an
    /// admin-provenance deny.
    pub fn new(deny_rules: Vec<Rule>, ssrf_opt_out: Vec<HostPattern>) -> Result<Self, FloorError> {
        for rule in &deny_rules {
            if rule.effect != Effect::Deny {
                return Err(FloorError::NotADenyRule);
            }
            if rule.provenance != Provenance::Admin {
                return Err(FloorError::NotAdminProvenance);
            }
        }
        Ok(Self {
            deny_rules,
            ssrf_opt_out,
        })
    }

    /// An empty floor (no admin denies, SSRF guard fully on).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            deny_rules: Vec::new(),
            ssrf_opt_out: Vec::new(),
        }
    }

    /// The floor's deny rules (evaluated above every other layer).
    #[must_use]
    pub fn deny_rules(&self) -> &[Rule] {
        &self.deny_rules
    }

    /// Deny-always-wins evaluation: admin denies run first, then the
    /// caller's rules under the standard grammar precedence. A user
    /// allow can never surface because a matching admin deny always
    /// outranks it.
    #[must_use]
    pub fn enforce(&self, rules: &[Rule], action: &Action, target: &Target) -> GrammarVerdict {
        let mut combined = self.deny_rules.clone();
        combined.extend_from_slice(rules);
        crate::perm::grammar::verdict(&combined, action, target)
    }
}

/// Floor construction failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FloorError {
    /// A floor rule was not a deny.
    #[error("admin floor rules must deny")]
    NotADenyRule,
    /// A floor rule did not carry admin provenance.
    #[error("admin floor rules must carry admin provenance")]
    NotAdminProvenance,
}

/// Classifies `host` against the SSRF categories; `None` means the
/// host is outside the default-blocked set. DNS-rebinding (a public
/// hostname resolving to a private IP) is out of scope for pure text
/// authoring — the egress proxy verifies at connect time.
#[must_use]
pub fn ssrf_violation(host: &str, floor: Option<&AdminFloor>) -> Option<SsrfKind> {
    let kind = classify_host(host)?;
    if let Some(floor) = floor
        && floor.ssrf_opt_out.iter().any(|p| p.matches_host(host))
    {
        return None;
    }
    Some(kind)
}

fn classify_host(host: &str) -> Option<SsrfKind> {
    let lower = host.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") {
        return Some(SsrfKind::Localhost);
    }
    classify_ip(parse_ip(&lower)?)
}

fn parse_ip(text: &str) -> Option<IpAddr> {
    if let Ok(ip) = text.parse::<IpAddr>() {
        return Some(ip);
    }
    // URL specs allow decimal-integer IPv4 hosts (http://2130706433/).
    let as_u32: u32 = text.parse().ok()?;
    Some(IpAddr::V4(Ipv4Addr::from(as_u32)))
}

fn classify_ip(ip: IpAddr) -> Option<SsrfKind> {
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_loopback() {
                Some(SsrfKind::Localhost)
            } else if v4.is_private() {
                Some(SsrfKind::Rfc1918)
            } else if v4.is_link_local() {
                Some(SsrfKind::LinkLocalMetadata)
            } else if v4.is_unspecified() {
                Some(SsrfKind::Unspecified)
            } else {
                None
            }
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return classify_ip(IpAddr::V4(v4));
            }
            if v6.is_loopback() {
                Some(SsrfKind::Localhost)
            } else if v6.is_unique_local() {
                Some(SsrfKind::Ipv6Ula)
            } else if v6.is_unicast_link_local() {
                Some(SsrfKind::Ipv6LinkLocal)
            } else if v6.is_unspecified() {
                Some(SsrfKind::Unspecified)
            } else {
                None
            }
        }
    }
}

/// Authors one egress grant with the SSRF guard applied by default.
///
/// # Errors
/// [`EgressAuthoringError::SsrfBlocked`] unless an admin floor
/// explicitly opted this host out; malformed patterns are typed.
pub fn author_egress(
    pattern: &str,
    floor: Option<&AdminFloor>,
) -> Result<HostPattern, EgressAuthoringError> {
    let parsed = HostPattern::parse(pattern).map_err(|_| EgressAuthoringError::Malformed {
        input: pattern.to_string(),
    })?;
    if let Some(kind) = ssrf_violation(pattern_host(&parsed), floor) {
        return Err(EgressAuthoringError::SsrfBlocked { kind });
    }
    Ok(parsed)
}

fn pattern_host(pattern: &HostPattern) -> &str {
    match pattern {
        HostPattern::Exact(h) | HostPattern::Wildcard(h) => h,
        HostPattern::WithPort(inner, _) => pattern_host(inner),
    }
}

/// Egress rule-authoring failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EgressAuthoringError {
    /// The pattern string did not parse.
    #[error("malformed egress pattern {input:?}")]
    Malformed {
        /// The rejected input.
        input: String,
    },
    /// The pattern names a default-blocked SSRF target.
    #[error("ssrf guard blocked egress rule: {kind:?}")]
    SsrfBlocked {
        /// Which category the host fell into.
        kind: SsrfKind,
    },
}
