# merge-gate.ps1 - client-side merge gate: prove every PR check is green
# before merging.
#
# Why this exists: the repo is private on a free GitHub plan, so server-side
# branch protection is unavailable - the settings API answers HTTP 403
# ("Upgrade to GitHub Pro or make this repository public"; receipt
# 2026-10-24). Until the plan changes, merging is gated by discipline:
# NEVER run `gh pr merge` unless this script printed PASS immediately
# before. This is the last line of defense after the pre-push local gate
# (ci-local/), not a replacement for it.
#
# Scheduled-workflow jobs (deep-qa, evals-scheduled) do not gate PRs by repo
# policy and typically do not appear on a PR's rollup at all. SKIPPED and
# NEUTRAL entries (e.g. cargo-dist release jobs that only run on tags) are
# printed as informational and do not block; every other check must be
# COMPLETED with conclusion SUCCESS.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/merge-gate.ps1 3
#
# Exit codes: 0 = all gate-relevant checks green; 1 = blocked or error.
param(
    [Parameter(Mandatory = $true, HelpMessage = 'PR number to gate, e.g. 3')]
    [int]$PrNumber
)

# NB: no PS-level `2>&1` here - PS 5.1 wraps native stderr in
# NativeCommandError records that either echo a fake stack dump (EAP
# Continue) or vanish entirely (EAP SilentlyContinue drops the record;
# both behaviors verified). cmd-level merging sidesteps the whole trap.
$out = @(& cmd /c "gh pr view $PrNumber --json statusCheckRollup 2>&1")
if ($LASTEXITCODE -ne 0) {
    Write-Output "MERGE GATE: FAIL - could not fetch checks for PR #$PrNumber (gh exit $LASTEXITCODE):"
    Write-Output ((($out -join "`n")).Trim())
    exit 1
}

try {
    $checks = @((ConvertFrom-Json ([string]::Join("`n", $out))).statusCheckRollup)
} catch {
    Write-Output "MERGE GATE: FAIL - could not parse gh output: $($_.Exception.Message)"
    exit 1
}

if ($checks.Count -eq 0) {
    Write-Output 'MERGE GATE: FAIL - no checks reported - refusing: CI must run before merge.'
    exit 1
}

$blocked = @()
foreach ($c in $checks) {
    $name = $c.name
    if (-not $name) { $name = $c.context }
    if ($null -ne $c.status) {
        # CheckRun shape: green = COMPLETED + SUCCESS; skipped/neutral informational.
        if ($c.conclusion -in @('SKIPPED', 'NEUTRAL')) {
            Write-Output ('INFO (skipped, not gating): {0} - {1}/{2}' -f $name, $c.status, $c.conclusion)
        } elseif ($c.status -eq 'COMPLETED' -and $c.conclusion -eq 'SUCCESS') {
            # green
        } else {
            $blocked += ('BLOCK: {0} - {1}/{2}' -f $name, $c.status, $c.conclusion)
        }
    } else {
        # StatusContext shape (commit statuses): only SUCCESS is green.
        if ($c.state -eq 'SUCCESS') { continue }
        $blocked += ('BLOCK: {0} - {1}' -f $name, $c.state)
    }
}

foreach ($line in $blocked) { Write-Output $line }
if ($blocked.Count -gt 0) {
    Write-Output ('MERGE GATE: FAIL - {0} of {1} checks not green. Fix or wait, then re-run this gate.' -f $blocked.Count, $checks.Count)
    exit 1
}

Write-Output ('MERGE GATE: PASS - all {0} checks green' -f $checks.Count)
Write-Output ('Next: gh pr merge {0} --merge' -f $PrNumber)
exit 0
