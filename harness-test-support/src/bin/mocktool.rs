//! The `mocktool` binary entry (see [`harness_test_support::mocktool`]
//! module docs for the mode contract). A test-support probe binary, not
//! a release artifact — cargo-dist must skip it.

fn main() {
    std::process::exit(harness_test_support::mocktool::run_bin());
}
