//! Plugin bundles with managed manifests (Devin §22): skills+rules+hooks+
//! MCP+subagents ship as ONE unit. A bundle's manifest pins every required
//! plugin by content hash, lists optional plugins by name, declares glob
//! DENY patterns for plugins that must not co-exist, and names the secret
//! refs its configuration needs (values resolve through the secret source,
//! never into config files). Bundles cannot nest.

use serde::{Deserialize, Serialize};

use crate::registry::error::RegistryError;
use crate::registry::governance::GovernanceStack;
use crate::registry::packfile::{
    validate_glob, validate_hash_hex, validate_item_name, validate_secret_name,
};

/// Whether a pinned plugin must ship with the bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Requirement {
    /// The bundle installs without it fail.
    Required,
    /// Listed for the user to opt into later (frontends prompt).
    Optional,
}

/// One pinned plugin reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundlePlugin {
    /// Plugin (registry entry) name.
    pub name: String,
    /// blake3 hex the plugin's pack bytes must hash to.
    pub pin: String,
    /// Required or optional.
    pub requirement: Requirement,
}

/// The managed bundle manifest (`bundle.json` inside a bundle pack).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleManifest {
    /// Pinned plugins (required + optional-by-pin).
    pub plugins: Vec<BundlePlugin>,
    /// Optional plugin names without pins (install-on-demand later).
    #[serde(default)]
    pub optional: Vec<String>,
    /// Glob deny: plugins that must NOT be installed alongside.
    #[serde(default)]
    pub forbidden: Vec<String>,
    /// Secret names the bundle's configuration references.
    #[serde(default)]
    pub secret_refs: Vec<String>,
}

impl BundleManifest {
    /// Validates manifest text (untrusted): names must be loader-safe,
    /// pins must be 64-char hex, globs and secret names must be sane.
    pub fn validate(&self) -> Result<(), RegistryError> {
        for plugin in &self.plugins {
            validate_item_name(&plugin.name)?;
            validate_hash_hex(&plugin.pin)?;
        }
        for name in &self.optional {
            validate_item_name(name)?;
        }
        for secret in &self.secret_refs {
            validate_secret_name(secret)?;
        }
        for glob in &self.forbidden {
            validate_glob(glob)?;
        }
        Ok(())
    }

    /// The required plugin names.
    pub fn required_names(&self) -> Vec<&str> {
        self.plugins
            .iter()
            .filter(|p| p.requirement == Requirement::Required)
            .map(|p| p.name.as_str())
            .collect()
    }
}

/// The outcome of a bundle pre-install check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleCheck {
    /// Required plugins that still need installing (not yet present).
    pub to_install: Vec<BundlePlugin>,
    /// Optional plugins available in the store but not auto-installed.
    pub optional_available: Vec<String>,
}

/// Pre-install bundle checks: forbidden-glob conflicts against the bundle's
/// own plugins AND the already-installed set, plus governance for every
/// plugin the bundle would install. SHA-pin and store-resolution checks
/// happen at fetch time in the install pipeline.
pub fn check_bundle(
    bundle: &BundleManifest,
    installed_names: &[String],
    governance: &GovernanceStack,
) -> Result<BundleCheck, RegistryError> {
    let candidates: Vec<&str> = bundle
        .plugins
        .iter()
        .map(|p| p.name.as_str())
        .chain(bundle.optional.iter().map(String::as_str))
        .collect();
    for pattern in &bundle.forbidden {
        for candidate in candidates
            .iter()
            .copied()
            .chain(installed_names.iter().map(String::as_str))
        {
            if crate::registry::governance::glob_match(pattern, candidate) {
                return Err(RegistryError::ForbiddenPluginConflict {
                    plugin: candidate.to_string(),
                    pattern: pattern.clone(),
                });
            }
        }
    }
    let mut to_install = Vec::new();
    for plugin in &bundle.plugins {
        if plugin.requirement != Requirement::Required {
            continue;
        }
        governance.permit(&plugin.name)?;
        if !installed_names.contains(&plugin.name) {
            to_install.push(plugin.clone());
        }
    }
    for name in &bundle.optional {
        governance.permit(name)?;
    }
    Ok(BundleCheck {
        to_install,
        optional_available: bundle.optional.clone(),
    })
}
