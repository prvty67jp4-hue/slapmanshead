//! The Ashen Vault drawn: the Vault Gate's circle in the hub, the bosses that aren't fighters
//! (the Warden of Wax, the Twin Assessors, the Chained Ledger, its thralls), every telegraph on the
//! floor (circles, cones, lines, rings, the Blood Ring's donut, the Sweeping Arc), the Regent's
//! blood waves, his four giant spikes, the pools (wax, fire, ash-blood), what's thrown, the plates,
//! the chains, the chest, the marked player's icon, revive rings, and the Regent's roof at night
//! under a red moon with a city's lights all round.

use super::*;
use crate::raid::{self, Cause, FoeKind, Stage, TeleKind, NO_MOVE};
use crate::vault::{self, ROOMS, VY};

mod boss_draw;
pub(super) use boss_draw::regalia;

/// Unlit, flat colour (fogged): the shader's material 2.
const FLAT: [f32; 4] = [-1.0, 2.0, 1.0, 0.0];

fn up_quad(mb: &mut MB, p: [V3; 4], col: [f32; 4]) {
    mb.quad(p, V3::UP, [[0.5, 0.5]; 4], col, FLAT);
}

/// A flat disc sector on the floor between radii `r0` and `r1`, from angle `a0` to `a1` (yaw).
fn sector(mb: &mut MB, c: V3, r0: f32, r1: f32, a0: f32, a1: f32, col: [f32; 4]) {
    if r1 <= r0 + 0.01 {
        return;
    }
    let n = (((a1 - a0).abs() / 0.12).ceil() as usize).clamp(3, 64);
    for k in 0..n {
        let t0 = a0 + (a1 - a0) * k as f32 / n as f32;
        let t1 = a0 + (a1 - a0) * (k + 1) as f32 / n as f32;
        let d0 = yaw_fwd(t0);
        let d1 = yaw_fwd(t1);
        up_quad(mb, [c + d0 * r0, c + d0 * r1, c + d1 * r1, c + d1 * r0], col);
    }
}

fn ring(mb: &mut MB, c: V3, r: f32, w: f32, col: [f32; 4]) {
    sector(mb, c, (r - w * 0.5).max(0.0), r + w * 0.5, 0.0, std::f32::consts::TAU, col);
}

/// An unlit box (the city's towers).
fn flat_box(mb: &mut MB, c: V3, h: V3, col: [f32; 4]) {
    let (x, y, z) = (v3(h.x, 0.0, 0.0), v3(0.0, h.y, 0.0), v3(0.0, 0.0, h.z));
    for (n, u, v) in [(x, z, y), (-x, y, z), (z, y, x), (-z, x, y), (y, x, z)] {
        let f = c + n;
        mb.quad([f - u - v, f + u - v, f + u + v, f - u + v], n.norm(), [[0.5, 0.5]; 4], col, FLAT);
    }
}

/// Cheap repeatable noise for the city.
fn hash(k: u32) -> f32 {
    let mut x = k.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

/// The colour a telegraph is drawn in.
fn tele_col(c: Cause) -> [u8; 3] {
    match c {
        Cause::Jab | Cause::BloodRing | Cause::Thrust | Cause::Arc | Cause::Charge => [235, 26, 38],
        Cause::AshPool => [200, 170, 255],
        _ => [255, 120, 40],
    }
}

/// Everything the vault adds to the scene.
pub fn draw(op: &mut MB, ad: &mut MB, al: &mut MB, cam: &Cam, g: &Game) {
    gate(ad, al, cam, g);
    bank_booth(op, ad, cam, g);
    pets(op, ad, cam, g);
    weather(ad, al, cam, g);
    let Some(r) = g.raid.as_ref() else { return };
    if crate::raid::night(g, cam.pos) > 0.0 {
        night(op, ad, al, cam, g);
    }
    let t = g.time;
    let floor = |p: V3| v3(p.x, VY + 0.04, p.z);
    // ---- the Warden's plates (and the rising wax)
    if r.room == 0 {
        for (k, (x, z)) in vault::PLATES.iter().enumerate() {
            let c = v3(*x, VY + 0.05, *z);
            let lit = r.plates[k];
            sector(al, c, 0.0, vault::PLATE_R, 0.0, std::f32::consts::TAU, lina([255, 200, 90], 0.15 + 0.5 * lit));
            ring(ad, c, vault::PLATE_R, 0.12, lina([255, 210, 120], 0.4 + 0.6 * lit));
        }
        if r.flood > 0.01 {
            let rm = ROOMS[0];
            let y = VY + 0.06 + r.flood * 0.35;
            up_quad(al, [v3(rm.x.0, y, rm.z.0), v3(rm.x.1, y, rm.z.0), v3(rm.x.1, y, rm.z.1), v3(rm.x.0, y, rm.z.1)], lina([236, 200, 120], 0.35 + 0.3 * r.flood));
        }
    }
    // ---- the bosses that aren't fighters: their models, posed (boss_draw)
    boss_draw::draw_all(op, ad, al, cam, g, r);
    for f in r.foes.iter().filter(|f| f.alive) {
        let p = if g.is_authority() { f.pos } else { f.shown };
        let m = M3::rot_y(f.yaw);
        let fwd = yaw_fwd(f.yaw);
        match f.kind {
            FoeKind::Thrall => {
                // an ink thrall: a crawling blot of ink with violet eyes, wobbling as it goes
                let ink = lin([18, 16, 30]);
                let wob = (t * 7.0 + p.x * 1.3).sin() * 0.06;
                op.ellipsoid(p + v3(0.0, 0.55, 0.0), &m, v3(0.45 + wob, 0.55 - wob, 0.45), ink, -1.0, 1.4, 0.0);
                op.ellipsoid(p + v3(0.0, 1.1, 0.0) + fwd * 0.05, &m, v3(0.28, 0.25, 0.28), ink, -1.0, 1.4, 0.0);
                for s in [-1.0f32, 1.0] {
                    ad.billboard(cam, p + v3(0.0, 1.13, 0.0) + fwd * 0.25 + yaw_right(f.yaw) * (0.1 * s), 0.06, lina([180, 120, 255], 1.0));
                }
                al.billboard(cam, v3(p.x, VY + 0.05, p.z) - fwd * 0.6, 0.5, lina([12, 8, 24], 0.6));
            }
            FoeKind::Regent if !r.regent.vanished => regent_extras(ad, al, cam, g, r, p, f.yaw),
            _ => {}
        }
    }
    // ---- the telegraphs
    for tl in &r.teles {
        let k = tl.k();
        let c = floor(tl.pos);
        let col = tele_col(tl.cause);
        let fill = lina(col, 0.10 + 0.32 * k);
        let edge = lina(col, 0.55 + 0.45 * k);
        let tau = std::f32::consts::TAU;
        match tl.kind {
            TeleKind::Circle => {
                sector(al, c, 0.0, tl.r * k, 0.0, tau, fill);
                ring(ad, c, tl.r, 0.1, edge);
                if tl.cause == Cause::AshPool && tl.t < 0.15 {
                    // the lightning bolt itself
                    let mut a = c + v3(0.0, 22.0, 0.0);
                    for s in 0..6 {
                        let b = c + v3(hash(s as u32 + (tl.pos.x * 10.0) as u32) - 0.5, 22.0 - (s + 1) as f32 * 3.7, hash(s as u32 * 7 + 3) - 0.5);
                        ad.beam(cam, a, b, 0.16, lina([230, 220, 255], 1.0));
                        a = b;
                    }
                }
            }
            TeleKind::Cone => {
                sector(al, c, 0.0, tl.r * k, tl.dir - tl.len, tl.dir + tl.len, fill);
                sector(ad, c, tl.r - 0.08, tl.r + 0.04, tl.dir - tl.len, tl.dir + tl.len, edge);
            }
            TeleKind::Line => {
                let fw = yaw_fwd(tl.dir);
                let side = yaw_right(tl.dir) * tl.r;
                let a = c - fw * 0.5;
                let b = c + fw * tl.len;
                let mid = a + (b - a) * k;
                up_quad(al, [a - side, a + side, mid + side, mid - side], fill);
                for s in [-1.0f32, 1.0] {
                    let o = side * s;
                    let w = side.norm() * 0.06;
                    up_quad(ad, [a + o - w, a + o + w, b + o + w, b + o - w], edge);
                }
            }
            TeleKind::Ring => {
                let cur = tl.r * k;
                ring(al, c, cur, 1.8, lina(col, 0.45));
                ring(ad, c, cur, 0.25, edge);
            }
            TeleKind::Donut => {
                // the blast: from the safe hug outwards; the hug itself glows pale
                let r0 = raid::DONUT_SAFE;
                sector(al, c, r0, r0 + (tl.r - r0) * k, 0.0, tau, fill);
                ring(ad, c, tl.r, 0.12, edge);
                sector(al, c, 0.0, r0, 0.0, tau, lina([255, 236, 210], 0.18));
                ring(ad, c, r0, 0.1, lina([255, 240, 220], 0.9));
            }
            TeleKind::Arc => {
                // all round him, except the wedge straight behind
                let back = tl.dir + std::f32::consts::PI;
                sector(al, c, 0.0, tl.r * k, back + tl.len, back + tau - tl.len, fill);
                sector(ad, c, tl.r - 0.08, tl.r + 0.04, back + tl.len, back + tau - tl.len, edge);
                sector(al, c, 0.4, tl.r, back - tl.len, back + tl.len, lina([255, 236, 210], 0.14));
            }
        }
    }
    // ---- the Regent's blood waves
    if r.room == 3 {
        let roof = ROOMS[3];
        for w in &r.waves {
            let (lo, hi) = if w.axis < 2 { roof.z } else { roof.x };
            let mut cuts: Vec<(f32, f32)> = w.gaps.iter().take(w.ngaps as usize).map(|g| (g - raid::GAP_HALF, g + raid::GAP_HALF)).collect();
            cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut spans = Vec::new();
            let mut from = lo;
            for (a, b) in cuts {
                if a > from {
                    spans.push((from, a));
                }
                from = from.max(b);
            }
            if hi > from {
                spans.push((from, hi));
            }
            let at = |along: f32, across: f32, y: f32| if w.axis < 2 { v3(along, VY + y, across) } else { v3(across, VY + y, along) };
            let dir = if w.axis % 2 == 0 { 1.0 } else { -1.0 };
            let surge = (t * 7.0).sin() * 0.1;
            for (a, b) in spans {
                for (off, h, alpha) in [(-raid::WAVE_HALF, 1.6, 0.55), (0.0, 2.2 + surge, 0.7), (raid::WAVE_HALF, 1.2, 0.45)] {
                    let x = w.pos + off * dir;
                    let n = if w.axis < 2 { v3(dir, 0.0, 0.0) } else { v3(0.0, 0.0, dir) };
                    al.quad([at(x, a, 0.0), at(x, b, 0.0), at(x, b, h), at(x, a, h)], n, [[0.5, 0.5]; 4], lina([150, 6, 18], alpha), FLAT);
                }
                // the foaming crest
                let steps = ((b - a) / 0.6).ceil() as usize;
                for s in 0..=steps {
                    let across = a + (b - a) * s as f32 / steps.max(1) as f32;
                    let bob = (t * 6.0 + across).sin() * 0.15;
                    ad.billboard(cam, at(w.pos, across, 2.2 + bob), 0.45, lina([255, 40, 50], 0.35));
                }
            }
            // the gaps shine faintly on the floor so they read from afar
            for g2 in w.gaps.iter().take(w.ngaps as usize) {
                let c = at(w.pos, *g2, 0.05);
                ad.billboard(cam, c + v3(0.0, 0.4, 0.0), 0.5, lina([255, 220, 200], 0.25));
            }
        }
        // the four giant spikes over their targets, and the cue
        let rg = r.regent;
        if rg.mode == raid::RM_SPIKES {
            for m in &r.members {
                let s = m.slot as usize;
                if rg.spikes & (1 << s) == 0 || s >= g.f.len() {
                    continue;
                }
                let p = g.f[s].pos;
                let k = (1.0 - rg.spike_t / 3.2).clamp(0.0, 1.0);
                // (a crystal of blood hanging over them, coming down as the cue nears)
                let tip = p + v3(0.0, 2.3 + rg.spike_t * 0.9, 0.0);
                op.tube(tip + v3(0.0, 3.2, 0.0), tip, 0.55, 0.02, v3(1.0, 0.0, 0.0), 0.8, lin([110, 6, 18]), -1.0, 1.8);
                for q in 0..3 {
                    let a = q as f32 * 2.1 + t * 0.6;
                    let side = v3(a.sin(), 0.0, a.cos()) * 0.35;
                    op.tube(tip + v3(0.0, 2.6, 0.0) + side, tip + v3(0.0, 0.9, 0.0) + side * 0.6, 0.22, 0.02, v3(1.0, 0.0, 0.0), 0.8, lin([150, 10, 24]), -1.0, 1.8);
                }
                ad.billboard(cam, tip + v3(0.0, 1.2, 0.0), 1.0, lina([255, 30, 40], 0.3 + 0.4 * k));
                ad.beam(cam, tip, v3(p.x, p.y + 0.1, p.z), 0.04, lina([255, 60, 60], 0.5 * k));
                // the cue: the floor ring closes on you; inside the window it flashes white
                let cue = rg.spike_t <= 0.9;
                ring(ad, floor(p), 0.6 + 2.4 * (1.0 - k), 0.14, if cue { lina([255, 250, 240], 0.9) } else { lina([255, 40, 50], 0.8) });
                sector(al, floor(p), 0.0, 0.6, 0.0, std::f32::consts::TAU, lina([255, 40, 50], 0.3));
            }
        }
    }
    // ---- the pools
    for pl in &r.pools {
        if pl.r <= 0.05 {
            continue;
        }
        let c = v3(pl.pos.x, VY + 0.03, pl.pos.z);
        let tau = std::f32::consts::TAU;
        match pl.kind {
            3 => {
                sector(al, c, 0.0, pl.r, 0.0, tau, lina([60, 4, 10], 0.88));
                sector(al, c, 0.0, pl.r * 0.7, 0.0, tau, lina([120, 10, 22], 0.5));
                ring(ad, c, pl.r, 0.12, lina([255, 30, 40], 0.5 + 0.2 * (t * 4.0 + pl.pos.x).sin()));
                for e in 0..3 {
                    let ph = (t * 0.7 + e as f32 * 0.33 + pl.pos.z * 0.1).fract();
                    let a = e as f32 * 2.1 + pl.pos.x;
                    ad.billboard(cam, c + v3(a.sin() * pl.r * 0.5, ph * 1.4, a.cos() * pl.r * 0.5), 0.07, lina([255, 80, 60], 1.0 - ph));
                }
            }
            2 => {
                sector(al, c + v3(0.0, 0.03, 0.0), 0.0, pl.r, 0.0, tau, lina([255, 110, 30], 0.35));
                ring(ad, c, pl.r, 0.2, lina([255, 140, 50], 0.8));
            }
            1 => sector(al, c, 0.0, pl.r, 0.0, tau, lina([20, 14, 40], 0.85)),
            _ => {
                sector(al, c, 0.0, pl.r, 0.0, tau, lina([240, 210, 140], 0.85));
                ring(ad, c, pl.r, 0.1, lina([255, 200, 100], 0.4));
            }
        }
    }
    // ---- what's thrown
    for s in &r.shots {
        match s.kind {
            3 => {
                op.ellipsoid(s.pos, &M3::rot_y(t * 5.0), v3(0.22, 0.22, 0.22), lin([30, 0, 6]), -1.0, 1.5, 0.0);
                ad.billboard(cam, s.pos, 0.7, lina([255, 20, 40], 0.7));
                ad.beam(cam, s.pos - s.vel.norm() * 1.2, s.pos, 0.12, lina([255, 30, 40], 0.5));
            }
            0 => {
                op.tube(s.pos, s.pos + v3(0.0, 0.02, 0.0), 0.08, 0.08, v3(1.0, 0.0, 0.0), 1.0, lin([255, 210, 90]), -1.0, 2.0);
                ad.billboard(cam, s.pos, 0.18, lina([255, 220, 120], 0.6));
            }
            _ => {
                ad.billboard(cam, s.pos, 0.35, lina([255, 160, 80], 0.9));
                ad.beam(cam, s.pos - s.vel.norm() * 0.8, s.pos, 0.06, lina([255, 200, 120], 0.6));
            }
        }
    }
    // ---- the Ledger's chains between two of you
    for (a, b, _, stretched) in &r.tethers {
        let (a, b) = (*a as usize, *b as usize);
        if a < g.f.len() && b < g.f.len() {
            let pa = g.f[a].pos + v3(0.0, 1.1, 0.0);
            let pb = g.f[b].pos + v3(0.0, 1.1, 0.0);
            let taut = (pa - pb).len() > raid::TETHER_LEN || *stretched > 0.0;
            ad.beam(cam, pa, pb, 0.05, if taut { lina([255, 60, 50], 0.9) } else { lina([180, 170, 200], 0.6) });
        }
    }
    // ---- the chest
    if let Some(c) = &r.chest {
        let p = c.pos + v3(0.0, VY - c.pos.y, 0.0);
        let wood = lin(if c.rare { [70, 30, 110] } else { [96, 60, 32] });
        let band = lin(if c.rare { [210, 150, 255] } else { [230, 190, 90] });
        let m = M3::I;
        op.obox(p + v3(0.0, 0.32, 0.0), &m, v3(0.7, 0.32, 0.45), wood, -1.0, 0.4);
        op.obox(p + v3(0.0, 0.32, 0.0), &m, v3(0.72, 0.06, 0.47), band, -1.0, 1.8);
        let lid = if c.open { M3::rot_x(-1.1) } else { M3::I };
        let hinge = p + v3(0.0, 0.64, -0.45);
        op.obox(hinge + lid.z * 0.45 + lid.y * 0.1, &lid, v3(0.7, 0.1, 0.45), wood, -1.0, 0.4);
        let glow = if c.rare { [190, 90, 255] } else { [255, 200, 110] };
        let pulse = 0.7 + 0.3 * (t * 3.0).sin();
        ad.billboard(cam, p + v3(0.0, 0.9, 0.0), if c.open { 1.6 } else { 0.9 } * pulse, lina(glow, if c.open { 0.6 } else { 0.35 }));
        if c.rare && c.open {
            // the purple moment: a column of light
            for k in 0..8 {
                ad.billboard(cam, p + v3(0.0, 0.8 + k as f32 * 0.7, 0.0), 0.9 - k as f32 * 0.08, lina([170, 80, 255], 0.35 - k as f32 * 0.035));
            }
        }
    }
    // ---- the downed (a revive ring), the marked (an icon over the head)
    let mark = if r.room == 3 && (r.regent.mv != NO_MOVE || matches!(r.regent.mode, raid::RM_JABS | raid::RM_STEPS | raid::RM_SPELL)) { r.regent.mark } else { r.call.map_or(255, |c| c.2) };
    for m in &r.members {
        let s = m.slot as usize;
        if s >= g.f.len() {
            continue;
        }
        let p = g.f[s].prev_pos.lerp(g.f[s].pos, 1.0);
        if m.downed {
            let c = v3(p.x, p.y + 0.04, p.z);
            ring(ad, c, raid::REVIVE_RANGE, 0.06, lina([120, 200, 255], 0.5));
            let k = (m.revive / raid::REVIVE_TIME).clamp(0.0, 1.0);
            if k > 0.0 {
                sector(al, c, 0.5, 0.75, 0.0, std::f32::consts::TAU * k, lina([140, 220, 255], 0.8));
            }
            ad.billboard(cam, p + v3(0.0, 1.6 + (t * 3.0).sin() * 0.1, 0.0), 0.22, lina([120, 200, 255], 0.9));
        }
        if m.slot == mark && r.stage == Stage::Fight {
            let head = p + v3(0.0, 2.45 + (t * 5.0).sin() * 0.06, 0.0);
            let s2 = 0.28;
            let (rt, up) = (cam.right * s2, cam.up * s2 * 1.3);
            let col = lina([255, 30, 40], 1.0);
            ad.quad([head - up, head + rt, head + up, head - rt], -cam.fwd, [[0.5, 0.5]; 4], col, FLAT);
            ad.billboard(cam, head, 0.7, lina([255, 40, 50], 0.35));
            ring(ad, v3(p.x, p.y + 0.05, p.z), 0.9, 0.1, lina([255, 40, 50], 0.8));
        }
    }
}

/// The Regent's own effects: his glowing red eyes, and each move's look as it winds up.
fn regent_extras(ad: &mut MB, al: &mut MB, cam: &Cam, g: &Game, r: &raid::Raid, p: V3, yaw: f32) {
    let t = g.time;
    let fwd = yaw_fwd(yaw);
    let rt = yaw_right(yaw);
    let eye = p + v3(0.0, 2.02, 0.0) + fwd * 0.12;
    for s in [-1.0f32, 1.0] {
        ad.billboard(cam, eye + rt * (0.042 * s), 0.035, lina([255, 30, 30], 1.0));
        ad.billboard(cam, eye + rt * (0.042 * s), 0.09, lina([255, 20, 20], 0.45));
    }
    let rg = r.regent;
    let tl = r.teles.iter().find(|x| matches!(x.cause, Cause::Jab | Cause::BloodRing | Cause::Thrust | Cause::Arc | Cause::Charge));
    let k = tl.map_or(0.0, |x| x.k());
    match rg.mv {
        raid::MV_JAB => {
            // the red flash, with a pop
            let flash = if k > 0.6 { 1.0 } else { 0.5 + 0.5 * (t * 30.0).sin().abs() };
            ad.billboard(cam, p + v3(0.0, 1.3, 0.0), 1.3, lina([255, 20, 30], 0.45 * flash));
        }
        raid::MV_RING => {
            // red energy drawn in, then the blast
            for n in 0..16 {
                let a = n as f32 / 16.0 * std::f32::consts::TAU + t * 2.0;
                let d = 4.5 * (1.0 - k) + 0.3;
                ad.billboard(cam, p + v3(a.sin() * d, 1.0 + (n % 3) as f32 * 0.4, a.cos() * d), 0.2, lina([255, 30, 40], 0.9));
            }
            ad.billboard(cam, p + v3(0.0, 1.2, 0.0), 0.8 + 0.8 * k, lina([200, 10, 20], 0.5));
        }
        raid::MV_THRUST => {
            // the lance shines along its line, both ways
            for s in [-1.0f32, 1.0] {
                ad.beam(cam, p + v3(0.0, 1.3, 0.0), p + v3(0.0, 1.3, 0.0) + fwd * (s * (2.0 + 4.0 * k)), 0.08, lina([255, 60, 60], 0.7));
            }
        }
        raid::MV_ARC => {
            // the spear curls round him
            let n = 20;
            for s in 0..n {
                let a = yaw + std::f32::consts::PI + 0.6 + (std::f32::consts::TAU - 1.2) * s as f32 / n as f32;
                let q = p + yaw_fwd(a) * 1.8 + v3(0.0, 1.1, 0.0);
                ad.billboard(cam, q, 0.16, lina([255, 50, 60], 0.3 + 0.6 * k));
            }
        }
        _ => {}
    }
    if rg.mode == raid::RM_STAGGER {
        for n in 0..5 {
            let a = t * 4.0 + n as f32 * 1.26;
            ad.billboard(cam, p + v3(a.sin() * 0.4, 2.55, a.cos() * 0.4), 0.09, lina([255, 245, 200], 0.9));
        }
        let _ = al;
    }
}

/// The Vault Gate in the hub: a circle of embers (brighter with more of you in it).
fn gate(ad: &mut MB, al: &mut MB, cam: &Cam, g: &Game) {
    if g.hub.is_none() {
        return;
    }
    let (x, z, gr) = vault::GATE;
    let c = v3(x, 0.03, z);
    if (c - cam.pos).len() > 45.0 {
        return;
    }
    let inside = g.f.iter().filter(|f| f.present && !f.is_bot && (f.pos - c).flat().len() < gr && f.pos.y > -5.0).count();
    let busy = g.raid.is_some();
    let col = if busy { [120, 110, 130] } else { [255, 120, 50] };
    let k = 0.35 + 0.15 * inside as f32;
    sector(al, c, 0.0, gr, 0.0, std::f32::consts::TAU, lina(col, 0.12 + 0.05 * inside as f32));
    ring(ad, c, gr, 0.12, lina(col, 0.5 + k));
    let n = 18;
    for s in 0..n {
        let a = s as f32 / n as f32 * std::f32::consts::TAU + g.time * 0.4;
        let h = ((g.time * 0.8 + s as f32 * 0.37).fract()) * 1.6;
        ad.billboard(cam, c + v3(a.sin() * gr, 0.1 + h, a.cos() * gr), 0.07, lina(col, (1.0 - h / 1.6) * 0.9));
    }
}

/// The Regent's roof at night: a dark sky low over it (hiding the hub's underside), stars, the
/// red moon, and a city of dark towers with lit windows all round.
fn night(op: &mut MB, ad: &mut MB, al: &mut MB, cam: &Cam, g: &Game) {
    let (rx, rz) = vault::ROOF_CENTRE;
    let c = v3(rx, VY, rz);
    // the sky: a lid over everything, and its stars
    let top = VY + 32.0;
    let e = 180.0;
    let sky = lina([10, 4, 12], 1.0);
    al.quad([v3(rx - e, top, rz - e), v3(rx + e, top, rz - e), v3(rx + e, top, rz + e), v3(rx - e, top, rz + e)], -V3::UP, [[0.5, 0.5]; 4], sky, FLAT);
    for s in 0..90u32 {
        let a = hash(s) * std::f32::consts::TAU;
        let d = 30.0 + hash(s + 500) * 120.0;
        let tw = 0.5 + 0.5 * (g.time * (1.0 + hash(s + 900) * 3.0) + s as f32).sin();
        ad.billboard(cam, v3(rx + a.sin() * d, top - 0.5, rz + a.cos() * d), 0.12 + 0.12 * hash(s + 77), lina([255, 230, 230], 0.25 + 0.4 * tw));
    }
    // the red moon (kept at the same bearing from wherever you stand, so it sits far off)
    // (to the north: behind him as you come up onto the roof)
    let dir = v3(0.14, 0.33, 0.93).norm();
    let mc = cam.pos + dir * 62.0;
    let mc = v3(mc.x, mc.y.min(top - 9.0), mc.z);
    let to_cam = (cam.pos - mc).norm();
    let u = to_cam.cross(V3::UP).norm();
    let w = u.cross(to_cam).norm();
    let rad = 7.5;
    let n = 40;
    for s in 0..n {
        let a0 = s as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (s + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let p0 = mc + (u * a0.cos() + w * a0.sin()) * rad;
        let p1 = mc + (u * a1.cos() + w * a1.sin()) * rad;
        al.quad([mc, p0, p1, mc], to_cam, [[0.5, 0.5]; 4], lina([230, 46, 40], 1.0), FLAT);
    }
    // its darker seas
    for (ox, oy, rr) in [(-0.3f32, 0.2f32, 0.28f32), (0.25, -0.15, 0.22), (0.05, 0.42, 0.14)] {
        let sc = mc + (u * ox + w * oy) * rad + to_cam * 0.05;
        for s in 0..16 {
            let a0 = s as f32 / 16.0 * std::f32::consts::TAU;
            let a1 = (s + 1) as f32 / 16.0 * std::f32::consts::TAU;
            al.quad([sc, sc + (u * a0.cos() + w * a0.sin()) * rad * rr, sc + (u * a1.cos() + w * a1.sin()) * rad * rr, sc], to_cam, [[0.5, 0.5]; 4], lina([170, 26, 26], 1.0), FLAT);
        }
    }
    ad.billboard(cam, mc - to_cam * 0.1, rad * 2.2, lina([255, 40, 40], 0.22));
    ad.billboard(cam, mc - to_cam * 0.1, rad * 4.0, lina([200, 20, 30], 0.10));
    // a dark wall all round far off (no daylight sky between the towers)
    let wall = lina([14, 6, 14], 1.0);
    let n = 48;
    for s in 0..n {
        let a0 = s as f32 / n as f32 * std::f32::consts::TAU;
        let a1 = (s + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let (p0, p1) = (c + v3(a0.sin(), 0.0, a0.cos()) * 125.0, c + v3(a1.sin(), 0.0, a1.cos()) * 125.0);
        let nn = (c - p0).flat().norm();
        op.quad([p0 - V3::UP * 45.0, p1 - V3::UP * 45.0, p1 + V3::UP * 33.0, p0 + V3::UP * 33.0], nn, [[0.5, 0.5]; 4], wall, FLAT);
    }
    // the city: towers in rings round the roof, windows lit
    let tower = lina([16, 12, 22], 1.0);
    for b in 0..56u32 {
        let a = b as f32 / 56.0 * std::f32::consts::TAU + hash(b) * 0.08;
        let d = 58.0 + hash(b + 100) * 40.0;
        let wdt = 3.0 + hash(b + 200) * 4.5;
        let h = 6.0 + hash(b + 300) * 26.0;
        let base = c + v3(a.sin() * d, -18.0, a.cos() * d);
        let ctr = base + v3(0.0, h * 0.5 + 9.0, 0.0);
        flat_box(op, ctr, v3(wdt * 0.5, h * 0.5 + 9.0, wdt * 0.5), tower);
        // a spire on some
        if hash(b + 400) > 0.7 {
            op.tube(ctr + v3(0.0, h * 0.5 + 9.0, 0.0), ctr + v3(0.0, h * 0.5 + 15.0, 0.0), 0.6, 0.05, v3(1.0, 0.0, 0.0), 1.0, tower, -1.0, 0.0);
            ad.billboard(cam, ctr + v3(0.0, h * 0.5 + 15.2, 0.0), 0.35, lina([255, 40, 40], 0.5 + 0.5 * (g.time * 2.0 + b as f32).sin().abs()));
        }
        // windows on the face toward the roof
        let face = (c - base).flat().norm();
        let side = v3(face.z, 0.0, -face.x);
        let front = ctr + face * (wdt * 0.5 + 0.05);
        let cols = (wdt / 1.1) as i32;
        let rows = ((h + 18.0) / 1.6) as i32;
        for row in 0..rows {
            for col in 0..cols {
                let kk = b * 977 + row as u32 * 31 + col as u32;
                if hash(kk) > 0.42 {
                    continue;
                }
                let x = (col as f32 - (cols as f32 - 1.0) * 0.5) * 1.1;
                let y = -(h * 0.5 + 9.0) + 1.0 + row as f32 * 1.6;
                let warm = if hash(kk + 7) > 0.8 { [255, 150, 90] } else { [255, 205, 130] };
                ad.billboard(cam, front + side * x + V3::UP * y, 0.22, lina(warm, 0.55 + 0.4 * hash(kk + 3)));
            }
        }
    }
}

/// The bank's booth: coin stacks and a ledger on the counter, strongboxes behind, a hanging
/// sign with a gold coin.
fn bank_booth(op: &mut MB, ad: &mut MB, cam: &Cam, g: &Game) {
    if g.hub.is_none() {
        return;
    }
    let (kx, _, kz) = crate::varrow::BANK;
    let c = v3(kx, 0.0, kz);
    if (c - cam.pos).len() > 40.0 {
        return;
    }
    let gold = lin([226, 170, 60]);
    let top = 1.12;
    // coin stacks
    for (k, (dz, h)) in [(-1.1f32, 5), (-0.85, 3), (0.9, 6), (1.15, 2)].iter().enumerate() {
        for q in 0..*h {
            let p = c + v3(-0.05 + (k % 2) as f32 * 0.08, top + 0.012 + q as f32 * 0.022, *dz);
            op.tube(p, p + v3(0.0, 0.018, 0.0), 0.05, 0.05, v3(1.0, 0.0, 0.0), 1.0, gold, -1.0, 2.0);
        }
    }
    // the ledger, open, with a quill
    let m = M3::rot_y(0.3);
    op.obox(c + v3(0.0, top + 0.02, 0.1), &m, v3(0.2, 0.015, 0.28), lin([70, 26, 20]), -1.0, 0.3);
    op.obox(c + v3(0.0, top + 0.04, 0.1), &m, v3(0.18, 0.008, 0.26), lin([232, 222, 196]), -1.0, 0.1);
    op.tube(c + v3(0.05, top + 0.05, 0.25), c + v3(0.2, top + 0.3, 0.38), 0.006, 0.002, v3(1.0, 0.0, 0.0), 1.0, lin([240, 236, 226]), -1.0, 0.2);
    // strongboxes behind the counter, banded in brass
    for (k, z) in [-1.2f32, 1.2].iter().enumerate() {
        let p = c + v3(1.75, 0.3 + k as f32 * 0.0, *z);
        op.obox(p, &M3::I, v3(0.3, 0.3, 0.42), lin([60, 40, 26]), -1.0, 0.3);
        for dz in [-0.3f32, 0.3] {
            op.obox(p + v3(0.0, 0.0, dz), &M3::I, v3(0.31, 0.31, 0.03), gold, -1.0, 2.0);
        }
        op.obox(p + v3(-0.31, 0.08, 0.0), &M3::I, v3(0.02, 0.06, 0.05), gold, -1.0, 2.0);
    }
    // the hanging sign: a dark board on two chains, a gold coin on it
    let sign = c + v3(-0.62, 2.75, 0.0);
    for dz in [-0.45f32, 0.45] {
        op.tube(sign + v3(0.0, 0.3, dz), sign + v3(0.0, 0.62, dz), 0.012, 0.012, v3(1.0, 0.0, 0.0), 1.0, lin([60, 60, 66]), -1.0, 1.5);
    }
    op.obox(sign, &M3::I, v3(0.04, 0.3, 0.62), lin([34, 50, 34]), -1.0, 0.3);
    op.obox(sign, &M3::I, v3(0.045, 0.32, 0.64), lin([200, 150, 50]), -1.0, 1.5);
    let coin_at = sign + v3(-0.06, 0.0, 0.0);
    op.tube(coin_at, coin_at + v3(-0.03, 0.0, 0.0), 0.22, 0.22, v3(0.0, 1.0, 0.0), 1.0, gold, -1.0, 2.2);
    op.tube(coin_at + v3(-0.03, 0.0, 0.0), coin_at + v3(-0.045, 0.0, 0.0), 0.15, 0.15, v3(0.0, 1.0, 0.0), 1.0, lin([250, 210, 100]), -1.0, 2.2);
    let shine = 0.5 + 0.5 * (g.time * 1.5).sin();
    ad.billboard(cam, coin_at + v3(-0.08, 0.0, 0.0), 0.35 + 0.05 * shine, lina([255, 210, 110], 0.25));
}

/// Pets: the Cinder Wisp follows whoever wears it, bobbing over their shoulder.
fn pets(op: &mut MB, ad: &mut MB, cam: &Cam, g: &Game) {
    // (and the menu's fighter, previewing a season pet)
    if g.hub.is_none() && g.kind != crate::game::Kind::Demo {
        return;
    }
    for (i, f) in g.f.iter().enumerate() {
        if !f.present || f.hub.look.pet == 0 || f.hub.townsfolk {
            continue;
        }
        let t = g.time + i as f32 * 1.3;
        let back = yaw_fwd(f.yaw) * -0.7 + yaw_right(f.yaw) * 0.55;
        let p = f.pos + back + v3((t * 1.1).sin() * 0.12, 1.75 + (t * 2.3).sin() * 0.1, (t * 0.9).cos() * 0.12);
        if (p - cam.pos).len() > 40.0 {
            continue;
        }
        // an ember core, a soft glow, a trail of sparks, two dark eyes looking ahead (a season pet
        // in its own colour)
        let (core, glow, spark) = match crate::season::pet_colour(f.hub.look.pet) {
            Some(c) => (c, [(c[0] as u16 * 4 / 5) as u8, (c[1] as u16 * 4 / 5) as u8, (c[2] as u16 * 4 / 5) as u8], [c[0].saturating_add(40), c[1].saturating_add(40), c[2].saturating_add(40)]),
            None => ([255, 150, 40], [255, 120, 30], [255, 140, 40]),
        };
        op.ellipsoid(p, &M3::rot_y(f.yaw), v3(0.11, 0.13, 0.11), lin(core), -1.0, 0.0, 0.0);
        ad.billboard(cam, p, 0.32, lina(glow, 0.55));
        ad.billboard(cam, p + v3(0.0, 0.1, 0.0), 0.16, lina([255, 230, 150], 0.8));
        for k in 0..4 {
            let ph = (t * 0.8 + k as f32 * 0.25).fract();
            let q = p - yaw_fwd(f.yaw) * (0.1 + ph * 0.4) + v3((k as f32 * 2.1 + t).sin() * 0.05, -ph * 0.25, 0.0);
            ad.billboard(cam, q, 0.05 * (1.0 - ph), lina(spark, 1.0 - ph));
        }
        let fw = yaw_fwd(f.yaw);
        for s in [-1.0f32, 1.0] {
            op.ellipsoid(p + fw * 0.1 + yaw_right(f.yaw) * (0.04 * s) + v3(0.0, 0.02, 0.0), &M3::I, v3(0.018, 0.026, 0.018), lin([40, 14, 8]), -1.0, 0.0, 0.0);
        }
    }
}

/// The hub's weather an admin set (crate::roles): rain falling round the camera.
fn weather(ad: &mut MB, al: &mut MB, cam: &Cam, g: &Game) {
    if g.hub.is_none() || g.admin.weather != 1 || cam.pos.y < -10.0 {
        return;
    }
    let t = g.time;
    for k in 0..260u32 {
        let x = hash(k) * 30.0 - 15.0;
        let z = hash(k + 1000) * 30.0 - 15.0;
        let fall = ((t * 1.4 + hash(k + 2000)).fract()) * 12.0;
        let p = v3((cam.pos.x / 30.0).floor() * 0.0 + cam.pos.x + x, cam.pos.y + 7.0 - fall, cam.pos.z + z);
        if p.y < 0.0 {
            continue;
        }
        al.beam(cam, p, p - v3(0.15, 0.6, 0.0), 0.008, lina([180, 195, 215], 0.35));
    }
    let _ = ad;
}
