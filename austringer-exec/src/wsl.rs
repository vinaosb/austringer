//! WSL2 bridge (plan todo 20): typed detection, `wsl.exe --exec` command
//! routing, and in-memory `C:\` ↔ `/mnt/c` path mapping.
//!
//! WSL is an OPTIONAL routing target, never a requirement: default
//! operation ([`crate::spec::RunIn::Native`]) never touches this module's
//! probe, and a WSL-free machine gets a typed [`WslStatus::NoWsl`] — no
//! panics, no stderr noise.
//!
//! # Detection truth
//!
//! "Available" means a default distribution actually EXECUTES commands:
//! `wsl.exe --exec true` exiting 0. The advertised default version is
//! parsed from `wsl.exe --status` output, which is UTF-16LE on current
//! Windows builds (see [`decode_wsl_output`]) — decoding is defensive and
//! a garbled version is typed [`WslVersion::Unknown`], never guessed.

use std::fmt;
use std::path::Prefix as ComponentPrefix;
use std::path::{Component, Path, PathBuf};

use crate::spec::{ExecSpec, RunIn};

/// Canonical wsl.exe location (probed first; PATH is the fallback).
pub const SYSTEM_WSL_EXE: &str = r"C:\Windows\System32\wsl.exe";

/// Why a path could not be mapped. Mapping is in-memory and pure; both
/// directions refuse anything outside the drive ↔ `/mnt` relationship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathMapError {
    /// The offending input.
    pub input: String,
    /// Human-facing reason.
    pub reason: &'static str,
}

/// Maps `C:\foo\bar` → `/mnt/c/foo/bar`. Accepts mixed separators.
/// UNC paths (incl. `\\wsl$`) are refused: they have no in-memory
/// `/mnt` equivalent (`\\wsl$` is a Windows-visible projection of WSL
/// files, the wrong direction for this bridge).
pub fn windows_to_wsl(path: &Path) -> Result<String, PathMapError> {
    let unsupported = |reason: &'static str| PathMapError {
        input: path.to_string_lossy().into_owned(),
        reason,
    };
    let Some(first) = path.components().next() else {
        return Err(unsupported("empty path"));
    };
    let Component::Prefix(prefix) = first else {
        return Err(unsupported("not an absolute drive path"));
    };
    let drive = match prefix.kind() {
        // `Prefix` and its `Disk` variant exist on every platform; only
        // Windows path parsing ever produces them, so non-Windows inputs
        // are refused by the "not an absolute drive path" arm above.
        ComponentPrefix::Disk(byte) => byte,
        _ => return Err(unsupported("UNC and device paths have no /mnt mapping")),
    };
    let Some(drive) = char::from_u32(u32::from(drive)) else {
        return Err(unsupported("drive byte is not a letter"));
    };
    if !drive.is_ascii_alphabetic() {
        return Err(unsupported("drive is not an ASCII letter"));
    }
    let mut mapped = format!("/mnt/{}/", drive.to_ascii_lowercase());
    let rest: Vec<&str> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_str().unwrap_or("")),
            _ => None,
        })
        .collect();
    mapped.push_str(&rest.join("/"));
    Ok(mapped)
}

/// Maps `/mnt/c/foo/bar` → `C:\foo\bar`.
pub fn wsl_to_windows(wsl: &str) -> Result<PathBuf, PathMapError> {
    let unsupported = |reason: &'static str| PathMapError {
        input: wsl.to_string(),
        reason,
    };
    let Some(stripped) = wsl.strip_prefix("/mnt/") else {
        return Err(unsupported("not a /mnt/<drive> path"));
    };
    let mut segments = stripped.split('/');
    let Some(drive) = segments.next() else {
        return Err(unsupported("missing drive segment"));
    };
    let Some(letter) = drive.chars().next() else {
        return Err(unsupported("empty drive segment"));
    };
    if drive.len() != 1 || !letter.is_ascii_alphabetic() {
        return Err(unsupported("drive must be a single letter (drvfs mount)"));
    }
    let mut windows = PathBuf::new();
    windows.push(format!("{}:\\", letter.to_ascii_uppercase()));
    for segment in segments {
        windows.push(segment);
    }
    Ok(windows)
}

/// WSL generation the default distribution advertises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WslVersion {
    /// WSL1 translation layer.
    Wsl1,
    /// The bridge's target: a real Linux VM.
    Wsl2,
    /// `--status` output was unparsable; command execution still works.
    Unknown,
}

/// Typed WSL availability. `NoWsl`/`NoDistribution` are normal states of
/// a healthy machine, not errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WslStatus {
    /// `wsl.exe --exec` demonstrably works; `exe` is its absolute path.
    Available {
        /// Resolved wsl.exe.
        exe: PathBuf,
        /// Advertised default-version.
        version: WslVersion,
    },
    /// No wsl.exe on this machine (e.g. Server core, stripped images).
    NoWsl,
    /// wsl.exe exists but no distribution is installed/running.
    NoDistribution,
    /// wsl.exe exists but could not be executed at all (broken install).
    ProbeFailed(String),
}

impl fmt::Display for WslStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Available { exe, version } => {
                write!(f, "available at {} (default {version:?})", exe.display())
            }
            Self::NoWsl => write!(f, "wsl.exe not found (native-only machine)"),
            Self::NoDistribution => write!(f, "wsl.exe present but no distribution"),
            Self::ProbeFailed(detail) => write!(f, "wsl.exe probe failed: {detail}"),
        }
    }
}

/// Captured command output of a probe run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WslProbeOutput {
    /// No bytes captured.
    Empty,
    /// Raw stdout bytes (encoding not yet assumed).
    Bytes(Vec<u8>),
}

/// The two probes detection needs; injectable for hermetic tests.
pub trait WslProbe {
    /// Locates wsl.exe (System32 first, PATH second), or `None`.
    fn find_wsl_exe(&self) -> Option<PathBuf>;
    /// Runs `wsl.exe <args>`, capturing exit code + stdout; `None` when
    /// the process could not be launched at all.
    fn run_wsl(&self, args: &[&str]) -> Option<(i32, WslProbeOutput)>;
}

/// Real-machine probe: std Command over the absolute System32 path.
#[derive(Debug, Default)]
pub struct SystemProbe;

impl WslProbe for SystemProbe {
    fn find_wsl_exe(&self) -> Option<PathBuf> {
        let system = PathBuf::from(SYSTEM_WSL_EXE);
        if system.is_file() {
            return Some(system);
        }
        std::env::var_os("PATH")?
            .to_string_lossy()
            .split(';')
            .find_map(|dir| {
                let candidate = Path::new(dir).join("wsl.exe");
                candidate.is_file().then_some(candidate)
            })
    }

    fn run_wsl(&self, args: &[&str]) -> Option<(i32, WslProbeOutput)> {
        let exe = self.find_wsl_exe()?;
        let output = std::process::Command::new(exe).args(args).output().ok()?;
        let code = output.status.code().unwrap_or(-1);
        Some((code, WslProbeOutput::Bytes(output.stdout)))
    }
}

/// Cached machine detection (the exec probe costs one process launch;
/// availability does not change mid-process).
pub fn detect_wsl() -> WslStatus {
    static CACHED: std::sync::OnceLock<WslStatus> = std::sync::OnceLock::new();
    CACHED.get_or_init(|| detect_wsl_with(&SystemProbe)).clone()
}

/// Detection over an injected probe, uncached (test seam).
pub fn detect_wsl_with(probe: &dyn WslProbe) -> WslStatus {
    let Some(exe) = probe.find_wsl_exe() else {
        return WslStatus::NoWsl;
    };
    // The availability oracle: a default distribution that executes.
    match probe.run_wsl(&["--exec", "true"]) {
        Some((0, _)) => {}
        Some(_) => return WslStatus::NoDistribution,
        None => return WslStatus::ProbeFailed("--exec true could not launch".into()),
    }
    let version = match probe.run_wsl(&["--status"]) {
        Some((0, WslProbeOutput::Bytes(bytes))) => {
            parse_default_version(&decode_wsl_output(&bytes))
        }
        Some((0, WslProbeOutput::Empty)) => WslVersion::Unknown,
        _ => WslVersion::Unknown,
    };
    WslStatus::Available { exe, version }
}

/// Decodes wsl.exe output defensively: current builds emit UTF-16LE when
/// piped, older ones UTF-8. Heuristic: an ASCII-heavy even-length byte
/// stream with NULs in odd positions is UTF-16LE; anything else is
/// UTF-8 lossy. Garbage never panics.
pub fn decode_wsl_output(bytes: &[u8]) -> String {
    let looks_utf16 = bytes.len() >= 2
        && bytes.len().is_multiple_of(2)
        && bytes
            .as_chunks::<2>()
            .0
            .iter()
            .take(32)
            .filter(|pair| pair[1] == 0 && pair[0].is_ascii())
            .count()
            >= bytes.len() / 4;
    if looks_utf16 {
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn parse_default_version(status: &str) -> WslVersion {
    let Some((_, tail)) = status.split_once("Default Version:") else {
        return WslVersion::Unknown;
    };
    let digit = tail.trim_start().chars().next().unwrap_or_default();
    match digit {
        '1' => WslVersion::Wsl1,
        '2' => WslVersion::Wsl2,
        _ => WslVersion::Unknown,
    }
}

/// Where one [`ExecSpec`] actually executes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Routing {
    /// Direct spawn: unchanged behavior, zero WSL involvement.
    Native,
    /// Spawn `exe --exec <program> <args…>` with drive paths mapped.
    Wsl {
        /// Resolved wsl.exe.
        exe: PathBuf,
        /// Full argument vector starting with `--exec`.
        argv: Vec<String>,
    },
    /// WSL was requested but the machine cannot provide it (typed).
    Unavailable(WslStatus),
}

/// Resolves one spec's routing. Native specs never probe WSL, so a
/// WSL-free machine pays nothing and errors nothing by default.
pub fn resolve_routing(spec: &ExecSpec, probe: &dyn WslProbe) -> Routing {
    if spec.run_in == RunIn::Native {
        return Routing::Native;
    }
    match detect_wsl_with(probe) {
        WslStatus::Available { exe, .. } => route_available(spec, &exe),
        status => Routing::Unavailable(status),
    }
}

/// Resolves one spec's routing against the process-lifetime cached
/// detection ([`detect_wsl`]): the two-spawn probe runs once per process,
/// not once per WSL-routed command. The production path.
pub fn resolve_routing_cached(spec: &ExecSpec) -> Routing {
    if spec.run_in == RunIn::Native {
        return Routing::Native;
    }
    match detect_wsl() {
        WslStatus::Available { exe, .. } => route_available(spec, &exe),
        status => Routing::Unavailable(status),
    }
}

/// Builds the WSL command line for an available machine: `--exec` with
/// the program verbatim and drive-path arguments mapped. Pure.
pub fn route_available(spec: &ExecSpec, exe: &Path) -> Routing {
    let mut argv = vec!["--exec".to_string(), spec.program.clone()];
    for arg in &spec.args {
        argv.push(map_argument(arg));
    }
    Routing::Wsl {
        exe: exe.to_path_buf(),
        argv,
    }
}

/// Maps one command argument when it is clearly an absolute Windows drive
/// path; everything else (flags, relative paths, URLs) passes untouched.
fn map_argument(arg: &str) -> String {
    let path = Path::new(arg);
    if !path.is_absolute() {
        return arg.to_string();
    }
    windows_to_wsl(path).unwrap_or_else(|_| arg.to_string())
}
