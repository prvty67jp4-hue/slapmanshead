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
    "cream": ((0.74, 0.68, 0.55), 0.0, 0.9),
    "leaf_dark": ((0.07, 0.17, 0.06), 0.0, 0.9),
    "leaf": ((0.15, 0.30, 0.09), 0.0, 0.9),
    "soil": ((0.10, 0.07, 0.05), 0.0, 1.0),
    "fruit_red": ((0.55, 0.06, 0.04), 0.0, 0.5),
    "fruit_orange": ((0.75, 0.32, 0.04), 0.0, 0.5),
    "fruit_green": ((0.25, 0.45, 0.08), 0.0, 0.5),
    "clay": ((0.50, 0.22, 0.10), 0.0, 0.8),
    "flower_pink": ((0.80, 0.30, 0.45), 0.0, 0.8),
    "flower_white": ((0.85, 0.82, 0.74), 0.0, 0.8),
    "flower_yellow": ((0.85, 0.62, 0.10), 0.0, 0.8),
})
T.RECIPES.update({
    "paper": ("cord", (0.70, 0.64, 0.50), (0.84, 0.79, 0.66), (0.52, 0.46, 0.34), 0.25, 0.4),
    "cloth_red": ("cord", (0.36, 0.07, 0.06), (0.52, 0.12, 0.09), (0.22, 0.04, 0.03), 0.4, 0.6),
    "cloth_teal": ("cord", (0.04, 0.24, 0.24), (0.07, 0.36, 0.35), (0.02, 0.14, 0.14), 0.4, 0.6),
    "cloth_ochre": ("cord", (0.52, 0.36, 0.10), (0.68, 0.50, 0.16), (0.34, 0.22, 0.06), 0.4, 0.6),
    "felt": ("cord", (0.06, 0.16, 0.09), (0.10, 0.24, 0.14), (0.03, 0.09, 0.05), 0.3, 0.5),
    "cream": ("cord", (0.62, 0.56, 0.44), (0.80, 0.74, 0.60), (0.46, 0.40, 0.30), 0.3, 0.5),
    "leaf_dark": ("leather", (0.04, 0.11, 0.035), (0.09, 0.20, 0.07), (0.02, 0.06, 0.02), 0.2, 0.8),
    "leaf": ("leather", (0.10, 0.22, 0.06), (0.20, 0.38, 0.11), (0.28, 0.45, 0.14), 0.5, 0.8),
    "soil": ("leather", (0.06, 0.04, 0.03), (0.12, 0.085, 0.06), (0.04, 0.03, 0.02), 0.2, 0.6),
    "fruit_red": ("brass", (0.42, 0.04, 0.03), (0.62, 0.09, 0.05), (0.95, 0.55, 0.45), 0.4, 0.5),
    "fruit_orange": ("brass", (0.62, 0.24, 0.03), (0.82, 0.40, 0.06), (1.0, 0.75, 0.40), 0.4, 0.5),
    "fruit_green": ("brass", (0.18, 0.36, 0.05), (0.30, 0.52, 0.10), (0.65, 0.85, 0.40), 0.4, 0.5),
    "clay": ("leather", (0.38, 0.16, 0.07), (0.56, 0.26, 0.12), (0.70, 0.42, 0.25), 0.5, 0.6),
    "flower_pink": ("cord", (0.66, 0.22, 0.34), (0.88, 0.40, 0.55), (0.95, 0.70, 0.78), 0.3, 0.4),
    "flower_white": ("cord", (0.72, 0.70, 0.62), (0.92, 0.90, 0.84), (1.0, 1.0, 0.95), 0.3, 0.4),
    "flower_yellow": ("cord", (0.70, 0.48, 0.06), (0.92, 0.70, 0.14), (1.0, 0.88, 0.45), 0.3, 0.4),
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


def blob(name, r, loc, mat, scale=(1, 1, 1), sub=2, seed=0):
    """A lumpy ball (a clump of leaves, a bush): an icosphere, its points pushed in and out."""
    import bmesh
    import random
    rnd = random.Random(seed)
    bm = bmesh.new()
    bmesh.ops.create_icosphere(bm, subdivisions=sub, radius=r)
    for v in bm.verts:
        v.co *= 1.0 + rnd.uniform(-0.12, 0.12)
    bmesh.ops.scale(bm, vec=scale, verts=bm.verts)
    bmesh.ops.translate(bm, vec=loc, verts=bm.verts)
    return W.new_object(name, bm, mat)


def stall(cloth, goods):
    """A market stall: timber posts, a counter, a striped awning sloping to the front with a
    scalloped edge, goods on the counter and a shelf behind, a lantern hung under the awning.
    3 m wide, 2 m deep; its front (the customers' side) faces -Y (the game's +z at yaw 0)."""
    import random
    rnd = random.Random(hash(goods) & 0xFFFF)
    parts = []
    for x in (-1.45, 1.45):
        parts.append(box("post_front", (0.1, 0.1, 2.3), (x, -0.95, 1.15), "wood_dark", 0.012))
        parts.append(box("post_back", (0.1, 0.1, 2.65), (x, 0.95, 1.325), "wood_dark", 0.012))
        parts.append(box("side_rail", (0.06, 1.9, 0.08), (x, 0.0, 2.2), "wood", 0.01))
    # the counter: a panelled front, a thick top
    parts.append(box("counter", (2.8, 0.55, 0.9), (0, -0.7, 0.45), "wood", 0.015))
    for x in (-0.95, 0.0, 0.95):
        parts.append(box("panel", (0.8, 0.03, 0.62), (x, -0.985, 0.46), "wood_dark", 0.01))
    parts.append(box("counter_top", (2.95, 0.68, 0.07), (0, -0.7, 0.93), "wood_dark", 0.012))
    # the shelf behind
    parts.append(box("shelf", (2.6, 0.4, 0.05), (0, 0.75, 1.1), "wood", 0.01))
    parts.append(box("shelf2", (2.6, 0.35, 0.05), (0, 0.78, 1.55), "wood", 0.01))
    # the awning: stripes from the back beam down past the front, a scalloped valance
    slope = math.atan2(0.55, 2.4)
    stripes = 10
    for k in range(stripes):
        x = -1.6 + (k + 0.5) * 3.2 / stripes
        s = box("awning", (3.2 / stripes + 0.002, 2.45, 0.03), (0, 0, 0), cloth if k % 2 == 0 else "cream", 0.006)
        s.rotation_euler = (-slope, 0, 0)
        s.location = (x, -0.25, 2.42)
        parts.append(s)
    for k in range(stripes * 2):
        x = -1.6 + (k + 0.5) * 3.2 / (stripes * 2)
        tab = W.cyl("valance", 0.085, -1.482, -1.458, cloth if (k // 2) % 2 == 0 else "cream", x=0.0, z=1.98, verts=10, axis="Y", bevel=0.002)
        tab.location = (x, 0, 0)
        parts.append(tab)
    parts.append(box("valance_band", (3.2, 0.03, 0.12), (0, -1.47, 2.08), cloth, 0.005))
    # goods
    if goods == "produce":
        for k in range(4):
            cx = -1.05 + k * 0.7
            parts.append(box("crate", (0.55, 0.42, 0.18), (cx, -0.72, 1.05), "wood", 0.01))
            mat = ("fruit_red", "fruit_orange", "fruit_green", "fruit_red")[k]
            for j in range(9):
                parts.append(W.sphere("fruit", 0.055, (cx - 0.18 + (j % 3) * 0.18 + rnd.uniform(-0.02, 0.02), -0.84 + (j // 3) * 0.12, 1.17 + rnd.uniform(0, 0.03)), mat, seg=8, rings=6))
        for k in range(5):
            parts.append(cyl_z("basket", 0.17, 1.13, 1.33, "wood_light", x=-1.0 + k * 0.5, y=0.78, r1=0.21, verts=10))
            parts.append(blob("greens", 0.15, (-1.0 + k * 0.5, 0.78, 1.37), "leaf", scale=(1, 1, 0.6), sub=1, seed=k))
    elif goods == "cloth":
        for k in range(5):
            cx = -1.1 + k * 0.55
            for j in range(4):
                mat = ("cloth_red", "cloth_teal", "cloth_ochre", "cream", "felt")[(k + j) % 5]
                parts.append(box("bolt", (0.46, 0.4, 0.07), (cx, -0.72, 1.0 + j * 0.075), mat, 0.02))
        for k in range(6):
            mat = ("cloth_teal", "cloth_red", "cream", "cloth_ochre", "felt", "cloth_teal")[k]
            parts.append(W.cyl("roll", 0.07, -0.18, 0.18, mat, x=0.78, z=1.2, verts=10, axis="X", bevel=0.01))
            parts[-1].location = (-1.1 + k * 0.44, 0, 0)
        for k in range(4):
            sc = box("scarf", (0.3, 0.02, 0.6), (-0.9 + k * 0.6, -0.92, 1.85), ("cloth_red", "cloth_ochre", "cloth_teal", "cream")[k], 0.01)
            parts.append(sc)
    else:  # pottery
        for k in range(6):
            cx = -1.15 + k * 0.46
            hgt = rnd.uniform(0.18, 0.34)
            parts.append(cyl_z("pot", 0.12, 0.965, 0.965 + hgt, "clay", x=cx, y=-0.72, r1=rnd.uniform(0.06, 0.1), verts=12))
            parts.append(cyl_z("pot_lip", 0.075, 0.965 + hgt, 0.985 + hgt, "clay", x=cx, y=-0.72, verts=12))
        for k in range(5):
            parts.append(W.sphere("jar", 0.16, (-1.0 + k * 0.5, 0.75, 1.3), "clay", seg=12, rings=8, scale=(1, 1, 1.15)))
            parts.append(W.sphere("jar2", 0.12, (-0.8 + k * 0.42, 0.78, 1.72), "cream", seg=10, rings=7, scale=(1, 1, 1.2)))
    # a sign board over the counter, a lantern hung from the awning's front beam
    parts.append(box("sign", (1.1, 0.05, 0.32), (0, -1.2, 2.45), "wood_dark", 0.01))
    parts.append(box("sign_face", (0.95, 0.02, 0.22), (0, -1.23, 2.45), "paper", 0.004))
    parts.append(cyl_z("lantern_chain", 0.008, 1.7, 2.05, "metal_dark", x=1.2, y=-0.95, verts=6))
    parts.append(box("lantern_glass", (0.16, 0.16, 0.22), (1.2, -0.95, 1.58), "glass", 0.004))
    parts.append(box("lantern_cap", (0.2, 0.2, 0.04), (1.2, -0.95, 1.71), "metal_dark", 0.005))
    return [W.bake(parts, f"stall_{goods}")]


def tree():
    """A plaza tree: a stone ring with soil, a leaning trunk with two branches, a crown of lumpy
    leaf clumps (darker inside, lighter out)."""
    parts = []
    for a in range(10):
        ang = a * math.tau / 10
        b = box("ring_stone", (0.62, 0.32, 0.42), (0, 0, 0), "stone", 0.03)
        b.rotation_euler = (0, 0, ang)
        b.location = (math.sin(ang) * 0.95, math.cos(ang) * 0.95, 0.21)
        parts.append(b)
    parts.append(cyl_z("soil", 0.9, 0.0, 0.36, "soil", verts=16))
    trunk = [(0.0, 0.0, 0.3), (0.05, 0.02, 1.2), (0.12, -0.03, 2.1), (0.08, 0.0, 2.9)]
    parts.append(W.tube_path("trunk", trunk, 0.17, "wood", sides=10))
    parts.append(W.tube_path("branch_a", [(0.1, 0.0, 1.9), (0.6, 0.2, 2.6), (0.9, 0.3, 3.1)], 0.07, "wood", sides=7))
    parts.append(W.tube_path("branch_b", [(0.08, 0.0, 2.2), (-0.5, -0.3, 2.9), (-0.8, -0.4, 3.3)], 0.06, "wood", sides=7))
    for k, (x, y, z, r, mat) in enumerate(((0.0, 0.0, 3.4, 1.15, "leaf_dark"), (0.9, 0.3, 3.3, 0.85, "leaf"), (-0.8, -0.4, 3.5, 0.85, "leaf"), (0.2, -0.8, 3.2, 0.75, "leaf"),
                                           (-0.2, 0.8, 3.6, 0.8, "leaf"), (0.1, 0.1, 4.15, 0.75, "leaf"), (0.7, -0.4, 3.9, 0.6, "leaf"), (-0.6, 0.5, 4.0, 0.6, "leaf"))):
        parts.append(blob("crown", r, (x, y, z), mat, scale=(1, 1, 0.82), sub=2, seed=k + 3))
    return [W.bake(parts, "tree")]


def planter():
    """A long stone planter: a bush along it and flowers in three colours."""
    import random
    rnd = random.Random(11)
    parts = [box("planter", (1.8, 0.62, 0.5), (0, 0, 0.25), "stone", 0.03), box("rim", (1.9, 0.72, 0.08), (0, 0, 0.5), "stone", 0.02),
             box("soil", (1.66, 0.5, 0.06), (0, 0, 0.5), "soil", 0.0)]
    for k in range(4):
        parts.append(blob("bush", 0.3, (-0.6 + k * 0.4, rnd.uniform(-0.05, 0.05), 0.68), "leaf" if k % 2 else "leaf_dark", scale=(1.1, 0.9, 0.85), sub=1, seed=k + 20))
    for k in range(22):
        mat = ("flower_pink", "flower_white", "flower_yellow")[k % 3]
        parts.append(W.sphere("flower", 0.05, (rnd.uniform(-0.8, 0.8), rnd.uniform(-0.24, 0.24), rnd.uniform(0.78, 0.98)), mat, seg=6, rings=4, scale=(1, 1, 0.6)))
    return [W.bake(parts, "planter")]


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
    "stall_produce": lambda: stall("cloth_red", "produce"),
    "stall_cloth": lambda: stall("cloth_teal", "cloth"),
    "stall_pottery": lambda: stall("cloth_ochre", "pottery"),
    "tree": tree,
    "planter": planter,
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
