//! The Ashen Vault's bosses as models (made in Blender by `tools/blender/build_bosses.py`,
//! exported as glTF into `assets/bosses`): the Warden of Wax, the Twin Assessors (Tally and Seal),
//! the Chained Ledger, the Ashen Magistrate, the Ashblood Regent's regalia, and two props (a chain
//! link, a floor anchor).
//!
//! Each model is in pieces, one per joint (a pelvis, a forearm, a halberd, a page wing...), each
//! piece's corners in the model's own frame (x right, y up, z forward, metres, the origin on the
//! floor under it) and its pivot where it turns. The renderer poses them (`Pose`): every piece
//! turns about its pivot and follows its parent, as a rig does. Markers (`fx_<piece>_<kind>_<n>`
//! in the file) say where flames, eyes, a mouth or a chain attach to a piece.
//!
//! Each boss has its own baked texture (1024 square, one layer of the fighters' texture array,
//! after the weapons'); the regalia and the props share one layer (a quarter each).

use crate::math::*;
use crate::weapons::{MAT_WEAPON, WVert};
use std::sync::OnceLock;

/// The models, in file order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Warden = 0,
    Tally,
    Seal,
    Ledger,
    Magistrate,
    Regalia,
    Props,
}

#[cfg_attr(not(test), allow(dead_code))]
pub const ALL: [Kind; 7] = [Kind::Warden, Kind::Tally, Kind::Seal, Kind::Ledger, Kind::Magistrate, Kind::Regalia, Kind::Props];

const FILES: [(&str, &[u8], &[u8]); 7] = [
    ("warden", include_bytes!("../assets/bosses/warden.glb"), include_bytes!("../assets/bosses/textures/warden.png")),
    ("tally", include_bytes!("../assets/bosses/tally.glb"), include_bytes!("../assets/bosses/textures/tally.png")),
    ("seal", include_bytes!("../assets/bosses/seal.glb"), include_bytes!("../assets/bosses/textures/seal.png")),
    ("ledger", include_bytes!("../assets/bosses/ledger.glb"), include_bytes!("../assets/bosses/textures/ledger.png")),
    ("magistrate", include_bytes!("../assets/bosses/magistrate.glb"), include_bytes!("../assets/bosses/textures/magistrate.png")),
    ("regalia", include_bytes!("../assets/bosses/regalia.glb"), include_bytes!("../assets/bosses/textures/regalia.png")),
    ("props", include_bytes!("../assets/bosses/props.glb"), include_bytes!("../assets/bosses/textures/props.png")),
];

/// The size of one texture layer.
pub const LAYER: u32 = 1024;

/// One piece of a model.
#[derive(Clone, Debug)]
pub struct Piece {
    pub name: String,
    /// Where it turns.
    pub pivot: V3,
    pub tris: Vec<WVert>,
}

/// A point something attaches to on a piece (a flame, an eye, a mouth...).
#[derive(Clone, Debug)]
pub struct Marker {
    pub piece: usize,
    pub kind: String,
    pub n: u32,
    pub p: V3,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub pieces: Vec<Piece>,
    pub marks: Vec<Marker>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub min: V3,
    #[cfg_attr(not(test), allow(dead_code))]
    pub max: V3,
}

impl Model {
    pub fn piece(&self, name: &str) -> Option<usize> {
        self.pieces.iter().position(|p| p.name == name)
    }
    pub fn marks<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Marker> + 'a {
        self.marks.iter().filter(move |m| m.kind == kind)
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn triangles(&self) -> usize {
        self.pieces.iter().map(|p| p.tris.len() / 3).sum()
    }
}

/// The layer and the corner (and share) of the texture of model `i`.
fn slot(i: usize) -> (f32, [f32; 2], f32) {
    let base = first_layer();
    match i {
        0..=4 => ((base + i) as f32, [0.0, 0.0], 1.0),
        5 => ((base + 5) as f32, [0.0, 0.0], 0.5),
        _ => ((base + 5) as f32, [0.5, 0.0], 0.5),
    }
}

/// The first texture layer after the weapons': where the bosses' textures start.
pub fn first_layer() -> usize {
    crate::weapons::first_layer() + crate::weapons::layer_count()
}

/// The bosses' texture layers (RGBA, `LAYER` square, each with its mip levels).
pub fn texture_layers() -> Vec<Vec<Vec<u8>>> {
    use image::imageops::{FilterType, overlay, resize};
    let load = |i: usize, size: u32| {
        let t = image::load_from_memory_with_format(FILES[i].2, image::ImageFormat::Png).map(|t| t.to_rgba8()).unwrap_or_else(|_| image::RgbaImage::from_pixel(size, size, image::Rgba([150, 150, 150, 255])));
        if t.width() != size || t.height() != size { resize(&t, size, size, FilterType::Triangle) } else { t }
    };
    let mut out = Vec::new();
    for l in 0..6 {
        let img = if l < 5 {
            load(l, LAYER)
        } else {
            let mut img = image::RgbaImage::from_pixel(LAYER, LAYER, image::Rgba([128, 128, 128, 255]));
            overlay(&mut img, &load(5, LAYER / 2), 0, 0);
            overlay(&mut img, &load(6, LAYER / 2), (LAYER / 2) as i64, 0);
            img
        };
        let mut img = img;
        let mut levels = Vec::new();
        loop {
            levels.push(img.as_raw().clone());
            let s = img.width();
            if s <= 1 {
                break;
            }
            img = resize(&img, s / 2, s / 2, FilterType::Triangle);
        }
        out.push(levels);
    }
    out
}

/// A model (loaded once, on first use); None if its file couldn't be read.
pub fn model(k: Kind) -> Option<&'static Model> {
    static MODELS: [OnceLock<Option<Model>>; 7] = [const { OnceLock::new() }; 7];
    let i = k as usize;
    MODELS[i]
        .get_or_init(|| match load(FILES[i].1, i) {
            Ok(m) => Some(m),
            Err(e) => {
                eprintln!("boss model {}: {e}", FILES[i].0);
                None
            }
        })
        .as_ref()
}

/// Glowing materials: drawn unlit in their own colour.
fn glow(name: &str) -> Option<[f32; 4]> {
    let c = if name.starts_with("glow_ember") {
        [1.0, 0.5, 0.12]
    } else if name.starts_with("glow_violet") {
        [0.72, 0.38, 1.0]
    } else if name.starts_with("glow_quill") {
        [0.55, 0.92, 1.0]
    } else if name.starts_with("glow_glass") {
        [0.85, 0.97, 1.0]
    } else if name.starts_with("glow_blood") {
        [1.0, 0.1, 0.14]
    } else {
        return None;
    };
    Some([c[0], c[1], c[2], 1.0])
}

fn load(bytes: &[u8], i: usize) -> Result<Model, String> {
    let (layer, corner, share) = slot(i);
    let g = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
    let blob = g.blob.clone().ok_or("no binary chunk")?;
    let get = |_b: gltf::Buffer| Some(blob.as_slice());
    let to_game = |v: [f32; 3]| v3(v[0], v[1], -v[2]);
    let mut pieces: Vec<Piece> = Vec::new();
    let mut marks: Vec<(String, String, u32, V3)> = Vec::new();
    let (mut lo, mut hi) = (v3(1e9, 1e9, 1e9), v3(-1e9, -1e9, -1e9));
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
        let name = node.name().unwrap_or("").to_string();
        let normal_m = glam::Mat3::from_mat4(world).inverse().transpose();
        let marker = name.strip_prefix("fx_").map(|s| s.to_string());
        let mut tris = Vec::new();
        let (mut sum, mut cnt) = (V3::ZERO, 0.0f32);
        for prim in mesh.primitives() {
            let r = prim.reader(get);
            let Some(pos) = r.read_positions() else { continue };
            let pos: Vec<[f32; 3]> = pos.collect();
            if marker.is_some() {
                for p in &pos {
                    sum += to_game(world.transform_point3(glam::Vec3::from(*p)).to_array());
                    cnt += 1.0;
                }
                continue;
            }
            let nrm: Vec<[f32; 3]> = r.read_normals().map(|n| n.collect()).unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
            let uvs: Vec<[f32; 2]> = r.read_tex_coords(0).map(|t| t.into_f32().collect()).unwrap_or_else(|| vec![[0.5, 0.5]; pos.len()]);
            let idx: Vec<u32> = match r.read_indices() {
                Some(i) => i.into_u32().collect(),
                None => (0..pos.len() as u32).collect(),
            };
            let mat = prim.material();
            let pbr = mat.pbr_metallic_roughness();
            let mname = mat.name().unwrap_or("");
            let (col, ex) = match glow(mname) {
                Some(c) => (c, [-1.0, 2.0, 1.0, 0.0]),
                None => {
                    let rough = pbr.roughness_factor();
                    let shine = (1.0 - rough) / 0.3;
                    let shine = if pbr.metallic_factor() > 0.5 { shine.clamp(1.5, 2.4) } else { shine.min(1.2) };
                    ([1.0, 1.0, 1.0, 1.0], [layer, MAT_WEAPON, 1.0, shine])
                }
            };
            for &k in &idx {
                let k = k as usize;
                if k >= pos.len() {
                    return Err("index out of range".into());
                }
                let p = to_game(world.transform_point3(glam::Vec3::from(pos[k])).to_array());
                let n = (normal_m * glam::Vec3::from(nrm[k])).normalize_or_zero();
                lo = v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
                hi = v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
                let t = uvs.get(k).copied().unwrap_or([0.5, 0.5]);
                let uv = [corner[0] + t[0].clamp(0.001, 0.999) * share, corner[1] + t[1].clamp(0.001, 0.999) * share];
                tris.push(WVert { p, n: to_game(n.to_array()), uv, col, ex });
            }
        }
        if let Some(m) = marker {
            // <piece>_<kind>_<n>, the piece's name may hold underscores itself
            let mut it = m.rsplitn(3, '_');
            let n = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let kind = it.next().unwrap_or("").to_string();
            let piece = it.next().unwrap_or("").to_string();
            if cnt > 0.0 {
                marks.push((piece, kind, n, sum * (1.0 / cnt)));
            }
            continue;
        }
        let (_, _, t) = world.to_scale_rotation_translation();
        if !tris.is_empty() {
            pieces.push(Piece { name, pivot: to_game([t.x, t.y, t.z]), tris });
        }
    }
    if pieces.is_empty() {
        return Err("no triangles".into());
    }
    let marks = marks.into_iter().filter_map(|(piece, kind, n, p)| pieces.iter().position(|q| q.name == piece).map(|k| Marker { piece: k, kind, n, p })).collect();
    Ok(Model { pieces, marks, min: lo, max: hi })
}

// ---------------------------------------------------------------- posing

/// A rigid move: turn by `m`, then shift by `t`.
#[derive(Clone, Copy, Debug)]
pub struct Xf {
    pub m: M3,
    pub t: V3,
}

impl Xf {
    pub const I: Xf = Xf { m: M3::I, t: V3::ZERO };
    pub fn apply(&self, p: V3) -> V3 {
        self.m.apply(p) + self.t
    }
}

/// How each piece is turned (about its pivot) and shifted, relative to its parent.
#[derive(Clone, Debug)]
pub struct Pose {
    pub local: Vec<(M3, V3)>,
}

impl Pose {
    pub fn rest(m: &Model) -> Pose {
        Pose { local: vec![(M3::I, V3::ZERO); m.pieces.len()] }
    }
    /// Turn a piece (by name) by `r` on top of what it has.
    pub fn turn(&mut self, m: &Model, name: &str, r: M3) {
        if let Some(i) = m.piece(name) {
            self.local[i].0 = self.local[i].0.mul(&r);
        }
    }
    pub fn shift(&mut self, m: &Model, name: &str, d: V3) {
        if let Some(i) = m.piece(name) {
            self.local[i].1 += d;
        }
    }
    /// Each piece's move in the model frame, following its parents (`parent` names each
    /// piece's parent; the rest hang from the model itself).
    pub fn solve(&self, m: &Model, parent: fn(&str) -> Option<&'static str>) -> Vec<Xf> {
        let n = m.pieces.len();
        let mut out: Vec<Option<Xf>> = vec![None; n];
        fn go(i: usize, m: &Model, pose: &Pose, parent: fn(&str) -> Option<&'static str>, out: &mut Vec<Option<Xf>>, depth: usize) -> Xf {
            if let Some(x) = out[i] {
                return x;
            }
            let up = if depth < 16 { parent(&m.pieces[i].name).and_then(|p| m.piece(p)) } else { None };
            let px = match up {
                Some(j) if j != i => go(j, m, pose, parent, out, depth + 1),
                _ => Xf::I,
            };
            let (r, d) = pose.local[i];
            let pv = m.pieces[i].pivot;
            let mm = px.m.mul(&r);
            // turn about the pivot, shift, then follow the parent
            let t = px.m.apply(pv + d - r.apply(pv)) + px.t;
            let x = Xf { m: mm, t };
            out[i] = Some(x);
            x
        }
        for i in 0..n {
            go(i, m, self, parent, &mut out, 0);
        }
        out.into_iter().map(|x| x.unwrap_or(Xf::I)).collect()
    }
}

/// Rotation helpers in the model's frame (x right, y up, z forward), each positive way said:
/// `lean(a)` tips the top forward, `raise(a)` swings a hanging limb forward and up, `roll(a)`
/// lifts the right side, `turn(a)` turns the front to the right.
pub fn lean(a: f32) -> M3 {
    M3::rot_x(a)
}
pub fn raise(a: f32) -> M3 {
    M3::rot_x(-a)
}
pub fn roll(a: f32) -> M3 {
    M3::rot_z(a)
}
pub fn turn(a: f32) -> M3 {
    M3::rot_y(a)
}

/// Each boss's rig: a piece's parent.
pub fn warden_parent(p: &str) -> Option<&'static str> {
    Some(match p {
        "torso" => "pelvis",
        "head" | "upper_l" | "upper_r" => "torso",
        "fore_l" => "upper_l",
        "fore_r" => "upper_r",
        "halberd" => "fore_r",
        "thigh_l" | "thigh_r" => "pelvis",
        "shin_l" => "thigh_l",
        "shin_r" => "thigh_r",
        _ => return None,
    })
}

pub fn robed_parent(p: &str) -> Option<&'static str> {
    Some(match p {
        "torso" => "robe",
        "head" | "upper_l" | "upper_r" => "torso",
        "fore_l" => "upper_l",
        "fore_r" => "upper_r",
        "scales" | "ledger" => "fore_l",
        "gavel" => "fore_r",
        _ => return None,
    })
}

pub fn book_parent(p: &str) -> Option<&'static str> {
    match p {
        "upper" | "wing_l" | "wing_r" | "quill" => Some("lower"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every boss loads with its pieces, pivots, markers and texture; the rig moves pieces
    /// about their pivots and they follow their parents.
    #[test]
    fn boss_models_load_and_pose() {
        for k in ALL {
            let m = model(k).unwrap_or_else(|| panic!("{k:?} loads"));
            println!("{k:?}: {} triangles, {} pieces, {} markers, {:?}..{:?}", m.triangles(), m.pieces.len(), m.marks.len(), m.min, m.max);
            assert!(m.triangles() > 300 && m.triangles() < 40000, "{k:?}: {} triangles", m.triangles());
            let (layer, c, s) = slot(k as usize);
            for v in m.pieces.iter().flat_map(|p| p.tris.iter()).filter(|v| v.ex[1] == MAT_WEAPON) {
                assert_eq!(v.ex[0], layer);
                assert!(v.uv[0] >= c[0] && v.uv[0] <= c[0] + s && v.uv[1] >= c[1] && v.uv[1] <= c[1] + s);
            }
        }
        let w = model(Kind::Warden).unwrap();
        for p in ["pelvis", "torso", "head", "upper_l", "fore_l", "upper_r", "fore_r", "halberd", "thigh_l", "shin_l", "thigh_r", "shin_r", "pool"] {
            assert!(w.piece(p).is_some(), "the Warden has its {p}");
        }
        // it towers: a helm near 4.5 m, a crown of flames above, ember eyes in the visor, forward
        assert!(w.max.y > 5.0 && w.max.y < 6.2, "the Warden's height {:.2}", w.max.y);
        assert_eq!(w.marks("flame").filter(|f| w.pieces[f.piece].name == "head").count(), 9);
        let eyes: Vec<&Marker> = w.marks("eye").collect();
        assert_eq!(eyes.len(), 2);
        assert!(eyes.iter().all(|e| e.p.z > 0.3 && e.p.y > 3.8), "eyes in the visor, in front");
        assert!(eyes[0].p.x * eyes[1].p.x < 0.0, "one each side");
        // right is +x: the halberd is in the right hand
        let hb = &w.pieces[w.piece("halberd").unwrap()];
        assert!(hb.pivot.x > 1.0, "the halberd is held on the right ({:?})", hb.pivot);
        for (k, parts) in [(Kind::Tally, &["robe", "torso", "head", "fore_l", "scales"][..]), (Kind::Seal, &["robe", "head", "fore_r", "ledger"]), (Kind::Ledger, &["lower", "upper", "wing_l", "wing_r", "quill"]), (Kind::Magistrate, &["robe", "torso", "head", "fore_r", "gavel"]), (Kind::Regalia, &["crown", "mantle"]), (Kind::Props, &["link", "anchor"])] {
            let m = model(k).unwrap();
            for p in parts {
                assert!(m.piece(p).is_some(), "{k:?} has its {p}");
            }
        }
        assert!(model(Kind::Magistrate).unwrap().max.y > 3.4, "the Magistrate is huge");
        assert!(model(Kind::Ledger).unwrap().marks("mouth").count() == 1);
        // posing: the forearm follows a raised upper arm; a turned piece keeps its pivot
        let mut pose = Pose::rest(w);
        let rest = pose.solve(w, warden_parent);
        let fi = w.piece("fore_r").unwrap();
        let el = w.pieces[fi].pivot;
        assert!((rest[fi].apply(el) - el).len() < 1e-4);
        pose.turn(w, "upper_r", raise(1.5));
        let xs = pose.solve(w, warden_parent);
        let ui = w.piece("upper_r").unwrap();
        let sh = w.pieces[ui].pivot;
        assert!((xs[ui].apply(sh) - sh).len() < 1e-4, "the shoulder stays put");
        let moved = xs[fi].apply(el);
        assert!(moved.z > el.z + 0.4 && moved.y > el.y + 0.4, "the elbow swings forward and up: {el:?} -> {moved:?}");
        let hi = w.piece("halberd").unwrap();
        assert!((xs[hi].apply(w.pieces[hi].pivot) - rest[hi].apply(w.pieces[hi].pivot)).len() > 0.5, "the halberd goes with the hand");
        // the texture layers: five of their own, the regalia and props sharing the sixth
        let layers = texture_layers();
        assert_eq!(layers.len(), 6);
        assert!(layers.iter().all(|l| l[0].len() == (LAYER * LAYER * 4) as usize));
    }
}
