//! Correction journal (Kilo kilo-memory pattern): captures the signals
//! that teach — hunk rejections with feedback, permission denials,
//! post-output edits — with SECRET REDACTION AT CAPTURE TIME. Secrets
//! never reach journal storage: every string field passes the
//! [`RedactionFilter`] before the entry is stored.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::forge::ForgeError;

/// A correction signal captured from a live session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CorrectionSignal {
    /// The user rejected a proposed edit hunk.
    HunkRejected {
        /// File or target the hunk applied to.
        target: String,
        /// Why it was rejected (user feedback).
        feedback: String,
    },
    /// A tool call was denied (permission system).
    PermissionDenied {
        /// The denied tool invocation.
        tool: String,
        /// Denial reason.
        reason: String,
    },
    /// The user edited agent output after it landed.
    PostOutputEdit {
        /// File or target edited.
        target: String,
        /// Summary of the corrective edit.
        edit_summary: String,
    },
}

impl CorrectionSignal {
    /// Mutable access to every string field, in declaration order — the
    /// single seam the capture-time redactor walks.
    fn text_fields(&mut self) -> [&mut String; 2] {
        match self {
            Self::HunkRejected { target, feedback }
            | Self::PostOutputEdit {
                target,
                edit_summary: feedback,
            } => [target, feedback],
            Self::PermissionDenied { tool, reason } => [tool, reason],
        }
    }
}

/// The result of redacting one string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionOutcome {
    /// Redacted text.
    pub text: String,
    /// How many replacements were made.
    pub count: usize,
}

/// Secret-token redaction: fixed token shapes (`sk-`, `ghp_`, AWS
/// `AKIA…`, `Bearer …`, CI assignment keys) plus caller-supplied custom
/// literals. Hand-rolled scanning — no regex dependency by crate policy.
#[derive(Debug, Clone, PartialEq)]
pub struct RedactionFilter {
    custom: Vec<String>,
}

const TOKEN_SHAPES: [(&str, &str); 6] = [
    ("sk-key", "sk-"),
    ("anthropic-key", "sk-ant-"),
    ("github-token", "ghp_"),
    ("github-token", "github_pat_"),
    ("slack-token", "xoxb-"),
    ("openrouter-key", "sk-or-"),
];

const ASSIGNMENT_KEYS: [&str; 5] = ["api_key=", "apikey=", "token=", "password=", "secret="];

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'
}

fn redact_after_marker(out: &mut String, marker: &str, replacement: &str) -> usize {
    let mut count = 0usize;
    let mut from = 0usize;
    while let Some(rel) = out.to_ascii_lowercase()[from..].find(marker) {
        let start = from + rel + marker.len();
        let value_end: usize = out[start..]
            .chars()
            .take_while(|c| is_token_char(*c))
            .map(char::len_utf8)
            .sum();
        if value_end == 0 {
            from = start;
            continue;
        }
        out.replace_range(start..start + value_end, replacement);
        count += 1;
        from = start + replacement.len();
    }
    count
}

impl RedactionFilter {
    /// A filter with the built-in token shapes plus custom literals.
    pub fn new(custom: &[&str]) -> Self {
        Self {
            custom: custom.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// Redacts every recognized secret in `text`. Case-insensitive
    /// markers compare ASCII-lowercase (byte-length-preserving, so
    /// replacement indices can never desync on multi-byte text).
    pub fn redact(&self, text: &str) -> RedactionOutcome {
        let mut count = 0usize;
        let mut out = text.to_string();
        for (kind, prefix) in TOKEN_SHAPES {
            let marker = format!("[REDACTED:{kind}]");
            let mut from = 0usize;
            while let Some(rel) = out[from..].find(prefix) {
                let start = from + rel;
                let token_end: usize = out[start + prefix.len()..]
                    .chars()
                    .take_while(|c| is_token_char(*c))
                    .map(char::len_utf8)
                    .sum();
                if token_end >= 8 {
                    out.replace_range(start..start + prefix.len() + token_end, &marker);
                    count += 1;
                    from = start + marker.len();
                } else {
                    // Too short to be a real token; skip past this prefix.
                    from = start + prefix.len();
                }
            }
        }
        count += redact_after_marker(&mut out, "bearer ", "[REDACTED:bearer]");
        for key in ASSIGNMENT_KEYS {
            count += redact_after_marker(&mut out, key, "[REDACTED:credential]");
        }
        // AWS access-key id: AKIA + exactly 16 [A-Z0-9].
        let mut from = 0usize;
        while let Some(rel) = out[from..].find("AKIA") {
            let start = from + rel;
            let tail_len: usize = out[start + 4..]
                .chars()
                .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                .map(char::len_utf8)
                .sum();
            if tail_len == 16 {
                out.replace_range(start..start + 4 + 16, "[REDACTED:aws-key]");
                count += 1;
                from = start + "[REDACTED:aws-key]".len();
            } else {
                from = start + 4;
            }
        }
        for literal in &self.custom {
            while let Some(rel) = out.find(literal.as_str()) {
                out.replace_range(rel..rel + literal.len(), "[REDACTED:custom]");
                count += 1;
            }
        }
        RedactionOutcome { text: out, count }
    }
}

/// One captured (and already-redacted) correction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// The redacted signal.
    pub signal: CorrectionSignal,
    /// Capture time (seconds since epoch, caller-supplied clock).
    pub captured_unix: u64,
    /// Number of redactions applied at capture.
    pub redactions: usize,
}

/// The journal: capture-time redaction, ordered entries, render for the
/// distiller prompt, JSON persistence.
#[derive(Debug, Clone, PartialEq)]
pub struct CorrectionJournal {
    entries: Vec<JournalEntry>,
    filter: RedactionFilter,
}

impl CorrectionJournal {
    /// An empty journal with the given redaction filter.
    pub fn new(filter: RedactionFilter) -> Self {
        Self {
            entries: Vec::new(),
            filter,
        }
    }

    /// Captures a signal, redacting every text field BEFORE storage.
    pub fn capture(&mut self, mut signal: CorrectionSignal, now_unix: u64) -> JournalEntry {
        let mut redactions = 0usize;
        for field in signal.text_fields() {
            let outcome = self.filter.redact(field);
            redactions += outcome.count;
            *field = outcome.text;
        }
        let entry = JournalEntry {
            signal,
            captured_unix: now_unix,
            redactions,
        };
        self.entries.push(entry.clone());
        entry
    }

    /// All captured entries (already redacted).
    pub fn entries(&self) -> &[JournalEntry] {
        &self.entries
    }

    /// A deterministic rendering of the journal for the distiller prompt.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for entry in &self.entries {
            match &entry.signal {
                CorrectionSignal::HunkRejected { target, feedback } => {
                    out.push_str(&format!("hunk rejected in {target}: {feedback}\n"));
                }
                CorrectionSignal::PermissionDenied { tool, reason } => {
                    out.push_str(&format!("permission denied for {tool}: {reason}\n"));
                }
                CorrectionSignal::PostOutputEdit {
                    target,
                    edit_summary,
                } => {
                    out.push_str(&format!("post-output edit of {target}: {edit_summary}\n"));
                }
            }
        }
        out
    }

    /// Persists the journal as pretty JSON (atomic tmp+rename).
    pub fn save(&self, path: &Path) -> Result<(), ForgeError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ForgeError::io("create journal dir", &e))?;
        }
        let bytes =
            serde_json::to_vec_pretty(&self.entries).map_err(|e| ForgeError::JournalCorrupt {
                detail: e.to_string(),
            })?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes).map_err(|e| ForgeError::io("write journal tmp", &e))?;
        std::fs::rename(&tmp, path).map_err(|e| ForgeError::io("commit journal", &e))
    }

    /// Loads a journal; a missing file is an empty journal (first run),
    /// a corrupt file is a typed error, never a panic.
    pub fn load(path: &Path, filter: RedactionFilter) -> Result<Self, ForgeError> {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    entries: Vec::new(),
                    filter,
                });
            }
            Err(e) => return Err(ForgeError::io("read journal", &e)),
        };
        let entries: Vec<JournalEntry> =
            serde_json::from_slice(&bytes).map_err(|e| ForgeError::JournalCorrupt {
                detail: e.to_string(),
            })?;
        Ok(Self { entries, filter })
    }
}
