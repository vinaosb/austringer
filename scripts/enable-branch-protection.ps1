# enable-branch-protection.ps1 - apply the dormant branch-protection payload
# (ci-local/branch-protection.json) to main via the GitHub settings API.
#
# Why dormant: the repo is private on a free GitHub plan, so the settings
# API answers HTTP 403 - "Upgrade to GitHub Pro or make this repository
# public to enable this feature" (receipt 2026-10-24). Repo-owner
# instruction, verbatim: "If it is possible to turn the branch protection
# on when it repo becomes public it will be nice, until then let's leave
# it commented or off." When the repo goes public (or the plan upgrades),
# re-run this script to activate protection. Side benefit: making the
# repo public also restores free Actions minutes on standard runners
# (the account is currently billing-blocked - jobs fail in ~2s with
# "spending limit" annotations), so the required checks can actually run.
#
# The JSON payload is kept byte-clean (no comment fields) - comments live
# here and in ci-local/README.md.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/enable-branch-protection.ps1
#
# Exit codes: 0 = protection ACTIVE (verified); 1 = dormant (expected
# while the repo is private/free) or error.
param()

# NB: cmd-level 2>&1 merge, per the PS 5.1 stderr trap documented in
# merge-gate.ps1 (NativeCommandError wrapping).
$json = (Join-Path $PSScriptRoot '..\ci-local\branch-protection.json')
if (-not (Test-Path $json)) {
    Write-Output "ENABLE PROTECTION: FAIL - payload not found: $json"
    exit 1
}

$out = @(& cmd /c "gh api -X PUT /repos/vinaosb/harness/branches/main/protection --input `"$json`" 2>&1")
if ($LASTEXITCODE -ne 0) {
    $msg = (($out -join "`n")).Trim()
    if ($msg -match 'HTTP 403') {
        Write-Output 'ENABLE PROTECTION: DORMANT - the settings API answered HTTP 403:'
        Write-Output $msg
        Write-Output 'Blocked while the repo is private on a free plan. Make the repo'
        Write-Output 'public (or upgrade the plan), then re-run this script. Going'
        Write-Output 'public also restores free Actions minutes on standard runners.'
    } else {
        Write-Output "ENABLE PROTECTION: FAIL - gh api exit ${LASTEXITCODE}:"
        Write-Output $msg
    }
    exit 1
}

# Applied: re-read the protection and summarize what the server enforces.
$check = @(& cmd /c 'gh api /repos/vinaosb/harness/branches/main/protection 2>&1')
if ($LASTEXITCODE -ne 0) {
    Write-Output "ENABLE PROTECTION: applied, but the verification GET failed (gh exit ${LASTEXITCODE}):"
    Write-Output (($check -join "`n")).Trim()
    exit 1
}
$p = ConvertFrom-Json ([string]::Join("`n", $check))
Write-Output ('BRANCH PROTECTION ACTIVE on main: {0} required contexts, enforce_admins = {1}' -f @($p.required_status_checks.contexts).Count, $p.enforce_admins)
exit 0
