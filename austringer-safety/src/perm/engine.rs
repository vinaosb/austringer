//! The unified permission engine (plan todo 27): ONE decision layer
//! consulted by every tool path — hardline floor → admin floor →
//! Action(target) grammar → security profile → hard action checks
//! (resolution/blocklist/sandbox ceilings/SSRF+egress) → stored
//! grants → soft checks (substitution coverage, secret reads,
//! read-only stance) → max-requests breaker → bypass conversion.
//! Every decision carries a typed reason and its resolution path,
//! and lands in the audit sink. The T7 chain and T8 store adapt onto
//! these seams (core wires an inspector that calls
//! [`PermissionEngine::decide`]); nothing evaluates permissions
//! ad-hoc beside this engine.

use std::sync::Arc;

use crate::egress::EgressPolicy;
use crate::perm::audit::{AuditEntry, AuditSink, MemoryAuditSink};
use crate::perm::breaker::{BreakerState, Clock, SystemClock};
use crate::perm::checks;
use crate::perm::compile::compile_egress;
use crate::perm::decision::{Decision, allow, ask, deny};
use crate::perm::grammar::{Action, Effect, GrammarVerdict, Target};
use crate::perm::hardline;
use crate::perm::profile::ClampOutcome;
use crate::perm::reasons::PermissionReason;
use crate::perm::resolve::{PathResolver, ProgramResolver};

pub use crate::perm::config::{BypassMode, EngineBuildError, EngineConfig};
pub use crate::perm::grants::{Grant, GrantOrigin, GrantSource, MemoryGrants};

/// The unified engine.
pub struct PermissionEngine {
    config: EngineConfig,
    grants: Arc<dyn GrantSource>,
    resolver: Box<dyn ProgramResolver>,
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock>,
    breaker: BreakerState,
    egress: EgressPolicy,
}

impl PermissionEngine {
    /// Default engine over `config` with platform seams.
    ///
    /// # Errors
    /// [`EngineBuildError::EgressCompile`] when a URL rule cannot
    /// compile into the sandbox egress allowlist (fail closed).
    pub fn new(config: EngineConfig) -> Result<Self, EngineBuildError> {
        Self::with_seams(
            config,
            Arc::new(MemoryGrants::new()),
            Box::new(PathResolver::from_path_env()),
            Arc::new(MemoryAuditSink::new()),
            Arc::new(SystemClock),
        )
    }

    /// Engine with injectable seams (tests, embedders).
    ///
    /// # Errors
    /// As [`PermissionEngine::new`].
    pub fn with_seams(
        config: EngineConfig,
        grants: Arc<dyn GrantSource>,
        resolver: Box<dyn ProgramResolver>,
        audit: Arc<dyn AuditSink>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, EngineBuildError> {
        let egress = compile_egress(&config.rules)?;
        let breaker = BreakerState::new(config.breaker);
        Ok(Self {
            config,
            grants,
            resolver,
            audit,
            clock,
            breaker,
            egress,
        })
    }

    /// The permissions-derived egress allowlist (one config source
    /// drives both layers).
    #[must_use]
    pub fn egress(&self) -> &EgressPolicy {
        &self.egress
    }

    /// ONE decision for one request. Audited, always.
    pub fn decide(&mut self, action: Action, target: Target) -> Decision {
        let mut layers = Vec::new();
        let mut decision = self.evaluate(action, &target, &mut layers);
        self.breaker_and_bypass(&mut decision, &mut layers);
        decision.layers = layers;
        self.audit.record(AuditEntry {
            action,
            target_summary: checks::summarize(&target, &self.config.secret_patterns),
            effect: decision.effect,
            reason: decision.reason.clone(),
            bypass: self.config.bypass == BypassMode::Yolo,
            at_ms: self.clock.now_ms(),
        });
        decision
    }

    fn evaluate(
        &self,
        action: Action,
        target: &Target,
        layers: &mut Vec<&'static str>,
    ) -> Decision {
        if let Some(reason) = checks::shape(action, target) {
            layers.push("shape");
            return deny(reason);
        }
        if let Target::Command(command) = target {
            layers.push("floor");
            if hardline::scan(command).is_some() {
                return deny(PermissionReason::HardlineFloor);
            }
        }
        layers.push("grammar");
        let mut candidate = match checks::grammar_verdict(
            &self.config.rules,
            &self.config.admin,
            &self.config.sandbox,
            &action,
            target,
        ) {
            GrammarVerdict::Denied { admin: true, .. } => {
                return deny(PermissionReason::AdminFloor);
            }
            GrammarVerdict::Denied { .. } => return deny(PermissionReason::ExplicitlyDenied),
            GrammarVerdict::Allowed { .. } => {
                Some(allow(PermissionReason::ExplicitlyAllowlisted, true))
            }
            GrammarVerdict::Asked { .. } => Some(ask(PermissionReason::AskRule)),
            GrammarVerdict::NoOpinion => None,
        };
        layers.push("profile");
        let pinned = self.config.profile.requires_ask(&action);
        // Hard action checks run BEFORE grants: no grant path may
        // skip resolution, the blocklist, sandbox ceilings, or the
        // SSRF/egress guard.
        match target {
            Target::Command(command) if action == Action::Execute => {
                layers.push("resolution");
                if let Some(reason) = checks::execute_denies(
                    command,
                    &self.config.commands,
                    self.resolver.as_ref(),
                    &self.config.profile,
                ) {
                    return deny(reason);
                }
            }
            Target::Path(path) if matches!(action, Action::Read | Action::Write | Action::Edit) => {
                layers.push("paths");
                if let Some(reason) = checks::path_denies(action, path, &self.config.sandbox) {
                    return deny(reason);
                }
            }
            Target::Url(url) if action.is_url_action() => {
                layers.push("egress");
                if let Some(reason) = checks::url_denies(url, &self.config.admin, &self.egress) {
                    return deny(reason);
                }
            }
            _ => {}
        }
        let authoritative_allow = candidate
            .as_ref()
            .is_some_and(|d| d.effect == Effect::Allow && d.authoritative);
        let relative = checks::relativize(&self.config.sandbox, target);
        if !authoritative_allow && !pinned {
            layers.push("grant");
            let grant = self.grants.lookup(&action, target).or_else(|| {
                relative
                    .as_ref()
                    .and_then(|rel| self.grants.lookup(&action, rel))
            });
            if let Some(grant) = grant {
                match self
                    .config
                    .profile
                    .check_grant(&action, &checks::grant_key(target))
                {
                    ClampOutcome::WithinProfile => {
                        candidate = Some(allow(
                            match grant.origin {
                                GrantOrigin::User => PermissionReason::StoredGrant,
                                GrantOrigin::Agent => PermissionReason::AgentDecided,
                            },
                            true,
                        ));
                    }
                    // A grant wider than the effective profile is the
                    // final answer: clamped + typed notice (Devin).
                    ClampOutcome::Clamped { notice } => {
                        return Decision {
                            effect: Effect::Ask,
                            authoritative: false,
                            reason: PermissionReason::ProfileClamp,
                            layers: Vec::new(),
                            notice: Some(notice),
                        };
                    }
                }
            }
        }
        match target {
            Target::Command(command) if action == Action::Execute => {
                layers.push("commands");
                match checks::execute_coverage(command, &self.config.commands) {
                    Some(reason) => candidate = Some(ask(reason)),
                    None if candidate.is_none() => {
                        candidate = Some(allow(PermissionReason::ExplicitlyAllowlisted, true));
                    }
                    None => {}
                }
            }
            Target::Path(path) if action == Action::Read => {
                layers.push("paths");
                match checks::path_soft(
                    action,
                    path,
                    &self.config.sandbox,
                    &self.config.secret_patterns,
                ) {
                    checks::SoftPath::SecretAsk(reason, notice) => match candidate.as_mut() {
                        Some(decision) => decision.notice = notice,
                        None => {
                            candidate = Some(Decision {
                                effect: Effect::Ask,
                                authoritative: false,
                                reason,
                                layers: Vec::new(),
                                notice,
                            })
                        }
                    },
                    checks::SoftPath::ReadOnlyStance if candidate.is_none() => {
                        candidate = Some(allow(PermissionReason::ReadOnly, false));
                    }
                    checks::SoftPath::ReadOnlyStance | checks::SoftPath::None => {}
                }
            }
            _ => {}
        }
        if pinned {
            candidate = Some(ask(PermissionReason::ProfileCeiling));
        }
        candidate.unwrap_or_else(|| ask(PermissionReason::FailClosed))
    }

    fn breaker_and_bypass(&mut self, decision: &mut Decision, layers: &mut Vec<&'static str>) {
        if decision.effect == Effect::Allow {
            layers.push("breaker");
            let now = self.clock.now_ms();
            if self.breaker.tripped(now) {
                *decision = Decision {
                    effect: Effect::Ask,
                    authoritative: false,
                    reason: PermissionReason::CircuitBreakerTripped {
                        observed: self.breaker.observed(now),
                        max: self.config.breaker.max_requests,
                    },
                    layers: Vec::new(),
                    notice: None,
                };
                return;
            }
            self.breaker.record_allow(now);
        }
        if decision.effect == Effect::Ask
            && self.config.bypass == BypassMode::Yolo
            && checks::yolo_convertible(&decision.reason)
        {
            layers.push("yolo");
            decision.effect = Effect::Allow;
            decision.authoritative = false;
            decision.reason = PermissionReason::RunToCompletion;
        }
    }
}
