//! Allow-stripping sanitization (plan todo 8): while Auto mode is active,
//! the model's own permission-influencing phrasing must never reach the
//! classifier stages. [`sanitize`] strips every
//! [`ALLOW_STRIP_PHRASES`] occurrence from classifier inputs.
//!
//! Matching is char-wise and case-insensitive, so multi-byte input can
//! never misalign the output; stripped gaps collapse to single spaces.

/// Phrases stripped from classifier inputs: self-approval phrasing,
/// invented user consent, and instruction-injection openers.
pub const ALLOW_STRIP_PHRASES: &[&str] = &[
    "you may allow",
    "you can allow",
    "you may approve",
    "you can approve",
    "you are allowed to",
    "you're allowed to",
    "permission is granted",
    "permission granted",
    "already approved",
    "already allowed",
    "the user allows",
    "the user allowed",
    "it is safe to allow",
    "feel free to allow",
    "ignore previous instructions",
    "ignore all instructions",
    "disregard previous instructions",
    "bypass permissions",
    "yolo",
];

/// Strips every [`ALLOW_STRIP_PHRASES`] occurrence (case-insensitive)
/// from `input` and collapses the whitespace the removals leave behind.
#[must_use]
pub fn sanitize(input: &str) -> String {
    let mut chars: Vec<char> = input.chars().collect();
    for phrase in ALLOW_STRIP_PHRASES {
        chars = strip_all_ci(&chars, &phrase.chars().collect::<Vec<_>>());
    }
    let mut out = String::new();
    let mut pending_space = false;
    for ch in chars {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(ch);
        }
    }
    out.trim_end().to_owned()
}

fn chars_eq_ci(left: char, right: char) -> bool {
    left == right
        || left
            .to_lowercase()
            .zip(right.to_lowercase())
            .all(|(a, b)| a == b)
}

fn strip_all_ci(haystack: &[char], needle: &[char]) -> Vec<char> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return haystack.to_vec();
    }
    let mut out = Vec::with_capacity(haystack.len());
    let mut index = 0;
    while index < haystack.len() {
        if index + needle.len() <= haystack.len()
            && haystack[index..index + needle.len()]
                .iter()
                .zip(needle)
                .all(|(a, b)| chars_eq_ci(*a, *b))
        {
            index += needle.len();
        } else {
            out.push(haystack[index]);
            index += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_const_phrase_strips_case_insensitively() {
        for phrase in ALLOW_STRIP_PHRASES {
            let mixed: String = phrase
                .chars()
                .enumerate()
                .map(|(i, c)| {
                    if i % 2 == 0 {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    }
                })
                .collect();
            let sanitized = sanitize(&format!("x {mixed} y"));
            assert_eq!(sanitized, "x y", "phrase {phrase:?} survived");
        }
    }
}
