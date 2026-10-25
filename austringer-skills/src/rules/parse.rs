//! The bullet rule grammar parser (see the module docs in `mod.rs`).
//! Exposed for the legacy-rule converter so converted skills preserve the
//! exact same grammar.

use std::path::Path;

use super::{RuleEntry, RuleLayer};

/// Parses the bullet rule grammar from raw text.
pub(crate) fn parse_rule_lines(
    path: &Path,
    text: &str,
    layer: RuleLayer,
    depth: u32,
) -> Vec<RuleEntry> {
    let mut out = Vec::new();
    let mut auto = 0u32;
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(bullet) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        else {
            continue;
        };
        let (id, text) = match bullet.strip_prefix("[[") {
            Some(rest) => match rest.split_once("]]") {
                Some((id, text)) => (id.trim().to_string(), text.trim().to_string()),
                None => {
                    auto += 1;
                    (format!("{auto}"), bullet.trim().to_string())
                }
            },
            None => {
                auto += 1;
                (format!("{auto}"), bullet.trim().to_string())
            }
        };
        let (layer, text) = match text.strip_prefix("TEAM:") {
            Some(rest) => (RuleLayer::Team, rest.trim().to_string()),
            None => (layer, text),
        };
        out.push(RuleEntry {
            id,
            text,
            layer,
            source: path.to_path_buf(),
            depth,
            order: 0,
        });
    }
    out
}
