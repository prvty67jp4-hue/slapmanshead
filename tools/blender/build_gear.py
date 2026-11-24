"""Ledger Duels gear: the Bridgewright's Rig (the Bridgeworks crane-riggers' set), fitted onto each
fighter's own body, for the game to skin onto their skeleton.

    cargo test --release dump_fighter_refs -- --ignored      (once: writes tools/blender/ref/)
    blender --background --factory-startup --python app/tools/blender/build_gear.py [-- names] [-- --no-renders]

The look of all Ledger gear: halfway between chunky low-poly MMO gear and the toon-shaded fighters:
big readable silhouettes, blocky plates, the same outlines and toon shading as the fighters (the
game draws gear with the fighters' shader), and two or three dye zones a piece. A dye zone is a
material: `dye_coat`, `dye_trim`, `dye_accent` (and `glow_accent`, drawn unlit) are baked in pale
neutral tones and the game tints them with the wearer's dyes (by default oiled brown canvas, brass
trim and a teal accent); everything else (dark iron, leather straps, rope) keeps its own colour.

The pieces (each its own mesh, named for the game: see GEAR_PIECES in src/chars.rs):
- bw_head: a riveted iron half-helm, a brass lens over one eye, a rolled leather brim.
- bw_body: a thick canvas work coat over a chunky iron chest plate, a crossed tool harness with
  hooks, a coiled rope on the back, a teal collar.
- bw_legs: padded canvas trousers with iron knee caps.
- bw_hands: oversized leather gauntlets with brass knuckle plates.
- bw_feet: heavy hobnailed boots with iron ankle guards.
- bw_charm: a small brass gear on a chain at the chest; its hub glows teal in the game while the
  set's bonus is on.

Writes app/assets/characters/gear_bridgewright_<name>.glb (one per fighter), app/assets/characters/
gear.png (the four bakes, a quarter each like the accessories), and turntables of each fighter in
the full set (front and back) to app/shots/character-art/gear/.
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

import build_accessories as A  # noqa: E402  (its body-fitting helpers and paints)
import weapons_lib as W  # noqa: E402
import weapons_tex as T  # noqa: E402

APP = A.APP
REF = A.REF
OUT = A.OUT
SHOTS = os.path.join(APP, "shots", "character-art", "gear")
NAMES = A.NAMES
QUARTER = A.QUARTER
TEX = 512
gv, game, UP, FWD, LEFT = A.gv, A.game, A.UP, A.FWD, A.LEFT

# the dye zones are pale and neutral (the game multiplies them by the wearer's dyes)
W.PALETTE.update({
    "dye_coat": ((0.62, 0.60, 0.57), 0.0, 0.85),
    "dye_trim": ((0.72, 0.72, 0.70), 0.9, 0.34),
    "dye_accent": ((0.70, 0.70, 0.70), 0.0, 0.7),
    "glow_accent": ((0.92, 0.92, 0.92), 0.0, 0.4),
    "rope": ((0.30, 0.24, 0.15), 0.0, 0.9),
})
T.RECIPES.update({
    "dye_coat": ("cord", (0.50, 0.49, 0.47), (0.68, 0.67, 0.64), (0.30, 0.29, 0.28), 0.45, 0.6),
    "dye_trim": ("metal", (0.55, 0.55, 0.54), (0.72, 0.72, 0.70), (0.96, 0.96, 0.95), 0.75, 0.6),
    "dye_accent": ("cord", (0.60, 0.60, 0.60), (0.74, 0.74, 0.74), (0.42, 0.42, 0.42), 0.3, 0.5),
    "glow_accent": ("glass", (0.88, 0.88, 0.88), (1.0, 1.0, 1.0), (1.0, 1.0, 1.0), 0.2, 0.0),
    "rope": ("cord", (0.22, 0.17, 0.10), (0.38, 0.30, 0.18), (0.12, 0.09, 0.05), 0.45, 0.7),
})
# the set's own colours, for the preview pictures (the game uses the wearer's dyes)
DEFAULT_DYES = {"dye_coat": (0.20, 0.11, 0.05), "dye_trim": (0.62, 0.42, 0.14), "dye_accent": (0.03, 0.42, 0.40), "glow_accent": (0.05, 0.75, 0.70)}


# ---------------------------------------------------------------- fitting helpers

def ring_around(body, centre, axis, side, n, out, rmin, rmax, smooth=True, own_side=False):
    """A loop round `axis` through `centre`: each point on the outermost surface seen from
    outside (clamped to rmin..rmax from the axis), `out` metres off it. With own_side (a leg
    or a boot) no point crosses the body's middle: rays towards the other leg stop short of it."""
    axis = axis.normalized()
    side = (side - axis * side.dot(axis)).normalized()
    third = axis.cross(side).normalized()
    rs = []
    for k in range(n):
        a = 2 * math.pi * k / n
        d = (side * math.cos(a) + third * math.sin(a)).normalized()
        p = body.outer(centre, d)
        rs.append((p - centre).length if p is not None else rmin)
    if smooth:
        rs = [(rs[k - 1] + 2 * rs[k] + rs[(k + 1) % n]) / 4 for k in range(n)]
    pts = []
    for k in range(n):
        a = 2 * math.pi * k / n
        d = (side * math.cos(a) + third * math.sin(a)).normalized()
        r = min(max(rs[k], rmin), rmax) + out
        if own_side and abs(centre.x) > 0.02 and d.x * centre.x < 0:
            r = min(r, (abs(centre.x) - 0.012) / max(abs(d.x), 1e-4))
        pts.append(centre + d * r)
    return pts


def fair_rows(rows, centres, roundness=0.8, passes=3):
    """Even out a sleeve's rings (each round its centre) so it reads as one padded, blocky
    shell instead of following every buckle and pad of the outfit under it: each ring is pushed
    out towards round (`roundness` of its widest), every point is at least as far out as the
    rings either side of it (no dents between them), then smoothed round and down. The fighter
    under it is hidden in the game, so it only has to enclose it."""
    n = len(rows[0])
    dirs, rs = [], []
    for ring, c in zip(rows, centres):
        d = [(p - c) for p in ring]
        rs.append([v.length for v in d])
        dirs.append([v.normalized() if v.length > 1e-6 else Vector((1, 0, 0)) for v in d])
    for r in rs:
        top = max(r)
        for k in range(n):
            r[k] = max(r[k], top * roundness)
    rs = [[max(rs[j][k] for j in range(max(0, i - 1), min(len(rs), i + 2))) for k in range(n)] for i in range(len(rs))]
    for _ in range(passes):
        rs = [[(r[k - 1] + 2 * r[k] + r[(k + 1) % n]) / 4 for k in range(n)] for r in rs]
        rs = [[(rs[max(i - 1, 0)][k] + 2 * rs[i][k] + rs[min(i + 1, len(rs) - 1)][k]) / 4 for k in range(n)] for i in range(len(rs))]
    return [[c + dirs[i][k] * rs[i][k] for k in range(n)] for i, c in enumerate(centres)]


def tube_shell(name, rows, mat, thick, cap=None):
    """A closed sleeve through rings of points (each the same length), thickened inward."""
    ob = A.grid(name, rows, mat, closed=True)
    return A.solidify(ob, thick, -1.0)


def plate(name, body, xs, ys, out, mat, thick, side="front"):
    """A plate over the front (or back) of the body: a grid sampled on its surface."""
    rows = []
    for y in ys:
        row = []
        for x in xs:
            if side == "front":
                h = body.front(x, y, out)
                p = h[0] if h else gv(x, y, 0.16)
            else:
                p = body.outer(gv(x, y, 0.0), -FWD, out)
                p = p if p is not None else gv(x, y, -0.16)
            row.append(p)
        rows.append(row)
    return A.solidify(A.grid(name, rows, mat), thick, 0.0)


def rivet(name, p, n, mat, r=0.008):
    return W.sphere(name, r, p + n * r * 0.4, mat, seg=8, rings=5, scale=(1, 1, 0.7))


# ---------------------------------------------------------------- the pieces

def head(body):
    hc = body.a["Head"]
    c = Vector((0.0, hc.y + 0.02, 0.0))
    parts = []
    # the half-helm: an iron dome over the crown (outside a hood), its rim at the brow. Sized
    # from the head as measured: its widest point (whatever is outermost there) and its top.
    h0 = hc.z
    rim_y = h0 + 0.145
    widest = 0.08
    for k in range(24):
        a = 2 * math.pi * k / 24
        d = (FWD * math.cos(a) + LEFT * math.sin(a)).normalized()
        for dy in (0.0, 0.04, 0.08):
            cen = Vector((0.0, c.y, rim_y + dy))
            p = body.outer(cen, d)
            if p is not None and (p - cen).length < 0.22:
                widest = max(widest, (p - cen).length)
    top_hit = body.ray(Vector((0.0, c.y, h0 + 0.6)), -UP)
    top = top_hit[0].z if top_hit is not None and top_hit[0].z > rim_y else h0 + 0.22
    R = widest + 0.03
    H = max(top - rim_y + 0.035, 0.08)
    helm = A.dome("helm", Vector((0.0, c.y, rim_y)), (R, R * 1.06, H), "iron", 0.0, keep=-0.05)
    A.solidify(helm, 0.014, 1.0)
    parts.append(helm)
    # a raised iron rib from the brow over the crown to the nape
    rib = []
    for k in range(11):
        a = math.radians(-80 + 160 * k / 10)
        rib.append(Vector((0.0, c.y, rim_y)) + Vector((0.0, -math.sin(a) * R * 1.06, math.cos(a) * H)) * 1.03)
    parts.append(W.tube_path("helm_rib", [(p.x, p.y, p.z) for p in rib], 0.012, "iron", sides=6))
    rows = [[Vector((0.0, c.y, rim_y)) + Vector((math.sin(2 * math.pi * k / 24) * R, -math.cos(2 * math.pi * k / 24) * R * 1.06, -0.05 * H)) for k in range(24)]]    # rivets round the rim (the trim zone)
    rim = rows[-1]
    for k in range(0, len(rim), 3):
        p = rim[k]
        n = (p - Vector((c.x, c.y, p.z))).normalized()
        parts.append(rivet("helm_rivet", p + n * 0.006, n, "dye_trim", 0.008))
    # the rolled leather brim (the coat zone)
    brim = [Vector((0.0, c.y, rim_y - 0.05 * H)) + Vector((math.sin(2 * math.pi * k / 28) * (R + 0.012), -math.cos(2 * math.pi * k / 28) * (R + 0.012) * 1.06, 0.0)) for k in range(28)]
    parts.append(W.tube_path("helm_brim", [(p.x, p.y, p.z) for p in brim], 0.018, "dye_coat", sides=8, closed=True))
    # the brass lens over the right eye: a thick ring, the glass in it, a little hinge arm
    hit = body.front(-0.034, h0 + 0.105, 0.028)
    p, n = hit if hit else (gv(-0.034, 1.705, 0.12), FWD)
    m = A.frame(p, n)
    lens_ring = W.ring("lens_ring", 0.024, 0.006, (0, 0, 0), "dye_trim", axis="Z", maj=18, minr=6)
    glass = W.cyl("lens_glass", 0.02, -0.003, 0.003, "dye_accent", verts=16, axis="Z", bevel=0.0)
    arm = W.box("lens_arm", (0.05, 0.01, 0.008), (-0.045, 0.012, -0.004), "dye_trim", bevel=0.002)
    for ob in (lens_ring, glass, arm):
        ob.matrix_world = m @ ob.matrix_world
        parts.append(ob)
    return W.bake(parts, "bw_head")


def body_piece(body):
    parts = []
    # the coat: rings round the torso from the collar to below the hips, then a stiff skirt
    rows = []
    ys = [1.43, 1.38, 1.31, 1.24, 1.17, 1.10, 1.03, 0.97]
    depth = body.a["spine_02"].y  # (Blender y: the body's front-back middle)
    centres = []
    for y in ys:
        centres.append(Vector((0.0, depth, y)))
        rows.append(ring_around(body, centres[-1], UP, FWD, 28, 0.05, 0.09, 0.24))
    rows = fair_rows(rows, centres, 0.75, 2)
    # the skirt hangs from the hips, flaring a little
    hip = rows[-1]
    hc = sum(hip, Vector()) / len(hip)
    for k, drop in enumerate((0.10, 0.20, 0.28)):
        flare = 1.0 + 0.07 * (k + 1)
        rows.append([hc + (p - hc) * flare - UP * drop for p in hip])
    coat = A.grid("coat", rows, "dye_coat", closed=True)
    # a yoke over the tops of the shoulders, from the coat's top edge in to the neck
    neck = body.a["neck_01"]
    ny = min(max(neck.z + 0.02, 1.48), 1.56)
    c0 = centres[0]
    yrows = [list(rows[0])]
    yrows.append([Vector((c0.x + (p.x - c0.x) * 0.92, neck.y + (p.y - neck.y) * 0.8, 1.475)) for p in rows[0]])
    n = len(rows[0])
    neck_ring = []
    for k in range(n):
        d = (rows[0][k] - c0)
        d = Vector((d.x, d.y, 0.0)).normalized()
        neck_ring.append(Vector((neck.x + d.x * 0.095, neck.y + d.y * 0.085, ny)))
    yrows.append(neck_ring)
    yoke = A.grid("coat_yoke", yrows, "dye_coat", closed=True)
    A.solidify(yoke, 0.016, -1.0)
    parts.append(yoke)
    # open the skirt at the front (a coat, not a dress)
    bm = bmesh.new()
    bm.from_mesh(coat.data)
    gone = [f for f in bm.faces if f.calc_center_median().z < 0.95 and f.calc_center_median().y < -0.02 and abs(f.calc_center_median().x) < 0.07]
    bmesh.ops.delete(bm, geom=gone, context="FACES")
    bm.to_mesh(coat.data)
    bm.free()
    A.solidify(coat, 0.016, -1.0)
    parts.append(coat)
    # short canvas sleeves over the tops of the arms, an iron pad on each shoulder
    for s in (1, -1):
        ua = body.a["upperarm_l" if s > 0 else "upperarm_r"]
        la = body.a["lowerarm_l" if s > 0 else "lowerarm_r"]
        axis = (la - ua).normalized()
        cs = [ua + axis * t for t in (-0.04, 0.04, 0.12, 0.19)]
        srows = [ring_around(body, c, axis, UP, 16, 0.022, 0.045, 0.06) for c in cs]
        sleeve = tube_shell(f"coat_sleeve_{s}", fair_rows(srows, cs, 0.9, 2), "dye_coat", 0.014)
        parts.append(sleeve)
        hit = body.outer(ua + axis * 0.03, UP, 0.05)
        if hit is not None:
            pad = A.dome(f"shoulder_pad_{s}", hit - UP * 0.03, (0.085, 0.075, 0.05), "iron", 0.0, keep=-0.2)
            A.solidify(pad, 0.012, 1.0)
            parts.append(pad)
            parts.append(rivet(f"pad_rivet_{s}", hit + UP * 0.02, UP, "dye_trim", 0.01))
    # the teal collar
    collar = ring_around(body, Vector((0.0, 0.0, 1.445)), UP, FWD, 26, 0.04, 0.07, 0.2)
    parts.append(W.tube_path("collar", [(p.x, p.y, p.z) for p in collar], 0.022, "dye_accent", sides=8, closed=True))
    # the iron chest plate, riveted at its corners, a brass edge
    xs = [-0.13 + 0.026 * i for i in range(11)]
    ys = [1.40 - 0.03 * j for j in range(9)]
    chest = plate("chest_plate", body, xs, ys, 0.062, "iron", 0.016)
    parts.append(chest)
    for (x, y) in ((-0.115, 1.385), (0.115, 1.385), (-0.115, 1.17), (0.115, 1.17)):
        h = body.front(x, y, 0.082)
        if h:
            parts.append(rivet("plate_rivet", h[0], h[1], "dye_trim", 0.011))
    edge = []
    for x in xs:
        h = body.front(x, 1.405, 0.074)
        if h:
            edge.append(h[0])
    if len(edge) > 2:
        parts.append(W.tube_path("plate_edge", [(p.x, p.y, p.z) for p in edge], 0.008, "dye_trim", sides=6))
    # the crossed tool harness: two straps over the shoulders crossing on the back, buckles and
    # two iron hooks hanging at the hips
    for s in (1, -1):
        path = []
        for k in range(14):
            t = k / 13
            # front: from the shoulder down to the opposite hip
            x = s * (0.12 - 0.24 * t)
            y = 1.42 - 0.40 * t
            h = body.front(x, y, 0.105)
            if h:
                path.append(h[0])
        if len(path) > 3:
            strap = []
            for k, p in enumerate(path):
                d = (path[min(k + 1, len(path) - 1)] - path[max(k - 1, 0)]).normalized()
                side = d.cross(FWD).normalized()
                strap.append([p - side * 0.024, p + side * 0.024])
            parts.append(A.solidify(A.grid(f"harness_front_{s}", strap, "leather"), 0.008, 0.0))
            parts.append(W.box("harness_buckle", (0.05, 0.012, 0.04), (path[len(path) // 2].x, path[len(path) // 2].y - 0.01, path[len(path) // 2].z), "dye_trim", bevel=0.004))
        back = []
        for k in range(12):
            t = k / 11
            x = s * (0.14 - 0.28 * t)
            y = 1.40 - 0.36 * t
            p = body.outer(gv(x, y, 0.0), -FWD, 0.09)
            if p is not None:
                back.append(p)
        if len(back) > 3:
            strap = []
            for k, p in enumerate(back):
                d = (back[min(k + 1, len(back) - 1)] - back[max(k - 1, 0)]).normalized()
                side = d.cross(FWD).normalized()
                strap.append([p - side * 0.024, p + side * 0.024])
            parts.append(A.solidify(A.grid(f"harness_back_{s}", strap, "leather"), 0.008, 0.0))
        # a hook hanging at each hip
        h = body.front(s * 0.15, 1.02, 0.11)
        if h:
            hp = h[0]
            hook = W.tube_path(f"tool_hook_{s}", [(hp.x, hp.y, hp.z), (hp.x, hp.y - 0.01, hp.z - 0.07), (hp.x + s * 0.01, hp.y - 0.03, hp.z - 0.11), (hp.x + s * 0.03, hp.y - 0.035, hp.z - 0.10), (hp.x + s * 0.035, hp.y - 0.03, hp.z - 0.08)], 0.009, "iron", sides=6)
            parts.append(hook)
            parts.append(W.ring(f"hook_ring_{s}", 0.016, 0.005, (hp.x, hp.y, hp.z + 0.01), "dye_trim", axis="Y", maj=12, minr=5))
    # the coiled rope on the back: three fat loops stacked on the shoulder blades
    bp = body.outer(gv(0.0, 1.27, 0.0), -FWD, 0.12)
    bp = bp if bp is not None else gv(0.0, 1.27, -0.2)
    for k in range(3):
        r = W.ring("rope_coil", 0.10 - 0.008 * k, 0.017, (bp.x, bp.y + 0.012 * k, bp.z + 0.0), "rope", axis="Y", maj=24, minr=7)
        r.rotation_euler = (0, 0, math.radians(8 * (k - 1)))
        parts.append(r)
    parts.append(W.box("rope_tie", (0.03, 0.05, 0.12), (bp.x, bp.y + 0.03, bp.z + 0.0), "leather", bevel=0.006))
    # (no shrinkwrap: the faired coat already encloses the fighter, who is hidden under it in
    # the game; fitting it to every pad and buckle is what made it ragged)
    return W.bake(parts, "bw_body")


def legs(body):
    parts = []
    for s in (1, -1):
        thigh = body.a["thigh_l" if s > 0 else "thigh_r"]
        calf = body.a["calf_l" if s > 0 else "calf_r"]
        rows, centres = [], []
        for y in [0.93, 0.86, 0.78, 0.70, 0.62, 0.55, 0.48, 0.40, 0.32, 0.25, 0.20]:
            t = (thigh.z - y) / max(thigh.z - calf.z, 0.01)
            cx = thigh.x + (calf.x - thigh.x) * min(max(t, 0.0), 1.0)
            cy = thigh.y + (calf.y - thigh.y) * min(max(t, 0.0), 1.0)
            # a leg alone (not the other one): rays from close in
            centres.append(Vector((cx, cy, y)))
            # a padded, tapering tube round the leg's own bones (the fighter's legs and whatever
            # hangs round them are hidden under it in the game)
            table = ((0.93, 0.100), (0.78, 0.092), (0.62, 0.082), (0.50, 0.076), (0.40, 0.069), (0.25, 0.066), (0.0, 0.066))
            r = next(rr for (yy, rr) in table if y >= yy) if y < 0.93 else 0.100
            for (ya, ra), (yb, rb) in zip(table, table[1:]):
                if yb <= y <= ya:
                    r = rb + (ra - rb) * (y - yb) / (ya - yb)
            ring = []
            for k in range(20):
                a = 2 * math.pi * k / 20
                ring.append(centres[-1] + FWD * (math.cos(a) * r * 0.92) + LEFT * (math.sin(a) * r))
            rows.append(ring)
        # (and never over the middle, where the other leg is)
        rows = [[Vector((p.x if s * p.x > 0.004 else s * 0.004, p.y, p.z)) for p in r] for r in rows]
        leg = tube_shell(f"trouser_{s}", rows, "dye_coat", 0.014)
        A.smooth(leg, 1)
        parts.append(leg)
        # the iron knee cap on the front of the trouser leg, a brass rivet in it
        knee = min(range(len(centres)), key=lambda i: abs(centres[i].z - (calf.z + 0.02)))
        front = min(rows[knee], key=lambda p: p.y)
        h = (front + FWD * 0.012, FWD)
        if h:
            p, n = h
            cap = A.dome(f"knee_{s}", p, (0.06, 0.05, 0.035), "iron", 0.0)
            cap.matrix_world = A.frame(p, n) @ Matrix.Scale(1.0, 4)
            parts.append(cap)
            parts.append(rivet(f"knee_rivet_{s}", p + n * 0.035, n, "dye_trim", 0.011))
        # a teal stitched band below the knee
        bi = min(range(len(centres)), key=lambda i: abs(centres[i].z - (calf.z - 0.10)))
        band = [centres[bi] + (p - centres[bi]) * 1.05 for p in rows[bi]]
        parts.append(W.tube_path(f"leg_band_{s}", [(p.x, p.y, p.z) for p in band], 0.007, "dye_accent", sides=5, closed=True))
    # the waistband joins the two legs under the coat
    waist = ring_around(body, Vector((0.0, body.a["pelvis"].y, 0.97)), UP, FWD, 26, 0.02, 0.1, 0.22)
    parts.append(W.tube_path("waistband", [(p.x, p.y, p.z) for p in waist], 0.02, "leather", sides=6, closed=True))
    return W.bake(parts, "bw_legs")


def hands(body):
    parts = []
    for s in (1, -1):
        hand = body.a["hand_l" if s > 0 else "hand_r"]
        lower = body.a["lowerarm_l" if s > 0 else "lowerarm_r"]
        axis = (hand - lower).normalized()
        # the cuff: flares out from the wrist up the forearm (oversized)
        rows = []
        for k, (t, flare) in enumerate(((-0.17, 0.035), (-0.12, 0.028), (-0.06, 0.018), (-0.01, 0.012))):
            c = hand + axis * t
            rows.append(ring_around(body, c, axis, UP, 18, 0.012 + flare, 0.04, 0.08))
        # the glove over the hand
        for t in (0.03, 0.07, 0.11, 0.15, 0.185):
            c = hand + axis * t
            rows.append(ring_around(body, c, axis, UP, 18, 0.014, 0.02, 0.07))
        tip = hand + axis * 0.205
        rows.append([tip + (p - tip) * 0.35 for p in rows[-1]])
        glove = tube_shell(f"gauntlet_{s}", rows, "dye_coat", 0.012)
        A.smooth(glove, 1)
        A.wrap_outside(glove, body, 0.02)
        parts.append(glove)
        # the brass knuckle plate on the back of the hand (up, in the reference pose)
        kp = body.outer(hand + axis * 0.11, UP, 0.03)
        if kp is not None:
            knuckles = W.box(f"knuckles_{s}", (0.045, 0.085, 0.016), (0, 0, 0), "dye_trim", bevel=0.005, segments=2)
            knuckles.matrix_world = Matrix.Translation(kp) @ Matrix.Rotation(math.radians(90), 4, "Z")
            parts.append(knuckles)
            for j in range(3):
                parts.append(rivet(f"knuckle_stud_{s}", kp + axis * (0.025 * (j - 1)) + UP * 0.008, UP, "iron", 0.008))
        # a strap round the cuff
        strap = ring_around(body, hand + axis * -0.09, axis, UP, 16, 0.045, 0.04, 0.09)
        parts.append(W.tube_path(f"cuff_strap_{s}", [(p.x, p.y, p.z) for p in strap], 0.007, "leather", sides=5, closed=True))
    return W.bake(parts, "bw_hands")


def feet(body):
    parts = []
    for s in (1, -1):
        foot = body.a["foot_l" if s > 0 else "foot_r"]
        ball = body.a["ball_l" if s > 0 else "ball_r"]
        mid = (foot + ball) / 2
        rows, centres = [], []
        for y in (0.30, 0.25, 0.20, 0.15, 0.11, 0.075, 0.045, 0.02):
            c = Vector((foot.x, mid.y if y < 0.13 else foot.y, y))
            centres.append(c)
            rows.append(ring_around(body, c, UP, FWD, 20, 0.03, 0.045, 0.16, own_side=True))
        rows = fair_rows(rows, centres, 0.7)
        rows = [[Vector((p.x if s * p.x > 0.006 else s * 0.006, p.y, p.z)) for p in r] for r in rows]
        # the sole: a little wider, closing the bottom
        last = rows[-1]
        lc = sum(last, Vector()) / len(last)
        rows.append([lc + (p - lc) * 1.06 - UP * 0.018 for p in last])
        rows.append([lc - UP * 0.02 + (p - lc) * 0.2 for p in last])
        boot = tube_shell(f"boot_{s}", rows, "dye_coat", 0.012)
        parts.append(boot)
        # iron ankle guards on both sides, hobnails round the sole
        for side in (1, -1):
            p = body.outer(Vector((foot.x, foot.y, 0.16)), LEFT * side, 0.035)
            if p is not None:
                g = W.box(f"ankle_guard_{s}{side}", (0.012, 0.07, 0.08), (p.x, p.y, p.z), "iron", bevel=0.004, segments=2)
                parts.append(g)
        for k in range(0, len(rows[-2]), 2):
            p = rows[-2][k]
            parts.append(W.sphere("hobnail", 0.006, p - UP * 0.004, "iron", seg=6, rings=4))
        # a brass buckle on the shaft
        bp = body.front(foot.x, 0.24, 0.035)
        if bp:
            parts.append(W.box(f"boot_buckle_{s}", (0.035, 0.008, 0.028), (bp[0].x, bp[0].y, bp[0].z), "dye_trim", bevel=0.003))
    return W.bake(parts, "bw_feet")


def charm(body):
    parts = []
    # the chain: from the collar down to the middle of the chest plate
    hit = body.front(0.0, 1.27, 0.115)
    p, n = hit if hit else (gv(0.0, 1.27, 0.2), FWD)
    links = []
    for s in (1, -1):
        a = body.front(s * 0.07, 1.42, 0.1)
        if a:
            path = [a[0].lerp(p + UP * 0.03, t / 6) for t in range(7)]
            for k, q in enumerate(path[:-1]):
                link = W.ring("chain_link", 0.008, 0.0022, (q.x, q.y, q.z), "iron", axis="X" if k % 2 else "Z", maj=8, minr=3)
                links.append(link)
    parts += links
    m = A.frame(p, n)
    # the gear: a toothed brass disc, a glowing hub
    teeth = []
    for k in range(20):
        a = 2 * math.pi * k / 20
        r = 0.034 if k % 2 == 0 else 0.026
        teeth.append((r * math.cos(a), r * math.sin(a)))
    gear = W.top_profile("charm_gear", teeth, 0.008, "dye_trim", z=0.0, bevel=0.0015)
    hub = W.cyl("charm_hub", 0.011, 0.004, 0.011, "glow_accent", verts=14, axis="Z", bevel=0.0)
    for ob in (gear, hub):
        ob.matrix_world = m @ ob.matrix_world
        parts.append(ob)
    return W.bake(parts, "bw_charm")


PIECES = [head, body_piece, legs, hands, feet, charm]


# ---------------------------------------------------------------- pictures

def tint_preview(parts):
    """For the pictures only: the dye zones in the set's own colours."""
    for ob in parts:
        for slot in ob.material_slots:
            m = slot.material
            if m is None:
                continue
            base = m.name.split("__")[0]
            if base not in DEFAULT_DYES:
                continue
            nt = m.node_tree
            bsdf = next((nd for nd in nt.nodes if nd.type == "BSDF_PRINCIPLED"), None)
            tex = next((nd for nd in nt.nodes if nd.type == "TEX_IMAGE"), None)
            if bsdf is None or tex is None:
                continue
            mix = nt.nodes.new("ShaderNodeMixRGB")
            mix.blend_type = "MULTIPLY"
            mix.inputs["Fac"].default_value = 1.0
            c = DEFAULT_DYES[base]
            mix.inputs["Color2"].default_value = (min(c[0] / 0.6, 1.0), min(c[1] / 0.6, 1.0), min(c[2] / 0.6, 1.0), 1.0)
            nt.links.new(tex.outputs["Color"], mix.inputs["Color1"])
            nt.links.new(mix.outputs["Color"], bsdf.inputs["Base Color"])
            if base == "glow_accent":
                emit = bsdf.inputs.get("Emission Color") or bsdf.inputs.get("Emission")
                if emit is not None:
                    emit.default_value = (*c, 1.0)
                    bsdf.inputs["Emission Strength"].default_value = 3.0


def pictures(parts, body, out_path):
    """The fighter (grey) in the full set: front, three-quarter, side and back."""
    scene = bpy.context.scene
    W._use_engine(scene, ["BLENDER_EEVEE", "BLENDER_EEVEE_NEXT"])
    scene.render.resolution_x = 700
    scene.render.resolution_y = 1000
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
    skin.use_nodes = True
    skin.node_tree.nodes.get("Principled BSDF").inputs["Base Color"].default_value = (0.03, 0.032, 0.04, 1.0)
    body.ob.data.materials.clear()
    body.ob.data.materials.append(skin)
    centre = Vector((0.0, 0.0, 0.98))
    lights = [
        W._light("key", "AREA", centre + Vector((1.4, -2.0, 1.6)), 110, centre, size=1.5),
        W._light("fill", "AREA", centre + Vector((-2.0, -1.0, 0.6)), 40, centre, size=2.0, color=(0.75, 0.85, 1.0)),
        W._light("rim", "AREA", centre + Vector((0.0, 2.2, 1.4)), 90, centre, size=1.5, color=(1.0, 0.85, 0.7)),
        W._light("rim2", "AREA", centre + Vector((-1.0, 2.0, 0.5)), 50, centre, size=1.5),
    ]
    tiles = []
    for k, az in enumerate((-90, -55, 0, 90)):
        a = math.radians(az)
        loc = centre + Vector((math.cos(a), math.sin(a), 0.05)) * 3.6
        cam = W._camera(f"view{k}", loc, centre)
        cam.data.lens = 50
        path = out_path.replace(".png", f"_{k}.png")
        scene.render.filepath = path
        bpy.ops.render.render(write_still=True)
        tiles.append(path)
        bpy.data.objects.remove(cam, do_unlink=True)
    for ob in lights:
        bpy.data.objects.remove(ob, do_unlink=True)
    W._stitch(tiles, out_path, cols=4)


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    renders = "--no-renders" not in argv
    names = [a for a in argv if not a.startswith("--")] or NAMES
    with open(os.path.join(REF, "anchors.json")) as f:
        anchors = json.load(f)
    os.makedirs(SHOTS, exist_ok=True)
    textures = {n: os.path.join(REF, f"gear_{n}_tex.png") for n in NAMES}
    for name in names:
        W.clear_scene()
        for m in list(bpy.data.materials):
            bpy.data.materials.remove(m)
        body = A.Body(name, anchors[name])
        parts = [build(body) for build in PIECES]
        for ob in parts:
            A.apply_mods(ob)
            print(f"[gear] {name} {ob.name}: size {tuple(round(v, 3) for v in ob.dimensions)}")
        T.bake_texture(parts, textures[name], size=TEX, samples=24)
        tris = sum(W.triangles(o) for o in parts)
        if renders:
            tint_preview(parts)
            pictures(parts, body, os.path.join(SHOTS, f"bridgewright_{name}.png"))
        A.remap_uvs(parts, QUARTER[name])
        W.export_glb(parts, os.path.join(OUT, f"gear_bridgewright_{name}.glb"))
        print(f"[gear] {name}: {tris} triangles, pieces {[o.name for o in parts]}")
    # the four bakes into one texture, a quarter each
    img = bpy.data.images.new("gear", TEX * 2, TEX * 2, alpha=False)
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
    img.filepath_raw = os.path.join(OUT, "gear.png")
    img.file_format = "PNG"
    img.save()


if __name__ == "__main__":
    main()
