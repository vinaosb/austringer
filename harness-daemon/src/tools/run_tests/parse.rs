//! Output parsers for the day-one frameworks (todo 17): nextest's
//! human rendering (per-test statuses WITH durations) and libtest's
//! stable text format (per-test statuses; per-test durations do not
//! exist there — only the suite total). Both shapes below are pinned
//! by unit tests against REAL captured output (nextest 0.9.144,
//! stable libtest), not paraphrase.
//!
//! nextest human shape (stderr):
//!
//! ```text
//!         FAIL [   0.051s] (1/2) rt_fixture fails
//!         PASS [   0.070s] (2/2) rt_fixture passes
//!      Summary [   0.070s] 2 tests run: 1 passed, 1 failed, 1 skipped
//! ```
//!
//! (`Summary` and everything after it — the failure recap — is not
//! per-test output; parsing stops there so recap lines never
//! double-count.)
//!
//! libtest shape (stdout):
//!
//! ```text
//! running 3 tests
//! test skips ... ignored
//! test passes ... ok
//! test fails ... FAILED
//! test result: FAILED. 1 passed; 1 failed; 1 ignored; 0 measured; ...
//! ```

use std::borrow::Cow;

use super::report::TestEntry;

/// Parses nextest's human per-test lines. The status vocabulary is
/// nextest's own (`PASS`, `FAIL`, `SKIP`, `ABORT`, `TIMEOUT`);
/// unknown status words are skipped rather than guessed.
pub(super) fn parse_nextest_human(text: &str) -> Vec<TestEntry> {
    let mut entries = Vec::new();
    for raw in text.lines() {
        let line = strip_ansi(raw.trim_end_matches('\r'));
        if line.trim_start().starts_with("Summary [") {
            // The failure recap follows the summary; entries above it
            // are the authoritative per-test record.
            break;
        }
        let Some(entry) = parse_nextest_line(&line) else {
            continue;
        };
        entries.push(entry);
    }
    entries
}

fn parse_nextest_line(line: &str) -> Option<TestEntry> {
    let trimmed = line.trim_start();
    let (status, rest) = trimmed.split_once(' ')?;
    let status = match status {
        "PASS" => "passed",
        "SKIP" => "ignored",
        "FAIL" | "ABORT" | "TIMEOUT" => "failed",
        _ => return None,
    };
    // `[   0.051s] (1/2) <binary> <test name>`
    let rest = rest.strip_prefix('[')?;
    let (duration_text, rest) = rest.split_once(']')?;
    let duration_ms = seconds_to_ms(duration_text.trim())?;
    let rest = rest.trim_start().strip_prefix('(')?;
    let (_, rest) = rest.split_once(") ")?;
    let name = rest.trim().to_owned();
    if name.is_empty() {
        return None;
    }
    Some(TestEntry {
        name,
        status: status.to_owned(),
        duration_ms: Some(duration_ms),
    })
}

/// Parses libtest's per-test result lines. Stable across toolchains
/// (the `--format json` variant is nightly-only); durations are
/// structurally absent per-test on this lane.
pub(super) fn parse_libtest(text: &str) -> Vec<TestEntry> {
    let mut entries = Vec::new();
    for raw in text.lines() {
        let line = strip_ansi(raw.trim_end_matches('\r'));
        let Some(rest) = line.strip_prefix("test ") else {
            continue;
        };
        // The `test result:` summary line also starts with "test ".
        if rest.starts_with("result:") {
            continue;
        }
        let Some((name, status)) = rest.rsplit_once(" ... ") else {
            continue;
        };
        let status = match status.trim() {
            "ok" => "passed",
            "FAILED" => "failed",
            "ignored" => "ignored",
            _ => continue,
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        entries.push(TestEntry {
            name: name.to_owned(),
            status: status.to_owned(),
            duration_ms: None,
        });
    }
    entries
}

/// Strips ANSI escape sequences from one output line before matching.
/// Defense-in-depth: the spawn seam forces `NO_COLOR=1` + `TERM=dumb`,
/// but a runner that colorizes anyway (CI runners color non-tty
/// output) must not break line matching. Covers CSI sequences
/// (SGR color/cursor, e.g. `ESC[35mESC[1m`) and drops other
/// two-character escape introducers.
fn strip_ansi(line: &str) -> Cow<'_, str> {
    if !line.contains('\u{1b}') {
        return Cow::Borrowed(line);
    }
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        if chars.next() == Some('[') {
            // CSI: parameter/intermediate bytes, then one final byte
            // in 0x40..=0x7E terminates the sequence.
            for b in chars.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&b) {
                    break;
                }
            }
        }
    }
    Cow::Owned(out)
}

/// `0.051s`-style seconds to whole milliseconds (rounded).
fn seconds_to_ms(text: &str) -> Option<u64> {
    let seconds: f64 = text.strip_suffix('s')?.parse().ok()?;
    Some((seconds * 1000.0).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEXTEST_HUMAN: &str = "\
 Starting 2 tests across 1 binary (1 test skipped)
        FAIL [   0.051s] (1/2) rt_fixture fails
  stdout ─────────────────────────────────────────

    running 1 test
    test fails ... FAILED

    failures:

    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s

  stderr ─────────────────────────────────────────

    thread 'fails' panicked at src\\lib.rs:5:14:
    fixture failure 51f2

        PASS [   0.070s] (2/2) rt_fixture passes
─────────────────────────────────────────────────
     Summary [   0.070s] 2 tests run: 1 passed, 1 failed, 1 skipped
        FAIL [   0.051s] (1/2) rt_fixture fails
error: test run failed
";

    #[test]
    fn nextest_human_parses_statuses_durations_and_skips_recap() {
        let entries = parse_nextest_human(NEXTEST_HUMAN);
        assert_eq!(entries.len(), 2, "the recap FAIL must not double-count");
        let failing = &entries[0];
        assert_eq!(failing.name, "rt_fixture fails");
        assert_eq!(failing.status, "failed");
        assert_eq!(failing.duration_ms, Some(51));
        let passing = &entries[1];
        assert_eq!(passing.name, "rt_fixture passes");
        assert_eq!(passing.status, "passed");
        assert_eq!(passing.duration_ms, Some(70));
        // The embedded libtest block inside the FAIL stdout section
        // never contributes entries (different line grammar).
        assert!(!entries.iter().any(|entry| entry.name == "fails"));
    }

    #[test]
    fn nextest_human_ignores_non_test_lines() {
        let entries = parse_nextest_human(
            " Nextest run ID 8a9b2c80 with nextest profile: default\n    Starting 2 tests across 1 binary\n",
        );
        assert!(entries.is_empty());
    }

    // The PR-1 CI failure: runners colorize nextest's piped (non-tty)
    // human output — exact escape codes from the CI log — and the
    // parser contracted on plain text, yielding "no parseable per-test
    // results" on all three OSes. The parser strips ANSI as
    // defense-in-depth; run.rs additionally forces NO_COLOR=1 +
    // TERM=dumb at the spawn seam.
    const NEXTEST_HUMAN_ANSI: &str = "\
\u{1b}[1m\u{1b}[92m   Compiling\u{1b}[0m rtfx_direct v0.1.0
\u{1b}[31m\u{1b}[1m        FAIL\u{1b}[0m \u{1b}[1m[   0.051s]\u{1b}[0m \u{1b}[2m(1/2)\u{1b}[0m \u{1b}[35m\u{1b}[1mrtfx_direct\u{1b}[0m \u{1b}[34m\u{1b}[1mfails\u{1b}[0m
\u{1b}[32m\u{1b}[1m        PASS\u{1b}[0m \u{1b}[1m[   0.070s]\u{1b}[0m \u{1b}[2m(2/2)\u{1b}[0m \u{1b}[35m\u{1b}[1mrtfx_direct\u{1b}[0m \u{1b}[34m\u{1b}[1mpasses\u{1b}[0m
\u{1b}[1m     Summary\u{1b}[0m \u{1b}[1m[   0.070s]\u{1b}[0m 2 tests run: 1 passed, 1 failed, 0 skipped
";

    #[test]
    fn nextest_human_parses_ansi_wrapped_lines() {
        let entries = parse_nextest_human(NEXTEST_HUMAN_ANSI);
        assert_eq!(
            entries.len(),
            2,
            "ANSI-wrapped per-test lines must parse (the CI failure): {entries:?}"
        );
        assert_eq!(entries[0].name, "rtfx_direct fails");
        assert_eq!(entries[0].status, "failed");
        assert_eq!(entries[0].duration_ms, Some(51));
        assert_eq!(entries[1].name, "rtfx_direct passes");
        assert_eq!(entries[1].status, "passed");
        assert_eq!(entries[1].duration_ms, Some(70));
    }

    #[test]
    fn libtest_text_parses_ansi_wrapped_lines() {
        let entries = parse_libtest(
            "running 1 test\ntest \u{1b}[32m\u{1b}[1mpasses\u{1b}[0m ... \u{1b}[32m\u{1b}[1mok\u{1b}[0m\n",
        );
        assert_eq!(entries.len(), 1, "ANSI-wrapped libtest lines must parse");
        assert_eq!(entries[0].name, "passes");
        assert_eq!(entries[0].status, "passed");
    }

    const LIBTEST_TEXT: &str = "
running 3 tests
test skips ... ignored
test passes ... ok
test fails ... FAILED

failures:

---- fails stdout ----

thread 'fails' panicked at src\\lib.rs:5:14:
fixture failure 51f2
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    fails

test result: FAILED. 1 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
";

    #[test]
    fn libtest_text_parses_all_three_statuses() {
        let entries = parse_libtest(LIBTEST_TEXT);
        assert_eq!(entries.len(), 3, "exactly the three result lines");
        assert_eq!(entries[0].name, "skips");
        assert_eq!(entries[0].status, "ignored");
        assert_eq!(entries[0].duration_ms, None);
        assert_eq!(entries[1].name, "passes");
        assert_eq!(entries[1].status, "passed");
        assert_eq!(entries[2].name, "fails");
        assert_eq!(entries[2].status, "failed");
    }

    #[test]
    fn libtest_text_ignores_summary_and_panic_lines() {
        let entries = parse_libtest("test result: ok. 0 passed; 0 failed; 0 ignored;\n");
        assert!(entries.is_empty());
        let entries = parse_libtest("running 1 test\r\ntest passes ... ok\r\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "passes");
    }
}
