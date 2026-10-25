//! Encryption-at-rest end to end: wrong keys, ciphertext on disk,
//! re-keyed forks, FTS exclusion.

use austringer_core::event::{ContentBlock, Event, Sequence};
use austringer_sessions::{
    ForkOptions, KdfParams, LeaseOwner, OpenMode, SessionError, SessionStore,
};

fn user(text: &str) -> Event {
    Event::User {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
    }
}

#[test]
fn wrong_key_ciphertext_and_typed_errors_end_to_end() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    store
        .create_encrypted(
            "sealed",
            "secret work",
            "correct horse",
            KdfParams::test_low(),
        )
        .unwrap();
    let owner = LeaseOwner::new("w");
    {
        let h = store
            .open_session_unlocked("sealed", "correct horse", OpenMode::ReadWrite(owner))
            .unwrap();
        h.append(user("the launch code is 0000")).unwrap();
    }

    // Opening without a passphrase is typed.
    assert!(matches!(
        store.open_session("sealed"),
        Err(SessionError::MissingPassphrase { .. })
    ));
    // Wrong passphrase is a clean decrypt error, never a panic, never
    // partial data.
    let wrong = store
        .open_session_unlocked("sealed", "battery staple", OpenMode::ReadOnly)
        .unwrap_err();
    assert!(matches!(wrong, SessionError::Decrypt { .. }));

    // Cryptographic assert: the stored payload is ciphertext — the
    // plaintext appears nowhere in the database file bytes.
    let raw = std::fs::read(root.path().join("store.db")).unwrap();
    let plaintext = b"the launch code is 0000";
    assert!(
        !raw.windows(plaintext.len()).any(|w| w == plaintext),
        "plaintext must not appear in the database file"
    );

    // Right passphrase round-trips the content.
    let ro = store
        .open_session_unlocked("sealed", "correct horse", OpenMode::ReadOnly)
        .unwrap();
    let events = ro.events_range(Sequence(1), Sequence(1)).unwrap();
    assert_eq!(events[0].event, user("the launch code is 0000"));
}

#[test]
fn encrypted_fork_rekeys_under_the_forks_own_passphrase() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    store
        .create_encrypted("src", "", "original-pw", KdfParams::test_low())
        .unwrap();
    let owner = LeaseOwner::new("w");
    {
        let h = store
            .open_session_unlocked("src", "original-pw", OpenMode::ReadWrite(owner))
            .unwrap();
        h.append(user("shared secret")).unwrap();
        h.append(user("to be dropped")).unwrap();
    }
    let fork_meta = store
        .fork(
            "fork-enc",
            "src",
            Sequence(1),
            &ForkOptions {
                unlock_passphrase: Some("original-pw".to_owned()),
                new_passphrase: Some("fork-pw".to_owned()),
                kdf: Some(KdfParams::test_low()),
                title: None,
            },
        )
        .unwrap();
    assert_eq!(fork_meta.event_count, 1);
    assert!(fork_meta.encrypted);
    let ro = store
        .open_session_unlocked("fork-enc", "fork-pw", OpenMode::ReadOnly)
        .unwrap();
    let events = ro.events_range(Sequence(1), Sequence(1)).unwrap();
    assert_eq!(events[0].event, user("shared secret"));
    // The fork's key is independent: the source passphrase cannot open it.
    assert!(matches!(
        store.open_session_unlocked("fork-enc", "original-pw", OpenMode::ReadOnly),
        Err(SessionError::Decrypt { .. })
    ));
}

#[test]
fn encrypted_session_never_enters_the_fts_index() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    store
        .create_encrypted("vault", "", "passphrase", KdfParams::test_low())
        .unwrap();
    let owner = LeaseOwner::new("w");
    let h = store
        .open_session_unlocked("vault", "passphrase", OpenMode::ReadWrite(owner))
        .unwrap();
    h.append(user("secret searchterm")).unwrap();
    assert!(store.search_events("searchterm", 10).unwrap().is_empty());
}
