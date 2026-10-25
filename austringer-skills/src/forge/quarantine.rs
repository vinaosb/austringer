//! Third-party skill quarantine (ClawHavoc lesson): every
//! marketplace/installed skill passes CONTENT-HASH SCAN → QUARANTINE
//! (inert holding dir the loader never scans) → `forge-lock.json`
//! LEDGER before activation. The [`InjectionScanner`] detects
//! prompt-injection patterns and marked-dangerous shapes with typed
//! findings; nothing quarantined with findings is ever released.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::forge::ForgeError;
use crate::lock::SkillsLockFile;

/// A typed scanner finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanFinding {
    /// A prompt-injection phrase was detected.
    PromptInjection {
        /// Stable pattern id (e.g. `ignore-previous-instructions`).
        pattern: String,
        /// 1-based line.
        line: u32,
    },
    /// A dangerous command shape was detected.
    DangerousShape {
        /// Stable shape id (e.g. `pipe-to-shell`).
        pattern: String,
        /// 1-based line.
        line: u32,
    },
}

impl ScanFinding {
    /// True for prompt-injection (vs command-shape) findings.
    pub fn is_injection(&self) -> bool {
        matches!(self, Self::PromptInjection { .. })
    }

    /// The stable pattern/shape id.
    pub fn pattern(&self) -> &str {
        match self {
            Self::PromptInjection { pattern, .. } | Self::DangerousShape { pattern, .. } => pattern,
        }
    }

    /// The 1-based line the finding came from.
    pub fn line(&self) -> u32 {
        match self {
            Self::PromptInjection { line, .. } | Self::DangerousShape { line, .. } => *line,
        }
    }
}

/// Line-based scanner over skill content. Hand-rolled ASCII-CI phrase
/// matching — no regex dependency (crate policy), deterministic.
#[derive(Debug, Clone, Copy, Default)]
pub struct InjectionScanner;

const INJECTION_PHRASES: [(&str, &str); 7] = [
    (
        "ignore-previous-instructions",
        "ignore previous instructions",
    ),
    ("ignore-previous-instructions", "ignore all previous"),
    ("disregard-guidance", "disregard all"),
    ("disregard-guidance", "disregard your"),
    ("disregard-guidance", "disregard any"),
    ("system-prompt-exfiltration", "your system prompt"),
    ("persona-override", "you are now"),
];

impl InjectionScanner {
    /// Scans arbitrary untrusted text for injection patterns and
    /// dangerous shapes.
    pub fn scan(&self, text: &str) -> Vec<ScanFinding> {
        let mut findings = Vec::new();
        for (idx, line) in text.lines().enumerate() {
            let lowered = line.to_ascii_lowercase();
            for (id, phrase) in INJECTION_PHRASES {
                if lowered.contains(phrase) {
                    findings.push(ScanFinding::PromptInjection {
                        pattern: id.to_string(),
                        line: idx as u32 + 1,
                    });
                }
            }
            if lowered.contains("exfiltrate") {
                findings.push(ScanFinding::PromptInjection {
                    pattern: "exfiltration".to_string(),
                    line: idx as u32 + 1,
                });
            }
            let fetch = lowered.contains("curl") || lowered.contains("wget");
            let pipes = ["| sh", "| bash", "|sh", "|bash"];
            if fetch && pipes.iter().any(|p| lowered.contains(p)) {
                findings.push(ScanFinding::DangerousShape {
                    pattern: "pipe-to-shell".to_string(),
                    line: idx as u32 + 1,
                });
            }
            if lowered.contains("rm -rf /") {
                findings.push(ScanFinding::DangerousShape {
                    pattern: "recursive-delete".to_string(),
                    line: idx as u32 + 1,
                });
            }
        }
        findings
    }
}

/// Ledger state for one received skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerState {
    /// Held in quarantine (never loadable).
    Held,
    /// Released from quarantine (activation path may proceed).
    Released,
}

/// One `forge-lock.json` record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerRecord {
    /// Skill name as received.
    pub name: String,
    /// Number of scanner findings at intake.
    pub findings: usize,
    /// Held/Released state.
    pub state: LedgerState,
}

/// The quarantine ledger: content-hash → record. Sorted-key stable
/// (BTreeMap) per the workspace serialization rule.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ForgeLedger {
    /// Records by blake3 content hash.
    pub entries: BTreeMap<String, LedgerRecord>,
}

/// One intake result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuarantineEntry {
    /// blake3 hex of the content bytes.
    pub content_hash: String,
    /// Skill name as received.
    pub name: String,
    /// Intake scanner findings.
    pub findings: Vec<ScanFinding>,
    /// Intake time (caller clock).
    pub received_unix: u64,
}

/// Quarantine holding area + ledger. The holding dir is NOT one of the
/// loader's vendor skills dirs: held content is inert by construction.
#[derive(Debug, Clone)]
pub struct Quarantine {
    root: PathBuf,
}

impl Quarantine {
    /// A quarantine rooted at `root` (holds + ledger live under it).
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn ledger_path(&self) -> PathBuf {
        self.root.join("forge-lock.json")
    }

    /// Receives untrusted skill content: hash, scan, hold, ledger.
    pub fn receive(
        &self,
        name: &str,
        content: &str,
        now_unix: u64,
    ) -> Result<QuarantineEntry, ForgeError> {
        let entry = QuarantineEntry {
            content_hash: SkillsLockFile::hash_bytes(content.as_bytes()),
            name: name.to_string(),
            findings: InjectionScanner.scan(content),
            received_unix: now_unix,
        };
        let holds = self.root.join("holds");
        std::fs::create_dir_all(&holds).map_err(|e| ForgeError::io("create holds dir", &e))?;
        std::fs::write(self.hold_file(&entry.content_hash), content)
            .map_err(|e| ForgeError::io("write hold file", &e))?;
        let mut ledger = self.ledger()?;
        ledger.entries.insert(
            entry.content_hash.clone(),
            LedgerRecord {
                name: name.to_string(),
                findings: entry.findings.len(),
                state: LedgerState::Held,
            },
        );
        self.save_ledger(&ledger)?;
        Ok(entry)
    }

    /// Path of one held content file.
    pub fn hold_file(&self, content_hash: &str) -> PathBuf {
        self.root.join("holds").join(format!("{content_hash}.md"))
    }

    /// Releases held content ONLY when intake found nothing; released
    /// content is byte-identical to what was received.
    pub fn release(&self, content_hash: &str) -> Result<String, ForgeError> {
        let mut ledger = self.ledger()?;
        let record =
            ledger
                .entries
                .get_mut(content_hash)
                .ok_or_else(|| ForgeError::QuarantineUnknown {
                    hash: content_hash.to_string(),
                })?;
        if record.findings > 0 {
            return Err(ForgeError::QuarantineHeld {
                hash: content_hash.to_string(),
                findings: record.findings,
            });
        }
        let held = std::fs::read_to_string(self.hold_file(content_hash))
            .map_err(|e| ForgeError::io("read hold file", &e))?;
        record.state = LedgerState::Released;
        self.save_ledger(&ledger)?;
        Ok(held)
    }

    /// Reads the ledger from disk (empty when absent).
    pub fn ledger(&self) -> Result<ForgeLedger, ForgeError> {
        match std::fs::read(self.ledger_path()) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| ForgeError::JournalCorrupt {
                detail: format!("forge-lock.json: {e}"),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ForgeLedger::default()),
            Err(e) => Err(ForgeError::io("read forge-lock.json", &e)),
        }
    }

    fn save_ledger(&self, ledger: &ForgeLedger) -> Result<(), ForgeError> {
        std::fs::create_dir_all(&self.root)
            .map_err(|e| ForgeError::io("create quarantine dir", &e))?;
        let bytes = serde_json::to_vec_pretty(ledger).map_err(|e| ForgeError::JournalCorrupt {
            detail: e.to_string(),
        })?;
        let tmp = self.root.join("forge-lock.json.tmp");
        std::fs::write(&tmp, bytes).map_err(|e| ForgeError::io("write ledger tmp", &e))?;
        std::fs::rename(&tmp, self.ledger_path()).map_err(|e| ForgeError::io("commit ledger", &e))
    }
}
