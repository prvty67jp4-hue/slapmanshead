"""The Ashen Vault's wearable drops (app/src/raid.rs LOOT, app/src/hub.rs LOOKS 70-84), fitted to
each of the four fighters like the wardrobe's other outfits (build_outfits.py, whose fitting
helpers this uses).

    blender --background --factory-startup --python app/tools/blender/build_vault_outfits.py [-- --only ranger] [--no-renders]

Three sets share one file per fighter and one baked texture:

- Vault Warden (wv_, pieces 130-136): a closed helm with cheek plates, a crest ridge and a
  candle finial burning on top; a mantle with a fur collar and metal-piped edges; a gorget of
  three riveted lames; a cuirass with shoulder lames, a wax-sealed crest, a leather tasset skirt
  and a buckled belt; greaves with knee cops over dark hose; gauntlets with flared cuffs;
  sabatons banded in iron.
- The Ashen Magistrate (mg_, pieces 140-143): a close judge's coif with a gilt band and two
  lappets down the back; the Cape of Verdicts, long, with scales on the back; the Ember Stole,
  two bands down the front ending in glowing embers; the Robe of Sentence, long, with bell
  sleeves, a front band and a rope of office.
- The Ashblood Regent (rg_, piece 150): the long coat. Fitted to the waist then falling to the
  shin, split front and back so it strides; a high standing collar flaring up round the jaw,
  piped; turned-back lapels and a double row of buttons; long sleeves to the wrist with deep
  cuffs; epaulettes with fringes; a sash from the shoulder to the hip.

Writes app/assets/characters/look_vault_<fighter>.glb, app/assets/characters/look_vault.png and
pictures to app/shots/character-art/outfits/vault_<set>_<fighter>.png.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy  # noqa: E402
from mathutils import Matrix, Vector  # noqa: E402

import build_outfits as O  # noqa: E402  (its fitting helpers; its own main doesn't run on import)

G, K, A, W, T = O.G, O.K, O.A, O.W, O.T
UP, FWD, LEFT = O.UP, O.FWD, O.LEFT
OUT, REF, NAMES, QUARTER, TEX = O.OUT, O.REF, O.NAMES, O.QUARTER, O.TEX
SHOTS = O.SHOTS

SET_DYES = {
    "warden": K.preview_dyes((0.18, 0.18, 0.20), (0.62, 0.42, 0.14), (0.55, 0.45, 0.30), (0.95, 0.6, 0.2)),
    "magistrate": K.preview_dyes((0.03, 0.03, 0.035), (0.62, 0.42, 0.14), (0.55, 0.12, 0.03), (1.0, 0.45, 0.08)),
    "regent": K.preview_dyes((0.025, 0.02, 0.03), (0.62, 0.02, 0.035), (0.45, 0.36, 0.14), (1.0, 0.1, 0.1)),
}


# ============================================================================ Vault Warden

def warden_head(body):
    c, rim, R, H = O.head_frame(body)
    parts = []
    helm = O.dome("wv_helm", Vector((0.0, c.y, rim - 0.01)), (R * 1.1, R * 1.14, H * 1.05), "dye_coat_metal", keep=-0.35, u=32, v=16)
    # (open at the face: a T-shaped opening for the eyes and mouth)
    O.cut(helm, lambda m: not (m.y < c.y - 0.04 and ((abs(m.x) < 0.07 and rim - 0.03 < m.z < rim + 0.035) or (abs(m.x) < 0.025 and rim - 0.12 < m.z < rim + 0.035))))
    A.solidify(helm, 0.012, 1.0)
    K.bev(helm, 0.003)
    parts.append(helm)
    # a brow band, riveted, and a crest ridge from the brow over the top to the nape
    band = O.ellipse(Vector((0.0, c.y, rim + 0.02)), R * 1.12, R * 1.16, 32)
    parts.append(K.tube("wv_brow", band, 0.009, "dye_trim_metal", sides=6, closed=True))
    parts += K.rivets("wv_brow_rivet", [band[k] + K.outward(band[k], Vector((0.0, c.y, band[k].z))) * 0.008 for k in range(0, 32, 4)], [K.outward(band[k], Vector((0.0, c.y, band[k].z))) for k in range(0, 32, 4)], "dye_trim_metal")
    ridge = []
    for k in range(13):
        a = math.pi * k / 12
        ridge.append(Vector((0.0, c.y - math.cos(a) * R * 1.14, rim - 0.01 + math.sin(a) * H * 1.05 + 0.012)))
    parts.append(K.tube("wv_crest", ridge, 0.014, "dye_trim_metal", sides=6, radii=[[0.014 * (0.7 + 0.5 * math.sin(math.pi * q / 12))] * 6 for q in range(13)]))
    # cheek plates hanging either side
    for s in (1, -1):
        g = W.box(f"wv_cheek_{s}", (0.012, 0.07, 0.1), (0, 0, 0), "dye_coat_metal", bevel=0.004, segments=2)
        g.location = (s * (R * 1.02), c.y - 0.02, rim - 0.08)
        g.rotation_euler = (0.0, 0.0, math.radians(s * -12))
        parts.append(g)
    # the candle finial: a short wax candle in a cup on top, its flame glowing
    top = Vector((0.0, c.y + 0.01, rim + H * 1.05 + 0.005))
    parts.append(W.cyl("wv_cup", 0.03, top.z - 0.005, top.z + 0.02, "dye_trim_metal", x=top.y, z=top.x, r1=0.018, verts=12, axis="Z", bevel=0.003))
    parts.append(W.cyl("wv_candle", 0.016, top.z + 0.02, top.z + 0.07, "wool_cream", x=top.y, z=top.x, verts=10, axis="Z", bevel=0.002))
    parts.append(W.sphere("wv_flame", 0.012, (top.x, top.y, top.z + 0.085), "glow_accent", seg=8, rings=6, scale=(0.8, 0.8, 1.5)))
    return W.bake(parts, "wv_head")


def warden_cape(body):
    rows, centres = O.back_cape_rows(body, bottom=0.75, arc=(70, 290), n=28, flare=0.05)
    rows = K.folds(rows, centres, 5, 0.02, start=2, arc=math.radians(220))
    parts = [K.shell("wv_mantle", rows, "dye_accent_cloth", 0.013, closed=False, bevel=0.003)]
    parts += O.cape_edges(rows, centres, "wv_mantle", "dye_coat_cloth", "dye_trim_metal", stitch_mat="dye_trim_metal")
    parts += K.fur_roll("wv_fur", K.resample(rows[0], 24, closed=False), 0.032, "fur_grey", closed=False)
    bp = body.outer(O.gv(0.0, 1.2, 0.0), -FWD, 0.11)
    bp = bp if bp is not None else O.gv(0.0, 1.2, -0.2)
    parts += O.crest("wv_mantle_crest", bp, -FWD, "dye_coat_cloth", "dye_trim_cloth", "dye_trim_metal", 0.07)
    return W.bake(parts, "wv_cape")


def warden_neck(body):
    neck = body.a["neck_01"]
    parts = []
    for k, (dz, rx, ry) in enumerate(((0.035, 0.085, 0.08), (-0.005, 0.11, 0.1), (-0.045, 0.14, 0.125))):
        ring = O.ellipse(Vector((0.0, neck.y, neck.z + dz)), rx, ry, 28)
        rows = [[p + UP * 0.018 for p in ring], [neck + (p - neck) * 1.0 - UP * 0.018 + K.outward(p, Vector((0.0, neck.y, p.z))) * 0.02 for p in ring]]
        parts.append(K.shell(f"wv_gorget{k}", rows, "dye_coat_metal", 0.008, bevel=0.002, offset=1.0))
        parts.append(K.tube(f"wv_gorget_rim{k}", [p - UP * 0.018 + K.outward(p, Vector((0.0, neck.y, p.z))) * 0.022 for p in ring], 0.004, "dye_trim_metal", sides=5, closed=True))
        for q in range(0, 28, 7):
            p = ring[q] + K.outward(ring[q], Vector((0.0, neck.y, ring[q].z))) * 0.012
            parts.append(G.rivet(f"wv_gorget_rivet{k}", p, K.outward(ring[q], Vector((0.0, neck.y, ring[q].z))), "dye_trim_metal", 0.005))
    return W.bake(parts, "wv_neck")


def warden_body(body):
    parts = []
    rows, centres = O.torso(body, n=32, out=0.06)
    parts.append(K.shell("wv_cuirass", rows, "dye_coat_metal", 0.016, bevel=0.004))
    yk, neck_ring = O.yoke(body, "wv", rows[0], centres[0], "dye_coat_metal")
    parts.append(yk)
    # a ridge down the breastplate and two plackarts over the belly
    ridge = [K.front_pt(body, 0.0, 1.44 - 0.04 * k, 0.07)[0] for k in range(11)]
    parts.append(K.tube("wv_ridge", ridge, 0.008, "dye_trim_metal", sides=6))
    for k, y in enumerate((1.12, 1.04)):
        pts, ns = O.chest_arc(body, y + 0.02, y - 0.02, 0.15, 0.08, n=12)
        parts.append(K.ribbon(f"wv_fauld{k}", pts, ns, 0.05, "dye_coat_metal", 0.008))
        parts.append(K.tube(f"wv_fauld_rim{k}", [p - UP * 0.025 + n * 0.006 for p, n in zip(pts, ns)], 0.004, "dye_trim_metal", sides=5))
    # a wax-sealed crest on the chest
    cp, cn = K.front_pt(body, 0.0, 1.31, 0.085)
    seal = [W.cyl("wv_seal", 0.04, -0.004, 0.006, "dye_accent_metal", verts=18, axis="Z", bevel=0.004, segments=2),
            W.ring("wv_seal_rim", 0.036, 0.005, (0, 0, 0.006), "dye_trim_metal", axis="Z", maj=18, minr=5),
            W.cyl("wv_seal_flame", 0.008, 0.006, 0.012, "glow_accent", verts=8, axis="Z", bevel=0.001)]
    for ob in seal:
        K.placed(ob, cp, cn)
        parts.append(ob)
    # shoulder lames over dark wool sleeves
    for s in (1, -1):
        srs, cs, axis = O.sleeve(body, s, 0.24, n=20, out=0.022)
        parts.append(K.shell(f"wv_sleeve_{s}", srs, "wool_dark", 0.012))
        parts += K.lames("wv", body, s, ("dye_coat_metal", "dye_trim_metal", "dye_trim_metal"), count=4, top_out=0.06)
    # the leather tasset skirt in strips, and the belt
    srows, scs = O.skirt(rows, (0.05, 0.11, 0.17), 0.06)
    srows = [rows[-1]] + srows
    scs = [centres[-1]] + scs
    for j in range(8):
        a, b = j * 4, j * 4 + 3
        strip = [[cc + (r[q % len(r)] - cc) * 1.03 for q in range(a, b + 1)] for r, cc in zip(srows, scs)]
        parts.append(K.shell(f"wv_tasset{j}", strip, "dye_coat_leather" if j % 2 else "leather_dark", 0.01, closed=False, bevel=0.002, offset=1.0))
        parts.append(G.rivet(f"wv_tasset_rivet{j}", strip[1][1] + K.outward(strip[1][1], scs[1]) * 0.012, K.outward(strip[1][1], scs[1]), "dye_trim_metal", 0.0055))
    bring, bc = rows[-1], centres[-1]
    belt = [bc + (p - bc) * 1.05 + K.outward(p, bc) * 0.02 for p in bring]
    bns = [K.outward(p, bc) for p in belt]
    parts.append(K.ribbon("wv_belt", belt, bns, 0.045, "leather_dark", 0.009, closed=True))
    k = min(range(len(belt)), key=lambda i: belt[i].y)
    parts += K.buckle("wv_belt_buckle", belt[k] + bns[k] * 0.007, bns[k], (belt[(k + 1) % len(belt)] - belt[k - 1]).normalized(), "dye_trim_metal", w=0.06, h=0.055)
    return W.bake(parts, "wv_body")


def warden_legs(body):
    parts = []
    for s in (1, -1):
        rows, cs = O.leg_rows(body, s, ((0.97, 0.108), (0.70, 0.098), (0.55, 0.086), (0.4, 0.074), (0.0, 0.07)), [0.96, 0.9, 0.84, 0.78, 0.72, 0.66, 0.6, 0.55, 0.5, 0.45, 0.4, 0.34, 0.28, 0.24, 0.2], n=24)
        parts.append(K.shell(f"wv_hose_{s}", rows, "wool_dark", 0.012))
        # cuisses over the thigh, a knee cop, a greave down the shin
        for y0, y1, nm in ((0.92, 0.62, "cuisse"), (0.48, 0.22, "greave")):
            sub = [(r, c) for r, c in zip(rows, cs) if y1 - 0.001 <= c.z <= y0 + 0.001]
            if len(sub) >= 2:
                plate = [[c + (p - c) * 1.12 for p in r] for r, c in sub]
                ob = K.shell(f"wv_{nm}_{s}", plate, "dye_coat_metal", 0.01, bevel=0.003)
                O.cut(ob, lambda m: m.y < 0.06)
                parts.append(ob)
                top, tc = sub[0]
                parts.append(K.tube(f"wv_{nm}_rim_{s}", [tc + (p - tc) * 1.16 for p in top if p.y < 0.05], 0.004, "dye_trim_metal", sides=5))
        kr, kc = K.ring_at(rows, cs, 0.55)
        k = min(range(len(kr)), key=lambda i: kr[i].y)
        p, n = kr[k] + K.outward(kr[k], kc) * 0.012, K.outward(kr[k], kc)
        cop = O.dome(f"wv_knee_{s}", Vector((0, 0, 0)), (0.055, 0.065, 0.035), "dye_coat_metal", keep=0.0, u=16, v=8)
        cop.matrix_world = A.frame(p, n)
        A.solidify(cop, 0.007, -1.0)
        parts.append(cop)
        parts.append(G.rivet(f"wv_knee_rivet_{s}", p + n * 0.036, n, "dye_trim_metal", 0.008))
        parts += O.flat_band(f"wv_greave_strap_{s}", rows, cs, 0.3, 0.018, "leather_dark", out=0.014, buckle_mat="dye_trim_metal")
    waist = G.ring_around(body, Vector((0.0, body.a["pelvis"].y, 0.97)), UP, FWD, 28, 0.03, 0.1, 0.22)
    parts.append(K.tube("wv_waist", waist, 0.02, "wool_dark", sides=8, closed=True))
    return W.bake(parts, "wv_legs")


def warden_hands(body):
    parts = []
    for s in (1, -1):
        rows, cs, hand, axis = O.glove_rows(body, s, 0.0)
        parts.append(O.glove("wv", s, rows, "leather_dark"))
        crs = []
        for t, r in ((-0.02, 0.008), (-0.07, 0.012), (-0.12, 0.018), (-0.17, 0.026)):
            c = hand + axis * t
            crs.append((G.ring_around(body, c, axis, UP, 20, 0.012 + r, 0.04, 0.09), c))
        parts.append(K.shell(f"wv_cuff_{s}", [r for r, _ in crs], "dye_coat_metal", 0.009, bevel=0.0025))
        rim, rc = crs[-1]
        parts.append(K.tube(f"wv_cuff_rim_{s}", [rc + (p - rc) * 1.03 for p in rim], 0.005, "dye_trim_metal", sides=6, closed=True))
        # plates over the back of the hand
        for k, t in enumerate((0.02, 0.06, 0.1)):
            top = G.ring_around(body, hand + axis * t, axis, UP, 16, 0.02, 0.02, 0.07)
            q = top[0]
            g = W.box(f"wv_hand_plate_{s}{k}", (0.06, 0.034, 0.006), (0, 0, 0), "dye_coat_metal", bevel=0.002, segments=2)
            K.placed(g, q + K.outward(q, hand + axis * t, False) * 0.004, K.outward(q, hand + axis * t, False), axis)
            parts.append(g)
    return W.bake(parts, "wv_hands")


def warden_feet(body):
    parts = []
    for s in (1, -1):
        rows, cs, foot = O.boot_rows(body, s, 0.3)
        parts += O.boot("wv_sabaton", s, rows, cs, "dye_coat_metal", "iron")
        for y in (0.2, 0.12, 0.07):
            parts += O.flat_band(f"wv_sabaton_band_{s}{int(y * 100)}", rows, cs, y, 0.01, "dye_trim_metal", out=0.006)
    return W.bake(parts, "wv_feet")


# ======================================================================== the Magistrate

def magistrate_head(body):
    c, rim, R, H = O.head_frame(body)
    parts = []
    coif = O.dome("mg_coif", Vector((0.0, c.y + 0.005, rim - 0.01)), (R * 1.04, R * 1.1, H * 0.95), "dye_coat_cloth", keep=-0.2, u=32, v=14)
    O.cut(coif, lambda m: not (m.y < c.y - 0.03 and m.z < rim + 0.01))
    A.solidify(coif, 0.01, 1.0)
    K.bev(coif, 0.003)
    parts.append(coif)
    band = O.ellipse(Vector((0.0, c.y, rim + 0.01)), R * 1.06, R * 1.12, 32)
    outs = [K.outward(p, Vector((0.0, c.y, p.z))) for p in band]
    parts.append(K.ribbon("mg_band", [p + d * 0.004 for p, d in zip(band, outs)], outs, 0.022, "dye_trim_metal", 0.006, closed=True))
    plate = W.box("mg_plate", (0.05, 0.03, 0.006), (0, 0, 0), "dye_trim_metal", bevel=0.002, segments=2)
    K.placed(plate, band[0] + outs[0] * 0.012, outs[0])
    parts.append(plate)
    gem = W.sphere("mg_gem", 0.01, (0, 0, 0), "glow_accent", seg=8, rings=6, scale=(1, 1, 0.6))
    K.placed(gem, band[0] + outs[0] * 0.018, outs[0])
    parts.append(gem)
    # two lappets hanging down the back of the head
    for s in (1, -1):
        top = Vector((s * 0.035, c.y + R * 1.05, rim + 0.0))
        rows = []
        for k in range(7):
            t = k / 6
            p = top + Vector((0.0, 0.02 * t, -0.24 * t))
            rows.append([p - LEFT * 0.025, p + Vector((0.0, 0.004, 0.0)), p + LEFT * 0.025])
        parts.append(K.shell(f"mg_lappet_{s}", rows, "dye_coat_cloth", 0.006, closed=False, bevel=0.0015, offset=0.0))
        parts.append(K.tube(f"mg_lappet_hem_{s}", rows[-1], 0.004, "dye_trim_metal", sides=5))
    return W.bake(parts, "mg_head")


def magistrate_cape(body):
    rows, centres = O.back_cape_rows(body, bottom=0.56, arc=(65, 295), n=30, flare=0.05)
    rows = K.folds(rows, centres, 6, 0.024, start=2, arc=math.radians(230))
    parts = [K.shell("mg_cape", rows, "dye_coat_cloth", 0.013, closed=False, bevel=0.003)]
    parts += O.cape_edges(rows, centres, "mg_cape", "dye_accent_cloth", "dye_trim_metal", stitch_mat="dye_trim_metal", width=0.04)
    # the scales of judgement on the back, in gilt
    bp = body.outer(O.gv(0.0, 1.18, 0.0), -FWD, 0.115)
    bp = bp if bp is not None else O.gv(0.0, 1.18, -0.21)
    beam = W.box("mg_scales_beam", (0.16, 0.008, 0.006), (0, 0.04, 0.004), "dye_trim_metal", bevel=0.002)
    post = W.box("mg_scales_post", (0.008, 0.12, 0.006), (0, -0.01, 0.004), "dye_trim_metal", bevel=0.002)
    pans = [W.cyl(f"mg_scales_pan{k}", 0.026, 0.0, 0.006, "dye_trim_metal", verts=12, axis="Z", bevel=0.002) for k in range(2)]
    for k, pan in enumerate(pans):
        pan.location = ((k * 2 - 1) * 0.075, -0.02, 0.004)
    for ob in [beam, post] + pans:
        bpy.context.view_layer.update()
        ob.data.transform(ob.matrix_basis)
        ob.matrix_basis = Matrix.Identity(4)
        K.placed(ob, bp, -FWD)
        parts.append(ob)
    for s in (1, -1):
        p, n = K.front_pt(body, s * 0.13, 1.42, 0.06)
        cl = W.cyl(f"mg_clasp_{s}", 0.02, -0.004, 0.005, "dye_trim_metal", verts=14, axis="Z", bevel=0.003, segments=2)
        K.placed(cl, p, n)
        parts.append(cl)
    return W.bake(parts, "mg_cape")


def magistrate_neck(body):
    parts = []
    for s in (1, -1):
        path, ns = K.path_on_front(body, [(s * 0.07, 1.47), (s * 0.085, 1.25), (s * 0.09, 1.02)], 0.09, 14)
        parts.append(K.ribbon(f"mg_stole_{s}", path, ns, 0.06, "dye_accent_cloth", 0.008))
        parts.append(K.stitches(f"mg_stole_st_{s}", [p + n * 0.006 for p, n in zip(path, ns)], ns, "dye_trim_metal", step=0.02))
        end = path[-1] - UP * 0.02
        parts.append(K.tube(f"mg_stole_fringe_{s}", [end - LEFT * 0.03, end + LEFT * 0.03], 0.006, "dye_trim_metal", sides=5))
        for k in range(3):
            parts.append(W.sphere(f"mg_ember_{s}{k}", 0.009, (end + LEFT * (0.02 * (k - 1)) - UP * 0.02).to_tuple(), "glow_accent", seg=8, rings=5))
    neck = body.a["neck_01"]
    wrap = O.ellipse(Vector((0.0, neck.y + 0.01, neck.z - 0.02)), 0.1, 0.092, 22)
    parts.append(K.tube("mg_stole_wrap", wrap, 0.022, "dye_accent_cloth", sides=8, closed=True))
    return W.bake(parts, "mg_neck")


def magistrate_body(body):
    parts = []
    rows, centres = O.torso(body, n=36)
    srows, scs = O.skirt(rows, (0.08, 0.17, 0.26, 0.35, 0.44, 0.52), 0.045)
    allrows, allc = rows + srows, centres + scs
    allrows = K.folds(allrows, allc, 10, 0.016, start=len(rows) - 2)
    robe = K.shell("mg_robe", allrows, "dye_coat_cloth", 0.016)
    O.cut(robe, lambda m: not (m.z < 0.8 and abs(m.x) < 0.018))
    parts.append(robe)
    yk, _ = O.yoke(body, "mg", rows[0], centres[0], "dye_coat_cloth")
    parts.append(yk)
    parts += K.hem_trim("mg_robe_hem", allrows[-1], allc[-1], "dye_accent_cloth", "dye_trim_metal", width=0.045, stitch_mat="dye_trim_metal")
    # the front band from the neck to the hem
    fp, fn = K.path_on_front(body, [(0.0, 1.46), (0.0, 1.2), (0.0, 0.98)], 0.07, 14)
    parts.append(K.ribbon("mg_front_band", fp, fn, 0.07, "dye_accent_cloth", 0.008))
    parts.append(K.tube("mg_front_pipe_a", [p + n * 0.006 - LEFT * 0.035 for p, n in zip(fp, fn)], 0.004, "dye_trim_metal", sides=5))
    parts.append(K.tube("mg_front_pipe_b", [p + n * 0.006 + LEFT * 0.035 for p, n in zip(fp, fn)], 0.004, "dye_trim_metal", sides=5))
    for s in (1, -1):
        srs, cs, axis = O.sleeve(body, s, 0.34, n=24, out=0.024, flare=0.05)
        srs = K.folds(srs, cs, 4, 0.006, start=1)
        parts.append(K.shell(f"mg_sleeve_{s}", srs, "dye_coat_cloth", 0.012))
        end, ce = srs[-1], cs[-1]
        parts.append(K.tube(f"mg_sleeve_trim_{s}", [ce + (p - ce) * 1.02 for p in end], 0.012, "dye_accent_cloth", sides=7, closed=True))
    # the rope of office, knotted, with gilt ends
    bring, bc = rows[-1], centres[-1]
    belt = [bc + (p - bc) * 1.0 + K.outward(p, bc) * 0.024 + UP * 0.03 for p in bring]
    parts.append(K.tube("mg_rope", belt, 0.012, "dye_trim_cloth", sides=7, closed=True))
    kp, _ = K.front_pt(body, -0.06, 1.0, 0.13)
    for dx in (0.0, 0.022):
        a = kp + LEFT * dx
        end = [a - UP * 0.2 * t / 5 for t in range(6)]
        parts.append(K.tube("mg_rope_end", end, 0.008, "dye_trim_cloth", sides=6))
        parts.append(W.cyl("mg_rope_cap", 0.012, end[-1].z - 0.03, end[-1].z, "dye_trim_metal", x=end[-1].y, z=end[-1].x, r1=0.006, verts=8, axis="Z", bevel=0.0))
    return W.bake(parts, "mg_body")


# ======================================================================== the Ashblood Regent

def regent_body(body):
    parts = []
    rows, centres = O.torso(body, n=36, out=0.055)
    srows, scs = O.skirt(rows, (0.08, 0.16, 0.25, 0.34, 0.43, 0.52, 0.6, 0.68), 0.035)
    allrows, allc = rows + srows, centres + scs
    allrows = K.folds(allrows, allc, 9, 0.016, start=len(rows) - 1)
    # the coat: fitted at the waist, open below it in front, a long split up the back
    coat = K.shell("rg_coat", allrows, "dye_coat_cloth", 0.016)
    O.cut(coat, lambda m: not ((m.y < -0.02 and m.z < 0.97 and abs(m.x) < 0.02 + (0.97 - m.z) * 0.18) or (m.z < 0.72 and m.y > 0.05 and abs(m.x) < 0.02)))
    parts.append(coat)
    yk, neck_ring = O.yoke(body, "rg", allrows[0], centres[0], "dye_coat_cloth")
    parts.append(yk)
    # the high standing collar, flaring up and out round the jaw, open at the throat
    neck = body.a["neck_01"]
    nc = Vector((0.0, neck.y, neck_ring[0].z))
    col = []
    for k, (up_, out_) in enumerate(((0.0, 0.0), (0.05, 0.012), (0.1, 0.03), (0.15, 0.058))):
        col.append([p + UP * up_ + K.outward(p, nc) * out_ for p in neck_ring])
    collar = K.shell("rg_collar", col, "dye_coat_cloth", 0.012, bevel=0.003, offset=1.0)
    O.cut(collar, lambda m: not (m.y < neck.y - 0.05 and abs(m.x) < 0.035 + (m.z - nc.z) * 0.35))
    parts.append(collar)
    # its lining in the accent, its edge piped
    lin = [[nc + (p - nc) * 0.97 for p in r] for r in col[1:]]
    lining = K.shell("rg_collar_lining", lin, "dye_trim_cloth", 0.004, bevel=0.0015, offset=-1.0)
    O.cut(lining, lambda m: not (m.y < neck.y - 0.05 and abs(m.x) < 0.04 + (m.z - nc.z) * 0.35))
    parts.append(lining)
    edge = [p for p in col[-1] if not (p.y < neck.y - 0.05 and abs(p.x) < 0.035 + (p.z - nc.z) * 0.35)]
    if len(edge) > 3:
        edge.sort(key=lambda p: math.atan2(p.x, p.y - neck.y))
        parts.append(K.tube("rg_collar_pipe", edge, 0.005, "dye_trim_metal", sides=6))
    # turned-back lapels down the chest, stitched
    for s in (1, -1):
        lap = []
        for k in range(8):
            y = 1.44 - 0.06 * k
            p, n = K.front_pt(body, s * (0.05 + 0.035 * math.sin(math.pi * k / 7)), y, 0.085)
            lap.append((p, n))
        rows2 = [[p, p + LEFT * s * (0.05 * math.sin(math.pi * (k + 0.5) / 8)) + n * 0.004] for k, (p, n) in enumerate(lap)]
        parts.append(K.shell(f"rg_lapel_{s}", rows2, "dye_trim_cloth", 0.008, closed=False, bevel=0.002, offset=0.0))
        parts.append(K.stitches(f"rg_lapel_st_{s}", [a.lerp(b, 0.8) + n * 0.006 for (a, b), (_, n) in zip(rows2, lap)], [n for _, n in lap], "dye_trim_metal", step=0.016))
    # the waistcoat between the lapels, and a double row of buttons
    for s in (1, -1):
        for k in range(5):
            p, n = K.front_pt(body, s * 0.045, 1.36 - 0.075 * k, 0.09)
            parts.append(K.button(f"rg_button_{s}{k}", p, n, "dye_trim_metal", 0.01))
    # long sleeves to the wrist, deep turned-back cuffs with buttons
    for s in (1, -1):
        srs, cs, axis = O.sleeve(body, s, 0.46, n=24, out=0.026)
        srs = K.folds(srs, cs, 3, 0.005, start=1)
        parts.append(K.shell(f"rg_sleeve_{s}", srs, "dye_coat_cloth", 0.013))
        cuff = [[cs[-1] + (p - cs[-1]) * 1.12 + axis * 0.01 for p in srs[-1]], [cs[-2] + (p - cs[-2]) * 1.14 for p in srs[-2]]]
        parts.append(K.shell(f"rg_cuff_{s}", cuff, "dye_trim_cloth", 0.01, bevel=0.002))
        parts.append(K.tube(f"rg_cuff_pipe_{s}", [cs[-2] + (p - cs[-2]) * 1.16 for p in srs[-2]], 0.004, "dye_trim_metal", sides=5, closed=True))
        top = srs[1][0] + K.outward(srs[1][0], cs[1], False) * 0.02
        ep = O.dome(f"rg_epaulette_{s}", top - UP * 0.025, (0.07, 0.065, 0.028), "dye_trim_metal", keep=-0.15)
        A.solidify(ep, 0.008, 1.0)
        K.bev(ep, 0.002)
        parts.append(ep)
        rim = [top - UP * 0.03 + Vector((math.cos(a) * 0.068, math.sin(a) * 0.062, 0.0)) for a in [2 * math.pi * q / 18 for q in range(18)]]
        parts.append(K.tube(f"rg_epaulette_rim_{s}", rim, 0.0055, "dye_trim_metal", sides=6, closed=True))
        for e in rim:
            if K.outward(e, top).x * s > -0.2:
                parts.append(K.tube(f"rg_fringe_{s}", [e, e - UP * 0.05 + K.outward(e, top) * 0.012], 0.0032, "dye_trim_metal", sides=4))
    # the sash from the right shoulder to the left hip, a brooch where it crosses
    fp, fn = K.path_on_front(body, [(-0.13, 1.43), (0.0, 1.22), (0.15, 0.99)], 0.1, 16)
    parts.append(K.ribbon("rg_sash", fp, fn, 0.06, "dye_accent_cloth", 0.008))
    bp, bn = fp[8] + fn[8] * 0.01, fn[8]
    for ob in (W.cyl("rg_brooch", 0.024, -0.004, 0.005, "dye_trim_metal", verts=14, axis="Z", bevel=0.003, segments=2),
               W.sphere("rg_brooch_gem", 0.012, (0, 0, 0.007), "glow_accent", seg=10, rings=6, scale=(1, 1, 0.7))):
        K.placed(ob, bp, bn)
        parts.append(ob)
    # the hem, piped
    parts += K.hem_trim("rg_coat_hem", allrows[-1], allc[-1], "dye_trim_cloth", "dye_trim_metal", width=0.03, stitch_mat="dye_trim_metal")
    # a belt at the waist, under the coat's fall
    bring, bc = rows[-1], centres[-1]
    belt = [bc + (p - bc) * 1.03 + K.outward(p, bc) * 0.018 + UP * 0.02 for p in bring]
    bns = [K.outward(p, bc) for p in belt]
    parts.append(K.ribbon("rg_belt", belt, bns, 0.04, "leather_dark", 0.008, closed=True))
    k = min(range(len(belt)), key=lambda i: belt[i].y)
    parts += K.buckle("rg_belt_buckle", belt[k] + bns[k] * 0.007, bns[k], (belt[(k + 1) % len(belt)] - belt[k - 1]).normalized(), "dye_trim_metal", w=0.05, h=0.046)
    return W.bake(parts, "rg_body")


SETS = {
    "warden": [warden_head, warden_cape, warden_neck, warden_body, warden_legs, warden_hands, warden_feet],
    "magistrate": [magistrate_head, magistrate_cape, magistrate_neck, magistrate_body],
    "regent": [regent_body],
}


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    renders = "--no-renders" not in argv
    only = argv[argv.index("--only") + 1] if "--only" in argv else None
    with open(os.path.join(REF, "anchors.json")) as f:
        anchors = G.json.load(f)
    os.makedirs(SHOTS, exist_ok=True)
    textures = {n: os.path.join(REF, f"look_vault_{n}_tex.png") for n in NAMES}
    for name in NAMES:
        if only and name != only:
            continue
        W.clear_scene()
        for m in list(bpy.data.materials):
            bpy.data.materials.remove(m)
        body = A.Body(name, anchors[name])
        by_set = {s: [build(body) for build in builds] for s, builds in SETS.items()}
        parts = [p for ps in by_set.values() for p in ps]
        for ob in parts:
            A.apply_mods(ob)
        T.bake_texture(parts, textures[name], size=TEX, samples=24)
        tris = sum(W.triangles(o) for o in parts)
        if renders:
            for s, ps in by_set.items():
                # (the pictures: each set on its own)
                for o in parts:
                    o.hide_render = o not in ps
                G.DEFAULT_DYES = SET_DYES[s]
                G.tint_preview(ps)
                G.pictures(ps, body, os.path.join(SHOTS, f"vault_{s}_{name}.png"))
            for o in parts:
                o.hide_render = False
        A.remap_uvs(parts, QUARTER[name])
        if not only:
            W.export_glb(parts, os.path.join(OUT, f"look_vault_{name}.glb"))
        print(f"[vault outfits] {name}: {tris} triangles, pieces {[(o.name, W.triangles(o)) for o in parts]}")
    if only:
        return
    img = bpy.data.images.new("look_vault", TEX * 2, TEX * 2, alpha=False)
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
    img.filepath_raw = os.path.join(OUT, "look_vault.png")
    img.file_format = "PNG"
    img.save()


main()
