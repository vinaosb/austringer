//! Stage-B tests: hashline anchors — content-hash of target line +
//! neighbors, drift detection, bounded re-anchor, typed rejection.

use harness_tools::LineTable;
use harness_tools::anchors::{AnchorStatus, HASH_NEIGHBORS, Hashline};

fn table(lines: usize) -> LineTable {
    let body: String = (1..=lines).map(|n| format!("line {n} payload\n")).collect();
    LineTable::from_string(body)
}

#[test]
fn hashline_verifies_unchanged_file_at_recorded_line() {
    let t = table(50);
    let anchor = Hashline::compute(&t, 20).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(anchor.line, 20);
    assert_eq!(anchor.before.len() + anchor.after.len(), 2 * HASH_NEIGHBORS);
    assert_eq!(anchor.verify(&t), AnchorStatus::Matched(20));
}

#[test]
fn hashline_reanchors_after_small_shift_above_target() {
    let t = table(50);
    let anchor = Hashline::compute(&t, 30).unwrap_or_else(|e| panic!("{e}"));
    // External edit inserted 3 lines above the target.
    let mut shifted = String::new();
    for n in 1..=3 {
        shifted.push_str(&format!("inserted {n}\n"));
    }
    for (line, content) in t.iter() {
        let _ = line;
        shifted.push_str(content);
        shifted.push('\n');
    }
    let shifted = LineTable::from_string(shifted);
    assert_eq!(
        anchor.verify(&shifted),
        AnchorStatus::ReAnchored { from: 30, to: 33 }
    );
}

#[test]
fn hashline_rejects_large_shift_typed_never_first_match() {
    let t = table(600);
    let anchor = Hashline::compute(&t, 500).unwrap_or_else(|e| panic!("{e}"));
    let mut shifted = String::new();
    for n in 1..=400 {
        shifted.push_str(&format!("noise {n}\n"));
    }
    for (_, content) in t.iter() {
        shifted.push_str(content);
        shifted.push('\n');
    }
    let shifted = LineTable::from_string(shifted);
    // The block still exists 400 lines down — outside the re-anchor radius
    // the anchor MUST reject (drift), never silently match the first
    // hash-equal line found anywhere.
    assert_eq!(
        anchor.verify(&shifted),
        AnchorStatus::Drifted { candidates: vec![] }
    );
}

#[test]
fn hashline_reanchor_ambiguity_is_typed_rejection() {
    // Three identical 10-line copies: anchoring mid-copy, then rewriting the
    // anchored line, leaves the re-anchor scan with two equally-plausible
    // candidates — rejection carries both, never a first-match pick.
    let mut text = String::new();
    for n in 1..=10 {
        text.push_str(&format!("prefix {n}\n"));
    }
    for _copy in 0..3 {
        for n in 1..=10 {
            text.push_str(&format!("line {n} payload\n"));
        }
    }
    let t = LineTable::from_string(text);
    let anchor = Hashline::compute(&t, 15).unwrap_or_else(|e| panic!("{e}"));
    let mut text = String::new();
    for (line, content) in t.iter() {
        if line == 15 {
            text.push_str("rewritten anchor line\n");
        } else {
            text.push_str(content);
            text.push('\n');
        }
    }
    let t2 = LineTable::from_string(text);
    assert_eq!(
        anchor.verify(&t2),
        AnchorStatus::Drifted {
            candidates: vec![25, 35]
        }
    );
}

#[test]
fn hashline_survives_crlf_reencoding_of_the_file() {
    // Hashes cover logical content (terminator-free): a wholesale CRLF
    // re-encode shifts zero lines and changes zero hashes.
    let t = table(30);
    let anchor = Hashline::compute(&t, 15).unwrap_or_else(|e| panic!("{e}"));
    let crlf: String = t.iter().map(|(_, c)| format!("{c}\r\n")).collect();
    let t2 = LineTable::from_string(crlf);
    assert_eq!(anchor.verify(&t2), AnchorStatus::Matched(15));
}

#[test]
fn hashline_compute_out_of_range_is_typed() {
    let t = table(10);
    let err = Hashline::compute(&t, 11).unwrap_err();
    assert!(
        matches!(
            err,
            harness_tools::ToolsError::LineOutOfRange {
                line: 11,
                total: 10
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn hashline_target_changed_is_drift_even_in_place() {
    // The recorded line now holds different content: no in-place match, and
    // nothing else in the file matches -> typed drift.
    let t = table(20);
    let anchor = Hashline::compute(&t, 10).unwrap_or_else(|e| panic!("{e}"));
    let mut text = String::new();
    for (line, content) in t.iter() {
        if line == 10 {
            text.push_str("totally rewritten line\n");
        } else {
            text.push_str(content);
            text.push('\n');
        }
    }
    let t2 = LineTable::from_string(text);
    assert_eq!(
        anchor.verify(&t2),
        AnchorStatus::Drifted { candidates: vec![] }
    );
}
