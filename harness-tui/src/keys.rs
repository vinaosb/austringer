//! Keybind configuration: one action set, default chords, TOML overrides.

use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyModifiers};
use serde::Deserialize;

/// Every action the UI loop can dispatch from a key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Quit,
    Reconnect,
    FocusOmnibar,
    BlurOmnibar,
    Submit,
    ToggleTranscript,
    ToggleTools,
    ToggleTeam,
    ToggleHud,
    AllowOnce,
    AllowSession,
    Deny,
    CancelAsk,
    ScrollUp,
    ScrollDown,
}

impl Action {
    /// Stable TOML key for this action.
    #[must_use]
    pub fn config_name(self) -> &'static str {
        ACTION_NAMES
            .iter()
            .find(|(action, _)| *action == self)
            .map_or("", |(_, name)| name)
    }

    fn from_config_name(name: &str) -> Option<Self> {
        ACTION_NAMES
            .iter()
            .find(|(_, candidate)| *candidate == name)
            .map(|(action, _)| *action)
    }
}

/// The one action vocabulary: action -> stable config name.
const ACTION_NAMES: &[(Action, &str)] = &[
    (Action::Quit, "quit"),
    (Action::Reconnect, "reconnect"),
    (Action::FocusOmnibar, "focus_omnibar"),
    (Action::BlurOmnibar, "blur_omnibar"),
    (Action::Submit, "submit"),
    (Action::ToggleTranscript, "toggle_transcript"),
    (Action::ToggleTools, "toggle_tools"),
    (Action::ToggleTeam, "toggle_team"),
    (Action::ToggleHud, "toggle_hud"),
    (Action::AllowOnce, "allow_once"),
    (Action::AllowSession, "allow_session"),
    (Action::Deny, "deny"),
    (Action::CancelAsk, "cancel_ask"),
    (Action::ScrollUp, "scroll_up"),
    (Action::ScrollDown, "scroll_down"),
];

/// One key chord: code plus active modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    /// Key code.
    pub code: KeyCode,
    /// Modifier mask (CTRL/ALT/SHIFT).
    pub mods: KeyModifiers,
}

/// Why a keybind file was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeybindError {
    /// The file could not be read.
    #[error("keybind file unreadable: {0}")]
    Io(String),
    /// The file is not valid TOML for this format.
    #[error("keybind file malformed: {0}")]
    Malformed(String),
    /// A chord string did not parse.
    #[error("unknown key: {0}")]
    UnknownKey(String),
    /// An action name is not in the vocabulary.
    #[error("unknown action: {0}")]
    UnknownAction(String),
    /// Two actions claimed the same chord.
    #[error("duplicate chord: {0}")]
    Duplicate(String),
}

/// The active binding set. Exactly one chord per action; rebinding an
/// action replaces its chord.
#[derive(Debug, Clone, Default)]
pub struct Keybinds {
    chords: HashMap<Key, Action>,
}

impl Keybinds {
    /// Action bound to a chord, if any.
    #[must_use]
    pub fn action(&self, key: Key) -> Option<Action> {
        self.chords.get(&key).copied()
    }

    /// Chord bound to an action, if any.
    #[must_use]
    pub fn chord(&self, action: Action) -> Option<Key> {
        self.chords
            .iter()
            .find(|(_, a)| **a == action)
            .map(|(k, _)| *k)
    }

    /// Replace (or set) one action's chord. Errors when the chord is
    /// already owned by a different action.
    pub fn rebind(&mut self, action: Action, key: Key) -> Result<(), KeybindError> {
        if let Some(owner) = self.chords.get(&key).copied()
            && owner != action
        {
            return Err(KeybindError::Duplicate(key.to_config_string()));
        }
        self.chords.retain(|_, a| *a != action);
        self.chords.insert(key, action);
        Ok(())
    }

    /// The built-in layout.
    #[must_use]
    pub fn defaults() -> Self {
        let mut binds = Self::default();
        let pairs: &[(Action, &str)] = &[
            (Action::Quit, "q"),
            (Action::Reconnect, "ctrl+r"),
            (Action::FocusOmnibar, "/"),
            (Action::FocusOmnibar, "i"),
            (Action::BlurOmnibar, "esc"),
            (Action::Submit, "enter"),
            (Action::ToggleTranscript, "ctrl+1"),
            (Action::ToggleTools, "ctrl+2"),
            (Action::ToggleTeam, "ctrl+3"),
            (Action::ToggleHud, "ctrl+4"),
            (Action::AllowOnce, "y"),
            (Action::AllowSession, "a"),
            (Action::Deny, "n"),
            (Action::CancelAsk, "c"),
            (Action::ScrollUp, "up"),
            (Action::ScrollDown, "down"),
        ];
        for (action, chord) in pairs {
            let key = parse_chord(chord);
            if let Ok(key) = key {
                let _ = binds.rebind(*action, key);
            }
        }
        binds
    }

    /// Load overrides from TOML (`[keys]` table: `action = "chord"`),
    /// applied on top of `self`.
    ///
    /// # Errors
    /// [`KeybindError`] naming the first offending entry.
    pub fn load_overrides(&mut self, text: &str) -> Result<(), KeybindError> {
        #[derive(Deserialize)]
        struct File {
            keys: Option<HashMap<String, String>>,
        }
        let parsed: File =
            toml::from_str(text).map_err(|e| KeybindError::Malformed(e.to_string()))?;
        let Some(entries) = parsed.keys else {
            return Ok(());
        };
        let mut staged = self.clone();
        for (action_name, chord) in entries {
            let action = Action::from_config_name(&action_name)
                .ok_or_else(|| KeybindError::UnknownAction(action_name.clone()))?;
            let key = parse_chord(&chord)?;
            staged.rebind(action, key)?;
        }
        *self = staged;
        Ok(())
    }
}

/// Parse a chord: `["ctrl+"|"alt+"|"shift+"]* key`, where key is a
/// single character or a named key.
///
/// # Errors
/// [`KeybindError::UnknownKey`] for any other shape.
pub fn parse_chord(chord: &str) -> Result<Key, KeybindError> {
    let mut mods = KeyModifiers::NONE;
    let mut parts = chord.split('+');
    let Some(last) = parts.next_back() else {
        return Err(KeybindError::UnknownKey(chord.to_owned()));
    };
    for prefix in parts {
        mods |= match prefix {
            "ctrl" => KeyModifiers::CONTROL,
            "alt" => KeyModifiers::ALT,
            "shift" => KeyModifiers::SHIFT,
            _ => return Err(KeybindError::UnknownKey(chord.to_owned())),
        };
    }
    let code = key_code(last).ok_or_else(|| KeybindError::UnknownKey(chord.to_owned()))?;
    Ok(Key { code, mods })
}

fn key_code(name: &str) -> Option<KeyCode> {
    let named: &[(&str, KeyCode)] = &[
        ("enter", KeyCode::Enter),
        ("esc", KeyCode::Esc),
        ("tab", KeyCode::Tab),
        ("backtab", KeyCode::BackTab),
        ("backspace", KeyCode::Backspace),
        ("delete", KeyCode::Delete),
        ("insert", KeyCode::Insert),
        ("home", KeyCode::Home),
        ("end", KeyCode::End),
        ("pageup", KeyCode::PageUp),
        ("pagedown", KeyCode::PageDown),
        ("left", KeyCode::Left),
        ("right", KeyCode::Right),
        ("up", KeyCode::Up),
        ("down", KeyCode::Down),
        ("null", KeyCode::Null),
    ];
    if let Some((_, code)) = named.iter().find(|(word, _)| *word == name) {
        return Some(*code);
    }
    if let Some(digits) = name.strip_prefix('f')
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && let Ok(n) = digits.parse::<u8>()
    {
        return Some(KeyCode::F(n));
    }
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(KeyCode::Char(c)),
        _ => None,
    }
}

impl Key {
    /// Canonical config spelling of this chord.
    #[must_use]
    pub fn to_config_string(self) -> String {
        let mut out = String::new();
        if self.mods.contains(KeyModifiers::CONTROL) {
            out.push_str("ctrl+");
        }
        if self.mods.contains(KeyModifiers::ALT) {
            out.push_str("alt+");
        }
        if self.mods.contains(KeyModifiers::SHIFT) {
            out.push_str("shift+");
        }
        out.push_str(&code_to_name(self.code));
        out
    }
}

fn code_to_name(code: KeyCode) -> String {
    match code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "enter".into(),
        KeyCode::Esc => "esc".into(),
        KeyCode::Tab => "tab".into(),
        KeyCode::BackTab => "backtab".into(),
        KeyCode::Backspace => "backspace".into(),
        KeyCode::Delete => "delete".into(),
        KeyCode::Insert => "insert".into(),
        KeyCode::Home => "home".into(),
        KeyCode::End => "end".into(),
        KeyCode::PageUp => "pageup".into(),
        KeyCode::PageDown => "pagedown".into(),
        KeyCode::Left => "left".into(),
        KeyCode::Right => "right".into(),
        KeyCode::Up => "up".into(),
        KeyCode::Down => "down".into(),
        KeyCode::F(n) => format!("f{n}"),
        KeyCode::Null => "null".into(),
        KeyCode::CapsLock
        | KeyCode::ScrollLock
        | KeyCode::NumLock
        | KeyCode::PrintScreen
        | KeyCode::Pause
        | KeyCode::Menu
        | KeyCode::KeypadBegin => "other".into(),
        KeyCode::Media(_) | KeyCode::Modifier(_) => "other".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, mods: KeyModifiers) -> Key {
        Key { code, mods }
    }

    #[test]
    fn defaults_bind_the_yan_gate_and_core_actions() {
        let binds = Keybinds::defaults();
        assert_eq!(
            binds.action(key(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(Action::Quit)
        );
        assert_eq!(
            binds.action(key(KeyCode::Char('y'), KeyModifiers::NONE)),
            Some(Action::AllowOnce)
        );
        assert_eq!(
            binds.action(key(KeyCode::Char('a'), KeyModifiers::NONE)),
            Some(Action::AllowSession)
        );
        assert_eq!(
            binds.action(key(KeyCode::Char('n'), KeyModifiers::NONE)),
            Some(Action::Deny)
        );
        assert_eq!(
            binds.action(key(KeyCode::Enter, KeyModifiers::NONE)),
            Some(Action::Submit)
        );
        assert_eq!(
            binds.action(key(KeyCode::Esc, KeyModifiers::NONE)),
            Some(Action::BlurOmnibar)
        );
        assert_eq!(
            binds.action(key(KeyCode::Char('r'), KeyModifiers::CONTROL)),
            Some(Action::Reconnect)
        );
    }

    #[test]
    fn chord_grammar_parses_modifiers_named_keys_and_chars() {
        assert_eq!(
            parse_chord("ctrl+alt+delete").map(|k| k.to_config_string()),
            Ok("ctrl+alt+delete".to_owned())
        );
        assert_eq!(
            parse_chord("f12").map(|k| k.to_config_string()),
            Ok("f12".to_owned())
        );
        assert_eq!(
            parse_chord("up").map(|k| k.to_config_string()),
            Ok("up".to_owned())
        );
        assert_eq!(
            parse_chord("X").map(|k| k.to_config_string()),
            Ok("X".to_owned())
        );
        assert!(matches!(
            parse_chord("hyper+x"),
            Err(KeybindError::UnknownKey(_))
        ));
        assert!(matches!(parse_chord(""), Err(KeybindError::UnknownKey(_))));
        assert!(matches!(
            parse_chord("ctrl+q+extra"),
            Err(KeybindError::UnknownKey(_))
        ));
    }

    #[test]
    fn toml_override_replaces_chord_for_named_action() {
        let mut binds = Keybinds::defaults();
        binds
            .load_overrides("[keys]\nquit = \"ctrl+q\"\n")
            .unwrap_or_default();
        assert_eq!(
            binds.action(key(KeyCode::Char('q'), KeyModifiers::CONTROL)),
            Some(Action::Quit)
        );
        assert_eq!(
            binds.action(key(KeyCode::Char('q'), KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn unknown_action_and_key_are_typed_errors() {
        let mut binds = Keybinds::defaults();
        assert_eq!(
            binds.load_overrides("[keys]\nwat = \"q\"\n"),
            Err(KeybindError::UnknownAction("wat".into()))
        );
        assert_eq!(
            binds.load_overrides("[keys]\nquit = \"nope\"\n"),
            Err(KeybindError::UnknownKey("nope".into()))
        );
    }

    #[test]
    fn duplicate_chord_across_actions_is_rejected_atomically() {
        let mut binds = Keybinds::defaults();
        let err = binds.load_overrides("[keys]\nreconnect = \"y\"\n");
        assert!(matches!(err, Err(KeybindError::Duplicate(_))));
        // Atomic: failed load leaves the previous layout intact.
        assert_eq!(
            binds.action(key(KeyCode::Char('y'), KeyModifiers::NONE)),
            Some(Action::AllowOnce)
        );
        assert_eq!(
            binds.action(key(KeyCode::Char('r'), KeyModifiers::CONTROL)),
            Some(Action::Reconnect)
        );
    }

    #[test]
    fn reverse_lookup_answers_chord_for_action() {
        let binds = Keybinds::defaults();
        let chord = binds.chord(Action::Deny);
        assert_eq!(chord.map(|k| k.code), Some(KeyCode::Char('n')));
    }
}
