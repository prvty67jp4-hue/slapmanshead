//! Animation controller: picks and cross-fades clips for each fighter from what it is doing,
//! then layers aiming, strafing, guard, sword swings and weapon holds on top.

use crate::anim::{Rig, Trs, blend_into, blend_masked};
use crate::game::{Capsule, Fighter, HitPose, Part, RUN_SPEED, Weapon};
use crate::math::{M3, V3, look_dir, v3, wrap_angle};
use crate::render::sword_arm;
use glam::{Mat4, Quat, Vec3};
use std::collections::VecDeque;
use std::f32::consts::PI;

fn gv(v: V3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}
fn ov(v: Vec3) -> V3 {
    v3(v.x, v.y, v.z)
}

#[derive(Clone)]
struct Play {
    clip: &'static str,
    t: f32,
    rate: f32,
    looped: bool,
}

pub struct CharState {
    cur: Play,
    prev: Option<Play>,
    fade: f32,
    last_time: f32,
    was_dashing: bool,
    was_alive: bool,
    body_yaw: f32,
    /// Recent blade positions (base, tip, age) for the swing trail.
    pub trail: VecDeque<(V3, V3, f32)>,
    scratch: Vec<Trs>,
    scratch2: Vec<Trs>,
}

impl CharState {
    pub fn new() -> CharState {
        CharState {
            cur: Play { clip: "Idle_Loop", t: 0.0, rate: 1.0, looped: true },
            prev: None,
            fade: 1.0,
            last_time: 0.0,
            was_dashing: false,
            was_alive: true,
            body_yaw: 0.0,
            trail: VecDeque::new(),
            scratch: Vec::new(),
            scratch2: Vec::new(),
        }
    }
}

/// What a posed character gives back to the renderer.
pub struct Posed {
    pub palette: Vec<Mat4>,
    /// Right hand position and the weapon's frame (z = where the weapon points).
    pub hand: V3,
    pub weapon: M3,
    pub head: V3,
    pub head_frame: M3,
    pub waist: V3,
    /// Hit shapes following the bones.
    pub hit: HitPose,
}

struct Bones {
    root: usize,
    pelvis: usize,
    spine1: usize,
    spine2: usize,
    spine3: usize,
    neck: usize,
    head: usize,
    up_r: usize,
    lo_r: usize,
    hand_r: usize,
    up_l: usize,
    lo_l: usize,
    hand_l: usize,
    clav_r: usize,
    thigh_l: usize,
    calf_l: usize,
    foot_l: usize,
    thigh_r: usize,
    calf_r: usize,
    foot_r: usize,
}

fn bones(rig: &Rig) -> Option<Bones> {
    Some(Bones {
        root: rig.node("root")?,
        pelvis: rig.node("pelvis")?,
        spine1: rig.node("spine_01")?,
        spine2: rig.node("spine_02")?,
        spine3: rig.node("spine_03")?,
        neck: rig.node("neck_01")?,
        head: rig.node("Head")?,
        up_r: rig.node("upperarm_r")?,
        lo_r: rig.node("lowerarm_r")?,
        hand_r: rig.node("hand_r")?,
        up_l: rig.node("upperarm_l")?,
        lo_l: rig.node("lowerarm_l")?,
        hand_l: rig.node("hand_l")?,
        clav_r: rig.node("clavicle_r")?,
        thigh_l: rig.node("thigh_l")?,
        calf_l: rig.node("calf_l")?,
        foot_l: rig.node("foot_l")?,
        thigh_r: rig.node("thigh_r")?,
        calf_r: rig.node("calf_r")?,
        foot_r: rig.node("foot_r")?,
    })
}

/// A rotation about an axis given in our space, expressed in the model's space.
fn ours(rig: &Rig, axis: Vec3, angle: f32) -> Quat {
    let s = rig.fix_sign;
    let det = s.x * s.y * s.z;
    Quat::from_axis_angle((axis * s).normalize(), angle * det)
}

/// Pose a fighter. `pos` is where to draw it (interpolated), `dt` the frame time.
pub fn animate(rig: &Rig, st: &mut CharState, f: &Fighter, pos: V3, now: f32) -> Option<Posed> {
    let b = bones(rig)?;
    let dt = (now - st.last_time).clamp(0.0, 0.1);
    st.last_time = now;

    // ---- choose the base clip
    let speed = f.vel.flat().len();
    let fwd = f.fwd();
    let right = v3(fwd.z, 0.0, -fwd.x);
    let local_v = v3(f.vel.dot(right), 0.0, f.vel.dot(fwd));
    let dashing = f.dash_t > 0.0;
    let melee = f.weapon.is_melee();
    let mut target_yaw = 0.0;
    let want: Play = if !f.alive {
        Play { clip: "Death01", t: 0.0, rate: 1.0, looped: false }
    } else if f.stun_t > 0.0 {
        Play { clip: "Hit_Head", t: 0.0, rate: 1.4, looped: false }
    } else if let Some(w) = f.wall {
        Play { clip: "Sprint_Loop", t: 0.0, rate: if w.vertical { 1.5 } else { 1.3 }, looped: true }
    } else if f.sliding > 0.0 && f.on_ground {
        // sliding: low crouch (leaned back below)
        Play { clip: "Crouch_Idle_Loop", t: 0.0, rate: 1.0, looped: true }
    } else if dashing && f.on_ground || (dashing && f.vel.y.abs() < 3.0 && !f.air_dash) {
        Play { clip: "Roll", t: 0.0, rate: 2.0, looped: false }
    } else if !f.on_ground {
        if f.vel.y > 1.5 { Play { clip: "Jump_Start", t: 0.0, rate: 1.0, looped: false } } else { Play { clip: "Jump_Loop", t: 0.0, rate: 1.0, looped: true } }
    } else if speed > 0.6 {
        let a = local_v.x.atan2(local_v.z);
        let back = a.abs() > 1.85;
        let yaw = if back { wrap_angle(a - PI) } else { a };
        target_yaw = yaw.clamp(-1.25, 1.25) * 0.85;
        let sprint = speed > RUN_SPEED * 1.15;
        let rate = if sprint { (speed / 8.0).clamp(0.8, 1.6) } else { (speed / 4.6).clamp(0.7, 1.8) };
        Play { clip: if sprint { "Sprint_Loop" } else { "Jog_Fwd_Loop" }, t: 0.0, rate: if back { -rate } else { rate }, looped: true }
    } else if melee {
        Play { clip: "Sword_Idle", t: 0.0, rate: 1.0, looped: true }
    } else if matches!(f.weapon, Weapon::Revolver | Weapon::HandCannon) {
        Play { clip: "Pistol_Idle_Loop", t: 0.0, rate: 1.0, looped: true }
    } else {
        Play { clip: "Idle_Loop", t: 0.0, rate: 1.0, looped: true }
    };
    let restart = (dashing && !st.was_dashing) || (f.alive != st.was_alive);
    st.was_dashing = dashing;
    st.was_alive = f.alive;
    if want.clip != st.cur.clip || restart {
        let mut next = want.clone();
        // keep the stride when switching between jog and sprint
        if st.cur.looped && next.looped {
            let ph = st.cur.t / rig.duration(st.cur.clip);
            next.t = ph * rig.duration(next.clip);
        }
        st.prev = Some(st.cur.clone());
        st.cur = next;
        st.fade = 0.0;
    } else {
        st.cur.rate = want.rate;
    }
    st.cur.t += dt * st.cur.rate;
    if let Some(p) = st.prev.as_mut() {
        p.t += dt * p.rate;
    }
    let fade_speed = if !f.alive || dashing { 12.0 } else { 7.0 };
    st.fade = (st.fade + dt * fade_speed).min(1.0);
    if st.fade >= 1.0 {
        st.prev = None;
    }
    st.body_yaw += (target_yaw - st.body_yaw) * (dt * 10.0).min(1.0);

    let mut pose = Vec::new();
    rig.sample(st.cur.clip, st.cur.t, st.cur.looped, &mut pose);
    if let Some(p) = &st.prev {
        rig.sample(p.clip, p.t, p.looped, &mut st.scratch);
        let mut prev_pose = std::mem::take(&mut st.scratch);
        blend_into(&mut prev_pose, &pose, st.fade);
        pose = prev_pose.clone();
        st.scratch = prev_pose;
    }

    let alive_upper = f.alive && f.stun_t <= 0.0;
    // ---- upper body layers
    if alive_upper {
        if !melee {
            // aim pose blended by pitch
            let (clip, w) = if f.pitch > 0.0 { ("Pistol_Aim_Up", (f.pitch / 1.0).min(1.0)) } else { ("Pistol_Aim_Down", (-f.pitch / 1.0).min(1.0)) };
            rig.sample("Pistol_Aim_Neutral", 0.3, false, &mut st.scratch);
            rig.sample(clip, 0.3, false, &mut st.scratch2);
            let mut aim = std::mem::take(&mut st.scratch);
            blend_into(&mut aim, &st.scratch2, w);
            if f.recoil > 0.0 {
                rig.sample("Pistol_Shoot", (1.0 - f.recoil) * rig.duration("Pistol_Shoot") * 0.6, false, &mut st.scratch2);
                blend_into(&mut aim, &st.scratch2, f.recoil * 0.8);
            }
            blend_masked(rig, &mut pose, &aim, 1.0, b.spine1);
            st.scratch = aim;
        } else if speed > 0.6 && f.on_ground && f.wall.is_none() {
            // sword held ready while running
            rig.sample("Sword_Idle", st.cur.t, true, &mut st.scratch);
            let tmp = std::mem::take(&mut st.scratch);
            blend_masked(rig, &mut pose, &tmp, 0.5, b.clav_r);
            st.scratch = tmp;
        }
    }
    if f.alive && f.hurt_t > 0.0 {
        let k = (f.hurt_t / 0.25).clamp(0.0, 1.0);
        rig.sample("Hit_Chest", (1.0 - k) * 0.35, false, &mut st.scratch2);
        let tmp = std::mem::take(&mut st.scratch2);
        blend_masked(rig, &mut pose, &tmp, k * 0.8, b.spine1);
        st.scratch2 = tmp;
    }

    // ---- strafing: hips turn towards where you run, chest keeps facing the aim
    if f.alive && st.body_yaw.abs() > 0.01 {
        let g = rig.globals(&pose);
        rig.rotate_global(&mut pose, &g, b.pelvis, ours(rig, Vec3::Y, st.body_yaw));
        let g = rig.globals(&pose);
        rig.rotate_global(&mut pose, &g, b.spine1, ours(rig, Vec3::Y, -st.body_yaw * 0.5));
        let g = rig.globals(&pose);
        rig.rotate_global(&mut pose, &g, b.spine3, ours(rig, Vec3::Y, -st.body_yaw * 0.5));
    }
    // ---- aim pitch through the spine and head
    if alive_upper {
        let p = f.pitch.clamp(-1.2, 1.2);
        let share = if melee { 0.15 } else { 0.3 };
        for bone in [b.spine2, b.spine3] {
            let g = rig.globals(&pose);
            rig.rotate_global(&mut pose, &g, bone, ours(rig, Vec3::X, -p * share));
        }
        let g = rig.globals(&pose);
        rig.rotate_global(&mut pose, &g, b.head, ours(rig, Vec3::X, -p * 0.35));
    }
    // ---- flinch: the part that was hit jerks back a little
    let flinch = if f.alive && f.hurt_t > 0.0 { (f.hurt_t / 0.25).clamp(0.0, 1.0) } else { 0.0 };
    let hit_part = Part::from_u8(f.hit_part);
    if flinch > 0.0 {
        let k = (flinch * std::f32::consts::PI).sin().max(flinch * 0.6); // quick snap, ease back
        let turn = |pose: &mut Vec<Trs>, bone: usize, axis: Vec3, a: f32| {
            let g = rig.globals(pose);
            rig.rotate_global(pose, &g, bone, ours(rig, axis, a));
        };
        match hit_part {
            Part::Head => turn(&mut pose, b.neck, Vec3::X, -0.5 * k),
            Part::Chest => turn(&mut pose, b.spine3, Vec3::X, -0.28 * k),
            Part::Hips => turn(&mut pose, b.pelvis, Vec3::Y, 0.3 * k),
            Part::LegL => turn(&mut pose, b.thigh_l, Vec3::X, 0.45 * k),
            Part::LegR => turn(&mut pose, b.thigh_r, Vec3::X, 0.45 * k),
            _ => {}
        }
    }

    // helpers between our space and the model's
    let s = rig.fix_sign;
    let to_model = |v: Vec3| v * s;
    let to_ours = |v: Vec3| v * s;
    let gpos = |pose: &Vec<Trs>, n: usize| -> Vec3 { to_ours(rig.globals(pose)[n].w_axis.truncate()) };

    // ---- arms
    let aim = gv(look_dir(0.0, f.pitch.clamp(-1.3, 1.3)));
    let sh_r = gpos(&pose, b.up_r);
    let sh_l = gpos(&pose, b.up_l);
    let reach = {
        let a = gpos(&pose, b.up_r);
        let e = gpos(&pose, b.lo_r);
        let h = gpos(&pose, b.hand_r);
        (e - a).length() + (h - e).length()
    };
    let chest = (sh_r + sh_l) * 0.5;
    let mut weapon_dir: Option<Vec3> = None;
    if alive_upper {
        let pole_r = sh_r + Vec3::new(0.5, -0.6, -0.3);
        let pole_l = sh_l + Vec3::new(-0.5, -0.6, -0.3);
        if melee && f.slash_t > 0.0 {
            let p = 1.0 - f.slash_t / f.weapon.slash_dur();
            let arm = sword_arm(&M3::I, f.slash_idx, p);
            let dir = gv(arm.apply(v3(0.0, -1.0, 0.0))).normalize();
            let target = sh_r + dir * reach * 0.97;
            rig.two_bone_ik(&mut pose, b.up_r, b.lo_r, b.hand_r, to_model(target), to_model(sh_r + Vec3::new(0.2, -0.8, -0.6)));
            weapon_dir = Some(dir);
        } else if melee && f.guard {
            let hr = chest + Vec3::new(0.05, 0.05, 0.42);
            rig.two_bone_ik(&mut pose, b.up_r, b.lo_r, b.hand_r, to_model(hr), to_model(pole_r));
            rig.two_bone_ik(&mut pose, b.up_l, b.lo_l, b.hand_l, to_model(hr + Vec3::new(-0.1, -0.06, -0.02)), to_model(pole_l));
            weapon_dir = Some(Vec3::new(-0.75, 0.55, 0.3).normalize());
        } else if !melee {
            let side = Vec3::X;
            let down = Vec3::NEG_Y;
            let (hr, hl) = match f.weapon {
                Weapon::Revolver => (sh_r + aim * reach * 0.9 - side * 0.06, None),
                // the hand cannon: both hands on it
                Weapon::HandCannon => {
                    let hr = sh_r + aim * reach * 0.88 - side * 0.1;
                    (hr, Some(hr - side * 0.07 + down * 0.04 - aim * 0.04))
                }
                Weapon::Rocket => {
                    let hr = sh_r + aim * 0.22 + down * 0.12;
                    (hr, Some(hr + aim * 0.32 - side * 0.12 + down * 0.04))
                }
                Weapon::Smg => {
                    let hr = chest + side * 0.1 + aim * 0.42 + down * 0.1;
                    (hr, Some(hr + aim * 0.18 - side * 0.04))
                }
                _ => {
                    let hr = chest + side * 0.12 + aim * 0.32 + down * 0.08;
                    (hr, Some(hr + aim * 0.36 - side * 0.06 + down * 0.02))
                }
            };
            let reload = f.reload_t > 0.0;
            let hr = if reload { hr + down * 0.15 - aim * 0.05 } else { hr };
            rig.two_bone_ik(&mut pose, b.up_r, b.lo_r, b.hand_r, to_model(hr), to_model(pole_r));
            if let Some(hl) = hl {
                let hl = if reload { hr + Vec3::new(-0.12, 0.05, 0.0) } else { hl };
                rig.two_bone_ik(&mut pose, b.up_l, b.lo_l, b.hand_l, to_model(hl), to_model(pole_l));
            }
            weapon_dir = Some(if reload { (aim + down * 0.7).normalize() } else { aim });
        }
    }

    if flinch > 0.0 && matches!(hit_part, Part::ArmL | Part::ArmR) {
        let k = (flinch * std::f32::consts::PI).sin().max(flinch * 0.6);
        let (bone, side) = if hit_part == Part::ArmL { (b.up_l, -1.0) } else { (b.up_r, 1.0) };
        let g = rig.globals(&pose);
        rig.rotate_global(&mut pose, &g, bone, ours(rig, Vec3::Z, side * 0.45 * k));
    }

    // ---- world placement with leans
    let mut lean = Quat::IDENTITY;
    if !f.alive {
        // the death clip does the falling
    } else if let Some(w) = f.wall {
        if w.vertical {
            lean = Quat::from_rotation_x(-0.45);
        } else {
            let side = right.dot(w.normal);
            lean = Quat::from_rotation_z(-side * 0.5);
        }
    } else if f.sliding > 0.0 && f.on_ground {
        lean = Quat::from_rotation_x(-0.32);
    } else if dashing && !f.on_ground && f.air_dash {
        lean = Quat::from_rotation_x(0.45);
    } else if f.on_ground && speed > 1.0 {
        lean = Quat::from_rotation_x(0.06 * (speed / RUN_SPEED).min(1.3));
    }
    // our yaw: forward = (sin yaw, 0, cos yaw)
    let world = Mat4::from_rotation_translation(Quat::from_rotation_y(f.yaw) * lean, gv(pos));
    let palette = rig.palette(&pose, world);

    // ---- attachment points in world space
    let g = rig.globals(&pose);
    let wm = world * rig.fix;
    let wp = |n: usize| ov(wm.transform_point3(g[n].w_axis.truncate()));
    let hand = wp(b.hand_r);
    let elbow = wp(b.lo_r);
    let to_world_dir = |d: Vec3| ov((Quat::from_rotation_y(f.yaw) * lean) * d).norm();
    let z = match weapon_dir {
        Some(d) => to_world_dir(d),
        None => (hand - elbow).norm(),
    };
    let up_hint = if z.y.abs() > 0.9 { yaw_fwd_of(f.yaw) } else { v3(0.0, 1.0, 0.0) };
    let x = up_hint.cross(z).norm();
    let y = z.cross(x).norm();
    let head = wp(b.head);
    let neck = wp(b.neck);
    let hy = (head - neck).norm();
    let hf = f.fwd();
    let hz = (hf - hy * hf.dot(hy)).norm();
    let hx = hy.cross(hz).norm();
    let _ = b.root;
    // hit shapes along the bones
    let cap = |a: V3, b2: V3, r: f32, part: Part| Capsule { a, b: b2, r, part };
    let (sh_l, sh_r) = (wp(b.up_l), wp(b.up_r));
    let hit = HitPose {
        caps: [
            cap(head + hy * 0.04, head + hy * 0.17, 0.115, Part::Head),
            cap(neck, head, 0.065, Part::Chest),
            cap(wp(b.spine2), wp(b.spine3), 0.16, Part::Chest),
            cap(sh_l, sh_r, 0.085, Part::Chest),
            cap(wp(b.pelvis), wp(b.spine1), 0.15, Part::Hips),
            cap(sh_l, wp(b.lo_l), 0.065, Part::ArmL),
            cap(wp(b.lo_l), wp(b.hand_l), 0.055, Part::ArmL),
            cap(sh_r, elbow, 0.065, Part::ArmR),
            cap(elbow, hand, 0.055, Part::ArmR),
            cap(wp(b.thigh_l), wp(b.calf_l), 0.085, Part::LegL),
            cap(wp(b.calf_l), wp(b.foot_l), 0.065, Part::LegL),
            cap(wp(b.thigh_r), wp(b.calf_r), 0.085, Part::LegR),
            cap(wp(b.calf_r), wp(b.foot_r), 0.065, Part::LegR),
        ],
    };
    Some(Posed { palette, hand, weapon: M3 { x, y, z }, head, head_frame: M3 { x: hx, y: hy, z: hz }, waist: wp(b.spine1), hit })
}
fn yaw_fwd_of(yaw: f32) -> V3 {
    v3(yaw.sin(), 0.0, yaw.cos())
}

#[cfg(test)]
mod pose_tests {
    use super::*;
    #[test]
    fn guns_point_forward() {
        let rig = Rig::load().unwrap();
        for (yaw, w) in [(0.0f32, Weapon::Revolver), (std::f32::consts::PI, Weapon::Revolver), (0.0, Weapon::Rifle), (0.0, Weapon::Rocket)] {
            let mut f = Fighter::new("t", 0, true);
            f.present = true;
            f.alive = true;
            f.yaw = yaw;
            f.loadout.gun1 = w;
            f.cur = 1;
            f.weapon = w;
            let mut st = CharState::new();
            let mut p = None;
            for k in 0..30 {
                p = animate(&rig, &mut st, &f, v3(0.0, 0.0, 0.0), k as f32 * 0.016);
            }
            let p = p.unwrap();
            assert!(p.weapon.z.dot(f.fwd()) > 0.9, "weapon should point where the fighter faces");
            assert!((p.hand - v3(0.0, 0.0, 0.0)).dot(f.fwd()) > 0.0, "hand should be in front");
            println!("{:?} yaw {:.2}: hand ({:.2},{:.2},{:.2}) weapon z ({:.2},{:.2},{:.2}) fwd ({:.2},{:.2})", w, yaw, p.hand.x, p.hand.y, p.hand.z, p.weapon.z.x, p.weapon.z.y, p.weapon.z.z, f.fwd().x, f.fwd().z);
        }
    }
}