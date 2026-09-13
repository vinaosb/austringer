//! Skill document rendering: the forge's SKILL.md emitter plus the body
//! section splitter used by the delta merger. Everything emitted here is
//! shaped to survive the REAL T30 parser
//! ([`crate::skill::parse_skill_document`]) — round-tripping is a test
//! guarantee, not an aspiration.

use crate::skill::{DESCRIPTION_MAX, SkillName};

/// Frontmatter specification the forge can emit. Deliberately narrower
/// than [`crate::skill::SkillMeta`]: forge-authored skills carry the
/// routing-relevant fields only.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillSpec {
    /// Validated skill name.
    pub name: SkillName,
    /// One-line description (newlines sanitized at render).
    pub description: String,
    /// Semantic version string (`major.minor.patch`).
    pub version: String,
    /// Positive trigger phrases.
    pub triggers: Vec<String>,
    /// Negative-scoping phrases.
    pub negative_triggers: Vec<String>,
    /// Declared allowed tools (may carry `*` wildcards).
    pub allowed_tools: Vec<String>,
}

/// One `## heading`-delimited body section. An empty heading represents
/// preamble text before the first heading (never delta-targetable).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillSection {
    /// Section heading text (without the `## ` marker).
    pub heading: String,
    /// Section body (trailing whitespace normalized at parse).
    pub body: String,
}

/// Renders a full SKILL.md document (frontmatter + sections) that the
/// real T30 loader parses without structural warnings.
pub fn render_document(spec: &SkillSpec, sections: &[SkillSection]) -> String {
    let mut description: String = spec
        .description
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    description = description.trim().chars().take(DESCRIPTION_MAX).collect();
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", spec.name.as_str()));
    out.push_str(&format!("description: {description}\n"));
    out.push_str(&format!("version: \"{}\"\n", spec.version));
    emit_seq(&mut out, "triggers", &spec.triggers);
    emit_seq(&mut out, "do-not-trigger-on", &spec.negative_triggers);
    emit_seq(&mut out, "allowed-tools", &spec.allowed_tools);
    out.push_str("---\n\n");
    for section in sections {
        if !section.heading.is_empty() {
            out.push_str(&format!("## {}\n\n", section.heading));
        }
        out.push_str(&section.body);
        out.push_str("\n\n");
    }
    out
}

fn emit_seq(out: &mut String, key: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    let quoted: Vec<String> = items.iter().map(|i| format!("\"{}\"", escape(i))).collect();
    out.push_str(&format!("{key}: [{}]\n", quoted.join(", ")));
}

fn escape(item: &str) -> String {
    item.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Returns the body after the frontmatter fence (whole input when there
/// is no frontmatter). Mirrors the loader's extraction so parse/render
/// round-trip through one body definition.
pub fn extract_body(document: &str) -> &str {
    let trimmed = document.trim_start_matches('\u{FEFF}');
    let mut lines = trimmed
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l));
    if lines.next().is_none_or(|l| l.trim_end() != "---") {
        return trimmed;
    }
    let mut consumed = trimmed.find('\n').map_or(trimmed.len(), |i| i + 1);
    for line in lines {
        let len = line.len();
        if line.trim_end() == "---" {
            return trimmed[(consumed + len + 1).min(trimmed.len())..].trim_start_matches('\n');
        }
        consumed += len + 1;
    }
    trimmed
}

/// Splits a body into `## `-heading sections; text before the first
/// heading becomes the empty-heading preamble section (omitted entirely
/// when absent).
pub fn parse_body_sections(body: &str) -> Vec<SkillSection> {
    let mut sections: Vec<SkillSection> = Vec::new();
    let mut preamble: Vec<&str> = Vec::new();
    for line in body.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            sections.push(SkillSection {
                heading: heading.trim().to_string(),
                body: String::new(),
            });
        } else if let Some(last) = sections.last_mut() {
            last.body.push_str(line);
            last.body.push('\n');
        } else {
            preamble.push(line);
        }
    }
    let preamble_text = preamble.join("\n").trim().to_string();
    if !preamble_text.is_empty() {
        sections.insert(
            0,
            SkillSection {
                heading: String::new(),
                body: preamble_text,
            },
        );
    }
    for section in &mut sections {
        section.body = section.body.trim().to_string();
    }
    sections
}
