# Engineering Compliance Skill — CAD-as-Code with Country-Specific Standards

## USE FOR
- Generating TIA-942-C Rated-3 compliant engineering designs (CAD, BIM, studies)
- Applying Brazilian ABNT NBR standards to foundation/structural/electrical designs
- Applying Eurocode (EN) standards for EU sites (Portugal, Spain, Italy, Germany, Estonia)
- Applying US codes (IBC, ASCE 7, NEC, ACI 318) for US sites
- Applying Paraguay INTEC + municipal codes for Paraguay sites
- Producing compliance-aware STEP/STL/IFC/CSV/JSON outputs with code references embedded
- Ensuring partner engineers have MINIMAL localization work

## DO NOT USE FOR
- Stamped/final engineering drawings (these require licensed PE per jurisdiction)
- Proprietary CAD formats (.dwg/.rvt/.sldprt) — use OpenSource only per plan MH23
- Replacing formal TIA-942-C certification audit (2,600+ criteria by licensed CAB)

## COMPLIANCE PARAMETERS BY JURISDICTION

### Brazil (ABNT NBR) — Phase 1 PRIMARY
| Standard | Scope | Key Parameters |
|----------|-------|----------------|
| NBR 6118:2014 | Concrete structural design | fck min=20MPa (C20), cover=40mm aggressive; fck=30MPa (C30) typical for DC pads |
| NBR 6122:2019 | Foundation design | Safety factors: bearing FS≥2.0, sliding FS≥1.5; allowable bearing per soil type |
| NBR 6123:1988 | Wind loads | Basic wind speed V₀ by region (Brazil: 38-50 m/s); pressure coefficients per shape |
| NBR 15421:2006 | Seismic design | Brazil is low seismic (zone 0-1); design spectral acceleration per zone |
| NBR 5410:2008 | Low-voltage electrical | Conductor sizing, protection coordination, grounding ≤10Ω |
| NBR 14039:2005 | Medium-voltage installations | 1kV-36.2kV switchgear, transformer protection |
| NBR 9077:2001 | Fire exits | Maximum travel distance, door widths, fire zone separation |
| NBR 17240:2010 | Fire alarm systems | Detection zones, notification, monitoring |
| NBR 9050:2015 | Accessibility | Ramp gradients, door widths, parking |
| ANEEL Res. 1.000/2021 | Distributed generation | Behind-the-meter solar PV registration, compensation |

### United States (IBC/ASCE/NEC/ACI)
| Standard | Scope | Key Parameters |
|----------|-------|----------------|
| IBC 2024 | International Building Code | Occupancy category, height limits, fire ratings |
| ASCE 7-22 | Minimum design loads | Wind: V ultimate by risk category; Seismic: SDS/SD1 per USGS; Snow: Pg by location |
| ACI 318-19 | Concrete building code | f'c min=2500psi(17MPa); cover per exposure; λ modification factors |
| NFPA 70 (NEC) | National Electrical Code | Article 690 (solar), Article 706 (BESS), Article 700 (emergency) |
| NFPA 70E | Electrical safety workplace | Arc flash boundary, PPE category, energized work permits |
| NFPA 76 | Telecom fire protection | Detection, suppression, notification for telecom facilities |
| NFPA 110 | Emergency/standby power | Generator Type 10 (10s start), Level 1 (life safety), 48h fuel |
| NFPA 855 | BESS installation | Fire separation, thermal runaway, UL 9540A required |

### EU (Eurocodes EN) — Phase 2
| Standard | Scope | Key Parameters |
|----------|-------|----------------|
| EN 1990:2002 | Basis of design | Reliability classes, partial safety factors |
| EN 1991-1-4:2005+A1:2010 | Wind actions | Basic wind velocity vb per national annex; terrain categories |
| EN 1992-1-1:2004 | Concrete design | C25/30 min for DC pads; cover per exposure class (XC4=40mm) |
| EN 1997-1:2004 | Geotechnical design | Bearing resistance, settlement, foundation categories |
| EN 1998-1:2004+A1:2013 | Seismic design | Low seismic for most EU sites (a_g ≤0.1g); design spectrum |

### TIA-942-C Rated-3 (Certification Target)
| Requirement | Parameter | Source |
|-------------|-----------|--------|
| Redundancy | N+1 (all systems: electrical, mechanical, telecom) | TIA-942-C §4.3 |
| Concurrent maintainability | Every component serviceable without disruption | TIA-942-C §4.3 |
| Cabinet width (MDA/IDA/HDA) | ≥800mm | TIA-942-C Annex F (NEW in 942-C) |
| Floor loading (≥20m² rooms) | 10 kPa (200 lbf/ft²) | TIA-942-C Annex B |
| Floor loading (<20m² rooms) | 5 kPa (100 lbf/ft²) | TIA-942-C Annex B (reduced) |
| Ceiling height | 3.6m minimum | TIA-942-C Annex B |
| Column grid | ≥7.2m × 7.2m | TIA-942-C Annex C |
| Door width | ≥1.2m equipment access | TIA-942-C |
| Fire suppression | Clean agent (Novec 1230/FM-200) preferred; pre-action acceptable; wet NOT for IT | TIA-942-C §6 |
| Perimeter fence | ≥2.4m height | TIA-942-C §7 |
| Fuel autonomy | 48h minimum at full load | NFPA 110 Level 1 |
| BESS fire separation | ≥5m from IT loads | NFPA 855 |
| Cable separation (A/B) | Physically separated pathways, independent fire zones | TIA-942-C §5 |
| Temperature | 18-27°C (ASHRAE A1) | ASHRAE TC 9.9 5th ed |
| Humidity | 20-80% RH (dew point ≤17°C for A1) | ASHRAE TC 9.9 5th ed |
| PUE monitoring | Per ISO/IEC 30134-2:2026 | TIA-942-C reference |

### Paraguay
| Standard | Scope | Notes |
|----------|-------|-------|
| INTEC (Instituto Nacional de Tecnología) | General building | Standards largely harmonized with MERCOSUR |
| Municipal codes (CDE/Asunción) | Local permits | Per-municipality building permits |
| ANDE (grid interconnection) | Grid connection | Resolution 48/2018 for distributed generation |

## LOAD COMBINATIONS BY JURISDICTION

### Brazil (NBR 8681:2003 Actions and Safety)
```
ULS: 1.4×Dead + 1.4×Live + 0.6×Wind (or 0.6×Seismic)
ULS: 1.0×Dead + 1.4×Wind (overturning check)
SLS: 1.0×Dead + 1.0×Live (settlement ≤25mm)
```

### USA (ASCE 7-22 LRFD)
```
ULS: 1.2×D + 1.6×L + 0.5×(Lr or S or R)
ULS: 1.2×D + 1.0×W + L + 0.5×(Lr or S or R)
ULS: 1.2×D + 1.0×E + L + 0.2×S
```

### EU (EN 1990:2002 ULS)
```
ULS: 1.35×Gk + 1.50×Qk,1 + ψ0×Qk,2
ULS: 1.0×Gk + 1.50×Qk,1 (unfavorable)
SLS: Gk + Qk,1 + ψ2×Qk,2
```

## MATERIAL PROPERTIES BY JURISDICTION

| Material | Brazil | USA | EU |
|----------|--------|-----|-----|
| Concrete (DC pad) | C30 (fck=30MPa, NBR 6118) | 4000psi (f'c=28MPa, ACI 318) | C30/37 (EN 1992) |
| Rebar | CA-50 (fy=500MPa, NBR 7480) | Grade 60 (fy=420MPa, ASTM A615) | B500B (fy=500MPa, EN 10080) |
| Steel structural | MR250 (fy=250MPa, NBR 7007) | A36 (fy=250MPa) | S235 (fy=235MPa, EN 10025) |
| Concrete cover (aggressive) | 40mm (NBR 6118) | 50mm (ACI 318) | 40mm (EN 1992, XC4) |

## SKILL USAGE IN ENGINEERING MODULES

When generating CAD/structural/electrical outputs, each module should:

1. **Load jurisdiction** from `site_yaml['country']` or `site_yaml['jurisdiction']`
2. **Apply code-specific parameters** (concrete grade, rebar grade, cover, load factors)
3. **Embed code references** in output metadata (STEP labels, IFC Psets, JSON fields)
4. **Calculate D/C ratios** using jurisdiction-appropriate safety factors
5. **Flag compliance gaps** in output reports

### Example: Container Foundation for Campinas (Brazil)
```python
# Load Brazilian code parameters
JURISDICTION = "Brazil"
CODES = {
    "structural": "NBR 6118:2014",
    "foundation": "NBR 6122:2019",
    "wind": "NBR 6123:1988",
    "concrete_grade": "C30 (fck=30MPa)",
    "rebar_grade": "CA-50 (fy=500MPa)",
    "concrete_cover_mm": 40,
    "safety_factor_bearing": 2.0,
    "safety_factor_sliding": 1.5,
    "load_combo_uls": "1.4D + 1.4L + 0.6W",
}
```

## COMPLIANCE CHECKLIST FOR ENGINEERING OUTPUTS

Each output file must include:

### STEP files (container_pad.step, site_layout.step, solar_field.step)
- [ ] Labels include jurisdiction code reference (e.g., "container_pad_NBR6118")
- [ ] Dimensions use jurisdiction-specific minimums (cover, pad thickness)
- [ ] TIA-942-C Rated-3 elements present (N+1, dual feeds, fire separation)

### IFC files (facility.ifc)
- [ ] Pset_TIA942_Rated3 with certification target
- [ ] Pset_SlabCommon with concrete grade per jurisdiction
- [ ] Pset_Zoning (White/Grey/Brown)
- [ ] ASHRAE TC 9.9 environmental parameters

### JSON files (structural_analysis.json)
- [ ] Load combinations per jurisdiction (NBR 8681 for Brazil)
- [ ] Safety factors per jurisdiction
- [ ] Code references in conclusion

### CSV files (solar_production.csv, bom_concrete_rebar.csv)
- [ ] Source YAML keys documented in BOM rows
- [ ] Material specifications per jurisdiction

### KiCad files (.net, .kicad_sch)
- [ ] Compliance codes in netlist header comments
- [ ] N+1 redundancy visible in netlist structure

## CROSS-REFERENCES

- Plan MH23 (line 191): CAD-as-code mandate
- `docs/engineering/civil-scope-base-case.md` (T11): Civil SOW
- `docs/engineering/electrical-scope-base-case.md` (T12): Electrical SOW
- `data/engineering/equipment_weights.yaml`: Structural tiers + equipment specs
- `data/engineering/solar_bess_tiers.yaml`: GHI tiers + BESS specs
- `tools/engineering/params.py`: Shared parameter loaders
- TIA-942-C (2024 edition): ANSI standard for datacenter infrastructure
- ABNT NBR standards: Brazilian Association of Technical Standards
- Eurocodes (EN): European Committee for Standardization
- IBC/ASCE/ACI: International Code Council / ASCE / American Concrete Institute
