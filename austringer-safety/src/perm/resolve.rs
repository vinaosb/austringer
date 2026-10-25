//! Program resolution (Droid pattern, plan todo 27): a hard blocklist
//! must match the command's ACTUAL program. Resolution runs entirely
//! in-process (bounded PATH scan + a single ≤4 KB wrapper read — no
//! subprocess, so it cannot hang), peels wrapper scripts exactly one
//! layer, collapses absolute paths to their stem, and treats quoting
//! tricks as suspicious. Failures fail CLOSED: `NotFound` and
//! `Suspicious` are denials, never passes.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::perm::commands::unquote;

const PEEK_BYTES: usize = 4096;

const SCRIPT_EXTS: &[&str] = &["cmd", "bat", "ps1", "sh", "bash"];
const EXE_EXTS: &[&str] = &["exe", "com", "msi", "jar", "vbs"];
const INTERPRETERS: &[&str] = &[
    "node",
    "python",
    "python3",
    "py",
    "ruby",
    "perl",
    "php",
    "powershell",
    "pwsh",
    "cmd",
    "sh",
    "bash",
    "java",
    "dotnet",
    "deno",
    "bun",
];
const CMD_SKIP_WORDS: &[&str] = &[
    "rem", "echo", "set", "if", "goto", "exit", "title", "call", "cd", "chcp", "cls", "pause",
    "shift", "setlocal", "endlocal", "for", "goto",
];

/// Outcome of resolving a program word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The actual program: `program` is the peeled/underlying program
    /// name; `wrapper` names the script it hides behind, if any. The
    /// blocklist matches BOTH.
    Resolved {
        /// The real program (or the script's own stem when opaque).
        program: String,
        /// The wrapper script's stem when resolution peeled a layer.
        wrapper: Option<String>,
    },
    /// Nothing on the PATH matched — fail closed.
    NotFound,
    /// The word's shape hides the real token — fail closed.
    Suspicious {
        /// What was wrong with the shape.
        detail: &'static str,
    },
}

/// Resolves program words to actual programs.
pub trait ProgramResolver {
    /// Resolve one program word.
    fn resolve(&self, program_word: &str) -> Resolution;
}

/// In-process PATH resolver (which/where semantics without spawning).
#[derive(Debug, Clone)]
pub struct PathResolver {
    dirs: Vec<PathBuf>,
    extensions: Vec<String>,
}

impl PathResolver {
    /// Builds a resolver over explicit PATH dirs with the platform's
    /// executable extensions.
    #[must_use]
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        let extensions: Vec<String> = if cfg!(windows) {
            [".exe", ".cmd", ".bat", ".com"]
                .iter()
                .map(ToString::to_string)
                .collect()
        } else {
            vec![String::new()]
        };
        Self { dirs, extensions }
    }

    /// Builds a resolver from the process `PATH`.
    #[must_use]
    pub fn from_path_env() -> Self {
        let dirs = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        Self::new(dirs)
    }

    fn locate(&self, word: &str) -> Option<PathBuf> {
        let mut candidates: Vec<String> = self.extensions.clone();
        if !candidates.iter().any(|c| c.is_empty()) {
            candidates.push(String::new());
        }
        for ext in candidates {
            for dir in &self.dirs {
                let candidate = dir.join(format!("{word}{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    fn resolve_file(&self, path: &Path) -> Resolution {
        let stem = stem_of(path);
        if !is_script(path) {
            return Resolution::Resolved {
                program: stem,
                wrapper: None,
            };
        }
        match peel_one_layer(path) {
            Some(inner) => Resolution::Resolved {
                program: inner,
                wrapper: Some(stem),
            },
            None => Resolution::Resolved {
                program: stem.clone(),
                wrapper: Some(stem),
            },
        }
    }
}

impl ProgramResolver for PathResolver {
    fn resolve(&self, program_word: &str) -> Resolution {
        let word = unquote(program_word.trim());
        if word.is_empty() {
            return Resolution::Suspicious {
                detail: "empty program word",
            };
        }
        if word.contains(['"', '\'']) {
            return Resolution::Suspicious {
                detail: "embedded quote hides the real token",
            };
        }
        if word.chars().any(char::is_whitespace) {
            return Resolution::Suspicious {
                detail: "program word contains whitespace",
            };
        }
        if word.contains(['/', '\\']) {
            let trimmed = word.trim_start_matches(r"\\?\");
            let stem = stem_of(Path::new(&trimmed.replace('/', "\\")));
            return Resolution::Resolved {
                program: stem,
                wrapper: None,
            };
        }
        match self.locate(&word) {
            Some(path) => self.resolve_file(&path),
            None => Resolution::NotFound,
        }
    }
}

fn stem_of(path: &Path) -> String {
    path.file_stem().map_or_else(
        || path.to_string_lossy().into_owned(),
        |s| s.to_string_lossy().into_owned(),
    )
}

fn is_script(path: &Path) -> bool {
    let is_script_ext = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SCRIPT_EXTS.contains(&e.to_ascii_lowercase().as_str()));
    if is_script_ext {
        return true;
    }
    head(path).is_some_and(|head| head.starts_with("#!"))
}

fn head(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut buf = vec![0_u8; PEEK_BYTES];
    let read = file.read(&mut buf).unwrap_or(0);
    buf.truncate(read);
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// Peels exactly one layer: find the program the script invokes.
fn peel_one_layer(path: &Path) -> Option<String> {
    let text = head(path)?;
    let shell_script = text.starts_with("#!");
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        let first = unquote(words.next()?);
        if shell_script {
            let program = if first == "exec" || first == "sudo" {
                words.next().map(unquote)?
            } else {
                first
            };
            return Some(program);
        }
        let bare = first.trim_start_matches('@');
        if bare.is_empty()
            || bare.starts_with('%')
            || bare.starts_with(':')
            || bare.starts_with('-')
        {
            continue;
        }
        let name = bare.split([':', '\\', '/']).next_back().unwrap_or(bare);
        let lower = name.to_ascii_lowercase();
        let stem = lower
            .split('.')
            .next()
            .unwrap_or(lower.as_str())
            .to_string();
        let looks_executable = name
            .rsplit('.')
            .next()
            .is_some_and(|ext| EXE_EXTS.contains(&ext) || SCRIPT_EXTS.contains(&ext))
            || INTERPRETERS.contains(&stem.as_str());
        if CMD_SKIP_WORDS.contains(&stem.as_str()) || !looks_executable {
            continue;
        }
        return Some(stem);
    }
    None
}
