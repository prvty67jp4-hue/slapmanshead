use crate::game::{Game, Input, SCOPE_IN, Weapon};
use crate::math::*;

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
        }
    }

    pub fn reset(&mut self) {
        self.target = None;
        self.retarget = 0.0;
    }

    pub fn think(&mut self, g: &Game, i: usize, dt: f32) -> Input {
        let me = &g.f[i];
        let mut inp = Input { yaw: me.yaw, pitch: me.pitch, ..Default::default() };
        if !me.alive {
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
        let lead = 0.05 + 0.12 * s;
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
        let want_yaw = to.x.atan2(to.z);
        let want_pitch = to.y.atan2(flat);
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
                    _ => 14.0,
                };
                inp.fwd = if dist > pref + 3.0 { 1.0 } else if dist < pref - 3.0 { -0.7 } else { 0.0 };
                inp.right = self.strafe;
            }
        } else {
            inp.fwd = 1.0;
            inp.right = self.strafe * 0.3;
        }

        // objective modes: go for the flag, bring it home, or return ours
        if let Some(goal) = g.objective(i) {
            let carrying = g.mt.flags.iter().any(|fl| fl.state == 1 && fl.carrier as usize == i);
            if carrying || !visible || dist > 11.0 {
                let to = goal - me.pos;
                if to.flat().len() > 0.5 {
                    inp.yaw = to.x.atan2(to.z);
                    inp.pitch = 0.0;
                    inp.fwd = 1.0;
                    inp.right = self.strafe * 0.15;
                }
            }
        }
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
        if self.dash_t <= 0.0 && visible {
            self.dash_t = 1.5 + self.rng.f() * (3.5 - 2.0 * s);
            inp.dash = true;
            if melee && dist > 3.0 {
                inp.fwd = 1.0;
                inp.right = 0.0;
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
                    if !en.weapon.is_melee() && facing_me && dist > 3.0 && self.rng.chance(dt * 2.5 * s) {
                        self.guard_t = 0.4 + self.rng.f() * 0.6;
                    }
                    if en.slashing() && dist < 3.2 && self.rng.chance(dt * 6.0 * s) {
                        self.guard_t = 0.35;
                    }
                    if self.guard_t > 0.0 {
                        inp.guard = true;
                    } else if dist < me.weapon.melee_range() && aim_off < 0.6 {
                        inp.fire_pressed = self.rng.chance(dt * (4.0 + 6.0 * s));
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
                    if aim_off < thresh && self.rng.chance(dt * (3.0 + 6.0 * s)) {
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
        // patch up when hurt
        if me.items > 0 && ((me.loadout.item == 0 && me.hp < 45.0) || (me.loadout.item == 1 && me.ap < 10.0)) && self.rng.chance(dt * 2.0) {
            inp.item = true;
        }
        inp
    }
}
