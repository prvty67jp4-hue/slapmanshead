use crate::game::{Ability, CHARGE_TIME, GUARD_MAX, Game, Input, KICK_RANGE, SCOPE_IN, STAR_SPEED, Weapon};
use crate::math::*;
use crate::nav::Move;

/// A simple practice opponent.
pub struct Bot {
    pub skill: f32,
    rng: Rng,
    strafe: f32,
    strafe_t: f32,
    jump_t: f32,
    dash_t: f32,
    err: V3,
    err_t: f32,
    choose_t: f32,
    want: usize,
    last_pos: V3,
    stuck_t: f32,
    guard_t: f32,
    jump_again: f32,
    style: f32,
    target: Option<usize>,
    retarget: f32,
    slide_hold: f32,
    /// Holding attack for a charged heavy (time left to hold).
    charge_hold: f32,
    /// Slash on the next tick (the katana's dash-through).
    slash_next: bool,
    /// Re-raising the guard against gunfire, to catch bullets in the deflect window.
    guard_tap: f32,
    /// Until the next think about the ability.
    ab_wait: f32,
    /// Somewhere to go regardless of anything else (tests).
    pub goto: Option<V3>,
    /// The way it's following (see crate::nav): the places, the next one, where it leads, and
    /// time until it's worked out again.
    nav_path: Vec<(V3, Move)>,
    nav_i: usize,
    nav_goal: V3,
    nav_t: f32,
    /// A dash to make once in the air (crossing a wide gap), and time without progress.
    dash_armed: bool,
    nav_stuck: f32,
    nav_last: V3,
}

impl Bot {
    pub fn new(skill: f32, seed: u64) -> Bot {
        let mut rng = Rng(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1);
        let style = rng.f();
        Bot {
            skill: skill.clamp(0.05, 1.0),
            rng,
            strafe: 1.0,
            strafe_t: 0.0,
            jump_t: 2.0,
            dash_t: 3.0,
            err: V3::ZERO,
            err_t: 0.0,
            choose_t: 0.0,
            want: 1,
            last_pos: V3::ZERO,
            stuck_t: 0.0,
            guard_t: 0.0,
            jump_again: 0.0,
            style,
            target: None,
            retarget: 0.0,
            slide_hold: 0.0,
            charge_hold: 0.0,
            slash_next: false,
            guard_tap: 0.0,
            ab_wait: 2.0,
            goto: None,
            nav_path: Vec::new(),
            nav_i: 0,
            nav_goal: V3::ZERO,
            nav_t: 0.0,
            dash_armed: false,
            nav_stuck: 0.0,
            nav_last: V3::ZERO,
        }
    }

    pub fn reset(&mut self) {
        self.target = None;
        self.retarget = 0.0;
        self.nav_path.clear();
        self.nav_t = 0.0;
    }

    /// Head for `goal` along the level's navigation: steer to the next place on the way, jump up
    /// ledges and over gaps, dash across the wide ones. `look`: face the way it's going (when
    /// there's nobody in sight to aim at).
    fn follow(&mut self, g: &Game, i: usize, goal: V3, inp: &mut Input, dt: f32, look: bool) {
        let me = &g.f[i];
        // no progress for a while: think again (and hop)
        if (me.pos - self.nav_last).len() < dt * 0.8 {
            self.nav_stuck += dt;
        } else {
            self.nav_stuck = (self.nav_stuck - dt).max(0.0);
        }
        self.nav_last = me.pos;
        self.nav_t -= dt;
        if self.nav_t <= 0.0 || (goal - self.nav_goal).len() > 2.5 || self.nav_stuck > 1.2 {
            let nav = crate::nav::for_world(&g.world);
            self.nav_path = nav.path(me.pos, goal).unwrap_or_default();
            self.nav_i = 0;
            self.nav_goal = goal;
            self.nav_t = 0.7 + self.rng.f() * 0.3;
            if self.nav_stuck > 1.2 && me.on_ground {
                inp.jump = true;
            }
            self.nav_stuck = 0.0;
        }
        // past the places already reached
        while let Some(&(p, _)) = self.nav_path.get(self.nav_i) {
            if (p - me.pos).flat().len() < 0.65 && (p.y - me.pos.y).abs() < 1.3 {
                self.nav_i += 1;
            } else {
                break;
            }
        }
        let (target, how) = match self.nav_path.get(self.nav_i) {
            Some(&(p, h)) => (p, h),
            None => (goal, Move::Walk),
        };
        let to = (target - me.pos).flat();
        let d = to.len();
        if d < 0.3 && self.nav_i >= self.nav_path.len() {
            inp.fwd = 0.0;
            inp.right = 0.0;
            return;
        }
        let dir = if d > 1e-3 { to * (1.0 / d) } else { yaw_fwd(me.yaw) };
        if look {
            let want = dir.x.atan2(dir.z);
            let turn = 9.0 * dt;
            inp.yaw = me.yaw + wrap_angle(want - me.yaw).clamp(-turn, turn);
            inp.pitch = approach(me.pitch, 0.0, turn);
        }
        let f = dir.dot(yaw_fwd(inp.yaw));
        let r = dir.dot(yaw_right(inp.yaw));
        let m = f.abs().max(r.abs()).max(1e-3);
        inp.fwd = f / m;
        inp.right = r / m;
        inp.crouch = false;
        // the ground ahead runs out: a gap (or a ledge to drop from)
        let ahead = me.pos + dir * 0.7 + v3(0.0, 0.3, 0.0);
        let edge = g.world.ground_below(ahead) < me.pos.y - 0.6;
        match how {
            Move::Jump if target.y > me.pos.y + 0.5 => {
                if me.on_ground && d < 1.7 {
                    inp.jump = true;
                }
            }
            Move::Jump | Move::Dash => {
                if me.on_ground && edge {
                    inp.jump = true;
                    self.dash_armed = how == Move::Dash;
                }
            }
            _ => {}
        }
        if self.dash_armed && !me.on_ground && me.vel.y < 6.0 && me.dash_charges >= 1.0 {
            inp.dash = true;
            self.dash_armed = false;
        }
        if me.on_ground && !edge {
            self.dash_armed = false;
        }
    }

    pub fn think(&mut self, g: &Game, i: usize, dt: f32) -> Input {
        let me = &g.f[i];
        let mut inp = Input { yaw: me.yaw, pitch: me.pitch, ..Default::default() };
        if !me.alive {
            return inp;
        }
        if let Some(goal) = self.goto {
            if !g.frozen() {
                self.follow(g, i, goal, &mut inp, dt, true);
            }
            return inp;
        }
        // keep a target for a while, re-pick now and then
        self.retarget -= dt;
        let valid = self.target.map_or(false, |t| t < g.f.len() && g.f[t].alive && g.is_enemy(i, t));
        if !valid || self.retarget <= 0.0 {
            self.target = g.nearest_enemy(i);
            self.retarget = 1.0 + self.rng.f();
        }
        let Some(ti) = self.target else {
            // nobody to fight: wander
            inp.fwd = 1.0;
            inp.yaw = me.yaw + (g.time * 0.7 + i as f32).sin() * 0.02;
            return inp;
        };
        let en = &g.f[ti];
        let s = self.skill;
        let mut lead = 0.05 + 0.12 * s;
        if me.weapon == Weapon::Shuriken {
            // stars take a moment to arrive: aim where they'll be
            lead += (g.f[ti].pos - me.pos).len() / STAR_SPEED * (0.5 + 0.5 * s);
        }
        let target = if en.alive { en.chest() + v3(0.0, 0.1, 0.0) + en.vel * lead } else { en.pos };
        self.err_t -= dt;
        if self.err_t <= 0.0 {
            self.err_t = 0.35 + self.rng.f() * 0.4;
            let m = (1.0 - s) * 1.6 + 0.12;
            self.err = v3(self.rng.range(-m, m), self.rng.range(-m * 0.6, m * 0.6), self.rng.range(-m, m));
        }
        let eye = me.eye();
        let to = target + self.err - eye;
        let flat = v3(to.x, 0.0, to.z).len().max(0.01);
        // recoil (the SMG's pattern, the rifle's kick) moves the aim: better bots pull it back down
        let want_yaw = to.x.atan2(to.z) - me.kick[0] * s;
        let want_pitch = to.y.atan2(flat) - me.kick[1] * s;
        let turn = (3.0 + 9.0 * s) * dt;
        let dy = wrap_angle(want_yaw - me.yaw);
        inp.yaw = me.yaw + dy.clamp(-turn, turn);
        inp.pitch = approach(me.pitch, want_pitch, turn);
        let aim_off = dy.abs() + (want_pitch - me.pitch).abs();

        if g.frozen() {
            return inp;
        }

        let dist = (en.pos - me.pos).len();
        let visible = en.alive && g.world.line_clear(eye, en.chest());

        // the ability, when it fits the moment (thought over every couple of seconds)
        self.ab_wait -= dt;
        if self.ab_wait <= 0.0 && g.abilities_on() && me.ab_charge >= 1.0 {
            self.ab_wait = 1.2 + self.rng.f() * 2.5;
            inp.ability = match Ability::of(me.loadout.body) {
                // find them behind cover
                Ability::Mark => !visible && dist < 28.0,
                // close the distance
                Ability::Grapple => dist > 14.0 && aim_off < 0.35,
                // under fire, or a blade coming in
                Ability::Wall => (me.hurt_t > 0.0 && !en.weapon.is_melee() && visible) || (en.slashing() && dist < 3.5),
                // slip away when hurt, or feint going in
                Ability::Afterimage => (me.hp < 45.0 && dist < 12.0) || (dist < 9.0 && self.rng.f() < 0.35),
            };
        }

        // weapon choice (with a little hesitation)
        self.choose_t -= dt;
        if self.choose_t <= 0.0 {
            self.choose_t = 0.6 + self.rng.f() * 0.6;
            let sword_range = me.loadout.melee.melee_range() + 0.8 + self.style * 4.0;
            // how well each gun suits this distance
            let suit = |w: Weapon| -> f32 {
                match w {
                    Weapon::Shotgun | Weapon::Smg => {
                        if dist < 12.0 { 2.0 } else { 0.5 }
                    }
                    Weapon::Rocket => {
                        if dist > 5.0 && dist < 22.0 { 1.6 } else { 0.3 }
                    }
                    Weapon::Sniper => {
                        if dist > 14.0 { 2.6 } else { 0.4 }
                    }
                    Weapon::HandCannon => {
                        if dist > 6.0 && dist < 26.0 { 2.1 } else { 0.9 }
                    }
                    Weapon::Shuriken => {
                        if dist < 18.0 { 2.2 } else { 0.5 }
                    }
                    _ => {
                        if dist > 9.0 { 2.0 } else { 1.0 }
                    }
                }
            };
            self.want = if g.mt.mode.melee_only() || dist < sword_range {
                0
            } else {
                let (a, b) = (me.slot_weapon(1), me.slot_weapon(2));
                let sa = suit(a) * if me.mag[1] > 0 { 1.0 } else { 0.2 };
                let sb = suit(b) * if me.mag[2] > 0 { 1.0 } else { 0.2 };
                if sa >= sb { 1 } else { 2 }
            };
        }
        if me.cur != self.want {
            inp.select = Some(self.want as u8);
        }
        let melee = me.weapon.is_melee();

        // movement
        self.strafe_t -= dt;
        if self.strafe_t <= 0.0 {
            self.strafe_t = 0.5 + self.rng.f() * 1.3;
            self.strafe = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
        }
        let fwd = yaw_fwd(inp.yaw);
        let _ = fwd;
        if visible {
            if melee {
                inp.fwd = 1.0;
                inp.right = self.strafe * 0.35;
            } else {
                let pref = match me.weapon {
                    Weapon::Shotgun | Weapon::Smg => 6.0,
                    Weapon::Rocket => 11.0,
                    Weapon::Sniper => 22.0,
                    Weapon::HandCannon => 12.0,
                    Weapon::Shuriken => 8.0,
                    _ => 14.0,
                };
                inp.fwd = if dist > pref + 3.0 { 1.0 } else if dist < pref - 3.0 { -0.7 } else { 0.0 };
                inp.right = self.strafe;
            }
        } else {
            inp.fwd = 1.0;
            inp.right = self.strafe * 0.3;
        }

        // where to go when there's more to it than strafing: the flag (objective modes), an enemy
        // out of sight, or one on another level; the way there comes from the level's navigation
        let carrying = g.mt.flags.iter().any(|fl| fl.state == 1 && fl.carrier as usize == i);
        let travel = match g.objective(i) {
            Some(goal) if carrying || !visible || dist > 11.0 => Some((goal, !visible || carrying)),
            _ if !visible && en.alive => Some((en.pos, true)),
            _ if visible && melee && (en.pos.y - me.pos.y).abs() > 1.2 => Some((en.pos, false)),
            _ => None,
        };
        let me_pos = me.pos;
        if let Some((goal, look)) = travel.filter(|(gp, _)| (*gp - me_pos).flat().len() > 1.0 || (gp.y - me_pos.y).abs() > 1.0) {
            self.follow(g, i, goal, &mut inp, dt, look);
        }
        let me = &g.f[i];
        // unstick: jump and wall-run over things
        let moved = (me.pos - self.last_pos).flat().len();
        self.last_pos = me.pos;
        if moved < 0.02 && (inp.fwd != 0.0 || inp.right != 0.0) {
            self.stuck_t += dt;
        } else {
            self.stuck_t = (self.stuck_t - dt).max(0.0);
        }
        self.jump_t -= dt;
        if (self.stuck_t > 0.25 || self.jump_t <= 0.0) && me.on_ground {
            inp.jump = true;
            self.jump_t = 1.2 + self.rng.f() * 3.0;
            self.jump_again = 0.28 + self.rng.f() * 0.2;
            self.stuck_t = 0.0;
        }
        if self.jump_again > 0.0 {
            self.jump_again -= dt;
            if self.jump_again <= 0.0 && !me.on_ground {
                inp.jump = true; // starts a wall run when next to a wall
            }
        }
        self.dash_t -= dt;
        if self.dash_t <= 0.0 && visible && me.dash_charges >= 1.0 {
            self.dash_t = 1.5 + self.rng.f() * (3.5 - 2.0 * s);
            inp.dash = true;
            if melee && dist > 3.0 {
                inp.fwd = 1.0;
                inp.right = 0.0;
                // the katana cuts straight through from a dash
                if me.weapon == Weapon::Katana && dist < 7.5 && aim_off < 0.5 && self.rng.chance(0.3 + 0.5 * s) {
                    self.slash_next = true;
                }
            }
        }

        // slide in at speed now and then (closing in with a blade, or dodging around)
        self.slide_hold -= dt;
        if self.slide_hold <= 0.0 && me.on_ground && me.vel.flat().len() > 8.0 && visible && dist > 5.0 && self.rng.chance(dt * (0.4 + s)) {
            self.slide_hold = 0.35 + self.rng.f() * 0.3;
        }
        if self.slide_hold > 0.0 {
            inp.crouch = true;
            inp.fwd = 1.0;
            if self.slide_hold < 0.08 && melee {
                inp.jump = true; // and jump out of it
            }
        }

        // attacking
        if visible && en.alive {
            match me.weapon {
                _ if melee => {
                    let facing_me = en.fwd().dot((me.pos - en.pos).flat().norm()) > 0.8;
                    self.guard_t -= dt;
                    self.guard_tap -= dt;
                    let low_guard = me.guard_hp < GUARD_MAX * 0.25;
                    if !en.weapon.is_melee() && facing_me && dist > 3.0 && !low_guard && self.rng.chance(dt * 2.5 * s) {
                        self.guard_t = 0.4 + self.rng.f() * 0.6;
                    }
                    if en.slashing() && dist < 3.2 && !low_guard && self.rng.chance(dt * 6.0 * s) {
                        self.guard_t = 0.35;
                    }
                    // against gunfire: drop and re-raise the guard now and then, so a bullet
                    // sometimes arrives right in the deflect window
                    let tap_gap = self.guard_t > 0.0 && !en.weapon.is_melee() && self.guard_tap <= 0.0 && self.rng.chance(dt * 3.0 * s);
                    if tap_gap {
                        self.guard_tap = 0.45 + self.rng.f() * 0.5;
                    }
                    if self.slash_next {
                        self.slash_next = false;
                        inp.fire_pressed = true;
                    } else if self.charge_hold > 0.0 {
                        // winding up a heavy: hold, then let go once it's ready
                        self.charge_hold -= dt;
                        inp.fire = self.charge_hold > 0.0;
                        if en.slashing() && dist < 2.5 {
                            self.charge_hold = 0.0; // too slow: let it go
                        }
                    } else if en.guard && dist < KICK_RANGE + 0.3 && aim_off < 0.8 && self.rng.chance(dt * (2.0 + 6.0 * s)) {
                        // a kick beats a guard
                        inp.kick = true;
                    } else if en.kick_t > 0.0 && dist < me.weapon.melee_range() && self.rng.chance(dt * 10.0 * s) {
                        // a slash beats a kick
                        inp.fire_pressed = true;
                    } else if me.weapon == Weapon::Greatsword && !me.on_ground && me.pos.y > en.pos.y + 1.4 && dist < 4.0 && !me.slashing() {
                        // plunge down on them
                        inp.fire_pressed = true;
                    } else if self.guard_t > 0.0 && me.guard_hp > 0.0 && !tap_gap {
                        inp.guard = true;
                    } else if (en.guard || en.stun_t > 0.0) && dist < me.weapon.melee_range() + 1.0 && self.rng.chance(dt * (0.4 + 1.2 * s)) {
                        // break the guard with a heavy (or punish the stagger)
                        self.charge_hold = CHARGE_TIME + 0.08;
                        inp.fire = true;
                    } else if dist < me.weapon.melee_range() && aim_off < 0.6 {
                        inp.fire_pressed = self.rng.chance(dt * (4.0 + 6.0 * s));
                    }
                    // low guard: back off a little while it refills
                    if low_guard && dist < 4.0 && en.weapon.is_melee() {
                        inp.fwd = -0.6;
                    }
                }
                Weapon::Shuriken => {
                    // up close: fan them; otherwise bursts
                    let thresh = 0.06 + 0.08 * (1.0 - s);
                    if dist < 5.0 && aim_off < 0.3 && self.rng.chance(dt * (1.0 + 3.0 * s)) {
                        inp.guard = true;
                    } else if aim_off < thresh && self.rng.chance(dt * (3.0 + 5.0 * s)) {
                        inp.fire = true;
                    }
                }
                Weapon::Sniper => {
                    // scope in, then take the shot once the scope is up
                    if dist > 4.0 {
                        inp.guard = true;
                    }
                    let thresh = 0.03 + 0.05 * (1.0 - s);
                    if (me.scope_t >= SCOPE_IN || dist <= 4.0) && aim_off < thresh && self.rng.chance(dt * (1.5 + 4.0 * s)) {
                        inp.fire = true;
                    }
                }
                _ => {
                    let thresh = 0.05 + 0.08 * (1.0 - s);
                    // pistols: tap, wait for the aim to settle (better bots wait longer)
                    let settled = me.weapon.spam().0 <= 0.0 || me.spread(&g.tune) < 0.02 + 0.05 * (1.0 - s);
                    if aim_off < thresh && settled && self.rng.chance(dt * (3.0 + 6.0 * s)) {
                        inp.fire = true;
                    }
                }
            }
            if me.loadout.item == 2 && dist > 8.0 && dist < 18.0 && me.items > 0 && self.rng.chance(dt * 0.12) {
                inp.item = true;
            }
        } else if !melee && me.mag[me.cur] < me.weapon.mag() && self.rng.chance(dt) {
            inp.reload = true;
        }
        // finish off a beaten, staggered enemy in reach (better bots spot it sooner)
        if g.finish_target(i).is_some() && self.rng.chance(dt * (2.0 + 8.0 * s)) {
            inp.finish = true;
        }
        // patch up when hurt
        if me.items > 0 && ((me.loadout.item == 0 && me.hp < 45.0) || (me.loadout.item == 1 && me.ap < 10.0)) && self.rng.chance(dt * 2.0) {
            inp.item = true;
        }
        inp
    }
}

#[cfg(test)]
mod nav_tests {
    use super::*;
    use crate::game::{Mode, Phase, TICK};
    use crate::world::MapId;

    /// A bot that only travels, from one team's spawn to `to`: how long it took (None: never).
    fn travel(from_blue: bool, to: V3, limit: f32) -> Option<f32> {
        let mut g = Game::offline("A", 1, 1.0, Mode::Tdm, 50);
        g.set_map(MapId::Bridgeworks);
        g.mt.phase = Phase::Fight;
        g.mt.timer = 999.0;
        g.f[0].present = false;
        g.f[0].alive = false;
        let n = g.world.spawn_points.len();
        let (p, yaw) = g.world.spawn_points[if from_blue { n / 2 + 2 } else { 2 }];
        g.f[1].respawn_at(p, yaw);
        g.f[1].spawn_prot = 0.0;
        let mut b = Bot::new(1.0, 7);
        b.goto = Some(to);
        g.bots[1] = Some(b);
        let mut t = 0.0;
        while t < limit {
            g.tick(&Input::default());
            t += TICK;
            let f = &g.f[1];
            if (f.pos - to).flat().len() < 1.4 && (f.pos.y - to.y).abs() < 1.0 {
                return Some(t);
            }
        }
        let f = &g.f[1];
        println!("  stopped at {:?}", f.pos);
        None
    }

    /// Bots get everywhere on the team map: every lane and level (rooftops, ramparts, the
    /// balcony, the riverbed, both flag rooms), from both bases, walking, jumping and dashing.
    #[test]
    fn bots_navigate_every_lane_and_level_of_bridgeworks() {
        let spots = crate::nav::tests::BRIDGEWORKS_SPOTS;
        let results: Vec<(String, bool, Option<f32>)> = std::thread::scope(|s| {
            let hs: Vec<_> = spots
                .iter()
                .flat_map(|(name, p)| [false, true].map(|blue| (name.to_string(), blue, v3(p[0], p[1], p[2]))))
                .map(|(name, blue, p)| s.spawn(move || (name, blue, travel(blue, p, 60.0))))
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let mut failed = Vec::new();
        for (name, blue, t) in &results {
            let side = if *blue { "blue" } else { "red" };
            match t {
                Some(t) => println!("{side} -> {name}: {t:.1} s"),
                None => {
                    println!("{side} -> {name}: NOT REACHED");
                    failed.push(format!("{side} -> {name}"));
                }
            }
        }
        assert!(failed.is_empty(), "bots couldn't get to: {failed:?}");
    }
}
