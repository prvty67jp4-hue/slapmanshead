"""The Varrow wardrobe's outfits (Fashionscape, app/src/hub.rs), fitted to each of the four
fighters' own bodies, for the game to skin onto their skeleton.

    blender --background --factory-startup --python app/tools/blender/build_outfits.py [-- herald smuggler pilgrim] [--only ranger] [--no-renders]

The look: halfway between chunky stylised MMO gear and our toon fighters. Big readable shapes,
but made like real clothes and kit: bevelled edges, cloth that folds and hangs, layered hems
(a band, gold piping, stitching), straps with real buckles, rivets and studs, fur, plates over
plates. Each piece mixes real materials (outfit_kit.MATERIALS): dyed cloth, leather and metal in
its three dye zones (coat, trim, accent), and leather, iron, brass, linen, wool, fur, rope and
wood in their own colours, so a dye tints the cloth or the gilding, never the whole outfit.

Sets (each piece is a mesh named <prefix>_<slot>; the game hides the fighter under the body,
legs, hands and feet pieces, so those always cover their part fully):

- Court Herald (hr_): a soft pleated bonnet with a gilt-piped brim, a jewelled brooch and three
  curling plumes; a back half-cape with a quartered crest, gold-clasped at the shoulders; a chain
  of office of gilt plates and a medallion; a quartered tabard over a dark wool doublet, gilt
  epaulettes with fringes, slashed puffed sleeves and a buckled belt with a pouch; paned trunk
  hose over hose with garters; stiff gauntlet cuffs over dark gloves; buckled court shoes.
- River Smuggler (sm_): an oilskin sou'wester with stitched seams, a buckled band and a chin
  strap; an oilcloth capelet with a fur collar, its hood rolled down, wooden toggles; a knotted
  neckerchief with a brass whistle; a long oilskin coat over a buttoned waistcoat, a bandolier
  with pouches, a leather shoulder guard, a wide belt; breeches with leather knee patches over
  ribbed stockings; fingerless gloves; tall wading boots with folded cuffs and buckled straps.
- Ash Pilgrim (ap_): a deep pleated hood with a liripipe and a wooden pilgrim's token; a long
  ragged mantle with a grey fur collar and a rope clasp; censer beads with a smouldering censer
  (it glows); a wrapped robe with bell sleeves, a rope belt with tassels and a satchel; loose
  trousers cross-bound to the knee with leather kneeling pads; bandaged hands; sandals over
  wrapped feet.

Writes app/assets/characters/look_<set>_<fighter>.glb, app/assets/characters/look_<set>.png (the
four fighters' bakes, a quarter each) and pictures to app/shots/character-art/outfits/.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bmesh  # noqa: E402
import bpy  # noqa: E402
from mathutils import Matrix, Vector  # noqa: E402

import build_gear as G  # noqa: E402  (the gear's fitting helpers and pictures)
import outfit_kit as K  # noqa: E402  (the outfits' building blocks and materials)

A, W, T = G.A, G.W, G.T
gv, UP, FWD, LEFT = G.gv, G.UP, G.FWD, G.LEFT
OUT, REF, NAMES, QUARTER, TEX = G.OUT, G.REF, G.NAMES, G.QUARTER, G.TEX
SHOTS = os.path.join(G.APP, "shots", "character-art", "outfits")

# each set's own colours for the pictures (they match the set's default dyes in hub.rs)
SET_DYES = {
    "herald": K.preview_dyes((0.05, 0.10, 0.35), (0.62, 0.42, 0.14), (0.50, 0.03, 0.06)),
    "smuggler": K.preview_dyes((0.03, 0.14, 0.05), (0.62, 0.42, 0.14), (0.40, 0.07, 0.04)),
    "pilgrim": K.preview_dyes((0.22, 0.23, 0.25), (0.55, 0.45, 0.30), (0.80, 0.30, 0.05), (0.95, 0.42, 0.08)),
}


# ======================================================================== shared fitting

def head_frame(body):
    """The head as measured: (centre at the brow, brow height, widest radius there, height to
    the top of whatever is on it)."""
    hc = body.a["Head"]
    c = Vector((0.0, hc.y + 0.02, 0.0))
    rim = hc.z + 0.145
    widest = 0.08
    for k in range(24):
        a = 2 * math.pi * k / 24
        d = (FWD * math.cos(a) + LEFT * math.sin(a)).normalized()
        for dy in (0.0, 0.04, 0.08):
            cen = Vector((0.0, c.y, rim + dy))
            p = body.outer(cen, d)
            if p is not None and (p - cen).length < 0.22:
                widest = max(widest, (p - cen).length)
    top_hit = body.ray(Vector((0.0, c.y, hc.z + 0.6)), -UP)
    top = top_hit[0].z if top_hit is not None and top_hit[0].z > rim else hc.z + 0.22
    return Vector((0.0, c.y, rim)), rim, widest + 0.03, max(top - rim + 0.035, 0.08)


def ellipse(c, rx, rz, n, y=None):
    """A ring of points round c (x across, z front to back) at height y."""
    y = c.z if y is None else y
    return [Vector((c.x + math.sin(2 * math.pi * k / n) * rx, c.y - math.cos(2 * math.pi * k / n) * rz, y)) for k in range(n)]


def dome(name, centre, radii, mat, keep=-0.05, u=32, v=14, pleats=0, pleat_amp=0.0, tilt=(0.0, 0.0)):
    """A dome (a sphere cut at `keep`), optionally pleated round its side (soft gathered cloth),
    tilted (x, y radians)."""
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=u, v_segments=v, radius=1.0)
    gone = [vv for vv in bm.verts if vv.co.z < keep]
    bmesh.ops.delete(bm, geom=gone, context="VERTS")
    for vv in bm.verts:
        if pleats:
            a = math.atan2(vv.co.y, vv.co.x)
            side = max(0.0, 1.0 - abs(vv.co.z))
            s = 1.0 + pleat_amp * side * (abs(math.cos(pleats * a / 2)) ** 0.6 * 2 - 1)
            vv.co.x *= s
            vv.co.y *= s
    bmesh.ops.scale(bm, vec=radii, verts=bm.verts)
    ob = W.new_object(name, bm, mat)
    ob.matrix_world = Matrix.Translation(centre) @ Matrix.Rotation(tilt[0], 4, "X") @ Matrix.Rotation(tilt[1], 4, "Y")
    return ob


def slab(name, ring, depth, mat, bevel=0.004):
    """A flat slab: the outline `ring` (a sole, a patch) extruded `depth` down (along -z)."""
    bm = bmesh.new()
    vs = [bm.verts.new(p) for p in ring]
    face = bm.faces.new(vs)
    ext = bmesh.ops.extrude_face_region(bm, geom=[face])
    moved = [g for g in ext["geom"] if isinstance(g, bmesh.types.BMVert)]
    bmesh.ops.translate(bm, verts=moved, vec=(0, 0, -depth))
    ob = W.new_object(name, bm, mat)
    if bevel:
        K.bev(ob, bevel, 2)
    return ob


def torso(body, n=32, ys=(1.43, 1.38, 1.31, 1.24, 1.17, 1.10, 1.03, 0.97), out=0.05):
    rows, centres = [], []
    depth = body.a["spine_02"].y
    for y in ys:
        c = Vector((0.0, depth, y))
        centres.append(c)
        rows.append(G.ring_around(body, c, UP, FWD, n, out, 0.09, 0.24))
    return G.fair_rows(rows, centres, 0.75, 2), centres


def skirt(rows, drops, flare=0.07):
    hip = rows[-1]
    hc = sum(hip, Vector()) / len(hip)
    srows, scs = [], []
    for k, drop in enumerate(drops):
        f = 1.0 + flare * (k + 1)
        srows.append([hc + (p - hc) * f - UP * drop for p in hip])
        scs.append(hc - UP * drop)
    return srows, scs


def cut(ob, keep):
    """Delete the faces whose centre `keep(centre)` says no to (an opening, a slit)."""
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    gone = [f for f in bm.faces if not keep(f.calc_center_median())]
    bmesh.ops.delete(bm, geom=gone, context="FACES")
    bm.to_mesh(ob.data)
    bm.free()
    return ob


def yoke(body, prefix, top_row, c0, mat, thick=0.016):
    """Over the shoulders from the top of the torso shell in to the neck."""
    neck = body.a["neck_01"]
    ny = min(max(neck.z + 0.02, 1.48), 1.56)
    yrows = [list(top_row), [Vector((c0.x + (p.x - c0.x) * 0.92, neck.y + (p.y - neck.y) * 0.8, 1.475)) for p in top_row]]
    ring = []
    for p in top_row:
        d = Vector(((p - c0).x, (p - c0).y, 0.0)).normalized()
        ring.append(Vector((neck.x + d.x * 0.095, neck.y + d.y * 0.085, ny)))
    yrows.append(ring)
    return K.shell(f"{prefix}_yoke", yrows, mat, thick), ring


def sleeve(body, s, length, n=20, puff=0.0, out=0.022, start=-0.04, flare=0.0):
    ua = body.a["upperarm_l" if s > 0 else "upperarm_r"]
    la = body.a["lowerarm_l" if s > 0 else "lowerarm_r"]
    axis = (la - ua).normalized()
    ts = [start + (length - start) * k / 4 for k in range(5)]
    cs = [ua + axis * t for t in ts]
    rows = []
    for k, c in enumerate(cs):
        bulge = puff * math.sin(math.pi * k / 4)
        gather = -0.01 if puff and k in (0, 4) else 0.0
        rows.append(G.ring_around(body, c, axis, UP, n, out + bulge + gather + flare * (k / 4) ** 2, 0.045, 0.07))
    return G.fair_rows(rows, cs, 0.9, 2), cs, axis


def leg_rows(body, s, table, ys, n=24):
    thigh = body.a["thigh_l" if s > 0 else "thigh_r"]
    calf = body.a["calf_l" if s > 0 else "calf_r"]
    rows, centres = [], []
    for y in ys:
        t = (thigh.z - y) / max(thigh.z - calf.z, 0.01)
        cx = thigh.x + (calf.x - thigh.x) * min(max(t, 0.0), 1.0)
        cy = thigh.y + (calf.y - thigh.y) * min(max(t, 0.0), 1.0)
        c = Vector((cx, cy, y))
        r = table[-1][1]
        for (ya, ra), (yb, rb) in zip(table, table[1:]):
            if yb <= y <= ya:
                r = rb + (ra - rb) * (y - yb) / (ya - yb)
        if y > table[0][0]:
            r = table[0][1]
        centres.append(c)
        rows.append([c + FWD * (math.cos(2 * math.pi * k / n) * r * 0.92) + LEFT * (math.sin(2 * math.pi * k / n) * r) for k in range(n)])
    rows = keep_side(rows, s, 0.022)
    return rows, centres


def keep_side(rows, s, margin):
    """No point of a leg's or a foot's rings nearer the middle than `margin` (on its own side)."""
    return [[Vector((p.x if s * p.x > margin else s * margin, p.y, p.z)) for p in r] for r in rows]


def wrinkle(rows, centres, at, amp, width=0.03):
    """Bunched cloth: rings near each height in `at` pushed out (a soft ridge round the leg)."""
    out = []
    for ring, c in zip(rows, centres):
        f = sum(amp * math.exp(-((c.z - y) / width) ** 2) for y in at)
        out.append([p + K.outward(p, c) * f for p in ring])
    return out


def ring_band(prefix, rows, centres, y, mat, scale=1.06, r=0.012):
    ring, c = K.ring_at(rows, centres, y)
    return W.tube_path(prefix, [(c + (p - c) * scale).to_tuple() for p in ring], r, mat, sides=6, closed=True)


def flat_band(prefix, rows, centres, y, width, mat, out=0.006, buckle_mat=None, stitch_mat=None):
    """A flat strap round a tube at height y, optionally buckled at the front and stitched."""
    ring, c = K.ring_at(rows, centres, y)
    outs = [K.outward(p, c) for p in ring]
    pts = [p + d * out for p, d in zip(ring, outs)]
    if abs(c.x) > 0.03:
        # (round a leg or a foot: it stays on its own side of the middle)
        pts = keep_side([pts], 1 if c.x > 0 else -1, 0.022)[0]
    rows2 = [[p + UP * width / 2 for p in pts], [p - UP * width / 2 for p in pts]]
    ob = A.grid(prefix, rows2, mat, closed=True)
    A.solidify(ob, 0.006, 1.0)
    K.bev(ob, 0.002, 1)
    parts = [ob]
    if buckle_mat:
        k = min(range(len(pts)), key=lambda i: pts[i].y)
        t = (pts[(k + 1) % len(pts)] - pts[k - 1]).normalized()
        parts += K.buckle(prefix + "_buckle", pts[k] + outs[k] * 0.006, outs[k], t, buckle_mat, w=width * 1.3, h=width * 1.25)
    if stitch_mat:
        for dy in (width * 0.32, -width * 0.32):
            parts.append(K.stitches(prefix + "_st", [p + UP * dy + d * 0.006 for p, d in zip(pts, outs)], outs, stitch_mat, closed=True, step=0.016))
    return parts


def boot_rows(body, s, top, n=24, round_=0.75):
    foot = body.a["foot_l" if s > 0 else "foot_r"]
    ball = body.a["ball_l" if s > 0 else "ball_r"]
    calf = body.a["calf_l" if s > 0 else "calf_r"]
    mid = (foot + ball) / 2
    ys = [y for y in (0.52, 0.45, 0.38, 0.30, 0.25, 0.20, 0.15, 0.11, 0.075, 0.045, 0.02) if y <= top + 0.001]
    rows, centres = [], []
    for y in ys:
        if y > 0.3:
            t = (y - 0.3) / max(calf.z - 0.3, 0.05)
            c = Vector((foot.x + (calf.x - foot.x) * t, foot.y + (calf.y - foot.y) * t, y))
        else:
            c = Vector((foot.x, mid.y if y < 0.13 else foot.y, y))
        centres.append(c)
        rows.append(G.ring_around(body, c, UP, FWD, n, 0.03, 0.045, 0.16, own_side=True))
    rows = G.fair_rows(rows, centres, round_)
    # (each foot well clear of the middle: the two must never touch, or the game joins them)
    rows = keep_side(rows, s, 0.03)
    return rows, centres, foot


def boot(prefix, s, rows, centres, mat, sole_mat, heel=True, sole_depth=0.022):
    """A boot from its rings: the upper, closed at the bottom, a thicker sole and a heel."""
    last = rows[-1]
    lc = sum(last, Vector()) / len(last)
    up_rows = rows + [[lc + (p - lc) * 0.25 - UP * 0.004 for p in last]]
    parts = [K.shell(f"{prefix}_upper_{s}", up_rows, mat, 0.012, bevel=0.003)]
    outline = [lc + (p - lc) * 1.07 for p in last]
    outline = keep_side([[Vector((p.x, p.y, last[0].z + 0.004)) for p in outline]], s, 0.025)[0]
    parts.append(slab(f"{prefix}_sole_{s}", outline, sole_depth, sole_mat))
    if heel:
        back = [p for p in outline if p.y > lc.y + 0.02]
        if len(back) > 3:
            hc = sum(back, Vector()) / len(back)
            ring = [hc + (p - hc) * 0.85 for p in back]
            ring = K.resample(ring, 10)
            parts.append(slab(f"{prefix}_heel_{s}", [Vector((p.x, p.y, outline[0].z - sole_depth + 0.002)) for p in ring], 0.02, sole_mat, 0.003))
    # stitching round the welt, just above the sole
    welt = [lc + (p - lc) * 1.03 + UP * 0.008 for p in last]
    parts.append(K.stitches(f"{prefix}_welt_{s}", welt, [K.outward(p, lc) for p in welt], "linen", closed=True, step=0.014))
    return parts


def glove_rows(body, s, cuff, fingers=True, n=18):
    hand = body.a["hand_l" if s > 0 else "hand_r"]
    lower = body.a["lowerarm_l" if s > 0 else "lowerarm_r"]
    axis = (hand - lower).normalized()
    rows, cs = [], []
    for t, flare in ((-0.16, cuff), (-0.11, cuff * 0.75), (-0.05, cuff * 0.4), (-0.01, 0.012)):
        cs.append(hand + axis * t)
        rows.append(G.ring_around(body, cs[-1], axis, UP, n, 0.012 + flare, 0.04, 0.08))
    ends = (0.03, 0.07, 0.11, 0.15, 0.185) if fingers else (0.03, 0.07, 0.105)
    for t in ends:
        cs.append(hand + axis * t)
        rows.append(G.ring_around(body, cs[-1], axis, UP, n, 0.014, 0.02, 0.07))
    if fingers:
        tip = hand + axis * 0.205
        rows.append([tip + (p - tip) * 0.35 for p in rows[-1]])
        cs.append(tip)
    return rows, cs, hand, axis


def glove(prefix, s, rows, mat):
    ob = K.shell(f"{prefix}_glove_{s}", rows, mat, 0.011, bevel=0.0025)
    return ob


def back_cape_rows(body, bottom=1.0, arc=(100, 260), n=24, flare=0.08, out=0.06):
    rows, centres = [], []
    depth = body.a["spine_02"].y
    neck = body.a["neck_01"]
    hs = [h for h in (1.50, 1.46, 1.40, 1.32, 1.24, 1.15, 1.05, 0.95, 0.85, 0.75, 0.65, 0.56) if h >= bottom - 0.001]
    for k, y in enumerate(hs):
        c = Vector((0.0, depth if y < 1.45 else neck.y, y))
        row = []
        for j in range(n):
            a = math.radians(arc[0] + (arc[1] - arc[0]) * j / (n - 1))
            d = (FWD * math.cos(a) + LEFT * math.sin(a)).normalized()
            p = body.outer(c, d)
            r = (p - c).length if p is not None else 0.16
            r = min(max(r, 0.08), 0.26 if y < 1.45 else 0.11)
            row.append(c + d * (r * (1.0 + flare * k) + out))
        rows.append(row)
        centres.append(c)
    return rows, centres


def cape_edges(rows, centres, prefix, band_mat, pipe_mat, stitch_mat=None, width=0.03):
    """A trimmed hem along the bottom of an open cape (rows) and its two side edges."""
    parts = K.hem_trim(prefix + "_hem", rows[-1], centres[-1], band_mat, pipe_mat, width=width, closed=False, stitch_mat=stitch_mat)
    for side in (0, -1):
        edge = [r[side] for r in rows]
        parts.append(K.tube(prefix + f"_edge{side}", edge, 0.0065, pipe_mat, sides=6))
    return parts


def chest_arc(body, y_mid, y_end, x_end, out, n=12):
    pts, ns = [], []
    for k in range(n + 1):
        t = k / n
        x = -x_end + 2 * x_end * t
        y = y_end + (y_mid - y_end) * math.sin(math.pi * t)
        p, nn = K.front_pt(body, x, y, out)
        pts.append(p)
        ns.append(nn)
    return pts, ns


def panel(body, prefix, xs, ys, out, mat, side="front", hang_below=0.98, thick=0.01):
    """A cloth panel over the front (or back) of the body, hanging straight below `hang_below`."""
    rows = []
    for y in ys:
        row = []
        for x in xs:
            yy = max(y, hang_below)
            p, n = (K.front_pt if side == "front" else K.back_pt)(body, x, yy, out)
            if y < hang_below:
                p = Vector((p.x, p.y + (-1 if side == "front" else 1) * 0.008 * (hang_below - y) / 0.1, y))
            row.append(p)
        rows.append(row)
    return rows, K.shell(prefix, rows, mat, thick, closed=False, bevel=0.003, offset=0.0)


def outline_of(rows):
    """The edge of a grid of rows: down the left, along the bottom, up the right, along the top."""
    left = [r[0] for r in rows]
    bottom = rows[-1][1:]
    right = [r[-1] for r in reversed(rows[:-1])]
    top = list(reversed(rows[0][1:-1]))
    return left + bottom + right + top


def crest(prefix, p, n, fill_a, fill_b, rim_mat, size=0.075):
    """A heraldic shield: quartered, its rim raised and gilt, a stud in the middle."""
    s = size
    shape = [(-s, s), (s, s), (s, -0.25 * s), (0.0, -1.25 * s), (-s, -0.25 * s)]
    parts = []
    rim = W.top_profile(prefix + "_rim", [(x * 1.18, y * 1.15 + 0.004) for x, y in shape], 0.008, rim_mat, z=0.0, bevel=0.002)
    parts.append(rim)
    halves = [
        ([(-s, s), (0.0, s), (0.0, 0.0), (-s, 0.0)], fill_a),
        ([(0.0, s), (s, s), (s, 0.0), (0.0, 0.0)], fill_b),
        ([(-s, 0.0), (0.0, 0.0), (0.0, -1.25 * s), (-s, -0.25 * s)], fill_b),
        ([(0.0, 0.0), (s, 0.0), (s, -0.25 * s), (0.0, -1.25 * s)], fill_a),
    ]
    for pts, m in halves:
        parts.append(W.top_profile(prefix + "_q", pts, 0.006, m, z=0.006, bevel=0.0015))
    parts.append(W.cyl(prefix + "_boss", 0.014, 0.008, 0.016, rim_mat, verts=12, axis="Z", bevel=0.003, segments=2))
    for ob in parts:
        K.placed(ob, p, n)
    return parts


def chain_links(prefix, pts, mat, size=0.012, plate_every=0, plate_mat=None):
    parts = []
    for k, q in enumerate(pts):
        if plate_every and k % plate_every == 0:
            t = (pts[min(k + 1, len(pts) - 1)] - pts[max(k - 1, 0)]).normalized()
            n = t.cross(LEFT).cross(t) if abs(t.x) < 0.9 else FWD
            ob = W.box(prefix + "_plate", (size * 2.6, size * 1.9, size * 0.6), (0, 0, 0), plate_mat or mat, bevel=size * 0.25, segments=2)
            K.placed(ob, q, -FWD if n.dot(FWD) > 0 else FWD, t)
            parts.append(ob)
        else:
            parts.append(W.ring(prefix + "_link", size, size * 0.3, q.to_tuple(), mat, axis="X" if k % 2 else "Z", maj=10, minr=4))
    return parts


# ============================================================================ Court Herald

def herald_head(body):
    c, rim, R, H = head_frame(body)
    parts = []
    # the bonnet: a soft, gathered crown, wider than the head, tipped to the right
    crown = dome("hr_crown", Vector((0.0, c.y + 0.005, rim + 0.005)), (R * 1.32, R * 1.36, H * 0.62), "dye_coat_cloth", keep=-0.1, pleats=16, pleat_amp=0.06, tilt=(0.0, math.radians(-8)))
    A.solidify(crown, 0.012, 1.0)
    K.bev(crown, 0.003)
    parts.append(crown)
    # a stiff brim, gilt-piped, and a band of the accent round the crown's base, stitched in gold
    inner = ellipse(Vector((0.0, c.y, rim - 0.006)), R * 1.0, R * 1.05, 32)
    outer = [p + K.outward(p, Vector((0.0, c.y, p.z))) * 0.04 - UP * 0.004 for p in inner]
    parts.append(K.shell("hr_brim", [inner, outer], "dye_coat_cloth", 0.012, bevel=0.003, offset=0.0))
    parts.append(K.tube("hr_brim_pipe", [p + K.outward(p, Vector((0.0, c.y, p.z))) * 0.004 for p in outer], 0.0065, "dye_trim_metal", sides=6, closed=True))
    band = ellipse(Vector((0.0, c.y, rim + 0.02)), R * 1.03, R * 1.08, 32)
    parts.append(K.shell("hr_hatband", [[p + UP * 0.016 for p in band], [p - UP * 0.012 for p in band]], "dye_accent_cloth", 0.008, bevel=0.002, offset=1.0))
    parts.append(K.stitches("hr_hatband_st", [p + K.outward(p, Vector((0.0, c.y, p.z))) * 0.009 for p in band], [K.outward(p, Vector((0.0, c.y, p.z))) for p in band], "dye_trim_metal", closed=True, step=0.014))
    # a jewelled brooch on the right side, and three plumes curling up and back from it
    side_p = Vector((-R * 1.06, c.y + 0.005, rim + 0.025))
    sn = Vector((-1.0, 0.0, 0.25)).normalized()
    brooch = [W.cyl("hr_brooch", 0.024, -0.004, 0.004, "dye_trim_metal", verts=16, axis="Z", bevel=0.003, segments=2),
              W.ring("hr_brooch_rim", 0.022, 0.0035, (0, 0, 0.004), "dye_trim_metal", axis="Z", maj=16, minr=5),
              W.sphere("hr_brooch_gem", 0.012, (0, 0, 0.007), "dye_accent_metal", seg=10, rings=6, scale=(1, 1, 0.7))]
    for ob in brooch:
        K.placed(ob, side_p, sn)
        parts.append(ob)
    for k, (mat, ln, wd, dx, curl) in enumerate((("dye_accent_cloth", 0.25, 0.07, 0.0, 0.45), ("feather", 0.21, 0.06, 0.02, 0.35), ("feather", 0.18, 0.05, -0.015, 0.55))):
        d = (UP * 0.75 - FWD * (0.6 + 0.12 * k) - LEFT * (0.25 + dx * 4)).normalized()
        parts += K.feather(f"hr_plume{k}", side_p + UP * 0.01 - FWD * 0.01 * k, d, ln, wd, mat, "dye_trim_metal", curl=curl, twist=0.6 - 0.3 * k)
    return W.bake(parts, "hr_head")


def herald_cape(body):
    rows, centres = back_cape_rows(body, bottom=1.05, arc=(70, 290), n=28, flare=0.05)
    rows = K.folds(rows, centres, 4.5, 0.022, start=2, arc=math.radians(220))
    parts = [K.shell("hr_cape", rows, "dye_coat_cloth", 0.013, closed=False, bevel=0.003)]
    parts += cape_edges(rows, centres, "hr_cape", "dye_accent_cloth", "dye_trim_metal", stitch_mat="dye_trim_metal")
    # the crest on the back
    bp = body.outer(gv(0.0, 1.26, 0.0), -FWD, 0.105)
    bp = bp if bp is not None else gv(0.0, 1.26, -0.2)
    parts += crest("hr_cape_crest", bp, -FWD, "dye_accent_cloth", "dye_coat_cloth", "dye_trim_metal", 0.072)
    # gold rosette clasps on the front of each shoulder, a chain slung between them
    clasps = []
    for s in (1, -1):
        p, n = K.front_pt(body, s * 0.13, 1.42, 0.06)
        clasps.append(p)
        for ob in (W.cyl(f"hr_clasp_{s}", 0.022, -0.004, 0.005, "dye_trim_metal", verts=14, axis="Z", bevel=0.003, segments=2),
                   W.sphere(f"hr_clasp_gem_{s}", 0.009, (0, 0, 0.007), "dye_accent_metal", seg=8, rings=5)):
            K.placed(ob, p, n)
            parts.append(ob)
    if len(clasps) == 2:
        a, b = clasps
        sag = [a.lerp(b, t / 10) - UP * 0.04 * math.sin(math.pi * t / 10) - FWD * 0.012 for t in range(1, 10)]
        parts += chain_links("hr_cape_chain", sag, "dye_trim_metal", 0.008)
    return W.bake(parts, "hr_cape")


def herald_neck(body):
    pts, ns = chest_arc(body, 1.31, 1.44, 0.11, 0.105, n=14)
    parts = chain_links("hr_chain", pts, "dye_trim_metal", 0.011, plate_every=2, plate_mat="dye_trim_metal")
    p, n = K.front_pt(body, 0.0, 1.25, 0.12)
    medal = [W.cyl("hr_medal", 0.045, -0.006, 0.006, "dye_trim_metal", verts=20, axis="Z", bevel=0.003, segments=2),
             W.ring("hr_medal_rim", 0.04, 0.0045, (0, 0, 0.006), "dye_trim_metal", axis="Z", maj=20, minr=5),
             W.ring("hr_medal_inner", 0.026, 0.0025, (0, 0, 0.007), "dye_trim_metal", axis="Z", maj=16, minr=4),
             W.sphere("hr_medal_gem", 0.017, (0, 0, 0.009), "dye_accent_metal", seg=12, rings=7, scale=(1, 1, 0.65)),
             W.sphere("hr_medal_pearl", 0.009, (0, -0.056, 0.0), "feather", seg=8, rings=6)]
    for ob in medal:
        K.placed(ob, p, n)
        parts.append(ob)
    return W.bake(parts, "hr_neck")


def herald_body(body):
    parts = []
    rows, centres = torso(body, n=32)
    srows, scs = skirt(rows, (0.05, 0.10, 0.15), 0.05)
    allrows = rows + srows
    allc = centres + scs
    allrows = K.folds(allrows, allc, 7, 0.012, start=len(rows))
    # the dark wool doublet and its short pleated skirt
    parts.append(K.shell("hr_doublet", allrows, "wool_dark", 0.016))
    yk, neck_ring = yoke(body, "hr", rows[0], centres[0], "wool_dark")
    parts.append(yk)
    # a standing collar, gilt-piped
    col_rows = [[p + UP * 0.0 for p in neck_ring], [K.outward(p, body.a["neck_01"]) * 0.006 + p + UP * 0.035 for p in neck_ring]]
    parts.append(K.shell("hr_collar", col_rows, "wool_dark", 0.01, bevel=0.002, offset=1.0))
    parts.append(K.tube("hr_collar_pipe", col_rows[1], 0.005, "dye_trim_metal", sides=6, closed=True))
    # slashed, puffed sleeves in the accent, a gilt band at their ends
    for s in (1, -1):
        srs, cs, axis = sleeve(body, s, 0.2, n=24, puff=0.045)
        srs = [[p + K.outward(p, c, False) * 0.008 * abs(math.cos(3 * 2 * math.pi * k / len(r))) for k, p in enumerate(r)] for r, c in zip(srs, cs)]
        parts.append(K.shell(f"hr_sleeve_{s}", srs, "dye_accent_cloth", 0.013))
        end, ce = srs[-1], cs[-1]
        parts.append(K.tube(f"hr_sleeve_band_{s}", [ce + (p - ce) * 1.04 for p in end], 0.008, "dye_trim_metal", sides=6, closed=True))
        # panes of the coat colour over the puff (the accent shows between them)
        for j in range(6):
            a = j * 4
            strip = [[cc + (r[q % len(r)] - cc) * 1.035 for q in range(a, a + 3)] for r, cc in zip(srs, cs)]
            parts.append(K.shell(f"hr_pane_{s}{j}", strip, "dye_coat_cloth", 0.006, closed=False, bevel=0.0015, offset=1.0))
        # a gilt epaulette with its fringe
        # (on the sleeve's own top: a ray down from above can miss a sloping shoulder)
        top = srs[1][0] + K.outward(srs[1][0], cs[1], False) * 0.02
        if top is not None:
            ep = dome(f"hr_epaulette_{s}", top - UP * 0.025, (0.075, 0.07, 0.03), "dye_trim_metal", keep=-0.15)
            A.solidify(ep, 0.008, 1.0)
            K.bev(ep, 0.002)
            parts.append(ep)
            edge = [top - UP * 0.03 + Vector((math.cos(a) * 0.072, math.sin(a) * 0.066, 0.0)) for a in [2 * math.pi * q / 18 for q in range(18)]]
            parts.append(K.tube(f"hr_epaulette_rim_{s}", edge, 0.006, "dye_trim_metal", sides=6, closed=True))
            for q, e in enumerate(edge):
                if K.outward(e, top).x * s > -0.2:
                    parts.append(K.tube(f"hr_fringe_{s}", [e, e - UP * 0.045 + K.outward(e, top) * 0.012], 0.0035, "dye_trim_metal", sides=4))
    # the tabard: quartered panels front and back, gilt-piped, a crest on the chest
    for side in ("front", "back"):
        xs = [-0.155 + 0.031 * i for i in range(11)]
        ys = [1.42 - 0.05 * j for j in range(15)]
        prow, _ = panel(body, f"hr_tabard_{side}_ref", xs, ys, 0.075, "dye_coat_cloth", side)
        bpy.data.objects.remove(_, do_unlink=True)
        # dagged hem: the bottom row scalloped
        last = prow[-1]
        prow[-1] = [Vector((p.x, p.y, p.z - 0.022 * abs(math.sin(math.pi * k / 2.5)))) for k, p in enumerate(last)]
        h, w = len(prow) // 2, len(xs) // 2
        quarters = [([r[:w + 1] for r in prow[:h + 1]], "dye_accent_cloth"), ([r[w:] for r in prow[:h + 1]], "dye_coat_cloth"),
                    ([r[:w + 1] for r in prow[h:]], "dye_coat_cloth"), ([r[w:] for r in prow[h:]], "dye_accent_cloth")]
        for qi, (q, m) in enumerate(quarters):
            parts.append(K.shell(f"hr_tabard_{side}{qi}", q, m, 0.01, closed=False, bevel=0.0025, offset=0.0))
        edge = outline_of(prow)
        nrm = FWD if side == "front" else -FWD
        parts.append(K.tube(f"hr_tabard_pipe_{side}", [p + nrm * 0.004 for p in edge] + [edge[0] + nrm * 0.004], 0.0065, "dye_trim_metal", sides=6))
        inner = [prow[1][1]] + [r[1] for r in prow[2:-1]]
        parts.append(K.stitches(f"hr_tabard_st_{side}", [p + nrm * 0.006 for p in inner], [nrm] * len(inner), "dye_trim_metal", step=0.02))
        inner2 = [r[-2] for r in prow[1:-1]]
        parts.append(K.stitches(f"hr_tabard_st2_{side}", [p + nrm * 0.006 for p in inner2], [nrm] * len(inner2), "dye_trim_metal", step=0.02))
        if side == "front":
            cp, cn = K.front_pt(body, 0.0, 1.27, 0.095)
            parts += crest("hr_crest", cp, cn, "dye_coat_cloth", "dye_accent_cloth", "dye_trim_metal", 0.06)
    # the belt over the tabard, a gilt buckle, a leather pouch on the right hip
    bring, bc = rows[-1], centres[-1]
    belt_pts = [bc + (p - bc) * 1.0 + K.outward(p, bc) * 0.022 + UP * 0.03 for p in bring]
    belt_ns = [K.outward(p, bc) for p in belt_pts]
    parts.append(K.ribbon("hr_belt", [Vector((p.x, p.y, p.z)) for p in belt_pts], belt_ns, 0.04, "leather_dark", 0.008, closed=True))
    k = min(range(len(belt_pts)), key=lambda i: belt_pts[i].y)
    t = (belt_pts[(k + 1) % len(belt_pts)] - belt_pts[k - 1]).normalized()
    parts += K.buckle("hr_belt_buckle", belt_pts[k] + belt_ns[k] * 0.006, belt_ns[k], t, "dye_trim_metal", w=0.058, h=0.052)
    kp = min(range(len(belt_pts)), key=lambda i: abs(K.outward(belt_pts[i], bc).dot(-LEFT) - 0.75) + (0 if K.outward(belt_pts[i], bc).y < 0.2 else 1))
    pp, pn = belt_pts[kp] - UP * 0.06 + belt_ns[kp] * 0.02, belt_ns[kp]
    parts += pouch("hr_pouch", pp, pn, "leather_brown", "dye_trim_metal")
    return W.bake(parts, "hr_body")


def pouch(prefix, p, n, mat, stud_mat, w=0.08, h=0.07, d=0.035):
    """A belt pouch: a rounded box, its flap with a stitched edge and a stud."""
    parts = [W.box(prefix, (w, h, d), (0.0, 0.0, d / 2), mat, bevel=0.009, segments=2),
             W.box(prefix + "_flap", (w * 1.04, h * 0.45, 0.008), (0.0, h * 0.3, d + 0.003), mat, bevel=0.003, segments=2),
             W.cyl(prefix + "_stud", 0.008, d + 0.006, d + 0.011, stud_mat, verts=10, axis="Z", bevel=0.002)]
    parts[-1].location = (0.0, h * 0.12, 0.0)
    bpy.context.view_layer.update()
    for ob in parts:
        ob.data.transform(ob.matrix_basis)
        ob.matrix_basis = Matrix.Identity(4)
        K.placed(ob, p, n)
    return parts


def herald_legs(body):
    parts = []
    for s in (1, -1):
        # paned trunk hose: a bulbous base, strips of the two colours over it
        rows, cs = leg_rows(body, s, ((0.97, 0.115), (0.88, 0.125), (0.80, 0.115), (0.74, 0.088), (0.70, 0.08), (0.0, 0.08)), [0.96, 0.91, 0.86, 0.81, 0.76, 0.73], n=24)
        parts.append(K.shell(f"hr_trunk_{s}", rows, "wool_dark", 0.012))
        for j in range(6):
            a, b = int(j * 4), int(j * 4 + 3)
            strip = [[cc + (r[q] - cc) * 1.04 for q in range(a, b + 1)] for r, cc in zip(rows, cs)]
            parts.append(K.shell(f"hr_pane_{s}{j}", strip, "dye_accent_cloth" if j % 2 else "dye_coat_cloth", 0.008, closed=False, bevel=0.002, offset=1.0))
        hem = [cs[-1] + (p - cs[-1]) * 1.05 for p in rows[-1]]
        parts.append(K.tube(f"hr_trunk_hem_{s}", hem, 0.009, "dye_trim_metal", sides=6, closed=True))
        # the hose below, wrinkled a little at the knee and ankle
        hrows, hcs = leg_rows(body, s, ((0.76, 0.084), (0.62, 0.078), (0.50, 0.074), (0.40, 0.068), (0.25, 0.062), (0.0, 0.062)), [0.76, 0.68, 0.60, 0.55, 0.50, 0.44, 0.36, 0.28, 0.20], n=18)
        hrows = keep_side(wrinkle(hrows, hcs, (0.55, 0.25), 0.006, 0.025), s, 0.022)
        parts.append(K.shell(f"hr_hose_{s}", hrows, "wool_cream", 0.012))
        # a garter below the knee, gilt-buckled, with a bow
        parts += flat_band(f"hr_garter_{s}", hrows, hcs, 0.48, 0.026, "dye_accent_cloth", buckle_mat="dye_trim_metal")
    waist = G.ring_around(body, Vector((0.0, body.a["pelvis"].y, 0.97)), UP, FWD, 28, 0.03, 0.1, 0.22)
    parts.append(K.tube("hr_waist", waist, 0.022, "wool_dark", sides=8, closed=True))
    return W.bake(parts, "hr_legs")


def herald_hands(body):
    parts = []
    for s in (1, -1):
        rows, cs, hand, axis = glove_rows(body, s, 0.0)
        parts.append(glove("hr", s, rows, "leather_dark"))
        # the stiff gauntlet cuff: flaring up the forearm, piped and stitched in gold, three studs
        crs = []
        for t, r in ((-0.03, 0.012), (-0.08, 0.022), (-0.13, 0.036), (-0.17, 0.05)):
            c = hand + axis * t
            crs.append((G.ring_around(body, c, axis, UP, 20, 0.014 + r, 0.04, 0.09), c))
        cuff_rows = [r for r, _ in crs]
        parts.append(K.shell(f"hr_cuff_{s}", cuff_rows, "dye_coat_leather", 0.01, bevel=0.0025))
        rim, rc = crs[-1]
        parts.append(K.tube(f"hr_cuff_pipe_{s}", [rc + (p - rc) * 1.03 for p in rim], 0.0055, "dye_trim_metal", sides=6, closed=True))
        mid, mc = crs[2]
        parts.append(K.stitches(f"hr_cuff_st_{s}", [mc + (p - mc) * 1.06 for p in mid], [K.outward(p, mc, False) for p in mid], "dye_trim_metal", closed=True, step=0.016))
        top = G.ring_around(body, hand + axis * -0.1, axis, UP, 20, 0.04, 0.04, 0.09)
        for q in (3, 5, 7):
            p = top[q % len(top)]
            parts.append(G.rivet(f"hr_cuff_stud_{s}", p, K.outward(p, hand + axis * -0.1, False), "dye_trim_metal", 0.0065))
    return W.bake(parts, "hr_hands")


def herald_feet(body):
    parts = []
    for s in (1, -1):
        rows, cs, foot = boot_rows(body, s, 0.30)
        # the hose's foot to above the ankle, then the shoe over it
        parts.append(K.shell(f"hr_sock_{s}", rows[:5], "wool_cream", 0.01, bevel=0.0))
        shoe = rows[4:]
        parts += boot("hr_shoe", s, shoe, cs[4:], "leather_dark", "leather_brown")
        # the tongue and its big square buckle
        bp, bn = K.front_pt(body, foot.x, 0.11, 0.045)
        t = LEFT
        tongue = W.box(f"hr_tongue_{s}", (0.05, 0.06, 0.008), (0.0, 0.01, 0.0), "leather_dark", bevel=0.004, segments=2)
        K.placed(tongue, bp, bn, t)
        parts.append(tongue)
        parts += K.buckle(f"hr_shoe_buckle_{s}", bp + bn * 0.006, bn, t, "dye_trim_metal", w=0.05, h=0.042, tongue=False)
        # a rosette of the accent on the instep
        ros = W.cyl(f"hr_rosette_{s}", 0.016, -0.004, 0.006, "dye_accent_cloth", verts=12, axis="Z", bevel=0.004, segments=2)
        K.placed(ros, bp + bn * 0.004 + UP * 0.03, bn)
        parts.append(ros)
    return W.bake(parts, "hr_feet")


# ========================================================================== River Smuggler

def smuggler_head(body):
    c, rim, R, H = head_frame(body)
    parts = []
    crown = dome("sm_crown", Vector((0.0, c.y, rim)), (R, R * 1.05, H * 0.9), "dye_coat_leather", keep=-0.05, u=32, v=14)
    A.solidify(crown, 0.012, 1.0)
    K.bev(crown, 0.003)
    parts.append(crown)
    # stitched seams up the crown: four panels
    for q in range(4):
        a = math.pi / 4 + q * math.pi / 2
        seam = []
        ns = []
        for k in range(8):
            phi = (math.pi / 2) * k / 8
            d = Vector((math.cos(a) * math.cos(phi), math.sin(a) * math.cos(phi), 0.0))
            p = Vector((0.0, c.y, rim)) + Vector((d.x * R, d.y * R * 1.05, math.sin(phi) * H * 0.9)) * 1.0
            n = (p - Vector((0.0, c.y, rim))).normalized()
            seam.append(p + n * 0.013)
            ns.append(n)
        parts.append(K.stitches(f"sm_seam{q}", seam, ns, "linen", step=0.014))
    # the brim: narrow at the front, wide and sloping down over the neck at the back, a rolled edge
    n = 36
    inner, outer = [], []
    for k in range(n):
        a = 2 * math.pi * k / n
        d = Vector((math.sin(a), -math.cos(a), 0.0))
        back = max(0.0, -d.y)
        inner.append(Vector((0.0, c.y, rim - 0.01)) + Vector((d.x * R, d.y * R * 1.05, 0.0)))
        w = 0.055 + 0.085 * back
        outer.append(Vector((0.0, c.y, rim - 0.012 - 0.07 * back + 0.012 * (1 - back))) + Vector((d.x * (R + w), d.y * (R * 1.05 + w), 0.0)))
    parts.append(K.shell("sm_brim", [inner, outer], "dye_coat_leather", 0.01, bevel=0.002, offset=0.0))
    parts.append(K.tube("sm_brim_roll", outer, 0.007, "dye_coat_leather", sides=6, closed=True))
    parts.append(K.stitches("sm_brim_st", [a.lerp(b, 0.8) + UP * 0.006 for a, b in zip(inner, outer)], [UP] * n, "linen", closed=True, step=0.016))
    # a leather band with a brass buckle
    band = ellipse(Vector((0.0, c.y, rim + 0.015)), R + 0.003, R * 1.05 + 0.003, 32)
    outs = [K.outward(p, Vector((0.0, c.y, p.z))) for p in band]
    parts.append(K.ribbon("sm_hatband", [p + d * 0.004 for p, d in zip(band, outs)], outs, 0.026, "leather_dark", 0.006, closed=True))
    k = 24
    t = (band[k + 1] - band[k - 1]).normalized()
    parts += K.buckle("sm_hat_buckle", band[k] + outs[k] * 0.012, outs[k], t, "brass", w=0.03, h=0.03)
    # the chin strap
    for s in (1, -1):
        a = Vector((s * R * 0.97, c.y + 0.01, rim - 0.015))
        b = Vector((s * 0.045, c.y - 0.055, rim - 0.17))
        path = [a.lerp(b, q / 6) for q in range(7)]
        nrm = [Vector((s, -0.3, 0.0)).normalized()] * 7
        parts.append(K.ribbon(f"sm_strap_{s}", path, nrm, 0.012, "leather_dark", 0.004))
        parts.append(G.rivet(f"sm_strap_rivet_{s}", a + nrm[0] * 0.006, nrm[0], "brass", 0.006))
    return W.bake(parts, "sm_head")


def smuggler_cape(body):
    rows, centres = back_cape_rows(body, bottom=1.15, arc=(40, 320), n=32, flare=0.09, out=0.065)
    rows = K.folds(rows, centres, 6, 0.016, start=1, arc=math.radians(280))
    parts = [K.shell("sm_cape", rows, "dye_coat_leather", 0.012, closed=False, bevel=0.003)]
    parts += cape_edges(rows, centres, "sm_cape", "dye_coat_leather", "leather_dark", stitch_mat="linen", width=0.024)
    # the hood, down: a thick folded roll behind the neck
    neck = body.a["neck_01"]
    roll = [Vector((math.sin(math.radians(a)) * 0.135, neck.y + math.cos(math.radians(a)) * 0.115, neck.z - 0.025 + 0.03 * math.cos(math.radians(a)))) for a in range(100, 261, 10)]
    radii = [[0.04 * (1.0 + 0.12 * math.sin(q * 1.3 + i)) for q in range(10)] for i in range(len(roll))]
    parts.append(K.tube("sm_hood_roll", roll, 0.04, "dye_coat_leather", sides=10, radii=radii))
    # a fur collar round the top
    collar = [r for r in rows[0]]
    parts += K.fur_roll("sm_fur", K.resample(collar, 22, closed=False), 0.03, "fur_brown", closed=False)
    # wooden toggles on leather loops at the front edges
    for s in (1, -1):
        p, n = K.front_pt(body, s * 0.1, 1.38, 0.075)
        tog = W.cyl(f"sm_toggle_{s}", 0.009, -0.025, 0.025, "wood", verts=8, axis="X", bevel=0.003)
        K.placed(tog, p, n)
        parts.append(tog)
        loop = W.ring(f"sm_toggle_loop_{s}", 0.014, 0.003, (0, 0, 0.004), "leather_dark", axis="Z", maj=10, minr=4)
        K.placed(loop, p + LEFT * s * 0.02, n)
        parts.append(loop)
    return W.bake(parts, "sm_cape")


def smuggler_neck(body):
    neck = body.a["neck_01"]
    parts = []
    ring = ellipse(Vector((0.0, neck.y, neck.z - 0.03)), 0.082, 0.077, 22)
    radii = [[0.022 * (1.0 + 0.25 * abs(math.sin(i * 1.7 + q))) for q in range(8)] for i in range(len(ring))]
    parts.append(K.tube("sm_kerchief", ring, 0.022, "dye_accent_cloth", sides=8, closed=True, radii=radii))
    p, n = K.front_pt(body, 0.0, neck.z - 0.075, 0.045)
    knot = W.sphere("sm_knot", 0.026, (0, 0, 0), "dye_accent_cloth", seg=12, rings=8, scale=(1.25, 0.95, 0.8))
    K.placed(knot, p, n)
    parts.append(knot)
    # the two tails, folded and hanging off to one side
    for k, (dx, ln) in enumerate(((0.02, 0.11), (-0.025, 0.085))):
        rows = []
        for j in range(6):
            t = j / 5
            q, qn = K.front_pt(body, dx + 0.02 * t, neck.z - 0.09 - ln * t, 0.04 + 0.01 * t)
            side = UP.cross(qn).normalized()
            half = 0.035 * (1.0 - 0.7 * t)
            rows.append([q - side * half, q + qn * 0.006, q + side * half])
        parts.append(K.shell(f"sm_tail{k}", rows, "dye_accent_cloth", 0.007, closed=False, bevel=0.0015, offset=0.0))
    # a brass whistle on a leather cord
    cord = chest_arc(body, 1.33, 1.44, 0.075, 0.07, n=10)[0]
    parts.append(K.tube("sm_cord", cord, 0.003, "leather_dark", sides=5))
    wp, wn = K.front_pt(body, 0.0, 1.3, 0.075)
    whistle = W.cyl("sm_whistle", 0.008, -0.03, 0.01, "brass", verts=10, axis="Y", bevel=0.002)
    K.placed(whistle, wp, wn)
    parts.append(whistle)
    return W.bake(parts, "sm_neck")


def smuggler_body(body):
    parts = []
    rows, centres = torso(body, n=32)
    srows, scs = skirt(rows, (0.08, 0.16, 0.24, 0.32), 0.06)
    allrows, allc = rows + srows, centres + scs
    allrows = K.folds(allrows, allc, 8, 0.014, start=len(rows) - 1)
    # the waistcoat under the coat (seen in the opening at the front), buttoned in brass
    vest = K.shell("sm_vest", rows, "dye_accent_cloth", 0.014)
    cut(vest, lambda m: m.y < 0.03)
    parts.append(vest)
    for k in range(6):
        p, n = K.front_pt(body, 0.0, 1.36 - 0.07 * k, 0.062)
        parts.append(K.button(f"sm_vest_button{k}", p, n, "dye_trim_metal", 0.0095))
    # the long oilskin coat: open at the front, a slit up the back
    coat_rows = [[c + (p - c) * 1.06 for p in r] for r, c in zip(allrows, allc)]
    coat = K.shell("sm_coat", coat_rows, "dye_coat_leather", 0.014)
    cut(coat, lambda m: not ((m.y < -0.02 and abs(m.x) < (0.06 if m.z > 1.2 else 0.085)) or (m.z < 0.93 and m.y > 0.05 and abs(m.x) < 0.02)))
    parts.append(coat)
    yk, _ = yoke(body, "sm", coat_rows[0], centres[0], "dye_coat_leather")
    parts.append(yk)
    # turned-back lapels down the opening, stitched
    for s in (1, -1):
        lap = []
        for k in range(7):
            y = 1.44 - 0.06 * k
            p, n = K.front_pt(body, s * (0.065 + 0.03 * math.sin(math.pi * k / 6)), y, 0.085)
            lap.append((p, n))
        rows2 = [[p, p + LEFT * s * (0.045 * math.sin(math.pi * (k + 0.5) / 7)) + n * 0.004] for k, (p, n) in enumerate(lap)]
        parts.append(K.shell(f"sm_lapel_{s}", rows2, "dye_coat_leather", 0.008, closed=False, bevel=0.002, offset=0.0))
        parts.append(K.stitches(f"sm_lapel_st_{s}", [a.lerp(b, 0.75) + n * 0.006 for (a, b), (_, n) in zip(rows2, lap)], [n for _, n in lap], "linen", step=0.016))
    # the coat's hem, stitched
    parts += K.hem_trim("sm_coat_hem", coat_rows[-1], allc[-1], "dye_coat_leather", "leather_dark", width=0.02, stitch_mat="linen")
    # sleeves rolled to the elbow, a turned-back cuff
    for s in (1, -1):
        srs, cs, axis = sleeve(body, s, 0.24, n=22, out=0.026)
        parts.append(K.shell(f"sm_sleeve_{s}", srs, "dye_coat_leather", 0.013))
        cuff = [[cs[-1] + (p - cs[-1]) * 1.12 for p in srs[-1]], [cs[-2] + (p - cs[-2]) * 1.12 + axis * 0.01 for p in srs[-2]]]
        parts.append(K.shell(f"sm_cuff_{s}", cuff, "dye_coat_leather", 0.01, bevel=0.002))
    # flap pockets on the skirt
    for s in (1, -1):
        p, n = K.front_pt(body, s * 0.13, 0.9, 0.11)
        flap = W.box(f"sm_pocket_flap_{s}", (0.1, 0.035, 0.008), (0, 0, 0.004), "dye_coat_leather", bevel=0.003, segments=2)
        K.placed(flap, p, n)
        parts.append(flap)
        parts.append(K.button(f"sm_pocket_button_{s}", p + n * 0.008 - UP * 0.008, n, "dye_trim_metal", 0.007))
    # the bandolier: right shoulder to left hip, front and back, pouches and a brass buckle
    fp, fn = K.path_on_front(body, [(-0.13, 1.44), (0.02, 1.2), (0.16, 0.98)], 0.11, 16)
    parts.append(K.ribbon("sm_bandolier_f", fp, fn, 0.045, "leather_dark", 0.008))
    parts += K.rivets("sm_band_rivet", [fp[k] + fn[k] * 0.006 for k in (1, 14)], [fn[1], fn[14]], "brass")
    t = (fp[9] - fp[7]).normalized()
    parts += K.buckle("sm_band_buckle", fp[8] + fn[8] * 0.007, fn[8], t, "brass", w=0.05, h=0.055)
    for k in (3, 5):
        parts += pouch(f"sm_band_pouch{k}", fp[k] + fn[k] * 0.008, fn[k], "leather_brown", "brass", w=0.045, h=0.05, d=0.025)
    bp, bn = K.path_on_back(body, [(-0.13, 1.44), (0.02, 1.2), (0.16, 0.98)], 0.11, 16)
    parts.append(K.ribbon("sm_bandolier_b", bp, bn, 0.045, "leather_dark", 0.008))
    # a leather guard on the left shoulder, laced plates
    parts += K.lames("sm", body, 1, ("leather_brown", "leather_dark", "brass"), count=3, top_out=0.055)
    # the wide belt with an iron buckle
    bring, bc = rows[-1], centres[-1]
    belt = [bc + (p - bc) * 1.06 + K.outward(p, bc) * 0.02 + UP * 0.02 for p in bring]
    bns = [K.outward(p, bc) for p in belt]
    parts.append(K.ribbon("sm_belt", belt, bns, 0.05, "leather_brown", 0.009, closed=True))
    k = min(range(len(belt)), key=lambda i: belt[i].y)
    parts += K.buckle("sm_belt_buckle", belt[k] + bns[k] * 0.007, bns[k], (belt[(k + 1) % len(belt)] - belt[k - 1]).normalized(), "iron", w=0.062, h=0.06)
    return W.bake(parts, "sm_body")


def smuggler_legs(body):
    parts = []
    for s in (1, -1):
        rows, cs = leg_rows(body, s, ((0.97, 0.106), (0.80, 0.098), (0.62, 0.088), (0.52, 0.084), (0.46, 0.08)), [0.96, 0.9, 0.84, 0.78, 0.70, 0.62, 0.56, 0.50, 0.46], n=28)
        rows = K.folds(rows, cs, 3, 0.006, start=0, ease=0.5)
        rows = keep_side(wrinkle(rows, cs, (0.56, 0.7), 0.007, 0.03), s, 0.022)
        parts.append(K.shell(f"sm_breeches_{s}", rows, "canvas", 0.013))
        # the knee patch, stitched round
        kr, kc = K.ring_at(rows, cs, 0.56)
        k = min(range(len(kr)), key=lambda i: kr[i].y)
        p, n = kr[k] + K.outward(kr[k], kc) * 0.006, K.outward(kr[k], kc)
        patch = W.box(f"sm_knee_{s}", (0.075, 0.09, 0.006), (0, 0, 0), "leather_brown", bevel=0.003, segments=2)
        K.placed(patch, p, n)
        parts.append(patch)
        edge = [p + (LEFT * x + UP * y) for x, y in ((-0.03, 0.038), (0.03, 0.038), (0.03, -0.038), (-0.03, -0.038))]
        parts.append(K.stitches(f"sm_knee_st_{s}", edge, [n] * 4, "linen", closed=True, step=0.012))
        # the knee band, buckled in iron
        parts += flat_band(f"sm_knee_band_{s}", rows, cs, 0.47, 0.024, "leather_dark", buckle_mat="iron")
        # ribbed wool stockings below
        st, sc = leg_rows(body, s, ((0.5, 0.074), (0.4, 0.068), (0.25, 0.062), (0.0, 0.062)), [0.5, 0.45, 0.40, 0.34, 0.28, 0.24, 0.20], n=32)
        st = keep_side(K.folds(st, sc, 16, 0.0025, start=0, ease=0.0), s, 0.022)
        parts.append(K.shell(f"sm_stocking_{s}", st, "wool_cream", 0.01))
    waist = G.ring_around(body, Vector((0.0, body.a["pelvis"].y, 0.97)), UP, FWD, 28, 0.03, 0.1, 0.22)
    parts.append(K.tube("sm_waist", waist, 0.02, "canvas", sides=8, closed=True))
    return W.bake(parts, "sm_legs")


def smuggler_hands(body):
    parts = []
    for s in (1, -1):
        rows, cs, hand, axis = glove_rows(body, s, 0.02, fingers=False)
        parts.append(glove("sm", s, rows, "leather_brown"))
        # the cut-off ends rolled, a seam stitched up the back of the hand
        end, ec = rows[-1], cs[-1]
        parts.append(K.tube(f"sm_glove_edge_{s}", [ec + (p - ec) * 1.04 for p in end], 0.004, "leather_brown", sides=5, closed=True))
        seam = [r[0] for r in rows[1:]]
        parts.append(K.stitches(f"sm_glove_st_{s}", [p + K.outward(p, c, False) * 0.003 for p, c in zip(seam, cs[1:])], [K.outward(p, c, False) for p, c in zip(seam, cs[1:])], "linen", step=0.013))
        # a wrist strap with an iron buckle
        wr = G.ring_around(body, hand + axis * -0.04, axis, UP, 18, 0.026, 0.04, 0.08)
        wc = hand + axis * -0.04
        outs = [K.outward(p, wc, False) for p in wr]
        parts.append(K.ribbon(f"sm_wrist_{s}", wr, outs, 0.018, "leather_dark", 0.005, closed=True))
        k = 0
        parts += K.buckle(f"sm_wrist_buckle_{s}", wr[k] + outs[k] * 0.005, outs[k], axis.cross(outs[k]).normalized(), "iron", w=0.024, h=0.024)
    return W.bake(parts, "sm_hands")


def smuggler_feet(body):
    parts = []
    for s in (1, -1):
        rows, cs, foot = boot_rows(body, s, 0.52, n=20, round_=0.8)
        rows = keep_side(wrinkle(rows, cs, (0.33, 0.2), 0.005, 0.02), s, 0.03)
        parts += boot("sm_boot", s, rows, cs, "leather_brown", "leather_dark")
        # the folded-down cuff at the top
        top, tc = rows[0], cs[0]
        cuff = [[tc + (p - tc) * 1.16 + UP * 0.012 for p in top], [tc + (p - tc) * 1.2 - UP * 0.06 for p in top]]
        parts.append(K.shell(f"sm_boot_cuff_{s}", cuff, "dye_coat_leather", 0.01, bevel=0.0025))
        parts.append(K.stitches(f"sm_boot_cuff_st_{s}", [tc + (p - tc) * 1.25 - UP * 0.05 for p in top], [K.outward(p, tc) for p in top], "linen", closed=True, step=0.015))
        # two buckled straps
        for y in (0.36, 0.22):
            parts += flat_band(f"sm_boot_strap_{s}{int(y * 100)}", rows, cs, y, 0.02, "leather_dark", out=0.008, buckle_mat="iron")
    return W.bake(parts, "sm_feet")


# ============================================================================== Ash Pilgrim

def pilgrim_head(body):
    c, rim, R, H = head_frame(body)
    parts = []
    # a cowl: from a peak above and behind the crown, down round the head, flaring out over the
    # shoulders; open at the face (a deep, shadowed opening)
    neck = body.a["neck_01"]
    n = 30
    levels = [(rim + H + 0.075, 0.02, 0.02, 0.085), (rim + H + 0.03, R * 0.75, R * 0.8, 0.06), (rim + H * 0.55, R + 0.035, R * 1.1 + 0.03, 0.035),
              (rim + 0.0, R + 0.045, R * 1.1 + 0.04, 0.02), (rim - 0.09, R + 0.04, R * 1.1 + 0.03, 0.015), (neck.z + 0.02, 0.125, 0.12, 0.02),
              (neck.z - 0.05, 0.19, 0.16, 0.025), (neck.z - 0.1, 0.23, 0.18, 0.03)]
    rows, cs = [], []
    for z, rx, ry, back in levels:
        cc = Vector((0.0, c.y + back, z))
        cs.append(cc)
        rows.append([cc + Vector((math.sin(2 * math.pi * k / n) * rx, -math.cos(2 * math.pi * k / n) * ry, 0.0)) for k in range(n)])
    rows = K.folds(rows, cs, 5, 0.012, start=4)
    hood = A.grid("ap_hood", rows, "dye_coat_cloth", closed=True)
    cut(hood, lambda m: not (m.y < c.y - 0.03 and neck.z + 0.0 < m.z < rim + 0.04 and abs(m.x) < 0.095))
    A.solidify(hood, 0.014, 1.0)
    K.bev(hood, 0.003, 1)
    parts.append(hood)
    # the stitched trim round the face
    edge = [Vector((math.sin(math.radians(a)) * 0.1, c.y - (R * 1.1 + 0.03) * 0.86, (neck.z + rim + 0.04) / 2 + (rim + 0.04 - neck.z) / 2 * math.cos(math.radians(a)))) for a in range(-90, 91, 10)]
    parts.append(K.tube("ap_hood_edge", edge, 0.013, "dye_trim_cloth", sides=7))
    parts.append(K.stitches("ap_hood_st", [p - FWD * 0.0 + UP * 0.0 + (p - Vector((0.0, p.y, rim - 0.05))).normalized() * 0.02 for p in edge], [FWD] * len(edge), "linen", step=0.016))
    # the liripipe: a long tail from the hood's point down the back
    top = Vector((0.0, c.y + 0.085, rim + H + 0.07))
    tail = [top + Vector((0.0, 0.05 * t + 0.03 * t * t, -0.3 * t)) for t in [q / 8 for q in range(9)]]
    radii = [[0.03 * (1.0 - 0.75 * q / 8) for _ in range(8)] for q in range(9)]
    parts.append(K.tube("ap_liripipe", tail, 0.03, "dye_coat_cloth", sides=8, radii=radii))
    # a cloth wrap round the neck, and a wooden pilgrim's token on the hood's side
    wrap = ellipse(Vector((0.0, neck.y + 0.01, neck.z + 0.01)), 0.112, 0.102, 22)
    radii = [[0.028 * (1.0 + 0.3 * abs(math.sin(i * 1.3 + q * 0.7))) for q in range(8)] for i in range(len(wrap))]
    parts.append(K.tube("ap_wrap", wrap, 0.028, "dye_accent_cloth", sides=8, closed=True, radii=radii))
    tp = Vector((R + 0.045, c.y + 0.0, rim - 0.02))
    token = W.cyl("ap_token", 0.022, -0.004, 0.004, "wood", verts=12, axis="Z", bevel=0.003, segments=2)
    K.placed(token, tp, LEFT)
    parts.append(token)
    cross = W.box("ap_token_mark", (0.006, 0.026, 0.004), (0, 0, 0.005), "glow_accent", bevel=0.001)
    K.placed(cross, tp, LEFT)
    parts.append(cross)
    return W.bake(parts, "ap_head")


def pilgrim_cape(body):
    rows, centres = back_cape_rows(body, bottom=0.56, arc=(75, 285), n=30, flare=0.045)
    rows = K.folds(rows, centres, 5, 0.03, start=2, arc=math.radians(210))
    # a ragged hem: the bottom row cut in uneven dags
    last = rows[-1]
    rows[-1] = [Vector((p.x, p.y, p.z + (0.05 if k % 3 == 1 else 0.0) + 0.02 * K._hash(k, 3))) for k, p in enumerate(last)]
    parts = [K.shell("ap_mantle", rows, "dye_coat_cloth", 0.013, closed=False, bevel=0.003)]
    parts.append(K.tube("ap_mantle_edge_a", [r[0] for r in rows], 0.007, "dye_trim_cloth", sides=6))
    parts.append(K.tube("ap_mantle_edge_b", [r[-1] for r in rows], 0.007, "dye_trim_cloth", sides=6))
    # a band of the trim a little above the hem, stitched
    band, bc = rows[-3], centres[-3]
    parts += K.hem_trim("ap_mantle_band", band, bc, "dye_trim_cloth", "dye_trim_cloth", width=0.03, closed=False, stitch_mat="linen", pipe_r=0.004)
    # a grey fur collar over the shoulders
    parts += K.fur_roll("ap_fur", K.resample(rows[0], 24, closed=False), 0.034, "fur_grey", closed=False)
    # a rope clasp across the chest between two wooden toggles
    pts, ns = chest_arc(body, 1.38, 1.42, 0.12, 0.075, n=8)
    parts.append(K.tube("ap_clasp", pts, 0.008, "rope", sides=6))
    for s, q in ((1, pts[0]), (-1, pts[-1])):
        tog = W.cyl(f"ap_toggle_{s}", 0.01, -0.025, 0.025, "wood", verts=8, axis="Y", bevel=0.003)
        K.placed(tog, q, FWD)
        parts.append(tog)
    return W.bake(parts, "ap_cape")


def pilgrim_neck(body):
    pts, ns = chest_arc(body, 1.2, 1.44, 0.1, 0.1, n=16)
    parts = []
    for k, (q, n) in enumerate(zip(pts, ns)):
        if k % 4 == 2:
            ob = W.cyl("ap_spacer", 0.016, -0.005, 0.005, "brass", verts=10, axis="Z", bevel=0.002)
            K.placed(ob, q, (pts[min(k + 1, len(pts) - 1)] - pts[max(k - 1, 0)]).normalized())
            parts.append(ob)
        else:
            parts.append(W.sphere("ap_bead", 0.015 if k % 2 else 0.019, q, "wood", seg=10, rings=7))
    p, n = K.front_pt(body, 0.0, 1.12, 0.13)
    censer = [W.cyl("ap_censer", 0.032, -0.03, 0.03, "iron", verts=10, axis="Y", bevel=0.004, segments=2),
              W.cyl("ap_censer_lid", 0.034, 0.03, 0.05, "iron", r1=0.01, verts=10, axis="Y", bevel=0.003),
              W.sphere("ap_censer_knob", 0.008, (0, 0.056, 0), "brass", seg=8, rings=5),
              W.ring("ap_censer_band", 0.033, 0.004, (0, 0.0, 0), "brass", axis="Y", maj=12, minr=4)]
    for k in range(3):
        censer.append(W.box("ap_censer_glow", (0.011, 0.024, 0.004), (0.016 * (k - 1), 0.0, 0.031), "glow_accent", bevel=0.0))
    for ob in censer:
        ob.matrix_world = A.frame(p, n) @ ob.matrix_world
        parts.append(ob)
    # the censer's chains up to the beads
    for dx in (-0.025, 0.025):
        a = p + LEFT * dx + UP * 0.05
        b = pts[len(pts) // 2] + LEFT * dx * 0.5
        parts += chain_links("ap_censer_chain", [a.lerp(b, t / 5) for t in range(1, 5)], "iron", 0.006)
    return W.bake(parts, "ap_neck")


def pilgrim_body(body):
    parts = []
    rows, centres = torso(body, n=36)
    srows, scs = skirt(rows, (0.08, 0.17, 0.26, 0.35, 0.42), 0.05)
    allrows, allc = rows + srows, centres + scs
    allrows = K.folds(allrows, allc, 9, 0.016, start=len(rows) - 2)
    robe = K.shell("ap_robe", allrows, "dye_coat_cloth", 0.016)
    # (a slit up the front and back of the skirt, so the legs can stride)
    cut(robe, lambda m: not (m.z < 0.8 and abs(m.x) < 0.018))
    parts.append(robe)
    yk, _ = yoke(body, "ap", rows[0], centres[0], "dye_coat_cloth")
    parts.append(yk)
    parts += K.hem_trim("ap_robe_hem", allrows[-1], allc[-1], "dye_trim_cloth", "dye_trim_cloth", width=0.035, stitch_mat="linen", pipe_r=0.004)
    # the front wrap: a diagonal overlapping edge from the right shoulder to the left hip, stitched
    fp, fn = K.path_on_front(body, [(-0.1, 1.46), (0.0, 1.27), (0.12, 1.0)], 0.07, 14)
    parts.append(K.ribbon("ap_wrap_edge", fp, fn, 0.04, "dye_accent_cloth", 0.008))
    parts.append(K.stitches("ap_wrap_st", [p + n * 0.006 for p, n in zip(fp, fn)], fn, "linen", step=0.016))
    # wide bell sleeves, folded, trimmed at the end
    for s in (1, -1):
        srs, cs, axis = sleeve(body, s, 0.3, n=24, out=0.024, flare=0.035)
        srs = K.folds(srs, cs, 4, 0.006, start=1)
        parts.append(K.shell(f"ap_sleeve_{s}", srs, "dye_coat_cloth", 0.012))
        end, ce = srs[-1], cs[-1]
        parts.append(K.tube(f"ap_sleeve_trim_{s}", [ce + (p - ce) * 1.02 for p in end], 0.011, "dye_trim_cloth", sides=7, closed=True))
    # the rope belt, knotted at the front with two tasselled ends
    bring, bc = rows[-1], centres[-1]
    belt = [bc + (p - bc) * 1.0 + K.outward(p, bc) * 0.024 + UP * 0.03 for p in bring]
    parts.append(K.tube("ap_belt", belt, 0.013, "rope", sides=7, closed=True))
    kp, kn = K.front_pt(body, 0.05, 1.0, 0.13)
    parts.append(W.sphere("ap_belt_knot", 0.022, kp.to_tuple(), "rope", seg=10, rings=7, scale=(1.2, 0.9, 1.0)))
    for dx, ln in ((0.0, 0.2), (0.025, 0.16)):
        a = kp + LEFT * dx
        end = [a - UP * ln * t / 5 - FWD * 0.012 * t / 5 for t in range(6)]
        parts.append(K.tube("ap_belt_end", end, 0.009, "rope", sides=6))
        parts.append(W.cyl("ap_tassel", 0.016, end[-1].z - 0.05, end[-1].z, "rope", x=end[-1].y, z=end[-1].x, r1=0.006, verts=8, axis="Z", bevel=0.0))
    # a satchel on the right hip, its strap over the left shoulder
    sp, sn = K.path_on_front(body, [(0.13, 1.44), (0.0, 1.22), (-0.14, 1.02)], 0.1, 14)
    parts.append(K.ribbon("ap_satchel_strap", sp, sn, 0.032, "leather_brown", 0.007))
    bk, bkn = K.path_on_back(body, [(0.13, 1.44), (0.0, 1.22), (-0.14, 1.02)], 0.1, 14)
    parts.append(K.ribbon("ap_satchel_strap_b", bk, bkn, 0.032, "leather_brown", 0.007))
    hp = body.outer(Vector((0.0, 0.0, 0.92)), -LEFT, 0.06)
    if hp is not None:
        parts += pouch("ap_satchel", hp + FWD * 0.02, -LEFT, "leather_brown", "brass", w=0.14, h=0.12, d=0.05)
    return W.bake(parts, "ap_body")


def pilgrim_legs(body):
    parts = []
    for s in (1, -1):
        rows, cs = leg_rows(body, s, ((0.97, 0.108), (0.70, 0.098), (0.55, 0.084), (0.4, 0.07), (0.0, 0.066)), [0.96, 0.9, 0.84, 0.78, 0.72, 0.66, 0.6, 0.55, 0.5, 0.45, 0.4, 0.34, 0.28, 0.24, 0.2], n=28)
        rows = keep_side(K.folds(rows, cs, 3, 0.008, start=0, ease=0.3), s, 0.022)
        parts.append(K.shell(f"ap_trousers_{s}", rows, "dye_coat_cloth", 0.013))
        # cross-bound from the ankle to below the knee: two strips spiralling opposite ways
        for d in (1, -1):
            path = []
            for q in range(25):
                t = q / 24
                y = 0.22 + 0.28 * t
                ring, c = K.ring_at(rows, cs, y)
                a = d * 2 * math.pi * 2.0 * t
                r = (ring[0] - c).length * 1.03 + 0.004
                path.append(Vector((c.x + math.sin(a) * r * s, c.y - math.cos(a) * r, y)))
            nrm = [K.outward(p, Vector((cc.x, cc.y, p.z))) for p, cc in zip(path, [K.ring_at(rows, cs, p.z)[1] for p in path])]
            parts.append(K.ribbon(f"ap_binding_{s}{d}", path, nrm, 0.022, "dye_trim_cloth", 0.005))
        # a leather kneeling pad, strapped on
        kr, kc = K.ring_at(rows, cs, 0.55)
        k = min(range(len(kr)), key=lambda i: kr[i].y)
        p, n = kr[k] + K.outward(kr[k], kc) * 0.008, K.outward(kr[k], kc)
        pad = dome(f"ap_knee_{s}", Vector((0, 0, 0)), (0.05, 0.06, 0.02), "leather_brown", keep=0.0, u=16, v=8)
        pad.matrix_world = A.frame(p, n)
        A.solidify(pad, 0.006, -1.0)
        parts.append(pad)
        parts.append(K.stitches(f"ap_knee_st_{s}", [p + (LEFT * math.cos(a) * 0.043 + UP * math.sin(a) * 0.052) + n * 0.008 for a in [2 * math.pi * q / 16 for q in range(16)]], [n] * 16, "linen", closed=True, step=0.012))
        parts += flat_band(f"ap_knee_strap_{s}", rows, cs, 0.62, 0.018, "leather_brown", out=0.006, buckle_mat="brass")
    waist = G.ring_around(body, Vector((0.0, body.a["pelvis"].y, 0.97)), UP, FWD, 28, 0.03, 0.1, 0.22)
    parts.append(K.tube("ap_waist", waist, 0.02, "dye_coat_cloth", sides=8, closed=True))
    return W.bake(parts, "ap_legs")


def pilgrim_hands(body):
    parts = []
    for s in (1, -1):
        rows, cs, hand, axis = glove_rows(body, s, 0.015)
        parts.append(glove("ap", s, rows, "linen"))
        # bandage strips spiralling up the hand and wrist, a leather palm guard
        path = []
        for q in range(30):
            t = q / 29
            pos = -0.15 + 0.29 * t
            c = hand + axis * pos
            ring = G.ring_around(body, c, axis, UP, 12, 0.02 if pos < 0 else 0.016, 0.02, 0.08)
            a = 2 * math.pi * 4.5 * t
            k = int(a / (2 * math.pi) * 12) % 12
            path.append(ring[k])
        nrm = [K.outward(p, hand + axis * (-0.15 + 0.29 * q / 29), False) for q, p in enumerate(path)]
        parts.append(K.ribbon(f"ap_bandage_{s}", path, nrm, 0.02, "dye_trim_cloth", 0.004))
        palm = body.outer(hand + axis * 0.06, -UP, 0.02)
        if palm is not None:
            g = W.box(f"ap_palm_{s}", (0.06, 0.07, 0.006), (0, 0, 0), "leather_brown", bevel=0.003, segments=2)
            K.placed(g, palm, -UP, axis)
            parts.append(g)
    return W.bake(parts, "ap_hands")


def pilgrim_feet(body):
    parts = []
    for s in (1, -1):
        rows, cs, foot = boot_rows(body, s, 0.30, n=24)
        rows = keep_side(wrinkle(rows, cs, (0.22, 0.14), 0.005, 0.02), s, 0.03)
        last = rows[-1]
        lc = sum(last, Vector()) / len(last)
        parts.append(K.shell(f"ap_footwrap_{s}", rows + [[lc + (p - lc) * 0.25 for p in last]], "linen", 0.01, bevel=0.0))
        # the wooden sole, and leather straps crossing the foot and round the ankle
        outline = keep_side([[Vector((q.x, q.y, last[0].z + 0.002)) for q in [lc + (p - lc) * 1.03 for p in last]]], s, 0.025)[0]
        parts.append(slab(f"ap_sole_{s}", outline, 0.02, "wood", 0.004))
        for y in (0.06, 0.1, 0.24):
            parts += flat_band(f"ap_sandal_{s}{int(y * 100)}", rows, cs, y, 0.016, "leather_brown", out=0.005, buckle_mat="brass" if y > 0.2 else None)
        for d in (1, -1):
            path = []
            for q in range(7):
                t = q / 6
                y = 0.04 + 0.14 * t
                ring, c = K.ring_at(rows, cs, y)
                k = int((0.25 + 0.5 * t if d > 0 else 0.75 - 0.5 * t) * len(ring)) % len(ring)
                path.append(c + (ring[k] - c) * 1.04)
            nrm = [K.outward(p, K.ring_at(rows, cs, p.z)[1]) for p in path]
            parts.append(K.ribbon(f"ap_sandal_x_{s}{d}", path, nrm, 0.014, "leather_brown", 0.004))
    return W.bake(parts, "ap_feet")


SETS = {
    "herald": [herald_head, herald_cape, herald_neck, herald_body, herald_legs, herald_hands, herald_feet],
    "smuggler": [smuggler_head, smuggler_cape, smuggler_neck, smuggler_body, smuggler_legs, smuggler_hands, smuggler_feet],
    "pilgrim": [pilgrim_head, pilgrim_cape, pilgrim_neck, pilgrim_body, pilgrim_legs, pilgrim_hands, pilgrim_feet],
}


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    renders = "--no-renders" not in argv
    only = argv[argv.index("--only") + 1] if "--only" in argv else None
    sets = [a for a in argv if a in SETS] or list(SETS)
    with open(os.path.join(REF, "anchors.json")) as f:
        anchors = G.json.load(f)
    os.makedirs(SHOTS, exist_ok=True)
    for set_name in sets:
        textures = {n: os.path.join(REF, f"look_{set_name}_{n}_tex.png") for n in NAMES}
        for name in NAMES:
            if only and name != only:
                continue
            W.clear_scene()
            for m in list(bpy.data.materials):
                bpy.data.materials.remove(m)
            body = A.Body(name, anchors[name])
            parts = [build(body) for build in SETS[set_name]]
            for ob in parts:
                A.apply_mods(ob)
            T.bake_texture(parts, textures[name], size=TEX, samples=24)
            tris = sum(W.triangles(o) for o in parts)
            if renders:
                G.DEFAULT_DYES = SET_DYES[set_name]
                G.tint_preview(parts)
                G.pictures(parts, body, os.path.join(SHOTS, f"{set_name}_{name}.png"))
            A.remap_uvs(parts, QUARTER[name])
            if not only:
                W.export_glb(parts, os.path.join(OUT, f"look_{set_name}_{name}.glb"))
            print(f"[outfits] {set_name} {name}: {tris} triangles, pieces {[(o.name, W.triangles(o)) for o in parts]}")
        if only:
            continue
        img = bpy.data.images.new(f"look_{set_name}", TEX * 2, TEX * 2, alpha=False)
        px = [0.5] * (TEX * 2 * TEX * 2 * 4)
        for name, path in textures.items():
            if not os.path.exists(path):
                continue
            src = bpy.data.images.load(path)
            sp = list(src.pixels)
            ox, oy = int(QUARTER[name][0] * TEX * 2), int(QUARTER[name][1] * TEX * 2)
            for y in range(TEX):
                row = (oy + y) * TEX * 2 + ox
                px[row * 4:(row + TEX) * 4] = sp[y * TEX * 4:(y + 1) * TEX * 4]
            bpy.data.images.remove(src)
        img.pixels = px
        img.filepath_raw = os.path.join(OUT, f"look_{set_name}.png")
        img.file_format = "PNG"
        img.save()


if __name__ == "__main__":
    main()
