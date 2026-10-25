//! FS-contention hardening tests (plan todo 20): ERROR_SHARING_VIOLATION
//! retry-with-backoff, long-path (\\?\) awareness, OneDrive detection.

// All three mechanisms under test are Windows-only FS semantics; the
// fixtures assert \\?\ prefixing and OneDrive root shapes that do not
// exist on other platforms.
#![cfg(windows)]

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use austringer_exec::fsutil::{self, OnedriveStatus};

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

// ---------- retry on ERROR_SHARING_VIOLATION ----------

#[test]
fn transient_sharing_violation_is_retried_to_success() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("contended.txt");

    // Hold the file with share_mode(0) briefly, exactly like Defender or
    // an indexer scanning a fresh file does.
    #[cfg(windows)]
    {
        let target_clone = target.clone();
        std::thread::spawn(move || {
            let _lock = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .share_mode(0)
                .open(&target_clone);
            std::thread::sleep(Duration::from_millis(120));
        });
        // Give the holder a head start so the first create attempt loses.
        std::thread::sleep(Duration::from_millis(30));
    }

    let budget = fsutil::RetryBudget::default();
    fsutil::retry_sharing_violation(&budget, || {
        let mut file = std::fs::File::create(&target)?;
        file.write_all(b"contention-hardened")?;
        file.sync_all()
    })
    .expect("retry must outlast the transient lock");
    assert_eq!(
        std::fs::read(&target).expect("read"),
        b"contention-hardened"
    );
}

#[test]
fn non_contention_errors_return_immediately() {
    let started = Instant::now();
    let result: io::Result<()> =
        fsutil::retry_sharing_violation(&fsutil::RetryBudget::default(), || {
            Err(io::Error::from(io::ErrorKind::NotFound))
        });
    assert!(result.is_err());
    assert!(
        started.elapsed() < Duration::from_millis(200),
        "no backoff burns on unrelated errors"
    );
}

#[test]
fn persistent_contention_surfaces_the_error_after_the_budget() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("locked-forever.txt");
    #[cfg(windows)]
    {
        let holder = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .share_mode(0)
            .open(&target)
            .expect("holder open");
        let budget = fsutil::RetryBudget::new(3, Duration::from_millis(10));
        let result = fsutil::retry_sharing_violation(&budget, || {
            std::fs::OpenOptions::new()
                .write(true)
                .open(&target)
                .map(|file| {
                    drop(file);
                })
        });
        let error = result.expect_err("holder keeps share_mode(0) the whole time");
        // The surfaced error is the ORIGINAL contention error, typed.
        #[cfg(windows)]
        assert_eq!(error.raw_os_error(), Some(32));
        drop(holder);
    }
}

// ---------- long-path (\\?\) awareness ----------

#[test]
fn short_and_relative_paths_stay_unchanged() {
    // Relative and already-extended paths pass through; every absolute
    // path gets the prefix (no length threshold to get wrong).
    assert_eq!(
        fsutil::long_path(Path::new("relative.txt")),
        PathBuf::from("relative.txt")
    );
    assert_eq!(
        fsutil::long_path(Path::new(r"\\?\C:\already\extended")),
        PathBuf::from(r"\\?\C:\already\extended")
    );
    assert_eq!(
        fsutil::long_path(Path::new(r"C:\normal\path.txt")),
        PathBuf::from(r"\\?\C:\normal\path.txt")
    );
}

#[test]
fn absolute_paths_get_the_extended_prefix() {
    let long =
        PathBuf::from(r"C:\some\very\long\directory\structure").join("x".repeat(240) + ".txt");
    let extended = fsutil::long_path(&long);
    assert!(
        extended
            .as_os_str()
            .to_string_lossy()
            .starts_with(r"\\?\C:\"),
        "extended form: {:?}",
        extended
    );
}

#[test]
fn long_path_round_trips_through_real_io() {
    // > 260 chars total: un-extended APIs fail, the \\?\ form works.
    let dir = tempfile::tempdir().expect("tempdir");
    let mut deep = dir.path().to_path_buf();
    for _ in 0..12 {
        deep = deep.join("nested-directory-with-a-name");
    }
    let file = deep.join("deep-file.txt");
    let path = fsutil::long_path(&file);
    let text = b"long-path aware IO";
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, text).expect("write long");
    assert_eq!(std::fs::read(&path).expect("read long"), text);
    assert!(
        file.as_os_str().len() > 260,
        "fixture must actually exceed MAX_PATH"
    );
}

// ---------- OneDrive detection ----------

#[test]
fn onedrive_roots_are_detected_case_insensitively() {
    let roots = [PathBuf::from(r"C:\Users\alice\OneDrive - Contoso")];
    assert_eq!(
        fsutil::onedrive_status_in(
            Path::new(r"c:\users\alice\ONEDRIVE - contoso\repo\a.txt"),
            &roots
        ),
        OnedriveStatus::Synced
    );
    assert_eq!(
        fsutil::onedrive_status_in(Path::new(r"C:\Users\alice\elsewhere\a.txt"), &roots),
        OnedriveStatus::Local
    );
    assert_eq!(
        fsutil::onedrive_status_in(Path::new(r"C:\Users\alice\OneDrive - Contoso"), &roots),
        OnedriveStatus::Synced,
        "the root itself counts"
    );
}

#[test]
fn onedrive_detection_without_roots_is_local() {
    assert_eq!(
        fsutil::onedrive_status_in(Path::new(r"C:\any\where"), &[]),
        OnedriveStatus::Local
    );
}

#[test]
fn runner_surfaces_onedrive_spill_root_warning() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = austringer_exec::runner::PtyRunner::with_spill_root(dir.path());
    assert_eq!(runner.spill_root_onedrive(), OnedriveStatus::Local);
    // A hypothetical synced root (pure check) classifies without any env.
    let synced = Path::new(r"C:\Users\alice\OneDrive\spill");
    assert_eq!(
        fsutil::onedrive_status_in(synced, &[PathBuf::from(r"C:\Users\alice\OneDrive")]),
        OnedriveStatus::Synced
    );
}
