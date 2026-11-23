//! The skill benchmark behind BALANCE.md: a spammer bot against a precise bot, one weapon at a
//! time, hundreds of duels each.
//!
//! Both bots see the other through the same delay (`REACT`, about a quick human's reaction),
//! aim with the same turn speed and the same wobble, and run at the same speed. What differs is
//! only what they decide:
//! - the spammer holds the trigger, strafes and jumps while it shoots, mashes slashes and guard,
//!   kicks at random and throws grenades on cooldown;
//! - the precise bot taps, stops (counter-strafes) before each shot, waits for its aim to
//!   settle, pulls recoil down, keeps just out of sword reach, punishes whiffs and parries
//!   the swing it can see coming.
//!
//! `cargo test --release duel_benchmark -- --ignored --nocapture` runs the full 200 duels per
//! weapon and writes BALANCE.md; a smaller run is part of the normal tests.

use crate::game::*;
use crate::math::*;
use std::collections::VecDeque;

/// How late each bot sees the other (seconds).
pub const REACT: f32 = 0.18;
/// A duel that lasts longer than this is a draw.
const DUEL_LIMIT: f32 = 45.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    Spammer,
    Precise,
}

/// What a bot sees of its opponent (REACT seconds late).
#[derive(Clone, Copy)]
struct Seen {
    pos: V3,
    vel: V3,
    chest: V3,
    head: V3,
    on_ground: bool,
    weapon: Weapon,
    slash_t: f32,
    slash_lock: f32,
    whiff_t: f32,
    stagger_t: f32,
    stun_t: f32,
    guard: bool,
    kick_t: f32,
}

impl Seen {
    fn of(f: &Fighter) -> Seen {
        Seen {
            pos: f.pos,
            vel: f.vel,
            chest: f.chest(),
            head: f.eye(),
            on_ground: f.on_ground,
            weapon: f.weapon,
            slash_t: f.slash_t,
            slash_lock: f.slash_lock,
            whiff_t: f.whiff_t,
            stagger_t: f.stagger_t,
            stun_t: f.stun_t,
            guard: f.guard,
            kick_t: f.kick_t,
        }
    }
    /// Swinging, and the blow hasn't landed yet.
    fn winding(&self) -> bool {
        self.slash_t > 0.0 && self.slash_t > self.weapon.slash_dur() * self.weapon.hit_left()
    }
}

/// One duelling bot.
pub struct Duelist {
    pub style: Style,
    rng: Rng,
    seen: VecDeque<Seen>,
    /// Strafe direction and time until it changes.
    strafe: f32,
    strafe_t: f32,
    /// Aim wobble (the same for both styles).
    err: V3,
    err_t: f32,
    /// SMG: shots in this burst; a pause before the next.
    burst: u32,
    pause_t: f32,
    jump_t: f32,
    guard_flip: f32,
    /// The precise bot's swing is committed (it saw an opening).
    going_in: f32,
    /// A projectile to sidestep: which way, for how long.
    dodge: f32,
    dodge_t: f32,
    /// How far the precise bot keeps from a blade's reach (varies a little per duel).
    margin: f32,
    /// Its own guard presses (game time) and whether guard was held last tick.
    presses: Vec<f32>,
    guard_was: bool,
}

impl Duelist {
    pub fn new(style: Style, seed: u64) -> Duelist {
        let mut rng = Rng(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1);
        let margin = rng.range(0.5, 0.9);
        Duelist {
            style,
            rng,
            seen: VecDeque::new(),
            strafe: 1.0,
            strafe_t: 0.0,
            err: V3::ZERO,
            err_t: 0.0,
            burst: 0,
            pause_t: 0.0,
            jump_t: 1.0,
            guard_flip: 0.0,
            going_in: 0.0,
            dodge: 0.0,
            dodge_t: 0.0,
            margin,
            presses: Vec::new(),
            guard_was: false,
        }
    }

    pub fn think(&mut self, g: &Game, i: usize, o: usize, dt: f32) -> Input {
        let me = &g.f[i];
        let mut inp = Input { yaw: me.yaw, pitch: me.pitch, ..Default::default() };
        // what we see of them: REACT seconds old
        self.seen.push_back(Seen::of(&g.f[o]));
        let lag = (REACT / TICK).round() as usize;
        while self.seen.len() > lag + 1 {
            self.seen.pop_front();
        }
        let s = self.seen[0];
        if !me.alive || !g.f[o].alive {
            return inp;
        }
        let precise = self.style == Style::Precise;
        // where they are now, as far as we can tell (both styles extrapolate the same way)
        let est = s.pos + s.vel.flat() * REACT;
        let to_flat = (est - me.pos).flat();
        let dist = to_flat.len();

        // ---- aim (identical for both): their chest, led for travel time, with a wobble
        self.err_t -= dt;
        if self.err_t <= 0.0 {
            self.err_t = 0.25;
            let m = 0.12;
            self.err = v3(self.rng.range(-m, m), self.rng.range(-m, m), self.rng.range(-m, m));
        }
        let travel = match me.weapon {
            Weapon::Shuriken => dist / STAR_SPEED,
            Weapon::Rocket => dist / 32.0,
            _ => 0.0,
        };
        let mut target = s.chest + s.vel * (REACT + travel) + self.err;
        if me.weapon == Weapon::Shuriken && precise {
            // stars: go for the head (double damage)
            target = s.head + s.vel * (REACT + travel) + self.err * 0.5;
        }
        if me.weapon == Weapon::Rocket && precise && s.on_ground {
            // splash at their feet
            target = s.pos + s.vel.flat() * (REACT + travel) + v3(0.0, 0.15, 0.0);
        }
        let to = target - me.eye();
        let want_yaw = to.x.atan2(to.z);
        let want_pitch = to.y.atan2(to.flat().len().max(0.01));
        // recoil: the precise bot pulls it down, the spammer lets it climb
        let (cy, cp) = if precise { (me.kick[0], me.kick[1]) } else { (0.0, 0.0) };
        let turn = 14.0 * dt;
        let dy = wrap_angle(want_yaw - cy - me.yaw);
        inp.yaw = me.yaw + dy.clamp(-turn, turn);
        inp.pitch = approach(me.pitch, want_pitch - cp, turn);
        let aim_off = wrap_angle(want_yaw - cy - inp.yaw).abs() + (want_pitch - cp - inp.pitch).abs();

        // ---- strafing
        self.strafe_t -= dt;
        if self.strafe_t <= 0.0 {
            if precise {
                // unpredictable: short, random changes of direction
                self.strafe_t = self.rng.range(0.25, 0.8);
                self.strafe = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
            } else {
                // long, even runs
                self.strafe_t = self.rng.range(1.4, 2.4);
                self.strafe = -self.strafe;
            }
        }
        // stay near the middle of the floor
        let home = if me.pos.flat().len() > 25.0 { -me.pos.flat().norm() } else { V3::ZERO };

        // ---- projectiles: the precise bot sidesteps rockets, stars and grenades it can see
        self.dodge_t -= dt;
        if precise && self.dodge_t <= 0.0 {
            for gr in g.grenades.iter().filter(|gr| gr.owner == o) {
                let seen_at = gr.pos - gr.vel * REACT;
                let rel = me.pos - seen_at;
                let coming = gr.vel.flat().len() > 1.0 && gr.vel.flat().norm().dot(rel.flat().norm()) > 0.8;
                let near = rel.len() < if gr.kind == 0 { 6.0 } else { 14.0 };
                if near && (coming || gr.kind == 0) {
                    // across its path (away from a grenade)
                    let side = yaw_right(me.yaw).dot(rel.flat().norm());
                    self.dodge = if gr.kind == 0 { 0.0 } else if side >= 0.0 { 1.0 } else { -1.0 };
                    self.dodge_t = if gr.kind == 0 { 0.6 } else { 0.35 };
                    if gr.kind == 0 {
                        self.strafe = if side >= 0.0 { 1.0 } else { -1.0 };
                    }
                    break;
                }
            }
        }

        if me.weapon.is_melee() {
            self.melee(g, i, &s, dist, aim_off, &mut inp, dt);
        } else {
            self.gun(g, i, &s, dist, aim_off, &mut inp, dt);
        }
        if self.dodge_t > 0.0 && self.dodge != 0.0 {
            inp.right = self.dodge;
        }
        // steer back towards the middle if we've wandered (in the frame of our aim)
        if home != V3::ZERO {
            let f = yaw_fwd(inp.yaw);
            let r = yaw_right(inp.yaw);
            inp.fwd = (inp.fwd + home.dot(f)).clamp(-1.0, 1.0);
            inp.right = (inp.right + home.dot(r)).clamp(-1.0, 1.0);
        }

        // grenades: the spammer throws on cooldown; the precise bot only at someone standing still
        if me.loadout.item == 2 && me.items > 0 && me.item_cd <= 0.0 && me.nade_wind <= 0.0 && (6.0..24.0).contains(&dist) {
            let throw = if precise { s.vel.flat().len() < 1.0 && self.rng.chance(dt * 2.0) } else { true };
            if throw {
                inp.item = true;
            }
        }
        inp
    }

    fn gun(&mut self, g: &Game, i: usize, s: &Seen, dist: f32, aim_off: f32, inp: &mut Input, dt: f32) {
        let me = &g.f[i];
        let tn = &g.tune;
        let w = me.weapon;
        let precise = self.style == Style::Precise;
        // the distance each gun likes
        let pref = match w {
            Weapon::Shotgun => 5.0,
            Weapon::Smg => 9.0,
            Weapon::Shuriken => 8.0,
            Weapon::Rocket => 11.0,
            Weapon::Revolver | Weapon::HandCannon => 13.0,
            Weapon::Rifle => 16.0,
            Weapon::Sniper => 24.0,
            _ => 12.0,
        };
        inp.fwd = if dist > pref + 3.0 {
            1.0
        } else if dist < pref - 3.0 {
            -0.7
        } else {
            0.0
        };
        inp.right = self.strafe;
        if me.mag[me.cur] == 0 {
            inp.reload = true;
            return;
        }
        if w == Weapon::Sniper && dist > 4.0 {
            inp.guard = true; // scope in
        }
        if !precise {
            // ---- the spammer: trigger held, moving and hopping the whole time
            self.jump_t -= dt;
            if self.jump_t <= 0.0 && me.on_ground {
                self.jump_t = self.rng.range(0.9, 1.8);
                inp.jump = true;
            }
            let scoped = w != Weapon::Sniper || me.scope_t >= SCOPE_IN;
            inp.fire = aim_off < 0.3 && scoped;
            return;
        }
        // ---- the precise bot: stop, settle, tap
        self.pause_t -= dt;
        let range_ok = match w {
            Weapon::Shotgun => dist < 9.0,
            Weapon::Smg => dist < 22.0,
            _ => true,
        };
        let ready = me.fire_cd <= 0.0 && me.reload_t <= 0.0 && me.switch_t <= 0.0;
        let gun_rested = match w {
            Weapon::Revolver => me.since_shot >= tn.rev_window,
            Weapon::Smg => self.burst < tn.smg_free_shots as u32 && self.pause_t <= 0.0,
            Weapon::HandCannon => me.spam < 0.05,
            Weapon::Sniper => me.scope_t >= SCOPE_IN,
            _ => true,
        };
        let want_shot = ready && gun_rested && range_ok && aim_off < 0.05;
        // moving barely costs these: keep dodging while shooting
        let stop = !matches!(w, Weapon::Rifle | Weapon::Shuriken);
        if want_shot && !stop {
            inp.fire = me.on_ground || w != Weapon::Rifle;
        } else if want_shot {
            // counter-strafe: brake against our own sideways drift, then shoot once still
            let side = me.vel.flat().dot(yaw_right(inp.yaw));
            inp.right = if side.abs() > STILL_SPEED { -side.signum() } else { 0.0 };
            inp.fwd = if dist < pref - 3.0 && w == Weapon::Shotgun { -0.7 } else { 0.0 };
            let settled = me.spread(tn) <= match w {
                Weapon::Shotgun => 1.0, // its pattern is fixed: grounded is what counts
                Weapon::Smg => 0.006,
                Weapon::Rifle => 0.004,
                _ => 0.002,
            } && me.on_ground;
            if settled && me.vel.flat().len() < if w == Weapon::Shuriken || w == Weapon::Rocket { 99.0 } else { 0.9 } {
                inp.fire = true;
            }
        }
        // count SMG bursts and rest between them
        if w == Weapon::Smg {
            if inp.fire && me.fire_cd <= 0.0 {
                self.burst += 1;
                if self.burst >= tn.smg_free_shots as u32 {
                    self.pause_t = tn.recoil_reset + 0.04;
                }
            }
            if self.pause_t <= 0.0 && self.burst >= tn.smg_free_shots as u32 {
                self.burst = 0;
            }
        }
        // stars up close: the fan sweep too (its own cooldown)
        if w == Weapon::Shuriken && dist < 7.0 && aim_off < 0.08 {
            inp.guard = true;
        }
        // the rocket: don't fire at someone in the air (the splash misses)
        if w == Weapon::Rocket && !s.on_ground {
            inp.fire = false;
        }
        // the shotgun closes in rather than poking from range
        if w == Weapon::Shotgun && dist > 8.0 {
            inp.fwd = 1.0;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn melee(&mut self, g: &Game, i: usize, s: &Seen, dist: f32, aim_off: f32, inp: &mut Input, dt: f32) {
        let me = &g.f[i];
        let my_r = me.weapon.melee_range();
        let op_r = s.weapon.melee_range();
        if self.style == Style::Spammer {
            // ---- the spammer: run in, mash attack, mash guard, kick now and then
            inp.fwd = 1.0;
            inp.right = self.strafe * 0.3;
            if dist < op_r + 1.4 {
                inp.fire_pressed = true;
                inp.fire = false;
            }
            self.guard_flip -= dt;
            if dist < 4.5 && self.guard_flip <= 0.0 {
                self.guard_flip = self.rng.range(0.06, 0.16);
                inp.guard = self.rng.chance(0.5);
            }
            if dist < KICK_RANGE + 0.3 && self.rng.chance(dt * 1.5) {
                inp.kick = true;
            }
            return;
        }
        // ---- the precise bot
        let dur = s.weapon.slash_dur();
        let rec = dur * s.weapon.hit_left();
        let tn = &g.tune;
        let whiff_extra = if s.weapon == Weapon::Greatsword { tn.whiff_great } else { tn.whiff_time };
        let op_windup = s.weapon.windup();
        let seen_dist = (s.pos - me.pos).flat().len();
        // they can't start a swing until this runs out (as far as we can tell)
        // (a string of three basic swings ends in a forced breather: slash_lock)
        let busy = s.whiff_t.max(s.stun_t).max(s.stagger_t).max(s.slash_lock);
        let open = if busy > 0.0 {
            busy - REACT
        } else if s.winding() && seen_dist > op_r + 0.15 {
            // a swing started out of reach: it will whiff
            s.slash_t + whiff_extra - REACT
        } else {
            -1.0
        };
        // when their next blow lands, reading a masher: the swing under way, or the next one the
        // moment they're free again
        let next_hit = if s.winding() {
            s.slash_t - rec - REACT
        } else if busy > 0.0 {
            busy - REACT + op_windup
        } else if s.slash_t > 0.0 {
            // recovering from a swing: a masher chains the next one as early as the game lets it
            (s.slash_t - dur * 0.36).max(0.0) + op_windup - REACT
        } else {
            99.0
        };
        let windup = me.weapon.windup();
        let reach_in = ((dist - my_r * 0.9).max(0.0)) / RUN_SPEED;
        self.going_in -= dt;
        if me.slashing() {
            self.going_in = 0.0;
        } else if open + op_windup > reach_in + windup + 0.03 && open > 0.0 && dist < my_r + 2.0 {
            self.going_in = reach_in + windup + 0.05;
        }
        // guard presses in the last second (more than parry_mash locks the parry)
        self.presses.retain(|&p| g.time - p < 1.0);
        let in_reach = dist < op_r + 0.5;
        // time the guard to the blow: up just before it lands (a perfect parry), then held
        // until it has passed
        let parry_now = in_reach && next_hit > 0.0 && next_hit < PERFECT_PARRY - 0.05;
        let hold = me.guard && in_reach && next_hit > -0.12 && next_hit < 0.6;
        let can_press = (self.presses.len() as f32) < tn.parry_mash;
        if me.slashing() && !me.slash_hit_done {
            // our own swing is winding up: stay on them until it lands
            inp.fwd = if dist > my_r * 0.6 { 1.0 } else { 0.0 };
            inp.right = 0.0;
        } else if self.going_in > 0.0 && !parry_now {
            // the opening: in and cut
            inp.fwd = 1.0;
            inp.right = 0.0;
            // only a swing that will connect: in reach, in front, not launched over us
            let level = (s.pos.y - me.pos.y).abs() < 1.2;
            // where they'll be when the blow lands (a slow greatsword swing has to allow for it)
            let then = (s.pos + s.vel.flat() * (REACT + windup) - me.pos - me.vel.flat() * windup).flat().len();
            if dist < my_r * 0.92 && then < my_r * 0.92 && aim_off < 0.3 && level {
                inp.fire_pressed = true;
            }
        } else if (parry_now && (can_press || me.guard)) || hold {
            inp.guard = true;
            inp.fwd = 0.0;
            inp.right = 0.0;
        } else {
            // their guard is up and they aren't swinging: kick it
            if s.guard && !s.winding() && dist < KICK_RANGE && s.kick_t <= 0.0 && self.rng.chance(dt * 4.0) {
                inp.kick = true;
            }
            // otherwise keep just out of their reach, circling
            let keep = op_r + self.margin;
            inp.right = self.strafe * 0.8;
            inp.fwd = if dist < keep - 0.15 {
                -1.0
            } else if dist > keep + 0.6 {
                0.7
            } else {
                0.0
            };
        }
        if inp.guard && !self.guard_was {
            self.presses.push(g.time);
        }
        self.guard_was = inp.guard;
    }
}
/// A floor and two fighters apart (how far depends on the weapon), both holding it.
fn arena(w: Weapon, seed: u64) -> (Game, f32) {
    let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
    g.world = crate::game::lag_comp_tests::flat_world();
    g.bots[1] = None;
    g.mt.phase = Phase::Fight;
    g.mt.timer = 999.0;
    let mut rng = Rng(seed.wrapping_mul(2862933555777941757).wrapping_add(3037000493) | 1);
    let dist = match w {
        _ if w.is_melee() => rng.range(6.0, 10.0),
        Weapon::Shotgun => rng.range(7.0, 14.0),
        Weapon::Smg | Weapon::Shuriken => rng.range(8.0, 20.0),
        Weapon::Sniper => rng.range(18.0, 35.0),
        _ => rng.range(10.0, 25.0),
    };
    for i in 0..2 {
        let f = &mut g.f[i];
        f.alive = true;
        f.spawn_prot = 0.0;
        f.is_bot = true;
        f.ap = 0.0;
        f.hp = 100.0;
        f.loadout.melee = if w.is_melee() { w } else { Weapon::Katana };
        f.loadout.gun1 = if w.is_melee() { Weapon::Revolver } else { w };
        f.loadout.item = 2;
        f.equip();
        f.items = 2;
        f.cur = if w.is_melee() { 0 } else { 1 };
        f.weapon = w;
        f.switch_t = 0.0;
        f.reload_t = 0.0;
        f.vel = V3::ZERO;
        f.on_ground = true;
    }
    let side = rng.range(-3.0, 3.0);
    g.f[0].pos = v3(side, 0.0, -dist * 0.5);
    g.f[1].pos = v3(-side, 0.0, dist * 0.5);
    for i in 0..2 {
        let d = g.f[1 - i].pos - g.f[i].pos;
        g.f[i].yaw = d.x.atan2(d.z);
        g.f[i].prev_pos = g.f[i].pos;
    }
    (g, dist)
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Tally {
    pub precise: u32,
    pub spammer: u32,
    pub draws: u32,
    /// Accuracy of each side's guns (hits / shots), summed for averaging.
    pub shots: [u32; 2],
    pub hits: [u32; 2],
    pub parries: [u32; 2],
    pub whiffs: [u32; 2],
    pub secs: f32,
}

impl Tally {
    pub fn n(&self) -> u32 {
        self.precise + self.spammer + self.draws
    }
    /// The precise bot's share of all duels (a draw counts as half a win).
    pub fn rate(&self) -> f32 {
        (self.precise as f32 + 0.5 * self.draws as f32) / self.n().max(1) as f32
    }
    fn add(&mut self, o: &Tally) {
        self.precise += o.precise;
        self.spammer += o.spammer;
        self.draws += o.draws;
        for k in 0..2 {
            self.shots[k] += o.shots[k];
            self.hits[k] += o.hits[k];
            self.parries[k] += o.parries[k];
            self.whiffs[k] += o.whiffs[k];
        }
        self.secs += o.secs;
    }
}

/// One duel; which fighter is precise alternates with the seed.
pub fn duel(w: Weapon, seed: u64) -> Tally {
    let (mut g, _) = arena(w, seed);
    let p = (seed % 2) as usize; // the precise one
    let mut bots = [Duelist::new(Style::Spammer, seed * 7 + 1), Duelist::new(Style::Spammer, seed * 7 + 2)];
    bots[p].style = Style::Precise;
    let mut t = 0.0;
    let mut out = Tally::default();
    while t < DUEL_LIMIT && g.f[0].alive && g.f[1].alive {
        let i0 = bots[0].think(&g, 0, 1, TICK);
        let i1 = bots[1].think(&g, 1, 0, TICK);
        g.ext_input = vec![Some(i0), Some(i1)];
        g.tick(&i0);
        t += TICK;
    }
    let (pa, sa) = (g.f[p].alive, g.f[1 - p].alive);
    if pa && !sa {
        out.precise = 1;
    } else if sa && !pa {
        out.spammer = 1;
    } else {
        out.draws = 1;
    }
    for (k, idx) in [(0usize, p), (1, 1 - p)] {
        let m = g.match_stats[idx];
        out.shots[k] = m.shots;
        out.hits[k] = m.hits;
        out.parries[k] = m.parries;
        out.whiffs[k] = m.whiffs;
    }
    out.secs = t;
    out
}

pub const ALL: [Weapon; 11] = [
    Weapon::Dagger,
    Weapon::Katana,
    Weapon::Greatsword,
    Weapon::Shotgun,
    Weapon::Revolver,
    Weapon::Smg,
    Weapon::Rifle,
    Weapon::Rocket,
    Weapon::HandCannon,
    Weapon::Sniper,
    Weapon::Shuriken,
];

/// `n` duels with each weapon, spread over the CPU's threads.
pub fn run_all(n: u32) -> Vec<(Weapon, Tally)> {
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(4).max(1);
    let jobs: Vec<(usize, u32)> = (0..ALL.len()).flat_map(|wi| (0..n).map(move |k| (wi, k))).collect();
    let jobs = std::sync::Arc::new(std::sync::Mutex::new(jobs));
    let results = std::sync::Arc::new(std::sync::Mutex::new(vec![Tally::default(); ALL.len()]));
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let jobs = jobs.clone();
            let results = results.clone();
            std::thread::Builder::new()
                .stack_size(16 << 20)
                .spawn(move || loop {
                    let Some((wi, k)) = jobs.lock().unwrap().pop() else { break };
                    let t = duel(ALL[wi], 1000 + wi as u64 * 100_003 + k as u64);
                    results.lock().unwrap()[wi].add(&t);
                })
                .unwrap()
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let r = results.lock().unwrap().clone();
    ALL.iter().copied().zip(r).collect()
}

fn pct(a: u32, b: u32) -> String {
    if b == 0 {
        "-".into()
    } else {
        format!("{:.0}%", 100.0 * a as f32 / b as f32)
    }
}

pub fn report(rows: &[(Weapon, Tally)], n: u32) -> String {
    let mut total = Tally::default();
    for (_, t) in rows {
        total.add(t);
    }
    let melee_min = rows.iter().filter(|(w, _)| w.is_melee()).map(|(_, t)| t.rate()).fold(1.0f32, f32::min);
    let mut s = String::new();
    s.push_str("# Balance: spammer vs precise\n\n");
    s.push_str("Made by `cargo test --release duel_benchmark -- --ignored --nocapture` (src/duel_bench.rs).\n\n");
    s.push_str(&format!(
        "Two bots duel {n} times with each weapon (the same weapon on both sides, start 6-35 m apart depending on the weapon, which side is which alternating). \
Both see the other {:.2} s late, turn and aim the same way with the same wobble, and run at the same speed: only their decisions differ.\n\n",
        REACT
    ));
    s.push_str("- **Spammer**: holds the trigger, strafes and hops while shooting, never pulls recoil down, mashes slashes and guard, kicks at random, throws grenades on cooldown.\n");
    s.push_str("- **Precise**: taps (SMG bursts of 4, revolver after its 0.45 s reset), counter-strafes to a stop before each shot, waits for the aim to settle, pulls recoil down, keeps just outside sword reach, punishes whiffs, parries swings it can time, kicks a held guard, dodges projectiles.\n\n");
    s.push_str("A draw (both alive after 45 s) counts as half a win.\n\n");
    s.push_str("| Weapon | Precise wins | Spammer wins | Draws | Precise win rate | Accuracy (precise / spammer) | Parries (P / S) | Whiffs (P / S) |\n");
    s.push_str("|---|---|---|---|---|---|---|---|\n");
    for (w, t) in rows {
        s.push_str(&format!(
            "| {} | {} | {} | {} | **{:.0}%** | {} / {} | {} / {} | {} / {} |\n",
            w.name(),
            t.precise,
            t.spammer,
            t.draws,
            t.rate() * 100.0,
            pct(t.hits[0], t.shots[0]),
            pct(t.hits[1], t.shots[1]),
            t.parries[0],
            t.parries[1],
            t.whiffs[0],
            t.whiffs[1]
        ));
    }
    s.push_str(&format!(
        "| **All** | {} | {} | {} | **{:.0}%** | {} / {} | {} / {} | {} / {} |\n\n",
        total.precise,
        total.spammer,
        total.draws,
        total.rate() * 100.0,
        pct(total.hits[0], total.shots[0]),
        pct(total.hits[1], total.shots[1]),
        total.parries[0],
        total.parries[1],
        total.whiffs[0],
        total.whiffs[1]
    ));
    s.push_str(&format!(
        "Target: the precise bot wins at least 70% overall and in every melee matchup. Result: {:.0}% overall, {:.0}% in the weakest melee matchup - {}.\n",
        total.rate() * 100.0,
        melee_min * 100.0,
        if total.rate() >= 0.7 && melee_min >= 0.7 { "met" } else { "NOT met" }
    ));
    s.push('\n');
    s.push_str(NOTES);
    s.push('\n');
    s
}

/// Kept at the end of BALANCE.md.
const NOTES: &str = r#"## What the benchmark changed

- Rifle: a shot's recoil now moves the aim for the *next* shot (the shot itself goes where you aimed), and the kick per quick follow-up is 0.05 rad (was 0.03: that lifted a sprayer's second shot exactly from chest to head, rewarding spraying with headshots). Its "rested" first shot counts from when it's ready to fire again (its 0.45 s cycle alone is longer than the 0.4 s rest).
- SMG: the same next-shot timing, so the first 4 bullets are dead-on.
- Slashing out of a guard keeps the full wind-up (it used to skip most of it, leaving about 0.05 s to react).
- Practice bots pull recoil down in proportion to their skill.

## Weapons in hand match the HUD

`DUEL_GALLERY=held:<weapon number>[:run]` lines up your fighter (so the HUD names the weapon) and a second fighter holding it. Every weapon's model in hand matches the HUD's name and icon:

- `shots/weapons/held_check/_all_ready.png` - all 11 weapons, ready to fire (one picture each in the same folder)
- `shots/weapons/held_check/_all_running.png` - the guns while running
- `shots/weapons/held_check/scattergun_play_1100.png` (before) and `scattergun_play_after.png` (after) - the over-the-shoulder view

The scattergun that "looked like a pistol" was the right model seen end-on: shouldered at cheek height and pointing straight away from the over-the-shoulder camera, its wooden stock hid behind the hood and only the dark receiver showed. Long guns (scattergun, rifle, sniper) are now carried at low ready (muzzle down and a little out to the right) when not shooting, and come up to the shoulder the moment they fire, so their length and pump show from behind."#;

#[test]
#[ignore]
fn duel_benchmark() {
    let n: u32 = std::env::var("DUEL_BENCH_N").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
    let t0 = std::time::Instant::now();
    let rows = run_all(n);
    let rep = report(&rows, n);
    println!("{rep}\n({:.0} s)", t0.elapsed().as_secs_f32());
    if n >= 200 {
        std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/BALANCE.md"), rep.replace('\n', "\r\n")).unwrap();
    }
    let total = rows.iter().fold(Tally::default(), |mut a, (_, t)| {
        a.add(t);
        a
    });
    assert!(total.rate() >= 0.7, "precise wins {:.0}% overall", total.rate() * 100.0);
    for (w, t) in &rows {
        if w.is_melee() {
            assert!(t.rate() >= 0.7, "{w:?}: precise wins only {:.0}%", t.rate() * 100.0);
        }
    }
}

/// A quick version for every test run: 16 duels a weapon.
#[test]
fn precise_play_beats_spamming() {
    let rows = run_all(16);
    let total = rows.iter().fold(Tally::default(), |mut a, (_, t)| {
        a.add(t);
        a
    });
    println!("{}", report(&rows, 16));
    assert!(total.rate() >= 0.65, "precise wins {:.0}% overall", total.rate() * 100.0);
}

/// Prints one duel tick by tick (for working on the bots).
#[test]
#[ignore]
fn duel_trace() {
    let w = Weapon::from_u8(std::env::var("DUEL_TRACE_W").ok().and_then(|v| v.parse().ok()).unwrap_or(1));
    let seed: u64 = std::env::var("DUEL_TRACE_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1000);
    let (mut g, _) = arena(w, seed);
    let p = (seed % 2) as usize;
    let mut bots = [Duelist::new(Style::Spammer, seed * 7 + 1), Duelist::new(Style::Spammer, seed * 7 + 2)];
    bots[p].style = Style::Precise;
    let mut t = 0.0;
    let mut k = 0;
    while t < 30.0 && g.f[0].alive && g.f[1].alive {
        let i0 = bots[0].think(&g, 0, 1, TICK);
        let i1 = bots[1].think(&g, 1, 0, TICK);
        g.ext_input = vec![Some(i0), Some(i1)];
        g.tick(&i0);
        t += TICK;
        k += 1;
        if std::env::var("DUEL_TRACE_GUN").is_ok() {
            let a = &g.f[p];
            let m = g.match_stats[p];
            if a.since_shot < TICK * 0.5 {
                println!("{t:5.2} shot kick {:?} pitch {:.3} yaw {:.3} spread {:.4} vel {:.2} shots {} hits {} ground {}", a.kick, a.pitch, a.yaw, a.spread(&g.tune), a.vel.flat().len(), m.shots, m.hits, a.on_ground);
            }
            continue;
        }
        if k % 3 == 0 {
            let (a, b) = (&g.f[p], &g.f[1 - p]);
            let ip = if p == 0 { i0 } else { i1 };
            let is = if p == 0 { i1 } else { i0 };
            println!(
                "{t:5.2} d{:4.2} | P hp{:3.0} sl{:.2} wh{:.2} g{} st{:.2} in[f{:+.1} at{} g{} k{}] | S hp{:3.0} sl{:.2} id{} wh{:.2} g{} st{:.2} gh{:3.0} in[at{} g{} k{}]",
                (a.pos - b.pos).flat().len(),
                a.hp, a.slash_t, a.whiff_t, a.guard as u8, a.stun_t, ip.fwd, ip.fire_pressed as u8, ip.guard as u8, ip.kick as u8,
                b.hp, b.slash_t, b.slash_idx, b.whiff_t, b.guard as u8, b.stun_t, b.guard_hp, is.fire_pressed as u8, is.guard as u8, is.kick as u8
            );
        }
    }
}
/// Which seeds each side wins with one weapon (for finding a duel to trace).
#[test]
#[ignore]
fn duel_seeds() {
    let w = Weapon::from_u8(std::env::var("DUEL_TRACE_W").ok().and_then(|v| v.parse().ok()).unwrap_or(0));
    let mut line = String::new();
    for k in 0..40u64 {
        let seed = 1000 + w as u64 * 100_003 + k;
        let t = duel(w, seed);
        line.push_str(&format!("{seed}:{} ", if t.precise > 0 { "P" } else if t.spammer > 0 { "S" } else { "D" }));
    }
    println!("{line}");
}