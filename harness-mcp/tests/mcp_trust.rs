//! T29 trust model: config fingerprints bind approval to the FULL server
//! entry (Cursor MCPoison CVE-2025-54136 TOCTOU), and org-managed
//! allowlists layer deny-wins above user trust.

use std::collections::BTreeMap;

use harness_mcp::allowlist::{OrgAllowlist, OrgVerdict};
use harness_mcp::config::{ServerEntry, TransportSpec};
use harness_mcp::trust::{MemoryTrustStore, TrustDecision, TrustStore};

fn stdio_entry(name: &str, command: &str, args: &[&str]) -> ServerEntry {
    ServerEntry::new(
        name.to_owned(),
        TransportSpec::Stdio {
            command: command.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        },
    )
}

#[test]
fn fingerprint_is_stable_across_eqivalent_env_orderings() {
    let mut env_a = BTreeMap::new();
    env_a.insert("B".to_owned(), "2".to_owned());
    env_a.insert("A".to_owned(), "1".to_owned());
    let mut a = stdio_entry("fs", "npx", &["-y", "mcp-fs"]);
    a.env = env_a;

    let mut env_b = BTreeMap::new();
    env_b.insert("A".to_owned(), "1".to_owned());
    env_b.insert("B".to_owned(), "2".to_owned());
    let mut b = stdio_entry("fs", "npx", &["-y", "mcp-fs"]);
    b.env = env_b;

    assert_eq!(a.fingerprint(), b.fingerprint());
}

#[test]
fn fingerprint_changes_on_every_identity_component() {
    let base = stdio_entry("fs", "npx", &["-y", "mcp-fs"]);
    let swapped_command = stdio_entry("fs", "node", &["-y", "mcp-fs"]);
    let swapped_arg = stdio_entry("fs", "npx", &["-y", "mcp-evil"]);
    let renamed = stdio_entry("fs2", "npx", &["-y", "mcp-fs"]);
    let mut extra_arg = stdio_entry("fs", "npx", &["-y", "mcp-fs", "--extra"]);
    let mut env_changed = stdio_entry("fs", "npx", &["-y", "mcp-fs"]);
    env_changed.env.insert("TOKEN".to_owned(), "x".to_owned());
    extra_arg.roots.push(std::path::PathBuf::from("/etc"));
    let http_entry = ServerEntry::new(
        "fs".to_owned(),
        TransportSpec::Http {
            url: "https://mcp.example.com/rpc".to_owned(),
        },
    );

    let base_fp = base.fingerprint();
    for other in [
        swapped_command,
        swapped_arg,
        renamed,
        extra_arg,
        env_changed,
        http_entry,
    ] {
        assert_ne!(base_fp, other.fingerprint(), "{other:?}");
    }
}

#[test]
fn first_use_needs_approval_then_trusts_until_entry_changes() {
    let store = MemoryTrustStore::new();
    let entry = stdio_entry("fs", "npx", &["-y", "mcp-fs"]);

    let TrustDecision::NeedsApproval { .. } = entry.trust_decision(&store) else {
        panic!("first use must ask, never auto-trust");
    };

    store.record_approval("fs", entry.fingerprint());
    assert!(matches!(
        entry.trust_decision(&store),
        TrustDecision::Trusted
    ));

    // MCPoison shape: benign-approved entry, command swapped underneath.
    let poisoned = stdio_entry("fs", "curl", &["evil.sh"]);
    let TrustDecision::ReApproval { .. } = poisoned.trust_decision(&store) else {
        panic!("changed entry must re-ask, never inherit trust");
    };
}

#[test]
fn revoked_entries_ask_again() {
    let store = MemoryTrustStore::new();
    let entry = stdio_entry("fs", "npx", &["-y", "mcp-fs"]);
    store.record_approval("fs", entry.fingerprint());
    store.revoke("fs");
    assert!(matches!(
        entry.trust_decision(&store),
        TrustDecision::NeedsApproval { .. }
    ));
}

#[test]
fn org_deny_wins_over_allow_and_over_user_trust() {
    let allowlist = OrgAllowlist::new()
        .allow_name("fs")
        .deny_stdio_command("curl")
        .allow_url_host("mcp.acme.com");

    let curl_disguised = stdio_entry("fs", "curl", &["-y", "whatever"]);
    assert!(matches!(
        allowlist.check(&curl_disguised),
        OrgVerdict::Denied { .. }
    ));
}

#[test]
fn org_allow_rules_default_deny_unknown_servers() {
    let allowlist = OrgAllowlist::new().allow_name("fs");
    let unknown = stdio_entry("other", "npx", &["mcp-other"]);
    assert!(matches!(
        allowlist.check(&unknown),
        OrgVerdict::Denied { .. }
    ));
    let known = stdio_entry("fs", "npx", &["mcp-fs"]);
    assert_eq!(allowlist.check(&known), OrgVerdict::Allowed);
}

#[test]
fn org_url_matching_requires_full_host_not_prefix() {
    let allowlist = OrgAllowlist::new().allow_url_host("mcp.acme.com");
    let evil_suffix = ServerEntry::new(
        "cloud".to_owned(),
        TransportSpec::Http {
            url: "https://mcp.acme.com.evil.com/rpc".to_owned(),
        },
    );
    assert!(matches!(
        allowlist.check(&evil_suffix),
        OrgVerdict::Denied { .. }
    ));
    let good = ServerEntry::new(
        "cloud".to_owned(),
        TransportSpec::Http {
            url: "https://mcp.acme.com/rpc".to_owned(),
        },
    );
    assert_eq!(allowlist.check(&good), OrgVerdict::Allowed);
}

#[test]
fn org_stdio_command_match_ignores_path_and_case_on_windows() {
    let allowlist = OrgAllowlist::new().deny_stdio_command("curl");
    let via_path = stdio_entry("fs", "C:\\Windows\\System32\\curl.exe", &[]);
    assert!(matches!(
        allowlist.check(&via_path),
        OrgVerdict::Denied { .. }
    ));
    let benign = stdio_entry("fs", "npx", &[]);
    assert_eq!(allowlist.check(&benign), OrgVerdict::Allowed);
}

#[test]
fn malformed_entries_rejected_typed() {
    let empty_name = ServerEntry::new(
        String::new(),
        TransportSpec::Stdio {
            command: "npx".to_owned(),
            args: vec![],
        },
    );
    assert!(empty_name.validate().is_err());

    let control_char_name = ServerEntry::new(
        "bad\u{0007}name".to_owned(),
        TransportSpec::Stdio {
            command: "npx".to_owned(),
            args: vec![],
        },
    );
    assert!(control_char_name.validate().is_err());

    let bad_url = ServerEntry::new(
        "cloud".to_owned(),
        TransportSpec::Http {
            url: "not a url".to_owned(),
        },
    );
    assert!(bad_url.validate().is_err());
}

#[test]
fn unicode_names_and_args_are_valid_identity() {
    let unicode = stdio_entry("ファイル", "npx", &["--名前=値"]);
    assert!(unicode.validate().is_ok());
    let ascii = stdio_entry("file", "npx", &["--名前=値"]);
    assert_ne!(unicode.fingerprint(), ascii.fingerprint());
}
