"""Helpers for building Duel Rush's stylised weapon models in Blender (headless).

Conventions (every weapon):
- Blender axes: +X is the weapon's right side, +Y points forward (barrel / blade), +Z is up.
  The game reads the exported glTF as x = right, y = up, z = forward.
- Units are metres. The origin is where the hand holds it: the palm on a gun's grip, the
  middle of a blade's handle.
- Everything is made from bevelled pieces (no plain boxes): extruded side profiles, swept
  blade sections, tubes. Flat, stylised colours like the Quaternius characters, no textures.
- Parts the game moves keep their own object: `pump` (slides back along -Y) and `cylinder`
  (spins about its own Y axis; its origin is the pivot).

Run through build_weapons.py:  blender --background --python build_weapons.py -- [names]
"""

import math
import bmesh
import bpy
from mathutils import Matrix, Vector

# ---------------------------------------------------------------- materials

PALETTE = {
    # name: (base colour (linear), metallic, roughness)
    "metal_dark": ((0.055, 0.057, 0.065), 0.9, 0.38),
    "metal_mid": ((0.20, 0.21, 0.23), 0.9, 0.34),
    "steel": ((0.66, 0.68, 0.72), 1.0, 0.24),
    "gunmetal": ((0.06, 0.065, 0.075), 0.9, 0.36),
    "engraved": ((0.55, 0.56, 0.60), 1.0, 0.30),
    "cord": ((0.10, 0.07, 0.045), 0.0, 0.85),
    "steel_edge": ((0.92, 0.93, 0.95), 1.0, 0.12),
    "steel_blue": ((0.30, 0.34, 0.42), 1.0, 0.30),
    "brass": ((0.72, 0.48, 0.15), 1.0, 0.32),
    "copper": ((0.60, 0.25, 0.10), 1.0, 0.36),
    "wood": ((0.20, 0.075, 0.025), 0.0, 0.62),
    "wood_dark": ((0.085, 0.034, 0.014), 0.0, 0.60),
    "wood_light": ((0.33, 0.15, 0.05), 0.0, 0.62),
    "leather": ((0.12, 0.06, 0.03), 0.0, 0.85),
    "leather_red": ((0.30, 0.04, 0.03), 0.0, 0.80),
    "wrap": ((0.025, 0.03, 0.07), 0.0, 0.80),
    "rayskin": ((0.58, 0.55, 0.47), 0.0, 0.70),
    "polymer": ((0.024, 0.024, 0.028), 0.0, 0.60),
    "olive": ((0.13, 0.17, 0.08), 0.0, 0.62),
    "olive_dark": ((0.06, 0.08, 0.04), 0.0, 0.66),
    "tan": ((0.42, 0.31, 0.17), 0.0, 0.66),
    "red": ((0.55, 0.04, 0.025), 0.0, 0.50),
    "rubber": ((0.018, 0.018, 0.02), 0.0, 0.92),
    # lenses glow a little in the game (drawn unlit)
    "glass": ((0.10, 0.45, 0.85), 0.0, 0.05),
}


def material(name):
    """A shared Principled material from the palette."""
    m = bpy.data.materials.get(name)
    if m is not None:
        return m
    col, metal, rough = PALETTE[name]
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = (*col, 1.0)
    bsdf.inputs["Metallic"].default_value = metal
    bsdf.inputs["Roughness"].default_value = rough
    if name == "glass":
        emit = bsdf.inputs.get("Emission Color") or bsdf.inputs.get("Emission")
        if emit is not None:
            emit.default_value = (0.15, 0.5, 1.0, 1.0)
        strength = bsdf.inputs.get("Emission Strength")
        if strength is not None:
            strength.default_value = 1.5
    m.diffuse_color = (*col, 1.0)
    m.metallic = metal
    m.roughness = rough
    return m


# ---------------------------------------------------------------- scene

def clear_scene():
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    for coll in (bpy.data.meshes, bpy.data.curves, bpy.data.cameras, bpy.data.lights):
        for b in list(coll):
            if b.users == 0:
                coll.remove(b)


def new_object(name, bm, mat, bevel=0.0, segments=1, angle=40.0):
    """Turn a bmesh into an object with one material and an optional bevel."""
    me = bpy.data.meshes.new(name)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    if mat is not None:
        ob.data.materials.append(material(mat))
    if bevel > 0.0:
        mod = ob.modifiers.new("bevel", "BEVEL")
        mod.width = bevel
        mod.segments = segments
        mod.limit_method = "ANGLE"
        mod.angle_limit = math.radians(angle)
        mod.harden_normals = False
    return ob


# ---------------------------------------------------------------- shapes

def profile(name, pts, thick, mat, x=0.0, bevel=0.003, segments=1):
    """A side profile (list of (y, z) points, in order) extruded `thick` across X, centred on x."""
    bm = bmesh.new()
    vs = [bm.verts.new((x - thick / 2, y, z)) for (y, z) in pts]
    face = bm.faces.new(vs)
    ext = bmesh.ops.extrude_face_region(bm, geom=[face])
    moved = [g for g in ext["geom"] if isinstance(g, bmesh.types.BMVert)]
    bmesh.ops.translate(bm, verts=moved, vec=(thick, 0, 0))
    return new_object(name, bm, mat, bevel, segments)


def top_profile(name, pts, height, mat, z=0.0, bevel=0.003, segments=1):
    """A top-view profile (list of (x, y) points) extruded `height` up Z, centred on z."""
    bm = bmesh.new()
    vs = [bm.verts.new((px, py, z - height / 2)) for (px, py) in pts]
    face = bm.faces.new(vs)
    ext = bmesh.ops.extrude_face_region(bm, geom=[face])
    moved = [g for g in ext["geom"] if isinstance(g, bmesh.types.BMVert)]
    bmesh.ops.translate(bm, verts=moved, vec=(0, 0, height))
    return new_object(name, bm, mat, bevel, segments)


def box(name, size, loc, mat, bevel=0.003, segments=1):
    """A bevelled box (size = full x, y, z) centred at loc."""
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=size, verts=bm.verts)
    bmesh.ops.translate(bm, vec=loc, verts=bm.verts)
    return new_object(name, bm, mat, bevel, segments)


def cyl(name, r, y0, y1, mat, x=0.0, z=0.0, r1=None, verts=16, bevel=0.002, segments=1, axis="Y"):
    """A bevelled cylinder (or cone, with r1) from y0 to y1 along `axis` (Y by default).
    Along Y it sits at (x, z); along X (from y0 to y1 as X values) at Y = x, Z = z; along Z
    (from y0 to y1 as Z values) at X = z, Y = x."""
    bm = bmesh.new()
    r1 = r if r1 is None else r1
    bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=verts, radius1=r, radius2=r1, depth=abs(y1 - y0))
    # create_cone builds along Z, centred; turn it to the axis and move into place
    if axis == "Y":
        bmesh.ops.rotate(bm, verts=bm.verts, cent=(0, 0, 0), matrix=Matrix.Rotation(-math.pi / 2, 3, "X"))
        bmesh.ops.translate(bm, verts=bm.verts, vec=(x, (y0 + y1) / 2, z))
    elif axis == "X":
        bmesh.ops.rotate(bm, verts=bm.verts, cent=(0, 0, 0), matrix=Matrix.Rotation(math.pi / 2, 3, "Y"))
        bmesh.ops.translate(bm, verts=bm.verts, vec=((y0 + y1) / 2, x, z))
    else:
        bmesh.ops.translate(bm, verts=bm.verts, vec=(z, x, (y0 + y1) / 2))
    return new_object(name, bm, mat, bevel, segments)


def ring(name, r_major, r_minor, loc, mat, axis="Y", maj=16, minr=6):
    """A torus around `axis` (a band, a wrap, a muzzle ring)."""
    bm = bmesh.new()
    grid = []
    for i in range(maj):
        a = 2 * math.pi * i / maj
        row = []
        for j in range(minr):
            b = 2 * math.pi * j / minr
            rr = r_major + r_minor * math.cos(b)
            p = Vector((rr * math.cos(a), r_minor * math.sin(b), rr * math.sin(a)))  # around Y
            if axis == "X":
                p = Vector((p.y, p.x, p.z))
            elif axis == "Z":
                p = Vector((p.x, p.z, p.y))
            row.append(bm.verts.new(p + Vector(loc)))
        grid.append(row)
    for i in range(maj):
        for j in range(minr):
            a, b = grid[i][j], grid[(i + 1) % maj][j]
            c, d = grid[(i + 1) % maj][(j + 1) % minr], grid[i][(j + 1) % minr]
            bm.faces.new((a, b, c, d))
    return new_object(name, bm, mat)


def tube_path(name, pts, r, mat, sides=8, closed=False):
    """A round tube following a path of (x, y, z) points (trigger guards, wire stocks, rods)."""
    pts = [Vector(p) for p in pts]
    n = len(pts)
    bm = bmesh.new()
    rings = []
    prev_side = None
    for i, p in enumerate(pts):
        if closed:
            t = (pts[(i + 1) % n] - pts[(i - 1) % n]).normalized()
        elif i == 0:
            t = (pts[1] - pts[0]).normalized()
        elif i == n - 1:
            t = (pts[-1] - pts[-2]).normalized()
        else:
            t = ((pts[i + 1] - pts[i]).normalized() + (pts[i] - pts[i - 1]).normalized()).normalized()
        ref = prev_side if prev_side is not None else (Vector((1, 0, 0)) if abs(t.x) < 0.9 else Vector((0, 0, 1)))
        side = (ref - t * ref.dot(t)).normalized()
        up = t.cross(side).normalized()
        prev_side = side
        rings.append([bm.verts.new(p + (side * math.cos(2 * math.pi * k / sides) + up * math.sin(2 * math.pi * k / sides)) * r) for k in range(sides)])
    segs = n if closed else n - 1
    for i in range(segs):
        a, b = rings[i], rings[(i + 1) % n]
        for k in range(sides):
            bm.faces.new((a[k], a[(k + 1) % sides], b[(k + 1) % sides], b[k]))
    if not closed:
        bm.faces.new(list(reversed(rings[0])))
        bm.faces.new(rings[-1])
    return new_object(name, bm, mat)


def sphere(name, r, loc, mat, seg=12, rings=8, scale=(1, 1, 1)):
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=seg, v_segments=rings, radius=r)
    bmesh.ops.scale(bm, vec=scale, verts=bm.verts)
    bmesh.ops.translate(bm, vec=loc, verts=bm.verts)
    return new_object(name, bm, mat)


def blade(name, stations, mat, edge_mat=None, double=False, fuller=None):
    """A blade swept along Y. Each station is (y, z_back, z_edge, half_thick, ridge[, fuller])
    where z_back is the spine (or the other edge, when `double`), z_edge the cutting edge,
    half_thick the thickness at the ridge, ridge (0..1) where the bevel starts (share of the
    width from the back; on a double edge, from each edge) and fuller (0..1) how deep a groove
    runs down the flat (0: none; defaults to `fuller`). The bevel from the ridge to the edge is
    its own face, in `edge_mat` (the bright cutting edge). The last station is the point."""
    bm = bmesh.new()
    mats = [material(mat)]
    if edge_mat:
        mats.append(material(edge_mat))
    rows = []
    for k, st in enumerate(stations):
        y, zb, ze, ht, ridge = st[:5]
        fd = st[5] if len(st) > 5 else (fuller or 0.0)
        if k == len(stations) - 1:
            pt = bm.verts.new((0, y, zb + (ze - zb) * (0.5 if double else 0.3)))
            rows.append([pt] * (12 if double else 11))
            continue
        if double:
            # back edge, its bevel line, the fuller (top, middle, bottom), the front bevel line,
            # the front edge, and back up the other side
            zr = zb + (ze - zb) * ridge
            zr2 = ze - (ze - zb) * ridge
            zm = (zb + ze) / 2
            fw = (zr2 - zr) * 0.32
            tm = ht * (1 - fd)
            side = lambda s: [
                bm.verts.new((s * ht, y, zr)),
                bm.verts.new((s * ht, y, zm - fw)),
                bm.verts.new((s * tm, y, zm)),
                bm.verts.new((s * ht, y, zm + fw)),
                bm.verts.new((s * ht, y, zr2)),
            ]
            a = bm.verts.new((0, y, zb))
            plus = side(1)
            e = bm.verts.new((0, y, ze))
            minus = side(-1)
            rows.append([a] + plus + [e] + list(reversed(minus)))
        else:
            # spine (both sides), the groove near the spine, the bevel line, the edge
            zr = zb + (ze - zb) * ridge
            back = ht * 0.8
            g0, g1 = zb + (zr - zb) * 0.18, zb + (zr - zb) * 0.62
            gm = (g0 + g1) / 2
            tm = ht * (1 - fd)
            p = [bm.verts.new((back, y, zb)), bm.verts.new((ht, y, g0)), bm.verts.new((tm, y, gm)), bm.verts.new((ht, y, g1)), bm.verts.new((ht, y, zr))]
            e = bm.verts.new((0, y, ze))
            m = [bm.verts.new((-ht, y, zr)), bm.verts.new((-ht, y, g1)), bm.verts.new((-tm, y, gm)), bm.verts.new((-ht, y, g0)), bm.verts.new((-back, y, zb))]
            rows.append(p + [e] + m)
    n = len(rows[0])
    # which ring sides are the edge bevel
    if double:
        edge_sides = {0, 5, 6, 11}
    else:
        edge_sides = {4, 5}
    for i in range(len(rows) - 1):
        a, b = rows[i], rows[i + 1]
        for k in range(n):
            k2 = (k + 1) % n
            quad = (a[k], a[k2], b[k2], b[k])
            uniq = []
            for v in quad:
                if v not in uniq:
                    uniq.append(v)
            if len(uniq) < 3:
                continue
            try:
                f = bm.faces.new(uniq)
            except ValueError:
                continue
            if edge_mat and k in edge_sides:
                f.material_index = 1
    base = list(dict.fromkeys(rows[0]))
    try:
        bm.faces.new(base)
    except ValueError:
        pass
    me = bpy.data.meshes.new(name)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    for m in mats:
        ob.data.materials.append(m)
    return ob


def strap(name, pts, width, thick, mat):
    """A flat strap (a sling) along a path of (y, z) points, `width` across X."""
    pts = [Vector((0, y, z)) for (y, z) in pts]
    bm = bmesh.new()
    rings = []
    for i, p in enumerate(pts):
        t = (pts[min(i + 1, len(pts) - 1)] - pts[max(i - 1, 0)]).normalized()
        nrm = Vector((1, 0, 0)).cross(t).normalized()
        w = Vector((width / 2, 0, 0))
        h = nrm * (thick / 2)
        rings.append([bm.verts.new(p + w + h), bm.verts.new(p - w + h), bm.verts.new(p - w - h), bm.verts.new(p + w - h)])
    for i in range(len(rings) - 1):
        a, b = rings[i], rings[i + 1]
        for k in range(4):
            bm.faces.new((a[k], a[(k + 1) % 4], b[(k + 1) % 4], b[k]))
    bm.faces.new(list(reversed(rings[0])))
    bm.faces.new(rings[-1])
    return new_object(name, bm, mat)

def boolean_cut(target, cutter):
    """Cut `cutter` out of `target` (the cutter is removed)."""
    mod = target.modifiers.new("cut", "BOOLEAN")
    mod.operation = "DIFFERENCE"
    mod.object = cutter
    try:
        mod.solver = "EXACT"
    except TypeError:
        pass
    # booleans go before the bevel
    i = target.modifiers.find(mod.name)
    if i > 0:
        target.modifiers.move(i, 0)
    cutter.hide_render = True
    cutter.hide_viewport = True
    return mod


def mirror_x(ob):
    """A mirrored copy across X (left and right parts)."""
    c = ob.copy()
    c.data = ob.data.copy()
    bpy.context.scene.collection.objects.link(c)
    c.scale.x = -1
    return c


# ---------------------------------------------------------------- finishing

def bake(objects, name, pivot=(0.0, 0.0, 0.0)):
    """Merge pieces (modifiers and transforms applied) into one object named `name`, its
    origin at `pivot`; smooth shading with sharp edges past 35 degrees (crisp bevels)."""
    depsgraph = bpy.context.evaluated_depsgraph_get()
    bm = bmesh.new()
    mats = []
    for ob in objects:
        if ob.hide_render:
            continue
        ev = ob.evaluated_get(depsgraph)
        me = bpy.data.meshes.new_from_object(ev, depsgraph=depsgraph)
        me.transform(ob.matrix_world)
        local = []
        for m in me.materials:
            if m not in mats:
                mats.append(m)
            local.append(mats.index(m))
        before = len(bm.faces)
        bm.from_mesh(me)
        bm.faces.ensure_lookup_table()
        for f in bm.faces[before:]:
            f.material_index = local[f.material_index] if local else 0
        bpy.data.meshes.remove(me)
    # cutters and pieces go
    for ob in objects:
        bpy.data.objects.remove(ob, do_unlink=True)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-6)
    bmesh.ops.translate(bm, verts=bm.verts, vec=(-pivot[0], -pivot[1], -pivot[2]))
    for f in bm.faces:
        f.smooth = True
    for e in bm.edges:
        if len(e.link_faces) == 2 and e.calc_face_angle(0.0) > math.radians(35):
            e.smooth = False
        elif len(e.link_faces) != 2:
            e.smooth = False
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    for m in mats:
        me.materials.append(m)
    ob = bpy.data.objects.new(name, me)
    ob.location = pivot
    bpy.context.scene.collection.objects.link(ob)
    return ob


def triangles(ob):
    return sum(len(p.vertices) - 2 for p in ob.data.polygons)


def export_glb(objects, path):
    bpy.ops.object.select_all(action="DESELECT")
    for ob in objects:
        ob.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        use_selection=True,
        export_apply=True,
        export_yup=True,
        export_normals=True,
        export_texcoords=True,
        export_image_format="NONE",
        export_materials="EXPORT",
        export_animations=False,
    )


# ---------------------------------------------------------------- pictures

def _bounds(objects):
    lo = Vector((1e9, 1e9, 1e9))
    hi = Vector((-1e9, -1e9, -1e9))
    for ob in objects:
        for c in ob.bound_box:
            w = ob.matrix_world @ Vector(c)
            lo = Vector((min(lo.x, w.x), min(lo.y, w.y), min(lo.z, w.z)))
            hi = Vector((max(hi.x, w.x), max(hi.y, w.y), max(hi.z, w.z)))
    return lo, hi


def _camera(name, loc, target, ortho=None):
    cam = bpy.data.cameras.new(name)
    if ortho:
        cam.type = "ORTHO"
        cam.ortho_scale = ortho
    else:
        cam.lens = 50
        cam.sensor_fit = "VERTICAL"
        cam.sensor_height = 24.0
    cam.clip_start = 0.01
    cam.clip_end = 50
    ob = bpy.data.objects.new(name, cam)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = loc
    d = (Vector(target) - Vector(loc)).normalized()
    ob.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()
    bpy.context.scene.camera = ob
    return ob


def _light(name, kind, loc, energy, target=(0, 0, 0), size=1.0, color=(1, 1, 1)):
    l = bpy.data.lights.new(name, kind)
    l.energy = energy
    l.color = color
    if hasattr(l, "size"):
        l.size = size
    ob = bpy.data.objects.new(name, l)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = loc
    d = (Vector(target) - Vector(loc)).normalized()
    ob.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()
    return ob


def _use_engine(scene, names):
    for n in names:
        try:
            scene.render.engine = n
            return n
        except TypeError:
            continue
    return scene.render.engine


def _pixels(path):
    img = bpy.data.images.load(path, check_existing=False)
    w, h = img.size
    px = list(img.pixels)
    bpy.data.images.remove(img)
    return w, h, px


def render_turntable(objects, title, out_path, size=(1600, 900)):
    """Four views around the weapon (a turntable) side by side in one picture."""
    scene = bpy.context.scene
    _use_engine(scene, ["BLENDER_EEVEE", "BLENDER_EEVEE_NEXT"])
    scene.render.film_transparent = False
    scene.render.resolution_x = size[0] // 2
    scene.render.resolution_y = size[1] // 2
    scene.render.image_settings.file_format = "PNG"
    # the palette's own colours (no filmic look), on a soft grey
    try:
        scene.view_settings.view_transform = "Standard"
    except TypeError:
        pass
    world = bpy.data.worlds.get("preview") or bpy.data.worlds.new("preview")
    world.use_nodes = True
    bg = world.node_tree.nodes.get("Background")
    bg.inputs[0].default_value = (0.16, 0.16, 0.18, 1.0)
    bg.inputs[1].default_value = 1.0
    scene.world = world
    lo, hi = _bounds(objects)
    centre = (lo + hi) / 2
    radius = max((hi - lo).length / 2, 0.08)
    made = []
    made.append(_light("key", "AREA", centre + Vector((1.2, -1.0, 1.4)) * radius * 3, 75 * radius * radius * 9, centre, size=radius * 2))
    made.append(_light("fill", "AREA", centre + Vector((-1.4, -0.6, 0.4)) * radius * 3, 28 * radius * radius * 9, centre, size=radius * 3, color=(0.75, 0.85, 1.0)))
    made.append(_light("rim", "AREA", centre + Vector((0.0, 1.5, 1.0)) * radius * 3, 90 * radius * radius * 9, centre, size=radius * 2, color=(1.0, 0.85, 0.7)))
    tiles = []
    # right side, front three-quarter, left side, from above behind: the whole weapon in frame
    # fitted across the wide picture (long weapons lie across it)
    half_fov = math.atan(12.0 * (size[0] / size[1]) / 50.0)
    dist = radius / math.sin(half_fov) * 1.06
    for k, (az, el) in enumerate(((0, 12), (55, 25), (180, 12), (300, 50))):
        a, e = math.radians(az), math.radians(el)
        loc = centre + Vector((math.cos(a) * math.cos(e), math.sin(a) * math.cos(e), math.sin(e))) * dist
        cam = _camera(f"turn{k}", loc, centre)
        path = out_path.replace(".png", f"_{k}.png")
        scene.render.filepath = path
        bpy.ops.render.render(write_still=True)
        tiles.append(path)
        bpy.data.objects.remove(cam, do_unlink=True)
    for ob in made:
        bpy.data.objects.remove(ob, do_unlink=True)
    _stitch(tiles, out_path, cols=2)
    return out_path


def _stitch(paths, out_path, cols=2):
    import os
    imgs = [_pixels(p) for p in paths]
    w, h = imgs[0][0], imgs[0][1]
    rows = (len(imgs) + cols - 1) // cols
    W, H = w * cols, h * rows
    out = [0.0] * (W * H * 4)
    for i, (iw, ih, px) in enumerate(imgs):
        cx, cy = i % cols, rows - 1 - i // cols  # Blender images start at the bottom
        for y in range(ih):
            src = y * iw * 4
            dst = ((cy * h + y) * W + cx * w) * 4
            out[dst:dst + iw * 4] = px[src:src + iw * 4]
    img = bpy.data.images.new("sheet", W, H, alpha=True)
    img.pixels.foreach_set(out)
    img.filepath_raw = out_path
    img.file_format = "PNG"
    img.save()
    bpy.data.images.remove(img)
    for p in paths:
        try:
            os.remove(p)
        except OSError:
            pass


def render_icon(objects, out_path, size=(520, 200), top=False):
    """The HUD / loadout icon: the weapon's right side, pointing right, in shades of grey on
    a clear background (the game tints it). Orthographic, so every icon lines up."""
    scene = bpy.context.scene
    _use_engine(scene, ["BLENDER_WORKBENCH"])
    sh = scene.display.shading
    sh.light = "STUDIO"
    sh.color_type = "TEXTURE"
    sh.show_cavity = True
    sh.cavity_type = "BOTH"
    sh.curvature_ridge_factor = 1.4
    sh.curvature_valley_factor = 1.6
    sh.show_object_outline = True
    sh.object_outline_color = (0.02, 0.02, 0.02)
    sh.show_specular_highlight = True
    scene.render.film_transparent = True
    scene.render.resolution_x, scene.render.resolution_y = size
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    lo, hi = _bounds(objects)
    centre = (lo + hi) / 2
    span_y = hi.y - lo.y
    span_z = hi.z - lo.z
    aspect = size[0] / size[1]
    ortho = max(span_y, span_z * aspect, 0.05) * 1.08
    if top:
        # flat things (the star) from above, forward still to the right
        span_x = hi.x - lo.x
        ortho = max(span_y, span_x * aspect, 0.05) * 1.08
        cam = _camera("iconcam", centre + Vector((0, 0, 3.0)), centre, ortho=ortho)
        cam.rotation_euler = (0, 0, math.pi / 2)
    else:
        # from the right side: forward (+Y) points to the right of the picture
        cam = _camera("iconcam", centre + Vector((3.0, 0, 0)), centre, ortho=ortho)
    scene.render.filepath = out_path
    bpy.ops.render.render(write_still=True)
    bpy.data.objects.remove(cam, do_unlink=True)
    # to grey: the game multiplies it by the icon colour, so keep it bright, with the shading
    w, h, px = _pixels(out_path)
    out = list(px)
    for i in range(0, len(px), 4):
        a = px[i + 3]
        if a <= 0.0:
            continue
        r, g, b = px[i], px[i + 1], px[i + 2]
        lum = 0.2126 * r + 0.7152 * g + 0.0722 * b
        v = 0.30 + 0.70 * min(1.0, lum * 1.6) ** 0.7
        out[i] = out[i + 1] = out[i + 2] = v
    img = bpy.data.images.new("icon", w, h, alpha=True)
    img.pixels.foreach_set(out)
    img.filepath_raw = out_path
    img.file_format = "PNG"
    img.save()
    bpy.data.images.remove(img)


def contact_sheet(paths, out_path, cols=3, tile=(640, 360)):
    """Every turntable, scaled down, on one picture."""
    tiles = []
    for p in paths:
        img = bpy.data.images.load(p, check_existing=False)
        img.scale(*tile)
        tiles.append(list(img.pixels))
        bpy.data.images.remove(img)
    w, h = tile
    rows = (len(tiles) + cols - 1) // cols
    W, H = w * cols, h * rows
    out = [0.0] * (W * H * 4)
    for i in range(3, len(out), 4):
        out[i] = 1.0
    for i, px in enumerate(tiles):
        cx, cy = i % cols, rows - 1 - i // cols
        for y in range(h):
            src = y * w * 4
            dst = ((cy * h + y) * W + cx * w) * 4
            out[dst:dst + w * 4] = px[src:src + w * 4]
    img = bpy.data.images.new("contact", W, H, alpha=True)
    img.pixels.foreach_set(out)
    img.filepath_raw = out_path
    img.file_format = "PNG"
    img.save()
    bpy.data.images.remove(img)


def export_far(parts, path, ratio=0.3):
    """A simplified copy for fighters far away (each piece decimated to `ratio` of its
    triangles, the moving parts kept separate), exported beside the full model."""
    copies = []
    for ob in parts:
        c = ob.copy()
        c.data = ob.data.copy()
        c.name = ob.name
        bpy.context.scene.collection.objects.link(c)
        mod = c.modifiers.new("far", "DECIMATE")
        mod.decimate_type = "COLLAPSE"
        mod.ratio = ratio
        mod.use_collapse_triangulate = True
        copies.append(c)
    # the copies take the original names in the file (the game finds `pump` and `cylinder` by name)
    for ob in parts:
        ob.name = ob.name + "_full"
    for c in copies:
        c.name = c.name.split(".")[0]
    export_glb(copies, path)
    depsgraph = bpy.context.evaluated_depsgraph_get()
    tris = 0
    for c in copies:
        me = bpy.data.meshes.new_from_object(c.evaluated_get(depsgraph), depsgraph=depsgraph)
        tris += sum(len(p.vertices) - 2 for p in me.polygons)
        bpy.data.meshes.remove(me)
    for c in copies:
        bpy.data.objects.remove(c, do_unlink=True)
    for ob in parts:
        ob.name = ob.name[: -len("_full")]
    return tris