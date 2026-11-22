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
        2 => root.mul(&M3::rot_x(-PI * 1.05 + e * PI * 0.95)),
        // rising launcher: from low behind up over the head
        _ => root.mul(&M3::rot_y(0.25)).mul(&M3::rot_x(0.35 - e * PI * 1.15)),
    }
}
