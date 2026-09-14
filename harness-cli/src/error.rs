//! Typed CLI failures. Every `Display` string is a stable contract the
//! assert_cmd suite pins: "unreachable" (daemon absent), "auth" (token
//! refused), "image" (attachment rejected). Usage errors are clap's
//! domain (exit 2); everything here exits 1.

/// One typed client-side failure.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// No daemon listening on the selected transport.
    #[error("daemon unreachable: {detail}")]
    Unreachable {
        /// Transport-specific detail.
        detail: String,
    },
    /// The daemon refused the token (or none was available).
    #[error("auth failed: {detail}")]
    Auth {
        /// What was presented and refused.
        detail: String,
    },
    /// The daemon spoke outside the client's protocol expectations.
    #[error("protocol error: {detail}")]
    Protocol {
        /// Bounded detail (never a raw serde dump).
        detail: String,
    },
    /// An `--image` attachment failed client-side validation.
    #[error("image attachment rejected: {detail}")]
    Image {
        /// Which path and why.
        detail: String,
    },
    /// Local filesystem or IPC I/O failure.
    #[error("io error: {detail}")]
    Io {
        /// Underlying failure description.
        detail: String,
    },
    /// The turn did not finish within the client-side deadline.
    #[error("turn timed out after {secs}s")]
    Timeout {
        /// The elapsed deadline in seconds.
        secs: u64,
    },
    /// The turn finished with a failure outcome.
    #[error("turn failed: {outcome}")]
    TurnFailed {
        /// The daemon's typed outcome summary (e.g. `failed::…`).
        outcome: String,
    },
}

impl CliError {
    /// Process exit code for this failure (usage errors exit 2 via clap;
    /// every typed CLI failure is 1).
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_strings_carry_the_pinned_keywords() {
        assert!(
            CliError::Unreachable {
                detail: "connect refused".into()
            }
            .to_string()
            .contains("unreachable")
        );
        assert!(
            CliError::Auth {
                detail: "401".into()
            }
            .to_string()
            .contains("auth")
        );
        assert!(
            CliError::Image {
                detail: "not a png".into()
            }
            .to_string()
            .contains("image")
        );
    }

    #[test]
    fn every_typed_failure_exits_one() {
        let samples = [
            CliError::Unreachable {
                detail: String::new(),
            },
            CliError::Auth {
                detail: String::new(),
            },
            CliError::Protocol {
                detail: String::new(),
            },
            CliError::Image {
                detail: String::new(),
            },
            CliError::Io {
                detail: String::new(),
            },
            CliError::Timeout { secs: 1 },
            CliError::TurnFailed {
                outcome: String::new(),
            },
        ];
        for sample in samples {
            assert_eq!(sample.exit_code(), 1);
        }
    }
}
