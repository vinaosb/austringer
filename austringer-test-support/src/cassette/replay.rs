//! Strict cassette replay: interactions are consumed in recording order, every
//! request must match its recorded counterpart, and finishing with unused
//! interactions is a hard error.

use super::error::CassetteError;
use super::record::Cassette;
use super::recorder::recorded_to_reply;
use crate::provider::{ProviderRequest, ScriptedReply};

/// Strict replayer over a loaded [`Cassette`].
#[derive(Debug)]
pub struct CassetteReplayer {
    name: String,
    interactions: Vec<super::record::RecordedInteraction>,
    next: usize,
}

impl CassetteReplayer {
    /// Creates a replayer positioned at the first interaction.
    pub fn new(cassette: Cassette) -> Self {
        Self {
            name: cassette.name,
            interactions: cassette.interactions,
            next: 0,
        }
    }

    /// Serves the next interaction to a matching request.
    ///
    /// Strict semantics: the incoming request must equal the next recorded
    /// request (`model` and full prompt); a mismatch aborts with
    /// [`CassetteError::Mismatch`] instead of seeking. A dry cassette aborts
    /// with [`CassetteError::Exhausted`].
    pub fn next_interaction(
        &mut self,
        request: &ProviderRequest,
    ) -> Result<ScriptedReply, CassetteError> {
        let Some(recorded) = self.interactions.get(self.next) else {
            return Err(CassetteError::Exhausted {
                name: self.name.clone(),
                model: request.model.clone(),
                prompt: request.prompt.clone(),
            });
        };
        if recorded.request.model != request.model || recorded.request.prompt != request.prompt {
            return Err(CassetteError::Mismatch {
                name: self.name.clone(),
                index: self.next,
                expected: Box::new(recorded.request.clone()),
                got: Box::new(request.clone()),
            });
        }
        let reply = recorded_to_reply(&recorded.response);
        self.next += 1;
        Ok(reply)
    }

    /// Whether every recorded interaction has been consumed.
    pub fn is_exhausted(&self) -> bool {
        self.next >= self.interactions.len()
    }

    /// Strict finish: fails while any recorded interaction was never replayed.
    pub fn finish(self) -> Result<(), CassetteError> {
        let unused = self.interactions.len() - self.next;
        if unused == 0 {
            Ok(())
        } else {
            Err(CassetteError::UnusedInteractions {
                name: self.name,
                count: unused,
            })
        }
    }
}
