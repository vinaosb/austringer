//! Claude + Cursor hook-format compatibility (todo 28): readers for both
//! formats' config shapes mapping onto the unified model, unknown
//! fields/events swallowed with typed warnings, foreign garbage never
//! crashes, unicode round-trips.

mod common;

use austringer_core::hooks::compat::{CompatWarning, read_claude_settings, read_flat};
use austringer_core::hooks::{HookEvent, HookKind};

#[test]
fn reads_claude_settings_nested_format() {
    let json = serde_json::json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Bash|Write",
                    "hooks": [
                        {"type": "command", "command": "./guard.sh", "timeout": 30},
                        {"type": "http", "url": "https://hooks.internal/gate", "timeout": 5}
                    ]
                }
            ],
            "Stop": [
                {"hooks": [{"type": "command", "command": "./stop.sh"}]}
            ]
        },
        "unknown_toplevel": true
    });
    let report = read_claude_settings(&json);
    assert_eq!(report.specs.len(), 3);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| matches!(w, CompatWarning::UnknownField { .. }))
    );

    let bash = &report.specs[0];
    assert_eq!(bash.events, vec![HookEvent::PreToolUse]);
    assert_eq!(bash.tool_matcher.as_deref(), Some("Bash|Write"));
    assert_eq!(bash.timeout_secs, 30);
    assert!(matches!(&bash.kind, HookKind::Shell { command } if command == "./guard.sh"));

    let http = &report.specs[1];
    assert!(
        matches!(&http.kind, HookKind::Http { url, .. } if url == "https://hooks.internal/gate")
    );
    assert_eq!(http.timeout_secs, 5);

    let stop = &report.specs[2];
    assert_eq!(stop.events, vec![HookEvent::Stop]);
    assert_eq!(stop.tool_matcher, None);
}

#[test]
fn reads_cursor_flat_format_with_aliases() {
    let json = serde_json::json!({
        "hooks": [
            {"event": "before_tool_call", "type": "prompt", "policy": "block risky calls", "matcher": "edit_file"},
            {"event": "after_tool_call", "type": "command", "command": "./log.sh"},
            {"event": "user_prompt_submit", "type": "mcp", "server": "audit", "tool": "record_prompt"},
            {"event": "stop_hook", "type": "prompt", "policy": "iterate until done"},
            {"event": "totally_unknown_event", "type": "command", "command": "./never.sh"}
        ]
    });
    let report = read_flat(&json);
    assert_eq!(report.specs.len(), 4);
    assert!(report.warnings.iter().any(
        |w| matches!(w, CompatWarning::UnknownEvent { raw } if raw == "totally_unknown_event")
    ));

    let before = &report.specs[0];
    assert_eq!(before.events, vec![HookEvent::PreToolUse]);
    assert!(matches!(&before.kind, HookKind::Prompt { policy } if policy == "block risky calls"));

    let after = &report.specs[1];
    assert_eq!(after.events, vec![HookEvent::PostToolUse]);

    let prompt = &report.specs[2];
    assert!(
        matches!(&prompt.kind, HookKind::Mcp { server, tool } if server == "audit" && tool == "record_prompt")
    );

    let stop = &report.specs[3];
    assert_eq!(stop.events, vec![HookEvent::Stop]);
}

#[test]
fn claude_event_names_accept_snake_and_camel_forms() {
    let json = serde_json::json!({
        "hooks": {
            "pre_tool_use": [{"hooks": [{"type": "command", "command": "./a.sh"}]}],
            "SessionEnd": [{"hooks": [{"type": "command", "command": "./b.sh"}]}],
            "notification": [{"hooks": [{"type": "command", "command": "./c.sh"}]}]
        }
    });
    let report = read_claude_settings(&json);
    assert_eq!(report.specs.len(), 3);
    // Object keys iterate sorted; assert by event, not position.
    let command_for = |event: HookEvent| {
        report
            .specs
            .iter()
            .find(|spec| spec.events == vec![event])
            .map(|spec| match &spec.kind {
                HookKind::Shell { command } => command.clone(),
                _ => String::new(),
            })
    };
    assert_eq!(
        command_for(HookEvent::PreToolUse).as_deref(),
        Some("./a.sh")
    );
    assert_eq!(
        command_for(HookEvent::SessionEnd).as_deref(),
        Some("./b.sh")
    );
    assert_eq!(
        command_for(HookEvent::Notification).as_deref(),
        Some("./c.sh")
    );
}

#[test]
fn foreign_garbage_never_crashes() {
    let not_object = serde_json::json!("just a string");
    assert!(read_claude_settings(&not_object).specs.is_empty());
    assert!(read_flat(&not_object).specs.is_empty());

    let hooks_is_number = serde_json::json!({"hooks": 42});
    assert!(read_claude_settings(&hooks_is_number).specs.is_empty());
    assert!(read_flat(&hooks_is_number).specs.is_empty());

    let entries_are_garbage = serde_json::json!({
        "hooks": [
            "a bare string",
            7,
            {"event": null, "type": null},
            {"event": "Stop", "type": "command"}
        ]
    });
    let report = read_flat(&entries_are_garbage);
    assert_eq!(report.specs.len(), 1);
    assert!(!report.warnings.is_empty());

    let timeout_garbage = serde_json::json!({
        "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "./x.sh", "timeout": "soon"}]}]}
    });
    let report = read_claude_settings(&timeout_garbage);
    assert_eq!(report.specs.len(), 1);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| matches!(w, CompatWarning::InvalidTimeout { .. }))
    );
    assert_eq!(
        report.specs[0].timeout_secs,
        austringer_core::hooks::DEFAULT_HOOK_TIMEOUT_SECS
    );

    let unknown_hook_type = serde_json::json!({
        "hooks": {"Stop": [{"hooks": [{"type": "websocket", "endpoint": "wss://x"}]}]}
    });
    let report = read_claude_settings(&unknown_hook_type);
    assert!(report.specs.is_empty());
    assert!(
        report
            .warnings
            .iter()
            .any(|w| matches!(w, CompatWarning::UnknownHookType { raw } if raw == "websocket"))
    );
}

#[test]
fn timeout_is_capped_at_ceiling() {
    let json = serde_json::json!({
        "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "./x.sh", "timeout": 3600}]}]}
    });
    let report = read_claude_settings(&json);
    assert_eq!(
        report.specs[0].timeout_secs,
        austringer_core::hooks::HOOK_TIMEOUT_CEILING_SECS
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| matches!(w, CompatWarning::TimeoutCapped { .. }))
    );
}

#[test]
fn unicode_commands_and_policies_round_trip() {
    let json = serde_json::json!({
        "hooks": [
            {"event": "user_prompt_submit", "type": "command", "command": "./豊か-にする.sh 🦀"},
            {"event": "stop_hook", "type": "prompt", "policy": "繰り返す until 🎯 done"}
        ]
    });
    let report = read_flat(&json);
    assert_eq!(report.specs.len(), 2);
    assert!(
        matches!(&report.specs[0].kind, HookKind::Shell { command } if command == "./豊か-にする.sh 🦀")
    );
    assert!(
        matches!(&report.specs[1].kind, HookKind::Prompt { policy } if policy == "繰り返す until 🎯 done")
    );
}

#[test]
fn matcher_pipe_lists_map_to_tool_matcher() {
    let json = serde_json::json!({
        "hooks": {
            "PreToolUse": [
                {"matcher": "Bash", "hooks": [{"type": "command", "command": "./a.sh"}]}
            ]
        }
    });
    let report = read_claude_settings(&json);
    assert_eq!(report.specs[0].tool_matcher.as_deref(), Some("Bash"));
}
