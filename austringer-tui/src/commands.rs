//! Submit-time command parsing (todo 23): the omnibar text against
//! the shared `austringer_daemon::grammar` table. Split from `omnibar`
//! (the input line + completions live there); both halves stay under
//! the size budget.

/// Parsed omnibar content on submit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    /// Chat text (mentions left in place for the engine to consume).
    Chat(String),
    /// `/help` — show the shared cheat sheet.
    Help,
    /// `/steer <text>` — explicit steer, regardless of turn state.
    Steer { text: String },
    /// `/cancel` — cancel the running turn.
    Cancel,
    /// `/vote-cancel [ask-id]` — withdraw a permission ask.
    VoteCancel { ask_id: Option<u64> },
    /// `/mode <name>`.
    Mode { name: String },
    /// `/rewind [steps]`.
    Rewind { steps: Option<u64> },
    /// `/skill <name> [args…]`.
    Skill { name: String, rest: String },
    /// `/condense` — manual context compaction.
    Condense,
    /// A CLI-local command (`/exit`, `/image`) — known word, refused
    /// here with the shared cli-only note.
    CliOnly { word: String },
    /// Unknown slash word.
    UnknownCommand { word: String },
    /// Known word, bad arguments.
    Malformed { usage: &'static str },
}

/// Slash commands offered by completion (the shared grammar's core
/// words; CLI-local commands never complete here). Pinned to
/// `austringer_daemon::grammar` by test.
pub const COMMANDS: &[&str] = &[
    "/help",
    "/steer",
    "/cancel",
    "/vote-cancel",
    "/mode",
    "/rewind",
    "/skill",
    "/condense",
];

/// Parse omnibar content on submit against the shared grammar table.
#[must_use]
pub fn parse(input: &str) -> Parsed {
    let Some(command) = input.strip_prefix('/') else {
        return Parsed::Chat(input.to_owned());
    };
    let (word, rest) = match command.split_once(' ') {
        Some((word, rest)) => (word, rest.trim()),
        None => (command, ""),
    };
    let usage = |word: &str| austringer_daemon::grammar::find(word).map_or("", |cmd| cmd.usage);
    match word {
        "help" => Parsed::Help,
        "steer" => match rest.is_empty() {
            true => Parsed::Malformed {
                usage: usage("steer"),
            },
            false => Parsed::Steer {
                text: rest.to_owned(),
            },
        },
        "cancel" => Parsed::Cancel,
        "vote-cancel" => match optional_count(rest) {
            Some(ask_id) => Parsed::VoteCancel { ask_id },
            None => Parsed::Malformed {
                usage: usage("vote-cancel"),
            },
        },
        "mode" => match rest.is_empty() {
            true => Parsed::Malformed {
                usage: usage("mode"),
            },
            false => Parsed::Mode {
                name: rest.to_owned(),
            },
        },
        "rewind" => match optional_count(rest) {
            Some(steps) => Parsed::Rewind { steps },
            None => Parsed::Malformed {
                usage: usage("rewind"),
            },
        },
        "skill" => match rest.is_empty() {
            true => Parsed::Malformed {
                usage: usage("skill"),
            },
            false => {
                let (name, args) = rest.split_once(' ').unwrap_or((rest, ""));
                Parsed::Skill {
                    name: name.to_owned(),
                    rest: args.to_owned(),
                }
            }
        },
        "condense" => Parsed::Condense,
        "exit" | "image" => Parsed::CliOnly {
            word: word.to_owned(),
        },
        other => Parsed::UnknownCommand {
            word: other.to_owned(),
        },
    }
}

/// `[count]` argument: empty, or a u64. `None` = malformed.
fn optional_count(rest: &str) -> Option<Option<u64>> {
    match rest.is_empty() {
        true => Some(None),
        false => rest.parse::<u64>().ok().map(Some),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            parse("/skill audit"),
            Parsed::Skill {
                name: "audit".into(),
                rest: String::new()
            }
        );
        assert_eq!(
            parse("/wat x"),
            Parsed::UnknownCommand { word: "wat".into() }
        );
        assert!(
            matches!(parse("/rewind x"), Parsed::Malformed { .. }),
            "rewind rejects non-numeric steps"
        );
        assert!(matches!(parse("/mode"), Parsed::Malformed { .. }));
        assert_eq!(parse("/condense"), Parsed::Condense);
        assert_eq!(parse("/condense now"), Parsed::Condense);
    }

    #[test]
    fn todo_23_core_commands_parse_typed() {
        assert_eq!(parse("/help"), Parsed::Help);
        assert_eq!(
            parse("/steer tighter types"),
            Parsed::Steer {
                text: "tighter types".into()
            }
        );
        assert_eq!(parse("/cancel"), Parsed::Cancel);
        assert_eq!(parse("/cancel now"), Parsed::Cancel);
        assert_eq!(parse("/vote-cancel"), Parsed::VoteCancel { ask_id: None });
        assert_eq!(
            parse("/vote-cancel 7"),
            Parsed::VoteCancel { ask_id: Some(7) }
        );
        assert_eq!(
            parse("/exit"),
            Parsed::CliOnly {
                word: "exit".into()
            }
        );
        assert_eq!(
            parse("/image pic.png"),
            Parsed::CliOnly {
                word: "image".into()
            }
        );
    }

    #[test]
    fn todo_23_malformed_usage_comes_from_the_shared_table() {
        assert_eq!(
            parse("/steer"),
            Parsed::Malformed {
                usage: "/steer <text>"
            }
        );
        assert_eq!(
            parse("/mode"),
            Parsed::Malformed {
                usage: "/mode plan|off"
            }
        );
        assert_eq!(
            parse("/skill"),
            Parsed::Malformed {
                usage: "/skill <name> [args]"
            }
        );
        assert_eq!(
            parse("/vote-cancel x"),
            Parsed::Malformed {
                usage: "/vote-cancel [ask-id]"
            }
        );
    }

    #[test]
    fn todo_23_completion_list_is_pinned_to_the_shared_grammar() {
        let expected: Vec<String> = austringer_daemon::grammar::core_command_words();
        assert_eq!(
            COMMANDS.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
            expected
        );
    }
}
