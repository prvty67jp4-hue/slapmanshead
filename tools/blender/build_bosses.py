"""Build the Ashen Vault's bosses: original, stylised, segmented models the game poses in code.

    blender --background --factory-startup --python app/tools/blender/build_bosses.py -- [name ...] [--no-renders]

Bosses: warden (the Warden of Wax), tally and seal (the Twin Assessors), ledger (the Chained
Ledger), magistrate (the Ashen Magistrate), regalia (the Ashblood Regent's crown and mantle),
props (a chain link and a floor anchor, drawn many times).

Writes:
    app/assets/bosses/<name>.glb           the model (one node per moving part, its origin at the joint)
    app/assets/bosses/textures/<name>.png  its baked, hand-painted-look texture (1024 x 1024)
    app/shots/bosses/<name>.png            a four-view turntable for review

Axes and units as weapons_lib.py: Blender +X right, +Y forward, +Z up, metres; the game reads x
right, y up, z forward. Every part is its own object named after its joint (the game turns it
about its origin). Objects named `fx_<part>_<kind>_<n>` are markers, not drawn: where flames,
eyes and the like attach to that part.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bmesh  # noqa: E402
import bpy  # noqa: E402
from mathutils import Matrix, Quaternion, Vector  # noqa: E402

import weapons_lib as W  # noqa: E402
import weapons_tex as T  # noqa: E402

APP = os.path.normpath(os.path.join(HERE, "..", ".."))
OUT = os.path.join(APP, "assets", "bosses")
OUT_TEX = os.path.join(OUT, "textures")
OUT_SHOTS = os.path.join(APP, "shots", "bosses")
TEX = 1024

# ---------------------------------------------------------------- materials

W.PALETTE.update({
    "wax": ((0.86, 0.74, 0.52), 0.0, 0.55),
    "wax_old": ((0.62, 0.48, 0.26), 0.0, 0.6),
    "iron": ((0.10, 0.10, 0.11), 0.85, 0.45),
    "gold": ((0.85, 0.60, 0.18), 1.0, 0.3),
    "cloth_gold": ((0.62, 0.42, 0.10), 0.3, 0.55),
    "cloth_ash": ((0.26, 0.26, 0.28), 0.0, 0.85),
    "cloth_dark": ((0.04, 0.04, 0.045), 0.0, 0.85),
    "cloth_red": ((0.38, 0.03, 0.03), 0.0, 0.75),
    "cloth_white": ((0.86, 0.85, 0.80), 0.0, 0.8),
    "wig": ((0.74, 0.74, 0.72), 0.0, 0.85),
    "skin_pale": ((0.70, 0.60, 0.50), 0.0, 0.7),
    "skin_grey": ((0.44, 0.44, 0.46), 0.0, 0.75),
    "leather_book": ((0.26, 0.06, 0.04), 0.0, 0.7),
    "parchment": ((0.82, 0.72, 0.54), 0.0, 0.85),
    "ink": ((0.03, 0.02, 0.07), 0.2, 0.2),
    "wood_boss": ((0.22, 0.09, 0.03), 0.0, 0.65),
    "glow_ember": ((1.0, 0.45, 0.1), 0.0, 0.5),
    "glow_violet": ((0.62, 0.3, 1.0), 0.0, 0.5),
    "glow_quill": ((0.55, 0.9, 1.0), 0.0, 0.5),
    "glow_glass": ((0.8, 0.95, 1.0), 0.0, 0.5),
    "glow_blood": ((1.0, 0.08, 0.12), 0.0, 0.5),
})

# (recipe, dark, light, edge colour, edge strength, recess darkening)
PAINT = {
    "wax": ("wax", (0.6, 0.42, 0.2), (0.9, 0.76, 0.5), (1.0, 0.92, 0.74), 0.5, 0.45),
    "wax_old": ("wax", (0.42, 0.30, 0.14), (0.70, 0.56, 0.32), (0.90, 0.80, 0.58), 0.45, 0.5),
    "iron": ("metal", (0.11, 0.115, 0.13), (0.24, 0.25, 0.28), (0.72, 0.7, 0.64), 0.85, 0.55),
    "gold": ("metal", (0.55, 0.34, 0.07), (0.92, 0.68, 0.24), (1.0, 0.92, 0.62), 0.8, 0.5),
    "cloth_gold": ("damask", (0.42, 0.26, 0.05), (0.78, 0.55, 0.14), (1.0, 0.82, 0.38), 0.5, 0.6),
    "cloth_ash": ("cloth", (0.2, 0.2, 0.21), (0.36, 0.36, 0.38), (0.55, 0.54, 0.52), 0.35, 0.5),
    "cloth_dark": ("cloth", (0.02, 0.02, 0.025), (0.06, 0.06, 0.07), (0.16, 0.15, 0.16), 0.35, 0.5),
    "cloth_red": ("cloth", (0.22, 0.015, 0.015), (0.48, 0.05, 0.04), (0.70, 0.18, 0.12), 0.35, 0.6),
    "cloth_white": ("cloth", (0.66, 0.64, 0.58), (0.92, 0.91, 0.86), (1.0, 1.0, 0.97), 0.3, 0.45),
    "wig": ("curls", (0.50, 0.50, 0.48), (0.86, 0.86, 0.84), (0.95, 0.95, 0.93), 0.3, 0.6),
    "skin_pale": ("skin", (0.60, 0.46, 0.36), (0.80, 0.68, 0.58), (0.88, 0.78, 0.68), 0.25, 0.5),
    "skin_grey": ("skin", (0.28, 0.28, 0.30), (0.48, 0.48, 0.50), (0.62, 0.62, 0.63), 0.25, 0.55),
    "leather_book": ("leather", (0.08, 0.012, 0.01), (0.2, 0.035, 0.022), (0.34, 0.1, 0.05), 0.35, 0.6),
    "parchment": ("parchment", (0.66, 0.56, 0.38), (0.92, 0.85, 0.68), (0.98, 0.94, 0.80), 0.3, 0.5),
    "ink": ("metal", (0.012, 0.008, 0.03), (0.05, 0.035, 0.1), (0.14, 0.1, 0.24), 0.5, 0.3),
    "wood_boss": ("wood", (0.10, 0.035, 0.012), (0.26, 0.10, 0.035), (0.40, 0.20, 0.08), 0.5, 0.6),
}


def paint_boss(mat, name):
    """The painted look of one boss material (emission, for the colour bake). Scaled for things
    metres big (the weapons' recipes are for centimetres)."""
    g = T.G(mat)
    if name.startswith("glow"):
        col, _, _ = W.PALETTE[name]
        g.emit(g.ramp(g.noise(g.coords(), 4.0), [(0.3, col), (0.8, tuple(min(1.0, c * 1.15 + 0.05) for c in col))]))
        return
    recipe, c0, c1, ce, edge_k, ao_k = PAINT.get(name, PAINT["iron"])
    edge = g.edge_mask(radius=0.02, gain=5.0)
    occ = g.ao(distance=0.25)
    if recipe == "wax":
        # soft, mottled, with runs streaming down
        n1 = g.noise(g.coords(), 2.2, 3.0)
        runs = g.wave(g.coords((1.0, 1.0, 0.25)), 5.0, 6.0, "X", detail=2.0)
        f = g.math("ADD", g.math("MULTIPLY", n1, 0.6), g.math("MULTIPLY", runs, 0.4))
        col = g.ramp(f, [(0.25, c0), (0.75, c1)])
        col = g.mix(g.math("MULTIPLY", edge, edge_k), col, ce)
    elif recipe == "metal":
        n1 = g.noise(g.coords(), 6.0, 5.0)
        col = g.ramp(n1, [(0.3, c0), (0.75, c1)])
        scr = g.math("GREATER_THAN", g.noise(g.coords((3.0, 3.0, 0.4)), 4.0, 8.0, 0.7), 0.7)
        col = g.mix(g.math("MULTIPLY", scr, 0.25), col, ce)
        col = g.mix(g.math("MULTIPLY", edge, edge_k), col, ce)
    elif recipe in ("cloth", "damask"):
        weave = g.wave(g.coords((1, 1, 1)), 40.0, 1.0, "DIAGONAL", detail=1.0)
        n1 = g.noise(g.coords(), 3.0, 4.0)
        col = g.ramp(g.math("ADD", g.math("MULTIPLY", n1, 0.8), g.math("MULTIPLY", weave, 0.2)), [(0.3, c0), (0.75, c1)])
        if recipe == "damask":
            # a fine brocade lattice, two sets of thin diagonal threads
            a = g.wave(g.coords((1, 1, 1)), 7.0, 0.0, "DIAGONAL", detail=0.0)
            b = g.wave(g.coords((-1, 1, 1)), 7.0, 0.0, "DIAGONAL", detail=0.0)
            lat = g.math("MAXIMUM", g.math("GREATER_THAN", a, 0.9), g.math("GREATER_THAN", b, 0.9))
            col = g.mix(g.math("MULTIPLY", lat, 0.35), col, ce)
        col = g.mix(g.math("MULTIPLY", edge, edge_k), col, ce)
    elif recipe == "curls":
        curls = g.wave(g.coords((1, 1, 1)), 9.0, 7.0, "DIAGONAL", kind="RINGS", detail=2.0)
        col = g.ramp(curls, [(0.2, c0), (0.8, c1)])
    elif recipe == "skin":
        n1 = g.noise(g.coords(), 5.0, 3.0)
        col = g.ramp(n1, [(0.3, c0), (0.75, c1)])
        col = g.mix(g.math("MULTIPLY", edge, edge_k), col, ce)
    elif recipe == "leather":
        n1 = g.noise(g.coords(), 8.0, 6.0, 0.7)
        col = g.ramp(n1, [(0.3, c0), (0.7, c1)])
        crease = g.math("LESS_THAN", g.voronoi(g.coords(), 5.0, "DISTANCE_TO_EDGE"), 0.04)
        col = g.mix(g.math("MULTIPLY", crease, 0.6), col, (0.05, 0.01, 0.01))
        col = g.mix(g.math("MULTIPLY", edge, edge_k), col, ce)
    elif recipe == "parchment":
        n1 = g.noise(g.coords(), 3.0, 4.0)
        col = g.ramp(n1, [(0.3, c0), (0.75, c1)])
        # lines of ink script
        lines = g.wave(g.coords((1, 1, 1)), 28.0, 2.5, "Z", detail=3.0)
        words = g.math("GREATER_THAN", g.noise(g.coords((1, 1, 6)), 12.0, 2.0), 0.48)
        ink = g.math("MULTIPLY", g.math("LESS_THAN", lines, 0.12), words)
        col = g.mix(g.math("MULTIPLY", ink, 0.8), col, (0.05, 0.03, 0.06))
    elif recipe == "wood":
        grain = g.wave(g.coords((1.0, 1.0, 0.1)), 14.0, 6.0, "X", detail=4.0)
        col = g.ramp(grain, [(0.0, c0), (1.0, c1)])
        col = g.mix(g.math("MULTIPLY", edge, edge_k), col, ce)
    else:
        col = g.ramp(g.noise(g.coords(), 4.0), [(0.3, c0), (0.7, c1)])
    shade = g.mix(occ, (1.0 - ao_k, 1.0 - ao_k, 1.0 - ao_k), (1.0, 1.0, 1.0))
    col = g.mix(1.0, col, shade, blend="MULTIPLY")
    g.emit(col)


def bake_texture(objects, out_path, size=TEX, samples=24):
    """Paint and bake every part into one image (all the parts share one UV square)."""
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = samples
    scene.cycles.device = "CPU"
    img = bpy.data.images.new("bake", size, size, alpha=False)
    img.colorspace_settings.name = "sRGB"
    mats = []
    for ob in objects:
        for slot in ob.material_slots:
            if slot.material and slot.material not in mats:
                mats.append(slot.material)
    for m in mats:
        paint_boss(m, m.name.split(".")[0])
        node = m.node_tree.nodes.new("ShaderNodeTexImage")
        node.image = img
        m.node_tree.nodes.active = node
    T.unwrap(objects)
    T._select(objects)
    scene.render.bake.margin = 4
    bpy.ops.object.bake(type="EMIT", margin=4, use_clear=True)
    img.filepath_raw = out_path
    img.file_format = "PNG"
    img.save()
    for ob in objects:
        for slot in ob.material_slots:
            if slot.material is not None:
                slot.material = T.display_material(slot.material.name.split(".")[0], img)
    return img


# ---------------------------------------------------------------- shapes

def hsh(k):
    x = math.sin(k * 12.9898 + 78.233) * 43758.5453
    return x - math.floor(x)


def lathe(name, prof, mat, segs=24, wob=None, loc=(0.0, 0.0, 0.0), sx=1.0, sy=1.0, bevel=0.0, a0=0.0):
    """A surface of revolution about Z: `prof` is (radius, z) from the bottom up; `wob(a, z)`
    adds a share of the radius at angle a (0 = +X, pi/2 = +Y, forward)."""
    bm = bmesh.new()
    rings = []
    for (r, z) in prof:
        ring = []
        for i in range(segs):
            a = a0 + 2 * math.pi * i / segs
            rr = max(r, 0.002) * (1.0 + (wob(a, z) if wob else 0.0))
            ring.append(bm.verts.new((loc[0] + rr * math.cos(a) * sx, loc[1] + rr * math.sin(a) * sy, loc[2] + z)))
        rings.append(ring)
    for k in range(len(rings) - 1):
        a, b = rings[k], rings[k + 1]
        for i in range(segs):
            j = (i + 1) % segs
            bm.faces.new((a[i], a[j], b[j], b[i]))
    bm.faces.new(list(reversed(rings[0])))
    bm.faces.new(rings[-1])
    return W.new_object(name, bm, mat, bevel)


def arc_wall(name, prof, mat, thick, open_deg=70.0, segs=24, sy=0.95):
    """A wall of revolution about Z (a stand-up collar) open at the front by `open_deg`,
    `thick` thick: `prof` is (radius, z) from the bottom up."""
    bm = bmesh.new()
    span = 2 * math.pi - math.radians(open_deg)
    a0 = math.pi / 2 + math.radians(open_deg) / 2
    rings = []
    for (r, z) in prof:
        rings.append([bm.verts.new((r * math.cos(a0 + span * i / segs), r * math.sin(a0 + span * i / segs) * sy, z)) for i in range(segs + 1)])
    for k in range(len(rings) - 1):
        for i in range(segs):
            bm.faces.new((rings[k][i], rings[k][i + 1], rings[k + 1][i + 1], rings[k + 1][i]))
    ob = W.new_object(name, bm, mat)
    mod = ob.modifiers.new("thick", "SOLIDIFY")
    mod.thickness = thick
    return ob


def ttube(name, pts, radii, mat, sides=10, flat=1.0):
    """A tube along points with a radius at each (tapering limbs, drips, wig curls). `flat`
    squashes it across its second axis (ribbons)."""
    pts = [Vector(p) for p in pts]
    n = len(pts)
    bm = bmesh.new()
    rings = []
    prev = None
    for i, p in enumerate(pts):
        if i == 0:
            t = (pts[1] - pts[0]).normalized()
        elif i == n - 1:
            t = (pts[-1] - pts[-2]).normalized()
        else:
            t = ((pts[i + 1] - pts[i]).normalized() + (pts[i] - pts[i - 1]).normalized()).normalized()
        ref = prev if prev is not None else (Vector((1, 0, 0)) if abs(t.x) < 0.9 else Vector((0, 0, 1)))
        side = (ref - t * ref.dot(t)).normalized()
        up = t.cross(side).normalized()
        prev = side
        r = radii[i]
        rings.append([bm.verts.new(p + (side * math.cos(2 * math.pi * k / sides) * r + up * math.sin(2 * math.pi * k / sides) * r * flat)) for k in range(sides)])
    for i in range(n - 1):
        a, b = rings[i], rings[i + 1]
        for k in range(sides):
            bm.faces.new((a[k], a[(k + 1) % sides], b[(k + 1) % sides], b[k]))
    bm.faces.new(list(reversed(rings[0])))
    bm.faces.new(rings[-1])
    return W.new_object(name, bm, mat)


def limb(name, a, b, r0, r1, mat, sides=12):
    return ttube(name, [a, b], [r0, r1], mat, sides)


def ball(name, r, loc, mat, scale=(1, 1, 1), seg=14, rings=9):
    return W.sphere(name, r, loc, mat, seg=seg, rings=rings, scale=scale)


def recenter(ob):
    """Move an object's origin to the middle of its mesh (so it turns in place)."""
    me = ob.data
    if len(me.vertices) == 0:
        return ob
    c = Vector((0.0, 0.0, 0.0))
    for v in me.vertices:
        c += v.co
    c /= len(me.vertices)
    me.transform(Matrix.Translation(-c))
    ob.location = ob.location + c
    return ob


def rot(ob, rx=0.0, ry=0.0, rz=0.0):
    """Turn a piece about its own middle (degrees)."""
    recenter(ob)
    ob.rotation_euler = (math.radians(rx), math.radians(ry), math.radians(rz))
    return ob


def zcyl(name, r, z0, z1, mat, x=0.0, y=0.0, r1=None, verts=16, bevel=0.0):
    """A cylinder (or cone: r at z0, r1 at z1) standing up Z at (x, y)."""
    return W.cyl(name, r, z0, z1, mat, x=y, z=x, r1=r1, verts=verts, bevel=bevel, axis="Z")


def xcyl(name, r, x0, x1, mat, y=0.0, z=0.0, r1=None, verts=16, bevel=0.0):
    return W.cyl(name, r, x0, x1, mat, x=y, z=z, r1=r1, verts=verts, bevel=bevel, axis="X")


def ycyl(name, r, y0, y1, mat, x=0.0, z=0.0, r1=None, verts=16, bevel=0.0):
    return W.cyl(name, r, y0, y1, mat, x=x, z=z, r1=r1, verts=verts, bevel=bevel, axis="Y")


def panel(name, pts_xz, thick, mat, y=0.0, bevel=0.01):
    """A flat shape in the XZ plane (a tabard, a jabot) `thick` deep, centred on y."""
    bm = bmesh.new()
    vs = [bm.verts.new((x, y - thick / 2, z)) for (x, z) in pts_xz]
    face = bm.faces.new(vs)
    ext = bmesh.ops.extrude_face_region(bm, geom=[face])
    moved = [g for g in ext["geom"] if isinstance(g, bmesh.types.BMVert)]
    bmesh.ops.translate(bm, verts=moved, vec=(0, thick, 0))
    return W.new_object(name, bm, mat, bevel)


def sheet(name, x0, x1, y0, y1, z, curl, mat, thick=0.025, nx=7, ny=4, droop=0.0):
    """A thin page from x0 to x1 (along X), y0 to y1, curling up by `curl` at its far end."""
    bm = bmesh.new()
    grid = []
    for i in range(nx + 1):
        u = i / nx
        row = []
        for j in range(ny + 1):
            v = j / ny
            x = x0 + (x1 - x0) * u
            yy = y0 + (y1 - y0) * v
            zz = z + curl * u * u - droop * u * (1 - abs(2 * v - 1))
            row.append(bm.verts.new((x, yy, zz)))
        grid.append(row)
    for i in range(nx):
        for j in range(ny):
            bm.faces.new((grid[i][j], grid[i + 1][j], grid[i + 1][j + 1], grid[i][j + 1]))
    ob = W.new_object(name, bm, mat)
    mod = ob.modifiers.new("thick", "SOLIDIFY")
    mod.thickness = thick
    return ob


def drip(name, top, length, r, mat, out=(0.0, 0.0), seed=0):
    """A run of molten wax down from `top`: it thins, then gathers in a drop."""
    n = 6
    pts, radii = [], []
    for k in range(n + 1):
        t = k / n
        w = math.sin(seed * 1.7 + t * 5.0) * 0.025
        pts.append((top[0] + out[0] * t + w, top[1] + out[1] * t + w * 0.5, top[2] - length * t))
        radii.append(r * (1.0 - 0.35 * t))
    objs = [ttube(name, pts, radii, mat, sides=8)]
    end = pts[-1]
    objs.append(ball(name + "_drop", r * 1.15, (end[0], end[1], end[2] - r * 0.3), mat, scale=(1, 1, 1.6), seg=10, rings=7))
    return objs


def prof_r(prof, z):
    """The radius of a lathe profile at height z (0 outside it)."""
    for (r0, z0), (r1, z1) in zip(prof, prof[1:]):
        if min(z0, z1) <= z <= max(z0, z1) and z1 != z0:
            return r0 + (r1 - r0) * (z - z0) / (z1 - z0)
    return 0.0


def drip_on(name, radius_at, a, z0, length, r, mat, sy=1.0, c=(0.0, 0.0), seed=0):
    """A run of wax hugging a round surface (radius_at(z) about the point c) from z0 down: thick
    where it starts, thinning, gathering in a drop at its end."""
    n = 8
    pts, radii = [], []
    for k in range(n + 1):
        t = k / n
        z = z0 - length * t
        rr = max(radius_at(z), 0.05) + r * 0.45
        aa = a + math.sin(seed * 1.3 + t * 4.0) * 0.04
        pts.append((c[0] + rr * math.cos(aa), c[1] + rr * math.sin(aa) * sy, z))
        radii.append(r * (1.25 - 0.55 * t))
    objs = [ttube(name, pts, radii, mat, sides=8)]
    e = pts[-1]
    objs.append(ball(name + "_drop", r * 1.05, (e[0], e[1], e[2] - r * 0.35), mat, scale=(1, 1, 1.55), seg=10, rings=7))
    return objs


def chain(name, pts, mat, link=0.11, r=0.022, step=None):
    """Iron links along a path, each turned a quarter from the last."""
    objs = []
    step = step or link * 0.82
    pts = [Vector(p) for p in pts]
    k = 0
    for i in range(len(pts) - 1):
        a, b = pts[i], pts[i + 1]
        seg = (b - a).length
        n = max(1, int(seg / step))
        d = (b - a).normalized()
        for j in range(n):
            c = a + d * (seg * (j + 0.5) / n)
            ob = W.ring(f"{name}{k}", link * 0.5, r, (0, 0, 0), mat, axis="Y", maj=8, minr=4)
            ob.scale = (1.0, 1.0, 1.55)
            # the link's long side (its Z) along d, every other one turned a quarter
            q = Vector((0, 0, 1)).rotation_difference(d)
            ob.rotation_mode = "QUATERNION"
            ob.rotation_quaternion = q @ Quaternion((0, 0, 1), math.pi / 2 * (k % 2))
            ob.location = c
            objs.append(ob)
            k += 1
    return objs


def marker(part_name, kind, n, loc):
    """A point the game attaches something to (not drawn)."""
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, segments=3, radius1=0.01, radius2=0.0, depth=0.01)
    bmesh.ops.translate(bm, verts=bm.verts, vec=loc)
    me = bpy.data.meshes.new(f"fx_{part_name}_{kind}_{n}")
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(f"fx_{part_name}_{kind}_{n}", me)
    bpy.context.scene.collection.objects.link(ob)
    ob.data.materials.append(W.material("glow_ember"))
    return ob


def part(objs, name, pivot):
    bpy.context.view_layer.update()
    flat = []
    for o in objs:
        if isinstance(o, (list, tuple)):
            flat.extend(o)
        else:
            flat.append(o)
    return W.bake(flat, name, pivot=pivot)


def robe_wob(n, amp, top):
    return lambda a, z: amp * math.sin(a * n + 0.4) * max(0.0, 1.0 - z / top)


# ---------------------------------------------------------------- the Warden of Wax

def warden():
    """A towering knight of melted candle wax: plate armour under runs of wax, a great helm with
    a crown of burning wicks, ember eyes in the visor, an ember heart showing through a crack in
    the breastplate, a ragged crimson tabard, wax pooled round its feet, and a halberd made from
    an iron candle-stand (its head a dish of candles over the axe)."""
    P = {}
    fx = []
    # --- pelvis
    o = []
    o.append(lathe("hips", [(0.6, 1.72), (0.74, 1.95), (0.8, 2.25)], "iron", segs=18, sy=0.78, bevel=0.02))
    belt = W.ring("belt", 0.8, 0.075, (0, 0, 2.17), "wax_old", axis="Z", maj=20, minr=6)
    belt.scale = (1.0, 0.8, 1.0)
    o.append(belt)
    for k, a in enumerate((-80, -50, 50, 80, 130, 180, 230)):
        ra = math.radians(a + 90)
        cx, cy = 0.82 * math.cos(ra), 0.66 * math.sin(ra)
        for j in range(2):
            b = W.box(f"tasset{k}_{j}", (0.46, 0.08, 0.36), (cx * (1 + j * 0.06), cy * (1 + j * 0.06), 1.95 - j * 0.3), "iron", bevel=0.02)
            recenter(b)
            b.rotation_euler = (math.radians(14 + j * 6), 0.0, ra - math.pi / 2)
            o.append(b)
    # the crimson tabard, ragged at the hem
    rag = [(-0.3, 2.15), (0.3, 2.15), (0.32, 0.95), (0.2, 1.08), (0.1, 0.9), (-0.02, 1.05), (-0.14, 0.88), (-0.24, 1.02), (-0.32, 0.92)]
    o.append(panel("tabard", rag, 0.04, "cloth_red", y=0.7, bevel=0.012))
    o.append(panel("tabard_back", rag, 0.04, "cloth_red", y=-0.62, bevel=0.012))
    hips = [(0.6, 1.72), (0.74, 1.95), (0.8, 2.25)]
    for k in range(5):
        a = k * 1.3 + 0.4
        o += drip_on(f"pdrip{k}", lambda z: prof_r(hips, z), a, 2.16, 0.25 + hsh(k) * 0.25, 0.055, "wax", sy=0.78, seed=k)
    P["pelvis"] = part(o, "pelvis", (0, 0, 2.05))
    # --- torso
    o = []
    chest = [(0.66, 2.2), (0.82, 2.5), (0.95, 2.95), (0.98, 3.2), (0.78, 3.45), (0.42, 3.6)]
    o.append(lathe("chest", chest, "iron", segs=22, sy=0.74, bevel=0.02))
    o.append(ball("breast", 0.95, (0, 0.14, 2.95), "iron", scale=(0.98, 0.62, 0.62), seg=20, rings=12))
    o.append(ball("plackart", 0.62, (0, 0.24, 2.42), "iron", scale=(1.0, 0.62, 0.42), seg=18, rings=10))
    # the ember heart in a crack through the breastplate
    for k, (x, z, a, l) in enumerate([(0.0, 3.0, 8, 0.36), (0.1, 2.78, -28, 0.22), (-0.12, 3.2, 30, 0.2), (0.16, 3.12, -60, 0.14)]):
        o.append(rot(W.box(f"crack{k}", (0.05, 0.12, l), (x, 0.74, z), "glow_ember", bevel=0.0), 0, a, 0))
    o.append(ball("heart", 0.13, (0.0, 0.68, 2.98), "glow_ember", seg=10, rings=7))
    o.append(W.ring("gorget", 0.48, 0.1, (0, 0, 3.47), "iron", axis="Z", maj=18, minr=6))
    # wax mantle poured over the shoulders, running down
    o.append(lathe("mantle", [(1.0, 3.18), (0.92, 3.36), (0.72, 3.52), (0.48, 3.62)], "wax", segs=24, sy=0.8,
                   wob=lambda a, z: (0.08 * math.sin(a * 7 + 1.3) + 0.05 * math.sin(a * 13)) * (1 if z < 3.3 else 0.3)))
    for k in range(8):
        a = k * (2 * math.pi / 8) + 0.2 + hsh(k + 3) * 0.3
        # (the breastplate stands out in front: runs there start on it)
        front = max(0.0, math.sin(a))
        L = 0.18 + hsh(k) * 0.42
        o += drip_on(f"tdrip{k}", lambda z, f=front: prof_r(chest, z) + 0.06 * f, a, 3.22, L, 0.085 + hsh(k + 9) * 0.035, "wax", sy=0.76, seed=k)
    for k, x in enumerate((-0.42, -0.21, 0.21, 0.42)):
        o.append(ball(f"rivet{k}", 0.045, (x, 0.72, 2.5 + abs(x) * 0.2), "gold", seg=8, rings=5))
    fx.append(marker("torso", "heart", 0, (0.0, 0.8, 2.98)))
    P["torso"] = part(o, "torso", (0, 0, 2.25))
    # --- head: the great helm and its crown of wicks
    o = []
    o.append(zcyl("neck", 0.3, 3.45, 3.72, "iron", verts=16))
    helm = [(0.36, 3.62), (0.43, 3.78), (0.45, 4.12), (0.43, 4.34), (0.36, 4.44)]
    o.append(lathe("helm", helm, "iron", segs=22, sy=0.96, bevel=0.015))
    o.append(ball("dome", 0.37, (0, 0, 4.4), "iron", scale=(1.0, 0.96, 0.4), seg=20, rings=10))
    o.append(W.box("crest", (0.07, 0.86, 0.1), (0, 0, 4.5), "iron", bevel=0.02))
    o.append(W.box("visor", (0.56, 0.07, 0.08), (0, 0.43, 4.08), "ink", bevel=0.01))
    for k in range(5):
        o.append(W.box(f"breath{k}", (0.03, 0.05, 0.16), (-0.16 + k * 0.08, 0.44, 3.86), "ink", bevel=0.0))
    for s in (-1, 1):
        o.append(ball(f"eye{s}", 0.055, (0.13 * s, 0.41, 4.08), "glow_ember", scale=(1.5, 0.6, 0.8), seg=10, rings=6))
        fx.append(marker("head", "eye", 0 if s < 0 else 1, (0.13 * s, 0.46, 4.08)))
    o.append(W.ring("crown", 0.4, 0.05, (0, 0, 4.46), "gold", axis="Z", maj=24, minr=6))
    heights = [0.2, 0.32, 0.16, 0.28, 0.22, 0.34, 0.18, 0.26]
    for k in range(8):
        a = 2 * math.pi * k / 8 + 0.2
        x, y = 0.38 * math.cos(a), 0.37 * math.sin(a)
        r = 0.065 + hsh(k + 1) * 0.025
        top = 4.44 + heights[k]
        o.append(zcyl(f"candle{k}", r, 4.4, top, "wax", x=x, y=y, r1=r * 0.92, verts=10))
        o.append(zcyl(f"wick{k}", 0.012, top - 0.02, top + 0.07, "ink", x=x, y=y, verts=5))
        o += drip(f"cdrip{k}", (x * 1.05, y * 1.05, top - 0.04), 0.25 + heights[k] * 0.8, 0.03, "wax", (x * 0.12, y * 0.12), seed=k + 20)
        fx.append(marker("head", "flame", k, (x, y, top + 0.16)))
    o.append(zcyl("great_candle", 0.15, 4.42, 4.98, "wax", r1=0.13, verts=14))
    o.append(zcyl("great_wick", 0.02, 4.96, 5.08, "ink", verts=6))
    fx.append(marker("head", "flame", 8, (0, 0, 5.24)))
    for k in range(6):
        a = k * 1.05 + 0.5
        if math.cos(a - math.pi / 2) > 0.8:
            continue  # (the visor stays clear)
        o += drip_on(f"hdrip{k}", lambda z: prof_r(helm, z), a, 4.42, 0.25 + hsh(k + 30) * 0.4, 0.045, "wax", sy=0.96, seed=k + 40)
    P["head"] = part(o, "head", (0, 0, 3.55))
    # --- arms (left -X, right +X)
    for s, side in ((-1, "l"), (1, "r")):
        sh = (1.0 * s, 0.0, 3.25)
        el = (1.22 * s, 0.06, 2.45)
        wr = (1.3 * s, 0.5, 1.95)
        o = []
        o.append(ball(f"pauldron{side}", 0.56, (1.06 * s, 0, 3.36), "iron", scale=(1.0, 0.95, 0.72), seg=18, rings=10))
        o.append(ball(f"lame{side}", 0.5, (1.14 * s, 0, 3.12), "iron", scale=(1.05, 1.0, 0.45), seg=18, rings=8))
        o.append(ball(f"waxcap{side}", 0.36, (1.0 * s, 0.02, 3.62), "wax", scale=(1.0, 1.0, 0.38), seg=14, rings=8))
        def paul(z, s=s):
            r1 = 0.56 * math.sqrt(max(0.0, 1 - ((z - 3.36) / (0.56 * 0.72)) ** 2))
            r2 = 0.5 * 1.05 * math.sqrt(max(0.0, 1 - ((z - 3.12) / (0.5 * 0.45)) ** 2))
            return max(r1, r2)
        for k in range(5):
            a = k * 1.25 + (0.3 if s > 0 else 2.0)
            o += drip_on(f"adrip{side}{k}", paul, a, 3.6, 0.35 + hsh(k + 50 + s) * 0.3, 0.05, "wax", c=(1.06 * s, 0.0), seed=k + 60)
        o.append(limb(f"upper{side}", (1.12 * s, 0, 3.15), el, 0.25, 0.21, "iron"))
        P[f"upper_{side}"] = part(o, f"upper_{side}", sh)
        o = []
        o.append(ball(f"couter{side}", 0.23, el, "iron", seg=12, rings=8))
        o.append(limb(f"vambrace{side}", el, wr, 0.21, 0.17, "iron"))
        o.append(limb(f"cuff{side}", (wr[0], wr[1] - 0.14, wr[2] + 0.12), (wr[0], wr[1] + 0.06, wr[2] - 0.04), 0.2, 0.25, "iron"))
        fist = (wr[0], wr[1] + 0.14, wr[2] - 0.12)
        o.append(W.box(f"fist{side}", (0.26, 0.28, 0.3), fist, "iron", bevel=0.04))
        for f in range(4):
            o.append(W.box(f"finger{side}{f}", (0.07, 0.08, 0.26), (fist[0] - 0.1 * s + f * 0.065 * s, fist[1] + 0.15, fist[2] - 0.02), "iron", bevel=0.02))
        o += drip(f"fdrip{side}", (el[0], el[1] + 0.1, el[2] + 0.1), 0.5, 0.045, "wax", (0.0, 0.08), seed=70 + s)
        P[f"fore_{side}"] = part(o, f"fore_{side}", el)
    # --- legs
    for s, side in ((-1, "l"), (1, "r")):
        hip = (0.46 * s, 0.0, 2.0)
        knee = (0.5 * s, 0.06, 1.05)
        ank = (0.52 * s, 0.0, 0.26)
        o = [limb(f"cuisse{side}", hip, knee, 0.31, 0.25, "iron", sides=14)]
        o.append(ball(f"poleyn{side}", 0.25, (knee[0], knee[1] + 0.12, knee[2] + 0.02), "iron", seg=12, rings=8))
        o.append(W.box(f"wing{side}", (0.06, 0.3, 0.3), (knee[0] + 0.22 * s, knee[1] + 0.05, knee[2]), "iron", bevel=0.02))
        o += drip(f"ldrip{side}", (hip[0] + 0.15 * s, 0.25, 1.9), 0.6, 0.05, "wax", (0.0, 0.05), seed=80 + s)
        P[f"thigh_{side}"] = part(o, f"thigh_{side}", hip)
        o = [limb(f"greave{side}", knee, ank, 0.23, 0.2, "iron", sides=14)]
        o.append(W.box(f"sabaton{side}", (0.38, 0.72, 0.22), (ank[0], 0.16, 0.12), "iron", bevel=0.05))
        o.append(ball(f"toe{side}", 0.2, (ank[0], 0.5, 0.12), "iron", scale=(0.95, 1.0, 0.6), seg=12, rings=8))
        o.append(ball(f"anklewax{side}", 0.32, (ank[0], 0.05, 0.22), "wax", scale=(1.0, 1.0, 0.5), seg=12, rings=8))
        for k in range(3):
            a = k * 2.1 + s
            o += drip_on(f"gdrip{side}{k}", lambda z: 0.2 + 0.03 * (z - 0.26) / 0.8, a, 0.95, 0.35 + hsh(k + 90) * 0.25, 0.045, "wax", c=(knee[0], 0.03), seed=90 + k)
        P[f"shin_{side}"] = part(o, f"shin_{side}", knee)
    # --- the wax pooled at its feet
    o = [lathe("pool", [(1.95, 0.0), (1.9, 0.04), (1.55, 0.075), (0.6, 0.09), (0.05, 0.09)], "wax", segs=36,
               wob=lambda a, z: 0.14 * math.sin(a * 5 + 0.7) + 0.07 * math.sin(a * 11 + 2.0))]
    for k in range(7):
        a = k * 0.9 + 0.3
        rr = 1.0 + hsh(k + 100) * 0.7
        o.append(ball(f"blob{k}", 0.16 + hsh(k) * 0.12, (rr * math.cos(a), rr * math.sin(a), 0.05), "wax", scale=(1.4, 1.0, 0.3), seg=10, rings=6))
    P["pool"] = part(o, "pool", (0, 0, 0))
    # --- the halberd: an iron candle-stand with an axe, held in the right fist
    gx, gy = 1.3, 0.64
    o = [zcyl("shaft", 0.065, 0.15, 4.9, "iron", x=gx, y=gy, verts=10)]
    o.append(zcyl("grip", 0.08, 1.5, 2.2, "cloth_dark", x=gx, y=gy, verts=10))
    for k, z in enumerate((0.6, 1.4, 2.3, 3.4, 4.15)):
        o.append(zcyl(f"band{k}", 0.09, z - 0.04, z + 0.04, "gold", x=gx, y=gy, verts=12))
    o.append(zcyl("butt", 0.1, 0.05, 0.2, "iron", x=gx, y=gy, r1=0.07, verts=10))
    o.append(lathe("dish", [(0.07, 4.22), (0.34, 4.32), (0.37, 4.38), (0.33, 4.4)], "gold", segs=18, loc=(gx, gy, 0.0)))
    for k in range(3):
        a = 2 * math.pi * k / 3 + 0.5
        x, y = gx + 0.22 * math.cos(a), gy + 0.22 * math.sin(a)
        h = 0.26 + 0.08 * k
        o.append(zcyl(f"hcandle{k}", 0.06, 4.36, 4.36 + h, "wax", x=x, y=y, verts=10))
        o.append(zcyl(f"hwick{k}", 0.01, 4.34 + h, 4.42 + h, "ink", x=x, y=y, verts=5))
        fx.append(marker("halberd", "flame", k, (x, y, 4.5 + h)))
        o += drip(f"hdrip{k}", (x, y, 4.36), 0.5 + 0.3 * k, 0.03, "wax", (0.0, 0.0), seed=110 + k)
    o += drip("shaftdrip", (gx + 0.05, gy, 4.2), 1.1, 0.04, "wax", (0.0, 0.0), seed=120)
    o.append(W.profile("axe", [(gy + 0.02, 4.42), (gy + 0.3, 4.52), (gy + 0.78, 4.86), (gy + 0.98, 4.6), (gy + 1.02, 4.18), (gy + 0.88, 3.84), (gy + 0.34, 4.0), (gy + 0.02, 4.1)], 0.06, "iron", x=gx, bevel=0.012, segments=1))
    o.append(W.profile("axe_edge", [(gy + 0.98, 4.6), (gy + 1.06, 4.62), (gy + 1.1, 4.18), (gy + 0.94, 3.82), (gy + 0.88, 3.84), (gy + 1.02, 4.18)], 0.03, "gold", x=gx, bevel=0.0))
    o.append(W.profile("hook", [(gy - 0.02, 4.24), (gy - 0.5, 4.14), (gy - 0.66, 3.9), (gy - 0.42, 4.04), (gy - 0.02, 4.06)], 0.05, "iron", x=gx, bevel=0.01))
    o.append(zcyl("spike", 0.075, 4.9, 5.55, "iron", x=gx, y=gy, r1=0.004, verts=8))
    fx.append(marker("halberd", "blade", 0, (gx, gy + 0.9, 4.3)))
    P["halberd"] = part(o, "halberd", (gx, gy - 0.06, 1.85))
    return list(P.values()), fx


# ---------------------------------------------------------------- the Twin Assessors

def tally():
    """Tally, the Gilded Assessor: squat and broad in heavy gold damask, a white ruff, a judge's
    curled wig under a wide gilded hat, a monocle on a chain, coin belt and chain of office; she
    holds out great scales of judgement in her left hand and throws coins with her right."""
    P = {}
    fx = []
    o = [lathe("robe", [(0.98, 0.0), (0.95, 0.16), (0.82, 0.6), (0.68, 1.1), (0.56, 1.48)], "cloth_gold", segs=30, wob=robe_wob(9, 0.07, 1.5), sy=0.92)]
    o.append(lathe("hem", [(1.0, 0.0), (1.0, 0.13), (0.96, 0.17)], "gold", segs=30, wob=robe_wob(9, 0.07, 1.5), sy=0.92))
    for s in (-1, 1):
        a = math.pi / 2 + s * 0.28
        pts = [((0.98 - z * 0.28) * math.cos(a) * 1.02, (0.98 - z * 0.28) * math.sin(a) * 0.94, z) for z in (0.02, 0.5, 1.0, 1.45)]
        o.append(ttube(f"stripe{s}", pts, [0.035] * 4, "gold", sides=6))
    o.append(W.ring("coins", 0.57, 0.06, (0, 0, 1.46), "gold", axis="Z", maj=22, minr=6))
    P["robe"] = part(o, "robe", (0, 0, 0))
    o = [lathe("chest", [(0.58, 1.42), (0.64, 1.72), (0.66, 1.95), (0.56, 2.14), (0.3, 2.28)], "cloth_gold", segs=24, sy=0.86)]
    o.append(W.ring("ruff", 0.3, 0.09, (0, 0, 2.26), "cloth_white", axis="Z", maj=20, minr=8))
    o.append(W.ring("ruff2", 0.34, 0.06, (0, 0, 2.2), "cloth_white", axis="Z", maj=20, minr=6))
    for s in (-1, 1):
        pts = [(0.28 * s, 0.1, 2.22), (0.36 * s, 0.42, 2.0), (0.3 * s, 0.6, 1.6), (0.28 * s, 0.66, 1.3)]
        o.append(ttube(f"stole{s}", pts, [0.08, 0.09, 0.09, 0.1], "cloth_red", sides=6, flat=0.35))
    o += chain("office", [(-0.3, 0.42, 2.12), (-0.18, 0.6, 1.86), (0.0, 0.66, 1.78), (0.18, 0.6, 1.86), (0.3, 0.42, 2.12)], "gold", link=0.09, r=0.016)
    o.append(ycyl("medal", 0.1, 0.62, 0.68, "gold", z=1.74, verts=14))
    fx.append(marker("torso", "chain", 0, (0.0, 0.7, 1.8)))
    P["torso"] = part(o, "torso", (0, 0, 1.45))
    o = [zcyl("neck", 0.13, 2.2, 2.38, "skin_pale", verts=10)]
    o.append(ball("face", 0.27, (0, 0.02, 2.56), "skin_pale", scale=(0.92, 0.95, 1.08), seg=18, rings=12))
    o.append(ball("jowls", 0.2, (0, 0.1, 2.42), "skin_pale", scale=(1.15, 0.8, 0.6), seg=14, rings=8))
    o.append(rot(ycyl("nose", 0.05, 0.22, 0.34, "skin_pale", z=2.57, r1=0.03, verts=8), -15, 0, 0))
    o.append(W.box("mouth", (0.14, 0.03, 0.025), (0, 0.27, 2.45), "ink", bevel=0.005))
    for s in (-1, 1):
        o.append(rot(W.box(f"brow{s}", (0.13, 0.05, 0.035), (0.1 * s, 0.25, 2.67), "wig", bevel=0.01), 0, -14 * s, 0))
        o.append(ball(f"eye{s}", 0.035, (0.1 * s, 0.25, 2.61), "ink", seg=8, rings=6))
        for k in range(3):
            o.append(ycyl(f"curl{s}{k}", 0.075, -0.2, 0.12, "wig", x=(0.28 + k * 0.02) * s, z=2.6 - k * 0.13, verts=12))
    o.append(ball("wigtop", 0.29, (0, -0.02, 2.66), "wig", scale=(1.0, 1.0, 0.7), seg=16, rings=10))
    o.append(W.ring("monocle", 0.065, 0.012, (0.1, 0.28, 2.61), "gold", axis="Y", maj=14, minr=5))
    o.append(ycyl("lens", 0.058, 0.272, 0.28, "glow_glass", x=0.1, z=2.61, verts=12))
    o.append(ttube("mchain", [(0.16, 0.27, 2.56), (0.2, 0.24, 2.4), (0.18, 0.28, 2.26)], [0.008] * 3, "gold", sides=4))
    fx.append(marker("head", "monocle", 0, (0.1, 0.3, 2.61)))
    o.append(lathe("hat", [(0.31, 2.76), (0.33, 2.84), (0.29, 2.98), (0.27, 3.04)], "gold", segs=20))
    brim = W.ring("brim", 0.44, 0.045, (0, 0, 2.78), "gold", axis="Z", maj=24, minr=6)
    brim.scale = (1.3, 0.95, 1.0)
    o.append(brim)
    o.append(lathe("brimfill", [(0.45, 2.76), (0.44, 2.8)], "cloth_gold", segs=24, sx=1.3, sy=0.95))
    for k in range(5):
        a = math.pi / 2 + (k - 2) * 0.5
        o.append(zcyl(f"point{k}", 0.05, 3.02, 3.18 + (0.06 if k == 2 else 0), "gold", x=0.27 * math.cos(a), y=0.26 * math.sin(a), r1=0.005, verts=6))
    P["head"] = part(o, "head", (0, 0, 2.3))
    for s, side in ((-1, "l"), (1, "r")):
        sh = (0.56 * s, 0.0, 2.04)
        el = (0.7 * s, 0.14, 1.56)
        wr = (0.72 * s, 0.5, 1.44)
        P[f"upper_{side}"] = part([ball(f"puff{side}", 0.24, sh, "cloth_gold", seg=12, rings=8), limb(f"upper{side}", sh, el, 0.2, 0.22, "cloth_gold")], f"upper_{side}", sh)
        o = [limb(f"sleeve{side}", el, wr, 0.21, 0.36, "cloth_gold", sides=14)]
        cuff = W.ring(f"cuff{side}", 0.35, 0.04, (0, 0, 0), "gold", axis="Y", maj=16, minr=5)
        cuff.location = wr
        cuff.rotation_mode = "QUATERNION"
        cuff.rotation_quaternion = Vector((0, 1, 0)).rotation_difference((Vector(wr) - Vector(el)).normalized())
        o.append(cuff)
        o.append(ball(f"hand{side}", 0.1, (wr[0], wr[1] + 0.12, wr[2] - 0.02), "skin_pale", scale=(0.9, 1.2, 0.8), seg=10, rings=7))
        P[f"fore_{side}"] = part(o, f"fore_{side}", el)
    fx.append(marker("fore_r", "hand", 0, (0.72, 0.66, 1.42)))
    # the scales of judgement, in the left hand
    hx, hy, hz = -0.72, 0.64, 1.42
    o = [zcyl("post", 0.03, hz - 0.05, hz + 0.78, "gold", x=hx, y=hy, verts=8)]
    o.append(ball("finial", 0.06, (hx, hy, hz + 0.84), "gold", seg=10, rings=6))
    o.append(xcyl("beam", 0.028, hx - 0.5, hx + 0.5, "gold", y=hy, z=hz + 0.72, verts=8))
    for s in (-1, 1):
        px = hx + 0.48 * s
        for k in range(3):
            a = 2 * math.pi * k / 3
            o.append(ttube(f"cord{s}{k}", [(px, hy, hz + 0.72), (px + 0.17 * math.cos(a), hy + 0.17 * math.sin(a), hz + 0.25)], [0.008, 0.008], "gold", sides=4))
        o.append(lathe(f"pan{s}", [(0.03, hz + 0.16), (0.2, hz + 0.21), (0.22, hz + 0.26)], "gold", segs=16, loc=(px, hy, 0.0)))
    for k in range(4):
        o.append(zcyl(f"coin{k}", 0.06, hz + 0.21 + k * 0.025, hz + 0.23 + k * 0.025, "gold", x=hx - 0.48 + (k % 2) * 0.03, y=hy + 0.02 * k, verts=10))
    P["scales"] = part(o, "scales", (hx, hy, hz))
    return list(P.values()), fx


def seal():
    """Seal, the Grey Assessor: tall and thin in ash robes with iron buckles, a gaunt grey face
    under long grey locks and a tall black hat with a quill in its band; an iron ledger hangs on
    a chain locked to its left wrist, and its right hand holds a great iron stamp."""
    P = {}
    fx = []
    o = [lathe("robe", [(0.64, 0.0), (0.62, 0.2), (0.52, 0.8), (0.44, 1.3), (0.4, 1.56)], "cloth_ash", segs=26, wob=robe_wob(7, 0.08, 1.6), sy=0.9)]
    o.append(lathe("hem", [(0.66, 0.0), (0.66, 0.1), (0.63, 0.14)], "cloth_dark", segs=26, wob=robe_wob(7, 0.08, 1.6), sy=0.9))
    for k, z in enumerate((0.5, 0.9, 1.3)):
        r = 0.62 - z * 0.15
        o.append(W.box(f"buckle{k}", (0.12, 0.05, 0.1), (0, r * 0.9 + 0.01, z), "iron", bevel=0.01))
    o.append(W.ring("belt", 0.42, 0.04, (0, 0, 1.52), "iron", axis="Z", maj=18, minr=5))
    P["robe"] = part(o, "robe", (0, 0, 0))
    o = [lathe("chest", [(0.42, 1.5), (0.46, 1.8), (0.46, 2.05), (0.38, 2.24), (0.22, 2.34)], "cloth_ash", segs=20, sy=0.86)]
    o.append(lathe("collar", [(0.22, 2.24), (0.27, 2.42), (0.3, 2.56)], "cloth_dark", segs=18, sy=0.9))
    o += chain("sash", [(-0.36, 0.3, 2.14), (-0.1, 0.42, 1.9), (0.2, 0.36, 1.66), (0.38, 0.2, 1.54)], "iron", link=0.1, r=0.02)
    fx.append(marker("torso", "chain", 0, (0.0, 0.46, 1.86)))
    P["torso"] = part(o, "torso", (0, 0, 1.5))
    o = [zcyl("neck", 0.1, 2.3, 2.5, "skin_grey", verts=10)]
    o.append(ball("face", 0.21, (0, 0.02, 2.66), "skin_grey", scale=(0.82, 0.92, 1.38), seg=16, rings=12))
    o.append(ball("chin", 0.09, (0, 0.13, 2.42), "skin_grey", scale=(0.8, 0.9, 0.9), seg=10, rings=6))
    o.append(rot(ycyl("nose", 0.04, 0.16, 0.32, "skin_grey", z=2.66, r1=0.012, verts=8), -25, 0, 0))
    o.append(W.box("mouth", (0.09, 0.02, 0.02), (0, 0.18, 2.5), "ink", bevel=0.0))
    for s in (-1, 1):
        o.append(ball(f"socket{s}", 0.05, (0.075 * s, 0.17, 2.74), "ink", scale=(1.2, 0.6, 0.8), seg=8, rings=6))
        o.append(rot(W.box(f"brow{s}", (0.1, 0.04, 0.03), (0.075 * s, 0.18, 2.8), "wig", bevel=0.008), 0, 18 * s, 0))
        for k in range(3):
            x = (0.16 + k * 0.03) * s
            o.append(ttube(f"lock{s}{k}", [(x, -0.02 - k * 0.04, 2.86), (x * 1.25, -0.04 - k * 0.04, 2.6), (x * 1.3, -0.02, 2.3)], [0.05, 0.045, 0.03], "wig", sides=6))
        fx.append(marker("head", "eye", 0 if s < 0 else 1, (0.075 * s, 0.22, 2.74)))
    o.append(lathe("hat", [(0.22, 2.86), (0.2, 3.05), (0.23, 3.58), (0.18, 3.6)], "cloth_dark", segs=18))
    o.append(W.ring("hatbrim", 0.29, 0.03, (0, 0, 2.87), "cloth_dark", axis="Z", maj=20, minr=5))
    o.append(lathe("hatband", [(0.212, 2.9), (0.212, 3.0)], "iron", segs=18))
    o.append(rot(W.profile("hatquill", [(0.0, 3.0), (0.06, 3.2), (0.04, 3.5), (0.0, 3.62), (-0.03, 3.4), (-0.02, 3.1)], 0.015, "parchment", x=0.2, bevel=0.0), 0, -12, 0))
    P["head"] = part(o, "head", (0, 0, 2.35))
    for s, side in ((-1, "l"), (1, "r")):
        sh = (0.42 * s, 0.0, 1.98)
        el = (0.5 * s, 0.1, 1.46)
        wr = (0.52 * s, 0.42, 1.24)
        P[f"upper_{side}"] = part([limb(f"upper{side}", sh, el, 0.14, 0.14, "cloth_ash")], f"upper_{side}", sh)
        o = [limb(f"sleeve{side}", el, wr, 0.14, 0.2, "cloth_ash")]
        o.append(ball(f"hand{side}", 0.08, (wr[0], wr[1] + 0.08, wr[2] - 0.03), "skin_grey", scale=(0.8, 1.3, 0.8), seg=10, rings=7))
        if s < 0:
            o.append(W.ring("manacle", 0.1, 0.03, (wr[0], wr[1] - 0.02, wr[2] + 0.01), "iron", axis="Y", maj=12, minr=5))
        else:
            # the great stamp: a wooden grip, an iron collar, the seal face below the fist
            o.append(zcyl("stamp_grip", 0.05, wr[2] - 0.1, wr[2] + 0.14, "wood_boss", x=wr[0], y=wr[1] + 0.1, verts=10))
            o.append(zcyl("stamp_neck", 0.07, wr[2] - 0.32, wr[2] - 0.1, "iron", x=wr[0], y=wr[1] + 0.1, r1=0.05, verts=10))
            o.append(zcyl("stamp_face", 0.2, wr[2] - 0.42, wr[2] - 0.3, "iron", x=wr[0], y=wr[1] + 0.1, verts=16))
            o.append(zcyl("stamp_glow", 0.15, wr[2] - 0.44, wr[2] - 0.42, "glow_blood", x=wr[0], y=wr[1] + 0.1, verts=14))
            fx.append(marker("fore_r", "hand", 0, (wr[0], wr[1] + 0.1, wr[2] - 0.44)))
        P[f"fore_{side}"] = part(o, f"fore_{side}", el)
    # the iron ledger, chained to the left wrist
    wx, wy, wz = -0.52, 0.4, 1.24
    o = chain("lchain", [(wx, wy, wz), (wx - 0.05, wy + 0.02, wz - 0.32), (wx - 0.06, wy, wz - 0.5)], "iron", link=0.1, r=0.02)
    bz = wz - 0.85
    o.append(W.box("lcover", (0.18, 0.6, 0.78), (wx - 0.06, wy, bz), "iron", bevel=0.025))
    o.append(W.box("lpages", (0.16, 0.56, 0.72), (wx - 0.06, wy + 0.03, bz), "parchment", bevel=0.0))
    for k, z in enumerate((bz - 0.22, bz + 0.22)):
        o.append(W.box(f"lstrap{k}", (0.21, 0.63, 0.06), (wx - 0.06, wy, z), "gold", bevel=0.01))
    o.append(W.box("lboss", (0.21, 0.18, 0.18), (wx - 0.06, wy, bz), "gold", bevel=0.02))
    P["ledger"] = part(o, "ledger", (wx, wy, wz))
    return list(P.values()), fx


# ---------------------------------------------------------------- the Chained Ledger

def ledger():
    """A giant living book: thick red-leather covers with brass corners are its jaws, rows of
    page-teeth inside, violet eyes on the upper cover, a ribbon tongue, page wings that flap at its
    sides, iron chains wrapped round it, and a glowing quill on a tail of ink behind."""
    P = {}
    fx = []
    hinge_y = -1.25
    o = [W.box("lcover", (2.2, 2.7, 0.14), (0, 0, -0.34), "leather_book", bevel=0.04)]
    o.append(W.box("block", (2.0, 2.45, 0.3), (0, 0.02, -0.12), "parchment", bevel=0.015))
    o.append(xcyl("spine", 0.34, -1.12, 1.12, "leather_book", y=hinge_y, z=-0.12, verts=18))
    for k, x in enumerate((-0.75, 0.0, 0.75)):
        o.append(W.ring(f"sband{k}", 0.35, 0.04, (x, hinge_y, -0.12), "gold", axis="X", maj=18, minr=5))
    for sx in (-1, 1):
        for sy in (-1, 1):
            o.append(W.box(f"lcorner{sx}{sy}", (0.34, 0.34, 0.18), (1.0 * sx, 1.2 * sy, -0.34), "gold", bevel=0.03))
    for k in range(10):
        x = -0.9 + k * 0.2
        o.append(zcyl(f"tooth{k}", 0.085, 0.02, 0.24 + 0.06 * (k % 2), "parchment", x=x, y=1.14, r1=0.006, verts=6))
    o.append(ttube("tongue", [(0, 0.2, 0.05), (0, 0.9, 0.08), (0, 1.35, 0.0), (0.05, 1.55, -0.3), (0.0, 1.6, -0.62)], [0.11, 0.12, 0.12, 0.1, 0.06], "cloth_red", sides=6, flat=0.25))
    # chains wrapped round the lower jaw
    for k, y in enumerate((-0.55, 0.45)):
        pts = []
        for j in range(13):
            a = math.pi * j / 12
            pts.append((1.15 * math.cos(a), y, -0.24 - 0.26 * math.sin(a)))
        o += chain(f"wrap{k}", pts, "iron", link=0.14, r=0.028)
    for k, (x, y) in enumerate(((-1.12, -0.55), (1.12, -0.55), (-1.12, 0.45), (1.12, 0.45))):
        fx.append(marker("lower", "chain", k, (x, y, -0.3)))
    fx.append(marker("lower", "mouth", 0, (0, 1.2, 0.05)))
    P["lower"] = part(o, "lower", (0, hinge_y, 0.0))
    o = [W.box("ucover", (2.2, 2.7, 0.14), (0, 0, 0.12), "leather_book", bevel=0.04)]
    for sx in (-1, 1):
        for sy in (-1, 1):
            o.append(W.box(f"ucorner{sx}{sy}", (0.34, 0.34, 0.18), (1.0 * sx, 1.2 * sy, 0.12), "gold", bevel=0.03))
    o.append(lathe("clasp", [(0.32, 0.18), (0.32, 0.22), (0.24, 0.26), (0.08, 0.27)], "gold", segs=20, loc=(0, -0.05, 0)))
    o.append(ball("sigil", 0.1, (0, -0.05, 0.27), "glow_violet", scale=(1, 1, 0.35), seg=12, rings=6))
    for s in (-1, 1):
        o.append(ball(f"eye{s}", 0.2, (0.48 * s, 0.95, 0.2), "glow_violet", scale=(1.3, 0.9, 0.5), seg=14, rings=8))
        er = W.ring(f"eyering{s}", 0.23, 0.04, (0.48 * s, 0.95, 0.2), "gold", axis="Z", maj=16, minr=5)
        o.append(er)
        o.append(rot(W.box(f"browplate{s}", (0.42, 0.1, 0.06), (0.48 * s, 0.76, 0.26), "gold", bevel=0.015), 0, 0, -14 * s))
        fx.append(marker("upper", "eye", 0 if s < 0 else 1, (0.48 * s, 0.98, 0.28)))
    o.append(W.box("strap", (0.24, 0.5, 0.06), (0, 1.36, 0.0), "gold", bevel=0.015))
    # page fangs hanging from the upper jaw (cones pointing down)
    for k in range(9):
        x = -0.8 + k * 0.2
        o.append(zcyl(f"fang{k}", 0.006, -0.2 - 0.05 * (k % 2), 0.04, "parchment", x=x, y=1.12, r1=0.08, verts=6))
    P["upper"] = part(o, "upper", (0, hinge_y, 0.06))
    # page wings, three sheets a side, fanned
    for s, side in ((-1, "l"), (1, "r")):
        o = []
        for k in range(3):
            length = 1.55 - k * 0.15
            sh = sheet(f"page{side}{k}", 0.0, length * s, -0.75 + k * 0.08, 0.55 - k * 0.1, 0.0, 0.32 + 0.1 * k, "parchment", droop=0.12)
            sh.location = (1.0 * s, -0.25, 0.02 + 0.05 * k)
            sh.rotation_euler = (0.0, math.radians(-18 * k) * s, math.radians(8 * k) * s)
            o.append(sh)
        P[f"wing_{side}"] = part(o, f"wing_{side}", (1.0 * s, -0.25, 0.02))
    # the ink tail and its glowing quill
    o = [ttube("tail", [(0, -1.45, -0.1), (0, -1.95, -0.3), (0, -2.5, -0.35), (0, -2.95, -0.2)], [0.2, 0.15, 0.1, 0.07], "ink", sides=10)]
    o.append(W.profile("vane", [(-2.9, -0.15), (-3.3, 0.05), (-3.8, 0.2), (-4.35, 0.18), (-4.6, 0.1), (-4.1, 0.02), (-3.5, -0.12), (-3.05, -0.22)], 0.03, "glow_quill", x=0.0, bevel=0.0))
    o.append(ycyl("shaftq", 0.03, -4.55, -2.85, "glow_glass", z=0.0, r1=0.012, verts=6))
    o.append(ycyl("nib", 0.06, -3.0, -2.75, "gold", z=-0.15, r1=0.012, verts=8))
    fx.append(marker("quill", "tip", 0, (0, -4.5, 0.12)))
    P["quill"] = part(o, "quill", (0, -1.45, -0.1))
    return list(P.values()), fx


def props():
    """A chain link and a floor anchor (the game draws them many times)."""
    link = W.ring("link", 0.07, 0.024, (0, 0, 0), "iron", axis="Y", maj=10, minr=5)
    link.scale = (1.0, 1.0, 1.6)
    out = [part([link], "link", (0, 0, 0))]
    o = [lathe("plate", [(0.55, 0.0), (0.55, 0.06), (0.48, 0.1), (0.2, 0.12)], "iron", segs=16)]
    for k in range(4):
        a = k * math.pi / 2 + 0.4
        o.append(ball(f"bolt{k}", 0.06, (0.38 * math.cos(a), 0.38 * math.sin(a), 0.1), "gold", seg=8, rings=5))
    o.append(W.ring("staple", 0.18, 0.045, (0, 0, 0.2), "iron", axis="Y", maj=12, minr=6))
    out.append(part(o, "anchor", (0, 0, 0)))
    return out, [marker("anchor", "ring", 0, (0, 0, 0.36))]


# ---------------------------------------------------------------- the Ashen Magistrate

def magistrate():
    """The Ashen Magistrate: a huge judge in ash-grey robes smouldering at the hem, a black
    mantle with red trim, a full-bottomed wig under a black cap, white bands and a chain of office
    with an ember seal; a stern grey face with ember eyes; a great burning gavel in its right hand."""
    P = {}
    fx = []
    o = [lathe("robe", [(1.15, 0.0), (1.1, 0.2), (0.96, 0.8), (0.82, 1.4), (0.72, 1.84)], "cloth_ash", segs=32, wob=robe_wob(11, 0.07, 1.9), sy=0.92)]
    o.append(lathe("hem", [(1.17, 0.0), (1.17, 0.1), (1.12, 0.16)], "cloth_dark", segs=32, wob=robe_wob(11, 0.07, 1.9), sy=0.92))
    for k in range(26):
        a = 2 * math.pi * k / 26 + hsh(k) * 0.2
        r = 1.17 * (1 + 0.07 * math.sin(a * 11 + 0.4))
        o.append(ball(f"ember{k}", 0.03 + hsh(k + 7) * 0.03, (r * math.cos(a), r * math.sin(a) * 0.92, 0.04 + hsh(k + 3) * 0.12), "glow_ember", seg=6, rings=4))
    for s in (-1, 1):
        a = math.pi / 2 + s * 0.2
        pts = [((1.12 - z * 0.22) * math.cos(a) * 1.02, (1.12 - z * 0.22) * math.sin(a) * 0.95, z) for z in (0.02, 0.6, 1.2, 1.8)]
        o.append(ttube(f"stripe{s}", pts, [0.07] * 4, "cloth_red", sides=6, flat=0.4))
    P["robe"] = part(o, "robe", (0, 0, 0))
    o = [lathe("chest", [(0.74, 1.78), (0.86, 2.15), (0.9, 2.5), (0.74, 2.78), (0.4, 2.92)], "cloth_ash", segs=26, sy=0.82)]
    o.append(lathe("mantle", [(1.0, 2.38), (0.96, 2.5), (0.78, 2.76), (0.46, 2.94)], "cloth_dark", segs=26, sy=0.86,
                   wob=lambda a, z: 0.05 * math.sin(a * 6) if z < 2.45 else 0.0))
    o.append(lathe("mtrim", [(1.02, 2.36), (1.02, 2.42), (0.99, 2.45)], "cloth_red", segs=26, sy=0.86, wob=lambda a, z: 0.05 * math.sin(a * 6)))
    for s in (-1, 1):
        o.append(rot(W.box(f"band{s}", (0.1, 0.03, 0.32), (0.06 * s, 0.76, 2.62), "cloth_white", bevel=0.01), 0, 6 * s, 0))
    o += chain("office", [(-0.5, 0.5, 2.7), (-0.32, 0.72, 2.4), (0.0, 0.8, 2.26), (0.32, 0.72, 2.4), (0.5, 0.5, 2.7)], "gold", link=0.12, r=0.022)
    o.append(ycyl("seal", 0.15, 0.76, 0.84, "gold", z=2.18, verts=16))
    o.append(ycyl("sealgem", 0.08, 0.83, 0.87, "glow_ember", z=2.18, verts=12))
    P["torso"] = part(o, "torso", (0, 0, 1.8))
    o = [zcyl("neck", 0.2, 2.82, 3.02, "skin_grey", verts=12)]
    o.append(ball("face", 0.3, (0, 0.04, 3.2), "skin_grey", scale=(0.9, 0.95, 1.12), seg=18, rings=12))
    o.append(ball("jaw", 0.22, (0, 0.12, 3.02), "skin_grey", scale=(1.05, 0.85, 0.6), seg=14, rings=8))
    o.append(rot(ycyl("nose", 0.06, 0.26, 0.42, "skin_grey", z=3.18, r1=0.03, verts=8), -20, 0, 0))
    o.append(W.box("mouth", (0.16, 0.03, 0.03), (0, 0.3, 3.03), "ink", bevel=0.005))
    for s in (-1, 1):
        o.append(rot(W.box(f"brow{s}", (0.16, 0.07, 0.05), (0.11 * s, 0.28, 3.31), "wig", bevel=0.015), 0, 20 * s, 0))
        o.append(ball(f"eye{s}", 0.045, (0.11 * s, 0.29, 3.24), "glow_ember", scale=(1.3, 0.6, 0.8), seg=8, rings=6))
        fx.append(marker("head", "eye", 0 if s < 0 else 1, (0.11 * s, 0.33, 3.24)))
        for k in range(4):
            z = 3.32 - k * 0.15
            o.append(ycyl(f"roll{s}{k}", 0.085, -0.25, 0.16, "wig", x=(0.34 + k * 0.025) * s, z=z, verts=12))
        o.append(ttube(f"lappet{s}", [(0.32 * s, 0.1, 3.0), (0.36 * s, 0.26, 2.75), (0.34 * s, 0.34, 2.45)], [0.11, 0.1, 0.08], "wig", sides=8))
    o.append(ball("wigtop", 0.37, (0, -0.04, 3.36), "wig", scale=(1.05, 1.05, 0.72), seg=18, rings=10))
    o.append(ball("wigback", 0.4, (0, -0.22, 2.98), "wig", scale=(1.0, 0.6, 0.9), seg=16, rings=10))
    o.append(rot(W.box("cap", (0.42, 0.42, 0.035), (0, -0.02, 3.62), "cloth_dark", bevel=0.01), 0, 0, 45))
    P["head"] = part(o, "head", (0, 0, 2.9))
    for s, side in ((-1, "l"), (1, "r")):
        sh = (0.86 * s, 0.0, 2.58)
        el = (1.02 * s, 0.14, 2.0)
        wr = (1.04 * s, 0.56, 1.74)
        P[f"upper_{side}"] = part([ball(f"shoulder{side}", 0.3, sh, "cloth_ash", seg=12, rings=8), limb(f"upper{side}", sh, el, 0.27, 0.3, "cloth_ash")], f"upper_{side}", sh)
        o = [limb(f"sleeve{side}", el, wr, 0.3, 0.46, "cloth_ash", sides=16)]
        o.append(limb(f"cuff{side}", (wr[0], wr[1] - 0.06, wr[2] + 0.03), (wr[0], wr[1] + 0.03, wr[2] - 0.01), 0.45, 0.47, "cloth_dark", sides=16))
        o.append(ball(f"hand{side}", 0.15, (wr[0], wr[1] + 0.16, wr[2] - 0.04), "skin_grey", scale=(0.9, 1.2, 0.8), seg=12, rings=8))
        if s < 0:
            o.append(ycyl("finger", 0.04, wr[1] + 0.25, wr[1] + 0.48, "skin_grey", x=wr[0], z=wr[2] - 0.02, r1=0.03, verts=8))
            fx.append(marker("fore_l", "hand", 0, (wr[0], wr[1] + 0.52, wr[2] - 0.02)))
        P[f"fore_{side}"] = part(o, f"fore_{side}", el)
    # the burning gavel, in the right hand: its head across, forward and back
    gx, gy, gz = 1.04, 0.72, 1.7
    o = [zcyl("handle", 0.07, gz - 0.35, gz + 1.15, "wood_boss", x=gx, y=gy, verts=10)]
    o.append(ball("pommel", 0.11, (gx, gy, gz - 0.38), "iron", seg=10, rings=6))
    o.append(zcyl("ferrule", 0.09, gz + 1.05, gz + 1.15, "iron", x=gx, y=gy, verts=10))
    hz = gz + 1.38
    o.append(ycyl("ghead", 0.3, gy - 0.5, gy + 0.5, "wood_boss", x=gx, z=hz, verts=18))
    for k, y in enumerate((gy - 0.44, gy - 0.1, gy + 0.1, gy + 0.44)):
        o.append(W.ring(f"gband{k}", 0.31, 0.03, (gx, y, hz), "iron", axis="Y", maj=18, minr=5))
    for k, y in enumerate((gy - 0.52, gy + 0.52)):
        o.append(ycyl(f"gface{k}", 0.27, y - 0.03, y + 0.03, "iron", x=gx, z=hz, verts=18))
    for k in range(7):
        a = k * 0.9 + 0.3
        o.append(rot(W.box(f"gcrack{k}", (0.03, 0.22, 0.04), (gx + 0.3 * math.cos(a), gy - 0.3 + k * 0.1, hz + 0.3 * math.sin(a)), "glow_ember", bevel=0.0), math.degrees(a), 0, 0))
    for k in range(5):
        fx.append(marker("gavel", "flame", k, (gx + (hsh(k) - 0.5) * 0.3, gy - 0.4 + k * 0.2, hz + 0.32)))
    P["gavel"] = part(o, "gavel", (gx, gy - 0.02, gz))
    return list(P.values()), fx


def regalia():
    """The Ashblood Regent's crown and high mantle, worn over his coat (origins: the head and the
    neck's base; human size)."""
    out = []
    o = [lathe("band", [(0.115, 0.13), (0.12, 0.17), (0.118, 0.19)], "iron", segs=20)]
    for k in range(9):
        a = 2 * math.pi * k / 9 + math.pi / 2
        h = 0.1 if k % 2 == 0 else 0.065
        x, y = 0.118 * math.cos(a), 0.118 * math.sin(a)
        o.append(zcyl(f"spike{k}", 0.022, 0.18, 0.18 + h, "iron", x=x, y=y, r1=0.002, verts=6))
    for k in range(3):
        a = math.pi / 2 + (k - 1) * 0.7
        o.append(ball(f"gem{k}", 0.016, (0.122 * math.cos(a), 0.122 * math.sin(a), 0.16), "glow_blood", seg=8, rings=5))
    o.append(W.ring("rim", 0.12, 0.008, (0, 0, 0.13), "gold", axis="Z", maj=20, minr=4))
    out.append(part(o, "crown", (0, 0, 0)))
    # a high collar standing up behind the head, open at the front, red outside, black within
    o = [arc_wall("collar", [(0.15, 0.0), (0.18, 0.1), (0.23, 0.2)], "cloth_red", 0.02)]
    o.append(arc_wall("collar_in", [(0.135, 0.0), (0.165, 0.1), (0.215, 0.2)], "cloth_dark", 0.012))
    o.append(arc_wall("ctrim", [(0.228, 0.19), (0.236, 0.215)], "gold", 0.02))
    for s in (-1, 1):
        o.append(ball(f"pauldron{s}", 0.13, (0.22 * s, -0.01, -0.06), "iron", scale=(1.1, 1.0, 0.7), seg=14, rings=8))
        o.append(ball(f"plame{s}", 0.12, (0.25 * s, -0.01, -0.12), "iron", scale=(1.1, 1.0, 0.45), seg=14, rings=6))
        o.append(W.ring(f"ptrim{s}", 0.12, 0.01, (0.22 * s, -0.01, -0.06), "gold", axis="Z", maj=16, minr=4))
        o.append(ball(f"pgem{s}", 0.022, (0.22 * s, 0.05, 0.0), "glow_blood", seg=8, rings=5))
    out.append(part(o, "mantle", (0, 0, 0)))
    return out, []


BOSSES = {
    "warden": warden,
    "tally": tally,
    "seal": seal,
    "ledger": ledger,
    "magistrate": magistrate,
    "regalia": regalia,
    "props": props,
}


def build(name, renders=True):
    W.clear_scene()
    parts, fx = BOSSES[name]()
    tris = sum(W.triangles(o) for o in parts)
    lo, hi = W._bounds(parts)
    os.makedirs(OUT_TEX, exist_ok=True)
    bake_texture(parts, os.path.join(OUT_TEX, f"{name}.png"), size=512 if name in ("regalia", "props") else TEX)
    W.export_glb(parts + fx, os.path.join(OUT, f"{name}.glb"))
    print(f"[bosses] {name}: {tris} triangles, {len(parts)} parts, {len(fx)} markers, size x {hi.x - lo.x:.2f} y {hi.y - lo.y:.2f} z {hi.z - lo.z:.2f} m")
    if renders:
        os.makedirs(OUT_SHOTS, exist_ok=True)
        for m in fx:
            m.hide_render = True
        W.render_turntable(parts, name, os.path.join(OUT_SHOTS, f"{name}.png"))
    return tris


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    renders = "--no-renders" not in argv
    names = [a for a in argv if not a.startswith("--")] or list(BOSSES)
    done = {n: build(n, renders) for n in names}
    print("[bosses] done:", ", ".join(f"{k} {v}" for k, v in done.items()))


main()
