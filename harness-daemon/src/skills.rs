//! Skills runtime (plan todo 6): loads skill packs at startup through
//! the harness-skills `SkillLoader` and governs EVERY activation path
//! with the trust/consent gates — explicit `/skill` commands AND
//! trigger-driven routing. Draft/quarantined packs are listed on
//! `GET /packs` but never activatable; refusals are typed and carry
//! the quarantine tier.
//!
//! Gate composition (none may be bypassed):
//! * **Tier gate** — the daemon-wide `pack-tiers.json` vocabulary
//!   ([`crate::automations::PackTier`]); an unknown pack is `Draft`
//!   (fail-closed). Draft packs refuse every activation path.
//! * **Consent gate** — trigger-driven activation additionally
//!   requires a recorded [`ConsentGate`] consent (loaded from
//!   `skill-consent.json`; absent file = nothing consented). An
//!   approved-but-unconsented skill stays inert: it never activates,
//!   it just does not fire. Explicit `/skill` invocation is the
//!   user's own consent — the tier gate still binds it.
//! * **Loader lock** — the index loads with `LockMode::Record`
//!   (content-hash pinning; tampered skills shadow out of routing).
//!
//! No code path here enables or starts an MCP server: pack MCP
//! entries land in `.harness/mcp.json` as DISABLED declarations
//! (harness-skills `WriteSet` forces `trusted_autostart: false`), and
//! this module only ever READS the manifest for rendering.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use harness_skills::consent::ConsentGate;
use harness_skills::loader::{InvokedSkill, LockMode, SkillLoader, SkillIndex};
use harness_skills::routing::RouteQuery;
use serde_json::{Value, json};

use crate::automations::{FixedPackGate, PackGate, PackTier};

/// Typed activation refusals. Every variant names the skill; the
/// quarantine variant carries the tier that refused it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SkillRefusal {
    /// The pack is quarantined at `tier` (draft/unvetted) — never
    /// activatable on any path.
    #[error("skill pack {name:?} is {}-tier; quarantined packs are never activatable (trust gate)", tier_name(*tier))]
    Quarantined {
        /// The refused skill.
        name: String,
        /// The tier that refused it.
        tier: PackTier,
    },
    /// No loaded skill carries this name.
    #[error("skill {name:?} not found")]
    Unknown {
        /// The requested skill.
        name: String,
    },
    /// The session host has no skills runtime bound (isolated hosts
    /// built outside the daemon state factory).
    #[error("skill {name:?} unusable: no skills runtime bound to this session")]
    NotBound {
        /// The requested skill.
        name: String,
    },
}

/// What the submit-lane gate decided for one piece of text.
#[derive(Debug, Clone, PartialEq)]
pub enum SubmitRouting {
    /// No gated skill matched: submit unchanged.
    Pass,
    /// A quarantined skill's trigger matched: refuse with the same
    /// typed refusal the `/skill` path produces.
    Refused(SkillRefusal),
    /// A trusted, consented skill's trigger matched: its context is
    /// injected with the original text.
    Activate(InvokedSkill),
}

/// The startup-loaded skills surface: one per daemon, cloned into
/// every session host.
#[derive(Clone)]
pub struct SkillsRuntime {
    data_dir: PathBuf,
    index: SkillIndex,
    gate: Arc<dyn PackGate>,
    consent: ConsentGate,
}

fn tier_name(tier: PackTier) -> &'static str {
    match tier {
        PackTier::Approved => "approved",
        PackTier::Draft => "draft",
    }
}

impl SkillsRuntime {
    /// Scans `{data_dir}/.harness/skills` (plus the other vendor
    /// dirs) with lock pinning. A scan error is loud but never fatal:
    /// the daemon serves on with an empty index (zero packs active —
    /// fail-closed for activation, not for availability).
    #[must_use]
    pub fn load(data_dir: &Path) -> Self {
        let index = match SkillLoader::new()
            .vendor_root(data_dir)
            .lock_mode(LockMode::Record)
            .load()
        {
            Ok(index) => index,
            Err(e) => {
                eprintln!("harness-daemon: skills scan failed; zero packs active: {e}");
                SkillIndex::empty()
            }
        };
        let gate: Arc<dyn PackGate> = match crate::automations::FilePackGate::open(
            &data_dir.join("pack-tiers.json"),
        ) {
            Ok(gate) => Arc::new(gate),
            Err(e) => {
                eprintln!("harness-daemon: pack tiers unreadable; every pack draft: {e}");
                Arc::new(FixedPackGate::new(Vec::new()))
            }
        };
        let consent = std::fs::read(data_dir.join("skill-consent.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            data_dir: data_dir.to_path_buf(),
            index,
            gate,
            consent,
        }
    }

    fn tier_of(&self, name: &str) -> PackTier {
        self.gate.tier(name)
    }

    /// Explicit `/skill` invocation: tier gate first, then the
    /// progressive-disclosure body load.
    ///
    /// # Errors
    /// [`SkillRefusal`] — never a bare denial.
    pub fn invoke(&self, name: &str) -> Result<InvokedSkill, SkillRefusal> {
        let tier = self.tier_of(name);
        if tier != PackTier::Approved {
            return Err(SkillRefusal::Quarantined {
                name: name.to_owned(),
                tier,
            });
        }
        self.index.invoke(name).map_err(|_| SkillRefusal::Unknown {
            name: name.to_owned(),
        })
    }

    /// The submit-lane gate: parse a `/skill` command, else run the
    /// trigger routing (tier + consent gates) over the text.
    #[must_use]
    pub fn route_submit(&self, text: &str) -> SubmitRouting {
        if let Some(name) = skill_command_target(text) {
            return match self.invoke(name) {
                Ok(skill) => SubmitRouting::Activate(skill),
                Err(refusal) => SubmitRouting::Refused(refusal),
            };
        }
        let query = RouteQuery {
            text,
            work_path: None,
        };
        let decision = self.index.route_with(&query, Some(&self.consent));
        // Quarantine outranks the consent partition: a draft-tier
        // trigger match refuses even when consent is unrecorded (the
        // submitter learns the skill is quarantined, not silence).
        let triggered: Vec<&str> = decision
            .matched
            .iter()
            .map(|m| m.name.as_str())
            .chain(decision.pending_consent.iter().map(|n| n.as_str()))
            .collect();
        for name in triggered {
            let tier = self.tier_of(name);
            if tier != PackTier::Approved {
                return SubmitRouting::Refused(SkillRefusal::Quarantined {
                    name: name.to_owned(),
                    tier,
                });
            }
        }
        for matched in &decision.matched {
            if let Ok(skill) = self.index.invoke(matched.name.as_str()) {
                return SubmitRouting::Activate(skill);
            }
        }
        SubmitRouting::Pass
    }

    /// The rendered context one activated skill injects: its
    /// progressive-disclosure body plus the caller's arguments.
    #[must_use]
    pub fn render_context(skill: &InvokedSkill, args: &str) -> String {
        let mut context = format!(
            "<skill name=\"{}\">\n{}\n</skill>",
            skill.name.as_str(),
            skill.body
        );
        if !args.is_empty() {
            context.push_str("\n\n");
            context.push_str(args);
        }
        context
    }

    /// The `GET /packs` body: every loaded skill with its tier and
    /// trust status, plus every declared MCP server with its disabled
    /// marker (`trusted_autostart` absent/false = disabled — the same
    /// field [`harness_mcp::manager::ScanReport::autostart_for`]
    /// reads, so the marker and the manager's deferred-autostart
    /// decision cannot disagree).
    #[must_use]
    pub fn packs_json(&self) -> Value {
        let packs: Vec<Value> = self
            .index
            .metas()
            .map(|meta| {
                let tier = self.tier_of(meta.name.as_str());
                json!({
                    "name": meta.name.as_str(),
                    "tier": tier_name(tier),
                    "trusted": tier == PackTier::Approved,
                    "vendor": meta.vendor.dir_name(),
                    "version": meta.version,
                    "description": meta.description,
                    "triggers": meta.triggers,
                })
            })
            .collect();
        let mcp_servers = self.mcp_declarations();
        json!({ "packs": packs, "mcp_servers": mcp_servers })
    }

    fn mcp_declarations(&self) -> Vec<Value> {
        let manifest = std::fs::read(self.data_dir.join(".harness").join("mcp.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .unwrap_or(Value::Null);
        let Some(servers) = manifest.get("mcpServers").and_then(Value::as_object) else {
            return Vec::new();
        };
        servers
            .iter()
            .map(|(server, entry)| {
                let disabled = entry
                    .get("trusted_autostart")
                    .and_then(Value::as_bool)
                    != Some(true);
                json!({ "server": server, "disabled": disabled })
            })
            .collect()
    }
}

/// `"/skill name …"` → `Some("name")`; any other text → `None`.
fn skill_command_target(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("/skill ")?;
    let name = rest.split_whitespace().next()?;
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_command_target_parses_only_the_command_form() {
        assert_eq!(skill_command_target("/skill audit run"), Some("audit"));
        assert_eq!(skill_command_target("/skill audit"), Some("audit"));
        assert_eq!(skill_command_target("please /skill me"), None);
        assert_eq!(skill_command_target("/skillx audit"), None);
        assert_eq!(skill_command_target("/skill "), None);
    }

    #[test]
    fn refusal_display_carries_the_tier() {
        let refusal = SkillRefusal::Quarantined {
            name: "draft-pack".into(),
            tier: PackTier::Draft,
        };
        let text = refusal.to_string();
        assert!(text.contains("draft-tier"), "{text}");
        assert!(text.contains("draft-pack"), "{text}");
    }
}
