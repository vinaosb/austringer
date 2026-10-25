//! Shell-env secret scrubbing (plan todo 26): child processes never
//! inherit *KEY*/*SECRET*/*TOKEN* variables unless explicitly excepted.

/// The default deny substrings, matched case-insensitively against
/// variable names.
pub const DEFAULT_PATTERNS: [&str; 3] = ["KEY", "SECRET", "TOKEN"];

/// Variables every child still needs (inherit-core policy).
pub const CORE_VARS: [&str; 9] = [
    "PATH",
    "SYSTEMROOT",
    "TEMP",
    "TMP",
    "PATHEXT",
    "COMSPEC",
    "WINDIR",
    "HOME",
    "LANG",
];

/// Scrub configuration: default patterns plus a typed exception list of
/// variable names that survive the default match.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EnvScrub {
    /// Variable names exempt from the default KEY/SECRET/TOKEN drop.
    pub exceptions: Vec<String>,
}

impl EnvScrub {
    /// Scrub with no exceptions.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Scrub that keeps the named variables even when they match a
    /// default pattern.
    #[must_use]
    pub fn with_exceptions<I: IntoIterator<Item = &'static str>>(names: I) -> Self {
        Self {
            exceptions: names.into_iter().map(str::to_string).collect(),
        }
    }

    /// Union of two scrub layers (merging may only tighten: exceptions
    /// intersect).
    #[must_use]
    pub fn union(&self, other: &EnvScrub) -> EnvScrub {
        EnvScrub {
            exceptions: self
                .exceptions
                .iter()
                .filter(|n| other.exceptions.contains(n))
                .cloned()
                .collect(),
        }
    }

    /// True when `name` would be dropped under this configuration.
    #[must_use]
    pub fn drops(&self, name: &str) -> bool {
        if self.exceptions.iter().any(|e| e == name) {
            return false;
        }
        if CORE_VARS.contains(&name) {
            return false;
        }
        let upper = name.to_ascii_uppercase();
        DEFAULT_PATTERNS.iter().any(|p| upper.contains(p))
    }
}

/// Result of scrubbing one environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrubbedEnv {
    /// Variables that survived.
    pub kept: Vec<(String, String)>,
    /// Names that were dropped (audit surface — never values).
    pub dropped: Vec<String>,
}

/// Apply the scrub to a full environment snapshot.
#[must_use]
pub fn scrub_env(vars: &[(String, String)], scrub: &EnvScrub) -> ScrubbedEnv {
    let mut kept = Vec::with_capacity(vars.len());
    let mut dropped = Vec::new();
    for (name, value) in vars {
        if scrub.drops(name) {
            dropped.push(name.clone());
        } else {
            kept.push((name.clone(), value.clone()));
        }
    }
    ScrubbedEnv { kept, dropped }
}
