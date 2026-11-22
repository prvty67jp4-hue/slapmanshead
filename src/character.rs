//! Animation controller: picks and cross-fades clips for each fighter from what it is doing,
//! then layers aiming, strafing, guard, sword swings and weapon holds on top.

use crate::anim::{Rig, Trs, blend_into, blend_masked};
use crate::game::{Capsule, FAN_THROW_TIME, Fighter, HitPose, KICK_TIME, Part, RUN_SPEED, THROW_TIME, Tuning, Weapon, finisher};
use crate::math::{M3, V3, look_dir, v3, wrap_angle};
use crate::render::{finisher_arm, finisher_body, sword_arm};
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
    // ---- feel: smoothed acceleration (for leaning), landings and stops
    prev_vel: V3,
    acc_f: f32,
    acc_s: f32,
    was_ground: bool,
    fall_vy: f32,
    land_k: f32,
    land_age: f32,
    stop_k: f32,
    crouch: f32,
    /// Recent blade positions (base, tip, age) for the swing trail.
    pub trail: VecDeque<(V3, V3, f32)>,
    /// The long coat: cloth points (rows down from the waist), where they were last step, and
    /// when it was last stepped.
    pub cloth: Vec<V3>,
    pub cloth_prev: Vec<V3>,
    pub cloth_time: f32,
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
            prev_vel: V3::ZERO,
            acc_f: 0.0,
            acc_s: 0.0,
            was_ground: true,
            fall_vy: 0.0,
            land_k: 0.0,
            land_age: 1.0,
            stop_k: 0.0,
            crouch: 0.0,
            trail: VecDeque::new(),
            cloth: Vec::new(),
            cloth_prev: Vec::new(),
            cloth_time: 0.0,
            scratch: Vec::new(),
            scratch2: Vec::new(),
        }
    }
}

/// Track acceleration, landings and stops; returns the extra (forward, sideways) lean in
/// radians and updates the crouch weight.
fn feel_update(st: &mut CharState, f: &Fighter, dt: f32, tn: &Tuning) -> (f32, f32) {
    let v = f.vel.flat();
    if dt > 0.0 && f.alive {
        let fwd = f.fwd();
        let right = v3(fwd.z, 0.0, -fwd.x);
        let a = (v - st.prev_vel) * (1.0 / dt);
        let a = if a.len() > 400.0 { V3::ZERO } else { a }; // a teleport or respawn
        let k = (dt * 12.0).min(1.0);
        st.acc_f += (a.dot(fwd) - st.acc_f) * k;
        st.acc_s += (a.dot(right) - st.acc_s) * k;
        // a hard stop: braking quickly from a run, feet stay planted a moment
        let sp = v.len();
        let braking = f.on_ground && f.dash_t <= 0.0 && st.prev_vel.len() > 3.0 && a.dot(st.prev_vel.norm()) < -25.0;
        let want = if braking { ((-a.dot(st.prev_vel.norm()) - 25.0) / 40.0).clamp(0.3, 1.0) * tn.skid.min(1.5) } else { 0.0 };
        st.stop_k += (want.min(1.0) - st.stop_k) * (dt * if want > st.stop_k { 30.0 } else { 6.0 }).min(1.0);
        if sp < 0.3 {
            st.stop_k *= 1.0 - (dt * 4.0).min(1.0);
        }
        // landings, heavier from higher
        if f.on_ground && !st.was_ground && st.fall_vy < -3.0 {
            st.land_k = ((-st.fall_vy - 3.0) / 14.0).clamp(0.2, 1.0) * tn.land_squash;
            st.land_age = 0.0;
        }
        if f.on_ground {
            st.fall_vy = 0.0;
        } else {
            st.fall_vy = st.fall_vy.min(f.vel.y);
        }
        st.was_ground = f.on_ground;
        st.land_age += dt;
    }
    st.prev_vel = v;
    if !f.alive {
        st.acc_f = 0.0;
        st.acc_s = 0.0;
        st.crouch = 0.0;
        return (0.0, 0.0);
    }
    // landing squash: in fast, out over a quarter second or so (longer when heavy)
    let hold = 0.18 + 0.22 * st.land_k.min(1.0);
    let land = if st.land_age < 0.05 { st.land_age / 0.05 } else { (1.0 - (st.land_age - 0.05) / hold).max(0.0) };
    let land_w = st.land_k * land * land.max(0.0);
    let land_w = land_w.max(0.0);
    let windup = if f.dash_wind > 0.0 { 0.9 } else { 0.0 };
    let target = (land_w * 0.85).max(windup).max(st.stop_k * 0.35).min(1.0);
    let rate = if target > st.crouch { 40.0 } else { 9.0 };
    st.crouch += (target - st.crouch) * (dt * rate).min(1.0);
    let grounded = f.on_ground && f.wall.is_none();
    let lean_k = if grounded { tn.lean } else { tn.lean * 0.3 };
    let pitch = (st.acc_f / 60.0).clamp(-1.0, 1.0) * 0.17 * lean_k + land_w.min(1.5) * 0.12 + windup * 0.12;
    let roll = (st.acc_s / 40.0).clamp(-1.0, 1.0) * 0.2 * lean_k;
    (pitch, roll)
}

/// The throw's three parts from its progress (0..1): pulling the hand in across the chest,
/// whipping it out, and bringing it back to the ready hold. Each 0..1.
fn throw_phases(tp: f32) -> (f32, f32, f32) {
    let smooth = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let pull = smooth(tp / 0.15);
    let w = ((tp - 0.12) / 0.43).clamp(0.0, 1.0);
    let whip = 1.0 - (1.0 - w).powi(3);
    let back = smooth((tp - 0.72) / 0.28);
    (pull, whip, back)
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
/// `tn` sizes the whole character and sets how much it leans, squashes and plants its feet
/// (the developer tuning panel changes them).
pub fn animate(rig: &Rig, st: &mut CharState, f: &Fighter, pos: V3, now: f32, tn: &Tuning) -> Option<Posed> {
    let b = bones(rig)?;
    let scale = tn.char_scale;
    let dt = (now - st.last_time).clamp(0.0, 0.1);
    st.last_time = now;
    let feel = feel_update(st, f, dt, tn);

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
    // planting the feet on a hard stop: the stride all but freezes
    let plant = if st.cur.looped && f.on_ground { 1.0 - st.stop_k * 0.8 } else { 1.0 };
    st.cur.t += dt * st.cur.rate * plant;
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

    // ---- crouch layer: landing squash, the dash wind-up, a planted stop
    if f.alive && st.crouch > 0.01 && !(f.sliding > 0.0 && f.on_ground) {
        rig.sample("Crouch_Idle_Loop", 0.2, true, &mut st.scratch2);
        let tmp = std::mem::take(&mut st.scratch2);
        blend_into(&mut pose, &tmp, st.crouch.min(1.0));
        st.scratch2 = tmp;
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

    // ---- throwing stars: how far through the throw (0..1), and the body twisting into it
    let throw_p = if alive_upper && f.weapon == Weapon::Shuriken && f.throw_t > 0.0 {
        let total = if f.throw_fan { FAN_THROW_TIME } else { THROW_TIME };
        Some((1.0 - f.throw_t / total).clamp(0.0, 1.0))
    } else {
        None
    };
    let phases = throw_p.map(throw_phases);
    if let Some((pull, whip, back)) = phases {
        // wound up to the left, then the chest whips round to the right as the arm goes out
        let (from, to) = if f.throw_fan { (-0.5, 0.45) } else { (-0.35, 0.3) };
        let twist = (from * pull + (to - from) * whip) * (1.0 - back);
        for (bone, share) in [(b.spine2, 0.4), (b.spine3, 0.6)] {
            let g = rig.globals(&pose);
            rig.rotate_global(&mut pose, &g, bone, ours(rig, Vec3::Y, twist * share));
        }
    }

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
        if melee && f.fin > 0 {
            // a finisher: its own blade path, reaching in and out
            let pr = (f.fin_t / finisher(f.fin).dur).clamp(0.0, 1.0);
            let (arm, rk) = finisher_arm(f.fin as usize - 1, pr);
            let dir = gv(arm.apply(v3(0.0, -1.0, 0.0))).normalize();
            let target = sh_r + dir * reach * rk;
            rig.two_bone_ik(&mut pose, b.up_r, b.lo_r, b.hand_r, to_model(target), to_model(sh_r + Vec3::new(0.2, -0.8, -0.6)));
            weapon_dir = Some(dir);
        } else if melee && f.slash_t > 0.0 {
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
                // throwing stars: one hand, drawn back a little, ready to flick
                Weapon::Shuriken => (sh_r + aim * reach * 0.45 - Vec3::Y * 0.16 + side * 0.02, None),
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
            // the throw: the hand starts tucked in across the chest and whips out forward and
            // to the side (the fan sweeps wider and flatter), elbow leading
            let (hr, pole_r) = match phases {
                Some((pull, whip, back)) => {
                    // tucked in at the opposite side of the chest, then whipped out to a straight
                    // arm in front and to the side, then back to the ready hold
                    let (start, end) = if f.throw_fan {
                        (chest - side * 0.26 + aim * 0.16 + Vec3::Y * 0.04, sh_r + aim * reach * 0.8 + side * reach * 0.5)
                    } else {
                        (chest - side * 0.2 + aim * 0.12 + Vec3::Y * 0.06, sh_r + aim * reach * 0.97 + side * reach * 0.2 + Vec3::Y * 0.03)
                    };
                    let arc = aim * (0.12 * (whip * PI).sin());
                    let p = hr.lerp(start, pull).lerp(end, whip) + arc;
                    (p.lerp(hr, back), sh_r + Vec3::new(0.7, -0.25, 0.05))
                }
                None => (hr, pole_r),
            };
            rig.two_bone_ik(&mut pose, b.up_r, b.lo_r, b.hand_r, to_model(hr), to_model(pole_r));
            if let Some(hl) = hl {
                let hl = if reload { hr + Vec3::new(-0.12, 0.05, 0.0) } else { hl };
                rig.two_bone_ik(&mut pose, b.up_l, b.lo_l, b.hand_l, to_model(hl), to_model(pole_l));
            }
            weapon_dir = Some(if reload { (aim + down * 0.7).normalize() } else { aim });
        }
    }

    // ---- kick: the right leg snaps forward and back
    if f.alive && f.kick_t > 0.0 {
        let p = 1.0 - (f.kick_t / KICK_TIME).clamp(0.0, 1.0);
        // fast out (it lands at about a third), slower back
        let k = if p < 0.3 { (p / 0.3).powf(0.7) } else { (1.0 - (p - 0.3) / 0.7).max(0.0).powi(2) };
        let g = rig.globals(&pose);
        rig.rotate_global(&mut pose, &g, b.thigh_r, ours(rig, Vec3::X, -1.45 * k));
        let g = rig.globals(&pose);
        rig.rotate_global(&mut pose, &g, b.calf_r, ours(rig, Vec3::X, 0.5 * (1.0 - k) * k * 2.0));
        let g = rig.globals(&pose);
        rig.rotate_global(&mut pose, &g, b.spine1, ours(rig, Vec3::X, 0.25 * k));
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
    if f.alive && f.wall.is_none() && !(f.sliding > 0.0 && f.on_ground) {
        // lean into speeding up, slowing down and turning; tip forward into a hard landing
        lean = lean * Quat::from_rotation_x(feel.0) * Quat::from_rotation_z(feel.1);
    }
    // our yaw: forward = (sin yaw, 0, cos yaw)
    // the chosen fighter: the pose carried over to its own skeleton (its own bone lengths), sized
    // to the rig's height so every fighter is the same target
    let model = crate::chars::model(f.loadout.body);
    let (pose, norm) = match model {
        Some(m) => (m.retarget(rig, &pose), m.norm),
        None => (pose, 1.0),
    };
    // a finisher turns, leans and lifts the whole body
    let (spin, fin_lean, lift) = if f.alive && f.fin > 0 { finisher_body(f.fin as usize - 1, (f.fin_t / finisher(f.fin).dur).clamp(0.0, 1.0)) } else { (0.0, 0.0, 0.0) };
    let lean = lean * Quat::from_rotation_x(fin_lean);
    let body_rot = Quat::from_rotation_y(f.yaw + spin) * lean;
    let world = Mat4::from_scale_rotation_translation(Vec3::splat(scale.clamp(0.2, 4.0) * norm), body_rot, gv(pos) + Vec3::Y * lift);
    let palette = match model {
        Some(m) => rig.palette_with(&pose, world, &m.inv_bind),
        None => rig.palette(&pose, world),
    };

    // ---- attachment points (and hit shapes) on that same skeleton, in world space
    let g = rig.globals(&pose);
    let wm = world * rig.fix;
    let wp = |n: usize| ov(wm.transform_point3(g[n].w_axis.truncate()));
    let hand = wp(b.hand_r);
    let elbow = wp(b.lo_r);
    let to_world_dir = |d: Vec3| ov(body_rot * d).norm();
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
    let sc = scale.clamp(0.2, 4.0);
    let cap = |a: V3, b2: V3, r: f32, part: Part| Capsule { a, b: b2, r: r * sc, part };
    let (sh_l, sh_r) = (wp(b.up_l), wp(b.up_r));
    let hit = HitPose {
        caps: [
            cap(head + hy * (0.04 * sc), head + hy * (0.17 * sc), 0.115, Part::Head),
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
                p = animate(&rig, &mut st, &f, v3(0.0, 0.0, 0.0), k as f32 * 0.016, &Tuning::default());
            }
            let p = p.unwrap();
            assert!(p.weapon.z.dot(f.fwd()) > 0.9, "weapon should point where the fighter faces");
            assert!((p.hand - v3(0.0, 0.0, 0.0)).dot(f.fwd()) > 0.0, "hand should be in front");
            println!("{:?} yaw {:.2}: hand ({:.2},{:.2},{:.2}) weapon z ({:.2},{:.2},{:.2}) fwd ({:.2},{:.2})", w, yaw, p.hand.x, p.hand.y, p.hand.z, p.weapon.z.x, p.weapon.z.y, p.weapon.z.z, f.fwd().x, f.fwd().z);
        }
    }

    #[test]
    fn stars_are_thrown_from_the_chest_outwards() {
        let rig = Rig::load().unwrap();
        for fan in [false, true] {
            let mut f = Fighter::new("t", 0, true);
            f.present = true;
            f.alive = true;
            f.yaw = 0.0;
            f.loadout.gun1 = Weapon::Shuriken;
            f.cur = 1;
            f.weapon = Weapon::Shuriken;
            f.throw_fan = fan;
            let total = if fan { FAN_THROW_TIME } else { THROW_TIME };
            let right = v3(1.0, 0.0, 0.0); // yaw 0 faces +z; the right hand side is +x
            let mut st = CharState::new();
            let mut at = |t_left: f32, st: &mut CharState, k: f32| {
                f.throw_t = t_left;
                let mut p = None;
                for j in 0..4 {
                    p = animate(&rig, st, &f, V3::ZERO, k + j as f32 * 0.001, &Tuning::default());
                }
                p.unwrap()
            };
            // tucked in just after the start, fully out a little past the middle
            let start = at(total * 0.86, &mut st, 1.0);
            let end = at(total * 0.38, &mut st, 2.0);
            let back = at(0.001, &mut st, 3.0);
            assert!((back.hand - end.hand).len() > 0.15, "fan {fan}: and back to the ready hold");
            let chest = (start.hit.caps[3].a + start.hit.caps[3].b) * 0.5;
            // starts tucked in near the middle of the chest, ends out to the right and in front
            assert!((start.hand - chest).dot(right) < 0.12, "fan {fan}: starts across the chest: {:?} vs chest {:?}", start.hand, chest);
            assert!(end.hand.dot(right) > start.hand.dot(right) + 0.2, "fan {fan}: the arm goes outwards: {:?} -> {:?}", start.hand, end.hand);
            assert!(end.hand.z > chest.z + 0.2, "fan {fan}: and forwards: hand z {} chest z {}", end.hand.z, chest.z);
            // the hit shapes follow the arm (stars leave from that hand)
            assert!((end.hit.caps[8].b - end.hand).len() < 0.05);
        }
    }
}
