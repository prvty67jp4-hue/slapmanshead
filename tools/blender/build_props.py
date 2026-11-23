"""Varrow Bridgeworks' own set pieces, built in Blender and baked with one hand-painted atlas.

    blender --background --factory-startup --python app/tools/blender/build_props.py [-- --no-renders]

Pieces (each its own glTF binary in app/assets/env/props, all textured from varrow_props.png, the
game's L_BRIDGE layer):

- varrow_railing.glb: a bridge railing 2 m long (dark wood posts and rails, iron brackets).
  Stands on the railing's hidden collision block.
- varrow_archway.glb: a stone archway, 5.4 m clear between its pillars (pillar centres at
  x = +-3 m), a segmental arch to 4.6 m with a keystone. Its pillars are hidden blocks.
- varrow_crane.glb: a dock crane on a plinth: a 6 m mast, a 5.5 m jib reaching forward over the
  water, a brace, a pulley, a rope and hook, a counterweight.
- varrow_crates.glb: three strapped cargo crates (two side by side, one on top).
- varrow_lantern.glb: a lantern on a wall bracket; its glass (material `glass`) glows in the game.

Axes (as the game's props expect): Blender +X is the prop's right, -Y its front (the game's +z),
+Z up; the origin is where it stands (the floor, or the wall face for the lantern). Metres.
Previews: app/shots/varrow/props/<name>.png.
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
SHOTS = os.path.join(APP, "shots", "varrow", "props")

W.PALETTE.update({
    "stone": ((0.36, 0.30, 0.24), 0.0, 0.85),
    "rope": ((0.30, 0.24, 0.15), 0.0, 0.9),
})
T.RECIPES.update({
    "rope": ("cord", (0.20, 0.15, 0.09), (0.36, 0.28, 0.17), (0.10, 0.07, 0.04), 0.4, 0.6),
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

FRONT = Vector((0, -1, 0))  # the prop's front (the game's +z)


def box(name, size, loc, mat, bevel=0.01):
    return W.box(name, size, loc, mat, bevel=bevel)


# ---------------------------------------------------------------- pieces

def railing():
    parts = []
    for x in (-0.95, 0.0, 0.95):
        parts.append(box("post", (0.11, 0.11, 1.0), (x, 0, 0.5), "wood_dark", 0.012))
        parts.append(box("cap", (0.15, 0.15, 0.05), (x, 0, 1.0), "metal_dark", 0.008))
    parts.append(box("rail_top", (2.04, 0.12, 0.08), (0, 0, 0.93), "wood", 0.012))
    parts.append(box("rail_mid", (2.0, 0.07, 0.06), (0, 0, 0.52), "wood", 0.01))
    # iron straps where the rails meet the posts
    for x in (-0.95, 0.0, 0.95):
        for z in (0.93, 0.52):
            parts.append(box("strap", (0.04, 0.15, 0.11), (x + 0.08, 0, z), "metal_dark", 0.004))
    return [W.bake(parts, "railing")]


def archway():
    parts = []
    for s in (-1, 1):
        x = 3.0 * s
        parts.append(box("pillar", (0.6, 0.6, 3.2), (x, 0, 1.6 + 0.2), "stone", 0.03))
        parts.append(box("plinth", (0.8, 0.8, 0.25), (x, 0, 0.125), "stone", 0.03))
        parts.append(box("capital", (0.8, 0.8, 0.22), (x, 0, 3.45), "stone", 0.03))
    # a segmental arch from the capitals (3.56 m) to 4.6 m, in voussoirs
    r = 5.2
    cy = 4.6 - r
    a0 = math.asin(3.0 / r)
    n = 11
    for k in range(n):
        t0 = -a0 + 2 * a0 * k / n
        t1 = -a0 + 2 * a0 * (k + 1) / n
        tm = (t0 + t1) / 2
        x = r * math.sin(tm)
        z = cy + r * math.cos(tm) + 0.25
        width = 2 * r * math.sin((t1 - t0) / 2) - 0.02
        ob = box("voussoir", (width, 0.62, 0.56 if k != n // 2 else 0.74), (0, 0, 0), "stone", 0.02)
        ob.rotation_euler = (0, tm, 0)
        ob.location = (x, 0, z + (0.08 if k == n // 2 else 0.0))
        parts.append(ob)
    return [W.bake(parts, "archway")]


def crane():
    parts = []
    # the mast and its foot
    parts.append(box("foot", (1.0, 1.0, 0.3), (0, 0, 0.15), "wood_dark", 0.02))
    parts.append(box("mast", (0.4, 0.4, 6.2), (0, 0, 3.1), "wood", 0.02))
    for z in (1.0, 3.0, 5.0):
        parts.append(box("band", (0.46, 0.46, 0.08), (0, 0, z), "metal_dark", 0.01))
    # the jib forward over the water, a short counter-jib behind with its weight
    parts.append(box("jib", (0.3, 6.6, 0.32), (0, -2.15, 6.0), "wood", 0.02))
    parts.append(box("weight", (0.7, 0.7, 0.7), (0, 1.0, 5.55), "metal_dark", 0.02))
    # a brace from the mast up to the jib
    brace = box("brace", (0.18, 3.0, 0.18), (0, 0, 0), "wood_dark", 0.01)
    brace.rotation_euler = (math.radians(-45), 0, 0)
    brace.location = (0, -1.05, 4.9)
    parts.append(brace)
    # the pulley at the jib's end, the rope and the hook
    wheel = W.cyl("pulley", 0.16, -0.05, 0.05, "metal_dark", axis="X", verts=16)
    wheel.location = (0, -5.2, 5.75)
    parts.append(wheel)
    parts.append(W.cyl("rope", 0.025, 3.3, 5.7, "rope", x=0, z=0, axis="Z", verts=6))
    parts[-1].location = (0, -5.36, 0)
    parts.append(W.tube_path("hook", [(0, -5.36, 3.3), (0, -5.36, 3.05), (0, -5.24, 2.92), (0, -5.12, 3.0), (0, -5.12, 3.1)], 0.03, "metal_dark", sides=6))
    return [W.bake(parts, "crane")]


def crates():
    parts = []
    for x in (-0.48, 0.48):
        parts.append(box("crate", (0.9, 0.9, 0.9), (x, 0, 0.45), "wood_light", 0.025))
        for dz in (0.18, 0.72):
            parts.append(box("strap", (0.93, 0.93, 0.05), (x, 0, dz), "metal_dark", 0.005))
    top = box("crate_top", (0.8, 0.8, 0.8), (0, 0, 0), "wood", 0.025)
    top.rotation_euler = (0, 0, math.radians(14))
    top.location = (0.05, 0.02, 1.3)
    parts.append(top)
    band = box("strap_top", (0.83, 0.83, 0.05), (0, 0, 0), "metal_dark", 0.005)
    band.rotation_euler = (0, 0, math.radians(14))
    band.location = (0.05, 0.02, 1.45)
    parts.append(band)
    return [W.bake(parts, "crates")]


def lantern():
    parts = []
    # the bracket out from the wall (the origin is on the wall face), and the hanging lantern
    parts.append(box("plate", (0.14, 0.03, 0.22), (0, -0.015, 0.0), "metal_dark", 0.005))
    parts.append(W.tube_path("arm", [(0, -0.02, 0.06), (0, -0.2, 0.08), (0, -0.32, 0.04), (0, -0.32, -0.04)], 0.014, "metal_dark", sides=6))
    c = Vector((0, -0.32, -0.32))
    parts.append(box("cap", (0.2, 0.2, 0.06), (c.x, c.y, c.z + 0.2), "metal_dark", 0.01))
    parts.append(box("roof", (0.13, 0.13, 0.06), (c.x, c.y, c.z + 0.25), "metal_dark", 0.01))
    parts.append(box("glass", (0.15, 0.15, 0.3), (c.x, c.y, c.z + 0.02), "glass", 0.005))
    for sx in (-1, 1):
        for sy in (-1, 1):
            parts.append(box("frame", (0.025, 0.025, 0.34), (c.x + 0.08 * sx, c.y + 0.08 * sy, c.z + 0.02), "metal_dark", 0.003))
    parts.append(box("base", (0.19, 0.19, 0.04), (c.x, c.y, c.z - 0.15), "metal_dark", 0.008))
    return [W.bake(parts, "lantern")]


PIECES = {"railing": railing, "archway": archway, "crane": crane, "crates": crates, "lantern": lantern}


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    renders = "--no-renders" not in argv
    W.clear_scene()
    built = {name: fn() for name, fn in PIECES.items()}
    everything = [ob for obs in built.values() for ob in obs]
    # one atlas for every piece (the game's L_BRIDGE layer)
    T.bake_texture(everything, os.path.join(OUT, "varrow_props.png"), size=1024, samples=32)
    os.makedirs(SHOTS, exist_ok=True)
    for name, obs in built.items():
        tris = sum(W.triangles(o) for o in obs)
        # each piece alone in the scene for its export and picture
        for o in everything:
            o.hide_render = o not in obs
            o.hide_set(o not in obs)
        if renders:
            W.render_turntable(obs, name, os.path.join(SHOTS, f"{name}.png"))
        W.export_glb(obs, os.path.join(OUT, f"varrow_{name}.glb"))
        lo, hi = W._bounds(obs)
        print(f"[props] {name}: {tris} triangles, size x {hi.x - lo.x:.2f} y {hi.y - lo.y:.2f} z {hi.z - lo.z:.2f}")


main()
