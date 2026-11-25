//! The vault's bosses drawn from their models (crate::bosses), posed every frame from what they're
//! doing: idle, walking, winding up, striking, staggered, dying. Each has its own moves:
//!
//! - the Warden of Wax: a heavy stride; the halberd raised behind the shoulder then cleaved down
//!   in front (the cone); both hands lifting it over the helm then driving it into the floor (the
//!   slam); a recoil every time it loses a sixth of its health; it melts into a puddle as it dies.
//!   Its crown of wicks and the candles on its halberd burn, its eyes glow in the visor.
//! - the Twin Assessors: Tally throws her coins with a flick; Seal lifts its stamp high and presses
//!   it down while red seals fall on the stamped (casting); while they're bound each lifts its
//!   scales or ledger to the other and a glowing chain joins them; dying, they crumple.
//! - the Chained Ledger: it hovers, its page wings flapping, its quill tail swaying, ink dripping
//!   from its jaw; before its ink beam the jaw gapes wide and it rears, then snaps and spits; in the
//!   page storm its wings beat wildly and pages whirl round it; dying, its chains break and it falls.
//! - the Ashen Magistrate: the burning gavel raised high and brought down (where you stand), the
//!   left hand pointing out the Verdict, the gavel swept across in front; dying, it kneels and falls.
//!
//! The renderer remembers a little about each boss between frames (its speed and stride, when it
//! was hurt, how long its move has run, when it fell), so clients animate exactly as the host does.

use super::*;
use crate::bosses::{self, Kind, Model, Pose, Xf, lean, raise, roll, turn};
use std::sync::Mutex;

fn ss(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What the renderer remembers of one boss.
#[derive(Clone, Copy, Debug)]
struct Mem {
    kind: FoeKind,
    pos: V3,
    speed: f32,
    phase: f32,
    hp: f32,
    /// The share of health that staggers it next.
    next: f32,
    stagger: f32,
    flash: f32,
    /// Seconds since it fell (negative while it stands).
    dead: f32,
    state: u8,
    /// How long its move has run, and what the host says is left of it.
    since: f32,
    left: f32,
    /// Its last move, and how much of it is left as it recovers.
    last: u8,
    after: f32,
    /// (the Ledger) where its ink beam was aimed.
    aim: f32,
}

struct Memory {
    room: usize,
    place: u16,
    time: f32,
    foes: Vec<Mem>,
}

static MEM: Mutex<Option<Memory>> = Mutex::new(None);

const STAGGER_STEP: f32 = 1.0 / 6.0;
const STAGGER_TIME: f32 = 1.2;
const RECOVER: f32 = 0.45;

fn next_stagger(frac: f32) -> f32 {
    ((frac / STAGGER_STEP).ceil() - 1.0).max(0.0) * STAGGER_STEP
}

/// How long each move lasts (the host's own clocks in raid.rs).
fn move_time(kind: FoeKind, state: u8) -> f32 {
    match (kind, state) {
        (FoeKind::Warden, 1) => 1.3,
        (FoeKind::Warden, 2) => 1.7,
        (FoeKind::Tally, _) => 0.5,
        (FoeKind::Seal, _) => 1.0,
        (FoeKind::Ledger, _) => 1.5,
        (FoeKind::Magistrate, 1) => 1.5,
        (FoeKind::Magistrate, 2) => 1.15,
        (FoeKind::Magistrate, 3) => 1.2,
        _ => 1.0,
    }
}

fn remember(g: &Game, r: &raid::Raid) -> Vec<Mem> {
    let mut lock = MEM.lock().unwrap_or_else(|e| e.into_inner());
    let t = g.time;
    let fresh = lock.as_ref().map_or(true, |m| m.room != r.room || m.place != r.place || t < m.time || t - m.time > 5.0);
    if fresh {
        *lock = Some(Memory { room: r.room, place: r.place, time: t, foes: Vec::new() });
    }
    let Some(mem) = lock.as_mut() else { return Vec::new() };
    let dt = (t - mem.time).clamp(0.0, 0.1);
    mem.time = t;
    let aim = r.teles.iter().find(|x| x.cause == Cause::InkBeam).map(|x| x.dir);
    for (i, f) in r.foes.iter().enumerate() {
        let p = if g.is_authority() { f.pos } else { f.shown };
        let frac = f.hp / f.max.max(1.0);
        if mem.foes.len() <= i || mem.foes[i].kind != f.kind {
            let m = Mem { kind: f.kind, pos: p, speed: 0.0, phase: 0.0, hp: f.hp, next: next_stagger(frac), stagger: 0.0, flash: 0.0, dead: if f.alive { -1.0 } else { 60.0 }, state: f.state, since: 0.0, left: f.t, last: 0, after: 0.0, aim: f.yaw };
            if mem.foes.len() <= i {
                mem.foes.push(m);
            } else {
                mem.foes[i] = m;
            }
        }
        let m = &mut mem.foes[i];
        if dt > 0.0 {
            let v = (p - m.pos).flat().len() / dt;
            m.speed += (v.min(8.0) - m.speed) * (dt * 6.0).min(1.0);
            m.phase += dt * m.speed * 1.5;
        }
        m.pos = p;
        m.left = f.t;
        if f.hp < m.hp - 0.5 {
            m.flash = 0.1;
        }
        m.hp = f.hp;
        m.flash = (m.flash - dt).max(0.0);
        m.stagger = (m.stagger - dt).max(0.0);
        if f.alive {
            if m.dead >= 0.0 {
                // risen again (a twin)
                m.dead = -1.0;
                m.next = next_stagger(frac);
            }
            if frac < m.next {
                m.stagger = STAGGER_TIME;
                m.next = next_stagger(frac);
            }
        } else if m.dead < 0.0 {
            m.dead = 0.0;
        } else {
            m.dead += dt;
        }
        if f.state != m.state {
            if m.state != 0 {
                m.last = m.state;
                m.after = RECOVER;
            }
            m.state = f.state;
            m.since = 0.0;
        } else {
            m.since += dt;
        }
        if f.state == 0 {
            m.after = (m.after - dt).max(0.0);
        }
        if f.kind == FoeKind::Ledger {
            if let Some(a) = aim {
                m.aim = a;
            }
        }
    }
    mem.foes.clone()
}

/// The move a boss is in (or recovering from): its number, how far through it (0..1) and how much
/// of it shows (1 while it runs, fading as it recovers).
fn doing(mm: &Mem) -> (u8, f32, f32) {
    if mm.dead >= 0.0 {
        return (0, 0.0, 0.0);
    }
    if mm.state != 0 {
        let dur = move_time(mm.kind, mm.state);
        let k = if mm.left > 0.0 { 1.0 - mm.left / dur } else { mm.since / dur };
        (mm.state, k.clamp(0.0, 1.0), 1.0)
    } else if mm.after > 0.0 {
        (mm.last, 1.0, ss(0.0, 1.0, mm.after / RECOVER))
    } else {
        (0, 0.0, 0.0)
    }
}

/// A recoil that snaps in and eases out (1 at its height).
fn recoil(mm: &Mem) -> f32 {
    if mm.stagger <= 0.0 || mm.dead >= 0.0 {
        return 0.0;
    }
    let u = 1.0 - mm.stagger / STAGGER_TIME;
    ss(0.0, 0.1, u) * (1.0 - ss(0.35, 1.0, u))
}

/// Draw a posed model: every piece's corners moved by its pose, bent by `warp` (melting,
/// crumpling), then placed at `base` in frame `fr` at scale `sc`. A fresh hit flashes it.
#[allow(clippy::too_many_arguments)]
fn emit(op: &mut MB, m: &Model, xs: &[Xf], base: V3, fr: &M3, sc: f32, flash: f32, skip: &dyn Fn(usize) -> bool, warp: &dyn Fn(V3) -> V3) {
    let lift = 1.0 + flash * 6.0;
    op.v.reserve(m.pieces.iter().map(|p| p.tris.len()).sum());
    for (i, piece) in m.pieces.iter().enumerate() {
        if skip(i) {
            continue;
        }
        let x = xs[i];
        for v in &piece.tris {
            let lp = warp(x.apply(v.p));
            let n = fr.apply(x.m.apply(v.n));
            let col = if v.ex[1] > 1.5 { v.col } else { [v.col[0] * lift, v.col[1] * lift, v.col[2] * lift, v.col[3]] };
            op.vert(base + fr.apply(lp * sc), n, v.uv, col, v.ex);
        }
    }
}

/// Where a marker is now.
fn mark_at(_m: &Model, xs: &[Xf], mk: &bosses::Marker, base: V3, fr: &M3, sc: f32, warp: &dyn Fn(V3) -> V3) -> V3 {
    base + fr.apply(warp(xs[mk.piece].apply(mk.p)) * sc)
}

fn no_skip(_: usize) -> bool {
    false
}
fn no_warp(v: V3) -> V3 {
    v
}

/// A candle flame: an orange tongue round a white-hot core, flickering.
fn flame(ad: &mut MB, cam: &Cam, p: V3, size: f32, t: f32, seed: f32) {
    if size <= 0.01 {
        return;
    }
    let fl = 0.85 + 0.15 * (t * 17.0 + seed * 3.1).sin() * (t * 7.3 + seed).cos();
    ad.billboard(cam, p + v3(0.0, size * 0.25, 0.0), size * fl, lina([255, 150, 50], 0.85));
    ad.billboard(cam, p + v3(0.0, size * 0.55, 0.0), size * 0.6 * fl, lina([255, 90, 30], 0.5));
    ad.billboard(cam, p, size * 0.38, lina([255, 245, 210], 1.0));
}

/// A chain of links from `a` to `b`, sagging by `sag` in the middle (glowing if `glow` is given).
fn chain(op: &mut MB, a: V3, b: V3, sag: f32, glow: Option<[f32; 4]>) {
    let Some(m) = bosses::model(Kind::Props) else { return };
    let Some(li) = m.piece("link") else { return };
    let link = &m.pieces[li];
    let len = (b - a).len();
    if len < 0.05 {
        return;
    }
    let n = ((len / 0.19).ceil() as usize).clamp(1, 200);
    let at = |t: f32| a + (b - a) * t - V3::UP * (sag * 4.0 * t * (1.0 - t));
    for j in 0..n {
        let t = (j as f32 + 0.5) / n as f32;
        let c = at(t);
        let d = (at((t + 0.5 / n as f32).min(1.0)) - at((t - 0.5 / n as f32).max(0.0))).norm();
        let side = if d.y.abs() < 0.9 { V3::UP.cross(d).norm() } else { v3(1.0, 0.0, 0.0) };
        let (x, z) = if j % 2 == 0 { (side, side.cross(d)) } else { (d.cross(side), side) };
        let fr = M3 { x, y: d, z: x.cross(d) };
        let _ = z;
        for v in &link.tris {
            let (col, ex) = match glow {
                Some(c) => (c, [-1.0, 2.0, 1.0, 0.0]),
                None => (v.col, v.ex),
            };
            op.vert(c + fr.apply(v.p * 1.3), fr.apply(v.n), v.uv, col, ex);
        }
    }
}

/// Every boss of the room (standing, or as it falls).
pub fn draw_all(op: &mut MB, ad: &mut MB, al: &mut MB, cam: &Cam, g: &Game, r: &raid::Raid) {
    let mems = remember(g, r);
    let mut chain_ends: [Option<V3>; 2] = [None, None];
    for (fi, f) in r.foes.iter().enumerate() {
        let Some(mm) = mems.get(fi) else { continue };
        if !f.alive && mm.dead > 30.0 {
            continue;
        }
        let p = mm.pos;
        // well behind the camera: nothing to draw
        let to = p + v3(0.0, 2.0, 0.0) - cam.pos;
        if to.dot(cam.fwd) < -6.0 && to.len() > 8.0 {
            continue;
        }
        match f.kind {
            FoeKind::Warden => warden(op, ad, al, cam, g, f, mm, p),
            FoeKind::Tally | FoeKind::Seal => {
                let bound = f.alive && matches!(raid::vulnerable(r, fi), Err(s) if s.starts_with("BOUND"));
                let end = assessor(op, ad, al, cam, g, r, f, mm, p, bound);
                if bound {
                    chain_ends[if f.kind == FoeKind::Tally { 0 } else { 1 }] = end;
                }
            }
            FoeKind::Ledger => ledger(op, ad, al, cam, g, r, f, mm, p),
            FoeKind::Magistrate => magistrate(op, ad, al, cam, g, r, f, mm, p),
            _ => {}
        }
    }
    // the Assessors bound: a glowing chain between them, pulsing
    if let [Some(a), Some(b)] = chain_ends {
        let t = g.time;
        let pulse = 0.75 + 0.25 * (t * 6.0).sin();
        chain(op, a, b, 0.5, Some([1.0 * pulse + 0.4, 0.78 * pulse + 0.25, 0.3 * pulse, 1.0]));
        let n = 14;
        for k in 0..=n {
            let s = k as f32 / n as f32;
            let q = a + (b - a) * s - V3::UP * (0.5 * 4.0 * s * (1.0 - s));
            let w = ((t * 3.0 - s * 6.0).sin() * 0.5 + 0.5).powi(3);
            ad.billboard(cam, q, 0.28 + 0.25 * w, lina([255, 210, 110], 0.18 + 0.35 * w));
        }
    }
}

// ---------------------------------------------------------------- the Warden of Wax

#[allow(clippy::too_many_arguments)]
fn warden(op: &mut MB, ad: &mut MB, al: &mut MB, cam: &Cam, g: &Game, f: &raid::Foe, mm: &Mem, p: V3) {
    let Some(m) = bosses::model(Kind::Warden) else { return };
    let t = g.time;
    let mut ps = Pose::rest(m);
    let alive = mm.dead < 0.0;
    let walk = if alive { (mm.speed / 2.0).clamp(0.0, 1.0) } else { 0.0 };
    let ph = mm.phase;
    let br = (t * 1.2).sin();
    // standing: breathing, a slow look round, the free arm a little out
    ps.turn(m, "torso", lean(0.025 * br));
    ps.turn(m, "head", turn(0.18 * (t * 0.37).sin()).mul(&lean(-0.04 * br)));
    ps.turn(m, "upper_l", roll(-0.1 - 0.03 * br));
    ps.turn(m, "fore_l", raise(0.25));
    // walking: a heavy stride
    let sw = ph.sin() * 0.42 * walk;
    ps.turn(m, "thigh_l", raise(sw));
    ps.turn(m, "thigh_r", raise(-sw));
    ps.turn(m, "shin_l", raise(-0.7 * walk * (ph + 2.0).sin().max(0.0)));
    ps.turn(m, "shin_r", raise(-0.7 * walk * (ph + 2.0 + PI).sin().max(0.0)));
    ps.shift(m, "pelvis", v3(0.0, -0.1 * walk * ph.sin().abs(), 0.0));
    ps.turn(m, "pelvis", roll(0.05 * walk * ph.sin()));
    ps.turn(m, "torso", turn(-0.12 * sw));
    ps.turn(m, "upper_l", raise(-0.5 * sw));
    // its moves
    let (st, k, w8) = doing(mm);
    let mut glint = 0.0;
    let mut slam_hit = 0.0;
    match st {
        1 => {
            // the cleave: the halberd raised behind the right shoulder, then down in front
            let wnd = ss(0.0, 0.8, k);
            let hit = ss(0.82, 0.97, k);
            let up_r = (2.9 * wnd * (1.0 - hit) + 0.9 * hit) * w8;
            let fo_r = 0.5 * wnd * (1.0 - hit) * w8;
            let theta = (-0.9 * wnd * (1.0 - hit) + 1.9 * hit) * w8;
            ps.turn(m, "upper_r", raise(up_r).mul(&roll(0.25 * wnd * (1.0 - hit) * w8)));
            ps.turn(m, "fore_r", raise(fo_r));
            ps.turn(m, "halberd", lean(theta + up_r + fo_r));
            ps.turn(m, "torso", turn((0.35 * wnd * (1.0 - hit) - 0.3 * hit) * w8).mul(&lean((-0.15 * wnd * (1.0 - hit) + 0.3 * hit) * w8)));
            ps.turn(m, "upper_l", raise((1.0 * wnd * (1.0 - hit) + 0.5 * hit) * w8));
            ps.shift(m, "pelvis", v3(0.0, -0.18 * hit * w8, 0.0));
            glint = wnd * (1.0 - hit) * w8;
        }
        2 => {
            // the slam: both hands lift it over the helm and drive it into the floor
            let wnd = ss(0.0, 0.82, k);
            let hit = ss(0.84, 0.97, k);
            let a_r = (2.8 * wnd * (1.0 - hit) + 1.1 * hit) * w8;
            let a_l = (2.6 * wnd * (1.0 - hit) + 1.0 * hit) * w8;
            let fo = 0.35 * wnd * (1.0 - hit) * w8;
            ps.turn(m, "upper_r", raise(a_r).mul(&roll(-0.25 * wnd * w8)));
            ps.turn(m, "upper_l", raise(a_l).mul(&roll(0.35 * wnd * w8)));
            ps.turn(m, "fore_r", raise(fo));
            ps.turn(m, "fore_l", raise(fo));
            let theta = (-0.2 * wnd * (1.0 - hit) + 2.4 * hit) * w8;
            ps.turn(m, "halberd", lean(theta + a_r + fo));
            ps.turn(m, "torso", lean((-0.25 * wnd * (1.0 - hit) + 0.45 * hit) * w8));
            ps.shift(m, "pelvis", v3(0.0, -0.4 * hit * w8, 0.0));
            ps.turn(m, "thigh_l", raise(0.55 * hit * w8));
            ps.turn(m, "thigh_r", raise(0.55 * hit * w8));
            ps.turn(m, "shin_l", raise(-0.9 * hit * w8));
            ps.turn(m, "shin_r", raise(-0.9 * hit * w8));
            glint = wnd * (1.0 - hit) * w8;
            slam_hit = if mm.state == 2 { ss(0.9, 1.0, k) * 0.3 } else { 0.3 + 0.7 * (1.0 - w8) };
        }
        _ => {}
    }
    // staggered: it reels back
    let rc = recoil(mm);
    if rc > 0.0 {
        ps.turn(m, "torso", lean(-0.35 * rc + 0.06 * (t * 16.0).sin() * rc));
        ps.turn(m, "head", lean(-0.4 * rc));
        ps.turn(m, "upper_l", roll(-0.8 * rc));
        ps.turn(m, "upper_r", roll(0.5 * rc));
        ps.shift(m, "pelvis", v3(0.0, -0.25 * rc, 0.0));
        ps.turn(m, "thigh_l", raise(0.25 * rc));
        ps.turn(m, "shin_l", raise(-0.4 * rc));
    }
    // dying: it slumps and melts into its pool
    let d = if alive { 0.0 } else { ss(0.0, 1.0, mm.dead / 3.5) };
    if d > 0.0 {
        ps.turn(m, "torso", lean(0.4 * d));
        ps.turn(m, "head", lean(0.7 * d));
        ps.turn(m, "upper_l", roll(-0.4 * d).mul(&raise(-0.3 * d)));
        ps.turn(m, "upper_r", roll(0.3 * d));
        ps.turn(m, "halberd", lean(1.3 * d).mul(&roll(0.4 * d)));
        ps.shift(m, "pelvis", v3(0.0, -0.3 * d, 0.0));
    }
    let xs = ps.solve(m, bosses::warden_parent);
    let fr = M3::rot_y(f.yaw);
    let kk = d.powf(1.2);
    let melt = move |v: V3| {
        if kk <= 0.0 {
            return v;
        }
        let y = v.y * (1.0 - 0.86 * kk);
        let spread = 1.0 + 0.45 * kk * (1.0 - (y / 4.5).clamp(0.0, 1.0));
        v3(v.x * spread, y, v.z * spread)
    };
    emit(op, m, &xs, p, &fr, 1.0, mm.flash, &no_skip, &melt);
    // its flames: the crown of wicks, the candles on the halberd (taller while it winds up)
    let fire = (1.0 - d * 1.2).max(0.0);
    for mk in m.marks("flame") {
        let q = mark_at(m, &xs, mk, p, &fr, 1.0, &melt);
        let big = if mk.n == 8 && m.pieces[mk.piece].name == "head" { 0.42 } else { 0.24 };
        flame(ad, cam, q, big * fire * (1.0 + 0.5 * glint), t, mk.n as f32 + mk.piece as f32 * 5.0);
    }
    // ember eyes, flaring red-hot through a move; the heart in its breastplate
    for mk in m.marks("eye") {
        let q = mark_at(m, &xs, mk, p, &fr, 1.0, &melt);
        let hot = (glint + rc).min(1.0);
        ad.billboard(cam, q, 0.08 + 0.06 * hot, lina([255, (170.0 - 110.0 * hot) as u8, 50], (1.0 - d).max(0.0)));
        ad.billboard(cam, q, 0.26 + 0.2 * hot, lina([255, 90, 30], 0.45 * (1.0 - d)));
    }
    for mk in m.marks("heart") {
        let q = mark_at(m, &xs, mk, p, &fr, 1.0, &melt);
        ad.billboard(cam, q, 0.3 + 0.06 * (t * 4.0).sin(), lina([255, 120, 30], 0.55 * (1.0 - d)));
    }
    // the blade glints as it winds up (it's coming)
    if glint > 0.05 {
        for mk in m.marks("blade") {
            let q = mark_at(m, &xs, mk, p, &fr, 1.0, &melt);
            ad.billboard(cam, q, 0.4 + 1.1 * glint, lina([255, 200, 120], 0.25 + 0.5 * glint));
        }
    }
    // the slam lands: wax splashes out in a ring
    if slam_hit > 0.0 && slam_hit < 1.0 {
        let rad = 0.8 + 4.0 * slam_hit;
        for n in 0..20 {
            let a = n as f32 * 0.314 + 0.2;
            let h = 1.2 * slam_hit * (1.0 - slam_hit) * 4.0;
            ad.billboard(cam, p + v3(a.sin() * rad, 0.2 + h * (0.6 + 0.4 * (n % 3) as f32), a.cos() * rad), 0.28, lina([255, 220, 150], 0.7 * (1.0 - slam_hit)));
        }
        ring(ad, v3(p.x, VY + 0.07, p.z), rad, 0.35, lina([255, 230, 170], 0.8 * (1.0 - slam_hit)));
    }
    // melting: wax runs and steam rising
    if d > 0.0 && d < 1.0 {
        for n in 0..10 {
            let ph2 = (t * 0.9 + n as f32 * 0.1).fract();
            let a = n as f32 * 0.63;
            let h = (1.0 - kk) * 4.0 * (1.0 - ph2);
            ad.billboard(cam, p + v3(a.sin() * 1.1, h, a.cos() * 0.9), 0.12, lina([255, 230, 180], 0.5 * (1.0 - ph2)));
            al.billboard(cam, p + v3(a.cos() * 0.8, 4.5 * (1.0 - kk) + ph2 * 2.0, a.sin() * 0.8), 0.5 + ph2, lina([200, 190, 175], 0.25 * (1.0 - ph2)));
        }
    }
}

// ---------------------------------------------------------------- the Twin Assessors

/// One Assessor; returns where its end of the binding chain is.
#[allow(clippy::too_many_arguments)]
fn assessor(op: &mut MB, ad: &mut MB, al: &mut MB, cam: &Cam, g: &Game, r: &raid::Raid, f: &raid::Foe, mm: &Mem, p: V3, bound: bool) -> Option<V3> {
    let gold = f.kind == FoeKind::Tally;
    let m = bosses::model(if gold { Kind::Tally } else { Kind::Seal })?;
    let t = g.time;
    let mut ps = Pose::rest(m);
    let alive = mm.dead < 0.0;
    let walk = if alive { (mm.speed / 1.8).clamp(0.0, 1.0) } else { 0.0 };
    let ph = mm.phase + if gold { 0.0 } else { 1.7 };
    // standing: robes swaying, a judging look round
    ps.turn(m, "robe", roll(0.05 * walk * ph.sin() + 0.015 * (t * 1.1).sin()));
    ps.shift(m, "robe", v3(0.0, 0.04 * walk * ph.sin().abs(), 0.0));
    ps.turn(m, "torso", lean(0.03 * (t * 1.3).sin()).mul(&turn(0.08 * walk * ph.sin())));
    ps.turn(m, "head", turn(0.22 * (t * 0.5 + if gold { 0.0 } else { 2.0 }).sin()));
    ps.turn(m, "upper_l", roll(-0.08).mul(&raise(0.25 * walk * ph.sin())));
    ps.turn(m, "upper_r", roll(0.08).mul(&raise(-0.25 * walk * ph.sin())));
    if gold {
        ps.turn(m, "scales", roll(0.12 * (t * 1.5).sin()));
    } else {
        ps.turn(m, "ledger", lean(0.15 * (t * 1.2).sin()));
    }
    // bound: each lifts its scales or ledger to the other
    let link = if bound { 1.0 } else { 0.0 };
    if bound {
        let pulse = 0.08 * (t * 3.0).sin();
        ps.turn(m, "upper_l", raise(2.0 + pulse).mul(&roll(-0.35)));
        ps.turn(m, "fore_l", raise(0.3));
        if gold {
            ps.turn(m, "scales", roll(0.3 * (t * 3.0).sin()));
        } else {
            ps.turn(m, "ledger", lean(-1.1));
        }
    }
    let (st, k, w8) = doing(mm);
    let mut charge = 0.0;
    if st == 1 {
        if gold {
            // the coin throw: a flick of the right hand
            let wnd = ss(0.0, 0.12, k);
            let rel = ss(0.08, 0.35, k);
            ps.turn(m, "upper_r", raise((-0.5 * wnd * (1.0 - rel) + 1.7 * rel) * w8).mul(&roll(0.6 * wnd * (1.0 - rel) * w8)));
            ps.turn(m, "fore_r", raise(0.6 * wnd * (1.0 - rel) * w8));
            ps.turn(m, "torso", turn((0.35 * wnd * (1.0 - rel) - 0.35 * rel) * w8));
            charge = if mm.state == 1 { 1.0 - ss(0.2, 0.6, k) } else { 0.0 };
        } else {
            // the stamp lifted high, the ledger to its chest, then pressed down
            let wnd = ss(0.0, 0.7, k);
            let hit = ss(0.7, 0.82, k);
            ps.turn(m, "upper_r", raise((2.7 * wnd * (1.0 - hit) + 0.8 * hit) * w8));
            ps.turn(m, "fore_r", raise(0.3 * wnd * (1.0 - hit) * w8));
            ps.turn(m, "torso", lean((-0.15 * wnd * (1.0 - hit) + 0.35 * hit) * w8));
            if !bound {
                ps.turn(m, "upper_l", raise(1.1 * wnd * w8));
                ps.turn(m, "ledger", lean(-1.0 * wnd * w8));
            }
            charge = if mm.state == 1 { wnd * (1.0 - hit) } else { 0.0 };
        }
    }
    let rc = recoil(mm) * 0.8;
    if rc > 0.0 {
        ps.turn(m, "torso", lean(-0.3 * rc));
        ps.turn(m, "head", lean(-0.35 * rc));
        ps.turn(m, "robe", lean(-0.08 * rc));
    }
    // dying: it crumples into its robes, then sinks away
    let d = if alive { 0.0 } else { ss(0.0, 1.0, mm.dead / 1.6) };
    if d > 0.0 {
        ps.turn(m, "torso", lean(0.9 * d));
        ps.turn(m, "head", lean(0.5 * d));
        ps.turn(m, "upper_l", raise(-0.2 * d));
        ps.turn(m, "upper_r", raise(-0.2 * d));
    }
    let xs = ps.solve(m, bosses::robed_parent);
    let fr = M3::rot_y(f.yaw);
    let crumple = move |v: V3| {
        if d <= 0.0 {
            return v;
        }
        let y = v.y * (1.0 - 0.62 * d);
        let s = 1.0 + 0.3 * d * (1.0 - (y / 2.0).clamp(0.0, 1.0));
        v3(v.x * s, y, v.z * s)
    };
    let sink = if mm.dead > 3.0 { ((mm.dead - 3.0) * 0.8).min(4.0) } else { 0.0 };
    let base = p - V3::UP * sink;
    emit(op, m, &xs, base, &fr, 1.0, mm.flash, &no_skip, &crumple);
    let fade = 1.0 - d;
    // their glints: Tally's monocle, Seal's eyes in their hollows
    for mk in m.marks("monocle").chain(m.marks("eye")) {
        let q = mark_at(m, &xs, mk, base, &fr, 1.0, &crumple);
        let c = if gold { [255, 240, 190] } else { [140, 200, 255] };
        ad.billboard(cam, q, 0.07, lina(c, 0.9 * fade));
        ad.billboard(cam, q, 0.2, lina(c, 0.25 * fade));
    }
    // the hand that acts glows as the move gathers
    if charge > 0.02 {
        for mk in m.marks("hand") {
            let q = mark_at(m, &xs, mk, base, &fr, 1.0, &crumple);
            let c = if gold { [255, 210, 90] } else { [255, 40, 50] };
            ad.billboard(cam, q, 0.3 + 0.7 * charge, lina(c, 0.3 + 0.5 * charge));
        }
    }
    // Seal's casting: a red seal falls on each stamped spot
    if !gold && alive {
        for tl in r.teles.iter().filter(|x| x.cause == Cause::Seal) {
            let k2 = tl.k();
            let h = (1.0 - k2) * 7.0 + 0.2;
            let c = v3(tl.pos.x, VY + h, tl.pos.z);
            ring(ad, c, tl.r * 0.75, 0.16, lina([255, 40, 50], 0.35 + 0.5 * k2));
            ring(ad, c, tl.r * 0.45, 0.1, lina([255, 90, 80], 0.3 + 0.4 * k2));
            ad.billboard(cam, c, 0.7, lina([255, 30, 40], 0.25 + 0.4 * k2));
        }
    }
    let _ = (al, link);
    if bound && alive {
        m.marks("chain").next().map(|mk| mark_at(m, &xs, mk, base, &fr, 1.0, &crumple))
    } else {
        None
    }
}

// ---------------------------------------------------------------- the Chained Ledger

#[allow(clippy::too_many_arguments)]
fn ledger(op: &mut MB, ad: &mut MB, al: &mut MB, cam: &Cam, g: &Game, r: &raid::Raid, f: &raid::Foe, mm: &Mem, p: V3) {
    let Some(m) = bosses::model(Kind::Ledger) else { return };
    let t = g.time;
    let alive = mm.dead < 0.0;
    let mut ps = Pose::rest(m);
    let storm = alive && r.teles.iter().any(|x| x.cause == Cause::PageStorm);
    let mut jaw = 0.12 + 0.06 * (t * 1.7).sin();
    let mut flap = if storm { 0.75 * (t * 11.0).sin() + 0.25 } else { 0.28 * (t * 2.6).sin() + 0.12 };
    let mut tilt = 0.04 * (t * 0.9).sin();
    let mut fwd = 0.0;
    let (st, k, w8) = doing(mm);
    let mut gape = 0.0;
    let mut spit = 0.0;
    if st == 1 {
        let wnd = ss(0.0, 0.82, k);
        let snap = ss(0.86, 0.95, k);
        jaw += (0.95 * wnd * (1.0 - snap)) * w8;
        tilt += (-0.22 * wnd * (1.0 - snap) + 0.25 * snap) * w8;
        fwd = 0.6 * snap * w8;
        gape = wnd * (1.0 - snap) * w8;
        spit = if mm.state == 1 { ss(0.9, 1.0, k) } else { w8 };
    }
    let rc = recoil(mm);
    tilt -= 0.3 * rc;
    jaw += 0.4 * rc;
    // dying: its chains break and it falls, jaw hanging open, wings drooping
    let d = if alive { 0.0 } else { ss(0.0, 1.0, mm.dead / 1.1) };
    let height = if alive { 3.0 + 0.2 * (t * 1.3).sin() } else { 3.0 + (0.42 - 3.0) * d * d };
    if d > 0.0 {
        jaw = jaw * (1.0 - d) + 0.7 * d;
        flap = flap * (1.0 - d) - 0.5 * d;
        tilt = tilt * (1.0 - d) + 0.12 * d;
    }
    ps.turn(m, "upper", raise(jaw));
    ps.turn(m, "wing_r", roll(flap));
    ps.turn(m, "wing_l", roll(-flap));
    ps.turn(m, "quill", turn(0.4 * (t * 0.9).sin() * (1.0 - d)).mul(&lean(0.25 * (t * 1.4).sin() * (1.0 - d) - 0.15)));
    let xs = ps.solve(m, bosses::book_parent);
    let fr = M3::rot_y(f.yaw).mul(&lean(tilt));
    let base = p + V3::UP * height + yaw_fwd(f.yaw) * fwd;
    emit(op, m, &xs, base, &fr, 1.0, mm.flash, &no_skip, &no_warp);
    let fade = 1.0 - d;
    // its violet eyes and sigil
    for mk in m.marks("eye") {
        let q = mark_at(m, &xs, mk, base, &fr, 1.0, &no_warp);
        ad.billboard(cam, q, 0.16 + 0.12 * gape, lina([190, 120, 255], 0.9 * fade));
        ad.billboard(cam, q, 0.5 + 0.4 * gape, lina([150, 70, 255], 0.3 * fade));
    }
    // the quill glows at its tip
    for mk in m.marks("tip") {
        let q = mark_at(m, &xs, mk, base, &fr, 1.0, &no_warp);
        ad.billboard(cam, q, 0.25 + 0.05 * (t * 5.0).sin(), lina([150, 230, 255], 0.7 * fade + 0.1));
    }
    // ink: drips from its jaw; gathering as it gapes; spat along the beam
    if let Some(mk) = m.marks("mouth").next() {
        let mouth = mark_at(m, &xs, mk, base, &fr, 1.0, &no_warp);
        if alive {
            for n in 0..6 {
                let ph2 = (t * 0.7 + n as f32 * 0.17).fract();
                let x = (n as f32 - 2.5) * 0.28;
                al.billboard(cam, mouth + fr.apply(v3(x, 0.0, -0.1)) - V3::UP * (ph2 * ph2 * height), 0.09, lina([24, 8, 40], 0.9 * (1.0 - ph2 * 0.5)));
            }
        }
        if gape > 0.05 {
            for n in 0..12 {
                let a = t * 4.0 + n as f32 * 0.52;
                let rr = 1.6 * (1.0 - ((t * 1.5 + n as f32 * 0.083).fract()));
                al.billboard(cam, mouth + v3(a.sin() * rr, a.cos() * rr * 0.6, 0.0), 0.16, lina([20, 6, 36], 0.8 * gape));
            }
            ad.billboard(cam, mouth, 0.4 + 0.9 * gape, lina([140, 60, 255], 0.35 * gape));
        }
        if spit > 0.02 {
            let dir = yaw_fwd(mm.aim);
            let end = mouth + dir * 20.0 - V3::UP * (height - 0.8);
            al.beam(cam, mouth, end, 0.7 * spit, lina([18, 6, 32], 0.9 * spit));
            ad.beam(cam, mouth, end, 0.25 * spit, lina([160, 80, 255], 0.6 * spit));
        }
    }
    // the page storm: its pages whirl round it
    if storm {
        let tl = r.teles.iter().find(|x| x.cause == Cause::PageStorm).map_or(0.5, |x| x.k());
        for n in 0..28 {
            let a = t * (2.0 + 1.5 * tl) + n as f32 * 0.224;
            let rr = 1.8 + (n % 7) as f32 * 0.65;
            let h = base.y - 1.8 + (n % 5) as f32 * 0.7 + (t * 2.0 + n as f32).sin() * 0.3;
            let c = v3(p.x + a.sin() * rr, h, p.z + a.cos() * rr);
            let spin = t * 6.0 + n as f32;
            let (s, co) = spin.sin_cos();
            let u = (cam.right * co + cam.up * s) * 0.16;
            let v = (cam.right * -s + cam.up * co) * 0.22;
            al.quad([c - u - v, c + u - v, c + u + v, c - u + v], -cam.fwd, [[0.5, 0.5]; 4], lina([238, 224, 186], 0.95), FLAT);
        }
    }
    // the chains: from four anchors on the floor up to the book (broken when it falls)
    let Some(pm) = bosses::model(Kind::Props) else { return };
    let rm = ROOMS[r.room.min(ROOMS.len() - 1)];
    let c = rm.centre();
    let ends: Vec<V3> = m.marks("chain").map(|mk| mark_at(m, &xs, mk, base, &fr, 1.0, &no_warp)).collect();
    for (q, (dx, dz)) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].iter().enumerate() {
        let a = v3(c.x + dx * 4.2, VY, c.z + dz * 4.2);
        // the anchor plate, turned towards the book
        if let Some(ai) = pm.piece("anchor") {
            let yaw = (p.x - a.x).atan2(p.z - a.z);
            let afr = M3::rot_y(yaw);
            for v in &pm.pieces[ai].tris {
                op.vert(a + afr.apply(v.p * 1.4), afr.apply(v.n), v.uv, v.col, v.ex);
            }
        }
        let ring_at = a + v3(0.0, 0.5, 0.0);
        let near = ends.iter().copied().min_by(|x, y| (*x - ring_at).len().total_cmp(&(*y - ring_at).len()));
        if let Some(e) = near {
            if alive {
                chain(op, ring_at, e, 0.25, None);
            } else {
                // a broken length lying out from the anchor
                let dir = (v3(e.x, ring_at.y, e.z) - ring_at).norm();
                chain(op, ring_at, ring_at + dir * 1.6 - V3::UP * 0.45, 0.05, None);
                let _ = q;
            }
        }
    }
}

// ---------------------------------------------------------------- the Ashen Magistrate

#[allow(clippy::too_many_arguments)]
fn magistrate(op: &mut MB, ad: &mut MB, al: &mut MB, cam: &Cam, g: &Game, r: &raid::Raid, f: &raid::Foe, mm: &Mem, p: V3) {
    let Some(m) = bosses::model(Kind::Magistrate) else { return };
    let t = g.time;
    let alive = mm.dead < 0.0;
    let mut ps = Pose::rest(m);
    let walk = if alive { (mm.speed / 2.4).clamp(0.0, 1.0) } else { 0.0 };
    let ph = mm.phase;
    ps.turn(m, "robe", roll(0.04 * walk * ph.sin() + 0.012 * (t * 0.9).sin()));
    ps.shift(m, "robe", v3(0.0, 0.05 * walk * ph.sin().abs(), 0.0));
    ps.turn(m, "torso", lean(0.025 * (t * 1.1).sin()).mul(&turn(0.07 * walk * ph.sin())));
    ps.turn(m, "head", turn(0.15 * (t * 0.42).sin()).mul(&lean(0.06)));
    ps.turn(m, "upper_l", roll(-0.1).mul(&raise(0.2 * walk * ph.sin())));
    ps.turn(m, "upper_r", roll(0.06));
    let (st, k, w8) = doing(mm);
    let mut blaze = 0.0;
    let mut point = 0.0;
    let mut sweep = 0.0;
    // (how far the right arm is raised, and which way the gavel should point: it's turned in the
    // hand to keep pointing there)
    let mut arm = 0.0;
    let mut aim_g = 0.0;
    match st {
        1 => {
            // the gavel raised high and brought down
            let wnd = ss(0.0, 0.8, k);
            let hit = ss(0.84, 0.97, k);
            let up_r = (2.9 * wnd * (1.0 - hit) + 1.0 * hit) * w8;
            let fo_r = 0.35 * wnd * (1.0 - hit) * w8;
            ps.turn(m, "upper_r", raise(up_r).mul(&roll(-0.15 * wnd * w8)));
            ps.turn(m, "fore_r", raise(fo_r));
            arm = up_r + fo_r;
            aim_g = (-0.6 * wnd * (1.0 - hit) + 1.7 * hit) * w8;
            ps.turn(m, "torso", lean((-0.22 * wnd * (1.0 - hit) + 0.4 * hit) * w8));
            ps.turn(m, "head", lean(-0.2 * wnd * (1.0 - hit) * w8));
            blaze = wnd * (1.0 - hit) * w8;
        }
        2 => {
            // the Verdict: the left hand points it out, the gavel brandished
            let aim = ss(0.0, 0.5, k) * w8;
            ps.turn(m, "upper_l", raise(1.45 * aim));
            ps.turn(m, "fore_l", raise(0.1 * aim));
            ps.turn(m, "upper_r", raise(0.9 * aim).mul(&roll(0.2 * aim)));
            arm = 0.9 * aim;
            aim_g = 0.25 * aim;
            ps.turn(m, "torso", turn(-0.2 * aim));
            point = if mm.state == 2 { ss(0.2, 0.95, k) } else { 0.0 };
        }
        3 => {
            // the sentence: the gavel swept across in front
            let wnd = ss(0.0, 0.78, k);
            let hit = ss(0.82, 0.97, k);
            ps.turn(m, "torso", turn((0.9 * wnd * (1.0 - hit) - 0.85 * hit) * w8));
            ps.turn(m, "upper_r", raise((1.3 * wnd) * w8).mul(&roll(0.9 * wnd * (1.0 - hit) * w8)));
            arm = 1.3 * wnd * w8;
            aim_g = 0.5 * wnd * w8;
            blaze = wnd * (1.0 - hit) * w8;
            sweep = if mm.state == 3 { hit } else { w8 };
        }
        _ => {}
    }
    if arm != 0.0 || aim_g != 0.0 {
        ps.turn(m, "gavel", lean(aim_g + arm));
    }
    let rc = recoil(mm);
    if rc > 0.0 {
        ps.turn(m, "torso", lean(-0.28 * rc));
        ps.turn(m, "head", lean(-0.3 * rc));
        ps.turn(m, "upper_l", roll(-0.5 * rc));
    }
    // dying: down to its knees, then forward
    let d = if alive { 0.0 } else { ss(0.0, 1.0, mm.dead / 2.2) };
    if d > 0.0 {
        ps.turn(m, "torso", lean(0.75 * d));
        ps.turn(m, "head", lean(0.5 * d));
        ps.turn(m, "upper_l", raise(-0.3 * d));
        ps.turn(m, "gavel", lean(1.4 * d).mul(&roll(0.5 * d)));
    }
    let xs = ps.solve(m, bosses::robed_parent);
    let fr = M3::rot_y(f.yaw);
    let kneel = move |v: V3| {
        if d <= 0.0 {
            return v;
        }
        let y = v.y * (1.0 - 0.42 * d);
        let s = 1.0 + 0.25 * d * (1.0 - (y / 2.0).clamp(0.0, 1.0));
        v3(v.x * s, y, v.z * s)
    };
    emit(op, m, &xs, p, &fr, 1.0, mm.flash, &no_skip, &kneel);
    let fade = 1.0 - d;
    // the burning gavel (roaring as it's raised), its ember eyes, the seal on its chain
    // (the gavel's head: its fire wraps all round it and rises, whichever way it's held)
    let fl: Vec<V3> = m.marks("flame").map(|mk| mark_at(m, &xs, mk, p, &fr, 1.0, &kneel)).collect();
    if !fl.is_empty() {
        let gi = m.piece("gavel").unwrap_or(0);
        let head_up = fr.apply(xs[gi].m.apply(V3::UP));
        let centre = fl.iter().fold(V3::ZERO, |a, b| a + *b) * (1.0 / fl.len() as f32) - head_up * 0.3;
        let along = (fl[fl.len() - 1] - fl[0]).norm();
        let size = (0.34 + 0.3 * blaze) * fade;
        for n in 0..7 {
            let s = n as f32 / 6.0 - 0.5;
            let a = t * 3.0 + n as f32 * 2.1;
            let side = along.cross(V3::UP).norm();
            let q = centre + along * (s * 0.8) + (side * a.sin() + V3::UP * a.cos().abs()) * 0.3;
            flame(ad, cam, q, size, t, n as f32);
        }
        ad.billboard(cam, centre, 1.1 + 0.8 * blaze, lina([255, 110, 30], 0.35 * fade));
        for n in 0..6 {
            let up = (t * 1.6 + n as f32 * 0.17).fract();
            let q = centre + along * ((n as f32 / 5.0 - 0.5) * 0.7) + V3::UP * (0.35 + up * (1.0 + 1.5 * blaze));
            ad.billboard(cam, q, 0.22 * (1.0 - up), lina([255, 140, 40], (0.5 + 0.4 * blaze) * fade));
        }
    }
    for mk in m.marks("eye") {
        let q = mark_at(m, &xs, mk, p, &fr, 1.0, &kneel);
        ad.billboard(cam, q, 0.07 + 0.05 * point, lina([255, 130, 40], fade));
        ad.billboard(cam, q, 0.22, lina([255, 80, 20], 0.4 * fade));
    }
    if point > 0.0 {
        for mk in m.marks("hand") {
            let q = mark_at(m, &xs, mk, p, &fr, 1.0, &kneel);
            ad.billboard(cam, q, 0.2 + 0.9 * point, lina([255, 60, 40], 0.3 + 0.6 * point));
        }
    }
    // the sweep's trail of fire across the front
    if sweep > 0.0 {
        let n = 16;
        for s in 0..n {
            let a = f.yaw + 1.0 - 2.0 * (s as f32 / n as f32) * sweep;
            let q = p + yaw_fwd(a) * 3.0 + V3::UP * 1.6;
            ad.billboard(cam, q, 0.55, lina([255, 120, 40], 0.6 * (1.0 - s as f32 / n as f32)));
        }
    }
    // the gavel's blow lands on the circle it marked: a burst of fire
    for tl in r.teles.iter().filter(|x| x.cause == Cause::Gavel) {
        let k2 = tl.k();
        if k2 > 0.85 {
            let c = v3(tl.pos.x, VY + 0.3, tl.pos.z);
            let b = (k2 - 0.85) / 0.15;
            ad.billboard(cam, c + V3::UP * (5.0 * (1.0 - b)), 0.9, lina([255, 160, 60], 0.8));
            ad.beam(cam, c + V3::UP * 6.0, c + V3::UP * (5.0 * (1.0 - b)), 0.25, lina([255, 110, 40], 0.5));
        }
    }
    let _ = al;
}

// ---------------------------------------------------------------- the Ashblood Regent's regalia

/// The Regent's spiked iron crown on his head and his high blood-red collar with iron pauldrons,
/// worn over his coat (he's drawn as a fighter; this follows his head and shoulders).
pub fn regalia(mb: &mut MB, add: &mut MB, cam: &Cam, f: &Fighter, p: &character::Posed, sc: f32, time: f32) {
    let Some(m) = bosses::model(Kind::Regalia) else { return };
    let (Some(ci), Some(mi)) = (m.piece("crown"), m.piece("mantle")) else { return };
    let put = |mb: &mut MB, i: usize, base: V3, fr: &M3, s: f32| {
        for v in &m.pieces[i].tris {
            mb.vert(base + fr.apply(v.p * s), fr.apply(v.n), v.uv, v.col, v.ex);
        }
    };
    let hf = p.head_frame;
    // (the head's point is at the neck: the crown sits round the top of the skull)
    let crown = p.head + hf.y * (0.13 * sc);
    put(mb, ci, crown, &hf, sc);
    // the collar sits at the neck's base, square to his shoulders
    let up = (p.head - p.waist).norm();
    let fwd0 = yaw_fwd(f.yaw);
    let fwd = (fwd0 - up * fwd0.dot(up)).norm();
    let fr = M3 { x: up.cross(fwd), y: up, z: fwd };
    put(mb, mi, p.head - up * (0.03 * sc), &fr, sc);
    // the crown's red gems catch the light
    for k in 0..3 {
        let a = PI / 2.0 + (k as f32 - 1.0) * 0.7;
        let q = crown + hf.apply(v3(a.cos() * 0.122, 0.16, a.sin() * 0.122) * sc);
        add.billboard(cam, q, 0.03 * sc * (1.0 + 0.2 * (time * 5.0 + k as f32).sin()), lina([255, 40, 50], 0.9));
    }
}
