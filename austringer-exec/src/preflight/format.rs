//! Stage (b): formatter adapters. Every adapter DETECTS its tool by
//! probing; an absent tool is a typed skip with a notice — never a
//! failure. Checks run in check-mode only: pre-flight never rewrites the
//! shadow (corrections belong to the injected fixer seam).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::ToolContext;
use super::error::{SkipReason, Stage};
use super::structured::{StructuredError, with_fallback};
use crate::spec::{ExecSpec, ScreenSize};

pub(super) const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// One formatter ecosystem behind the adapter trait.
pub trait FormatterAdapter: Send + Sync {
    fn tool(&self) -> &'static str;
    /// Does this adapter govern `rel`?
    fn claims(&self, rel: &Path) -> bool;
    /// Cheap availability probe (spawn success = present).
    fn probe_spec(&self, root: &Path) -> ExecSpec;
    /// The check-mode invocation over the claimed files.
    fn check_spec(&self, ctx: &ToolContext) -> ExecSpec;
    /// Structured misformatted-file extraction from the bounded output.
    fn extract(&self, text: &str) -> Vec<StructuredError>;
}

pub struct Rustfmt;
pub struct Prettier;
pub struct Gofmt;
pub struct Black;

pub(super) fn default_formatters() -> Vec<Box<dyn FormatterAdapter>> {
    vec![
        Box::new(Rustfmt),
        Box::new(Prettier),
        Box::new(Gofmt),
        Box::new(Black),
    ]
}

fn ext(rel: &Path, wanted: &[&str]) -> bool {
    rel.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| wanted.contains(&e))
}

fn spec(
    program: &str,
    args: &[String],
    root: &Path,
    timeout: Duration,
    size: ScreenSize,
) -> ExecSpec {
    ExecSpec {
        program: program.to_owned(),
        args: args.to_vec(),
        cwd: Some(root.to_path_buf()),
        env: BTreeMap::new(),
        env_scrub: None,
        timeout,
        size,
        run_in: crate::spec::RunIn::Native,
    }
}

fn err(stage: Stage, tool: &'static str, path: &str, line: Option<usize>) -> StructuredError {
    StructuredError {
        stage,
        tool,
        path: Some(PathBuf::from(path)),
        line,
        column: None,
        message: "file is not formatted".to_owned(),
    }
}

impl FormatterAdapter for Rustfmt {
    fn tool(&self) -> &'static str {
        "rustfmt"
    }
    fn claims(&self, rel: &Path) -> bool {
        ext(rel, &["rs"])
    }
    fn probe_spec(&self, root: &Path) -> ExecSpec {
        // `cargo fmt --version`, not the rustfmt library crate: the lib
        // API is unstable and would add a dependency; the cargo surface
        // is the stable one and rides the same PATH probe discipline.
        spec(
            "cargo",
            &["fmt".to_owned(), "--version".to_owned()],
            root,
            PROBE_TIMEOUT,
            ScreenSize::default(),
        )
    }
    fn check_spec(&self, ctx: &ToolContext) -> ExecSpec {
        spec(
            "cargo",
            &["fmt".to_owned(), "--check".to_owned()],
            ctx.root,
            ctx.timeout,
            ctx.size,
        )
    }
    fn extract(&self, text: &str) -> Vec<StructuredError> {
        let mut parsed = Vec::new();
        for line in text.lines() {
            let Some(rest) = line.trim().strip_prefix("Diff in ") else {
                continue;
            };
            if let Some(at) = rest.find(" at line ") {
                let path = &rest[..at];
                let line_no = rest[at + 9..].trim_end_matches('.').parse::<usize>().ok();
                parsed.push(err(Stage::Format, self.tool(), path, line_no));
            }
        }
        with_fallback(Stage::Format, self.tool(), parsed, text)
    }
}

impl FormatterAdapter for Prettier {
    fn tool(&self) -> &'static str {
        "prettier"
    }
    fn claims(&self, rel: &Path) -> bool {
        ext(
            rel,
            &[
                "ts", "tsx", "js", "jsx", "mjs", "cjs", "json", "md", "css", "scss", "html",
                "yaml", "yml",
            ],
        )
    }
    fn probe_spec(&self, root: &Path) -> ExecSpec {
        spec(
            "prettier",
            &["--version".to_owned()],
            root,
            PROBE_TIMEOUT,
            ScreenSize::default(),
        )
    }
    fn check_spec(&self, ctx: &ToolContext) -> ExecSpec {
        let mut args = vec!["--check".to_owned()];
        args.extend(
            capped_files(ctx)
                .iter()
                .map(|p| p.to_string_lossy().into_owned()),
        );
        spec("prettier", &args, ctx.root, ctx.timeout, ctx.size)
    }
    fn extract(&self, text: &str) -> Vec<StructuredError> {
        let mut parsed = Vec::new();
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("[warn] ") else {
                continue;
            };
            if rest.contains("Code style") || rest.trim().is_empty() {
                continue;
            }
            parsed.push(err(Stage::Format, self.tool(), rest.trim(), None));
        }
        with_fallback(Stage::Format, self.tool(), parsed, text)
    }
}

impl FormatterAdapter for Gofmt {
    fn tool(&self) -> &'static str {
        "gofmt"
    }
    fn claims(&self, rel: &Path) -> bool {
        ext(rel, &["go"])
    }
    fn probe_spec(&self, root: &Path) -> ExecSpec {
        // `gofmt -h` is an invalid flag: it prints usage and exits
        // immediately — the probe only cares that the spawn happened.
        spec(
            "gofmt",
            &["-h".to_owned()],
            root,
            PROBE_TIMEOUT,
            ScreenSize::default(),
        )
    }
    fn check_spec(&self, ctx: &ToolContext) -> ExecSpec {
        let mut args = vec!["-l".to_owned()];
        args.extend(
            capped_files(ctx)
                .iter()
                .map(|p| p.to_string_lossy().into_owned()),
        );
        spec("gofmt", &args, ctx.root, ctx.timeout, ctx.size)
    }
    fn extract(&self, text: &str) -> Vec<StructuredError> {
        let parsed = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| err(Stage::Format, self.tool(), l.trim(), None))
            .collect();
        with_fallback(Stage::Format, self.tool(), parsed, text)
    }
}

impl FormatterAdapter for Black {
    fn tool(&self) -> &'static str {
        "black"
    }
    fn claims(&self, rel: &Path) -> bool {
        ext(rel, &["py", "pyi"])
    }
    fn probe_spec(&self, root: &Path) -> ExecSpec {
        spec(
            "black",
            &["--version".to_owned()],
            root,
            PROBE_TIMEOUT,
            ScreenSize::default(),
        )
    }
    fn check_spec(&self, ctx: &ToolContext) -> ExecSpec {
        let mut args = vec!["--check".to_owned()];
        args.extend(
            capped_files(ctx)
                .iter()
                .map(|p| p.to_string_lossy().into_owned()),
        );
        spec("black", &args, ctx.root, ctx.timeout, ctx.size)
    }
    fn extract(&self, text: &str) -> Vec<StructuredError> {
        let parsed = text
            .lines()
            .filter_map(|l| l.strip_prefix("would reformat "))
            .map(|path| err(Stage::Format, self.tool(), path.trim(), None))
            .collect();
        with_fallback(Stage::Format, self.tool(), parsed, text)
    }
}

pub(super) fn capped_files<'a>(ctx: &'a ToolContext<'_>) -> &'a [PathBuf] {
    let end = ctx.files.len().min(super::MAX_FORMAT_FILES);
    &ctx.files[..end]
}

pub(super) fn file_cap_notice(capped: usize) -> SkipReason {
    SkipReason::FileCap { capped }
}
