//! The executor registry: builder-validated construction, then an
//! immutable dispatch authority. Mirrors the discipline of
//! `harness-core/src/tools/registry.rs` (typed errors, no half-way
//! states, one dispatch path).
//!
//! ## Immutability discipline (plan M6 adversarial: stale-state)
//!
//! An [`ExecutorRegistry`] has NO mutation methods: it is frozen by
//! [`ExecutorRegistryBuilder::build`]. Adapter todos (M11+) land by
//! producing a new registry through the builder - a live registry handed
//! out for dispatch can never be re-wired underneath its callers. The
//! builder itself validates every wiring rule at registration time, so a
//! frozen registry is complete by construction:
//!
//! - probe must be a bound pure handler (the plan's MUST-NOT: no format
//!   registered without probe + (extract | typed-unsupported));
//! - a descriptor `Supported` claim must have a dispatch path (Pure or
//!   Subprocess) - README-ahead-of-code is unrepresentable;
//! - a bound handler must have a `Supported` claim - code-ahead-of-README
//!   is unrepresentable too;
//! - one format, one adapter (duplicates are refused).

use std::collections::BTreeMap;

use super::binding::{AdapterEntry, OpBinding, ProbeHandler};
use super::types::{AdapterTier, OpOutput, OpRequest, Produced, ProducerInfo};
use crate::descriptor::{Operation, OpSupport};
use crate::error::MediaError;
use crate::format::FormatId;

/// Wiring bugs the builder refuses, each naming the fix.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExecutorWiringError {
    /// Two adapters claimed one format.
    #[error("two adapters claimed format {format}: one format, one adapter - fix the wiring")]
    DuplicateAdapter {
        /// The contested format.
        format: FormatId,
    },
    /// The plan's MUST-NOT, constructor-enforced.
    #[error(
        "format {format} registered without a bound probe handler: registration requires probe + (extract handler | typed-unsupported)"
    )]
    ProbeNotBound {
        /// The offending format.
        format: FormatId,
    },
    /// The descriptor claims support the bindings do not dispatch.
    #[error(
        "descriptor claims {op} supported for {format} but no dispatch path is bound: bind a handler or fix the claim in the same change"
    )]
    SupportedWithoutDispatch {
        /// The over-claimed format.
        format: FormatId,
        /// The over-claimed operation.
        op: Operation,
    },
    /// A handler is bound where the descriptor refuses.
    #[error(
        "a {op} handler is bound for {format} but the descriptor refuses it: flip the descriptor to Supported in the same change"
    )]
    HandlerWithoutClaim {
        /// The under-claimed format.
        format: FormatId,
        /// The under-claimed operation.
        op: Operation,
    },
}

/// Collects [`AdapterEntry`]s under the wiring rules, then freezes them.
#[derive(Debug, Default)]
pub struct ExecutorRegistryBuilder {
    entries: BTreeMap<FormatId, AdapterEntry>,
}

impl ExecutorRegistryBuilder {
    /// An empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register one adapter entry under the wiring rules (see the module
    /// docs). The entry is keyed by its descriptor's format id.
    pub fn register(&mut self, entry: AdapterEntry) -> Result<(), ExecutorWiringError> {
        let format = entry.descriptor.id;
        if self.entries.contains_key(&format) {
            return Err(ExecutorWiringError::DuplicateAdapter { format });
        }
        if !matches!(entry.ops.probe, OpBinding::<ProbeHandler>::Pure { .. }) {
            return Err(ExecutorWiringError::ProbeNotBound { format });
        }
        let claims = &entry.descriptor.ops;
        let missing = || OpSupport::Unsupported {
            reason: "op missing from the descriptor table".into(),
        };
        macro_rules! sync_check {
            ($op:expr, $binding:expr) => {
                match ($binding, claims.get(&$op).cloned().unwrap_or_else(missing)) {
                    (OpBinding::Pure { .. }, OpSupport::Unsupported { .. }) => {
                        return Err(ExecutorWiringError::HandlerWithoutClaim {
                            format,
                            op: $op,
                        });
                    }
                    (OpBinding::Unsupported { .. }, OpSupport::Supported) => {
                        return Err(ExecutorWiringError::SupportedWithoutDispatch {
                            format,
                            op: $op,
                        });
                    }
                    _ => {}
                }
            };
        }
        sync_check!(Operation::Probe, &entry.ops.probe);
        sync_check!(Operation::Extract, &entry.ops.extract);
        sync_check!(Operation::Generate, &entry.ops.generate);
        sync_check!(Operation::Verify, &entry.ops.verify);
        self.entries.insert(format, entry);
        Ok(())
    }

    /// Freeze: the registry is immutable from here on.
    #[must_use]
    pub fn build(self) -> ExecutorRegistry {
        ExecutorRegistry {
            entries: self.entries,
        }
    }
}

/// The frozen dispatch authority over registered formats.
#[derive(Debug, Clone, Default)]
pub struct ExecutorRegistry {
    entries: BTreeMap<FormatId, AdapterEntry>,
}

impl ExecutorRegistry {
    /// Look up a format's adapter entry; unregistered formats are the
    /// typed [`MediaError::UnsupportedFormat`] - never a guess.
    pub fn lookup(&self, format: &FormatId) -> Result<&AdapterEntry, MediaError> {
        self.entries.get(format).ok_or(MediaError::UnsupportedFormat)
    }

    /// Every registered format id (registry-completeness enumeration).
    pub fn formats(&self) -> impl Iterator<Item = FormatId> + '_ {
        self.entries.keys().copied()
    }

    /// Dispatch one operation. Every success is a [`Produced`] - artifact
    /// plus producer disclosure, type-enforced. Every refusal is typed:
    /// unregistered format, unsupported op (verbatim reason), or the
    /// pre-M15 subprocess seam (tool + install hint).
    pub fn dispatch(
        &self,
        format: FormatId,
        request: OpRequest<'_>,
    ) -> Result<Produced, MediaError> {
        let entry = self.lookup(&format)?;
        let op = request.operation();
        match request {
            OpRequest::Probe { bytes, name_hint } => match &entry.ops.probe {
                OpBinding::Pure { engine, handler } => Ok(Produced::new(
                    OpOutput::Probe(handler(bytes, name_hint)),
                    pure_producer(engine),
                )),
                other => Err(other.refusal(format, op)),
            },
            OpRequest::Extract { bytes, budgets } => match &entry.ops.extract {
                OpBinding::Pure { engine, handler } => Ok(Produced::new(
                    OpOutput::Extract(handler(bytes, budgets)?),
                    pure_producer(engine),
                )),
                other => Err(other.refusal(format, op)),
            },
            OpRequest::Generate { spec } => match &entry.ops.generate {
                OpBinding::Pure { engine, handler } => Ok(Produced::new(
                    OpOutput::Generate(handler(spec)?),
                    pure_producer(engine),
                )),
                other => Err(other.refusal(format, op)),
            },
            OpRequest::Verify { expected, bytes } => match &entry.ops.verify {
                OpBinding::Pure { engine, handler } => Ok(Produced::new(
                    OpOutput::Verify(handler(expected, bytes)?),
                    pure_producer(engine),
                )),
                other => Err(other.refusal(format, op)),
            },
        }
    }
}

fn pure_producer(engine: &str) -> ProducerInfo {
    ProducerInfo {
        tier: AdapterTier::Pure,
        engine: engine.to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        command: None,
    }
}
