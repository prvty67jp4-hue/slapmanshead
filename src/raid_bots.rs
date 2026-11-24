//! Bots in the Ashen Vault (crate::raid): a solo player's 1-3 allies, or bots filling a party.
//! They play each room's mechanic in a role the room gives them (by their place among the
//! party's bots), dodge what's telegraphed on the floor and what pools on it, and go to whoever is
//! down to revive them (the host counts a bot standing over a downed ally as holding E).
//!
//! - The Warden: the first bots hold the plates (as many as are needed), the next tanks.
//! - The Assessors: half on each; they stand beyond their own Assessor so the two are drawn apart,
//!   keep their health level and finish both together.
//! - The Ledger: chained bots close the distance; thralls are shot first.
//! - The Regent: marked, a bot answers his move (back from the jab, into him on the ring,
//!   sideways from the thrust, behind him on the arc); across a wave it slides to a gap and
//!   goes straight through; on the spikes it hides behind a chimney or guards on the cue; in a
//!   jab chain it parries each jab; out of a charge's path, then guards the spell.
//! - The Magistrate: off a condemned island (jump-dashing across), apart on SPREAD, onto the
//!   marked ally on STACK.

use crate::game::*;
use crate::math::*;
use crate::raid::*;
use crate::vault::{self, ROOMS, VY};

/// Where a party member is among the bots (0 the first bot), and how many bots.
fn bot_rank(r: &Raid, i: usize) -> (usize, usize) {
    let bots: Vec<u8> = r.members.iter().filter(|m| m.bot).map(|m| m.slot).collect();
    // (a player isn't counted among them: they come after every bot)
    (bots.iter().position(|&s| s as usize == i).unwrap_or(bots.len()), bots.len())
}

/// How many players (not bots) are in the party.
fn humans(r: &Raid) -> usize {
    r.members.iter().filter(|m| !m.bot).count()
}

fn steer(inp: &mut Input, me: &Fighter, yaw: f32, goal: V3, slow: f32) {
    let d = (goal - me.pos).flat();
    if d.len() < slow {
        return;
    }
    let dir = d.norm();
    inp.fwd = dir.dot(yaw_fwd(yaw));
    inp.right = dir.dot(yaw_right(yaw));
}

/// The bot's input this tick.
pub fn input(g: &Game, i: usize, _dt: f32) -> Input {
    let me = &g.f[i];
    let mut inp = Input { yaw: me.yaw, pitch: me.pitch, ..Default::default() };
    let Some(r) = g.raid.as_ref() else { return inp };
    if r.is_downed(i) {
        return inp;
    }
    let pos = me.pos;
    let (rank, nbots) = bot_rank(r, i);
    let room = ROOMS[r.room];
    // between rooms: on to the next one (through the corridor's middle)
    match r.stage {
        Stage::Cleared => {
            if r.room < 4 {
                let next = ROOMS[r.room + 1];
                let goal = match room.out {
                    Some((x, z)) if room.contains(pos, 0.0) => v3((x.0 + x.1) * 0.5, VY, (z.0 + z.1) * 0.5),
                    _ => next.centre(),
                };
                let yaw = (goal - pos).flat().x.atan2((goal - pos).flat().z);
                inp.yaw = yaw;
                steer(&mut inp, me, yaw, goal, 0.5);
            }
            return inp;
        }
        Stage::Fight => {}
        _ => return inp,
    }
    // ---- what to shoot, where to stand
    let alive: Vec<(usize, &Foe)> = r.foes.iter().enumerate().filter(|(_, f)| f.alive).collect();
    let boss = r.boss().filter(|b| b.alive);
    let mut target: Option<usize> = None;
    let mut stand: Option<V3> = None;
    let mut range = 9.0;
    match r.room {
        0 => {
            let need = plates_needed(r.members.len());
            // players hold no plates in a bot's reckoning: the bots take them first
            if rank < need.min(nbots) && !(humans(r) == 0 && rank == nbots - 1) {
                let (x, z) = vault::PLATES[rank];
                stand = Some(v3(x, VY, z));
                range = 0.4;
            } else {
                range = 3.2;
            }
            target = alive.iter().find(|(_, f)| f.kind == FoeKind::Warden).map(|x| x.0);
        }
        1 => {
            let tally = alive.iter().find(|(_, f)| f.kind == FoeKind::Tally).copied();
            let seal = alive.iter().find(|(_, f)| f.kind == FoeKind::Seal).copied();
            let mine = if rank % 2 == 0 { tally.or(seal) } else { seal.or(tally) };
            if let (Some(t), Some(s)) = (tally, seal) {
                let (ft, fs) = (t.1.hp / t.1.max, s.1.hp / s.1.max);
                // keep them level; hold off one about to fall until the other is close too
                target = if (ft - fs).abs() > 0.1 { Some(if ft > fs { t.0 } else { s.0 }) } else { mine.map(|m| m.0) };
                if let Some(tk) = target {
                    let f = r.foes[tk].hp / r.foes[tk].max;
                    let other = if tk == t.0 { fs } else { ft };
                    if f < 0.06 && other > 0.12 {
                        target = Some(if tk == t.0 { s.0 } else { t.0 });
                    }
                }
                // stand beyond my own Assessor, so it's drawn away from its twin
                // (each side takes its Assessor to opposite corners of the court, so they part)
                if let Some((_, mf)) = mine {
                    // (far enough from it that it follows: Tally keeps 7 m off, Seal comes close)
                    let other = if mf.kind == FoeKind::Tally { s.1.pos } else { t.1.pos };
                    let away = (mf.pos - other).flat();
                    let away = if away.len() > 0.1 { away.norm() } else { v3(1.0, 0.0, 0.0) };
                    let _ = away;
                    // the spot in the court far from the other twin, at a distance mine will follow to
                    let keep = if mf.kind == FoeKind::Tally { 8.5 } else { 3.5 };
                    let (x0, x1, z0, z1) = (room.x.0 + 2.5, room.x.1 - 2.5, room.z.0 + 2.5, room.z.1 - 2.5);
                    let spots = [v3(x0, VY, z0), v3(x1, VY, z0), v3(x0, VY, z1), v3(x1, VY, z1), v3((x0 + x1) * 0.5, VY, z0), v3((x0 + x1) * 0.5, VY, z1), v3(x0, VY, (z0 + z1) * 0.5), v3(x1, VY, (z0 + z1) * 0.5)];
                    let score = |p: &V3| (*p - other).flat().len() - ((*p - mf.pos).flat().len() - keep).abs() * 0.7;
                    stand = spots.iter().copied().max_by(|a, b| score(a).total_cmp(&score(b)));
                    range = 1.0;
                }
            } else {
                target = tally.or(seal).map(|x| x.0);
            }
        }
        2 => {
            // chained: to my partner
            for &(a, b, _, _) in &r.tethers {
                let other = if a as usize == i { Some(b) } else if b as usize == i { Some(a) } else { None };
                if let Some(o) = other.and_then(|o| g.f.get(o as usize)) {
                    // (both make for the point between them)
                    stand = Some((o.pos + pos) * 0.5);
                    range = 0.5;
                }
            }
            let ledger = alive.iter().find(|(_, f)| f.kind == FoeKind::Ledger).map(|x| (x.0, x.1.pos));
            let thrall = alive.iter().filter(|(_, f)| f.kind == FoeKind::Thrall).min_by(|a, b| {
                let c = ledger.map_or(V3::ZERO, |l| l.1);
                (a.1.pos - c).flat().len().total_cmp(&(b.1.pos - c).flat().len())
            });
            target = thrall.map(|t| t.0).or(ledger.map(|l| l.0));
            if stand.is_none() {
                range = 10.0;
            }
            // (unchained: a post round the book, out of its page storm, shooting what comes)
            if stand.is_none() {
                if let Some((_, lp)) = ledger {
                    let k = r.members.iter().position(|m| m.slot as usize == i).unwrap_or(0);
                    let a = k as f32 * std::f32::consts::TAU / r.members.len().max(1) as f32 + 0.4;
                    stand = Some(v3(lp.x + a.sin() * 7.5, VY, lp.z + a.cos() * 5.5));
                    range = 0.8;
                }
            }
        }
        3 => {
            let bk = alive.iter().find(|(_, f)| f.kind == FoeKind::Regent).map(|x| x.0);
            let rg = r.regent;
            let marked = rg.mark as usize == i;
            if let Some(k) = bk.filter(|_| !rg.vanished) {
                target = Some(k);
                range = 4.5;
                let b = &r.foes[k];
                let fwd = v3(b.yaw.sin(), 0.0, b.yaw.cos());
                let away = (pos - b.pos).flat();
                let away = if away.len() > 0.1 { away.norm() } else { -fwd };
                if marked && rg.mode == RM_MOVES && rg.mv != NO_MOVE {
                    match rg.mv {
                        MV_JAB => {
                            stand = Some(b.pos + away * 6.5);
                            range = 0.3;
                        }
                        MV_RING => {
                            stand = Some(b.pos);
                            range = 0.6;
                            inp.dash = (pos - b.pos).flat().len() > 3.0 && me.dash_charges >= 1.0;
                        }
                        MV_THRUST => {
                            let side = v3(fwd.z, 0.0, -fwd.x);
                            let s = if away.dot(side) >= 0.0 { 1.0 } else { -1.0 };
                            stand = Some(b.pos + side * (3.2 * s) + fwd * fwd.dot(pos - b.pos));
                            range = 0.3;
                        }
                        MV_ARC => {
                            stand = Some(b.pos - fwd * 1.5);
                            range = 0.35;
                            inp.dash = (pos - (b.pos - fwd * 1.5)).flat().len() > 3.5 && me.dash_charges >= 1.0;
                        }
                        _ => {}
                    }
                }
                // the jab chain: blade out, parry each jab as it lands
                if rg.mode == RM_JABS && marked {
                    if me.cur != 0 {
                        inp.select = Some(0);
                    }
                    inp.guard = rg.mv != NO_MOVE && rg.wind < 0.32;
                    inp.fire = false;
                    stand = None;
                    target = None;
                }
            }
            // the spikes: behind a chimney (from him), or guard on the cue
            if rg.mode == RM_SPIKES && rg.spikes & (1 << i) != 0 {
                let from = bk.map_or(v3(vault::ROOF_CENTRE.0, VY, vault::ROOF_CENTRE.1), |k| r.foes[k].pos);
                let ch = vault::CHIMNEYS.iter().map(|(x, z)| v3(*x, VY, *z)).min_by(|a, b| (*a - pos).flat().len().total_cmp(&(*b - pos).flat().len())).unwrap_or(from);
                let behind = ch + (ch - from).flat().norm() * (vault::CHIMNEY_HALF + 0.7);
                if (behind - pos).flat().len() / 5.0 < rg.spike_t - 0.4 {
                    stand = Some(behind);
                    range = 0.25;
                } else {
                    if me.cur != 0 {
                        inp.select = Some(0);
                    }
                    inp.guard = rg.spike_t < 0.45;
                    target = None;
                }
            }
            // the waves: slide (straight, across their way) to a gap before one reaches you
            //   (the nearest one still coming: line up with its gap early, then hold still)
            let coming = r.waves.iter().filter_map(|w| {
                let (along, across) = if w.axis < 2 { (pos.x, pos.z) } else { (pos.z, pos.x) };
                let ahead = if w.axis % 2 == 0 { along - w.pos } else { w.pos - along };
                (ahead > -0.7 && ahead < 22.0).then_some((ahead, across, w))
            }).min_by(|a, b| a.0.total_cmp(&b.0));
            // (a wall passing through you right now: hold still in its gap till it's gone)
            let passing = r.waves.iter().any(|w| {
                let along = if w.axis < 2 { pos.x } else { pos.z };
                (along - w.pos).abs() < WAVE_HALF + 1.2
            });
            if passing && !r.waves.is_empty() {
                stand = Some(pos);
                range = 5.0;
                inp.fwd = 0.0;
                inp.right = 0.0;
            } else if let Some((_, across, w)) = coming {
                let gap = w.gaps.iter().take(w.ngaps as usize).copied().min_by(|a, b| (a - across).abs().total_cmp(&(b - across).abs())).unwrap_or(across);
                // (and in off the edge it comes from: the roof's middle gives time to line up)
                let rf = ROOMS[3];
                let to = if w.axis < 2 { v3(pos.x.clamp(rf.x.0 + 4.0, rf.x.1 - 4.0), VY, gap) } else { v3(gap, VY, pos.z.clamp(rf.z.0 + 4.0, rf.z.1 - 4.0)) };
                stand = Some(to);
                range = 0.15;
                inp.dash = (gap - across).abs() > 3.0 && me.dash_charges >= 1.0;
            }
            // a charge coming: off his path (sideways to where he comes from); his spell: guard
            if marked && rg.mode == RM_STEPS {
                let (sx, sz) = side_point(rg.side, ROOMS[3], 0.5);
                let path = (pos - v3(sx, VY, sz)).flat();
                let side = v3(path.z, 0.0, -path.x).norm();
                if rg.t < 0.9 {
                    stand = Some(pos + side * 3.5);
                    range = 0.2;
                }
                if me.cur != 0 {
                    inp.select = Some(0);
                }
            }
            if marked && rg.mode == RM_SPELL {
                if me.cur != 0 {
                    inp.select = Some(0);
                }
            }
            if r.shots.iter().any(|s| s.kind == 3 && (s.pos - pos).len() < 5.0) && me.cur == 0 {
                inp.guard = true;
            }
        }
        _ => {
            target = boss.map(|_| r.foes.iter().position(|f| f.kind == FoeKind::Magistrate).unwrap_or(0));
            if r.phase == 2 {
                // an island that isn't condemned (not where the fire is)
                let here = nearest_island(pos);
                let banned = std::iter::once(0usize).chain(r.condemned.map(|c| c.0 as usize)).chain(r.pools.iter().filter(|p| p.kind == 2).map(|p| nearest_island(p.pos)).filter_map(|c| vault::ISLANDS.iter().position(|x| *x == c))).collect::<Vec<_>>();
                let here_k = vault::ISLANDS.iter().position(|x| *x == here).unwrap_or(0);
                // (one of you to each outer island, so one Verdict can't catch you all)
                //  (stay put unless it's condemned: every crossing over the fire is a risk; then the
                //  nearest island that isn't)
                let goal_k = if !banned.contains(&here_k) {
                    here_k
                } else {
                    // the outer island fewest of the party are on
                    let crowd = |k: usize| {
                        let (ix, iz) = vault::ISLANDS[k];
                        r.members.iter().filter(|m| m.slot as usize != i && !m.downed).filter_map(|m| g.f.get(m.slot as usize)).filter(|f| (f.pos - v3(ix, VY, iz)).flat().len() < vault::ISLAND_HALF + 0.5).count()
                    };
                    {
                        let me_k = r.members.iter().position(|m| m.slot as usize == i).unwrap_or(0);
                        (1..5).filter(|k| !banned.contains(k)).min_by_key(|k| crowd(*k) * 10 + (*k + me_k) % 4).unwrap_or(0)
                    }
                };
                // (the outer islands only join through the middle one: cross by way of it)
                let hop = if here_k != 0 && goal_k != here_k { 0 } else { goal_k };
                let (x, z) = vault::ISLANDS[hop];
                stand = Some(v3(x, VY, z));
                range = if hop == goal_k { 0.6 } else { 0.3 };
                // a gap to cross: run, jump, dash (straight across, not on the diagonal)
                if hop != here_k && me.on_ground {
                    let edge = (v3(here.0, 0.0, here.1) - pos).flat().len() > vault::ISLAND_HALF - 0.9;
                    if edge {
                        inp.jump = true;
                        inp.dash = me.dash_charges >= 1.0;
                    }
                }
            }
            if let Some((c, _, m)) = r.call {
                let b = boss.map_or(ROOMS[4].centre(), |b| b.pos);
                match c {
                    Call::Stack => {
                        if let Some(f) = g.f.get(m as usize) {
                            stand = Some(f.pos);
                            range = 1.0;
                        }
                    }
                    Call::Spread => {
                        let k = r.members.iter().position(|mm| mm.slot as usize == i).unwrap_or(0);
                        let a = k as f32 * std::f32::consts::TAU / r.members.len().max(1) as f32;
                        stand = Some(b + v3(a.sin() * 9.0, 0.0, a.cos() * 9.0));
                        range = 0.8;
                    }
                }
            }
        }
    }
    // ---- someone down: go to them (if there are revives left)
    if r.revives_left > 0 {
        let down = r.members.iter().filter(|m| m.downed).filter_map(|m| g.f.get(m.slot as usize).map(|f| f.pos)).min_by(|a, b| (*a - pos).flat().len().total_cmp(&(*b - pos).flat().len()));
        if let Some(d) = down.filter(|d| (*d - pos).flat().len() < 16.0) {
            // (unless holding a plate the others need, or chained)
            let chained = r.tethers.iter().any(|t| t.0 as usize == i || t.1 as usize == i);
            let busy = r.room == 0 && stand.is_some() && rank < plates_needed(r.members.len());
            if !chained && (!busy || rank == nbots.saturating_sub(1)) {
                stand = Some(d);
                range = 0.9;
            }
        }
    }
    // ---- aim and fire
    let mut yaw = me.yaw;
    if let Some(k) = target {
        let f = &r.foes[k];
        let aim = f.centre() + v3((g.time * 1.7 + i as f32).sin() * 0.2, 0.0, 0.0);
        let eye = me.eye();
        let to = aim - eye;
        let flat = v3(to.x, 0.0, to.z).len().max(0.01);
        yaw = to.x.atan2(to.z);
        inp.yaw = yaw;
        inp.pitch = to.y.atan2(flat);
        let dist = flat;
        let melee_close = dist < 2.6;
        let want: u8 = if melee_close { 0 } else { 1 };
        if me.cur != want as usize {
            inp.select = Some(want);
        }
        if g.world.line_clear(eye, aim) || melee_close {
            inp.fire = true;
            inp.fire_pressed = (g.time * 7.0 + i as f32 * 0.3).fract() < 0.5;
        }
        if !me.weapon.is_melee() && me.mag[me.cur.min(2)] == 0 {
            inp.reload = true;
        }
        if stand.is_none() {
            // keep a distance that suits
            let d = (f.pos - pos).flat();
            if d.len() > range + 1.5 {
                stand = Some(f.pos);
            } else if d.len() < range - 2.0 {
                // back off, but round towards the room's middle
                let to_mid = (room.centre() - pos).flat();
                let to_mid = if to_mid.len() > 0.1 { to_mid.norm() } else { V3::ZERO };
                stand = Some(pos + (-d.norm() + to_mid * 0.8).flat().norm() * 3.0);
            }
        }
    }
    if let Some(goal) = stand {
        steer(&mut inp, me, yaw, goal, range.max(0.35));
    }
    // ---- don't walk back into something about to land (a little margin round it)
    let danger = |p: V3| {
        r.teles.iter().any(|t| {
            !(r.room == 3 && matches!(t.cause, Cause::Jab | Cause::BloodRing | Cause::Thrust | Cause::Arc))
                && t.kind != TeleKind::Ring
                && [V3::ZERO, v3(0.7, 0.0, 0.0), v3(-0.7, 0.0, 0.0), v3(0.0, 0.0, 0.7), v3(0.0, 0.0, -0.7)].iter().any(|o| t.covers(p + *o))
        }) || r.waves.iter().any(|w| {
            // (a blood wave's wall, with room to spare: never walk up into one, even from behind)
            let (along, across) = if w.axis < 2 { (p.x, p.z) } else { (p.z, p.x) };
            (along - w.pos).abs() < WAVE_HALF + 1.0 && !w.gaps.iter().take(w.ngaps as usize).any(|gp| (across - gp).abs() < GAP_HALF - 0.2)
        })
    };
    let wish = yaw_fwd(yaw) * inp.fwd + yaw_right(yaw) * inp.right;
    if wish.len() > 0.05 && !danger(pos) && [0.4f32, 0.8, 1.2].iter().any(|k| danger(pos + wish.norm() * *k)) {
        inp.fwd = 0.0;
        inp.right = 0.0;
    }
    // ---- dodge: out of whatever is about to land, out of the wax and the fire
    let mut push = V3::ZERO;
    for t in &r.teles {
        // (the Regent's four moves are answered above, by whoever is marked: running from them
        // the usual way is wrong for the ring and the arc)
        if r.room == 3 && matches!(t.cause, Cause::Jab | Cause::BloodRing | Cause::Thrust | Cause::Arc) {
            continue;
        }
        if t.kind == TeleKind::Ring {
            let cur = t.r * t.k();
            if ((pos - t.pos).flat().len() - cur).abs() < 2.0 && me.on_ground {
                inp.jump = true;
            }
            continue;
        }
        if t.covers(pos) && t.t < t.total * 0.9 {
            let away = match t.kind {
                TeleKind::Line => {
                    let fwd = v3(t.dir.sin(), 0.0, t.dir.cos());
                    let d = (pos - t.pos).flat();
                    let side = d - fwd * d.dot(fwd);
                    if side.len() > 0.05 { side.norm() } else { v3(fwd.z, 0.0, -fwd.x) }
                }
                _ => {
                    // out, and round towards the room's middle (never into a corner)
                    let d = (pos - t.pos).flat();
                    let out = if d.len() > 0.05 { d.norm() } else { v3(1.0, 0.0, 0.0) };
                    let side = v3(out.z, 0.0, -out.x);
                    let to_mid = (room.centre() - pos).flat();
                    let side = if side.dot(to_mid) >= 0.0 { side } else { -side };
                    let near_wall = pos.x < room.x.0 + 4.0 || pos.x > room.x.1 - 4.0 || pos.z < room.z.0 + 4.0 || pos.z > room.z.1 - 4.0;
                    let k = if t.kind == TeleKind::Cone || near_wall { 1.0 } else { 0.4 };
                    (out + side * k + if near_wall { to_mid.norm() * 0.6 } else { V3::ZERO }).flat().norm()
                }
            };
            // (chained and already far apart: dodge less, stay together)
            let partner_far = r.tethers.iter().any(|c| (c.0 as usize == i || c.1 as usize == i) && g.f.get(if c.0 as usize == i { c.1 as usize } else { c.0 as usize }).is_some_and(|o| (o.pos - pos).flat().len() > TETHER_LEN - 1.5));
            push = push + away * if partner_far { 0.35 } else { 1.0 };
            // about to land and still inside: dash out
            if t.t < t.total * 0.6 && me.dash_charges >= 1.0 {
                inp.dash = true;
            }
        }
    }
    for p in &r.pools {
        // (a condemned island is left by the way across, above: not by running off its edge)
        if p.kind == 2 {
            continue;
        }
        let d = (pos - p.pos).flat();
        if d.len() < p.r + 0.6 && (pos.y - p.pos.y).abs() < 1.5 {
            // (a plate-holder stays on the plate through a little wax)
            if !(r.room == 0 && stand.is_some_and(|s| (s - pos).flat().len() < vault::PLATE_R)) {
                push = push + if d.len() > 0.05 { d.norm() } else { v3(0.0, 0.0, 1.0) };
            }
        }
    }
    for s in &r.shots {
        let to_me = pos + v3(0.0, 1.2, 0.0) - s.pos;
        let along = to_me.dot(s.vel.norm());
        if along > 0.0 && along < 6.0 && (to_me - s.vel.norm() * along).len() < 0.9 {
            push = push + v3(s.vel.z, 0.0, -s.vel.x).norm();
        }
    }
    // (on the Magistrate's islands over the fire: never dodge off the edge)
    if r.room == 4 && r.floor_down && push.len() > 0.01 {
        let (ix, iz) = nearest_island(pos);
        let next = pos + push.flat().norm() * 1.2;
        let half = vault::ISLAND_HALF - 0.5;
        if (next.x - ix).abs() > half || (next.z - iz).abs() > half {
            // (a Verdict line can be left by either side: take the one that stays on the island)
            let _ = (ix, iz);
            push = -push;
        }
    }
    if push.len() > 0.01 {
        let dir = push.flat().norm();
        inp.fwd = dir.dot(yaw_fwd(yaw));
        inp.right = dir.dot(yaw_right(yaw));
    }
    let _ = room;
    inp
}

/// (host) Bots standing over a downed ally hold E for them.
pub fn host_hands(g: &mut Game) {
    let positions: Vec<(usize, V3)> = g.f.iter().enumerate().map(|(k, f)| (k, f.pos)).collect();
    let Some(r) = g.raid.as_mut() else { return };
    let downed: Vec<V3> = r.members.iter().filter(|m| m.downed).filter_map(|m| positions.get(m.slot as usize).map(|p| p.1)).collect();
    for m in &mut r.members {
        if !m.bot {
            continue;
        }
        let p = positions.get(m.slot as usize).map_or(V3::ZERO, |p| p.1);
        m.holding = !m.downed && downed.iter().any(|d| (*d - p).flat().len() < REVIVE_RANGE);
    }
}