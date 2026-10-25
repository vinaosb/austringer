//! The `mocktool` binary entry (see [`austringer_test_support::mocktool`]
//! module docs for the mode contract). A test-support probe binary, not
//! a release artifact — cargo-dist must skip it.

fn main() {
    std::process::exit(austringer_test_support::mocktool::run_bin());
}
