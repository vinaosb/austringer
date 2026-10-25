//! LSP position math: UTF-16 ↔ character-index conversion and line lookup.
//!
//! The LSP wire format defaults to UTF-16 code-unit columns; the austringer
//! negotiates UTF-8 when the server honors it and converts server-reported
//! columns through the file's own text otherwise. All functions here treat
//! their inputs as untrusted data and clamp instead of panicking.

/// Converts a 0-based UTF-16 code-unit column on `line` into a 0-based
/// character index. Clamps to the line length when the input overshoots
/// (surrogate-pair arithmetic off a hostile server never panics).
#[must_use]
pub fn utf16_col_to_utf8(line: &str, utf16_col: usize) -> u32 {
    let mut units = 0usize;
    for (char_index, ch) in line.chars().enumerate() {
        if units >= utf16_col {
            return char_index as u32;
        }
        units += ch.len_utf16();
    }
    line.chars().count() as u32
}

/// Returns the 0-based `line` from `text` ( `\n`-separated, `\r` trimmed),
/// or an empty string when the index overshoots.
#[must_use]
pub fn line_at(text: &str, zero_based_line: usize) -> &str {
    text.split('\n')
        .nth(zero_based_line)
        .map(|line| line.trim_end_matches('\r'))
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_at_trims_crlf_and_clamps() {
        let text = "alpha\r\nbeta\r\ngamma";
        assert_eq!(line_at(text, 0), "alpha");
        assert_eq!(line_at(text, 2), "gamma");
        assert_eq!(line_at(text, 9), "");
    }

    #[test]
    fn ascii_columns_are_identity() {
        assert_eq!(utf16_col_to_utf8("plain ascii", 5), 5);
    }
}
