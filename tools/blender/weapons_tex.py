"""Stylised, hand-painted-looking textures for the weapons, baked in Blender (Cycles).

Every palette material gets a procedural "paint" recipe: wood with visible grain and darker
edges, metal with subtle wear, lighter worn edges and darker recesses, brass, leather and cord
wraps, rayskin, engraving. Edge highlights (from a bevel-normal mask) and ambient occlusion are
painted into the colour, so the weapons read at a distance with no lighting tricks. The whole
weapon (all its parts) is unwrapped into one square and baked into one image; the game draws
the model with that image.
"""

import math
import bpy

from weapons_lib import PALETTE

# per material: (recipe, base colour, second colour, edge colour, edge strength, ao strength)
RECIPES = {
    "metal_dark": ("metal", (0.050, 0.052, 0.060), (0.085, 0.088, 0.098), (0.42, 0.43, 0.46), 0.75, 0.75),
    "gunmetal": ("metal", (0.045, 0.050, 0.060), (0.080, 0.088, 0.100), (0.40, 0.42, 0.46), 0.8, 0.75),
    "metal_mid": ("metal", (0.16, 0.165, 0.18), (0.24, 0.245, 0.26), (0.62, 0.63, 0.66), 0.7, 0.7),
    "steel": ("brushed", (0.56, 0.58, 0.62), (0.70, 0.72, 0.76), (0.95, 0.96, 0.98), 0.7, 0.55),
    "steel_edge": ("brushed", (0.85, 0.87, 0.90), (0.95, 0.96, 0.98), (1.0, 1.0, 1.0), 0.6, 0.25),
    "steel_blue": ("metal", (0.20, 0.24, 0.32), (0.28, 0.33, 0.42), (0.70, 0.74, 0.80), 0.7, 0.7),
    "engraved": ("engraved", (0.50, 0.51, 0.55), (0.66, 0.67, 0.70), (0.92, 0.93, 0.95), 0.6, 0.6),
    "brass": ("brass", (0.62, 0.40, 0.11), (0.80, 0.56, 0.20), (1.0, 0.86, 0.52), 0.8, 0.65),
    "copper": ("brass", (0.50, 0.20, 0.08), (0.65, 0.30, 0.12), (0.95, 0.62, 0.42), 0.7, 0.65),
    "wood": ("wood", (0.10, 0.032, 0.010), (0.24, 0.09, 0.028), (0.035, 0.012, 0.004), 0.75, 0.75),
    "wood_dark": ("wood", (0.055, 0.018, 0.007), (0.13, 0.05, 0.018), (0.02, 0.007, 0.003), 0.75, 0.75),
    "wood_light": ("wood", (0.20, 0.085, 0.028), (0.38, 0.18, 0.06), (0.07, 0.03, 0.01), 0.7, 0.7),
    "leather": ("leather", (0.085, 0.042, 0.020), (0.15, 0.08, 0.04), (0.035, 0.018, 0.009), 0.5, 0.7),
    "leather_red": ("cord", (0.22, 0.03, 0.02), (0.40, 0.07, 0.04), (0.10, 0.012, 0.008), 0.45, 0.7),
    "cord": ("cord", (0.07, 0.045, 0.03), (0.16, 0.11, 0.07), (0.03, 0.02, 0.012), 0.45, 0.7),
    "wrap": ("cord", (0.018, 0.022, 0.055), (0.05, 0.06, 0.13), (0.008, 0.01, 0.025), 0.4, 0.75),
    "rayskin": ("rayskin", (0.50, 0.47, 0.40), (0.74, 0.71, 0.62), (0.30, 0.28, 0.24), 0.3, 0.8),
    "polymer": ("polymer", (0.020, 0.020, 0.024), (0.040, 0.040, 0.046), (0.20, 0.20, 0.22), 0.55, 0.6),
    "rubber": ("polymer", (0.014, 0.014, 0.016), (0.03, 0.03, 0.032), (0.10, 0.10, 0.11), 0.35, 0.6),
    "olive": ("paint", (0.10, 0.13, 0.06), (0.15, 0.19, 0.09), (0.40, 0.41, 0.40), 0.7, 0.7),
    "olive_dark": ("paint", (0.045, 0.06, 0.03), (0.08, 0.10, 0.05), (0.32, 0.33, 0.32), 0.7, 0.7),
    "tan": ("leather", (0.32, 0.22, 0.11), (0.45, 0.32, 0.17), (0.16, 0.10, 0.05), 0.5, 0.7),
    "red": ("paint", (0.45, 0.03, 0.02), (0.60, 0.06, 0.03), (0.45, 0.43, 0.42), 0.6, 0.7),
    "glass": ("glass", (0.08, 0.35, 0.75), (0.30, 0.65, 1.0), (0.9, 0.95, 1.0), 0.5, 0.2),
}


# ---------------------------------------------------------------- node helpers

class G:
    """A small builder over a material's node tree."""

    def __init__(self, mat):
        mat.use_nodes = True
        self.nt = mat.node_tree
        self.nt.nodes.clear()
        self.x = 0

    def n(self, kind, **props):
        node = self.nt.nodes.new(kind)
        node.location = (self.x, 0)
        self.x += 200
        for k, v in props.items():
            setattr(node, k, v)
        return node

    def link(self, a, b):
        self.nt.links.new(a, b)

    def rgb(self, c):
        node = self.n("ShaderNodeRGB")
        node.outputs[0].default_value = (*c, 1.0)
        return node.outputs[0]

    def value(self, v):
        node = self.n("ShaderNodeValue")
        node.outputs[0].default_value = v
        return node.outputs[0]

    def math(self, op, a, b=None, clamp=False):
        node = self.n("ShaderNodeMath", operation=op, use_clamp=clamp)
        for i, s in enumerate((a, b)):
            if s is None:
                continue
            if isinstance(s, (int, float)):
                node.inputs[i].default_value = s
            else:
                self.link(s, node.inputs[i])
        return node.outputs[0]

    def mix(self, fac, a, b, blend="MIX"):
        node = self.n("ShaderNodeMix", data_type="RGBA", blend_type=blend)
        node.clamp_factor = True
        cols = [s for s in node.inputs if s.type == "RGBA"]
        facs = [s for s in node.inputs if s.type == "VALUE"]
        if isinstance(fac, (int, float)):
            facs[0].default_value = fac
        else:
            self.link(fac, facs[0])
        for sock, v in ((cols[0], a), (cols[1], b)):
            if isinstance(v, tuple):
                sock.default_value = (*v, 1.0)
            else:
                self.link(v, sock)
        return [s for s in node.outputs if s.type == "RGBA"][0]

    def coords(self, scale=(1, 1, 1)):
        tc = self.n("ShaderNodeTexCoord")
        mp = self.n("ShaderNodeMapping")
        mp.inputs["Scale"].default_value = scale
        self.link(tc.outputs["Object"], mp.inputs["Vector"])
        return mp.outputs[0]

    def noise(self, vec, scale, detail=4.0, rough=0.55):
        node = self.n("ShaderNodeTexNoise")
        node.inputs["Scale"].default_value = scale
        node.inputs["Detail"].default_value = detail
        node.inputs["Roughness"].default_value = rough
        self.link(vec, node.inputs["Vector"])
        return node.outputs["Fac"]

    def wave(self, vec, scale, distortion, direction="Z", kind="BANDS", detail=3.0):
        node = self.n("ShaderNodeTexWave", wave_type=kind)
        if kind == "BANDS":
            node.bands_direction = direction
        node.inputs["Scale"].default_value = scale
        node.inputs["Distortion"].default_value = distortion
        node.inputs["Detail"].default_value = detail
        self.link(vec, node.inputs["Vector"])
        return node.outputs["Fac"]

    def voronoi(self, vec, scale, feature="F1"):
        node = self.n("ShaderNodeTexVoronoi", feature=feature)
        node.inputs["Scale"].default_value = scale
        self.link(vec, node.inputs["Vector"])
        return node.outputs["Distance"]

    def ramp(self, fac, stops):
        node = self.n("ShaderNodeValToRGB")
        el = node.color_ramp.elements
        while len(el) > 2:
            el.remove(el[-1])
        for i, (pos, col) in enumerate(stops):
            e = el[i] if i < 2 else el.new(pos)
            e.position = pos
            e.color = (*col, 1.0) if len(col) == 3 else col
        self.link(fac, node.inputs["Fac"])
        return node.outputs["Color"]

    def edge_mask(self, radius=0.0035, gain=7.0):
        """1 on convex edges and corners, 0 on flat faces (the bevel shader's normal leans away
        from the true normal near an edge)."""
        bev = self.n("ShaderNodeBevel")
        bev.samples = 8
        bev.inputs["Radius"].default_value = radius
        geo = self.n("ShaderNodeNewGeometry")
        dot = self.n("ShaderNodeVectorMath", operation="DOT_PRODUCT")
        self.link(bev.outputs["Normal"], dot.inputs[0])
        self.link(geo.outputs["Normal"], dot.inputs[1])
        d = dot.outputs["Value"]
        return self.math("MULTIPLY", self.math("SUBTRACT", 1.0, d), gain, clamp=True)

    def ao(self, distance=0.03):
        node = self.n("ShaderNodeAmbientOcclusion")
        node.samples = 16
        node.only_local = True
        node.inputs["Distance"].default_value = distance
        return node.outputs["AO"]

    def emit(self, col):
        em = self.n("ShaderNodeEmission")
        self.link(col, em.inputs["Color"])
        em.inputs["Strength"].default_value = 1.0
        out = self.n("ShaderNodeOutputMaterial")
        self.link(em.outputs[0], out.inputs["Surface"])


def paint_material(mat, palette_name):
    """Turn a material into its painted recipe (output as emission, for the colour bake)."""
    recipe, c0, c1, ce, edge_k, ao_k = RECIPES.get(palette_name, RECIPES["metal_mid"])
    g = G(mat)
    edge = g.edge_mask()
    occ = g.ao()
    if recipe == "wood":
        # grain runs along the length (Y): bands across the other two axes, wobbling
        v = g.coords((1.0, 0.035, 1.0))
        grain = g.wave(v, 26.0, 7.0, "X", detail=4.0)
        rings = g.wave(g.coords((0.6, 0.05, 1.0)), 9.0, 4.0, "Z", detail=2.0)
        f = g.math("MULTIPLY", g.math("ADD", grain, rings), 0.5)
        col = g.ramp(f, [(0.0, c0), (0.45, tuple((a + b) / 2 for a, b in zip(c0, c1))), (1.0, c1)])
        blot = g.noise(g.coords((3, 0.4, 3)), 4.0)
        col = g.mix(g.math("MULTIPLY", blot, 0.35), col, c0)
        # darker, handled edges
        col = g.mix(g.math("MULTIPLY", edge, edge_k), col, ce)
    elif recipe in ("metal", "paint", "engraved"):
        n1 = g.noise(g.coords(), 38.0, 5.0)
        col = g.ramp(n1, [(0.3, c0), (0.75, c1)])
        # fine scratches along the length
        scr = g.noise(g.coords((14.0, 1.2, 14.0)), 9.0, 8.0, 0.7)
        scr = g.math("GREATER_THAN", scr, 0.68)
        col = g.mix(g.math("MULTIPLY", scr, 0.22), col, ce)
        if recipe == "engraved":
            # scrollwork: thin dark lines cut into the metal
            lines = g.voronoi(g.coords((1, 1, 1)), 190.0, "DISTANCE_TO_EDGE")
            curls = g.wave(g.coords((1, 1, 1)), 70.0, 9.0, "DIAGONAL", kind="RINGS", detail=2.0)
            cut = g.math("MAXIMUM", g.math("LESS_THAN", lines, 0.045), g.math("LESS_THAN", curls, 0.12))
            col = g.mix(g.math("MULTIPLY", cut, 0.8), col, (0.06, 0.06, 0.07))
        if recipe == "paint":
            # chipped paint shows bare metal at the edges
            chips = g.math("GREATER_THAN", g.math("ADD", edge, g.math("MULTIPLY", g.noise(g.coords(), 60.0), 0.6)), 0.95)
            col = g.mix(chips, col, ce)
        else:
            col = g.mix(g.math("MULTIPLY", edge, edge_k), col, ce)
    elif recipe == "brushed":
        streak = g.noise(g.coords((40.0, 0.6, 40.0)), 12.0, 6.0, 0.6)
        col = g.ramp(streak, [(0.25, c0), (0.8, c1)])
        col = g.mix(g.math("MULTIPLY", edge, edge_k), col, ce)
    elif recipe == "brass":
        n1 = g.noise(g.coords(), 30.0, 4.0)
        col = g.ramp(n1, [(0.3, c0), (0.8, c1)])
        col = g.mix(g.math("MULTIPLY", edge, edge_k), col, ce)
    elif recipe == "leather":
        n1 = g.noise(g.coords(), 70.0, 6.0, 0.7)
        col = g.ramp(n1, [(0.3, c0), (0.7, c1)])
        crease = g.math("LESS_THAN", g.voronoi(g.coords(), 40.0, "DISTANCE_TO_EDGE"), 0.05)
        col = g.mix(g.math("MULTIPLY", crease, 0.5), col, ce)
        col = g.mix(g.math("MULTIPLY", edge, edge_k), col, ce)
    elif recipe == "cord":
        # twisted cord: tight diagonal fibres
        fib = g.wave(g.coords((1, 1, 1)), 140.0, 2.0, "DIAGONAL", detail=1.0)
        col = g.ramp(fib, [(0.2, c0), (0.8, c1)])
        col = g.mix(g.math("MULTIPLY", edge, edge_k), col, ce)
    elif recipe == "rayskin":
        dots = g.voronoi(g.coords(), 220.0)
        col = g.ramp(dots, [(0.15, c1), (0.45, c0)])
    elif recipe == "polymer":
        n1 = g.noise(g.coords(), 120.0, 3.0)
        col = g.ramp(n1, [(0.3, c0), (0.7, c1)])
        col = g.mix(g.math("MULTIPLY", edge, edge_k), col, ce)
    else:  # glass
        col = g.ramp(g.noise(g.coords(), 8.0), [(0.2, c0), (0.8, c1)])
    # darker recesses
    shade = g.mix(occ, (1.0 - ao_k, 1.0 - ao_k, 1.0 - ao_k), (1.0, 1.0, 1.0))
    col = g.mix(1.0, col, shade, blend="MULTIPLY")
    g.emit(col)


# ---------------------------------------------------------------- unwrap and bake

def _select(objects):
    bpy.ops.object.select_all(action="DESELECT")
    for ob in objects:
        ob.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]


def unwrap(objects):
    """One UV square for all the weapon's parts (their islands packed together)."""
    _select(objects)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=math.radians(60), island_margin=0.006, scale_to_bounds=False)
    try:
        bpy.ops.uv.pack_islands(rotate=True, margin=0.006)
    except TypeError:
        bpy.ops.uv.pack_islands(margin=0.006)
    bpy.ops.object.mode_set(mode="OBJECT")


def bake_texture(objects, out_path, size=512, samples=48):
    """Paint and bake every part's materials into one image, saved at `out_path`; then the
    parts wear display materials that use it (for the preview renders and the glTF)."""
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
        paint_material(m, m.name.split(".")[0])
        node = m.node_tree.nodes.new("ShaderNodeTexImage")
        node.image = img
        m.node_tree.nodes.active = node
    unwrap(objects)
    _select(objects)
    scene.render.bake.margin = 6
    scene.render.bake.use_clear = True
    bpy.ops.object.bake(type="EMIT", margin=6, use_clear=True)
    img.filepath_raw = out_path
    img.file_format = "PNG"
    img.save()
    # display materials: the baked colour, each material's own shine
    for ob in objects:
        for i, slot in enumerate(ob.material_slots):
            if slot.material is None:
                continue
            pname = slot.material.name.split(".")[0]
            slot.material = display_material(pname, img)
    return img


def display_material(pname, img):
    name = f"{pname}__shown"
    m = bpy.data.materials.get(name)
    if m is None:
        m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    nt.nodes.clear()
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    nt.links.new(bsdf.outputs[0], out.inputs["Surface"])
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = img
    nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    _, metal, rough = PALETTE.get(pname, ((0.5, 0.5, 0.5), 0.0, 0.5))
    bsdf.inputs["Metallic"].default_value = metal
    bsdf.inputs["Roughness"].default_value = rough
    if pname == "glass":
        emit = bsdf.inputs.get("Emission Color")
        if emit is not None:
            nt.links.new(tex.outputs["Color"], emit)
            bsdf.inputs["Emission Strength"].default_value = 1.5
    return m
