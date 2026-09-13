# Structural Engineering Skill — FEA + Topology Optimization + Code Compliance

## USE FOR
- Calculating beam/column/foundation loads using PyNite FEA (replaces simplified hand-calcs)
- Topology optimization for structural material distribution (TopOpt concepts)
- Code-compliant structural design per NBR 6118 (Brazil), ACI 318 (USA), Eurocode EN 1992 (EU)
- Wind load analysis per NBR 6123 / ASCE 7-22 / EN 1991-1-4
- Seismic design per NBR 15421 / ASCE 7-22 / EN 1998-1
- Producing stamped-ready structural analysis reports (JSON + CSV)

## TOOLS INSTALLED
- **PyNite** (`pip install PyNite`) — Python FEA for frames/beams/trusses
- **OpenSeesPy** — Advanced nonlinear FEA (already installed)
- **AnaStruct** — 2D finite element for simple beams (already installed)
- **build123d** — Export structural geometry to STEP/STL (already installed)

## WORKFLOW (4-phase per shared engineering stack)
1. **Define loads**: Dead (container weight 22t), Live (server racks), Wind (NBR 6123), Seismic (NBR 15421)
2. **Build FEA model**: PyNite for frame/beams, OpenSeesPy for foundation
3. **Analyze**: Load combinations per NBR 8681 (1.4D + 1.4L + 0.6W)
4. **Export**: Results to JSON/CSV + updated STEP geometry with optimized dimensions

## CODE COMPLIANCE PARAMETERS

### Brazil (NBR 6118:2014)
```python
CONCRETE = {
    'fc_min_mpa': 20,       # C20 minimum
    'fc_typical_mpa': 30,   # C30 for DC pads
    'cover_class_III_mm': 40,  # Aggressive (coastal/industrial)
    'delta_c_mm': 10,       # Tolerance
}
REBAR = {'grade': 'CA-50', 'fy_mpa': 500, 'standard': 'NBR 7480'}
LOAD_COMBOS = {'uls': '1.4D + 1.4L + 0.6W', 'uls_seismic': '1.4D + 1.4L + 1.0E'}
```

### USA (ACI 318-19)
```python
CONCRETE = {'fc_min_psi': 2500, 'fc_typical_psi': 4000, 'cover_aggressive_in': 2.0}
REBAR = {'grade': 'Grade 60', 'fy_ksi': 60, 'standard': 'ASTM A615'}
LOAD_COMBOS = {'uls': '1.2D + 1.6L + 0.5W', 'uls_seismic': '1.2D + 1.0E + L + 0.2S'}
```

### EU (EN 1992-1-1:2004)
```python
CONCRETE = {'fck_min_mpa': 20, 'fck_typical_mpa': 30, 'cover_XC4_mm': 30}
REBAR = {'grade': 'B500B', 'fyk_mpa': 500, 'standard': 'EN 10080'}
LOAD_COMBOS = {'uls': '1.35Gk + 1.50Qk', 'uls_seismic': 'Gk + 1.0E + ψ2Qk'}
```

## PyNite USAGE PATTERN
```python
from PyNite import FEModel3D
model = FEModel3D()
model.add_node('N1', 0, 0, 0)
model.add_node('N2', 6, 0, 0)  # 6m span
model.add_member('M1', 'N1', 'N2', E=30e3, G=12.5e3, Iy=8.33e-3, Iz=8.33e-3, J=1.67e-2, A=0.3)
model.add_support('N1', support_type='fixed')
model.add_support('N2', support_type='fixed')
model.add_member_load('M1', direction='Fy', p1=-215.8, p2=-215.8, x1=0, x2=6)  # 215.8 kN dead
model.analyze()
# Get results: model.members['M1'].shear(), .moment(), .deflection()
```

## CROSS-REFERENCES
- `data/engineering/equipment_weights.yaml` — structural tiers + pad dimensions
- `data/engineering/jurisdictions/brazil.yaml` — full NBR parameters
- `tools/engineering/structural_analysis.py` — current module (upgrade with PyNite)
- `tools/engineering/container_foundation.py` — STEP/STL pad geometry
- Compliance skill: `engineering-compliance/SKILL.md`
