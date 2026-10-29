//! The PromptBuilder-routed session prompt path: core's
//! prefix-cache-stable composition surface (`ContextSpec` +
//! [`PromptBuilder`]), previously library-only, promoted into the
//! daemon's real spawn path (research §32, the jcode stable-prefix
//! receipt).
//!
//! The daemon composes each session's system prompt through the
//! builder: a [`ContextSpec`] is built at spawn (system = the frozen
//! [`SESSION_SYSTEM_PROMPT`](crate::engine::SESSION_SYSTEM_PROMPT),
//! repomap = the same bounded map text
//! [`crate::repomap::compose_system_prompt`] hydrates, all other
//! frozen segments empty as they exist today), rendered ONCE, and the
//! provider-facing system string is derived from the rendered
//! segments. That derivation is a byte-identity adapter, pinned by
//! `prompt_composition_tests`: the provider string stays EXACTLY
//! `SESSION_SYSTEM_PROMPT` + the `<repomap>` block
//! [`crate::repomap::compose_system_prompt`] produces — the
//! seven-segment layout is the daemon's canonical internal
//! composition (validated by the builder's volatile scan, hashed for
//! the tracker), not a wire-format change.
//!
//! The cache-violation tracker rides along daemon-side: each session
//! keeps an in-memory [`PromptEpoch`] (the blake3 stable-prefix chain
//! plus the last rendered prompt, surviving host eviction like the
//! cost ledgers, never persisted). A re-composition — a re-created
//! host after idle eviction — that fails to extend the previous
//! stable prefix fires a typed [`ViolationEvent`], surfaced as a
//! `prompt_cache` transcript notice: prefix churn is an observable
//! event, never a silent cost spike, and never a spawn failure.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use austringer_core::context::{
    BuildError, CacheViolationTracker, ContextSpec, EffortSettings, PromptBuilder, RenderedPrompt,
    SegmentName, TurnObservation, ViolationEvent, ViolationKind, common_prefix_len,
};
use austringer_core::event::{ContentHash, Sequence};

/// Transcript-notice kind for prompt-epoch cache violations (the
/// `Notification.kind` vocabulary alongside `fallback`).
pub const PROMPT_CACHE_KIND: &str = "prompt_cache";

/// One spawn-time composition outcome.
pub enum ComposedSessionPrompt {
    /// Rendered through the builder: the byte-identity system string
    /// plus the full rendered prompt (hash + segment map).
    Rendered {
        /// The provider-facing system string.
        system: String,
        /// The canonical seven-segment render.
        rendered: RenderedPrompt,
    },
    /// The builder refused the spec (typed cause: volatile data above
    /// L2 — the layout law). `system` carries the exact
    /// [`crate::repomap::compose_system_prompt`] bytes over the same
    /// inputs, so session spawn never fails; the caller MUST surface
    /// the refusal as a transcript notice (fail-open, never silent —
    /// the hooks doctrine).
    Refused {
        /// The legacy-composed system string.
        system: String,
        /// Why the builder refused.
        cause: BuildError,
    },
}

impl ComposedSessionPrompt {
    /// The provider-facing system string either way.
    #[must_use]
    pub fn system(&self) -> &str {
        match self {
            Self::Rendered { system, .. } | Self::Refused { system, .. } => system,
        }
    }
}

/// Composes the session prompt through [`PromptBuilder`].
#[must_use]
pub fn compose_session_prompt(
    workspace_root: &Path,
    cache_dir: &Path,
    token_budget: u32,
) -> ComposedSessionPrompt {
    let spec = ContextSpec {
        system: crate::engine::SESSION_SYSTEM_PROMPT.to_owned(),
        base_schema: String::new(),
        rules: Vec::new(),
        skills: Vec::new(),
        repomap: crate::repomap::map_text(workspace_root, cache_dir, token_budget),
        history: Vec::new(),
        last_user_message: String::new(),
        effort: EffortSettings::default(),
        beta_effort: None,
    };
    match PromptBuilder::render(&spec) {
        Ok(rendered) => ComposedSessionPrompt::Rendered {
            system: flat_system(&spec.system, rendered.segment_content(SegmentName::Repomap)),
            rendered,
        },
        Err(cause) => ComposedSessionPrompt::Refused {
            system: flat_system(&spec.system, Some(spec.repomap.as_str())),
            cause,
        },
    }
}

/// The byte-identity adapter: the flat provider-facing system string
/// (base prompt, `\n\n`, repomap block without the layout's trailing
/// newline; tag sourced from the layout's own vocabulary). Pinned
/// byte-equal to [`crate::repomap::compose_system_prompt`] by test.
fn flat_system(base: &str, map_text: Option<&str>) -> String {
    match map_text {
        Some(text) if !text.is_empty() => {
            let tag = SegmentName::Repomap.tag();
            format!("{base}\n\n<{tag}>\n{text}\n</{tag}>")
        }
        _ => base.to_owned(),
    }
}

/// One session's in-memory prompt epoch: the cache-violation
/// tracker's append-only chain plus the last rendered prompt.
/// Daemon-held (survives host eviction); never persisted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptEpoch {
    tracker: CacheViolationTracker,
    last_prompt: String,
    compositions: u64,
}

impl PromptEpoch {
    /// An empty epoch (no baseline yet).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tracker: CacheViolationTracker::new(),
            last_prompt: String::new(),
            compositions: 0,
        }
    }

    /// Records one composition. The first observation only establishes
    /// the chain (a cold baseline is a write, not a violation); a
    /// later composition that fails to extend the previous stable
    /// prefix fires a typed [`ViolationEvent`] naming the diverged
    /// segment and byte offset. Returns THIS observation's violations.
    pub fn observe(&mut self, rendered: &RenderedPrompt) -> Vec<ViolationEvent> {
        self.compositions += 1;
        let common = common_prefix_len(&self.last_prompt, &rendered.prompt) as u64;
        let fired = self.tracker.observe(TurnObservation {
            turn: Sequence(self.compositions),
            rendered,
            common_prefix_with_previous: (self.compositions > 1).then_some(common),
            report: None,
        });
        self.last_prompt = rendered.prompt.clone();
        fired
    }

    /// The live baseline: the chain head's expected stable-prefix hash.
    #[must_use]
    pub fn stable_prefix_hash(&self) -> Option<ContentHash> {
        self.tracker
            .chain()
            .last()
            .map(|entry| entry.expected_prefix_hash)
    }

    /// How many compositions this epoch has observed (chain length).
    #[must_use]
    pub const fn compositions(&self) -> u64 {
        self.compositions
    }
}

impl Default for PromptEpoch {
    fn default() -> Self {
        Self::new()
    }
}

/// The session-factory seam: compose, record the epoch, and detect
/// drift. Returns the provider-facing system string plus the notice
/// messages to publish on the session hub (the caller publishes them
/// as `PROMPT_CACHE_KIND` notifications — never silent, never fatal).
#[must_use]
pub fn bind_system_prompt(
    epochs: &Mutex<HashMap<String, PromptEpoch>>,
    session_id: &str,
    workspace_root: &Path,
    cache_dir: &Path,
    token_budget: u32,
) -> (String, Vec<String>) {
    let composition = compose_session_prompt(workspace_root, cache_dir, token_budget);
    let mut notices = Vec::new();
    let system = match &composition {
        ComposedSessionPrompt::Rendered { system, rendered } => {
            let mut epochs = lock_epochs(epochs);
            let violations = match epochs.get_mut(session_id) {
                Some(epoch) => epoch.observe(rendered),
                None => epochs
                    .entry(session_id.to_owned())
                    .or_default()
                    .observe(rendered),
            };
            notices.extend(violations.iter().map(violation_notice));
            system.clone()
        }
        ComposedSessionPrompt::Refused { system, cause } => {
            notices.push(format!(
                "prompt builder refused the session composition ({cause}); \
                 spawning on the legacy byte-identical system prompt with no \
                 tracked prefix epoch"
            ));
            system.clone()
        }
    };
    (system, notices)
}

/// One notice message per fired violation (typed inside the epoch;
/// the hub's `Notification` frame carries the rendered text).
fn violation_notice(violation: &ViolationEvent) -> String {
    match &violation.kind {
        ViolationKind::ChainBreak {
            expected_common,
            actual_common,
        } => format!(
            "stable prompt prefix changed since the session's frozen epoch: \
             chain break at byte {} (segment {:?}); expected at least \
             {expected_common} common bytes, measured {actual_common} — the \
             provider prefix cache is invalidated",
            violation.byte_offset, violation.diverged_segment
        ),
        ViolationKind::CacheMiss {
            expected_bytes,
            reported_bytes,
        } => format!(
            "provider served {reported_bytes} cache-read bytes against an \
             expected {expected_bytes}-byte cacheable prefix (segment {:?} at \
             byte {})",
            violation.diverged_segment, violation.byte_offset
        ),
    }
}

fn lock_epochs(
    epochs: &Mutex<HashMap<String, PromptEpoch>>,
) -> std::sync::MutexGuard<'_, HashMap<String, PromptEpoch>> {
    match epochs.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn zero_budget_routes_to_the_plain_prompt() {
        let dir = tempdir();
        let cache = tempdir();
        let composition = compose_session_prompt(dir.path(), cache.path(), 0);
        let ComposedSessionPrompt::Rendered { system, rendered } = composition else {
            panic!("empty inputs must render");
        };
        assert_eq!(system, crate::engine::SESSION_SYSTEM_PROMPT);
        assert_eq!(
            rendered.segment_content(SegmentName::Repomap),
            Some(""),
            "budget 0 keeps the repomap segment empty"
        );
    }

    #[test]
    fn epoch_first_observation_establishes_then_stable_rerender_is_silent() {
        let dir = tempdir();
        let cache = tempdir();
        let first = compose_session_prompt(dir.path(), cache.path(), 0);
        let ComposedSessionPrompt::Rendered { rendered: a, .. } = first else {
            panic!("render");
        };
        let mut epoch = PromptEpoch::new();
        assert!(
            epoch.observe(&a).is_empty(),
            "the first observation only establishes the chain"
        );
        let second = compose_session_prompt(dir.path(), cache.path(), 0);
        let ComposedSessionPrompt::Rendered { rendered: b, .. } = second else {
            panic!("render");
        };
        assert_eq!(a.stable_prefix_hash, b.stable_prefix_hash);
        assert!(
            epoch.observe(&b).is_empty(),
            "an unchanged epoch never fires"
        );
        assert_eq!(epoch.stable_prefix_hash(), Some(a.stable_prefix_hash));
    }
}
