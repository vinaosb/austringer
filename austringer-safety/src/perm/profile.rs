//! Security profiles (Devin pattern, plan todo 27): profiles bind at
//! a hierarchy org → automation → session, LOWER LEVELS ONLY TIGHTEN
//! (intersect), and mid-session grants CLAMP to the effective
//! intersection — granting wider than the profile yields the clamped
//! outcome plus a typed notice, never a widened permission.

use serde::{Deserialize, Serialize};

use crate::egress::HostPattern;
use crate::perm::grammar::Action;

/// One profile level's ceiling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityProfile {
    /// Hosts fetchable under url actions; `None` = unrestricted at
    /// this level (an outer restriction still applies after
    /// intersection).
    pub read_url_hosts: Option<Vec<HostPattern>>,
    /// Programs this level forbids outright (unions upward).
    pub denied_programs: Vec<String>,
    /// Actions this level pins to ask-always (unions upward).
    pub require_ask: Vec<Action>,
}

impl SecurityProfile {
    /// The unrestricted profile (leaf default).
    #[must_use]
    pub fn unrestricted() -> Self {
        Self {
            read_url_hosts: None,
            denied_programs: Vec::new(),
            require_ask: Vec::new(),
        }
    }

    /// Whether this profile pins `action` to ask-always.
    #[must_use]
    pub fn requires_ask(&self, action: &Action) -> bool {
        self.require_ask.contains(action)
    }

    /// Clamps a mid-session grant to this effective profile.
    ///
    /// `target` is the URL host for url actions and the resolved
    /// program name for Execute.
    #[must_use]
    pub fn check_grant(&self, action: &Action, target: &str) -> ClampOutcome {
        if self.requires_ask(action) {
            return ClampOutcome::Clamped {
                notice: "profile pins this action to ask".into(),
            };
        }
        let outside_hosts = match (&self.read_url_hosts, action) {
            (Some(hosts), Action::ReadUrl | Action::ExecuteUrl) => {
                !hosts.iter().any(|p| p.matches_host(target))
            }
            _ => false,
        };
        if outside_hosts {
            return ClampOutcome::Clamped {
                notice: "grant clamped to the profile's host intersection".into(),
            };
        }
        if *action == Action::Execute
            && self
                .denied_programs
                .iter()
                .any(|p| p.eq_ignore_ascii_case(target))
        {
            return ClampOutcome::Clamped {
                notice: "grant clamped: program denied by profile".into(),
            };
        }
        ClampOutcome::WithinProfile
    }
}

/// Grant-clamping outcome with the typed notice the decision
/// surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClampOutcome {
    /// The grant fits inside the effective profile.
    WithinProfile,
    /// The grant exceeds the profile and was clamped down.
    Clamped {
        /// User-facing explanation of the clamp.
        notice: String,
    },
}

/// Intersects three hierarchy levels; every dimension only tightens.
#[must_use]
pub fn intersect(
    org: &SecurityProfile,
    automation: &SecurityProfile,
    session: &SecurityProfile,
) -> SecurityProfile {
    let midway = tighten(org, automation);
    tighten(&midway, session)
}

fn tighten(base: &SecurityProfile, overlay: &SecurityProfile) -> SecurityProfile {
    let read_url_hosts = match (&base.read_url_hosts, &overlay.read_url_hosts) {
        (None, None) => None,
        (Some(_), None) => base.read_url_hosts.clone(),
        (None, Some(_)) => overlay.read_url_hosts.clone(),
        (Some(a), Some(b)) => Some(
            a.iter()
                .filter(|p| b.contains(p))
                .cloned()
                .collect::<Vec<_>>(),
        ),
    };
    SecurityProfile {
        read_url_hosts,
        denied_programs: union_of(&base.denied_programs, &overlay.denied_programs),
        require_ask: union_of(&base.require_ask, &overlay.require_ask),
    }
}

fn union_of<T: PartialEq + Clone>(base: &[T], overlay: &[T]) -> Vec<T> {
    let mut merged = base.to_vec();
    for item in overlay {
        if !merged.contains(item) {
            merged.push(item.clone());
        }
    }
    merged
}
