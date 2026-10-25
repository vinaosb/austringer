# Electrical Engineering Skill — Power Flow + Solar + Schematics + Code Compliance

## USE FOR
- Power flow analysis using pandapower (replaces basic NetworkX topology)
- Solar irradiance modeling using pvlib (replaces simplified clear-sky sinusoidal)
- Professional single-line diagrams using QElectroTech + SKiDL
- Code-compliant electrical design per NBR 5410/14039 (Brazil), NEC NFPA 70 (USA), EN 50549 (EU)
- Microgrid simulation: PV + BESS + Generator + Grid dispatch optimization
- Cable sizing, protection coordination, arc flash analysis

## TOOLS INSTALLED
- **pandapower** — Power flow analysis (replaces ETAP)
- **pvlib** — Solar irradiance + PV system modeling (replaces PVSyst basic functions)
- **SKiDL** — Python circuit design → KiCad netlists
- **QElectroTech** — Professional SLD drawing (desktop app, installed via winget)
- **NetworkX** — Topology graph analysis (already installed)

## WORKFLOW (4-phase per shared engineering stack)
1. **Size the system**: pvlib for solar production, pandapower for load flow
2. **Simulate dispatch**: Solar → BESS → Grid → Generator optimization
3. **Design protection**: Circuit breaker coordination, arc flash boundaries
4. **Generate SLD**: pandapower net → QElectroTech schematic → KiCad netlist

## pandapower USAGE PATTERN
```python
import pandapower as pp
net = pp.create_empty_network()
# Create buses
bus_grid = pp.create_bus(net, vn_kv=13.8, name="Grid POI")
bus_lv = pp.create_bus(net, vn_kv=0.48, name="LV Bus 480V")
# Transformer
pp.create_transformer(net, bus_grid, bus_lv, std_type="0.63 MVA 20/0.4 kV")
# Loads
pp.create_load(net, bus_lv, p_mw=0.5, name="IT Load 500kW")
# Solar generator
pp.create_sgen(net, bus_lv, p_mw=4.0, name="Solar PV 4MW")
# BESS
pp.create_storage(net, bus_lv, p_mw=0.65, max_e_mwh=10.4, name="BESS 10.4MWh")
# Run power flow
pp.runpp(net)
# Results: net.res_bus.vm_pu, net.res_line.loading_percent
```

## pvlib USAGE PATTERN
```python
import pvlib
from pvlib.location import Location
site = Location(latitude=-22.9, longitude=-47.06, tz='America/Sao_Paulo', altitude=600)
# Generate clear-sky irradiance
times = pd.date_range(start='2026-01-01', end='2026-12-31', freq='1h', tz=site.tz)
cs = site.get_clearsky(times)  # ghi, dni, dhi
# PV system
system = pvlib.pvsystem.PVSystem(surface_tilt=lat, surface_azimuth=0,
                                  module_parameters={'pdc0': 4e6},  # 4MWp
                                  inverter_parameters={'pdc0': 3.38e6})
# Annual production
ac = system.get_ac(pvlib.pvsystem.dc_ohms_from_percent(cs['ghi']))
annual_kwh = ac.sum() / 1000
```

## CODE COMPLIANCE PARAMETERS

### Brazil (NBR 5410:2008)
```python
LV = {'voltage_single': [127, 220], 'voltage_three': [220, 380], 'vdrop_max_pct': 4.0}
MV = {'voltage_kv': [13.8, 34.5], 'interrupting_ka': 31.5}
GROUNDING = 'TN-S'
```

### USA (NEC NFPA 70)
```python
LV = {'voltage_single': [120, 240], 'voltage_three': [208, 480]}
CONTINUOUS_LOAD_FACTOR = 1.25  # NEC 125% rule
VOLTAGE_DROP_FEEDER = 3.0  # %
VOLTAGE_DROP_TOTAL = 5.0  # %
```

## CRITICAL DESIGN PARAMETERS
```python
# Solar+BESS Datacenter Electrical Stack
IT_LOAD_KW = 500         # Container module (500 kW IT)
PUE = 1.30               # Total facility 650 kW
SOLAR_MWP = 4.40         # Solar capacity (medium_high_ghi tier)
BESS_KWH = 10400         # 16h autonomy
BESS_CHEMISTRY = 'LFP'   # Lithium Iron Phosphate
BESS_DOD = 0.90          # Depth of discharge
BESS_RTE = 0.88          # Round-trip efficiency
GENERATOR_KW = 700       # N+1 standby
FUEL_H = 48              # 48h autonomy at full load
GRID_VOLTAGE_KV = 13.8   # Typical interconnection
INVERTER_LOADING_RATIO = 1.30
REDUNDANCY = 'N+1'       # TIA-942-C Rated 3
```

## CROSS-REFERENCES
- `data/engineering/solar_bess_tiers.yaml` — GHI tiers + BESS specs
- `tools/engineering/microgrid_topology.py` — NetworkX topology (upgrade with pandapower)
- `tools/engineering/solar_energy_model.py` — stdlib CSV (upgrade with pvlib)
- `tools/engineering/bess_schematic.py` — SKiDL netlist
- `tools/engineering/microgrid_single_line.py` — full SLD
- Compliance skill: `engineering-compliance/SKILL.md`
