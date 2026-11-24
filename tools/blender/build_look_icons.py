"""Icons for the wardrobe (app/src/hub_ui.rs): every look in hub::LOOKS rendered on its own, in its
set's own colours, three-quarters on, on a transparent background, into one atlas.

    blender --background --factory-startup --python app/tools/blender/build_look_icons.py

Reads the models the game uses (assets/characters/look_<set>_ranger.glb, gear_bridgewright_ranger.glb
and assets/weapons/<name>.glb) and writes app/assets/ui/look_icons.png: 8 icons a row, 128 px
each, in the order of ICONS below (the same order as hub::LOOKS; the game finds an icon by the
look's position in that list).
"""

import math
import os

import bpy
from mathutils import Vector

HERE = os.path.dirname(os.path.abspath(__file__))
APP = os.path.normpath(os.path.join(HERE, "..", ".."))
CHARS = os.path.join(APP, "assets", "characters")
WEAPONS = os.path.join(APP, "assets", "weapons")
OUT = os.path.join(APP, "assets", "ui", "look_icons.png")
TMP = os.path.join(os.environ.get("TEMP", "/tmp"), "look_icons")
SIZE = 128
COLS = 8

# the dyes (linear, from crate::gear::DYES) each set wears by default: coat, trim, accent
RIG = ((0.20, 0.11, 0.05), (0.62, 0.42, 0.14), (0.03, 0.42, 0.40))
HERALD = ((0.05, 0.10, 0.35), (0.62, 0.42, 0.14), (0.50, 0.03, 0.06))
SMUGGLER = ((0.03, 0.14, 0.05), (0.62, 0.42, 0.14), (0.40, 0.07, 0.04))
PILGRIM = ((0.22, 0.23, 0.25), (0.55, 0.45, 0.30), (0.80, 0.30, 0.05))
WARDEN = ((0.18, 0.18, 0.20), (0.62, 0.42, 0.14), (0.55, 0.45, 0.30))
MAGISTRATE = ((0.03, 0.03, 0.035), (0.62, 0.42, 0.14), (0.55, 0.12, 0.03))
REGENT = ((0.025, 0.02, 0.03), (0.62, 0.02, 0.035), (0.45, 0.36, 0.14))

# (file, mesh name prefix, dyes), in hub::LOOKS order
ICONS = [
    ("gear_bridgewright_ranger.glb", "bw_head", RIG), ("gear_bridgewright_ranger.glb", "bw_body", RIG),
    ("gear_bridgewright_ranger.glb", "bw_legs", RIG), ("gear_bridgewright_ranger.glb", "bw_hands", RIG),
    ("gear_bridgewright_ranger.glb", "bw_feet", RIG), ("gear_bridgewright_ranger.glb", "bw_charm", RIG),
    ("w:rigger_hook.glb", "", RIG), ("w:rigger_snub.glb", "", RIG),
] + [(f"look_{s}_ranger.glb", f"{p}_{slot}", d) for s, p, d in (("herald", "hr", HERALD), ("smuggler", "sm", SMUGGLER), ("pilgrim", "ap", PILGRIM))
     for slot in ("head", "cape", "neck", "body", "legs", "hands", "feet")] + [
    ("w:katana.glb", "", RIG), ("w:greatsword.glb", "", RIG), ("w:dagger.glb", "", RIG), ("w:revolver.glb", "", RIG),
] + [("look_vault_ranger.glb", f"wv_{slot}", WARDEN) for slot in ("head", "cape", "neck", "body", "legs", "hands", "feet")] + [
    ("look_vault_ranger.glb", f"mg_{slot}", MAGISTRATE) for slot in ("head", "cape", "neck", "body")] + [
    ("w:katana.glb", "", RIG), ("w:greatsword.glb", "", RIG), ("w:dagger.glb", "", RIG), ("w:revolver.glb", "", RIG),
    ("look_vault_ranger.glb", "rg_body", REGENT), ("w:ashblood_glaive.glb", "", RIG),
]


def clear():
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)


def texture(objs, path):
    """The model's own painted texture (the game keeps it apart from the model)."""
    if not os.path.exists(path):
        return
    img = bpy.data.images.load(path, check_existing=True)
    for ob in objs:
        for slot in ob.material_slots:
            m = slot.material
            if m is None:
                continue
            m.use_nodes = True
            nt = m.node_tree
            bsdf = next((n for n in nt.nodes if n.type == "BSDF_PRINCIPLED"), None)
            if bsdf is None:
                continue
            tex = next((n for n in nt.nodes if n.type == "TEX_IMAGE"), None)
            if tex is None:
                tex = nt.nodes.new("ShaderNodeTexImage")
            tex.image = img
            nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
            bsdf.inputs["Roughness"].default_value = max(bsdf.inputs["Roughness"].default_value, 0.45)


def tint(objs, dyes):
    for ob in objs:
        for slot in ob.material_slots:
            m = slot.material
            if m is None or not m.use_nodes:
                continue
            base = m.name.split("__")[0]
            zone = 0 if base.startswith("dye_coat") else 1 if base.startswith("dye_trim") else 2 if base.startswith("dye_accent") else -1
            if zone < 0 or m.get("tinted"):
                continue
            m["tinted"] = True
            nt = m.node_tree
            bsdf = next((n for n in nt.nodes if n.type == "BSDF_PRINCIPLED"), None)
            tex = next((n for n in nt.nodes if n.type == "TEX_IMAGE"), None)
            if bsdf is None:
                continue
            c = dyes[zone]
            col = (min(c[0] / 0.6, 1.0), min(c[1] / 0.6, 1.0), min(c[2] / 0.6, 1.0), 1.0)
            if tex is None:
                bsdf.inputs["Base Color"].default_value = col
                continue
            mix = nt.nodes.new("ShaderNodeMixRGB")
            mix.blend_type = "MULTIPLY"
            mix.inputs["Fac"].default_value = 1.0
            mix.inputs["Color2"].default_value = col
            nt.links.new(tex.outputs["Color"], mix.inputs["Color1"])
            nt.links.new(mix.outputs["Color"], bsdf.inputs["Base Color"])


def setup_scene():
    scene = bpy.context.scene
    for name in ("BLENDER_EEVEE_NEXT", "BLENDER_EEVEE"):
        try:
            scene.render.engine = name
            break
        except TypeError:
            continue
    scene.render.resolution_x = SIZE * 2
    scene.render.resolution_y = SIZE * 2
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    try:
        scene.view_settings.view_transform = "Standard"
    except TypeError:
        pass
    world = bpy.data.worlds.get("icons") or bpy.data.worlds.new("icons")
    world.use_nodes = True
    world.node_tree.nodes.get("Background").inputs[0].default_value = (0.35, 0.35, 0.38, 1.0)
    scene.world = world


def light(name, loc, energy, color=(1, 1, 1)):
    d = bpy.data.lights.new(name, "AREA")
    d.energy = energy
    d.size = 2.0
    d.color = color
    ob = bpy.data.objects.new(name, d)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = loc
    direction = -Vector(loc)
    ob.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()
    return ob


def texture_for(file):
    if file.startswith("w:"):
        return os.path.join(WEAPONS, "textures", file[2:].replace(".glb", ".png"))
    if file.startswith("gear_"):
        return os.path.join(CHARS, "gear.png")
    return os.path.join(CHARS, file.split("_ranger")[0] + ".png")


def render_one(path, prefix, dyes, out, tex_path):
    clear()
    bpy.ops.import_scene.gltf(filepath=path)
    keep = [o for o in bpy.data.objects if o.type == "MESH" and (not prefix or o.name.startswith(prefix))]
    for o in list(bpy.data.objects):
        if o not in keep:
            bpy.data.objects.remove(o, do_unlink=True)
    if not keep:
        return False
    texture(keep, tex_path)
    tint(keep, dyes)
    # (a pair of gloves or boots: frame the left one)
    one_side = prefix.endswith(("_hands", "_feet"))
    lo = Vector((1e9, 1e9, 1e9))
    hi = Vector((-1e9, -1e9, -1e9))
    for o in keep:
        for v in o.data.vertices:
            w = o.matrix_world @ v.co
            if one_side and w.x < 0.0:
                continue
            lo = Vector((min(lo.x, w.x), min(lo.y, w.y), min(lo.z, w.z)))
            hi = Vector((max(hi.x, w.x), max(hi.y, w.y), max(hi.z, w.z)))
    c = (lo + hi) / 2
    r = max((hi - lo).length / 2, 0.02)
    # three-quarters on from the front (glTF: the fighter faces +z, Blender's -y)
    a = math.radians(-30)
    d = Vector((math.sin(a), -math.cos(a), 0.35)).normalized()
    cam_d = bpy.data.cameras.new("cam")
    cam_d.type = "ORTHO"
    cam_d.ortho_scale = r * 2.15
    cam = bpy.data.objects.new("cam", cam_d)
    bpy.context.scene.collection.objects.link(cam)
    cam.location = c + d * (r * 4 + 1)
    cam.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
    bpy.context.scene.camera = cam
    for nm, off, e, col in (("key", (1.5, -2.5, 2.5), 160, (1, 0.96, 0.9)), ("fill", (-2.5, -1.5, 0.5), 60, (0.8, 0.88, 1)), ("rim", (0, 3, 2), 120, (1, 0.9, 0.8))):
        light(nm, c + Vector(off), e, col)
    bpy.context.scene.render.filepath = out
    bpy.ops.render.render(write_still=True)
    return True


def main():
    os.makedirs(TMP, exist_ok=True)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    setup_scene()
    rows = (len(ICONS) + COLS - 1) // COLS
    atlas = [0.0] * (COLS * SIZE * rows * SIZE * 4)
    W = COLS * SIZE
    for k, (file, prefix, dyes) in enumerate(ICONS):
        path = os.path.join(WEAPONS, file[2:]) if file.startswith("w:") else os.path.join(CHARS, file)
        out = os.path.join(TMP, f"icon_{k:02d}.png")
        if not render_one(path, prefix, dyes, out, texture_for(file)):
            print(f"[icons] {k} {file} {prefix}: nothing found")
            continue
        img = bpy.data.images.load(out)
        img.scale(SIZE, SIZE)
        px = list(img.pixels)
        bpy.data.images.remove(img)
        cx, cy = k % COLS, k // COLS
        # (Blender's images start at the bottom row)
        for y in range(SIZE):
            dst = ((rows - 1 - cy) * SIZE + y) * W + cx * SIZE
            atlas[dst * 4:(dst + SIZE) * 4] = px[y * SIZE * 4:(y + 1) * SIZE * 4]
        print(f"[icons] {k} {file} {prefix}")
    img = bpy.data.images.new("look_icons", W, rows * SIZE, alpha=True)
    img.pixels = atlas
    img.filepath_raw = OUT
    img.file_format = "PNG"
    img.save()
    print(f"[icons] wrote {OUT} ({len(ICONS)} icons)")


main()
