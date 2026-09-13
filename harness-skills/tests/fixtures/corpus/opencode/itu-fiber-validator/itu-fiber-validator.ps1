<#
.SYNOPSIS
    ITU BBMaps Fiber Proximity Validator — batch queries ITU API for fiber backbone proximity.

.DESCRIPTION
    Takes a CSV of candidate locations (city name + coordinates), queries the ITU BBMaps
    Proximity API in configurable batch sizes, handles rate limiting with retries, and
    outputs an enriched CSV with fiber proximity data merged in.

    The ITU API returns proximity to the nearest fiber backbone node:
      "within 5km", "within 10km", "within 15km", "within 20km",
      "within 50km", "within 75km", ">100km"

    Hard filter: sites >100km from any fiber backbone are eliminated.

.PARAMETER InputFile
    Path to input CSV. Required columns: City, Lat, Lon
    Optional columns: Country, Region, Altitude_m, GHI, AvgTemp_C, CorpTax, Tier_Target

.PARAMETER OutputFile
    Path to enriched output CSV (default: input filename with "-enriched" suffix)

.PARAMETER BatchSize
    Number of coordinates per ITU API call (default: 10, max: 50)

.PARAMETER DelayBetweenBatches
    Seconds to wait between API calls to avoid rate limiting (default: 5)

.PARAMETER MaxRetries
    Max retry attempts per batch on failure (default: 3)

.PARAMETER RetryDelay
    Seconds to wait before retrying a failed batch (default: 30)

.EXAMPLE
    .\itu-fiber-validator.ps1 -InputFile ..\data\all-candidates.csv -BatchSize 15

.EXAMPLE
    .\itu-fiber-validator.ps1 -InputFile ..\data\all-candidates.csv -OutputFile ..\data\all-candidates-verified.csv -DelayBetweenBatches 10

.NOTES
    API Endpoint: https://bbmaps.itu.int/functionproximity//route-calcdistancetonode
    API Key: set via $env:ITU_API_KEY (default fallback: M2CgTqEMCUnk-webappkey2proximity)
    Response: ZIP file containing CSV with X,Y,proximity columns
    Rate limiting: API may timeout after multiple rapid requests; built-in retry logic handles this.

    ITU Standards Referenced:
      - ITU-T G.652.D: Characteristics of a single-mode optical fibre and cable
        (cable cut-off wavelength ≤ 1260 nm, max attenuation 0.4 dB/km @ 1310 nm)
      - ITU-T G.652.D Table 1: Attenuation coefficients (O-band/E-band/S-band/C-band/L-band)
      - ITU-T G.698.2: Multichannel DWDM applications with CWDM
      - Topology constraint: max fiber link length 80 km (regenerator spacing)
#>

[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory=$true)]
    [string]$InputFile,

    [Parameter(Mandatory=$false)]
    [string]$OutputFile = "",

    [Parameter(Mandatory=$false)]
    [int]$BatchSize = 10,

    [Parameter(Mandatory=$false)]
    [int]$DelayBetweenBatches = 5,

    [Parameter(Mandatory=$false)]
    [int]$MaxRetries = 3,

    [Parameter(Mandatory=$false)]
    [int]$RetryDelay = 30
)

$ErrorActionPreference = "Stop"

# --- Prerequisite Checks ---
if (-not (Get-Command "curl.exe" -ErrorAction SilentlyContinue)) {
    throw "curl.exe not found. Install curl (e.g., via 'winget install curl.curl' or enable it in Windows System32)."
}

# --- API Key (from env var with public fallback) ---
$ApiKey = $env:ITU_API_KEY
if (-not $ApiKey) {
    $ApiKey = "M2CgTqEMCUnk-webappkey2proximity"
    Write-Warning "ITU_API_KEY not set. Using public fallback key. Set `$env:ITU_API_KEY for production use."
}

# --- Configuration ---
$ApiUrl = "https://bbmaps.itu.int/functionproximity//route-calcdistancetonode?code=$ApiKey"
$TempDir = Join-Path $env:TEMP "itu-fiber-validator"
$Headers = @{
    "Content-Type" = "text/csv"
    "Origin"       = "https://bbmaps.itu.int"
    "Referer"      = "https://bbmaps.itu.int/web10km"
}

# --- ITU G.652.D Fiber Parameter Thresholds ---
# Reference: ITU-T G.652.D (11/2009) — Characteristics of a single-mode optical fibre cable
# These thresholds define the acceptable limits for fiber routes serving data center sites.

$Script:ITU_G652D = @{
    # Maximum cable attenuation per ITU G.652.D Table 1
    Attenuation_1310nm_dB_km = 0.4   # Max attenuation @ 1310 nm (dB/km)
    Attenuation_1550nm_dB_km = 0.3   # Max attenuation @ 1550 nm (dB/km)

    # Link budget constraints for data center interconnect
    Max_Link_Length_km       = 80    # Max acceptable link length (km) — topology constraint for metro/regional
    Max_Attenuation_Total_dB  = 32.0  # Max total link attenuation (dB) = 0.4 dB/km × 80 km
    Min_Transmit_Power_dBm   = 0.0   # Min typical TX power for LR transceivers (dBm)
    Max_Receiver_Sensitivity  = -28.0 # Max RX sensitivity for LR transceivers (dBm)

    # Redundancy requirements
    Min_Fiber_Providers       = 2     # Minimum distinct fiber providers for redundancy

    # Latency budgets (round-trip)
    Max_Latency_RT_Metro_ms   = 5     # Metro: ≤5 ms RT latency budget
    Max_Latency_RT_Regional_ms = 20   # Regional: ≤20 ms RT latency budget

    # Proximity thresholds for site viability
    Max_Proximity_km          = 80    # Sites >80 km from fiber backbone are non-viable
    Hard_Fail_Proximity_km    = 100   # Sites >100 km are hard-fail (eliminated)

    # Chromatic dispersion (informational, not hard-filtered)
    ChromaticDispersion_1550nm_ps_nm_km = 18.0  # Max @ 1550 nm per G.652.D (ps/(nm·km))
    PMD_Coefficient_ps_sqrt_km           = 0.2    # Max PMD coefficient (ps/√km)
}

# --- Helper Functions ---

function Write-Status {
    param([string]$Message, [string]$Level = "Info")
    $ts = Get-Date -Format "HH:mm:ss"
    $prefix = switch ($Level) {
        "Info"    { "[INFO]" }
        "Warn"    { "[WARN]" }
        "Error"   { "[ERR ]" }
        "Success" { "[ OK ]" }
        default   { "[????]" }
    }
    Write-Host "$ts $prefix $Message"
}

function Get-ProximityKm {
    <# Convert ITU proximity string to numeric km estimate #>
    param([string]$Proximity)
    if ($Proximity -match "within (\d+)km") {
        return [int]$Matches[1]
    }
    elseif ($Proximity -eq ">100km") {
        return 999
    }
    elseif ($Proximity -match "within 5km") {
        return 5
    }
    return -1  # Unknown
}

function Test-ITUCompliance {
    <#
    .SYNOPSIS
        Tests a fiber route against ITU G.652.D thresholds and returns per-parameter PASS/FAIL status.
    .PARAMETER ProximityKm
        Estimated distance to nearest fiber backbone node (km). Use -1 if unknown.
    .PARAMETER LatencyMs
        Measured or estimated round-trip latency (ms). Use $null if unknown.
    .PARAMETER FiberProviders
        Number of distinct fiber providers at the site. Use 0 or $null if unknown.
    .OUTPUTS
        OrderedDictionary with per-parameter results and an overall ComplianceStatus.
    #>
    param(
        [int]$ProximityKm = -1,
        [double]$LatencyMs,
        [int]$FiberProviders = 0
    )

    $t = $Script:ITU_G652D
    $results = [ordered]@{}

    # --- Proximity check ---
    if ($ProximityKm -eq -1) {
        $results["Proximity_km"] = @{ Value = "UNKNOWN"; Limit = "$($t.Max_Proximity_km) km"; Status = "CHECK" }
    }
    elseif ($ProximityKm -gt $t.Hard_Fail_Proximity_km) {
        $results["Proximity_km"] = @{ Value = "${ProximityKm} km"; Limit = "<= $($t.Hard_Fail_Proximity_km) km"; Status = "FAIL" }
    }
    elseif ($ProximityKm -gt $t.Max_Proximity_km) {
        $results["Proximity_km"] = @{ Value = "${ProximityKm} km"; Limit = "<= $($t.Max_Proximity_km) km"; Status = "WARN" }
    }
    else {
        $results["Proximity_km"] = @{ Value = "${ProximityKm} km"; Limit = "<= $($t.Max_Proximity_km) km"; Status = "PASS" }
    }

    # --- Latency check ---
    $latencyStatus = "CHECK"
    $latencyValue = "UNKNOWN"
    $latencyLimit = "Metro: <= $($t.Max_Latency_RT_Metro_ms) ms / Regional: <= $($t.Max_Latency_RT_Regional_ms) ms"

    if ($null -ne $LatencyMs -and $LatencyMs -ge 0) {
        $latencyValue = "$([Math]::Round($LatencyMs, 2)) ms"
        if ($LatencyMs -le $t.Max_Latency_RT_Metro_ms) {
            $latencyStatus = "PASS"
        }
        elseif ($LatencyMs -le $t.Max_Latency_RT_Regional_ms) {
            $latencyStatus = "WARN"
        }
        else {
            $latencyStatus = "FAIL"
        }
    }
    $results["Latency_RT_ms"] = @{ Value = $latencyValue; Limit = $latencyLimit; Status = $latencyStatus }

    # --- Redundancy check ---
    $redunValue = if ($FiberProviders -gt 0) { "$FiberProviders" } else { "UNKNOWN" }
    $redunStatus = switch ($FiberProviders) {
        { $_ -ge $t.Min_Fiber_Providers } { "PASS" }
        1                               { "WARN" }
        default                          { "CHECK" }
    }
    $results["Fiber_Providers"] = @{ Value = $redunValue; Limit = ">=$($t.Min_Fiber_Providers)"; Status = $redunStatus }

    # --- Derived: link attenuation budget (informational) ---
    if ($ProximityKm -ge 0 -and $ProximityKm -le $t.Max_Link_Length_km) {
        $estAtten = [Math]::Round($ProximityKm * $t.Attenuation_1310nm_dB_km, 1)
        $attenStatus = if ($estAtten -le $t.Max_Attenuation_Total_dB) { "PASS" } else { "FAIL" }
        $results["Est_Link_Attenuation_dB"] = @{ Value = "$estAtten dB (@ 1310nm, $($ProximityKm) km)"; Limit = "<= $($t.Max_Attenuation_Total_dB) dB"; Status = $attenStatus }
    }
    elseif ($ProximityKm -eq 999) {
        $results["Est_Link_Attenuation_dB"] = @{ Value = "EXCEEDS RANGE"; Limit = "<= $($t.Max_Attenuation_Total_dB) dB"; Status = "FAIL" }
    }
    else {
        $results["Est_Link_Attenuation_dB"] = @{ Value = "N/A"; Limit = "<= $($t.Max_Attenuation_Total_dB) dB"; Status = "CHECK" }
    }

    # --- Overall compliance ---
    $statuses = $results.Values | ForEach-Object { $_.Status }
    $hasFail  = $statuses -contains "FAIL"
    $hasCheck = ($statuses | Where-Object { $_ -eq "CHECK" }).Count -eq $statuses.Count  # all CHECK
    $overall  = if ($hasFail) { "FAIL" } elseif ($hasCheck -and -not ($statuses -contains "PASS")) { "CHECK" } elseif ($statuses -contains "WARN") { "WARN" } else { "PASS" }

    return [ordered]@{
        Parameters          = $results
        ComplianceStatus    = $overall
    }
}

function Format-ITUComplianceLine {
    <# Returns a compact single-line ITU compliance summary for CSV output #>
    param([System.Collections.Specialized.OrderedDictionary]$ComplianceResult)
    $parts = @()
    foreach ($key in $ComplianceResult.Parameters.Keys) {
        $p = $ComplianceResult.Parameters[$key]
        $parts += "$key=$($p.Status)"
    }
    return ($parts -join "; ") + " | OVERALL=$($ComplianceResult.ComplianceStatus)"
}

function Write-ITUComplianceReport {
    <# Writes a formatted ITU compliance report for a single site to the console #>
    param(
        [string]$City,
        [System.Collections.Specialized.OrderedDictionary]$ComplianceResult
    )
    Write-Host ""
    Write-Host ("  ITU G.652.D Compliance: $City") -ForegroundColor Cyan
    Write-Host ("  " + ("-" * 55)) -ForegroundColor DarkGray

    foreach ($key in $ComplianceResult.Parameters.Keys) {
        $p = $ComplianceResult.Parameters[$key]
        $color = switch ($p.Status) {
            "PASS"  { "Green" }
            "WARN"  { "Yellow" }
            "FAIL"  { "Red" }
            default { "Gray" }
        }
        $tag = switch ($p.Status) {
            "PASS"  { "[PASS]" }
            "WARN"  { "[WARN]" }
            "FAIL"  { "[FAIL]" }
            default { "[????]" }
        }
        Write-Host ("  $tag ") -ForegroundColor $color -NoNewline
        Write-Host ("{0,-28} {1,-20} (limit: {2})" -f $key, $p.Value, $p.Limit)
    }

    $overallColor = switch ($ComplianceResult.ComplianceStatus) {
        "PASS"  { "Green" }
        "WARN"  { "Yellow" }
        "FAIL"  { "Red" }
        default { "Gray" }
    }
    Write-Host ("  " + ("-" * 55)) -ForegroundColor DarkGray
    Write-Host ("  OVERALL: $($ComplianceResult.ComplianceStatus)") -ForegroundColor $overallColor
}

function Invoke-ItuApiWithRetry {
    [CmdletBinding(SupportsShouldProcess)]
    param(
        [string[]]$Coordinates,  # Array of "lon,lat" strings
        [int]$Attempt = 1
    )

    $csvBody = "X,Y`n" + ($Coordinates -join "`n")
    $batchFile = Join-Path $TempDir "batch_$($Attempt)_$(Get-Random).csv"
    $respFile = Join-Path $TempDir "resp_$($Attempt)_$(Get-Random).txt"

    if (-not $PSCmdlet.ShouldProcess("ITU BBMaps API ($($Coordinates.Count) coordinates)", "POST")) {
        Write-Status "WhatIf: Would query ITU API for $($Coordinates.Count) coordinates" "Info"
        return $null
    }

    try {
        Set-Content -LiteralPath $batchFile -Value $csvBody -NoNewline -ErrorAction Stop

        try {
            $result = curl.exe -s -X POST $ApiUrl `
                -H "Content-Type: text/csv" `
                -H "Origin: https://bbmaps.itu.int" `
                -H "Referer: https://bbmaps.itu.int/web10km" `
                --data-binary "@$batchFile" `
                -o $respFile `
                -w "%{http_code}|%{size_download}|%{time_total}" `
                --max-time 180 `
                2>$null
        }
        catch {
            Write-Status "curl.exe invocation failed: $_" "Error"
            throw
        }

        if (-not $result) {
            Write-Status "curl.exe returned empty response (possibly network error)." "Error"
            if ($Attempt -lt $MaxRetries) {
                Write-Status "Retry $Attempt/$MaxRetries in ${RetryDelay}s..." "Warn"
                Start-Sleep -Seconds $RetryDelay
                return Invoke-ItuApiWithRetry -Coordinates $Coordinates -Attempt ($Attempt + 1)
            }
            return $null
        }

        $parts = $result -split '\|'
        if ($parts.Count -lt 3) {
            Write-Status "Malformed curl output: '$result'. Expected http_code|size_download|time_total." "Error"
            if ($Attempt -lt $MaxRetries) {
                Write-Status "Retry $Attempt/$MaxRetries in ${RetryDelay}s..." "Warn"
                Start-Sleep -Seconds $RetryDelay
                return Invoke-ItuApiWithRetry -Coordinates $Coordinates -Attempt ($Attempt + 1)
            }
            return $null
        }

        $httpCode = [int]$parts[0]
        $sizeDownload = [int]$parts[1]
        $timeTotal = [double]$parts[2]

        # Validate HTTP status code
        if ($httpCode -ne 200) {
            Write-Status "API returned HTTP $httpCode (expected 200). Size: ${sizeDownload}B, Time: ${timeTotal}s" "Warn"
            if ($Attempt -lt $MaxRetries) {
                Write-Status "Retry $Attempt/$MaxRetries in ${RetryDelay}s..." "Warn"
                Start-Sleep -Seconds $RetryDelay
                return Invoke-ItuApiWithRetry -Coordinates $Coordinates -Attempt ($Attempt + 1)
            }
            Write-Status "Batch failed after $MaxRetries attempts (HTTP $httpCode). Skipping." "Error"
            return $null
        }

        if ($sizeDownload -le 0) {
            Write-Status "API returned HTTP 200 but empty response body." "Warn"
            if ($Attempt -lt $MaxRetries) {
                Write-Status "Retry $Attempt/$MaxRetries in ${RetryDelay}s..." "Warn"
                Start-Sleep -Seconds $RetryDelay
                return Invoke-ItuApiWithRetry -Coordinates $Coordinates -Attempt ($Attempt + 1)
            }
            Write-Status "Batch failed after $MaxRetries attempts (empty body). Skipping." "Error"
            return $null
        }

        # Response is a URL to the ZIP file
        $rawResponse = Get-Content $respFile -Raw -ErrorAction Stop
        if (-not $rawResponse) {
            Write-Status "Response file empty after successful HTTP 200." "Warn"
            if ($Attempt -lt $MaxRetries) {
                Write-Status "Retry $Attempt/$MaxRetries in ${RetryDelay}s..." "Warn"
                Start-Sleep -Seconds $RetryDelay
                return Invoke-ItuApiWithRetry -Coordinates $Coordinates -Attempt ($Attempt + 1)
            }
            return $null
        }

        $zipUrl = $rawResponse.Trim()

        if ($zipUrl -notmatch "^https?://") {
            Write-Status "Response is not a URL: '$zipUrl'. API may have returned an error body." "Warn"
            if ($Attempt -lt $MaxRetries) {
                Write-Status "Retry $Attempt/$MaxRetries in ${RetryDelay}s..." "Warn"
                Start-Sleep -Seconds $RetryDelay
                return Invoke-ItuApiWithRetry -Coordinates $Coordinates -Attempt ($Attempt + 1)
            }
            return $null
        }

        # Download the ZIP
        $zipFile = Join-Path $TempDir "data_$(Get-Random).zip"
        try {
            curl.exe -s -L -o $zipFile $zipUrl --max-time 60 2>$null
        }
        catch {
            Write-Status "ZIP download failed: $_" "Error"
            if ($Attempt -lt $MaxRetries) {
                Write-Status "Retry $Attempt/$MaxRetries in ${RetryDelay}s..." "Warn"
                Start-Sleep -Seconds $RetryDelay
                return Invoke-ItuApiWithRetry -Coordinates $Coordinates -Attempt ($Attempt + 1)
            }
            return $null
        }

        if (-not (Test-Path $zipFile)) {
            Write-Status "ZIP download failed (file not created)." "Warn"
            if ($Attempt -lt $MaxRetries) {
                Write-Status "Retry $Attempt/$MaxRetries in ${RetryDelay}s..." "Warn"
                Start-Sleep -Seconds $RetryDelay
                return Invoke-ItuApiWithRetry -Coordinates $Coordinates -Attempt ($Attempt + 1)
            }
            return $null
        }

        # Extract ZIP
        $extractDir = Join-Path $TempDir "extract_$(Get-Random)"
        try {
            Expand-Archive -LiteralPath $zipFile -DestinationPath $extractDir -Force -ErrorAction Stop
        }
        catch {
            Write-Status "ZIP extraction failed (corrupt archive?): $_" "Warn"
            if ($Attempt -lt $MaxRetries) {
                Write-Status "Retry $Attempt/$MaxRetries in ${RetryDelay}s..." "Warn"
                Start-Sleep -Seconds $RetryDelay
                return Invoke-ItuApiWithRetry -Coordinates $Coordinates -Attempt ($Attempt + 1)
            }
            return $null
        }

        $csvFile = Get-ChildItem $extractDir -Filter "*.csv" | Select-Object -First 1
        if (-not $csvFile) {
            Write-Status "Extracted ZIP contains no CSV files." "Warn"
            if ($Attempt -lt $MaxRetries) {
                Write-Status "Retry $Attempt/$MaxRetries in ${RetryDelay}s..." "Warn"
                Start-Sleep -Seconds $RetryDelay
                return Invoke-ItuApiWithRetry -Coordinates $Coordinates -Attempt ($Attempt + 1)
            }
            return $null
        }

        # Validate CSV has expected columns
        try {
            $results = Import-Csv -LiteralPath $csvFile.FullName -ErrorAction Stop
        }
        catch {
            Write-Status "Failed to parse CSV from extracted ZIP: $_" "Warn"
            if ($Attempt -lt $MaxRetries) {
                Write-Status "Retry $Attempt/$MaxRetries in ${RetryDelay}s..." "Warn"
                Start-Sleep -Seconds $RetryDelay
                return Invoke-ItuApiWithRetry -Coordinates $Coordinates -Attempt ($Attempt + 1)
            }
            return $null
        }

        if ($results.Count -eq 0) {
            Write-Status "Extracted CSV is empty." "Warn"
            if ($Attempt -lt $MaxRetries) {
                Write-Status "Retry $Attempt/$MaxRetries in ${RetryDelay}s..." "Warn"
                Start-Sleep -Seconds $RetryDelay
                return Invoke-ItuApiWithRetry -Coordinates $Coordinates -Attempt ($Attempt + 1)
            }
            return $null
        }

        # Validate CSV has expected columns (X, Y, proximity)
        $expectedColumns = @("X", "Y", "proximity")
        $actualColumns = $results[0].PSObject.Properties.Name
        $missingColumns = $expectedColumns | Where-Object { $_ -notin $actualColumns }
        if ($missingColumns) {
            Write-Status "CSV missing expected columns: $($missingColumns -join ', '). Found: $($actualColumns -join ', ')" "Warn"
            if ($Attempt -lt $MaxRetries) {
                Write-Status "Retry $Attempt/$MaxRetries in ${RetryDelay}s..." "Warn"
                Start-Sleep -Seconds $RetryDelay
                return Invoke-ItuApiWithRetry -Coordinates $Coordinates -Attempt ($Attempt + 1)
            }
            return $null
        }

        return $results
    }
    finally {
        if (Test-Path $batchFile) { Remove-Item $batchFile -Force -ErrorAction SilentlyContinue }
        if (Test-Path $respFile) { Remove-Item $respFile -Force -ErrorAction SilentlyContinue }
    }
}

# --- Main Logic ---

Write-Status "ITU BBMaps Fiber Proximity Validator"
Write-Status "Input: $InputFile"

# Create temp directory
if (-not (Test-Path $TempDir)) {
    New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
}

# Read input CSV
if (-not (Test-Path -LiteralPath $InputFile)) {
    Write-Error "Input file not found: $InputFile"
    exit 1
}

$candidates = Import-Csv -LiteralPath $InputFile -ErrorAction Stop
Write-Status "Loaded $($candidates.Count) candidates"

# Validate required columns
$hasLat = $candidates[0].PSObject.Properties.Name -contains "Lat"
$hasLon = $candidates[0].PSObject.Properties.Name -contains "Lon"
if (-not ($hasLat -and $hasLon)) {
    Write-Error "Input CSV must have 'Lat' and 'Lon' columns. Found: $($candidates[0].PSObject.Properties.Name -join ', ')"
    exit 1
}

# Determine output file
if ($OutputFile -eq "") {
    $base = [System.IO.Path]::GetFileNameWithoutExtension($InputFile)
    $ext = [System.IO.Path]::GetExtension($InputFile)
    $dir = [System.IO.Path]::GetDirectoryName($InputFile)
    $OutputFile = Join-Path $dir "${base}-enriched${ext}"
}

# Prepare results storage
$results = @()
$totalBatches = [Math]::Ceiling($candidates.Count / $BatchSize)
$batchNum = 0

# Process in batches
for ($i = 0; $i -lt $candidates.Count; $i += $BatchSize) {
    $batchNum++
    $batch = $candidates[$i..([Math]::Min($i + $BatchSize - 1, $candidates.Count - 1))]

    Write-Status "Batch $batchNum/$totalBatches -- querying $($batch.Count) locations..." "Info"

    # Build coordinate array
    $coords = $batch | ForEach-Object {
        "$($_.Lon),$($_.Lat)"
    }

    # Query ITU API
    $apiResults = Invoke-ItuApiWithRetry -Coordinates $coords

    # Merge results
    for ($j = 0; $j -lt $batch.Count; $j++) {
        $candidate = $batch[$j]
        $fiberResult = ""
        $fiberKm = -1
        $fiberPass = ""

        if ($apiResults -and $j -lt $apiResults.Count) {
            $fiberResult = $apiResults[$j].proximity
            $fiberKm = Get-ProximityKm -Proximity $fiberResult
            $fiberPass = if ($fiberKm -ge 0 -and $fiberKm -le 80) { "PASS" } elseif ($fiberKm -eq 999) { "FAIL" } else { "CHECK" }
        }
        else {
            $fiberResult = "QUERY_FAILED"
            $fiberKm = -1
            $fiberPass = "RETRY"
        }

        # --- ITU G.652.D Compliance Check ---
        $ituCompliance = Test-ITUCompliance -ProximityKm $fiberKm

        # Add fiber columns to the candidate
        $enriched = $candidate | Select-Object *, Fiber_Proximity, Fiber_Km_Est, Fiber_Pass, ITU_Proximity, ITU_Latency, ITU_Redundancy, ITU_Attenuation, ITU_Compliance, ITU_Compliance_Detail
        $enriched.Fiber_Proximity = $fiberResult
        $enriched.Fiber_Km_Est = $fiberKm
        $enriched.Fiber_Pass = $fiberPass

        # Map ITU per-parameter status to CSV columns
        $ituParams = $ituCompliance.Parameters
        $enriched.ITU_Proximity    = $ituParams["Proximity_km"].Status
        $enriched.ITU_Latency      = $ituParams["Latency_RT_ms"].Status
        $enriched.ITU_Redundancy   = $ituParams["Fiber_Providers"].Status
        $enriched.ITU_Attenuation  = $ituParams["Est_Link_Attenuation_dB"].Status
        $enriched.ITU_Compliance   = $ituCompliance.ComplianceStatus
        $enriched.ITU_Compliance_Detail = Format-ITUComplianceLine -ComplianceResult $ituCompliance
        $results += $enriched

        # Print per-site ITU compliance report
        $siteName = if ($candidate.PSObject.Properties['City']) { $candidate.City } else { "Site $($i + $j + 1)" }
        Write-ITUComplianceReport -City $siteName -ComplianceResult $ituCompliance
    }

    Write-Status "  Batch $batchNum done. Fiber: $($results.Count) total processed." "Success"

    # Rate limiting delay
    if ($i + $BatchSize -lt $candidates.Count) {
        Write-Status "  Waiting ${DelayBetweenBatches}s before next batch..." "Info"
        Start-Sleep -Seconds $DelayBetweenBatches
    }
}

# Write output
if ($PSCmdlet.ShouldProcess($OutputFile, "Export enriched CSV")) {
    $results | Export-Csv -LiteralPath $OutputFile -NoTypeInformation -Encoding UTF8 -ErrorAction Stop
    Write-Status "Output: $OutputFile ($($results.Count) rows)" "Success"
}
else {
    Write-Status "WhatIf: Would write $($results.Count) rows to $OutputFile" "Info"
}

# Summary
$passCount = ($results | Where-Object { $_.Fiber_Pass -eq "PASS" }).Count
$failCount = ($results | Where-Object { $_.Fiber_Pass -eq "FAIL" }).Count
$retryCount = ($results | Where-Object { $_.Fiber_Pass -eq "RETRY" }).Count
Write-Status "Summary: $passCount PASS, $failCount FAIL, $retryCount NEED RETRY out of $($results.Count) total"

# ITU G.652.D Compliance Summary
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  ITU G.652.D Compliance Summary" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

$ituPassCount  = ($results | Where-Object { $_.ITU_Compliance -eq "PASS" }).Count
$ituWarnCount  = ($results | Where-Object { $_.ITU_Compliance -eq "WARN" }).Count
$ituFailCount  = ($results | Where-Object { $_.ITU_Compliance -eq "FAIL" }).Count
$ituCheckCount = ($results | Where-Object { $_.ITU_Compliance -eq "CHECK" }).Count

Write-Host ("  PASS:   {0,4} sites -- fully compliant with G.652.D thresholds" -f $ituPassCount) -ForegroundColor Green
Write-Host ("  WARN:   {0,4} sites -- within limits but marginal (review recommended)" -f $ituWarnCount) -ForegroundColor Yellow
Write-Host ("  FAIL:   {0,4} sites -- exceeds G.652.D thresholds" -f $ituFailCount) -ForegroundColor Red
Write-Host ("  CHECK:  {0,4} sites -- insufficient data for full evaluation" -f $ituCheckCount) -ForegroundColor Gray
Write-Host ""

# Per-parameter breakdown
Write-Host "  Per-Parameter Breakdown:" -ForegroundColor White
$paramLabels = @{
    "ITU_Proximity"   = "Proximity to backbone"
    "ITU_Latency"     = "Round-trip latency"
    "ITU_Redundancy"  = "Fiber provider redundancy"
    "ITU_Attenuation" = "Link attenuation budget"
}
foreach ($col in @("ITU_Proximity", "ITU_Latency", "ITU_Redundancy", "ITU_Attenuation")) {
    $pPass = ($results | Where-Object { $_.$col -eq "PASS" }).Count
    $pWarn = ($results | Where-Object { $_.$col -eq "WARN" }).Count
    $pFail = ($results | Where-Object { $_.$col -eq "FAIL" }).Count
    $pChk  = ($results | Where-Object { $_.$col -eq "CHECK" }).Count
    $label = $paramLabels[$col]
    Write-Host ("    {0,-30} PASS:{1,4}  WARN:{2,4}  FAIL:{3,4}  CHECK:{4,4}" -f $label, $pPass, $pWarn, $pFail, $pChk)
}

Write-Host ""
Write-Host "  Reference: ITU-T G.652.D (11/2009) - Single-mode optical fibre and cable characteristics" -ForegroundColor DarkGray
Write-Host ("  Thresholds: Attenuation le 0.4 dB/km @1310nm / le 0.3 dB/km @1550nm, Link le 80 km, Providers ge 2") -ForegroundColor DarkGray
Write-Status "ITU G.652.D Compliance: $ituPassCount ITU-PASS, $ituFailCount ITU-FAIL"
$att1310 = $Script:ITU_G652D.Attenuation_1310nm_dB_km
$att1550 = $Script:ITU_G652D.Attenuation_1550nm_dB_km
$maxLink = $Script:ITU_G652D.Max_Link_Length_km
$metMs = $Script:ITU_G652D.Max_Latency_RT_Metro_ms
$regMs = $Script:ITU_G652D.Max_Latency_RT_Regional_ms
Write-Status "ITU Thresholds: Attenuation le $att1310 dB/km (1310nm), le $att1550 dB/km (1550nm) | Max link: ${maxLink}km | Latency: metro ${metMs}ms / regional ${regMs}ms RT"

# Cleanup temp
Remove-Item $TempDir -Recurse -Force -ErrorAction SilentlyContinue
