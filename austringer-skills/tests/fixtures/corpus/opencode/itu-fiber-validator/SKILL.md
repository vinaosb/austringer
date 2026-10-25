# ITU Fiber Validator

Validate site coordinates against ITU BBMaps fiber backbone proximity API.

## USE FOR
- Fiber backbone proximity check for DC candidate sites
- Batch validate coordinates from CSV
- Enrich location CSVs with fiber proximity data
- Filter sites >80km from backbone

## DO NOT USE FOR
- Precise fiber route mapping (use TeleGeography)
- Provider identification (ITU = proximity only, no names)
- Real-time availability (data periodic, not live)

## Requirements

- **PowerShell 5.1+** (Windows)
- **curl.exe** in PATH (`winget install curl.curl`)
- **ITU_API_KEY** env var (optional) — fallback to public key if unset

## Files

### Script: `itu-fiber-validator.ps1`

**Location**: skill dir + `scripts/` mirror (identical copies).

**Input**: CSV with `City, Lat, Lon` (+ optional columns)

**Output**: Enriched CSV:
- `Fiber_Proximity` — raw ITU response ("within 5km", ">100km")
- `Fiber_Km_Est` — numeric km (5, 10, 15, 20, 50, 75, 999)
- `Fiber_Pass` — PASS (≤80km), FAIL (>100km), CHECK, RETRY

**Usage**:
```powershell
# Basic (10 cities/batch, 5s delay)
.\scripts\itu-fiber-validator.ps1 -InputFile .\data\all-candidates.csv

# Custom output, larger batches, rate-limit delays
.\scripts\itu-fiber-validator.ps1 -InputFile .\data\all-candidates.csv -OutputFile .\data\verified.csv -BatchSize 15 -DelayBetweenBatches 10

# Conservative (API rate-limiting)
.\scripts\itu-fiber-validator.ps1 -InputFile .\data\all-candidates.csv -BatchSize 5 -DelayBetweenBatches 15 -MaxRetries 5
```

**Parameters**:
| Parameter | Default | Description |
|-----------|---------|-------------|
| InputFile | (required) | Path to input CSV |
| OutputFile | auto-generated | Path to output CSV |
| BatchSize | 10 | Coordinates per API call (max 50) |
| DelayBetweenBatches | 5 | Seconds between calls |
| MaxRetries | 3 | Retry attempts per failed batch |
| RetryDelay | 30 | Seconds before retry |

### Data Files

| File | Description | Rows |
|------|-------------|------|
| `data/all-candidates.csv` | Master candidate list with ALL locations, enriched | ~95 |
| `data/fiber-results.csv` | All ITU fiber query results (historical) | ~80 |

### API Details

- **Endpoint**: `https://bbmaps.itu.int/functionproximity//route-calcdistancetonode`
- **API Key** (public): `M2CgTqEMCUnk-webappkey2proximity`
- **Method**: POST with `Content-Type: text/csv`
- **Body**: CSV with header `X,Y` (X=longitude, Y=latitude), one coordinate per line
- **Headers**: `Origin: https://bbmaps.itu.int`, `Referer: https://bbmaps.itu.int/web10km`
- **Response**: URL to ZIP file containing CSV with `X,Y,proximity` columns
- **Proximity values**: `within 5km`, `within 10km`, `within 15km`, `within 20km`, `within 50km`, `within 75km`, `>100km`
- **Rate limit**: API may timeout (504) after multiple rapid requests; use 5-15s delays between batches

### Master CSV Columns (`data/all-candidates.csv`)

| Column | Description |
|--------|-------------|
| City | City/municipality name |
| Country | Country name |
| Region | Golden Zone region from topology prompt |
| Lat | Latitude (decimal degrees, WGS84) |
| Lon | Longitude (decimal degrees, WGS84) |
| Altitude_m | Elevation above sea level (meters) |
| GHI_annual_kWh_m2_day | Annual average Global Horizontal Irradiance |
| GHI_worst_month | Worst month GHI estimate |
| AvgTemp_C | Annual average temperature (°C) |
| CorpTax_pct | Corporate tax rate (%) |
| SEZ_Tax | Special economic zone tax rate/notes |
| Fiber_Proximity | ITU API result (e.g., "within 5km") |
| Fiber_Km_Est | Numeric km estimate |
| Fiber_Pass | PASS/FAIL/CHECK/RETRY |
| Tier_DeepCore | TRUE if meets Deep Core minimums |
| Tier_Regional | TRUE if meets Regional minimums |
| Tier_MetroEdge | TRUE if meets Metro Edge minimums |
| Notes | Additional context |

### Tier Minimums Reference

| Threshold | Deep Core | Regional | Metro Edge |
|-----------|-----------|----------|------------|
| GHI annual | ≥ 6.0 | ≥ 5.0 | ≥ 4.5 |
| GHI worst month | ≥ 4.5 | ≥ 3.5 | ≥ 3.0 |
| Altitude | ≥ 1,000m | ≥ 500m | ≥ 300m |
| Tax | ≤ 25% | ≤ 30% | ≤ 35% |
| Fiber | ≤ 80km, ≥ 2 providers | ≤ 80km, ≥ 2 providers | ≤ 80km, ≥ 2 providers |

## Workflow

1. Add candidates to `data/all-candidates.csv` (City, Country, Lat, Lon, Altitude_m, GHI, Tier flags)
2. Run: `.\scripts\itu-fiber-validator.ps1 -InputFile .\data\all-candidates.csv`
3. Review Fiber_Pass: FAIL = eliminate, RETRY = re-query
4. Copy enriched output back to `data/all-candidates.csv`
5. Add qualifying candidates (PASS + tier minimums) to DC topology prompt

## Status (2026-05-19)

- **80 cities queried** (3 batches): 67 PASS, 6 FAIL, 7 need re-query
- **26 Golden Zone entries** in prompt, **~40 new candidates** added
- **Remaining**: Andean (Mendoza, Sucre, Ica), African (Adama, Gaborone), Asian (Chiang Rai, Davao)