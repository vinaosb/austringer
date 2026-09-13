//! Org-managed MCP server allowlists (Copilot pattern): an unoverridable
//! org layer of allow/deny matchers evaluated ABOVE user config and trust.
//! Deny always wins; when allow rules exist, servers must match one.
//! Matching is exact for names, full-host for URLs (real URL parsing, so
//! `mcp.acme.com.evil.com` never matches `mcp.acme.com`), and
//! program-name equality for stdio commands (path and, on Windows, case
//! ignored — `C:\Windows\System32\curl.exe` matches `curl`).

use crate::config::{ServerEntry, TransportSpec};

/// One org rule matcher. `Display` renders the rule for denials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerMatcher {
    /// Exact server-name match.
    Name(String),
    /// Exact http(s) host match (optionally `host:port`).
    UrlHost(String),
    /// Stdio program-name match (path-insensitive).
    StdioCommand(String),
}

impl std::fmt::Display for ServerMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerMatcher::Name(name) => write!(f, "name == {name:?}"),
            ServerMatcher::UrlHost(host) => write!(f, "url host == {host:?}"),
            ServerMatcher::StdioCommand(command) => write!(f, "stdio command == {command:?}"),
        }
    }
}

impl ServerMatcher {
    fn matches(&self, entry: &ServerEntry) -> bool {
        match (self, &entry.transport) {
            (ServerMatcher::Name(name), _) => *name == entry.name,
            (ServerMatcher::UrlHost(pattern), TransportSpec::Http { url }) => {
                match url::Url::parse(url) {
                    Ok(parsed) => {
                        let Some(host) = parsed.host_str() else {
                            return false;
                        };
                        match pattern.rsplit_once(':') {
                            Some((pattern_host, pattern_port)) => {
                                host == pattern_host
                                    && parsed.port().map(|port| port.to_string()).as_deref()
                                        == Some(pattern_port)
                            }
                            None => host == pattern.as_str(),
                        }
                    }
                    Err(_) => false,
                }
            }
            (ServerMatcher::StdioCommand(pattern), TransportSpec::Stdio { command, .. }) => {
                program_of(command)
                    .is_some_and(|program| names_equal(strip_exe_suffix(program), pattern.as_str()))
            }
            _ => false,
        }
    }
}

fn program_of(command: &str) -> Option<&str> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.rsplit(['/', '\\']).next()
}

/// On Windows the loader resolves bare program names to `.exe`; matching
/// treats the suffix as absent there (and only there).
#[cfg(windows)]
fn strip_exe_suffix(program: &str) -> &str {
    program.strip_suffix(".exe").unwrap_or(program)
}

#[cfg(not(windows))]
fn strip_exe_suffix(program: &str) -> &str {
    program
}

#[cfg(windows)]
fn names_equal(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

#[cfg(not(windows))]
fn names_equal(a: &str, b: &str) -> bool {
    a == b
}

/// The org layer's verdict for one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrgVerdict {
    /// The entry passes the org layer.
    Allowed,
    /// The entry is blocked; `rule` names the matched (or missing) rule.
    Denied { rule: String },
}

/// An ordered set of org allow/deny rules (deny-wins).
#[derive(Debug, Clone, Default)]
pub struct OrgAllowlist {
    deny: Vec<ServerMatcher>,
    allow: Vec<ServerMatcher>,
}

impl OrgAllowlist {
    /// An empty (absent) org layer — everything passes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an allow-by-name rule (builder).
    #[must_use]
    pub fn allow_name(mut self, name: &str) -> Self {
        self.allow.push(ServerMatcher::Name(name.to_owned()));
        self
    }

    /// Adds an allow-by-url-host rule (builder).
    #[must_use]
    pub fn allow_url_host(mut self, host: &str) -> Self {
        self.allow.push(ServerMatcher::UrlHost(host.to_owned()));
        self
    }

    /// Adds a deny-by-stdio-command rule (builder).
    #[must_use]
    pub fn deny_stdio_command(mut self, command: &str) -> Self {
        self.deny
            .push(ServerMatcher::StdioCommand(command.to_owned()));
        self
    }

    /// Adds a deny-by-name rule (builder).
    #[must_use]
    pub fn deny_name(mut self, name: &str) -> Self {
        self.deny.push(ServerMatcher::Name(name.to_owned()));
        self
    }

    /// Evaluates one entry: any deny match blocks; with allow rules
    /// present, entries must match at least one allow.
    #[must_use]
    pub fn check(&self, entry: &ServerEntry) -> OrgVerdict {
        for rule in &self.deny {
            if rule.matches(entry) {
                return OrgVerdict::Denied {
                    rule: rule.to_string(),
                };
            }
        }
        if self.allow.is_empty() {
            return OrgVerdict::Allowed;
        }
        for rule in &self.allow {
            if rule.matches(entry) {
                return OrgVerdict::Allowed;
            }
        }
        OrgVerdict::Denied {
            rule: "no org allow rule matches this server".to_owned(),
        }
    }
}
