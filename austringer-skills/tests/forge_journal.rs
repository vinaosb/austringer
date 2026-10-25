//! Cycle-1 correction journal: hunk rejections, permission denials,
//! post-output edits; SECRET REDACTION AT CAPTURE TIME (never store
//! keys/tokens); corrupt journal files are typed errors.

use std::fs;
use std::path::Path;

use austringer_skills::forge::journal::{CorrectionJournal, CorrectionSignal, RedactionFilter};

#[test]
fn secrets_are_redacted_at_capture_time_before_storage() {
    let filter = RedactionFilter::new(&["SUPERSECRET-PROJECT-NAME"]);
    let mut journal = CorrectionJournal::new(filter);

    let before = "rejected hunk in deploy.sh: curl -H 'Authorization: Bearer eyJhbGciOi.payload.sig' https://api\nkey = sk-proj-4xK9mQzT3vBnR8wLp2sHf\npat = ghp_AbCdEfGh1234567890\naws = AKIAIOSFODNN7EXAMPLE\nproject SUPERSECRET-PROJECT-NAME pinned";
    let entry = journal.capture(
        CorrectionSignal::HunkRejected {
            target: "deploy.sh".to_string(),
            feedback: before.to_string(),
        },
        1_000,
    );
    // BEFORE (raw input) vs AFTER (what the journal stores) — the QA proof pair.
    assert!(before.contains("sk-proj-4xK9mQzT3vBnR8wLp2sHf"));
    assert!(before.contains("Bearer eyJhbGciOi.payload.sig"));
    let stored = &journal.entries()[0];
    let stored_text = match &stored.signal {
        CorrectionSignal::HunkRejected { feedback, .. } => feedback,
        other => panic!("wrong signal stored: {other:?}"),
    };
    assert!(
        !stored_text.contains("sk-proj-")
            && !stored_text.contains("Bearer eyJ")
            && !stored_text.contains("ghp_AbCdEfGh")
            && !stored_text.contains("AKIAIOSFODNN7EXAMPLE")
            && !stored_text.contains("SUPERSECRET-PROJECT-NAME"),
        "no secret material survives capture: {stored_text:?}"
    );
    assert!(
        stored_text.contains("[REDACTED"),
        "redaction is visible, not silent truncation: {stored_text:?}"
    );
    assert!(
        entry.redactions >= 5,
        "every secret class redacted: {entry:?}"
    );
}

#[test]
fn benign_feedback_passes_through_untouched() {
    let mut journal = CorrectionJournal::new(RedactionFilter::new(&[]));
    journal.capture(
        CorrectionSignal::PermissionDenied {
            tool: "Bash(rm:-rf)".to_string(),
            reason: "destructive command denied by policy".to_string(),
        },
        1_001,
    );
    journal.capture(
        CorrectionSignal::PostOutputEdit {
            target: "README.md".to_string(),
            edit_summary: "user softened the tone of the intro paragraph".to_string(),
        },
        1_002,
    );
    assert_eq!(journal.entries().len(), 2);
    assert_eq!(journal.entries()[0].captured_unix, 1_000 + 1);
    let rendered = journal.render();
    assert!(rendered.contains("destructive command denied"));
    assert!(rendered.contains("post-output edit"), "{rendered}");
    assert!(rendered.contains("README.md"));
}

#[test]
fn journal_round_trips_and_corrupt_files_are_typed_errors() {
    let tmp = tempfile::tempdir().expect("tmp");
    let path = tmp.path().join("journal.json");
    let filter = RedactionFilter::new(&[]);
    let mut journal = CorrectionJournal::new(filter.clone());
    journal.capture(
        CorrectionSignal::HunkRejected {
            target: "a.rs".to_string(),
            feedback: "wrong import order".to_string(),
        },
        42,
    );
    journal.save(&path).expect("save");
    let loaded = CorrectionJournal::load(&path, filter).expect("load");
    assert_eq!(loaded.entries().len(), 1);

    fs::write(&path, "{ not json").expect("corrupt");
    let err = CorrectionJournal::load(&path, RedactionFilter::new(&[])).expect_err("corrupt");
    assert!(
        matches!(
            err,
            austringer_skills::forge::ForgeError::JournalCorrupt { .. }
        ),
        "{err:?}"
    );

    let err = CorrectionJournal::load(
        Path::new("Z:/definitely/not/here/journal.json"),
        RedactionFilter::new(&[]),
    );
    // A missing journal loads empty (first run), never aborts the loop.
    assert!(err.is_ok(), "missing journal is an empty journal: {err:?}");
}
