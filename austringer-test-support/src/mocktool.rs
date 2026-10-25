//! `mocktool`: the hermetic stand-in for external media tools (plan M15,
//! T40 mock-child precedent). One implementation, two entries:
//!
//! - the real `mocktool` binary (`src/bin/mocktool.rs`), for standalone
//!   use and F3/dev-machine lanes;
//! - the test re-exec entry ([`run_from_env`]): a test binary re-executes
//!   itself with `--exact <mocktool test>` and the mode words arrive via
//!   `AUSTRINGER_MOCKTOOL_ARGV` (exactly how T40's mock backend child works).
//!
//! Modes (argv words, e.g. `exit 1 boom`):
//!
//! - `env` — print the child's environment KEY names (never values),
//!   sorted, one per line. The env-redaction assertion surface.
//! - `golden <name>` — print golden file `<name>.golden` from
//!   `AUSTRINGER_MOCKTOOL_GOLDEN_DIR` byte-for-byte. Golden files are real
//!   captures (see `golden/mocktool/capture.md`); never hand-written.
//! - `stagecopy` — copy the file at `AUSTRINGER_MEDIA_STAGE_IN` to
//!   `AUSTRINGER_MEDIA_STAGE_OUT` (the M17 adapter's staging contract: the
//!   adapter publishes its staged input/output paths in the child env;
//!   real ffmpeg ignores them, the mock is the pass-through remux echo).
//! - `stagegolden <name>` — write golden `<name>.golden` bytes to
//!   `AUSTRINGER_MEDIA_STAGE_OUT` (the M17 canned-thumbnail lane).
//! - `blendbuild` / `blenddump` — the M16 parameterized blender
//!   stand-in: derive the scene census from the builder script's
//!   embedded spec / emit a fixture's census between the dump markers
//!   (see `mocktool/blend.rs` for the python-validation gate and the
//!   cwd staging contract).
//! - `stepbuild` / `stepdump` - the M20 parameterized freecadcmd
//!   stand-in: derive the topology census from the builder script's
//!   embedded spec, writing the AP214-flavored .step / emit the census
//!   between the dump markers (see `mocktool/step.rs`).
//! - `gdaltranslate` / `gdalinfo` - the M21 parameterized GDAL
//!   stand-in: georeferenced (optionally COG) GeoTIFF writer over the
//!   staged baseline + spec contract / gdalinfo-shaped `-json` census
//!   parsed back out of the staged raster (see `mocktool/gdal`).
//! - `exit <code> [msg...]` — write msg to stderr, exit with code.
//! - `write <path> <text>` — write text to path (utf-8), exit 0.
//! - `hang [children]` — spawn `children` (default 1) `sleep 60`
//!   grandchildren, record child+self pids to
//!   `AUSTRINGER_MOCKTOOL_PIDS_FILE` if set, then sleep 60. The
//!   timeout-kill-tree test surface.
//! - `sleep <secs>` — sleep, exit 0.
//!
//! No mode words: exit 2 with a stderr note (never a panic).

use std::io::Write;
use std::process::Command;

mod blend;
mod gdal;
mod python;
mod step;

pub use python::validate_python;

/// Re-exec entry: mode words come from `AUSTRINGER_MOCKTOOL_ARGV`
/// (whitespace-split, e.g. `"golden ffprobe/probe-av-json"`).
pub fn run_from_env() -> i32 {
    let Some(words) = std::env::var("AUSTRINGER_MOCKTOOL_ARGV").ok() else {
        eprintln!("mocktool: AUSTRINGER_MOCKTOOL_ARGV is unset");
        return 2;
    };
    dispatch_words(&words)
}

/// Dispatch mode words from an arbitrary env-var string (the M17
/// per-tool child entries read `AUSTRINGER_MOCKTOOL_FFMPEG_ARGV` /
/// `AUSTRINGER_MOCKTOOL_FFPROBE_ARGV` so one adapter call can drive two
/// distinct tool children).
pub fn dispatch_words(words: &str) -> i32 {
    let split: Vec<String> = words.split_whitespace().map(String::from).collect();
    let refs: Vec<&str> = split.iter().map(String::as_str).collect();
    dispatch(&refs)
}

/// Binary entry: mode words are argv[1..] (argv[0] is the program).
pub fn run_bin() -> i32 {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    dispatch(&refs)
}

fn dispatch(words: &[&str]) -> i32 {
    let Some((mode, rest)) = words.split_first() else {
        eprintln!("mocktool: no mode given");
        return 2;
    };
    match *mode {
        "env" => env_mode(),
        "golden" => golden_mode(rest.first().copied()),
        "stagecopy" => stagecopy_mode(),
        "stagegolden" => stagegolden_mode(rest.first().copied()),
        "blendbuild" => blend::build_mode(),
        "blenddump" => blend::dump_mode(),
        "stepbuild" => step::build_mode(),
        "stepdump" => step::dump_mode(),
        "gdaltranslate" => gdal::translate_mode(),
        "gdalinfo" => gdal::info_mode(),
        "exit" => exit_mode(rest),
        "write" => write_mode(rest),
        "hang" => hang_mode(rest.first().and_then(|n| n.parse::<u32>().ok())),
        "sleep" => sleep_mode(rest.first().and_then(|s| s.parse::<u64>().ok())),
        other => {
            eprintln!("mocktool: unknown mode {other}");
            2
        }
    }
}

fn stage_var(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
}

fn stagecopy_mode() -> i32 {
    let (Some(source), Some(target)) = (
        stage_var("AUSTRINGER_MEDIA_STAGE_IN"),
        stage_var("AUSTRINGER_MEDIA_STAGE_OUT"),
    ) else {
        eprintln!("mocktool: stagecopy needs AUSTRINGER_MEDIA_STAGE_IN/OUT");
        return 2;
    };
    match std::fs::copy(&source, &target) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!(
                "mocktool: stagecopy {} -> {} failed: {e}",
                source.display(),
                target.display()
            );
            2
        }
    }
}

fn stagegolden_mode(name: Option<&str>) -> i32 {
    let (Some(name), Some(target)) = (name, stage_var("AUSTRINGER_MEDIA_STAGE_OUT")) else {
        eprintln!("mocktool: stagegolden needs <name> and AUSTRINGER_MEDIA_STAGE_OUT");
        return 2;
    };
    let Some(dir) = std::env::var_os("AUSTRINGER_MOCKTOOL_GOLDEN_DIR").filter(|d| !d.is_empty())
    else {
        eprintln!("mocktool: AUSTRINGER_MOCKTOOL_GOLDEN_DIR is unset");
        return 2;
    };
    let path = std::path::Path::new(&dir).join(format!("{name}.golden"));
    match std::fs::read(&path) {
        Ok(bytes) => match std::fs::write(&target, &bytes) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!(
                    "mocktool: stagegolden write {} failed: {e}",
                    target.display()
                );
                2
            }
        },
        Err(e) => {
            eprintln!("mocktool: golden {name} unreadable: {e}");
            2
        }
    }
}

fn env_mode() -> i32 {
    let mut keys: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
    keys.sort();
    let mut out = std::io::stdout();
    for key in keys {
        let _ = writeln!(out, "{key}");
    }
    0
}

fn golden_mode(name: Option<&str>) -> i32 {
    let Some(name) = name else {
        eprintln!("mocktool: golden mode needs a name");
        return 2;
    };
    let Some(dir) = std::env::var_os("AUSTRINGER_MOCKTOOL_GOLDEN_DIR").filter(|d| !d.is_empty())
    else {
        eprintln!("mocktool: AUSTRINGER_MOCKTOOL_GOLDEN_DIR is unset");
        return 2;
    };
    let path = std::path::Path::new(&dir).join(format!("{name}.golden"));
    match std::fs::read(&path) {
        Ok(bytes) => {
            let mut out = std::io::stdout();
            let _ = out.write_all(&bytes);
            let _ = out.flush();
            0
        }
        Err(e) => {
            eprintln!("mocktool: golden {name} unreadable: {e}");
            2
        }
    }
}

fn exit_mode(rest: &[&str]) -> i32 {
    let Some(code) = rest.first().and_then(|c| c.parse::<i32>().ok()) else {
        eprintln!("mocktool: exit mode needs a numeric code");
        return 2;
    };
    if !rest.is_empty() {
        let msg = rest[1..].join(" ");
        if !msg.is_empty() {
            eprintln!("{msg}");
        }
    }
    code
}

fn write_mode(rest: &[&str]) -> i32 {
    let (Some(path), Some(text)) = (rest.first().copied(), rest.get(1).copied()) else {
        eprintln!("mocktool: write mode needs <path> <text>");
        return 2;
    };
    match std::fs::write(path, text) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("mocktool: write {path} failed: {e}");
            2
        }
    }
}

fn hang_mode(children: Option<u32>) -> i32 {
    let spawn = current_spawn_style();
    let mut pids: Vec<u32> = Vec::new();
    for _ in 0..children.unwrap_or(1) {
        let mut child = spawn("sleep 60")
            .stdout(std::process::Stdio::null())
            .spawn();
        if let Ok(child) = child.as_mut() {
            pids.push(child.id());
        }
    }
    if let Some(file) = std::env::var_os("AUSTRINGER_MOCKTOOL_PIDS_FILE").filter(|f| !f.is_empty())
    {
        let mut line = std::process::id().to_string();
        for pid in &pids {
            line.push(' ');
            line.push_str(&pid.to_string());
        }
        let _ = std::fs::write(file, format!("{line}\n"));
    }
    std::thread::sleep(std::time::Duration::from_secs(60));
    0
}

fn sleep_mode(secs: Option<u64>) -> i32 {
    std::thread::sleep(std::time::Duration::from_secs(secs.unwrap_or(60)));
    0
}

// -- self-spawn styles ------------------------------------------------

/// Binary lane: children are `mocktool <mode words>` of this same exe.
fn bin_spawn(mode: &str) -> Command {
    let words: Vec<String> = mode.split_whitespace().map(String::from).collect();
    let mut cmd = Command::new(std::env::current_exe().unwrap_or_default());
    cmd.args(&words);
    cmd
}

/// Re-exec lane: children re-run the host test binary's mocktool entry;
/// the mode travels via `AUSTRINGER_MOCKTOOL_ARGV` (T40 pattern), so the
/// child argv needs only the libtest selector.
fn reexec_spawn(mode: &str) -> Command {
    let mut cmd = Command::new(std::env::current_exe().unwrap_or_default());
    let argv: Vec<String> = std::env::args().collect();
    if let Some(pos) = argv.iter().position(|a| a == "--exact")
        && let Some(name) = argv.get(pos + 1)
    {
        cmd.arg("--exact").arg(name).arg("--nocapture");
    }
    cmd.env("AUSTRINGER_MOCKTOOL_ARGV", mode);
    cmd
}

fn current_spawn_style() -> fn(&str) -> Command {
    if std::env::var_os("AUSTRINGER_MOCKTOOL_ARGV").is_some() {
        reexec_spawn
    } else {
        bin_spawn
    }
}
