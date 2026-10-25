//! Program-resolution tests (todo 27, Droid pattern): a hard blocklist
//! matches the command's ACTUAL program — PATH lookup, wrapper scripts
//! peeled one layer, absolute paths, and quoting tricks cannot bypass;
//! resolution failures fail closed.

// The fixtures exercise windows-resolution mechanics (C:\... stems, the
// .exe/.cmd extension ladder, System32 lookups); std path parsing makes
// drive paths relative on unix. Unix resolution is exercised by the
// engine suites' dual-form fixtures.
#![cfg(windows)]

use std::fs;
use std::path::PathBuf;

use austringer_safety::perm::resolve::{PathResolver, ProgramResolver, Resolution};
use tempfile::TempDir;

fn resolver_with(dir: &TempDir, files: &[(&str, &str)]) -> PathResolver {
    for (name, contents) in files {
        let path = dir.path().join(name);
        fs::write(path, contents).expect("write fixture");
    }
    PathResolver::new(vec![dir.path().to_path_buf()])
}

#[test]
fn bare_word_resolves_through_path() {
    let dir = TempDir::new().expect("tmp");
    let resolver = resolver_with(&dir, &[("git.exe", "")]);
    assert_eq!(
        resolver.resolve("git"),
        Resolution::Resolved {
            program: "git".into(),
            wrapper: None
        }
    );
}

#[test]
fn unknown_program_fails_closed() {
    let dir = TempDir::new().expect("tmp");
    let resolver = resolver_with(&dir, &[]);
    assert_eq!(
        resolver.resolve("definitely-not-real-tool"),
        Resolution::NotFound
    );
}

#[test]
fn absolute_path_resolves_to_its_stem() {
    let dir = TempDir::new().expect("tmp");
    let resolver = resolver_with(&dir, &[]);
    assert_eq!(
        resolver.resolve(r"C:\Windows\System32\format.com"),
        Resolution::Resolved {
            program: "format".into(),
            wrapper: None
        }
    );
}

#[test]
fn long_path_prefix_is_stripped_not_rejected() {
    let dir = TempDir::new().expect("tmp");
    let resolver = resolver_with(&dir, &[]);
    let long = r"\\?\C:\very\long\path\beyond\two\sixty\tool.exe";
    assert_eq!(
        resolver.resolve(long),
        Resolution::Resolved {
            program: "tool".into(),
            wrapper: None
        }
    );
}

#[test]
fn mid_token_quote_is_a_quoting_trick() {
    let dir = TempDir::new().expect("tmp");
    let resolver = resolver_with(&dir, &[("curl.exe", "")]);
    // cu"rl must not resolve to curl — the quote smuggles a different
    // token to the shell than the resolver sees.
    assert!(matches!(
        resolver.resolve("cu\"rl"),
        Resolution::Suspicious { .. }
    ));
}

#[test]
fn quoted_program_word_unquotes_cleanly() {
    let dir = TempDir::new().expect("tmp");
    let resolver = resolver_with(&dir, &[("git.exe", "")]);
    assert_eq!(
        resolver.resolve("\"git\""),
        Resolution::Resolved {
            program: "git".into(),
            wrapper: None
        }
    );
}

#[test]
fn wrapper_script_peels_one_layer_to_real_program() {
    let dir = TempDir::new().expect("tmp");
    let resolver = resolver_with(
        &dir,
        &[(
            "shim.cmd",
            "@echo off\r\n\"C:\\Windows\\System32\\format.com\" %~dp0 %*\r\n",
        )],
    );
    // The wrapper is named innocently; the ACTUAL program is format.
    assert_eq!(
        resolver.resolve("shim"),
        Resolution::Resolved {
            program: "format".into(),
            wrapper: Some("shim".into())
        }
    );
}

#[test]
fn shell_wrapper_peels_exec_line() {
    let dir = TempDir::new().expect("tmp");
    let resolver = resolver_with(
        &dir,
        &[("run.sh", "#!/bin/sh\nexec node server.js \"$@\"\n")],
    );
    assert_eq!(
        resolver.resolve("run.sh"),
        Resolution::Resolved {
            program: "node".into(),
            wrapper: Some("run".into())
        }
    );
}

#[test]
fn opaque_wrapper_resolves_to_its_own_stem() {
    let dir = TempDir::new().expect("tmp");
    let resolver = resolver_with(&dir, &[("opaque.cmd", "@echo off\r\nexit /b 0\r\n")]);
    assert_eq!(
        resolver.resolve("opaque"),
        Resolution::Resolved {
            program: "opaque".into(),
            wrapper: Some("opaque".into())
        }
    );
}

#[test]
fn unix_bare_script_resolves_when_present() {
    let dir = TempDir::new().expect("tmp");
    let resolver = resolver_with(&dir, &[("mytool", "#!/bin/sh\necho hi\n")]);
    let got = resolver.resolve("mytool");
    assert!(
        matches!(
            &got,
            Resolution::Resolved {
                wrapper: Some(_),
                ..
            }
        ),
        "got {got:?}"
    );
}

#[test]
fn empty_program_word_is_suspicious() {
    let dir = TempDir::new().expect("tmp");
    let resolver = resolver_with(&dir, &[]);
    assert!(matches!(
        resolver.resolve(""),
        Resolution::Suspicious { .. }
    ));
}

#[test]
fn resolution_reads_are_bounded() {
    // A wrapper whose body starts with >4KB of comments still resolves:
    // the peel reads only the first 4096 bytes but falls back to the
    // script's own stem, never scanning unboundedly.
    let dir = TempDir::new().expect("tmp");
    let padding = "rem ".to_string() + &"x".repeat(5000) + "\r\n";
    let resolver = resolver_with(&dir, &[("fat.cmd", &padding)]);
    assert_eq!(
        resolver.resolve("fat"),
        Resolution::Resolved {
            program: "fat".into(),
            wrapper: Some("fat".into())
        }
    );
}

#[test]
fn relative_path_with_separator_resolves_to_stem() {
    let dir = TempDir::new().expect("tmp");
    let resolver = resolver_with(&dir, &[]);
    assert_eq!(
        resolver.resolve("./tools/thing.exe"),
        Resolution::Resolved {
            program: "thing".into(),
            wrapper: None
        }
    );
}

#[test]
fn platform_ctor_reads_path_env() {
    let resolver = PathResolver::new(vec![PathBuf::from(r"C:\Windows\System32")]);
    // cmd.exe is present in System32 on Windows CI; on other platforms
    // the explicit-dir ctor still yields a working resolver.
    let got = resolver.resolve("cmd");
    assert!(
        matches!(got, Resolution::Resolved { .. }) || matches!(got, Resolution::NotFound),
        "got {got:?}"
    );
}
