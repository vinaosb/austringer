//! The lexical layer of the setup-steps parser (plan todo 22): line
//! preprocessing (tabs, comments, CRLF), the copyable [`Line`] record,
//! the [`Lines`] cursor, key/value splitting, quote stripping, flow-list
//! splitting, and the two key-expectation helpers. Semantic block parsing
//! lives in [`super::setup_job`].

use super::setup::SetupStepsError;

#[derive(Clone, Copy)]
pub(super) struct Line<'a> {
    pub(super) number: usize,
    pub(super) indent: usize,
    pub(super) content: &'a str,
}

pub(super) struct Lines<'a> {
    pub(super) lines: &'a [Line<'a>],
    pub(super) at: usize,
}

impl<'a> Lines<'a> {
    pub(super) fn new(lines: &'a [Line<'a>]) -> Self {
        Lines { lines, at: 0 }
    }

    pub(super) fn peek(&self) -> Option<Line<'a>> {
        self.lines.get(self.at).copied()
    }

    pub(super) fn next(&mut self) -> Option<Line<'a>> {
        let line = self.lines.get(self.at).copied();
        if line.is_some() {
            self.at += 1;
        }
        line
    }
}

pub(super) fn preprocess(text: &str) -> Result<Vec<Line<'_>>, SetupStepsError> {
    let mut lines = Vec::new();
    for (index, raw) in text.split('\n').enumerate() {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let without_indent = raw.trim_start_matches(' ');
        let indent = raw.len() - without_indent.len();
        if raw[..indent].contains('\t') || without_indent.starts_with('\t') {
            return Err(SetupStepsError::TabIndent { line: index + 1 });
        }
        if without_indent.is_empty() || without_indent.starts_with('#') {
            continue;
        }
        lines.push(Line {
            number: index + 1,
            indent,
            content: without_indent.trim_end(),
        });
    }
    Ok(lines)
}

/// Split `key: value` at the first colon; value may be empty.
pub(super) fn split_key(content: &str) -> (&str, &str) {
    match content.split_once(':') {
        Some((key, value)) => (key.trim(), value.trim()),
        None => (content.trim(), ""),
    }
}

/// Strip ONE matching surrounding quote pair; no escape processing.
pub(super) fn strip_quotes(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let (first, last) = (bytes[0], bytes[bytes.len() - 1]);
        if (first == last) && (first == b'"' || first == b'\'') {
            return value[1..value.len() - 1].to_owned();
        }
    }
    value.to_owned()
}

/// Parse `[ a, "b" ]` flow lists; trailing text after `]` is ignored.
pub(super) fn split_flow_list(
    value: &str,
    line: usize,
    field: &'static str,
) -> Result<Vec<String>, SetupStepsError> {
    let Some(open) = value.find('[') else {
        return Err(SetupStepsError::NotAList { field, line });
    };
    let Some(close) = value[open..].find(']') else {
        return Err(SetupStepsError::NotAList { field, line });
    };
    let inner = &value[open + 1..open + close];
    Ok(inner
        .split(',')
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .map(strip_quotes)
        .collect())
}

pub(super) fn expect_top_key(
    cursor: &mut Lines<'_>,
    key: &'static str,
) -> Result<(), SetupStepsError> {
    match cursor.next() {
        Some(line) if line.indent == 0 && split_key(line.content).0 == key => Ok(()),
        // The other legal top-level key where `on` belongs means the on
        // block is missing entirely.
        Some(line) if key == "on" && line.indent == 0 && split_key(line.content).0 == "jobs" => {
            Err(SetupStepsError::MissingField {
                field: key,
                line: line.number,
            })
        }
        Some(line) if line.indent == 0 => Err(SetupStepsError::UnknownKey {
            key: split_key(line.content).0.to_owned(),
            line: line.number,
        }),
        Some(line) => Err(SetupStepsError::Unexpected {
            line: line.number,
            text: line.content.to_owned(),
        }),
        None => Err(SetupStepsError::MissingField {
            field: key,
            line: 1,
        }),
    }
}

pub(super) fn expect_nested(
    cursor: &mut Lines<'_>,
    indent: usize,
    key: &'static str,
) -> Result<(), SetupStepsError> {
    match cursor.next() {
        Some(line) if line.indent == indent && split_key(line.content).0 == key => Ok(()),
        Some(line) if line.indent == indent => Err(SetupStepsError::UnknownKey {
            key: split_key(line.content).0.to_owned(),
            line: line.number,
        }),
        Some(line) => Err(SetupStepsError::Unexpected {
            line: line.number,
            text: line.content.to_owned(),
        }),
        None => Err(SetupStepsError::MissingField {
            field: key,
            line: 1,
        }),
    }
}
