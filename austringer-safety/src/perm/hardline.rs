//! Yolo-surviving hardline floor (hermes pattern, plan todo 27): a
//! deobfuscated dangerous-command blocklist that fires even in
//! bypass/yolo modes — bypass escapes APPROVAL FLOWS, never this
//! floor. Obfuscated payloads (base64, hex, PowerShell
//! `-EncodedCommand`, `$IFS` indirection) are decoded-or-flagged:
//! anything we can decode gets scanned, anything we can only detect
//! gets flagged. Floor hits are Deny + typed `HardlineFloor` reason +
//! audit, unconditionally.

use base64ct::{Base64, Encoding};

use crate::perm::commands::split_compound;

/// Programs whose invocation is unambiguously destructive; matched
/// against RESOLVED program names (and wrapper stems).
pub const BLOCKED_PROGRAMS: &[&str] = &[
    "shutdown", "poweroff", "halt", "reboot", "fdisk", "sfdisk", "gdisk", "parted", "wipefs",
    "diskpart", "format",
];

/// Literal shapes that are dangerous regardless of program
/// resolution (normalized: lowercase, whitespace-collapsed,
/// quote-stripped).
const DANGEROUS_LITERALS: &[&str] = &[
    "rm -rf /",
    "rm -fr /",
    "rm -rf ~",
    "rm -fr ~",
    "dd if=/dev/zero",
    "of=/dev/sd",
    "of=/dev/nvme",
    "of=/dev/vd",
    ":|:&",
    "chmod -r 777 /",
    "chmod 777 /",
    "rd /s /q",
    "del /f /s /q",
    "| sh",
    "| bash",
    "| zsh",
    "| ksh",
    "| powershell",
    "| pwsh",
    "| iex",
    "| invoke-expression",
];

/// A floor hit; `detail` is safe for transcripts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloorHit {
    /// What tripped the floor.
    pub detail: String,
}

/// Whether a RESOLVED program name (or wrapper stem) is blocked.
#[must_use]
pub fn program_blocked(program: &str) -> bool {
    let lower = program.to_ascii_lowercase();
    BLOCKED_PROGRAMS.contains(&lower.as_str()) || lower.starts_with("mkfs")
}

/// Scans a raw command for floor hits (decoded-or-flagged).
#[must_use]
pub fn scan(command: &str) -> Option<FloorHit> {
    let norm = normalize(command);
    if ["${ifs}", "$ifs", "%ifs%"].iter().any(|m| norm.contains(m)) {
        return Some(FloorHit {
            detail: "env-var indirection building command text".into(),
        });
    }
    // Payload decoding MUST use case-preserved text: base64 is
    // case-sensitive, so lowercasing first would corrupt payloads
    // into undecodable noise (the guard against exactly that bug).
    if let Some(payload) = encoded_command_payload(command) {
        let detail = decode_payload(&payload)
            .and_then(|text| literal_match(&normalize(&text)))
            .map(str::to_string)
            .unwrap_or_else(|| "hidden encoded-command payload".into());
        return Some(FloorHit { detail });
    }
    if let Some(detail) = decode_runs(command) {
        return Some(FloorHit { detail });
    }
    if let Some(literal) = literal_match(&norm) {
        return Some(FloorHit {
            detail: literal.to_string(),
        });
    }
    if let Some(program) = blocked_program_token(&norm) {
        return Some(FloorHit {
            detail: format!("blocked program `{program}`"),
        });
    }
    None
}

fn normalize(command: &str) -> String {
    command
        .to_lowercase()
        .replace(['"', '\''], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn literal_match(text: &str) -> Option<&'static str> {
    DANGEROUS_LITERALS
        .iter()
        .find(|lit| text.contains(**lit))
        .copied()
}

fn blocked_program_token(norm: &str) -> Option<String> {
    let segments = split_compound(norm).ok()?;
    segments.iter().find_map(|segment| {
        let mut words = segment.text.split_whitespace();
        let first = words.next()?;
        let program = if matches!(first, "sudo" | "doas") {
            words.next()?
        } else {
            first
        };
        let lower = program.to_ascii_lowercase();
        program_blocked(&lower).then_some(lower)
    })
}

fn encoded_command_payload(command: &str) -> Option<String> {
    let tokens: Vec<&str> = command.split_whitespace().collect();
    for (idx, token) in tokens.iter().enumerate() {
        let lower = token.trim_matches([':', ',']).to_ascii_lowercase();
        // "-e" is deliberately NOT a marker: it is a common flag on
        // unrelated tools (grep -e); only the unambiguous PowerShell
        // forms may gate this.
        if matches!(lower.as_str(), "-encodedcommand" | "-enc") {
            let payload = tokens.get(idx + 1)?;
            let payload = payload.trim_matches(['"', ',']);
            if !payload.is_empty() {
                return Some(payload.to_string());
            }
        }
    }
    None
}

/// Decodes embedded base64/hex runs and scans every decodable payload
/// for dangerous literals ("decoded" half of decoded-or-flagged).
fn decode_runs(norm: &str) -> Option<String> {
    for token in norm.split_whitespace() {
        let token = token.trim_matches([',', ';', '(', ')', '"', '\'']);
        if token.len() >= 16
            && token.len() % 2 == 0
            && token.chars().all(|c| c.is_ascii_hexdigit())
            && let Some(text) = hex_decode(token)
            && let Some(literal) = literal_match(&normalize(&text))
        {
            return Some(format!("hex-decoded payload: {literal}"));
        }
        if token.len() >= 12
            && token.len() % 4 == 0
            && looks_base64(token)
            && let Some(text) = decode_payload(token)
            && let Some(literal) = literal_match(&normalize(&text))
        {
            return Some(format!("base64-decoded payload: {literal}"));
        }
    }
    None
}

fn looks_base64(token: &str) -> bool {
    token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
}

fn decode_payload(payload: &str) -> Option<String> {
    let bytes = Base64::decode_vec(payload).ok()?;
    if bytes.len().is_multiple_of(2) && bytes.iter().skip(1).step_by(2).all(|b| *b == 0) {
        let utf16: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        return char::decode_utf16(utf16)
            .map(|c| c.map_or_else(|_| '?', char::from))
            .collect::<String>()
            .into();
    }
    let printable = bytes.iter().filter(|b| **b >= 0x20 && **b < 0x7f).count();
    if printable * 10 >= bytes.len() * 9 {
        return String::from_utf8(bytes).ok();
    }
    None
}

fn hex_decode(token: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(token.len() / 2);
    let digits = token.as_bytes();
    let mut idx = 0;
    while idx + 1 < digits.len() {
        let hi = (digits[idx] as char).to_digit(16)?;
        let lo = (digits[idx + 1] as char).to_digit(16)?;
        bytes.push((hi * 16 + lo) as u8);
        idx += 2;
    }
    if bytes.iter().any(|b| *b < 0x20 || *b >= 0x7f) {
        return None;
    }
    String::from_utf8(bytes).ok()
}
