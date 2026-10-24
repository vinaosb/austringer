//! The userland command egress guard (todo 4, ecosystem alignment):
//! the filter-proxy fallback verdict layer applied at exec seams whose
//! spawn lane has no kernel network isolation. Pure string-pattern
//! matching — the guard never opens a socket.

use super::{DenyReason, EgressPolicy, EgressVerdict};

/// Program words whose purpose is network egress. A command segment
/// led by one of these (after path/extension normalization, so
/// `curl.exe` and `/usr/bin/curl` match) is judged against the egress
/// allowlist before it may run.
const EGRESS_PROGRAMS: &[&str] = &[
    "curl", "wget", "ftp", "sftp", "scp", "ssh", "nc", "ncat", "telnet", "ping",
];

/// The verdict for one command line under the userland egress guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandEgress {
    /// No segment attempts network egress.
    NoEgress,
    /// Every network-shaped segment's destination is allowlisted.
    Allowed,
    /// A network-shaped segment cannot run: the typed, fail-closed
    /// denial naming the offending segment and (when one was parsed)
    /// the destination host.
    Denied {
        /// The offending segment text.
        segment: String,
        /// The destination host when one was resolvable.
        host: Option<String>,
        /// Why the segment was denied.
        reason: DenyReason,
    },
}

/// The userland egress guard over raw command TEXT: segments the
/// compound command exactly like the permission layer (`npm i && curl
/// …` judges the curl segment on its own — an allowed prefix never
/// covers an appended egress attempt), and for every segment led by an
/// egress-shaped program word, resolves its destination tokens and
/// verdicts them against `policy`.
///
/// Fail-closed by construction: a command that cannot be segmented, an
/// egress program with no destination tokens, and a destination the
/// guard cannot resolve into a (host, port) pair are ALL denials — the
/// guard denies what it cannot prove allowed.
#[must_use]
pub fn guard_command(command: &str, policy: &EgressPolicy) -> CommandEgress {
    let segments = match crate::perm::commands::split_compound(command) {
        Ok(segments) => segments,
        Err(_) => {
            return CommandEgress::Denied {
                segment: command.to_owned(),
                host: None,
                reason: DenyReason::MalformedHost,
            };
        }
    };
    let mut any_egress = false;
    for segment in &segments {
        if !is_egress_program(&segment.program_word) {
            continue;
        }
        any_egress = true;
        let destinations = segment
            .text
            .split_whitespace()
            .skip(1)
            .filter(|token| !token.starts_with('-'));
        let mut resolved = false;
        for token in destinations {
            let Some((host, port)) = destination_host(token) else {
                // Unparsable evidence: deny rather than guess.
                return CommandEgress::Denied {
                    segment: segment.text.clone(),
                    host: None,
                    reason: DenyReason::MalformedHost,
                };
            };
            resolved = true;
            if let EgressVerdict::Denied { reason, .. } = policy.verdict(&host, port) {
                return CommandEgress::Denied {
                    segment: segment.text.clone(),
                    host: Some(host),
                    reason,
                };
            }
        }
        if !resolved {
            // Flags only: the intent is network, the destination is
            // hidden.
            return CommandEgress::Denied {
                segment: segment.text.clone(),
                host: None,
                reason: DenyReason::MalformedHost,
            };
        }
    }
    if any_egress {
        CommandEgress::Allowed
    } else {
        CommandEgress::NoEgress
    }
}

/// Whether a (possibly quoted, path-qualified, extension-carrying)
/// program word names a known egress program.
fn is_egress_program(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    let Some(component) = lower.rsplit(['/', '\\']).next() else {
        return false;
    };
    let Some(stem) = component.split('.').next() else {
        return false;
    };
    EGRESS_PROGRAMS.contains(&stem)
}

/// Reduce one destination token to a (host, port) pair: strips a URL
/// scheme (deriving the default port), path/query/fragment, and
/// userinfo, and parses an explicit port. Anything else — IPv6
/// literals, embedded colons, empty hosts — is unresolvable.
fn destination_host(token: &str) -> Option<(String, u16)> {
    let (mut port, rest) = match token.split_once("://") {
        Some((scheme, rest)) => (
            match scheme {
                "http" => 80,
                "https" => 443,
                "ftp" => 21,
                _ => 0,
            },
            rest,
        ),
        None => (0, token),
    };
    let authority = rest.split(['/', '\\', '?', '#']).next()?;
    let hostport = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    if hostport.is_empty() {
        return None;
    }
    let parts: Vec<&str> = hostport.split(':').collect();
    let host = match parts.as_slice() {
        [host] => (*host).to_owned(),
        [host, explicit] => {
            port = explicit.parse().ok()?;
            (*host).to_owned()
        }
        _ => return None,
    };
    if host.is_empty() {
        return None;
    }
    Some((host, port))
}
