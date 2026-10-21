//! The harness daemon binary: binds loopback, generates/persists the
//! auth token, serves HTTP/WS/SSE + IPC, and adopts an inherited
//! listener when spawned as a reload successor.

use std::sync::Arc;

use harness_daemon::config::DaemonConfig;
use harness_daemon::reload;
use harness_daemon::server::{DaemonState, build_router};

fn main() {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("harness-daemon: runtime init failed: {e}");
            std::process::exit(1);
        }
    };
    if let Err(failure) = runtime.block_on(run()) {
        eprintln!("harness-daemon: {failure}");
        std::process::exit(1);
    }
}

fn config_from_env_and_args() -> Result<DaemonConfig, String> {
    let mut config = DaemonConfig::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => {
                if let Some(dir) = args.next() {
                    config.data_dir = dir.into();
                }
            }
            "--port" => {
                if let Some(port) = args.next() {
                    config.bind_addr = format!("127.0.0.1:{port}");
                }
            }
            "--ipc-name" => {
                if let Some(name) = args.next() {
                    config.ipc_name = name;
                }
            }
            _ => {}
        }
    }
    if config.bind_addr != "127.0.0.1:0" && !config.bind_addr.starts_with("127.0.0.1:") {
        return Err(format!(
            "refusing non-loopback bind {}: the daemon is loopback-only",
            config.bind_addr
        ));
    }
    config.provider = provider_from_env()?;
    Ok(config)
}

/// The provider seam (M32): family, model, key-variable NAME, optional
/// base URL and timeout, read from the environment. No HARNESS_PROVIDER
/// leaves the daemon unconfigured (frontends and the protocol stay
/// exercisable; inference fails typed); a set-but-invalid value is a
/// startup error. Key VALUES are resolved from the named variable at
/// factory build — never stored, never logged, never committed.
fn provider_from_env() -> Result<Option<harness_daemon::provider::ProviderSelection>, String> {
    let env_str = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
    let Some(family) = env_str("HARNESS_PROVIDER") else {
        return Ok(None);
    };
    let timeout = match env_str("HARNESS_PROVIDER_TIMEOUT_SECS") {
        Some(raw) => Some(raw.trim().parse::<u64>().map_err(|_| {
            format!("HARNESS_PROVIDER_TIMEOUT_SECS must be whole seconds, got {raw:?}")
        })?),
        None => None,
    };
    let selection = harness_daemon::provider::ProviderSelection::from_parts(
        &family,
        env_str("HARNESS_MODEL").as_deref(),
        env_str("HARNESS_PROVIDER_KEY_ENV").as_deref(),
        env_str("HARNESS_PROVIDER_BASE_URL").as_deref(),
        timeout,
    )
    .map_err(|failure| failure.to_string())?;
    Ok(Some(selection))
}

/// The session-model mediation candidate (plan M10): the resolved
/// provider plus its M7 catalog caps. An unknown model maps fail-closed
/// to no caps — it never mediates; the utility route still may. The
/// second `resolve` can only fail where `compose_factory`'s already did
/// (startup exits there), so the miss is unreachable in practice.
fn session_mediation(
    provider: &harness_daemon::provider::ProviderSelection,
) -> Option<harness_daemon::projection::ResolvedMediation> {
    let resolved = provider.resolve().ok()?;
    let caps = harness_intel::catalog::Catalog::seeded()
        .capabilities(&provider.model)
        .map_or_else(
            |_| harness_core::media_delivery::DeliveryCapabilities::default(),
            harness_daemon::projection::delivery_caps,
        );
    Some(harness_daemon::projection::ResolvedMediation {
        resolved,
        model_id: provider.model.clone(),
        caps,
        timeout: provider.inference_timeout,
    })
}

/// The utility-route mediation candidate from the T42 routing table
/// (`{data_dir}/routing.json`): the summarizer role's model on the same
/// provider/credential seam as the session model. An absent file is
/// silence (unconfigured is the default posture); a present but
/// unusable one is loud and never fatal to inference.
fn utility_mediation(
    provider: &harness_daemon::provider::ProviderSelection,
    data_dir: &std::path::Path,
) -> Option<harness_daemon::projection::ResolvedMediation> {
    let json = std::fs::read_to_string(data_dir.join("routing.json")).ok()?;
    let route = match harness_daemon::projection::utility_route(
        &json,
        &harness_intel::catalog::Catalog::seeded(),
    ) {
        Ok(route) => route,
        Err(failure) => {
            eprintln!("harness-daemon: routing.json unusable for the projection utility route: {failure}");
            return None;
        }
    };
    let mut selection = provider.clone();
    selection.model = route.model.clone();
    let resolved = selection.resolve().ok()?;
    Some(harness_daemon::projection::ResolvedMediation {
        resolved,
        model_id: route.model,
        caps: route.caps,
        timeout: selection.inference_timeout,
    })
}

async fn run() -> Result<(), String> {
    let config = config_from_env_and_args()?;
    std::fs::create_dir_all(&config.data_dir)
        .map_err(|e| format!("data dir {}: {e}", config.data_dir.display()))?;
    let ipc_name = config.ipc_name.clone();

    // M32: config-driven provider composition over the production
    // reqwest transport (the same seam cassette tests swap for in-memory
    // transports). An unset key variable fails HERE — at startup, typed
    // and actionable — instead of mid-turn.
    let transport: Arc<dyn harness_providers::transport::Transport> = Arc::new(
        harness_providers::transport_reqwest::ReqwestTransport::new()
            .map_err(|e| format!("provider transport init: {e}"))?,
    );
    let factory = harness_daemon::provider::compose_factory(config.provider.as_ref(), transport.clone())
        .map_err(|e| format!("provider config: {e}"))?;
    // M10: submit-time projection mediation — the session model when
    // its M7 caps cover the media, else the configured capable utility
    // role via the T42 routing table. Computed before `config` moves
    // into the daemon state.
    let session = config.provider.as_ref().and_then(session_mediation);
    let utility = config
        .provider
        .as_ref()
        .and_then(|provider| utility_mediation(provider, &config.data_dir));
    let mut state =
        DaemonState::new(config, factory).map_err(|e| format!("token generation: {e}"))?;
    state.projection_factory =
        harness_daemon::projection::compose_intake_factory(transport, session, utility);
    let state = Arc::new(state);

    // Reload successor mode: adopt the inherited listener instead of
    // binding (zero-downtime handoff — the socket never went down).
    let std_listener = {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.len() == 2 && args[0] == reload::LISTEN_FLAG {
            let raw = args[1]
                .parse::<u64>()
                .map_err(|_| "bad handoff listener id".to_owned())?;
            reload::adopt(raw).map_err(|e| format!("adopt listener: {e}"))?
        } else {
            std::net::TcpListener::bind(
                state
                    .config
                    .bind_addr
                    .parse::<std::net::SocketAddr>()
                    .map_err(|e| format!("bind addr: {e}"))?,
            )
            .map_err(|e| format!("bind: {e}"))?
        }
    };
    let keep = std_listener.try_clone();
    // tokio requires the socket in non-blocking mode; a blocking-mode
    // listener silently never accepts (kernel backlog completes the
    // handshake, the reactor never fires).
    std_listener
        .set_nonblocking(true)
        .map_err(|e| format!("nonblocking: {e}"))?;
    let listener =
        tokio::net::TcpListener::from_std(std_listener).map_err(|e| format!("listener: {e}"))?;
    if let Ok(keep) = keep {
        state.register_listener(keep);
    }
    let bound = listener
        .local_addr()
        .map_err(|e| format!("local addr: {e}"))?;

    // Token handoff to frontends: file under the data dir + stdout.
    let token_path = state.config.data_dir.join("daemon-token");
    std::fs::write(&token_path, &state.token).map_err(|e| format!("persist token: {e}"))?;
    println!(
        "harness-daemon listening on http://{bound} (token: {})",
        state.token
    );

    // Todo 41: scheduled runs + notifications. Declarative automations
    // load from the data dir; a broken file leaves the scheduler idle
    // (loudly) without taking the transports down. Fail-closed: only
    // approved-tier packs ever schedule.
    let data_dir = state.config.data_dir.clone();
    let defs =
        match harness_daemon::automations::FilePackGate::open(&data_dir.join("pack-tiers.json")) {
            Ok(pack_gate) => {
                match harness_daemon::automations::AutomationStore::load(&data_dir, &pack_gate) {
                    Ok(store) => {
                        println!(
                            "harness-daemon: {} automation(s) scheduled",
                            store.defs().len()
                        );
                        store.defs().to_vec()
                    }
                    Err(e) => {
                        eprintln!("harness-daemon: automations invalid; scheduler idle: {e}");
                        Vec::new()
                    }
                }
            }
            Err(e) => {
                eprintln!("harness-daemon: pack tiers unreadable; scheduler idle: {e}");
                Vec::new()
            }
        };
    let mut notifier = harness_daemon::notify::Notifier::new();
    notifier.add(Arc::new(harness_daemon::notify::TracingSink));
    notifier.add(Arc::new(harness_daemon::osc::stdout_sink("harness")));
    // Scheduled-run notifications mirror onto the owning session's
    // event stream (kinds sched/verdict) so attached clients see them.
    notifier.add(Arc::new(harness_daemon::engine::EventStreamSink::new(Arc::new({
        let state = Arc::clone(&state);
        move |sid: &str| state.try_session(sid)
    }))));
    let run_sink = match harness_daemon::bridge::RunSink::open(
        &data_dir,
        Arc::new(harness_daemon::gate::CriteriaGate),
        Arc::new(notifier),
    ) {
        Ok(sink) => Arc::new(sink),
        Err(e) => return Err(format!("run history open: {e}")),
    };
    let wall_clock: harness_daemon::scheduler::SchedulerClock = Arc::new(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs())
    });
    match harness_daemon::runner::CronRunner::open(
        &data_dir,
        defs,
        Arc::new(harness_daemon::kickoff::DaemonKicker::new(Arc::clone(
            &state,
        ))),
        run_sink,
        harness_daemon::runner::DEFAULT_JITTER_WINDOW_SECS,
        harness_daemon::automations::NetworkPolicy::Online,
    ) {
        Ok(runner) => runner.spawn_loop(wall_clock, std::time::Duration::from_secs(5)),
        Err(e) => eprintln!("harness-daemon: scheduler failed to start: {e}"),
    }

    let ipc_state = Arc::clone(&state);
    tokio::spawn(async move {
        if let Err(e) = harness_daemon::ipc::serve_ipc(ipc_state, &ipc_name).await {
            eprintln!("harness-daemon: ipc listener failed: {e}");
        }
    });

    let app = build_router(Arc::clone(&state));
    let mut drain_rx = state.drain.subscribe();
    let serve = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        let _ = drain_rx.changed().await;
    });
    serve.await.map_err(|e| format!("serve: {e}"))
}
