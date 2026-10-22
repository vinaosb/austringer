//! REPL input parsing (todo 23): one typed line against the shared
//! `harness_daemon::grammar` table. Every core command parses typed;
//! `/exit` and `/image` are the CLI-local dispatches the other
//! frontends refuse; unknown words get the shared help string.

/// One parsed REPL line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplLine {
    /// Plain text submits a turn.
    Turn(String),
    /// `/help` — print the shared cheat sheet.
    Help,
    /// `/steer <text>`.
    Steer(String),
    /// `/cancel` — cancel the running turn.
    Cancel,
    /// `/vote-cancel [ask-id]` — withdraw a permission ask.
    VoteCancel(Option<u64>),
    /// `/mode <name>` (raw argument; dispatch maps via `mode_on`).
    Mode(String),
    /// `/rewind [steps]`.
    Rewind(Option<u64>),
    /// `/skill …` — rides the submit lane verbatim.
    Skill(String),
    /// `/exit` — quit the repl.
    Exit,
    /// `/image <path>` — attach to the next submit.
    Image(String),
    /// Known word, bad arguments (usage from the shared table).
    Malformed(&'static str),
    /// Unknown slash word.
    Unknown(String),
}

/// Parse one REPL line. Empty input is the caller's concern (the loop
/// skips it before parsing).
#[must_use]
pub fn parse_line(line: &str) -> ReplLine {
    let Some(command) = line.strip_prefix('/') else {
        return ReplLine::Turn(line.to_owned());
    };
    let (word, rest) = match command.split_once(' ') {
        Some((word, rest)) => (word, rest.trim()),
        None => (command, ""),
    };
    let usage = |word: &str| harness_daemon::grammar::find(word).map_or("", |cmd| cmd.usage);
    match word {
        "help" => ReplLine::Help,
        "steer" => match rest.is_empty() {
            true => ReplLine::Malformed(usage("steer")),
            false => ReplLine::Steer(rest.to_owned()),
        },
        "cancel" => ReplLine::Cancel,
        "vote-cancel" => match optional_id(rest) {
            Some(ask_id) => ReplLine::VoteCancel(ask_id),
            None => ReplLine::Malformed(usage("vote-cancel")),
        },
        "mode" => match rest.is_empty() {
            true => ReplLine::Malformed(usage("mode")),
            false => ReplLine::Mode(rest.to_owned()),
        },
        "rewind" => match optional_id(rest) {
            Some(steps) => ReplLine::Rewind(steps),
            None => ReplLine::Malformed(usage("rewind")),
        },
        "skill" => match rest.is_empty() {
            true => ReplLine::Malformed(usage("skill")),
            false => ReplLine::Skill(format!("/skill {rest}")),
        },
        "exit" => ReplLine::Exit,
        "image" => match rest.is_empty() {
            true => ReplLine::Malformed(usage("image")),
            false => ReplLine::Image(rest.to_owned()),
        },
        other => ReplLine::Unknown(other.to_owned()),
    }
}

fn optional_id(rest: &str) -> Option<Option<u64>> {
    match rest.is_empty() {
        true => Some(None),
        false => rest.parse::<u64>().ok().map(Some),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_and_every_core_command_parse_typed() {
        assert_eq!(parse_line("fix the bug"), ReplLine::Turn("fix the bug".into()));
        assert_eq!(parse_line("/help"), ReplLine::Help);
        assert_eq!(parse_line("/steer hurry"), ReplLine::Steer("hurry".into()));
        assert_eq!(parse_line("/cancel"), ReplLine::Cancel);
        assert_eq!(parse_line("/cancel now"), ReplLine::Cancel);
        assert_eq!(parse_line("/vote-cancel"), ReplLine::VoteCancel(None));
        assert_eq!(parse_line("/vote-cancel 7"), ReplLine::VoteCancel(Some(7)));
        assert_eq!(parse_line("/mode plan"), ReplLine::Mode("plan".into()));
        assert_eq!(parse_line("/rewind"), ReplLine::Rewind(None));
        assert_eq!(parse_line("/rewind 3"), ReplLine::Rewind(Some(3)));
        assert_eq!(
            parse_line("/skill audit run twice"),
            ReplLine::Skill("/skill audit run twice".into())
        );
        assert_eq!(parse_line("/exit"), ReplLine::Exit);
        assert_eq!(
            parse_line("/image pic.png"),
            ReplLine::Image("pic.png".into())
        );
    }

    #[test]
    fn malformed_usage_comes_from_the_shared_table() {
        assert_eq!(parse_line("/steer"), ReplLine::Malformed("/steer <text>"));
        assert_eq!(parse_line("/mode"), ReplLine::Malformed("/mode plan|off"));
        assert_eq!(
            parse_line("/rewind x"),
            ReplLine::Malformed("/rewind [steps]")
        );
        assert_eq!(
            parse_line("/vote-cancel x"),
            ReplLine::Malformed("/vote-cancel [ask-id]")
        );
        assert_eq!(
            parse_line("/skill"),
            ReplLine::Malformed("/skill <name> [args]")
        );
        assert_eq!(parse_line("/image"), ReplLine::Malformed("/image <path>"));
    }

    #[test]
    fn unknown_words_carry_the_shared_help_string() {
        assert_eq!(parse_line("/model sonnet"), ReplLine::Unknown("model".into()));
        assert_eq!(
            harness_daemon::grammar::unknown_help("model"),
            "unknown command /model — /help lists commands"
        );
    }
}
