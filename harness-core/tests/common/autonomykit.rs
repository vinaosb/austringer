//! Shared fixtures for autonomy (todo 8) tests: deterministic scripted
//! classifier stages that capture every sanitized request they see.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use harness_core::agent_loop::stages::StageError;
use harness_core::autonomy::{Intent, IntentStage, Risk, RiskStage, SanitizedRequest};

/// Scripted stage A (intent): serves queued intents in order and records
/// every request it was consulted on.
pub struct ScriptedIntent {
    queue: Mutex<VecDeque<Intent>>,
    seen: Mutex<Vec<SanitizedRequest>>,
}

impl ScriptedIntent {
    /// A stage answering with `intents` in order.
    pub fn new(intents: Vec<Intent>) -> Self {
        Self {
            queue: Mutex::new(intents.into_iter().collect()),
            seen: Mutex::new(Vec::new()),
        }
    }

    /// The sanitized requests this stage received, in order.
    pub fn seen(&self) -> Vec<SanitizedRequest> {
        self.seen.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn answer(&self, request: SanitizedRequest) -> Result<Intent, StageError> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(request);
        }
        self.queue
            .lock()
            .map_err(|_| StageError {
                message: "script poisoned".to_owned(),
            })
            .and_then(|mut queue| {
                queue.pop_front().ok_or_else(|| StageError {
                    message: "intent script exhausted".to_owned(),
                })
            })
    }
}

#[async_trait]
impl IntentStage for ScriptedIntent {
    async fn intent(&mut self, request: &SanitizedRequest) -> Result<Intent, StageError> {
        self.answer(request.clone())
    }
}

/// Scripted stage B (risk): serves queued risks in order and records every
/// request it was consulted on.
pub struct ScriptedRisk {
    queue: Mutex<VecDeque<Risk>>,
    seen: Mutex<Vec<SanitizedRequest>>,
}

impl ScriptedRisk {
    /// A stage answering with `risks` in order.
    pub fn new(risks: Vec<Risk>) -> Self {
        Self {
            queue: Mutex::new(risks.into_iter().collect()),
            seen: Mutex::new(Vec::new()),
        }
    }

    /// The sanitized requests this stage received, in order.
    pub fn seen(&self) -> Vec<SanitizedRequest> {
        self.seen.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn answer(&self, request: SanitizedRequest) -> Result<Risk, StageError> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(request);
        }
        self.queue
            .lock()
            .map_err(|_| StageError {
                message: "script poisoned".to_owned(),
            })
            .and_then(|mut queue| {
                queue.pop_front().ok_or_else(|| StageError {
                    message: "risk script exhausted".to_owned(),
                })
            })
    }
}

#[async_trait]
impl RiskStage for ScriptedRisk {
    async fn risk(
        &mut self,
        request: &SanitizedRequest,
        _intent: Intent,
    ) -> Result<Risk, StageError> {
        self.answer(request.clone())
    }
}

/// An intent stage that always fails (transport error probe).
pub struct FailingIntent;

#[async_trait]
impl IntentStage for FailingIntent {
    async fn intent(&mut self, _request: &SanitizedRequest) -> Result<Intent, StageError> {
        Err(StageError {
            message: "intent stage offline".to_owned(),
        })
    }
}

/// A risk stage that always fails (transport error probe).
pub struct FailingRisk;

#[async_trait]
impl RiskStage for FailingRisk {
    async fn risk(
        &mut self,
        _request: &SanitizedRequest,
        _intent: Intent,
    ) -> Result<Risk, StageError> {
        Err(StageError {
            message: "risk stage offline".to_owned(),
        })
    }
}
