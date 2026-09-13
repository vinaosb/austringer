//! Per-install consent (Gemini CLI pattern): every skill install surfaces
//! a plain-language INSTALL_WARNING generated from its declared tools and
//! triggers BEFORE first activation; the skill stays inert until a
//! [`ConsentRecord`] is recorded against the same warning.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::skill::SkillMeta;

/// Plain-language description of what an installed skill can do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallWarning {
    /// Human-readable warning text (shown before first activation).
    pub text: String,
}

/// Recorded consent for one skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsentRecord {
    /// Skill the consent applies to.
    pub skill: String,
    /// The warning text the user saw and accepted.
    pub warning: String,
}

/// Consent ledger: skills not present here are inert for routing
/// (see [`crate::routing::route_with_consent`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsentGate {
    granted: BTreeMap<String, ConsentRecord>,
}

impl ConsentGate {
    /// An empty (most restrictive) gate: nothing is consented.
    pub fn new() -> Self {
        Self::default()
    }

    /// Generates the INSTALL_WARNING for a skill from its declared tools
    /// and triggers — never from its body (bodies are untrusted data).
    pub fn install_warning(meta: &SkillMeta) -> InstallWarning {
        let tools = if meta.allowed_tools.is_empty() {
            "no external tools".to_string()
        } else {
            meta.allowed_tools.join(", ")
        };
        let triggers: Vec<String> = meta
            .triggers
            .iter()
            .filter(|t| !t.starts_with('/'))
            .take(5)
            .cloned()
            .collect();
        let trigger_text = if triggers.is_empty() {
            format!("the /{} command", meta.name)
        } else {
            triggers.join("; ")
        };
        InstallWarning {
            text: format!(
                "INSTALL WARNING for skill '{}': once active it can use these tools: [{}], and it activates automatically when your prompt matches its triggers ({}). Review both lists before enabling; decline to keep the skill inert.",
                meta.name, tools, trigger_text
            ),
        }
    }

    /// Records consent against a warning; the skill becomes routable.
    pub fn grant(&mut self, meta: &SkillMeta, warning: InstallWarning) -> ConsentRecord {
        let record = ConsentRecord {
            skill: meta.name.to_string(),
            warning: warning.text,
        };
        self.granted.insert(record.skill.clone(), record.clone());
        record
    }

    /// Whether the named skill has recorded consent.
    pub fn is_granted(&self, name: &str) -> bool {
        self.granted.contains_key(name)
    }

    /// All granted records (for persistence/audit).
    pub fn records(&self) -> impl Iterator<Item = &ConsentRecord> {
        self.granted.values()
    }
}
