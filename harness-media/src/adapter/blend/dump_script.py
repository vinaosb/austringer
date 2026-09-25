# harness-media blend scene-dump, template v1 (schema harness-blend-dump/1)
# Runs INSIDE headless blender (--background --factory-startup) over the
# .blend opened from the command line; prints the scene census between
# unique markers so blender's own startup chatter cannot corrupt it.
import bpy
import json

MAX_DEPTH = 8


def collection_census(scene):
    names = []
    depth = [0]

    def walk(parent, level):
        for child in parent.children:
            names.append(child.name)
            if level + 1 > depth[0]:
                depth[0] = level + 1
            if level + 1 < MAX_DEPTH:
                walk(child, level + 1)

    walk(scene.collection, 0)
    return {"depth": depth[0], "names": names}


def material_entry(material):
    entry = {"name": material.name, "base_color": [0.8, 0.8, 0.8, 1.0], "metallic": 0.0, "roughness": 0.5}
    if material.use_nodes:
        bsdf = material.node_tree.nodes.get("Principled BSDF")
        if bsdf is not None:
            entry["base_color"] = list(bsdf.inputs["Base Color"].default_value)
            entry["metallic"] = bsdf.inputs["Metallic"].default_value
            entry["roughness"] = bsdf.inputs["Roughness"].default_value
    return entry


def object_entry(obj):
    mesh = None
    if obj.type == "MESH" and obj.data is not None:
        mesh = {
            "verts": len(obj.data.vertices),
            "edges": len(obj.data.edges),
            "faces": len(obj.data.polygons),
        }
    slots = [material_entry(slot.material) for slot in obj.material_slots if slot.material is not None]
    return {
        "name": obj.name,
        "type": obj.type,
        "parent": obj.parent.name if obj.parent is not None else None,
        "world_transform": [list(row) for row in obj.matrix_world],
        "mesh": mesh,
        "modifiers": [modifier.name for modifier in obj.modifiers],
        "material_slots": slots,
    }


def census():
    scene = bpy.context.scene
    world = None
    if scene.world is not None:
        world = {"name": scene.world.name, "background": list(scene.world.color)}
    return {
        "schema": "harness-blend-dump/1",
        "scenes": [scene.name for scene in bpy.data.scenes],
        "collections": collection_census(scene),
        "action_count": len(bpy.data.actions),
        "world": world,
        "objects": [object_entry(obj) for obj in scene.objects],
    }


print("HARNESS_BLEND_DUMP_BEGIN")
print(json.dumps(census(), sort_keys=True))
print("HARNESS_BLEND_DUMP_END")
