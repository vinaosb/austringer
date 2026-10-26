//! The mediation policy vocabulary: which attached clients may resolve
//! an ask. Shared by the permission and plan lanes (task 1's routing
//! contract: one gate, every ask type).

/// Which attached clients may resolve an ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediationPolicy {
    /// First valid answer from any client resolves (crush pattern: one
    /// answer applies to all).
    FirstResponder,
    /// Only the designated client's answer counts.
    Designated {
        /// The client allowed to answer.
        client_id: String,
    },
    /// `quorum` agreeing allows resolve; any deny fails closed; deadline
    /// denies.
    ConsensusQuorum {
        /// How many allows are required.
        quorum: usize,
    },
    /// Only the local owner may answer; remote clients observe the ask
    /// but can never grant permissions.
    LocalOnly {
        /// The owning (local) client id.
        client_id: String,
    },
}

/// Whether `client`'s vote counts under `policy`.
pub(super) fn vote_counts(policy: &MediationPolicy, client: &str) -> bool {
    match policy {
        MediationPolicy::FirstResponder | MediationPolicy::ConsensusQuorum { .. } => true,
        MediationPolicy::Designated { client_id } | MediationPolicy::LocalOnly { client_id } => {
            client_id == client
        }
    }
}
