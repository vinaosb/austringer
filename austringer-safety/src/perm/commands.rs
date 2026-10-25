//! Command layer (plan todo 27): compound-command segmentation and the
//! command-prefix allow/deny rules with longest-prefix precedence and
//! the Roo dangerous-substitution guard. `npm i` being allowed must
//! NOT auto-approve `npm i && curl evil`: EVERY segment (including
//! pipe stages, subshell bodies, and backtick payloads) must be
//! independently allowed, or the whole command cannot auto-approve.

/// One independently-judgeable piece of a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSegment {
    /// The segment's text (trimmed).
    pub text: String,
    /// The leading token with matched quote pairs stripped.
    pub program_word: String,
}

/// Typed command-shape failure (fail closed).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandShapeError {
    /// The command could not be segmented.
    #[error("malformed command: {detail}")]
    Malformed {
        /// What was wrong with the shape.
        detail: String,
    },
}

const MAX_SUBSHELL_DEPTH: usize = 2;

/// Splits a compound command into independently-judgeable segments:
/// `&&`, `||`, `;`, `|`, newlines outside quotes separate stages;
/// `$(...)` and backtick bodies are appended as their own segments
/// (nested one more level); quoting guards every separator.
///
/// # Errors
/// [`CommandShapeError::Malformed`] for empty input, unbalanced
/// quotes, or unbalanced `$(` groups.
pub fn split_compound(command: &str) -> Result<Vec<CommandSegment>, CommandShapeError> {
    let mut segments = Vec::new();
    split_inner(command, 0, &mut segments)?;
    if segments.is_empty() {
        return Err(CommandShapeError::Malformed {
            detail: "command is empty".into(),
        });
    }
    Ok(segments)
}

fn split_inner(
    text: &str,
    depth: usize,
    out: &mut Vec<CommandSegment>,
) -> Result<(), CommandShapeError> {
    let mut pieces: Vec<String> = Vec::new();
    let mut extracted: Vec<CommandSegment> = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        let substitutable = !in_single;
        let unquoted = !in_single && !in_double;
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '$' if substitutable && chars.peek() == Some(&'(') => {
                chars.next();
                let body = collect_subshell(&mut chars)?;
                extract_substitution(&body, depth, &mut extracted)?;
                current.push_str("$()");
            }
            '`' if substitutable => {
                let body = collect_until_backtick(&mut chars)?;
                extract_substitution(&body, depth, &mut extracted)?;
                current.push('`');
            }
            '&' | '|' if unquoted => {
                if chars.peek() == Some(&ch) {
                    chars.next();
                }
                pieces.push(std::mem::take(&mut current));
            }
            ';' | '\n' | '\r' if unquoted => {
                pieces.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    if in_single || in_double {
        return Err(CommandShapeError::Malformed {
            detail: "unbalanced quoting".into(),
        });
    }
    pieces.push(current);
    for piece in pieces {
        let trimmed = piece.trim();
        if !trimmed.is_empty() {
            out.push(segment_of(trimmed));
        }
    }
    out.extend(extracted);
    Ok(())
}

fn extract_substitution(
    body: &str,
    depth: usize,
    out: &mut Vec<CommandSegment>,
) -> Result<(), CommandShapeError> {
    if depth < MAX_SUBSHELL_DEPTH {
        split_inner(body, depth + 1, out)
    } else {
        out.push(segment_of("$(…)"));
        Ok(())
    }
}

fn collect_subshell(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<String, CommandShapeError> {
    let mut body = String::new();
    let mut depth = 1;
    loop {
        let Some(ch) = chars.next() else {
            return Err(CommandShapeError::Malformed {
                detail: "unbalanced $(".into(),
            });
        };
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(body);
                }
            }
            _ => {}
        }
        body.push(ch);
    }
}

fn collect_until_backtick(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<String, CommandShapeError> {
    let mut body = String::new();
    loop {
        let Some(ch) = chars.next() else {
            return Err(CommandShapeError::Malformed {
                detail: "unbalanced backtick".into(),
            });
        };
        if ch == '`' {
            return Ok(body);
        }
        body.push(ch);
    }
}

fn segment_of(text: &str) -> CommandSegment {
    let word = text
        .split_whitespace()
        .next()
        .map_or_else(String::new, unquote);
    let program_word = word.strip_prefix('(').map_or(word.clone(), str::to_string);
    CommandSegment {
        text: text.to_string(),
        program_word,
    }
}

pub(crate) fn unquote(token: &str) -> String {
    let mut out = token;
    while let Some(inner) = out
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            out.strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        })
    {
        out = inner;
    }
    out.to_string()
}

/// A single segment's verdict under the prefix rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixVerdict {
    /// The longest matching prefix is an allow.
    Allowed,
    /// The longest matching prefix is a deny.
    Denied,
    /// No prefix matched (cannot auto-approve).
    Uncovered,
}

/// Command-prefix allow/deny lists with longest-prefix precedence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandRules {
    /// Prefixes that allow.
    pub allow: Vec<String>,
    /// Prefixes that deny; ties and shorter-vs-longer apply per rule.
    pub deny: Vec<String>,
}

impl CommandRules {
    /// Builds the rule pair; empty prefixes are rejected.
    ///
    /// # Errors
    /// [`CommandRuleError::EmptyPrefix`] on any blank entry.
    pub fn new(allow: &[&str], deny: &[&str]) -> Result<Self, CommandRuleError> {
        let build = |list: &[&str]| -> Result<Vec<String>, CommandRuleError> {
            list.iter()
                .map(|p| {
                    if p.trim().is_empty() {
                        Err(CommandRuleError::EmptyPrefix)
                    } else {
                        Ok((*p).to_string())
                    }
                })
                .collect()
        };
        Ok(Self {
            allow: build(allow)?,
            deny: build(deny)?,
        })
    }

    /// Longest-prefix verdict for one segment.
    #[must_use]
    pub fn segment_verdict(&self, segment: &CommandSegment) -> PrefixVerdict {
        let allow_len = longest_match(&self.allow, &segment.text);
        let deny_len = longest_match(&self.deny, &segment.text);
        match allow_len.cmp(&deny_len) {
            std::cmp::Ordering::Greater => PrefixVerdict::Allowed,
            std::cmp::Ordering::Less => PrefixVerdict::Denied,
            std::cmp::Ordering::Equal => {
                if deny_len == 0 {
                    PrefixVerdict::Uncovered
                } else {
                    PrefixVerdict::Denied
                }
            }
        }
    }

    /// Full-compound verdict: every segment must be independently
    /// allowed or the whole cannot auto-approve.
    ///
    /// # Errors
    /// [`CommandShapeError`] when the command cannot be segmented.
    pub fn evaluate(&self, command: &str) -> Result<CommandVerdict, CommandShapeError> {
        let segments = split_compound(command)?;
        for segment in &segments {
            match self.segment_verdict(segment) {
                PrefixVerdict::Denied => {
                    return Ok(CommandVerdict::DeniedByPrefix {
                        segment: segment.text.clone(),
                    });
                }
                PrefixVerdict::Uncovered => {
                    return Ok(CommandVerdict::SubstitutionBlocked {
                        offending: segment.text.clone(),
                    });
                }
                PrefixVerdict::Allowed => {}
            }
        }
        Ok(CommandVerdict::AllAllowed {
            segments: segments.len(),
        })
    }
}

/// Prefix-rule authoring failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandRuleError {
    /// A blank prefix entry.
    #[error("command prefix must not be empty")]
    EmptyPrefix,
}

fn longest_match(prefixes: &[String], text: &str) -> usize {
    prefixes
        .iter()
        .filter(|p| boundary_prefix(text, p))
        .map(String::len)
        .max()
        .unwrap_or(0)
}

fn boundary_prefix(text: &str, pattern: &str) -> bool {
    text == pattern || text.starts_with(&format!("{pattern} "))
}

/// The whole-command verdict under prefix rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandVerdict {
    /// Every segment independently allowed.
    AllAllowed {
        /// How many segments were judged.
        segments: usize,
    },
    /// At least one segment was not independently allowed — the whole
    /// command cannot auto-approve (Roo substitution guard).
    SubstitutionBlocked {
        /// The first segment that lacked coverage.
        offending: String,
    },
    /// A deny prefix won for some segment.
    DeniedByPrefix {
        /// The denied segment.
        segment: String,
    },
}
