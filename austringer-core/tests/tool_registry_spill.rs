//! Tool-I/O byte caps with transparent disk spill (todo 6): oversized tool
//! results spill to a content-addressed store; the context receives a
//! pointer plus the first N bytes (cap configurable, default 4 KiB);
//! spilled content is recoverable by pointer; binary/invalid-UTF-8 bytes
//! spill without corruption.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use austringer_core::tools::{IoCap, ToolProvider, ToolRegistry, ToolSchema};
use common::TempSession;
use serde_json::{Value, json};

/// Stub executor returning fixed (possibly binary) bytes.
struct BlobExecutor {
    output: Vec<u8>,
}

#[async_trait]
impl ToolProvider for BlobExecutor {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "execute_command".to_owned(),
            description: "blob executor".to_owned(),
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

fn deterministic_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut rng = common::XorShift64::new(seed);
    (0..len).map(|_| rng.next_u64() as u8).collect()
}

fn blob(len: usize, seed: u64) -> Arc<BlobExecutor> {
    Arc::new(BlobExecutor {
        output: deterministic_bytes(seed, len),
    })
}

#[tokio::test]
async fn oversized_result_spills_with_pointer_and_first_bytes() {
    let session = TempSession::new("spill-oversized");
    let output = deterministic_bytes(21, 10 * 1024); // 10 KiB > 4 KiB cap
    let mut registry = ToolRegistry::open(session.path(), IoCap::DEFAULT).expect("open registry");
    registry
        .bind_executor(
            "execute_command",
            Arc::new(BlobExecutor {
                output: output.clone(),
            }),
        )
        .expect("bind");
    let pinned = registry.pin_current();

    let result = registry
        .dispatch(
            &pinned,
            "execute_command",
            &json!({ "command": "cargo test" }),
        )
        .await
        .expect("dispatch");

    let spill = result.spill.as_ref().expect("oversized result spills");
    assert_eq!(spill.total_bytes, 10 * 1024u64);
    assert_eq!(spill.prefix.len(), IoCap::DEFAULT.max_inline_bytes);
    assert_eq!(spill.prefix, &output[..IoCap::DEFAULT.max_inline_bytes]);
    // The observation carries the pointer and the first-N preview, bounded.
    assert!(
        result.observation.contains("spilled"),
        "{}",
        result.observation
    );
    assert!(result.observation.contains(&spill.hash.to_ref()));
    assert!(
        result.observation.len() < 2 * IoCap::DEFAULT.max_inline_bytes,
        "context stays bounded: {}",
        result.observation.len()
    );
    // Recoverable by pointer: the exact original bytes come back.
    assert_eq!(
        registry
            .spill_store()
            .read(&spill.hash)
            .expect("read")
            .expect("present"),
        output,
        "read-back must be lossless"
    );
}

#[tokio::test]
async fn under_cap_result_stays_inline() {
    let session = TempSession::new("spill-inline");
    let mut registry = ToolRegistry::open(session.path(), IoCap::DEFAULT).expect("open registry");
    // Valid UTF-8 payload: the inline rendering passes it through untouched.
    registry
        .bind_executor(
            "execute_command",
            Arc::new(BlobExecutor {
                output: "x".repeat(100).into_bytes(),
            }),
        )
        .expect("bind");
    let pinned = registry.pin_current();

    let result = registry
        .dispatch(&pinned, "execute_command", &json!({ "command": "echo hi" }))
        .await
        .expect("dispatch");
    assert!(result.spill.is_none());
    assert_eq!(
        result.observation.len(),
        100,
        "content passes through untouched"
    );
    assert_eq!(result.observation, "x".repeat(100));
}

#[tokio::test]
async fn cap_boundary_is_exclusive() {
    // Exactly-cap results stay inline; one byte over spills.
    let session = TempSession::new("spill-boundary");
    let cap = IoCap {
        max_inline_bytes: 64,
    };
    let mut registry = ToolRegistry::open(session.path(), cap).expect("open registry");
    registry
        .bind_executor("execute_command", blob(64, 23))
        .expect("bind");
    let pinned = registry.pin_current();
    let at_cap = registry
        .dispatch(&pinned, "execute_command", &json!({}))
        .await
        .expect("dispatch");
    assert!(at_cap.spill.is_none());

    registry
        .bind_executor("execute_command", blob(65, 24))
        .expect("rebind");
    let over_cap = registry
        .dispatch(&pinned, "execute_command", &json!({}))
        .await
        .expect("dispatch");
    let spill = over_cap.spill.as_ref().expect("one byte over spills");
    assert_eq!(spill.total_bytes, 65);
    assert_eq!(spill.prefix.len(), 64);
}

#[tokio::test]
async fn cap_is_configurable() {
    let session = TempSession::new("spill-configurable");
    let registry = ToolRegistry::open(
        session.path(),
        IoCap {
            max_inline_bytes: 8,
        },
    )
    .expect("open registry");
    assert_eq!(IoCap::DEFAULT.max_inline_bytes, 4096, "documented default");
    let _ = registry;
}

#[tokio::test]
async fn binary_invalid_utf8_spills_without_corruption() {
    // Malformed input class: invalid-UTF-8 bytes must not corrupt the spill
    // or panic the lossy context rendering.
    let session = TempSession::new("spill-binary");
    let mut output = deterministic_bytes(25, 8 * 1024);
    for byte in output.iter_mut().step_by(3) {
        *byte = 0xFF; // dense invalid-UTF-8 sequences
    }
    let original = output.clone();
    let mut registry = ToolRegistry::open(session.path(), IoCap::DEFAULT).expect("open registry");
    registry
        .bind_executor("execute_command", Arc::new(BlobExecutor { output }))
        .expect("bind");
    let pinned = registry.pin_current();

    let result = registry
        .dispatch(
            &pinned,
            "execute_command",
            &json!({ "command": "xxd blob.bin" }),
        )
        .await
        .expect("binary results dispatch without panic");
    let spill = result.spill.as_ref().expect("spills");
    assert!(
        !result.observation.is_empty() && result.observation.contains(&spill.hash.to_ref()),
        "pointer + lossy preview present"
    );
    assert_eq!(
        registry
            .spill_store()
            .read(&spill.hash)
            .expect("read")
            .expect("present"),
        original,
        "binary bytes read back exactly"
    );
}

#[tokio::test]
async fn spill_files_are_content_addressed_and_deduped() {
    let session = TempSession::new("spill-dedup");
    let mut registry = ToolRegistry::open(session.path(), IoCap::DEFAULT).expect("open registry");
    registry
        .bind_executor("execute_command", blob(5120, 26))
        .expect("bind");
    let pinned = registry.pin_current();

    let first = registry
        .dispatch(&pinned, "execute_command", &json!({}))
        .await
        .expect("first");
    let second = registry
        .dispatch(&pinned, "execute_command", &json!({}))
        .await
        .expect("second");
    assert_eq!(
        first.spill.as_ref().expect("spill").hash,
        second.spill.as_ref().expect("spill").hash,
        "identical content maps to one address"
    );

    let spill_dir = session.path().join("spill");
    let entries: Vec<_> = std::fs::read_dir(&spill_dir)
        .expect("spill dir")
        .collect::<Result<_, _>>()
        .expect("entries");
    assert_eq!(entries.len(), 1, "no duplicate spill files");
}
