//! The bare `austringer` front door: exec the TUI and pass its exit
//! code through (research §34.2 default-experience pattern — the
//! interactive client IS the product; subcommands stay for scripts).

use crate::error::CliError;

/// Run `austringer-tui` (located via [`austringer_launch::find_tui_exe`])
/// attached to this terminal.
///
/// # Errors
/// typed failure when the TUI binary is missing or cannot be run.
pub fn exec_tui() -> Result<i32, CliError> {
    let exe = austringer_launch::find_tui_exe().map_err(|failure| CliError::Unreachable {
        detail: failure.to_string(),
    })?;
    let status = std::process::Command::new(exe)
        .status()
        .map_err(|source| CliError::Io {
            detail: source.to_string(),
        })?;
    Ok(status.code().unwrap_or(0))
}
