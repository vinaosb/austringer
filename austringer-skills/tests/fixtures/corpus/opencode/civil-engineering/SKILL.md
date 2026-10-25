# Civil Engineering Skill — Foundation + Site Grading + Utilities + Code Compliance

## USE FOR
- Foundation design: bearing capacity, settlement, sliding, overturning
- Site grading: cut/fill volumes, drainage, erosion control
- Underground utilities: trench routing, pipe sizing, separation distances
- Access roads: geometry, pavement, turning radius
- Security perimeter: fencing, gates, bollards
- Code compliance: NBR 6122 (Brazil foundation), ASCE 7 (US loads), EN 1997 (EU geotech)

## TOOLS INSTALLED
- **build123d** — Export civil geometry (pads, roads, trenches) to STEP/STL
- **NetworkX** — Trench network topology + routing optimization
- **PyNite** — Foundation beam analysis (structural verification)
- **AnaStruct** — Simple pad edge beam verification

## FOUNDATION DESIGN PARAMETERS (per NBR 6122:2019)
```python
FOUNDATION = {
    'soil_bearing_kpa': {
        'soft_clay': 80,
        'stiff_clay': 150,
        'dense_sand': 200,
        'rock': 500,
    },
    'fos_bearing': 2.0,          # Factor of safety (ultimate)
    'fos_sliding': 1.5,
    'settlement_max_mm': 25,     # Datacenter tolerance
    'pad_thickness_mm': {
        'low_tier': 200,         # Low seismic/wind
        'medium_tier': 300,      # Typical (Campinas/Brasilia)
        'base_case_tier': 300,   # Reference
        'high_tier': 400,        # High seismic/wind
        'extreme_tier': 500,     # Extreme conditions
    },
    'excavation_overcut_pct': 20,  # 20% over-excavation
}
```

## SITE GRADING
```python
GRADING = {
    'max_slope_pct': 5,           # Maximum site slope
    'min_drainage_slope_pct': 1,  # Minimum for water runoff
    'cut_fill_balance': True,     # Minimize import/export
    'erosion_control': 'silt_fence + vegetation',
    'stormwater_retention_m3': 50, # Per hectare
}
```

## TRENCH ROUTING (per TIA-942-C §5 separation requirements)
```python
TRENCH = {
    'power_width_mm': 600,
    'fiber_width_mm': 300,
    'grounding_width_mm': 200,
    'fire_detection_width_mm': 150,
    'water_width_mm': 400,
    'drainage_width_mm': 500,
    'depth_power_mm': 800,
    'depth_fiber_mm': 600,
    'min_separation_ab_feeds_m': 0.3,  # 300mm horizontal between A/B cable trays
}
```

## CROSS-REFERENCES
- `data/engineering/equipment_weights.yaml` — structural tiers + declivity tiers
- `data/engineering/jurisdictions/brazil.yaml` — NBR 6122 + NBR 6123 parameters
- `tools/engineering/container_foundation.py` — STEP/STL pad generator
- `tools/engineering/site_layout.py` — complete facility assembly
- `tools/engineering/trench_network.py` — NetworkX trench topology
- `tools/engineering/bom_concrete_rebar.py` — material takeoff CSV
- Compliance skill: `engineering-compliance/SKILL.md`
