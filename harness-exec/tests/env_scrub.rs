//! Env-scrub enforcement at the spawn seam (todo 4, ecosystem
//! alignment): a spec carrying an EnvScrub must have the scrubbed
//! variables REMOVED from the child's actual environment — inherited
//! ambient set and additions both — observable through a real PTY
//! child echoing the variable back.

mod common;

use std::time::Duration;

use common::{live_gate, runner_with_temp_spill, spec};
use harness_exec::spec::ExecSpec;
use harness_safety::scrub::EnvScrub;

/// Dropped by the default scrub (contains `SECRET`, not a CORE_VAR).
const SENTINEL: &str = "HARNESS_EXEC_TEST_SECRET_SENTINEL";
const SENTINEL_VALUE: &str = "scrubbed-sentinel-value-4c1d";

#[cfg(windows)]
const SHELL: (&str, [&str; 3]) = (
    "cmd",
    ["/d", "/c", "echo %HARNESS_EXEC_TEST_SECRET_SENTINEL%"],
);
#[cfg(unix)]
const SHELL: (&str, [&str; 2]) = ("sh", ["-c", "echo $HARNESS_EXEC_TEST_SECRET_SENTINEL"]);

fn sentinel_spec(env_scrub: Option<EnvScrub>) -> ExecSpec {
    let mut command = spec(SHELL.0, &SHELL.1);
    command
        .env
        .insert(SENTINEL.to_owned(), SENTINEL_VALUE.to_owned());
    command.timeout = Duration::from_secs(30);
    command.env_scrub = env_scrub;
    command
}

#[test]
fn env_scrub_removes_dropped_variables_from_the_child_environment() {
    let _gate = live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);

    let unscrubbed = runner
        .run(&sentinel_spec(None))
        .expect("unscrubbed run");
    assert!(
        unscrubbed.model_text.contains(SENTINEL_VALUE),
        "control run: the addition must reach the child without a scrub\n{}",
        unscrubbed.model_text
    );

    let scrubbed = runner
        .run(&sentinel_spec(Some(EnvScrub::default())))
        .expect("scrubbed run");
    assert!(
        !scrubbed.model_text.contains(SENTINEL_VALUE),
        "the scrub must remove the SECRET-named variable before spawn\n{}",
        scrubbed.model_text
    );
}

#[test]
fn env_scrub_keeps_scrub_exceptions() {
    let _gate = live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);

    let excepted = runner
        .run(&sentinel_spec(Some(EnvScrub::with_exceptions([
            "HARNESS_EXEC_TEST_SECRET_SENTINEL",
        ]))))
        .expect("excepted run");
    assert!(
        excepted.model_text.contains(SENTINEL_VALUE),
        "an explicit exception survives the scrub\n{}",
        excepted.model_text
    );
}
