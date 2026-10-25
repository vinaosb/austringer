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

# Debian 12 (bookworm) builder to match the distroless/cc-debian12 runtime
# glibc — building on trixie produces binaries needing GLIBC_2.39+.
FROM rust:1.98-slim-bookworm AS build
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
# Stub every DECLARED target path (bins/benches with explicit or implicit
# paths) plus each lib, so workspace manifests resolve in the cache layer;
# `--lib` compiles only the (empty) libs, warming dependency compilation.
# Real sources overwrite every stub in the final stage - no hollow bins.
RUN for d in austringer-*/; do \
        mkdir -p "$d/src" "$d/src/bin" "$d/benches"; \
        printf '' > "$d/src/lib.rs"; \
    done \
 && for d in austringer-*/; do \
        awk -v d="$d" \
            'function emit() { \
                 if (path != "") print d path; \
                 else if (name != "") print d sec "/" name ".rs"; \
                 name = ""; path = "" \
             } \
             /^\[\[bin\]\]/ { if (sec != "") emit(); sec = "src/bin"; next } \
             /^\[\[bench\]\]/ { if (sec != "") emit(); sec = "benches"; next } \
             /^\[/ { if (sec != "") emit(); sec = ""; next } \
             sec != "" && /^name/ { gsub(/[\r", ]/, "", $3); name = $3 } \
             sec != "" && /^path/ { gsub(/[\r", ]/, "", $3); path = $3 } \
             END { if (sec != "") emit() }' "$d/Cargo.toml"; \
    done | sort -u | xargs -r -n1 sh -c 'mkdir -p "$(dirname "$0")"; printf "" > "$0"' \
 && cargo build --release --locked --lib

# --- real build ----------------------------------------------------------
COPY --parents austringer-*/src ./
# Embedded non-src resources (austringer-web assets/, austringer-media
# fixture fonts) pulled in by include_str!/include_bytes! at compile time.
COPY --parents austringer-*/assets austringer-*/fixtures ./
# BuildKit COPY preserves (possibly older-than-stub) source mtimes, which
# cargo would read as "unchanged since the stub compile". Touch everything
# so the real sources always outrank the cache-layer stubs.
RUN find austringer-*/src -type f -exec touch {} + \
 && cargo build --release --locked --bin austringer-daemon --bin austringer \
 && mkdir /out \
 && cp target/release/austringer-daemon target/release/austringer /out/

FROM gcr.io/distroless/cc-debian12:nonroot
# Daemon is the entrypoint; the CLI is equally present for
# `docker run <image> austringer <args>` exec-style usage.
COPY --from=build /out/austringer-daemon /usr/local/bin/austringer-daemon
COPY --from=build /out/austringer /usr/local/bin/austringer
# Listening port(s) are pinned with the daemon protocol (todo 35); add
# EXPOSE then.
ENTRYPOINT ["austringer-daemon"]
