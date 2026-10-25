//! T29 transport pool (fingerprint-keyed sharing, refcounted release,
//! idle drain, budget hysteresis — no flapping) and the schema disk
//! cache (keyed by config fingerprint, advertised at spawn, reconciled
//! against live tools/list with typed drift).

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use austringer_mcp::client::McpClient;
use austringer_mcp::config::{Fingerprint, ServerEntry, TransportSpec};
use austringer_mcp::error::McpClientError;
use austringer_mcp::pool::{Connector, PoolConfig, PoolError, PoolEvent, TransportPool};
use austringer_mcp::reference_server::ReferenceFsServer;
use austringer_mcp::schema_cache::{SchemaDiskCache, SchemaDrift, spawn_advertised};
use common::duplex_wire;
use serde_json::json;
use tempfile::TempDir;

fn entry(name: &str, salt: u32) -> ServerEntry {
    ServerEntry::new(
        name.to_owned(),
        TransportSpec::Stdio {
            command: format!("npx-{salt}"),
            args: vec![],
        },
    )
}

fn counting_connector(root: std::path::PathBuf, connects: Arc<AtomicUsize>) -> Connector {
    Arc::new(move |entry: ServerEntry| {
        let root = root.clone();
        let connects = connects.clone();
        Box::pin(async move {
            connects.fetch_add(1, Ordering::SeqCst);
            let wire = duplex_wire(ReferenceFsServer::new(root.clone())).await;
            McpClient::connect_with_transport(entry, wire).await
        })
    })
}

async fn pool_with(config: PoolConfig) -> (Arc<TransportPool>, Arc<AtomicUsize>, TempDir) {
    let dir = TempDir::new().expect("temp");
    let connects = Arc::new(AtomicUsize::new(0));
    let pool = Arc::new(TransportPool::with_connector(
        config,
        counting_connector(dir.path().to_path_buf(), connects.clone()),
    ));
    (pool, connects, dir)
}

#[tokio::test]
async fn same_fingerprint_shares_one_connection() {
    let (pool, connects, _dir) = pool_with(PoolConfig {
        max_live: 8,
        warn_at: 6,
        hysteresis_gap: 2,
        idle_drain_after: Duration::from_secs(60),
    })
    .await;
    let a = pool.acquire(&entry("fs", 1)).await.expect("first acquire");
    let b = pool
        .acquire(&entry("fs", 1))
        .await
        .expect("second acquire reuses");
    assert_eq!(
        connects.load(Ordering::SeqCst),
        1,
        "one connect for two acquires"
    );
    assert_eq!(pool.live_count(), 1);
    drop(a);
    drop(b);
}

#[tokio::test]
async fn different_fingerprints_never_share() {
    let (pool, connects, _dir) = pool_with(PoolConfig {
        max_live: 8,
        warn_at: 6,
        hysteresis_gap: 2,
        idle_drain_after: Duration::from_secs(60),
    })
    .await;
    let _a = pool.acquire(&entry("fs", 1)).await.expect("a");
    let _b = pool.acquire(&entry("fs", 2)).await.expect("b");
    let _c = pool.acquire(&entry("other", 1)).await.expect("c");
    assert_eq!(connects.load(Ordering::SeqCst), 3);
    assert_eq!(pool.live_count(), 3);
}

#[tokio::test]
async fn budget_hysteresis_does_not_flap() {
    // warn at 1, enforce at 3, release only at <= enforce-gap = 1.
    let (pool, _connects, _dir) = pool_with(PoolConfig {
        max_live: 3,
        warn_at: 1,
        hysteresis_gap: 2,
        idle_drain_after: Duration::from_secs(60),
    })
    .await;

    let first = pool.acquire(&entry("fs1", 1)).await.expect("1");
    let second = pool.acquire(&entry("fs2", 1)).await.expect("2");
    let third = pool.acquire(&entry("fs3", 1)).await.expect("3");
    assert!(matches!(
        pool.acquire(&entry("fs4", 1)).await,
        Err(PoolError::BudgetExhausted { .. })
    ));

    // Drop to 2 live: still enforcing (the gap forbids flapping back).
    drop(third);
    assert!(
        matches!(
            pool.acquire(&entry("fs4", 1)).await,
            Err(PoolError::BudgetExhausted { .. })
        ),
        "within the gap the pool keeps enforcing"
    );

    // Drop to 1 live (release threshold): acquisition is allowed again.
    drop(second);
    let _recovered = pool
        .acquire(&entry("fs4", 1))
        .await
        .expect("recovered below release threshold");
    drop(first);
}

#[tokio::test]
async fn warn_level_records_typed_event() {
    let (pool, _connects, _dir) = pool_with(PoolConfig {
        max_live: 8,
        warn_at: 1,
        hysteresis_gap: 2,
        idle_drain_after: Duration::from_secs(60),
    })
    .await;
    let _a = pool.acquire(&entry("fs1", 1)).await.expect("a");
    let _b = pool.acquire(&entry("fs2", 1)).await.expect("b");
    let events = pool.take_events();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, PoolEvent::BudgetWarn { live } if *live == 2)),
        "{events:?}"
    );
}

#[tokio::test]
async fn idle_handles_drain_after_window() {
    let (pool, connects, _dir) = pool_with(PoolConfig {
        max_live: 8,
        warn_at: 6,
        hysteresis_gap: 2,
        idle_drain_after: Duration::from_millis(40),
    })
    .await;
    let handle = pool.acquire(&entry("fs", 1)).await.expect("acquire");
    assert!(pool.tick().is_empty(), "in-use handle is never drained");
    drop(handle);
    tokio::time::sleep(Duration::from_millis(60)).await;
    let events = pool.tick();
    assert_eq!(pool.live_count(), 0);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, PoolEvent::Drained { .. })),
        "{events:?}"
    );
    // A fresh acquire reconnects (the drained one is gone).
    let _again = pool.acquire(&entry("fs", 1)).await.expect("reacquire");
    assert_eq!(connects.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn poisoned_handles_drain_immediately() {
    let (pool, _connects, _dir) = pool_with(PoolConfig {
        max_live: 8,
        warn_at: 6,
        hysteresis_gap: 2,
        idle_drain_after: Duration::from_secs(60),
    })
    .await;
    let handle = pool
        .acquire(&entry("fs", 1))
        .await
        .expect("acquire")
        .with_call_timeout(Duration::from_millis(150));
    let hang = handle.client().call_tool("hang", &json!({})).await;
    assert!(matches!(hang, Err(McpClientError::Unresponsive { .. })));
    assert!(handle.client().is_poisoned());
    drop(handle);
    let events = pool.tick();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, PoolEvent::Drained { .. })),
        "poisoned connections do not wait out the idle window: {events:?}"
    );
    assert_eq!(pool.live_count(), 0);
}

#[tokio::test]
async fn schema_cache_cold_then_warm_then_drift() {
    let dir = TempDir::new().expect("temp");
    let cache = SchemaDiskCache::open(dir.path()).expect("open cache");
    let root = dir.path().to_path_buf();

    // COLD: no cache entry — the live list becomes the advertisement.
    let wire = duplex_wire(ReferenceFsServer::new(root.clone())).await;
    let client = McpClient::connect_with_transport(entry("fs", 1), wire)
        .await
        .expect("connect");
    let cold = spawn_advertised(&cache, &client).await.expect("cold spawn");
    assert!(
        cold.advertised.is_none(),
        "cold spawn has no disk advertisement"
    );
    assert!(cold.drift == SchemaDrift::InSync);
    assert!(cold.tools.iter().any(|tool| tool.name == "read_file"));

    // WARM: disk advertisement survives even when the live server cannot
    // answer tools/list (zero first-snapshot cache miss).
    let wire = duplex_wire(ReferenceFsServer::new(root.clone())).await;
    let warm_client = McpClient::connect_with_transport(entry("fs", 1), wire)
        .await
        .expect("warm connect");
    let warm = spawn_advertised(&cache, &warm_client)
        .await
        .expect("warm spawn");
    assert_eq!(
        warm.advertised.as_ref().map(|tools| tools.len()),
        cold.tools.len().into(),
        "warm spawn advertises the disk cache without a fresh snapshot"
    );
    assert_eq!(warm.drift, SchemaDrift::InSync);

    // Drift: same fingerprint, a stale tool set on disk.
    let fp = warm_client.entry().fingerprint();
    let stale = vec![
        austringer_mcp::client::ToolDescriptor {
            name: "read_file".to_owned(),
            description: String::new(),
            input_schema: serde_json::json!({}),
        },
        austringer_mcp::client::ToolDescriptor {
            name: "gone_tool".to_owned(),
            description: String::new(),
            input_schema: serde_json::json!({}),
        },
    ];
    cache.store(&fp, &stale).expect("seed stale cache");
    let wire = duplex_wire(ReferenceFsServer::new(root)).await;
    let drift_client = McpClient::connect_with_transport(entry("fs", 1), wire)
        .await
        .expect("drift connect");
    let drifted = spawn_advertised(&cache, &drift_client)
        .await
        .expect("drift spawn");
    assert!(
        matches!(&drifted.drift, SchemaDrift::Drifted { removed, .. } if removed.contains(&"gone_tool".to_owned())),
        "{:?}",
        drifted.drift
    );
    // Reconciled: the cache now matches live (drift resolved on disk).
    assert!(
        drifted.tools.iter().any(|tool| tool.name == "read_file")
            && !drifted.tools.iter().any(|tool| tool.name == "gone_tool")
    );
    drop(client);
    drop(warm_client);
    drop(drift_client);
}

#[tokio::test]
async fn corrupt_cache_file_is_a_miss_not_a_crash() {
    let dir = TempDir::new().expect("temp");
    let cache = SchemaDiskCache::open(dir.path()).expect("open");
    let fp = Fingerprint("deadbeef".to_owned());
    std::fs::write(dir.path().join("deadbeef.json"), "{not json").ok();
    assert!(
        cache.load(&fp).is_none(),
        "corrupt entries degrade to cache-miss"
    );
}
