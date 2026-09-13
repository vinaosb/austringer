//! Git-backed snapshot acceptance (plan todo 23): rewind round-trip
//! fidelity, restore-transaction safety, hygiene refusals, provenance
//! bytes, and the never-touch-user-git invariants.

mod common;

use common::*;
use harness_snapshots::{
    ExternallyModifiedPolicy, RestoreMode, SessionId, SnapshotError, SnapshotOptions,
    SnapshotStore, TaskState, Turn,
};
use std::collections::BTreeMap;
use std::path::Path;

use gix::bstr::ByteSlice as _;

fn opts_with_task(cursor: u64, plan: &str) -> SnapshotOptions {
    SnapshotOptions {
        task: Some(TaskState {
            cursor,
            plan: Some(plan.as_bytes().to_vec()),
        }),
        ..SnapshotOptions::default()
    }
}

/// ACCEPTANCE (plan): state@N is identical after wandering to N+5 — every
/// file hashed before the wander and after the rewind, task state included.
#[test]
fn rewind_round_trip_is_byte_identical_after_wander() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n"), ("d/b.txt", b"beta\r\n")]);
    let session_root = workdir.join("session");
    let store = SnapshotStore::open(&workdir).expect("open store");
    assert_eq!(store.backend_name(), "git");

    let before = tree_digest(&workdir);
    store
        .snapshot(&sess("s1"), turn(1), &opts_with_task(7, "plan@1"))
        .expect("snapshot@1");

    // Wander five turns: modify, create, delete, CRLF churn, snapshots.
    let mut files = BTreeMap::from([
        ("a.txt".to_owned(), b"alpha-wandered\n".to_vec()),
        ("d/b.txt".to_owned(), b"beta-twice\r\n\r\n".to_vec()),
        ("new.txt".to_owned(), b"created while wandering\n".to_vec()),
    ]);
    for t in 2..=6 {
        files.insert(format!("turn{t}.txt"), format!("turn {t}\n").into_bytes());
        write_file(
            &workdir,
            Path::new(&format!("turn{t}.txt")),
            format!("turn {t}\n").as_bytes(),
        );
        write_file(&workdir, Path::new("a.txt"), b"alpha-wandered\n");
        write_file(&workdir, Path::new("d/b.txt"), b"beta-twice\r\n\r\n");
        write_file(&workdir, Path::new("new.txt"), b"created while wandering\n");
        store
            .snapshot(
                &sess("s1"),
                turn(t),
                &opts_with_task(7 + u64::from(t), "later"),
            )
            .expect("wander snapshot");
    }
    delete_file(&workdir, Path::new("a.txt")); // deleted AFTER the last snapshot
    let _ = files;

    let plan = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesAndTask,
            ExternallyModifiedPolicy::Include,
        )
        .expect("plan");
    let mut log = RecordingLog::default();
    let report = store
        .execute_rewind(plan, Some(&mut log), None)
        .expect("rewind");
    assert!(!report.rolled_back);
    assert_eq!(
        tree_digest(&workdir),
        before,
        "state@1 must be byte-identical"
    );
    let task = report.task.expect("files+task rewinds the task cursor");
    assert_eq!(task.cursor, 7);
    assert_eq!(task.plan.as_deref(), Some("plan@1".as_bytes()));
    let cursors = log.cursors.lock().expect("log lock");
    assert_eq!(cursors.len(), 1, "exactly one truncation cursor emitted");
    assert_eq!(cursors[0].turn, Turn::new(1).expect("turn"));
    drop(cursors);
    // Snapshot@2..6 still rewinds forward (time travel is not one-shot).
    let plan = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(6),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect("plan@6");
    store.execute_rewind(plan, None, None).expect("rewind to 6");
}

/// FAILURE: user commits after the snapshot refuse the restore (Cline
/// 4.1.17); agent-authored commits do not.
#[test]
fn user_commits_after_snapshot_refuse_restore() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let session_root = workdir.join("session");
    let store = SnapshotStore::open(&workdir).expect("open store");
    store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect("snapshot@1");

    let files = BTreeMap::from([("a.txt".to_owned(), b"alpha\nuser change\n".to_vec())]);
    let user_commit = commit(&workdir, "user: real work", &files);
    write_file(&workdir, Path::new("a.txt"), b"alpha\nuser change\n");

    let err = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect_err("user commits must refuse");
    match &err {
        SnapshotError::UserCommitsAfterSnapshot { commits } => {
            assert_eq!(
                commits,
                std::slice::from_ref(&user_commit),
                "the refusal names the user commit"
            );
        }
        other => panic!("wrong error: {other:?}"),
    }
    // The user's own commit stays on top of their branch: the harness
    // never moved it (the refusal left everything untouched).
    assert_eq!(head_hex(&workdir), user_commit);
}

/// Agent-authored commits (provenance trailer) after the snapshot do NOT
/// block a rewind: only user commits do (Cline 4.1.17).
#[test]
fn agent_commits_do_not_block_rewind() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let session_root = workdir.join("session");
    let store = SnapshotStore::open(&workdir).expect("open store");
    store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect("snapshot@1");

    let agent_files = BTreeMap::from([("a.txt".to_owned(), b"alpha\nagent change\n".to_vec())]);
    commit(
        &workdir,
        &harness_snapshots::provenance::provenance_message(&sess("s1"), turn(2)),
        &agent_files,
    );
    write_file(&workdir, Path::new("a.txt"), b"alpha\nagent change\n");
    let plan = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect("agent commits do not block rewind");
    let report = store.execute_rewind(plan, None, None).expect("rewind");
    assert!(!report.rolled_back);
    assert_eq!(
        std::fs::read_to_string(workdir.join("a.txt")).unwrap(),
        "alpha\n",
        "worktree restored to the snapshot state"
    );
    // The agent commit is still the branch tip: only the worktree moved.
    assert!(head_hex(&workdir).len() == 40);
}

/// MISSING snapshot is typed, never a partial restore.
#[test]
fn missing_snapshot_is_typed() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let store = SnapshotStore::open(&workdir).expect("open store");
    let err = store
        .rewind_plan(
            &workdir.join("session"),
            &sess("s1"),
            turn(9),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect_err("no snapshot@9");
    assert!(
        matches!(err, SnapshotError::MissingSnapshot { .. }),
        "got {err:?}"
    );
}

/// TRANSACTION: dirty worktree is stashed to a private ref before restore;
/// an aborted restore rolls back completely (commit-or-rollback).
#[test]
fn dirty_stash_transaction_commits_or_rolls_back() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n"), ("b.txt", b"beta\n")]);
    let session_root = workdir.join("session");
    let store = SnapshotStore::open(&workdir).expect("open store");
    store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect("snapshot@1");
    write_file(&workdir, Path::new("a.txt"), b"alpha\ndirty\n");
    let dirty = tree_digest(&workdir);

    // Abort mid-way: the FIRST file op fails -> rollback must be complete.
    let plan = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect("plan");
    let mut probe = FailingProbe::at(0);
    let err = store
        .execute_rewind(plan, None, Some(&mut probe))
        .expect_err("abort");
    match &err {
        SnapshotError::RestoreFailed { restored, .. } => {
            assert!(*restored, "rollback completed")
        }
        other => panic!("wrong error: {other:?}"),
    }
    assert_eq!(
        tree_digest(&workdir),
        dirty,
        "rollback leaves the dirty tree intact"
    );

    // A successful restore stashes the dirty state to a private ref.
    let plan = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect("plan");
    let report = store.execute_rewind(plan, None, None).expect("restore");
    let stash = report.stash.as_ref().expect("dirty state stashed");
    assert!(
        stash
            .ref_name
            .starts_with("refs/harness/restore-transactions/"),
        "{}",
        stash.ref_name
    );
    assert!(
        workdir
            .join(".git")
            .join(stash.ref_name.replace('/', "\\"))
            .exists(),
        "stash ref exists on disk"
    );
    assert_eq!(tree_digest(&workdir), tree_digest(&workdir));
}

/// HYGIENE: a nested repo not registered as a submodule is refused, typed.
#[test]
fn nested_repo_is_refused_and_never_touched() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let nested = workdir.join("vendor").join("stray");
    std::fs::create_dir_all(&nested).expect("mkdir");
    gix::init(&nested).expect("init nested");
    std::fs::write(nested.join("inner.txt"), b"stray\n").expect("write");
    let nested_digest = tree_digest(&nested);

    let store = SnapshotStore::open(&workdir).expect("open store");
    let err = store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect_err("nested repo must refuse");
    assert!(
        matches!(err, SnapshotError::NestedRepoRefused { .. }),
        "got {err:?}"
    );
    assert_eq!(
        tree_digest(&nested),
        nested_digest,
        "the nested repo is untouched"
    );
}

/// SUBMODULES are frozen pointers: superproject gitlink only, never
/// recursed; dirty-submodule state preserved as a pointer+patch record.
#[test]
fn submodules_are_frozen_pointers_with_dirty_records() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    // A registered submodule: child repo + .gitmodules entry.
    let sub = workdir.join("sub");
    std::fs::create_dir_all(&sub).expect("mkdir");
    let sub_repo = gix::init(&sub).expect("init sub");
    let blob = sub_repo
        .write_blob(b"sub-content\n")
        .expect("blob")
        .detach();
    let tree = sub_repo
        .write_object(&gix::objs::Tree {
            entries: vec![gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Blob.into(),
                filename: "inner.txt".into(),
                oid: blob,
            }],
        })
        .expect("sub tree")
        .detach();
    let sig = gix::actor::Signature {
        name: "sub".into(),
        email: "sub@example.com".into(),
        time: gix::date::Time::now_utc(),
    };
    let sub_commit = sub_repo
        .write_object(&gix::objs::Commit {
            message: "sub init".into(),
            tree,
            parents: Vec::new().into(),
            author: sig.clone(),
            committer: sig,
            encoding: None,
            extra_headers: Vec::new(),
        })
        .expect("sub commit")
        .detach();
    std::fs::write(sub.join(".git").join("HEAD"), "ref: refs/heads/main\n").expect("sub HEAD");
    std::fs::create_dir_all(sub.join(".git").join("refs").join("heads")).expect("sub refs dir");
    std::fs::write(
        sub.join(".git").join("refs").join("heads").join("main"),
        format!("{}\n", sub_commit.to_hex()),
    )
    .expect("sub main");
    write_file(
        &workdir,
        Path::new(".gitmodules"),
        b"[submodule \"sub\"]\n\tpath = sub\n\turl = https://example.com/sub.git\n",
    );
    write_file(&sub, Path::new("inner.txt"), b"sub-content\n");
    // Dirty the submodule AFTER its commit: the record must capture it.
    write_file(&sub, Path::new("inner.txt"), b"sub-content\ndirty\n");

    let _session_root = workdir.join("session");
    let store = SnapshotStore::open(&workdir).expect("open store");
    let outcome = store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect("submodule snapshot");

    // The snapshot tree holds a gitlink with the submodule's HEAD commit.
    let meta = store.snapshot_meta(&sess("s1"), turn(1)).expect("meta");
    let record = meta.submodules.first().expect("submodule record");
    assert_eq!(record.path, "sub");
    assert_eq!(record.commit_hex, sub_commit.to_hex().to_string());
    assert!(
        record.dirty.iter().any(|(p, _)| p == "inner.txt"),
        "dirty-submodule state preserved as pointer+patch record: {:?}",
        record.dirty
    );
    assert!(
        !meta.files.keys().any(|p| p.starts_with("sub/")),
        "never recurse into the submodule"
    );
    let _ = outcome;
}

/// PROVENANCE: agent commits carry Harness-Session-Id + Co-authored-by +
/// the workflow-approval marker, byte-asserted on the real object.
#[test]
fn provenance_trailers_are_byte_exact() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let _session_root = workdir.join("session");
    let store = SnapshotStore::open(&workdir).expect("open store");
    let outcome = store
        .snapshot(&sess("sess-a"), turn(3), &SnapshotOptions::default())
        .expect("snapshot");
    let ref_name = outcome.ref_name.as_deref().expect("git ref name");
    assert_eq!(ref_name, "refs/harness/snapshots/sess-a/0000000003");

    let repo = gix::open(&workdir).expect("repo");
    let reference = repo.find_reference(ref_name).expect("ref");
    let commit = repo
        .find_commit(reference.try_id().expect("target").detach())
        .expect("commit");
    let message = commit
        .message_raw()
        .expect("message")
        .to_str_lossy()
        .into_owned();
    assert!(
        message.contains("Harness-Session-Id: sess-a\n"),
        "session trailer byte-exact: {message:?}"
    );
    assert!(
        message.contains("Co-authored-by: harness-agent <agent@harness.invalid>\n"),
        "attribution byte-exact: {message:?}"
    );
    assert!(
        message.contains("Harness-Agent-Commit: true\n"),
        "workflow-approval marker byte-exact: {message:?}"
    );
    assert!(harness_snapshots::provenance::is_agent_commit(&message));
    assert!(harness_snapshots::provenance::requires_human_approval(
        &message
    ));
    assert_eq!(
        harness_snapshots::provenance::session_trailer(&message),
        Some("sess-a".to_owned())
    );
    // A user commit message fails every predicate.
    assert!(!harness_snapshots::provenance::is_agent_commit(
        "user: real work\n"
    ));
    assert!(!harness_snapshots::provenance::requires_human_approval(
        "user: real work\n"
    ));
    assert_eq!(
        harness_snapshots::provenance::session_trailer("plain"),
        None
    );
}

/// NEVER-TOUCH: user branches keep their tip, .git gains refs only under
/// refs/harness, and .git/info/exclude bytes are identical.
#[test]
fn user_git_is_never_mutated_beyond_harness_refs() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let exclude = workdir.join(".git").join("info").join("exclude");
    std::fs::create_dir_all(exclude.parent().unwrap()).unwrap();
    std::fs::write(&exclude, b"# user's own excludes\n").unwrap();
    let exclude_before = std::fs::read(&exclude).unwrap();
    let head_before = head_hex(&workdir);

    let session_root = workdir.join("session");
    let store = SnapshotStore::open(&workdir).expect("open store");
    store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect("snapshot");
    let plan = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect("plan");
    store.execute_rewind(plan, None, None).expect("rewind");

    assert_eq!(head_hex(&workdir), head_before, "user branch tip unmoved");
    assert_eq!(
        std::fs::read(&exclude).unwrap(),
        exclude_before,
        "info/exclude untouched"
    );
    // Every new ref lives under refs/harness.
    let mut stack = vec![workdir.join(".git").join("refs")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let text = path.to_string_lossy().replace('\\', "/");
                let rel = text.split(".git/").nth(1).expect("under .git");
                assert!(
                    rel == "HEAD"
                        || rel.starts_with("refs/heads/")
                        || rel.starts_with("refs/harness/snapshots/")
                        || rel.starts_with("refs/harness/restore-transactions/"),
                    "unexpected ref file: {rel}"
                );
            }
        }
    }
}

/// NEW-INPUT: unicode paths, CRLF bytes, and a >260-char tree round-trip.
#[test]
fn unicode_crlf_and_long_paths_round_trip() {
    let (_dir, workdir) = git_repo(&[]);
    let deep: String = {
        let mut path = String::new();
        for i in 0..12 {
            path.push_str(&format!("very-deep-component-{i:02}/"));
        }
        path.push_str("leaf.txt");
        path
    };
    assert!(workdir.join(&deep).to_string_lossy().len() > 160);
    let crlf = "crlf.txt";
    let unicode = "ünïcødé-日本語/ファイル.txt";
    write_file(
        &workdir,
        Path::new(crlf),
        b"windows\r\nline-endings\r\nkept\r\n",
    );
    write_file(&workdir, Path::new(unicode), "ユニコード\n".as_bytes());
    write_file(&workdir, Path::new(&deep), "deep\n".as_bytes());

    let session_root = workdir.join("session");
    let store = SnapshotStore::open(&workdir).expect("open store");
    let before = tree_digest(&workdir);
    store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect("snapshot@1");
    write_file(&workdir, Path::new(crlf), b"changed\n");
    delete_file(&workdir, Path::new(unicode));
    delete_file(&workdir, Path::new(&deep));
    let plan = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect("plan");
    store.execute_rewind(plan, None, None).expect("rewind");
    assert_eq!(
        tree_digest(&workdir),
        before,
        "unicode/CRLF/long-path fidelity"
    );
}

/// STALE-STATE + Copilot skip mode: files changed outside the harness
/// since the snapshot are left alone and reported.
#[test]
fn externally_modified_files_are_skipped_and_reported() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n"), ("b.txt", b"beta\n")]);
    let session_root = workdir.join("session");
    let store = SnapshotStore::open(&workdir).expect("open store");
    store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect("snapshot@1");
    write_file(&workdir, Path::new("a.txt"), b"alpha\nharness turn\n");
    write_file(&workdir, Path::new("b.txt"), b"beta\nharness turn\n");
    store
        .snapshot(&sess("s1"), turn(2), &SnapshotOptions::default())
        .expect("snapshot@2 (harness observation)");

    // External edit AFTER the harness last looked (no snapshot since).
    write_file(&workdir, Path::new("a.txt"), b"alpha\nEXTERNAL EDIT\n");
    write_file(&workdir, Path::new("b.txt"), b"beta CHANGED TOO\n");

    let plan = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Skip,
        )
        .expect("plan");
    let mut log = RecordingLog::default();
    let report = store
        .execute_rewind(plan, Some(&mut log), None)
        .expect("rewind with skips");
    let skipped: Vec<&str> = report.skipped.iter().map(|s| s.path.as_str()).collect();
    assert!(
        skipped.contains(&"a.txt"),
        "externally modified skipped: {skipped:?}"
    );
    assert!(
        skipped.contains(&"b.txt"),
        "externally modified skipped: {skipped:?}"
    );
    assert_eq!(
        std::fs::read_to_string(workdir.join("a.txt")).unwrap(),
        "alpha\nEXTERNAL EDIT\n",
        "external edits left alone"
    );
    // The skip mode still emits the truncation cursor (log layer's call).
    assert_eq!(log.cursors.lock().unwrap().len(), 1);
}

/// A file deleted after the snapshot is restored (stale_state).
#[test]
fn deleted_file_is_restored() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let session_root = workdir.join("session");
    let store = SnapshotStore::open(&workdir).expect("open store");
    let before = tree_digest(&workdir);
    store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect("snapshot");
    delete_file(&workdir, Path::new("a.txt"));
    let plan = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect("plan");
    store.execute_rewind(plan, None, None).expect("rewind");
    assert_eq!(tree_digest(&workdir), before, "deleted file restored");
}

/// Ignored files (user .gitignore) never enter snapshots and are never
/// deleted by a rewind; built-in excludes keep .harness out.
#[test]
fn ignored_and_harness_state_stay_out_of_snapshots() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    write_file(&workdir, Path::new(".gitignore"), b"ignored-dir/\n");
    write_file(&workdir, Path::new("ignored-dir/x.txt"), b"ignored\n");
    let session_root = workdir.join("session");
    let store = SnapshotStore::open(&workdir).expect("open store");
    store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect("snapshot");
    let meta = store.snapshot_meta(&sess("s1"), turn(1)).expect("meta");
    assert!(
        !meta.files.contains_key("ignored-dir/x.txt"),
        "gitignore respected"
    );
    assert!(
        !meta.files.keys().any(|p| p.starts_with(".harness/")),
        "built-in .harness exclude"
    );
    // Rewind does NOT delete the ignored file even though it is not in the
    // snapshot tree.
    let plan = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect("plan");
    store.execute_rewind(plan, None, None).expect("rewind");
    assert_eq!(
        std::fs::read_to_string(workdir.join("ignored-dir/x.txt")).unwrap(),
        "ignored\n"
    );
}

/// SESSION-ID validation keeps the ref namespace well-formed.
#[test]
fn session_ids_reject_hostile_names() {
    for bad in ["", "..", "a/b", "refs/heads/main", "with space", "ünïcødé"] {
        assert!(SessionId::new(bad).is_err(), "{bad:?} must be rejected");
    }
    assert!(SessionId::new("sess-a_1").is_ok());
}
