"""The Varrow wardrobe's outfits (Fashionscape, app/src/hub.rs), fitted to each of the four
fighters with the Ledger gear's pipeline (build_gear.py): the same chunky, blocky style, our toon
shading and outlines in the game, three dye zones per piece (dye_coat, dye_trim, dye_accent).

    blender --background --factory-startup --python app/tools/blender/build_outfits.py [-- herald smuggler pilgrim] [--no-renders]

Sets (each piece is a mesh named <prefix>_<slot>; the game hides the fighter under body, legs,
hands and feet pieces, so those always cover their part fully):

- Court Herald (hr_): a plumed pillbox cap, a heraldic half-cape with a crest, a chain of
  office with its medallion, a quartered tabard over a jacket with puffed sleeves, striped hose
  with garters, gauntleted cuffs, buckled court shoes.
- River Smuggler (sm_): an oilskin sou'wester, an oilcloth short cloak with its hood down, a
  knotted neckerchief, an oilskin waistcoat over rolled shirt sleeves, rolled breeches over
  stockings, fingerless gloves, tall wading boots.
- Ash Pilgrim (ap_): a deep hood with a cloth wrap, a long ashen mantle, a string of censer beads
  with a smouldering censer (it glows), a wrapped robe with a rope belt, bound leg-wraps,
  bandaged hands, wrapped feet on sandal soles.

Writes app/assets/characters/look_<set>_<fighter>.glb, app/assets/characters/look_<set>.png
(the four fighters' bakes, a quarter each) and pictures to app/shots/character-art/outfits/.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy  # noqa: E402
from mathutils import Vector  # noqa: E402

import build_gear as G  # noqa: E402  (the gear's fitting helpers, palette and pictures)

A, W, T = G.A, G.W, G.T
gv, UP, FWD, LEFT = G.gv, G.UP, G.FWD, G.LEFT
OUT, REF, NAMES, QUARTER, TEX = G.OUT, G.REF, G.NAMES, G.QUARTER, G.TEX
SHOTS = os.path.join(G.APP, "shots", "character-art", "outfits")

W.PALETTE.update({
    "linen": ((0.46, 0.42, 0.35), 0.0, 0.9),
    "wool": ((0.52, 0.50, 0.46), 0.0, 0.95),
    "feather": ((0.82, 0.80, 0.76), 0.0, 0.8),
})
T.RECIPES.update({
    "linen": ("cord", (0.40, 0.36, 0.30), (0.52, 0.48, 0.40), (0.30, 0.27, 0.22), 0.35, 0.5),
    "wool": ("cord", (0.44, 0.42, 0.38), (0.58, 0.56, 0.52), (0.32, 0.30, 0.27), 0.5, 0.6),
    "feather": ("cord", (0.74, 0.72, 0.68), (0.90, 0.88, 0.84), (0.60, 0.58, 0.54), 0.3, 0.4),
})

# each set's own colours for the pictures (the game uses the wearer's dyes; these match the
# set's default dyes in hub.rs)
SET_DYES = {
    "herald": {"dye_coat": (0.05, 0.10, 0.35), "dye_trim": (0.62, 0.42, 0.14), "dye_accent": (0.50, 0.03, 0.06), "glow_accent": (0.9, 0.5, 0.1)},
    "smuggler": {"dye_coat": (0.10, 0.20, 0.06), "dye_trim": (0.55, 0.22, 0.08), "dye_accent": (0.40, 0.07, 0.04), "glow_accent": (0.9, 0.5, 0.1)},
    "pilgrim": {"dye_coat": (0.22, 0.23, 0.25), "dye_trim": (0.03, 0.03, 0.035), "dye_accent": (0.80, 0.30, 0.05), "glow_accent": (0.95, 0.42, 0.08)},
}


# ---------------------------------------------------------------- shared builders

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


def jacket(body, prefix, mat="dye_coat", skirt=(0.10, 0.20, 0.28), open_front=True, sleeve=0.19, sleeve_mat=None, sleeve_puff=0.0):
    """A coat body like the Rig's: torso rings from the shoulders to the hips, a skirt below,
    a yoke over the shoulders to the neck and sleeves over the tops of the arms. Covers the
    torso and the upper arms (the game hides the fighter under them)."""
    parts = []
    rows, centres = [], []
    depth = body.a["spine_02"].y
    for y in [1.43, 1.38, 1.31, 1.24, 1.17, 1.10, 1.03, 0.97]:
        centres.append(Vector((0.0, depth, y)))
        rows.append(G.ring_around(body, centres[-1], UP, FWD, 28, 0.05, 0.09, 0.24))
    rows = G.fair_rows(rows, centres, 0.75, 2)
    hip = rows[-1]
    hc = sum(hip, Vector()) / len(hip)
    for k, drop in enumerate(skirt):
        flare = 1.0 + 0.07 * (k + 1)
        rows.append([hc + (p - hc) * flare - UP * drop for p in hip])
    coat = A.grid(f"{prefix}_coat", rows, mat, closed=True)
    if open_front:
        bm = G.bmesh.new()
        bm.from_mesh(coat.data)
        gone = [f for f in bm.faces if f.calc_center_median().z < 0.95 and f.calc_center_median().y < -0.02 and abs(f.calc_center_median().x) < 0.07]
        G.bmesh.ops.delete(bm, geom=gone, context="FACES")
        bm.to_mesh(coat.data)
        bm.free()
    A.solidify(coat, 0.016, -1.0)
    parts.append(coat)
    neck = body.a["neck_01"]
    ny = min(max(neck.z + 0.02, 1.48), 1.56)
    c0 = centres[0]
    yrows = [list(rows[0]), [Vector((c0.x + (p.x - c0.x) * 0.92, neck.y + (p.y - neck.y) * 0.8, 1.475)) for p in rows[0]]]
    ring = []
    for p in rows[0]:
        d = Vector(((p - c0).x, (p - c0).y, 0.0)).normalized()
        ring.append(Vector((neck.x + d.x * 0.095, neck.y + d.y * 0.085, ny)))
    yrows.append(ring)
    yoke = A.grid(f"{prefix}_yoke", yrows, mat, closed=True)
    A.solidify(yoke, 0.016, -1.0)
    parts.append(yoke)
    for s in (1, -1):
        ua = body.a["upperarm_l" if s > 0 else "upperarm_r"]
        la = body.a["lowerarm_l" if s > 0 else "lowerarm_r"]
        axis = (la - ua).normalized()
        ts = [-0.04, 0.04, (0.04 + sleeve) / 2, sleeve]
        cs = [ua + axis * t for t in ts]
        srows = [G.ring_around(body, c, axis, UP, 16, 0.022 + sleeve_puff * (1.0 - abs(2 * k / 3 - 1)), 0.045, 0.06) for k, c in enumerate(cs)]
        parts.append(G.tube_shell(f"{prefix}_sleeve_{s}", G.fair_rows(srows, cs, 0.9, 2), sleeve_mat or mat, 0.014))
    return parts, rows, centres


def leg_tubes(body, prefix, mat="dye_coat", table=None, top=0.93, bottom=0.2):
    """Padded tubes round each leg (like the Rig's trousers). Returns the parts and each leg's
    rings (rows, centres) for trims."""
    table = table or ((0.93, 0.100), (0.78, 0.092), (0.62, 0.082), (0.50, 0.076), (0.40, 0.069), (0.25, 0.066), (0.0, 0.066))
    parts, legs = [], []
    ys = [y for y in [0.93, 0.86, 0.78, 0.70, 0.62, 0.55, 0.48, 0.40, 0.32, 0.25, 0.20] if bottom - 0.001 <= y <= top + 0.001]
    for s in (1, -1):
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
            rows.append([c + FWD * (math.cos(2 * math.pi * k / 20) * r * 0.92) + LEFT * (math.sin(2 * math.pi * k / 20) * r) for k in range(20)])
        rows = [[Vector((p.x if s * p.x > 0.004 else s * 0.004, p.y, p.z)) for p in r] for r in rows]
        leg = G.tube_shell(f"{prefix}_leg_{s}", rows, mat, 0.014)
        A.smooth(leg, 1)
        parts.append(leg)
        legs.append((s, rows, centres))
    return parts, legs


def band(name, rows, centres, y, mat, scale=1.06, r=0.012):
    """A band round a tube at height y (from its nearest ring)."""
    k = min(range(len(centres)), key=lambda i: abs(centres[i].z - y))
    ring = [centres[k] + (p - centres[k]) * scale for p in rows[k]]
    return W.tube_path(name, [(p.x, p.y, p.z) for p in ring], r, mat, sides=6, closed=True)


def gloves(body, prefix, mat="dye_coat", cuff=0.03, cuff_mat=None, fingers=True):
    """Gloves (the Rig's gauntlets, made general): a cuff up the forearm and the glove over the
    hand (to the fingertips, or only to the knuckles)."""
    parts = []
    for s in (1, -1):
        hand = body.a["hand_l" if s > 0 else "hand_r"]
        lower = body.a["lowerarm_l" if s > 0 else "lowerarm_r"]
        axis = (hand - lower).normalized()
        rows = []
        for t, flare in ((-0.16, cuff), (-0.11, cuff * 0.75), (-0.05, cuff * 0.4), (-0.01, 0.012)):
            rows.append(G.ring_around(body, hand + axis * t, axis, UP, 18, 0.012 + flare, 0.04, 0.08))
        ends = (0.03, 0.07, 0.11, 0.15, 0.185) if fingers else (0.03, 0.07, 0.105)
        for t in ends:
            rows.append(G.ring_around(body, hand + axis * t, axis, UP, 18, 0.014, 0.02, 0.07))
        if fingers:
            tip = hand + axis * 0.205
            rows.append([tip + (p - tip) * 0.35 for p in rows[-1]])
        glove = G.tube_shell(f"{prefix}_glove_{s}", rows, mat, 0.012)
        A.smooth(glove, 1)
        A.wrap_outside(glove, body, 0.02)
        parts.append(glove)
        if cuff_mat:
            # a trim band round the cuff's edge (it sits on the cuff, not off it)
            ring = rows[0]
            parts.append(W.tube_path(f"{prefix}_cuff_{s}", [(p.x, p.y, p.z) for p in ring], 0.009, cuff_mat, sides=6, closed=True))
    return parts


def boots(body, prefix, mat="dye_coat", top=0.30, round_=0.7, sole_mat="leather"):
    """Boots (the Rig's, made general): rings from `top` down to the sole, closed underneath."""
    parts, feet = [], []
    for s in (1, -1):
        foot = body.a["foot_l" if s > 0 else "foot_r"]
        ball = body.a["ball_l" if s > 0 else "ball_r"]
        mid = (foot + ball) / 2
        ys = [y for y in (0.52, 0.45, 0.38, 0.30, 0.25, 0.20, 0.15, 0.11, 0.075, 0.045, 0.02) if y <= top + 0.001]
        rows, centres = [], []
        calf = body.a["calf_l" if s > 0 else "calf_r"]
        for y in ys:
            if y > 0.3:
                t = (y - 0.3) / max(calf.z - 0.3, 0.05)
                c = Vector((foot.x + (calf.x - foot.x) * t, foot.y + (calf.y - foot.y) * t, y))
            else:
                c = Vector((foot.x, mid.y if y < 0.13 else foot.y, y))
            centres.append(c)
            rows.append(G.ring_around(body, c, UP, FWD, 20, 0.03, 0.045, 0.16, own_side=True))
        rows = G.fair_rows(rows, centres, round_)
        rows = [[Vector((p.x if s * p.x > 0.006 else s * 0.006, p.y, p.z)) for p in r] for r in rows]
        last = rows[-1]
        lc = sum(last, Vector()) / len(last)
        rows.append([lc + (p - lc) * 1.06 - UP * 0.018 for p in last])
        rows.append([lc - UP * 0.02 + (p - lc) * 0.2 for p in last])
        parts.append(G.tube_shell(f"{prefix}_boot_{s}", rows, mat, 0.012))
        feet.append((s, rows, centres, foot))
    return parts, feet


def back_cape(body, prefix, mat, bottom=1.0, arc=(100, 260), n=16, flare=0.08, out=0.06):
    """A cape over the shoulders and down the back: rings round the body over an arc (degrees
    from the front, through the back), from the neck down to `bottom`, flaring out."""
    rows = []
    depth = body.a["spine_02"].y
    neck = body.a["neck_01"]
    hs = [h for h in (1.50, 1.46, 1.40, 1.30, 1.18, 1.05, 0.92, 0.80, 0.68, 0.56) if h >= bottom - 0.001]
    for k, y in enumerate(hs):
        c = Vector((0.0, depth if y < 1.45 else neck.y, y))
        row = []
        for j in range(n):
            a = math.radians(arc[0] + (arc[1] - arc[0]) * j / (n - 1))
            d = (FWD * math.cos(a) + LEFT * math.sin(a)).normalized()
            p = body.outer(c, d)
            r = (p - c).length if p is not None else 0.16
            r = min(max(r, 0.08), 0.26 if y < 1.45 else 0.11)
            spread = 1.0 + flare * k
            row.append(c + d * (r * spread + out))
        rows.append(row)
    cape = A.grid(f"{prefix}_cape", rows, mat, closed=False)
    A.solidify(cape, 0.014, -1.0)
    return cape, rows


def chain(prefix, pts, mat, size=0.012):
    links = []
    for k, q in enumerate(pts):
        links.append(W.ring(f"{prefix}_link", size, size * 0.3, (q.x, q.y, q.z), mat, axis="X" if k % 2 else "Z", maj=8, minr=3))
    return links


def chest_arc(body, y_mid, y_end, x_end, out, n=12):
    """Points across the chest from one shoulder to the other, dipping to y_mid in the middle."""
    pts = []
    for k in range(n + 1):
        t = k / n
        x = -x_end + 2 * x_end * t
        y = y_end + (y_mid - y_end) * math.sin(math.pi * t)
        h = body.front(x, y, out)
        if h:
            pts.append(h[0])
    return pts


# ================================================================== the Court Herald

def herald_head(body):
    c, rim, R, H = head_frame(body)
    parts = []
    # a pillbox cap sitting on the crown, a trimmed band, a tall plume
    cap = W.cyl("hr_cap", R * 0.98, rim - 0.01, rim + H * 0.75, "dye_coat", x=c.y, z=0.0, r1=R * 0.92, verts=24, axis="Z", bevel=0.01)
    parts.append(cap)
    parts.append(W.cyl("hr_cap_top", R * 0.9, rim + H * 0.75, rim + H * 0.8, "dye_coat", x=c.y, z=0.0, verts=24, axis="Z", bevel=0.006))
    band_pts = ellipse(c, R + 0.004, R + 0.004, 26, rim + 0.012)
    parts.append(W.tube_path("hr_cap_band", [(p.x, p.y, p.z) for p in band_pts], 0.016, "dye_trim", sides=6, closed=True))
    # the plume: a stack of blades sweeping up and back from the right side
    base = c + LEFT * -(R * 0.7) + UP * (H * 0.6)
    for k in range(7):
        t = k / 6
        p = base + UP * (0.05 + 0.16 * t) - FWD * (0.02 + 0.12 * t * t)
        blade = W.box("hr_plume", (0.018, 0.05 - 0.02 * t, 0.07), (0, 0, 0), "dye_accent" if k % 2 == 0 else "feather", bevel=0.006)
        blade.rotation_euler = (math.radians(-20 - 40 * t), 0, math.radians(-10))
        blade.location = p
        parts.append(blade)
    parts.append(W.sphere("hr_plume_pin", 0.02, base, "dye_trim", seg=10, rings=6))
    return W.bake(parts, "hr_head")


def herald_cape(body):
    cape, rows = back_cape(body, "hr", "dye_coat", bottom=1.05, arc=(70, 290), flare=0.05)
    parts = [cape]
    hem = rows[-1]
    parts.append(W.tube_path("hr_cape_hem", [(p.x, p.y, p.z) for p in hem], 0.014, "dye_trim", sides=6))
    # the crest on the back: a shield of the accent, edged with the trim
    bp = body.outer(gv(0.0, 1.27, 0.0), -FWD, 0.1)
    bp = bp if bp is not None else gv(0.0, 1.27, -0.2)
    shield = W.top_profile("hr_crest", [(-0.07, 0.07), (0.07, 0.07), (0.07, -0.02), (0.0, -0.09), (-0.07, -0.02)], 0.012, "dye_accent", z=0.0, bevel=0.003)
    edge = W.top_profile("hr_crest_edge", [(-0.085, 0.085), (0.085, 0.085), (0.085, -0.025), (0.0, -0.105), (-0.085, -0.025)], 0.008, "dye_trim", z=-0.006, bevel=0.002)
    m = A.frame(bp, -FWD)
    for ob in (shield, edge):
        ob.matrix_world = m @ ob.matrix_world
        parts.append(ob)
    return W.bake(parts, "hr_cape")


def herald_neck(body):
    pts = chest_arc(body, 1.31, 1.44, 0.11, 0.11, n=12)
    parts = chain("hr_chain", pts, "dye_trim", 0.014)
    mid = body.front(0.0, 1.27, 0.12)
    p, n = mid if mid else (gv(0.0, 1.27, 0.2), FWD)
    m = A.frame(p, n)
    medal = W.cyl("hr_medal", 0.04, -0.006, 0.006, "dye_trim", verts=18, axis="Z", bevel=0.003)
    gem = W.cyl("hr_medal_gem", 0.018, 0.004, 0.012, "dye_accent", verts=12, axis="Z", bevel=0.002)
    for ob in (medal, gem):
        ob.matrix_world = m @ ob.matrix_world
        parts.append(ob)
    return W.bake(parts, "hr_neck")


def herald_body(body):
    parts, rows, centres = jacket(body, "hr", "dye_coat", skirt=(0.10, 0.18, 0.24), open_front=False, sleeve=0.19, sleeve_mat="dye_accent", sleeve_puff=0.025)
    # the tabard: four quarters over the front and back (coat and accent alternating), trimmed
    for side, sgn in (("front", 1), ("back", -1)):
        for qx in (0, 1):
            for qy in (0, 1):
                xs = [(-0.13 + 0.13 * qx) + 0.026 * i for i in range(6)]
                ys = [(1.38 - 0.2 * qy) - 0.04 * j for j in range(6)]
                mat = "dye_accent" if (qx + qy) % 2 == 0 else "dye_coat"
                parts.append(G.plate(f"hr_tabard_{side}", body, xs, ys, 0.085, mat, 0.01, side=side))
        edge = []
        for x in [-0.14 + 0.028 * i for i in range(11)]:
            if side == "front":
                h = body.front(x, 0.97, 0.095)
                if h:
                    edge.append(h[0])
            else:
                p = body.outer(gv(x, 0.97, 0.0), -FWD, 0.095)
                if p is not None:
                    edge.append(p)
        if len(edge) > 2:
            parts.append(W.tube_path(f"hr_tabard_hem_{side}", [(p.x, p.y, p.z) for p in edge], 0.01, "dye_trim", sides=6))
    # a belt with a big buckle
    belt = [centres[-1] + (p - centres[-1]) * 1.05 for p in rows[len(centres) - 1]]
    parts.append(W.tube_path("hr_belt", [(p.x, p.y, p.z) for p in belt], 0.02, "leather", sides=6, closed=True))
    bp = body.front(0.0, 0.97, 0.11)
    if bp:
        parts.append(W.box("hr_buckle", (0.07, 0.016, 0.05), (bp[0].x, bp[0].y, bp[0].z), "dye_trim", bevel=0.004))
    return W.bake(parts, "hr_body")


def herald_legs(body):
    parts, legs = leg_tubes(body, "hr", "dye_coat")
    for s, rows, centres in legs:
        for k, y in enumerate((0.86, 0.74, 0.62, 0.40, 0.30)):
            parts.append(band(f"hr_stripe_{s}{k}", rows, centres, y, "dye_accent", 1.05, 0.02))
        parts.append(band(f"hr_garter_{s}", rows, centres, 0.5, "dye_trim", 1.08, 0.014))
    return W.bake(parts, "hr_legs")


def herald_hands(body):
    return W.bake(gloves(body, "hr", "leather", cuff=0.045, cuff_mat="dye_trim") + [], "hr_hands")


def herald_feet(body):
    parts, feet = boots(body, "hr", "dye_coat", top=0.25, round_=0.75)
    for s, rows, centres, foot in feet:
        bp = body.front(foot.x, 0.09, 0.05)
        if bp:
            parts.append(W.box(f"hr_shoe_buckle_{s}", (0.05, 0.012, 0.04), (bp[0].x, bp[0].y, bp[0].z), "dye_trim", bevel=0.004))
        parts.append(band(f"hr_shoe_top_{s}", rows, centres, 0.25, "dye_accent", 1.05, 0.012))
    return W.bake(parts, "hr_feet")


# ================================================================== the River Smuggler

def smuggler_head(body):
    c, rim, R, H = head_frame(body)
    parts = []
    crown = A.dome("sm_crown", Vector((0.0, c.y, rim)), (R, R * 1.05, H * 0.9), "dye_coat", 0.0, keep=-0.05)
    A.solidify(crown, 0.012, 1.0)
    parts.append(crown)
    # the brim: narrow at the front, wide and sloping down over the neck at the back
    n = 28
    inner, outer = [], []
    for k in range(n):
        a = 2 * math.pi * k / n
        d = Vector((math.sin(a), -math.cos(a), 0.0))
        back = max(0.0, -d.y)
        inner.append(Vector((0.0, c.y, rim - 0.01)) + Vector((d.x * R, d.y * R * 1.05, 0.0)))
        w = 0.05 + 0.09 * back
        outer.append(Vector((0.0, c.y, rim - 0.01 - 0.07 * back)) + Vector((d.x * (R + w), d.y * (R * 1.05 + w), 0.0)))
    brim = A.grid("sm_brim", [inner, outer], "dye_coat", closed=True)
    A.solidify(brim, 0.012, 0.0)
    parts.append(brim)
    parts.append(W.tube_path("sm_hatband", [(p.x, p.y, p.z + 0.012) for p in inner], 0.012, "dye_trim", sides=6, closed=True))
    # a leather chin strap
    for s in (1, -1):
        a = Vector((s * R * 0.95, c.y, rim - 0.02))
        b = Vector((s * 0.05, c.y - 0.06, rim - 0.17))
        parts.append(W.tube_path(f"sm_strap_{s}", [(a.x, a.y, a.z), (b.x, b.y, b.z)], 0.006, "leather", sides=5))
    return W.bake(parts, "sm_head")


def smuggler_cape(body):
    cape, rows = back_cape(body, "sm", "dye_coat", bottom=0.95, arc=(60, 300), flare=0.06)
    parts = [cape]
    # the hood, down: a thick roll behind the neck
    neck = body.a["neck_01"]
    roll = [Vector((math.sin(math.radians(a)) * 0.13, neck.y + math.cos(math.radians(a)) * 0.11, neck.z - 0.02 + 0.03 * math.cos(math.radians(a)))) for a in range(110, 251, 14)]
    parts.append(W.tube_path("sm_hood_roll", [(p.x, p.y, p.z) for p in roll], 0.045, "dye_coat", sides=8))
    # toggles on the shoulders' front edges
    for s in (1, -1):
        h = body.front(s * 0.15, 1.4, 0.07)
        if h:
            parts.append(W.cyl(f"sm_toggle_{s}", 0.009, -0.03, 0.03, "dye_trim", x=h[0].y, z=h[0].z, verts=6, axis="X"))
    return W.bake(parts, "sm_cape")


def smuggler_neck(body):
    neck = body.a["neck_01"]
    parts = []
    ring = ellipse(Vector((0.0, neck.y, neck.z - 0.03)), 0.08, 0.075, 18)
    parts.append(W.tube_path("sm_kerchief", [(p.x, p.y, p.z) for p in ring], 0.024, "dye_accent", sides=8, closed=True))
    h = body.front(0.0, neck.z - 0.07, 0.045)
    p, n = h if h else (gv(0.0, 1.45, 0.12), FWD)
    parts.append(W.sphere("sm_knot", 0.03, p, "dye_accent", seg=10, rings=6, scale=(1.2, 0.8, 1.0)))
    tri = W.top_profile("sm_kerchief_tail", [(-0.06, 0.0), (0.06, 0.0), (0.0, -0.1)], 0.01, "dye_accent", z=0.0, bevel=0.003)
    tri.matrix_world = A.frame(p - UP * 0.02, n) @ tri.matrix_world
    parts.append(tri)
    return W.bake(parts, "sm_neck")


def smuggler_body(body):
    parts, rows, centres = jacket(body, "sm", "dye_coat", skirt=(0.06, 0.1), open_front=True, sleeve=0.26, sleeve_mat="linen")
    # the rolled sleeve ends, pockets and buttons on the waistcoat
    for s in (1, -1):
        ua = body.a["upperarm_l" if s > 0 else "upperarm_r"]
        la = body.a["lowerarm_l" if s > 0 else "lowerarm_r"]
        axis = (la - ua).normalized()
        ring = G.ring_around(body, ua + axis * 0.25, axis, UP, 14, 0.03, 0.045, 0.06)
        parts.append(W.tube_path(f"sm_roll_{s}", [(p.x, p.y, p.z) for p in ring], 0.013, "linen", sides=6, closed=True))
        h = body.front(s * 0.09, 1.08, 0.1)
        if h:
            parts.append(W.box(f"sm_pocket_{s}", (0.09, 0.02, 0.07), (h[0].x, h[0].y, h[0].z), "dye_coat", bevel=0.006))
            parts.append(W.box(f"sm_flap_{s}", (0.095, 0.022, 0.02), (h[0].x, h[0].y - 0.004, h[0].z + 0.035), "dye_trim", bevel=0.004))
    for k in range(5):
        h = body.front(0.03, 1.36 - 0.08 * k, 0.1)
        if h:
            parts.append(W.sphere("sm_button", 0.011, h[0], "dye_trim", seg=8, rings=5))
    return W.bake(parts, "sm_body")


def smuggler_legs(body):
    parts, legs = leg_tubes(body, "sm", "dye_coat", top=0.93, bottom=0.48)
    st, sl = leg_tubes(body, "sm_stocking", "wool", table=((0.93, 0.08), (0.5, 0.072), (0.4, 0.066), (0.25, 0.062), (0.0, 0.062)), top=0.55, bottom=0.2)
    parts += st
    for s, rows, centres in legs:
        parts.append(band(f"sm_cuff_{s}", rows, centres, 0.48, "dye_coat", 1.12, 0.03))
        parts.append(band(f"sm_cuff_tie_{s}", rows, centres, 0.5, "dye_trim", 1.16, 0.008))
    return W.bake(parts, "sm_legs")


def smuggler_hands(body):
    parts = gloves(body, "sm", "leather", cuff=0.02, fingers=False)
    for s in (1, -1):
        hand = body.a["hand_l" if s > 0 else "hand_r"]
        lower = body.a["lowerarm_l" if s > 0 else "lowerarm_r"]
        axis = (hand - lower).normalized()
        ring = G.ring_around(body, hand + axis * -0.03, axis, UP, 14, 0.03, 0.04, 0.07)
        parts.append(W.tube_path(f"sm_wrist_{s}", [(p.x, p.y, p.z) for p in ring], 0.008, "dye_trim", sides=5, closed=True))
    return W.bake(parts, "sm_hands")


def smuggler_feet(body):
    parts, feet = boots(body, "sm", "dye_coat", top=0.52, round_=0.8)
    for s, rows, centres, foot in feet:
        parts.append(band(f"sm_boot_top_{s}", rows, centres, 0.5, "dye_coat", 1.14, 0.03))
        parts.append(band(f"sm_boot_strap_{s}", rows, centres, 0.2, "dye_trim", 1.06, 0.01))
    return W.bake(parts, "sm_feet")


# ==================================================================== the Ash Pilgrim

def pilgrim_head(body):
    c, rim, R, H = head_frame(body)
    parts = []
    # a deep hood: a big dome over the head, open at the face, its tail down onto the shoulders
    hood = A.dome("ap_hood", Vector((0.0, c.y + 0.015, rim - 0.04)), (R + 0.035, R * 1.1 + 0.035, H + 0.04), "dye_coat", 0.0, keep=-0.6)
    bm = G.bmesh.new()
    bm.from_mesh(hood.data)
    mw = hood.matrix_world
    gone = [f for f in bm.faces if (mw @ f.calc_center_median()).y < c.y - 0.05 and (mw @ f.calc_center_median()).z < rim + 0.02 and abs((mw @ f.calc_center_median()).x) < 0.085]
    G.bmesh.ops.delete(bm, geom=gone, context="FACES")
    bm.to_mesh(hood.data)
    bm.free()
    A.solidify(hood, 0.016, 1.0)
    parts.append(hood)
    # the hood's edge round the face (the trim), the cloth mask over the mouth and nose
    edge = [Vector((math.sin(math.radians(a)) * 0.095, c.y - (R * 1.1 + 0.02) * 0.8, rim - 0.11 + 0.15 * math.cos(math.radians(a)))) for a in range(-80, 81, 16)]
    parts.append(W.tube_path("ap_hood_edge", [(p.x, p.y, p.z) for p in edge], 0.014, "dye_trim", sides=6))
    # a cloth wrap round the hood's base, over the neck (the accent)
    neck = body.a["neck_01"]
    wrap = ellipse(Vector((0.0, neck.y + 0.01, neck.z + 0.01)), 0.11, 0.1, 20)
    parts.append(W.tube_path("ap_wrap", [(p.x, p.y, p.z) for p in wrap], 0.03, "dye_accent", sides=8, closed=True))
    return W.bake(parts, "ap_head")


def pilgrim_cape(body):
    cape, rows = back_cape(body, "ap", "dye_coat", bottom=0.56, arc=(80, 280), flare=0.045)
    parts = [cape]
    # notched blocks along the hem, a patch, a strap across the chest
    hem = rows[-1]
    for k in range(0, len(hem), 2):
        p = hem[k]
        parts.append(W.box("ap_hem_block", (0.06, 0.02, 0.05), (p.x, p.y, p.z - 0.02), "dye_trim", bevel=0.006))
    pts = chest_arc(body, 1.4, 1.42, 0.12, 0.07, n=8)
    if len(pts) > 2:
        parts.append(W.tube_path("ap_clasp_cord", [(q.x, q.y, q.z) for q in pts], 0.008, "rope", sides=5))
    return W.bake(parts, "ap_cape")


def pilgrim_neck(body):
    pts = chest_arc(body, 1.2, 1.44, 0.1, 0.1, n=14)
    parts = []
    for k, q in enumerate(pts):
        parts.append(W.sphere("ap_bead", 0.017 if k % 3 else 0.022, q, "dye_trim" if k % 3 else "wood", seg=8, rings=6))
    bottom = body.front(0.0, 1.14, 0.12)
    p, n = bottom if bottom else (gv(0.0, 1.14, 0.2), FWD)
    m = A.frame(p, n)
    censer = W.cyl("ap_censer", 0.03, -0.035, 0.035, "iron", verts=8, axis="Y", bevel=0.004)
    slots = [W.box("ap_censer_glow", (0.012, 0.004, 0.035), (0.016 * (k - 1), 0.0, 0.031), "glow_accent", bevel=0.0) for k in range(3)]
    lid = W.cyl("ap_censer_lid", 0.032, 0.035, 0.05, "iron", r1=0.008, verts=8, axis="Y", bevel=0.002)
    for ob in [censer, lid] + slots:
        ob.matrix_world = m @ ob.matrix_world
        parts.append(ob)
    return W.bake(parts, "ap_neck")


def pilgrim_body(body):
    parts, rows, centres = jacket(body, "ap", "dye_coat", skirt=(0.12, 0.24, 0.36), open_front=False, sleeve=0.2)
    # wraps across the chest (the trim), a rope belt
    for s in (1, -1):
        path = []
        for k in range(10):
            t = k / 9
            h = body.front(s * (0.14 - 0.28 * t), 1.40 - 0.34 * t, 0.095)
            if h:
                path.append(h[0])
        if len(path) > 3:
            parts.append(W.tube_path(f"ap_wrap_{s}", [(p.x, p.y, p.z) for p in path], 0.016, "dye_trim", sides=6))
    belt = [centres[-1] + (p - centres[-1]) * 1.06 for p in rows[len(centres) - 1]]
    parts.append(W.tube_path("ap_belt", [(p.x, p.y, p.z) for p in belt], 0.016, "rope", sides=6, closed=True))
    h = body.front(0.06, 0.95, 0.11)
    if h:
        parts.append(W.tube_path("ap_belt_end", [(h[0].x, h[0].y, h[0].z), (h[0].x + 0.01, h[0].y - 0.01, h[0].z - 0.16)], 0.012, "rope", sides=5))
    return W.bake(parts, "ap_body")


def pilgrim_legs(body):
    parts, legs = leg_tubes(body, "ap", "dye_coat", table=((0.93, 0.096), (0.62, 0.08), (0.4, 0.068), (0.0, 0.064)))
    for s, rows, centres in legs:
        for k, y in enumerate((0.84, 0.70, 0.56, 0.44, 0.34, 0.26)):
            parts.append(band(f"ap_wrapband_{s}{k}", rows, centres, y, "dye_trim", 1.05, 0.013))
    return W.bake(parts, "ap_legs")


def pilgrim_hands(body):
    parts = gloves(body, "ap", "dye_coat", cuff=0.02)
    for s in (1, -1):
        hand = body.a["hand_l" if s > 0 else "hand_r"]
        lower = body.a["lowerarm_l" if s > 0 else "lowerarm_r"]
        axis = (hand - lower).normalized()
        for k, t in enumerate((-0.12, -0.06, 0.04, 0.1)):
            ring = G.ring_around(body, hand + axis * t, axis, UP, 12, 0.026, 0.03, 0.08)
            parts.append(W.tube_path(f"ap_bandage_{s}{k}", [(p.x, p.y, p.z) for p in ring], 0.007, "dye_trim", sides=5, closed=True))
    return W.bake(parts, "ap_hands")


def pilgrim_feet(body):
    parts, feet = boots(body, "ap", "dye_coat", top=0.25, round_=0.75)
    for s, rows, centres, foot in feet:
        sole = rows[-2]
        lc = sum(sole, Vector()) / len(sole)
        out = [lc + (p - lc) * 1.12 - UP * 0.012 for p in sole]
        parts.append(W.tube_path(f"ap_sole_{s}", [(p.x, p.y, p.z) for p in out], 0.012, "leather", sides=5, closed=True))
        for y in (0.06, 0.14, 0.22):
            parts.append(band(f"ap_strap_{s}{y}", rows, centres, y, "leather", 1.05, 0.008))
    return W.bake(parts, "ap_feet")


SETS = {
    "herald": [herald_head, herald_cape, herald_neck, herald_body, herald_legs, herald_hands, herald_feet],
    "smuggler": [smuggler_head, smuggler_cape, smuggler_neck, smuggler_body, smuggler_legs, smuggler_hands, smuggler_feet],
    "pilgrim": [pilgrim_head, pilgrim_cape, pilgrim_neck, pilgrim_body, pilgrim_legs, pilgrim_hands, pilgrim_feet],
}


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    renders = "--no-renders" not in argv
    sets = [a for a in argv if a in SETS] or list(SETS)
    with open(os.path.join(REF, "anchors.json")) as f:
        anchors = G.json.load(f)
    os.makedirs(SHOTS, exist_ok=True)
    for set_name in sets:
        textures = {n: os.path.join(REF, f"look_{set_name}_{n}_tex.png") for n in NAMES}
        for name in NAMES:
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
            W.export_glb(parts, os.path.join(OUT, f"look_{set_name}_{name}.glb"))
            print(f"[outfits] {set_name} {name}: {tris} triangles, pieces {[o.name for o in parts]}")
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


main()
