# Local CI parity image — mirrors the ubuntu-latest legs of
# .github/workflows/ci.yml (lint + test jobs) as closely as a container can.
#
# WHY docker: every CI red in this repo's receipt chain was a
# dev-PC-vs-GH-runner divergence, not a logic bug — ANSI escapes breaking
# log parsers (46fe3ef), hardcoded windows separators surviving into unix
# fixture paths (3e984a4 / 30ce144), autocrlf fixture eol drift (e7a81a2),
# and a sibling-process flake that only reproduced under the runner's
# process layout (88674b7). Building/testing the workspace in an
# ubuntu:24.04 container (the generation GH ubuntu runners are built from)
# surfaces those classes BEFORE credits are spent on Actions.
#
# Layout notes:
#  * RUSTUP_HOME=/opt/rustup and CARGO_HOME=/opt/cargo are baked and owned
#    by the non-root `runner` user (uid 1000 — the GH runner convention).
#    scripts/local-ci.ps1 mounts the named volume `austringer-localci-cargo`
#    at /opt/cargo: docker seeds an empty named volume from the image's
#    directory on first mount, so the pinned tools survive; -RebuildImage
#    drops the volume so a rebuilt image re-seeds it.
#  * /target (named volume `austringer-localci-target`) is the cargo target
#    dir, so incremental state persists across runs.
#  * /src is the bind-mounted repo (rw — tests write temp fixtures under
#    the workspace).
#
# Tool pins (MUST match .github/workflows/ci.yml):
#   rust 1.98 (workspace MSRV, Cargo.toml rust-version)
#   cargo-nextest@0.9.144, cargo-deny@0.20.2

FROM ubuntu:24.04

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential curl git ca-certificates pkg-config procps \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    PATH=/opt/cargo/bin:${PATH}

# Same install shape as ci.yml's "Install Rust 1.98 (workspace MSRV)"
# step: pinned toolchain, minimal profile, then rustfmt+clippy components
# (the runner default minimal profile ships neither for new toolchains).
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y \
        --default-toolchain 1.98 --profile minimal --no-modify-path \
    && rustup component add rustfmt clippy \
    && cargo install --locked cargo-nextest@0.9.144 cargo-deny@0.20.2 \
    && rm -rf /opt/cargo/registry /opt/cargo/git

# Env parity with ci.yml's workflow env, plus the repo's ANSI discipline:
# NO_COLOR=1 / CARGO_TERM_COLOR=never — this repo has a receipt chain of
# CI-reds caused by ANSI codes defeating log parsers (46fe3ef, and the
# wasm run-count guard added in 28b27b4). CI pipes logs into machines;
# the local rig behaves the same instead of being prettier.
ENV CARGO_TERM_COLOR=never \
    NO_COLOR=1 \
    CARGO_PROFILE_DEV_DEBUG=0 \
    CARGO_PROFILE_TEST_DEBUG=0 \
    CARGO_TARGET_DIR=/target

# ubuntu:24.04 already ships a non-root user `ubuntu` with uid 1000 —
# exactly the GH runner uid convention — so reuse it instead of creating
# a duplicate uid (useradd would fail: "UID 1000 is not unique").
RUN mkdir -p /target /src \
    && chown -R ubuntu:ubuntu /opt/cargo /opt/rustup /target

USER ubuntu
WORKDIR /src
