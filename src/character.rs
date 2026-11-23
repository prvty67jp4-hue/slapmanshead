//! Animation controller: picks and cross-fades clips for each fighter from what it is doing,
//! then layers aiming, strafing, guard, sword swings and weapon holds on top.

use crate::anim::{Rig, Trs, blend_into, blend_masked};
use crate::game::{Capsule, FAN_THROW_TIME, Fighter, HitPose, KICK_TIME, Part, RUN_SPEED, SLASH_HEAVY, SLASH_SLAM, SLASH_THROUGH, THROW_TIME, Tuning, Weapon, finisher};
use crate::math::{M3, V3, look_dir, v3, wrap_angle};
use crate::render::{finisher_arm, finisher_body, sword_arm};
use glam::{Mat3, Mat4, Quat, Vec3};
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
    /// Stop the clip here (a reaction only plays its first part).
    hold: f32,
}

impl Play {
    fn new(clip: &'static str, rate: f32, looped: bool) -> Play {
        Play { clip, t: 0.0, rate, looped, hold: f32::MAX }
    }
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
    /// Recent blade positions (base, tip, time, brightness) for the swing trail.
    pub trail: VecDeque<(V3, V3, f32, f32)>,
    /// A swing that just ended keeps going a moment (its follow-through and recovery).
    tail: Option<Tail>,
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
            cur: Play::new("Idle_Loop", 1.0, true),
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
            tail: None,
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
    /// Where a blade is anchored (its handle's middle; the fist sits near the guard). The hand
    /// for guns and in the game's own poses.
    pub grip: V3,
    /// The right hand, from its bones: across the knuckles (little finger to index), and from
    /// the wrist towards the knuckles; the forearm (elbow to wrist); how far the index
    /// fingertip is from the wrist (small when the fist is closed).
    pub knuckles: V3,
    pub hand_axis: V3,
    pub forearm: V3,
    pub fingertip: f32,
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
    index1_r: usize,
    index3_r: usize,
    middle1_r: usize,
    middle3_r: usize,
    pinky1_r: usize,
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
        index1_r: rig.node("index_01_r")?,
        index3_r: rig.node("index_03_r")?,
        middle1_r: rig.node("middle_01_r")?,
        middle3_r: rig.node("middle_03_r")?,
        pinky1_r: rig.node("pinky_01_r")?,
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
    animate_with(rig, st, f, pos, now, tn, false, false)
}

/// The picture: the same, but melee moves, blocks, staggers and knockbacks play real animation
/// clips (CC0 Quaternius), timed so each blow's contact frame lands exactly when the game's hit
/// does. The game keeps posing its hit shapes with `animate`, so timing, reach and hitboxes
/// don't change.
pub fn animate_show(rig: &Rig, st: &mut CharState, f: &Fighter, pos: V3, now: f32, tn: &Tuning) -> Option<Posed> {
    animate_with(rig, st, f, pos, now, tn, true, false)
}

/// The picture, optionally in the fighter's hero pose (the menu and the loadout screen): each
/// fighter's own stance, weight on one leg, breathing, the blade held their way.
pub fn animate_show_as(rig: &Rig, st: &mut CharState, f: &Fighter, pos: V3, now: f32, tn: &Tuning, hero: bool) -> Option<Posed> {
    animate_with(rig, st, f, pos, now, tn, true, hero)
}

/// How the blade leaves a closed fist: from the knuckle line (little finger to index) turned
/// this far (radians) towards the hand's own axis, so it runs on from the forearm the way a
/// sword held in a firm fist does, out on the thumb side.
pub const GRIP_ANGLE: f32 = 0.87;

/// The blade's direction and edge from the hand's knuckle line `a` and wrist-to-knuckles axis.
pub fn grip_axes(a: V3, hand_axis: V3) -> (V3, V3) {
    let a = a.norm();
    let h = (hand_axis - a * hand_axis.dot(a)).norm();
    let (c, s) = (GRIP_ANGLE.cos(), GRIP_ANGLE.sin());
    ((a * c + h * s).norm(), (h * c - a * s).norm())
}

/// The right hand's frame in our space: knuckle line, wrist-to-knuckles axis (square to it).
fn hand_frame(rig: &Rig, b: &Bones, pose: &[Trs]) -> (Vec3, Vec3, Vec3) {
    let g = rig.globals(pose);
    let p = |n: usize| g[n].w_axis.truncate() * rig.fix_sign;
    let a = (p(b.index1_r) - p(b.pinky1_r)).normalize_or(Vec3::Z);
    let h0 = (p(b.middle1_r) - p(b.hand_r)).normalize_or(Vec3::NEG_Y);
    let h = (h0 - a * h0.dot(a)).normalize_or(h0);
    (a, h, p(b.hand_r))
}

/// Turn the right hand (at the wrist) so the blade it holds points along `blade`, with its edge
/// towards `edge` when given (otherwise the smallest turn).
fn aim_hand(rig: &Rig, b: &Bones, pose: &mut [Trs], blade: Vec3, edge: Vec3) {
    let (a, h, _) = hand_frame(rig, b, pose);
    let (c, s) = (GRIP_ANGLE.cos(), GRIP_ANGLE.sin());
    let cb = (a * c + h * s).normalize();
    let ce = (h * c - a * s).normalize();
    let blade = blade.normalize_or(cb);
    let e = edge - blade * edge.dot(blade);
    let r = if edge.length_squared() > 1e-6 && e.length() > 0.1 {
        let e = e.normalize();
        Mat3::from_cols(blade, e, blade.cross(e)) * Mat3::from_cols(cb, ce, cb.cross(ce)).transpose()
    } else {
        Mat3::from_quat(Quat::from_rotation_arc(cb, blade))
    };
    let sm = Mat3::from_diagonal(rig.fix_sign);
    let q = Quat::from_mat3(&(sm * r * sm)).normalize();
    let g = rig.globals(pose);
    rig.rotate_global(pose, &g, b.hand_r, q);
}

/// Both hands closed into fists (the idle clip's fists): one round the handle, the other relaxed.
fn close_fists(rig: &Rig, pose: &mut [Trs], scratch: &mut Vec<Trs>) {
    rig.sample("Idle_Loop", 0.5, true, scratch);
    for side in ["r", "l"] {
        for finger in ["thumb", "index", "middle", "ring", "pinky"] {
            for k in 1..=3 {
                if let Some(n) = rig.node(&format!("{finger}_0{k}_{side}")) {
                    pose[n].r = scratch[n].r;
                }
            }
        }
    }
}

/// A fighter's hero pose for the menu and the loadout screen, on top of the idle clip: their
/// stance (weight on one leg, feet planted), a breath through the chest, and both arms placed
/// their way. Returns the blade's direction and edge (our space).
fn hero_pose(rig: &Rig, b: &Bones, pose: &mut Vec<Trs>, f: &Fighter, now: f32) -> Option<(Vec3, Vec3)> {
    let s = rig.fix_sign;
    let gpos = |pose: &Vec<Trs>, n: usize| rig.globals(pose)[n].w_axis.truncate() * s;
    let body = f.loadout.body;
    let great = f.weapon == Weapon::Greatsword;
    // (pelvis shift, hip roll, left foot, right foot, chest turn, bounce)
    let (shift, roll, foot_l, foot_r, twist, bounce) = match body {
        // Ranger: calm, weight settled on the left leg
        0 => (Vec3::new(-0.035, -0.012, 0.0), -0.05, Vec3::ZERO, Vec3::new(0.04, 0.0, 0.07), 0.06, 0.0),
        // Huntress: poised, low, feet split front and back, chest turned
        1 => (Vec3::new(0.01, -0.045, 0.0), 0.03, Vec3::new(-0.03, 0.0, 0.13), Vec3::new(0.05, 0.0, -0.09), 0.18, 0.003),
        // Brawler: planted wide, chest out
        2 => (Vec3::new(0.0, -0.02, 0.0), -0.02, Vec3::new(-0.08, 0.0, 0.02), Vec3::new(0.08, 0.0, -0.02), -0.06, 0.0),
        // Striker: light on the feet, low and ready, bouncing a little
        _ => (Vec3::new(0.01, -0.07, 0.02), 0.0, Vec3::new(-0.03, 0.0, 0.16), Vec3::new(0.04, 0.0, -0.1), 0.25, 0.012),
    };
    let breathe = (now * 1.7).sin();
    let g = rig.globals(pose);
    let rot = |n: usize| g[n].to_scale_rotation_translation().1;
    let (rot_fl, rot_fr) = (rot(b.foot_l), rot(b.foot_r));
    let fl = gpos(pose, b.foot_l) + foot_l;
    let fr = gpos(pose, b.foot_r) + foot_r;
    // the pelvis, in its parent's space
    let pel = g[b.pelvis].w_axis.truncate();
    let moved = pel + (shift + Vec3::Y * (bounce * (now * 3.2).sin().abs())) * s;
    let parent = rig.parent_of(b.pelvis).map_or(Mat4::IDENTITY, |p| g[p]);
    pose[b.pelvis].t = parent.inverse().transform_point3(moved);
    let turn = |pose: &mut Vec<Trs>, n: usize, axis: Vec3, a: f32| {
        let g = rig.globals(pose);
        rig.rotate_global(pose, &g, n, ours(rig, axis, a));
    };
    turn(pose, b.pelvis, Vec3::Z, roll);
    for bone in [b.spine2, b.spine3] {
        turn(pose, bone, Vec3::Z, -roll * 0.6);
        turn(pose, bone, Vec3::Y, twist * 0.5);
    }
    // chest up, and a slow breath
    turn(pose, b.spine3, Vec3::X, -0.05 - 0.018 * breathe);
    turn(pose, b.head, Vec3::X, 0.03 + 0.01 * breathe);
    turn(pose, b.head, Vec3::Y, -twist * 0.6);
    // feet back on the ground, knees forward, soles as they were
    for (th, ca, ft, target, keep) in [(b.thigh_l, b.calf_l, b.foot_l, fl, rot_fl), (b.thigh_r, b.calf_r, b.foot_r, fr, rot_fr)] {
        let knee = gpos(pose, ca);
        rig.two_bone_ik(pose, th, ca, ft, target * s, (knee + Vec3::new(0.0, 0.0, 0.6)) * s);
        let g = rig.globals(pose);
        let cur = g[ft].to_scale_rotation_translation().1;
        rig.rotate_global(pose, &g, ft, (keep * cur.inverse()).normalize());
    }
    // arms
    let sh_r = gpos(pose, b.up_r);
    let sh_l = gpos(pose, b.up_l);
    let chest = (sh_r + sh_l) * 0.5;
    let v = Vec3::new;
    let shoulder_rest = great || body == 2;
    let (hr, pr, blade, edge) = if shoulder_rest {
        // the blade resting back over the right shoulder, the fist in front of it
        let hr = sh_r + v(-0.06, -0.16, 0.2);
        (hr, sh_r + v(0.15, -0.8, 0.35), (sh_r + v(0.05, 0.14, -0.12) - hr).normalize(), v(1.0, 0.0, 0.0))
    } else {
        match body {
            // lowered at the side, point down and forward
            0 => (sh_r + v(0.07, -0.47, 0.16), sh_r + v(0.35, -0.25, -0.4), v(0.22, -0.5, 0.84), v(0.0, -0.4, 1.0)),
            // low in front, point up at the enemy
            1 => (sh_r + v(-0.06, -0.4, 0.3), sh_r + v(0.35, -0.35, -0.2), v(-0.2, 0.32, 0.93), v(0.0, -1.0, 0.0)),
            // ready: at the waist, angled up across the body
            _ => (sh_r + v(-0.04, -0.34, 0.32), sh_r + v(0.3, -0.4, -0.2), v(-0.35, 0.5, 0.79), v(0.0, -1.0, 0.0)),
        }
    };
    let (hl, pl) = match body {
        // relaxed fist at the side
        0 => (sh_l + v(-0.05, -0.5, 0.05), sh_l + v(-0.3, -0.3, -0.4)),
        // a loose fist low in front of the hip
        1 => (sh_l + v(0.06, -0.45, 0.16), sh_l + v(-0.35, -0.3, -0.3)),
        // fist on the hip, elbow out
        2 => (sh_l + v(-0.1, -0.42, -0.03), sh_l + v(-0.6, -0.2, -0.1)),
        // a guard: left fist up in front of the chest
        _ => (chest + v(-0.06, -0.1, 0.3), sh_l + v(-0.3, -0.5, 0.0)),
    };
    rig.two_bone_ik(pose, b.up_r, b.lo_r, b.hand_r, hr * s, pr * s);
    rig.two_bone_ik(pose, b.up_l, b.lo_l, b.hand_l, hl * s, pl * s);
    Some((blade, edge))
}

fn animate_with(rig: &Rig, st: &mut CharState, f: &Fighter, pos: V3, now: f32, tn: &Tuning, show: bool, hero: bool) -> Option<Posed> {
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
    // the hero pose: standing still with a blade, in the menu or on the loadout screen
    let hero = hero && show && melee && f.alive && f.on_ground && f.stun_t <= 0.0 && speed < 0.6 && f.slash_t <= 0.0 && f.fin == 0 && !f.guard;
    let mut target_yaw = 0.0;
    let want: Play = if !f.alive {
        Play::new("Death01", 1.0, false)
    } else if f.stun_t > 0.0 {
        if show && (f.stagger_t > 0.0 || f.guard_broke > 0.0) {
            // knocked off balance (parried, guard broken, kicked): a real stagger
            Play::new("Idle_Shield_Break", 1.25, false)
        } else if show && !f.on_ground {
            // launched or juggled: thrown back, arms flung out (the fall's first part only)
            Play { hold: 0.34, ..Play::new("Hit_Knockback", 1.0, false) }
        } else {
            Play::new("Hit_Head", 1.4, false)
        }
    } else if let Some(w) = f.wall {
        Play::new("Sprint_Loop", if w.vertical { 1.5 } else { 1.3 }, true)
    } else if f.sliding > 0.0 && f.on_ground {
        // sliding: low crouch (leaned back below)
        Play::new("Crouch_Idle_Loop", 1.0, true)
    } else if dashing && f.on_ground || (dashing && f.vel.y.abs() < 3.0 && !f.air_dash) {
        Play::new("Roll", 2.0, false)
    } else if !f.on_ground {
        if f.vel.y > 1.5 { Play::new("Jump_Start", 1.0, false) } else { Play::new("Jump_Loop", 1.0, true) }
    } else if speed > 0.6 {
        let a = local_v.x.atan2(local_v.z);
        let back = a.abs() > 1.85;
        let yaw = if back { wrap_angle(a - PI) } else { a };
        target_yaw = yaw.clamp(-1.25, 1.25) * 0.85;
        let sprint = speed > RUN_SPEED * 1.15;
        let rate = if sprint { (speed / 8.0).clamp(0.8, 1.6) } else { (speed / 4.6).clamp(0.7, 1.8) };
        Play::new(if sprint { "Sprint_Loop" } else { "Jog_Fwd_Loop" }, if back { -rate } else { rate }, true)
    } else if hero {
        Play::new("Idle_Loop", 1.0, true)
    } else if melee {
        Play::new("Sword_Idle", 1.0, true)
    } else if matches!(f.weapon, Weapon::Revolver | Weapon::HandCannon) {
        Play::new("Pistol_Idle_Loop", 1.0, true)
    } else {
        Play::new("Idle_Loop", 1.0, true)
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
    rig.sample(st.cur.clip, st.cur.t.min(st.cur.hold), st.cur.looped, &mut pose);
    if let Some(p) = &st.prev {
        rig.sample(p.clip, p.t.min(p.hold), p.looped, &mut st.scratch);
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
        // the picture: a head hit snaps the head back, anything else rocks the chest
        let (clip, len) = if show && Part::from_u8(f.hit_part) == Part::Head { ("Hit_Head", 0.43) } else { ("Hit_Chest", 0.35) };
        rig.sample(clip, (1.0 - k) * len, false, &mut st.scratch2);
        let tmp = std::mem::take(&mut st.scratch2);
        blend_masked(rig, &mut pose, &tmp, k * if show { 0.95 } else { 0.8 }, b.spine1);
        st.scratch2 = tmp;
    }
    // ---- melee clips (the picture only): swings, finishers, the heavy's wind-up, the block,
    // and each swing's follow-through and recovery after it
    let mut clip_arm = false;
    if show && f.alive && f.stun_t <= 0.0 && melee && !hero {
        if let Some((mp, w, full)) = melee_layer(rig, st, f, dt) {
            if full && f.on_ground && speed < 2.5 {
                blend_into(&mut pose, &mp, w);
            } else {
                blend_masked(rig, &mut pose, &mp, w, b.spine1);
            }
            clip_arm = w > 0.5;
            // a charged heavy trembles with held power
            if f.charged && f.slash_t <= 0.0 {
                let g = rig.globals(&pose);
                rig.rotate_global(&mut pose, &g, b.spine3, ours(rig, Vec3::Y, (now * 47.0).sin() * 0.025));
            }
        }
    } else if show {
        st.tail = None;
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
    // a clip holds the sword arm: no hand-built swing on top of it
    let clip_drives = show && clip_arm;
    if alive_upper && clip_drives {
        // the blade leaves the fist on the index-finger side
        if let (Some(ix), Some(pk)) = (rig.node("index_01_r"), rig.node("pinky_01_r")) {
            let d = gpos(&pose, ix) - gpos(&pose, pk);
            if d.length() > 1e-4 {
                weapon_dir = Some(d.normalize());
            }
        }
    } else if alive_upper {
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

    // ---- the picture's grip: fists closed, and the sword hand turned so the blade leaves the fist
    // where the swing (or the stance) wants it; the greatsword's other hand joins on the handle
    if show && melee && f.alive {
        let hero_blade = if hero { hero_pose(rig, &b, &mut pose, f, now) } else { None };
        close_fists(rig, &mut pose, &mut st.scratch2);
        // in a clip-driven swing the clip's hand is the grip; otherwise aim the hand
        let want = if clip_drives {
            None
        } else if let Some(hb) = hero_blade {
            Some(hb)
        } else if let Some(d) = weapon_dir {
            Some((d, Vec3::ZERO))
        } else if f.on_ground && speed < 2.5 && f.stun_t <= 0.0 {
            // held ready: the arm low and forward, the point forward and a little out
            let sh = gpos(&pose, b.up_r);
            rig.two_bone_ik(&mut pose, b.up_r, b.lo_r, b.hand_r, to_model(sh + Vec3::new(0.06, -0.4, 0.3)), to_model(sh + Vec3::new(0.35, -0.3, -0.4)));
            Some((Vec3::new(0.25, -0.38, 0.89), Vec3::new(0.0, -1.0, 0.3)))
        } else {
            // on the move: the clip's own hand
            None
        };
        if let Some((blade, edge)) = want {
            aim_hand(rig, &b, &mut pose, blade, edge);
        }
        let two_hands = f.weapon == Weapon::Greatsword && hero_blade.is_none() && !clip_drives && f.slash_t <= 0.0 && f.fin == 0 && !f.guard && f.on_ground && speed < 2.5 && f.stun_t <= 0.0;
        if two_hands {
            let (a, h, wrist) = hand_frame(rig, &b, &pose);
            let (blade, _) = grip_axes(ov(a), ov(h));
            let target = wrist + h * 0.07 - gv(blade) * 0.13;
            let pole = gpos(&pose, b.up_l) + Vec3::new(-0.4, -0.4, -0.2);
            rig.two_bone_ik(&mut pose, b.up_l, b.lo_l, b.hand_l, to_model(target), to_model(pole));
        }
    }

    // ---- kick: the right leg snaps forward and back
    if show && f.alive && f.kick_t > 0.0 {
        show_kick(rig, &b, &mut pose, 1.0 - (f.kick_t / KICK_TIME).clamp(0.0, 1.0));
    } else if f.alive && f.kick_t > 0.0 {
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
    let knuckles = (wp(b.index1_r) - wp(b.pinky1_r)).norm();
    let hand_axis = (wp(b.middle1_r) - hand).norm();
    let fingertip = (wp(b.index3_r) - hand).len() / (scale.clamp(0.2, 4.0) * norm).max(0.01);
    let (x, y, z, grip) = if show && melee && f.alive {
        // the blade comes out of the closed fist; its handle's middle sits just below the fist
        let (z, y) = grip_axes(knuckles, hand_axis);
        let fist = hand * 0.35 + wp(b.middle1_r) * 0.35 + wp(b.middle3_r) * 0.3;
        (y.cross(z).norm(), y, z, fist - z * (0.05 * scale.clamp(0.2, 4.0)))
    } else {
        let z = match weapon_dir {
            Some(d) => to_world_dir(d),
            None => (hand - elbow).norm(),
        };
        let up_hint = if z.y.abs() > 0.9 { yaw_fwd_of(f.yaw) } else { v3(0.0, 1.0, 0.0) };
        let x = up_hint.cross(z).norm();
        (x, z.cross(x).norm(), z, hand)
    };
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
    Some(Posed { palette, hand, weapon: M3 { x, y, z }, head, head_frame: M3 { x: hx, y: hy, z: hz }, waist: wp(b.spine1), hit, grip, knuckles, hand_axis, forearm: (hand - elbow).norm(), fingertip })
}
// ---------------------------------------------------------------------------------- melee clips

/// The game's swing: the blow lands when this share of the swing time is left.
const HIT_LEFT: f32 = 0.72;
/// How long a finished swing's follow-through and recovery keep showing.
const TAIL: f32 = 0.36;

/// A melee move made from a clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mv {
    A,
    B,
    C,
    HeavyFirst,
    HeavyMid,
    Slam,
    Dash,
    Hook,
    Combo2,
    Combo3,
}
const MOVES: [Mv; 10] = [Mv::A, Mv::B, Mv::C, Mv::HeavyFirst, Mv::HeavyMid, Mv::Slam, Mv::Dash, Mv::Hook, Mv::Combo2, Mv::Combo3];

struct MoveDef {
    clip: &'static str,
    /// Where the wind-up starts, where (around) the blade is fastest, and where it ends.
    start: f32,
    window: (f32, f32),
    end: f32,
    /// The pack's recovery back to the ready stance, played after.
    rec: Option<&'static str>,
}

fn def(m: Mv) -> MoveDef {
    let d = |clip, start, window, end, rec| MoveDef { clip, start, window, end, rec };
    match m {
        Mv::A => d("Sword_Regular_A", 0.0, (0.1, 0.4), 0.43, Some("Sword_Regular_A_Rec")),
        Mv::B => d("Sword_Regular_B", 0.0, (0.1, 0.5), 0.53, Some("Sword_Regular_B_Rec")),
        Mv::C => d("Sword_Regular_C", 0.42, (0.45, 0.8), 1.05, None),
        Mv::HeavyFirst => d("Sword_Heavy_Combo", 0.05, (0.25, 0.6), 0.85, None),
        Mv::HeavyMid => d("Sword_Heavy_Combo", 1.45, (1.6, 2.0), 2.15, None),
        Mv::Slam => d("Sword_Heavy_Combo", 2.25, (2.4, 2.8), 3.05, None),
        Mv::Dash => d("Sword_Dash", 0.1, (0.2, 0.45), 0.75, None),
        Mv::Hook => d("Melee_Hook", 0.0, (0.15, 0.35), 0.47, Some("Melee_Hook_Rec")),
        Mv::Combo2 => d("Sword_Regular_Combo", 0.0, (0.6, 0.85), 1.0, None),
        Mv::Combo3 => d("Sword_Regular_Combo", 0.0, (1.4, 1.7), 1.9, None),
    }
}

/// Where in each move's clip the blade is fastest (its contact frame), and where the hand
/// pauses at the top of the backswing before it (the cocked pose), found once from the clips.
fn timings(rig: &Rig) -> &'static [(f32, f32); 10] {
    static T: std::sync::OnceLock<[(f32, f32); 10]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut out = [(0.0, 0.0); 10];
        let Some(hand) = rig.node("hand_r") else { return out };
        let mut pose = Vec::new();
        for (k, m) in MOVES.iter().enumerate() {
            let d = def(*m);
            let step = 1.0 / 120.0;
            let mut prev: Option<Vec3> = None;
            let mut best = (d.window.0, -1.0f32);
            let mut speeds = Vec::new();
            let mut t = d.start;
            while t <= d.window.1 + 1e-4 {
                rig.sample(d.clip, t, false, &mut pose);
                let h = rig.globals(&pose)[hand].w_axis.truncate();
                if let Some(ph) = prev {
                    let v = (h - ph).length() / step;
                    speeds.push((t, v));
                    if t >= d.window.0 && v > best.1 {
                        best = (t, v);
                    }
                }
                prev = Some(h);
                t += step;
            }
            let contact = best.0;
            let cocked = speeds.iter().filter(|(t, _)| *t < contact - 0.05).fold((d.start, f32::MAX), |m, s| if s.1 < m.1 { (s.0, s.1) } else { m }).0;
            out[k] = (contact, cocked.min(contact));
        }
        out
    })
}

fn contact_of(rig: &Rig, m: Mv) -> f32 {
    timings(rig)[MOVES.iter().position(|x| *x == m).unwrap_or(0)].0
}
fn cocked_of(rig: &Rig, m: Mv) -> f32 {
    timings(rig)[MOVES.iter().position(|x| *x == m).unwrap_or(0)].1
}

/// Which move a swing plays.
fn slash_move(idx: u8, w: Weapon) -> Mv {
    match idx {
        0 if w == Weapon::Dagger => Mv::Hook,
        0 => Mv::A,
        1 => Mv::B,
        2 | SLASH_SLAM => Mv::Slam,
        3 => Mv::C,
        SLASH_HEAVY if w == Weapon::Greatsword => Mv::HeavyMid,
        SLASH_HEAVY => Mv::HeavyFirst,
        SLASH_THROUGH => Mv::Dash,
        _ => Mv::A,
    }
}

/// Each finisher style's move (and where its wind-up starts).
fn finisher_move(style: usize) -> (Mv, Option<f32>) {
    match style {
        0 => (Mv::Hook, None),         // Throat Cut
        1 => (Mv::Combo2, None),       // Twin Stab: two stabs, the second kills
        2 => (Mv::C, Some(0.0)),       // Spinning Edge: the whole spin
        3 => (Mv::Dash, Some(0.0)),    // Quick Draw
        4 => (Mv::Combo3, None),       // Cross Cut: three cuts
        5 => (Mv::A, None),            // Rising Cleave
        6 => (Mv::Slam, None),         // Overhead Crush
        7 => (Mv::HeavyFirst, None),   // Impale
        8 => (Mv::HeavyMid, None),     // Wheel Cleave
        _ => (Mv::Slam, Some(2.05)),   // Sky Splitter: from high overhead
    }
}

/// The clip time for a move, `until` game seconds before its contact (negative after), with
/// `pre` seconds of wind-up and `post` of follow-through. The wind-up lingers at the start (so it
/// reads) and accelerates into the blow; the follow-through decelerates.
fn warp(rig: &Rig, m: Mv, start: Option<f32>, until: f32, pre: f32, post: f32) -> f32 {
    let d = def(m);
    let contact = contact_of(rig, m);
    let start = start.unwrap_or(d.start).min(contact);
    if until > 0.0 {
        let u = 1.0 - (until / pre.max(1e-3)).min(1.0);
        start + (contact - start) * u.powf(1.6)
    } else {
        let v = (-until / post.max(1e-3)).min(1.0);
        contact + (d.end - contact) * (1.0 - (1.0 - v) * (1.0 - v))
    }
}

#[derive(Clone, Copy, Debug)]
struct Tail {
    mv: Mv,
    /// Clip time when the swing ended.
    t: f32,
    age: f32,
}

/// The melee clip pose to lay over the body now, its weight, and whether the legs may follow.
fn melee_layer(rig: &Rig, st: &mut CharState, f: &Fighter, dt: f32) -> Option<(Vec<Trs>, f32, bool)> {
    let mut pose = Vec::new();
    let dur = f.weapon.slash_dur();
    if f.fin > 0 {
        let style = finisher(f.fin);
        let (m, start) = finisher_move(f.fin as usize - 1);
        let pr = (f.fin_t / style.dur).clamp(0.0, 1.0);
        let until = (style.hit - pr) * style.dur;
        let t = warp(rig, m, start, until, style.hit * style.dur, (1.0 - style.hit) * style.dur);
        rig.sample(def(m).clip, t, false, &mut pose);
        st.tail = Some(Tail { mv: m, t, age: 0.0 });
        return Some((pose, 1.0, true));
    }
    if f.slash_t > 0.0 {
        let m = slash_move(f.slash_idx, f.weapon);
        let until = f.slash_t - dur * HIT_LEFT;
        let pre = if f.slash_idx == SLASH_HEAVY { dur * (1.25 - HIT_LEFT) } else { dur * (1.0 - HIT_LEFT) };
        let start = if f.slash_idx == SLASH_HEAVY { Some(cocked_of(rig, m)) } else { None };
        let t = warp(rig, m, start, until, pre, dur * HIT_LEFT);
        rig.sample(def(m).clip, t, false, &mut pose);
        st.tail = Some(Tail { mv: m, t, age: 0.0 });
        let full = matches!(f.slash_idx, SLASH_HEAVY | SLASH_THROUGH) || (f.slash_idx <= 1 && f.on_ground);
        return Some((pose, 1.0, full));
    }
    // charging a heavy: draw back into the heavy's cocked pose and hold it (easy to read)
    if f.charge_t > 0.15 {
        let m = if f.weapon == Weapon::Greatsword { Mv::HeavyMid } else { Mv::HeavyFirst };
        let w = ((f.charge_t - 0.15) / 0.25).clamp(0.0, 1.0);
        let w = w * w * (3.0 - 2.0 * w);
        rig.sample(def(m).clip, cocked_of(rig, m), false, &mut pose);
        st.tail = None;
        return Some((pose, w, false));
    }
    // the block: the guard snaps up (the raise is the parry's moment), then holds
    if f.guard {
        rig.sample("Sword_Block", (f.guard_t * 1.35).min(0.38), false, &mut pose);
        st.tail = None;
        return Some((pose, 1.0, false));
    }
    // after a swing: carry through, then settle back into the stance
    if let Some(tl) = st.tail.as_mut() {
        tl.age += dt;
        if tl.age >= TAIL {
            st.tail = None;
            return None;
        }
        let tl = *tl;
        let d = def(tl.mv);
        match d.rec {
            Some(rec) => rig.sample(rec, tl.age * 1.4, false, &mut pose),
            None => rig.sample(d.clip, (tl.t + tl.age).min(d.end + 0.3), false, &mut pose),
        }
        let k = 1.0 - tl.age / TAIL;
        return Some((pose, k * k * (3.0 - 2.0 * k), false));
    }
    None
}

/// The kick in the picture: the knee chambers up (the tell), the leg snaps out to land exactly
/// when the game's kick does, holds a beat, then recoils and steps down; the body leans back and
/// the arms swing to balance.
fn show_kick(rig: &Rig, b: &Bones, pose: &mut Vec<Trs>, p: f32) {
    let hit = crate::game::KICK_HIT_AT / KICK_TIME;
    let smooth = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    // chamber (knee up, foot tucked), extend (straight leg), recoil (tucked again), step down
    let chamber = smooth(p / (hit * 0.55));
    let extend = smooth((p - hit * 0.55) / (hit * 0.45));
    let recoil = smooth((p - hit - 0.12) / 0.25);
    let down = smooth((p - hit - 0.35) / (1.0 - hit - 0.35));
    let thigh = -1.05 * chamber - 0.65 * extend + 0.3 * recoil;
    let thigh = thigh * (1.0 - down);
    let calf = (1.7 * chamber - 1.95 * extend + 1.5 * recoil) * (1.0 - down);
    let lean = (0.12 * chamber + 0.22 * extend - 0.1 * recoil) * (1.0 - down);
    let turn = |pose: &mut Vec<Trs>, bone: usize, axis: Vec3, a: f32| {
        let g = rig.globals(pose);
        rig.rotate_global(pose, &g, bone, ours(rig, axis, a));
    };
    turn(pose, b.thigh_r, Vec3::X, thigh);
    turn(pose, b.calf_r, Vec3::X, calf.max(-0.3));
    turn(pose, b.spine1, Vec3::X, lean);
    // the arms counterbalance: out to the sides and back as the leg goes out
    let arms = (chamber * 0.6 + extend * 0.4) * (1.0 - down);
    turn(pose, b.up_l, Vec3::Z, -0.5 * arms);
    turn(pose, b.up_r, Vec3::Z, 0.35 * arms);
}

#[cfg(test)]
mod melee_clip_tests {
    use super::*;

    #[test]
    fn every_move_has_its_clip_and_its_blow_lands_on_the_games_hit() {
        let rig = crate::anim::shared().expect("rig");
        for m in MOVES {
            let d = def(m);
            assert!(rig.clips.contains_key(d.clip), "{m:?}: clip {} is loaded", d.clip);
            if let Some(r) = d.rec {
                assert!(rig.clips.contains_key(r), "{m:?}: recovery {r} is loaded");
            }
            let c = contact_of(rig, m);
            assert!(c >= d.window.0 - 1e-3 && c <= d.window.1 + 1e-3, "{m:?}: contact {c} in its window");
            assert!(cocked_of(rig, m) <= c, "{m:?}: the wind-up comes first");
            // at the game's hit moment the clip is exactly at its contact frame
            assert!((warp(rig, m, None, 0.0, 0.1, 0.26) - c).abs() < 1e-4);
            // before it, the wind-up; after it, the follow-through, both in order
            assert!(warp(rig, m, None, 0.08, 0.1, 0.26) < warp(rig, m, None, 0.02, 0.1, 0.26));
            assert!(warp(rig, m, None, -0.05, 0.1, 0.26) < warp(rig, m, None, -0.2, 0.1, 0.26));
            assert!(warp(rig, m, None, -0.26, 0.1, 0.26) <= d.end + 1e-4);
        }
        for k in ["Sword_Block", "Hit_Knockback", "Idle_Shield_Break", "Hit_Chest", "Hit_Head"] {
            assert!(rig.clips.contains_key(k), "{k} is loaded");
        }
    }

    #[test]
    fn the_picture_never_moves_the_hit_shapes() {
        // the game poses its hit shapes without the clips: a swing, a block, a charge or a kick
        // in the picture leaves them exactly as before
        let rig = crate::anim::shared().expect("rig");
        let tn = Tuning::default();
        let mut f = Fighter::new("t", 0, true);
        f.present = true;
        f.alive = true;
        f.on_ground = true;
        f.weapon = Weapon::Katana;
        let cases: [(&str, fn(&mut Fighter)); 5] = [
            ("swing", |f| {
                f.slash_idx = 1;
                f.slash_t = 0.2;
            }),
            ("heavy", |f| {
                f.slash_idx = SLASH_HEAVY;
                f.slash_t = 0.4;
            }),
            ("guard", |f| {
                f.guard = true;
                f.guard_t = 0.1;
            }),
            ("charge", |f| f.charge_t = 0.4),
            ("kick", |f| f.kick_t = 0.25),
        ];
        for (name, set) in cases {
            let mut g = f.clone();
            set(&mut g);
            let (mut a, mut b, mut c) = (CharState::new(), CharState::new(), CharState::new());
            let mut hits = None;
            let mut shows = None;
            let mut again = None;
            for k in 0..6 {
                let t = k as f32 / 60.0;
                hits = animate(rig, &mut a, &g, V3::ZERO, t, &tn).map(|p| p.hit);
                shows = animate_show(rig, &mut b, &g, V3::ZERO, t, &tn).map(|p| p.hit);
                again = animate(rig, &mut c, &g, V3::ZERO, t, &tn).map(|p| p.hit);
            }
            let (h, a2) = (hits.expect("posed"), again.expect("posed"));
            for (x, y) in h.caps.iter().zip(a2.caps.iter()) {
                assert!((x.a - y.a).len() < 1e-5 && (x.b - y.b).len() < 1e-5, "{name}: hit shapes are steady");
            }
            // and the picture really does differ (the clips are in use)
            let s = shows.expect("posed");
            let moved = h.caps.iter().zip(s.caps.iter()).any(|(x, y)| (x.b - y.b).len() > 0.02);
            assert!(moved, "{name}: the picture plays the clip");
        }
    }
}
fn yaw_fwd_of(yaw: f32) -> V3 {
    v3(yaw.sin(), 0.0, yaw.cos())
}

#[cfg(test)]
mod pose_tests {
    use super::*;

    /// The sword sits in a closed fist and points where the hand says, in every state the
    /// picture shows: held ready in a match, each fighter's hero pose, guarding and mid-swing.
    /// (Guards against the grip breaking again: blade through the wrist, open hands, a blade
    /// across the forearm or into the floor.)
    #[test]
    fn sword_grip_follows_the_hand() {
        let rig = Rig::load().unwrap();
        for body in 0..crate::chars::COUNT as u8 {
            for w in Weapon::MELEE {
                for state in ["ready", "hero", "guard", "slash"] {
                    let mut f = Fighter::new("t", 0, true);
                    f.present = true;
                    f.alive = true;
                    f.on_ground = true;
                    f.loadout.body = body;
                    f.loadout.melee = w;
                    f.cur = 0;
                    f.weapon = w;
                    let mut st = CharState::new();
                    let mut p = None;
                    for k in 0..40 {
                        let mut g = f.clone();
                        match state {
                            "guard" => g.guard = true,
                            "slash" => {
                                g.slash_idx = 0;
                                g.slash_t = w.slash_dur() * (1.0 - (k as f32 / 40.0) * 0.6);
                            }
                            _ => {}
                        }
                        p = animate_show_as(&rig, &mut st, &g, V3::ZERO, k as f32 * 0.016, &Tuning::default(), state == "hero");
                    }
                    let p = p.expect("posed");
                    let what = format!("{} {:?} {state}", crate::chars::name(body), w);
                    // the blade leaves the fist at the grip's angle to the hand bones
                    let (blade, edge) = grip_axes(p.knuckles, p.hand_axis);
                    assert!(p.weapon.z.dot(blade) > 0.999, "{what}: blade {:?} vs hand {:?}", p.weapon.z, blade);
                    assert!(p.weapon.y.dot(edge) > 0.99, "{what}: the edge follows the knuckles");
                    // never back through the wrist or square across the forearm (resting on the
                    // shoulder, the blade lies back and up over it instead)
                    let along = p.weapon.z.dot(p.forearm);
                    let shoulder = state == "hero" && (w == Weapon::Greatsword || body == 2);
                    if shoulder {
                        assert!(p.weapon.z.y > 0.3 && p.weapon.z.dot(f.fwd()) < 0.0, "{what}: back over the shoulder: {:?}", p.weapon.z);
                    } else {
                        assert!(along > 0.2, "{what}: blade vs forearm {along:.2}");
                    }
                    // the fist is closed and the hilt sits in it
                    assert!(p.fingertip < 0.135, "{what}: hand open ({:.3})", p.fingertip);
                    let fist = p.grip + p.weapon.z * 0.05;
                    assert!((fist - p.hand).len() < 0.14, "{what}: hilt {:.3} m from the wrist", (fist - p.hand).len());
                    // and the point stays off the floor while standing
                    if state != "slash" {
                        let reach = match w {
                            Weapon::Dagger => 0.56,
                            Weapon::Greatsword => 1.72,
                            _ => 1.28,
                        };
                        let tip = p.grip + p.weapon.z * reach;
                        assert!(tip.y > 0.0, "{what}: the point goes into the floor ({:.2})", tip.y);
                    }
                    if state == "ready" {
                        let want = v3(0.25, -0.38, 0.89).norm();
                        assert!(p.weapon.z.dot(want) > 0.9, "{what}: held ready low and forward: {:?}", p.weapon.z);
                    }
                    println!("{what:32} along forearm {along:.2}  fingertip {:.3}  blade ({:.2},{:.2},{:.2})", p.fingertip, p.weapon.z.x, p.weapon.z.y, p.weapon.z.z);
                }
            }
        }
    }
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
