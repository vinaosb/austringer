//! Host-function BEHAVIOR: per-call state, virtual-path resolution with
//! canonicalized containment, and the log budget. The typed surface lives
//! in [`crate::hosttypes`]; the wasmtime linker glue in
//! [`crate::hostlink`].

use std::path::PathBuf;

use crate::hosttypes::deny_code;
use crate::manifest::PreopenMode;

pub use crate::hosttypes::{DenyReason, HostEvent, LogLevel, LogLine, PluginGrant, ResolvedScope};

const MAX_LOG_LINES: usize = 64;
const MAX_LOG_LINE_BYTES: usize = 1024;
const MAX_LOG_TOTAL_BYTES: usize = 4096;
const MAX_FILE_BYTES: usize = 4096;

/// Per-call host state: the granted scopes plus everything the plugin did
/// through host functions (logs and audit events).
pub(crate) struct HostCtx {
    scopes: Vec<ResolvedScope>,
    logs: Vec<LogLine>,
    events: Vec<HostEvent>,
    log_bytes: usize,
}

impl HostCtx {
    /// New context for a call running under the given scopes.
    pub fn new(scopes: Vec<ResolvedScope>) -> Self {
        Self {
            scopes,
            logs: Vec::new(),
            events: Vec::new(),
            log_bytes: 0,
        }
    }

    pub(crate) fn deny_read(&mut self, path: &str, reason: DenyReason) -> i32 {
        self.events.push(HostEvent::ReadDenied {
            path: path.into(),
            reason,
        });
        deny_code(reason)
    }

    pub(crate) fn deny_write(&mut self, path: &str, reason: DenyReason) -> i32 {
        self.events.push(HostEvent::WriteDenied {
            path: path.into(),
            reason,
        });
        deny_code(reason)
    }

    pub(crate) fn push_read_ok(&mut self, path: &str, bytes: usize) {
        self.events.push(HostEvent::ReadOk {
            path: path.into(),
            bytes,
        });
    }

    pub(crate) fn push_write_ok(&mut self, path: &str, bytes: usize) {
        self.events.push(HostEvent::WriteOk {
            path: path.into(),
            bytes,
        });
    }

    pub(crate) fn record_log(&mut self, level: i32, text: &str) {
        if self.logs.len() >= MAX_LOG_LINES || self.log_bytes >= MAX_LOG_TOTAL_BYTES {
            if !self
                .events
                .iter()
                .any(|e| matches!(e, HostEvent::LogDropped { .. }))
            {
                self.events.push(HostEvent::LogDropped {
                    reason: "per-call log budget exhausted",
                });
            }
            return;
        }
        let line = if text.len() > MAX_LOG_LINE_BYTES {
            let mut cut = MAX_LOG_LINE_BYTES;
            while cut > 0 && !text.is_char_boundary(cut) {
                cut -= 1;
            }
            format!("{}[...]", &text[..cut])
        } else {
            text.into()
        };
        self.log_bytes += line.len();
        self.logs.push(LogLine {
            level: match level {
                1 => LogLevel::Warn,
                2 => LogLevel::Error,
                _ => LogLevel::Info,
            },
            text: line,
        });
    }

    /// Split a virtual path `<scope>/<relative>` and resolve it strictly
    /// inside the granted scope root.
    pub(crate) fn resolve(
        &self,
        virtual_path: &str,
    ) -> Result<(PathBuf, &ResolvedScope), DenyReason> {
        let (scope_name, rest) = virtual_path
            .split_once('/')
            .ok_or(DenyReason::OutsideScopes)?;
        let scope = self
            .scopes
            .iter()
            .find(|s| s.name == scope_name)
            .ok_or(DenyReason::OutsideScopes)?;
        let root = scope
            .host_dir
            .canonicalize()
            .map_err(|_| DenyReason::NotFound)?;
        let resolved = root
            .join(rest)
            .canonicalize()
            .map_err(|_| DenyReason::NotFound)?;
        if !resolved.starts_with(&root) {
            return Err(DenyReason::Escape);
        }
        Ok((resolved, scope))
    }

    /// Host `read_file`: read `virtual_path` into the guest buffer.
    pub fn read_file(&mut self, virtual_path: &str) -> Result<Vec<u8>, DenyReason> {
        let (resolved, _) = self.resolve(virtual_path)?;
        let content = std::fs::read(&resolved).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => DenyReason::NotFound,
            _ => DenyReason::Io,
        })?;
        if content.len() > MAX_FILE_BYTES {
            return Err(DenyReason::TooLarge);
        }
        Ok(content)
    }

    /// Host `write_file`: write `data` to `virtual_path` in an RW scope.
    ///
    /// Ordering matters for security: the mode denial is decided before any
    /// filesystem probing, and because the TARGET may not exist yet, only
    /// its parent directory is canonicalized (with containment enforced on
    /// the canonical parent, and the file name validated as a single safe
    /// component).
    pub fn write_file(&mut self, virtual_path: &str, data: &[u8]) -> Result<usize, DenyReason> {
        let (scope_name, rest) = virtual_path
            .split_once('/')
            .ok_or(DenyReason::OutsideScopes)?;
        let scope = self
            .scopes
            .iter()
            .find(|s| s.name == scope_name)
            .ok_or(DenyReason::OutsideScopes)?;
        if scope.mode == PreopenMode::ReadOnly {
            return Err(DenyReason::ReadOnlyScope);
        }
        if data.len() > MAX_FILE_BYTES {
            return Err(DenyReason::TooLarge);
        }
        let (parent_rel, file_name) = match rest.rsplit_once('/') {
            Some((parent, name)) => (parent, name),
            None => ("", rest),
        };
        if file_name.is_empty()
            || file_name == "."
            || file_name == ".."
            || file_name.contains('\\')
            || parent_rel.contains('\\')
        {
            return Err(DenyReason::Escape);
        }
        let root = scope
            .host_dir
            .canonicalize()
            .map_err(|_| DenyReason::NotFound)?;
        let parent = if parent_rel.is_empty() {
            root.clone()
        } else {
            root.join(parent_rel)
        };
        let canonical_parent = parent.canonicalize().map_err(|_| DenyReason::NotFound)?;
        if !canonical_parent.starts_with(&root) {
            return Err(DenyReason::Escape);
        }
        std::fs::write(canonical_parent.join(file_name), data).map_err(|_| DenyReason::Io)?;
        Ok(data.len())
    }

    /// Consume the context, yielding the logs and events for the report.
    pub fn into_parts(self) -> (Vec<LogLine>, Vec<HostEvent>) {
        (self.logs, self.events)
    }
}
