//! Version reporting with nightly SHA embedding (plan todo 39): nightly
//! builds embed the head SHA in the version tag
//! (`vX.Y-nightly.DATE.SHA`) so tag<->SHA traceability holds by
//! construction — the release pipeline injects `austringer_NIGHTLY_DATE`
//! and `austringer_NIGHTLY_SHA` at build time; release builds embed
//! neither and print the bare package version.

/// The full version string for `--version`.
#[must_use]
pub fn full(pkg_version: &str) -> String {
    match (nightly_date(), nightly_sha()) {
        (Some(date), Some(sha)) => nightly_tag(pkg_version, date, sha),
        _ => pkg_version.to_owned(),
    }
}

/// Compose the nightly tag: `X.Y.Z-nightly.DATE.SHA`.
#[must_use]
pub fn nightly_tag(pkg_version: &str, date: &str, sha: &str) -> String {
    format!("{pkg_version}-nightly.{date}.{sha}")
}

fn nightly_date() -> Option<&'static str> {
    option_env!("austringer_NIGHTLY_DATE")
}

fn nightly_sha() -> Option<&'static str> {
    option_env!("austringer_NIGHTLY_SHA")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nightly_tag_carries_date_and_sha_by_construction() {
        assert_eq!(
            nightly_tag("0.1.0", "2026-09-13", "4f8fc31"),
            "0.1.0-nightly.2026-09-13.4f8fc31"
        );
    }

    #[test]
    fn full_falls_back_to_the_package_version_without_nightly_env() {
        // This workspace never sets the nightly envs in test builds, so
        // the fallback arm is the observable behavior here.
        let built = full("9.9.9");
        if nightly_date().is_some() && nightly_sha().is_some() {
            assert!(built.starts_with("9.9.9-nightly."));
        } else {
            assert_eq!(built, "9.9.9");
        }
    }
}
