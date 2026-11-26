//! The weapon models (made in Blender by `tools/blender/build_weapons.py`, exported as glTF into
//! `assets/weapons`) and their icons.
//!
//! Each model is in the weapon's own frame: x to its right, y up, z forward (barrel or blade), in
//! metres, the origin where the hand holds it (a gun's grip, the middle of a blade's handle). The
//! renderer places that frame on the hand. Moving parts are their own pieces: the scattergun's
//! `pump` slides back along z, the revolver's `cylinder` turns about its own axis (z, through its
//! pivot).
//!
//! Each weapon has a baked, hand-painted-style texture (`assets/weapons/textures`, 512 x 512). Four
//! share one layer of the fighters' texture array (one per quarter), after the fighters' own
//! layers; the shader colours a weapon's corners from it.

use crate::game::Weapon;
use crate::math::*;
use std::sync::OnceLock;

/// How far the scattergun's pump slides back when it's racked (metres).
pub const PUMP_TRAVEL: f32 = 0.07;
/// Where the front hand holds the pump at rest (metres ahead of the grip).
pub const PUMP_HOLD: f32 = 0.33;

/// What a piece of a model does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind {
    Body,
    Pump,
    Cylinder,
}

/// One corner of a triangle, ready for the renderer: position, normal, texture position (in the
/// shared layer), colour (linear) and the material word (texture layer, 1 for a weapon, occlusion,
/// shine).
#[derive(Clone, Copy, Debug)]
pub struct WVert {
    pub p: V3,
    pub n: V3,
    pub uv: [f32; 2],
    pub col: [f32; 4],
    pub ex: [f32; 4],
}

/// Each weapon's texture size, and the size of a shared layer (four textures, one per quarter).
pub const TEX: u32 = 512;
pub const LAYER: u32 = 1024;
/// The material word's second value for a textured weapon (the shader's own branch).
pub const MAT_WEAPON: f32 = 1.0;
/// A blade's cutting edge, drawn unlit in this pale steel.
const EDGE_GLINT: [f32; 4] = [0.78, 0.82, 0.88, 1.0];

#[derive(Clone, Debug)]
pub struct Part {
    pub kind: PartKind,
    /// Where it turns (the cylinder's axis passes through it, along z).
    pub pivot: V3,
    /// Triangles, three corners each.
    pub tris: Vec<WVert>,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub parts: Vec<Part>,
    /// The model's extent in its own frame (the tests check the near end).
    #[cfg_attr(not(test), allow(dead_code))]
    pub min: V3,
    pub max: V3,
}

impl Model {
    #[cfg(test)]
    pub fn triangles(&self) -> usize {
        self.parts.iter().map(|p| p.tris.len() / 3).sum()
    }
}

/// Each weapon's files: its name, the model, the simplified copy for far away, the icon, the texture.
/// A weapon's icon (PNG) by its file name ("katana", "ashblood_glaive"...): crate::achievements
/// draws its icons from them.
pub fn icon_png(name: &str) -> Option<&'static [u8]> {
    FILES.iter().find(|f| f.0 == name).map(|f| f.3)
}

const FILES: [(&str, &[u8], &[u8], &[u8], &[u8]); 14] = [
    ("dagger", include_bytes!("../assets/weapons/dagger.glb"), include_bytes!("../assets/weapons/dagger_far.glb"), include_bytes!("../assets/weapons/icons/dagger.png"), include_bytes!("../assets/weapons/textures/dagger.png")),
    ("katana", include_bytes!("../assets/weapons/katana.glb"), include_bytes!("../assets/weapons/katana_far.glb"), include_bytes!("../assets/weapons/icons/katana.png"), include_bytes!("../assets/weapons/textures/katana.png")),
    ("greatsword", include_bytes!("../assets/weapons/greatsword.glb"), include_bytes!("../assets/weapons/greatsword_far.glb"), include_bytes!("../assets/weapons/icons/greatsword.png"), include_bytes!("../assets/weapons/textures/greatsword.png")),
    ("scattergun", include_bytes!("../assets/weapons/scattergun.glb"), include_bytes!("../assets/weapons/scattergun_far.glb"), include_bytes!("../assets/weapons/icons/scattergun.png"), include_bytes!("../assets/weapons/textures/scattergun.png")),
    ("revolver", include_bytes!("../assets/weapons/revolver.glb"), include_bytes!("../assets/weapons/revolver_far.glb"), include_bytes!("../assets/weapons/icons/revolver.png"), include_bytes!("../assets/weapons/textures/revolver.png")),
    ("smg", include_bytes!("../assets/weapons/smg.glb"), include_bytes!("../assets/weapons/smg_far.glb"), include_bytes!("../assets/weapons/icons/smg.png"), include_bytes!("../assets/weapons/textures/smg.png")),
    ("rifle", include_bytes!("../assets/weapons/rifle.glb"), include_bytes!("../assets/weapons/rifle_far.glb"), include_bytes!("../assets/weapons/icons/rifle.png"), include_bytes!("../assets/weapons/textures/rifle.png")),
    ("hand_cannon", include_bytes!("../assets/weapons/hand_cannon.glb"), include_bytes!("../assets/weapons/hand_cannon_far.glb"), include_bytes!("../assets/weapons/icons/hand_cannon.png"), include_bytes!("../assets/weapons/textures/hand_cannon.png")),
    ("sniper", include_bytes!("../assets/weapons/sniper.glb"), include_bytes!("../assets/weapons/sniper_far.glb"), include_bytes!("../assets/weapons/icons/sniper.png"), include_bytes!("../assets/weapons/textures/sniper.png")),
    ("rocket_launcher", include_bytes!("../assets/weapons/rocket_launcher.glb"), include_bytes!("../assets/weapons/rocket_launcher_far.glb"), include_bytes!("../assets/weapons/icons/rocket_launcher.png"), include_bytes!("../assets/weapons/textures/rocket_launcher.png")),
    ("shuriken", include_bytes!("../assets/weapons/shuriken.glb"), include_bytes!("../assets/weapons/shuriken_far.glb"), include_bytes!("../assets/weapons/icons/shuriken.png"), include_bytes!("../assets/weapons/textures/shuriken.png")),
    // Ledger Duels: the Bridgewright's Rig's own weapons (held in place of the kind they fight as)
    ("rigger_hook", include_bytes!("../assets/weapons/rigger_hook.glb"), include_bytes!("../assets/weapons/rigger_hook_far.glb"), include_bytes!("../assets/weapons/icons/rigger_hook.png"), include_bytes!("../assets/weapons/textures/rigger_hook.png")),
    ("rigger_snub", include_bytes!("../assets/weapons/rigger_snub.glb"), include_bytes!("../assets/weapons/rigger_snub_far.glb"), include_bytes!("../assets/weapons/icons/rigger_snub.png"), include_bytes!("../assets/weapons/textures/rigger_snub.png")),
    // the Ashen Vault's Ashblood Glaive (a wardrobe skin for the main hand; fights as a greatsword)
    ("ashblood_glaive", include_bytes!("../assets/weapons/ashblood_glaive.glb"), include_bytes!("../assets/weapons/ashblood_glaive_far.glb"), include_bytes!("../assets/weapons/icons/ashblood_glaive.png"), include_bytes!("../assets/weapons/textures/ashblood_glaive.png")),
];

/// The held id of the Ashblood Glaive's model (the wardrobe's look 94).
pub const GLAIVE: u8 = 59;

/// A Ledger Duels item with its own model (the Rigger's Hook and Snub): its file.
fn item_file(id: u8) -> Option<usize> {
    match id {
        crate::gear::HOOK => Some(11),
        crate::gear::SNUB => Some(12),
        GLAIVE => Some(13),
        _ => None,
    }
}

/// An item's own model (near or far), if it has one.
pub fn item_model(id: u8, far: bool) -> Option<&'static Model> {
    static NEAR: [OnceLock<Option<Model>>; 3] = [const { OnceLock::new() }; 3];
    static FAR_M: [OnceLock<Option<Model>>; 3] = [const { OnceLock::new() }; 3];
    let i = item_file(id)?;
    if far {
        load_once(&FAR_M[i - 11], i, FILES[i].2).or_else(|| load_once(&NEAR[i - 11], i, FILES[i].1))
    } else {
        load_once(&NEAR[i - 11], i, FILES[i].1)
    }
}

/// An item's icon (grey shades), if it has its own model.
pub fn item_icon(id: u8) -> Option<(Vec<u8>, [usize; 2])> {
    let i = item_file(id)?;
    let img = image::load_from_memory_with_format(FILES[i].3, image::ImageFormat::Png).ok()?.to_rgba8();
    let (iw, ih) = img.dimensions();
    Some((img.into_raw(), [iw as usize, ih as usize]))
}

/// Which file a weapon's model is.
fn file_index(w: Weapon) -> Option<usize> {
    Some(match w {
        Weapon::Dagger => 0,
        Weapon::Katana => 1,
        Weapon::Greatsword => 2,
        Weapon::Shotgun => 3,
        Weapon::Revolver => 4,
        Weapon::Smg => 5,
        Weapon::Rifle => 6,
        Weapon::HandCannon => 7,
        Weapon::Sniper => 8,
        Weapon::Rocket => 9,
        Weapon::Shuriken => 10,
    })
}

/// Beyond this distance from the camera a weapon is drawn from its simplified copy.
pub const FAR: f32 = 9.0;

/// A weapon's model (loaded once, on first use); None if its file couldn't be read.
pub fn model(w: Weapon) -> Option<&'static Model> {
    static MODELS: [OnceLock<Option<Model>>; 11] = [const { OnceLock::new() }; 11];
    let i = file_index(w)?;
    load_once(&MODELS[i], i, FILES[i].1)
}

/// The first texture-array layer after the fighters' own: where the weapons' textures start.
pub fn first_layer() -> usize {
    crate::chars::TEXTURES.len()
}

/// How many texture layers the weapons take.
pub fn layer_count() -> usize {
    FILES.len().div_ceil(4)
}

/// Weapon `i`'s texture: its layer and the corner of its quarter.
fn slot(i: usize) -> (f32, [f32; 2]) {
    let q = i % 4;
    ((first_layer() + i / 4) as f32, [(q % 2) as f32 * 0.5, (q / 2) as f32 * 0.5])
}

/// The weapons' texture layers (RGBA, `LAYER` square, each with its mip levels), to add after the
/// fighters' layers.
pub fn texture_layers() -> Vec<Vec<Vec<u8>>> {
    use image::imageops::{FilterType, overlay, resize};
    let n = FILES.len().div_ceil(4);
    (0..n)
        .map(|l| {
            let mut img = image::RgbaImage::from_pixel(LAYER, LAYER, image::Rgba([128, 128, 128, 255]));
            for q in 0..4 {
                let i = l * 4 + q;
                if i >= FILES.len() {
                    break;
                }
                let tex = image::load_from_memory_with_format(FILES[i].4, image::ImageFormat::Png).map(|t| t.to_rgba8()).unwrap_or_else(|_| image::RgbaImage::from_pixel(TEX, TEX, image::Rgba([150, 150, 150, 255])));
                let tex = if tex.width() != TEX { resize(&tex, TEX, TEX, FilterType::Triangle) } else { tex };
                overlay(&mut img, &tex, ((q % 2) as u32 * TEX) as i64, ((q / 2) as u32 * TEX) as i64);
            }
            let mut levels = Vec::new();
            loop {
                levels.push(img.as_raw().clone());
                let s = img.width();
                if s <= 1 {
                    break;
                }
                img = resize(&img, s / 2, s / 2, FilterType::Triangle);
            }
            levels
        })
        .collect()
}

/// Its simplified copy for fighters far away (the full model if that's missing).
pub fn model_far(w: Weapon) -> Option<&'static Model> {
    static MODELS: [OnceLock<Option<Model>>; 11] = [const { OnceLock::new() }; 11];
    let i = file_index(w)?;
    load_once(&MODELS[i], i, FILES[i].2).or_else(|| model(w))
}

fn load_once(cell: &'static OnceLock<Option<Model>>, i: usize, bytes: &'static [u8]) -> Option<&'static Model> {
    cell
        .get_or_init(|| match load(bytes, i) {
            Ok(m) => Some(m),
            Err(e) => {
                eprintln!("weapon model {}: {e}", FILES[i].0);
                None
            }
        })
        .as_ref()
}

/// A weapon's icon: RGBA pixels (grey shades, the game tints them) and its size.
pub fn icon_pixels(w: Weapon) -> Option<(Vec<u8>, [usize; 2])> {
    let i = file_index(w)?;
    let img = image::load_from_memory_with_format(FILES[i].3, image::ImageFormat::Png).ok()?.to_rgba8();
    let (iw, ih) = img.dimensions();
    Some((img.into_raw(), [iw as usize, ih as usize]))
}

/// The material word for a glTF material on a weapon textured from `layer`: shiny metal reads as
/// metal in the game's shader (shine above 1.4), rough materials stay matte, lenses glow (drawn
/// unlit, untextured).
fn material_word(name: &str, metallic: f32, rough: f32, layer: f32) -> [f32; 4] {
    if name.starts_with("glass") {
        return [-1.0, 2.0, 1.0, 0.0];
    }
    // a blade's cutting edge: a bright steel line whatever the light, so blades read from afar
    if name.starts_with("steel_edge") {
        return [-1.0, 2.0, 1.0, 0.0];
    }
    let shine = (1.0 - rough) / 0.3;
    let shine = if metallic > 0.5 { shine.clamp(1.5, 2.6) } else { shine.min(1.35) };
    [layer, MAT_WEAPON, 1.0, shine]
}

/// Read weapon `i`'s model: every mesh node's triangles in the weapon frame (glTF's -z is our
/// forward), its texture positions moved into its quarter of the shared layer.
fn load(bytes: &[u8], i: usize) -> Result<Model, String> {
    let (layer, corner) = slot(i);
    let g = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
    let blob = g.blob.clone().ok_or("no binary chunk")?;
    let get = |_b: gltf::Buffer| Some(blob.as_slice());
    let to_game = |v: [f32; 3]| v3(v[0], v[1], -v[2]);
    let mut parts = Vec::new();
    let (mut lo, mut hi) = (v3(1e9, 1e9, 1e9), v3(-1e9, -1e9, -1e9));
    // nodes with their world matrices (parents first)
    let mut stack: Vec<(gltf::Node, glam::Mat4)> = Vec::new();
    for scene in g.document.scenes() {
        for n in scene.nodes() {
            stack.push((n, glam::Mat4::IDENTITY));
        }
    }
    while let Some((node, parent)) = stack.pop() {
        let local = glam::Mat4::from_cols_array_2d(&node.transform().matrix());
        let world = parent * local;
        for c in node.children() {
            stack.push((c, world));
        }
        let Some(mesh) = node.mesh() else { continue };
        let name = node.name().unwrap_or("").to_lowercase();
        let kind = if name.starts_with("pump") {
            PartKind::Pump
        } else if name.starts_with("cylinder") {
            PartKind::Cylinder
        } else {
            PartKind::Body
        };
        let (_, _, t) = world.to_scale_rotation_translation();
        let pivot = to_game([t.x, t.y, t.z]);
        let normal_m = glam::Mat3::from_mat4(world).inverse().transpose();
        let mut tris = Vec::new();
        for prim in mesh.primitives() {
            let r = prim.reader(get);
            let Some(pos) = r.read_positions() else { continue };
            let pos: Vec<[f32; 3]> = pos.collect();
            let nrm: Vec<[f32; 3]> = r.read_normals().map(|n| n.collect()).unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
            let uvs: Vec<[f32; 2]> = r.read_tex_coords(0).map(|t| t.into_f32().collect()).unwrap_or_else(|| vec![[0.5, 0.5]; pos.len()]);
            let idx: Vec<u32> = match r.read_indices() {
                Some(i) => i.into_u32().collect(),
                None => (0..pos.len() as u32).collect(),
            };
            let mat = prim.material();
            let pbr = mat.pbr_metallic_roughness();
            let mname = mat.name().unwrap_or("");
            // (a lens is drawn unlit: a bright glint of its colour)
            // (the texture carries the colour, so the corners are white; lenses keep a glint colour)
            let col = if mname.starts_with("glass") {
                [0.35, 0.75, 1.0, 1.0]
            } else if mname.starts_with("steel_edge") {
                EDGE_GLINT
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };
            let ex = material_word(mname, pbr.metallic_factor(), pbr.roughness_factor(), layer);
            for &k in &idx {
                let k = k as usize;
                if k >= pos.len() {
                    return Err("index out of range".into());
                }
                let p = world.transform_point3(glam::Vec3::from(pos[k]));
                let n = (normal_m * glam::Vec3::from(nrm[k])).normalize_or_zero();
                let p = to_game(p.to_array());
                lo = v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
                hi = v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
                // its quarter, a hair inside it so filtering never reaches the neighbour
                let t = uvs.get(k).copied().unwrap_or([0.5, 0.5]);
                let uv = [corner[0] + t[0].clamp(0.002, 0.998) * 0.5, corner[1] + t[1].clamp(0.002, 0.998) * 0.5];
                tris.push(WVert { p, n: to_game(n.to_array()), uv, col, ex });
            }
        }
        if !tris.is_empty() {
            parts.push(Part { kind, pivot, tris });
        }
    }
    if parts.is_empty() {
        return Err("no triangles".into());
    }
    Ok(Model { parts, min: lo, max: hi })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Weapon; 11] = [
        Weapon::Dagger,
        Weapon::Katana,
        Weapon::Greatsword,
        Weapon::Shotgun,
        Weapon::Revolver,
        Weapon::Smg,
        Weapon::Rifle,
        Weapon::HandCannon,
        Weapon::Sniper,
        Weapon::Rocket,
        Weapon::Shuriken,
    ];

    /// Every weapon loads, at a sensible size and detail, held at its origin, pointing forward;
    /// the moving parts are there; every icon decodes with a clear background.
    #[test]
    fn every_weapon_model_and_icon_loads() {
        for w in ALL {
            let m = model(w).unwrap_or_else(|| panic!("{w:?} loads"));
            let tris = m.triangles();
            let len = m.max.z - m.min.z;
            println!("{w:?}: {tris} triangles, {:.2} m long, from z {:.2} to {:.2}", len, m.min.z, m.max.z);
            assert!((300..=4000).contains(&tris), "{w:?}: {tris} triangles (bevelled detail, but cheap to draw)");
            // textured: every corner reads its own quarter of its layer (lenses excepted)
            let (layer, c) = slot(file_index(w).unwrap());
            for v in m.parts.iter().flat_map(|p| p.tris.iter()).filter(|v| v.ex[1] == MAT_WEAPON) {
                assert_eq!(v.ex[0], layer);
                assert!(v.uv[0] >= c[0] && v.uv[0] <= c[0] + 0.5 && v.uv[1] >= c[1] && v.uv[1] <= c[1] + 0.5, "{w:?}: uv {:?} outside its quarter", v.uv);
                assert!(v.col[0] > 0.9, "{w:?}: the texture carries the colour");
            }
            // the hand's point is on the weapon (the star is pinched just above the fingers)
            for (a, b, c) in [(m.min.x, m.max.x, "x"), (m.min.y, m.max.y, "y"), (m.min.z, m.max.z, "z")].into_iter().filter(|_| w != Weapon::Shuriken) {
                assert!(a <= 0.02 && b >= -0.02, "{w:?}: the origin is off the model along {c} ({a:.3}..{b:.3})");
            }
            // forward is +z: the muzzle or point is the far end
            assert!(m.max.z > -m.min.z * 0.9 || w == Weapon::Shuriken, "{w:?}: points forward ({:.2}..{:.2})", m.min.z, m.max.z);
            let (px, [iw, ih]) = icon_pixels(w).expect("icon");
            assert!(iw > 200 && ih > 60, "{w:?} icon size {iw}x{ih}");
            assert_eq!(px[3], 0, "{w:?}: the icon's corner is clear");
            assert!(px.chunks(4).any(|p| p[3] > 200), "{w:?}: the icon shows the weapon");
        }
        // the far copies are much lighter, with the same moving parts
        for w in ALL {
            let (near, far) = (model(w).unwrap(), model_far(w).unwrap());
            // (blades and stars keep more: they read by their outline)
            let keep = if w.is_melee() || w == Weapon::Shuriken { 0.75 } else { 0.5 };
            assert!((far.triangles() as f32) < near.triangles() as f32 * keep, "{w:?}: far copy {} vs {}", far.triangles(), near.triangles());
            let kinds = |m: &Model| m.parts.iter().map(|p| p.kind as u8).collect::<Vec<_>>();
            assert_eq!(kinds(near).len(), kinds(far).len(), "{w:?}: the same pieces");
            assert!((far.max.z - near.max.z).abs() < 0.03, "{w:?}: the same length");
        }
        // reach matches the game's blades (the swing trail and hit reach assume these)
        let tip = |w| model(w).unwrap().max.z;
        assert!((tip(Weapon::Katana) - 1.28).abs() < 0.04, "katana point {:.2}", tip(Weapon::Katana));
        assert!((tip(Weapon::Greatsword) - 1.72).abs() < 0.05, "greatsword point {:.2}", tip(Weapon::Greatsword));
        assert!(tip(Weapon::Dagger) < 0.6);
        // the scattergun's pump and the revolver's cylinder move on their own
        let pump = model(Weapon::Shotgun).unwrap().parts.iter().find(|p| p.kind == PartKind::Pump).expect("a pump");
        let zs: Vec<f32> = pump.tris.iter().map(|v| v.p.z).collect();
        let (zmin, zmax) = (zs.iter().cloned().fold(f32::MAX, f32::min), zs.iter().cloned().fold(f32::MIN, f32::max));
        // (its action bars reach back towards the receiver; the wooden fore-end is the front 17 cm)
        assert!(PUMP_HOLD > zmax - 0.17 && PUMP_HOLD < zmax, "the front hand holds the fore-end: {zmin:.2}..{zmax:.2}, hold {PUMP_HOLD}");
        let cyl = model(Weapon::Revolver).unwrap().parts.iter().find(|p| p.kind == PartKind::Cylinder).expect("a cylinder");
        assert!((cyl.pivot.z - 0.06).abs() < 0.01 && (cyl.pivot.y - 0.03).abs() < 0.01, "cylinder pivot {:?}", cyl.pivot);
        // its corners are all round the pivot (it spins in place)
        let r: f32 = cyl.tris.iter().map(|v| v3(v.p.x - cyl.pivot.x, v.p.y - cyl.pivot.y, 0.0).len()).fold(0.0, f32::max);
        assert!(r < 0.03, "the cylinder spins about its own axis: radius {r:.3}");
        // shiny metal reads as metal, wood stays matte, lenses glow
        assert!(material_word("steel", 1.0, 0.28, 5.0)[3] > 1.4);
        assert!(material_word("wood", 0.0, 0.62, 5.0)[3] < 1.4);
        assert_eq!(material_word("glass", 0.0, 0.05, 5.0)[1], 2.0);
        assert_eq!(material_word("steel_edge__shown", 1.0, 0.12, 5.0)[1], 2.0, "the cutting edge reads whatever the light");
        let edge = model(Weapon::Katana).unwrap().parts[0].tris.iter().filter(|v| v.ex[1] == 2.0).count();
        assert!(edge > 30, "the katana has a bright edge ({edge} corners)");
        // the texture layers: three, each a full mip chain, every weapon's quarter painted
        let layers = texture_layers();
        assert_eq!(layers.len(), FILES.len().div_ceil(4));
        // the Bridgewright's Rig's own weapons
        for id in [crate::gear::HOOK, crate::gear::SNUB] {
            assert!(item_model(id, false).is_some_and(|m| m.triangles() > 300), "item {id} has its model");
            assert!(item_model(id, true).is_some() && item_icon(id).is_some());
        }
        assert!(layers.iter().all(|l| l[0].len() == (LAYER * LAYER * 4) as usize && l.last().unwrap().len() == 4));
        let px = |l: usize, x: u32, y: u32| { let o = ((y * LAYER + x) * 4) as usize; [layers[l][0][o], layers[l][0][o + 1], layers[l][0][o + 2]] };
        assert_ne!(px(0, 100, 100), [128, 128, 128], "the dagger's quarter is painted");
    }
}
