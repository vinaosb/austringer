//! Outposts-style reconciliation (Devin §22): workers claim sessions
//! from NAMED QUEUES; a claim EXPIRES at its deadline and the sweep
//! re-enqueues the session — it SELF-HEALS back into the queue without
//! operator action. Pure injected-clock state machine (deterministic
//! under fake time).

use std::collections::HashMap;

/// One active claim by a worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// Claimed session id.
    pub session: String,
    /// Queue it was claimed from.
    pub queue: String,
    /// Claiming worker id.
    pub worker: String,
    /// Claim time (epoch seconds, injected clock).
    pub claimed_at: u64,
    /// Deadline (epoch seconds); expiry ⇒ self-heal.
    pub deadline: u64,
}

/// Typed claim ledger over named queues.
#[derive(Default)]
pub struct ClaimLedger {
    queues: HashMap<String, Vec<String>>,
    claims: HashMap<String, Claim>,
}

impl ClaimLedger {
    /// Empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueue a session on a named queue (back of the line).
    pub fn enqueue(&mut self, queue: &str, session: &str) {
        self.queues
            .entry(queue.to_owned())
            .or_default()
            .push(session.to_owned());
    }

    /// Queue depth for a named queue.
    #[must_use]
    pub fn depth(&self, queue: &str) -> usize {
        self.queues.get(queue).map_or(0, Vec::len)
    }

    /// Claim the head session of `queue` for `worker`, valid until
    /// `now + ttl`. A session already actively claimed cannot be
    /// double-claimed (its queue entry is not present then).
    pub fn claim_next(&mut self, queue: &str, worker: &str, now: u64, ttl: u64) -> Option<Claim> {
        let waiting = self.queues.get_mut(queue)?;
        if waiting.is_empty() {
            return None;
        }
        let session = waiting.remove(0);
        let claim = Claim {
            session: session.clone(),
            queue: queue.to_owned(),
            worker: worker.to_owned(),
            claimed_at: now,
            deadline: now.saturating_add(ttl),
        };
        self.claims.insert(session, claim.clone());
        Some(claim)
    }

    /// Mark a claimed session complete (removes the claim).
    pub fn complete(&mut self, session: &str) -> bool {
        self.claims.remove(session).is_some()
    }

    /// The active claim on a session, if any.
    #[must_use]
    pub fn claim_on(&self, session: &str) -> Option<&Claim> {
        self.claims.get(session)
    }

    /// Expiry sweep: every claim past its deadline is removed and its
    /// session RE-ENQUEUED (self-heal). Returns the healed sessions.
    pub fn sweep(&mut self, now: u64) -> Vec<Claim> {
        let expired: Vec<Claim> = self
            .claims
            .values()
            .filter(|c| now >= c.deadline)
            .cloned()
            .collect();
        for claim in &expired {
            self.claims.remove(&claim.session);
            self.queues
                .entry(claim.queue.clone())
                .or_default()
                .push(claim.session.clone());
        }
        expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claims_serve_fifo_per_named_queue() {
        let mut ledger = ClaimLedger::new();
        ledger.enqueue("reviews", "s1");
        ledger.enqueue("reviews", "s2");
        ledger.enqueue("nightly", "n1");
        let c1 = ledger.claim_next("reviews", "w1", 0, 100);
        assert_eq!(c1.map(|c| c.session), Some("s1".to_owned()));
        let c2 = ledger.claim_next("reviews", "w2", 1, 100);
        assert_eq!(c2.map(|c| c.session), Some("s2".to_owned()));
        assert_eq!(ledger.depth("reviews"), 0);
        assert_eq!(ledger.depth("nightly"), 1);
    }

    #[test]
    fn claim_expiry_self_heals_session_back_to_queue() {
        let mut ledger = ClaimLedger::new();
        ledger.enqueue("builds", "b1");
        let claimed = ledger.claim_next("builds", "w1", 0, 10);
        assert!(claimed.is_some());
        assert_eq!(ledger.claim_on("b1").map(|c| c.worker.as_str()), Some("w1"));

        // Before the deadline: nothing heals.
        assert!(ledger.sweep(9).is_empty());
        // At/after the deadline: the claim expires and the session is
        // re-enqueued on its ORIGINAL queue.
        let healed = ledger.sweep(10);
        assert_eq!(healed.len(), 1);
        assert_eq!(healed[0].session, "b1");
        assert_eq!(healed[0].queue, "builds");
        assert_eq!(ledger.depth("builds"), 1);
        assert!(ledger.claim_on("b1").is_none());
    }

    #[test]
    fn healed_session_is_reclaimable_by_another_worker() {
        let mut ledger = ClaimLedger::new();
        ledger.enqueue("builds", "b1");
        assert!(ledger.claim_next("builds", "w1", 0, 5).is_some());
        let healed = ledger.sweep(5);
        assert_eq!(healed.len(), 1);
        let second = ledger.claim_next("builds", "w2", 6, 5);
        assert_eq!(
            second.map(|c| (c.session, c.worker)),
            Some(("b1".to_owned(), "w2".to_owned()))
        );
        assert!(ledger.sweep(10).is_empty(), "fresh claim must not heal");
    }

    #[test]
    fn complete_removes_claim_without_reenqueue() {
        let mut ledger = ClaimLedger::new();
        ledger.enqueue("q", "s1");
        assert!(ledger.claim_next("q", "w1", 0, 100).is_some());
        assert!(ledger.complete("s1"));
        assert!(ledger.sweep(1000).is_empty());
        assert_eq!(ledger.depth("q"), 0);
    }

    #[test]
    fn empty_queue_claim_is_none() {
        let mut ledger = ClaimLedger::new();
        assert!(ledger.claim_next("missing", "w1", 0, 10).is_none());
    }
}
