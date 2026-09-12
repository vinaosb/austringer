//! RSS-sampled bounded-LSP guard (forge §26): the always-on watchdog that
//! samples spawned-server RSS and kills runaways. Decision routing and the
//! kill path are exercised with a fake probe + flag target; the REAL
//! sysinfo probe is exercised against a real Windows process; the loop is
//! driven at interval-scale with tolerant deadlines.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use harness_lsp::config::{Language, RssGuardConfig};
use harness_lsp::guard::{GuardTarget, RssProbe, SysinfoProbe, enforce_once, run_guard_loop};
use harness_lsp::router::RouterEvent;

/// Probe returning a configurable fixed RSS for every pid.
struct FakeProbe {
    rss: AtomicU64,
}

impl FakeProbe {
    fn new(rss: u64) -> Arc<Self> {
        Arc::new(Self {
            rss: AtomicU64::new(rss),
        })
    }
}

impl RssProbe for FakeProbe {
    fn rss_bytes(&self, _pid: u32) -> Option<u64> {
        Some(self.rss.load(Ordering::Relaxed))
    }
}

fn flag_target(language: Language, pid: Option<u32>, killed: Arc<AtomicBool>) -> GuardTarget {
    GuardTarget::new(language, pid, move || {
        killed.store(true, Ordering::SeqCst);
    })
}

#[tokio::test]
async fn over_cap_sample_kills_and_emits_rss_capped() {
    let (events, mut events_rx) = tokio::sync::mpsc::channel(16);
    let killed = Arc::new(AtomicBool::new(false));
    let targets = vec![flag_target(Language::Rust, Some(4242), Arc::clone(&killed))];

    enforce_once(
        &targets,
        FakeProbe::new(9 * 1024 * 1024 * 1024).as_ref(),
        2_000_000_000,
        &events,
    )
    .await;

    assert!(
        killed.load(Ordering::SeqCst),
        "over-cap server must be killed"
    );
    assert_eq!(
        events_rx.try_recv(),
        Ok(RouterEvent::RssCapped {
            language: Language::Rust,
            rss_bytes: 9 * 1024 * 1024 * 1024,
            cap_bytes: 2_000_000_000,
        })
    );
}

#[tokio::test]
async fn healthy_sample_and_missing_pid_are_left_alone() {
    let (events, mut events_rx) = tokio::sync::mpsc::channel(16);
    let killed = Arc::new(AtomicBool::new(false));
    let attached_no_pid = flag_target(Language::Go, None, Arc::clone(&killed));
    let spawned_healthy = flag_target(Language::Rust, Some(7), Arc::clone(&killed));

    enforce_once(
        &[attached_no_pid, spawned_healthy],
        FakeProbe::new(1024).as_ref(),
        2_000_000_000,
        &events,
    )
    .await;

    assert!(
        !killed.load(Ordering::SeqCst),
        "healthy/pid-less targets are never killed"
    );
    assert!(
        events_rx.try_recv().is_err(),
        "no events for healthy servers"
    );
}

#[tokio::test]
async fn sysinfo_probe_reads_a_real_windows_process() {
    let probe = SysinfoProbe::new();
    let rss = probe
        .rss_bytes(std::process::id())
        .expect("the current process must be sampleable");
    assert!(rss > 0, "a live process reports a positive RSS, got {rss}");
}

#[tokio::test]
async fn guard_loop_kills_a_runaway_within_a_few_intervals() {
    let (events_tx, mut events) = tokio::sync::mpsc::channel::<RouterEvent>(16);
    let killed = Arc::new(AtomicBool::new(false));
    let provide = {
        let killed = Arc::clone(&killed);
        move || vec![flag_target(Language::Rust, Some(99), Arc::clone(&killed))]
    };

    let loop_handle = tokio::spawn(run_guard_loop(
        provide,
        FakeProbe::new(u64::MAX),
        RssGuardConfig {
            enabled: true,
            sample_interval: Duration::from_millis(20),
            rss_cap_bytes: 100,
        },
        events_tx,
    ));

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !killed.load(Ordering::SeqCst) {
        assert!(
            std::time::Instant::now() < deadline,
            "guard never killed the runaway"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(matches!(
        events.try_recv(),
        Ok(RouterEvent::RssCapped { .. })
    ));
    loop_handle.abort();
}

#[tokio::test]
async fn real_process_kill_path_through_the_guard() {
    // A real child process (any stdin-blocked process stands in for a fat
    // server): the REAL probe must see it, and the guard kill must end it.
    let mut child = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("powershell spawns");
    let pid = child.id();

    let probe = SysinfoProbe::new();
    let real_rss = probe.rss_bytes(pid).expect("child RSS sampled");
    assert!(real_rss > 0);

    let killed = Arc::new(AtomicBool::new(false));
    let kill_flag = Arc::clone(&killed);
    let target = GuardTarget::new(Language::Rust, Some(pid), move || {
        kill_flag.store(true, Ordering::SeqCst);
    });
    let (events, mut events_rx) = tokio::sync::mpsc::channel(16);
    enforce_once(&[target], &probe, 1, &events).await; // cap=1 byte: always over

    assert!(killed.load(Ordering::SeqCst));
    let _ = child.kill();
    let _ = child.wait();
    // RSS naturally drifts between samples; pin the shape, not the number.
    match events_rx.try_recv() {
        Ok(RouterEvent::RssCapped {
            rss_bytes,
            cap_bytes,
            ..
        }) => {
            assert!(rss_bytes > 0);
            assert_eq!(cap_bytes, 1);
        }
        other => panic!("expected RssCapped, got {other:?}"),
    }
}
