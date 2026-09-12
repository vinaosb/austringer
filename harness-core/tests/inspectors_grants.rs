//! Grant-memory tests (todo 7): the in-memory permission store's scopes
//! and clearing, `a`-grant auto-allow for matching calls, and
//! deny-rule-beats-grant precedence.

use std::sync::Arc;

use harness_core::agent_loop::PendingCall;
use harness_core::inspectors::permission::{Effect, PermissionConfig, PermissionInspector, Rule};
use harness_core::inspectors::secret_files::TranscriptRedactor;
use harness_core::inspectors::store::{Grant, GrantScope, MemoryStore, PermissionStore};
use harness_core::inspectors::verdict::{CallInspector, Verdict};

fn call(tool: &str, arguments: serde_json::Value) -> PendingCall {
    PendingCall {
        call_id: format!("call-{tool}"),
        tool: tool.to_owned(),
        arguments,
    }
}

async fn inspect(
    inspector: &mut PermissionInspector,
    tool: &str,
    arguments: serde_json::Value,
) -> Verdict {
    inspector
        .inspect(&call(tool, arguments))
        .await
        .expect("inspect")
}

#[test]
fn memory_store_scopes_and_clearing() {
    let store = MemoryStore::new();
    store
        .record(&Grant {
            tool: "read_file".to_owned(),
            target: Some("secrets/token.txt".to_owned()),
            scope: GrantScope::Session,
        })
        .expect("record");
    assert_eq!(
        store.lookup("read_file", Some("secrets/token.txt")),
        Some(GrantScope::Session)
    );
    assert_eq!(store.lookup("read_file", Some("secrets/other.txt")), None);
    assert_eq!(store.lookup("other_tool", Some("secrets/token.txt")), None);

    store
        .record(&Grant {
            tool: "read_file".to_owned(),
            target: Some("secrets/forever.txt".to_owned()),
            scope: GrantScope::Forever,
        })
        .expect("record");
    store.clear_session().expect("clear");
    assert_eq!(store.lookup("read_file", Some("secrets/token.txt")), None);
    assert_eq!(
        store.lookup("read_file", Some("secrets/forever.txt")),
        Some(GrantScope::Forever)
    );
    // Windows-separator normalization on lookup keys.
    store
        .record(&Grant {
            tool: "read_file".to_owned(),
            target: Some("docs/readme.md".to_owned()),
            scope: GrantScope::Session,
        })
        .expect("record");
    assert_eq!(
        store.lookup("read_file", Some(r"docs\readme.md")),
        Some(GrantScope::Session)
    );
}

#[tokio::test]
async fn stored_grant_auto_allows_matching_calls() {
    let store = MemoryStore::new();
    let mut insp = PermissionInspector::new(
        PermissionConfig { rules: vec![] },
        Arc::new(store.clone()),
        TranscriptRedactor::default(),
    );
    store
        .record(&Grant {
            tool: "read_file".to_owned(),
            target: Some("secrets/token.txt".to_owned()),
            scope: GrantScope::Session,
        })
        .expect("record");
    // The 'a' grant: an identical call auto-allows with authority.
    assert_eq!(
        inspect(
            &mut insp,
            "read_file",
            serde_json::json!({"path": "secrets/token.txt"})
        )
        .await,
        Verdict::Allow {
            authoritative: true
        }
    );
    // The grant is target-scoped: a sibling path gets no opinion from
    // this inspector (the chain's fallback ask covers it).
    assert!(matches!(
        inspect(
            &mut insp,
            "read_file",
            serde_json::json!({"path": "secrets/other.txt"})
        )
        .await,
        Verdict::Allow {
            authoritative: false
        }
    ));
}

#[tokio::test]
async fn deny_rule_beats_stored_grant() {
    let store = MemoryStore::new();
    store
        .record(&Grant {
            tool: "read_file".to_owned(),
            target: Some("secrets/token.txt".to_owned()),
            scope: GrantScope::Forever,
        })
        .expect("record");
    let mut insp = PermissionInspector::new(
        PermissionConfig {
            rules: vec![Rule::path(Effect::Deny, "read_file", "secrets/**").expect("valid")],
        },
        Arc::new(store),
        TranscriptRedactor::default(),
    );
    assert!(matches!(
        inspect(
            &mut insp,
            "read_file",
            serde_json::json!({"path": "secrets/token.txt"})
        )
        .await,
        Verdict::Deny { .. }
    ));
}
