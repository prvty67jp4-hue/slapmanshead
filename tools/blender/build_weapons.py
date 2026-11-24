"""Build Duel Rush's weapon models, their textures, icons and turntable previews.

    blender --background --python app/tools/blender/build_weapons.py -- [weapon ...] [--no-renders]

With no names every weapon is built. Writes:
    app/assets/weapons/<name>.glb           the model the game loads (with its UVs)
    app/assets/weapons/<name>_far.glb       a simplified copy for fighters far away
    app/assets/weapons/textures/<name>.png  its baked, hand-painted-style texture (512 x 512)
    app/assets/weapons/icons/<name>.png     the HUD / loadout icon (grey; the game tints it)
    app/shots/weapons/<name>.png            a four-view turntable picture for review
    app/shots/weapons/_all.png              every turntable on one sheet (when all are built)

Axes and units: see weapons_lib.py (Blender +X right, +Y forward, +Z up; metres; the origin is
where the hand holds it). Moving parts: `pump` (scattergun) and `cylinder` (revolver, hand
cannon). Textures: see weapons_tex.py.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy  # noqa: E402

import weapons_lib as W  # noqa: E402
import weapons_tex as T  # noqa: E402

APP = os.path.normpath(os.path.join(HERE, "..", ".."))
OUT_MODELS = os.path.join(APP, "assets", "weapons")
OUT_TEX = os.path.join(OUT_MODELS, "textures")
OUT_ICONS = os.path.join(OUT_MODELS, "icons")
OUT_SHOTS = os.path.join(APP, "shots", "weapons")


# ---------------------------------------------------------------- shared bits

def pistol_grip(name, mat, top=0.006, depth=0.1, rake=0.034, width=0.034, front=0.022, back=-0.034):
    """A raked pistol grip below the hand point (front, back: where it meets the frame)."""
    pts = [
        (back, top), (front, top), (front - 0.006, top - 0.026), (front - rake * 0.55, -depth + 0.012),
        (front - rake * 0.75, -depth), (back - rake * 0.55, -depth + 0.004), (back - rake * 0.35, -depth * 0.55),
        (back - 0.004, top - 0.03),
    ]
    return W.profile(name, pts, width, mat, bevel=0.005, segments=2)


def trigger_group(y0, y1, z_top, depth=0.034, mat="metal_mid", r=0.0038):
    """A rounded trigger guard from y0 to y1 under z_top, with its trigger."""
    guard = W.tube_path("guard", [
        (0, y0, z_top), (0, y0, z_top - depth * 0.6), (0, y0 + 0.012, z_top - depth),
        (0, y1 - 0.014, z_top - depth), (0, y1, z_top - depth * 0.65), (0, y1 + 0.004, z_top),
    ], r, mat, sides=6)
    ty = y0 + (y1 - y0) * 0.42
    trig = W.profile("trigger", [(ty - 0.004, z_top), (ty + 0.006, z_top), (ty + 0.003, z_top - depth * 0.55), (ty - 0.006, z_top - depth * 0.68)], 0.006, "gunmetal", bevel=0.0015)
    return [guard, trig]


def bands_on_grip(y0, y1, n, r, mat, zscale=1.0, minor=0.0028):
    """Wrap bands round a round handle along Y."""
    out = []
    for k in range(n):
        y = y0 + (y1 - y0) * (k + 0.5) / n
        b = W.ring(f"band{k}", r, minor, (0, y, 0), mat, axis="Y", maj=10, minr=4)
        b.scale = (1, 1, zscale)
        out.append(b)
    return out


def sling(y_front, y_back, z_top, sag, mat="leather"):
    """A leather sling hanging under the weapon between two swivels."""
    pts = []
    for k in range(13):
        t = k / 12
        y = y_front + (y_back - y_front) * t
        z = z_top - sag * math.sin(math.pi * t) - 0.004
        pts.append((y, z))
    parts = [W.strap("sling", pts, 0.026, 0.004, mat)]
    for y in (y_front, y_back):
        parts.append(W.ring("swivel", 0.008, 0.0022, (0, y, z_top + 0.004), "metal_mid", axis="X", maj=10, minr=4))
    return parts


def cylinder_part(r, y0, y1, zc, chamber_r, flute_r, mat="steel"):
    """A six-shot cylinder (fluted, brass in the chambers) turning about Y through (0, mid, zc)."""
    pivot = (0.0, (y0 + y1) / 2, zc)
    cylp = W.cyl("cyl", r, y0, y1, mat, z=zc, verts=18, bevel=0.002)
    cutters = []
    for k in range(6):
        a = math.radians(30 + 60 * k)
        cutter = W.cyl("flute", flute_r, y0 + (y1 - y0) * 0.15, y1 - (y1 - y0) * 0.15, None, x=math.cos(a) * r * 1.02, z=zc + math.sin(a) * r * 1.02, verts=8, bevel=0.0)
        W.boolean_cut(cylp, cutter)
        cutters.append(cutter)
    parts = [cylp]
    for k in range(6):
        a = math.radians(60 * k)
        parts.append(W.cyl("chamber", chamber_r, y1 - 0.0004, y1 + 0.0006, "brass", x=math.cos(a) * r * 0.6, z=zc + math.sin(a) * r * 0.6, verts=8, bevel=0.0))
    parts.append(W.cyl("pin", r * 0.2, y1, y1 + 0.004, "gunmetal", z=zc, verts=8, bevel=0.0))
    return parts + cutters, pivot


# ---------------------------------------------------------------- the weapons

def scattergun():
    p = []
    # walnut stock: a straight English-style stock with a slim wrist where the hand sits
    p.append(W.profile("stock", [
        (0.035, 0.050), (-0.06, 0.041), (-0.11, 0.037), (-0.355, 0.043), (-0.365, 0.033),
        (-0.365, -0.104), (-0.35, -0.114), (-0.13, -0.058), (-0.07, -0.036), (-0.02, -0.024), (0.035, -0.018),
    ], 0.042, "wood", bevel=0.006, segments=2))
    p.append(W.box("buttpad", (0.044, 0.016, 0.152), (0, -0.373, -0.036), "rubber", bevel=0.004))
    p.append(W.profile("receiver", [(0.03, -0.018), (0.205, -0.018), (0.205, 0.060), (0.188, 0.078), (0.052, 0.078), (0.03, 0.064)], 0.05, "gunmetal", bevel=0.004, segments=2))
    p.append(W.box("port", (0.004, 0.06, 0.022), (0.025, 0.125, 0.045), "polymer", bevel=0.0015))
    p.append(W.box("gate", (0.022, 0.06, 0.004), (0, 0.15, -0.019), "metal_mid", bevel=0.001))
    p += trigger_group(0.045, 0.13, -0.018)
    p.append(W.cyl("barrel", 0.017, 0.2, 0.765, "gunmetal", z=0.055, verts=16))
    p.append(W.box("rib", (0.008, 0.55, 0.006), (0, 0.48, 0.074), "metal_mid", bevel=0.0015))
    for k in range(10):
        p.append(W.box(f"vent{k}", (0.006, 0.012, 0.004), (0, 0.235 + k * 0.055, 0.0695), "gunmetal", bevel=0.0))
    p.append(W.sphere("bead", 0.0045, (0, 0.752, 0.081), "brass", seg=8, rings=6))
    p.append(W.cyl("muzzle", 0.0195, 0.745, 0.768, "metal_mid", z=0.055, verts=16))
    p.append(W.cyl("bore", 0.012, 0.7675, 0.769, "polymer", z=0.055, verts=12, bevel=0.0))
    p.append(W.cyl("magtube", 0.014, 0.2, 0.705, "gunmetal", z=0.015, verts=12))
    p.append(W.cyl("magcap", 0.0165, 0.70, 0.725, "brass", z=0.015, verts=12))
    p.append(W.profile("clamp", [(0.655, 0.0), (0.69, 0.0), (0.69, 0.068), (0.655, 0.068)], 0.03, "metal_mid", bevel=0.004))
    # the pump: a ribbed wooden fore-end round the magazine tube, with its action bars
    pump = [W.cyl("forend", 0.031, 0.245, 0.415, "wood", z=0.017, verts=14, bevel=0.005, segments=2)]
    for k in range(5):
        pump.append(W.ring(f"groove{k}", 0.0305, 0.0034, (0, 0.27 + k * 0.03, 0.017), "wood_dark", maj=14, minr=4))
    for x in (-0.017, 0.017):
        pump.append(W.tube_path("bar", [(x, 0.18, 0.008), (x, 0.25, 0.008)], 0.0028, "metal_mid", sides=6))
    return p, {"pump": (pump, (0, 0, 0))}


def katana():
    p = []
    zs = 1.22  # the handle is a little taller than wide
    core = W.cyl("tsuka", 0.0152, -0.125, 0.125, "rayskin", verts=10, bevel=0.0)
    core.scale = (1, 1, zs)
    p.append(core)
    # the wrap: ribbons crossing over the rayskin, leaving diamonds
    for k in range(7):
        y = -0.105 + k * 0.035
        for s in (-1, 1):
            r = W.ring(f"ito{k}{s}", 0.0157, 0.0046, (0, 0, 0), "wrap", axis="Y", maj=10, minr=3)
            r.location = (0, y, 0)
            r.rotation_euler = (s * 0.42, 0, 0)
            r.scale = (1, 1, zs)
            p.append(r)
    kashira = W.cyl("kashira", 0.0168, -0.142, -0.124, "gunmetal", verts=12, bevel=0.003)
    kashira.scale = (1, 1, zs)
    p.append(kashira)
    fuchi = W.cyl("fuchi", 0.0172, 0.116, 0.129, "brass", verts=12, bevel=0.002)
    fuchi.scale = (1, 1, zs)
    p.append(fuchi)
    tsuba = W.cyl("tsuba", 0.043, 0.130, 0.139, "gunmetal", verts=20, bevel=0.002, segments=2)
    tsuba.scale = (1, 1, 1.12)
    p.append(tsuba)
    rim = W.ring("tsuba_rim", 0.043, 0.0026, (0, 0.1345, 0), "brass", maj=20, minr=4)
    rim.scale = (1, 1, 1.12)
    p.append(rim)
    p.append(W.cyl("seppa_a", 0.02, 0.127, 0.130, "brass", verts=12, bevel=0.0))
    p.append(W.cyl("seppa_b", 0.02, 0.139, 0.142, "brass", verts=12, bevel=0.0))
    p.append(W.box("habaki", (0.015, 0.026, 0.047), (0, 0.155, 0.006), "brass", bevel=0.002))
    # the blade: wide enough to read at a distance, curved (the tip bends towards the spine),
    # a groove (bo-hi) down the flat near the spine, and a bright bevelled edge (+Z)
    st = []
    y0, y1 = 0.16, 1.28
    n = 16
    for k in range(n + 1):
        t = k / n
        y = y0 + (y1 - y0) * t
        bend = -0.045 * t * t
        w = 0.044 - 0.009 * t
        zb = -0.016 + bend
        ze = zb + w
        ht = 0.0062 - 0.0024 * t
        groove = 0.45 if 0.05 < t < 0.72 else 0.0
        if y > 1.18:
            u = (y - 1.18) / (y1 - 1.18)
            ze = zb + w * max(0.0, 1 - u ** 1.6)
        st.append((y, zb, ze, ht, 0.55, groove))
    p.append(W.blade("blade", st, "steel", edge_mat="steel_edge"))
    return p, {}


def greatsword():
    p = []
    # a long two-hand grip, leather under a cord wrap
    grip = W.cyl("grip", 0.019, -0.27, 0.19, "leather", verts=10, bevel=0.0)
    grip.scale = (1, 1, 1.15)
    p.append(grip)
    for k in range(12):
        y = -0.255 + k * 0.037
        for s in (-1, 1):
            r = W.ring(f"cord{k}{s}", 0.0195, 0.0036, (0, 0, 0), "cord", axis="Y", maj=10, minr=3)
            r.location = (0, y, 0)
            r.rotation_euler = (s * 0.38, 0, 0)
            r.scale = (1, 1, 1.15)
            p.append(r)
    # a heavy faceted pommel
    p.append(W.cyl("pommel", 0.046, -0.335, -0.285, "brass", verts=8, bevel=0.006, segments=2))
    p.append(W.cyl("pommel_neck", 0.024, -0.29, -0.268, "gunmetal", verts=8, bevel=0.003))
    p.append(W.sphere("pommel_cap", 0.016, (0, -0.338, 0), "gunmetal", seg=8, rings=6))
    # a big crossguard: thick quillons sweeping forward, a block in the middle
    p.append(W.profile("guard", [
        (0.19, -0.05), (0.17, -0.12), (0.172, -0.19), (0.205, -0.205), (0.225, -0.16), (0.226, -0.07), (0.242, -0.04),
        (0.242, 0.04), (0.226, 0.07), (0.225, 0.16), (0.205, 0.205), (0.172, 0.19), (0.17, 0.12), (0.19, 0.05),
    ], 0.042, "gunmetal", bevel=0.006, segments=2))
    p.append(W.box("guard_block", (0.05, 0.06, 0.07), (0, 0.212, 0), "brass", bevel=0.007, segments=2))
    p.append(W.sphere("guard_gem", 0.011, (0.026, 0.212, 0), "red", seg=8, rings=6))
    p.append(W.sphere("guard_gem2", 0.011, (-0.026, 0.212, 0), "red", seg=8, rings=6))
    # a broad double-edged blade (twice a katana's width), a deep fuller most of its length
    st = []
    y0, y1 = 0.24, 1.72
    n = 12
    for k in range(n + 1):
        t = k / n
        y = y0 + (y1 - y0) * t
        hw = 0.052 - 0.01 * t
        if t > 0.78:
            u = (t - 0.78) / 0.22
            hw *= max(0.0, 1 - u) ** 0.8
        ht = 0.012 - 0.004 * t
        groove = 0.5 if t < 0.72 else 0.0
        st.append((y, -hw, hw, ht, 0.27, groove))
    p.append(W.blade("blade", st, "steel", edge_mat="steel_edge", double=True))
    # a leather collar over the ricasso
    p.append(W.box("ricasso", (0.028, 0.03, 0.11), (0, 0.262, 0), "leather", bevel=0.004))
    return p, {}


def dagger():
    """A fighting knife: short, broad, a clip point and a belly, so it reads apart from swords."""
    p = []
    # a wooden handle with finger grooves, a brass pommel cap and a brass S-guard
    p.append(W.profile("handle", [
        (-0.07, -0.013), (-0.05, -0.016), (-0.035, -0.012), (-0.02, -0.017), (-0.005, -0.012), (0.01, -0.017),
        (0.025, -0.012), (0.04, -0.017), (0.062, -0.014), (0.062, 0.016), (0.0, 0.017), (-0.07, 0.015),
    ], 0.026, "wood_dark", bevel=0.005, segments=2))
    for y in (-0.045, 0.035):
        p.append(W.cyl("rivet", 0.004, -0.0145, 0.0145, "brass", x=y, verts=8, axis="X", bevel=0.0))
    p.append(W.profile("pommel", [(-0.088, -0.015), (-0.07, -0.018), (-0.07, 0.018), (-0.088, 0.016), (-0.094, 0.0)], 0.03, "brass", bevel=0.004, segments=2))
    p.append(W.profile("guard", [
        (0.062, -0.026), (0.068, -0.04), (0.078, -0.046), (0.083, -0.036), (0.074, -0.024), (0.076, 0.0),
        (0.074, 0.024), (0.068, 0.036), (0.072, 0.048), (0.082, 0.046), (0.083, 0.03), (0.062, 0.026),
    ], 0.02, "brass", bevel=0.003, segments=2))
    # the blade: single edge (+Z) with a belly, the spine clipped down to the point
    st = []
    y0, y1 = 0.08, 0.37
    n = 9
    for k in range(n + 1):
        t = k / n
        y = y0 + (y1 - y0) * t
        zb = -0.014
        if t > 0.58:
            zb += 0.034 * ((t - 0.58) / 0.42) ** 1.3  # the clip
        ze = 0.034 + 0.006 * math.sin(math.pi * min(1.0, t * 1.1))
        if t > 0.7:
            ze -= 0.03 * ((t - 0.7) / 0.3) ** 1.6  # the belly sweeps up to the point
        st.append((y, zb, max(ze, zb + 0.004), 0.0065 - 0.002 * t, 0.5, 0.35 if 0.08 < t < 0.55 else 0.0))
    p.append(W.blade("blade", st, "steel", edge_mat="steel_edge"))
    return p, {}


def revolver():
    p = []
    p.append(W.profile("grip", [
        (-0.028, 0.004), (0.016, 0.004), (0.011, -0.03), (0.003, -0.084), (-0.009, -0.1), (-0.039, -0.098), (-0.047, -0.084), (-0.039, -0.03),
    ], 0.032, "wood", bevel=0.005, segments=2))
    p.append(W.tube_path("backstrap", [(0, -0.031, 0.006), (0, -0.044, -0.04), (0, -0.05, -0.084), (0, -0.044, -0.1)], 0.004, "metal_mid", sides=6))
    p.append(W.cyl("grip_screw", 0.004, -0.0175, 0.0175, "brass", z=-0.045, x=-0.012, verts=8, axis="X", bevel=0.0))
    p.append(W.profile("frame_low", [(-0.03, -0.004), (0.104, -0.004), (0.104, 0.009), (0.03, 0.009), (-0.03, 0.013)], 0.026, "metal_mid", bevel=0.003))
    p.append(W.profile("shield", [(-0.03, 0.008), (0.031, 0.008), (0.031, 0.06), (0.004, 0.064), (-0.03, 0.05)], 0.03, "metal_mid", bevel=0.004, segments=2))
    p.append(W.box("strap", (0.017, 0.062, 0.009), (0, 0.06, 0.0565), "metal_mid", bevel=0.003))
    p.append(W.profile("frame_front", [(0.088, -0.004), (0.106, -0.004), (0.106, 0.061), (0.088, 0.061)], 0.026, "metal_mid", bevel=0.003))
    p.append(W.profile("hammer", [(-0.018, 0.054), (-0.004, 0.058), (-0.01, 0.07), (-0.03, 0.081), (-0.035, 0.075), (-0.022, 0.064)], 0.008, "gunmetal", bevel=0.002))
    p += trigger_group(0.010, 0.072, -0.004, depth=0.032)
    p.append(W.cyl("barrel", 0.011, 0.1, 0.27, "metal_mid", z=0.04, verts=14))
    p.append(W.box("rib", (0.008, 0.17, 0.008), (0, 0.186, 0.052), "metal_mid", bevel=0.002))
    p.append(W.profile("sight", [(0.252, 0.054), (0.268, 0.054), (0.268, 0.066), (0.26, 0.066)], 0.004, "gunmetal", bevel=0.001))
    p.append(W.cyl("ejector", 0.006, 0.1, 0.2, "metal_mid", z=0.022, verts=8))
    p.append(W.cyl("bore", 0.006, 0.2695, 0.2715, "polymer", z=0.04, verts=10, bevel=0.0))
    cyl, pivot = cylinder_part(0.021, 0.033, 0.087, 0.030, 0.0052, 0.0052)
    return p, {"cylinder": (cyl, pivot)}


def hand_cannon():
    """The boss pistol: an oversized revolver, a long heavy barrel with a full underlug and a
    vented rib, an exposed spur hammer, engraved steel and a wooden grip with brass."""
    p = []
    p.append(W.profile("grip", [
        (-0.034, 0.004), (0.02, 0.004), (0.014, -0.034), (0.006, -0.098), (-0.008, -0.118), (-0.046, -0.116), (-0.056, -0.1), (-0.047, -0.034),
    ], 0.04, "wood", bevel=0.006, segments=2))
    p.append(W.profile("butt_cap", [(-0.009, -0.114), (-0.046, -0.112), (-0.058, -0.1), (-0.06, -0.118), (-0.05, -0.128), (-0.006, -0.124)], 0.042, "brass", bevel=0.003))
    # brass medallions set in both grip panels
    for x0, x1 in ((0.0195, 0.0215), (-0.0215, -0.0195)):
        p.append(W.cyl("medallion", 0.008, x0, x1, "brass", x=-0.02, z=-0.05, verts=10, axis="X", bevel=0.0))
    p.append(W.tube_path("backstrap", [(0, -0.036, 0.006), (0, -0.05, -0.04), (0, -0.058, -0.098)], 0.0048, "engraved", sides=6))
    # the engraved frame: under the cylinder, the recoil shield, the top strap, the front
    p.append(W.profile("frame_low", [(-0.036, -0.006), (0.13, -0.006), (0.13, 0.012), (0.04, 0.012), (-0.036, 0.016)], 0.032, "engraved", bevel=0.004, segments=2))
    p.append(W.profile("shield", [(-0.036, 0.01), (0.038, 0.01), (0.038, 0.074), (0.004, 0.08), (-0.036, 0.064)], 0.036, "engraved", bevel=0.005, segments=2))
    p.append(W.box("strap", (0.022, 0.08, 0.012), (0, 0.076, 0.072), "engraved", bevel=0.004))
    p.append(W.profile("frame_front", [(0.11, -0.006), (0.132, -0.006), (0.132, 0.078), (0.11, 0.078)], 0.032, "engraved", bevel=0.004))
    # a big spur hammer, cocked back
    p.append(W.profile("hammer", [(-0.022, 0.066), (-0.006, 0.072), (-0.014, 0.088), (-0.04, 0.104), (-0.052, 0.1), (-0.046, 0.092), (-0.028, 0.08)], 0.011, "gunmetal", bevel=0.0025))
    p += trigger_group(0.012, 0.086, -0.006, depth=0.038, mat="engraved", r=0.0048)
    # the long heavy barrel: square-sided with a full-length underlug, a vented rib on top
    p.append(W.cyl("barrel", 0.019, 0.13, 0.43, "gunmetal", z=0.056, verts=16, bevel=0.003))
    p.append(W.profile("underlug", [(0.13, 0.008), (0.405, 0.012), (0.43, 0.03), (0.43, 0.052), (0.13, 0.052)], 0.026, "gunmetal", bevel=0.005, segments=2))
    rib = W.box("rib", (0.013, 0.3, 0.012), (0, 0.28, 0.079), "gunmetal", bevel=0.003)
    for k in range(6):
        c = W.box("vent", (0.03, 0.018, 0.008), (0, 0.17 + k * 0.045, 0.082), None, bevel=0.0)
        W.boolean_cut(rib, c)
    p.append(rib)
    p.append(W.profile("front_sight", [(0.405, 0.084), (0.428, 0.084), (0.428, 0.102), (0.415, 0.102)], 0.006, "brass", bevel=0.0015))
    p.append(W.box("rear_sight", (0.026, 0.012, 0.012), (0, -0.01, 0.086), "gunmetal", bevel=0.002))
    p.append(W.cyl("bore", 0.0095, 0.4305, 0.4325, "polymer", z=0.056, verts=12, bevel=0.0))
    p.append(W.ring("crown", 0.0165, 0.0035, (0, 0.43, 0.056), "metal_mid", maj=16, minr=4))
    cyl, pivot = cylinder_part(0.027, 0.04, 0.11, 0.042, 0.0068, 0.0065, mat="engraved")
    return p, {"cylinder": (cyl, pivot)}


def smg():
    p = []
    p.append(W.profile("receiver", [(-0.09, 0.005), (0.26, 0.005), (0.26, 0.085), (0.242, 0.095), (-0.072, 0.095), (-0.09, 0.08)], 0.044, "gunmetal", bevel=0.005, segments=2))
    p.append(W.box("rail", (0.02, 0.24, 0.007), (0, 0.08, 0.099), "metal_mid", bevel=0.001))
    for k in range(10):
        p.append(W.box(f"slot{k}", (0.022, 0.006, 0.004), (0, -0.025 + k * 0.022, 0.1035), "metal_mid", bevel=0.0))
    p.append(W.box("rear_sight", (0.026, 0.014, 0.016), (0, -0.06, 0.106), "gunmetal", bevel=0.002))
    p.append(W.box("front_sight", (0.008, 0.012, 0.018), (0, 0.235, 0.107), "gunmetal", bevel=0.002))
    p.append(W.box("port", (0.003, 0.06, 0.022), (0.0225, 0.12, 0.06), "polymer", bevel=0.001))
    p.append(W.cyl("handle", 0.005, -0.045, -0.02, "metal_mid", x=0.18, z=0.075, verts=8, axis="X"))
    p.append(W.sphere("knob", 0.007, (-0.05, 0.18, 0.075), "polymer", seg=8, rings=6))
    shroud = W.cyl("shroud", 0.021, 0.255, 0.37, "gunmetal", z=0.05, verts=14, bevel=0.002)
    for k in range(3):
        c = W.cyl("hole", 0.0055, -0.04, 0.04, None, x=0.28 + k * 0.03, z=0.05, verts=8, axis="X", bevel=0.0)
        W.boolean_cut(shroud, c)
    p.append(shroud)
    p.append(W.cyl("barrel", 0.0085, 0.36, 0.41, "metal_mid", z=0.05, verts=10))
    p.append(W.cyl("bore", 0.0045, 0.4095, 0.411, "polymer", z=0.05, verts=8, bevel=0.0))
    p.append(pistol_grip("grip", "polymer", width=0.034))
    p += trigger_group(0.03, 0.092, 0.006, depth=0.036)
    p.append(W.profile("mag", [(0.10, 0.006), (0.136, 0.006), (0.146, -0.15), (0.11, -0.15)], 0.026, "metal_mid", bevel=0.003))
    p.append(W.box("mag_base", (0.032, 0.044, 0.01), (0, 0.128, -0.154), "polymer", bevel=0.002))
    for x in (-0.017, 0.017):
        p.append(W.tube_path("stock", [(x, -0.09, 0.074), (x, -0.24, 0.07), (x, -0.262, 0.04), (x, -0.262, -0.005), (x, -0.24, -0.02), (x, -0.09, 0.02)], 0.0045, "metal_mid", sides=6))
    p.append(W.box("stock_pad", (0.05, 0.012, 0.09), (0, -0.268, 0.025), "rubber", bevel=0.003))
    return p, {}


def rifle():
    p = []
    p.append(W.profile("stock", [(-0.05, 0.07), (-0.41, 0.058), (-0.43, 0.05), (-0.43, -0.078), (-0.41, -0.09), (-0.08, -0.02), (-0.05, 0.005)], 0.042, "wood_dark", bevel=0.006, segments=2))
    p.append(W.box("buttplate", (0.044, 0.012, 0.13), (0, -0.436, -0.013), "gunmetal", bevel=0.003))
    p.append(W.profile("receiver", [(-0.05, 0.0), (0.25, 0.0), (0.25, 0.075), (-0.05, 0.08)], 0.046, "gunmetal", bevel=0.004, segments=2))
    p.append(W.box("cover", (0.04, 0.28, 0.012), (0, 0.1, 0.083), "metal_mid", bevel=0.005, segments=2))
    p.append(W.box("port", (0.003, 0.06, 0.02), (0.0235, 0.12, 0.05), "polymer", bevel=0.001))
    p.append(W.box("charge", (0.026, 0.012, 0.012), (0.034, 0.2, 0.055), "metal_mid", bevel=0.002))
    p.append(W.profile("grip", [(-0.036, 0.002), (0.02, 0.002), (0.015, -0.02), (0.0, -0.1), (-0.038, -0.095), (-0.042, -0.03)], 0.032, "wood_dark", bevel=0.005, segments=2))
    p += trigger_group(0.03, 0.1, 0.0)
    p.append(W.profile("mag", [(0.15, 0.0), (0.16, -0.06), (0.18, -0.12), (0.215, -0.19), (0.18, -0.208), (0.143, -0.132), (0.124, -0.066), (0.113, 0.0)], 0.03, "gunmetal", bevel=0.003))
    p.append(W.profile("handguard", [(0.25, 0.0), (0.52, 0.012), (0.532, 0.03), (0.52, 0.078), (0.25, 0.082)], 0.05, "wood", bevel=0.006, segments=2))
    p.append(W.cyl("gastube", 0.009, 0.52, 0.665, "gunmetal", z=0.068, verts=10))
    p.append(W.box("gasblock", (0.022, 0.03, 0.042), (0, 0.668, 0.056), "metal_mid", bevel=0.003))
    p.append(W.cyl("barrel", 0.011, 0.52, 0.885, "gunmetal", z=0.045, verts=12))
    p.append(W.profile("post", [(0.842, 0.045), (0.872, 0.045), (0.866, 0.09), (0.85, 0.09)], 0.012, "gunmetal", bevel=0.002))
    brake = W.cyl("brake", 0.015, 0.88, 0.935, "metal_mid", z=0.045, verts=12, bevel=0.002)
    for k in range(2):
        c = W.box("slot", (0.05, 0.008, 0.012), (0, 0.897 + k * 0.018, 0.045), None, bevel=0.0)
        W.boolean_cut(brake, c)
    p.append(brake)
    p.append(W.cyl("bore", 0.006, 0.9345, 0.936, "polymer", z=0.045, verts=8, bevel=0.0))
    p.append(W.profile("dot", [(0.04, 0.088), (0.15, 0.088), (0.14, 0.122), (0.06, 0.127)], 0.03, "polymer", bevel=0.004, segments=2))
    p.append(W.cyl("lens", 0.0105, 0.1395, 0.1415, "glass", z=0.107, verts=12, bevel=0.0))
    return p, {}


def sniper():
    p = []
    # a walnut stock with a leather cheek pad
    p.append(W.profile("stock", [
        (0.45, 0.032), (0.45, 0.0), (0.40, -0.012), (0.12, -0.03), (0.03, -0.03), (0.01, -0.06), (0.0, -0.1), (-0.04, -0.1),
        (-0.045, -0.05), (-0.09, -0.025), (-0.2, -0.045), (-0.455, -0.105), (-0.47, -0.095), (-0.47, 0.055), (-0.455, 0.07),
        (-0.2, 0.078), (-0.13, 0.05), (-0.06, 0.03), (0.0, 0.028),
    ], 0.046, "wood", bevel=0.006, segments=2))
    p.append(W.profile("cheek", [(-0.36, 0.045), (-0.16, 0.06), (-0.16, 0.0), (-0.36, -0.02)], 0.05, "leather", bevel=0.006, segments=2))
    for y in (-0.33, -0.19):
        p.append(W.cyl("lace", 0.003, -0.026, 0.026, "cord", x=y, z=0.02, verts=6, axis="X", bevel=0.0))
    p.append(W.box("buttpad", (0.048, 0.018, 0.162), (0, -0.478, -0.022), "rubber", bevel=0.004))
    p.append(W.cyl("receiver", 0.019, -0.01, 0.25, "gunmetal", z=0.05, verts=16))
    p.append(W.cyl("bolt", 0.0085, -0.045, -0.005, "steel", z=0.05, verts=10))
    p.append(W.tube_path("bolt_handle", [(0.0, 0.03, 0.05), (0.04, 0.03, 0.046), (0.055, 0.028, 0.026)], 0.005, "steel", sides=6))
    p.append(W.sphere("bolt_knob", 0.009, (0.058, 0.027, 0.02), "steel", seg=10, rings=6))
    p += trigger_group(0.025, 0.09, -0.03, depth=0.03)
    p.append(W.box("mag", (0.03, 0.07, 0.034), (0, 0.1, -0.042), "gunmetal", bevel=0.003))
    p.append(W.cyl("barrel", 0.0155, 0.25, 1.12, "gunmetal", r1=0.012, z=0.05, verts=14))
    brake = W.box("brake", (0.034, 0.085, 0.028), (0, 1.16, 0.05), "metal_mid", bevel=0.004, segments=2)
    for k in range(3):
        c = W.box("slot", (0.05, 0.008, 0.016), (0, 1.133 + k * 0.02, 0.05), None, bevel=0.0)
        W.boolean_cut(brake, c)
    p.append(brake)
    p.append(W.cyl("bore", 0.006, 1.2025, 1.204, "polymer", z=0.05, verts=8, bevel=0.0))
    # the scope: tube, bells, turrets, lenses, rings, and flip-up caps standing open
    zc = 0.125
    p.append(W.cyl("scope", 0.015, -0.03, 0.245, "gunmetal", z=zc, verts=16))
    p.append(W.cyl("objective_cone", 0.015, 0.245, 0.285, "gunmetal", r1=0.026, z=zc, verts=16))
    p.append(W.cyl("objective", 0.026, 0.285, 0.33, "gunmetal", z=zc, verts=16))
    p.append(W.cyl("ocular_cone", 0.02, -0.06, -0.03, "gunmetal", r1=0.015, z=zc, verts=16))
    p.append(W.cyl("ocular", 0.021, -0.1, -0.06, "gunmetal", z=zc, verts=16))
    p.append(W.ring("ocular_grip", 0.0215, 0.0025, (0, -0.08, zc), "rubber", maj=16, minr=4))
    p.append(W.cyl("turret_top", 0.01, 0.138, 0.158, "brass", x=0.12, verts=12, axis="Z"))
    p.append(W.cyl("turret_side", 0.009, 0.013, 0.032, "brass", x=0.12, z=zc, verts=12, axis="X"))
    p.append(W.cyl("lens_front", 0.022, 0.3295, 0.3315, "glass", z=zc, verts=16, bevel=0.0))
    p.append(W.cyl("lens_back", 0.016, -0.1015, -0.0995, "glass", z=zc, verts=16, bevel=0.0))
    for y, r in ((0.334, 0.027), (-0.104, 0.022)):
        cap = W.cyl("cap", r, -0.003, 0.003, "polymer", verts=16, bevel=0.0015)
        cap.location = (0, y, zc + r + 0.003)
        cap.rotation_euler = (math.radians(-70 if y > 0 else 70), 0, 0)
        p.append(cap)
    for y in (0.03, 0.2):
        p.append(W.ring("scope_ring", 0.0168, 0.0042, (0, y, zc), "metal_mid", maj=16, minr=4))
        p.append(W.box("ring_base", (0.02, 0.014, 0.05), (0, y, 0.09), "metal_mid", bevel=0.003))
    p.append(W.box("bipod_hinge", (0.03, 0.02, 0.016), (0, 0.41, -0.012), "gunmetal", bevel=0.003))
    for x in (-0.012, 0.012):
        p.append(W.tube_path("bipod_leg", [(x, 0.405, -0.02), (x, 0.16, -0.032)], 0.0042, "gunmetal", sides=6))
    p += sling(0.36, -0.3, -0.03, 0.09)
    return p, {}


def rocket_launcher():
    p = []
    zc = 0.13
    p.append(W.cyl("tube", 0.062, -0.5, 0.5, "gunmetal", z=zc, verts=20, bevel=0.003))
    for y in (-0.38, 0.38):
        p.append(W.ring("band", 0.0625, 0.006, (0, y, zc), "brass", maj=20, minr=4))
    # a wooden heat guard round the middle, held by two straps
    p.append(W.cyl("guard", 0.0705, -0.17, 0.16, "wood", z=zc, verts=20, bevel=0.006, segments=2))
    for y in (-0.15, 0.14):
        p.append(W.ring("guard_strap", 0.071, 0.0045, (0, y, zc), "metal_mid", maj=20, minr=4))
    p.append(W.cyl("venturi", 0.085, -0.62, -0.5, "gunmetal", r1=0.062, z=zc, verts=20, bevel=0.003))
    p.append(W.cyl("venturi_in", 0.07, -0.6215, -0.6195, "polymer", z=zc, verts=16, bevel=0.0))
    p.append(W.cyl("muzzle", 0.072, 0.5, 0.56, "metal_mid", z=zc, verts=20, bevel=0.004))
    # the warhead loaded in the front: a painted body with a red band and a steel nose
    p.append(W.cyl("warhead_base", 0.05, 0.53, 0.58, "olive_dark", z=zc, verts=16))
    p.append(W.cyl("warhead", 0.058, 0.58, 0.68, "olive", r1=0.04, z=zc, verts=16))
    p.append(W.ring("warhead_band", 0.05, 0.0045, (0, 0.66, zc), "red", maj=16, minr=4))
    p.append(W.cyl("warhead_nose", 0.04, 0.68, 0.74, "metal_mid", r1=0.008, z=zc, verts=16))
    p.append(W.sphere("warhead_tip", 0.009, (0, 0.742, zc), "brass", seg=8, rings=6))
    p.append(W.box("mount", (0.03, 0.12, 0.07), (0, 0.02, 0.045), "gunmetal", bevel=0.005, segments=2))
    p.append(pistol_grip("grip", "wood", width=0.034))
    p += trigger_group(0.03, 0.092, 0.006, depth=0.036)
    p.append(W.profile("foregrip", [(0.28, 0.075), (0.32, 0.075), (0.315, -0.03), (0.285, -0.03)], 0.03, "wood", bevel=0.005, segments=2))
    p.append(W.box("sight", (0.014, 0.06, 0.05), (-0.068, 0.06, 0.19), "gunmetal", bevel=0.003))
    p.append(W.cyl("sight_lens", 0.012, 0.0895, 0.0915, "glass", x=-0.068, z=0.195, verts=12, bevel=0.0))
    p.append(W.box("sight_hood", (0.03, 0.03, 0.006), (-0.068, 0.1, 0.214), "rubber", bevel=0.002))
    p.append(W.box("post_front", (0.006, 0.01, 0.03), (0, 0.45, 0.205), "gunmetal", bevel=0.001))
    p.append(W.box("post_rear", (0.018, 0.01, 0.026), (0, -0.2, 0.203), "gunmetal", bevel=0.001))
    p.append(W.box("shoulder", (0.06, 0.18, 0.02), (0, -0.3, 0.062), "leather", bevel=0.005, segments=2))
    p += sling(0.42, -0.42, 0.068, 0.12)
    return p, {}


def shuriken():
    p = []
    cy, cz = 0.08, 0.05
    pts = []
    for k in range(8):
        a = math.radians(45 * k)
        r = 0.1 if k % 2 == 0 else 0.026
        pts.append((math.cos(a) * r, cy + math.sin(a) * r))
    star = W.top_profile("star", pts, 0.009, "steel", z=cz, bevel=0.0042, segments=1)
    star.modifiers["bevel"].angle_limit = math.radians(25)
    hole = W.cyl("hole", 0.0075, cz - 0.02, cz + 0.02, None, x=cy, verts=12, axis="Z", bevel=0.0)
    W.boolean_cut(star, hole)
    p.append(star)
    for s in (-1, 1):
        p.append(W.ring("hub", 0.015, 0.0022, (0, cy, cz + s * 0.0045), "brass", axis="Z", maj=14, minr=4))
    return p, {}


# the teal of the Bridgewright's Rig (lit from within in the game, like a lens)
W.PALETTE["glass_teal"] = ((0.04, 0.62, 0.58), 0.0, 0.05)
T.RECIPES["glass_teal"] = ("glass", (0.03, 0.50, 0.46), (0.25, 0.85, 0.80), (0.8, 1.0, 1.0), 0.5, 0.2)


def rigger_hook():
    """The Rigger's Hook (Ledger Duels, the Bridgewright's Rig): a one-handed hooked axe. An
    oak haft wrapped in leather, a dark iron socket, a broad steel bit with a bright edge, a
    curved hook on the back (it catches on a chain when thrown), a teal inlay, and a short chain
    hanging from the ring at its butt."""
    p = []
    p.append(W.cyl("haft", 0.016, -0.15, 0.54, "wood", verts=10))
    p.append(W.cyl("grip_wrap", 0.0185, -0.13, 0.09, "leather", verts=10))
    for k in range(5):
        p.append(W.ring(f"wrap_band{k}", 0.0188, 0.003, (0, -0.11 + k * 0.045, 0), "leather", maj=10, minr=4))
    p.append(W.ring("pommel_ring", 0.026, 0.006, (0, -0.175, 0), "metal_dark", axis="X", maj=14, minr=5))
    for k in range(5):
        r = W.ring(f"chain{k}", 0.012, 0.0032, (0, -0.2 - 0.004 * k, -0.02 - 0.02 * k), "metal_dark", axis="X" if k % 2 else "Z", maj=10, minr=4)
        p.append(r)
    p.append(W.cyl("socket", 0.023, 0.40, 0.54, "metal_dark", verts=10, bevel=0.003))
    p.append(W.cyl("socket_band", 0.026, 0.40, 0.415, "brass", verts=12, bevel=0.0))
    p.append(W.profile("bit", [(0.40, 0.02), (0.57, 0.02), (0.63, 0.08), (0.635, 0.17), (0.56, 0.215), (0.46, 0.19), (0.40, 0.10)], 0.014, "steel", bevel=0.004, segments=2))
    p.append(W.profile("bit_edge", [(0.615, 0.075), (0.645, 0.08), (0.648, 0.175), (0.565, 0.228), (0.555, 0.212), (0.632, 0.168), (0.628, 0.085)], 0.007, "steel_edge", bevel=0.001))
    p.append(W.profile("hook", [(0.46, -0.02), (0.535, -0.02), (0.525, -0.075), (0.47, -0.125), (0.395, -0.145), (0.36, -0.125), (0.42, -0.112), (0.46, -0.075)], 0.016, "metal_dark", bevel=0.004, segments=2))
    p.append(W.cyl("inlay", 0.016, -0.0085, 0.0085, "glass_teal", verts=14, axis="X", bevel=0.0))
    inlay = p[-1]
    inlay.location = (0, 0.52, 0.115)
    p.append(W.box("cap", (0.024, 0.03, 0.024), (0, 0.55, 0), "metal_dark", bevel=0.004))
    return p, {}


def rigger_snub():
    """The Rigger's Snub (Ledger Duels): a stubby off-hand pistol. A brass frame, a short dark
    barrel with a brass muzzle ring, a wooden grip with a lanyard ring, a teal stripe."""
    p = []
    p.append(W.profile("grip", [(-0.026, 0.004), (0.014, 0.004), (0.01, -0.03), (0.002, -0.08), (-0.01, -0.094), (-0.036, -0.092), (-0.042, -0.08), (-0.035, -0.03)], 0.03, "wood", bevel=0.005, segments=2))
    p.append(W.profile("frame", [(-0.03, -0.004), (0.075, -0.004), (0.075, 0.052), (-0.018, 0.058), (-0.03, 0.046)], 0.03, "brass", bevel=0.004, segments=2))
    p.append(W.profile("hammer", [(-0.02, 0.05), (-0.006, 0.054), (-0.012, 0.066), (-0.03, 0.074), (-0.034, 0.068)], 0.008, "gunmetal", bevel=0.002))
    p += trigger_group(0.008, 0.05, -0.004, depth=0.028)
    p.append(W.cyl("barrel", 0.015, 0.075, 0.17, "gunmetal", z=0.034, verts=14, bevel=0.002))
    p.append(W.ring("muzzle", 0.0165, 0.004, (0, 0.168, 0.034), "brass", maj=14, minr=5))
    p.append(W.cyl("bore", 0.008, 0.169, 0.171, "polymer", z=0.034, verts=10, bevel=0.0))
    p.append(W.ring("lanyard", 0.011, 0.0028, (0, -0.03, -0.1), "metal_dark", axis="X", maj=10, minr=4))
    p.append(W.box("stripe", (0.031, 0.06, 0.006), (0, 0.025, 0.053), "glass_teal", bevel=0.0))
    return p, {}


def ashblood_glaive():
    """The Ashblood Glaive (the Ashen Vault, from Lord Vesper Kael): a polearm held like a
    greatsword. A long black shaft with gilt bands, a leather grip where the hands go, a long
    curved blade with a bright edge, a hooked spur on its back, a socket with a red gem, a red
    tassel under the head and a spiked butt cap."""
    p = []
    p.append(W.cyl("shaft", 0.017, -0.95, 1.34, "metal_dark", verts=10, bevel=0.0))
    p.append(W.cyl("grip", 0.0195, -0.3, 0.22, "leather", verts=10, bevel=0.0))
    for k, y in enumerate((-0.32, 0.24, 0.7, 1.05)):
        p.append(W.cyl(f"band{k}", 0.023, y - 0.018, y + 0.018, "brass", verts=12, bevel=0.003))
    p.append(W.cyl("butt", 0.024, -1.05, -0.95, "brass", r1=0.02, verts=10, bevel=0.003))
    p.append(W.cyl("butt_spike", 0.02, -1.17, -1.05, "steel", r1=0.002, verts=8, bevel=0.0))
    p.append(W.cyl("socket", 0.03, 1.3, 1.44, "metal_dark", r1=0.024, verts=10, bevel=0.004, segments=2))
    p.append(W.sphere("socket_gem", 0.017, (0, 1.37, 0.028), "red", seg=10, rings=6))
    p.append(W.sphere("socket_gem2", 0.017, (0, 1.37, -0.028), "red", seg=10, rings=6))
    # the blade: long, curving back towards its tip, single-edged
    st = []
    n = 14
    for k in range(n + 1):
        t = k / n
        y = 1.42 + 0.78 * t
        hw = 0.095 * (1.0 - t) ** 0.55 + 0.006
        ht = 0.011 - 0.006 * t
        st.append((y, -hw * 0.4, hw, ht, 0.22, 0.35 if 0.1 < t < 0.7 else 0.0))
    blade = W.blade("blade", st, "steel", edge_mat="steel_edge", double=False)
    p.append(blade)
    p.append(W.profile("spur", [(1.45, -0.02), (1.53, -0.02), (1.56, -0.07), (1.6, -0.14), (1.55, -0.11), (1.5, -0.06)], 0.012, "metal_dark", bevel=0.003, segments=2))
    # the red tassel under the head
    for k in range(5):
        a = k * 1.256
        p.append(W.cyl(f"tassel{k}", 0.006, 1.12, 1.29, "red", x=math.sin(a) * 0.012, z=math.cos(a) * 0.012, r1=0.009, verts=5, bevel=0.0))
    p.append(W.ring("tassel_knot", 0.021, 0.006, (0, 1.29, 0), "brass", maj=10, minr=4))
    return p, {}


WEAPONS = {
    "scattergun": scattergun,
    "katana": katana,
    "revolver": revolver,
    "hand_cannon": hand_cannon,
    "smg": smg,
    "rifle": rifle,
    "sniper": sniper,
    "rocket_launcher": rocket_launcher,
    "shuriken": shuriken,
    "dagger": dagger,
    "greatsword": greatsword,
    "rigger_hook": rigger_hook,
    "rigger_snub": rigger_snub,
    "ashblood_glaive": ashblood_glaive,
}
MELEE = {"katana", "dagger", "greatsword", "shuriken", "rigger_hook", "ashblood_glaive"}


def build(name, renders=True):
    W.clear_scene()
    static, moving = WEAPONS[name]()
    body = W.bake(static, "body")
    parts = [body]
    for part, (objs, pivot) in moving.items():
        parts.append(W.bake(objs, part, pivot=pivot))
    tris = sum(W.triangles(o) for o in parts)
    lo, hi = W._bounds(parts)
    for d in (OUT_MODELS, OUT_TEX):
        os.makedirs(d, exist_ok=True)
    T.bake_texture(parts, os.path.join(OUT_TEX, f"{name}.png"))
    W.export_glb(parts, os.path.join(OUT_MODELS, f"{name}.glb"))
    # blades keep more of their shape far away (they're thin, and read by their outline)
    far = W.export_far(parts, os.path.join(OUT_MODELS, f"{name}_far.glb"), ratio=0.6 if name in MELEE else 0.3)
    print(f"[weapons] {name}: {tris} triangles ({far} far away), {len(parts)} parts, size x {hi.x - lo.x:.3f} y {hi.y - lo.y:.3f} z {hi.z - lo.z:.3f} m")
    if renders:
        os.makedirs(OUT_ICONS, exist_ok=True)
        os.makedirs(OUT_SHOTS, exist_ok=True)
        W.render_icon(parts, os.path.join(OUT_ICONS, f"{name}.png"), top=(name == "shuriken"))
        W.render_turntable(parts, name, os.path.join(OUT_SHOTS, f"{name}.png"))
    return tris


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    renders = "--no-renders" not in argv
    names = [a for a in argv if not a.startswith("--")] or list(WEAPONS)
    total = {}
    for n in names:
        total[n] = build(n, renders)
    if renders and len(names) == len(WEAPONS):
        W.contact_sheet([os.path.join(OUT_SHOTS, f"{n}.png") for n in WEAPONS], os.path.join(OUT_SHOTS, "_all.png"))
    print("[weapons] done:", ", ".join(f"{k} {v}" for k, v in total.items()))


main()
