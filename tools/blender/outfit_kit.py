"""Building blocks for the wardrobe's outfits (build_outfits.py): fitted shells with bevelled
edges and cloth folds, layered hem trims (a band, gold piping, stitching), straps with real
buckles, rivets and studs, fur rolls, layered shoulder plates, feathers.

Materials: every outfit part names one of the materials below. The dyed ones
(`dye_<zone>_<surface>`) are baked pale and neutral but keep their surface (a cloth's weave and
folds, leather's grain and darker worn edges, metal's brushed streaks and bright edges, fur's
strands): the game multiplies them by the wearer's dye for that zone (coat, trim, accent) and
shades them as that surface. The rest keep their own colour.
"""

import math

import bmesh
import bpy
from mathutils import Matrix, Vector

import build_gear as G
import weapons_tex as T

A, W = G.A, G.W
gv, UP, FWD, LEFT = G.gv, G.UP, G.FWD, G.LEFT

# ------------------------------------------------------------------------------ materials

# name: (display colour (linear), metallic, roughness), paint (kind, low, high, edge colour,
# edge strength, occlusion strength)
_N = (0.0, 0.0, 0.0)
MATERIALS = {
    # dyed: neutral, the game tints them
    "dye_coat_cloth": (((0.6, 0.6, 0.6), 0.0, 0.9), ("cloth", (0.55, 0.55, 0.54), (0.64, 0.64, 0.63), (0.76, 0.76, 0.75), 0.3, 0.8)),
    "dye_coat_leather": (((0.6, 0.6, 0.6), 0.0, 0.55), ("leather", (0.54, 0.54, 0.53), (0.63, 0.63, 0.62), (0.30, 0.30, 0.30), 0.6, 0.75)),
    "dye_coat_metal": (((0.6, 0.6, 0.6), 0.9, 0.35), ("metal", (0.50, 0.50, 0.50), (0.66, 0.66, 0.66), (0.98, 0.98, 0.98), 0.85, 0.6)),
    "dye_trim_metal": (((0.7, 0.7, 0.7), 0.9, 0.3), ("metal", (0.52, 0.52, 0.52), (0.70, 0.70, 0.70), (1.0, 1.0, 1.0), 0.9, 0.55)),
    "dye_trim_cloth": (((0.6, 0.6, 0.6), 0.0, 0.85), ("cloth", (0.56, 0.56, 0.55), (0.65, 0.65, 0.64), (0.78, 0.78, 0.77), 0.3, 0.75)),
    "dye_trim_leather": (((0.6, 0.6, 0.6), 0.0, 0.55), ("leather", (0.54, 0.54, 0.53), (0.63, 0.63, 0.62), (0.30, 0.30, 0.30), 0.6, 0.7)),
    "dye_accent_cloth": (((0.6, 0.6, 0.6), 0.0, 0.85), ("cloth", (0.56, 0.56, 0.55), (0.65, 0.65, 0.64), (0.78, 0.78, 0.77), 0.3, 0.75)),
    "dye_accent_metal": (((0.7, 0.7, 0.7), 0.9, 0.25), ("metal", (0.55, 0.55, 0.55), (0.74, 0.74, 0.74), (1.0, 1.0, 1.0), 0.9, 0.5)),
    "dye_accent_fur": (((0.6, 0.6, 0.6), 0.0, 1.0), ("fur", (0.40, 0.40, 0.40), (0.80, 0.80, 0.80), (0.85, 0.85, 0.85), 0.0, 0.75)),
    # their own colours
    "leather_brown": (((0.14, 0.07, 0.035), 0.0, 0.6), ("leather", (0.10, 0.052, 0.026), (0.155, 0.085, 0.045), (0.045, 0.024, 0.012), 0.55, 0.7)),
    "leather_dark": (((0.06, 0.04, 0.03), 0.0, 0.55), ("leather", (0.045, 0.03, 0.022), (0.09, 0.06, 0.042), (0.16, 0.12, 0.09), 0.45, 0.65)),
    "iron": (((0.16, 0.165, 0.18), 0.9, 0.36), None),
    "brass": (((0.72, 0.48, 0.15), 1.0, 0.32), ("metal", (0.55, 0.36, 0.10), (0.75, 0.52, 0.18), (1.0, 0.85, 0.50), 0.85, 0.6)),
    "steel": (((0.6, 0.62, 0.66), 1.0, 0.25), ("metal", (0.42, 0.43, 0.46), (0.60, 0.61, 0.64), (0.95, 0.96, 0.98), 0.8, 0.6)),
    "linen": (((0.62, 0.57, 0.47), 0.0, 0.9), ("cloth", (0.52, 0.47, 0.38), (0.66, 0.61, 0.50), (0.74, 0.70, 0.60), 0.2, 0.65)),
    "wool_dark": (((0.06, 0.06, 0.08), 0.0, 0.95), ("cloth", (0.045, 0.046, 0.06), (0.085, 0.086, 0.11), (0.14, 0.14, 0.17), 0.3, 0.6)),
    "canvas": (((0.24, 0.18, 0.11), 0.0, 0.9), ("cloth", (0.19, 0.145, 0.09), (0.27, 0.21, 0.13), (0.33, 0.27, 0.18), 0.25, 0.75)),
    "wool_cream": (((0.55, 0.52, 0.45), 0.0, 0.95), ("cloth", (0.44, 0.41, 0.35), (0.58, 0.55, 0.48), (0.66, 0.63, 0.56), 0.2, 0.65)),
    "fur_brown": (((0.20, 0.13, 0.08), 0.0, 1.0), ("fur", (0.10, 0.065, 0.04), (0.36, 0.25, 0.16), (0.4, 0.3, 0.2), 0.0, 0.75)),
    "fur_grey": (((0.32, 0.31, 0.30), 0.0, 1.0), ("fur", (0.16, 0.155, 0.15), (0.55, 0.54, 0.52), (0.6, 0.6, 0.6), 0.0, 0.75)),
    "rope": (((0.30, 0.24, 0.15), 0.0, 0.9), ("rope", (0.22, 0.17, 0.10), (0.40, 0.32, 0.20), (0.12, 0.09, 0.05), 0.4, 0.7)),
    "wood": (((0.20, 0.08, 0.03), 0.0, 0.6), ("wood", (0.12, 0.05, 0.02), (0.26, 0.12, 0.05), (0.05, 0.02, 0.008), 0.6, 0.7)),
    "feather": (((0.82, 0.80, 0.76), 0.0, 0.8), ("cloth", (0.72, 0.70, 0.66), (0.90, 0.88, 0.84), (0.95, 0.94, 0.92), 0.2, 0.5)),
    "glow_accent": (((0.92, 0.92, 0.92), 0.0, 0.4), ("glow", (0.88, 0.88, 0.88), (1.0, 1.0, 1.0), (1.0, 1.0, 1.0), 0.2, 0.0)),
}

for _name, (_disp, _paint) in MATERIALS.items():
    if _name != "iron":
        W.PALETTE[_name] = _disp

_previous_paint = T.paint_material


def _paint(mat, name):
    spec = MATERIALS.get(name, (None, None))[1]
    if spec is None:
        return _previous_paint(mat, name)
    kind, lo, hi, ec, ek, ao_k = spec
    g = T.G(mat)
    if kind == "glow":
        g.emit(g.rgb(hi))
        return
    edge = g.edge_mask(radius=0.004, gain=6.0)
    occ = g.ao(0.05)
    if kind == "cloth":
        n1 = g.noise(g.coords(), 12.0, 3.0, 0.5)
        col = g.ramp(n1, [(0.2, lo), (0.8, hi)])
        fine = g.noise(g.coords(), 170.0, 2.0)
        col = g.mix(g.math("MULTIPLY", fine, 0.16), col, lo)
        col = g.mix(g.math("MULTIPLY", edge, ek), col, ec)
    elif kind == "leather":
        n1 = g.noise(g.coords(), 60.0, 5.0, 0.65)
        col = g.ramp(n1, [(0.3, lo), (0.75, hi)])
        crease = g.math("LESS_THAN", g.voronoi(g.coords(), 75.0, "DISTANCE_TO_EDGE"), 0.035)
        col = g.mix(g.math("MULTIPLY", crease, 0.22), col, ec)
        col = g.mix(g.math("MULTIPLY", edge, ek), col, ec)
    elif kind == "metal":
        streak = g.noise(g.coords((30.0, 1.0, 30.0)), 8.0, 6.0, 0.6)
        col = g.ramp(streak, [(0.25, lo), (0.8, hi)])
        col = g.mix(g.math("MULTIPLY", edge, ek), col, ec)
    elif kind == "fur":
        strands = g.wave(g.coords((1, 1, 1)), 90.0, 14.0, "Z", detail=4.0)
        col = g.ramp(strands, [(0.15, lo), (0.85, hi)])
        tips = g.noise(g.coords(), 35.0)
        col = g.mix(g.math("MULTIPLY", tips, 0.3), col, hi)
    elif kind == "rope":
        fib = g.wave(g.coords((1, 1, 1)), 140.0, 2.0, "DIAGONAL", detail=1.0)
        col = g.ramp(fib, [(0.2, lo), (0.8, hi)])
    else:  # wood
        grain = g.wave(g.coords((1.0, 0.05, 1.0)), 30.0, 6.0, "X", detail=3.0)
        col = g.ramp(grain, [(0.0, lo), (1.0, hi)])
        col = g.mix(g.math("MULTIPLY", edge, ek), col, ec)
    shade = g.mix(occ, (1.0 - ao_k,) * 3, (1.0, 1.0, 1.0))
    col = g.mix(1.0, col, shade, blend="MULTIPLY")
    # hand-painted light: tops a little lighter, undersides darker (it reads at any distance)
    geo = g.n("ShaderNodeNewGeometry")
    sep = g.n("ShaderNodeSeparateXYZ")
    g.link(geo.outputs["Normal"], sep.inputs[0])
    up = g.math("ADD", g.math("MULTIPLY", sep.outputs["Z"], 0.5), 0.5)
    lit = g.ramp(up, [(0.0, (0.72, 0.72, 0.74)), (0.55, (0.95, 0.95, 0.95)), (1.0, (1.12, 1.11, 1.08))])
    g.emit(g.mix(1.0, col, lit, blend="MULTIPLY"))


T.paint_material = _paint


def preview_dyes(coat, trim, accent, glow=(0.9, 0.5, 0.1)):
    """The dye of each material for the pictures (by its zone)."""
    out = {}
    for name in MATERIALS:
        if name.startswith("dye_coat"):
            out[name] = coat
        elif name.startswith("dye_trim"):
            out[name] = trim
        elif name.startswith("dye_accent"):
            out[name] = accent
    out["glow_accent"] = glow
    return out


# ------------------------------------------------------------------------------ geometry

def bev(ob, w=0.0035, seg=2, angle=40.0):
    """Round the hard edges (the rims a thickness leaves)."""
    m = ob.modifiers.new("bev", "BEVEL")
    m.width = w
    m.segments = seg
    m.limit_method = "ANGLE"
    m.angle_limit = math.radians(angle)
    return ob


def shell(name, rows, mat, thick, closed=True, bevel=0.0035, sub=0, offset=-1.0):
    """A sheet through rings (or rows) of points, thickened, its rims bevelled (one segment:
    the toon shading and outline read it as a rounded edge)."""
    ob = A.grid(name, rows, mat, closed=closed)
    if sub:
        A.smooth(ob, sub)
    A.solidify(ob, thick, offset)
    if bevel:
        bev(ob, bevel, 1)
    return ob


def resample(ring, n, closed=True):
    """`n` points evenly along a ring (or an open polyline)."""
    pts = list(ring) + ([ring[0]] if closed else [])
    lens = [0.0]
    for a, b in zip(pts, pts[1:]):
        lens.append(lens[-1] + (b - a).length)
    total = lens[-1]
    out = []
    count = n if closed else n - 1
    j = 0
    for k in range(n):
        s = total * k / max(count, 1)
        while j < len(lens) - 2 and lens[j + 1] < s:
            j += 1
        seg = max(lens[j + 1] - lens[j], 1e-9)
        out.append(pts[j].lerp(pts[j + 1], (s - lens[j]) / seg))
    return out


def outward(p, c, flat=True):
    d = p - c
    if flat:
        d = Vector((d.x, d.y, 0.0))
    return d.normalized() if d.length > 1e-6 else Vector((1.0, 0.0, 0.0))


def folds(rows, centres, count, amp, start=0, ease=1.2, phase=0.0, arc=None):
    """Cloth folds: ridges round each ring, growing from row `start` down (sharp crests, round
    hollows). `arc` (radians) for open rows (a cape): the angle the row spans."""
    out = []
    m = len(rows)
    for i, (ring, c) in enumerate(zip(rows, centres)):
        t = 0.0 if i < start else ((i - start + 1) / max(m - start, 1)) ** ease
        n = len(ring)
        new = []
        for k, p in enumerate(ring):
            a = (arc * k / max(n - 1, 1)) if arc else 2 * math.pi * k / n
            w = count * a + phase + 0.5 * math.sin(1.7 * a + i * 0.6)
            r = math.cos(w)
            f = (abs(r) ** 0.6) * (1.0 if r > 0 else -0.7)
            new.append(p + outward(p, c) * f * amp * t)
        out.append(new)
    return out


def ring_at(rows, centres, y):
    k = min(range(len(centres)), key=lambda i: abs(centres[i].z - y))
    return rows[k], centres[k]


def tube(name, pts, r, mat, sides=6, closed=False, radii=None):
    """A tube along a path, each point its own radius (`radii`) if given."""
    if radii is None:
        return W.tube_path(name, [(p.x, p.y, p.z) for p in pts], r, mat, sides=sides, closed=closed)
    pts = [Vector(p) for p in pts]
    n = len(pts)
    bm = bmesh.new()
    rings = []
    prev = None
    for i, p in enumerate(pts):
        if closed:
            t = (pts[(i + 1) % n] - pts[(i - 1) % n]).normalized()
        else:
            t = (pts[min(i + 1, n - 1)] - pts[max(i - 1, 0)]).normalized()
        ref = prev if prev is not None else (Vector((1, 0, 0)) if abs(t.x) < 0.9 else Vector((0, 0, 1)))
        side = (ref - t * ref.dot(t)).normalized()
        up = t.cross(side).normalized()
        prev = side
        rings.append([bm.verts.new(p + (side * math.cos(2 * math.pi * k / sides) + up * math.sin(2 * math.pi * k / sides)) * radii[i][k % len(radii[i])]) for k in range(sides)])
    for i in range(n if closed else n - 1):
        a, b = rings[i], rings[(i + 1) % n]
        for k in range(sides):
            bm.faces.new((a[k], a[(k + 1) % sides], b[(k + 1) % sides], b[k]))
    if not closed:
        bm.faces.new(list(reversed(rings[0])))
        bm.faces.new(rings[-1])
    return W.new_object(name, bm, mat)


def _hash(i, k):
    return (math.sin(i * 12.9898 + k * 78.233) * 43758.5453) % 1.0


def fur_roll(name, pts, r, mat, closed=True, sides=10, lump=0.38):
    """A fat roll of fur along a path: lumpy (clumps), with pointed tufts along its lower edge."""
    radii = []
    for i in range(len(pts)):
        radii.append([r * (1.0 + lump * (_hash(i, k) - 0.35) + 0.25 * math.sin(i * 2.1)) for k in range(sides)])
    parts = [tube(name, pts, r, mat, sides=sides, closed=closed, radii=radii)]
    bm = bmesh.new()
    c = sum(pts, Vector()) / len(pts)
    for i in range(0, len(pts), 2):
        p = pts[i]
        d = outward(p, c)
        tip = p + d * r * 1.1 - UP * r * 1.25
        base = [p + (d * math.cos(a) + UP.cross(d).normalized() * math.sin(a)) * r * 0.55 - UP * r * 0.2 for a in (0.0, 2.1, 4.2)]
        vs = [bm.verts.new(q) for q in base]
        t = bm.verts.new(tip)
        for k in range(3):
            bm.faces.new((vs[k], vs[(k + 1) % 3], t))
    if len(bm.verts):
        parts.append(W.new_object(name + "_tufts", bm, mat))
    else:
        bm.free()
    return parts


def polyline_samples(pts, step, closed=False):
    """(point, tangent, index) every `step` metres along a polyline."""
    path = list(pts) + ([pts[0]] if closed else [])
    out = []
    carry = step * 0.5
    for i, (a, b) in enumerate(zip(path, path[1:])):
        seg = (b - a).length
        if seg < 1e-6:
            continue
        t = (b - a) / seg
        s = carry
        while s < seg:
            out.append((a + t * s, t, i))
            s += step
        carry = s - seg
    return out


def stitches(name, pts, normals, mat, step=0.022, length=0.009, width=0.0028, lift=0.0025, closed=False):
    """Running stitches: small raised dashes along a seam (`normals` per point: outward)."""
    # (each a flat dash just proud of the surface: two triangles, and no outline round it)
    bm = bmesh.new()
    for p, t, i in polyline_samples(pts, step, closed):
        n = normals[min(i, len(normals) - 1)]
        side = n.cross(t).normalized()
        c = p + n * lift
        q = [bm.verts.new(c + t * (dl * length) + side * (dw * width)) for dl, dw in ((-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5))]
        f = bm.faces.new(q)
        f.normal_update()
        if f.normal.dot(n) < 0:
            f.normal_flip()
    # (not through W.new_object: recalculating normals could turn a lone dash over)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    ob.data.materials.append(W.material(mat))
    return ob


def ribbon(name, pts, normals, width, mat, thick=0.008, bevel=0.002, closed=False):
    """A flat strap along a path lying on a surface (`normals`: the surface's, per point)."""
    rows = []
    n = len(pts)
    for k, p in enumerate(pts):
        if closed:
            t = (pts[(k + 1) % n] - pts[(k - 1) % n]).normalized()
        else:
            t = (pts[min(k + 1, n - 1)] - pts[max(k - 1, 0)]).normalized()
        side = t.cross(normals[k]).normalized()
        rows.append([p - side * width / 2, p + side * width / 2])
    if closed:
        rows.append(rows[0])
    ob = A.grid(name, rows, mat)
    A.solidify(ob, thick, 0.0)
    if bevel:
        bev(ob, bevel, 1)
    return ob


def placed(ob, p, n, t=None):
    """Move a part built round the origin (x across, y up its face, z out) onto a surface."""
    z = n.normalized()
    if t is None:
        y = (UP - z * UP.dot(z))
        y = y.normalized() if y.length > 1e-4 else FWD.copy()
        x = y.cross(z).normalized()
    else:
        x = (t - z * t.dot(z)).normalized()
        y = z.cross(x).normalized()
    m = Matrix((x, y, z)).transposed().to_4x4()
    m.translation = p
    ob.matrix_world = m @ ob.matrix_world
    return ob


def buckle(name, p, n, t, mat, w=0.046, h=0.042, bar=0.0075, depth=0.007, tongue=True):
    """A buckle: a bevelled rectangular frame, its bar and prong, lying on a strap."""
    parts = [
        W.box(name + "_f", (w, bar, depth), (0.0, (h - bar) / 2, depth / 2), mat, bevel=0.0018),
        W.box(name + "_f", (w, bar, depth), (0.0, -(h - bar) / 2, depth / 2), mat, bevel=0.0018),
        W.box(name + "_f", (bar, h, depth), ((w - bar) / 2, 0.0, depth / 2), mat, bevel=0.0018),
        W.box(name + "_f", (bar, h, depth), (-(w - bar) / 2, 0.0, depth / 2), mat, bevel=0.0018),
        W.box(name + "_bar", (bar * 0.8, h, depth * 0.8), (-w * 0.08, 0.0, depth * 0.55), mat, bevel=0.0012),
    ]
    if tongue:
        parts.append(W.box(name + "_prong", (w * 0.55, 0.0035, 0.004), (w * 0.17, 0.0, depth * 0.9), mat, bevel=0.0008))
    for ob in parts:
        placed(ob, p, n, t)
    return parts


def rivets(name, pts, normals, mat, r=0.0065):
    return [G.rivet(name, p, n, mat, r) for p, n in zip(pts, normals)]


def button(name, p, n, mat, r=0.011):
    ob = W.cyl(name, r, -0.003, 0.004, mat, verts=12, axis="Z", bevel=0.0022, segments=2)
    return placed(ob, p + n * 0.002, n)


def front_pt(body, x, y, out):
    h = body.front(x, y, out)
    return h if h else (gv(x, y, 0.16 + out), FWD.copy())


def back_pt(body, x, y, out):
    p = body.outer(gv(x, y, 0.0), -FWD, out)
    return (p if p is not None else gv(x, y, -0.16 - out)), -FWD


def hem_trim(prefix, ring, centre, band_mat, pipe_mat, width=0.032, out=0.008, closed=True, stitch_mat=None, pipe_r=0.0055, drop=0.004):
    """A layered hem: a flat band round the bottom edge of a shell (pushed a little proud of it),
    a piping roll along its top and running stitches along its middle."""
    parts = []
    outs = [outward(p, centre) for p in ring]
    top = [p + d * out + UP * width for p, d in zip(ring, outs)]
    bot = [p + d * (out + 0.002) - UP * drop for p, d in zip(ring, outs)]
    rows = [top, bot]
    ob = A.grid(prefix + "_band", rows, band_mat, closed=closed)
    A.solidify(ob, 0.008, -1.0)
    bev(ob, 0.0025)
    parts.append(ob)
    pipe = [p + d * 0.004 for p, d in zip(top, outs)]
    parts.append(tube(prefix + "_pipe", pipe, pipe_r, pipe_mat, sides=6, closed=closed))
    if stitch_mat:
        midline = [(a + b) / 2 + d * 0.0 for a, b, d in zip(top, bot, outs)]
        parts.append(stitches(prefix + "_stitch", midline, outs, stitch_mat, closed=closed))
    return parts


def path_on_front(body, xy, out, samples=14):
    """A path over the front of the body through (x, y) control points: (points, normals)."""
    pts, ns = [], []
    for k in range(samples):
        t = k / (samples - 1)
        seg = t * (len(xy) - 1)
        i = min(int(seg), len(xy) - 2)
        f = seg - i
        x = xy[i][0] + (xy[i + 1][0] - xy[i][0]) * f
        y = xy[i][1] + (xy[i + 1][1] - xy[i][1]) * f
        p, n = front_pt(body, x, y, out)
        pts.append(p)
        ns.append(n)
    return pts, ns


def path_on_back(body, xy, out, samples=14):
    pts, ns = [], []
    for k in range(samples):
        t = k / (samples - 1)
        seg = t * (len(xy) - 1)
        i = min(int(seg), len(xy) - 2)
        f = seg - i
        x = xy[i][0] + (xy[i + 1][0] - xy[i][0]) * f
        y = xy[i][1] + (xy[i + 1][1] - xy[i][1]) * f
        p, n = back_pt(body, x, y, out)
        pts.append(p)
        ns.append(n)
    return pts, ns


def feather(name, base, direction, length, width, mat, spine_mat, curl=0.3, twist=0.0, side=None):
    """A stylised feather: a vane that widens then tapers, notched along its edges, curling back
    along its length, with a raised spine."""
    d = direction.normalized()
    s = side.normalized() if side is not None else d.cross(UP).normalized()
    if s.length < 1e-4:
        s = LEFT.copy()
    up = s.cross(d).normalized()
    n = 10
    spine, rows = [], []
    for k in range(n + 1):
        t = k / n
        p = base + d * (length * t) + up * (curl * length * t * t)
        spine.append(p)
        w = width * math.sin(math.pi * min(t * 1.15, 1.0)) * (0.55 if k % 2 else 1.0) if k else 0.0
        a = twist * t
        ss = s * math.cos(a) + up * math.sin(a)
        rows.append([p - ss * w * 0.5, p, p + ss * w * 0.5])
    ob = A.grid(name, rows, mat)
    A.solidify(ob, 0.004, 0.0)
    parts = [ob, tube(name + "_spine", spine, 0.0035, spine_mat, sides=5)]
    return parts


def lames(prefix, body, s, mats, count=3, top_out=0.05, spread=(-95, 95), along=(-0.06, 0.13)):
    """A shoulder guard of overlapping plates over one upper arm: each plate an arc over the
    top of the arm, edged with a rolled rim and riveted at its ends. mats: (plate, rim, rivet)."""
    plate_mat, rim_mat, rivet_mat = mats
    ua = body.a["upperarm_l" if s > 0 else "upperarm_r"]
    la = body.a["lowerarm_l" if s > 0 else "lowerarm_r"]
    axis = (la - ua).normalized()
    parts = []
    span = (along[1] - along[0]) / count
    for j in range(count):
        t0 = along[0] + span * j * 0.85
        t1 = t0 + span * 1.35
        rows, cs = [], []
        for tt in (t0, (t0 + t1) / 2, t1):
            c = ua + axis * tt
            cs.append(c)
            full = G.ring_around(body, c, axis, UP, 36, top_out + 0.012 * (count - j), 0.05, 0.11)
            arc = []
            for k in range(36):
                a = math.degrees(2 * math.pi * k / 36)
                a = a - 360 if a > 180 else a
                if spread[0] <= a <= spread[1]:
                    arc.append((a, full[k]))
            arc.sort(key=lambda x: x[0])
            rows.append([p for _, p in arc])
        plate = shell(f"{prefix}_lame_{s}{j}", rows, plate_mat, 0.009, closed=False, bevel=0.003, offset=0.0)
        parts.append(plate)
        rim = rows[-1]
        c = cs[-1]
        parts.append(tube(f"{prefix}_lame_rim_{s}{j}", [p + outward(p, c, False) * 0.004 for p in rim], 0.0055, rim_mat, sides=6))
        for p in (rim[1], rim[-2]):
            parts.append(G.rivet(f"{prefix}_lame_rivet_{s}{j}", p + outward(p, c, False) * 0.006, outward(p, c, False), rivet_mat, 0.007))
    return parts
