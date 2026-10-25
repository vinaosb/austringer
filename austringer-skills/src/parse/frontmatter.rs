//! Frontmatter-driven [`SkillMeta`] construction: maps a parsed
//! [`FmDoc`] into the typed metadata surface, applying agentskills.io
//! validation (name kebab <=64, description <=1024) and emitting typed
//! warnings for derived/unknown/over-cap fields.

use std::path::Path;

use crate::frontmatter::{FmDoc, FmValue};
use crate::skill::{
    DESCRIPTION_MAX, ModelGuidance, NAME_MAX, ParseErrorKind, SkillMeta, SkillName,
    SkillParseError, SkillWarning, WarningKind,
};

use crate::triggers::derive_triggers;

use super::limits::KNOWN_KEYS;

pub(super) fn build_from_frontmatter(
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

    let mut unknown: Vec<String> = fm
        .entries
        .iter()
        .filter(|e| !KNOWN_KEYS.contains(&e.key.as_str()))
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

pub(super) fn empty_meta(path: &Path, name: SkillName) -> SkillMeta {
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
        vendor: crate::loader::Vendor::Austringer,
        scope: None,
        skill_dir: path.parent().map(Path::to_path_buf).unwrap_or_default(),
    }
}
