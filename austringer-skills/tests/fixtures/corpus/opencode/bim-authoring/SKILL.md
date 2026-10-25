# BIM Authoring Skill — IFC + BlenderBIM + TIA-942-C Evidence Package

## USE FOR
- Creating IFC4 BIM models with full spatial hierarchy (Project → Site → Building → Storey → Elements)
- Adding TIA-942-C Rated-3 compliance Psets (redundancy, fire separation, environmental)
- Space zoning (White/Grey/Brown per TIA-942-C)
- Producing evidence package documentation for certification audit
- Routing electrical/plumbing systems through structural geometry (BlenderBIM)

## TOOLS INSTALLED
- **IfcOpenShell** — Python API for IFC4 authoring (already installed)
- **BlenderBIM** — Blender addon for BIM authoring (install separately)
- **build123d** — Export structural geometry for BIM import (already installed)

## WORKFLOW
1. **Generate geometry**: build123d → STEP files (pads, buildings, containers)
2. **Create IFC hierarchy**: IfcOpenShell → IfcProject → IfcSite → IfcBuilding → IfcBuildingStorey
3. **Add elements**: IfcSlab (pads), IfcWall (fire separation), IfcBuildingElementProxy (containers/BESS/gen)
4. **Attach Psets**: TIA-942-C compliance, structural codes, BESS safety, generator specs
5. **Zone spaces**: White (IT), Grey (support), Brown (non-critical)
6. **Export**: facility.ifc (openable in BlenderBIM, BIMcollab, Solibri, Revit)

## TIA-942-C SPACE ZONING
```python
ZONES = {
    'white': {'description': 'IT equipment area (computer room)', 'security': 'high', 'fire_zone': 'FZ01'},
    'grey': {'description': 'Support infrastructure (electrical/mechanical rooms)', 'security': 'high', 'fire_zone': 'FZ02'},
    'brown': {'description': 'Loading, storage, non-critical areas', 'security': 'medium', 'fire_zone': 'FZ03'},
}
```

## IFC Pset PATTERNS
```python
# TIA-942-C compliance
Pset_TIA942_Rated3 = {
    'Certification_Target': 'TIA-942-C Rated 3',
    'Redundancy': 'N+1',
    'Cabinet_Width_mm': 800,  # NEW in 942-C for MDA/IDA/HDA
    'Floor_Loading_kPa': 10,
    'Ceiling_Height_m': 3.6,
    'Fire_Suppression': 'Clean Agent (Novec 1230)',
    'Fuel_Autonomy_h': 48,
}

# Structural compliance
Pset_SlabCommon = {
    'Concrete_Grade': 'C30 (NBR 6118 fck=30MPa)',
    'Cover_mm': 40,  # Class III aggressive
    'Codes': 'NBR 6118; NBR 6122; ACI 318',
}

# BESS safety
Pset_BESS_NFPA855 = {
    'Chemistry': 'LFP',
    'Capacity_kWh': 10400,
    'Fire_Separation_m': 5.0,
    'UL_Listing': 'UL 9540A',
    'Codes': 'NFPA 855; NBR 17240',
}
```

## CROSS-REFERENCES
- `tools/engineering/bim_model.py` — current IFC4 generator (upgrade with full hierarchy)
- `tools/engineering/site_layout.py` — STEP assembly (import into BIM)
- Compliance skill: `engineering-compliance/SKILL.md`
- TIA-942-C research: `.sisyphus/evidence/final-qa/F3-manual-qa.md`
