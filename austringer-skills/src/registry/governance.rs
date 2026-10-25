//! Plugin governance (Devin §22): authority layers Org > Team > Project >
//! User; the HIGHEST authority that mentions a plugin decides, and at
//! equal authority deny wins — so a forbid-'*' rule is a lockdown that
//! lower (and, at the same authority, permissive) rules cannot weaken,
//! mirroring the todo-27 admin-floor merge discipline.

use serde::{Deserialize, Serialize};

use crate::registry::error::RegistryError;

/// Authority layers; ordering is significance (Org is highest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Authority {
    /// Organization-wide policy (highest).
    Org,
    /// Team policy.
    Team,
    /// Project policy.
    Project,
    /// Individual user policy (lowest).
    User,
}

/// The decision for one plugin name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginDecision {
    /// Not mentioned anywhere: allowed by default (items are declarative
    /// data; integrity is hash/signature-gated at install).
    Allowed,
    /// Explicitly optional per governance.
    Optional,
    /// Forbidden — install must refuse.
    Forbidden {
        /// Authority whose rule fired.
        by: Authority,
        /// The matching forbidden glob.
        pattern: String,
    },
    /// Explicitly required per governance.
    Required {
        /// Authority whose rule fired.
        by: Authority,
    },
}

/// One layer's policy. Globs use `*` wildcards (e.g. `telemetry-*`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernancePolicy {
    /// The authority this policy speaks for.
    pub authority: Authority,
    /// Glob patterns plugins must NOT match.
    #[serde(default)]
    pub forbidden: Vec<String>,
    /// Names/globs plugins that are required.
    #[serde(default)]
    pub required: Vec<String>,
    /// Names/globs plugins that are explicitly allowed-but-optional.
    #[serde(default)]
    pub optional: Vec<String>,
}

impl GovernancePolicy {
    /// A policy that only forbids.
    pub fn forbidden(authority: Authority, globs: Vec<String>) -> Self {
        Self {
            authority,
            forbidden: globs,
            required: Vec::new(),
            optional: Vec::new(),
        }
    }

    /// A policy that only requires.
    pub fn required(authority: Authority, names: Vec<String>) -> Self {
        Self {
            authority,
            forbidden: Vec::new(),
            required: names,
            optional: Vec::new(),
        }
    }
}

/// Ordered policy stack. `new` sorts highest authority first and, within
/// one authority, puts forbidden-lists before permissive ones (deterministic
/// same-authority deny-first evaluation).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GovernanceStack {
    policies: Vec<GovernancePolicy>,
}

impl GovernanceStack {
    /// From an unordered set of policies.
    pub fn new(mut policies: Vec<GovernancePolicy>) -> Self {
        policies.sort_by(|a, b| {
            a.authority.cmp(&b.authority).then_with(|| {
                usize::from(a.forbidden.is_empty()).cmp(&usize::from(b.forbidden.is_empty()))
            })
        });
        Self { policies }
    }

    /// A permissive stack (no layers).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Decides for one plugin: the first (highest) policy that mentions
    /// it in any list wins; forbidden is checked before required before
    /// optional within a policy.
    pub fn decide(&self, plugin: &str) -> PluginDecision {
        for policy in &self.policies {
            if let Some(pattern) = policy.forbidden.iter().find(|g| glob_match(g, plugin)) {
                return PluginDecision::Forbidden {
                    by: policy.authority,
                    pattern: pattern.clone(),
                };
            }
            if policy.required.iter().any(|g| glob_match(g, plugin)) {
                return PluginDecision::Required {
                    by: policy.authority,
                };
            }
            if policy.optional.iter().any(|g| glob_match(g, plugin)) {
                return PluginDecision::Optional;
            }
        }
        PluginDecision::Allowed
    }

    /// Refuses the install with the typed governance error unless the
    /// decision is permissive. A forbid-'*' match surfaces as [`RegistryError::Lockdown`].
    pub fn permit(&self, plugin: &str) -> Result<(), RegistryError> {
        match self.decide(plugin) {
            PluginDecision::Allowed
            | PluginDecision::Optional
            | PluginDecision::Required { .. } => Ok(()),
            PluginDecision::Forbidden { by, pattern } => {
                if pattern != "*" {
                    return Err(RegistryError::GovernanceDeny {
                        plugin: plugin.to_string(),
                        by,
                        pattern,
                    });
                }
                // forbid-'*': lockdown, unless the same authority also lists
                // the plugin required/optional — a policy conflict that stays
                // denied but is reported as a plugin-specific deny.
                let explicitly_mentioned = self.policies.iter().any(|p| {
                    p.authority == by
                        && (p.required.iter().any(|g| glob_match(g, plugin))
                            || p.optional.iter().any(|g| glob_match(g, plugin)))
                });
                if explicitly_mentioned {
                    Err(RegistryError::GovernanceDeny {
                        plugin: plugin.to_string(),
                        by,
                        pattern,
                    })
                } else {
                    Err(RegistryError::Lockdown { by })
                }
            }
        }
    }
}

/// `*`-wildcard glob match (lexical, no regex — same discipline as
/// `crate::skill::tool_pattern_matches`).
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let mut rest = name;
    let mut parts = pattern.split('*');
    let Some(first) = parts.next() else {
        return false;
    };
    if !rest.starts_with(first) {
        return false;
    }
    rest = &rest[first.len()..];
    for part in parts {
        if part.is_empty() {
            continue;
        }
        match rest.find(part) {
            Some(idx) => rest = &rest[idx + part.len()..],
            None => return false,
        }
    }
    true
}
