"""Each fighter's signature accessory, fitted onto their own body and baked with a hand-painted
texture, for the game to skin onto their skeleton.

    cargo test --release dump_fighter_refs -- --ignored      (once: writes tools/blender/ref/)
    blender --background --factory-startup --python app/tools/blender/build_accessories.py [-- names] [-- --no-renders]

- Corvin Vale (ranger): a brass warden badge on the coat's left breast, on a leather backing.
- Sable Rook (huntress): a long knitted scarf, wrapped twice round the neck, its two tails hanging
  down the chest (the game sways them).
- Bram "Anvil" Kask (brawler): a pauldron of three hammered iron plates following the left
  shoulder's curve (each bends with the skin under it in the game), a strap across the chest, and a leather smith's apron on a belt (its skirt sways and follows the thighs).
- Ilya Fen (striker): a hood over the head and shoulders with a thin trim along its front edge (an
  arc over the brow, down to the jaw): a soft accent in the player's colour in the game, brighter
  while her ability is on.

Space: the game's model space for each fighter (metres, y up, the fighter faces +z, their left is
+x), read from tools/blender/ref (`anchors.json` for the bones, `<name>.obj` for the body).
Blender sees it as x = left, -y = forward, z = up; the glTF export turns it back.

What it writes:
- app/assets/characters/acc_<name>.glb: the accessory, every piece its own mesh (the game picks
  how each piece follows the skeleton by its name: see ACCESSORIES in src/chars.rs).
- app/assets/characters/accessories.png: the four baked textures, one quarter each (ranger top
  left, huntress top right, brawler bottom left, striker bottom right); the models' UVs point
  into their quarter.
- app/shots/character-art/accessories/<name>.png: a turntable of the accessory on the body.
"""

import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bmesh  # noqa: E402
import bpy  # noqa: E402
from mathutils import Matrix, Vector  # noqa: E402
from mathutils.bvhtree import BVHTree  # noqa: E402

import weapons_lib as W  # noqa: E402
import weapons_tex as T  # noqa: E402

APP = os.path.normpath(os.path.join(HERE, "..", ".."))
REF = os.path.join(HERE, "ref")
OUT = os.path.join(APP, "assets", "characters")
SHOTS = os.path.join(APP, "shots", "character-art", "accessories")
NAMES = ["ranger", "huntress", "brawler", "striker"]
QUARTER = {"ranger": (0.0, 0.5), "huntress": (0.5, 0.5), "brawler": (0.0, 0.0), "striker": (0.5, 0.0)}
TEX = 512

UP = Vector((0, 0, 1))
FWD = Vector((0, -1, 0))  # the way the fighter faces
LEFT = Vector((1, 0, 0))

# extra paints: the scarf's knit takes the team colour (light), the hood's is dark; the glowing
# trim is drawn by the game in the player's colour
W.PALETTE.update({
    "knit": ((0.62, 0.60, 0.56), 0.0, 0.9),
    "hood_cloth": ((0.05, 0.05, 0.06), 0.0, 0.9),
    "apron": ((0.30, 0.18, 0.09), 0.0, 0.75),
    "glow": ((0.9, 0.9, 0.9), 0.0, 0.4),
    "iron": ((0.16, 0.165, 0.18), 0.9, 0.36),
})
T.RECIPES.update({
    "knit": ("cord", (0.30, 0.29, 0.27), (0.50, 0.48, 0.45), (0.18, 0.17, 0.16), 0.4, 0.6),
    "hood_cloth": ("cord", (0.030, 0.030, 0.038), (0.075, 0.075, 0.09), (0.012, 0.012, 0.016), 0.35, 0.6),
    "apron": ("leather", (0.20, 0.11, 0.05), (0.34, 0.20, 0.10), (0.08, 0.045, 0.02), 0.55, 0.7),
    "glow": ("glass", (0.85, 0.85, 0.85), (1.0, 1.0, 1.0), (1.0, 1.0, 1.0), 0.2, 0.0),
    "iron": ("metal", (0.075, 0.078, 0.088), (0.12, 0.124, 0.136), (0.40, 0.41, 0.44), 0.55, 0.7),
})


_paint = T.paint_material


def paint_iron(mat):
    """Dark hammered iron: shallow hammer marks, a little grime, and thin worn highlights on the
    edges only (no bright faces)."""
    g = T.G(mat)
    edge = g.edge_mask(radius=0.004, gain=6.0)
    occ = g.ao()
    dents = g.voronoi(g.coords(), 55.0)
    col = g.ramp(dents, [(0.0, (0.022, 0.023, 0.027)), (0.55, (0.05, 0.052, 0.058)), (1.0, (0.066, 0.068, 0.076))])
    col = g.mix(g.math("MULTIPLY", g.noise(g.coords(), 18.0), 0.35), col, (0.018, 0.018, 0.021))
    wear = g.math("GREATER_THAN", edge, 0.6)
    col = g.mix(g.math("MULTIPLY", wear, 0.75), col, (0.30, 0.31, 0.33))
    shade = g.mix(occ, (0.35, 0.35, 0.35), (1.0, 1.0, 1.0))
    g.emit(g.mix(1.0, col, shade, blend="MULTIPLY"))


def paint_knit(mat):
    """The scarf's knit: fine diagonal ribs and pale stripes across it (the game tints it in the
    player's colour, so the stripes come out a pale shade of it and the scarf reads as a scarf
    against any outfit)."""
    g = T.G(mat)
    occ = g.ao()
    fib = g.wave(g.coords((1, 1, 1)), 140.0, 2.0, "DIAGONAL", detail=1.0)
    col = g.ramp(fib, [(0.2, (0.30, 0.29, 0.27)), (0.8, (0.47, 0.45, 0.42))])
    band = g.wave(g.coords((1, 1, 1)), 9.0, 0.0, "Z", detail=0.0)
    col = g.mix(g.math("GREATER_THAN", band, 0.8), col, (0.92, 0.90, 0.86))
    shade = g.mix(occ, (0.4, 0.4, 0.4), (1.0, 1.0, 1.0))
    g.emit(g.mix(1.0, col, shade, blend="MULTIPLY"))


def paint(mat, name):
    if name == "iron":
        return paint_iron(mat)
    if name == "knit":
        return paint_knit(mat)
    return _paint(mat, name)


T.paint_material = paint


def gv(x, y, z):
    """A point in the game's model space, in Blender's."""
    return Vector((x, -z, y))


def game(v):
    return (v.x, v.z, -v.y)


# ---------------------------------------------------------------- the body to fit onto

class Body:
    def __init__(self, name, anchors):
        bpy.ops.wm.obj_import(filepath=os.path.join(REF, f"{name}.obj"), forward_axis="NEGATIVE_Z", up_axis="Y")
        self.ob = bpy.context.selected_objects[0]
        self.ob.name = f"ref_{name}"
        dg = bpy.context.evaluated_depsgraph_get()
        self.bvh = BVHTree.FromObject(self.ob, dg)
        self.a = {k: gv(*v) for k, v in anchors.items()}

    def ray(self, origin, direction):
        """The first surface hit (point, normal), or None."""
        p, n, _, _ = self.bvh.ray_cast(origin, direction.normalized(), 5.0)
        return None if p is None else (p, n)

    def front(self, x, y, out=0.0):
        """The front of the body at (x, y) (game left-right, height), `out` metres off it."""
        h = self.ray(gv(x, y, 0.7), -FWD)
        if h is None:
            return None
        p, n = h
        if n.dot(FWD) < 0:
            n = -n
        return p + n * out, n

    def outer(self, centre, direction, out=0.0):
        """The outermost surface seen coming in from `direction` towards `centre`."""
        h = self.ray(centre + direction * 0.6, -direction)
        return None if h is None else h[0] + direction * out


def grid(name, rows, mat, closed=False):
    """A sheet through rows of points (each row the same length), quads between."""
    bm = bmesh.new()
    vs = [[bm.verts.new(p) for p in r] for r in rows]
    n = len(rows[0])
    for i in range(len(rows) - 1):
        for k in range(n if closed else n - 1):
            k2 = (k + 1) % n
            bm.faces.new((vs[i][k], vs[i][k2], vs[i + 1][k2], vs[i + 1][k]))
    return W.new_object(name, bm, mat)


def solidify(ob, thick, offset=1.0):
    m = ob.modifiers.new("thick", "SOLIDIFY")
    m.thickness = thick
    m.offset = offset
    m.use_even_offset = True
    return ob


def smooth(ob, levels=1):
    m = ob.modifiers.new("smooth", "SUBSURF")
    m.levels = levels
    m.render_levels = levels
    return ob


def wrap_outside(ob, body, offset, above=None):
    """Push anything inside the body out to `offset` above its surface (only the vertices higher
    than `above` metres, if given)."""
    m = ob.modifiers.new("fit", "SHRINKWRAP")
    m.target = body.ob
    m.wrap_method = "NEAREST_SURFACEPOINT"
    m.wrap_mode = "OUTSIDE_SURFACE"
    m.offset = offset
    if above is not None:
        vg = ob.vertex_groups.new(name="fit")
        vg.add([v.index for v in ob.data.vertices if (ob.matrix_world @ v.co).z > above], 1.0, "REPLACE")
        m.vertex_group = "fit"
    return ob


def apply_mods(ob):
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob
    for m in list(ob.modifiers):
        bpy.ops.object.modifier_apply(modifier=m.name)


def frame(origin, normal):
    """A matrix whose z is `normal` and y as close to up as it can be."""
    z = normal.normalized()
    y = (UP - z * UP.dot(z)).normalized()
    x = y.cross(z)
    m = Matrix((x, y, z)).transposed().to_4x4()
    m.translation = origin
    return m


# ---------------------------------------------------------------- Corvin Vale: warden badge

def ranger(body):
    hit = body.front(0.088, 1.37)
    p, n = hit if hit else (gv(0.088, 1.37, 0.14), FWD)
    m = frame(p + n * 0.004, n)
    parts = []
    # a heater shield, rounded at the top corners
    shield = [(-0.040, 0.042), (-0.034, 0.050), (0.034, 0.050), (0.040, 0.042), (0.040, 0.004), (0.028, -0.026), (0.0, -0.052), (-0.028, -0.026), (-0.040, 0.004)]
    back = W.top_profile("badge_back", [(x * 1.25, y * 1.18) for x, y in shield], 0.004, "leather", z=0.0, bevel=0.0015)
    plate = W.top_profile("badge_plate", shield, 0.006, "brass", z=0.005, bevel=0.002, segments=2)
    star = []
    for k in range(10):
        a = math.pi / 2 + k * math.pi / 5
        r = 0.021 if k % 2 == 0 else 0.009
        star.append((r * math.cos(a), r * math.sin(a) + 0.002))
    emb = W.top_profile("badge_star", star, 0.004, "metal_dark", z=0.009, bevel=0.0012)
    rim = W.ring("badge_rim", 0.026, 0.0022, (0, 0.002, 0.0085), "brass", axis="Z", maj=24, minr=5)
    for ob in (back, plate, emb, rim):
        ob.matrix_world = m @ ob.matrix_world
        parts.append(ob)
    return [W.bake(parts, "badge")]


# ---------------------------------------------------------------- Sable Rook: scarf

def neck_ring(body, y, out, n=28, rmax=0.135):
    """A loop round the neck at height `y`, over whatever is outermost there (the hood's collar),
    but no wider than `rmax` (so it passes inside the pauldrons, not round them)."""
    centre = body.a["neck_01"].copy()
    centre.z = y
    centre += FWD * 0.02
    rs = []
    for k in range(n):
        a = 2 * math.pi * k / n
        d = (FWD * math.cos(a) + LEFT * math.sin(a)).normalized()
        p = body.outer(centre, d)
        rs.append((p - centre).length if p is not None else 0.1)
    # smooth it round
    rs = [(rs[k - 1] + 2 * rs[k] + rs[(k + 1) % n]) / 4 for k in range(n)]
    pts = []
    for k in range(n):
        a = 2 * math.pi * k / n
        d = (FWD * math.cos(a) + LEFT * math.sin(a)).normalized()
        pts.append(centre + d * (min(max(rs[k], 0.07), rmax) + out))
    return pts


def scarf_band(name, ring, height, mat):
    height *= 0.7
    rows = [[p + UP * (height / 2) for p in ring], [p - UP * (height / 2) for p in ring]]
    # bulge out a little in the middle of the band
    mid = [p + (p - sum(ring, Vector()) / len(ring)).normalized() * 0.012 for p in ring]
    rows.insert(1, mid)
    ob = grid(name, rows, mat, closed=True)
    return smooth(solidify(ob, 0.009, -1.0))


def huntress(body):
    y0 = body.a["neck_01"].z - 0.045
    a = scarf_band("scarf", neck_ring(body, y0, 0.03), 0.07, "knit")
    b = scarf_band("scarf_b", neck_ring(body, y0 - 0.055, 0.04, rmax=0.15), 0.065, "knit")
    # the knot at the front left, and two tails down the chest
    knot_at = neck_ring(body, y0 - 0.03, 0.075, n=28)[3]
    knot = W.sphere("scarf_knot", 0.026, knot_at - FWD * 0.012, "knit", seg=12, rings=8, scale=(1.1, 0.55, 0.9))
    tails = []
    for k, (dx, length, w) in enumerate(((0.0, 0.46, 0.10), (0.055, 0.36, 0.085))):
        rows = []
        steps = 12
        x0 = knot_at.x + dx
        for s in range(steps + 1):
            t = s / steps
            y = knot_at.z - 0.02 - t * length
            x = x0 + 0.03 * t + 0.012 * math.sin(t * 5.0 + k)
            hit = body.front(x, y, 0.03 + 0.02 * t)
            if hit is None:
                c, n = gv(x, y, 0.15), FWD
            else:
                c, n = hit
            side = UP.cross(n).normalized()
            half = w * (1.0 - 0.25 * t) / 2
            rows.append([c - side * half, c + side * half])
        tail = solidify(grid(f"scarf_tail_{k}", rows, "knit"), 0.012, 0.0)
        tails.append(tail)
    parts = [a, b, knot] + tails
    for ob in tails:
        wrap_outside(ob, body, 0.012)
    return [W.bake([a, b, knot], "scarf")] + [W.bake([t], t.name) for t in tails]


# ---------------------------------------------------------------- Bram Kask: pauldron and apron

def dome(name, centre, radii, mat, tilt, keep=-0.15):
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=20, v_segments=12, radius=1.0)
    gone = [v for v in bm.verts if v.co.z < keep]
    bmesh.ops.delete(bm, geom=gone, context="VERTS")
    bmesh.ops.scale(bm, vec=radii, verts=bm.verts)
    ob = W.new_object(name, bm, mat)
    ob.matrix_world = Matrix.Translation(centre) @ Matrix.Rotation(tilt, 4, "Y")
    return ob


def brawler(body):
    parts = []
    # the pauldron follows the shoulder's own curve: three plates laid over the shoulder and down
    # the upper arm like scales, the top one outermost. In the game each plate copies the bones of
    # the skin under it, so it bends with the shoulder, stays snug and never stands out past the
    # arm's outline.
    joint = body.a["upperarm_l"]
    back = Vector((0, 1, 0))
    plates = [
        # (from x, to x along the arm, distance off the skin, half the angle it wraps, degrees)
        (0.105, 0.215, 0.024, 112),
        (0.18, 0.272, 0.017, 104),
        (0.24, 0.322, 0.011, 96),
    ]
    pauldron = []
    for k, (x0, x1, off, half) in enumerate(plates):
        rows = []
        nu, nv = 8, 16
        for i in range(nu + 1):
            gx = x0 + (x1 - x0) * i / nu
            axis = Vector((gx, joint.y, joint.z))
            row = []
            for j in range(nv + 1):
                a = math.radians(-half + 2 * half * j / nv)
                d = (UP * math.cos(a) + back * math.sin(a)).normalized()
                h = body.ray(axis + d * 0.4, -d)
                p = h[0] if h is not None and (h[0] - axis).length < 0.3 else axis + d * 0.08
                # each plate's lower edge flares a touch, like a rolled rim
                row.append(p + d * (off + 0.004 * (i / nu) ** 2))
            rows.append(row)
        ob = grid(f"pauldron_{k}", rows, "iron")
        solidify(ob, 0.007, 1.0)
        pauldron.append(ob)
        # a rolled rim along the plate's lower edge
        rim = [p + (p - Vector((p.x, joint.y, joint.z))).normalized() * 0.004 for p in rows[-1]]
        pauldron.append(W.tube_path(f"pauldron_rim_{k}", [(p.x, p.y, p.z) for p in rim], 0.0045, "iron", sides=6))
    # a few dark rivets along the top plate's lower edge
    top = plates[0]
    for j in (0.2, 0.5, 0.8):
        a = math.radians(-top[3] + 2 * top[3] * j)
        d = (UP * math.cos(a) + back * math.sin(a)).normalized()
        axis = Vector((top[1] - 0.012, joint.y, joint.z))
        h = body.ray(axis + d * 0.4, -d)
        if h is not None:
            pauldron.append(W.sphere("rivet", 0.0055, h[0] + d * (top[2] + 0.009), "metal_dark", seg=8, rings=5))
    # a strap from the pauldron across the chest to the right hip
    path = []
    for k in range(12):
        t = k / 11
        x = 0.14 - 0.30 * t
        y = 1.44 - 0.40 * t
        hit = body.front(x, y, 0.022)
        path.append(hit[0] if hit else gv(x, y, 0.16))
    strap = []
    for k, p in enumerate(path):
        d = (path[min(k + 1, 11)] - path[max(k - 1, 0)]).normalized()
        hit = body.front(*game(p)[:2], 0.022)
        n = hit[1] if hit else FWD
        side = d.cross(n).normalized()
        strap.append([p - side * 0.026, p + side * 0.026])
    s_ob = solidify(grid("strap", strap, "leather"), 0.007, 0.0)
    wrap_outside(s_ob, body, 0.01)
    buckle = W.box("strap_buckle", (0.05, 0.012, 0.044), (0, 0, 0), "brass", bevel=0.003)
    mid = path[6]
    hit = body.front(*game(mid)[:2], 0.03)
    buckle.matrix_world = frame(hit[0] if hit else mid, hit[1] if hit else FWD) @ Matrix.Rotation(math.radians(90), 4, "X")
    parts.append(W.bake(pauldron, "pauldron"))
    parts.append(W.bake([s_ob, buckle], "strap"))

    # the apron: hugs the belly, then hangs straight from the widest point
    xs = [-0.19 + 0.038 * i for i in range(11)]
    ys = [1.07 - 0.04 * j for j in range(15)]
    rows = []
    hang = None
    for j, y in enumerate(ys):
        zs = []
        for x in xs:
            h = body.front(x, y)
            zs.append(game(h[0])[2] if h else None)
        known = [z for z in zs if z is not None]
        row_max = max(known) if known else 0.12
        z_row = row_max if hang is None else max(hang, row_max)
        if y < 1.0:
            hang = z_row if hang is None else max(hang, z_row)
        row = []
        for i, x in enumerate(xs):
            u = x / 0.19
            z = (z_row if y < 1.0 else (zs[i] if zs[i] is not None else row_max - 0.02)) + 0.03 - 0.035 * u * u
            row.append(gv(x * (1.0 + 0.12 * (1.07 - y)), y, z))
        rows.append(row)
    apron = solidify(grid("apron", rows, "apron"), 0.008, 0.0)
    wrap_outside(apron, body, 0.018)
    # its belt, round the waist
    ring = []
    c = body.a["spine_01"].copy()
    c.z = 1.075
    for k in range(28):
        a = 2 * math.pi * k / 28
        d = (FWD * math.cos(a) + LEFT * math.sin(a)).normalized()
        p = body.outer(c, d, 0.02)
        ring.append(p if p is not None else c + d * 0.16)
    belt = W.tube_path("belt", [(p.x, p.y, p.z) for p in ring], 0.02, "leather", sides=6, closed=True)
    belt.scale = (1.0, 1.0, 0.9)
    bk = W.box("belt_buckle", (0.06, 0.014, 0.05), (0, 0, 0), "brass", bevel=0.004)
    bk.matrix_world = frame(ring[0] + FWD * 0.01, FWD)
    parts.append(W.bake([apron], "apron"))
    parts.append(W.bake([belt, bk], "belt"))
    return parts


# ---------------------------------------------------------------- Ilya Fen: hood with glowing trim

HOOD = [
    # (height, half width, half depth, centre forward)
    (1.83, 0.015, 0.015, -0.07),
    (1.815, 0.07, 0.075, -0.05),
    (1.78, 0.115, 0.125, -0.025),
    (1.72, 0.135, 0.145, -0.01),
    (1.64, 0.14, 0.15, 0.0),
    (1.56, 0.125, 0.135, -0.005),
    (1.50, 0.12, 0.12, -0.01),
    (1.46, 0.155, 0.13, -0.01),
    (1.425, 0.2, 0.15, -0.015),
    (1.395, 0.215, 0.165, -0.02),
]


def boundary_loops(ob):
    """The open edges of a mesh, as ordered loops of vertex positions."""
    me = ob.data
    bm = bmesh.new()
    bm.from_mesh(me)
    edges = [e for e in bm.edges if len(e.link_faces) == 1]
    nbr = {}
    for e in edges:
        a, b = e.verts
        nbr.setdefault(a.index, []).append(b.index)
        nbr.setdefault(b.index, []).append(a.index)
    pos = {v.index: ob.matrix_world @ v.co.copy() for v in bm.verts}
    loops = []
    seen = set()
    for start in nbr:
        if start in seen:
            continue
        loop = [start]
        seen.add(start)
        prev, cur = None, start
        while True:
            nxt = [v for v in nbr[cur] if v != prev and v not in seen]
            if not nxt:
                break
            prev, cur = cur, nxt[0]
            loop.append(cur)
            seen.add(cur)
        loops.append([pos[i] for i in loop])
    bm.free()
    return loops


def striker(body):
    hc = body.a["Head"]
    rows = []
    for (y, rx, rz, cz) in HOOD:
        row = []
        for k in range(24):
            a = 2 * math.pi * k / 24
            row.append(gv(rx * math.sin(a), y, cz + rz * math.cos(a)))
        rows.append(row)
    hood = grid("hood", rows, "hood_cloth", closed=True)
    # close the top
    bm = bmesh.new()
    bm.from_mesh(hood.data)
    tip = bm.verts.new(gv(0.0, 1.84, -0.075))
    bm.verts.ensure_lookup_table()
    for k in range(24):
        bm.faces.new((bm.verts[k], tip, bm.verts[(k + 1) % 24]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(hood.data)
    bm.free()
    # only the head part is fitted (the cowl keeps its own shape over the shoulders: the arms
    # stand out sideways in the reference pose)
    wrap_outside(hood, body, 0.02, above=1.49)
    smooth(hood, 1)
    apply_mods(hood)
    # open the face: an oval at the front
    bm = bmesh.new()
    bm.from_mesh(hood.data)
    face_c = (0.0, hc.z + 0.085)
    gone = []
    for f in bm.faces:
        c = f.calc_center_median()
        gx, gy, gz = game(c)
        if gz > 0.0 and (gx / 0.085) ** 2 + ((gy - face_c[1]) / 0.115) ** 2 < 1.0:
            gone.append(f)
    bmesh.ops.delete(bm, geom=gone, context="FACES")
    # the opening's edge onto the oval itself (no stair steps)
    for v in bm.verts:
        if any(len(e.link_faces) == 1 for e in v.link_edges):
            gx, gy, gz = game(v.co)
            if gy > 1.45:  # the opening's edge (the hem is lower)
                dx, dy = gx / 0.085, (gy - face_c[1]) / 0.115
                r = math.hypot(dx, dy)
                if r > 1e-4:
                    v.co = gv(gx / r, face_c[1] + (gy - face_c[1]) / r, gz)
    # and its depth smoothed along the edge (no steps seen from the side)
    edge = [v for v in bm.verts if any(len(e.link_faces) == 1 for e in v.link_edges) and game(v.co)[1] > 1.45]
    for _ in range(12):
        depth = {}
        for v in edge:
            nb = [e.other_vert(v) for e in v.link_edges if len(e.link_faces) == 1]
            if len(nb) == 2:
                depth[v] = (2 * v.co.y + nb[0].co.y + nb[1].co.y) / 4
        for v, d in depth.items():
            v.co.y = d
    bm.to_mesh(hood.data)
    bm.free()
    loops = boundary_loops(hood)
    # the trim: a thin line along the hood's front edge only, over the brow and down the sides to
    # the jaw (an open arc, not a ring round the face; none at the hem)
    trims = []
    face = [l for l in loops if len(l) >= 8 and sum(game(p)[1] for p in l) / len(l) > 1.5]
    if face:
        loop = face[0]
        cut = face_c[1] - 0.06
        up = [game(p)[1] > cut for p in loop]
        n = len(loop)
        start = next((i for i in range(n) if up[i] and not up[i - 1]), 0)
        arc = []
        for s in range(n):
            i = (start + s) % n
            if not up[i]:
                break
            arc.append(loop[i])
        if len(arc) >= 4:
            trims.append(W.tube_path("hood_trim", [(p.x, p.y, p.z) for p in arc], 0.0032, "glow", sides=5))
    solidify(hood, 0.008, -1.0)
    return [W.bake([hood], "hood")] + [W.bake([t], t.name) for t in trims]


BUILDERS = {"ranger": ranger, "huntress": huntress, "brawler": brawler, "striker": striker}


# ---------------------------------------------------------------- texture quarters

def remap_uvs(objects, off):
    for ob in objects:
        uv = ob.data.uv_layers.active
        if uv is None:
            continue
        for d in uv.data:
            d.uv = (d.uv.x * 0.5 + off[0], d.uv.y * 0.5 + off[1])


def compose(textures, out_path):
    """Four 512 bakes into one 1024 image, a quarter each (v up, as Blender stores them)."""
    size = TEX * 2
    img = bpy.data.images.new("accessories", size, size, alpha=False)
    px = [0.5] * (size * size * 4)
    for name, path in textures.items():
        if not os.path.exists(path):
            continue
        src = bpy.data.images.load(path)
        sp = list(src.pixels)
        ox, oy = int(QUARTER[name][0] * size), int(QUARTER[name][1] * size)
        for y in range(TEX):
            row = (oy + y) * size + ox
            px[row * 4:(row + TEX) * 4] = sp[y * TEX * 4:(y + 1) * TEX * 4]
        bpy.data.images.remove(src)
    img.pixels = px
    img.filepath_raw = out_path
    img.file_format = "PNG"
    img.save()


def preview(parts, body, title, out_path, size=(1600, 1000)):
    """Four views of the accessory on the body: front, front three-quarter, side, back
    three-quarter, framed on the upper body."""
    scene = bpy.context.scene
    W._use_engine(scene, ["BLENDER_EEVEE", "BLENDER_EEVEE_NEXT"])
    scene.render.resolution_x = size[0] // 2
    scene.render.resolution_y = size[1] // 2
    scene.render.image_settings.file_format = "PNG"
    try:
        scene.view_settings.view_transform = "Standard"
    except TypeError:
        pass
    world = bpy.data.worlds.get("preview") or bpy.data.worlds.new("preview")
    world.use_nodes = True
    world.node_tree.nodes.get("Background").inputs[0].default_value = (0.16, 0.16, 0.18, 1.0)
    scene.world = world
    skin = bpy.data.materials.new("ref_grey")
    skin.diffuse_color = (0.55, 0.52, 0.5, 1.0)
    skin.use_nodes = True
    skin.node_tree.nodes.get("Principled BSDF").inputs["Base Color"].default_value = (0.05, 0.055, 0.07, 1.0)
    body.ob.data.materials.clear()
    body.ob.data.materials.append(skin)
    lo, hi = W._bounds(parts)
    lo = Vector((min(lo.x, -0.3), min(lo.y, -0.25), min(lo.z, 1.05)))
    hi = Vector((max(hi.x, 0.3), max(hi.y, 0.25), max(hi.z, 1.9)))
    centre = (lo + hi) / 2
    radius = (hi - lo).length / 2
    lights = [
        W._light("key", "AREA", centre + Vector((1.2, -1.6, 1.4)), 120, centre, size=1.0),
        W._light("fill", "AREA", centre + Vector((-1.6, -0.8, 0.4)), 45, centre, size=1.5, color=(0.75, 0.85, 1.0)),
        W._light("rim", "AREA", centre + Vector((0.0, 1.8, 1.0)), 120, centre, size=1.0, color=(1.0, 0.85, 0.7)),
    ]
    dist = radius / math.tan(math.radians(13.5)) * 0.62
    tiles = []
    for k, az in enumerate((-90, -50, 0, 130)):
        a = math.radians(az)
        loc = centre + Vector((math.cos(a), math.sin(a), 0.12)) * dist
        cam = W._camera(f"view{k}", loc, centre)
        path = out_path.replace(".png", f"_{k}.png")
        scene.render.filepath = path
        bpy.ops.render.render(write_still=True)
        tiles.append(path)
        bpy.data.objects.remove(cam, do_unlink=True)
    for ob in lights:
        bpy.data.objects.remove(ob, do_unlink=True)
    W._stitch(tiles, out_path, cols=2)


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    renders = "--no-renders" not in argv
    names = [a for a in argv if not a.startswith("--")] or NAMES
    with open(os.path.join(REF, "anchors.json")) as f:
        anchors = json.load(f)
    os.makedirs(SHOTS, exist_ok=True)
    # (each fighter's own bake is kept with the references, so one can be rebuilt alone)
    textures = {n: os.path.join(REF, f"acc_{n}_tex.png") for n in NAMES}
    for name in names:
        W.clear_scene()
        for m in list(bpy.data.materials):
            bpy.data.materials.remove(m)
        body = Body(name, anchors[name])
        parts = BUILDERS[name](body)
        for ob in parts:
            apply_mods(ob)
        T.bake_texture(parts, textures[name], size=TEX, samples=32)
        tris = sum(W.triangles(o) for o in parts)
        if renders:
            preview(parts, body, f"{name} accessory", os.path.join(SHOTS, f"{name}.png"))
        remap_uvs(parts, QUARTER[name])
        W.export_glb(parts, os.path.join(OUT, f"acc_{name}.glb"))
        print(f"[accessories] {name}: {tris} triangles, pieces {[o.name for o in parts]}")
    compose(textures, os.path.join(OUT, "accessories.png"))


if __name__ == "__main__":
    main()
