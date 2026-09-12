//! Permission memory: the grant store trait (todo 8 persists this to
//! SQLite), the in-memory implementation, and the ask-resolution policy
//! seam that decides what happens to an approval request.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

/// How long a user grant lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GrantScope {
    /// Valid for the rest of this session (`y`-style one-off approvals are
    /// not stored at all; `a`-with-session answers land here).
    Session,
    /// Valid across sessions until revoked.
    Forever,
}

/// One stored user grant: `tool` + optional normalized `target` (path or
/// command text) that the user approved with `a` semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// Tool name the grant covers (exact match).
    pub tool: String,
    /// Normalized target value, if the approved call carried one.
    pub target: Option<String>,
    /// How long the grant lives.
    pub scope: GrantScope,
}

/// Why a store operation failed. Persistence failures are surfaced loudly
/// (never silently degraded to allow).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("permission store failure: {message}")]
pub struct StoreError {
    /// Human-readable cause.
    pub message: String,
}

/// Session/forever grant memory. Methods take `&self` — implementations own
/// their interior mutability (a connection pool is naturally `&self`), which
/// lets the chain and the permission inspector share one store. Todo 8
/// replaces the in-memory default with SQLite behind this same trait.
pub trait PermissionStore: Send + Sync {
    /// Records (or overwrites) a grant.
    fn record(&self, grant: &Grant) -> Result<(), StoreError>;

    /// The scope of the stored grant for exactly this tool + target, if any.
    fn lookup(&self, tool: &str, target: Option<&str>) -> Option<GrantScope>;

    /// Drops all session-scoped grants (session end). Forever grants stay.
    fn clear_session(&self) -> Result<(), StoreError>;
}

/// The grant map key: tool plus optional normalized target.
type GrantKey = (String, Option<String>);

/// In-memory [`PermissionStore`]: a shared, cloneable map keyed by
/// `(tool, target)`. Clones observe the same underlying storage.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    inner: Arc<Mutex<HashMap<GrantKey, GrantScope>>>,
}

impl MemoryStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HashMap<GrantKey, GrantScope>>, StoreError> {
        self.inner.lock().map_err(|_| StoreError {
            message: "memory store mutex poisoned".to_owned(),
        })
    }

    fn normalize(target: Option<&str>) -> Option<String> {
        target.map(|value| value.replace('\\', "/"))
    }
}

impl PermissionStore for MemoryStore {
    fn record(&self, grant: &Grant) -> Result<(), StoreError> {
        self.lock()?.insert(
            (grant.tool.clone(), Self::normalize(grant.target.as_deref())),
            grant.scope,
        );
        Ok(())
    }

    fn lookup(&self, tool: &str, target: Option<&str>) -> Option<GrantScope> {
        self.lock().ok().and_then(|grants| {
            grants
                .get(&(tool.to_owned(), Self::normalize(target)))
                .copied()
        })
    }

    fn clear_session(&self) -> Result<(), StoreError> {
        self.lock()?
            .retain(|_, scope| *scope != GrantScope::Session);
        Ok(())
    }
}

/// One approval request routed to the user-facing ask policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskRequest {
    /// Tool that wants to run.
    pub tool: String,
    /// The real (unredacted) normalized target, if the call carried one —
    /// the approving human needs it; the model never sees this struct.
    pub target: Option<String>,
    /// Redacted, model-safe reason (reused verbatim as feedback if the ask
    /// is refused).
    pub reason: String,
}

/// The user's answer to an [`AskRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskResolution {
    /// Not approved (headless default included).
    Refuse,
    /// Approved for this one call (`y`): nothing is persisted.
    GrantOnce,
    /// Approved for the pattern (`a`), stored at the given scope.
    GrantAlways(GrantScope),
}

/// How approval asks are resolved. The default [`RefuseAsks`] models the
/// headless posture: an unresolved ask is a refusal, never a silent allow.
#[async_trait]
pub trait AskPolicy: Send {
    /// Decides one approval request.
    async fn resolve(&mut self, request: AskRequest) -> AskResolution;
}

/// Headless default ask policy: refuse everything.
#[derive(Debug, Clone, Copy, Default)]
pub struct RefuseAsks;

#[async_trait]
impl AskPolicy for RefuseAsks {
    async fn resolve(&mut self, _request: AskRequest) -> AskResolution {
        AskResolution::Refuse
    }
}
