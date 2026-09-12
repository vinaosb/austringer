//! RepetitionInspector: identical-call window detection.
//!
//! A call's identity is `(tool, canonical arguments)` — the correlation id
//! is excluded (two calls with distinct ids but identical intent are the
//! same call). Canonical arguments serialize with sorted keys, so argument
//! key order is irrelevant. The inspector keeps a sliding window of the
//! last `window` fingerprints; when the current call is the Nth occurrence
//! of its fingerprint inside that window (default N = 3), it returns a
//! typed [`Verdict::Intervene`] — block plus model-feedback suggesting
//! variation — never a silent pass. Occurrences keep counting while the
//! model repeats, so the 4th, 5th, ... identical calls stay blocked too.

use std::collections::VecDeque;

use async_trait::async_trait;

use crate::agent_loop::PendingCall;
use crate::agent_loop::stages::StageError;
use crate::inspectors::verdict::{CallInspector, Verdict};

/// Repetition detector tuning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepetitionConfig {
    /// How many occurrences of one fingerprint inside the window trip the
    /// intervention.
    pub threshold: u32,
    /// How many recent calls the sliding window covers.
    pub window: usize,
}

impl Default for RepetitionConfig {
    fn default() -> Self {
        Self {
            threshold: 3,
            window: 32,
        }
    }
}

/// The identical-call circuit breaker.
#[derive(Debug, Clone)]
pub struct RepetitionInspector {
    threshold: u32,
    window: usize,
    recent: VecDeque<[u8; 32]>,
}

impl RepetitionInspector {
    /// Builds the detector from `config`.
    #[must_use]
    pub fn new(config: RepetitionConfig) -> Self {
        Self {
            threshold: config.threshold.max(1),
            window: config.window.max(1),
            recent: VecDeque::new(),
        }
    }
}

/// Stable identity of a call: tool + canonical (sorted-key) arguments,
/// hashed with BLAKE3. Serialization failure is impossible for
/// `serde_json::Value` in practice; a failing value degrades to the empty
/// encoding rather than panicking.
fn fingerprint(call: &PendingCall) -> [u8; 32] {
    let canonical = serde_json::to_string(&call.arguments).unwrap_or_default();
    let mut hasher = blake3::Hasher::new();
    hasher.update(call.tool.as_bytes());
    hasher.update(&[0]);
    hasher.update(canonical.as_bytes());
    *hasher.finalize().as_bytes()
}

#[async_trait]
impl CallInspector for RepetitionInspector {
    fn name(&self) -> &'static str {
        "repetition"
    }

    async fn inspect(&mut self, call: &PendingCall) -> Result<Verdict, StageError> {
        let current = fingerprint(call);
        self.recent.push_back(current);
        while self.recent.len() > self.window {
            self.recent.pop_front();
        }
        let occurrences = self.recent.iter().filter(|seen| **seen == current).count() as u32;
        if occurrences >= self.threshold {
            Ok(Verdict::Intervene {
                reason: format!(
                    "repetition intervention: tool '{}' was invoked identically \
                     {occurrences} times within the last {} calls — vary the arguments \
                     or take a different approach",
                    call.tool, self.window
                ),
            })
        } else {
            Ok(Verdict::Allow {
                authoritative: false,
            })
        }
    }
}
