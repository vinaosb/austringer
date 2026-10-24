//! Tokenizer-level Python syntax validation for the mocktool blend
//! modes (plan M16 hermetic hardening: a syntactically broken builder
//! script must fail HERMETICALLY - the mock never executes bpy, but it
//! must parse Python).
//!
//! Honest bound: this is a lexer pass, not a full AST parse. It catches
//! the realistic breakage classes of a Rust-templated script -
//! unterminated strings (all quote forms), unbalanced or crossed
//! `()[]{}` delimiters, stray control bytes, and truncated files. A
//! full grammar parse would need either `python` on PATH (not
//! hermetic-guaranteed on CI) or a parser dependency (unsanctioned
//! weight for a test-support mock); the real `py_compile` equivalence
//! is exercised by the `#[ignore]` real-Blender tier.

/// Validate `source` as Python at the tokenizer level. `Err` carries an
/// actionable message with the 1-based line number.
pub fn validate_python(source: &str) -> Result<(), String> {
    let mut scanner = Scanner {
        bytes: source.as_bytes(),
        pos: 0,
        line: 1,
    };
    let mut stack: Vec<(u8, usize)> = Vec::new();
    while let Some(byte) = scanner.peek() {
        match byte {
            b'#' => scanner.skip_line(),
            b'\n' => {
                scanner.bump();
                scanner.line += 1;
            }
            b'\\' => {
                scanner.bump();
                scanner.bump();
            }
            b'\'' | b'"' => scanner.string(byte)?,
            b'(' | b'[' | b'{' => {
                stack.push((byte, scanner.line));
                scanner.bump();
            }
            b')' | b']' | b'}' => {
                let open = match byte {
                    b')' => b'(',
                    b']' => b'[',
                    _ => b'{',
                };
                match stack.pop() {
                    Some((expected, _)) if expected == open => {}
                    Some((expected, at)) => {
                        return Err(format!(
                            "line {}: '{}' closes the '{}' opened on line {at}",
                            scanner.line, byte as char, expected as char
                        ));
                    }
                    None => {
                        return Err(format!(
                            "line {}: '{}' with no matching open delimiter",
                            scanner.line, byte as char
                        ));
                    }
                }
                scanner.bump();
            }
            0..=8 | 14..=31 | 127 => {
                return Err(format!(
                    "line {}: control byte 0x{byte:02x} outside string/comment",
                    scanner.line
                ));
            }
            _ => scanner.bump(),
        }
    }
    if let Some((open, at)) = stack.pop() {
        return Err(format!(
            "line {at}: '{open}' is never closed",
            open = open as char
        ));
    }
    Ok(())
}

struct Scanner<'a> {
    bytes: &'a [u8],
    pos: usize,
    line: usize,
}

impl Scanner<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos + offset).copied()
    }

    fn bump(&mut self) {
        if self.pos < self.bytes.len() {
            self.pos += 1;
        }
    }

    fn skip_line(&mut self) {
        while let Some(byte) = self.peek()
            && byte != b'\n'
        {
            self.pos += 1;
        }
    }

    /// Consume one string literal starting at the opening quote. Escape
    /// sequences skip their following byte (which keeps `\<newline>`
    /// continuations inside single-quoted strings working); triple
    /// quotes may span lines (counting them).
    fn string(&mut self, quote: u8) -> Result<(), String> {
        let start_line = self.line;
        let triple = self.peek_at(1) == Some(quote) && self.peek_at(2) == Some(quote);
        let quotes = if triple { 3 } else { 1 };
        let repeats = if triple { 2 } else { 0 };
        for _ in 0..quotes {
            self.bump();
        }
        let mut seen = 0;
        while let Some(byte) = self.peek() {
            if byte == b'\\' {
                if !triple && self.peek_at(1) == Some(b'\n') {
                    self.line += 1;
                }
                self.bump();
                self.bump();
                continue;
            }
            if byte == b'\n' {
                self.line += 1;
                if !triple {
                    return Err(format!(
                        "line {start_line}: single-quoted string is never closed"
                    ));
                }
            }
            if byte == quote {
                seen += 1;
                self.bump();
                if seen > repeats {
                    return Ok(());
                }
                continue;
            }
            seen = 0;
            self.bump();
        }
        Err(format!(
            "line {start_line}: string literal is never closed (end of script)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::validate_python;

    #[test]
    fn valid_python_passes() {
        let source = "def f(x):\n    return {'a': [1, 2], \"b\": (3,)}\ns = \"\"\"multi\nline\"\"\"\nr = r'C:\\path'\nt = 'esc\\'aped'\n";
        assert!(validate_python(source).is_ok());
    }

    #[test]
    fn unterminated_string_fails_with_line() {
        let error = validate_python("x = 1\ns = 'open\n").expect_err("unterminated");
        assert!(error.contains("never closed"), "{error}");
    }

    #[test]
    fn unbalanced_delimiters_fail() {
        assert!(validate_python("def broken(:\n    pass\n").is_err());
        assert!(validate_python("list = [1, (2])\n").is_err());
        assert!(validate_python("close = )\n").is_err());
        assert!(validate_python("value = {\n").is_err());
    }
}
