//! The known frontmatter key set. Keys outside this set are tolerated and
//! reported as sorted typed warnings (forward-compatible with future
//! agentskills.io extensions).

/// Recognized frontmatter keys; anything else becomes an `UnknownKey`
/// warning.
pub(super) const KNOWN_KEYS: [&str; 11] = [
    "name",
    "description",
    "license",
    "metadata",
    "triggers",
    "do-not-trigger-on",
    "languages",
    "version",
    "allowed-tools",
    "model-guidance",
    "context_fork",
];
