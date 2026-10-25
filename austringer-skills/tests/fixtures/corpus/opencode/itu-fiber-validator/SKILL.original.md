# ITU Fiber Validator

Validates candidate site coordinates against the ITU BBMaps fiber backbone proximity API.

## USE FOR
- Check fiber backbone proximity for data center candidate sites
- Batch validate coordinates from CSV files
- Enrich location CSVs with fiber proximity data
- Filter out sites >80km from any fiber backbone

## DO NOT USE FOR
- Precise fiber route mapping (use TeleGeography/submarinecablemap.com)
- Provider identification (ITU only returns proximity, not provider names)
- Real-time fiber availability (data is periodic, not live)

## Requirements

- **PowerShell 5.1+** (Windows)
- **curl.exe** — must be available in PATH. Install via `winget install curl.curl` or enable in `C:\Windows\System32\`.
- **ITU_API_KEY** environment variable (optional) — set `$env:ITU_API_KEY` for production use. Falls back to a public key if not set.

## Files

### Script: `itu-fiber-validator.ps1`

**Location**: `C:\Users\vsbb1\.config\opencode\skills\itu-fiber-validator\itu-fiber-validator.ps1` (skill directory)  
**Mirror**: `scripts/itu-fiber-validator.ps1` in the repo (kept in sync)

Both copies are identical. The script in the repo points to the skill directory via a cross-reference comment.

**Input**: CSV with columns `City, Lat, Lon` (plus any optional columns)

**Output**: Enriched CSV with added columns:
- `Fiber_Proximity` — raw ITU response (e.g., "within 5km", ">100km")
- `Fiber_Km_Est` — numeric km estimate (5, 10, 15, 20, 50, 75, 999)
- `Fiber_Pass` — PASS (≤80km), FAIL (>100km), CHECK (needs review), RETRY (query failed)

**Usage** (from repo root):
```powershell
# Basic — uses defaults (10 cities/batch, 5s delay)
.\scripts\itu-fiber-validator.ps1 -InputFile .\data\all-candidates.csv

# Custom output, larger batches, longer delays for rate limiting
.\scripts\itu-fiber-validator.ps1 -InputFile .\data\all-candidates.csv -OutputFile .\data\verified.csv -BatchSize 15 -DelayBetweenBatches 10

# Conservative — small batches, long delays (when API is rate-limiting)
.\scripts\itu-fiber-validator.ps1 -InputFile .\data\all-candidates.csv -BatchSize 5 -DelayBetweenBatches 15 -MaxRetries 5
```

**Usage** (from skill directory):
```powershell
# Run directly from skill location
.\itu-fiber-validator.ps1 -InputFile .\data\all-candidates.csv
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
| `data/all-candidates.csv` | Master candidate list with ALL locations, enriched data | ~95 |
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

1. **Add candidates** to `data/all-candidates.csv` with City, Country, Lat, Lon, Altitude_m, GHI estimates, Tier flags
2. **Run validator**: `.\scripts\itu-fiber-validator.ps1 -InputFile .\data\all-candidates.csv`
3. **Review output**: Check Fiber_Pass column. FAIL = eliminate. RETRY = re-query.
4. **Update master**: Copy enriched output back to `data/all-candidates.csv`
5. **Update prompt**: Add qualifying candidates (PASS + meets tier minimums) to Section 8 of GLOBAL-DC-TOPOLOGY-PROMPT.md

## Status (as of 2026-05-19)

- **80 cities queried** via ITU API across 3 batches
- **67 PASS** (fiber within 80km), **6 FAIL** (>100km), **7 need re-query** (API timeout)
- **26 original Golden Zone entries** in prompt
- **~40 new candidates** added to master CSV (Turkey, Lesotho, Colombia, Ecuador, Tajikistan, Kyrgyzstan, Afghanistan, Madagascar, Algeria, Myanmar, Brazil NE/SE/S)
- **Remaining**: Andean cities (Mendoza, San Juan, La Rioja, Catamarca, Sucre, Potosí, Ica, Nazca), some African cities (Adama, Dire Dawa, Nyeri, Kericho, Gaborone, Mbombela, Fianarantsoa), and a few Asian cities (Chiang Rai, Bontoc, Davao highlands)
