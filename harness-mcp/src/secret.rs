//! The SecretStore seam (todo 29): `{{secret:NAME}}` placeholders in MCP
//! config files resolve through an injectable store at INSTANTIATION
//! time — secrets never live in config files, never enter scan results,
//! and a missing secret fails closed with the NAME (never a value).

use std::collections::BTreeMap;

/// Resolves secret values by name (OS keyring, env broker, …).
pub trait SecretStore: Send + Sync {
    /// The value for `name`, if this store knows it.
    fn resolve(&self, name: &str) -> Option<String>;
}

/// An in-memory [`SecretStore`] (tests and ephemeral sessions).
#[derive(Debug, Clone, Default)]
pub struct MapSecretStore {
    values: BTreeMap<String, String>,
}

impl MapSecretStore {
    #[must_use]
    pub fn new(values: BTreeMap<String, String>) -> Self {
        Self { values }
    }
}

impl SecretStore for MapSecretStore {
    fn resolve(&self, name: &str) -> Option<String> {
        self.values.get(name).cloned()
    }
}

/// Why substitution failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretError {
    /// The named secret is unknown to the store; the config keeps its
    /// placeholder.
    #[error("missing secret: {0}")]
    Missing(String),
}

/// Replaces every `{{secret:NAME}}` in `text`. Missing names fail closed
/// (`Err` naming the missing secret; no partial text leaks values).
pub fn substitute_secrets(text: &str, store: &dyn SecretStore) -> Result<String, SecretError> {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{secret:") {
        output.push_str(&rest[..start]);
        let after = &rest[start + "{{secret:".len()..];
        match after.find("}}") {
            Some(end) => {
                let name = &after[..end];
                let value = store
                    .resolve(name)
                    .ok_or_else(|| SecretError::Missing(name.to_owned()))?;
                output.push_str(&value);
                rest = &after[end + 2..];
            }
            None => {
                // Unterminated placeholder: keep it verbatim (a template
                // may legitimately contain other braces).
                output.push_str(&rest[start..start + "{{secret:".len()]);
                rest = after;
            }
        }
    }
    output.push_str(rest);
    Ok(output)
}
