//! OS-native sandbox backends, egress control, and credential hygiene
//! (plan todo 26).
//!
//! One typed [`SandboxPolicy`](policy::decl::SandboxPolicy), many
//! backends (Copilot MXC pattern): Windows AppContainer + ACLs + Job
//! Objects (runtime-tested), Linux Landlock + seccomp (CI-owned),
//! macOS Seatbelt (profile generation tested cross-platform), and an
//! optional hardened container backend. Egress is a typed allowlist
//! that starts EMPTY (zero vendor-escape domains), backed by a userland
//! filter-proxy fallback where the kernel lacks host-level network
//! isolation. Every sandboxed run reports requested-vs-enforced with
//! named gaps — isolation is never claimed where it was not
//! established, and unenforceable isolation REFUSES to start.
#![deny(missing_docs)]

pub mod backends;
pub mod creds;
pub mod egress;
pub mod oidc;
pub mod perm;
pub mod policy;
pub mod report;
pub mod scrub;
pub mod supply;
