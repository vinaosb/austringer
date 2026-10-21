//! Stage (c): static-checker adapters, one per ecosystem. `cargo check
//! --message-format=json` for Rust (structured extraction of file:line +
//! PRIMARY message only) and `tsc --noEmit` for TypeScript. Absent tools
//! and missing ecosystem markers are typed skips.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::ToolContext;
use super::error::{SkipReason, Stage};
use super::format::PROBE_TIMEOUT;
use super::jsonscan;
use super::structured::StructuredError;
use crate::spec::{ExecSpec, RunIn, ScreenSize};

/// One compiler/static-check ecosystem behind the adapter trait.
pub trait StaticChecker: Send + Sync {
    fn tool(&self) -> &'static str;
    fn claims(&self, rel: &Path) -> bool;
    /// Ecosystem marker present in the workspace root (Cargo.toml, …)?
    fn marker(&self, root: &Path) -> bool;
    fn probe_spec(&self, root: &Path) -> ExecSpec;
    fn check_spec(&self, ctx: &ToolContext) -> ExecSpec;
    fn extract(&self, text: &str) -> Vec<StructuredError>;
}

pub struct CargoCheck;
pub struct TscCheck;

pub(super) fn default_checkers() -> Vec<Box<dyn StaticChecker>> {
    vec![Box::new(CargoCheck), Box::new(TscCheck)]
}

fn spec(
    program: &str,
    args: &[String],
    root: &Path,
    timeout: Duration,
    env: BTreeMap<String, String>,
) -> ExecSpec {
    ExecSpec {
        program: program.to_owned(),
        args: args.to_vec(),
        cwd: Some(root.to_path_buf()),
        env,
        env_scrub: None,
        timeout,
        size: ScreenSize {
            cols: 1000,
            rows: 60,
        },
        run_in: RunIn::Native,
    }
}

fn ext(rel: &Path, wanted: &[&str]) -> bool {
    rel.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| wanted.contains(&e))
}

impl StaticChecker for CargoCheck {
    fn tool(&self) -> &'static str {
        "cargo check"
    }
    fn claims(&self, rel: &Path) -> bool {
        ext(rel, &["rs", "toml"])
    }
    fn marker(&self, root: &Path) -> bool {
        root.join("Cargo.toml").is_file()
    }
    fn probe_spec(&self, root: &Path) -> ExecSpec {
        spec(
            "cargo",
            &["--version".to_owned()],
            root,
            PROBE_TIMEOUT,
            BTreeMap::new(),
        )
    }
    fn check_spec(&self, ctx: &ToolContext) -> ExecSpec {
        // A shadow-private target dir keeps the user's build cache
        // untouched; cold builds inside a fresh shadow are the honest
        // cost of isolation (the daemon may later wire a shared dir).
        let mut env = BTreeMap::new();
        env.insert(
            "CARGO_TARGET_DIR".to_owned(),
            ctx.root
                .join(".harness/preflight-target")
                .to_string_lossy()
                .into_owned(),
        );
        spec(
            "cargo",
            &[
                "check".to_owned(),
                "--workspace".to_owned(),
                "--message-format=json".to_owned(),
            ],
            ctx.root,
            ctx.timeout,
            env,
        )
    }
    fn extract(&self, text: &str) -> Vec<StructuredError> {
        jsonscan::cargo_errors(text, self.tool())
    }
}

impl StaticChecker for TscCheck {
    fn tool(&self) -> &'static str {
        "tsc"
    }
    fn claims(&self, rel: &Path) -> bool {
        ext(rel, &["ts", "tsx"])
    }
    fn marker(&self, root: &Path) -> bool {
        root.join("package.json").is_file() || root.join("tsconfig.json").is_file()
    }
    fn probe_spec(&self, root: &Path) -> ExecSpec {
        spec(
            "tsc",
            &["--version".to_owned()],
            root,
            PROBE_TIMEOUT,
            BTreeMap::new(),
        )
    }
    fn check_spec(&self, ctx: &ToolContext) -> ExecSpec {
        spec(
            "tsc",
            &["--noEmit".to_owned()],
            ctx.root,
            ctx.timeout,
            BTreeMap::new(),
        )
    }
    fn extract(&self, text: &str) -> Vec<StructuredError> {
        extract_tsc(text, self.tool())
    }
}

pub(super) fn no_marker_notice() -> SkipReason {
    SkipReason::NoMarker
}

/// `path(line,col): error TS1234: message` lines. Parsed from the END of
/// the line (rfind on the `(line,col)` group immediately before the
/// marker), so paths containing parentheses survive. Non-error lines
/// never match.
fn extract_tsc(text: &str, tool: &'static str) -> Vec<StructuredError> {
    let mut parsed = Vec::new();
    for line in text.lines() {
        // The marker's leading ')' closes the (line,col) group.
        let Some(marker) = line.find("): error TS") else {
            continue;
        };
        let message = line[marker + 2..].trim().to_owned();
        let head = &line[..marker];
        let Some(open) = head.rfind('(') else {
            continue;
        };
        let Some((line_no, column)) = split_position(&head[open + 1..]) else {
            continue;
        };
        parsed.push(StructuredError {
            stage: Stage::Check,
            tool,
            path: Some(PathBuf::from(head[..open].trim_end())),
            line: Some(line_no),
            column: Some(column),
            message,
        });
    }
    super::structured::with_fallback(Stage::Check, tool, parsed, text)
}

fn split_position(position: &str) -> Option<(usize, usize)> {
    let (line, column) = position.split_once(',')?;
    Some((line.trim().parse().ok()?, column.trim().parse().ok()?))
}
