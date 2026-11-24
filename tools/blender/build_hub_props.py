"""Varrow Centre's own set pieces (the hub, see app/src/varrow.rs), built in Blender and baked
into one atlas, in the Bridgeworks props' style: chunky, blocky, readable, dressed stone, dark
timber and black iron, brass for the accents.

    blender --background --factory-startup --python app/tools/blender/build_hub_props.py [-- --no-renders]

Pieces (each its own glTF binary in app/assets/env/props, textured from varrow_hub.png, the
game's L_HUB layer; material `glass` glows):

- hub_fountain.glb: the fountain's top on its pedestal (2.4 m): a wide stone bowl, a column, a
  smaller bowl and a brass bell on top (the Ferryman's Bell), four spouts; plus the basin's
  coping.
- hub_bench.glb: a timber bench (seat 0.42 m) on iron legs, a back rail.
- hub_lamp.glb: an iron lamp post, 3.3 m, a glass lantern on top.
- hub_board.glb: a notice board under a little roof, pinned with notices (the Challenge Boards
  and the scoreboards). Its origin is on the wall face, its middle 2.2 m up (the game places it).
- hub_lectern.glb: the Gallery's lectern: a sloped timber top on a post, a brass plate.
- hub_target.glb: a practice target on a timber frame (dressing down the lanes).
- hub_table.glb: a square games table, top 0.75 m over its deck, an inlaid board.
- hub_chair.glb: a timber chair, seat 0.45 m, its back to -Y (it faces the table).
- hub_mirror.glb: a tall mirror in a carved frame, against the wall (origin on the wall face).
- hub_wardrobe.glb: a tall open wardrobe, clothes on a rail inside.

Axes as build_props.py: Blender +X right, -Y the prop's front (the game's +z), +Z up; origin
where it stands. Previews: app/shots/varrow/hub_props/<name>.png.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy  # noqa: E402
from mathutils import Vector  # noqa: E402

import weapons_lib as W  # noqa: E402
import weapons_tex as T  # noqa: E402

APP = os.path.normpath(os.path.join(HERE, "..", ".."))
OUT = os.path.join(APP, "assets", "env", "props")
SHOTS = os.path.join(APP, "shots", "varrow", "hub_props")

W.PALETTE.update({
    "stone": ((0.36, 0.30, 0.24), 0.0, 0.85),
    "paper": ((0.78, 0.72, 0.58), 0.0, 0.9),
    "cloth_red": ((0.46, 0.10, 0.08), 0.0, 0.9),
    "cloth_teal": ((0.05, 0.32, 0.32), 0.0, 0.9),
    "cloth_ochre": ((0.62, 0.44, 0.12), 0.0, 0.9),
    "felt": ((0.08, 0.20, 0.12), 0.0, 0.95),
})
T.RECIPES.update({
    "paper": ("cord", (0.70, 0.64, 0.50), (0.84, 0.79, 0.66), (0.52, 0.46, 0.34), 0.25, 0.4),
    "cloth_red": ("cord", (0.36, 0.07, 0.06), (0.52, 0.12, 0.09), (0.22, 0.04, 0.03), 0.4, 0.6),
    "cloth_teal": ("cord", (0.04, 0.24, 0.24), (0.07, 0.36, 0.35), (0.02, 0.14, 0.14), 0.4, 0.6),
    "cloth_ochre": ("cord", (0.52, 0.36, 0.10), (0.68, 0.50, 0.16), (0.34, 0.22, 0.06), 0.4, 0.6),
    "felt": ("cord", (0.06, 0.16, 0.09), (0.10, 0.24, 0.14), (0.03, 0.09, 0.05), 0.3, 0.5),
})
_paint = T.paint_material


def paint_stone(mat):
    """Dressed sandstone: blocks with soft joints, a little weathering, worn lighter edges."""
    g = T.G(mat)
    edge = g.edge_mask(radius=0.02, gain=4.0)
    occ = g.ao(distance=0.15)
    cells = g.voronoi(g.coords((1.0, 1.0, 2.2)), 3.2, "DISTANCE_TO_EDGE")
    joint = g.math("LESS_THAN", cells, 0.035)
    col = g.ramp(g.noise(g.coords(), 9.0, 5.0), [(0.25, (0.30, 0.24, 0.18)), (0.75, (0.42, 0.35, 0.27))])
    col = g.mix(g.math("MULTIPLY", joint, 0.65), col, (0.14, 0.11, 0.09))
    col = g.mix(g.math("MULTIPLY", edge, 0.5), col, (0.55, 0.48, 0.38))
    shade = g.mix(occ, (0.4, 0.4, 0.4), (1.0, 1.0, 1.0))
    g.emit(g.mix(1.0, col, shade, blend="MULTIPLY"))


def paint(mat, name):
    if name == "stone":
        return paint_stone(mat)
    return _paint(mat, name)


T.paint_material = paint


def box(name, size, loc, mat, bevel=0.01):
    return W.box(name, size, loc, mat, bevel=bevel)


def cyl_z(name, r, z0, z1, mat, x=0.0, y=0.0, r1=None, verts=16, bevel=0.004):
    """An upright cylinder (or cone) from z0 to z1 at (x, y)."""
    return W.cyl(name, r, z0, z1, mat, x=y, z=x, r1=r1, verts=verts, bevel=bevel, axis="Z")


# ---------------------------------------------------------------- pieces

def fountain():
    parts = []
    # the basin's coping (on the 0.6 m walls, 8.8 m square outside, 7.6 inside)
    for (sx, sy, x, y) in ((8.9, 0.7, 0, -4.1), (8.9, 0.7, 0, 4.1), (0.7, 7.6, -4.1, 0), (0.7, 7.6, 4.1, 0)):
        parts.append(box("coping", (sx, sy, 0.12), (x, y, 0.66), "stone", 0.03))
    # the pedestal's base moulding and cap (the pedestal itself is a level block, 1.8 m square)
    parts.append(box("plinth", (2.1, 2.1, 0.3), (0, 0, 0.15), "stone", 0.04))
    parts.append(box("cap", (2.0, 2.0, 0.16), (0, 0, 2.48), "stone", 0.03))
    # the big bowl, its column, the small bowl, the bell
    parts.append(cyl_z("bowl", 1.15, 2.56, 2.86, "stone", r1=1.45, verts=20, bevel=0.02))
    parts.append(cyl_z("bowl_lip", 1.5, 2.86, 2.98, "stone", verts=20, bevel=0.02))
    parts.append(cyl_z("column", 0.28, 2.98, 3.9, "stone", r1=0.22, verts=12))
    parts.append(cyl_z("bowl2", 0.42, 3.9, 4.1, "stone", r1=0.66, verts=16, bevel=0.015))
    parts.append(cyl_z("bell_hook", 0.06, 4.1, 4.5, "metal_dark", verts=8))
    parts.append(cyl_z("bell", 0.42, 4.05, 4.5, "brass", r1=0.18, verts=16, bevel=0.01))
    parts.append(W.sphere("bell_top", 0.18, (0, 0, 4.52), "brass", seg=12, rings=6, scale=(1, 1, 0.6)))
    parts.append(W.sphere("clapper", 0.07, (0, 0, 4.0), "metal_dark", seg=8, rings=5))
    # four spouts out of the pedestal, lion-ish blocks in brass
    for a in range(4):
        ang = a * math.pi / 2
        d = Vector((math.sin(ang), -math.cos(ang), 0))
        p = d * 0.95 + Vector((0, 0, 1.6))
        sp = box("spout", (0.22, 0.34, 0.2), (0, 0, 0), "brass", 0.02)
        sp.rotation_euler = (0, 0, ang)
        sp.location = p + d * 0.12
        parts.append(sp)
    return [W.bake(parts, "fountain")]


def bench():
    parts = []
    for x in (-0.48, 0.48):
        parts.append(box("leg", (0.08, 0.42, 0.4), (x, 0.0, 0.2), "metal_dark", 0.01))
        parts.append(box("back_post", (0.07, 0.06, 0.48), (x, 0.24, 0.62), "metal_dark", 0.008))
    for k, y in enumerate((-0.15, -0.03, 0.09, 0.21)):
        parts.append(box("slat", (1.18, 0.1, 0.05), (0, y, 0.42), "wood" if k % 2 else "wood_dark", 0.01))
    for z in (0.62, 0.8):
        parts.append(box("back_rail", (1.18, 0.05, 0.1), (0, 0.27, z), "wood_dark", 0.01))
    return [W.bake(parts, "bench")]


def lamp():
    parts = []
    parts.append(cyl_z("base", 0.22, 0.0, 0.25, "metal_dark", r1=0.16, verts=10))
    parts.append(cyl_z("post", 0.06, 0.25, 3.0, "metal_dark", r1=0.05, verts=8))
    parts.append(cyl_z("collar", 0.1, 1.2, 1.3, "brass", verts=10))
    parts.append(box("cap", (0.34, 0.34, 0.06), (0, 0, 3.0), "metal_dark", 0.01))
    parts.append(box("glass", (0.24, 0.24, 0.34), (0, 0, 3.2), "glass", 0.005))
    for sx in (-1, 1):
        for sy in (-1, 1):
            parts.append(box("frame", (0.03, 0.03, 0.38), (0.13 * sx, 0.13 * sy, 3.2), "metal_dark", 0.004))
    parts.append(cyl_z("roof", 0.24, 3.38, 3.56, "metal_dark", r1=0.04, verts=4))
    parts.append(W.sphere("finial", 0.05, (0, 0, 3.6), "brass", seg=8, rings=5))
    return [W.bake(parts, "lamp")]


def board():
    parts = []
    # backing and frame (the origin is the wall face at the board's middle)
    parts.append(box("back", (2.4, 0.06, 1.4), (0, -0.04, 0), "wood_dark", 0.01))
    for (sx, sz, x, z) in ((2.56, 0.1, 0, 0.75), (2.56, 0.1, 0, -0.75), (0.1, 1.5, -1.25, 0), (0.1, 1.5, 1.25, 0)):
        parts.append(box("frame", (sx, 0.12, sz), (x, -0.08, z), "wood", 0.012))
    # a little roof
    roof = box("roof", (2.8, 0.5, 0.06), (0, 0, 0), "wood_dark", 0.01)
    roof.rotation_euler = (math.radians(-25), 0, 0)
    roof.location = (0, -0.2, 0.92)
    parts.append(roof)
    for x in (-1.2, 1.2):
        parts.append(box("bracket", (0.06, 0.4, 0.06), (x, -0.2, 0.8), "metal_dark", 0.006))
    # notices pinned on it (paper, a few red seals)
    import random
    rnd = random.Random(7)
    for k in range(9):
        w, hgt = rnd.uniform(0.3, 0.5), rnd.uniform(0.32, 0.5)
        x = -0.85 + (k % 3) * 0.85 + rnd.uniform(-0.08, 0.08)
        z = 0.38 - (k // 3) * 0.42 + rnd.uniform(-0.04, 0.04)
        n = box("notice", (w, 0.01, hgt), (0, 0, 0), "paper", 0.002)
        n.rotation_euler = (0, rnd.uniform(-0.12, 0.12), 0)
        n.location = (x, -0.085, z)
        parts.append(n)
        parts.append(W.sphere("seal", 0.03, (x, -0.095, z + hgt * 0.38), "cloth_red", seg=8, rings=5, scale=(1, 0.4, 1)))
    return [W.bake(parts, "board")]


def lectern():
    parts = []
    parts.append(box("foot", (0.6, 0.5, 0.08), (0, 0, 0.04), "wood_dark", 0.012))
    parts.append(box("post", (0.16, 0.16, 0.95), (0, 0, 0.55), "wood", 0.012))
    top = box("top", (0.62, 0.46, 0.06), (0, 0, 0), "wood_dark", 0.01)
    top.rotation_euler = (math.radians(-22), 0, 0)
    top.location = (0, 0.02, 1.06)
    parts.append(top)
    plate = box("plate", (0.3, 0.2, 0.012), (0, 0, 0), "brass", 0.003)
    plate.rotation_euler = (math.radians(-22), 0, 0)
    plate.location = (0, -0.0, 1.1)
    parts.append(plate)
    return [W.bake(parts, "lectern")]


def target():
    parts = []
    for x in (-0.55, 0.55):
        parts.append(box("leg", (0.08, 0.08, 1.6), (x, 0.08, 0.8), "wood_dark", 0.01))
    parts.append(box("bar", (1.24, 0.08, 0.08), (0, 0.08, 1.55), "wood", 0.01))
    # the target face: rings
    for k, (r, mat) in enumerate(((0.48, "paper"), (0.36, "cloth_red"), (0.24, "paper"), (0.12, "cloth_red"))):
        parts.append(W.cyl(f"face{k}", r, -0.02 - 0.012 * k, 0.03, mat, x=0.0, z=1.05, verts=20, axis="Y"))
    return [W.bake(parts, "target")]


def table():
    parts = []
    parts.append(box("top", (1.02, 1.02, 0.07), (0, 0, 0.715), "wood", 0.015))
    parts.append(box("inlay", (0.84, 0.84, 0.01), (0, 0, 0.753), "felt", 0.003))
    parts.append(box("apron", (0.9, 0.9, 0.12), (0, 0, 0.62), "wood_dark", 0.01))
    for sx in (-1, 1):
        for sy in (-1, 1):
            parts.append(box("leg", (0.08, 0.08, 0.6), (0.4 * sx, 0.4 * sy, 0.3), "wood_dark", 0.01))
    return [W.bake(parts, "table")]


def chair():
    parts = []
    parts.append(box("seat", (0.46, 0.44, 0.06), (0, 0, 0.45), "wood", 0.012))
    for sx in (-1, 1):
        for sy in (-1, 1):
            parts.append(box("leg", (0.05, 0.05, 0.42), (0.19 * sx, 0.18 * sy, 0.21), "wood_dark", 0.008))
        parts.append(box("back_post", (0.05, 0.05, 0.55), (0.19 * sx, 0.2, 0.72), "wood_dark", 0.008))
    for z in (0.75, 0.92):
        parts.append(box("back_rail", (0.42, 0.04, 0.08), (0, 0.2, z), "wood", 0.008))
    return [W.bake(parts, "chair")]


def mirror():
    parts = []
    # a tall glass in a carved frame, standing against the wall (origin on the wall face)
    parts.append(box("glass", (1.5, 0.03, 2.3), (0, -0.06, 1.45), "glass", 0.004))
    for (sx, sz, x, z) in ((1.8, 0.16, 0, 2.68), (1.8, 0.2, 0, 0.22), (0.16, 2.6, -0.83, 1.45), (0.16, 2.6, 0.83, 1.45)):
        parts.append(box("frame", (sx, 0.14, sz), (x, -0.07, z), "wood_dark", 0.02))
    parts.append(box("crest", (0.6, 0.12, 0.3), (0, -0.07, 2.88), "brass", 0.02))
    parts.append(W.sphere("crest_knob", 0.08, (0, -0.1, 3.05), "brass", seg=10, rings=6))
    for x in (-0.65, 0.65):
        parts.append(box("foot", (0.24, 0.3, 0.12), (x, -0.12, 0.06), "wood_dark", 0.015))
    return [W.bake(parts, "mirror")]


def wardrobe():
    parts = []
    parts.append(box("back", (1.1, 0.06, 2.2), (0, 0.42, 1.1), "wood_dark", 0.01))
    for x in (-0.53, 0.53):
        parts.append(box("side", (0.06, 0.9, 2.2), (x, 0, 1.1), "wood_dark", 0.01))
    parts.append(box("top", (1.2, 0.95, 0.1), (0, 0, 2.24), "wood", 0.015))
    parts.append(box("base", (1.2, 0.95, 0.12), (0, 0, 0.06), "wood", 0.015))
    # the doors standing open
    for s in (-1, 1):
        d = box("door", (0.55, 0.04, 2.0), (0, 0, 0), "wood", 0.01)
        d.rotation_euler = (0, 0, math.radians(-70 * s))
        d.location = (0.53 * s + 0.1 * s, -0.55, 1.1)
        parts.append(d)
    # the rail and the clothes on it
    parts.append(W.cyl("rail", 0.015, -0.5, 0.5, "brass", x=0.1, z=1.9, verts=6, axis="X"))
    for k, mat in enumerate(("cloth_red", "cloth_teal", "cloth_ochre", "cloth_red", "cloth_teal")):
        x = -0.36 + 0.18 * k
        parts.append(box("coat", (0.08, 0.5, 0.95), (x, 0.1, 1.4), mat, 0.03))
    return [W.bake(parts, "wardrobe")]


PIECES = {
    "fountain": fountain,
    "bench": bench,
    "lamp": lamp,
    "board": board,
    "lectern": lectern,
    "target": target,
    "table": table,
    "chair": chair,
    "mirror": mirror,
    "wardrobe": wardrobe,
}


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    renders = "--no-renders" not in argv
    W.clear_scene()
    built = {name: fn() for name, fn in PIECES.items()}
    everything = [ob for obs in built.values() for ob in obs]
    T.bake_texture(everything, os.path.join(OUT, "varrow_hub.png"), size=1024, samples=32)
    os.makedirs(SHOTS, exist_ok=True)
    for name, obs in built.items():
        tris = sum(W.triangles(o) for o in obs)
        for o in everything:
            o.hide_render = o not in obs
            o.hide_set(o not in obs)
        if renders:
            W.render_turntable(obs, name, os.path.join(SHOTS, f"{name}.png"))
        W.export_glb(obs, os.path.join(OUT, f"hub_{name}.glb"))
        lo, hi = W._bounds(obs)
        print(f"[hub props] {name}: {tris} triangles, size x {hi.x - lo.x:.2f} y {hi.y - lo.y:.2f} z {hi.z - lo.z:.2f}")


main()
