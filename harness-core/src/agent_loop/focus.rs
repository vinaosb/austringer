//! Focus-chain anchor: the model-maintained todo list rendered as a stable
//! environment block every turn, typed completion blocking, and the
//! plan.md hard-block gate.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Identifier of a focus-chain todo. Distinct from any other id kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TodoId(pub u64);

impl From<u64> for TodoId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// One todo in the focus chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusTodo {
    /// Stable identity of the todo.
    pub id: TodoId,
    /// What remains to do.
    pub text: String,
    /// Whether the todo is closed.
    pub done: bool,
}

/// The focus chain: the task's todo list, re-injected into every inference
/// request as a stable environment block (Roo's anti-drift anchor).
///
/// Rendering is serde-stable in spirit: todos are keyed in a `BTreeMap`
/// (sorted by id), no timestamps, no ambient data — identical todo state
/// renders byte-identically across turns, so a later prefix-cache layer
/// (todo 11) can rely on it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FocusChain {
    todos: BTreeMap<TodoId, FocusTodo>,
}

impl FocusChain {
    /// An empty chain.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or replaces the todo with this id.
    pub fn upsert(&mut self, id: TodoId, text: impl Into<String>) {
        self.todos.insert(
            id,
            FocusTodo {
                id,
                text: text.into(),
                done: false,
            },
        );
    }

    /// Marks the todo done; returns whether it existed and was open.
    pub fn mark_done(&mut self, id: TodoId) -> bool {
        match self.todos.get_mut(&id) {
            Some(todo) if !todo.done => {
                todo.done = true;
                true
            }
            _ => false,
        }
    }

    /// Number of open (not done) todos.
    #[must_use]
    pub fn open_count(&self) -> usize {
        self.todos.values().filter(|todo| !todo.done).count()
    }

    /// True when no open todo remains.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.open_count() == 0
    }

    /// The stable environment block for this state. Byte-identical for
    /// identical todo state regardless of insertion order.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::from("<focus-chain>\n");
        out.push_str(&format!(
            "status: {} open / {} total\n",
            self.open_count(),
            self.todos.len()
        ));
        for todo in self.todos.values() {
            let state = if todo.done { "done" } else { "open" };
            out.push_str(&format!("[{state}] {}: {}\n", todo.id.0, todo.text));
        }
        out.push_str("</focus-chain>");
        out
    }

    /// The typed blocked state to re-prompt the model with, when open todos
    /// forbid task completion (`None` when nothing blocks). The rendering
    /// lists ONLY the open todos — exactly what remains to be done.
    #[must_use]
    pub fn completion_block(&self) -> Option<CompletionBlock> {
        if self.is_complete() {
            return None;
        }
        let open_count = self.open_count();
        let mut rendered = String::from("<focus-chain open>\n");
        rendered.push_str(&format!(
            "status: {open_count} open / {} total\n",
            self.todos.len()
        ));
        for todo in self.todos.values().filter(|todo| !todo.done) {
            rendered.push_str(&format!("[open] {}: {}\n", todo.id.0, todo.text));
        }
        rendered.push_str("</focus-chain>");
        Some(CompletionBlock::OpenTodos {
            open_count,
            rendered,
        })
    }
}

/// Why task-level completion was refused. Carries the open-chain rendering
/// so the re-prompt shows the model exactly what remains (Roo's
/// `preventCompletionWithOpenTodos`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionBlock {
    /// Open todos remain in the focus chain.
    OpenTodos {
        /// How many todos are open.
        open_count: usize,
        /// The rendered open chain for the re-prompt.
        rendered: String,
    },
    /// A registered plan.md is still hard-blocked.
    PlanGate {
        /// The notice shown to the model.
        rendered: String,
    },
}

impl CompletionBlock {
    /// Number of open todos causing the block (0 for the plan gate).
    #[must_use]
    pub fn open_count(&self) -> usize {
        match self {
            Self::OpenTodos { open_count, .. } => *open_count,
            Self::PlanGate { .. } => 0,
        }
    }
}

/// A plan.md hard-block handle the daemon wires; the loop enforces it:
/// while hard-blocked, task-level completion is deferred.
///
/// A handle type (cloneable, shared) so the registering side can clear the
/// gate from anywhere without touching the loop.
#[derive(Debug, Clone, Default)]
pub struct PlanGate {
    blocked: Arc<AtomicBool>,
}

impl PlanGate {
    /// An unblocked gate.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the plan.md hard-block.
    pub fn hard_block(&self) {
        self.blocked.store(true, Ordering::Release);
    }

    /// Clears the hard-block (the plan was accepted/deleted).
    pub fn clear(&self) {
        self.blocked.store(false, Ordering::Release);
    }

    /// Whether task-level completion is currently forbidden.
    #[must_use]
    pub fn is_hard_blocked(&self) -> bool {
        self.blocked.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_is_stable_across_insertion_orders() {
        let mut a = FocusChain::new();
        a.upsert(TodoId(2), "two");
        a.upsert(TodoId(1), "one");
        let mut b = FocusChain::new();
        b.upsert(TodoId(1), "one");
        b.upsert(TodoId(2), "two");
        assert_eq!(a.render(), b.render());
    }

    #[test]
    fn empty_chain_renders_minimal_block() {
        let chain = FocusChain::new();
        assert_eq!(
            chain.render(),
            "<focus-chain>\nstatus: 0 open / 0 total\n</focus-chain>"
        );
        assert_eq!(chain.completion_block(), None);
    }
}
