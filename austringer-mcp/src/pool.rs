//! The MCP transport pool (todo 29, Qwen pattern): live connections keyed
//! by (server name + config fingerprint) and shared across sessions;
//! handles are refcounted, idle handles drain after a window, poisoned
//! handles drain immediately, and the live-connection budget enforces
//! with HYSTERESIS — once `max_live` trips the enforce level it holds
//! until live drops to `max_live - hysteresis_gap`, so connections
//! hovering at the ceiling cannot flap between allow and reject.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use crate::client::McpClient;
use crate::config::ServerEntry;
use crate::error::McpClientError;

/// How new connections are created (the pool is transport-blind;
/// production uses [`McpClient::connect`], tests inject in-process wires).
pub type Connector = Arc<
    dyn Fn(
            ServerEntry,
        ) -> futures_util::future::BoxFuture<'static, Result<McpClient, McpClientError>>
        + Send
        + Sync,
>;

/// Pool tuning.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Enforce ceiling: at this many live connections new acquires reject
    /// (typed) until the release threshold.
    pub max_live: usize,
    /// Warn threshold: crossing it records a typed event (still allowed).
    pub warn_at: usize,
    /// Hysteresis gap: enforcing releases only at
    /// `max_live - hysteresis_gap`.
    pub hysteresis_gap: usize,
    /// A zero-refcount connection drains once idle this long.
    pub idle_drain_after: Duration,
}

/// Why an acquire failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PoolError {
    /// The budget is enforcing; active (refcounted) connections are at
    /// the ceiling.
    #[error("mcp pool budget enforcing: {live} active >= {limit}")]
    BudgetExhausted { limit: usize, live: usize },
    /// The connector failed (typed passthrough).
    #[error("{0}")]
    Connect(#[from] McpClientError),
}

/// Observable pool lifecycle events (warn thresholds, drains).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolEvent {
    /// Live count crossed the warn threshold.
    BudgetWarn { live: usize },
    /// A connection was drained (idle timeout or poison).
    Drained { key: String },
}

struct Pooled {
    client: Arc<McpClient>,
    refs: usize,
    idle_since: Option<Instant>,
}

/// The shared connection pool. Wrap in an `Arc` to acquire (handles hold
/// a weak reference back; a dropped pool simply stops tracking releases).
pub struct TransportPool {
    config: PoolConfig,
    connector: Connector,
    live: Mutex<BTreeMap<String, Pooled>>,
    events: Mutex<Vec<PoolEvent>>,
    enforcing: AtomicU8,
    live_total: AtomicUsize,
}

fn key_of(entry: &ServerEntry) -> String {
    let fingerprint = entry.fingerprint();
    format!("{}\u{0}{}", entry.name, fingerprint.0)
}

impl TransportPool {
    /// A pool that connects for real (stdio spawn / Streamable HTTP).
    #[must_use]
    pub fn new(config: PoolConfig) -> Arc<Self> {
        let connector: Connector =
            Arc::new(|entry: ServerEntry| Box::pin(async move { McpClient::connect(entry).await }));
        Arc::new(Self::with_connector(config, connector))
    }

    /// A pool over an injected connector (tests embed in-process wires).
    #[must_use]
    pub fn with_connector(config: PoolConfig, connector: Connector) -> Self {
        Self {
            config,
            connector,
            live: Mutex::new(BTreeMap::new()),
            events: Mutex::new(Vec::new()),
            enforcing: AtomicU8::new(0),
            live_total: AtomicUsize::new(0),
        }
    }

    /// Current live connection count (all keys).
    #[must_use]
    pub fn live_count(&self) -> usize {
        self.live_total.load(Ordering::SeqCst)
    }

    /// Acquires a handle: reuses the live connection when the key exists
    /// (bumping its refcount), otherwise connects under the budget. The
    /// ceiling counts ACTIVE (refcounted) connections; idle connections
    /// awaiting drain never hold it. At `max_live` (or while enforcing)
    /// the acquire rejects typed; the enforce level persists until active
    /// drops to `max_live - hysteresis_gap`.
    pub async fn acquire(self: &Arc<Self>, entry: &ServerEntry) -> Result<PoolHandle, PoolError> {
        let key = key_of(entry);
        {
            let mut live = self.lock_live();
            if let Some(pooled) = live.get_mut(&key) {
                pooled.refs += 1;
                pooled.idle_since = None;
                let client = Arc::clone(&pooled.client);
                return Ok(PoolHandle {
                    key,
                    client,
                    pool: Arc::downgrade(self),
                });
            }
            let count = live.values().filter(|pooled| pooled.refs > 0).count();
            if self.enforcing.load(Ordering::SeqCst) == 1 || count >= self.config.max_live {
                self.enforcing.store(1, Ordering::SeqCst);
                return Err(PoolError::BudgetExhausted {
                    limit: self.config.max_live,
                    live: count,
                });
            }
            if count >= self.config.warn_at {
                self.record_all(&[PoolEvent::BudgetWarn { live: count + 1 }]);
            }
            self.live_total.fetch_add(1, Ordering::SeqCst);
        }
        let client = match (self.connector)(entry.clone()).await {
            Ok(client) => client,
            Err(error) => {
                self.live_total.fetch_sub(1, Ordering::SeqCst);
                return Err(PoolError::Connect(error));
            }
        };
        let client = Arc::new(client);
        let mut live = self.lock_live();
        // A concurrent acquire may have won the connect race for this key;
        // last-writer-wins keeps exactly one entry per fingerprint key.
        live.insert(
            key.clone(),
            Pooled {
                client: Arc::clone(&client),
                refs: 1,
                idle_since: None,
            },
        );
        Ok(PoolHandle {
            key,
            client,
            pool: Arc::downgrade(self),
        })
    }

    /// Drains idle connections: zero-refcount past the idle window, and
    /// POISONED zero-refcount connections immediately (an unresponsive
    /// peer never lingers). Recomputes the enforce level afterwards
    /// (release at `max_live - hysteresis_gap`).
    pub fn tick(&self) -> Vec<PoolEvent> {
        let now = Instant::now();
        let mut drained = Vec::new();
        {
            let mut live = self.lock_live();
            let keys: Vec<String> = live.keys().cloned().collect();
            for key in keys {
                let Some(pooled) = live.get(&key) else {
                    continue;
                };
                let idle_long_enough = pooled
                    .idle_since
                    .is_some_and(|since| now.duration_since(since) >= self.config.idle_drain_after);
                let poisoned = pooled.client.is_poisoned();
                if pooled.refs == 0 && (idle_long_enough || poisoned) {
                    live.remove(&key);
                    self.live_total.fetch_sub(1, Ordering::SeqCst);
                    drained.push(PoolEvent::Drained { key });
                }
            }
        }
        let release = self
            .config
            .max_live
            .saturating_sub(self.config.hysteresis_gap);
        if self.live_count() <= release {
            self.enforcing.store(0, Ordering::SeqCst);
        }
        self.record_all(&drained);
        drained
    }

    /// Drains and clears recorded events (observability seam).
    pub fn take_events(&self) -> Vec<PoolEvent> {
        match self.events.lock() {
            Ok(mut events) => std::mem::take(&mut *events),
            Err(poisoned) => std::mem::take(&mut poisoned.into_inner()),
        }
    }

    fn release(&self, key: &str, poisoned: bool) {
        let mut live = self.lock_live();
        if let Some(pooled) = live.get_mut(key) {
            pooled.refs = pooled.refs.saturating_sub(1);
            // Poisoned connections skip the idle window: backdate the
            // idle marker so the very next tick drains them.
            pooled.idle_since = Some(if poisoned {
                Instant::now() - self.config.idle_drain_after
            } else {
                Instant::now()
            });
        }
        // Budget recovery counts ACTIVE (refcounted) connections only:
        // idle connections awaiting drain must not hold the ceiling.
        let active = live.values().filter(|pooled| pooled.refs > 0).count();
        let release = self
            .config
            .max_live
            .saturating_sub(self.config.hysteresis_gap);
        if active <= release {
            self.enforcing.store(0, Ordering::SeqCst);
        }
    }

    fn record_all(&self, new_events: &[PoolEvent]) {
        if new_events.is_empty() {
            return;
        }
        match self.events.lock() {
            Ok(mut events) => events.extend_from_slice(new_events),
            Err(poisoned) => poisoned.into_inner().extend_from_slice(new_events),
        }
    }

    fn lock_live(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Pooled>> {
        match self.live.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

/// A pooled connection handle. Dropping it releases the refcount; the
/// underlying connection stays live for other sessions until idle drain.
pub struct PoolHandle {
    key: String,
    client: Arc<McpClient>,
    pool: Weak<TransportPool>,
}

impl PoolHandle {
    /// The shared client (all calls go through it: roots confinement,
    /// timeouts, bounded in-flight).
    #[must_use]
    pub fn client(&self) -> &McpClient {
        &self.client
    }

    /// Re-arms the per-call deadline on the shared client.
    #[must_use]
    pub fn with_call_timeout(self, timeout: Duration) -> Self {
        self.client.set_call_timeout(timeout);
        self
    }
}

impl Drop for PoolHandle {
    fn drop(&mut self) {
        let poisoned = self.client.is_poisoned();
        if let Some(pool) = Weak::upgrade(&self.pool) {
            pool.release(&self.key, poisoned);
        }
    }
}

/// The budget guardrail level (hysteresis-aware vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetLevel {
    /// Below the warn threshold.
    Off,
    /// At/above the warn threshold (allowed, recorded).
    Warn,
    /// Enforcing (rejects) until the release threshold.
    Enforce,
}

impl TransportPool {
    /// The current budget level.
    #[must_use]
    pub fn budget_level(&self) -> BudgetLevel {
        if self.enforcing.load(Ordering::SeqCst) == 1 {
            return BudgetLevel::Enforce;
        }
        if self.live_count() >= self.config.warn_at {
            return BudgetLevel::Warn;
        }
        BudgetLevel::Off
    }
}
