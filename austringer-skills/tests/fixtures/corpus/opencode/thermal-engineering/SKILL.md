# Thermal Engineering Skill — CFD + HVAC + ASHRAE TC 9.9

## USE FOR
- Datacenter cooling load calculation (PUE-based)
- Hot aisle/cold aisle containment verification
- CRAC/CRAH unit sizing and N+1 redundancy verification
- ASHRAE TC 9.9 thermal compliance (Classes A1-A3, H1)
- CFD airflow simulation concepts (OpenFOAM integration path)
- Energy modeling via OpenStudio/EnergyPlus concepts

## CRITICAL: Heat rejection is the #1 datacenter design problem
Servers convert electricity to heat. If cooling fails, the datacenter goes offline in seconds.
This skill ensures cooling design is properly sized and compliant.

## TOOLS (CONCEPTUAL — SOME REQUIRE STANDALONE INSTALLS)
- **pvlib** (installed) — can calculate solar heat gain through walls/roof
- **OpenStudio/EnergyPlus** (standalone) — DOE building energy simulation
- **OpenFOAM** (standalone) — CFD for airflow modeling in server rooms

## ASHRAE TC 9.9 (5th Edition) THERMAL CLASSES
| Class | Temp (°C) | Humidity (% RH) | Dew Point (°C) | Application |
|-------|-----------|-----------------|-----------------|-------------|
| A1 | 18-27 | 20-80 | ≤17 | Enterprise datacenter |
| A2 | 18-27 | 20-80 | ≤21 | Office/server room |
| A3 | 5-40 | 8-85 | ≤24 | High-temp tolerant |
| H1 | Per ASHRAE | Per ASHRAE | — | High-density AI (40kW+/rack) |

## COOLING DESIGN PARAMETERS
```python
COOLING = {
    'it_load_kw': 500,           # Container module IT load
    'pue_target': 1.30,          # PUE design target
    'total_facility_kw': 650,    # IT + cooling + overhead
    'cooling_load_kw': 150,      # = (PUE-1) × IT_load = 0.30 × 500
    'crac_units': 'N+1',         # TIA-942-C Rated 3 requirement
    'crac_capacity_each_kw': 75, # 2× 75kW units (N=2, +1=3 units for 150kW)
    'airflow_cfm': 12000,        # Per CRAC unit
    'hot_cold_aisle': True,      # Containment required for Rated-3
    'cooling_method': 'RDHx',    # Rear Door Heat Exchanger (liquid)
    ' chilled_water_temp_c': 7,  # CHW supply temperature
    'condenser_temp_c': 35,     # Ambient design temp
}
```

## PUE CALCULATION (3-SEGMENT PER solar_bess_tiers.yaml)
```python
PUE_SEGMENTS = {
    'it_load': 1.00,         # Baseline IT power
    'cooling': 0.20,         # CRAC + chillers + pumps
    'power_distribution': 0.10,  # UPS + transformer losses
}
# Design PUE = 1.00 + 0.20 + 0.10 = 1.30
# Stretch PUE = 1.25 (optimized cooling)
# Worst case = 1.40 (degraded cooling)
```

## CROSS-REFERENCES
- `data/engineering/solar_bess_tiers.yaml` — 3-segment PUE model + altitude cooling strategy
- `tools/engineering/site_layout.py` — container module positioning (hot/cold aisle)
- ASHRAE TC 9.9, 5th Edition — thermal guidelines for datacenters
