//! Network egress control (plan todo 26): typed allowlist matching plus
//! the userland filter-proxy fallback verdict layer.
//!
//! Droid AVOID encoded here: the allowlist starts EMPTY. Zero built-in
//! vendor-escape domains, ever. Matching is pure text comparison — no
//! regex engine, no evaluation of untrusted host strings.

use std::fmt;

/// One allowed egress destination.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HostPattern {
    /// Exact hostname (any port).
    Exact(String),
    /// Hostname suffix match on label boundaries: `*.example.com`
    /// matches `example.com` and `a.example.com`, nothing else.
    Wildcard(String),
    /// Host pattern scoped to a single TCP port.
    WithPort(Box<HostPattern>, u16),
}

impl HostPattern {
    /// Parse a host pattern: `host`, `*.suffix`, `host:port`.
    ///
    /// # Errors
    /// [`EgressError::MalformedPattern`] for empty input, whitespace,
    /// path separators, dot-segments, scheme prefixes, or bad ports.
    pub fn parse(text: &str) -> Result<Self, EgressError> {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed != text {
            return Err(EgressError::MalformedPattern {
                input: text.to_string(),
            });
        }
        if text.contains('/') || text.contains('\\') || text.contains(' ') {
            return Err(EgressError::MalformedPattern {
                input: text.to_string(),
            });
        }
        if text.starts_with('.') || text.contains("..") {
            return Err(EgressError::MalformedPattern {
                input: text.to_string(),
            });
        }
        let (host, port) = match text.rsplit_once(':') {
            Some((h, p)) => {
                let Ok(port) = p.parse::<u16>() else {
                    return Err(EgressError::MalformedPattern {
                        input: text.to_string(),
                    });
                };
                (h.to_string(), Some(port))
            }
            None => (text.to_string(), None),
        };
        if host.is_empty() || host.contains(':') {
            return Err(EgressError::MalformedPattern {
                input: text.to_string(),
            });
        }
        let pattern = if let Some(base) = host.strip_prefix("*.") {
            HostPattern::Wildcard(base.to_string())
        } else {
            HostPattern::Exact(host)
        };
        match port {
            Some(p) => Ok(HostPattern::WithPort(Box::new(pattern), p)),
            None => Ok(pattern),
        }
    }

    fn host_matches(&self, host: &str) -> bool {
        match self {
            HostPattern::Exact(h) => h == host,
            HostPattern::Wildcard(base) => host == base || host.ends_with(&format!(".{base}")),
            HostPattern::WithPort(inner, _) => inner.host_matches(host),
        }
    }

    /// Whether this pattern matches `host` on any port (todo-27
    /// permission-layer seam: profile ceilings + SSRF opt-outs).
    #[must_use]
    pub fn matches_host(&self, host: &str) -> bool {
        self.host_matches(host)
    }

    fn matches(&self, host: &str, port: u16) -> bool {
        match self {
            HostPattern::WithPort(inner, p) => inner.host_matches(host) && *p == port,
            other => other.host_matches(host),
        }
    }
}

impl fmt::Display for HostPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostPattern::Exact(h) => write!(f, "{h}"),
            HostPattern::Wildcard(base) => write!(f, "*.{base}"),
            HostPattern::WithPort(inner, port) => write!(f, "{inner}:{port}"),
        }
    }
}

/// The egress allowlist. Starts empty; every entry is an explicit,
/// auditable user grant.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EgressPolicy {
    /// Explicitly allowed destinations (empty by default).
    pub allowed: Vec<HostPattern>,
}

impl EgressPolicy {
    /// The empty allowlist: everything is denied.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// True when no host is allowed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.allowed.is_empty()
    }

    /// Iterate the allowlist (audit surfaces).
    pub fn iter(&self) -> impl Iterator<Item = &HostPattern> {
        self.allowed.iter()
    }

    /// Add one explicit grant.
    ///
    /// # Errors
    /// [`EgressError::MalformedPattern`] is impossible here (the pattern
    /// is already typed); kept `Result` for API stability.
    pub fn allow(&mut self, pattern: HostPattern) -> Result<(), EgressError> {
        self.allowed.push(pattern);
        Ok(())
    }

    /// Intersection used by policy merging: only grants both layers made
    /// survive (lower layers may only tighten).
    #[must_use]
    pub fn intersect(&self, other: &EgressPolicy) -> EgressPolicy {
        EgressPolicy {
            allowed: self
                .allowed
                .iter()
                .filter(|p| other.allowed.contains(p))
                .cloned()
                .collect(),
        }
    }

    /// Decide one (host, port) connection attempt.
    #[must_use]
    pub fn verdict(&self, host: &str, port: u16) -> EgressVerdict {
        if !plausible_host(host) {
            return EgressVerdict::Denied {
                host: host.to_string(),
                reason: DenyReason::MalformedHost,
            };
        }
        if self.allowed.iter().any(|p| p.matches(host, port)) {
            EgressVerdict::Allowed
        } else {
            EgressVerdict::Denied {
                host: host.to_string(),
                reason: DenyReason::NotInAllowlist,
            }
        }
    }
}

fn plausible_host(host: &str) -> bool {
    !host.is_empty() && !host.contains([' ', '/', '\\']) && !host.contains("..")
}

/// The typed verdict the filter proxy and permission layers consume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EgressVerdict {
    /// Destination is explicitly allowlisted.
    Allowed,
    /// Destination denied, with the machine-readable reason.
    Denied {
        /// The rejected destination.
        host: String,
        /// Why it was rejected.
        reason: DenyReason,
    },
}

impl EgressVerdict {
    /// True when this is any denial.
    #[must_use]
    pub fn is_denied(&self) -> bool {
        matches!(self, EgressVerdict::Denied { .. })
    }
}

/// Denial taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyReason {
    /// The host is not in the (possibly empty) allowlist.
    NotInAllowlist,
    /// The host string itself is malformed.
    MalformedHost,
}

/// Egress-layer typed errors.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EgressError {
    /// A pattern string could not be parsed.
    #[error("malformed host pattern {input:?}")]
    MalformedPattern {
        /// The rejected input.
        input: String,
    },
}
