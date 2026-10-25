# austringer-media step topology-dump, template v1 (schema
# austringer-step-topology/1). Runs INSIDE freecadcmd over the STEP file
# named by argv[-1]; prints the topology census (per-solid faces/edges/
# vertices, volume, BoundBox) between unique markers so freecadcmd's own
# startup chatter cannot corrupt it. Topology facts only: spec names are
# the extract skeleton's lane, not this census.
import json
import sys

import Part

DUMP_SCHEMA = "austringer-step-topology/1"


def solid_entry(solid):
    box = solid.BoundBox
    return {
        "bbox_max": [box.XMax, box.YMax, box.ZMax],
        "bbox_min": [box.XMin, box.YMin, box.ZMin],
        "edges": len(solid.Edges),
        "faces": len(solid.Faces),
        "vertices": len(solid.Vertexes),
        "volume": solid.Volume,
    }


def census():
    shape = Part.Shape()
    shape.read(sys.argv[-1])
    solids = shape.Solids
    return {
        "schema": DUMP_SCHEMA,
        "solid_count": len(solids),
        "solids": [solid_entry(solid) for solid in solids],
    }


print("AUSTRINGER_STEP_DUMP_BEGIN")
print(json.dumps(census(), sort_keys=True))
print("AUSTRINGER_STEP_DUMP_END")
