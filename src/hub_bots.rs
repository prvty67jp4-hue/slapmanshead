//! Varrow's regulars: the bots that make the town square feel lived in. Each picks something to
//! do, walks there along the square's paths and does it: sits on a bench and chats, plays at a
//! table (against another regular or the table's bot), struts the catwalk and strikes a pose on
//! the podium, watches a duel from the pits' walkway or a game from beside its table, or stops to
//! wave, laugh or clap. They say yes to most challenges and fight their duels with the usual bot.
//! Only the host runs them.

use crate::boards::Level;
use crate::game::*;
use crate::hub::*;
use crate::math::*;
use crate::varrow;

/// What a regular is up to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Plan {
    /// Deciding (for this long).
    Idle(f32),
    /// Walking to a spot, then doing something there.
    Go(V3, Then),
    /// Sitting (for this long).
    Sitting(f32),
    /// At a table (until the game's over, or this long passes with nobody to play).
    Playing(usize, f32),
    /// Watching something (for this long), facing it.
    Watching(V3, f32),
    /// Doing an emote (for this long).
    Emoting(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Then {
    Sit(u16),
    Table(usize, u8),
    Watch(V3),
    Emote(Emote),
    /// The catwalk: the podium next.
    Strut(u8),
}

#[derive(Clone, Debug)]
pub struct Regular {
    pub plan: Plan,
    /// The way there: points still to reach.
    pub path: Vec<V3>,
    /// A challenge being thought over (id, seconds left).
    pub consider: Option<(u16, f32)>,
    rng: crate::boards::Rng,
    stuck: f32,
    last_pos: V3,
}

impl Default for Regular {
    fn default() -> Self {
        Regular::new(1)
    }
}

impl Regular {
    pub fn new(seed: u64) -> Regular {
        Regular { plan: Plan::Idle(0.5 + (seed % 5) as f32 * 0.6), path: Vec::new(), consider: None, rng: crate::boards::Rng(seed * 7919 + 13), stuck: 0.0, last_pos: V3::ZERO }
    }
}

/// The square's walking paths: points with clear lines between them (on the ground, or the
/// terrace and walkway over the pits).
pub fn nodes() -> Vec<V3> {
    vec![
        v3(0.0, 0.0, -12.0),
        v3(11.0, 0.0, -11.0),
        v3(-11.0, 0.0, -11.0),
        v3(-14.0, 0.0, 0.0),
        v3(14.0, 0.0, 0.0),
        v3(-11.0, 0.0, 11.0),
        v3(11.0, 0.0, 11.0),
        v3(0.0, 0.0, 11.5),
        // the steps up to the terrace, the terrace, the walkway each side
        v3(0.0, 4.5, 23.0),
        v3(-12.0, 4.5, 21.8),
        v3(12.0, 4.5, 21.8),
        // the pavilion's side, the tailor's, the catwalk's foot, the Gallery door
        v3(-19.5, 0.0, -4.0),
        v3(-8.0, 0.0, -26.0),
        v3(6.0, 0.0, -15.5),
        v3(21.0, 0.0, -31.0),
        v3(19.0, 0.0, -20.0),
    ]
}

/// Can you walk straight from a to b (nothing in the way at knee and chest height)?
fn clear(w: &crate::world::World, a: V3, b: V3) -> bool {
    // (the steps up to the terrace are the one way up)
    if (a.y - b.y).abs() > 1.0 {
        let (lo, hi) = if a.y < b.y { (a, b) } else { (b, a) };
        return lo.x.abs() < 1.0 && (lo.z - 11.5).abs() < 1.0 && hi.x.abs() < 1.0 && (hi.z - 23.0).abs() < 1.0;
    }
    [0.6f32, 1.4].iter().all(|&h| w.line_clear(a + v3(0.0, h, 0.0), b + v3(0.0, h, 0.0)))
}

/// The way from `a` to `b` along the paths (just `b` when it's in sight).
pub fn route(w: &crate::world::World, a: V3, b: V3) -> Vec<V3> {
    if clear(w, a, b) {
        return vec![b];
    }
    let ns = nodes();
    let n = ns.len();
    // (start, the nodes, the goal) and the shortest way through them
    let mut pts = vec![a];
    pts.extend(ns);
    pts.push(b);
    let m = n + 2;
    let mut dist = vec![f32::MAX; m];
    let mut prev = vec![usize::MAX; m];
    let mut done = vec![false; m];
    dist[0] = 0.0;
    for _ in 0..m {
        let Some(u) = (0..m).filter(|&k| !done[k] && dist[k] < f32::MAX).min_by(|&x, &y| dist[x].total_cmp(&dist[y])) else { break };
        done[u] = true;
        if u == m - 1 {
            break;
        }
        for v in 0..m {
            if done[v] || v == u {
                continue;
            }
            let d = (pts[u] - pts[v]).len();
            if dist[u] + d < dist[v] && clear(w, pts[u], pts[v]) {
                dist[v] = dist[u] + d;
                prev[v] = u;
            }
        }
    }
    if prev[m - 1] == usize::MAX {
        return vec![b];
    }
    let mut way = Vec::new();
    let mut k = m - 1;
    while k != 0 && k != usize::MAX {
        way.push(pts[k]);
        k = prev[k];
    }
    way.reverse();
    way
}

/// Where to stand to watch something at `p` (a pit's duel from the walkway, a table from its side).
fn watch_spot(rng: &mut crate::boards::Rng, p: V3, pit: bool) -> V3 {
    if pit {
        v3(p.x + (rng.unit() - 0.5) * 8.0, varrow::PIT_WALL, 21.7)
    } else {
        let a = rng.unit() * std::f32::consts::TAU;
        // round the table, clear of its chairs (east-west of it)
        let off = v3(a.sin() * 0.3, 0.0, if a.cos() >= 0.0 { 1.5 } else { -1.5 });
        v3(p.x + off.x, varrow::DECK, p.z + off.z)
    }
}

/// Decide what each regular does next (the host, once a tick).
pub fn think_all(g: &mut Game, dt: f32) {
    let n = g.f.len();
    for s in 0..n {
        if !g.f[s].present || !g.f[s].is_bot || s == g.local {
            continue;
        }
        if g.hub.as_ref().is_none_or(|h| h.pit_of(s).is_some()) {
            continue;
        }
        let Some(mut r) = g.hub.as_mut().and_then(|h| h.regulars.get_mut(s).map(std::mem::take)) else { continue };
        think(g, s, &mut r, dt);
        if let Some(h) = g.hub.as_mut() {
            if let Some(slot) = h.regulars.get_mut(s) {
                *slot = r;
            }
        }
    }
}

fn think(g: &mut Game, s: usize, r: &mut Regular, dt: f32) {
    // a challenge: most say yes after a moment
    if let Some((id, t)) = r.consider {
        let t = t - dt;
        if t <= 0.0 {
            r.consider = None;
            let yes = r.rng.unit() < 0.85;
            g.hub_remote(s, HubEv::Answer { id, by: s as u8, accept: yes });
            return;
        }
        r.consider = Some((id, t));
    }
    // an open challenge on the board: now and then someone takes it
    let open = g.hub.as_ref().and_then(|h| h.challenges.iter().find(|c| c.to == 255 && c.from as usize != s).copied());
    if let Some(c) = open {
        if matches!(r.plan, Plan::Idle(_) | Plan::Watching(..)) && r.rng.unit() < dt * 0.15 {
            g.hub_remote(s, HubEv::Answer { id: c.id, by: s as u8, accept: true });
            return;
        }
    }
    let pos = g.f[s].pos;
    if (pos - r.last_pos).len() < 0.02 && matches!(r.plan, Plan::Go(..)) {
        r.stuck += dt;
    } else {
        r.stuck = 0.0;
    }
    r.last_pos = pos;
    match r.plan {
        Plan::Idle(t) => {
            if t - dt > 0.0 {
                r.plan = Plan::Idle(t - dt);
            } else {
                choose(g, s, r);
            }
        }
        Plan::Go(to, then) => {
            if r.stuck > 3.0 {
                r.plan = Plan::Idle(1.0);
                r.path.clear();
                return;
            }
            let reach = if matches!(then, Then::Sit(_) | Then::Table(..)) { 0.9 } else { 0.7 };
            if let Some(&next) = r.path.first() {
                if (next - pos).flat().len() < reach && (next.y - pos.y).abs() < 1.2 {
                    r.path.remove(0);
                }
            }
            if r.path.is_empty() || (to - pos).flat().len() < reach {
                r.path.clear();
                arrive(g, s, r, then);
            }
        }
        Plan::Sitting(t) => {
            if t - dt <= 0.0 || g.f[s].hub.seat == SEAT_NONE {
                g.hub_remote(s, HubEv::Sit { who: s as u8, seat: SEAT_NONE });
                r.plan = Plan::Idle(0.5);
            } else {
                r.plan = Plan::Sitting(t - dt);
                // a word now and then (the talking-while-seated look is the sit emote's own)
                if r.rng.unit() < dt * 0.04 {
                    let e = [Emote::Laugh, Emote::Clap, Emote::Wave][r.rng.below(3)];
                    let _ = e;
                }
            }
        }
        Plan::Playing(t, left) => {
            let (over, alone) = g.hub.as_ref().map_or((true, true), |h| {
                let tb = &h.tables[t];
                (tb.outcome != crate::boards::Outcome::Playing, tb.seats.iter().filter(|x| **x != Sitter::Empty).count() < 2)
            });
            let left = left - dt;
            if alone && left < 50.0 {
                // nobody came: play the table's bot
                g.hub_remote(s, HubEv::Table { table: t as u8, who: s as u8, act: TableAct::Bot([Level::Easy, Level::Medium][r.rng.below(2)]) });
            }
            if over || left <= 0.0 || g.f[s].hub.seat == SEAT_NONE {
                if over && r.rng.unit() < 0.5 && left > 0.0 {
                    // a rematch
                    g.hub_remote(s, HubEv::Table { table: t as u8, who: s as u8, act: TableAct::Reset });
                    r.plan = Plan::Playing(t, left.min(120.0));
                } else {
                    g.hub_remote(s, HubEv::Sit { who: s as u8, seat: SEAT_NONE });
                    r.plan = Plan::Idle(1.0);
                }
            } else {
                r.plan = Plan::Playing(t, left);
            }
        }
        Plan::Watching(at, t) => {
            if t - dt <= 0.0 {
                r.plan = Plan::Idle(0.5);
            } else {
                r.plan = Plan::Watching(at, t - dt);
                if r.rng.unit() < dt * 0.08 {
                    let e = [Emote::Clap, Emote::Cheer][r.rng.below(2)];
                    g.f[s].hub.emote = e as u8;
                    g.f[s].hub.emote_t = g.time;
                    if g.kind == Kind::Host {
                        g.out_events.push(Ev::Hub(HubEv::Emote { who: s as u8, emote: e as u8 }));
                    }
                }
            }
        }
        Plan::Emoting(t) => {
            r.plan = if t - dt <= 0.0 { Plan::Idle(0.8) } else { Plan::Emoting(t - dt) };
        }
    }
}

/// Pick what to do next.
fn choose(g: &mut Game, s: usize, r: &mut Regular) {
    let pos = g.f[s].pos;
    let h = g.hub.as_ref().expect("hub");
    // something worth watching: a duel on, a game in progress
    let duel_at = (0..2).find(|&p| h.pits[p].is_some()).map(|p| v3((varrow::PITS[p].x.0 + varrow::PITS[p].x.1) * 0.5, 0.0, 28.0));
    let game_at = h.tables.iter().enumerate().find(|(_, t)| t.playing() && t.seats.iter().all(|x| matches!(x, Sitter::Player(_)))).map(|(k, _)| k);
    let free_table = h.tables.iter().enumerate().filter(|(_, t)| t.seats.iter().any(|x| *x == Sitter::Empty) && !t.seats.iter().any(|x| matches!(x, Sitter::Bot(_)))).map(|(k, t)| (k, t.seats.iter().position(|x| *x == Sitter::Empty).unwrap_or(0))).collect::<Vec<_>>();
    let roll = r.rng.unit();
    let world = &g.world;
    let mut go = |r: &mut Regular, to: V3, then: Then| {
        r.path = route(world, pos, to);
        r.plan = Plan::Go(to, then);
    };
    if let Some(at) = duel_at.filter(|_| roll < 0.45) {
        let spot = watch_spot(&mut r.rng, at, true);
        go(r, spot, Then::Watch(at));
        return;
    }
    if roll < 0.3 && !free_table.is_empty() {
        let (t, side) = free_table[r.rng.below(free_table.len())];
        let seat = TABLE_SEATS + (t * 2 + side) as u16;
        if let Some((p, yaw)) = seat_pose(seat) {
            let fwd = v3(yaw.sin(), 0.0, yaw.cos());
            go(r, v3(p.x, varrow::DECK, p.z) + fwd * 0.1, Then::Table(t, side as u8));
            return;
        }
    }
    if let Some(t) = game_at.filter(|_| roll < 0.42) {
        let (x, z, _) = varrow::TABLES[t];
        let spot = watch_spot(&mut r.rng, v3(x, 0.0, z), false);
        go(r, spot, Then::Watch(v3(x, varrow::TABLE_TOP, z)));
        return;
    }
    if roll < 0.62 {
        // a bench (or the fountain's rim)
        let seats = varrow::seats();
        for _ in 0..6 {
            let k = r.rng.below(seats.len().min(24)) as u16;
            if !g.f.iter().any(|f| f.present && f.hub.seat == k) {
                let (p, yaw) = seats[k as usize];
                let fwd = v3(yaw.sin(), 0.0, yaw.cos());
                go(r, v3(p.x, 0.0, p.z) + fwd * 0.1, Then::Sit(k));
                return;
            }
        }
    }
    if roll < 0.75 {
        // the catwalk
        let ((x0, x1), (_, z1), top) = varrow::CATWALK;
        go(r, v3((x0 + x1) * 0.5, top, z1 - 0.5), Then::Strut(0));
        return;
    }
    // a wander and a gesture
    let a = r.rng.unit() * std::f32::consts::TAU;
    let d = 6.0 + r.rng.unit() * 8.0;
    let to = v3(a.sin() * d, 0.0, a.cos() * d - 2.0);
    let e = [Emote::Wave, Emote::Laugh, Emote::Bow, Emote::Salute, Emote::Dance, Emote::Cheer][r.rng.below(6)];
    go(r, to, Then::Emote(e));
}

fn arrive(g: &mut Game, s: usize, r: &mut Regular, then: Then) {
    match then {
        Then::Sit(seat) => {
            g.hub_remote(s, HubEv::Sit { who: s as u8, seat });
            let t = 12.0 + r.rng.unit() * 25.0;
            r.plan = if g.f[s].hub.seat == seat { Plan::Sitting(t) } else { Plan::Idle(0.5) };
        }
        Then::Table(t, side) => {
            let seat = TABLE_SEATS + (t * 2 + side as usize) as u16;
            g.hub_remote(s, HubEv::Sit { who: s as u8, seat });
            r.plan = if g.f[s].hub.seat == seat { Plan::Playing(t, 60.0 + r.rng.unit() * 60.0) } else { Plan::Idle(0.5) };
        }
        Then::Watch(at) => {
            r.plan = Plan::Watching(at, 10.0 + r.rng.unit() * 15.0);
        }
        Then::Emote(e) => {
            g.f[s].hub.emote = e as u8;
            g.f[s].hub.emote_t = g.time;
            if g.kind == Kind::Host {
                g.out_events.push(Ev::Hub(HubEv::Emote { who: s as u8, emote: e as u8 }));
            }
            r.plan = Plan::Emoting(e.length().unwrap_or(5.0));
        }
        Then::Strut(step) => {
            let ((x0, x1), (z0, _), top) = varrow::CATWALK;
            let ((px0, px1), (pz0, pz1), ptop) = varrow::PODIUM;
            match step {
                0 => {
                    let to = v3((x0 + x1) * 0.5, top, z0 + 0.4);
                    r.path = vec![to];
                    r.plan = Plan::Go(to, Then::Strut(1));
                }
                1 => {
                    let to = v3((px0 + px1) * 0.5, ptop, (pz0 + pz1) * 0.5);
                    r.path = vec![to];
                    r.plan = Plan::Go(to, Then::Strut(2));
                }
                _ => {
                    let e = [Emote::Bow, Emote::Salute, Emote::Wave, Emote::Cheer][r.rng.below(4)];
                    g.f[s].hub.emote = e as u8;
                    g.f[s].hub.emote_t = g.time;
                    if g.kind == Kind::Host {
                        g.out_events.push(Ev::Hub(HubEv::Emote { who: s as u8, emote: e as u8 }));
                    }
                    r.plan = Plan::Emoting(e.length().unwrap_or(3.0) + 0.5);
                }
            }
        }
    }
}

/// A regular's input this tick: walk the path, face what they watch or the seat's way.
pub fn input(g: &Game, s: usize) -> Input {
    let f = &g.f[s];
    let mut inp = Input { yaw: f.yaw, ..Default::default() };
    let Some(r) = g.hub.as_ref().and_then(|h| h.regulars.get(s)) else { return inp };
    match r.plan {
        Plan::Go(to, _) => {
            let next = r.path.first().copied().unwrap_or(to);
            let d = (next - f.pos).flat();
            if d.len() > 0.15 {
                inp.yaw = d.x.atan2(d.z);
                inp.fwd = if d.len() > 1.2 { 1.0 } else { 0.6 };
                // a step that's too tall: hop
                inp.jump = r.stuck > 0.6;
                // walk, don't run, on the catwalk
                if matches!(r.plan, Plan::Go(_, Then::Strut(_))) {
                    inp.sneak = true;
                }
            }
        }
        Plan::Watching(at, _) => {
            let d = (at - f.pos).flat();
            if d.len() > 0.1 {
                inp.yaw = d.x.atan2(d.z);
            }
        }
        _ => {}
    }
    inp
}

/// The Rooftop Run's route for a scripted runner: where to head, and what to do on reaching
/// it (0 just pass, 1 jump, 2 jump and dash in the air).
pub fn course() -> Vec<(V3, u8)> {
    [
        (-17.0, 0.0, -19.0, 0),
        (-19.5, 1.2, -17.3, 0),
        (-19.5, 2.2, -15.9, 0),
        (-19.5, 3.2, -14.5, 0),
        (-21.5, 4.2, -13.5, 0),
        (-27.0, 4.2, -2.0, 0),
        (-28.0, 4.2, 7.4, 1),
        (-28.0, 5.0, 12.0, 0),
        (-28.0, 5.0, 13.0, 2),
        (-29.0, 5.0, 22.0, 0),
        (-27.6, 5.0, 21.7, 0),
        (-23.5, 4.5, 21.7, 0),
        (-14.0, 4.5, 21.8, 0),
        (0.0, 4.5, 21.6, 0),
        (0.0, 0.0, 9.5, 0),
    ]
    .iter()
    .map(|&(x, y, z, a)| (v3(x, y, z), a))
    .collect()
}

/// A scripted runner's progress along the course.
#[derive(Clone, Debug, Default)]
pub struct Runner {
    pub k: usize,
    stuck: f32,
    last: V3,
    dash: bool,
}

/// The scripted runner's input this tick (run, jump onto what's ahead, jump and dash the gap).
pub fn run_input(g: &Game, i: usize, route: &[(V3, u8)], st: &mut Runner) -> Input {
    let f = &g.f[i];
    let (to_p, act) = route[st.k.min(route.len() - 1)];
    let to = to_p - f.pos;
    let flat = to.flat();
    let mut inp = Input { fwd: 1.0, yaw: flat.x.atan2(flat.z), ..Default::default() };
    if f.on_ground && to.y > 0.4 && flat.len() < 2.2 {
        inp.jump = true;
    }
    if flat.len() < 0.7 && (to.y.abs() < 1.2 || act > 0) {
        if act > 0 && f.on_ground {
            inp.jump = true;
            st.dash = act == 2;
        }
        st.k += 1;
    }
    if st.dash && !f.on_ground && f.vel.y < 2.0 {
        inp.dash = true;
        st.dash = false;
    }
    if st.stuck > 0.4 && f.on_ground {
        inp.jump = true;
    }
    st.stuck = if (f.pos - st.last).len() < 0.01 { st.stuck + TICK } else { 0.0 };
    st.last = f.pos;
    if st.k >= route.len() {
        inp.fwd = 0.0;
    }
    inp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_paths_join_everything_up() {
    let _quiet = crate::heavy();
        let w = crate::varrow::world();
        let ns = nodes();
        // every node can reach every other along the paths
        for a in &ns {
            for b in &ns {
                let way = route(&w, *a, *b);
                assert!(!way.is_empty());
                let mut at = *a;
                for p in &way {
                    assert!(clear(&w, at, *p) || (at - *p).len() < 0.01, "{a:?} -> {b:?}: blocked between {at:?} and {p:?}");
                    at = *p;
                }
                assert!((at - *b).len() < 0.01);
            }
        }
    }
}
