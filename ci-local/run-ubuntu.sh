#!/usr/bin/env bash
# Local CI parity runner — the ubuntu-latest legs of
# .github/workflows/ci.yml, step for step, inside the ci-local Dockerfile
# image (ubuntu:24.04, rust 1.98, nextest pinned, ANSI-free env).
#
# Usage (normally via scripts/local-ci.ps1, which supplies the mounts):
#   ./ci-local/run-ubuntu.sh lint   # fmt + clippy (+feature matrices) + deny
#   ./ci-local/run-ubuntu.sh test   # nextest archive+run, wasm-plugins, doctests
#
# The step set mirrors ci.yml exactly — nothing local-only is added and
# nothing CI-red is skipped; a green run here means the GH ubuntu legs of
# the lint/test jobs would have been green (receipt chain for why that
# needs a container: 46fe3ef, 3e984a4/30ce144, e7a81a2, 88674b7).
#
# NO_COLOR=1 + CARGO_TERM_COLOR=never come from the image env; the wasm
# guard still strips ANSI defensively, exactly like the CI step (28b27b4).

set -euo pipefail

mode="${1:-}"
if [ "$mode" != "lint" ] && [ "$mode" != "test" ]; then
    echo "usage: run-ubuntu.sh lint|test (got: '${mode}')" >&2
    exit 2
fi

# step <name> <cmd...> — print a CI-style step banner with timing; exit
# non-zero (naming the failed step) on the first failure.
step() {
    local name="$1"
    shift
    local start rc
    start=$(date +%s)
    printf '\n-- STEP: %s --\n' "$name"
    if "$@"; then
        printf '\n-- STEP: %s ok (%ss) --\n' "$name" "$(( $(date +%s) - start ))"
    else
        rc=$?
        printf '\n-- STEP: %s FAILED (exit %s after %ss) --\n' \
            "$name" "$rc" "$(( $(date +%s) - start ))" >&2
        exit "$rc"
    fi
}

# Mirrors the "Run wasm-plugin tests (feature-gated out of the archive)"
# step: run with the feature on, then assert the run count is > 0 so the
# suites cannot silently shrink to zero tests again (ci.yml comment,
# todo 25). Reads the color-stripped log for the count.
wasm_feature_run_with_count_guard() {
    set -o pipefail
    NO_COLOR=1 cargo nextest run --locked \
        -p austringer-wasm --features wasm-plugins 2>&1 | tee /tmp/wasm-nextest.log
    runs=$(sed $'s/\x1b\[[0-9;]*m//g' /tmp/wasm-nextest.log \
        | grep -Eo '[0-9]+ tests? run' | tail -n1 | grep -Eo '^[0-9]+' || true)
    if [ -z "$runs" ] || [ "$runs" -eq 0 ]; then
        echo "austringer-wasm --features wasm-plugins ran ${runs:-0} tests (expected >0)" >&2
        exit 1
    fi
    echo "austringer-wasm feature-gated tests executed: $runs"
}

case "$mode" in
lint)
    # ci.yml `lint` job, ubuntu-latest, in workflow order.
    step "rustfmt" cargo fmt --all -- --check
    step "clippy" cargo clippy --locked --all-targets -- -D warnings
    step "clippy (austringer-bench: dhat)" \
        cargo clippy --locked -p austringer-bench --all-targets --features dhat -- -D warnings
    step "clippy (austringer-bench: mimalloc)" \
        cargo clippy --locked -p austringer-bench --all-targets --features mimalloc -- -D warnings
    step "clippy (austringer-wasm: wasm-plugins)" \
        cargo clippy --locked -p austringer-wasm --all-targets --features wasm-plugins -- -D warnings
    step "cargo-deny (advisories, bans, licenses)" cargo deny check
    ;;
test)
    # ci.yml `test` job, ubuntu-latest matrix leg, in workflow order.
    # Archive goes to /tmp (inside the container) so the mounted worktree
    # stays clean; CI uploads the archive for replay — locally the same
    # file is consumed immediately by the run-from-archive step.
    step "nextest archive (workspace)" \
        cargo nextest archive --workspace --locked --archive-file /tmp/nextest-archive.tar.zst
    step "nextest run (from archive)" \
        cargo nextest run --archive-file /tmp/nextest-archive.tar.zst
    step "wasm-plugin tests (feature-on, count-guarded)" wasm_feature_run_with_count_guard
    step "doc tests" cargo test --doc --workspace --locked
    ;;
esac

printf '\n-- run-ubuntu.sh %s: ALL STEPS PASSED --\n' "$mode"
