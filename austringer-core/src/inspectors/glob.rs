//! Fail-closed glob engine for permission and secret-file patterns.
//!
//! Semantics:
//!
//! - `*` matches any run of non-separator characters; `**` matches any run
//!   including separators.
//! - `?` matches exactly one non-separator character.
//! - `[abc]`, `[a-z]`, `[!abc]` match one non-separator character from the
//!   class (`!` negates the membership test).
//! - `\x` escapes `x` (a literal `*` is written `\*`).
//! - Patterns use `/` as the separator; candidates are normalized first
//!   (Windows `\` becomes `/`). Matching is case-sensitive and dotfiles are
//!   ordinary names (`*` matches `.env`).
//!
//! Compilation is fail-closed: malformed patterns (empty, unclosed class,
//! trailing escape, reversed range) return [`InvalidGlob`] instead of a
//! permissive matcher — a glob that cannot resolve must never yield an
//! allow.

use std::fmt;

/// A pattern that failed to compile. The error is typed and carries the
/// offending pattern so configuration surfaces can reject it loudly.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid glob {pattern:?}: {reason}")]
pub struct InvalidGlob {
    /// The pattern that failed to compile.
    pub pattern: String,
    /// Why it failed.
    pub reason: &'static str,
}

/// One compiled class item: a single char or an inclusive range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClassItem {
    /// Matches this character.
    Char(char),
    /// Matches the inclusive range `lo..=hi`.
    Range(char, char),
}

/// One compiled pattern token.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    /// Matches one literal character.
    Literal(char),
    /// Matches one non-separator character.
    AnyOne,
    /// Matches any run (possibly empty) of non-separator characters.
    AnySeq,
    /// Matches any run (possibly empty) of characters, separators included.
    AnyAll,
    /// Matches one non-separator character per the class.
    Class {
        /// Class members.
        items: Vec<ClassItem>,
        /// True when the match is on non-membership.
        negated: bool,
    },
    /// A leading `**/`: matches the empty string or any prefix that ends
    /// with `/` (so `**/x` matches both `x` and `d/x` — the zero-directory
    /// case).
    PrefixGlobSlash,
    /// An interior `/**/`: matches `/` or any substring that starts and
    /// ends with `/` (so `a/**/b` matches `a/b` too).
    InfixGlobSlash,
}

/// A compiled glob pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glob {
    pattern: String,
    tokens: Vec<Token>,
}

impl Glob {
    /// Compiles `pattern`. Malformed patterns are rejected with a typed
    /// error (fail closed).
    pub fn new(pattern: &str) -> Result<Self, InvalidGlob> {
        if pattern.is_empty() {
            return Err(Self::invalid(pattern, "empty pattern"));
        }
        let chars: Vec<char> = pattern.chars().collect();
        let mut tokens = Vec::new();
        let mut index = 0;
        while index < chars.len() {
            match chars[index] {
                '*' => {
                    if chars.get(index + 1) == Some(&'*') {
                        tokens.push(Token::AnyAll);
                        index += 2;
                    } else {
                        tokens.push(Token::AnySeq);
                        index += 1;
                    }
                }
                '?' => {
                    tokens.push(Token::AnyOne);
                    index += 1;
                }
                '[' => {
                    let (class, next) = Self::parse_class(&chars, index)?;
                    tokens.push(class);
                    index = next;
                }
                '\\' => {
                    let Some(&escaped) = chars.get(index + 1) else {
                        return Err(Self::invalid(pattern, "trailing escape"));
                    };
                    tokens.push(Token::Literal(escaped));
                    index += 2;
                }
                literal => {
                    tokens.push(Token::Literal(literal));
                    index += 1;
                }
            }
        }
        Ok(Self {
            pattern: pattern.to_owned(),
            tokens: Self::compose_globs(tokens),
        })
    }

    /// Rewrites `**/` sequences into their composite tokens so the
    /// zero-directory cases match (`**/x` matches `x`; `a/**/b` matches
    /// `a/b`). Left-to-right and greedy: `/**/**/` composes once then
    /// leaves `**/` as ordinary tokens.
    fn compose_globs(tokens: Vec<Token>) -> Vec<Token> {
        let mut composed = Vec::with_capacity(tokens.len());
        let mut index = 0;
        while index < tokens.len() {
            let leading = index == 0
                && tokens[index] == Token::AnyAll
                && tokens.get(index + 1) == Some(&Token::Literal('/'));
            let interior = tokens[index] == Token::Literal('/')
                && tokens.get(index + 1) == Some(&Token::AnyAll)
                && tokens.get(index + 2) == Some(&Token::Literal('/'));
            if leading {
                composed.push(Token::PrefixGlobSlash);
                index += 2;
            } else if interior {
                composed.push(Token::InfixGlobSlash);
                index += 3;
            } else {
                composed.push(tokens[index].clone());
                index += 1;
            }
        }
        composed
    }

    /// The original pattern text (for feedback and diagnostics).
    #[must_use]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Whether the normalized candidate matches the pattern. Candidates on
    /// Windows-style paths are matched after `\` is normalized to `/`.
    #[must_use]
    pub fn matches(&self, candidate: &str) -> bool {
        let name: Vec<char> = candidate.replace('\\', "/").chars().collect();
        // Dynamic programming over (token row, candidate prefix): each row
        // answers "does tokens[..=i] match name[..j]?".
        let mut previous = vec![false; name.len() + 1];
        previous[0] = true;
        for token in &self.tokens {
            let mut current = vec![false; name.len() + 1];
            match token {
                Token::Literal(c) => {
                    for (j, cell) in current.iter_mut().enumerate().skip(1) {
                        *cell = previous[j - 1] && name[j - 1] == *c;
                    }
                }
                Token::AnyOne => {
                    for (j, cell) in current.iter_mut().enumerate().skip(1) {
                        *cell = previous[j - 1] && name[j - 1] != '/';
                    }
                }
                Token::Class { items, negated } => {
                    for (j, cell) in current.iter_mut().enumerate().skip(1) {
                        let ch = name[j - 1];
                        let member = items.iter().any(|item| match item {
                            ClassItem::Char(c) => ch == *c,
                            ClassItem::Range(lo, hi) => *lo <= ch && ch <= *hi,
                        });
                        *cell = previous[j - 1] && ch != '/' && member != *negated;
                    }
                }
                Token::AnySeq => {
                    current[0] = previous[0];
                    for j in 1..=name.len() {
                        current[j] = previous[j] || (current[j - 1] && name[j - 1] != '/');
                    }
                }
                Token::AnyAll => {
                    current[0] = previous[0];
                    for j in 1..=name.len() {
                        current[j] = previous[j] || current[j - 1];
                    }
                }
                Token::PrefixGlobSlash => {
                    // Empty form, or any consumed prefix ending in `/`.
                    let mut any_previous = vec![false; name.len() + 1];
                    any_previous[0] = previous[0];
                    for m in 1..=name.len() {
                        any_previous[m] = any_previous[m - 1] || previous[m];
                    }
                    current[0] = previous[0];
                    for j in 1..=name.len() {
                        current[j] = previous[j] || (name[j - 1] == '/' && any_previous[j - 1]);
                    }
                }
                Token::InfixGlobSlash => {
                    // `/` alone, or any substring that both starts and ends
                    // with `/`.
                    let mut starts_slash = vec![false; name.len() + 1];
                    for m in 0..name.len() {
                        starts_slash[m + 1] = starts_slash[m] || (name[m] == '/' && previous[m]);
                    }
                    for j in 1..=name.len() {
                        current[j] = name[j - 1] == '/' && starts_slash[j];
                    }
                }
            }
            previous = current;
        }
        previous[name.len()]
    }

    fn parse_class(chars: &[char], open: usize) -> Result<(Token, usize), InvalidGlob> {
        let mut index = open + 1;
        let negated = chars.get(index) == Some(&'!');
        if negated {
            index += 1;
        }
        let mut items = Vec::new();
        // A `]` as the first member is a literal, not the closing bracket.
        let mut first = true;
        while index < chars.len() {
            if chars[index] == ']' && !first {
                if items.is_empty() {
                    return Err(Self::invalid(
                        &chars.iter().collect::<String>(),
                        "empty character class",
                    ));
                }
                return Ok((Token::Class { items, negated }, index + 1));
            }
            first = false;
            let lo = chars[index];
            if chars.get(index + 1) == Some(&'-') && chars.get(index + 2).is_some_and(|&c| c != ']')
            {
                let hi = chars[index + 2];
                if lo > hi {
                    return Err(Self::invalid(
                        &chars.iter().collect::<String>(),
                        "reversed character range",
                    ));
                }
                items.push(ClassItem::Range(lo, hi));
                index += 3;
            } else {
                items.push(ClassItem::Char(lo));
                index += 1;
            }
        }
        Err(Self::invalid(
            &chars.iter().collect::<String>(),
            "unclosed character class",
        ))
    }

    fn invalid(pattern: &str, reason: &'static str) -> InvalidGlob {
        InvalidGlob {
            pattern: pattern.to_owned(),
            reason,
        }
    }
}

impl fmt::Display for Glob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.pattern)
    }
}
