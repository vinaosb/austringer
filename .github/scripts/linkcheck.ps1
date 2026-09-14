# Link check for the community surface (plan todo 46).
#
# Scope — exactly what the todo's acceptance criteria name:
#   1. Relative links in the Markdown storefront (repo-root *.md plus any
#      Markdown under docs/). External http(s)/mailto links are out of
#      scope; this gate proves in-repo targets resolve.
#   2. AsciiDoc pages under docs/:
#        - include:: targets resolve to files,
#        - xref:/<<>> references resolve to defined stable IDs
#          (explicit [[id]] / [#id] / anchor:id[] anchors only — the
#          docs convention is that every cross-referenced anchor is
#          explicit, never a generated title id),
#        - relative link: targets resolve to files.
#
# Runs locally (Windows PowerShell 5.1 and pwsh 7) and in CI
# (.github/workflows/docs.yml, ubuntu pwsh). Exit 0 = zero broken;
# exit 1 prints every finding first.

param(
    [string]$RepoRoot = ""
)

if (-not $RepoRoot) {
    # .github/scripts/linkcheck.ps1 -> repo root two levels up.
    $RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
}
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path

$errors = New-Object System.Collections.Generic.List[string]

function Resolve-LinkTarget {
    param([string]$FromFile, [string]$Target)
    # Returns $true when $Target (relative to $FromFile's directory) exists.
    if ($Target -eq "" -or $Target -eq ".") { return $true }
    $decoded = [Uri]::UnescapeDataString($Target)
    $baseDir = Split-Path -Parent $FromFile
    $joined = Join-Path $baseDir $decoded
    return (Test-Path -LiteralPath $joined -PathType Leaf) -or
           (Test-Path -LiteralPath $joined -PathType Container)
}

# ---------------------------------------------------------------------------
# 1. Markdown storefront: repo-root *.md + docs/**/*.md
# ---------------------------------------------------------------------------
$mdFiles = @()
$mdFiles += Get-ChildItem -LiteralPath $RepoRoot -Filter *.md -File |
    Where-Object { $_.Name -ne "prompts.md" } # local working notes, not storefront
$docsDir = Join-Path $RepoRoot "docs"
if (Test-Path -LiteralPath $docsDir) {
    $mdFiles += Get-ChildItem -LiteralPath $docsDir -Filter *.md -File -Recurse
}

$mdLinkPattern = '(?<!\!)\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)'

foreach ($file in $mdFiles) {
    $lineNo = 0
    foreach ($line in (Get-Content -LiteralPath $file.FullName)) {
        $lineNo++
        foreach ($match in [regex]::Matches($line, $mdLinkPattern)) {
            $target = $match.Groups[1].Value
            if ($target -match '^(https?:|mailto:|data:|#)') { continue }
            $pathPart = ($target -split '#')[0]
            if ($pathPart -eq '') { continue } # same-file fragment
            $pathPart = ($pathPart -split '\?')[0]
            if (-not (Resolve-LinkTarget -FromFile $file.FullName -Target $pathPart)) {
                $rel = $file.FullName.Substring($RepoRoot.Length + 1)
                $errors.Add("markdown broken link: ${rel}:${lineNo} -> $target")
            }
        }
    }
}

# ---------------------------------------------------------------------------
# 2. AsciiDoc pages under docs/
# ---------------------------------------------------------------------------
$adocFiles = @()
if (Test-Path -LiteralPath $docsDir) {
    $adocFiles = Get-ChildItem -LiteralPath $docsDir -Filter *.adoc -File -Recurse
}

# Collect explicit IDs per file and globally (intra-page xrefs may resolve
# into content included from partials, so the global set is the safe judge).
$idsByFile = @{}
$allIds = New-Object 'System.Collections.Generic.HashSet[string]'
foreach ($file in $adocFiles) {
    $ids = New-Object 'System.Collections.Generic.HashSet[string]'
    $content = Get-Content -LiteralPath $file.FullName -Raw
    foreach ($m in [regex]::Matches($content, '\[\[([A-Za-z0-9_.:-]+)(?:,[^\]]*)?\]\]')) {
        [void]$ids.Add($m.Groups[1].Value)
    }
    foreach ($m in [regex]::Matches($content, '(?:^|\s)\[#([A-Za-z0-9_.:-]+)(?:,[^\]]*)?\]')) {
        [void]$ids.Add($m.Groups[1].Value)
    }
    foreach ($m in [regex]::Matches($content, 'anchor:([A-Za-z0-9_.:-]+)\[')) {
        [void]$ids.Add($m.Groups[1].Value)
    }
    $idsByFile[$file.FullName] = $ids
    foreach ($id in $ids) { [void]$allIds.Add($id) }
}

function Test-AdocRef {
    param([string]$FromFile, [string]$Ref, [string]$Kind, [int]$LineNo)

    # File-qualified reference: path.adoc or path.adoc#anchor
    if ($Ref -match '\.adoc(?:#([A-Za-z0-9_.:-]+))?$') {
        $anchor = $Matches[1]
        $filePart = ($Ref -split '#')[0]
        if (-not (Resolve-LinkTarget -FromFile $FromFile -Target $filePart)) {
            $errors.Add("${FromFile}:${LineNo} broken $Kind (missing file): $Ref")
            return
        }
        if ($anchor) {
            $targetFile = Join-Path (Split-Path -Parent $FromFile) ([Uri]::UnescapeDataString($filePart))
            $targetFile = [System.IO.Path]::GetFullPath($targetFile)
            if ($idsByFile.ContainsKey($targetFile)) {
                if (-not $idsByFile[$targetFile].Contains($anchor)) {
                    $errors.Add("${FromFile}:${LineNo} broken $Kind (anchor '$anchor' not explicit in $filePart): $Ref")
                }
            } else {
                $errors.Add("${FromFile}:${LineNo} broken $Kind (anchor check: $filePart not scanned): $Ref")
            }
        }
        return
    }

    # Non-.adoc file reference with a path separator or extension: file check.
    if ($Ref -match '[/\\]' -or $Ref -match '\.[A-Za-z0-9]+$') {
        $filePart = ($Ref -split '#')[0]
        if ($filePart -ne '' -and -not (Resolve-LinkTarget -FromFile $FromFile -Target $filePart)) {
            $errors.Add("${FromFile}:${LineNo} broken $Kind (missing file): $Ref")
        }
        return
    }

    # Bare id: must be an explicit anchor somewhere in the docs set.
    if (-not $allIds.Contains($Ref)) {
        $errors.Add("${FromFile}:${LineNo} broken $Kind (no explicit anchor '$Ref'): $Ref")
    }
}

foreach ($file in $adocFiles) {
    $rel = $file.FullName
    $lineNo = 0
    foreach ($line in (Get-Content -LiteralPath $file.FullName)) {
        $lineNo++
        foreach ($m in [regex]::Matches($line, 'include::([^\[\s]+)\[')) {
            if (-not (Resolve-LinkTarget -FromFile $file.FullName -Target $m.Groups[1].Value)) {
                $errors.Add("${rel}:${lineNo} broken include: $($m.Groups[1].Value)")
            }
        }
        foreach ($m in [regex]::Matches($line, 'xref:([^\[\s]+)\[')) {
            Test-AdocRef -FromFile $file.FullName -Ref $m.Groups[1].Value -Kind "xref" -LineNo $lineNo
        }
        foreach ($m in [regex]::Matches($line, '<<([^>,\]]+)(?:,[^>]*)?>>')) {
            Test-AdocRef -FromFile $file.FullName -Ref $m.Groups[1].Value.Trim() -Kind "xref-short" -LineNo $lineNo
        }
        foreach ($m in [regex]::Matches($line, 'link:([^\[\s]+)\[')) {
            $target = $m.Groups[1].Value
            if ($target -match '^(https?:|mailto:|data:)') { continue }
            if ($target -eq '#') { continue }
            Test-AdocRef -FromFile $file.FullName -Ref $target -Kind "link" -LineNo $lineNo
        }
    }
}

# ---------------------------------------------------------------------------
$scanned = $mdFiles.Count + $adocFiles.Count
if ($errors.Count -gt 0) {
    Write-Output "linkcheck: FAILED ($($errors.Count) finding(s) across $scanned scanned files)"
    $errors | ForEach-Object { Write-Output "  $_" }
    exit 1
}
Write-Output "linkcheck: OK ($($mdFiles.Count) Markdown + $($adocFiles.Count) AsciiDoc files, zero broken relative links / includes / xrefs)"
exit 0
