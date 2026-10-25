# syntax=docker/dockerfile:1-labs
# Multi-arch (linux/amd64 + linux/arm64) image carrying the daemon AND the
# CLI — the CI-runner consumption path (gh-aw / Copilot-Actions pattern).
# Built and pushed to GHCR on release tags by .github/workflows/
# publish-container.yml (cargo-dist publish job; see dist-workspace.toml).
#
# The [[bin]] targets (austringer-daemon: todo 35, austringer-cli: todo 39) do not
# exist yet at scaffold stage; the final stage's --bin build below is the
# switch that turns this image on — by design, image builds only succeed
# once both binaries land.
#
# Runtime base: gcr.io/distroless/cc-debian12:nonroot. Chosen over
# debian:slim: no shell / package manager (smaller attack surface), ~30 MB
# smaller, ships ca-certificates (the daemon speaks HTTPS to model APIs),
# libgcc + glibc for Rust gnu binaries, and a nonroot UID (65532) by default.

FROM rust:1.98-slim AS build
WORKDIR /app
# rust:slim ships no toolchain linker; gcc/libc6-dev are required once real
# [[bin]] targets exist. Unpinned on purpose (hadolint ignore=DL3008): the
# base image floats across Debian releases where package versions differ,
# and this throwaway build stage never reaches the runtime image.
# hadolint ignore=DL3008
RUN apt-get update \
 && apt-get install -y --no-install-recommends gcc libc6-dev \
 && rm -rf /var/lib/apt/lists/*

# --- dependency-caching layer -------------------------------------------
# Manifests only + dummy lib sources: `cargo build` warms and layer-caches
# all dependency compilation (buildx cache-from/to type=gha in the publish
# workflow), so later builds recompile only real source changes. COPY
# --parents (dockerfile:1-labs) preserves per-crate directory layout.
# Dummy libs (never dummy bins): a dummy src/main.rs would survive the
# real-source COPY below (COPY merges dirs) and fabricate hollow
# `fn main() {}` binaries — the final --bin build must FAIL loudly while
# the real [[bin]] targets do not exist, not ship empty ones.
COPY Cargo.toml Cargo.lock ./
COPY --parents austringer-*/Cargo.toml ./
RUN for d in austringer-*/; do \
        mkdir -p "$d/src" && printf '' > "$d/src/lib.rs"; \
    done \
 && cargo build --release --locked

# --- real build ----------------------------------------------------------
COPY --parents austringer-*/src ./
RUN cargo build --release --locked --bin austringer-daemon --bin austringer-cli \
 && mkdir /out \
 && cp target/release/austringer-daemon target/release/austringer-cli /out/

FROM gcr.io/distroless/cc-debian12:nonroot
# Daemon is the entrypoint; the CLI is equally present for
# `docker run <image> austringer-cli <args>` exec-style usage.
COPY --from=build /out/austringer-daemon /usr/local/bin/austringer-daemon
COPY --from=build /out/austringer-cli /usr/local/bin/austringer-cli
# Listening port(s) are pinned with the daemon protocol (todo 35); add
# EXPOSE then.
ENTRYPOINT ["austringer-daemon"]
