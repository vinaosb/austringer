# harness-media step builder, template v1 (schema harness-step-build/1)
# Runs inside freecadcmd (FreeCAD 1.0.x, OCCT-backed). Constructs the
# spec's primitives via Part.makeBox / makeCylinder / makeSphere with a
# Placement (T * Rz * Ry * Rx, XYZ euler radians), exports ONE compound
# as AP214-flavored STEP, then pins the FILE_NAME wall-clock stamp to
# the spec epoch for byte-determinism (M1 contract; the topology diff
# strips it regardless).
import math
import re
import sys

import FreeCAD as App
import Part

SPEC = __SPEC_JSON__


def primitive(entry):
    shape = entry["shape"]
    if "box" in shape:
        size = shape["box"]["size"]
        return Part.makeBox(size[0], size[1], size[2])
    if "cylinder" in shape:
        cylinder = shape["cylinder"]
        return Part.makeCylinder(cylinder["radius"], cylinder["height"])
    sphere = shape["sphere"]
    return Part.makeSphere(sphere["radius"])


def euler_rotation(rotation):
    # combined = Rz * Ry * Rx (column vectors, XYZ euler radians):
    # multiply composes "this rotation first, then the argument".
    result = App.Rotation(App.Vector(1.0, 0.0, 0.0), math.degrees(rotation[0]))
    result = result.multiply(App.Rotation(App.Vector(0.0, 1.0, 0.0), math.degrees(rotation[1])))
    result = result.multiply(App.Rotation(App.Vector(0.0, 0.0, 1.0), math.degrees(rotation[2])))
    return result


def build():
    solids = []
    for entry in SPEC["solids"]:
        shape = primitive(entry)
        transform = entry["transform"]
        shape.Placement = App.Placement(
            App.Vector(transform["location"]), euler_rotation(transform["rotation_xyz"]),
        )
        solids.append(shape)
    out = sys.argv[-1]
    if len(solids) == 1:
        solids[0].exportStep(out)
    else:
        Part.makeCompound(solids).exportStep(out)
    with open(out, "r", encoding="utf-8") as handle:
        text = handle.read()
    text = re.sub(
        r"(FILE_NAME\('[^']*',')[^']*(')",
        r"\g<1>" + SPEC["epoch"] + r"\g<2>",
        text,
        count=1,
    )
    with open(out, "w", encoding="utf-8") as handle:
        handle.write(text)


build()
