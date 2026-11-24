//! The Gallery's aim rooms (Varrow Centre's east side): five drills, played on this computer,
//! alone or two side by side on the same targets (the host hands both lanes the same seed).
//!
//! - Static flicks: 30 targets, one at a time, anywhere in front of you; hit each as fast as you
//!   can (2.5 s each before it's a miss).
//! - Moving tracking: 30 s on one target weaving across the lane: hold your sights on it with the
//!   trigger down.
//! - Wall-run shots: run down the lane; targets come up high by the walls ahead of you; hits in
//!   the air or on a wall count three times over.
//! - Sniper range: ten small targets 38-64 m out, five seconds each.
//! - Sword parry timing: a sparring post swings at you twenty times; raise your guard on the
//!   blow (within 60 ms is perfect).
//!
//! Each gives a score, accuracy and reaction time; scores go to the Gallery's scoreboard.

use crate::boards;
use crate::game::*;
use crate::hub::HubEv;
use crate::math::*;
use crate::varrow;

pub const DRILLS: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drill {
    Flicks = 0,
    Tracking = 1,
    WallRun = 2,
    Sniper = 3,
    Parry = 4,
}

impl Drill {
    pub const ALL: [Drill; DRILLS] = [Drill::Flicks, Drill::Tracking, Drill::WallRun, Drill::Sniper, Drill::Parry];
    pub fn from_u8(v: u8) -> Drill {
        match v {
            1 => Drill::Tracking,
            2 => Drill::WallRun,
            3 => Drill::Sniper,
            4 => Drill::Parry,
            _ => Drill::Flicks,
        }
    }
    pub fn name(self) -> &'static str {
        ["Static Flicks", "Moving Tracking", "Wall-Run Shots", "Sniper Range", "Parry Timing"][self as usize]
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Drill::Flicks => "30 targets, one at a time: snap to each and fire",
            Drill::Tracking => "30 seconds: keep your sights on the weaving target, trigger held",
            Drill::WallRun => "run the lane; shoot the high targets from a wall or the air (x3)",
            Drill::Sniper => "ten small targets 38-64 m out, five seconds each",
            Drill::Parry => "twenty blows from the sparring post: guard on the strike",
        }
    }
    /// What you hold for it.
    pub fn weapon(self) -> Weapon {
        match self {
            Drill::Flicks | Drill::WallRun => Weapon::Revolver,
            Drill::Tracking => Weapon::Smg,
            Drill::Sniper => Weapon::Sniper,
            Drill::Parry => Weapon::Katana,
        }
    }
    fn targets(self) -> u32 {
        match self {
            Drill::Flicks => 30,
            Drill::Tracking => 1,
            Drill::WallRun => 16,
            Drill::Sniper => 10,
            Drill::Parry => 20,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub pos: V3,
    pub r: f32,
    /// When it came up (drill time).
    pub born: f32,
}

/// One blow from the sparring post: its tell begins, then it lands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swing {
    pub tell: f32,
    pub impact: f32,
    pub judged: bool,
}

pub const COUNTDOWN: f32 = 3.0;
pub const TRACK_TIME: f32 = 30.0;
/// Parry timing windows (seconds from the blow).
pub const PERFECT: f32 = 0.06;
pub const GOOD: f32 = 0.15;
pub const LATE: f32 = 0.3;

#[derive(Clone, Debug)]
pub struct AimRun {
    pub drill: Drill,
    pub lane: usize,
    pub seed: u32,
    rng: boards::Rng,
    /// Drill time (negative: the countdown).
    pub t: f32,
    pub target: Option<Target>,
    pub spawned: u32,
    pub hits: u32,
    pub shots: u32,
    react_sum: f32,
    react_n: u32,
    /// Tracking: seconds on target.
    pub on_target: f32,
    pub swings: Vec<Swing>,
    /// Points so far.
    pub points: f32,
    /// The last call ("Perfect!", "+142"), and how long it shows.
    pub call: Option<(String, f32)>,
    pub done: bool,
    /// Side by side with this player, and their score once they've finished.
    pub duo: Option<u8>,
    pub partner: Option<u32>,
    last_since: f32,
    guard_was: bool,
}

impl AimRun {
    pub fn new(drill: Drill, lane: usize, seed: u32, duo: Option<u8>) -> AimRun {
        let mut rng = boards::Rng(seed as u64 * 2654435761 + 1);
        let mut swings = Vec::new();
        if drill == Drill::Parry {
            let mut t = 1.2;
            for _ in 0..drill.targets() {
                let wind = 0.45 + rng.unit() * 0.4;
                swings.push(Swing { tell: t, impact: t + wind, judged: false });
                t += wind + 1.0 + rng.unit() * 0.7;
            }
        }
        AimRun { drill, lane: lane.min(1), seed, rng, t: -COUNTDOWN, target: None, spawned: 0, hits: 0, shots: 0, react_sum: 0.0, react_n: 0, on_target: 0.0, swings, points: 0.0, call: None, done: false, duo, partner: None, last_since: 99.0, guard_was: false }
    }

    /// The drill is on (past the countdown, not finished).
    pub fn live(&self) -> bool {
        self.t >= 0.0 && !self.done
    }

    pub fn accuracy(&self) -> f32 {
        if self.shots == 0 { 0.0 } else { self.hits as f32 / self.shots as f32 }
    }

    /// Mean reaction time in milliseconds (0 when there's none yet).
    pub fn reaction_ms(&self) -> u32 {
        if self.react_n == 0 { 0 } else { (self.react_sum / self.react_n as f32 * 1000.0).round() as u32 }
    }

    /// The final score: the points, with accuracy weighing in for the shooting drills.
    pub fn score(&self) -> u32 {
        let acc = match self.drill {
            Drill::Parry => 1.0,
            Drill::Tracking => 0.85 + 0.15 * self.accuracy(),
            _ => 0.5 + 0.5 * self.accuracy(),
        };
        (self.points * acc).round().max(0.0) as u32
    }

    fn lane_x(&self) -> (f32, f32) {
        varrow::LANES[self.lane].x
    }

    fn centre(&self) -> V3 {
        let (a, b) = self.lane_x();
        v3((a + b) * 0.5, 0.0, varrow::FIRING_Z)
    }

    /// A new target for the shooting drills (`me`: where the player is).
    fn spawn(&mut self, me: V3) {
        let c = self.centre();
        let (x0, x1) = self.lane_x();
        let r = &mut self.rng;
        let pos = match self.drill {
            Drill::Flicks => v3(c.x + (r.unit() - 0.5) * 3.8, 0.7 + r.unit() * 2.5, c.z + 8.0 + r.unit() * 14.0),
            Drill::Sniper => v3(c.x + (r.unit() - 0.5) * 3.6, 0.5 + r.unit() * 2.2, c.z + 38.0 + r.unit() * 26.0),
            Drill::WallRun => {
                let side = if r.unit() < 0.5 { x0 + 0.7 } else { x1 - 0.7 };
                let z = (me.z + 9.0 + r.unit() * 6.0).min(varrow::HALF - 2.0);
                v3(side, 2.4 + r.unit() * 2.0, z)
            }
            _ => c + v3(0.0, 1.5, 12.0),
        };
        let rad = match self.drill {
            Drill::Sniper => 0.3,
            Drill::WallRun => 0.45,
            _ => 0.35,
        };
        self.target = Some(Target { pos, r: rad, born: self.t });
        self.spawned += 1;
    }

    /// Where the tracking target is at drill time `t`.
    pub fn tracking_pos(&self, t: f32) -> V3 {
        let c = self.centre();
        let s = self.seed as f32 * 0.001;
        let x = (t * 0.9 + s).sin() * 1.3 + (t * 2.3 + s * 3.0).sin() * 0.45;
        let y = 1.6 + (t * 1.4 + s * 2.0).sin() * 0.55;
        c + v3(x, y, 12.0)
    }

    /// The sparring post (in front of the lane) and how far its arm is through a swing (0 at
    /// rest, 1 at the blow).
    pub fn post(&self) -> (V3, f32) {
        let c = self.centre() + v3(0.0, 0.0, 2.6);
        let k = self.swings.iter().find(|s| self.t >= s.tell - 0.05 && self.t <= s.impact + 0.25).map_or(0.0, |s| ((self.t - s.tell) / (s.impact - s.tell)).clamp(0.0, 1.0));
        (c, k)
    }

    /// A shot from `o` along `d` (the drill checks it against its target).
    pub fn shot(&mut self, o: V3, d: V3, airborne: bool) {
        if !self.live() || self.drill == Drill::Parry {
            return;
        }
        self.shots += 1;
        if self.drill == Drill::Tracking {
            if let Some(tg) = self.target {
                if ray_sphere(o, d, tg.pos, tg.r) {
                    self.hits += 1;
                }
            }
            return;
        }
        let Some(tg) = self.target else { return };
        if !ray_sphere(o, d, tg.pos, tg.r) {
            return;
        }
        let react = self.t - tg.born;
        self.hits += 1;
        self.react_sum += react;
        self.react_n += 1;
        let pts = match self.drill {
            Drill::Flicks => 100.0 + 60.0 * (1.0 - react / 1.2).max(0.0),
            Drill::Sniper => 150.0 + 100.0 * (1.0 - react / 3.0).max(0.0),
            Drill::WallRun => {
                if airborne { 150.0 + 50.0 * (1.0 - react / 2.0).max(0.0) } else { 50.0 }
            }
            _ => 0.0,
        };
        self.points += pts;
        self.call = Some((format!("+{}{}", pts.round(), if self.drill == Drill::WallRun && airborne { "  AIR" } else { "" }), 0.8));
        self.target = None;
    }

    /// One tick: `me` where the player is, `aim` their sights (from, along), `fire` the trigger
    /// held, `guard` their guard up. Returns true the tick it finishes.
    pub fn step(&mut self, dt: f32, me: V3, aim: (V3, V3), fire: bool, guard: bool) -> bool {
        if self.done {
            return false;
        }
        self.t += dt;
        if let Some((_, t)) = self.call.as_mut() {
            *t -= dt;
        }
        if self.call.as_ref().is_some_and(|c| c.1 <= 0.0) {
            self.call = None;
        }
        if self.t < 0.0 {
            return false;
        }
        match self.drill {
            Drill::Tracking => {
                let pos = self.tracking_pos(self.t);
                self.target = Some(Target { pos, r: 0.45, born: 0.0 });
                if fire && ray_sphere(aim.0, aim.1, pos, 0.45) {
                    self.on_target += dt;
                }
                self.points = self.on_target / TRACK_TIME * 1000.0;
                if self.t >= TRACK_TIME {
                    self.done = true;
                }
            }
            Drill::Parry => {
                let pressed = guard && !self.guard_was;
                self.guard_was = guard;
                let t = self.t;
                if pressed {
                    // the swing this press answers: the one whose blow is nearest
                    if let Some(s) = self.swings.iter_mut().filter(|s| !s.judged && t >= s.tell - 0.3).min_by(|a, b| (a.impact - t).abs().total_cmp(&(b.impact - t).abs())) {
                        let err = (t - s.impact).abs();
                        s.judged = true;
                        let (pts, word) = if t < s.tell {
                            (0.0, "Too early")
                        } else if err <= PERFECT {
                            (100.0, "Perfect!")
                        } else if err <= GOOD {
                            (60.0, "Good")
                        } else if err <= LATE {
                            (20.0, if t < s.impact { "Early" } else { "Late" })
                        } else {
                            (0.0, "Miss")
                        };
                        if pts > 0.0 {
                            self.hits += 1;
                            self.react_sum += (t - s.tell).max(0.0);
                            self.react_n += 1;
                        }
                        self.shots += 1;
                        self.points += pts;
                        self.call = Some((format!("{word}  {:+.0} ms", (t - s.impact) * 1000.0), 0.9));
                    }
                }
                // a blow nobody guarded
                for s in self.swings.iter_mut() {
                    if !s.judged && t > s.impact + LATE {
                        s.judged = true;
                        self.shots += 1;
                        self.call = Some(("Missed the blow".into(), 0.9));
                    }
                }
                if self.swings.iter().all(|s| s.judged) {
                    self.done = true;
                }
            }
            _ => {
                let life = match self.drill {
                    Drill::Flicks => 2.5,
                    Drill::Sniper => 5.0,
                    _ => 4.0,
                };
                if let Some(tg) = self.target {
                    if self.t - tg.born > life {
                        self.target = None;
                        self.call = Some(("Missed".into(), 0.6));
                    }
                }
                if self.target.is_none() {
                    if self.spawned >= self.drill.targets() {
                        self.done = true;
                    } else {
                        self.spawn(me);
                    }
                }
                if self.drill == Drill::WallRun && self.t > 60.0 {
                    self.done = true;
                }
            }
        }
        self.done
    }
}

/// Does a ray (from `o` along unit `d`) pass within `r` of `c`, in front of `o`?
pub fn ray_sphere(o: V3, d: V3, c: V3, r: f32) -> bool {
    let oc = c - o;
    let t = oc.dot(d);
    if t < 0.0 {
        return false;
    }
    (oc - d * t).len() <= r
}

impl Game {
    /// Start a drill in a lane: you're brought to the lane's firing line with its weapon.
    pub fn start_aim(&mut self, drill: Drill, lane: usize, seed: u32, duo: Option<u8>) {
        let me = self.local;
        let lane = lane.min(1);
        let stand = varrow::LANES[lane].stand;
        let w = drill.weapon();
        let f = &mut self.f[me];
        f.hub.seat = crate::hub::SEAT_NONE;
        f.hub.emote = 0;
        f.respawn_at(stand, 0.0);
        f.spawn_prot = 0.0;
        if w.is_melee() {
            f.loadout.melee = w;
        } else {
            f.loadout.gun1 = w;
        }
        f.equip();
        f.cur = if w.is_melee() { 0 } else { 1 };
        f.weapon = w;
        if let Some(h) = self.hub.as_mut() {
            h.aim = Some(AimRun::new(drill, lane, seed, duo));
        }
    }

    /// Leave the aim room (back to your own look's weapon).
    pub fn stop_aim(&mut self) {
        let me = self.local;
        if let Some(h) = self.hub.as_mut() {
            h.aim = None;
        }
        self.f[me].equip();
        self.apply_look_weapon(me);
    }

    /// Ask the player at the other lectern to race this drill side by side.
    pub fn aim_duo(&mut self, to: usize, drill: Drill) {
        let me = self.local as u8;
        self.hub_send(HubEv::AimDuo { from: me, to: to as u8, drill: drill as u8 });
    }

    /// Say yes to a side-by-side drill: the host hands out the same seed to both.
    pub fn aim_duo_yes(&mut self, from: u8, drill: u8) {
        let me = self.local as u8;
        let seed = (self.time * 1000.0) as u32 ^ 0x5eed;
        let go = HubEv::AimGo { a: from, b: me, drill, seed };
        if let Some(h) = self.hub.as_mut() {
            h.duo_ask = None;
        }
        // (through the host: it tells both)
        if self.is_authority() {
            if self.kind == Kind::Host {
                self.out_events.push(Ev::Hub(go.clone()));
            }
            self.aim_go(go);
        } else {
            self.out_events.push(Ev::Hub(go));
        }
    }

    pub(crate) fn aim_go(&mut self, go: HubEv) {
        if let HubEv::AimGo { a, b, drill, seed } = go {
            let me = self.local as u8;
            let (lane, other) = if a == me { (0, b) } else if b == me { (1, a) } else { return };
            self.start_aim(Drill::from_u8(drill), lane, seed, Some(other));
        }
    }
}

/// The aim room's tick (the local player's drill).
pub fn tick(g: &mut Game, dt: f32) {
    let me = g.local;
    let Some(mut run) = g.hub.as_mut().and_then(|h| h.aim.take()) else { return };
    let f = &g.f[me];
    let airborne = !f.on_ground || f.wall.is_some();
    let aim = g.shot_ray(me);
    // a shot fired this tick (the weapon's clock started again)
    let fired = f.since_shot < run.last_since && f.since_shot < 0.05;
    run.last_since = f.since_shot;
    let firing = f.since_shot < 0.12 && !f.weapon.is_melee();
    let guard = f.guard;
    let pos = f.pos;
    // walking away from the Gallery's firing line ends it
    let lane_x = varrow::LANES[run.lane].x;
    let away = run.drill != Drill::WallRun && (pos.x < lane_x.0 - 1.5 || pos.x > lane_x.1 + 1.5 || pos.z > varrow::FIRING_Z + 6.0 || pos.z < varrow::FIRING_Z - 5.0);
    if fired {
        run.shot(aim.0, aim.1, airborne);
    }
    let finished = run.step(dt, pos, aim, firing, guard);
    if away && !run.done {
        if let Some(h) = g.hub.as_mut() {
            h.toast("Left the aim room");
        }
        g.stop_aim();
        return;
    }
    if finished {
        let score = run.score();
        let d = run.drill;
        let text = format!("{}: {} points  ·  accuracy {:.0}%  ·  reaction {} ms", d.name(), score, run.accuracy() * 100.0, run.reaction_ms());
        if let Some(h) = g.hub.as_mut() {
            let best = score > h.aim_best[d as usize];
            if best {
                h.aim_best[d as usize] = score;
            }
            h.aim_result = Some((format!("{text}{}", if best { "  ·  a new best!" } else { "" }), 8.0));
        }
        let who = me as u8;
        g.hub_send(HubEv::Score { who, board: d as u8, value: score });
    }
    if let Some(h) = g.hub.as_mut() {
        if h.aim.is_none() {
            h.aim = Some(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aim_at(o: V3, p: V3) -> (V3, V3) {
        (o, (p - o).norm())
    }

    #[test]
    fn flicks_score_hits_reaction_and_accuracy() {
        let mut a = AimRun::new(Drill::Flicks, 0, 42, None);
        let eye = varrow::LANES[0].stand + v3(0.0, 1.6, 0.0);
        // the countdown: nothing yet
        a.step(1.0, eye, (eye, v3(0.0, 0.0, 1.0)), false, false);
        assert!(a.target.is_none() && !a.live());
        a.step(2.1, eye, (eye, v3(0.0, 0.0, 1.0)), false, false);
        assert!(a.live() && a.target.is_some());
        let mut n = 0;
        while !a.done && n < 2000 {
            if let Some(tg) = a.target {
                // a miss first, then the hit, 0.3 s after it came up
                a.step(0.3, eye, (eye, v3(0.0, 0.0, 1.0)), false, false);
                a.shot(eye, v3(0.0, -1.0, 0.0), false);
                let (o, d) = aim_at(eye, tg.pos);
                a.shot(o, d, false);
            }
            a.step(1.0 / 60.0, eye, (eye, v3(0.0, 0.0, 1.0)), false, false);
            n += 1;
        }
        assert!(a.done);
        assert_eq!(a.hits, 30);
        assert_eq!(a.shots, 60);
        assert!((a.accuracy() - 0.5).abs() < 1e-6);
        assert!((280..=320).contains(&a.reaction_ms()), "{}", a.reaction_ms());
        // 30 hits at 0.3 s (145 points each) with half the shots missing
        assert!((a.score() as i32 - (30.0 * 145.0 * 0.75) as i32).abs() <= 2, "{}", a.score());
    }

    #[test]
    fn the_same_seed_gives_both_lanes_the_same_targets() {
        let mut a = AimRun::new(Drill::Flicks, 0, 9, Some(1));
        let mut b = AimRun::new(Drill::Flicks, 1, 9, Some(0));
        let ea = varrow::LANES[0].stand;
        let eb = varrow::LANES[1].stand;
        for _ in 0..200 {
            a.step(0.05, ea, (ea, v3(0.0, 0.0, 1.0)), false, false);
            b.step(0.05, eb, (eb, v3(0.0, 0.0, 1.0)), false, false);
        }
        let (ta, tb) = (a.target.unwrap(), b.target.unwrap());
        // the same spot, each in its own lane
        let off = varrow::LANES[1].stand.x - varrow::LANES[0].stand.x;
        assert!((ta.pos.x + off - tb.pos.x).abs() < 1e-4 && (ta.pos.y - tb.pos.y).abs() < 1e-4 && (ta.pos.z - tb.pos.z).abs() < 1e-4);
        assert_eq!(a.spawned, b.spawned);
    }

    #[test]
    fn tracking_counts_time_on_target_with_the_trigger_down() {
        let mut a = AimRun::new(Drill::Tracking, 1, 5, None);
        let eye = varrow::LANES[1].stand + v3(0.0, 1.6, 0.0);
        let mut t = -COUNTDOWN;
        while !a.done {
            let p = a.tracking_pos((t + 1.0 / 60.0).max(0.0));
            // on it for the first half, off it after
            let on = t < TRACK_TIME / 2.0;
            let aim = if on { aim_at(eye, p) } else { (eye, v3(0.0, 1.0, 0.0)) };
            a.step(1.0 / 60.0, eye, aim, true, false);
            t += 1.0 / 60.0;
        }
        assert!((a.on_target - TRACK_TIME / 2.0).abs() < 0.3, "{}", a.on_target);
        assert!((a.points - 500.0).abs() < 12.0, "{}", a.points);
    }

    #[test]
    fn parry_timing_is_judged_against_the_blow() {
        let mut a = AimRun::new(Drill::Parry, 0, 3, None);
        let eye = V3::ZERO;
        let dt = 1.0 / 120.0;
        let swings = a.swings.clone();
        assert_eq!(swings.len(), 20);
        // perfect on the first, 100 ms late on the second, way too early on the third, none on the rest
        let presses = [swings[0].impact + 0.02, swings[1].impact + 0.1, swings[2].tell - 0.5];
        let mut t = -COUNTDOWN;
        let mut held = false;
        while !a.done {
            let next = t + dt;
            let press = presses.iter().any(|&p| p >= t && p < next);
            held = press || (held && false);
            a.step(dt, eye, (eye, v3(0.0, 0.0, 1.0)), false, press);
            t = next;
        }
        assert_eq!(a.hits, 2, "perfect and good");
        assert!((a.points - 160.0).abs() < 1e-3, "{}", a.points);
        assert_eq!(a.shots, 20, "every blow judged");
        let _ = held;
    }

    #[test]
    fn wall_run_hits_in_the_air_count_more() {
        let mut ground = AimRun::new(Drill::WallRun, 0, 8, None);
        let mut air = AimRun::new(Drill::WallRun, 0, 8, None);
        let me = varrow::LANES[0].stand;
        for r in [&mut ground, &mut air] {
            r.step(COUNTDOWN + 0.2, me, (me, v3(0.0, 0.0, 1.0)), false, false);
        }
        let tg = ground.target.unwrap();
        let (o, d) = aim_at(me + v3(0.0, 1.6, 0.0), tg.pos);
        ground.shot(o, d, false);
        air.shot(o, d, true);
        assert!(air.points > ground.points * 2.5);
    }
}
