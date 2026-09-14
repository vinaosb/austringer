//! The CLI eval-adapter contract (todo 44: "evals adapter drives CLI
//! --output-json"). This module owns the ARGV the scheduled sweep
//! runner spawns per case and the exit-code mapping; the NDJSON the
//! invocation prints is parsed by [`crate::wire`]. NOTHING here spawns
//! a process — live-model sweeps are SCHEDULED ONLY (daemon todo 41)
//! and hermetic tests never execute anything.

use crate::dataset::EvalCase;

/// Outcome of a CLI run derived from its exit code + turn_result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliOutcome {
    /// Turn finished successfully (exit 0).
    Success,
    /// Turn finished with a failure outcome (exit 1, `ok == false`).
    TurnFailure,
    /// CLI-level failure (transport, timeout: exit ≥ 2).
    CliError,
}

/// Maps a `harness run` exit code + `turn_result.ok` to the eval
/// outcome. Codes: 0 success, 1 failed turn (the CLI's `TurnFailed`),
/// anything else is a CLI-level error.
#[must_use]
pub fn cli_outcome(exit_code: i32, turn_ok: Option<bool>) -> CliOutcome {
    match (exit_code, turn_ok) {
        (0, Some(true)) | (0, None) => CliOutcome::Success,
        (_, Some(false)) => CliOutcome::TurnFailure,
        (1, Some(true)) => CliOutcome::Success,
        _ => CliOutcome::CliError,
    }
}

/// Builds the argv for one eval-case invocation of the CLI:
/// `<binary> --output-json run --session eval-<case> --timeout <n>
/// <prompt>` (the prompt is one trailing argument; clap's
/// `trailing_var_arg` collects it as the single prompt word).
///
/// The eval session id embeds the case id so runs are URL-addressable
/// and auditable per case.
#[must_use]
pub fn run_argv(binary: &str, case: &EvalCase, timeout_secs: u32) -> Vec<String> {
    vec![
        binary.to_owned(),
        "--output-json".to_owned(),
        "run".to_owned(),
        "--session".to_owned(),
        format!("eval-{}", case.id),
        "--timeout".to_owned(),
        timeout_secs.to_string(),
        case.prompt.clone(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::EvalCase;
    use crate::grader::GraderSpec;

    #[test]
    fn argv_drives_output_json_with_a_per_case_session() {
        let case = EvalCase {
            id: "auth-3".to_owned(),
            prompt: "fix the auth bug".to_owned(),
            grader: GraderSpec::AllOf {
                expected: vec!["x".to_owned()],
            },
            threshold: 1.0,
            canary: None,
            tags: vec![],
        };
        let argv = run_argv("harness", &case, 60);
        assert_eq!(argv[0], "harness");
        assert!(argv.contains(&"--output-json".to_owned()));
        let run_pos = argv.iter().position(|a| a == "run");
        assert_eq!(
            run_pos,
            Some(2),
            "run must follow the global flag: {argv:?}"
        );
        assert!(argv.contains(&"--session".to_owned()));
        assert!(argv.contains(&"eval-auth-3".to_owned()));
        assert!(argv.contains(&"60".to_owned()));
        assert_eq!(argv.last(), Some(&"fix the auth bug".to_owned()));
    }

    #[test]
    fn exit_codes_map_to_typed_outcomes() {
        assert_eq!(cli_outcome(0, Some(true)), CliOutcome::Success);
        assert_eq!(cli_outcome(1, Some(false)), CliOutcome::TurnFailure);
        assert_eq!(cli_outcome(0, Some(false)), CliOutcome::TurnFailure);
        assert_eq!(cli_outcome(2, None), CliOutcome::CliError);
        assert_eq!(cli_outcome(101, Some(true)), CliOutcome::CliError);
    }
}
