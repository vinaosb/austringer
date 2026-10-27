#!/bin/sh
# One command, one release: installs austringer (CLI), austringer-daemon,
# and austringer-tui from the SAME tag — the latest by default, or pass a
# tag: `install.sh v0.1.2`. Delegates to the official cargo-dist
# installers (they verify checksums); this script only pins them together.
set -eu

tag="${1:-latest}"
base="https://github.com/vinaosb/austringer/releases"

for pkg in cli daemon tui; do
    if [ "$tag" = "latest" ]; then
        url="$base/latest/download/austringer-$pkg-installer.sh"
    else
        url="$base/download/$tag/austringer-$pkg-installer.sh"
    fi
    echo "==> installing austringer-$pkg from $url"
    curl --proto '=https' --tlsv1.2 -LsSf "$url" | sh
done

echo "austringer installed. Health-check with: austringer doctor"
