//! Skinned character: loads a rigged glTF (Quaternius "Universal Animation Library", CC0),
//! samples and blends its animation clips, adds the moves the pack doesn't have (strafing,
//! wall-runs, dashes, guard, sword combo swings, two-handed gun poses) by layering bone
//! rotations and arm IK on the same skeleton, and produces the bone matrices for GPU skinning.

use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;

pub static RIG_GLB: &[u8] = include_bytes!("../assets/UAL1_Standard.glb");

#[derive(Clone, Copy, Debug)]
pub struct Trs {
    pub t: Vec3,
    pub r: Quat,
    pub s: Vec3,
}

impl Trs {
    pub fn mat(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.s, self.r, self.t)
    }
    pub fn blend(&self, o: &Trs, w: f32) -> Trs {
        let r = if self.r.dot(o.r) < 0.0 { -o.r } else { o.r };
        Trs { t: self.t.lerp(o.t, w), r: self.r.lerp(r, w).normalize(), s: self.s.lerp(o.s, w) }
    }
}

enum Values {
    T(Vec<Vec3>),
    R(Vec<Quat>),
    S(Vec<Vec3>),
}

struct Channel {
    node: usize,
    times: Vec<f32>,
    values: Values,
}

pub struct Clip {
    pub duration: f32,
    channels: Vec<Channel>,
}

#[derive(Clone, Copy)]
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinVtx {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub joints: [u32; 4],
    pub weights: [f32; 4],
    /// Low byte: material (or 1 = team-colour accent on a textured fighter); bits 8+: texture
    /// layer of the fighters' texture set (255 = untextured).
    pub mat: u32,
    pub uv: [f32; 2],
}

/// Texture layer meaning "no texture" (the plain mannequin).
pub const NO_LAYER: u32 = 255;

/// The character rig, loaded once and shared (the game poses hit capsules with it, the
/// renderer draws with it).
pub fn shared() -> Option<&'static Rig> {
    static RIG: std::sync::OnceLock<Option<Rig>> = std::sync::OnceLock::new();
    RIG.get_or_init(|| Rig::load().ok()).as_ref()
}

pub struct Rig {
    parent: Vec<Option<usize>>,
    rest: Vec<Trs>,
    /// Nodes ordered so parents come before children.
    order: Vec<usize>,
    /// Skin joint -> node.
    pub joints: Vec<usize>,
    inv_bind: Vec<Mat4>,
    node_by_name: HashMap<String, usize>,
    /// Each node's name ("" when it has none).
    pub node_names: Vec<String>,
    pub clips: HashMap<String, Clip>,
    pub verts: Vec<SkinVtx>,
    pub indices: Vec<u32>,
    /// Turns model space into ours (forward +z, right hand on +x).
    pub fix: Mat4,
    pub fix_sign: Vec3,
}

fn interval(times: &[f32], t: f32) -> (usize, usize, f32) {
    if times.len() <= 1 || t <= times[0] {
        return (0, 0, 0.0);
    }
    let last = times.len() - 1;
    if t >= times[last] {
        return (last, last, 0.0);
    }
    let i = match times.binary_search_by(|x| x.partial_cmp(&t).unwrap_or(std::cmp::Ordering::Less)) {
        Ok(i) => return (i, i, 0.0),
        Err(i) => i - 1,
    };
    let span = (times[i + 1] - times[i]).max(1e-6);
    (i, i + 1, (t - times[i]) / span)
}

impl Rig {
    pub fn load() -> Result<Rig, String> {
        let g = gltf::Gltf::from_slice(RIG_GLB).map_err(|e| e.to_string())?;
        let blob = g.blob.clone().ok_or("no binary chunk")?;
        let doc = &g.document;
        let get = |_b: gltf::Buffer| Some(blob.as_slice());
        let n = doc.nodes().count();
        let mut parent = vec![None; n];
        let mut rest = vec![Trs { t: Vec3::ZERO, r: Quat::IDENTITY, s: Vec3::ONE }; n];
        let mut node_by_name = HashMap::new();
        let mut node_names = vec![String::new(); n];
        for node in doc.nodes() {
            for c in node.children() {
                parent[c.index()] = Some(node.index());
            }
            let (t, r, s) = node.transform().decomposed();
            rest[node.index()] = Trs { t: Vec3::from(t), r: Quat::from_array(r).normalize(), s: Vec3::from(s) };
            if let Some(name) = node.name() {
                node_by_name.insert(name.to_string(), node.index());
                node_names[node.index()] = name.to_string();
            }
        }
        // parents before children
        let mut order = Vec::with_capacity(n);
        let mut placed = vec![false; n];
        while order.len() < n {
            let before = order.len();
            for i in 0..n {
                if !placed[i] && parent[i].map_or(true, |p| placed[p]) {
                    placed[i] = true;
                    order.push(i);
                }
            }
            if order.len() == before {
                return Err("node loop".into());
            }
        }
        let skin = doc.skins().next().ok_or("no skin")?;
        let joints: Vec<usize> = skin.joints().map(|j| j.index()).collect();
        let inv_bind: Vec<Mat4> = skin
            .reader(get)
            .read_inverse_bind_matrices()
            .map(|it| it.map(|m| Mat4::from_cols_array_2d(&m)).collect())
            .unwrap_or_else(|| vec![Mat4::IDENTITY; joints.len()]);

        let mut verts = Vec::new();
        let mut indices = Vec::new();
        for mesh in doc.meshes() {
            for prim in mesh.primitives() {
                let r = prim.reader(get);
                let base = verts.len() as u32;
                let pos: Vec<[f32; 3]> = r.read_positions().ok_or("no positions")?.collect();
                let nrm: Vec<[f32; 3]> = r.read_normals().map(|i| i.collect()).unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
                let jnt: Vec<[u16; 4]> = r.read_joints(0).map(|j| j.into_u16().collect()).unwrap_or_else(|| vec![[0; 4]; pos.len()]);
                let wts: Vec<[f32; 4]> = r.read_weights(0).map(|w| w.into_f32().collect()).unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 0.0]; pos.len()]);
                let mat = prim.material().index().unwrap_or(0) as u32;
                for k in 0..pos.len() {
                    let j = jnt[k];
                    verts.push(SkinVtx { pos: pos[k], nrm: nrm[k], joints: [j[0] as u32, j[1] as u32, j[2] as u32, j[3] as u32], weights: wts[k], mat: (NO_LAYER << 8) | mat, uv: [0.0, 0.0] });
                }
                match r.read_indices() {
                    Some(ix) => indices.extend(ix.into_u32().map(|i| i + base)),
                    None => indices.extend(base..base + pos.len() as u32),
                }
            }
        }

        let mut clips = HashMap::new();
        for anim in doc.animations() {
            let mut channels = Vec::new();
            let mut duration = 0.0f32;
            for ch in anim.channels() {
                let r = ch.reader(get);
                let Some(inputs) = r.read_inputs() else { continue };
                let times: Vec<f32> = inputs.collect();
                if let Some(&t) = times.last() {
                    duration = duration.max(t);
                }
                let values = match r.read_outputs() {
                    Some(gltf::animation::util::ReadOutputs::Translations(it)) => Values::T(it.map(Vec3::from).collect()),
                    Some(gltf::animation::util::ReadOutputs::Rotations(it)) => Values::R(it.into_f32().map(|q| Quat::from_array(q).normalize()).collect()),
                    Some(gltf::animation::util::ReadOutputs::Scales(it)) => Values::S(it.map(Vec3::from).collect()),
                    _ => continue,
                };
                // cubic-spline data has in/out tangents around each value: keep the values
                let values = if ch.sampler().interpolation() == gltf::animation::Interpolation::CubicSpline {
                    match values {
                        Values::T(v) => Values::T(v.chunks(3).map(|c| c[1]).collect()),
                        Values::R(v) => Values::R(v.chunks(3).map(|c| c[1]).collect()),
                        Values::S(v) => Values::S(v.chunks(3).map(|c| c[1]).collect()),
                    }
                } else {
                    values
                };
                channels.push(Channel { node: ch.target().node().index(), times, values });
            }
            let name = anim.name().unwrap_or("").to_string();
            clips.insert(name, Clip { duration: duration.max(0.01), channels });
        }

        let mut rig = Rig { parent, rest, order, joints, inv_bind, node_by_name, node_names, clips, verts, indices, fix: Mat4::IDENTITY, fix_sign: Vec3::ONE };
        // which way does the model face, and which side is its right hand?
        let g = rig.globals(&rig.rest.clone());
        let p = |name: &str| rig.node(name).map(|n| g[n].w_axis.truncate());
        let (Some(foot), Some(ball), Some(hand_r)) = (p("foot_l"), p("ball_l"), p("hand_r")) else {
            return Err("skeleton is missing feet or hands".into());
        };
        let sz = if ball.z - foot.z >= 0.0 { 1.0 } else { -1.0 };
        let sx = if hand_r.x >= 0.0 { 1.0 } else { -1.0 };
        rig.fix_sign = Vec3::new(sx, 1.0, sz);
        rig.fix = Mat4::from_scale(rig.fix_sign);
        Ok(rig)
    }

    pub fn node(&self, name: &str) -> Option<usize> {
        self.node_by_name.get(name).copied()
    }

    pub fn rest_pose(&self) -> Vec<Trs> {
        self.rest.clone()
    }

    /// The rig's rest transform of one node (relative to its parent).
    pub fn rest_local(&self, n: usize) -> Trs {
        self.rest[n]
    }

    /// The rig's own inverse bind matrix for skin joint `k`.
    pub fn palette_inv_bind(&self, k: usize) -> Mat4 {
        self.inv_bind.get(k).copied().unwrap_or(Mat4::IDENTITY)
    }

    /// Names of the skin joints, in the order bone matrices are given to the GPU.
    pub fn joint_names(&self) -> Vec<String> {
        self.joints.iter().map(|&n| self.node_names[n].clone()).collect()
    }

    /// Pose of a clip at time `t` (looping or held at the end).
    pub fn sample(&self, clip: &str, t: f32, looped: bool, out: &mut Vec<Trs>) {
        out.clear();
        out.extend_from_slice(&self.rest);
        let Some(c) = self.clips.get(clip) else { return };
        let t = if looped { t.rem_euclid(c.duration) } else { t.clamp(0.0, c.duration) };
        for ch in &c.channels {
            let (a, b, w) = interval(&ch.times, t);
            let tr = &mut out[ch.node];
            match &ch.values {
                Values::T(v) => tr.t = v[a].lerp(v[b], w),
                Values::R(v) => tr.r = v[a].slerp(v[b], w).normalize(),
                Values::S(v) => tr.s = v[a].lerp(v[b], w),
            }
        }
    }

    pub fn duration(&self, clip: &str) -> f32 {
        self.clips.get(clip).map_or(1.0, |c| c.duration)
    }

    pub fn globals(&self, pose: &[Trs]) -> Vec<Mat4> {
        let mut g = vec![Mat4::IDENTITY; pose.len()];
        for &i in &self.order {
            let l = pose[i].mat();
            g[i] = match self.parent[i] {
                Some(p) => g[p] * l,
                None => l,
            };
        }
        g
    }

    /// Is `node` inside the subtree rooted at `top`?
    pub fn is_under(&self, mut node: usize, top: usize) -> bool {
        loop {
            if node == top {
                return true;
            }
            match self.parent[node] {
                Some(p) => node = p,
                None => return false,
            }
        }
    }

    /// Rotate a bone by `q` in model space (about its own joint).
    pub fn rotate_global(&self, pose: &mut [Trs], g: &[Mat4], node: usize, q: Quat) {
        let (_, gr, _) = g[node].to_scale_rotation_translation();
        let pr = match self.parent[node] {
            Some(p) => g[p].to_scale_rotation_translation().1,
            None => Quat::IDENTITY,
        };
        pose[node].r = (pr.inverse() * (q * gr)).normalize();
    }

    /// Two-bone IK: bend `upper`/`lower` so the end of `lower` (the `end` node) reaches
    /// `target`, with the elbow pushed towards `pole`. Model space.
    pub fn two_bone_ik(&self, pose: &mut Vec<Trs>, upper: usize, lower: usize, end: usize, target: Vec3, pole: Vec3) {
        let g = self.globals(pose);
        let pa = g[upper].w_axis.truncate();
        let pb = g[lower].w_axis.truncate();
        let pc = g[end].w_axis.truncate();
        let l1 = (pb - pa).length();
        let l2 = (pc - pb).length();
        if l1 < 1e-4 || l2 < 1e-4 {
            return;
        }
        let to = target - pa;
        let d = to.length().clamp(1e-3, (l1 + l2) * 0.999);
        let dir = to.normalize_or(Vec3::Z);
        let a = (l1 * l1 - l2 * l2 + d * d) / (2.0 * d);
        let h = (l1 * l1 - a * a).max(0.0).sqrt();
        let mut pdir = pole - pa;
        pdir -= dir * pdir.dot(dir);
        let pdir = pdir.normalize_or(Vec3::NEG_Y);
        let elbow = pa + dir * a + pdir * h;
        let qa = Quat::from_rotation_arc((pb - pa).normalize(), (elbow - pa).normalize());
        self.rotate_global(pose, &g, upper, qa);
        let g = self.globals(pose);
        let pb = g[lower].w_axis.truncate();
        let pc = g[end].w_axis.truncate();
        let goal = pa + dir * d;
        let qb = Quat::from_rotation_arc((pc - pb).normalize(), (goal - pb).normalize_or(pc - pb));
        self.rotate_global(pose, &g, lower, qb);
    }

    /// Bone matrices for skinning, with `world` (our space) applied.
    pub fn palette(&self, pose: &[Trs], world: Mat4) -> Vec<Mat4> {
        self.palette_with(pose, world, &self.inv_bind)
    }

    /// Bone matrices for a mesh bound to another skeleton with the same bones (a fighter
    /// model): its own inverse bind matrices, in this rig's joint order.
    pub fn palette_with(&self, pose: &[Trs], world: Mat4, inv_bind: &[Mat4]) -> Vec<Mat4> {
        let g = self.globals(pose);
        let m = world * self.fix;
        self.joints.iter().enumerate().map(|(j, &n)| m * g[n] * inv_bind.get(j).copied().unwrap_or(Mat4::IDENTITY)).collect()
    }
}

pub fn blend_into(a: &mut [Trs], b: &[Trs], w: f32) {
    if w <= 0.0 {
        return;
    }
    for (x, y) in a.iter_mut().zip(b.iter()) {
        *x = x.blend(y, w.min(1.0));
    }
}

/// Blend only the bones under `top` (an upper-body layer).
pub fn blend_masked(rig: &Rig, a: &mut [Trs], b: &[Trs], w: f32, top: usize) {
    if w <= 0.0 {
        return;
    }
    for i in 0..a.len() {
        if rig.is_under(i, top) {
            a[i] = a[i].blend(&b[i], w.min(1.0));
        }
    }
}

#[cfg(test)]
mod rig_tests {
    use super::*;
    #[test]
    fn print_rest_layout() {
        let rig = Rig::load().unwrap();
        let g = rig.globals(&rig.rest_pose());
        for n in ["root", "pelvis", "head", "Head", "hand_r", "hand_l", "upperarm_r", "lowerarm_r", "foot_l", "ball_l", "foot_r", "ball_r"] {
            if let Some(i) = rig.node(n) {
                let p = g[i].w_axis.truncate() * rig.fix_sign;
                println!("{n:12} ours = ({:.3}, {:.3}, {:.3})", p.x, p.y, p.z);
            }
        }
        println!("fix_sign = {:?}", rig.fix_sign);
        let mut pose = Vec::new();
        rig.sample("Pistol_Aim_Neutral", 0.3, false, &mut pose);
        let g = rig.globals(&pose);
        for n in ["upperarm_r", "lowerarm_r", "hand_r", "Head"] {
            let p = g[rig.node(n).unwrap()].w_axis.truncate() * rig.fix_sign;
            println!("aim {n:12} ours = ({:.3}, {:.3}, {:.3})", p.x, p.y, p.z);
        }
    }
}