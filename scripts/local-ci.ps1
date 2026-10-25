# Local CI parity austringer — run the repo's GitHub Actions jobs on the
# author's PC before spending CI/CD credits. Ubuntu legs run inside the
# ci-local Docker image (runner parity); windows legs run natively.
# See ci-local/README.md for the full contract.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/local-ci.ps1
#   powershell ... -File scripts/local-ci.ps1 -Phases native-test
#   powershell ... -File scripts/local-ci.ps1 -All          # adds the optional phases
#   powershell ... -File scripts/local-ci.ps1 -SkipDocker   # native legs only
#   powershell ... -File scripts/local-ci.ps1 -RebuildImage # rebuild image + re-seed cargo volume
param(
    [string[]]$Phases = @('docker-lint', 'docker-test', 'native-test'),
    [switch]$All,
    [switch]$SkipDocker,
    [switch]$RebuildImage
)

$ErrorActionPreference = 'Stop'
$script:Failed = @()

# ── guards ────────────────────────────────────────────────────────────────
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (-not (Test-Path -LiteralPath (Join-Path $RepoRoot '.git'))) {
    throw "not a git worktree: $RepoRoot (run scripts/local-ci.ps1 from a repo checkout)"
}
# NB: under $ErrorActionPreference='Stop', PS 5.1 turns any redirected
# native stderr (`*> $null`) into a terminating NativeCommandError — every
# existence probe below must catch, not just check $LASTEXITCODE.
$hasHead = $false
try { git -C $RepoRoot rev-parse --verify HEAD *> $null; $hasHead = ($LASTEXITCODE -eq 0) } catch { $hasHead = $false }
if (-not $hasHead) { throw 'repo has no HEAD (nothing committed yet) — refusing to run' }

# ANSI-free parity for the native legs (same discipline as the container;
# receipt chain 46fe3ef / 28b27b4 — colored output breaks log parsing).
$env:NO_COLOR = '1'
$env:CARGO_TERM_COLOR = 'never'

# ── helpers ───────────────────────────────────────────────────────────────
function Invoke-Checked([string]$Name, [scriptblock]$Body) {
    & $Body
    if ($LASTEXITCODE -ne 0) { throw "$Name exited $LASTEXITCODE" }
}

# Dev-machine cargo wrapper (MSVC pin + link-OOM guard; the repo-documented
# invocation shape). Falls back to plain cargo on machines that don't carry
# the wrapper but have a standard rustup PATH.
$CargoWrapper = Join-Path $env:TEMP 'opencode\cargo-env.cmd'
function Invoke-Cargo([string[]]$CargoArgs, [string]$WorkDir = $RepoRoot) {
    $inner = if (Test-Path $CargoWrapper) { "set CARGO_BUILD_JOBS=3&& `"$CargoWrapper`" cargo $($CargoArgs -join ' ')" } else { "cargo $($CargoArgs -join ' ')" }
    Push-Location $WorkDir
    try {
        Invoke-Checked "cargo $($CargoArgs -join ' ')" { cmd /c $inner }
    } finally { Pop-Location }
}

# Keep POSIX sh entrypoints LF on disk even when core.autocrlf=true has
# re-checked them out CRLF (bash in the container / git hooks chokes on
# CRLF). Git-normalized text stays clean either way.
function Normalize-Lf([string]$RelPath) {
    $p = Join-Path $RepoRoot $RelPath
    $t = [IO.File]::ReadAllText($p)
    $lf = $t -replace "`r`n", "`n"
    if ($lf -ne $t) { [IO.File]::WriteAllText($p, $lf) }
}

function Test-DockerImage([string]$Tag) {
    try { docker image inspect $Tag *> $null } catch { return $false }
    return ($LASTEXITCODE -eq 0)
}

function Ensure-LocalCiImage {
    $hash = (Get-FileHash -LiteralPath (Join-Path $RepoRoot 'ci-local\Dockerfile') -Algorithm SHA256).Hash.Substring(0, 8).ToLowerInvariant()
    $tag = "austringer-localci:$hash"
    if (-not (Test-DockerImage $tag) -or $RebuildImage) {
        if ($RebuildImage) {
            # Drop the seeded cargo home so a rebuilt image re-seeds its
            # pinned tools into the volume (target volume is kept: build
            # cache survives image rebuilds).
            try { docker volume rm -f austringer-localci-cargo *> $null } catch { }
        }
        Write-Host "building local CI image $tag ..."
        Invoke-Checked 'docker build' { docker build -t $tag -t austringer-localci:latest (Join-Path $RepoRoot 'ci-local') }
        $imageId = docker image inspect --format '{{.Id}}' $tag
        Write-Host "local CI image id: $imageId"
    }
    return $tag
}

# One ubuntu parity container per phase, exactly like one GH job per
# phase: repo rw-mounted at /src (tests write fixtures under the
# workspace), named volumes for the cargo home and /target. --cpus=4
# mirrors the standard ubuntu-latest runner (4 vCPUs): nextest sizes its
# default parallelism from core count, and on a shared dev host unbounded
# container parallelism starves the wall-clock budget tests
# (austringer-search::index_perf tripped its 15 s/2 s bounds two runs in a
# row under 25+-way parallelism; runner-shaped 4-cpu contention is what
# those bounds were calibrated against).
function Invoke-DockerUbuntu([string]$Mode) {
    $tag = Ensure-LocalCiImage
    Normalize-Lf 'ci-local\run-ubuntu.sh'
    Invoke-Checked "docker run ($Mode)" {
        docker run --rm --init --cpus=4 `
            --env CARGO_TERM_COLOR=never --env NO_COLOR=1 `
            --env CARGO_PROFILE_DEV_DEBUG=0 --env CARGO_PROFILE_TEST_DEBUG=0 `
            -v "$($RepoRoot):/src" -v austringer-localci-cargo:/opt/cargo -v austringer-localci-target:/target `
            -w /src $tag ci-local/run-ubuntu.sh $Mode
    }
}

function Invoke-Phase([string]$Name, [scriptblock]$Body) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    Write-Host "`n===== PHASE: $Name ====="
    $ok = $true
    try {
        & $Body
    } catch {
        $ok = $false
        $script:Failed += $Name
        Write-Warning "PHASE $Name FAIL: $($_.Exception.Message)"
    }
    $sw.Stop()
    $dur = '{0:00}:{1:00}' -f $sw.Elapsed.Minutes, $sw.Elapsed.Seconds
    $verdict = if ($ok) { 'PASS' } else { 'FAIL' }
    Write-Host "===== PHASE: $Name $verdict ($dur) ====="
}

# ── native phase bodies (host is Windows; the windows CI legs) ───────────
function Invoke-NativeTest {
    $archive = Join-Path $env:TEMP 'nextest-archive-win.tar.zst'
    if (Test-Path $archive) { Remove-Item $archive }
    Invoke-Cargo @('nextest', 'archive', '--workspace', '--locked', "--archive-file=$archive")
    Invoke-Cargo @('nextest', 'run', "--archive-file=$archive")
}

function Invoke-NativeFrontends {
    Invoke-Cargo @('build', '--locked', '-p', 'austringer-web', '--example', 'e2e_daemon')
    Push-Location (Join-Path $RepoRoot 'austringer-vscode')
    try {
        Invoke-Checked 'npm ci' { npm ci }
        Invoke-Checked 'npm test (austringer-vscode)' { npm test }
    } finally { Pop-Location }
    $targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $RepoRoot 'target' }
    $env:austringer_E2E_EXE = Join-Path $targetDir 'debug\examples\e2e_daemon.exe'
    $env:NODE_PATH = (npm root -g)  # playwright-core resolves from the global npm tree, as in CI
    if (-not $env:PLAYWRIGHT_CHROMIUM -and -not (Test-Path "$env:LOCALAPPDATA\ms-playwright")) {
        # CI points the override at the runner's Chrome stable; fall back
        # to the same when no ms-playwright cache exists.
        $chrome = 'C:\Program Files\Google\Chrome\Application\chrome.exe'
        if (Test-Path $chrome) { $env:PLAYWRIGHT_CHROMIUM = $chrome }
    }
    Invoke-Checked 'web e2e (run-e2e.mjs)' { node (Join-Path $RepoRoot 'austringer-web\e2e\run-e2e.mjs') }
}

function Invoke-Evals {
    Invoke-Cargo @('run', '-p', 'austringer-evals', '--bin', 'run_sweep', '--', '--conformance')
}

function Invoke-Docs {
    Invoke-Checked 'linkcheck' { powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $RepoRoot '.github\scripts\linkcheck.ps1') }
    # Site build mirrors docs.yml's job (fresh ubuntu, asciidoctor installed
    # at run time, zero-warning gate). PAGEFILES list must stay in sync with
    # the workflow's env block. Output goes to /tmp — the worktree stays clean.
    $docScript = 'set -e; apt-get update -qq && apt-get install -y -qq asciidoctor >/dev/null && mkdir -p /tmp/_site && for f in docs/index.adoc docs/architecture.adoc docs/manual.adoc docs/research-companion.adoc docs/release/leaderboard.adoc docs/release/evals.adoc; do asciidoctor --failure-level WARN -a toc=left -a linkcss -D /tmp/_site "$f"; done && echo "docs site built warning-clean"'
    Invoke-Checked 'asciidoctor site build' { docker run --rm -v "$($RepoRoot):/src" -w /src ubuntu:24.04 bash -c $docScript }
}

function Invoke-Coverage {
    $probe = if (Test-Path $CargoWrapper) { "`"$CargoWrapper`" cargo llvm-cov --version" } else { 'cargo llvm-cov --version' }
    $haveCov = $false
    try { cmd /c $probe *> $null; $haveCov = ($LASTEXITCODE -eq 0) } catch { $haveCov = $false }
    if (-not $haveCov) {
        throw 'cargo-llvm-cov is not installed on this host. CI pins cargo-llvm-cov@0.9.1 (ci.yml coverage job): cargo install --locked cargo-llvm-cov@0.9.1'
    }
    Invoke-Cargo @('llvm-cov', 'nextest', '--workspace', '--locked', '--lcov', "--output-path=$(Join-Path $env:TEMP 'lcov.info')")
    Write-Host "lcov report: $(Join-Path $env:TEMP 'lcov.info')"
}

function Invoke-PerfGate {
    Invoke-Cargo @('build', '--release', '--locked', '-p', 'austringer-cli', '-p', 'austringer-daemon', '-p', 'austringer-bench')
    $rel = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $RepoRoot 'target' }
    $rel = Join-Path $rel 'release'
    Invoke-Cargo @('run', '--release', '--locked', '-q', '-p', 'austringer-bench', '--bin', 'perf-gate', '--',
        '--budgets', 'austringer-bench/budgets.json', '--baseline', 'austringer-bench/baselines/gate_baseline.json',
        '--cli-bin', "$rel\austringer.exe", '--daemon-bin', "$rel\austringer-daemon.exe",
        '--binary', "austringer=$rel\austringer.exe", '--binary', "austringer-daemon=$rel\austringer-daemon.exe",
        '--report', "$($env:TEMP)\perf-report.json")
    Invoke-Cargo @('run', '--release', '--locked', '-q', '-p', 'austringer-bench', '--bin', 'heap-profile', '--features', 'dhat', '--',
        '--check', 'austringer-bench/baselines/heap_profile.json', '--report', "$($env:TEMP)\heap-profile-report.json")
    Write-Host "perf reports: $env:TEMP\perf-report.json, $env:TEMP\heap-profile-report.json"
}

# ── dispatch ──────────────────────────────────────────────────────────────
if ($All) {
    $Phases = @($Phases + @('native-frontends', 'evals', 'docs', 'coverage', 'perf-gate')) | Select-Object -Unique
}
if ($SkipDocker) { $Phases = @($Phases | Where-Object { $_ -notlike 'docker-*' }) }

$total = [Diagnostics.Stopwatch]::StartNew()
foreach ($phase in $Phases) {
    switch ($phase) {
        'docker-lint' { Invoke-Phase $phase { Invoke-DockerUbuntu lint } }
        'docker-test' { Invoke-Phase $phase { Invoke-DockerUbuntu test } }
        'native-test' { Invoke-Phase $phase { Invoke-NativeTest } }
        'native-frontends' { Invoke-Phase $phase { Invoke-NativeFrontends } }
        'evals' { Invoke-Phase $phase { Invoke-Evals } }
        'docs' { Invoke-Phase $phase { Invoke-Docs } }
        'coverage' { Invoke-Phase $phase { Invoke-Coverage } }
        'perf-gate' { Invoke-Phase $phase { Invoke-PerfGate } }
        default { throw "unknown phase: $phase (valid: docker-lint, docker-test, native-test, native-frontends, evals, docs, coverage, perf-gate)" }
    }
}

$totalDur = '{0:00}:{1:00}' -f $total.Elapsed.Minutes, $total.Elapsed.Seconds
Write-Host "`nlocal-ci total elapsed: $totalDur"
if ($script:Failed.Count -gt 0) {
    Write-Warning ("FAILED phases: {0}" -f ($script:Failed -join ', '))
    exit 1
}
Write-Host 'ALL REQUESTED PHASES PASSED'
exit 0
