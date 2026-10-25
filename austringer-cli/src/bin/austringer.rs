//! The `austringer` binary (plan todo 39): parse, resolve the transport
//! target, dispatch to the subcommand flows, and convert typed
//! failures into stderr + exit codes.

use std::process::ExitCode;
use std::time::Duration;

use austringer_cli::attach;
use austringer_cli::cli::{Cli, Command, SessionsArgs, SessionsCommand};
use austringer_cli::client::{Connect, LIVE_ONLY, SessionTransport};
use austringer_cli::doctor::{DoctorOptions, doctor};
use austringer_cli::error::CliError;
use austringer_cli::image;
use austringer_cli::mcp;
use austringer_cli::repl;
use austringer_cli::run::{self, RunOptions};
use austringer_cli::sessions;
use austringer_cli::target::{Target, TargetSpec, default_session};
use clap::Parser as _;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("austringer: runtime init failed: {error}");
            return ExitCode::from(1);
        }
    };
    match runtime.block_on(dispatch(cli)) {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(error) => {
            eprintln!("austringer: {error}");
            ExitCode::from(error.exit_code().clamp(0, 255) as u8)
        }
    }
}

async fn dispatch(cli: Cli) -> Result<i32, CliError> {
    // `mcp` commands are daemon-less (scan/serve/call never contact
    // the daemon), so they run before transport resolution — no token
    // file is required to inspect configs or serve stdio.
    if let Command::Mcp(args) = &cli.command {
        return mcp::run(&args.command).await;
    }
    let spec = TargetSpec {
        url: cli.url.clone(),
        token: cli.token.clone(),
        data_dir: cli.data_dir.clone(),
        ipc_name: cli.ipc_name.clone(),
    };
    // The `sessions` verb family rides the HTTP route surface only
    // (the IPC carrier has no HTTP base), so it checks its transport
    // BEFORE resolve — a missing --url is the typed hint, not a
    // token-file failure.
    if let Command::Sessions(SessionsArgs { command }) = &cli.command {
        return match command {
            SessionsCommand::Search(args) => sessions::search(&spec, args).await,
            SessionsCommand::List => sessions::list(&spec).await,
            SessionsCommand::History(args) => sessions::history(&spec, &args.session).await,
        };
    }
    let target = Target::resolve(&spec)?;
    let images = image::attachments(&cli.images, false)?;
    match cli.command {
        Command::Run(args) => {
            let session = args.session.unwrap_or_else(default_session);
            let options = RunOptions {
                json: cli.output_json,
                timeout: Duration::from_secs(args.timeout.unwrap_or(120)),
            };
            let connect = connect_for(&spec, &target, &session, LIVE_ONLY);
            let mut transport = SessionTransport::connect(&connect).await?;
            run::run(
                &mut transport,
                &args.prompt.join(" "),
                &images,
                target.session_dir(&session).as_deref(),
                &options,
            )
            .await
        }
        Command::Capture(args) => {
            let session = args.session.unwrap_or_else(default_session);
            let connect = connect_for(&spec, &target, &session, LIVE_ONLY);
            let mut transport = SessionTransport::connect(&connect).await?;
            run::capture(
                &mut transport,
                &args.prompt.join(" "),
                &images,
                target.session_dir(&session).as_deref(),
            )
            .await
        }
        Command::Repl(args) => {
            let session = args.session.unwrap_or_else(default_session);
            let connect = connect_for(&spec, &target, &session, LIVE_ONLY);
            let mut transport = SessionTransport::connect(&connect).await?;
            repl::repl(
                &mut transport,
                &images,
                target.session_dir(&session).as_deref(),
            )
            .await
        }
        Command::Attach(args) => {
            let session = args.session.unwrap_or_else(default_session);
            let after = args.after.unwrap_or(0);
            let connect = connect_for(&spec, &target, &session, after);
            let mut transport = SessionTransport::connect(&connect).await?;
            attach::attach(&mut transport, cli.output_json, args.max_events).await
        }
        Command::Doctor(args) => {
            doctor(
                &target,
                &DoctorOptions {
                    sessions_root: args.sessions,
                },
            )
            .await
        }
        Command::Mcp(_) => unreachable!("mcp dispatched before transport resolution"),
        Command::Sessions(_) => unreachable!("sessions dispatched before transport resolution"),
    }
}

fn connect_for(spec: &TargetSpec, target: &Target, session: &str, after: u64) -> Connect {
    let token = match target {
        Target::Http { token, .. } => token.clone(),
        Target::Ipc { token, .. } => Some(token.clone()),
    };
    Connect {
        url: spec.url.clone(),
        token,
        ipc_name: spec
            .ipc_name
            .clone()
            .unwrap_or_else(|| austringer_cli::target::DEFAULT_IPC_NAME.to_owned()),
        data_dir: spec
            .data_dir
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from(austringer_cli::target::DEFAULT_DATA_DIR)),
        session: session.to_owned(),
        after,
        client_id: format!("cli-{}", std::process::id()),
    }
}
