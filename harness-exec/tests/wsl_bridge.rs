//! WSL2 bridge tests (plan todo 20): typed path mapping both directions,
//! defensive wsl.exe output parsing, typed detection (fake-probe matrix),

// The bridge maps Windows drive paths <-> /mnt drives; std path parsing
// makes drive paths relative on unix, so the whole suite is windows-only.
#![cfg(windows)]
//! routing resolution, and a live bridge run gated on real availability.

use std::path::{Path, PathBuf};

use harness_exec::spec::{ExecError, ExecSpec, RunIn};
use harness_exec::wsl::{
    self, PathMapError, SYSTEM_WSL_EXE, WslProbeOutput, WslStatus, WslVersion,
};

// ---------- in-memory path mapping: C:\ <-> /mnt/c ----------

#[test]
fn windows_drive_paths_map_to_mnt() {
    let cases = [
        (r"C:\foo\bar", "/mnt/c/foo/bar"),
        (r"c:\", "/mnt/c/"),
        (r"C:\", "/mnt/c/"),
        (r"D:\deep\nested\file.txt", "/mnt/d/deep/nested/file.txt"),
        // forward slashes accepted
        (r"C:/foo/bar", "/mnt/c/foo/bar"),
        (r"Z:\a b\c-d", "/mnt/z/a b/c-d"),
    ];
    for (win, expect) in cases {
        let mapped = wsl::windows_to_wsl(Path::new(win)).expect("map");
        assert_eq!(mapped, expect, "mapping {win}");
    }
}

#[test]
fn non_drive_windows_paths_are_typed_errors() {
    for bad in [r"\\server\share\file", r"relative\path", "", r"1:\x"] {
        let result = wsl::windows_to_wsl(Path::new(bad));
        assert!(matches!(result, Err(PathMapError { .. })), "{bad}");
    }
}

#[test]
fn mnt_paths_map_back_to_drives() {
    let cases = [
        ("/mnt/c/foo/bar", PathBuf::from(r"C:\foo\bar")),
        ("/mnt/d/", PathBuf::from(r"D:\")),
        ("/mnt/z/a b/c-d.txt", PathBuf::from(r"Z:\a b\c-d.txt")),
        // single-component drive root keeps the trailing separator shape
        ("/mnt/e", PathBuf::from(r"E:\")),
    ];
    for (wsl_path, expect) in cases {
        let mapped = wsl::wsl_to_windows(wsl_path).expect("map back");
        assert_eq!(mapped, expect, "reverse {wsl_path}");
    }
}

#[test]
fn non_mnt_wsl_paths_are_typed_errors() {
    for bad in ["/home/user", "/mnt/usb/file", "mnt/c/x", "", "/mnt//x"] {
        let result = wsl::wsl_to_windows(bad);
        assert!(matches!(result, Err(PathMapError { .. })), "{bad}");
    }
}

#[test]
fn mapping_round_trips() {
    let original = r"C:\repo\harness\Cargo.toml";
    let there = wsl::windows_to_wsl(Path::new(original)).expect("there");
    let back = wsl::wsl_to_windows(&there).expect("back");
    assert_eq!(back, PathBuf::from(original));
}

// ---------- defensive wsl.exe output decoding ----------

#[test]
fn wsl_output_decodes_utf16le_and_utf8_and_garbage() {
    // `wsl --status` emits UTF-16LE on current Windows builds.
    let utf16: Vec<u8> = "Default Version: 2"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let decoded = wsl::decode_wsl_output(&utf16);
    assert_eq!(decoded, "Default Version: 2");

    let utf8 = b"Default Version: 2".to_vec();
    assert_eq!(wsl::decode_wsl_output(&utf8), "Default Version: 2");

    // Garbage never panics; content is only used for version sniffing.
    let garbage = [0xff, 0x00, 0xfe, 0x01, 0xd8, 0x00];
    let _ = wsl::decode_wsl_output(&garbage);
}

// ---------- typed detection over an injectable probe ----------

struct FakeProbe {
    exe: Option<PathBuf>,
    status: Option<i32>,
    status_out: WslProbeOutput,
    exec_exit: Option<i32>,
}

impl FakeProbe {
    fn available() -> Self {
        Self {
            exe: Some(PathBuf::from(r"C:\Windows\System32\wsl.exe")),
            status: Some(0),
            status_out: WslProbeOutput::Bytes(
                "Default Version: 2"
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
            ),
            exec_exit: Some(0),
        }
    }
}

impl wsl::WslProbe for FakeProbe {
    fn find_wsl_exe(&self) -> Option<PathBuf> {
        self.exe.clone()
    }
    fn run_wsl(&self, args: &[&str]) -> Option<(i32, WslProbeOutput)> {
        match args {
            ["--status"] => self.status.map(|code| (code, self.status_out.clone())),
            ["--exec", "true"] => self.exec_exit.map(|code| (code, WslProbeOutput::Empty)),
            _ => None,
        }
    }
}

#[test]
fn detection_reports_available_wsl2() {
    let status = wsl::detect_wsl_with(&FakeProbe::available());
    let WslStatus::Available { version, .. } = &status else {
        panic!("expected Available, got {status:?}");
    };
    assert_eq!(*version, WslVersion::Wsl2);
}

#[test]
fn detection_types_missing_wsl_without_errors() {
    let mut probe = FakeProbe::available();
    probe.exe = None;
    assert_eq!(wsl::detect_wsl_with(&probe), WslStatus::NoWsl);
}

#[test]
fn detection_types_wsl_without_any_distribution() {
    let mut probe = FakeProbe::available();
    probe.exec_exit = Some(-1);
    assert_eq!(wsl::detect_wsl_with(&probe), WslStatus::NoDistribution);
}

#[test]
fn detection_parses_version_defensively() {
    let mut probe = FakeProbe::available();
    probe.status_out = WslProbeOutput::Bytes(b"\xff\xfe\xff\xfe".to_vec()); // garbage
    let status = wsl::detect_wsl_with(&probe);
    // exec succeeded => Available; unparsable version is typed Unknown,
    // never a guess.
    let WslStatus::Available { version, .. } = status else {
        panic!("expected Available, got {status:?}");
    };
    assert_eq!(version, WslVersion::Unknown);
}

#[test]
fn detection_types_probe_failure() {
    let mut probe = FakeProbe::available();
    probe.exec_exit = None; // wsl.exe exists but cannot execute anything
    assert!(matches!(
        wsl::detect_wsl_with(&probe),
        WslStatus::ProbeFailed(_)
    ));
}

#[test]
fn wsl1_default_is_typed_not_available_wsl2() {
    let mut probe = FakeProbe::available();
    probe.status_out = WslProbeOutput::Bytes(b"Default Version: 1".to_vec());
    let status = wsl::detect_wsl_with(&probe);
    let WslStatus::Available { version, .. } = status else {
        panic!("expected Available, got {status:?}");
    };
    assert_eq!(version, WslVersion::Wsl1);
}

// ---------- routing resolution (pure) ----------

fn native_spec() -> ExecSpec {
    harness_exec::spec::ExecSpec {
        program: "cmd".into(),
        args: vec!["/c".into(), "echo hi".into()],
        cwd: None,
        env: Default::default(),
        timeout: std::time::Duration::from_secs(5),
        size: harness_exec::spec::ScreenSize::default(),
        run_in: RunIn::Native,
    }
}

#[test]
fn native_routing_never_probes_wsl() {
    let routing = wsl::resolve_routing(
        &native_spec(),
        &FakeProbe {
            exe: None, // no WSL anywhere: default operation must not care
            status: None,
            status_out: WslProbeOutput::Empty,
            exec_exit: None,
        },
    );
    assert!(matches!(routing, wsl::Routing::Native));
}

#[test]
fn wsl_routing_resolves_the_exec_command_line() {
    let mut spec = native_spec();
    spec.run_in = RunIn::Wsl;
    spec.program = "grep".into();
    spec.args = vec!["-n".into(), "needle".into(), r"C:\logs\a.txt".into()];
    let routing = wsl::resolve_routing(&spec, &FakeProbe::available());
    let wsl::Routing::Wsl { exe, argv } = routing else {
        panic!("expected Wsl routing");
    };
    assert_eq!(exe, PathBuf::from(r"C:\Windows\System32\wsl.exe"));
    assert_eq!(
        argv,
        vec![
            "--exec".to_string(),
            "grep".to_string(),
            "-n".to_string(),
            "needle".to_string(),
            "/mnt/c/logs/a.txt".to_string(),
        ]
    );
}

#[test]
fn wsl_routing_on_a_wsl_free_machine_is_a_typed_error() {
    let mut spec = native_spec();
    spec.run_in = RunIn::Wsl;
    let routing = wsl::resolve_routing(
        &spec,
        &FakeProbe {
            exe: None,
            status: None,
            status_out: WslProbeOutput::Empty,
            exec_exit: None,
        },
    );
    let wsl::Routing::Unavailable(status) = routing else {
        panic!("expected Unavailable");
    };
    assert_eq!(status, WslStatus::NoWsl);
    // The typed error shape the runner surfaces.
    let error = ExecError::from(status);
    assert!(matches!(error, ExecError::WslUnavailable { .. }));
}

// ---------- live bridge (gated on real availability) ----------

#[test]
fn live_wsl_bridge_runs_a_linux_command_when_available() {
    let status = wsl::detect_wsl();
    let WslStatus::Available { .. } = &status else {
        eprintln!("skipping live bridge: WSL not available ({status:?})");
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = harness_exec::runner::PtyRunner::with_spill_root(dir.path());
    let mut spec = native_spec();
    spec.run_in = RunIn::Wsl;
    spec.program = "echo".into();
    spec.args = vec!["BRIDGE-20-OK".into()];
    spec.timeout = std::time::Duration::from_secs(120);
    let outcome = runner.run(&spec).expect("run");
    assert_eq!(
        outcome.status,
        harness_exec::spec::ExecStatus::Completed { exit_code: 0 },
        "model text: {}",
        outcome.model_text
    );
    assert!(
        outcome.model_text.contains("BRIDGE-20-OK"),
        "bridge output must reach the screen: {}",
        outcome.model_text
    );
}

#[test]
fn wsl_exe_constant_points_at_system32() {
    assert_eq!(
        PathBuf::from(SYSTEM_WSL_EXE),
        PathBuf::from(r"C:\Windows\System32\wsl.exe")
    );
}
