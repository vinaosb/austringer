//! The unified command grammar every frontend parses (todo 23): one
//! const table of slash commands, the canonical allow-scope labels,
//! and the UI vocabulary terms, mirrored byte-for-byte into
//! `harness-web/assets/grammar.js` and `harness-vscode/src/grammar.ts`
//! (both pinned to `grammar.json` by their own tests; this module pins
//! it here). Frontends parse CORE commands identically; `local`
//! commands (`/exit`, `/image`) are the CLI's own.

/// One slash command in the shared grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashCommand {
    /// Command word without the leading slash.
    pub word: &'static str,
    /// Usage line (also the malformed-argument feedback).
    pub usage: &'static str,
    /// One-line description for the cheat sheet.
    pub help: &'static str,
    /// True for CLI-local commands other frontends recognize but
    /// refuse with `local_only_note`.
    pub local: bool,
}

/// The command table. Order is the cheat-sheet order.
pub const COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        word: "help",
        usage: "/help",
        help: "show this command list",
        local: false,
    },
    SlashCommand {
        word: "steer",
        usage: "/steer <text>",
        help: "steer the running turn with more instructions",
        local: false,
    },
    SlashCommand {
        word: "cancel",
        usage: "/cancel",
        help: "cancel the running turn",
        local: false,
    },
    SlashCommand {
        word: "vote-cancel",
        usage: "/vote-cancel [ask-id]",
        help: "withdraw an open permission ask (no id = the single open ask)",
        local: false,
    },
    SlashCommand {
        word: "mode",
        usage: "/mode plan|off",
        help: "enter or exit plan mode",
        local: false,
    },
    SlashCommand {
        word: "rewind",
        usage: "/rewind [steps]",
        help: "rewind to a previous restore point (default 1)",
        local: false,
    },
    SlashCommand {
        word: "skill",
        usage: "/skill <name> [args]",
        help: "run a trusted skill through the submit lane",
        local: false,
    },
    SlashCommand {
        word: "exit",
        usage: "/exit",
        help: "quit this repl",
        local: true,
    },
    SlashCommand {
        word: "image",
        usage: "/image <path>",
        help: "attach an image to the next submit",
        local: true,
    },
];

/// Allow-scope canonical labels (TUI modal, web gate buttons, VS Code
/// quick-picks render these verbatim).
pub const ALLOW_ONCE: &str = "y (once)";
/// Session-scope allow label.
pub const ALLOW_SESSION: &str = "a (session)";
/// Deny label.
pub const DENY: &str = "n (deny)";
/// Ask-withdraw label (permission asks only).
pub const VOTE_CANCEL: &str = "c (vote-cancel)";

/// The four allow-scope labels in canonical display order.
pub const ALLOW_SCOPES: &[&str] = &[ALLOW_ONCE, ALLOW_SESSION, DENY, VOTE_CANCEL];

/// Vocabulary term: the child-agent grid is copy-named "agents"
/// (wire/team.rs internals keep their own spelling).
pub const VOCAB_AGENTS: &str = "agents";
/// Vocabulary term: a snapshot event marks a restore point.
pub const VOCAB_REWIND_POINT: &str = "rewind point";
/// Vocabulary term: share boxes mint redacted share links.
pub const VOCAB_SHARE_LINK: &str = "redacted share link";
/// Vocabulary term: a replayed state snapshot in CLI output.
pub const VOCAB_STATE_REPLAYED: &str = "state replayed";

/// Look up a command by word.
#[must_use]
pub fn find(word: &str) -> Option<&'static SlashCommand> {
    COMMANDS.iter().find(|command| command.word == word)
}

/// The core (non-local) command words with their leading slash, in
/// table order — the completion list every non-CLI frontend offers.
#[must_use]
pub fn core_command_words() -> Vec<String> {
    COMMANDS
        .iter()
        .filter(|command| !command.local)
        .map(|command| format!("/{}", command.word))
        .collect()
}

/// The `/help` cheat sheet: one `{usage} — {help}` line per command,
/// local commands marked `(cli only)`. Identical in every frontend.
#[must_use]
pub fn cheat_sheet() -> String {
    COMMANDS
        .iter()
        .map(|command| {
            if command.local {
                format!("{} — {} (cli only)", command.usage, command.help)
            } else {
                format!("{} — {}", command.usage, command.help)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The unknown-command feedback. Byte-identical in every frontend —
/// the cross-frontend contract test pins it.
#[must_use]
pub fn unknown_help(word: &str) -> String {
    format!("unknown command /{word} — /help lists commands")
}

/// Feedback when a CLI-local command is typed on another frontend.
#[must_use]
pub fn local_only_note(word: &str) -> String {
    format!("/{word} is cli-only")
}

/// Plan-mode argument mapping: `plan` enters, `off`/`exit` leaves,
/// anything else is engine-side and notes honestly (None).
#[must_use]
pub fn mode_on(name: &str) -> Option<bool> {
    match name {
        "plan" => Some(true),
        "off" | "exit" => Some(false),
        _ => None,
    }
}

/// The honest note for an unwired mode name (shared copy).
#[must_use]
pub fn mode_note(name: &str) -> String {
    format!(
        "/mode {name}: daemon plan mode is the wired surface (/mode plan, /mode off); \
         autonomy levels are engine-side"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct CommandDoc {
        word: &'static str,
        usage: &'static str,
        help: &'static str,
        local: bool,
    }

    #[derive(Serialize)]
    struct VocabularyDoc {
        agents: &'static str,
        rewind_point: &'static str,
        share_link: &'static str,
        state_replayed: &'static str,
    }

    #[derive(Serialize)]
    struct GrammarDoc {
        commands: Vec<CommandDoc>,
        allow_scopes: Vec<&'static str>,
        vocabulary: VocabularyDoc,
    }

    fn doc() -> GrammarDoc {
        GrammarDoc {
            commands: COMMANDS
                .iter()
                .map(|command| CommandDoc {
                    word: command.word,
                    usage: command.usage,
                    help: command.help,
                    local: command.local,
                })
                .collect(),
            allow_scopes: ALLOW_SCOPES.to_vec(),
            vocabulary: VocabularyDoc {
                agents: VOCAB_AGENTS,
                rewind_point: VOCAB_REWIND_POINT,
                share_link: VOCAB_SHARE_LINK,
                state_replayed: VOCAB_STATE_REPLAYED,
            },
        }
    }

    /// Byte pin: grammar.json (which the JS and TS ports deep-equal in
    /// their own suites) must be exactly this table serialized.
    #[test]
    fn grammar_json_is_byte_pinned_to_the_const_table() {
        let serialized = serde_json::to_string_pretty(&doc()).expect("grammar doc serializes");
        assert_eq!(
            serialized,
            include_str!("grammar.json").trim_end(),
            "grammar.json drifted from grammar.rs — regenerate it from the table"
        );
    }

    #[test]
    fn cheat_sheet_renders_one_line_per_command() {
        let expected = "\
/help — show this command list
/steer <text> — steer the running turn with more instructions
/cancel — cancel the running turn
/vote-cancel [ask-id] — withdraw an open permission ask (no id = the single open ask)
/mode plan|off — enter or exit plan mode
/rewind [steps] — rewind to a previous restore point (default 1)
/skill <name> [args] — run a trusted skill through the submit lane
/exit — quit this repl (cli only)
/image <path> — attach an image to the next submit (cli only)";
        assert_eq!(cheat_sheet(), expected);
    }

    #[test]
    fn unknown_help_and_local_notes_are_the_contract_strings() {
        assert_eq!(
            unknown_help("model"),
            "unknown command /model — /help lists commands"
        );
        assert_eq!(local_only_note("exit"), "/exit is cli-only");
    }

    #[test]
    fn core_command_words_exclude_local_commands() {
        assert_eq!(
            core_command_words(),
            vec![
                "/help", "/steer", "/cancel", "/vote-cancel", "/mode", "/rewind", "/skill",
            ]
        );
    }

    #[test]
    fn mode_arguments_map_with_an_honest_none() {
        assert_eq!(mode_on("plan"), Some(true));
        assert_eq!(mode_on("off"), Some(false));
        assert_eq!(mode_on("exit"), Some(false));
        assert_eq!(mode_on("auto"), None);
        assert!(mode_note("auto").contains("/mode auto"));
    }

    #[test]
    fn allow_scopes_carry_the_canonical_labels() {
        assert_eq!(
            ALLOW_SCOPES,
            &["y (once)", "a (session)", "n (deny)", "c (vote-cancel)"]
        );
    }
}
