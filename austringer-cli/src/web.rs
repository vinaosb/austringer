//! `austringer web`: ensure a daemon is running (the auto-spawn
//! posture from [`austringer_launch`]), print the dashboard URL it
//! serves, and open the user's browser unless `--no-browser`.

use std::path::Path;

use austringer_launch as launch;

use crate::error::CliError;
use crate::target::TargetSpec;

/// Resolve the dashboard URL: an explicit `--url` daemon is used as
/// given; otherwise the local daemon's `daemon-url` file is probed and
/// a daemon is spawned when nothing answers.
async fn dashboard_url(spec: &TargetSpec) -> Result<String, CliError> {
    if let Some(url) = &spec.url {
        return Ok(url.trim_end_matches('/').to_owned());
    }
    let data_dir = spec
        .data_dir
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from(launch::DEFAULT_DATA_DIR));
    if let Some(url) = live_url(&data_dir).await {
        return Ok(url);
    }
    let ipc_name = spec
        .ipc_name
        .clone()
        .unwrap_or_else(|| launch::DEFAULT_IPC_NAME.to_owned());
    launch::ensure_daemon(&data_dir, &ipc_name)
        .await
        .map_err(|failure| CliError::Unreachable {
            detail: failure.to_string(),
        })?;
    launch::read_daemon_url(&data_dir).ok_or_else(|| CliError::Unreachable {
        detail: "daemon became ready but wrote no daemon-url".to_owned(),
    })
}

async fn live_url(data_dir: &Path) -> Option<String> {
    let url = launch::read_daemon_url(data_dir)?;
    let addr = url.trim_start_matches("http://").to_owned();
    let probe = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    .ok()?
    .ok()?;
    drop(probe);
    Some(url)
}

/// Platform browser opener; a missing opener is a printed hint, never
/// a failure (the URL is already on stdout).
fn open_browser(url: &str) {
    let result = if cfg!(windows) {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    if result.is_err() {
        eprintln!("austringer: no browser opener found; open {url} manually");
    }
}

/// Run the `web` subcommand. # Errors
/// typed failure when no daemon can be reached or spawned.
pub async fn open(spec: &TargetSpec, no_browser: bool) -> Result<i32, CliError> {
    let url = dashboard_url(spec).await?;
    println!("austringer web: {url}");
    if !no_browser {
        open_browser(&url);
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn explicit_url_daemon_shortcuts_discovery() {
        let url = dashboard_url(&TargetSpec {
            url: Some("http://127.0.0.1:9/".into()),
            token: None,
            data_dir: None,
            ipc_name: None,
        })
        .await
        .unwrap_or_else(|e| panic!("url: {e}"));
        assert_eq!(url, "http://127.0.0.1:9");
    }

    #[test]
    fn missing_url_file_without_spawn_yields_none() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        assert!(launch::read_daemon_url(dir.path()).is_none());
    }
}
