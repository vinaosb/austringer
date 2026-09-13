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

/// Inference stub until provider wiring (todo 40 composes the T9
/// adapter into the daemon's session factory); frontends and the
/// protocol layer are fully exercisable without a provider.
struct UnconfiguredInference;

#[async_trait::async_trait]
impl harness_core::agent_loop::Inference for UnconfiguredInference {
    async fn infer(
        &mut self,
        _request: &harness_core::agent_loop::InferenceRequest<'_>,
        _cancel: &harness_core::agent_loop::CancelToken,
    ) -> Result<harness_core::agent_loop::AssistantTurn, harness_core::agent_loop::InferenceError>
    {
        Err(harness_core::agent_loop::InferenceError::Failure(
            "no provider configured (daemon hosts the loop; provider wiring lands with todo 40)"
                .into(),
        ))
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
    Ok(config)
}

async fn run() -> Result<(), String> {
    let config = config_from_env_and_args()?;
    std::fs::create_dir_all(&config.data_dir)
        .map_err(|e| format!("data dir {}: {e}", config.data_dir.display()))?;
    let ipc_name = config.ipc_name.clone();

    let factory: Arc<dyn Fn() -> Box<dyn harness_core::agent_loop::Inference> + Send + Sync> =
        Arc::new(|| Box::new(UnconfiguredInference));
    let state =
        Arc::new(DaemonState::new(config, factory).map_err(|e| format!("token generation: {e}"))?);

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
