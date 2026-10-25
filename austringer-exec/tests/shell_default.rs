//! PowerShell 7 default-shell resolution tests (plan todo 20):
//! pwsh > powershell > cmd probing, config override, no-shell typed error.
//! The resolution ORDER is part of the product contract and is documented
//! on [`austringer_exec::shell::ShellKind`].

// The shell module itself is windows-only (pwsh/powershell/cmd probing).
#![cfg(windows)]

use std::path::PathBuf;

use austringer_exec::shell::{self, ExeFinder, ShellError, ShellKind};

/// Scripted finder: maps exe name -> first hit.
struct FakeFinder {
    on_path: Vec<(&'static str, PathBuf)>,
}

impl FakeFinder {
    fn with(paths: &[(&'static str, &str)]) -> Self {
        Self {
            on_path: paths
                .iter()
                .map(|(name, path)| (*name, PathBuf::from(path)))
                .collect(),
        }
    }
}

impl ExeFinder for FakeFinder {
    fn find(&self, name: &str) -> Option<PathBuf> {
        self.on_path
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, path)| path.clone())
    }
}

#[test]
fn full_probe_order_is_pwsh_then_powershell_then_cmd() {
    let finder = FakeFinder::with(&[
        ("pwsh.exe", r"C:\Program Files\PowerShell\7\pwsh.exe"),
        (
            "powershell.exe",
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
        ),
        ("cmd.exe", r"C:\Windows\System32\cmd.exe"),
    ]);
    let shell = shell::resolve_default_shell(None, &finder).expect("resolve");
    assert_eq!(shell.kind, ShellKind::Pwsh);
    assert_eq!(
        shell.path,
        PathBuf::from(r"C:\Program Files\PowerShell\7\pwsh.exe")
    );
}

#[test]
fn missing_pwsh_falls_back_to_windows_powershell() {
    let finder = FakeFinder::with(&[
        (
            "powershell.exe",
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
        ),
        ("cmd.exe", r"C:\Windows\System32\cmd.exe"),
    ]);
    let shell = shell::resolve_default_shell(None, &finder).expect("resolve");
    assert_eq!(shell.kind, ShellKind::WindowsPowershell);
}

#[test]
fn last_resort_is_cmd() {
    let finder = FakeFinder::with(&[("cmd.exe", r"C:\Windows\System32\cmd.exe")]);
    let shell = shell::resolve_default_shell(None, &finder).expect("resolve");
    assert_eq!(shell.kind, ShellKind::Cmd);
}

#[test]
fn cmd_always_resolves_from_system32_even_off_path() {
    // cmd.exe is the floor: probed at its absolute System32 location so a
    // stripped PATH can still run commands.
    let finder = FakeFinder::with(&[]);
    let shell = shell::resolve_default_shell(None, &finder).expect("resolve");
    assert_eq!(shell.kind, ShellKind::Cmd);
    assert_eq!(
        shell.path.to_string_lossy().to_lowercase(),
        r"c:\windows\system32\cmd.exe"
    );
}

#[test]
fn config_override_pins_one_kind_and_bypasses_order() {
    let finder = FakeFinder::with(&[
        ("pwsh.exe", r"C:\p\pwsh.exe"),
        ("powershell.exe", r"C:\w\powershell.exe"),
        ("cmd.exe", r"C:\s\cmd.exe"),
    ]);
    let shell =
        shell::resolve_default_shell(Some(ShellKind::WindowsPowershell), &finder).expect("pin");
    assert_eq!(shell.kind, ShellKind::WindowsPowershell);
    assert_eq!(shell.path, PathBuf::from(r"C:\w\powershell.exe"));
}

#[test]
fn override_for_a_missing_shell_is_a_typed_error() {
    let finder = FakeFinder::with(&[]);
    let error = shell::resolve_default_shell(Some(ShellKind::Pwsh), &finder)
        .expect_err("pwsh is not installed here");
    assert!(matches!(error, ShellError::NotFound { .. }));
}

#[test]
fn live_machine_resolves_an_existing_shell() {
    let shell = shell::resolve_default_shell(None, &shell::PathFinder).expect("live");
    assert!(
        shell.path.is_file(),
        "resolved shell must exist: {:?}",
        shell.path
    );
    if shell::PathFinder.find("pwsh.exe").is_some() {
        assert_eq!(shell.kind, ShellKind::Pwsh, "pwsh present => it wins");
    }
}
