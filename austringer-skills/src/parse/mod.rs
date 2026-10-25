//! SKILL.md document parsing: frontmatter extraction, typed mapping to
//! [`SkillMeta`], and trigger derivation from description prose.

mod frontmatter;
mod limits;

use std::path::Path;

use crate::frontmatter::{FmDoc, FmError, parse_frontmatter};
use crate::skill::{
    ParseErrorKind, SkillMeta, SkillName, SkillParseError, SkillWarning, WarningKind,
};

use crate::triggers::derive_triggers;

use frontmatter::{build_from_frontmatter, empty_meta};

/// Parses a SKILL.md document (raw bytes) into a metadata surface plus
/// typed warnings. `dir_name` is the containing directory's name, used to
/// derive the skill name when the frontmatter omits it (opencode compat).
pub fn parse_skill_document(
    path: &Path,
    dir_name: &str,
    bytes: &[u8],
) -> Result<(SkillMeta, Vec<SkillWarning>), SkillParseError> {
    let invalid_utf8 = std::str::from_utf8(bytes).is_err();
    let text = String::from_utf8_lossy(bytes)
        .trim_start_matches('\u{FEFF}')
        .to_string();
    let mut lines: Vec<&str> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }

    let mut warnings = Vec::new();
    if invalid_utf8 {
        let replaced = text.matches('\u{FFFD}').count();
        if replaced > 0 {
            warnings.push(SkillWarning {
                path: path.to_path_buf(),
                line: 0,
                kind: WarningKind::InvalidUtf8 { replaced },
            });
        }
    }

    let doc: Option<FmDoc> = if first_fence(&lines) {
        match find_close(&lines) {
            Some(end) => Some(
                parse_frontmatter(&lines[1..end], 2)
                    .map_err(|(line, kind)| to_parse_error(path, line, kind))?,
            ),
            None => {
                return Err(SkillParseError {
                    path: path.to_path_buf(),
                    line: lines.len() as u32 + 1,
                    kind: ParseErrorKind::UnterminatedFrontmatter,
                });
            }
        }
    } else {
        None
    };

    match doc {
        Some(fm) => build_from_frontmatter(path, dir_name, fm, warnings),
        None => build_bare_markdown(path, dir_name, &lines, warnings),
    }
}

fn first_fence(lines: &[&str]) -> bool {
    lines.first().is_some_and(|l| l.trim_end() == "---")
}

fn find_close(lines: &[&str]) -> Option<usize> {
    lines[1..]
        .iter()
        .position(|l| l.trim_end() == "---")
        .map(|i| i + 1)
}

fn to_parse_error(path: &Path, line: u32, kind: FmError) -> SkillParseError {
    let mapped = match kind {
        FmError::NotAMapping(detail) => ParseErrorKind::NotAMapping(detail),
        FmError::DuplicateKey(key) => ParseErrorKind::DuplicateKey(key),
        FmError::BadFlowSeq(detail) => ParseErrorKind::BadFlowSeq(detail),
    };
    SkillParseError {
        path: path.to_path_buf(),
        line,
        kind: mapped,
    }
}

fn build_bare_markdown(
    path: &Path,
    dir_name: &str,
    lines: &[&str],
    mut warnings: Vec<SkillWarning>,
) -> Result<(SkillMeta, Vec<SkillWarning>), SkillParseError> {
    warnings.push(SkillWarning {
        path: path.to_path_buf(),
        line: 1,
        kind: WarningKind::MissingFrontmatter,
    });
    let heading = lines
        .iter()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches('#').trim().to_string())
        .unwrap_or_default();
    let name = SkillName::sanitize(dir_name);
    let mut meta = empty_meta(path, name);
    meta.description = heading;
    derive_triggers(&mut meta);
    Ok((meta, warnings))
}
