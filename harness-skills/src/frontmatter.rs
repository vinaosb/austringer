//! Minimal YAML-subset frontmatter scanner.
//!
//! agentskills.io frontmatter is a constrained YAML shape (flat mapping of
//! scalars, flow/block sequences, and one nesting level under `metadata`/
//! `model-guidance`). serde_yaml is archived and a full YAML parser would
//! be a new dependency for shapes this loader never sees, so the subset is
//! scanned by hand: strict on structure (errors carry line numbers),
//! tolerant of unknown keys (the caller emits typed warnings for them).

/// A parsed frontmatter value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FmValue {
    /// Scalar (quotes stripped, folded blocks joined).
    Scalar(String),
    /// Flow sequence `[a, "b"]`.
    FlowSeq(Vec<String>),
    /// Block sequence (`- item` lines).
    BlockSeq(Vec<String>),
    /// One-level nested mapping (`metadata:` / `model-guidance:`).
    Map(Vec<(String, FmValue)>),
}

/// One top-level frontmatter entry with its 1-based file line.
#[derive(Debug, Clone)]
pub(crate) struct FmEntry {
    pub key: String,
    pub line: u32,
    pub value: FmValue,
}

/// Parsed frontmatter document (top-level entries in file order).
#[derive(Debug, Clone, Default)]
pub(crate) struct FmDoc {
    pub entries: Vec<FmEntry>,
}

/// Structure errors the subset scanner can raise. Line numbers are
/// 1-based positions in the full file.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub(crate) enum FmError {
    /// A line inside the frontmatter is not `key: value` shaped.
    #[error("expected a `key: value` mapping line, found {0:?}")]
    NotAMapping(String),
    /// A top-level key appears twice.
    #[error("duplicate key {0:?}")]
    DuplicateKey(String),
    /// A flow sequence is malformed (unbalanced brackets/quotes).
    #[error("malformed flow sequence: {0}")]
    BadFlowSeq(String),
}

// '.' allowed for nested model ids like `gpt-5.5` under model-guidance.
const KEY_CHARS: fn(char) -> bool =
    |c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.';

fn split_key(line: &str) -> Option<(&str, &str)> {
    let (key, rest) = line.split_once(':')?;
    let valid = !key.is_empty() && key.chars().all(KEY_CHARS) && !key.starts_with('-');
    if valid { Some((key, rest)) } else { None }
}

fn strip_quotes(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2
        && ((t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\'')))
    {
        t[1..t.len() - 1].replace("\\\"", "\"")
    } else {
        t.to_string()
    }
}

fn parse_flow_seq(inner: &str, line: u32) -> Result<Vec<String>, (u32, FmError)> {
    let body = inner.trim();
    let body = body
        .strip_prefix('[')
        .and_then(|b| b.strip_suffix(']'))
        .ok_or_else(|| {
            (
                line,
                FmError::BadFlowSeq(format!("unbalanced brackets in {body:?}")),
            )
        })?;
    let mut items = Vec::new();
    let mut current = String::new();
    let mut in_quotes: Option<char> = None;
    for ch in body.chars() {
        match in_quotes {
            Some(q) => {
                current.push(ch);
                if ch == q {
                    in_quotes = None;
                }
            }
            None => match ch {
                '"' | '\'' => {
                    in_quotes = Some(ch);
                    current.push(ch);
                }
                ',' => {
                    items.push(strip_quotes(&current));
                    current.clear();
                }
                _ => current.push(ch),
            },
        }
    }
    if in_quotes.is_some() {
        return Err((line, FmError::BadFlowSeq("unterminated quote".into())));
    }
    if !current.trim().is_empty() || !items.is_empty() {
        items.push(strip_quotes(&current));
    }
    Ok(items.into_iter().filter(|i| !i.is_empty()).collect())
}

/// Parses frontmatter `lines` (between the `---` fences), where `lines[0]`
/// is file line `first_line`.
pub(crate) fn parse_frontmatter(lines: &[&str], first_line: u32) -> Result<FmDoc, (u32, FmError)> {
    let mut doc = FmDoc::default();
    let mut i = 0usize;
    while i < lines.len() {
        let raw = lines[i];
        let line_no = first_line + i as u32;
        let trimmed = raw.trim_end();
        if trimmed.trim().is_empty() || trimmed.trim_start().starts_with('#') {
            i += 1;
            continue;
        }
        if trimmed.starts_with(' ') || trimmed.starts_with('\t') {
            return Err((line_no, FmError::NotAMapping(trimmed.to_string())));
        }
        let (key, rest) = split_key(trimmed)
            .ok_or_else(|| (line_no, FmError::NotAMapping(trimmed.to_string())))?;
        if doc.entries.iter().any(|e| e.key == key) {
            return Err((line_no, FmError::DuplicateKey(key.to_string())));
        }
        let inline = rest.trim();
        let value = parse_inline_or_block(lines, &mut i, inline, line_no, first_line)?;
        doc.entries.push(FmEntry {
            key: key.to_string(),
            line: line_no,
            value,
        });
        i += 1;
    }
    Ok(doc)
}

fn parse_inline_or_block(
    lines: &[&str],
    i: &mut usize,
    inline: &str,
    line_no: u32,
    first_line: u32,
) -> Result<FmValue, (u32, FmError)> {
    match inline {
        "" => {
            if let Some(v) = collect_nested_map(lines, i, first_line)? {
                Ok(v)
            } else if let Some(v) = collect_block_seq(lines, i) {
                Ok(v)
            } else {
                Ok(FmValue::Scalar(String::new()))
            }
        }
        ">" | ">-" | ">+" => Ok(FmValue::Scalar(fold_block(lines, i, " "))),
        "|" | "|-" | "|+" => Ok(FmValue::Scalar(fold_block(lines, i, "\n"))),
        _ if inline.starts_with('[') => Ok(FmValue::FlowSeq(parse_flow_seq(inline, line_no)?)),
        _ => Ok(FmValue::Scalar(strip_quotes(inline))),
    }
}

fn collect_nested_map(
    lines: &[&str],
    i: &mut usize,
    first_line: u32,
) -> Result<Option<FmValue>, (u32, FmError)> {
    let mut sub: Vec<(String, FmValue)> = Vec::new();
    let mut j = *i + 1;
    while j < lines.len() {
        let candidate = lines[j];
        if candidate.trim().is_empty() || candidate.trim_start().starts_with('#') {
            j += 1;
            continue;
        }
        if !candidate.starts_with(' ') {
            break;
        }
        let dedented = candidate.trim_start();
        if dedented.starts_with("- ") {
            break;
        }
        let line_no = first_line + j as u32;
        let (key, rest) = split_key(dedented)
            .ok_or_else(|| (line_no, FmError::NotAMapping(candidate.trim().to_string())))?;
        let inline = rest.trim();
        let value = if inline.starts_with('[') {
            FmValue::FlowSeq(parse_flow_seq(inline, line_no)?)
        } else {
            FmValue::Scalar(strip_quotes(inline))
        };
        sub.push((key.to_string(), value));
        j += 1;
    }
    if sub.is_empty() {
        Ok(None)
    } else {
        *i = j - 1;
        Ok(Some(FmValue::Map(sub)))
    }
}

fn collect_block_seq(lines: &[&str], i: &mut usize) -> Option<FmValue> {
    let mut items = Vec::new();
    let mut j = *i + 1;
    while j < lines.len() {
        let candidate = lines[j];
        if candidate.trim().is_empty() {
            j += 1;
            continue;
        }
        let dedented = candidate.trim_start();
        if let Some(item) = dedented.strip_prefix("- ") {
            items.push(strip_quotes(item.trim_end()));
            j += 1;
        } else {
            break;
        }
    }
    if items.is_empty() {
        None
    } else {
        *i = j - 1;
        Some(FmValue::BlockSeq(items))
    }
}

fn fold_block(lines: &[&str], i: &mut usize, joiner: &str) -> String {
    let mut collected: Vec<String> = Vec::new();
    let mut j = *i + 1;
    while j < lines.len() {
        let candidate = lines[j];
        if candidate.trim().is_empty() {
            collected.push(String::new());
            j += 1;
            continue;
        }
        if !candidate.starts_with(' ') && !candidate.starts_with('\t') {
            break;
        }
        collected.push(candidate.trim().to_string());
        j += 1;
    }
    while collected.last().is_some_and(String::is_empty) {
        collected.pop();
    }
    *i = j - 1;
    collected.join(joiner)
}
