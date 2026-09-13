//! SKILL.md document parsing: frontmatter extraction, typed mapping to
//! [`SkillMeta`], and trigger derivation from description prose.

use std::path::Path;

use crate::frontmatter::{FmDoc, FmError, FmValue, parse_frontmatter};
use crate::skill::{
    DESCRIPTION_MAX, ModelGuidance, NAME_MAX, ParseErrorKind, SkillMeta, SkillName,
    SkillParseError, SkillWarning, WarningKind,
};

use crate::triggers::derive_triggers;

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

fn build_from_frontmatter(
    path: &Path,
    dir_name: &str,
    fm: FmDoc,
    mut warnings: Vec<SkillWarning>,
) -> Result<(SkillMeta, Vec<SkillWarning>), SkillParseError> {
    let entry = |key: &str| fm.entries.iter().find(|e| e.key == key);
    let scalar = |key: &str| -> Option<String> {
        entry(key).and_then(|e| match &e.value {
            FmValue::Scalar(s) => Some(s.clone()),
            _ => None,
        })
    };

    let name = match scalar("name") {
        Some(raw) => match SkillName::new(raw.trim()) {
            Some(n) => n,
            None if raw.len() > NAME_MAX => {
                return Err(SkillParseError {
                    path: path.to_path_buf(),
                    line: entry("name").map(|e| e.line).unwrap_or(1),
                    kind: ParseErrorKind::FieldTooLong {
                        field: "name",
                        len: raw.len(),
                        max: NAME_MAX,
                    },
                });
            }
            None => {
                return Err(SkillParseError {
                    path: path.to_path_buf(),
                    line: entry("name").map(|e| e.line).unwrap_or(1),
                    kind: ParseErrorKind::InvalidName(raw.clone()),
                });
            }
        },
        None => {
            warnings.push(SkillWarning {
                path: path.to_path_buf(),
                line: 0,
                kind: WarningKind::DerivedName,
            });
            SkillName::sanitize(dir_name)
        }
    };

    let description = match scalar("description") {
        Some(d) if d.len() > DESCRIPTION_MAX => {
            warnings.push(SkillWarning {
                path: path.to_path_buf(),
                line: entry("description").map(|e| e.line).unwrap_or(0),
                kind: WarningKind::DescriptionTruncated {
                    original_len: d.len(),
                },
            });
            d.chars().take(DESCRIPTION_MAX).collect::<String>()
        }
        Some(d) => d,
        None => {
            warnings.push(SkillWarning {
                path: path.to_path_buf(),
                line: 0,
                kind: WarningKind::DerivedDescription,
            });
            String::new()
        }
    };

    let seq = |key: &str| -> Vec<String> {
        entry(key)
            .map(|e| match &e.value {
                FmValue::FlowSeq(v) | FmValue::BlockSeq(v) => v.clone(),
                FmValue::Scalar(s) if s.is_empty() => Vec::new(),
                _ => Vec::new(),
            })
            .unwrap_or_default()
    };

    let mut context_fork = false;
    if let Some(v) = scalar("context_fork") {
        match v.as_str() {
            "true" => context_fork = true,
            "false" => {}
            _ => warnings.push(SkillWarning {
                path: path.to_path_buf(),
                line: 0,
                kind: WarningKind::ContextForkNotBool,
            }),
        }
    }

    let model_guidance = entry("model-guidance")
        .map(|e| match &e.value {
            FmValue::Map(pairs) => pairs
                .iter()
                .filter_map(|(model, v)| match v {
                    FmValue::Scalar(s) => Some(ModelGuidance {
                        model: model.clone(),
                        guidance: s.clone(),
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .unwrap_or_default();

    let known = [
        "name",
        "description",
        "license",
        "metadata",
        "triggers",
        "do-not-trigger-on",
        "languages",
        "version",
        "allowed-tools",
        "model-guidance",
        "context_fork",
    ];
    let mut unknown: Vec<String> = fm
        .entries
        .iter()
        .filter(|e| !known.contains(&e.key.as_str()))
        .map(|e| e.key.clone())
        .collect();
    unknown.sort();
    for key in unknown {
        warnings.push(SkillWarning {
            path: path.to_path_buf(),
            line: 0,
            kind: WarningKind::UnknownKey { key },
        });
    }

    let mut meta = empty_meta(path, name);
    meta.description = description;
    meta.version = scalar("version");
    meta.languages = seq("languages");
    meta.allowed_tools = seq("allowed-tools");
    meta.triggers = seq("triggers");
    meta.negative_triggers = seq("do-not-trigger-on");
    meta.model_guidance = model_guidance;
    meta.context_fork = context_fork;
    derive_triggers(&mut meta);
    Ok((meta, warnings))
}

fn empty_meta(path: &Path, name: SkillName) -> SkillMeta {
    SkillMeta {
        name,
        description: String::new(),
        version: None,
        languages: Vec::new(),
        triggers: Vec::new(),
        negative_triggers: Vec::new(),
        allowed_tools: Vec::new(),
        model_guidance: Vec::new(),
        context_fork: false,
        vendor: crate::loader::Vendor::Harness,
        scope: None,
        skill_dir: path.parent().map(Path::to_path_buf).unwrap_or_default(),
    }
}
