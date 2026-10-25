//! M15 external tool executor tests: lifecycle through the mocktool
//! stand-in (T40 mock-child precedent), env redaction, bounded
//! NonZeroExit, timeout kill-tree with a ps check, the SHA-256 install
//! gate, typed sandbox denial, default-disabled manifests, and registry
//! routing through the executor. Hermetic: no network, no real tools.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use austringer_media::error::{InstallFailure, MediaError};
use austringer_media::format::FormatId;
use austringer_media::tool::{
    OP_VERSION_CHECK, ToolCall, ToolConfig, ToolError, ToolExecutor, ToolOverride, manifests,
};

/// Compile-time-anchored golden dir (a hardcoded `"\\..\\..."` suffix
/// breaks on unix, where the backslash is not a path separator).
fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("austringer-test-support")
        .join("golden")
        .join("mocktool")
}

/// T40-style re-exec entry: when AUSTRINGER_MOCKTOOL_ARGV is set, THIS test
/// becomes a mocktool child; the executor spawns this binary as the
/// substituted tool program.
#[test]
fn mocktool_child_entry() {
    if std::env::var("AUSTRINGER_MOCKTOOL_ARGV").is_err() {
        return; // normal suite run: nothing to do
    }
    std::process::exit(austringer_test_support::mocktool::run_from_env());
}

/// The libtest banner printed before the child test body runs; tool
/// stdout assertions look at everything after it.
fn payload(stdout: &[u8]) -> String {
    let text = String::from_utf8_lossy(stdout).into_owned();
    match text.split_once("running 1 test\n") {
        Some((_, tail)) => tail.to_owned(),
        None => text,
    }
}

fn mocktool_override(timeout_ms: Option<u64>) -> ToolOverride {
    ToolOverride {
        program: Some(
            std::env::current_exe()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ),
        args: vec![
            "--exact".into(),
            "mocktool_child_entry".into(),
            "--nocapture".into(),
        ],
        timeout_ms,
    }
}

fn executor_with(tool: &str, r#override: ToolOverride, session_temp: PathBuf) -> ToolExecutor {
    ToolExecutor::new(ToolConfig {
        enabled: [(tool.to_owned(), r#override)].into(),
        install_enabled: false,
        session_temp,
        env: BTreeMap::new(),
    })
}

fn env_map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

/// MANIFESTS: the plan tools (ffmpeg and ffprobe counted separately),
/// every one disabled by default.
#[test]
fn manifests_cover_the_plan_tools_all_disabled_by_default() {
    let table = manifests();
    for name in [
        "blender",
        "ffmpeg",
        "ffprobe",
        "whisper-cpp",
        "libreoffice",
        "pandoc",
        "freecadcmd",
        "gdal",
    ] {
        let spec = table
            .get(name)
            .unwrap_or_else(|| panic!("manifest for {name} is committed"));
        assert!(!spec.install_hint.is_empty(), "{name}: actionable hint");
        assert!(!spec.version_pin.is_empty(), "{name}: pinned version line");
        assert!(spec.timeout_ms > 0, "{name}: hard timeout declared");
        assert!(
            spec.op_template(OP_VERSION_CHECK).is_some(),
            "{name}: version-check template declared"
        );
    }
    assert!(ToolConfig::default().enabled.is_empty(), "opt-in only");
}

/// DEFAULT-DISABLED: running any tool without opt-in is the typed
/// ToolUnavailable carrying the manifest install hint - nothing spawns.
#[test]
fn disabled_tool_is_typed_unavailable_with_manifest_hint() {
    let executor = ToolExecutor::new(ToolConfig::default());
    let error = executor
        .run(&ToolCall::new("blender", OP_VERSION_CHECK).env(&env_map(&[("PATH", "C:\\bin")])))
        .expect_err("disabled tool refuses");
    match error {
        ToolError::ToolUnavailable { tool, install_hint } => {
            assert_eq!(tool, "blender");
            assert!(
                install_hint.contains("opt-in"),
                "hint names the opt-in path: {install_hint}"
            );
        }
        other => panic!("expected ToolUnavailable, got {other:?}"),
    }
    let unknown = executor
        .run(&ToolCall::new("not-a-tool", OP_VERSION_CHECK))
        .expect_err("unknown tool refuses");
    assert!(matches!(unknown, ToolError::ToolUnavailable { .. }));
}

/// LIFECYCLE + GOLDEN: an enabled mocktool substitution runs the op
/// through the executor and returns the REAL captured golden bytes with
/// producer disclosure (engine = tool, pinned version, command line).
#[test]
fn enabled_mocktool_runs_and_serves_the_real_golden_capture() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = executor_with(
        "ffprobe",
        mocktool_override(None),
        temp.path().to_path_buf(),
    );
    let golden = golden_dir().display().to_string();
    let env = env_map(&[
        ("PATH", "C:\\Windows"),
        ("AUSTRINGER_MOCKTOOL_ARGV", "golden ffprobe/probe-av-json"),
        ("AUSTRINGER_MOCKTOOL_GOLDEN_DIR", golden.as_str()),
    ]);
    let run = executor
        .run(&ToolCall::new("ffprobe", OP_VERSION_CHECK).env(&env))
        .expect("mocktool lifecycle is green");
    assert_eq!(run.exit_code, 0);
    let probe: serde_json::Value =
        serde_json::from_str(&payload(&run.stdout)).expect("real ffprobe JSON capture parses");
    assert_eq!(probe["streams"][0]["codec_name"], "pcm_s16le");
    let producer = run.producer();
    assert_eq!(producer.engine, "ffprobe");
    assert_eq!(
        producer.version,
        manifests()
            .get("ffprobe")
            .expect("ffprobe manifest")
            .version_pin
    );
    assert!(
        producer
            .command
            .as_deref()
            .is_some_and(|command| command.contains("--exact"))
    );
}

/// ENV REDACTION: ANTHROPIC_API_KEY never reaches the child; the rest of
/// the caller env does.
#[test]
fn child_environment_is_redacted() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = executor_with("pandoc", mocktool_override(None), temp.path().to_path_buf());
    let env = env_map(&[
        ("PATH", "C:\\Windows"),
        ("ANTHROPIC_API_KEY", "sk-never-reaches-the-child"),
        ("OPENAI_API_KEY", "sk-either"),
        ("AUSTRINGER_MOCKTOOL_ARGV", "env"),
    ]);
    let run = executor
        .run(&ToolCall::new("pandoc", OP_VERSION_CHECK).env(&env))
        .expect("env mode runs");
    let names: Vec<String> = payload(&run.stdout).lines().map(String::from).collect();
    assert!(
        !names.iter().any(|name| name == "ANTHROPIC_API_KEY"),
        "secrets are absent from the child env: {names:?}"
    );
    assert!(
        !names.iter().any(|name| name == "OPENAI_API_KEY"),
        "the whole default deny list applies"
    );
    assert!(
        names.iter().any(|name| name == "PATH"),
        "innocuous vars survive"
    );
}

/// NON-ZERO EXIT: typed error with the exit code and a bounded stderr
/// head - never unbounded piping.
#[test]
fn nonzero_exit_is_typed_with_bounded_stderr_head() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = executor_with("ffmpeg", mocktool_override(None), temp.path().to_path_buf());
    let env = env_map(&[("AUSTRINGER_MOCKTOOL_ARGV", "exit 7 boom exploded")]);
    let error = executor
        .run(&ToolCall::new("ffmpeg", OP_VERSION_CHECK).env(&env))
        .expect_err("failing tool exit is typed");
    match error {
        ToolError::NonZeroExit {
            tool,
            code,
            stderr_head,
        } => {
            assert_eq!(tool, "ffmpeg");
            assert_eq!(code, 7);
            assert!(stderr_head.contains("boom"), "stderr head carries why");
            assert!(stderr_head.len() <= 4096, "stderr is head-bounded");
        }
        other => panic!("expected NonZeroExit, got {other:?}"),
    }
}

/// TIMEOUT KILL-TREE: a hung tool with live grandchildren is killed
/// whole (Job Object on Windows); a ps check proves no orphans survive.
#[test]
fn timeout_kills_the_whole_tree_no_orphans() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pids_file = temp.path().join("pids.txt");
    let pids_path = pids_file.to_string_lossy().into_owned();
    let executor = executor_with(
        "blender",
        mocktool_override(Some(1_500)),
        temp.path().to_path_buf(),
    );
    let env = env_map(&[
        ("AUSTRINGER_MOCKTOOL_ARGV", "hang 2"),
        ("AUSTRINGER_MOCKTOOL_PIDS_FILE", &pids_path),
    ]);
    let error = executor
        .run(&ToolCall::new("blender", OP_VERSION_CHECK).env(&env))
        .expect_err("hung tool is timed out");
    match error {
        ToolError::Timeout { tool } => assert_eq!(tool, "blender"),
        other => panic!("expected Timeout, got {other:?}"),
    }
    let pids: Vec<u32> = std::fs::read_to_string(&pids_file)
        .expect("child recorded the tree pids before hanging")
        .split_whitespace()
        .filter_map(|word| word.parse::<u32>().ok())
        .filter(|pid| *pid > 0)
        .collect();
    assert!(pids.len() >= 3, "child + spawned grandchildren recorded");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    for pid in pids {
        while pid_is_alive(pid) {
            assert!(
                std::time::Instant::now() < deadline,
                "pid {pid} survived the kill-tree"
            );
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

#[cfg(windows)]
fn pid_is_alive(pid: u32) -> bool {
    let out = std::process::Command::new("tasklist")
        .args(["/NH", "/FI", &format!("PID eq {pid}")])
        .output();
    match out {
        Ok(out) => String::from_utf8_lossy(&out.stdout).contains(&pid.to_string()),
        Err(_) => false,
    }
}

#[cfg(unix)]
fn pid_is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .is_ok_and(|out| out.status.success())
}

/// INSTALL GATE: SHA-256 is verified before anything is written or run;
/// a wrong digest fails closed with nothing executed.
#[test]
fn wrong_sha_asset_fails_closed_nothing_executed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = ToolExecutor::new(ToolConfig {
        enabled: BTreeMap::new(),
        install_enabled: true,
        session_temp: temp.path().to_path_buf(),
        env: BTreeMap::new(),
    });
    let pin = austringer_safety::supply::ReleaseAsset {
        url: "https://example.invalid/pandoc.zip".into(),
        sha256: digest_of(b"good-bytes"),
        version: manifests()
            .get("pandoc")
            .expect("pandoc manifest")
            .version_pin
            .to_owned(),
    };
    let error = executor
        .install_asset("pandoc", &pin, b"tampered-bytes")
        .expect_err("wrong sha refuses");
    assert_eq!(
        error,
        ToolError::InstallFailed {
            tool: "pandoc".into(),
            reason: InstallFailure::ShaMismatch
        }
    );
    let store = temp.path().join("toolstore");
    assert!(
        store
            .read_dir()
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(true),
        "nothing written on a failed pin"
    );
    let malformed = austringer_safety::supply::ReleaseAsset {
        sha256: "zz".into(),
        ..pin.clone()
    };
    assert_eq!(
        executor.install_asset("pandoc", &malformed, b"good-bytes"),
        Err(ToolError::InstallFailed {
            tool: "pandoc".into(),
            reason: InstallFailure::ShaMismatch
        }),
        "malformed pins fail closed"
    );
    let installed = executor
        .install_asset("pandoc", &pin, b"good-bytes")
        .expect("matching sha installs");
    assert!(installed.is_file(), "verified asset lands in the store");
    assert_eq!(std::fs::read(&installed).ok(), Some(b"good-bytes".to_vec()));
}

/// INSTALL OPT-IN: with the install flag off, install refuses before any
/// fetch or write.
#[test]
fn install_requires_the_explicit_opt_in_flag() {
    let temp = tempfile::tempdir().expect("tempdir");
    let executor = ToolExecutor::new(ToolConfig {
        enabled: BTreeMap::new(),
        install_enabled: false,
        session_temp: temp.path().to_path_buf(),
        env: BTreeMap::new(),
    });
    let pin = austringer_safety::supply::ReleaseAsset {
        url: "https://example.invalid/x".into(),
        sha256: digest_of(b"good-bytes"),
        version: "0".into(),
    };
    let error = executor
        .install_asset("pandoc", &pin, b"good-bytes")
        .expect_err("install is opt-in");
    assert!(matches!(error, ToolError::ToolUnavailable { .. }));
}

/// SANDBOX VIOLATION: an unsanctionable policy (relative session temp =
/// a relative writable root) is a typed denial carrying an austringer-safety
/// ViolationEvent - never a silent degrade, never a spawn.
#[test]
fn sandbox_violation_is_typed_denial_with_audit() {
    let executor = ToolExecutor::new(ToolConfig {
        enabled: [("blender".to_owned(), mocktool_override(None))].into(),
        install_enabled: false,
        session_temp: PathBuf::from("relative/not/absolute"),
        env: BTreeMap::new(),
    });
    let error = executor
        .run(&ToolCall::new("blender", OP_VERSION_CHECK).env(&env_map(&[])))
        .expect_err("relative writable root is unsanctionable");
    match error {
        ToolError::SandboxDenied { violation, .. } => {
            assert!(!violation.detail.is_empty(), "audit names the violation");
        }
        other => panic!("expected SandboxDenied, got {other:?}"),
    }
}

/// REGISTRY ROUTING: the M6 subprocess bindings consult the executor.
/// Default registry: genuinely-not-enabled -> ToolUnavailable. Enabled
/// tool: the binding reports the pending adapter honestly instead.
#[test]
fn registry_routes_subprocess_bindings_through_the_tool_executor() {
    use austringer_media::descriptor::Operation;
    use austringer_media::executor::OpRequest;

    let default = austringer_media::executor::registry();
    let error = default
        .dispatch(
            FormatId::Blend,
            OpRequest::Extract {
                bytes: b"BLENDER_v305",
                budgets: Default::default(),
            },
        )
        .expect_err("default: blender not enabled");
    match error {
        MediaError::ToolUnavailable { tool, install_hint } => {
            assert_eq!(tool, "blender");
            assert!(!install_hint.is_empty());
        }
        other => panic!("expected ToolUnavailable, got {other:?}"),
    }

    let temp = tempfile::tempdir().expect("tempdir");
    let enabled_registry =
        austringer_media::executor::registry().with_tools(ToolExecutor::new(ToolConfig {
            enabled: [("ffmpeg".to_owned(), mocktool_override(None))].into(),
            install_enabled: false,
            session_temp: temp.path().to_path_buf(),
            env: BTreeMap::new(),
        }));
    let error = enabled_registry
        .dispatch(
            FormatId::Webm,
            OpRequest::Extract {
                bytes: b"\x1A\x45\xDF\xA3",
                budgets: Default::default(),
            },
        )
        .expect_err("tool enabled, adapter pending");
    match error {
        MediaError::UnsupportedOperation { format, op, reason } => {
            assert_eq!(format, FormatId::Webm);
            assert_eq!(op, Operation::Extract);
            assert!(
                reason.to_lowercase().contains("adapter"),
                "names the pending adapter: {reason}"
            );
        }
        other => panic!("expected UnsupportedOperation, got {other:?}"),
    }
}

fn digest_of(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
