use crate::math::*;
use eframe::egui::{Pos2, Rect};
use std::f32::consts::PI;

/// Camera used both by the 3D renderer and the HUD (to place name tags and numbers).
pub struct Cam {
    pub pos: V3,
    pub fwd: V3,
    pub right: V3,
    pub up: V3,
    pub focal: f32,
    pub center: Pos2,
    pub near: f32,
}

impl Cam {
    pub fn new(pos: V3, yaw: f32, pitch: f32, fov_deg: f32, rect: Rect) -> Cam {
        let fwd = look_dir(yaw, pitch);
        let right = yaw_right(yaw);
        let up = fwd.cross(right);
        let focal = (rect.width() * 0.5) / (fov_deg.to_radians() * 0.5).tan();
        Cam { pos, fwd, right, up, focal, center: rect.center(), near: 0.08 }
    }
    pub fn to_view(&self, p: V3) -> V3 {
        let d = p - self.pos;
        v3(d.dot(self.right), d.dot(self.up), d.dot(self.fwd))
    }
    pub fn proj_view(&self, v: V3) -> Pos2 {
        Pos2::new(self.center.x + v.x / v.z * self.focal, self.center.y - v.y / v.z * self.focal)
    }
    pub fn project(&self, p: V3) -> Option<Pos2> {
        let v = self.to_view(p);
        if v.z < self.near { None } else { Some(self.proj_view(v)) }
    }
}

/// Arm rotation for each sword swing at progress p (0..1).
pub fn sword_arm(root: &M3, idx: u8, p: f32) -> M3 {
    let e = 1.0 - (1.0 - p.clamp(0.0, 1.0)).powi(3);
    match idx {
        0 => root.mul(&M3::rot_y(1.5 - e * 3.0)).mul(&M3::rot_x(-PI * 0.5 + 0.1)),
        1 => root.mul(&M3::rot_y(-1.5 + e * 3.0)).mul(&M3::rot_x(-PI * 0.5 - 0.25)),
        2 | 6 => root.mul(&M3::rot_x(-PI * 1.05 + e * PI * 0.95)),
        // charged heavy: raised high behind the head, brought all the way down
        4 => root.mul(&M3::rot_y(0.2)).mul(&M3::rot_x(-PI * 1.25 + e * PI * 1.2)),
        // dash-through: one long flat cut, held out to the side as you pass
        5 => root.mul(&M3::rot_y(1.7 - e * 3.6)).mul(&M3::rot_x(-PI * 0.5)),
        // rising launcher: from low behind up over the head
        _ => root.mul(&M3::rot_y(0.25)).mul(&M3::rot_x(0.35 - e * PI * 1.15)),
    }
}

/// How far through `a..b` progress `p` is (0..1).
fn seg(p: f32, a: f32, b: f32) -> f32 {
    ((p - a) / (b - a)).clamp(0.0, 1.0)
}

/// A finisher's blade arm at progress `p` (0..1) for style `style` (see game::FINISHERS), and
/// how far out the arm reaches (share of a full reach).
pub fn finisher_arm(style: usize, p: f32) -> (M3, f32) {
    let id = M3::I;
    match style {
        // throat cut: one fast, flat slice
        0 => (sword_arm(&id, 1, seg(p, 0.3, 0.6)), 0.95),
        // twin stab: two quick thrusts
        1 => {
            let out = if p < 0.5 { (seg(p, 0.22, 0.42) * PI).sin() } else { (seg(p, 0.55, 0.78) * PI).sin() };
            (id.mul(&M3::rot_x(-PI * 0.5 + 0.2)), 0.5 + 0.47 * out)
        }
        // spinning edge: the blade held out flat while the body turns
        2 => (id.mul(&M3::rot_y(1.1)).mul(&M3::rot_x(-PI * 0.5)), 0.97),
        // quick draw: low and still, then one flash of a cut
        3 => (sword_arm(&id, 0, seg(p, 0.45, 0.58)), 0.97),
        // cross cut: one way, then back the other
        4 => {
            if p < 0.5 {
                (sword_arm(&id, 0, seg(p, 0.2, 0.42)), 0.97)
            } else {
                (sword_arm(&id, 1, seg(p, 0.55, 0.78)), 0.97)
            }
        }
        // rising cleave: from low behind, up through
        5 => (sword_arm(&id, 3, seg(p, 0.3, 0.65)), 0.97),
        // overhead crush: a long wind-up, then all the way down
        6 => (sword_arm(&id, 2, seg(p, 0.45, 0.68)), 0.97),
        // impale: a heavy thrust
        7 => (id.mul(&M3::rot_x(-PI * 0.5 + 0.1)), 0.5 + 0.47 * seg(p, 0.4, 0.62)),
        // wheel cleave: the greatsword out flat through a full turn
        8 => (id.mul(&M3::rot_y(1.3)).mul(&M3::rot_x(-PI * 0.5 - 0.1)), 0.97),
        // sky splitter: up with them, then straight down
        _ => (sword_arm(&id, 2, seg(p, 0.5, 0.72)), 0.97),
    }
}

/// A finisher's body motion at progress `p`: (extra turn about the vertical in radians, lean
/// forward in radians, rise in metres).
pub fn finisher_body(style: usize, p: f32) -> (f32, f32, f32) {
    let ease = |x: f32| 1.0 - (1.0 - x).powi(3);
    match style {
        2 => (std::f32::consts::TAU * ease(seg(p, 0.25, 0.7)), 0.1, 0.0),
        3 => (0.0, if p < 0.45 { 0.35 * seg(p, 0.0, 0.3) } else { 0.35 - 0.5 * seg(p, 0.45, 0.6) }, 0.0),
        5 => (0.0, -0.15 * seg(p, 0.3, 0.65), 0.25 * (seg(p, 0.3, 0.6) * PI).sin()),
        7 => (0.0, 0.35 * seg(p, 0.4, 0.62), 0.0),
        8 => (std::f32::consts::TAU * ease(seg(p, 0.2, 0.75)), 0.05, 0.0),
        9 => (0.0, 0.3 * seg(p, 0.5, 0.72), 0.8 * (seg(p, 0.0, 0.5) * PI * 0.5).sin() * (1.0 - seg(p, 0.6, 0.75))),
        _ => (0.0, 0.12 * (p * PI).sin(), 0.0),
    }
}