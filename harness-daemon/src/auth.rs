//! Hardened loopback auth: generated bearer token, peer-address trust,
//! and a defensive Host-header rejection layer. The Host header is NEVER
//! an auth factor (dsh CVE-2026-82533 class: DNS-rebinding sends an
//! attacker-chosen Host at a loopback listener) — it is only ever an
//! additional reason to refuse.

use std::net::SocketAddr;

/// Auth-layer failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    /// OS entropy source unavailable.
    #[error("entropy unavailable: {0}")]
    Entropy(String),
}

/// Token byte length (before hex encoding) for generated tokens.
pub const TOKEN_BYTES: usize = 32;

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(HEX[usize::from(b >> 4)]));
        out.push(char::from(HEX[usize::from(b & 0x0f)]));
    }
    out
}

/// Generate a fresh daemon token from OS entropy (hex-encoded).
pub fn generate_token() -> Result<String, AuthError> {
    let mut buf = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut buf).map_err(|e| AuthError::Entropy(e.to_string()))?;
    Ok(hex(&buf))
}

/// Constant-time equality (length check aside, no early exit).
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Presented token check. Constant-time on the token bytes.
#[must_use]
pub fn token_matches(expected: &str, presented: &str) -> bool {
    constant_time_eq(expected.as_bytes(), presented.as_bytes())
}

/// Loopback-only peer policy: the daemon binds 127.0.0.1 by default and
/// refuses anything arriving from a non-loopback peer regardless of
/// token. Trust lives in the kernel-reported peer address, never in a
/// header.
#[must_use]
pub fn peer_allowed(addr: SocketAddr) -> bool {
    addr.ip().is_loopback()
}

/// Defensively reject Host headers that are not loopback names. This is
/// NOT authentication — every request still needs the token. It exists
/// so a rebinding probe (browser resolving `attacker.example` to
/// 127.0.0.1) is refused even before token evaluation, and so a future
/// refactor can never accidentally start trusting Host.
#[must_use]
pub fn host_allowed(host_header: &str) -> bool {
    let host = host_header.trim();
    if host.is_empty() {
        return false;
    }
    let name = match host.rsplit_once(':') {
        Some((name, port)) if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() => name,
        _ => host,
    };
    matches!(name, "127.0.0.1" | "localhost" | "[::1]" | "::1")
}

/// The full gate every daemon transport applies, in order: peer, host,
/// token. Returns the first refusal reason.
#[must_use]
pub fn check(
    peer: SocketAddr,
    host_header: Option<&str>,
    expected_token: &str,
    presented_token: Option<&str>,
) -> AuthVerdict {
    if !peer_allowed(peer) {
        return AuthVerdict::RefusedPeer;
    }
    if let Some(host) = host_header
        && !host_allowed(host)
    {
        return AuthVerdict::RefusedHost;
    }
    match presented_token {
        None => AuthVerdict::RefusedNoToken,
        Some(presented) => {
            if token_matches(expected_token, presented) {
                AuthVerdict::Allowed
            } else {
                AuthVerdict::RefusedTokenMismatch
            }
        }
    }
}

/// Outcome of [`check`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthVerdict {
    /// All gates passed.
    Allowed,
    /// Non-loopback peer.
    RefusedPeer,
    /// Host header not in the loopback set.
    RefusedHost,
    /// No token presented.
    RefusedNoToken,
    /// Token did not match.
    RefusedTokenMismatch,
}

impl AuthVerdict {
    /// True only for [`AuthVerdict::Allowed`].
    #[must_use]
    pub const fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_are_hex_and_unique() {
        let a = generate_token().unwrap_or_default();
        let b = generate_token().unwrap_or_default();
        assert_eq!(a.len(), TOKEN_BYTES * 2);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn token_compare_is_exact() {
        let expected = "abcdef0123456789";
        assert!(token_matches(expected, "abcdef0123456789"));
        assert!(!token_matches(expected, "abcdef0123456788"));
        assert!(!token_matches(expected, "short"));
    }

    fn addr(s: &str) -> SocketAddr {
        use std::net::ToSocketAddrs;
        s.to_socket_addrs()
            .ok()
            .and_then(|mut it| it.next())
            .unwrap_or(SocketAddr::from(([127, 0, 0, 1], 1)))
    }

    #[test]
    fn peer_policy_is_loopback_only() {
        assert!(peer_allowed(addr("127.0.0.1:51000")));
        assert!(peer_allowed(addr("127.0.0.99:1")));
        assert!(peer_allowed(addr("[::1]:1")));
        assert!(!peer_allowed(addr("192.168.1.5:1")));
        assert!(!peer_allowed(addr("8.8.8.8:1")));
    }

    #[test]
    fn host_policy_accepts_only_loopback_names() {
        for ok in [
            "127.0.0.1",
            "127.0.0.1:8907",
            "localhost",
            "localhost:8907",
            "[::1]",
            "[::1]:8907",
        ] {
            assert!(host_allowed(ok), "should allow {ok}");
        }
        for evil in [
            "evil.example.com",
            "evil.example.com:8907",
            "127.0.0.1.evil.com",
            "localhost.evil.com",
            "",
            "unix",
        ] {
            assert!(!host_allowed(evil), "should refuse {evil}");
        }
    }

    #[test]
    fn full_gate_order_peer_host_token() {
        let token = "tok";
        let lo = addr("127.0.0.1:9");
        let remote = addr("10.0.0.5:9");

        // CVE-2026-82533-class negative: spoofed Host + no token.
        assert_eq!(
            check(remote, Some("evil.example.com"), token, None),
            AuthVerdict::RefusedPeer
        );
        assert_eq!(
            check(lo, Some("evil.example.com"), token, None),
            AuthVerdict::RefusedHost
        );
        assert_eq!(
            check(lo, Some("evil.example.com"), token, Some("tok")),
            AuthVerdict::RefusedHost
        );
        assert_eq!(
            check(lo, Some("127.0.0.1"), token, None),
            AuthVerdict::RefusedNoToken
        );
        assert_eq!(
            check(lo, Some("127.0.0.1"), token, Some("wrong")),
            AuthVerdict::RefusedTokenMismatch
        );
        assert_eq!(check(lo, None, token, Some("tok")), AuthVerdict::Allowed);
        assert!(check(lo, Some("localhost:1"), token, Some("tok")).is_allowed());
    }
}
