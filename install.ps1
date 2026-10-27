# One command, one release: installs austringer (CLI), austringer-daemon,
# and austringer-tui from the SAME tag — the latest by default, or pass a
# tag: `install.ps1 -Tag v0.1.2`. Delegates to the official cargo-dist
# installers (they verify checksums); this script only pins them together.
param(
    [string]$Tag = "latest"
)

$ErrorActionPreference = "Stop"
$base = "https://github.com/vinaosb/austringer/releases"

foreach ($pkg in "cli", "daemon", "tui") {
    if ($Tag -eq "latest") {
        $url = "$base/latest/download/austringer-$pkg-installer.ps1"
    }
    else {
        $url = "$base/download/$Tag/austringer-$pkg-installer.ps1"
    }
    Write-Host "==> installing austringer-$pkg from $url"
    Invoke-Expression (Invoke-RestMethod -Uri $url)
}

Write-Host "austringer installed. Health-check with: austringer doctor"
