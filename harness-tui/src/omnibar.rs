//! Omnibar: one input line owning @file/@symbol/@git mentions and the
//! `/mode`, `/rewind`, `/skill` command grammar, with pluggable
//! completion sources.

/// Mention namespace selectable inside chat text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MentionKind {
    /// `@file:` — repository paths.
    File,
    /// `@symbol:` — code symbols.
    Symbol,
    /// `@git:` — commits/refs.
    Git,
}

impl MentionKind {
    /// Prefix spelling inside a mention token.
    #[must_use]
    pub fn prefix(self) -> &'static str {
        match self {
            Self::File => "@file:",
            Self::Symbol => "@symbol:",
            Self::Git => "@git:",
        }
    }
}

/// Parsed omnibar content on submit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    /// Chat text (mentions left in place for the engine to consume).
    Chat(String),
    /// `/mode <name>`.
    Mode { name: String },
    /// `/rewind [steps]`.
    Rewind { steps: Option<u64> },
    /// `/skill <name> [args…]`.
    Skill { name: String, rest: String },
    /// Unknown slash word.
    UnknownCommand { word: String },
    /// Known word, bad arguments.
    Malformed { usage: &'static str },
}

/// Provider of completion candidates. The binary wires a real one;
/// tests wire fixtures.
pub trait CompletionSource: Send + Sync {
    /// Path candidates matching `query`.
    fn files(&self, query: &str) -> Vec<String>;
    /// Symbol candidates matching `query`.
    fn symbols(&self, query: &str) -> Vec<String>;
    /// Git ref/commit candidates matching `query`.
    fn git_refs(&self, query: &str) -> Vec<String>;
}

/// Empty provider (no completions anywhere).
pub struct NoCompletions;

impl CompletionSource for NoCompletions {
    fn files(&self, _query: &str) -> Vec<String> {
        Vec::new()
    }
    fn symbols(&self, _query: &str) -> Vec<String> {
        Vec::new()
    }
    fn git_refs(&self, _query: &str) -> Vec<String> {
        Vec::new()
    }
}

/// Slash commands offered by completion.
pub const COMMANDS: &[&str] = &["/mode", "/rewind", "/skill"];

/// Cap applied to every completion query regardless of caller bound.
pub const COMPLETE_LIMIT: usize = 20;

/// The omnibar input line.
#[derive(Debug, Clone, Default)]
pub struct Omnibar {
    chars: Vec<char>,
    cursor: usize,
}

impl Omnibar {
    /// New empty input line.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Current text.
    #[must_use]
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    /// Insert one character at the cursor.
    pub fn insert(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    /// Delete backwards; no-op at position 0.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    /// Replace the whole line (completion application, tests).
    pub fn set_text(&mut self, text: &str) {
        self.chars = text.chars().collect();
        self.cursor = self.chars.len();
    }

    /// The in-progress token the cursor sits in, when it starts with
    /// `@` (mention) or is the first token and starts with `/`
    /// (command).
    #[must_use]
    pub fn active_token(&self) -> Option<(MentionKind, String)> {
        let before: String = self.chars[..self.cursor].iter().collect();
        let start = before.rfind(['@', '/'])?;
        if start > 0 && self.chars[start - 1] != ' ' {
            return None;
        }
        let token: String = self.chars[start..].iter().collect();
        let token = token.split(' ').next().unwrap_or_default().to_owned();
        classify_token(&token)
    }

    fn active_command(&self) -> Option<String> {
        let (start, len) = self.token_span()?;
        let first = start == 0 && self.chars.first() == Some(&'/');
        first.then(|| self.chars[..len].iter().collect())
    }

    /// Completion candidates for the active token, bounded.
    #[must_use]
    pub fn completions(&self, source: &dyn CompletionSource, limit: usize) -> Vec<String> {
        let mut items: Vec<String> = if let Some(word) = self.active_command() {
            COMMANDS
                .iter()
                .filter(|cmd| cmd.starts_with(word.as_str()))
                .map(|s| (*s).to_owned())
                .collect()
        } else {
            match self.active_token() {
                Some((MentionKind::File, query)) => source.files(&query),
                Some((MentionKind::Symbol, query)) => source.symbols(&query),
                Some((MentionKind::Git, query)) => source.git_refs(&query),
                None => Vec::new(),
            }
        };
        items.truncate(limit);
        items
    }

    /// Replace the active token with a chosen completion. No token →
    /// no-op. Mention completions are rewritten into their fully
    /// qualified `@kind:` form.
    pub fn apply_completion(&mut self, choice: &str) {
        let Some((start, len)) = self.token_span() else {
            return;
        };
        let replacement = if self.active_command().is_some() || choice.starts_with('@') {
            choice.to_owned()
        } else {
            let kind = self.active_token().map_or(MentionKind::File, |(k, _)| k);
            format!("{}{}", kind.prefix(), choice)
        };
        self.replace_span(start, len, &replacement);
    }

    fn token_span(&self) -> Option<(usize, usize)> {
        let before: String = self.chars[..self.cursor].iter().collect();
        let start = before.rfind(['@', '/'])?;
        if start > 0 && self.chars[start - 1] != ' ' {
            return None;
        }
        let end = self.chars[start..]
            .iter()
            .position(|c| *c == ' ')
            .map_or(self.chars.len(), |offset| start + offset);
        Some((start, end - start))
    }

    fn replace_span(&mut self, start: usize, len: usize, replacement: &str) {
        self.chars.drain(start..start + len);
        let mut at = start;
        for c in replacement.chars() {
            self.chars.insert(at, c);
            at += 1;
        }
        self.cursor = at;
    }
}

fn classify_token(token: &str) -> Option<(MentionKind, String)> {
    for kind in [MentionKind::Symbol, MentionKind::Git] {
        if let Some(rest) = token.strip_prefix(kind.prefix()) {
            return Some((kind, rest.to_owned()));
        }
    }
    let rest = token.strip_prefix('@')?;
    Some((MentionKind::File, rest.to_owned()))
}

/// Parse omnibar content on submit.
#[must_use]
pub fn parse(input: &str) -> Parsed {
    let Some(command) = input.strip_prefix('/') else {
        return Parsed::Chat(input.to_owned());
    };
    let (word, rest) = match command.split_once(' ') {
        Some((word, rest)) => (word, rest.trim()),
        None => (command, ""),
    };
    match word {
        "mode" => match rest.is_empty() {
            true => Parsed::Malformed {
                usage: "usage: /mode <name>",
            },
            false => Parsed::Mode {
                name: rest.to_owned(),
            },
        },
        "rewind" => match rest.is_empty() {
            true => Parsed::Rewind { steps: None },
            false => match rest.parse::<u64>() {
                Ok(steps) => Parsed::Rewind { steps: Some(steps) },
                Err(_) => Parsed::Malformed {
                    usage: "usage: /rewind [steps]",
                },
            },
        },
        "skill" => match rest.split_once(' ') {
            Some((name, args)) => Parsed::Skill {
                name: name.to_owned(),
                rest: args.to_owned(),
            },
            None => Parsed::Malformed {
                usage: "usage: /skill <name> [args]",
            },
        },
        other => Parsed::UnknownCommand {
            word: other.to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture;

    impl CompletionSource for Fixture {
        fn files(&self, query: &str) -> Vec<String> {
            ["src/lib.rs", "src/main.rs", "README.md"]
                .iter()
                .filter(|f| f.contains(query))
                .map(|s| (*s).to_owned())
                .collect()
        }
        fn symbols(&self, query: &str) -> Vec<String> {
            ["Omnibar::insert", "Omnibar::backspace"]
                .iter()
                .filter(|s| s.contains(query))
                .map(|s| (*s).to_owned())
                .collect()
        }
        fn git_refs(&self, _query: &str) -> Vec<String> {
            vec!["main".into(), "HEAD".into()]
        }
    }

    #[test]
    fn editing_builds_text_and_backspace_removes() {
        let mut bar = Omnibar::default();
        for c in "hello".chars() {
            bar.insert(c);
        }
        assert_eq!(bar.text(), "hello");
        bar.backspace();
        assert_eq!(bar.text(), "hell");
        bar.set_text("reset");
        assert_eq!(bar.text(), "reset");
    }

    #[test]
    fn chat_text_with_mentions_parses_verbatim() {
        let parsed = parse("look at @file:src/lib.rs and @symbol:Omnibar now");
        assert_eq!(
            parsed,
            Parsed::Chat("look at @file:src/lib.rs and @symbol:Omnibar now".to_owned())
        );
    }

    #[test]
    fn slash_commands_parse_typed() {
        assert_eq!(
            parse("/mode auto"),
            Parsed::Mode {
                name: "auto".into()
            }
        );
        assert_eq!(parse("/rewind"), Parsed::Rewind { steps: None });
        assert_eq!(parse("/rewind 3"), Parsed::Rewind { steps: Some(3) });
        assert_eq!(
            parse("/skill audit run twice"),
            Parsed::Skill {
                name: "audit".into(),
                rest: "run twice".into()
            }
        );
        assert_eq!(
            parse("/wat x"),
            Parsed::UnknownCommand { word: "wat".into() }
        );
        assert!(matches!(parse("/rewind x"), Parsed::Malformed { .. }));
        assert!(matches!(parse("/mode"), Parsed::Malformed { .. }));
    }

    #[test]
    fn active_token_detects_mention_kinds_and_defaults_to_file() {
        let mut bar = Omnibar::default();
        bar.set_text("fix @fi");
        assert_eq!(bar.active_token(), Some((MentionKind::File, "fi".into())));
        bar.set_text("fix @symbol:Omn");
        assert_eq!(
            bar.active_token(),
            Some((MentionKind::Symbol, "Omn".into()))
        );
        bar.set_text("see @git:ma");
        assert_eq!(bar.active_token(), Some((MentionKind::Git, "ma".into())));
        bar.set_text("no tokens here");
        assert_eq!(bar.active_token(), None);
        bar.set_text("email a@b.com");
        assert_eq!(bar.active_token(), None);
    }

    #[test]
    fn completions_filter_by_kind_and_bounded() {
        let mut bar = Omnibar::default();
        bar.set_text("open @src");
        let got = bar.completions(&Fixture, 5);
        assert_eq!(got, vec!["src/lib.rs".to_owned(), "src/main.rs".to_owned()]);
        bar.set_text("open @src");
        let bounded = bar.completions(&Fixture, 1);
        assert_eq!(bounded.len(), 1);
        bar.set_text("/re");
        let cmds = bar.completions(&Fixture, 5);
        assert_eq!(cmds, vec!["/rewind".to_owned()]);
    }

    #[test]
    fn apply_completion_replaces_token_in_place() {
        let mut bar = Omnibar::default();
        bar.set_text("open @src and see");
        bar.apply_completion("src/lib.rs");
        assert_eq!(bar.text(), "open @file:src/lib.rs and see");
        bar.set_text("/rew");
        bar.apply_completion("/rewind");
        assert_eq!(bar.text(), "/rewind");
    }
}
