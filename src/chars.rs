//! The four fighters you can play as, built from CC0 parts by Quaternius ("Modular Character
//! Outfits - Fantasy" and "Universal Base Characters", both rigged to the same skeleton as the
//! animation library). Each fighter is an outfit (body, arms, legs, boots, hood...), a head with
//! face, eyes and eyebrows taken from a base character, and hair. Every part is moved onto the
//! outfit's skeleton when loaded, so one set of bone matrices drives the whole fighter; the
//! animations (made for the library's mannequin) are carried over bone by bone.

use crate::anim::{Rig, SkinVtx, Trs};
use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;

/// One glTF file with its binary buffer.
struct File {
    json: &'static str,
    bin: &'static [u8],
}

macro_rules! file {
    ($n:literal) => {
        File { json: include_str!(concat!("../assets/characters/", $n, ".gltf")), bin: include_bytes!(concat!("../assets/characters/", $n, ".bin")) }
    };
}

/// The fighters' texture set, one layer each (image bytes, all PNG).
pub const TEXTURES: [(&str, &[u8]); 11] = [
    ("ranger", include_bytes!("../assets/characters/ranger.png")),
    ("ranger_alt", include_bytes!("../assets/characters/ranger_alt.png")),
    ("peasant", include_bytes!("../assets/characters/peasant.png")),
    ("peasant_alt", include_bytes!("../assets/characters/peasant_alt.png")),
    ("skin_male", include_bytes!("../assets/characters/skin_male.png")),
    ("skin_female", include_bytes!("../assets/characters/skin_female.png")),
    ("hero_male", include_bytes!("../assets/characters/hero_male.png")),
    ("hero_female", include_bytes!("../assets/characters/hero_female.png")),
    ("hair1", include_bytes!("../assets/characters/hair1.png")),
    ("hair2", include_bytes!("../assets/characters/hair2.png")),
    ("eye", include_bytes!("../assets/characters/eye.png")),
];

fn layer(name: &str) -> u32 {
    TEXTURES.iter().position(|t| t.0 == name).unwrap_or(0) as u32
}

/// Which texture a material uses (a fighter can swap one for a recoloured version).
fn material_layer(mat: &str, swaps: &[(&str, &str)]) -> u32 {
    if let Some((_, l)) = swaps.iter().find(|(m, _)| *m == mat) {
        return layer(l);
    }
    layer(match mat {
        "MI_Ranger" => "ranger",
        "MI_Peasant" => "peasant",
        "MI_Regular_Male" => "skin_male",
        "MI_Regular_Female" => "skin_female",
        "MI_Superhero_Male" => "hero_male",
        "MI_Superhero_Female" => "hero_female",
        "MI_Hair_1" => "hair1",
        "MI_Hair_2" => "hair2",
        "MI_Eyes" => "eye",
        _ => "skin_male",
    })
}

/// A piece of a fighter.
struct Part {
    file: fn() -> File,
    /// Only the head and neck (taken from a full-body base character).
    head_only: bool,
}

struct Spec {
    name: &'static str,
    blurb: &'static str,
    /// The outfit whose skeleton the fighter uses.
    skeleton: fn() -> File,
    parts: &'static [Part],
    /// Meshes (by name, a prefix is enough) shown in the team colour.
    accent: &'static [&'static str],
    swaps: &'static [(&'static str, &'static str)],
}

fn male_ranger() -> File {
    file!("Male_Ranger")
}
fn female_ranger() -> File {
    file!("Female_Ranger")
}
fn male_peasant() -> File {
    file!("Male_Peasant")
}
fn female_peasant() -> File {
    file!("Female_Peasant")
}
fn hero_male() -> File {
    file!("Superhero_Male_FullBody")
}
fn hero_female() -> File {
    file!("Superhero_Female_FullBody")
}
fn hair_beard() -> File {
    file!("Hair_Beard")
}
fn hair_buzzed() -> File {
    file!("Hair_Buzzed")
}
fn hair_long() -> File {
    file!("Hair_Long")
}

const SPECS: [Spec; 4] = [
    Spec {
        name: "Corvin Vale",
        blurb: "Ranger: hooded scout in leather, bracers and a shoulder guard",
        skeleton: male_ranger,
        parts: &[Part { file: male_ranger, head_only: false }, Part { file: hero_male, head_only: true }, Part { file: hair_beard, head_only: false }],
        accent: &["Male_Ranger_Head_Hood", "Male_Ranger_Acc_Pauldron"],
        swaps: &[],
    },
    Spec {
        name: "Sable Rook",
        blurb: "Huntress: light hooded hunter with pauldrons, in forest colours",
        skeleton: female_ranger,
        parts: &[Part { file: female_ranger, head_only: false }, Part { file: hero_female, head_only: true }],
        // only the pauldrons take the team colour: her hood keeps its own brown, so from behind she
        // doesn't look like the Ranger
        accent: &["Female_Ranger_Acc_Pauldrons"],
        swaps: &[("MI_Ranger", "ranger_alt")],
    },
    Spec {
        name: "Bram \"Anvil\" Kask",
        blurb: "Brawler: bearded street fighter in a rough tunic and boots",
        skeleton: male_peasant,
        parts: &[Part { file: male_peasant, head_only: false }, Part { file: hero_male, head_only: true }, Part { file: hair_buzzed, head_only: false }, Part { file: hair_beard, head_only: false }],
        accent: &["Male_Peasant_Body"],
        swaps: &[],
    },
    Spec {
        name: "Ilya Fen",
        blurb: "Striker: quick duelist with long hair and a fitted tunic",
        skeleton: female_peasant,
        parts: &[Part { file: female_peasant, head_only: false }, Part { file: hero_female, head_only: true }, Part { file: hair_long, head_only: false }],
        accent: &["Female_Peasant_Body"],
        swaps: &[("MI_Peasant", "peasant_alt")],
    },
];

pub const COUNT: usize = SPECS.len();

pub fn name(i: u8) -> &'static str {
    SPECS[(i as usize).min(COUNT - 1)].name
}
pub fn blurb(i: u8) -> &'static str {
    SPECS[(i as usize).min(COUNT - 1)].blurb
}

/// A fighter ready to draw and animate.
pub struct Model {
    pub verts: Vec<SkinVtx>,
    pub indices: Vec<u32>,
    /// Its skeleton's inverse bind matrices, in the rig's joint order.
    pub inv_bind: Vec<Mat4>,
    /// Its skeleton's rest transforms, per rig node (None where it has no such bone).
    rest: Vec<Option<Trs>>,
    /// Scale that brings it to the rig's height (everyone is the same size: fair hit shapes).
    pub norm: f32,
}

/// All four fighters, loaded once (None if the files couldn't be read).
pub fn shared() -> Option<&'static Vec<Model>> {
    static MODELS: std::sync::OnceLock<Option<Vec<Model>>> = std::sync::OnceLock::new();
    MODELS
        .get_or_init(|| {
            let rig = crate::anim::shared()?;
            let mut out = Vec::new();
            for s in &SPECS {
                match build(rig, s) {
                    Ok(m) => out.push(m),
                    Err(e) => {
                        eprintln!("fighter {} not loaded: {e}", s.name);
                        return None;
                    }
                }
            }
            Some(out)
        })
        .as_ref()
}

/// One fighter's model, if they're loaded.
pub fn model(i: u8) -> Option<&'static Model> {
    shared().and_then(|m| m.get((i as usize).min(COUNT - 1)))
}

struct Skeleton {
    /// Inverse bind matrix of each bone, by name.
    inv_bind: HashMap<String, Mat4>,
    /// Rest transform of each node, by name.
    rest: HashMap<String, Trs>,
}

fn parse(f: &File) -> Result<gltf::Document, String> {
    let g = gltf::Gltf::from_slice(f.json.as_bytes()).map_err(|e| e.to_string())?;
    Ok(g.document)
}

fn skeleton(f: &File) -> Result<Skeleton, String> {
    let doc = parse(f)?;
    let get = |_b: gltf::Buffer| Some(f.bin);
    let skin = doc.skins().next().ok_or("no skin")?;
    let names: Vec<String> = skin.joints().map(|j| j.name().unwrap_or("").to_string()).collect();
    let ib: Vec<Mat4> = skin.reader(get).read_inverse_bind_matrices().map(|it| it.map(|m| Mat4::from_cols_array_2d(&m)).collect()).unwrap_or_default();
    let mut inv_bind = HashMap::new();
    for (k, n) in names.iter().enumerate() {
        inv_bind.insert(n.clone(), ib.get(k).copied().unwrap_or(Mat4::IDENTITY));
    }
    let mut rest = HashMap::new();
    for node in doc.nodes() {
        if let Some(n) = node.name() {
            let (t, r, s) = node.transform().decomposed();
            rest.insert(n.to_string(), Trs { t: Vec3::from(t), r: Quat::from_array(r).normalize(), s: Vec3::from(s) });
        }
    }
    Ok(Skeleton { inv_bind, rest })
}

fn build(rig: &Rig, spec: &Spec) -> Result<Model, String> {
    let sk = skeleton(&(spec.skeleton)())?;
    let rig_joints = rig.joint_names();
    let joint_index: HashMap<&str, usize> = rig_joints.iter().enumerate().map(|(k, n)| (n.as_str(), k)).collect();
    let mut verts = Vec::new();
    let mut indices = Vec::new();
    for part in spec.parts {
        let f = (part.file)();
        let doc = parse(&f)?;
        let get = |_b: gltf::Buffer| Some(f.bin);
        let Some(skin) = doc.skins().next() else { continue };
        let pnames: Vec<String> = skin.joints().map(|j| j.name().unwrap_or("").to_string()).collect();
        let pib: Vec<Mat4> = skin.reader(get).read_inverse_bind_matrices().map(|it| it.map(|m| Mat4::from_cols_array_2d(&m)).collect()).unwrap_or_default();
        // per joint of this part: its bind space -> the fighter's bind space, and the rig index
        let conv: Vec<Mat4> = pnames
            .iter()
            .enumerate()
            .map(|(j, n)| match sk.inv_bind.get(n) {
                Some(ibc) => ibc.inverse() * pib.get(j).copied().unwrap_or(Mat4::IDENTITY),
                None => Mat4::IDENTITY,
            })
            .collect();
        let remap: Vec<u32> = pnames.iter().map(|n| joint_index.get(n.as_str()).copied().unwrap_or(0) as u32).collect();
        let head_bones: Vec<bool> = pnames.iter().map(|n| matches!(n.as_str(), "Head" | "neck_01")).collect();
        for mesh in doc.meshes() {
            let mname = mesh.name().unwrap_or("");
            let accent = spec.accent.iter().any(|a| mname.starts_with(a));
            for prim in mesh.primitives() {
                let r = prim.reader(get);
                let Some(pos) = r.read_positions() else { continue };
                let pos: Vec<[f32; 3]> = pos.collect();
                let nrm: Vec<[f32; 3]> = r.read_normals().map(|i| i.collect()).unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
                let uv: Vec<[f32; 2]> = r.read_tex_coords(0).map(|t| t.into_f32().collect()).unwrap_or_else(|| vec![[0.0, 0.0]; pos.len()]);
                let jnt: Vec<[u16; 4]> = r.read_joints(0).map(|j| j.into_u16().collect()).unwrap_or_else(|| vec![[0; 4]; pos.len()]);
                let wts: Vec<[f32; 4]> = r.read_weights(0).map(|w| w.into_f32().collect()).unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 0.0]; pos.len()]);
                let mat_name = prim.material().name().unwrap_or("");
                let lay = material_layer(mat_name, spec.swaps);
                let tri: Vec<u32> = match r.read_indices() {
                    Some(ix) => ix.into_u32().collect(),
                    None => (0..pos.len() as u32).collect(),
                };
                // a head taken off a full body: keep the triangles that move with the head or neck
                let keep_vert: Vec<bool> = (0..pos.len())
                    .map(|k| {
                        if !part.head_only || mname != hero_body_mesh(&doc) {
                            return true;
                        }
                        let (j, w) = (jnt[k], wts[k]);
                        let mut best = 0;
                        for q in 1..4 {
                            if w[q] > w[best] {
                                best = q;
                            }
                        }
                        head_bones.get(j[best] as usize).copied().unwrap_or(false)
                    })
                    .collect();
                let base = verts.len() as u32;
                let mut used = vec![u32::MAX; pos.len()];
                let mut next = base;
                for t in tri.chunks(3) {
                    if t.len() < 3 || !t.iter().all(|&k| keep_vert.get(k as usize).copied().unwrap_or(false)) {
                        continue;
                    }
                    for &k in t {
                        let k = k as usize;
                        if used[k] == u32::MAX {
                            used[k] = next;
                            next += 1;
                            // move into the fighter's bind space, blending each bone's offset
                            let (j, w) = (jnt[k], wts[k]);
                            let p = Vec3::from(pos[k]);
                            let n = Vec3::from(nrm[k]);
                            let mut pp = Vec3::ZERO;
                            let mut nn = Vec3::ZERO;
                            let mut ws = 0.0;
                            for q in 0..4 {
                                if w[q] <= 0.0 {
                                    continue;
                                }
                                let m = conv.get(j[q] as usize).copied().unwrap_or(Mat4::IDENTITY);
                                pp += m.transform_point3(p) * w[q];
                                nn += m.transform_vector3(n) * w[q];
                                ws += w[q];
                            }
                            let (pp, nn) = if ws > 0.0 { (pp / ws, nn.normalize_or(n)) } else { (p, n) };
                            let jr = [0, 1, 2, 3].map(|q| remap.get(j[q] as usize).copied().unwrap_or(0));
                            verts.push(SkinVtx { pos: pp.to_array(), nrm: nn.to_array(), joints: jr, weights: w, mat: (lay << 8) | accent as u32, uv: uv[k] });
                        }
                        indices.push(used[k]);
                    }
                }
                let _ = base;
            }
        }
    }
    if indices.is_empty() {
        return Err("no triangles".into());
    }
    // its skeleton in the rig's terms
    let inv_bind: Vec<Mat4> = rig_joints.iter().enumerate().map(|(k, n)| sk.inv_bind.get(n).copied().unwrap_or_else(|| rig.palette_inv_bind(k))).collect();
    let n_nodes = rig.node_names.len();
    let rest: Vec<Option<Trs>> = (0..n_nodes).map(|i| sk.rest.get(&rig.node_names[i]).copied().filter(|_| !rig.node_names[i].is_empty())).collect();
    let mut m = Model { verts, indices, inv_bind, rest, norm: 1.0 };
    // same height as the rig: compare where the head bone sits at rest
    if let Some(head) = rig.node("Head") {
        let rig_g = rig.globals(&rig.rest_pose());
        let mine = m.retarget(rig, &rig.rest_pose());
        let my_g = rig.globals(&mine);
        let (a, b) = (rig_g[head].w_axis.truncate().length(), my_g[head].w_axis.truncate().length());
        if b > 0.1 {
            m.norm = (a / b).clamp(0.8, 1.25);
        }
    }
    Ok(m)
}

/// The base character's body mesh (the one we cut the head from).
fn hero_body_mesh(doc: &gltf::Document) -> &'static str {
    for m in doc.meshes() {
        if let Some(n) = m.name() {
            if n.starts_with("Superhero_Female") {
                return "Superhero_Female";
            }
            if n.starts_with("Sphere.005_Retopology") {
                return "Sphere.005_Retopology.004";
            }
        }
    }
    ""
}

impl Model {
    /// Carry a pose of the rig over to this fighter's skeleton: each bone turns by as much as
    /// the rig's bone turned from its rest; bones keep their own lengths, and the hips move by
    /// as much (scaled to the leg length).
    pub fn retarget(&self, rig: &Rig, pose: &[Trs]) -> Vec<Trs> {
        pose.iter()
            .enumerate()
            .map(|(n, p)| match self.rest.get(n).copied().flatten() {
                Some(cr) => {
                    let rr = rig.rest_local(n);
                    let r = (cr.r * (rr.r.inverse() * p.r)).normalize();
                    let k = if rr.t.length() > 1e-4 { cr.t.length() / rr.t.length() } else { 1.0 };
                    let t = cr.t + (p.t - rr.t) * k;
                    Trs { t, r, s: p.s }
                }
                None => *p,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Fighter, Tuning};
    use crate::math::v3;

    #[test]
    fn four_fighters_load_and_sit_on_the_skeleton() {
        let rig = crate::anim::shared().expect("rig");
        let models = shared().expect("fighters load");
        assert_eq!(models.len(), 4);
        let nj = rig.joint_names().len() as u32;
        for (k, m) in models.iter().enumerate() {
            assert!(m.verts.len() > 3000 && m.indices.len() % 3 == 0, "{}: a real mesh", name(k as u8));
            assert!(m.verts.iter().all(|v| v.joints.iter().all(|&j| j < nj)), "joints in range");
            assert!(m.verts.iter().all(|v| (v.mat >> 8) < TEXTURES.len() as u32), "every part textured");
            assert!(m.verts.iter().any(|v| v.mat & 1 == 1), "{}: has team-colour pieces", name(k as u8));
            // at rest the skinned mesh lands exactly where it was modelled (the parts were moved
            // onto this skeleton correctly, and the pose carries over without twisting)
            let pose = m.retarget(rig, &rig.rest_pose());
            let pal = rig.palette_with(&pose, Mat4::IDENTITY, &m.inv_bind);
            let mut worst: f32 = 0.0;
            for v in m.verts.iter().step_by(37) {
                let p = Vec3::from(v.pos);
                let mut s = Vec3::ZERO;
                for q in 0..4 {
                    s += pal[v.joints[q] as usize].transform_point3(p) * v.weights[q];
                }
                worst = worst.max((s - rig.fix.transform_point3(p)).length());
            }
            assert!(worst < 0.01, "{}: rest pose off by {worst} m", name(k as u8));
        }
    }

    #[test]
    fn every_fighter_is_the_same_target() {
        let rig = crate::anim::shared().expect("rig");
        let mut tops = Vec::new();
        for b in 0..COUNT as u8 {
            let mut f = Fighter::new("t", 0, true);
            f.present = true;
            f.alive = true;
            f.loadout.body = b;
            let mut st = crate::character::CharState::new();
            let mut p = None;
            for k in 0..20 {
                p = crate::character::animate(rig, &mut st, &f, v3(0.0, 0.0, 0.0), k as f32 * 0.016, &Tuning::default());
            }
            let p = p.expect("posed");
            let head = p.hit.caps[0];
            tops.push(head.a.y.max(head.b.y) + head.r);
            // the hand holds the weapon where the model's hand is
            assert!((p.hit.caps[8].b - p.hand).len() < 0.02);
        }
        let (lo, hi) = tops.iter().fold((f32::MAX, f32::MIN), |(a, b), &t| (a.min(t), b.max(t)));
        assert!(hi - lo < 0.06, "head heights {tops:?}");
        assert!(lo > 1.6 && hi < 2.05);
    }
}