//! Deferred tool loading (todo 6): `tool_search` hydrates the FULL schema
//! of matched tools INTO the observation of the search call itself — the
//! serialized base schema (the system-prefix tool section) is never
//! touched — followed by a hydrate-then-call roundtrip through the
//! registry dispatch path. Unknown/garbage/empty/unicode queries degrade
//! to typed suggestions, never a panic.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use austringer_core::tools::{
    IoCap, ToolDispatchError, ToolProvider, ToolRegistry, ToolSchema, ToolSearchError,
};
use common::TempSession;
use serde_json::{Value, json};

/// Stub executor returning a fixed payload.
struct StubExecutor {
    output: Vec<u8>,
}

#[async_trait]
impl ToolProvider for StubExecutor {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "edit_lines".to_owned(),
            description: "stub edit executor".to_owned(),
            parameters: json!({ "type": "object" }),
        }
    }

    async fn execute(
        &self,
        _arguments: &Value,
    ) -> Result<Vec<u8>, austringer_core::tools::ProviderError> {
        Ok(self.output.clone())
    }
}

/// Stub aux tool with a matchable description.
struct AuxStub {
    name: &'static str,
    description: &'static str,
}

#[async_trait]
impl ToolProvider for AuxStub {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name.to_owned(),
            description: self.description.to_owned(),
            parameters: json!({ "type": "object" }),
        }
    }

    async fn execute(
        &self,
        _arguments: &Value,
    ) -> Result<Vec<u8>, austringer_core::tools::ProviderError> {
        Ok(Vec::new())
    }
}

fn open_registry(name: &str) -> ToolRegistry {
    ToolRegistry::open(TempSession::new(name).path(), IoCap::DEFAULT).expect("open registry")
}

#[tokio::test]
async fn tool_search_hydrates_full_schema_into_observation() {
    let registry = open_registry("hydrate-full");
    let pinned = registry.pin_current();
    let prefix_before = registry.render_base_schema(&pinned);

    let result = registry
        .dispatch(&pinned, "tool_search", &json!({ "query": "edit_lines" }))
        .await
        .expect("tool_search succeeds");

    // The observation carries the COMPLETE schema for the matched tool:
    // name, description, and the full parameters object. Compare the
    // hydrated payload structurally against the core-set schema (byte
    // stability of the rendering is locked by the schema-invariance tests).
    let expected = registry.render_core_schemas(&pinned);
    let parsed: Value = serde_json::from_str(&expected).expect("core schemas render as JSON");
    let edit_schema = parsed
        .as_array()
        .expect("array")
        .iter()
        .find(|schema| schema["name"] == json!("edit_lines"))
        .expect("edit_lines in core set")
        .clone();
    let payload = result
        .observation
        .lines()
        .nth(1)
        .expect("observation has a header line and a JSON payload line");
    let hydrated: Value = serde_json::from_str(payload).expect("hydrated payload is JSON");
    assert_eq!(
        hydrated,
        Value::Array(vec![edit_schema]),
        "hydration must return the complete schema"
    );

    // Hydration never touches the system prefix: byte-identical after.
    let prefix_after = registry.render_base_schema(&pinned);
    assert_eq!(
        prefix_before.as_bytes(),
        prefix_after.as_bytes(),
        "hydration must not touch the serialized base schema"
    );
    assert!(
        result.spill.is_none(),
        "a schema hydration is small; nothing spills"
    );
}

#[tokio::test]
async fn hydrate_then_call_roundtrip_through_dispatch() {
    let session = TempSession::new("hydrate-roundtrip");
    let mut registry = ToolRegistry::open(session.path(), IoCap::DEFAULT).expect("open registry");
    registry
        .bind_executor(
            "edit_lines",
            Arc::new(StubExecutor {
                output: b"+3 -1 applied".to_vec(),
            }),
        )
        .expect("bind stub executor");
    let pinned = registry.pin_current();

    // Step 1: hydrate the schema via tool_search.
    let hydrated = registry
        .dispatch(&pinned, "tool_search", &json!({ "query": "edit_lines" }))
        .await
        .expect("hydrate");
    assert!(hydrated.observation.contains("edit_lines"));

    // Step 2: the hydrated tool is called through the same dispatch path.
    let called = registry
        .dispatch(
            &pinned,
            "edit_lines",
            &json!({ "path": "src/lib.rs", "edits": [] }),
        )
        .await
        .expect("edit_lines dispatch succeeds");
    assert_eq!(called.observation, "+3 -1 applied");
    assert!(called.spill.is_none());
}

#[tokio::test]
async fn unknown_tool_search_yields_typed_suggestion_never_panic() {
    let mut registry = open_registry("hydrate-unknown");
    registry
        .register(Arc::new(AuxStub {
            name: "grep_search",
            description: "Search project files with a regex pattern.",
        }))
        .expect("register aux");
    let pinned = registry.pin_current();

    // Library path: typed error listing closest matches + the aux table.
    let error = registry
        .search(&pinned, "edit_liness")
        .expect_err("typo query must not match");
    assert!(
        error.suggestions.contains(&"edit_lines".to_owned()),
        "closest match listed: {:?}",
        error.suggestions
    );
    assert!(
        error.aux_table.contains("grep_search"),
        "aux-table entry present in the typed error"
    );

    // Model-facing path: the same miss renders as a graceful observation.
    let result = registry
        .dispatch(&pinned, "tool_search", &json!({ "query": "edit_liness" }))
        .await
        .expect("graceful observation, not an error crash");
    assert!(
        result.observation.contains("edit_lines"),
        "suggestion reaches the model: {}",
        result.observation
    );
    assert!(result.observation.contains("grep_search"));
}

#[tokio::test]
async fn empty_query_is_graceful() {
    let registry = open_registry("hydrate-empty");
    let pinned = registry.pin_current();

    let error = registry
        .search(&pinned, "")
        .expect_err("empty query rejected");
    assert!(error.suggestions.is_empty());
    assert!(
        !error.aux_table.is_empty(),
        "aux table offered for browsing"
    );

    let result = registry
        .dispatch(&pinned, "tool_search", &json!({ "query": "" }))
        .await
        .expect("empty query is graceful");
    assert!(
        result.observation.to_lowercase().contains("empty"),
        "observation explains the empty query"
    );
}

#[tokio::test]
async fn garbage_and_unicode_queries_are_graceful() {
    let registry = open_registry("hydrate-garbage");
    let pinned = registry.pin_current();
    let queries = [
        "&&&???",
        "🦀漢字\u{1}",
        "\0\0\0",
        "a-very-long-garbage-query-without-any-match-at-all-whatsoever-1234567890",
    ];
    for query in queries {
        let error = registry.search(&pinned, query).expect_err("no match");
        // Suggestions are always finite, sorted, panic-free.
        assert!(error.suggestions.len() <= 3, "bounded suggestions");

        let result = registry
            .dispatch(&pinned, "tool_search", &json!({ "query": query }))
            .await
            .unwrap_or_else(|error| panic!("query {query:?} must stay graceful: {error}"));
        assert!(
            result.observation.contains("closest") || result.observation.contains("no tool"),
            "graceful feedback for {query:?}: {}",
            result.observation
        );
    }
}

#[tokio::test]
async fn non_object_arguments_to_tool_search_are_typed_errors() {
    let registry = open_registry("hydrate-malformed");
    let pinned = registry.pin_current();
    for arguments in [json!(null), json!("edit_lines"), json!(42), json!({})] {
        let error = registry
            .dispatch(&pinned, "tool_search", &arguments)
            .await
            .expect_err("malformed arguments");
        assert!(
            matches!(error, ToolDispatchError::InvalidArguments { .. }),
            "typed InvalidArguments for {arguments}, got {error:?}"
        );
    }
}

#[tokio::test]
async fn search_matches_by_name_and_by_description() {
    let mut registry = open_registry("hydrate-matching");
    registry
        .register(Arc::new(AuxStub {
            name: "grep_search",
            description: "Search project files with a regex pattern.",
        }))
        .expect("register aux");
    let pinned = registry.pin_current();

    let by_name = registry.search(&pinned, "grep").expect("name match");
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].name, "grep_search");

    let by_description = registry
        .search(&pinned, "regex")
        .expect("description match");
    assert_eq!(by_description.len(), 1);
    assert_eq!(by_description[0].name, "grep_search");

    // Exact name match ranks ahead of substring matches, deterministically.
    let ranked = registry.search(&pinned, "edit").expect("ranked matches");
    let names: Vec<String> = ranked.iter().map(|schema| schema.name.clone()).collect();
    assert_eq!(names.first().map(String::as_str), Some("edit_lines"));
}

#[test]
fn tool_search_error_is_debug_and_displayable() {
    // The typed error must remain introspectable (no opaque String soup).
    let error = ToolSearchError {
        query: "zzz".to_owned(),
        reason: "no tool matched".to_owned(),
        suggestions: vec!["edit_lines".to_owned()],
        aux_table: "| tool | description |".to_owned(),
    };
    assert!(format!("{error:?}").contains("zzz"));
    assert!(format!("{error}").contains("edit_lines"));
}
