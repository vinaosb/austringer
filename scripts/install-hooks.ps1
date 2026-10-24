# Wire git to the versioned local-CI hooks (no copying, no per-clone
# state): git runs whatever core.hooksPath points at, so the hooks are
# reviewed like any other code. Idempotent — safe to re-run any time.
#
#   pre-commit  fast gate:  cargo fmt --check (host toolchain, seconds)
#   pre-push    full gate:  scripts/local-ci.ps1 (docker ubuntu legs +
#                          native windows legs — the GH Actions parity run)
#
# Bypass either hook once:  HARNESS_SKIP_LOCAL_CI=1 git commit|push ...
# Uninstall:                git config --unset core.hooksPath
$ErrorActionPreference = 'Stop'

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

git -C $repo config core.hooksPath scripts/hooks
if ($LASTEXITCODE -ne 0) { throw "git config core.hooksPath failed in $repo" }

# The hooks are POSIX sh; core.autocrlf=true checkouts hand them CRLF and
# git's bundled sh then chokes on them. Re-normalize to LF (no-op when
# already LF; git status stays clean — text blobs are LF in the index).
foreach ($hook in 'pre-commit', 'pre-push') {
    $p = Join-Path $repo "scripts/hooks/$hook"
    $t = [IO.File]::ReadAllText($p)
    $lf = $t -replace "`r`n", "`n"
    if ($lf -ne $t) { [IO.File]::WriteAllText($p, $lf) }
}

Write-Host "installed: git config core.hooksPath = scripts/hooks (pre-commit: fmt gate, pre-push: full local CI)"
Write-Host 'bypass once:      HARNESS_SKIP_LOCAL_CI=1 git commit|push ...'
Write-Host 'uninstall:        git config --unset core.hooksPath'
exit 0
