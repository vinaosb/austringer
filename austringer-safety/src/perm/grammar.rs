//! Action(target) permission grammar (Antigravity pattern, plan todo
//! 27): permissions are ACTIONS over NAMED TARGETS with precedence
//! Deny > Ask > Allow (strictest match wins), prefix-default matching,
//! and OPT-IN regex (regex off by default). `read_url` and
//! `execute_url` are distinct actions: a fetch grant never actuates.

use std::path::PathBuf;

/// A permissioned action. `ReadUrl` fetches; `ExecuteUrl` actuates
/// (fetch-then-execute) — the split means a read grant never executes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Read a file.
    Read,
    /// Write a file (new content).
    Write,
    /// Edit a file (targeted change).
    Edit,
    /// Execute a command.
    Execute,
    /// Fetch a URL (fetch ≠ actuate).
    ReadUrl,
    /// Fetch and actuate a URL (pipe-to-shell class).
    ExecuteUrl,
}

impl Action {
    /// Actions whose target is a URL.
    #[must_use]
    pub fn is_url_action(self) -> bool {
        matches!(self, Self::ReadUrl | Self::ExecuteUrl)
    }
}

/// A parsed fetch target. Hosts are lowercased; IPv6 brackets are
/// stripped so IP-literal checks parse cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlTarget {
    /// Lowercase scheme (`http`/`https` only).
    pub scheme: String,
    /// Lowercase host, brackets stripped for IPv6.
    pub host: String,
    /// Explicit or scheme-default port.
    pub port: u16,
}

impl UrlTarget {
    /// Parses and canonicalizes an http(s) URL.
    ///
    /// # Errors
    /// [`MalformedTarget`] naming the failure; non-http(s) schemes are
    /// refused because fetch semantics are undefined for them.
    pub fn parse(raw: &str) -> Result<Self, MalformedTarget> {
        let parsed = url::Url::parse(raw).map_err(|e| match e {
            url::ParseError::EmptyHost => MalformedTarget::MissingHost {
                input: raw.to_string(),
            },
            _ => MalformedTarget::NotAUrl {
                input: raw.to_string(),
            },
        })?;
        let scheme = parsed.scheme().to_ascii_lowercase();
        if !matches!(scheme.as_str(), "http" | "https") {
            return Err(MalformedTarget::UnsupportedScheme { scheme });
        }
        let host = parsed
            .host_str()
            .map(|h| h.trim_matches(['[', ']']).to_ascii_lowercase())
            .filter(|h| !h.is_empty())
            .ok_or_else(|| MalformedTarget::MissingHost {
                input: raw.to_string(),
            })?;
        let port = parsed.port_or_known_default().unwrap_or(80);
        Ok(Self { scheme, host, port })
    }
}

/// Typed malformed-target failures (fail closed everywhere).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MalformedTarget {
    /// The string is not a URL at all.
    #[error("not a url: {input:?}")]
    NotAUrl {
        /// The rejected input.
        input: String,
    },
    /// Only http/https have defined fetch semantics here.
    #[error("unsupported url scheme: {scheme:?}")]
    UnsupportedScheme {
        /// The refused scheme.
        scheme: String,
    },
    /// The URL carried no host.
    #[error("url has no host: {input:?}")]
    MissingHost {
        /// The rejected input.
        input: String,
    },
}

/// What a rule constrains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A filesystem path.
    Path(PathBuf),
    /// A command string (compound allowed; engine segments it).
    Command(String),
    /// An http(s) URL.
    Url(UrlTarget),
    /// A tool name (rules over the call surface itself).
    Tool(String),
}

impl Target {
    fn key(&self) -> String {
        match self {
            Self::Path(p) => p.to_string_lossy().replace('\\', "/"),
            Self::Command(c) => c.clone(),
            Self::Url(u) => format!("{}://{}:{}", u.scheme, u.host, u.port),
            Self::Tool(t) => t.clone(),
        }
    }
}

/// Target constraint. Prefix is the DEFAULT mode and never regexes;
/// regex is explicit opt-in and validated at authoring time.
#[derive(Debug, Clone)]
pub enum TargetPattern {
    /// Word/label-boundary prefix (default mode).
    Prefix(String),
    /// Opt-in regular expression over the target's key string.
    Regex(regex::Regex),
}

impl TargetPattern {
    /// The default prefix pattern.
    #[must_use]
    pub fn prefix(pattern: &str) -> Self {
        Self::Prefix(pattern.to_string())
    }

    /// Opt-in regex; invalid patterns fail authoring.
    ///
    /// # Errors
    /// [`RuleError::InvalidRegex`] when the pattern does not compile.
    pub fn regex(pattern: &str) -> Result<Self, RuleError> {
        regex::Regex::new(pattern)
            .map(Self::Regex)
            .map_err(|e| RuleError::InvalidRegex {
                pattern: pattern.to_string(),
                detail: e.to_string(),
            })
    }

    /// Whether this pattern matches `target`.
    #[must_use]
    pub fn matches(&self, target: &Target) -> bool {
        match self {
            Self::Prefix(p) => prefix_matches(p, target),
            Self::Regex(re) => re.is_match(&target.key()),
        }
    }
}

fn prefix_matches(pattern: &str, target: &Target) -> bool {
    match target {
        Target::Path(path) => boundary_prefix(
            &path.to_string_lossy().replace('\\', "/"),
            &pattern.replace('\\', "/"),
            '/',
        ),
        Target::Command(text) => boundary_prefix(text.trim(), pattern, ' '),
        Target::Url(url) => url_prefix_matches(pattern, url),
        Target::Tool(name) => boundary_prefix(name, pattern, ':'),
    }
}

fn boundary_prefix(value: &str, pattern: &str, boundary: char) -> bool {
    if pattern.is_empty() {
        return false;
    }
    let Some(rest) = value.strip_prefix(pattern) else {
        return false;
    };
    // A pattern ending in a non-alphanumeric char (".", "/", "_", ...)
    // declares its own boundary; an alphanumeric tail demands the
    // boundary char or end-of-value, so `src` never covers
    // `src_backdoor`.
    rest.is_empty()
        || rest.starts_with(boundary)
        || pattern.ends_with(boundary)
        || pattern.chars().last().is_some_and(|c| !c.is_alphanumeric())
}

fn url_prefix_matches(pattern: &str, url: &UrlTarget) -> bool {
    if let Some(base) = pattern.strip_prefix("*.") {
        url.host == base || url.host.ends_with(&format!(".{base}"))
    } else {
        url.host == pattern
    }
}

/// Where a rule comes from; admin rules cannot be weakened by lower
/// layers (Cursor admin floor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// Organization/admin floor — strongest.
    Admin,
    /// Project-level configuration.
    Project,
    /// User-level configuration.
    User,
    /// Session-scoped.
    Session,
}

/// The effect a rule has on a matching request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Matching requests are allowed.
    Allow,
    /// Matching requests require approval.
    Ask,
    /// Matching requests are refused.
    Deny,
}

/// One grammar rule: effect + action + target constraint + provenance.
#[derive(Debug, Clone)]
pub struct Rule {
    /// What the rule does on match.
    pub effect: Effect,
    /// Which action it constrains.
    pub action: Action,
    /// Target constraint.
    pub pattern: TargetPattern,
    /// Configuration layer the rule came from.
    pub provenance: Provenance,
    /// Human-facing rule label surfaced in reasons.
    pub origin: String,
}

impl Rule {
    /// Builds a rule; rejects empty patterns and origins.
    ///
    /// # Errors
    /// [`RuleError`] on empty pattern/origin.
    pub fn new(
        effect: Effect,
        action: Action,
        pattern: TargetPattern,
        provenance: Provenance,
        origin: &str,
    ) -> Result<Self, RuleError> {
        if origin.trim().is_empty() {
            return Err(RuleError::EmptyOrigin);
        }
        if matches!(&pattern, TargetPattern::Prefix(p) if p.trim().is_empty()) {
            return Err(RuleError::EmptyPattern);
        }
        Ok(Self {
            effect,
            action,
            pattern,
            provenance,
            origin: origin.to_string(),
        })
    }
}

/// Rule authoring failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    /// The pattern was empty.
    #[error("rule pattern must not be empty")]
    EmptyPattern,
    /// The origin label was empty.
    #[error("rule origin must not be empty")]
    EmptyOrigin,
    /// An opt-in regex did not compile.
    #[error("invalid regex {pattern:?}: {detail}")]
    InvalidRegex {
        /// The rejected pattern.
        pattern: String,
        /// The compiler's diagnosis.
        detail: String,
    },
}

/// The grammar layer's judgment over one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrammarVerdict {
    /// A deny rule matched; `admin` marks admin-provenance denies.
    Denied {
        /// Matching rule's origin label.
        origin: String,
        /// Whether the deny comes from the admin floor.
        admin: bool,
    },
    /// An ask rule matched.
    Asked {
        /// Matching rule's origin label.
        origin: String,
    },
    /// An allow rule matched.
    Allowed {
        /// Matching rule's origin label.
        origin: String,
    },
    /// No rule matched; lower layers decide.
    NoOpinion,
}

/// Evaluates the grammar with strictest-wins precedence:
/// any deny > any ask > any allow > no opinion.
#[must_use]
pub fn verdict(rules: &[Rule], action: &Action, target: &Target) -> GrammarVerdict {
    let mut deny = None;
    let mut ask = None;
    let mut allow = None;
    for rule in rules {
        if rule.action != *action || !rule.pattern.matches(target) {
            continue;
        }
        match rule.effect {
            Effect::Deny if deny.is_none() => deny = Some(rule),
            Effect::Ask if ask.is_none() => ask = Some(rule),
            Effect::Allow if allow.is_none() => allow = Some(rule),
            _ => {}
        }
    }
    if let Some(rule) = deny {
        return GrammarVerdict::Denied {
            origin: rule.origin.clone(),
            admin: rule.provenance == Provenance::Admin,
        };
    }
    if let Some(rule) = ask {
        return GrammarVerdict::Asked {
            origin: rule.origin.clone(),
        };
    }
    if let Some(rule) = allow {
        return GrammarVerdict::Allowed {
            origin: rule.origin.clone(),
        };
    }
    GrammarVerdict::NoOpinion
}
