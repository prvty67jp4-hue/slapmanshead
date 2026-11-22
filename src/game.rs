use crate::bot::Bot;
use crate::math::*;
use crate::world::{Mat, World};
use std::collections::VecDeque;

pub const RADIUS: f32 = 0.36;
pub const HEIGHT: f32 = 1.8;
pub const GRAVITY: f32 = 25.0;
pub const JUMP_VEL: f32 = 9.4;
pub const RUN_SPEED: f32 = 8.2;
pub const MAX_HP: f32 = 100.0;
pub const MAX_AP: f32 = 50.0;
pub const TICK: f32 = 1.0 / 120.0;
pub const MAX_PLAYERS: usize = 8;
pub const RESPAWN_TIME: f32 = 3.0;
pub const SPAWN_PROT: f32 = 1.5;
pub const MATCH_TIME: f32 = 600.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weapon {
    Dagger = 0,
    Katana = 1,
    Greatsword = 2,
    Shotgun = 3,
    Revolver = 4,
    Smg = 5,
    Rifle = 6,
    Rocket = 7,
    HandCannon = 8,
    Sniper = 9,
}

/// Number of weapons (ids 0..N_WEAPONS); kill-feed ids at or above it are other things.
pub const N_WEAPONS: u8 = 10;
/// Kill-feed id for grenades.
pub const GRENADE_TOOL: u8 = 20;
/// Damage that kills outright, straight through armour (hand-cannon headshots, sniper body hits).
pub const LETHAL: f32 = 999.0;
/// Sniper hits on hands, forearms, shins or feet.
pub const SNIPER_LIMB: f32 = 60.0;
/// How long the sniper's scope takes to come up; once up, it's dead-on.
pub const SCOPE_IN: f32 = 0.15;
/// Rocket launcher: most damage (dead centre), blast radius and flight speed.
pub const ROCKET_MAX: f32 = 55.0;
pub const ROCKET_RADIUS: f32 = 3.5;
pub const ROCKET_SPEED: f32 = 24.0;

/// How a gun shoots. Spread is the tangent of the cone's half-angle: every pellet leaves within
/// it, and the crosshair is drawn exactly that wide.
pub struct GunStats {
    pub pellets: usize,
    pub spread_still: f32,
    /// Most extra spread from moving: `spread_still * move_cap` at full speed or in the air.
    pub move_cap: f32,
    /// Extra spread right after a shot (times the recoil, which fades quickly).
    pub bloom: f32,
    pub dmg: f32,
    pub range: f32,
    pub head: f32,
    pub knock: f32,
}

impl Weapon {
    pub const MELEE: [Weapon; 3] = [Weapon::Dagger, Weapon::Katana, Weapon::Greatsword];
    pub const GUNS: [Weapon; 7] = [Weapon::Shotgun, Weapon::Revolver, Weapon::HandCannon, Weapon::Smg, Weapon::Rifle, Weapon::Sniper, Weapon::Rocket];

    pub fn from_u8(v: u8) -> Weapon {
        match v {
            0 => Weapon::Dagger,
            1 => Weapon::Katana,
            2 => Weapon::Greatsword,
            3 => Weapon::Shotgun,
            4 => Weapon::Revolver,
            5 => Weapon::Smg,
            6 => Weapon::Rifle,
            8 => Weapon::HandCannon,
            9 => Weapon::Sniper,
            _ => Weapon::Rocket,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Weapon::Dagger => "Dagger",
            Weapon::Katana => "Katana",
            Weapon::Greatsword => "Greatsword",
            Weapon::Shotgun => "Scattergun",
            Weapon::Revolver => "Revolver",
            Weapon::Smg => "SMG",
            Weapon::Rifle => "Rifle",
            Weapon::Rocket => "Rocket Launcher",
            Weapon::HandCannon => "Hand Cannon",
            Weapon::Sniper => "Sniper",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Weapon::Dagger => "fast, short reach, light hits",
            Weapon::Katana => "balanced",
            Weapon::Greatsword => "slow, long reach, every hit breaks guards",
            Weapon::Shotgun => "9 pellets, deadly up close",
            Weapon::Revolver => "precise, big headshots",
            Weapon::Smg => "hold to spray, 30 rounds",
            Weapon::Rifle => "semi-auto marksman, 10 rounds",
            Weapon::Rocket => "explodes, rocket-jump with it",
            Weapon::HandCannon => "7 heavy rounds, a headshot always kills",
            Weapon::Sniper => "scope up (right click): one hit to the body kills",
        }
    }
    pub fn is_melee(self) -> bool {
        (self as u8) < 3
    }
    pub fn mag(self) -> u8 {
        match self {
            Weapon::Shotgun => 5,
            Weapon::Revolver => 6,
            Weapon::Smg => 30,
            Weapon::Rifle => 10,
            Weapon::Rocket => 1,
            Weapon::HandCannon => 7,
            Weapon::Sniper => 5,
            _ => 0,
        }
    }
    pub fn interval(self) -> f32 {
        match self {
            Weapon::Shotgun => 0.85,
            Weapon::Revolver => 0.30,
            Weapon::Smg => 0.075,
            Weapon::Rifle => 0.45,
            Weapon::Rocket => 0.9,
            Weapon::HandCannon => 0.4,
            Weapon::Sniper => 1.3,
            _ => 0.0,
        }
    }
    pub fn reload_time(self) -> f32 {
        match self {
            Weapon::Shotgun => 1.9,
            Weapon::Revolver => 1.5,
            Weapon::Smg => 1.8,
            Weapon::Rifle => 2.2,
            Weapon::Rocket => 2.6,
            Weapon::HandCannon => 1.9,
            Weapon::Sniper => 2.8,
            _ => 0.0,
        }
    }
    pub fn gun(self) -> GunStats {
        match self {
            Weapon::Shotgun => GunStats { pellets: 9, spread_still: 0.075, move_cap: 1.0, bloom: 0.0, dmg: 7.5, range: 40.0, head: 1.5, knock: 1.1 },
            Weapon::Smg => GunStats { pellets: 1, spread_still: 0.012, move_cap: 1.5, bloom: 0.25, dmg: 9.0, range: 80.0, head: 1.4, knock: 0.4 },
            Weapon::Rifle => GunStats { pellets: 1, spread_still: 0.003, move_cap: 2.0, bloom: 0.6, dmg: 34.0, range: 200.0, head: 2.0, knock: 2.0 },
            Weapon::HandCannon => GunStats { pellets: 1, spread_still: 0.012, move_cap: 1.5, bloom: 1.5, dmg: 45.0, range: 160.0, head: 1.0, knock: 2.6 },
            // unscoped the sniper sprays wide; scoped it's exact (see Fighter::spread)
            Weapon::Sniper => GunStats { pellets: 1, spread_still: 0.09, move_cap: 1.0, bloom: 0.0, dmg: SNIPER_LIMB, range: 300.0, head: 1.0, knock: 4.0 },
            _ => GunStats { pellets: 1, spread_still: 0.004, move_cap: 1.5, bloom: 0.35, dmg: 24.0, range: 150.0, head: 1.75, knock: 1.6 },
        }
    }
    /// The most one hit can do (used by the host to throw out impossible hits).
    pub fn max_hit(self) -> f32 {
        match self {
            Weapon::Dagger => 32.0 * 0.72,
            Weapon::Katana => 32.0,
            Weapon::Greatsword => 32.0 * 1.55,
            Weapon::Rocket => ROCKET_MAX,
            Weapon::HandCannon | Weapon::Sniper => LETHAL,
            w => {
                let g = w.gun();
                g.dmg * g.head
            }
        }
    }
    /// Length of one swing.
    pub fn slash_dur(self) -> f32 {
        match self {
            Weapon::Dagger => 0.26,
            Weapon::Greatsword => 0.52,
            _ => 0.36,
        }
    }
    /// After this long without firing, the next shot goes exactly where the crosshair points.
    pub fn rest_time(self) -> f32 {
        match self {
            Weapon::HandCannon => 0.65,
            Weapon::Rifle => 0.6,
            _ => REST_TIME,
        }
    }
    /// Right-click zoom with this gun (1 = none).
    pub fn zoom(self) -> f32 {
        match self {
            Weapon::Sniper => 3.0,
            Weapon::Rifle | Weapon::Revolver | Weapon::HandCannon => 1.5,
            Weapon::Shotgun | Weapon::Rocket => 1.2,
            _ => 1.0, // blades, and the SMG (right click does nothing with it)
        }
    }
    pub fn melee_mult(self) -> f32 {
        match self {
            Weapon::Dagger => 0.72,
            Weapon::Greatsword => 1.55,
            _ => 1.0,
        }
    }
    pub fn melee_range(self) -> f32 {
        match self {
            Weapon::Dagger => 2.3,
            Weapon::Greatsword => 3.4,
            _ => 2.8,
        }
    }
}

pub const ITEM_NAMES: [&str; 3] = ["Medkit", "Armor Kit", "Grenades"];
pub const ITEM_BLURBS: [&str; 3] = ["+50 health, twice a life", "+50 armor, twice a life", "two grenades a life"];

/// What a player brings: a melee weapon, two guns and an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loadout {
    pub melee: Weapon,
    pub gun1: Weapon,
    pub gun2: Weapon,
    pub item: u8,
}

impl Default for Loadout {
    fn default() -> Self {
        Loadout { melee: Weapon::Katana, gun1: Weapon::Shotgun, gun2: Weapon::Revolver, item: 2 }
    }
}

impl Loadout {
    pub fn to_bytes(self) -> [u8; 4] {
        [self.melee as u8, self.gun1 as u8, self.gun2 as u8, self.item]
    }
    /// Read and repair (a melee in the melee slot, guns in the gun slots).
    pub fn from_bytes(b: [u8; 4]) -> Loadout {
        let mut l = Loadout { melee: Weapon::from_u8(b[0]), gun1: Weapon::from_u8(b[1]), gun2: Weapon::from_u8(b[2]), item: b[3].min(2) };
        if !l.melee.is_melee() {
            l.melee = Weapon::Katana;
        }
        if l.gun1.is_melee() {
            l.gun1 = Weapon::Shotgun;
        }
        if l.gun2.is_melee() || l.gun2 == l.gun1 {
            l.gun2 = if l.gun1 == Weapon::Revolver { Weapon::Smg } else { Weapon::Revolver };
        }
        l
    }
    pub fn has(&self, w: Weapon) -> bool {
        self.melee == w || self.gun1 == w || self.gun2 == w
    }
    pub fn random(rng: &mut Rng) -> Loadout {
        let m = Weapon::MELEE[(rng.next_u32() % 3) as usize];
        let n = Weapon::GUNS.len();
        let a = rng.next_u32() as usize % n;
        let b = (a + 1 + rng.next_u32() as usize % (n - 1)) % n;
        Loadout { melee: m, gun1: Weapon::GUNS[a], gun2: Weapon::GUNS[b], item: (rng.next_u32() % 3) as u8 }
    }
}

/// What killed someone, for the kill feed (weapons by number, GRENADE_TOOL for grenades).
pub fn kill_tool(w: u8) -> &'static str {
    if w == GRENADE_TOOL { "Grenade" } else if w < N_WEAPONS { Weapon::from_u8(w).name() } else { "?" }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub fwd: f32,
    pub right: f32,
    pub jump: bool,
    pub dash: bool,
    pub fire: bool,
    pub fire_pressed: bool,
    pub guard: bool,
    pub select: Option<u8>,
    pub reload: bool,
    pub item: bool,
    /// Held: slide while running.
    pub crouch: bool,
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct WallRun {
    pub normal: V3,
    pub dir: V3,
    pub t: f32,
    pub vertical: bool,
    /// Runs as fast as you arrived (at least 10).
    pub speed: f32,
}

/// Each chained move adds this much speed...
pub const FLOW_STEP: f32 = 0.06;
/// ...up to this many steps (+30%).
pub const FLOW_MAX: u32 = 5;
/// Moves count as chained if you weren't on the ground longer than this before them.
pub const CHAIN_GROUND: f32 = 0.3;
/// Landing from a dash, wall-jump or air dash keeps your speed this long.
pub const MOMENTUM_HOLD: f32 = 0.4;
/// Jump or dash this soon after landing for a perfect-timing boost.
pub const PERFECT_WINDOW: f32 = 0.1;
/// Slides start at this speed and last at most this long.
pub const SLIDE_MIN_SPEED: f32 = 7.0;
pub const SLIDE_TIME: f32 = 0.85;
/// Health given back over a moment for each kill.
pub const KILL_HEAL: f32 = 35.0;

/// One received update of a remote fighter.
#[derive(Clone, Copy, Debug)]
pub struct Snap {
    /// The sender's clock when it was sent.
    pub t: f32,
    pub pos: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub slash_t: f32,
    pub slash_idx: u8,
}

/// A part of the body that can be hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Head = 0,
    Chest = 1,
    Hips = 2,
    ArmL = 3,
    ArmR = 4,
    LegL = 5,
    LegR = 6,
}

impl Part {
    pub fn from_u8(v: u8) -> Part {
        match v {
            0 => Part::Head,
            2 => Part::Hips,
            3 => Part::ArmL,
            4 => Part::ArmR,
            5 => Part::LegL,
            6 => Part::LegR,
            _ => Part::Chest,
        }
    }
    /// Damage taken by this part, relative to the chest (the head uses the gun's headshot bonus).
    pub fn mult(self) -> f32 {
        match self {
            Part::Head | Part::Chest => 1.0,
            Part::Hips => 0.9,
            _ => 0.8,
        }
    }
}

/// A hit shape: the segment a-b grown by r, belonging to one part of the body.
#[derive(Clone, Copy, Debug)]
pub struct Capsule {
    pub a: V3,
    pub b: V3,
    pub r: f32,
    pub part: Part,
}

pub const N_CAPS: usize = 13;

/// A fighter's hit shapes, following the animated skeleton (what you see is what you hit).
#[derive(Clone, Copy, Debug)]
pub struct HitPose {
    pub caps: [Capsule; N_CAPS],
}

impl HitPose {
    /// The first capsule a ray meets (each grown by `pad` to allow for network jitter).
    pub fn ray(&self, o: V3, d: V3, max_t: f32, pad: f32) -> Option<(f32, Part)> {
        self.ray_cap(o, d, max_t, pad).map(|(t, i)| (t, self.caps[i].part))
    }
    /// Like `ray`, but says which capsule (see `is_far_limb`).
    pub fn ray_cap(&self, o: V3, d: V3, max_t: f32, pad: f32) -> Option<(f32, usize)> {
        let mut best: Option<(f32, usize)> = None;
        for (i, c) in self.caps.iter().enumerate() {
            if let Some(t) = ray_capsule(o, d, max_t, c.a, c.b, c.r + pad) {
                if best.map_or(true, |(bt, _)| t < bt) {
                    best = Some((t, i));
                }
            }
        }
        best
    }
    /// Forearms and hands, shins and feet (capsules 6, 8, 10 and 12).
    pub fn is_far_limb(i: usize) -> bool {
        matches!(i, 6 | 8 | 10 | 12)
    }
    /// Does the ray (grown by `pad`) touch any capsule that isn't a far limb?
    pub fn ray_core(&self, o: V3, d: V3, max_t: f32, pad: f32) -> bool {
        self.caps.iter().enumerate().any(|(i, c)| !HitPose::is_far_limb(i) && ray_capsule(o, d, max_t, c.a, c.b, c.r + pad).is_some())
    }
    /// Does the ray touch this part (grown by `pad`)?
    pub fn ray_part(&self, o: V3, d: V3, max_t: f32, pad: f32, part: Part) -> bool {
        self.caps.iter().filter(|c| c.part == part).any(|c| ray_capsule(o, d, max_t, c.a, c.b, c.r + pad).is_some())
    }
    /// The part nearest a point, and how far its surface is.
    pub fn nearest(&self, p: V3) -> (Part, f32) {
        let mut best = (Part::Chest, f32::MAX);
        for c in &self.caps {
            let d = seg_dist(p, c.a, c.b) - c.r;
            if d < best.1 {
                best = (c.part, d);
            }
        }
        best
    }
    /// A box around every shape.
    pub fn bounds(&self) -> Aabb {
        let mut lo = v3(f32::MAX, f32::MAX, f32::MAX);
        let mut hi = v3(f32::MIN, f32::MIN, f32::MIN);
        for c in &self.caps {
            for p in [c.a, c.b] {
                lo = v3(lo.x.min(p.x - c.r), lo.y.min(p.y - c.r), lo.z.min(p.z - c.r));
                hi = v3(hi.x.max(p.x + c.r), hi.y.max(p.y + c.r), hi.z.max(p.z + c.r));
            }
        }
        Aabb::new(lo, hi)
    }
    pub fn lerp(&self, o: &HitPose, t: f32) -> HitPose {
        let mut out = *self;
        for (c, oc) in out.caps.iter_mut().zip(o.caps.iter()) {
            c.a = c.a.lerp(oc.a, t);
            c.b = c.b.lerp(oc.b, t);
        }
        out
    }
    /// A plain standing pose at `pos` facing `yaw` (when the animated one isn't available).
    pub fn standing(pos: V3, yaw: f32) -> HitPose {
        let r = yaw_right(yaw);
        let at = |x: f32, y: f32| pos + r * x + v3(0.0, y, 0.0);
        let c = |a: V3, b: V3, r: f32, part: Part| Capsule { a, b, r, part };
        HitPose {
            caps: [
                c(at(0.0, 1.6), at(0.0, 1.72), 0.115, Part::Head),
                c(at(0.0, 1.45), at(0.0, 1.56), 0.065, Part::Chest),
                c(at(0.0, 1.12), at(0.0, 1.36), 0.16, Part::Chest),
                c(at(-0.19, 1.42), at(0.19, 1.42), 0.085, Part::Chest),
                c(at(0.0, 0.9), at(0.0, 1.05), 0.15, Part::Hips),
                c(at(-0.2, 1.42), at(-0.24, 1.15), 0.065, Part::ArmL),
                c(at(-0.24, 1.15), at(-0.26, 0.9), 0.055, Part::ArmL),
                c(at(0.2, 1.42), at(0.24, 1.15), 0.065, Part::ArmR),
                c(at(0.24, 1.15), at(0.26, 0.9), 0.055, Part::ArmR),
                c(at(-0.1, 0.9), at(-0.1, 0.5), 0.085, Part::LegL),
                c(at(-0.1, 0.5), at(-0.09, 0.1), 0.065, Part::LegL),
                c(at(0.1, 0.9), at(0.1, 0.5), 0.085, Part::LegR),
                c(at(0.1, 0.5), at(0.09, 0.1), 0.065, Part::LegR),
            ],
        }
    }
}

/// How far behind remote fighters are shown, so a late or lost update never makes them stutter.
/// This is the least; on a jumpy internet connection it grows (see `Fighter::interp`).
pub const INTERP_DELAY: f32 = 0.075;
/// The most a remote fighter is held back on a very jumpy connection.
pub const MAX_INTERP: f32 = 0.16;

#[derive(Clone, Debug)]
pub struct Fighter {
    pub name: String,
    pub slot: usize,
    pub owned: bool,
    pub is_bot: bool,
    pub present: bool,
    pub team: u8,
    /// Watching instead of playing (Duel queue, or waiting for the next round).
    pub spectate: bool,
    /// Rounds won (Duel, Last Man Standing).
    pub wins: i32,
    pub kills: i32,
    pub deaths: i32,
    pub color_idx: u8,
    pub spawn_prot: f32,
    pub respawn_t: f32,
    pub last_weapon: u8,
    pub pos: V3,
    pub vel: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub hp: f32,
    pub ap: f32,
    pub alive: bool,
    pub death_t: f32,
    pub on_ground: bool,
    pub weapon: Weapon,
    pub mag: [u8; 3],
    pub reload_t: f32,
    pub fire_cd: f32,
    pub switch_t: f32,
    pub slash_t: f32,
    pub slash_idx: u8,
    pub slash_hit_done: bool,
    pub slash_queued: bool,
    pub combo_window: f32,
    pub guard: bool,
    pub dash_t: f32,
    pub dash_cd: f32,
    pub air_dash: bool,
    pub wall: Option<WallRun>,
    pub wallruns_left: u8,
    pub items: u8,
    pub item_cd: f32,
    pub loadout: Loadout,
    pub cur: usize,
    pub ready: bool,
    pub vote: bool,
    pub ping_ms: u16,
    pub walk_phase: f32,
    pub hurt_t: f32,
    pub stun_t: f32,
    pub recoil: f32,
    /// Seconds since the last shot, and the moving-accuracy multiplier (eases between 1 and the
    /// weapon's cap as you speed up or slow down).
    pub since_shot: f32,
    pub move_spread: f32,
    /// The body part last hit (for the flinch and the white flash) and the flash left.
    pub hit_part: u8,
    pub flash_t: f32,
    /// How long the guard has been up (a perfect parry needs it raised just in time).
    pub guard_t: f32,
    /// Reeling from a perfect parry: open to a 1.5x counter.
    pub stagger_t: f32,
    // ---- movement flow
    /// Speed carried after landing from a move, and how long it's held before easing off.
    pub carry: f32,
    pub carry_t: f32,
    /// In the air because of a dash, wall-jump or air dash (landing keeps the speed).
    pub from_move: bool,
    /// Time on the ground / in the air without a break, time since landing, last air time.
    pub ground_t: f32,
    pub air_t: f32,
    pub land_t: f32,
    pub last_air: f32,
    /// Moves chained (each +6% speed), its little pop on screen, and the slow fade on foot.
    pub flow: u32,
    pub flow_pop: f32,
    flow_fade: f32,
    still_t: f32,
    /// Sliding time left (0 = not sliding).
    pub sliding: f32,
    /// "Perfect" timing pop on screen.
    pub perfect_t: f32,
    /// Health still to come back after a kill.
    pub regen_left: f32,
    /// How long the scope (right click with a gun) has been up; 0 when not aiming.
    pub scope_t: f32,
    /// Whether the last hit taken was a headshot (for the kill feed).
    pub last_head: bool,
    pub last_attacker: Option<usize>,
    // network smoothing for remote copies
    pub net_pos: V3,
    pub net_age: f32,
    /// Recent updates for a remote copy, played back a fixed moment behind (smooth under lag).
    pub snaps: VecDeque<Snap>,
    /// Our clock minus the sender's clock.
    pub clock_off: Option<f32>,
    /// How late this player's updates tend to arrive (beyond the quickest), and the playback
    /// delay that follows from it.
    pub lateness: f32,
    pub interp: f32,
    /// Share of this player's packets the host is losing (percent).
    pub loss_pct: u8,
    // footstep sounds: which step we're on, feet on the ground last tick, fastest fall, wall patter
    pub step_n: i32,
    pub was_ground: bool,
    pub fall_vy: f32,
    pub wall_step: f32,
    pub prev_pos: V3,
    pub jump_buf: f32,
    pub coyote: f32,
    pub dash_buf: f32,
    pub ground_dash_t: f32,
    pub bfly_t: f32,
    pub air_slashes: u8,
    pub float_t: f32,
    pub walljumps: u8,
}

impl Fighter {
    pub fn new(name: &str, slot: usize, owned: bool) -> Fighter {
        Fighter {
            name: name.to_string(),
            slot,
            owned,
            is_bot: false,
            present: false,
            team: 0,
            spectate: false,
            wins: 0,
            kills: 0,
            deaths: 0,
            color_idx: slot as u8,
            spawn_prot: 0.0,
            respawn_t: 0.0,
            last_weapon: 2,
            pos: V3::ZERO,
            vel: V3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            hp: MAX_HP,
            ap: MAX_AP,
            alive: false,
            death_t: 0.0,
            on_ground: true,
            weapon: Weapon::Shotgun,
            mag: [0, Weapon::Shotgun.mag(), Weapon::Revolver.mag()],
            reload_t: 0.0,
            fire_cd: 0.0,
            switch_t: 0.0,
            slash_t: 0.0,
            slash_idx: 0,
            slash_hit_done: true,
            slash_queued: false,
            combo_window: 0.0,
            guard: false,
            dash_t: 0.0,
            dash_cd: 0.0,
            air_dash: false,
            wall: None,
            wallruns_left: 2,
            items: 2,
            item_cd: 0.0,
            loadout: Loadout::default(),
            cur: 1,
            ready: false,
            vote: false,
            ping_ms: 0,
            walk_phase: 0.0,
            hurt_t: 0.0,
            stun_t: 0.0,
            recoil: 0.0,
            since_shot: 9.0,
            move_spread: 1.0,
            hit_part: 1,
            flash_t: 0.0,
            guard_t: 0.0,
            stagger_t: 0.0,
            carry: 0.0,
            carry_t: 0.0,
            from_move: false,
            ground_t: 1.0,
            air_t: 0.0,
            land_t: 9.0,
            last_air: 0.0,
            flow: 0,
            flow_pop: 0.0,
            flow_fade: 0.0,
            still_t: 0.0,
            sliding: 0.0,
            perfect_t: 0.0,
            regen_left: 0.0,
            scope_t: 0.0,
            last_head: false,
            last_attacker: None,
            net_pos: V3::ZERO,
            net_age: 0.0,
            snaps: VecDeque::new(),
            clock_off: None,
            lateness: 0.0,
            interp: INTERP_DELAY,
            loss_pct: 0,
            step_n: 0,
            was_ground: true,
            fall_vy: 0.0,
            wall_step: 0.0,
            prev_pos: V3::ZERO,
            jump_buf: 0.0,
            coyote: 0.0,
            dash_buf: 0.0,
            ground_dash_t: 0.0,
            bfly_t: 0.0,
            air_slashes: 0,
            float_t: 0.0,
            walljumps: 2,
        }
    }

    /// Fresh body at a spawn point; who the player is (name, team, score) is kept.
    pub fn respawn_at(&mut self, p: V3, yaw: f32) {
        let mut n = Fighter::new(&self.name, self.slot, self.owned);
        n.is_bot = self.is_bot;
        n.present = self.present;
        n.team = self.team;
        n.kills = self.kills;
        n.wins = self.wins;
        n.deaths = self.deaths;
        n.color_idx = self.color_idx;
        n.loadout = self.loadout;
        n.ready = self.ready;
        n.vote = self.vote;
        n.ping_ms = self.ping_ms;
        n.loss_pct = self.loss_pct;
        n.equip();
        n.alive = true;
        n.spawn_prot = SPAWN_PROT;
        n.pos = p;
        n.net_pos = p;
        n.prev_pos = p;
        n.yaw = yaw;
        *self = n;
    }

    pub fn slot_weapon(&self, i: usize) -> Weapon {
        match i {
            0 => self.loadout.melee,
            1 => self.loadout.gun1,
            _ => self.loadout.gun2,
        }
    }

    /// Hand out the loadout: full magazines, item uses, first gun in hand.
    pub fn equip(&mut self) {
        self.loadout = Loadout::from_bytes(self.loadout.to_bytes());
        self.mag = [0, self.loadout.gun1.mag(), self.loadout.gun2.mag()];
        self.cur = 1;
        self.weapon = self.loadout.gun1;
        self.items = 2;
        self.reload_t = 0.0;
        self.slash_t = 0.0;
    }

    pub fn eye(&self) -> V3 {
        self.pos + v3(0.0, 1.62, 0.0)
    }

    /// Speed bonus from chained moves (1.0 to 1.3).
    pub fn flow_mult(&self) -> f32 {
        1.0 + FLOW_STEP * self.flow.min(FLOW_MAX) as f32
    }

    /// A move in a chain: counts if you were barely on the ground before it, else starts a new one.
    pub fn chain(&mut self) {
        self.flow = if self.ground_t <= CHAIN_GROUND { (self.flow + 1).min(FLOW_MAX) } else { 1 };
        self.flow_pop = 0.35;
        self.flow_fade = 0.0;
    }

    /// The spread the next shot will have (tangent of the cone's half-angle; 0 = dead on).
    /// The shot and the crosshair both use this, so every pellet lands inside the crosshair.
    pub fn spread(&self) -> f32 {
        if self.weapon.is_melee() || self.weapon == Weapon::Rocket {
            return 0.0;
        }
        let gs = self.weapon.gun();
        if gs.pellets > 1 {
            return gs.spread_still * self.move_spread; // the shotgun's cone is the cone
        }
        if self.weapon == Weapon::Sniper {
            // wild from the hip, dead-on the moment the scope is up (quickscopes welcome)
            return if self.scope_t >= SCOPE_IN { 0.0 } else { gs.spread_still };
        }
        if self.since_shot >= self.weapon.rest_time() {
            return 0.0; // a rested first shot is perfectly accurate
        }
        gs.spread_still * self.move_spread * (1.0 + gs.bloom * self.recoil)
    }
    pub fn chest(&self) -> V3 {
        self.pos + v3(0.0, 1.2, 0.0)
    }
    pub fn fwd(&self) -> V3 {
        yaw_fwd(self.yaw)
    }
    pub fn body_box(&self) -> Aabb {
        Aabb::new(self.pos + v3(-0.42, 0.0, -0.42), self.pos + v3(0.42, 1.5, 0.42))
    }
    pub fn head_box(&self) -> Aabb {
        Aabb::new(self.pos + v3(-0.22, 1.5, -0.22), self.pos + v3(0.22, 1.9, 0.22))
    }
    pub fn slashing(&self) -> bool {
        self.slash_t > 0.0
    }
    pub fn flags(&self) -> u8 {
        let mut f = 0u8;
        if self.alive {
            f |= 1;
        }
        if self.on_ground {
            f |= 2;
        }
        if self.guard {
            f |= 4;
        }
        if self.wall.is_some() {
            f |= 8;
        }
        if self.wall.map_or(false, |w| w.vertical) {
            f |= 16;
        }
        if self.dash_t > 0.0 {
            f |= 32;
        }
        if self.reload_t > 0.0 {
            f |= 64;
        }
        if self.stun_t > 0.0 {
            f |= 128;
        }
        f
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Ffa = 0,
    Tdm = 1,
    Duel = 2,
    Gladiator = 3,
    Lms = 4,
    Ctf = 5,
}

impl Mode {
    pub const ALL: [Mode; 6] = [Mode::Ffa, Mode::Tdm, Mode::Duel, Mode::Gladiator, Mode::Lms, Mode::Ctf];
    pub fn from_u8(v: u8) -> Mode {
        match v {
            1 => Mode::Tdm,
            2 => Mode::Duel,
            3 => Mode::Gladiator,
            4 => Mode::Lms,
            5 => Mode::Ctf,
            _ => Mode::Ffa,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Mode::Ffa => "Free-for-All",
            Mode::Tdm => "Team Deathmatch",
            Mode::Duel => "Duel",
            Mode::Gladiator => "Gladiator",
            Mode::Lms => "Last Man Standing",
            Mode::Ctf => "Capture the Flag",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Mode::Ffa => "everyone for themselves, first to the kill limit",
            Mode::Tdm => "Red vs Blue, team kills count",
            Mode::Duel => "1v1 rounds, the winner stays on, the rest queue",
            Mode::Gladiator => "melee weapons only, first to the kill limit",
            Mode::Lms => "rounds with no respawns, last one alive wins",
            Mode::Ctf => "Red vs Blue, bring the enemy flag to your base",
        }
    }
    pub fn teams(self) -> bool {
        matches!(self, Mode::Tdm | Mode::Ctf)
    }
    pub fn rounds(self) -> bool {
        matches!(self, Mode::Duel | Mode::Lms)
    }
    pub fn melee_only(self) -> bool {
        self == Mode::Gladiator
    }
    /// What the score limit counts.
    pub fn goal_word(self) -> &'static str {
        match self {
            Mode::Duel | Mode::Lms => "round wins",
            Mode::Ctf => "captures",
            _ => "kills",
        }
    }
}

/// A team's flag (Capture the Flag).
#[derive(Clone, Copy, Debug)]
pub struct Flag {
    /// 0 at base, 1 carried, 2 dropped
    pub state: u8,
    pub carrier: u8,
    pub pos: V3,
    /// seconds until a dropped flag goes home
    pub timer: f32,
}

pub const ROUND_TIME: f32 = 120.0;

pub fn flag_base(team: usize) -> V3 {
    if team == 0 { v3(0.0, 0.0, -21.5) } else { v3(0.0, 0.0, 21.5) }
}

pub const TEAM_NAMES: [&str; 2] = ["Red", "Blue"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Warmup = 0,
    Loadout = 1,
    Countdown = 2,
    Fight = 3,
    MatchOver = 4,
}

impl Phase {
    pub fn from_u8(v: u8) -> Phase {
        match v {
            0 => Phase::Warmup,
            1 => Phase::Loadout,
            2 => Phase::Countdown,
            3 => Phase::Fight,
            _ => Phase::MatchOver,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MatchState {
    pub mode: Mode,
    pub phase: Phase,
    pub timer: f32,
    /// Kills to win (a player's in Free-for-All, a team's in Team Deathmatch).
    pub target: u16,
    pub team_score: [u16; 2],
    /// Goes up at every (re)start, so everyone respawns together.
    pub number: u32,
    /// Round modes: the round number, and time left in the pause between rounds.
    pub round: u32,
    pub round_end: f32,
    /// Duel: who is fighting (255 = nobody).
    pub duel: [u8; 2],
    pub flags: [Flag; 2],
}

#[derive(Clone, Debug)]
pub struct Grenade {
    pub pos: V3,
    pub vel: V3,
    pub fuse: f32,
    pub owner: usize,
    /// 0 grenade, 1 rocket
    pub kind: u8,
}

#[derive(Clone, Debug)]
pub struct Particle {
    pub pos: V3,
    pub vel: V3,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub color: [u8; 3],
    pub gravity: f32,
}

#[derive(Clone, Debug)]
pub struct Tracer {
    pub a: V3,
    pub b: V3,
    pub life: f32,
    pub max_life: f32,
    pub width: f32,
    pub color: [u8; 3],
}

impl Tracer {
    /// A bullet's streak: the sniper's is bright, thick and lingers.
    pub fn shot(a: V3, b: V3, w: Weapon, color: [u8; 3]) -> Tracer {
        match w {
            Weapon::Sniper => Tracer { a, b, life: 0.45, max_life: 0.45, width: 0.07, color: [255, 250, 235] },
            Weapon::HandCannon => Tracer { a, b, life: 0.12, max_life: 0.12, width: 0.045, color },
            _ => Tracer { a, b, life: 0.09, max_life: 0.09, width: 0.03, color },
        }
    }
}

#[derive(Clone, Debug)]
pub struct Casing {
    pub pos: V3,
    pub vel: V3,
    pub life: f32,
    pub spin: f32,
    pub color: [u8; 3],
}

#[derive(Clone, Debug)]
pub struct Flash {
    pub pos: V3,
    pub dir: V3,
    pub life: f32,
    pub size: f32,
}

/// The slow-motion look at the kill that ended the match.
#[derive(Clone, Debug)]
pub struct KillCam {
    pub victim: usize,
    pub killer: Option<usize>,
    pub tool: &'static str,
    pub head: bool,
    pub t: f32,
}

pub const KILLCAM_TIME: f32 = 3.4;

#[derive(Clone, Debug)]
pub struct DmgNum {
    pub pos: V3,
    pub amount: i32,
    pub life: f32,
    pub head: bool,
}

/// A kill feed line.
#[derive(Clone, Debug)]
pub struct FeedLine {
    pub killer: Option<(String, u8)>,
    pub victim: (String, u8),
    pub tool: &'static str,
    /// A headshot kill (a small mark next to the weapon).
    pub head: bool,
    pub text: Option<String>,
    pub life: f32,
}

/// Events that cross the network (each tagged with the player it came from).
#[derive(Clone, Debug)]
pub enum Ev {
    Shot { o: V3, e: V3, w: u8, spark: u8 },
    /// From a player: "I hit them" (the host checks it). From the host: "you were hit".
    Hit { victim: u8, dmg: f32, knock: V3, stun: f32, head: bool, w: u8, o: V3, dir: V3 },
    Grenade { p: V3, v: V3, kind: u8 },
    /// Only the host (or the game on this computer) decides deaths.
    Died { victim: u8, killer: u8, w: u8, head: bool },
    Chat { text: String },
    /// Used a medkit (0) or armor kit (1).
    Item { kind: u8 },
    Ready,
    Vote,
}

#[derive(Clone, Debug)]
pub struct ChatLine {
    pub name: String,
    pub color: u8,
    pub text: String,
    pub life: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Demo,
    Offline,
    Host,
    Client,
}

pub struct Game {
    pub world: World,
    pub f: Vec<Fighter>,
    pub local: usize,
    pub kind: Kind,
    pub mt: MatchState,
    /// Duel: who's waiting, in order (host only).
    pub queue: VecDeque<usize>,
    duel_loser: Option<usize>,
    pub grenades: Vec<Grenade>,
    pub particles: Vec<Particle>,
    pub tracers: Vec<Tracer>,
    pub dmg_nums: Vec<DmgNum>,
    pub casings: Vec<Casing>,
    pub flashes: Vec<Flash>,
    /// A short freeze when a sword hit lands (seconds of real time left).
    pub hitstop: f32,
    /// Slow motion left (real seconds).
    pub slowmo: f32,
    pub killcam: Option<KillCam>,
    last_kill: Option<(usize, Option<usize>, u8, bool)>,
    pub rng: Rng,
    pub bots: Vec<Option<Bot>>,
    pub out_events: Vec<Ev>,
    pub feed: Vec<FeedLine>,
    pub chat: Vec<ChatLine>,
    /// Where everyone was recently (for checking hits against the past).
    pub history: Vec<VecDeque<(f32, V3)>>,
    /// The loadout you'll respawn with.
    pub my_loadout: Loadout,
    pub hitmarker: f32,
    /// Where recent hits on you came from (world point, time left).
    pub hurt_dirs: Vec<(V3, f32)>,
    /// Kill confirmation: time left, who, kills in a row.
    pub kill_t: f32,
    pub kill_name: String,
    pub streak: u32,
    last_kill_time: f32,
    pub headshot_t: f32,
    pub hurt_flash: f32,
    pub shake: f32,
    pub time: f32,
    pub banner: Option<(String, f32)>,
    /// Sounds to play: where they happened (None = not in the world, e.g. a hit marker).
    pub sounds: Vec<(Sfx, Option<V3>, f32)>,
    /// When each kind of sound last played (to keep a shotgun's pellets from stacking up).
    sfx_last: [f32; SFX_SLOTS],
    /// When each player's last gunshot sound played (remote shots arrive once per pellet).
    shot_last: [f32; MAX_PLAYERS],
    /// Training room state (only in Training).
    pub train: Option<Training>,
    /// (player) after healing ourselves, ignore the host's older health figure for this long.
    pub hp_hold: f32,
    /// Each fighter's animation as the game sees it, and the hit shapes it gives.
    anim: Vec<crate::character::CharState>,
    pub hit_poses: Vec<Option<HitPose>>,
    /// (host) recent hit shapes, to check hits against the past.
    pose_history: Vec<VecDeque<(f32, HitPose)>>,
    /// Hits in a row (their tick climbs in pitch) and when the last one landed.
    hit_chain: u32,
    hit_chain_t: f32,
    /// Hit marker details: headshot, and whether it was a kill.
    pub hit_head: bool,
    /// A white flash on screen after a perfect parry.
    pub parry_flash: f32,
    /// "+35 HP" shown after a kill.
    pub heal_pop: f32,
    /// Upward aim kick from the last shot, for the screen to apply to the mouse aim.
    pub aim_kick: f32,
}

/// Moves the player has pulled off in the training room.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrainStats {
    pub jumps: u32,
    pub dashes: u32,
    pub air_dashes: u32,
    pub wallruns: u32,
    pub wallclimbs: u32,
    pub walljumps: u32,
    pub slash_hits: u32,
    pub cancels: u32,
    pub butterflies: u32,
    pub launches: u32,
    pub air_hits: u32,
    pub rocket_jumps: u32,
    pub slides: u32,
    pub slide_jumps: u32,
    pub momentum: u32,
    pub perfects: u32,
    pub best_flow: u32,
    pub cannon_heads: u32,
    pub sniper_hits: u32,
}

pub struct Lesson {
    pub title: &'static str,
    pub how: &'static str,
    pub count: fn(&TrainStats) -> u32,
    pub need: u32,
    /// A gun to hand over in slot 3 for this lesson.
    pub gun: Option<Weapon>,
}

pub const LESSONS: [Lesson; 18] = [
    Lesson { title: "Move and jump", how: "Move with W A S D and look with the mouse. Press SPACE to jump.", count: |s| s.jumps, need: 3, gun: None },
    Lesson { title: "Dash", how: "Press SHIFT (or double-tap a direction key) to dash. Dash in different directions.", count: |s| s.dashes, need: 3, gun: None },
    Lesson { title: "Air dash", how: "Jump with SPACE, then press SHIFT in the air for a second burst of speed.", count: |s| s.air_dashes, need: 2, gun: None },
    Lesson { title: "Wall-run", how: "Run along a wall, jump, then press SPACE again next to the wall while moving along it.", count: |s| s.wallruns, need: 2, gun: None },
    Lesson { title: "Wall climb", how: "Face a wall, jump, then press SPACE again while looking straight at it to run up.", count: |s| s.wallclimbs, need: 1, gun: None },
    Lesson { title: "Wall-jump", how: "While on a wall, press SPACE again to kick off it. Chain wall to wall!", count: |s| s.walljumps, need: 2, gun: None },
    Lesson { title: "Slide", how: "Run, then hold C to slide along the ground. Press SPACE mid-slide to jump out of it with your speed.", count: |s| s.slides.min(1) + s.slide_jumps.min(1), need: 2, gun: None },
    Lesson { title: "Momentum", how: "Land from a dash, air dash or wall-jump: you keep your speed for a moment instead of slowing to a run.", count: |s| s.momentum, need: 3, gun: None },
    Lesson { title: "Perfect timing", how: "Jump or dash the instant you land (within a tenth of a second) for an extra boost and a whoosh.", count: |s| s.perfects, need: 2, gun: None },
    Lesson { title: "Flow chain", how: "Chain moves without staying on the ground: dash, wall-run, wall-jump, air dash. Each adds +6% speed. Reach x4!", count: |s| (s.best_flow >= 4) as u32, need: 1, gun: None },
    Lesson { title: "Sword slash", how: "Press 1 for your katana and click to slash a dummy. Click again to chain swings.", count: |s| s.slash_hits, need: 4, gun: None },
    Lesson { title: "Slash cancel", how: "Right after a slash hits, press SHIFT, SPACE or hold right-click to cut its recovery short.", count: |s| s.cancels, need: 3, gun: None },
    Lesson { title: "Butterfly", how: "Slash, hold right-click to guard, release and slash again. Keep the rhythm going!", count: |s| s.butterflies, need: 4, gun: None },
    Lesson { title: "Launcher", how: "Dash at a dummy and slash during the dash (or land the 3rd hit of a ground combo) to launch it.", count: |s| s.launches, need: 2, gun: None },
    Lesson { title: "Air combo", how: "Launch a dummy, jump after it and slash it in mid-air before it lands.", count: |s| s.air_hits, need: 2, gun: None },
    Lesson { title: "Hand cannon", how: "Press 3 for the hand cannon. A headshot always kills, even through armour. Land 2 headshots on a dummy (wait a beat between shots: the first is dead-on).", count: |s| s.cannon_heads, need: 2, gun: Some(Weapon::HandCannon) },
    Lesson { title: "Sniper", how: "Press 3 for the sniper and hold right click to scope: it's dead-on once the scope is up. Hit a dummy twice (one body hit kills a real player).", count: |s| s.sniper_hits, need: 2, gun: Some(Weapon::Sniper) },
    Lesson { title: "Rocket jump", how: "Press 2 for the rocket launcher. Look down at your feet, jump and fire at the same moment.", count: |s| s.rocket_jumps, need: 1, gun: None },
];

pub struct Training {
    /// Current lesson (LESSONS.len() = all done).
    pub lesson: usize,
    pub stats: TrainStats,
    /// Stats when this lesson started.
    pub base: TrainStats,
    /// "Done!" flash before moving on.
    pub done_t: f32,
    pub combo: u32,
    pub combo_t: f32,
    pub combo_dmg: f32,
    pub best_combo: u32,
    pub homes: [V3; 3],
}

impl Training {
    pub fn progress(&self) -> (u32, u32) {
        match LESSONS.get(self.lesson) {
            Some(l) => (((l.count)(&self.stats) - (l.count)(&self.base)).min(l.need), l.need),
            None => (0, 0),
        }
    }
    pub fn go_to(&mut self, lesson: usize) {
        self.lesson = lesson.min(LESSONS.len());
        self.base = self.stats;
        self.done_t = 0.0;
    }
}

/// Room for every kind of sound (must stay above the number of `Sfx` kinds; a test checks).
pub const SFX_SLOTS: usize = 64;

/// Every sound the game makes (recorded clips live in audio.rs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sfx {
    Shotgun,
    Revolver,
    Smg,
    Rifle,
    Rocket,
    Slash,
    Clang,
    Flesh,
    BulletFlesh,
    ImpactStone,
    Boom,
    Dash,
    Jump,
    Land,
    StepStone,
    StepWood,
    WallRun,
    Reload,
    ReloadEnd,
    SwitchBlade,
    SwitchGun,
    Throw,
    Heal,
    Armor,
    Hurt,
    Tick,
    Hit,
    Kill,
    Headshot,
    UiClick,
    UiConfirm,
    UiBack,
    MatchStart,
    MatchOver,
    HitHeavy,
    ArmorBreak,
    ParryPerfect,
    Perfect,
    Slide,
    HandCannon,
    Sniper,
    ScopeIn,
}

impl Sfx {
    /// The gunshot of a weapon.
    pub fn gun(w: Weapon) -> Sfx {
        match w {
            Weapon::Shotgun => Sfx::Shotgun,
            Weapon::Smg => Sfx::Smg,
            Weapon::Rifle => Sfx::Rifle,
            Weapon::Rocket => Sfx::Rocket,
            Weapon::HandCannon => Sfx::HandCannon,
            Weapon::Sniper => Sfx::Sniper,
            _ => Sfx::Revolver,
        }
    }
}

/// Raise the guard no more than this long before a slash lands for a perfect parry.
pub const PERFECT_PARRY: f32 = 0.15;
/// How long a perfectly parried attacker reels.
pub const STAGGER_TIME: f32 = 0.6;

/// After this long without firing, the next shot goes exactly where the crosshair points.
pub const REST_TIME: f32 = 0.4;

/// What the host could confirm about a player's hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClaimCheck {
    pub head_ok: bool,
    pub core_ok: bool,
}

pub enum Act {
    /// A gunshot with the spread it had the moment it was fired.
    Gun { w: Weapon, spread: f32 },
    Slash,
    Throw,
    Item { kind: u8 },
}

pub const BOT_NAMES: [&str; 7] = ["Ash", "Vex", "Kiro", "Nyx", "Rook", "Sable", "Juno"];

impl Game {
    pub fn new(kind: Kind, local_slot: usize, mode: Mode, target: u16) -> Game {
        let world = World::arena();
        let f = (0..MAX_PLAYERS).map(|s| Fighter::new("", s, true)).collect();
        let bots = (0..MAX_PLAYERS).map(|_| None).collect();
        let (phase, timer) = match kind {
            Kind::Demo => (Phase::Countdown, 0.3),
            Kind::Offline => (Phase::Loadout, 120.0),
            Kind::Host | Kind::Client => (Phase::Warmup, 0.0),
        };
        Game {
            world,
            f,
            local: local_slot,
            kind,
            mt: MatchState {
                mode,
                phase,
                timer,
                target: target.max(1),
                team_score: [0, 0],
                number: 1,
                round: 0,
                round_end: 0.0,
                duel: [255, 255],
                flags: [0, 1].map(|t| Flag { state: 0, carrier: 255, pos: flag_base(t), timer: 0.0 }),
            },
            queue: VecDeque::new(),
            duel_loser: None,
            grenades: Vec::new(),
            particles: Vec::new(),
            tracers: Vec::new(),
            dmg_nums: Vec::new(),
            casings: Vec::new(),
            flashes: Vec::new(),
            hitstop: 0.0,
            slowmo: 0.0,
            killcam: None,
            last_kill: None,
            rng: Rng(0x9E3779B97F4A7C15 ^ (local_slot as u64 + 1).wrapping_mul(7919)),
            bots,
            out_events: Vec::new(),
            feed: Vec::new(),
            chat: Vec::new(),
            history: (0..MAX_PLAYERS).map(|_| VecDeque::new()).collect(),
            my_loadout: Loadout::default(),
            hitmarker: 0.0,
            hurt_dirs: Vec::new(),
            kill_t: 0.0,
            kill_name: String::new(),
            streak: 0,
            last_kill_time: -100.0,
            headshot_t: 0.0,
            hurt_flash: 0.0,
            shake: 0.0,
            time: 0.0,
            banner: match kind {
                Kind::Host | Kind::Client => Some(("WARM-UP".into(), 2.5)),
                _ => Some(("GET READY".into(), 2.0)),
            },
            sounds: Vec::new(),
            sfx_last: [-1.0; SFX_SLOTS],
            shot_last: [-1.0; MAX_PLAYERS],
            train: None,
            hp_hold: 0.0,
            anim: (0..MAX_PLAYERS).map(|_| crate::character::CharState::new()).collect(),
            hit_poses: vec![None; MAX_PLAYERS],
            pose_history: (0..MAX_PLAYERS).map(|_| VecDeque::new()).collect(),
            hit_chain: 0,
            hit_chain_t: -10.0,
            hit_head: false,
            parry_flash: 0.0,
            heal_pop: 0.0,
            aim_kick: 0.0,
        }
    }

    /// The training room: you, a katana and a rocket launcher, and three dummies that can't die.
    pub fn training(name: &str) -> Game {
        let mut g = Game::new(Kind::Offline, 0, Mode::Ffa, 9999);
        g.my_loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Rocket, gun2: Weapon::Revolver, item: 2 };
        g.add_player(0, name, true, None, 0);
        let homes = [v3(-3.5, 0.0, -6.0), v3(0.0, 0.0, -6.2), v3(3.5, 0.0, -6.0)];
        for (k, &h) in homes.iter().enumerate() {
            let s = k + 1;
            g.add_player(s, &format!("Dummy {}", s), true, Some(0.0), 0);
            g.bots[s] = None; // stands still
            g.f[s].loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Revolver, gun2: Weapon::Shotgun, item: 0 };
            g.f[s].equip();
            g.f[s].respawn_at(h, 0.0);
            g.f[s].spawn_prot = 0.0;
        }
        g.f[0].respawn_at(v3(0.0, 0.0, -10.6), 0.0); // the long wall behind you is for wall-runs
        g.f[0].spawn_prot = 0.0;
        g.mt.phase = Phase::Fight;
        g.mt.timer = 1e9;
        g.banner = Some(("TRAINING".into(), 2.0));
        g.train = Some(Training { lesson: 0, stats: TrainStats::default(), base: TrainStats::default(), done_t: 0.0, combo: 0, combo_t: 0.0, combo_dmg: 0.0, best_combo: 0, homes });
        g
    }

    /// Count a move for the training lessons (only the local player, only in training).
    fn note(&mut self, i: usize, f: impl FnOnce(&mut TrainStats)) {
        if i == self.local {
            if let Some(t) = self.train.as_mut() {
                f(&mut t.stats);
            }
        }
    }

    /// Training combo counter: hits that land within 1.5 s of each other keep the combo going.
    fn combo_hit(&mut self, amount: f32) {
        if let Some(t) = self.train.as_mut() {
            t.combo += 1;
            t.combo_t = 1.5;
            t.combo_dmg += amount;
            t.best_combo = t.best_combo.max(t.combo);
        }
    }

    /// Training room upkeep: nobody dies, health comes back, dummies return home, lessons advance.
    fn update_training(&mut self, dt: f32) {
        let Some(mut t) = self.train.take() else { return };
        let me = self.local;
        for i in 0..self.f.len() {
            let ff = &mut self.f[i];
            if !ff.present {
                continue;
            }
            ff.alive = true;
            if ff.hp < 1.0 {
                ff.hp = 1.0;
            }
            if ff.hurt_t <= 0.0 {
                ff.hp = (ff.hp + 60.0 * dt).min(100.0);
            }
            if i != me {
                ff.ap = 0.0;
                // face the player
                let d = self.f[me].pos - self.f[i].pos;
                self.f[i].yaw = d.x.atan2(d.z);
                let home = t.homes[(i - 1).min(2)];
                let ff = &mut self.f[i];
                if ff.on_ground && ff.float_t <= 0.0 && ff.stun_t <= 0.0 && (ff.pos - home).flat().len() > 4.0 {
                    ff.pos = home;
                    ff.prev_pos = home;
                    ff.vel = V3::ZERO;
                }
            }
        }
        // refill ammo and grenades so practice never runs dry; hand over this lesson's gun
        if let Some(f) = self.f.get_mut(me) {
            f.items = f.items.max(1);
            t.stats.best_flow = t.stats.best_flow.max(f.flow);
            if let Some(gw) = LESSONS.get(t.lesson).and_then(|l| l.gun) {
                if f.loadout.gun2 != gw {
                    f.loadout.gun2 = gw;
                    f.mag[2] = gw.mag();
                    if f.cur == 2 {
                        f.weapon = gw;
                        f.reload_t = 0.0;
                    }
                    self.my_loadout.gun2 = gw;
                }
            }
        }
        if t.combo_t > 0.0 {
            t.combo_t -= dt;
            if t.combo_t <= 0.0 {
                t.combo = 0;
                t.combo_dmg = 0.0;
            }
        }
        if t.lesson < LESSONS.len() {
            let (have, need) = t.progress();
            if t.done_t > 0.0 {
                t.done_t -= dt;
                if t.done_t <= 0.0 {
                    let next = t.lesson + 1;
                    t.go_to(next);
                    if next >= LESSONS.len() {
                        self.banner = Some(("TRAINING COMPLETE!".into(), 3.0));
                    }
                }
            } else if have >= need {
                t.done_t = 1.6;
                self.sfx2d(Sfx::Kill);
            }
        }
        self.train = Some(t);
    }

    /// Training: jump to the next (or previous) lesson.
    pub fn train_skip(&mut self, step: i32) {
        if let Some(t) = self.train.as_mut() {
            let l = (t.lesson as i32 + step).clamp(0, LESSONS.len() as i32) as usize;
            t.go_to(l);
        }
    }

    /// A practice match on this computer: you in slot 0 and `bots` bots.
    pub fn offline(name: &str, bots: usize, skill: f32, mode: Mode, target: u16) -> Game {
        let mut g = Game::new(Kind::Offline, 0, mode, target);
        g.add_player(0, name, true, None, 0);
        for b in 0..bots.min(MAX_PLAYERS - 1) {
            let s = b + 1;
            let team = (s % 2) as u8;
            g.add_player(s, BOT_NAMES[b], true, Some(skill), team);
            g.f[s].ready = true;
        }
        g.banner = Some(("CHOOSE YOUR LOADOUT".into(), 2.5));
        g
    }

    /// The menu backdrop: bots fighting each other.
    pub fn demo() -> Game {
        let mut g = Game::new(Kind::Demo, 0, Mode::Ffa, 15);
        for s in 0..4 {
            g.add_player(s, BOT_NAMES[s], true, Some(0.7), 0);
        }
        // start them near the middle so the backdrop shows the action
        let starts = [v3(-2.5, 0.0, -6.5), v3(2.5, 0.0, 6.5), v3(-6.0, 0.0, 4.0), v3(6.0, 0.0, -4.0)];
        for s in 0..4 {
            let p = starts[s];
            g.f[s].respawn_at(p, (-p.x).atan2(-p.z));
            g.f[s].spawn_prot = 0.0;
        }
        g
    }

    pub fn add_player(&mut self, slot: usize, name: &str, owned: bool, bot_skill: Option<f32>, team: u8) {
        let mut f = Fighter::new(name, slot, owned);
        f.present = true;
        f.team = team;
        f.is_bot = bot_skill.is_some();
        f.loadout = if f.is_bot { Loadout::random(&mut self.rng) } else if slot == self.local { self.my_loadout } else { Loadout::default() };
        f.ready = f.is_bot;
        f.equip();
        self.bots[slot] = bot_skill.map(|s| Bot::new(s, 1234 + slot as u64 * 77));
        self.history[slot].clear();
        self.f[slot] = f;
        self.refresh_colors();
        // in round modes, newcomers wait for the next round
        let waiting = self.mt.mode.rounds() && self.mt.phase == Phase::Fight;
        if self.mt.mode == Mode::Duel && !self.queue.contains(&slot) && self.mt.duel.iter().all(|&d| d as usize != slot) {
            self.queue.push_back(slot);
        }
        if waiting {
            let f = &mut self.f[slot];
            f.spectate = true;
            f.alive = false;
            f.respawn_t = 1e9;
        } else if owned {
            let (p, y) = self.pick_spawn(slot);
            self.f[slot].respawn_at(p, y);
        }
    }

    pub fn remove_player(&mut self, slot: usize) {
        self.f[slot] = Fighter::new("", slot, true);
        self.bots[slot] = None;
        self.queue.retain(|&q| q != slot);
        for d in self.mt.duel.iter_mut() {
            if *d as usize == slot {
                *d = 255;
            }
        }
    }

    /// Which side of a duel a player is on, if fighting.
    pub fn duel_side(&self, slot: usize) -> Option<usize> {
        self.mt.duel.iter().position(|&d| d as usize == slot)
    }

    /// Players taking part right now (not watching).
    pub fn participants(&self) -> Vec<usize> {
        (0..self.f.len()).filter(|&i| self.f[i].present && !self.f[i].spectate).collect()
    }

    /// Where a bot should head in objective modes.
    pub fn objective(&self, i: usize) -> Option<V3> {
        if self.mt.mode != Mode::Ctf {
            return None;
        }
        let my = (self.f[i].team as usize).min(1);
        let enemy = 1 - my;
        let ef = self.mt.flags[enemy];
        let mf = self.mt.flags[my];
        if ef.state == 1 && ef.carrier as usize == i {
            return Some(flag_base(my));
        }
        if mf.state == 2 {
            return Some(mf.pos);
        }
        if ef.state != 1 {
            return Some(ef.pos);
        }
        None
    }

    pub fn present(&self) -> Vec<usize> {
        (0..self.f.len()).filter(|&i| self.f[i].present).collect()
    }

    pub fn team_counts(&self) -> [usize; 2] {
        let mut c = [0, 0];
        for f in self.f.iter().filter(|f| f.present) {
            c[(f.team as usize).min(1)] += 1;
        }
        c
    }

    /// The team a newcomer should join.
    pub fn balance_team(&self) -> u8 {
        let c = self.team_counts();
        if c[1] < c[0] { 1 } else { 0 }
    }

    pub fn is_enemy(&self, a: usize, b: usize) -> bool {
        a != b && self.f[b].present && !self.f[b].spectate && (!self.mt.mode.teams() || self.f[a].team != self.f[b].team)
    }

    /// Closest living enemy, preferring ones in sight.
    pub fn nearest_enemy(&self, i: usize) -> Option<usize> {
        let me = self.f[i].eye();
        let mut best: Option<(f32, usize)> = None;
        for j in 0..self.f.len() {
            if !self.is_enemy(i, j) || !self.f[j].alive {
                continue;
            }
            let mut d = (self.f[j].pos - self.f[i].pos).len();
            if !self.world.line_clear(me, self.f[j].chest()) {
                d += 25.0;
            }
            if best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, j));
            }
        }
        best.map(|(_, j)| j)
    }

    /// The spawn point furthest from enemies (on your team's side in Team Deathmatch).
    pub fn pick_spawn(&mut self, slot: usize) -> (V3, f32) {
        if self.mt.mode == Mode::Duel && self.mt.phase == Phase::Fight {
            if let Some(side) = self.duel_side(slot) {
                let n = self.world.spawn_points.len();
                return self.world.spawn_points[if side == 0 { 0 } else { n / 2 }];
            }
        }
        let tdm = self.mt.mode.teams();
        let team = self.f[slot].team;
        let mut best = (f32::MIN, self.world.spawn_points[0]);
        for &(p, yaw) in &self.world.spawn_points {
            if tdm && ((team == 0) != (p.z < 0.0)) {
                continue;
            }
            let mut near = 60.0f32;
            for j in 0..self.f.len() {
                if j != slot && self.f[j].present && self.f[j].alive && self.is_enemy(slot, j) {
                    near = near.min((self.f[j].pos - p).len());
                }
            }
            let score = near + self.rng.range(0.0, 6.0);
            if score > best.0 {
                best = (score, (p, yaw));
            }
        }
        best.1
    }

    pub fn refresh_colors(&mut self) {
        let tdm = self.mt.mode.teams();
        for f in self.f.iter_mut() {
            f.color_idx = if tdm { f.team.min(1) } else { f.slot as u8 };
        }
    }

    pub fn is_authority(&self) -> bool {
        self.kind != Kind::Client
    }

    pub fn frozen(&self) -> bool {
        matches!(self.mt.phase, Phase::Loadout | Phase::Countdown | Phase::MatchOver)
    }

    pub fn scoring(&self) -> bool {
        self.mt.phase == Phase::Fight
    }

    /// A sound in the world at `at`.
    fn sfx(&mut self, s: Sfx, at: V3) {
        // quick repeats of small sounds (pellets, impacts) play once
        let gap = match s {
            Sfx::ImpactStone | Sfx::BulletFlesh | Sfx::Flesh | Sfx::Clang => 0.035,
            _ => 0.0,
        };
        let k = s as usize;
        if gap > 0.0 && self.time - self.sfx_last[k] < gap {
            return;
        }
        self.sfx_last[k] = self.time;
        // only sounds near the local view
        let d = (at - self.f[self.local].pos).len();
        if d < 80.0 || self.kind == Kind::Demo {
            self.sounds.push((s, Some(at), 1.0));
        }
    }

    /// Footsteps on stone or wood, landings and the patter of a wall run, for everyone.
    fn footsteps(&mut self, dt: f32) {
        if self.kind == Kind::Demo {
            return;
        }
        for i in 0..self.f.len() {
            let f = &mut self.f[i];
            if !f.present || !f.alive || f.spectate {
                f.was_ground = true;
                continue;
            }
            let mut out = None;
            if f.on_ground {
                if !f.was_ground && f.fall_vy < -5.5 {
                    out = Some(Sfx::Land);
                }
                f.fall_vy = 0.0;
                let n = (f.walk_phase / std::f32::consts::PI).floor() as i32;
                if n != f.step_n && f.vel.flat().len() > 2.0 && out.is_none() {
                    out = Some(Sfx::StepStone); // the surface is checked below
                }
                f.step_n = n;
            } else {
                f.fall_vy = f.fall_vy.min(f.vel.y);
            }
            if f.wall.is_some() {
                f.wall_step -= dt;
                if f.wall_step <= 0.0 {
                    f.wall_step = 0.12;
                    out = Some(Sfx::WallRun);
                }
            } else {
                f.wall_step = 0.0;
            }
            f.was_ground = f.on_ground;
            let p = f.pos;
            if let Some(mut s) = out {
                if s == Sfx::StepStone && matches!(self.world.ground_mat(p + v3(0.0, 0.02, 0.0)), Mat::Crate | Mat::Trim) {
                    s = Sfx::StepWood;
                }
                self.sfx(s, p);
            }
        }
    }

    /// A sound that isn't in the world (hit markers, countdown, jingles).
    fn sfx2d(&mut self, s: Sfx) {
        self.sounds.push((s, None, 1.0));
    }

    pub fn say(&mut self, s: String) {
        self.feed.push(FeedLine { killer: None, victim: (String::new(), 0), tool: "", head: false, text: Some(s), life: 5.0 });
        if self.feed.len() > 6 {
            self.feed.remove(0);
        }
    }

    /// Advance one fixed tick. `local_input` drives the local human fighter.
    pub fn tick(&mut self, local_input: &Input) {
        let dt = TICK;
        self.time += dt;
        let frozen = self.frozen();
        for ff in self.f.iter_mut() {
            ff.prev_pos = ff.pos;
            ff.flash_t = (ff.flash_t - dt).max(0.0);
        }
        self.parry_flash = (self.parry_flash - dt).max(0.0);
        self.heal_pop = (self.heal_pop - dt).max(0.0);
        // pose everyone: shots this tick hit the shapes you see
        self.update_hit_poses();

        for i in 0..self.f.len() {
            if !self.f[i].owned || !self.f[i].present {
                continue;
            }
            self.f[i].spawn_prot = (self.f[i].spawn_prot - dt).max(0.0);
            // respawning
            if !self.f[i].alive {
                self.f[i].respawn_t -= dt;
                if self.f[i].respawn_t <= 0.0 && self.mt.phase != Phase::MatchOver && !self.f[i].spectate {
                    let (p, y) = self.pick_spawn(i);
                    if i == self.local && !self.f[i].is_bot {
                        self.f[i].loadout = self.my_loadout;
                    }
                    self.f[i].respawn_at(p, y);
                    if self.f[i].is_bot {
                        if let Some(b) = self.bots[i].as_mut() {
                            b.reset();
                        }
                    }
                }
            }
            let input = if self.f[i].is_bot {
                let mut bot = self.bots[i].take();
                let inp = match bot.as_mut() {
                    Some(b) => b.think(self, i, dt),
                    None => Input { yaw: self.f[i].yaw, ..Default::default() },
                };
                self.bots[i] = bot;
                inp
            } else {
                *local_input
            };
            let acts = self.step_fighter(i, &input, dt, frozen);
            for a in acts {
                self.do_action(i, a);
            }
            // slash hit timing
            if self.f[i].slash_t > 0.0 && !self.f[i].slash_hit_done && self.f[i].slash_t < self.f[i].weapon.slash_dur() * 0.72 {
                self.f[i].slash_hit_done = true;
                self.resolve_slash(i);
            }
        }

        if self.train.is_some() {
            self.update_training(dt);
        }

        // deaths are decided here only on the host (or a game on this computer)
        if self.is_authority() {
            let rounds = self.mt.mode.rounds() && self.mt.phase == Phase::Fight;
            for i in 0..self.f.len() {
                let ff = &mut self.f[i];
                if !ff.present {
                    continue;
                }
                if !ff.owned && !ff.alive {
                    ff.respawn_t -= dt;
                }
                if ff.alive && ff.hp <= 0.0 {
                    ff.alive = false;
                    ff.death_t = 0.0;
                    ff.wall = None;
                    ff.respawn_t = RESPAWN_TIME;
                    if rounds {
                        // no coming back until the next round
                        ff.respawn_t = 1e9;
                    }
                    let killer = ff.last_attacker;
                    let w = ff.last_weapon;
                    let head = ff.last_head;
                    self.on_death(i, killer, w, head);
                    if self.kind == Kind::Host {
                        self.out_events.push(Ev::Died { victim: i as u8, killer: killer.map_or(255, |k| k as u8), w, head });
                    }
                }
            }
            if self.mt.mode == Mode::Ctf && self.scoring() {
                self.update_ctf(dt);
            }
            for ff in self.f.iter_mut() {
                if ff.regen_left > 0.0 {
                    if ff.alive && ff.present {
                        let h = (24.0 * dt).min(ff.regen_left);
                        ff.hp = (ff.hp + h).min(MAX_HP);
                        ff.regen_left -= h;
                        if ff.hp >= MAX_HP {
                            ff.regen_left = 0.0;
                        }
                    } else {
                        ff.regen_left = 0.0;
                    }
                }
            }
            // remember where everyone was (and how they stood), to check hits against the past
            let now = self.time;
            for i in 0..self.f.len() {
                if self.f[i].present {
                    let p = self.f[i].pos;
                    let h = &mut self.history[i];
                    h.push_back((now, p));
                    while h.front().map_or(false, |e| now - e.0 > 1.2) {
                        h.pop_front();
                    }
                    let ph = &mut self.pose_history[i];
                    if let Some(hp) = self.hit_poses[i] {
                        ph.push_back((now, hp));
                    }
                    while ph.front().map_or(false, |e| now - e.0 > 1.2) {
                        ph.pop_front();
                    }
                }
            }
        }

        // remote fighters: played back a moment behind from their recent updates
        let now = self.time;
        self.hp_hold = (self.hp_hold - dt).max(0.0);
        for ff in self.f.iter_mut() {
            if ff.owned || !ff.present {
                continue;
            }
            ff.net_age += dt;
            if let Some((p, yaw, pitch, slash)) = playback(ff, now) {
                let err = p - ff.pos;
                ff.pos = if err.len() > 4.0 { p } else { ff.pos + err * 0.6 };
                ff.yaw = yaw;
                ff.pitch = pitch;
                if let Some((st, si)) = slash {
                    ff.slash_t = st;
                    ff.slash_idx = si;
                }
            } else {
                let predicted = ff.net_pos + ff.vel * ff.net_age.min(0.15);
                ff.pos = ff.pos.lerp(predicted, 0.25);
                if (ff.pos - predicted).len() > 4.0 {
                    ff.pos = predicted;
                }
                if ff.slash_t > 0.0 {
                    ff.slash_t -= dt;
                }
            }
            if !ff.alive {
                ff.death_t += dt;
            }
            let sp = ff.vel.flat().len();
            if ff.on_ground && sp > 0.5 {
                ff.walk_phase += sp * dt * 1.6;
            }
            if ff.hurt_t > 0.0 {
                ff.hurt_t -= dt;
            }
            ff.guard_t = if ff.guard { ff.guard_t + dt } else { 0.0 };
        }

        self.footsteps(dt);
        self.update_grenades(dt);
        self.update_fx(dt);
        for c in self.chat.iter_mut() {
            c.life -= dt;
        }
        self.chat.retain(|c| c.life > 0.0);
        self.refresh_colors();
        if self.is_authority() {
            self.update_match(dt);
        } else {
            self.mt.timer = (self.mt.timer - dt).max(0.0);
        }
        if let Some((_, t)) = &mut self.banner {
            *t -= dt;
            if *t <= 0.0 {
                self.banner = None;
            }
        }
    }

    /// Someone died (on this computer, or we were told). Feed line, and the score on the authority.
    fn on_death(&mut self, victim: usize, killer: Option<usize>, w: u8, head: bool) {
        let vname = (self.f[victim].name.clone(), self.f[victim].color_idx);
        let k = killer.filter(|&k| k != victim && k < self.f.len() && self.f[k].present);
        let kname = k.map(|k| (self.f[k].name.clone(), self.f[k].color_idx));
        self.feed.push(FeedLine { killer: kname, victim: vname, tool: kill_tool(w), head, text: None, life: 6.0 });
        if self.feed.len() > 6 {
            self.feed.remove(0);
        }
        self.sfx(Sfx::Hurt, self.f[victim].pos);
        let p = self.f[victim].chest();
        self.burst(p, [200, 30, 40], 26, 6.0);
        self.last_kill = Some((victim, k, w, head));
        if k == Some(self.local) && victim != self.local && self.is_enemy(self.local, victim) {
            self.streak = if self.time - self.last_kill_time < 4.0 { self.streak + 1 } else { 1 };
            self.last_kill_time = self.time;
            self.kill_t = 1.6;
            self.heal_pop = 1.6;
            self.hitmarker = self.hitmarker.max(0.35); // the hit confirm turns red
            self.kill_name = self.f[victim].name.clone();
            self.sfx2d(Sfx::Kill);
        }
        if self.is_authority() && self.scoring() {
            self.f[victim].deaths += 1;
            if let Some(k) = k {
                if self.is_enemy(k, victim) {
                    self.f[k].kills += 1;
                    // a kill gives health back over a moment
                    self.f[k].regen_left = (self.f[k].regen_left + KILL_HEAL).min(MAX_HP);
                    if self.mt.mode == Mode::Tdm {
                        let t = (self.f[k].team as usize).min(1);
                        self.mt.team_score[t] += 1;
                    }
                } else {
                    self.f[k].kills -= 1; // team kill
                }
            }
            self.check_win();
        }
    }

    fn check_win(&mut self) {
        let t = self.mt.target as i32;
        let won = match self.mt.mode {
            Mode::Ffa | Mode::Gladiator => self.f.iter().any(|f| f.present && f.kills >= t),
            Mode::Tdm | Mode::Ctf => self.mt.team_score.iter().any(|&s| s as i32 >= t),
            Mode::Duel | Mode::Lms => self.f.iter().any(|f| f.present && f.wins >= t),
        };
        if won {
            self.end_match();
        }
    }

    /// Who's winning, in words.
    pub fn leader_text(&self) -> String {
        match self.mt.mode {
            Mode::Tdm | Mode::Ctf => {
                let [r, b] = self.mt.team_score;
                if r > b {
                    "RED TEAM WINS".into()
                } else if b > r {
                    "BLUE TEAM WINS".into()
                } else {
                    "DRAW".into()
                }
            }
            _ => {
                let rounds = self.mt.mode.rounds();
                let score = |f: &Fighter| if rounds { f.wins } else { f.kills };
                let mut best: Option<&Fighter> = None;
                for f in self.f.iter().filter(|f| f.present) {
                    if best.map_or(true, |b| score(f) > score(b)) {
                        best = Some(f);
                    }
                }
                match best {
                    Some(b) => format!("{} WINS", b.name.to_uppercase()),
                    None => "MATCH OVER".into(),
                }
            }
        }
    }

    /// Start the slow-motion final kill cam (when the match ends with a kill).
    pub fn start_killcam(&mut self) {
        if self.kind == Kind::Demo {
            return;
        }
        if let Some((victim, killer, w, head)) = self.last_kill {
            self.killcam = Some(KillCam { victim, killer, tool: kill_tool(w), head, t: 0.0 });
            self.slowmo = KILLCAM_TIME;
        }
    }

    fn end_match(&mut self) {
        self.start_killcam();
        self.mt.phase = Phase::MatchOver;
        self.mt.timer = if self.kind == Kind::Demo { 5.0 } else { 9999.0 };
        self.banner = Some((self.leader_text(), 9999.0));
        if self.kind != Kind::Demo {
            self.sfx2d(Sfx::MatchOver);
        }
    }

    fn update_match(&mut self, dt: f32) {
        match self.mt.phase {
            Phase::Warmup => {}
            Phase::Loadout => {
                self.mt.timer -= dt;
                let humans_ready = self.f.iter().filter(|f| f.present).all(|f| f.ready);
                if self.mt.timer <= 0.0 || humans_ready {
                    self.mt.phase = Phase::Countdown;
                    self.mt.timer = 3.0;
                    self.banner = Some(("GET READY".into(), 2.0));
                }
            }
            Phase::Countdown => {
                let before = self.mt.timer.ceil();
                self.mt.timer -= dt;
                if self.mt.timer.ceil() < before && self.mt.timer > 0.0 && self.kind != Kind::Demo {
                    self.sfx2d(Sfx::Tick); // 3, 2, 1
                }
                if self.mt.timer <= 0.0 {
                    self.mt.phase = Phase::Fight;
                    self.mt.timer = MATCH_TIME;
                    self.banner = Some(("FIGHT!".into(), 1.0));
                    if self.kind != Kind::Demo {
                        self.sfx2d(Sfx::MatchStart);
                    }
                    if self.mt.mode.rounds() {
                        self.start_round();
                    }
                }
            }
            Phase::Fight if self.mt.mode.rounds() => self.update_rounds(dt),
            Phase::Fight => {
                self.mt.timer -= dt;
                if self.mt.timer <= 0.0 {
                    self.end_match();
                }
            }
            Phase::MatchOver => {
                self.mt.timer -= dt;
                if self.kind == Kind::Demo && self.mt.timer <= 0.0 {
                    self.start_match();
                    self.mt.phase = Phase::Countdown;
                    self.mt.timer = 1.0;
                }
                // rematch vote: more than half of the people (bots don't vote)
                let humans: Vec<&Fighter> = self.f.iter().filter(|f| f.present && !f.is_bot).collect();
                let votes = humans.iter().filter(|f| f.vote).count();
                if !humans.is_empty() && votes * 2 > humans.len() {
                    self.start_match();
                }
            }
        }
    }

    /// Round modes: who's still standing, round over, next round.
    fn update_rounds(&mut self, dt: f32) {
        if self.mt.round_end > 0.0 {
            self.mt.round_end -= dt;
            if self.mt.round_end <= 0.0 {
                self.start_round();
            }
            return;
        }
        self.mt.timer -= dt;
        let parts = self.participants();
        let alive: Vec<usize> = parts.iter().copied().filter(|&i| self.f[i].alive).collect();
        let waiting = self.f.iter().any(|f| f.present && f.spectate);
        let enough = parts.len() >= 2;
        if !enough {
            // someone left (or a seat is free): bring in whoever is waiting
            if waiting {
                self.start_round();
            }
            return;
        }
        if alive.len() <= 1 || self.mt.timer <= 0.0 {
            let winner = if alive.len() == 1 { Some(alive[0]) } else { None };
            self.finish_round(winner);
        }
    }

    fn finish_round(&mut self, winner: Option<usize>) {
        if self.mt.mode == Mode::Duel {
            self.duel_loser = self.mt.duel.iter().map(|&d| d as usize).find(|&d| d < self.f.len() && Some(d) != winner);
        }
        let text = match winner {
            Some(w) => {
                self.f[w].wins += 1;
                format!("{} WINS THE ROUND", self.f[w].name.to_uppercase())
            }
            None => "DRAW".to_string(),
        };
        self.banner = Some((text.clone(), 2.5));
        self.say(text.to_lowercase());
        self.mt.round_end = 3.0;
        self.check_win();
    }

    /// Next round: everyone taking part respawns; in a duel the loser goes to the back of the line.
    pub fn start_round(&mut self) {
        self.mt.round += 1;
        self.mt.round_end = 0.0;
        self.mt.timer = ROUND_TIME;
        self.grenades.clear();
        if self.mt.mode == Mode::Duel {
            for &s in &self.present() {
                if !self.queue.contains(&s) && self.duel_side(s).is_none() {
                    self.queue.push_back(s);
                }
            }
            if let Some(l) = self.duel_loser.take() {
                for d in self.mt.duel.iter_mut() {
                    if *d as usize == l {
                        *d = 255;
                    }
                }
                if self.f[l].present && !self.queue.contains(&l) {
                    self.queue.push_back(l);
                }
            }
            for k in 0..2 {
                let gone = self.mt.duel[k] == 255 || !self.f[self.mt.duel[k] as usize].present;
                if gone {
                    self.mt.duel[k] = 255;
                    while let Some(q) = self.queue.pop_front() {
                        if self.f[q].present && self.duel_side(q).is_none() {
                            self.mt.duel[k] = q as u8;
                            break;
                        }
                    }
                }
            }
        }
        for i in 0..self.f.len() {
            if !self.f[i].present {
                continue;
            }
            let plays = self.mt.mode != Mode::Duel || self.duel_side(i).is_some();
            if plays {
                self.f[i].spectate = false;
                if self.f[i].owned {
                    let (p, y) = self.pick_spawn(i);
                    if i == self.local && !self.f[i].is_bot {
                        self.f[i].loadout = self.my_loadout;
                    }
                    self.f[i].respawn_at(p, y);
                } else {
                    // a player on another computer: their game respawns them for the new round
                    self.f[i].respawn_t = 0.0;
                    self.f[i].hp = MAX_HP;
                    self.f[i].ap = MAX_AP;
                }
            } else {
                let f = &mut self.f[i];
                f.spectate = true;
                f.alive = false;
                f.respawn_t = 1e9;
            }
        }
        self.banner = Some((format!("ROUND {}", self.mt.round), 1.5));
        self.sfx2d(Sfx::Tick);
    }

    /// Capture the Flag: pick-ups, returns, drops and captures.
    fn update_ctf(&mut self, dt: f32) {
        for t in 0..2usize {
            let mut fl = self.mt.flags[t];
            let tname = TEAM_NAMES[t];
            match fl.state {
                1 => {
                    let c = fl.carrier as usize;
                    if c >= self.f.len() || !self.f[c].present || !self.f[c].alive || self.f[c].spectate {
                        fl.state = 2;
                        fl.timer = 20.0;
                        fl.pos = if c < self.f.len() { self.f[c].pos } else { flag_base(t) };
                        fl.carrier = 255;
                        if c < self.f.len() {
                            let n = self.f[c].name.clone();
                            self.say(format!("{n} dropped the {tname} flag"));
                        }
                    } else {
                        fl.pos = self.f[c].pos;
                        let home = (self.f[c].team as usize).min(1);
                        let near_home = (self.f[c].pos - flag_base(home)).flat().len() < 1.8;
                        if near_home && self.mt.flags[home].state == 0 {
                            self.mt.team_score[home] += 1;
                            let n = self.f[c].name.clone();
                            self.say(format!("{n} captured the {tname} flag!"));
                            self.banner = Some((format!("{} TEAM SCORES", TEAM_NAMES[home].to_uppercase()), 2.0));
                            fl = Flag { state: 0, carrier: 255, pos: flag_base(t), timer: 0.0 };
                            self.sfx2d(Sfx::Kill);
                            self.mt.flags[t] = fl;
                            self.check_win();
                            continue;
                        }
                    }
                }
                _ => {
                    if fl.state == 2 {
                        fl.timer -= dt;
                        if fl.timer <= 0.0 {
                            fl = Flag { state: 0, carrier: 255, pos: flag_base(t), timer: 0.0 };
                            self.say(format!("The {tname} flag went home"));
                        }
                    }
                    for i in 0..self.f.len() {
                        let f = &self.f[i];
                        if !f.present || !f.alive || f.spectate {
                            continue;
                        }
                        let d = f.pos - fl.pos;
                        if d.flat().len() > 1.3 || d.y.abs() > 2.2 {
                            continue;
                        }
                        let name = f.name.clone();
                        if (f.team as usize).min(1) == t {
                            if fl.state == 2 {
                                fl = Flag { state: 0, carrier: 255, pos: flag_base(t), timer: 0.0 };
                                self.say(format!("{name} returned the {tname} flag"));
                            }
                        } else {
                            fl.state = 1;
                            fl.carrier = i as u8;
                            self.say(format!("{name} took the {tname} flag"));
                            break;
                        }
                    }
                }
            }
            self.mt.flags[t] = fl;
        }
    }

    /// Start (or restart) the match: scores reset and everyone respawns.
    pub fn start_match(&mut self) {
        self.mt.number += 1;
        self.killcam = None;
        self.slowmo = 0.0;
        self.last_kill = None;
        self.mt.phase = Phase::Loadout;
        self.mt.timer = if self.kind == Kind::Offline { 120.0 } else { 20.0 };
        self.mt.team_score = [0, 0];
        self.mt.round = 0;
        self.mt.round_end = 0.0;
        self.mt.duel = [255, 255];
        self.mt.flags = [0, 1].map(|t| Flag { state: 0, carrier: 255, pos: flag_base(t), timer: 0.0 });
        self.queue = self.present().into_iter().collect();
        self.duel_loser = None;
        self.banner = Some(("CHOOSE YOUR LOADOUT".into(), 2.5));
        self.grenades.clear();
        for i in 0..self.f.len() {
            if self.f[i].present {
                self.f[i].spectate = false;
                self.f[i].wins = 0;
                self.f[i].kills = 0;
                self.f[i].deaths = 0;
                self.f[i].vote = false;
                self.f[i].ready = self.f[i].is_bot;
                if i == self.local && !self.f[i].is_bot {
                    self.f[i].loadout = self.my_loadout;
                }
                if self.f[i].owned {
                    let (p, y) = self.pick_spawn(i);
                    self.f[i].respawn_at(p, y);
                }
            }
        }
    }

    /// Client side: follow the host's match state.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_match(&mut self, mode: Mode, phase: Phase, timer: f32, target: u16, team_score: [u16; 2], number: u32, round: u32, round_end: f32, duel: [u8; 2], flags: [Flag; 2]) {
        self.mt.mode = mode;
        self.mt.duel = duel;
        self.mt.flags = flags;
        self.mt.round_end = round_end;
        if round != self.mt.round {
            self.mt.round = round;
            let l = self.local;
            if round > 0 && self.f[l].present && !self.f[l].spectate && phase == Phase::Fight {
                // new round: back in at your starting spot
                self.mt.phase = phase;
                let (p, y) = self.pick_spawn(l);
                if !self.f[l].is_bot {
                    self.f[l].loadout = self.my_loadout;
                }
                self.f[l].respawn_at(p, y);
                self.grenades.clear();
                self.banner = Some((format!("ROUND {round}"), 1.5));
            }
        }
        if number != self.mt.number {
            self.mt.number = number;
            self.killcam = None;
            self.slowmo = 0.0;
            self.last_kill = None;
            self.grenades.clear();
            let l = self.local;
            if self.f[l].present {
                let (p, y) = self.pick_spawn(l);
                if !self.f[l].is_bot {
                    self.f[l].loadout = self.my_loadout;
                }
                self.f[l].ready = self.f[l].is_bot;
                self.f[l].vote = false;
                self.f[l].respawn_at(p, y);
                if self.f[l].is_bot && self.kind == Kind::Client {
                    self.out_events.push(Ev::Ready);
                }
            }
        }
        let was = self.mt.phase;
        self.mt.phase = phase;
        self.mt.timer = timer;
        self.mt.target = target;
        self.mt.team_score = team_score;
        if phase != was {
            match phase {
                Phase::Fight => self.banner = Some(("FIGHT!".into(), 1.0)),
                Phase::Loadout => self.banner = Some(("CHOOSE YOUR LOADOUT".into(), 2.5)),
                Phase::Countdown => self.banner = Some(("GET READY".into(), 2.0)),
                Phase::MatchOver => {
                    self.banner = Some((self.leader_text(), 9999.0));
                    self.start_killcam();
                }
                Phase::Warmup => self.banner = Some(("WARM-UP".into(), 2.0)),
            }
        }
    }

    // ------------------------------------------------------------------ movement

    fn touching_wall(&self, pos: V3) -> Option<V3> {
        let mut best: Option<(f32, V3)> = None;
        let reach = RADIUS + 0.28;
        for bl in &self.world.blocks {
            let b = &bl.b;
            if b.max.y < pos.y + 0.4 || b.min.y > pos.y + 1.4 {
                continue;
            }
            let cx = pos.x.clamp(b.min.x, b.max.x);
            let cz = pos.z.clamp(b.min.z, b.max.z);
            let d = v3(pos.x - cx, 0.0, pos.z - cz);
            let l = d.len();
            if l < reach && l > 1e-4 && best.map_or(true, |(bl, _)| l < bl) {
                // prefer axis normals so wall runs follow faces cleanly
                let n = if d.x.abs() > d.z.abs() { v3(d.x.signum(), 0.0, 0.0) } else { v3(0.0, 0.0, d.z.signum()) };
                best = Some((l, n));
            }
        }
        best.map(|(_, n)| n)
    }

    fn collide(&self, ff: &mut Fighter, dt: f32) {
        let was_ground = ff.on_ground;
        // horizontal axes
        for axis in 0..2 {
            let delta = if axis == 0 { ff.vel.x * dt } else { ff.vel.z * dt };
            if axis == 0 {
                ff.pos.x += delta;
            } else {
                ff.pos.z += delta;
            }
            for bl in &self.world.blocks {
                let b = &bl.b;
                let pb = Aabb::new(ff.pos + v3(-RADIUS, 0.0, -RADIUS), ff.pos + v3(RADIUS, HEIGHT, RADIUS));
                if !pb.overlaps(b) {
                    continue;
                }
                let step = b.max.y - ff.pos.y;
                if was_ground && step > 0.0 && step <= 0.55 {
                    // check head room
                    let up = Aabb::new(v3(pb.min.x, b.max.y + 0.01, pb.min.z), v3(pb.max.x, b.max.y + HEIGHT, pb.max.z));
                    if !self.world.blocks.iter().any(|o| o.b.overlaps(&up)) {
                        ff.pos.y = b.max.y;
                        continue;
                    }
                }
                if axis == 0 {
                    let c = (b.min.x + b.max.x) * 0.5;
                    ff.pos.x = if ff.pos.x < c { b.min.x - RADIUS - 0.001 } else { b.max.x + RADIUS + 0.001 };
                    ff.vel.x = 0.0;
                } else {
                    let c = (b.min.z + b.max.z) * 0.5;
                    ff.pos.z = if ff.pos.z < c { b.min.z - RADIUS - 0.001 } else { b.max.z + RADIUS + 0.001 };
                    ff.vel.z = 0.0;
                }
            }
        }
        // vertical
        ff.pos.y += ff.vel.y * dt;
        ff.on_ground = false;
        for bl in &self.world.blocks {
            let b = &bl.b;
            let pb = Aabb::new(ff.pos + v3(-RADIUS, 0.0, -RADIUS), ff.pos + v3(RADIUS, HEIGHT, RADIUS));
            if !pb.overlaps(b) {
                continue;
            }
            let c = (b.min.y + b.max.y) * 0.5;
            if ff.pos.y + HEIGHT * 0.5 > c || ff.vel.y <= 0.0 {
                ff.pos.y = b.max.y;
                if ff.vel.y <= 0.0 {
                    ff.vel.y = 0.0;
                    ff.on_ground = true;
                }
            } else {
                ff.pos.y = b.min.y - HEIGHT - 0.001;
                ff.vel.y = ff.vel.y.min(0.0);
            }
        }
        // standing check (resting exactly on a surface)
        if !ff.on_ground && ff.vel.y <= 0.0 {
            let feet = Aabb::new(ff.pos + v3(-RADIUS + 0.02, -0.03, -RADIUS + 0.02), ff.pos + v3(RADIUS - 0.02, 0.0, RADIUS - 0.02));
            if self.world.blocks.iter().any(|o| o.b.overlaps(&feet)) {
                ff.on_ground = true;
                ff.vel.y = 0.0;
            }
        }
        let h = self.world.half - RADIUS;
        ff.pos.x = ff.pos.x.clamp(-h, h);
        ff.pos.z = ff.pos.z.clamp(-h, h);
        if ff.pos.y < -5.0 {
            ff.pos = self.world.spawn_points[ff.slot % self.world.spawn_points.len()].0;
            ff.vel = V3::ZERO;
        }
    }

    fn step_fighter(&mut self, i: usize, inp: &Input, dt: f32, frozen: bool) -> Vec<Act> {
        let mut acts = Vec::new();
        let mut ff = self.f[i].clone();
        let mut sounds: Vec<Sfx> = Vec::new();

        ff.fire_cd = (ff.fire_cd - dt).max(0.0);
        ff.switch_t = (ff.switch_t - dt).max(0.0);
        ff.dash_cd = (ff.dash_cd - dt).max(0.0);
        ff.item_cd = (ff.item_cd - dt).max(0.0);
        ff.hurt_t = (ff.hurt_t - dt).max(0.0);
        ff.stun_t = (ff.stun_t - dt).max(0.0);
        ff.combo_window = (ff.combo_window - dt).max(0.0);
        ff.recoil = (ff.recoil - dt * 8.0).max(0.0);
        ff.since_shot += dt;
        if !ff.weapon.is_melee() {
            // moving costs a little accuracy, eased in and out so the crosshair moves smoothly
            let cap = ff.weapon.gun().move_cap;
            let speed_k = if ff.on_ground && ff.wall.is_none() { ((ff.vel.flat().len() - 1.0) / (RUN_SPEED - 1.0)).clamp(0.0, 1.0) } else { 1.0 };
            let want = 1.0 + (cap - 1.0) * speed_k;
            let rate = if want > ff.move_spread { 10.0 } else { 7.0 };
            ff.move_spread += (want - ff.move_spread) * (dt * rate).min(1.0);
            ff.move_spread = ff.move_spread.clamp(1.0, cap.max(1.0));
        }
        ff.jump_buf = (ff.jump_buf - dt).max(0.0);
        ff.coyote = (ff.coyote - dt).max(0.0);
        ff.dash_buf = (ff.dash_buf - dt).max(0.0);
        ff.ground_dash_t = (ff.ground_dash_t - dt).max(0.0);
        ff.bfly_t = (ff.bfly_t - dt).max(0.0);
        ff.float_t = (ff.float_t - dt).max(0.0);
        ff.land_t += dt;
        ff.flow_pop = (ff.flow_pop - dt).max(0.0);
        ff.perfect_t = (ff.perfect_t - dt).max(0.0);
        // carried speed: held for a moment after landing, then eased off
        if ff.carry_t > 0.0 {
            ff.carry_t -= dt;
        } else if ff.carry > 0.0 {
            ff.carry -= 14.0 * dt;
            if ff.carry <= RUN_SPEED * 0.5 {
                ff.carry = 0.0;
            }
        }
        if inp.jump {
            ff.jump_buf = 0.13;
        }
        if inp.dash {
            ff.dash_buf = 0.15;
        }
        if ff.on_ground {
            ff.coyote = 0.1;
        }

        if !ff.alive {
            ff.death_t += dt;
            ff.vel.x *= 1.0 - (4.0 * dt).min(1.0);
            ff.vel.z *= 1.0 - (4.0 * dt).min(1.0);
            ff.vel.y -= GRAVITY * dt;
            self.collide(&mut ff, dt);
            self.f[i] = ff;
            return acts;
        }

        ff.yaw = wrap_angle(inp.yaw);
        ff.pitch = inp.pitch.clamp(-1.45, 1.45);
        let fwd = yaw_fwd(ff.yaw);
        let right = yaw_right(ff.yaw);
        let can_act = !frozen && ff.stun_t <= 0.0;
        let mut wish = fwd * inp.fwd + right * inp.right;
        if wish.len() > 1.0 {
            wish = wish.norm();
        }
        if !can_act {
            wish = V3::ZERO;
        }
        let flow_k = ff.flow_mult();
        let grounded_before = ff.on_ground;
        // jumping or dashing right as you land: a little extra
        let perfect_window = ff.on_ground && ff.land_t < PERFECT_WINDOW && ff.last_air > 0.25;

        // weapon switching
        if let Some(s) = inp.select {
            let s = (s as usize).min(2);
            if s != ff.cur && can_act {
                ff.cur = s;
                ff.weapon = ff.slot_weapon(s);
                ff.switch_t = 0.22;
                sounds.push(if ff.weapon.is_melee() { Sfx::SwitchBlade } else { Sfx::SwitchGun });
                ff.reload_t = 0.0;
                ff.slash_t = 0.0;
                ff.slash_queued = false;
            }
        }
        // reloading
        if self.mt.mode.melee_only() && ff.cur != 0 {
            ff.cur = 0; // Gladiator: blades only
            ff.reload_t = 0.0;
        }
        let wi = ff.cur.min(2);
        ff.weapon = ff.slot_weapon(wi);
        let melee = ff.weapon.is_melee();
        let dur = ff.weapon.slash_dur();
        if ff.reload_t > 0.0 {
            ff.reload_t -= dt;
            if ff.reload_t <= 0.0 {
                ff.reload_t = 0.0;
                ff.mag[wi] = ff.weapon.mag();
                sounds.push(Sfx::ReloadEnd);
            }
        } else if inp.reload && !melee && ff.mag[wi] < ff.weapon.mag() && can_act {
            ff.reload_t = ff.weapon.reload_time();
            sounds.push(Sfx::Reload);
        }

        // K-style cancels: once a slash has landed (its hit frame passed), its recovery can be
        // cut short by a guard, a dash or a jump. Before that, those inputs wait in a buffer.
        let slash_locked = ff.slashing() && !ff.slash_hit_done;
        let can_cancel = ff.slashing() && ff.slash_hit_done;
        let cancel_slash = |ff: &mut Fighter| {
            if ff.slash_t > 0.0 {
                ff.slash_t = 0.0;
                ff.slash_queued = false;
                ff.combo_window = 0.45;
            }
        };
        if can_act && melee && inp.guard && can_cancel {
            self.note(i, |s| s.cancels += 1);
            ff.chain();
            cancel_slash(&mut ff);
            ff.bfly_t = 0.4; // butterfly: guard-cancel, then slash again
        }
        ff.guard = can_act && melee && inp.guard && !ff.slashing() && ff.switch_t <= 0.0;
        ff.guard_t = if ff.guard { ff.guard_t + dt } else { 0.0 };
        ff.stagger_t = (ff.stagger_t - dt).max(0.0);
        let scoping = can_act && !melee && inp.guard && ff.weapon.zoom() > 1.0 && ff.reload_t <= 0.0 && ff.switch_t <= 0.0;
        if scoping && ff.scope_t <= 0.0 && ff.weapon == Weapon::Sniper {
            sounds.push(Sfx::ScopeIn);
        }
        ff.scope_t = if scoping { ff.scope_t + dt } else { 0.0 };

        // dash / tumble
        if ff.dash_buf > 0.0 && can_act && ff.dash_cd <= 0.0 && ff.wall.is_none() && !slash_locked {
            let dir = if wish.len() > 0.1 { wish.norm() } else { fwd };
            let mut dashed = false;
            if ff.on_ground {
                let boost = if perfect_window { 1.12 } else { 1.0 };
                ff.vel = dir * (18.5 * flow_k * boost) + v3(0.0, 2.2, 0.0);
                ff.on_ground = false;
                ff.dash_t = 0.2;
                ff.dash_cd = 0.38;
                ff.ground_dash_t = 0.32; // a slash now becomes a rising launcher
                ff.sliding = 0.0;
                if perfect_window {
                    ff.perfect_t = 0.6;
                    sounds.push(Sfx::Perfect);
                    self.note(i, |s| s.perfects += 1);
                }
                dashed = true;
            } else if !ff.air_dash {
                ff.vel = dir * (15.0 * flow_k) + v3(0.0, 3.5, 0.0);
                ff.air_dash = true;
                ff.dash_t = 0.18;
                ff.dash_cd = 0.32;
                dashed = true;
            }
            if dashed {
                ff.from_move = true;
                ff.chain();
                let (air, cancel) = (ff.ground_dash_t < 0.3, ff.slash_t > 0.0);
                self.note(i, |s| {
                    if air { s.air_dashes += 1 } else { s.dashes += 1 }
                    if cancel {
                        s.cancels += 1
                    }
                });
                ff.dash_buf = 0.0;
                cancel_slash(&mut ff);
                sounds.push(Sfx::Dash);
            }
        }

        let mut speed = RUN_SPEED * flow_k;
        if ff.guard {
            speed = 4.5;
        }
        if ff.slashing() {
            speed = 5.5;
        }

        let mut gravity = true;
        if let Some(mut w) = ff.wall {
            w.t += dt;
            let touching = self.touching_wall(ff.pos).map_or(false, |n| n.dot(w.normal) > 0.7);
            let max_t = if w.vertical { 0.55 } else { 1.15 };
            if inp.jump && can_act {
                // wall jump off the wall run: away from the wall, steered by where you look
                let look_along = (fwd - w.normal * fwd.dot(w.normal)).flat();
                let along = if w.vertical { w.normal * 2.5 } else { w.dir * 4.5 };
                ff.vel = (w.normal * 7.5 + along + look_along * 3.0) * flow_k + v3(0.0, 8.8, 0.0);
                self.note(i, |s| s.walljumps += 1);
                ff.from_move = true;
                ff.chain();
                ff.wall = None;
                ff.air_dash = false;
                ff.air_slashes = 0;
                ff.wallruns_left = ff.wallruns_left.max(1);
                ff.jump_buf = 0.0;
                cancel_slash(&mut ff);
                sounds.push(Sfx::Jump);
            } else if ff.on_ground || !can_act {
                ff.wall = None;
            } else if !touching {
                ff.wall = None;
                if w.vertical {
                    // climbed over the top: hop onto the ledge
                    ff.vel = v3(0.0, 6.5, 0.0) - w.normal * 3.5;
                }
            } else if w.t > max_t {
                ff.wall = None;
                if w.vertical {
                    ff.vel = w.normal * 2.5 + v3(0.0, 1.0, 0.0);
                }
            } else {
                gravity = false;
                if w.vertical {
                    ff.vel = v3(0.0, 8.0, 0.0) - w.normal * 1.0;
                } else {
                    ff.vel = w.dir * w.speed + v3(0.0, 1.8 - w.t * 3.5, 0.0) - w.normal * 1.0;
                }
                ff.wall = Some(w);
            }
        } else if ff.dash_t > 0.0 {
            ff.dash_t -= dt;
            gravity = !ff.air_dash || ff.dash_t <= 0.0;
            if ff.on_ground && ff.dash_t <= 0.0 {
                // out of a ground dash: keep the speed a moment, then ease off
                let flat = ff.vel.flat().len();
                ff.carry = flat.min(RUN_SPEED * 1.7 * flow_k);
                ff.carry_t = MOMENTUM_HOLD;
            }
        } else if ff.on_ground {
            let cur = ff.vel.flat();
            // start a slide: crouch while running fast
            if inp.crouch && ff.sliding <= 0.0 && can_act && !ff.slashing() && cur.len() >= SLIDE_MIN_SPEED {
                ff.sliding = SLIDE_TIME;
                let s = (cur.len() + 1.5).max(10.5);
                let d = cur.norm();
                ff.vel.x = d.x * s;
                ff.vel.z = d.z * s;
                ff.chain();
                self.note(i, |s| s.slides += 1);
                sounds.push(Sfx::Slide);
            }
            if ff.sliding > 0.0 {
                // sliding: keep the momentum, steer a little, ease to a stop
                ff.sliding -= dt;
                let cur = ff.vel.flat();
                let sp = cur.len();
                let mut d = if sp > 0.01 { cur * (1.0 / sp) } else { fwd };
                if wish.len() > 0.1 {
                    d = (d + wish.norm() * (2.0 * dt)).norm();
                }
                let sp = (sp - 3.0 * dt).max(0.0);
                ff.vel.x = d.x * sp;
                ff.vel.z = d.z * sp;
                if !inp.crouch || ff.sliding <= 0.0 || sp < 4.0 || !can_act {
                    ff.sliding = 0.0;
                    ff.carry = sp.min(RUN_SPEED * 1.7 * flow_k);
                    ff.carry_t = 0.15;
                }
            } else {
                // run, holding on to carried speed (it eases off rather than snapping back)
                let cap = speed.max(ff.carry);
                let target = wish * cap;
                let diff = target - cur;
                let carrying = ff.carry > speed;
                let slowing = target.len() + 0.01 < cur.len();
                let acc = if wish.len() > 0.1 {
                    if carrying && slowing { 30.0 } else { 120.0 }
                } else if carrying {
                    25.0
                } else {
                    80.0
                } * dt;
                let step = if diff.len() > acc { diff.norm() * acc } else { diff };
                ff.vel.x += step.x;
                ff.vel.z += step.z;
            }
            if ff.jump_buf > 0.0 && can_act && !slash_locked {
                let cancel = ff.slash_t > 0.0;
                let slide_jump = ff.sliding > 0.0;
                self.note(i, |s| {
                    s.jumps += 1;
                    if cancel {
                        s.cancels += 1
                    }
                    if slide_jump {
                        s.slide_jumps += 1
                    }
                });
                ff.vel.y = JUMP_VEL;
                if perfect_window {
                    // a perfectly timed bounce
                    ff.vel.y = JUMP_VEL * 1.12;
                    let flat = ff.vel.flat();
                    let boosted = (flat.len() * 1.08).min(RUN_SPEED * 2.2);
                    if flat.len() > 0.1 {
                        let f2 = flat.norm() * boosted;
                        ff.vel.x = f2.x;
                        ff.vel.z = f2.z;
                    }
                    ff.perfect_t = 0.6;
                    sounds.push(Sfx::Perfect);
                    self.note(i, |s| s.perfects += 1);
                }
                if cancel || slide_jump || perfect_window {
                    ff.chain();
                }
                if slide_jump || ff.carry > speed {
                    ff.from_move = true; // a slide or momentum jump keeps going on landing
                }
                ff.sliding = 0.0;
                ff.on_ground = false;
                ff.jump_buf = 0.0;
                ff.coyote = 0.0;
                cancel_slash(&mut ff);
                sounds.push(Sfx::Jump);
            }
        } else if ff.jump_buf > 0.0 && ff.coyote > 0.0 && can_act && ff.vel.y <= 0.5 {
            // jump just after running off a ledge
            self.note(i, |s| s.jumps += 1);
            ff.vel.y = JUMP_VEL;
            ff.jump_buf = 0.0;
            ff.coyote = 0.0;
            sounds.push(Sfx::Jump);
        } else {
            // air control
            if wish.len() > 0.1 {
                let target = wish * speed.max(ff.vel.flat().len().min(12.0 * flow_k));
                let cur = ff.vel.flat();
                let diff = target - cur;
                let acc = 22.0 * dt;
                let step = if diff.len() > acc { diff.norm() * acc } else { diff };
                ff.vel.x += step.x;
                ff.vel.z += step.z;
            }
            // start a wall run
            if ff.jump_buf > 0.0 && can_act && ff.wallruns_left == 0 && ff.walljumps > 0 && !slash_locked {
                // out of wall runs: still kick off the wall
                if let Some(n) = self.touching_wall(ff.pos) {
                    ff.vel = (n * 8.0 + (fwd - n * fwd.dot(n)).flat() * 3.0) * flow_k + v3(0.0, 8.2, 0.0);
                    self.note(i, |s| s.walljumps += 1);
                    ff.from_move = true;
                    ff.chain();
                    ff.walljumps -= 1;
                    ff.jump_buf = 0.0;
                    ff.air_dash = false;
                    cancel_slash(&mut ff);
                    sounds.push(Sfx::Jump);
                }
            }
            if ff.jump_buf > 0.0 && can_act && ff.wallruns_left > 0 && !slash_locked {
                if let Some(n) = self.touching_wall(ff.pos) {
                    cancel_slash(&mut ff);
                    let into = fwd.dot(-n);
                    let tangent = v3(-n.z, 0.0, n.x);
                    let vertical = into > 0.75;
                    let dir = if fwd.dot(tangent) >= 0.0 { tangent } else { -tangent };
                    // run along it as fast as you arrived (at least 10)
                    let entry = ff.vel.flat().len().clamp(10.0, 22.0);
                    ff.wall = Some(WallRun { normal: n, dir, t: 0.0, vertical, speed: entry });
                    ff.chain();
                    self.note(i, |s| if vertical { s.wallclimbs += 1 } else { s.wallruns += 1 });
                    ff.wallruns_left -= 1;
                    ff.jump_buf = 0.0;
                    ff.air_dash = false;
                    gravity = false;
                    sounds.push(Sfx::Jump);
                }
            }
        }
        if gravity && ff.wall.is_none() {
            // launched fighters hang in the air a moment so they can be juggled
            let k = if ff.float_t > 0.0 && ff.vel.y < 3.0 { 0.35 } else { 1.0 };
            ff.vel.y -= GRAVITY * k * dt;
        }
        ff.vel.y = ff.vel.y.max(-40.0);

        self.collide(&mut ff, dt);
        if ff.on_ground && !grounded_before {
            // landed: from a dash, wall-jump or air dash you keep your speed a moment
            ff.last_air = ff.air_t;
            ff.land_t = 0.0;
            let flat = ff.vel.flat().len();
            if ff.from_move && flat > RUN_SPEED * flow_k * 0.95 {
                ff.carry = flat.min(RUN_SPEED * 1.7 * flow_k);
                ff.carry_t = MOMENTUM_HOLD;
                self.note(i, |s| s.momentum += 1);
            }
            ff.from_move = false;
        }
        if ff.on_ground && ff.wall.is_none() {
            ff.ground_t += dt;
            ff.air_t = 0.0;
        } else {
            ff.ground_t = 0.0;
            ff.air_t += dt;
        }
        if !ff.on_ground {
            ff.sliding = 0.0;
        }
        // the chain fades if you stand still, or slowly after a while on foot
        if ff.on_ground && ff.vel.flat().len() < 1.0 && wish.len() < 0.1 {
            ff.still_t += dt;
            if ff.still_t > 0.2 {
                ff.flow = 0;
            }
        } else {
            ff.still_t = 0.0;
        }
        if ff.ground_t > 1.0 && ff.flow > 0 && ff.sliding <= 0.0 {
            ff.flow_fade += dt;
            if ff.flow_fade > 0.6 {
                ff.flow_fade = 0.0;
                ff.flow -= 1;
            }
        }
        if ff.on_ground {
            ff.wallruns_left = 2;
            ff.walljumps = 2;
            ff.air_slashes = 0;
            ff.air_dash = false;
            if ff.wall.is_some() {
                ff.wall = None;
            }
        }
        let sp = ff.vel.flat().len();
        if ff.on_ground && sp > 0.5 {
            ff.walk_phase += sp * dt * 1.6;
        } else if ff.wall.is_some() {
            ff.walk_phase += 12.0 * dt;
        }

        // attacks
        if ff.slash_t > 0.0 {
            ff.slash_t -= dt;
            if ff.slash_t <= 0.0 {
                ff.slash_t = 0.0;
                ff.combo_window = 0.35;
            }
        }
        if can_act && ff.switch_t <= 0.0 {
            match ff.weapon {
                _ if melee => {
                    if inp.fire_pressed {
                        ff.slash_queued = true;
                    }
                    if ff.slash_queued && ff.slash_t < dur * 0.36 {
                        ff.slash_queued = false;
                        let chain = ff.slash_t > 0.0 || ff.combo_window > 0.0;
                        let butterfly = ff.guard || ff.bfly_t > 0.0;
                        let airborne = !ff.on_ground && ff.wall.is_none();
                        let mut start = dur;
                        // slash kinds: 0/1 normal swings, 2 = air smash (down), 3 = rising launcher
                        let idx = if butterfly {
                            // slash out of a guard: quick, keeps alternating the first two swings
                            start = dur * 0.83;
                            if chain && ff.slash_idx == 0 { 1 } else { 0 }
                        } else if ff.ground_dash_t > 0.0 {
                            3
                        } else if airborne {
                            if ff.air_slashes >= 2 { 2 } else if chain && ff.slash_idx == 0 { 1 } else { 0 }
                        } else if chain {
                            match ff.slash_idx {
                                0 => 1,
                                1 => 3,
                                _ => 0,
                            }
                        } else {
                            0
                        };
                        if butterfly {
                            self.note(i, |s| s.butterflies += 1);
                        }
                        ff.guard = false;
                        ff.bfly_t = 0.0;
                        ff.slash_idx = idx;
                        ff.slash_t = start;
                        ff.slash_hit_done = false;
                        match idx {
                            3 => {
                                // rise with the launcher so you can follow up in the air
                                ff.vel.x = fwd.x * 4.0;
                                ff.vel.z = fwd.z * 4.0;
                                ff.vel.y = ff.vel.y.max(7.2);
                                ff.on_ground = false;
                                ff.dash_t = 0.0;
                                ff.ground_dash_t = 0.0;
                            }
                            2 => {
                                ff.vel.x = fwd.x * 4.0;
                                ff.vel.z = fwd.z * 4.0;
                                ff.vel.y = -12.0;
                                ff.air_slashes = ff.air_slashes.saturating_add(1);
                            }
                            _ if airborne => {
                                // air slashes hover you so you can keep juggling
                                ff.vel.x = ff.vel.x * 0.4 + fwd.x * 3.5;
                                ff.vel.z = ff.vel.z * 0.4 + fwd.z * 3.5;
                                ff.vel.y = ff.vel.y.max(2.8);
                                ff.air_slashes = ff.air_slashes.saturating_add(1);
                            }
                            _ => {
                                let lunge = if butterfly { 3.0 } else { 5.0 };
                                ff.vel.x = fwd.x * lunge;
                                ff.vel.z = fwd.z * lunge;
                            }
                        }
                        acts.push(Act::Slash);
                    }
                }
                w => {
                    if inp.fire && ff.fire_cd <= 0.0 && ff.reload_t <= 0.0 {
                        if ff.mag[wi] > 0 {
                            ff.mag[wi] -= 1;
                            ff.fire_cd = w.interval();
                            let spread = ff.spread(); // what the crosshair showed
                            ff.recoil = 1.0;
                            ff.since_shot = 0.0;
                            acts.push(Act::Gun { w, spread });
                            if ff.mag[wi] == 0 {
                                ff.reload_t = w.reload_time();
                            }
                        } else {
                            ff.reload_t = w.reload_time();
                            sounds.push(Sfx::Reload);
                        }
                    }
                }
            }
            if inp.item && ff.items > 0 && ff.item_cd <= 0.0 {
                match ff.loadout.item {
                    2 if self.mt.mode.melee_only() => {}
                    2 => {
                        ff.items -= 1;
                        ff.item_cd = 0.9;
                        acts.push(Act::Throw);
                    }
                    k => {
                        let full = if k == 0 { ff.hp >= MAX_HP } else { ff.ap >= MAX_AP };
                        if !full {
                            ff.items -= 1;
                            ff.item_cd = 1.0;
                            acts.push(Act::Item { kind: k });
                        }
                    }
                }
            }
        }

        let p = ff.pos;
        self.f[i] = ff;
        for s in sounds {
            self.sfx(s, p);
        }
        acts
    }

    // ------------------------------------------------------------------ combat

    /// Third-person camera for a fighter: (camera position, look direction).
    pub fn camera_for(&self, i: usize) -> (V3, V3) {
        let ff = &self.f[i];
        self.camera_from(ff.pos, ff.yaw, ff.pitch)
    }

    pub fn camera_from(&self, pos: V3, yaw: f32, pitch: f32) -> (V3, V3) {
        let (pivot, dir, dist) = self.camera_rig(pos, yaw, pitch);
        (pivot - dir * dist, dir)
    }

    /// Over-the-right-shoulder camera: the point it orbits around, the look direction and how far
    /// back it can sit before hitting a wall (camera collision).
    pub fn camera_rig(&self, pos: V3, yaw: f32, pitch: f32) -> (V3, V3, f32) {
        const SHOULDER: f32 = 0.78;
        const HEIGHT: f32 = 1.98;
        const BACK: f32 = 3.2;
        const PAD: f32 = 0.35;
        let dir = look_dir(yaw, pitch);
        let right = yaw_right(yaw);
        let head = pos + v3(0.0, 1.6, 0.0);
        // keep the shoulder point out of walls on the right
        let mut side = SHOULDER;
        if let Some((t, _)) = self.world.raycast(head, right, SHOULDER + PAD) {
            side = (t - PAD).max(0.0);
        }
        let mut pivot = head + right * side;
        if let Some((t, _)) = self.world.raycast(pivot, V3::UP, HEIGHT - 1.6 + PAD) {
            pivot.y += (t - PAD).max(0.0);
        } else {
            pivot.y += HEIGHT - 1.6;
        }
        // pull the camera in when anything is behind it (a few rays, so the edge of the
        // view doesn't poke through walls either)
        let back = -dir;
        let cam_up = dir.cross(right).norm();
        let mut dist = BACK;
        for off in [V3::ZERO, right * PAD, -right * PAD, cam_up * PAD, -cam_up * PAD] {
            if let Some((t, _)) = self.world.raycast(pivot + off, back, BACK + PAD) {
                dist = dist.min((t - PAD).max(0.15));
            }
        }
        (pivot, dir, dist)
    }

    /// Pose every fighter with the same animation the screen shows, and keep their hit shapes.
    fn update_hit_poses(&mut self) {
        let rig = crate::anim::shared();
        for i in 0..self.f.len() {
            let f = &self.f[i];
            if !f.present || !f.alive || f.spectate {
                self.hit_poses[i] = None;
                continue;
            }
            let posed = rig.and_then(|r| crate::character::animate(r, &mut self.anim[i], f, f.pos, self.time));
            self.hit_poses[i] = Some(match posed {
                Some(p) => p.hit,
                None => HitPose::standing(f.pos, f.yaw),
            });
        }
    }

    /// A fighter's hit shapes now (posed this tick, or standing if not posed yet).
    pub fn hit_pose(&self, j: usize) -> HitPose {
        self.hit_poses[j].unwrap_or_else(|| HitPose::standing(self.f[j].pos, self.f[j].yaw))
    }

    /// Where a ray first meets a fighter's body, and which part.
    pub fn ray_fighter(&self, j: usize, o: V3, d: V3, max_t: f32) -> Option<(f32, Part, bool)> {
        let pose = self.hit_pose(j);
        // quick reject: the box around all the shapes
        pose.bounds().ray(o, d, max_t)?;
        pose.ray_cap(o, d, max_t, 0.0).map(|(t, i)| (t, pose.caps[i].part, HitPose::is_far_limb(i)))
    }

    /// (host) a fighter's hit shapes `t` seconds into the game, from recent history.
    pub fn pose_at(&self, slot: usize, t: f32) -> HitPose {
        let h = &self.pose_history[slot];
        let mut prev: Option<&(f32, HitPose)> = None;
        for e in h.iter() {
            if e.0 >= t {
                return match prev {
                    Some(p) if e.0 > p.0 => p.1.lerp(&e.1, (t - p.0) / (e.0 - p.0)),
                    _ => e.1,
                };
            }
            prev = Some(e);
        }
        self.hit_pose(slot)
    }

    /// Someone else's shot hit near `p`: flash and flinch the part it hit.
    fn mark_hit_at(&mut self, p: V3) {
        let mut best: Option<(usize, Part, f32)> = None;
        for j in 0..self.f.len() {
            if !self.f[j].present || !self.f[j].alive {
                continue;
            }
            let (part, d) = self.hit_pose(j).nearest(p);
            if d < 0.35 && best.map_or(true, |b| d < b.2) {
                best = Some((j, part, d));
            }
        }
        if let Some((j, part, _)) = best {
            let f = &mut self.f[j];
            f.hit_part = part as u8;
            f.flash_t = 0.12;
            f.hurt_t = f.hurt_t.max(0.25);
        }
    }

    /// The enemy under the crosshair, if any (the camera's aim ray, stopped by walls).
    pub fn aim_enemy(&self, i: usize, yaw: f32, pitch: f32) -> Option<usize> {
        let (pivot, dir, dist) = self.camera_rig(self.f[i].pos, yaw, pitch);
        let cam = pivot - dir * dist;
        let start = cam + dir * (pivot - cam).dot(dir).max(0.0);
        let mut t = 200.0;
        if let Some((wt, _)) = self.world.raycast(start, dir, 200.0) {
            t = wt;
        }
        let mut best = None;
        for j in 0..self.f.len() {
            if !self.f[j].alive || !self.is_enemy(i, j) {
                continue;
            }
            if let Some((ht, _, _)) = self.ray_fighter(j, start, dir, t) {
                t = ht;
                best = Some(j);
            }
        }
        best
    }

    /// Where a fighter's shot starts and which way it goes.
    pub fn shot_ray(&self, i: usize) -> (V3, V3) {
        let ff = &self.f[i];
        let muzzle = ff.pos + v3(0.0, 1.38, 0.0) + yaw_right(ff.yaw) * 0.28 + ff.fwd() * 0.45;
        if ff.is_bot {
            return (ff.eye(), look_dir(ff.yaw, ff.pitch));
        }
        let (pivot, dir, dist) = self.camera_rig(ff.pos, ff.yaw, ff.pitch);
        let cam = pivot - dir * dist;
        let start = cam + dir * (pivot - cam).dot(dir).max(0.0);
        let mut t = 120.0;
        if let Some((wt, _)) = self.world.raycast(start, dir, 120.0) {
            t = wt;
        }
        for j in 0..self.f.len() {
            if !self.f[j].alive || !self.is_enemy(i, j) {
                continue;
            }
            if let Some((ht, _, _)) = self.ray_fighter(j, start, dir, t) {
                t = t.min(ht);
            }
        }
        let aim = start + dir * t;
        let d = (aim - muzzle).norm();
        // don't shoot backwards when the aim point is very close
        if d.dot(dir) < 0.2 { (muzzle, dir) } else { (muzzle, d) }
    }

    fn do_action(&mut self, i: usize, a: Act) {
        match a {
            Act::Item { kind } => {
                self.use_item(i, kind);
                if self.kind == Kind::Client && i == self.local {
                    self.hp_hold = self.f[i].ping_ms as f32 / 1000.0 + 0.2;
                    self.out_events.push(Ev::Item { kind });
                }
            }
            Act::Gun { w: Weapon::Rocket, .. } => {
                let (o, dir) = self.shot_ray(i);
                let v = dir * ROCKET_SPEED;
                self.grenades.push(Grenade { pos: o, vel: v, fuse: 5.0, owner: i, kind: 1 });
                if self.kind == Kind::Host || self.kind == Kind::Client {
                    self.out_events.push(Ev::Grenade { p: o, v, kind: 1 });
                }
                self.sfx(Sfx::Rocket, o);
                for _ in 0..8 {
                    let v = -dir * self.rng.range(2.0, 5.0) + v3(self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0));
                    self.particles.push(Particle { pos: o, vel: v, life: 0.3, max_life: 0.3, size: 0.25, color: [150, 150, 150], gravity: -1.0 });
                }
                if i == self.local {
                    self.shake = self.shake.max(0.5);
                }
            }
            Act::Gun { w, spread } => {
                let (o, dir) = self.shot_ray(i);
                let right = yaw_right(self.f[i].yaw);
                let gs = w.gun();
                let (pellets, dmg, range) = (gs.pellets, gs.dmg, gs.range);
                for _ in 0..pellets {
                    let d = pellet_dir(dir, right, spread, &mut self.rng);
                    self.fire_ray(i, o, d, dmg, range, w);
                }
                self.sfx(Sfx::gun(w), o);
                // muzzle flash
                let fsize = match w {
                    Weapon::Shotgun => 0.55,
                    Weapon::Rifle => 0.5,
                    Weapon::Smg => 0.28,
                    _ => 0.38,
                };
                self.flashes.push(Flash { pos: o + dir * 0.05, dir, life: 0.06, size: fsize });
                for _ in 0..5 {
                    let v = dir * self.rng.range(2.0, 6.0) + v3(self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0));
                    self.particles.push(Particle { pos: o, vel: v, life: 0.08, max_life: 0.08, size: 0.12, color: [255, 220, 120], gravity: 0.0 });
                }
                // muzzle smoke
                for _ in 0..2 {
                    let v = dir * self.rng.range(0.5, 1.5) + v3(0.0, self.rng.range(0.2, 0.6), 0.0);
                    self.particles.push(Particle { pos: o + dir * 0.1, vel: v, life: 0.5, max_life: 0.5, size: 0.16, color: [150, 150, 150], gravity: -0.5 });
                }
                // spent shells fly out to the right
                if w != Weapon::Revolver {
                    let color = if w == Weapon::Shotgun { [170, 30, 30] } else { [214, 172, 70] };
                    let v = right * self.rng.range(2.0, 3.2) + v3(0.0, self.rng.range(2.0, 3.0), 0.0) - dir * 0.5;
                    self.casings.push(Casing { pos: o - dir * 0.35 + right * 0.05, vel: v, life: 2.0, spin: self.rng.range(0.0, 6.0), color });
                    if self.casings.len() > 80 {
                        self.casings.remove(0);
                    }
                }
                if i == self.local && !self.f[i].is_bot {
                    // recoil: the hand cannon and sniper kick the aim up (it settles back)
                    self.aim_kick += match w {
                        Weapon::HandCannon => 0.075,
                        Weapon::Sniper => 0.05,
                        Weapon::Rifle => 0.018,
                        Weapon::Revolver => 0.02,
                        _ => 0.0,
                    };
                }
                if i == self.local {
                    // only a nudge: a shaking view would throw off the aim
                    self.shake = self.shake.max(match w {
                        Weapon::Shotgun | Weapon::Rifle => 0.2,
                        Weapon::Smg => 0.05,
                        _ => 0.12,
                    });
                }
            }
            Act::Slash => {
                let p = self.f[i].pos;
                self.sfx(Sfx::Slash, p);
            }
            Act::Throw => {
                let tp = self.f[i].pos + v3(0.0, 1.4, 0.0);
                self.sfx(Sfx::Throw, tp);
                let ff = &self.f[i];
                let dir = look_dir(ff.yaw, ff.pitch + 0.15);
                let p = ff.eye() + ff.fwd() * 0.5;
                let v = dir * 17.0 + v3(0.0, 3.0, 0.0) + ff.vel * 0.4;
                self.grenades.push(Grenade { pos: p, vel: v, fuse: 1.7, owner: i, kind: 0 });
                if self.kind == Kind::Host || self.kind == Kind::Client {
                    self.out_events.push(Ev::Grenade { p, v, kind: 0 });
                }
            }
        }
    }

    fn fire_ray(&mut self, shooter: usize, o: V3, d: V3, dmg: f32, range: f32, w: Weapon) {
        let mut t = range;
        let mut hit_world = false;
        if let Some((wt, _)) = self.world.raycast(o, d, range) {
            t = wt;
            hit_world = true;
        }
        let mut victim: Option<(usize, Part, bool)> = None;
        for j in 0..self.f.len() {
            if !self.f[j].alive || !self.is_enemy(shooter, j) {
                continue;
            }
            if let Some((ht, part, far)) = self.ray_fighter(j, o, d, t) {
                t = ht;
                victim = Some((j, part, far));
            }
        }
        let end = o + d * t;
        let mut spark = 0u8;
        if let Some((j, part, far)) = victim {
            let head = part == Part::Head;
            let gs = w.gun();
            let falloff = match w {
                Weapon::Shotgun => (1.0 - (t - 8.0).max(0.0) / 22.0).max(0.25),
                Weapon::Smg => (1.0 - (t - 20.0).max(0.0) / 60.0).max(0.5),
                _ => 1.0,
            };
            let mut amount = match w {
                // a hand-cannon headshot always kills
                Weapon::HandCannon if head => LETHAL,
                // the sniper kills on anything but hands, forearms, shins and feet
                Weapon::Sniper => {
                    if far {
                        SNIPER_LIMB
                    } else {
                        LETHAL
                    }
                }
                _ => dmg * falloff * if head { gs.head } else { part.mult() },
            };
            let v = &self.f[j];
            let blocked = v.guard && v.fwd().dot(-d) > 0.45;
            if blocked {
                amount = amount.min(gs.dmg.max(45.0)) * 0.12; // a sword guard stops even the big ones
                spark = 2;
            } else {
                // armour throws sparks along with the blood
                spark = if v.ap > 0.0 { 5 } else { 1 };
                let f = &mut self.f[j];
                f.hit_part = part as u8;
                f.flash_t = 0.12;
                f.hurt_t = f.hurt_t.max(0.25);
            }
            let knock = d.flat().norm() * gs.knock;
            self.damage(shooter, j, amount, knock, 0.0, head && !blocked, end, w as u8, o, d);
            if !blocked {
                match w {
                    Weapon::HandCannon if head => self.note(shooter, |s| s.cannon_heads += 1),
                    Weapon::Sniper => self.note(shooter, |s| s.sniper_hits += 1),
                    _ => {}
                }
            }
        } else if hit_world {
            spark = 3;
        }
        let color = if shooter == self.local { [255, 230, 150] } else { [255, 170, 120] };
        self.tracers.push(Tracer::shot(o, end, w, color));
        self.impact_fx(end, spark);
        if spark == 1 || spark == 5 {
            self.sfx(Sfx::BulletFlesh, end);
        }
        if self.kind == Kind::Host || self.kind == Kind::Client {
            self.out_events.push(Ev::Shot { o, e: end, w: w as u8, spark });
        }
    }

    pub fn impact_fx(&mut self, p: V3, spark: u8) {
        match spark {
            1 => {
                self.burst(p, [200, 25, 35], 14, 3.5);
                // a puff of red mist
                for _ in 0..2 {
                    let v = v3(self.rng.range(-0.6, 0.6), self.rng.range(0.0, 0.6), self.rng.range(-0.6, 0.6));
                    self.particles.push(Particle { pos: p, vel: v, life: 0.35, max_life: 0.35, size: 0.22, color: [130, 12, 18], gravity: 0.5 });
                }
            }
            2 => {
                self.burst(p, [255, 255, 200], 14, 7.0);
                self.sfx(Sfx::Clang, p);
            }
            4 => {
                // perfect parry: a bright burst
                self.burst(p, [255, 255, 230], 34, 9.0);
                self.burst(p, [255, 200, 90], 18, 5.0);
                self.sfx(Sfx::ParryPerfect, p);
                self.sfx(Sfx::Clang, p);
                let near = (p - self.f[self.local].pos).len() < 6.0;
                if near {
                    self.parry_flash = 0.3;
                }
            }
            5 => {
                // a hit on armour: sparks and a little blood
                self.burst(p, [255, 215, 140], 12, 6.0);
                self.burst(p, [200, 25, 35], 6, 3.0);
            }
            3 => {
                self.sfx(Sfx::ImpactStone, p);
                self.burst(p, [200, 190, 160], 5, 2.5);
                // sparks off stone
                self.burst(p, [255, 205, 120], 5, 7.0);
            }
            _ => {}
        }
    }

    pub fn burst(&mut self, p: V3, color: [u8; 3], n: usize, speed: f32) {
        for _ in 0..n {
            let v = v3(self.rng.range(-1.0, 1.0), self.rng.range(-0.3, 1.2), self.rng.range(-1.0, 1.0)).norm() * self.rng.range(0.3, 1.0) * speed;
            let life = self.rng.range(0.25, 0.6);
            self.particles.push(Particle { pos: p, vel: v, life, max_life: life, size: self.rng.range(0.05, 0.12), color, gravity: 9.0 });
        }
    }

    fn resolve_slash(&mut self, i: usize) {
        let a = self.f[i].clone();
        let fwd = a.fwd();
        for j in 0..self.f.len() {
            if !self.f[j].alive || !self.is_enemy(i, j) {
                continue;
            }
            let b = &self.f[j];
            let d = b.pos - a.pos;
            let dist = d.flat().len();
            let wpn = a.weapon;
            if dist > wpn.melee_range() || d.y.abs() > if a.on_ground { 1.9 } else { 2.5 } {
                continue;
            }
            if dist > 0.3 && fwd.dot(d.flat().norm()) < 0.25 {
                continue;
            }
            if !self.world.line_clear(a.chest(), b.chest()) {
                continue;
            }
            let heavy = (a.slash_idx >= 2 && wpn != Weapon::Dagger) || wpn == Weapon::Greatsword;
            let facing = b.fwd().dot(-d.flat().norm()) > 0.3;
            // where the blade meets the body: along the line from our chest to theirs
            let (from, to) = (a.chest(), b.chest());
            let dir = (to - from).norm();
            let (hit_at, part) = match self.hit_pose(j).ray(from, dir, (to - from).len() + 1.0, 0.05) {
                Some((t, part)) => (from + dir * t, part),
                None => (to, Part::Chest),
            };
            let counter = b.stagger_t > 0.0;
            if b.guard && facing && b.guard_t <= PERFECT_PARRY && !counter {
                // perfect parry: the attacker reels, open to a counter
                let at = &mut self.f[i];
                at.stun_t = STAGGER_TIME;
                at.stagger_t = STAGGER_TIME;
                at.slash_t = 0.0;
                at.slash_queued = false;
                at.vel = at.vel * 0.2 - d.flat().norm() * 2.5;
                self.impact_fx(hit_at, 4);
                if j == self.local || i == self.local {
                    self.shake = self.shake.max(0.35);
                    if self.kind == Kind::Offline {
                        self.slowmo = self.slowmo.max(0.35);
                    } else {
                        self.hitstop = self.hitstop.max(0.12); // slow motion would put us out of step online
                    }
                    self.say(if j == self.local { "PERFECT PARRY!".to_string() } else { "Parried - you're staggered!".to_string() });
                }
                if self.kind == Kind::Host || self.kind == Kind::Client {
                    self.out_events.push(Ev::Shot { o: hit_at, e: hit_at, w: 0, spark: 4 });
                }
                continue;
            }
            if b.guard && facing && !heavy && !counter {
                // parried
                self.impact_fx(hit_at, 2);
                if j == self.local || i == self.local {
                    self.shake = self.shake.max(0.2);
                }
                if self.kind == Kind::Host || self.kind == Kind::Client {
                    self.out_events.push(Ev::Shot { o: hit_at, e: hit_at, w: 0, spark: 2 });
                }
                continue;
            }
            let air = !a.on_ground && a.wall.is_none();
            let (dmg, kb, up, stun) = match a.slash_idx {
                3 => (22.0, 2.5, 11.5, 0.7), // launcher: pops them into the air
                2 => (26.0, 4.0, -14.0, 0.5), // air smash: slams them down
                _ if air => (15.0, 2.0, 5.5, 0.35), // juggle hit: keeps them up
                0 => (18.0, 3.5, 1.5, 0.12),
                _ => (18.0, 4.0, 2.0, 0.12),
            };
            let guard_break = b.guard && facing && heavy && !counter;
            let dmg = dmg * wpn.melee_mult() * if counter { 1.5 } else { 1.0 };

            let kb = kb * if wpn == Weapon::Greatsword { 1.3 } else { 1.0 };
            let knock = d.flat().norm() * kb + v3(0.0, up, 0.0);
            let (launch, air_hit) = (a.slash_idx == 3, air && !b.on_ground);
            if counter && i == self.local {
                self.say("COUNTER! x1.5".to_string());
            }
            if a.slash_idx == 3 && i == self.local {
                self.say("LAUNCH!".to_string());
            }
            self.note(i, |s| {
                s.slash_hits += 1;
                if launch {
                    s.launches += 1
                }
                if air_hit {
                    s.air_hits += 1
                }
            });
            {
                let f = &mut self.f[j];
                f.hit_part = part as u8;
                f.flash_t = 0.12;
                f.hurt_t = f.hurt_t.max(0.25);
            }
            self.damage(i, j, if guard_break { dmg * 0.5 } else { dmg }, knock, if guard_break { 0.9 } else { stun }, false, hit_at, wpn as u8, a.chest(), d.norm());
            self.impact_fx(hit_at, 1);
            self.sfx(if guard_break { Sfx::Clang } else { Sfx::Flesh }, hit_at);
            if i == self.local || j == self.local {
                let big = heavy || a.slash_idx >= 2;
                self.hitstop = self.hitstop.max(if big { 0.11 } else { 0.06 });
                self.shake = self.shake.max(if big { 0.55 } else { 0.22 });
            }
            if self.kind == Kind::Host || self.kind == Kind::Client {
                self.out_events.push(Ev::Shot { o: hit_at, e: hit_at, w: 0, spark: 1 });
            }
        }
    }

    /// A hit seen on this computer. The host (or a game on this computer) applies it right away;
    /// a player sends it to the host, which checks it against where the target was a moment ago.
    #[allow(clippy::too_many_arguments)]
    fn damage(&mut self, attacker: usize, victim: usize, amount: f32, knock: V3, stun: f32, head: bool, at: V3, w: u8, o: V3, dir: V3) {
        if self.f[victim].spawn_prot > 0.0 {
            return;
        }
        if attacker == self.local && victim != self.local {
            self.combo_hit(amount);
            self.hitmarker = if head { 0.35 } else { 0.25 };
            self.hit_head = head || (self.hit_head && self.time - self.hit_chain_t < 0.02);
            let shown = if amount >= LETHAL { (self.f[victim].hp + self.f[victim].ap).max(1.0) } else { amount };
            self.dmg_nums.push(DmgNum { pos: at + v3(0.0, 0.3, 0.0), amount: shown.round() as i32, life: 0.9, head: head || amount >= LETHAL });
            // one crisp tick per shot (a shotgun's pellets count once), climbing in pitch while
            // the hits keep coming
            let volley = self.time - self.hit_chain_t < 0.02;
            if !volley {
                self.hit_chain = if self.time - self.hit_chain_t < 1.5 { (self.hit_chain + 1).min(14) } else { 0 };
                let pitch = 2f32.powf(self.hit_chain as f32 * 1.5 / 12.0);
                self.sounds.push((Sfx::Hit, None, pitch));
            }
            self.hit_chain_t = self.time;
            if head {
                self.headshot_t = 0.7;
                self.sfx2d(Sfx::Headshot);
            }
            if amount >= 35.0 && !volley {
                self.sounds.push((Sfx::HitHeavy, None, 0.8)); // a deep thump for big hits
            }
            let v = &self.f[victim];
            if v.ap > 0.0 && amount * 0.6 >= v.ap {
                self.sounds.push((Sfx::ArmorBreak, None, 1.1));
                self.burst(at, [140, 190, 255], 16, 6.0);
            }
        }
        if self.is_authority() {
            self.authority_hit(victim, Some(attacker), amount, knock, stun, head, w);
        } else {
            self.out_events.push(Ev::Hit { victim: victim as u8, dmg: amount, knock, stun, head, w, o, dir });
        }
    }

    /// The host's (or this computer's) final word on a hit: health goes down here, and the
    /// victim's computer is told so it can stagger.
    #[allow(clippy::too_many_arguments)]
    pub fn authority_hit(&mut self, victim: usize, attacker: Option<usize>, amount: f32, knock: V3, stun: f32, head: bool, w: u8) {
        if !self.f[victim].present || !self.f[victim].alive || self.f[victim].spawn_prot > 0.0 {
            return;
        }
        if self.f[victim].owned {
            let src = attacker.filter(|&a| a < self.f.len()).map(|a| self.f[a].pos);
            self.apply_hit(victim, attacker, amount, knock, stun, head, w, src);
        } else {
            if matches!(self.mt.phase, Phase::Countdown | Phase::MatchOver | Phase::Loadout) {
                return;
            }
            let ff = &mut self.f[victim];
            let mut a = amount;
            if amount >= LETHAL {
                a = ff.hp; // through any armour
            } else if ff.ap > 0.0 {
                let absorbed = (a * 0.6).min(ff.ap);
                ff.ap -= absorbed;
                a -= absorbed;
            }
            ff.hp -= a;
            ff.hurt_t = 0.25;
            ff.flow = 0;
            ff.last_weapon = w;
            ff.last_head = head;
            if let Some(at) = attacker {
                ff.last_attacker = Some(at);
            }
            // tell the victim, with where the attacker stands (for their hit-direction marker)
            let o = attacker.filter(|&a| a < self.f.len()).map_or(V3::ZERO, |a| self.f[a].pos);
            self.out_events.push(Ev::Hit { victim: victim as u8, dmg: amount, knock, stun, head, w, o, dir: V3::ZERO });
        }
    }

    /// Where a fighter was 	 seconds into the game (from the host's recent history).
    pub fn pos_at(&self, slot: usize, t: f32) -> V3 {
        let h = &self.history[slot];
        let mut prev: Option<(f32, V3)> = None;
        for &(ht, p) in h.iter() {
            if ht >= t {
                return match prev {
                    Some((pt, pp)) if ht > pt => pp.lerp(p, (t - pt) / (ht - pt)),
                    _ => p,
                };
            }
            prev = Some((ht, p));
        }
        self.f[slot].pos
    }

    /// Lag compensation: does a player's reported hit hold up against where the target was when
    /// that player saw it (their ping ago)? Also throws out impossible damage and weapons they
    /// don't carry.
    /// Returns None when the claim is thrown out; otherwise whether a claimed headshot holds up
    /// and whether the shot touched the body proper (not just a hand, forearm, shin or foot).
    #[allow(clippy::too_many_arguments)]
    pub fn validate_claim(&self, shooter: usize, victim: usize, w: u8, dmg: f32, o: V3, dir: V3, head: bool) -> Option<ClaimCheck> {
        if victim >= self.f.len() || !self.f[victim].present || !self.f[victim].alive || !self.is_enemy(shooter, victim) {
            return None;
        }
        let s = &self.f[shooter];
        if !s.alive {
            return None;
        }
        let wpn = Weapon::from_u8(w);
        if w >= N_WEAPONS || !s.loadout.has(wpn) || wpn == Weapon::Rocket {
            return None; // explosions are worked out by the host itself
        }
        if !(dmg > 0.0) || dmg > wpn.max_hit() * 1.5 * 1.05 {
            return None; // (a counter-slash does 1.5x)
        }
        // what they saw: our copy of the target, their ping plus their playback delay ago (which
        // depends on how jumpy their connection is, so try the whole range it can be in)
        let ping = s.ping_ms as f32 / 1000.0;
        let d = dir.norm();
        let sp_now = s.pos;
        let mut hit = false;
        let mut head_ok = false;
        let mut core_ok = false;
        for back in [INTERP_DELAY, (INTERP_DELAY + MAX_INTERP) * 0.5, MAX_INTERP] {
            let rewind = (ping + back + 0.015).clamp(0.0, 0.7);
            let t = self.time - rewind;
            let vp = self.pos_at(victim, t);
            let sp_then = self.pos_at(shooter, t);
            if wpn.is_melee() {
                let reach = wpn.melee_range() + 1.6;
                let ok = |sp: V3| (vp - sp).flat().len() <= reach && (vp.y - sp.y).abs() <= 3.0;
                if ok(sp_then) || ok(sp_now) {
                    return Some(ClaimCheck { head_ok: false, core_ok: true });
                }
                continue;
            }
            let near = |sp: V3| (o - (sp + v3(0.0, 1.4, 0.0))).len() < 3.5;
            if !(near(sp_then) || near(sp_now)) || d.len() < 0.5 {
                continue;
            }
            // the body as it stood then, each part a little fatter for network jitter
            let pose = self.pose_at(victim, t);
            let range = wpn.gun().range + 2.0;
            if pose.ray(o, d, range, 0.25).is_some() {
                hit = true;
                if head && pose.ray_part(o, d, range, 0.12, Part::Head) {
                    head_ok = true;
                }
                if pose.ray_core(o, d, range, 0.15) {
                    core_ok = true;
                }
            }
        }
        hit.then_some(ClaimCheck { head_ok, core_ok })
    }

    /// Medkit (0) or armor kit (1).
    pub fn use_item(&mut self, i: usize, kind: u8) {
        let ff = &mut self.f[i];
        if kind == 0 {
            ff.hp = (ff.hp + 50.0).min(MAX_HP);
        } else {
            ff.ap = (ff.ap + 50.0).min(MAX_AP);
        }
        let p = ff.chest();
        self.burst(p, if kind == 0 { [90, 255, 120] } else { [120, 180, 255] }, 14, 2.5);
        self.sfx(if kind == 0 { Sfx::Heal } else { Sfx::Armor }, p);
    }

    pub fn add_chat(&mut self, slot: usize, text: &str) {
        let text: String = text.chars().filter(|c| !c.is_control()).take(100).collect();
        if text.trim().is_empty() {
            return;
        }
        let (name, color) = if slot < self.f.len() && self.f[slot].present { (self.f[slot].name.clone(), self.f[slot].color_idx) } else { ("?".into(), 0) };
        self.chat.push(ChatLine { name, color, text, life: 12.0 });
        if self.chat.len() > 8 {
            self.chat.remove(0);
        }
    }

    /// Something you typed: shown here and sent to everyone.
    pub fn say_chat(&mut self, text: &str) {
        let l = self.local;
        self.add_chat(l, text);
        if self.kind == Kind::Host || self.kind == Kind::Client {
            self.out_events.push(Ev::Chat { text: text.chars().take(100).collect() });
        }
    }

    /// You confirmed your loadout (in the loadout phase).
    pub fn set_ready(&mut self, lo: Loadout) {
        self.my_loadout = lo;
        let l = self.local;
        self.f[l].loadout = lo;
        self.f[l].ready = true;
        if self.f[l].alive && matches!(self.mt.phase, Phase::Warmup | Phase::Loadout | Phase::Countdown) {
            self.f[l].equip();
        }
        if self.kind == Kind::Client {
            self.out_events.push(Ev::Ready);
        }
    }

    /// Vote for a rematch at the end of a match.
    pub fn vote_rematch(&mut self) {
        let l = self.local;
        self.f[l].vote = true;
        if self.kind == Kind::Client {
            self.out_events.push(Ev::Vote);
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn apply_hit(&mut self, victim: usize, attacker: Option<usize>, amount: f32, knock: V3, stun: f32, head: bool, w: u8, src: Option<V3>) {
        let ff = &mut self.f[victim];
        if !ff.alive || !ff.present || ff.spawn_prot > 0.0 {
            return;
        }
        if self.mt.phase == Phase::Countdown || self.mt.phase == Phase::MatchOver {
            return;
        }
        ff.last_weapon = w;
        ff.last_head = head;
        let mut a = amount;
        if amount >= LETHAL {
            a = ff.hp; // through any armour
        } else if ff.ap > 0.0 {
            let absorbed = (a * 0.6).min(ff.ap);
            ff.ap -= absorbed;
            a -= absorbed;
        }
        ff.hp -= a;
        ff.hurt_t = 0.25;
        ff.flow = 0;
        if knock.y.abs() > 4.5 {
            // launches, juggles and slams set the vertical speed outright
            ff.vel.x += knock.x;
            ff.vel.z += knock.z;
            ff.vel.y = knock.y;
            if knock.y > 7.0 {
                ff.float_t = 0.9;
            } else if knock.y > 0.0 && !ff.on_ground {
                ff.float_t = ff.float_t.max(0.55);
            } else if knock.y < 0.0 {
                ff.float_t = 0.0;
            }
        } else {
            ff.vel += knock;
        }
        if knock.y > 0.5 {
            ff.on_ground = false;
        }
        if stun > 0.0 {
            ff.stun_t = ff.stun_t.max(stun);
            ff.slash_t = 0.0;
            ff.wall = None;
        }
        if let Some(at) = attacker {
            ff.last_attacker = Some(at);
        }
        if victim == self.local {
            let me = self.f[victim].pos;
            let explosive = w == GRENADE_TOOL || w == Weapon::Rocket as u8;
            let from = match (src, attacker) {
                _ if explosive && knock.flat().len() > 0.1 => me - knock.flat().norm() * 3.0,
                (Some(s), _) if s != V3::ZERO => s,
                (_, Some(a)) if a < self.f.len() && a != victim => self.f[a].pos,
                _ => me - knock.flat(),
            };
            if (from - me).flat().len() > 0.3 {
                self.hurt_dirs.push((from, 1.2));
                if self.hurt_dirs.len() > 6 {
                    self.hurt_dirs.remove(0);
                }
            }
            self.hurt_flash = 0.5;
            let heavy = amount >= 30.0 || knock.y.abs() > 6.0;
            self.shake = self.shake.max(if heavy { 0.6 } else { 0.3 });
            if w < 3 {
                // a sword hit on you: brief freeze
                self.hitstop = self.hitstop.max(if heavy { 0.1 } else { 0.05 });
            }
            let p = self.f[victim].pos;
            self.sfx(Sfx::Hurt, p);
        }
    }

    fn update_grenades(&mut self, dt: f32) {
        let mut explode = Vec::new();
        for (gi, g) in self.grenades.iter_mut().enumerate() {
            g.fuse -= dt;
            if g.kind == 1 {
                // rocket: straight line, bursts on whatever it touches
                let np = g.pos + g.vel * dt;
                let hit_wall = self.world.blocks.iter().any(|b| b.b.contains(np));
                let hit_body = self.f.iter().any(|f| {
                    f.present && f.alive && f.slot != g.owner && {
                        let b = f.body_box();
                        Aabb::new(b.min - v3(0.15, 0.15, 0.15), b.max + v3(0.15, 0.5, 0.15)).contains(np)
                    }
                });
                if hit_wall || hit_body || g.fuse <= 0.0 {
                    explode.push(gi);
                } else {
                    g.pos = np;
                }
                continue;
            }
            g.vel.y -= GRAVITY * dt;
            for axis in 0..3 {
                let mut np = g.pos;
                match axis {
                    0 => np.x += g.vel.x * dt,
                    1 => np.y += g.vel.y * dt,
                    _ => np.z += g.vel.z * dt,
                }
                let inside = self.world.blocks.iter().any(|b| b.b.contains(np));
                if inside {
                    match axis {
                        0 => g.vel.x *= -0.45,
                        1 => {
                            g.vel.y *= -0.35;
                            g.vel.x *= 0.7;
                            g.vel.z *= 0.7;
                        }
                        _ => g.vel.z *= -0.45,
                    }
                } else {
                    g.pos = np;
                }
            }
            if g.fuse <= 0.0 {
                explode.push(gi);
            }
        }
        for gi in explode.into_iter().rev() {
            let g = self.grenades.remove(gi);
            self.explode(g.pos, g.owner, g.kind);
        }
    }

    fn explode(&mut self, p: V3, owner: usize, kind: u8) {
        self.sfx(Sfx::Boom, p);
        self.burst(p, [255, 170, 60], 40, 9.0);
        self.burst(p, [90, 90, 90], 20, 4.0);
        for _ in 0..10 {
            let v = v3(self.rng.range(-1.0, 1.0), self.rng.range(0.0, 1.0), self.rng.range(-1.0, 1.0)) * 2.0;
            self.particles.push(Particle { pos: p, vel: v, life: 0.45, max_life: 0.45, size: 1.2, color: [255, 200, 90], gravity: -2.0 });
        }
        let lp = self.f[self.local].pos;
        let d = (lp - p).len();
        if d < 20.0 {
            self.shake = self.shake.max(1.0 * (1.0 - d / 20.0) + 0.15);
        }
        // damage is worked out by the host (or this computer when playing alone)
        if !self.is_authority() {
            return;
        }
        let (radius, base, extra, w) = if kind == 1 { (ROCKET_RADIUS, 8.0, ROCKET_MAX - 8.0, Weapon::Rocket as u8) } else { (4.8, 18.0, 42.0, GRENADE_TOOL) };
        for j in 0..self.f.len() {
            if !self.f[j].alive || !self.f[j].present || self.f[j].spawn_prot > 0.0 {
                continue;
            }
            if j != owner && owner < self.f.len() && !self.is_enemy(owner, j) {
                continue;
            }
            let c = self.f[j].chest();
            let d = (c - p).len();
            if d < radius && self.world.line_clear(p + v3(0.0, 0.2, 0.0), c) {
                let k = 1.0 - d / radius;
                // rockets fall off fast from the centre (squared); grenades evenly
                let mut dmg = if kind == 1 { base + extra * k * k } else { base + extra * k };
                let mut knock = (c - p).norm() * (6.0 + 8.0 * k) + v3(0.0, 4.0 * k + 2.0, 0.0);
                if j == owner {
                    // your own blast hurts less and throws you further (rocket jump)
                    dmg *= 0.35;
                    knock = knock * 1.5 + v3(0.0, 3.0, 0.0);
                    if kind == 1 {
                        self.note(j, |s| s.rocket_jumps += 1);
                    }
                }
                if owner == self.local && j != self.local {
                    self.combo_hit(dmg);
                    self.hitmarker = 0.25;
                    self.dmg_nums.push(DmgNum { pos: c + v3(0.0, 0.5, 0.0), amount: dmg.round() as i32, life: 0.9, head: false });
                }
                self.authority_hit(j, Some(owner), dmg, knock, 0.3, false, w);
            }
        }
    }
    fn update_casings(&mut self, dt: f32) {
        for k in 0..self.casings.len() {
            let mut c = self.casings[k].clone();
            c.life -= dt;
            c.vel.y -= GRAVITY * dt;
            c.spin += dt * 18.0 * (c.vel.len() * 0.2).min(1.0);
            let np = c.pos + c.vel * dt;
            let floor = self.world.ground_below(c.pos + v3(0.0, 0.05, 0.0));
            if np.y <= floor + 0.01 && c.vel.y < 0.0 {
                c.pos.y = floor + 0.01;
                c.vel.y = -c.vel.y * 0.35;
                c.vel.x *= 0.6;
                c.vel.z *= 0.6;
                if c.vel.y < 0.4 {
                    c.vel = V3::ZERO;
                }
            } else {
                c.pos = np;
            }
            self.casings[k] = c;
        }
        self.casings.retain(|c| c.life > 0.0);
        for f in self.flashes.iter_mut() {
            f.life -= dt;
        }
        self.flashes.retain(|f| f.life > 0.0);
    }

    fn update_fx(&mut self, dt: f32) {
        self.update_casings(dt);
        for p in self.particles.iter_mut() {
            p.life -= dt;
            p.vel.y -= p.gravity * dt;
            p.pos += p.vel * dt;
        }
        self.particles.retain(|p| p.life > 0.0);
        if self.particles.len() > 900 {
            let n = self.particles.len() - 900;
            self.particles.drain(0..n);
        }
        for t in self.tracers.iter_mut() {
            t.life -= dt;
        }
        self.tracers.retain(|t| t.life > 0.0);
        for d in self.dmg_nums.iter_mut() {
            d.life -= dt;
            d.pos.y += dt * 1.2;
        }
        self.dmg_nums.retain(|d| d.life > 0.0);
        for f in self.feed.iter_mut() {
            f.life -= dt;
        }
        self.feed.retain(|f| f.life > 0.0);
        self.hitmarker = (self.hitmarker - dt).max(0.0);
        self.kill_t = (self.kill_t - dt).max(0.0);
        self.headshot_t = (self.headshot_t - dt).max(0.0);
        for h in self.hurt_dirs.iter_mut() {
            h.1 -= dt;
        }
        self.hurt_dirs.retain(|h| h.1 > 0.0);
        self.hurt_flash = (self.hurt_flash - dt).max(0.0);
        self.shake = (self.shake - dt * 2.5).max(0.0);
    }

    // ------------------------------------------------------------------ network glue

    pub fn handle_remote_event(&mut self, from: usize, ev: Ev) {
        match ev {
            Ev::Shot { o, e, w, spark } => {
                if w != 0 {
                    self.tracers.push(Tracer::shot(o, e, Weapon::from_u8(w), [255, 170, 120]));
                    for _ in 0..4 {
                        let v = (e - o).norm() * self.rng.range(2.0, 5.0);
                        self.particles.push(Particle { pos: o, vel: v, life: 0.08, max_life: 0.08, size: 0.18, color: [255, 220, 120], gravity: 0.0 });
                    }
                    if from < MAX_PLAYERS && self.time - self.shot_last[from] > 0.04 {
                        self.shot_last[from] = self.time;
                        self.sfx(Sfx::gun(Weapon::from_u8(w)), o);
                    }
                    if spark == 1 || spark == 5 {
                        self.sfx(Sfx::BulletFlesh, e);
                    }
                } else if spark == 1 {
                    self.sfx(Sfx::Flesh, e); // a sword hit
                }
                if spark == 1 || spark == 5 {
                    self.mark_hit_at(e);
                }
                self.impact_fx(e, spark);
            }
            Ev::Hit { victim, dmg, knock, stun, head, w, o, dir } => {
                let v = victim as usize;
                if v >= self.f.len() {
                    return;
                }
                if self.kind == Kind::Host {
                    // a player says they hit someone: check it against the recent past
                    if from != 0 {
                        if let Some(chk) = self.validate_claim(from, v, w, dmg, o, dir, head) {
                            let wpn = Weapon::from_u8(w);
                            let gs = wpn.gun();
                            let mut dmg = dmg;
                            // what the host can't confirm still counts, at the lesser damage
                            if dmg >= LETHAL {
                                match wpn {
                                    Weapon::HandCannon if !(head && chk.head_ok) => dmg = gs.dmg,
                                    Weapon::Sniper if !chk.core_ok => dmg = SNIPER_LIMB,
                                    _ => {}
                                }
                            } else if head && !chk.head_ok && !wpn.is_melee() {
                                dmg /= gs.head.max(1.0);
                            }
                            self.authority_hit(v, Some(from), dmg, knock, stun, head && chk.head_ok, w);
                        }
                    }
                } else if from == 0 && v == self.local && self.f[v].owned {
                    // the host confirmed we were hit
                    self.apply_hit(v, None, dmg, knock, stun, head, w, Some(o));
                }
            }
            Ev::Grenade { p, v, kind } => {
                self.grenades.push(Grenade { pos: p, vel: v, fuse: if kind == 1 { 5.0 } else { 1.7 }, owner: from, kind });
            }
            Ev::Died { victim, killer, w, head } => {
                let v = victim as usize;
                if self.kind == Kind::Client && from == 0 && v < self.f.len() && self.f[v].present {
                    self.f[v].alive = false;
                    self.f[v].death_t = 0.0;
                    if v == self.local {
                        let rounds = self.mt.mode.rounds() && self.mt.phase == Phase::Fight;
                        self.f[v].respawn_t = if rounds { 1e9 } else { RESPAWN_TIME };
                        self.f[v].wall = None;
                    }
                    let k = if (killer as usize) < self.f.len() { Some(killer as usize) } else { None };
                    self.on_death(v, k, w, head);
                }
            }
            Ev::Chat { text } => {
                self.add_chat(from, &text);
            }
            Ev::Item { kind } => {
                if self.kind == Kind::Host && from < self.f.len() && self.f[from].alive && self.f[from].items > 0 && kind < 2 && self.f[from].loadout.item == kind {
                    self.f[from].items -= 1;
                    self.use_item(from, kind);
                }
            }
            Ev::Ready => {
                if self.kind == Kind::Host && from < self.f.len() {
                    self.f[from].ready = true;
                }
            }
            Ev::Vote => {
                if self.kind == Kind::Host && from < self.f.len() {
                    self.f[from].vote = true;
                }
            }
        }    }
}


#[cfg(test)]
mod k_style_tests {
    use super::*;

    fn setup() -> Game {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        // a dummy: a bot with no brain stands still
        g.bots[1] = None;
        g.f[0].spawn_prot = 0.0;
        g.f[1].spawn_prot = 0.0;
        g.mt.phase = Phase::Fight;
        g.mt.timer = 99.0;
        g.f[1].loadout.melee = Weapon::Katana;
        g.f[0].pos = v3(-12.0, 0.0, 0.0);
        g.f[1].pos = v3(-10.4, 0.0, 0.0);
        g.f[0].cur = 0;
        g.f[0].loadout.melee = Weapon::Katana;
        g.f[1].cur = 2;
        g.f[1].loadout.gun2 = Weapon::Revolver;
        // A faces B (+x)
        g.f[0].yaw = std::f32::consts::FRAC_PI_2;
        for _ in 0..30 {
            let i = Input { yaw: std::f32::consts::FRAC_PI_2, ..Default::default() };
            g.tick(&i);
        }
        g
    }

    #[test]
    fn dash_slash_launches() {
        let mut g = setup();
        let yaw = std::f32::consts::FRAC_PI_2;
        g.f[1].pos = v3(-9.0, 0.0, 0.0);
        g.tick(&Input { yaw, dash: true, ..Default::default() });
        g.tick(&Input { yaw, fire_pressed: true, ..Default::default() });
        let mut max_vy: f32 = 0.0;
        let mut floated = false;
        for _ in 0..40 {
            g.tick(&Input { yaw, ..Default::default() });
            max_vy = max_vy.max(g.f[1].vel.y);
            floated |= g.f[1].float_t > 0.0;
        }
        assert_eq!(g.f[0].slash_idx, 3, "dash + slash should be the launcher");
        assert!(max_vy > 7.0, "victim should be launched, got {max_vy}");
        assert!(floated, "victim should hang in the air");
    }

    #[test]
    fn jump_cancels_landed_slash() {
        let mut g = setup();
        let yaw = std::f32::consts::FRAC_PI_2;
        g.tick(&Input { yaw, fire_pressed: true, ..Default::default() });
        // wait for the hit frame
        for _ in 0..16 {
            g.tick(&Input { yaw, ..Default::default() });
        }
        assert!(g.f[0].slashing() && g.f[0].slash_hit_done);
        g.tick(&Input { yaw, jump: true, ..Default::default() });
        assert!(!g.f[0].slashing(), "jump should cancel the slash recovery");
        assert!(g.f[0].vel.y > 5.0);
    }

    #[test]
    fn guard_cancel_then_quick_slash() {
        let mut g = setup();
        let yaw = std::f32::consts::FRAC_PI_2;
        g.tick(&Input { yaw, fire_pressed: true, ..Default::default() });
        for _ in 0..16 {
            g.tick(&Input { yaw, ..Default::default() });
        }
        g.tick(&Input { yaw, guard: true, ..Default::default() });
        assert!(!g.f[0].slashing() && g.f[0].guard, "guard should cancel into a block");
        g.tick(&Input { yaw, guard: true, fire_pressed: true, ..Default::default() });
        assert!(g.f[0].slashing(), "slash straight out of the guard (butterfly)");
        assert_eq!(g.f[0].slash_idx, 1);
    }
}
#[cfg(test)]
mod lag_comp_tests {
    use super::*;

    /// Just a floor, so nothing gets in the way.
    pub fn flat_world() -> World {
        World {
            blocks: vec![crate::world::Block { b: Aabb::new(v3(-80.0, -1.0, -80.0), v3(80.0, 0.0, 80.0)), mat: Mat::Floor }],
            spawn_points: vec![(V3::ZERO, 0.0)],
            half: 80.0,
            deco: Vec::new(),
            braziers: Vec::new(),
            banners: Vec::new(),
        }
    }

    #[test]
    fn host_checks_hits_against_the_past() {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g.world = flat_world();
        g.bots[1] = None;
        g.mt.phase = Phase::Fight;
        g.f[0].loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Rifle, gun2: Weapon::Revolver, item: 0 };
        g.f[0].pos = v3(0.0, 0.0, -10.0);
        g.f[1].pos = v3(0.0, 0.0, 0.0);
        g.f[0].spawn_prot = 0.0;
        g.f[1].spawn_prot = 0.0;
        // the target runs sideways for half a second
        for k in 0..60 {
            g.f[1].pos = v3(k as f32 * 0.1, 0.0, 0.0);
            g.f[1].vel = V3::ZERO;
            g.tick(&Input { yaw: 0.0, ..Default::default() });
            if k % 10 == 0 || k > 55 {
                println!("k {k} t {:.3} pos {:?} hist {:?} pose-hips {:?} posehist {:?}", g.time, g.f[1].pos, g.history[1].back(), g.hit_poses[1].map(|p| p.caps[4].a), g.pose_history[1].back().map(|e| (e.0, e.1.caps[4].a)));
            }
            g.f[1].pos = v3(k as f32 * 0.1, 0.0, 0.0);
        }
        g.f[0].ping_ms = 200;
        let o = g.f[0].pos + v3(0.0, 1.4, 0.0);
        // where the target was about a quarter second ago (x ~ 3.4), not where it is now (x ~ 5.9)
        let past = g.pos_at(1, g.time - 0.26);
        assert!(past.x < 4.5, "history should remember the past, got {:?}", past);
        let dir_past = (past + v3(0.0, 1.0, 0.0) - o).norm();
        let dir_now = (g.f[1].pos + v3(0.0, 1.0, 0.0) - o).norm();
        assert!(g.validate_claim(0, 1, Weapon::Rifle as u8, 34.0, o, dir_past, false).is_some(), "a hit on what the shooter saw counts");
        assert!(g.validate_claim(0, 1, Weapon::Rifle as u8, 48.0, o, v3(-1.0, 0.0, 0.0), false).is_none(), "a miss is thrown out");
        assert!(g.validate_claim(0, 1, Weapon::Rifle as u8, 500.0, o, dir_past, false).is_none(), "impossible damage is thrown out");
        assert!(g.validate_claim(0, 1, Weapon::Smg as u8, 9.0, o, dir_past, false).is_none(), "a gun you don't carry is thrown out");
        assert!(g.validate_claim(0, 1, Weapon::Rifle as u8, 48.0, o + v3(30.0, 0.0, 0.0), dir_now, false).is_none(), "shots from far away are thrown out");
        // a headshot claim: aimed at the head as it was then holds up, aimed at the legs it doesn't
        // what a 200 ms player saw: their ping plus their playback delay ago
        let head_then = g.pose_at(1, g.time - (0.2 + INTERP_DELAY + 0.015)).caps[0];
        let at_head = ((head_then.a + head_then.b) * 0.5 - o).norm();
        assert_eq!(g.validate_claim(0, 1, Weapon::Rifle as u8, 68.0, o, at_head, true).map(|c| c.head_ok), Some(true), "a real headshot counts as one");
        let at_legs = (past + v3(0.0, 0.5, 0.0) - o).norm();
        assert_eq!(g.validate_claim(0, 1, Weapon::Rifle as u8, 68.0, o, at_legs, true).map(|c| c.head_ok), Some(false), "a leg shot claimed as a headshot counts as a body shot");
    }
}
#[cfg(test)]
mod mode_tests {
    use super::*;

    fn game(mode: Mode, bots: usize) -> Game {
        let mut g = Game::offline("P", bots, 0.5, mode, 5);
        for i in 0..g.f.len() {
            g.bots[i] = None; // dummies
        }
        g.f[0].ready = true;
        // through loadout and countdown into the fight
        for _ in 0..(4.0 / TICK) as usize {
            g.tick(&Input::default());
        }
        assert_eq!(g.mt.phase, Phase::Fight);
        g
    }

    fn kill(g: &mut Game, victim: usize, killer: usize) {
        g.f[victim].last_attacker = Some(killer);
        g.f[victim].hp = 0.0;
        g.tick(&Input::default());
    }

    #[test]
    fn duel_winner_stays_loser_queues() {
        let mut g = game(Mode::Duel, 3);
        let d = g.mt.duel;
        assert!(d[0] != 255 && d[1] != 255, "two fighters");
        let watching = g.f.iter().filter(|f| f.present && f.spectate).count();
        assert_eq!(watching, 2, "the other two wait");
        let (a, b) = (d[0] as usize, d[1] as usize);
        kill(&mut g, b, a);
        assert_eq!(g.f[a].wins, 1);
        for _ in 0..(3.5 / TICK) as usize {
            g.tick(&Input::default());
        }
        assert!(g.duel_side(a).is_some(), "the winner stays on");
        assert!(g.duel_side(b).is_none(), "the loser goes to the back of the line");
        assert!(g.queue.back() == Some(&b));
        assert!(g.f[b].spectate);
    }

    #[test]
    fn last_man_standing_has_no_respawns() {
        let mut g = game(Mode::Lms, 2);
        kill(&mut g, 1, 0);
        for _ in 0..(5.0 / TICK) as usize {
            g.tick(&Input::default());
            if g.mt.round_end > 0.0 {
                break;
            }
        }
        assert!(!g.f[1].alive, "no respawn during the round");
        kill(&mut g, 2, 0);
        assert_eq!(g.f[0].wins, 1, "last one standing wins the round");
        for _ in 0..(3.5 / TICK) as usize {
            g.tick(&Input::default());
        }
        assert!(g.f.iter().filter(|f| f.present).all(|f| f.alive), "everyone back for the next round");
        assert_eq!(g.mt.round, 2);
    }

    #[test]
    fn gladiator_is_melee_only() {
        let mut g = game(Mode::Gladiator, 1);
        g.tick(&Input { select: Some(1), ..Default::default() });
        g.tick(&Input::default());
        assert!(g.f[0].weapon.is_melee());
    }

    #[test]
    fn capture_the_flag() {
        let mut g = game(Mode::Ctf, 1);
        let red = (0..g.f.len()).find(|&i| g.f[i].present && g.f[i].team == 0).unwrap();
        g.f[red].spawn_prot = 0.0;
        g.f[red].pos = flag_base(1);
        g.tick(&Input::default());
        assert_eq!(g.mt.flags[1].state, 1, "touching the enemy flag picks it up");
        g.f[red].pos = flag_base(0);
        g.tick(&Input::default());
        assert_eq!(g.mt.team_score[0], 1, "bringing it home scores");
        assert_eq!(g.mt.flags[1].state, 0, "and the flag goes back");
    }
}
#[cfg(test)]
mod training_tests {
    use super::*;

    fn run(g: &mut Game, secs: f32, inp: Input) {
        for _ in 0..(secs / TICK) as usize {
            g.tick(&inp);
        }
    }

    #[test]
    fn lessons_advance_and_dummies_never_die() {
        let mut g = Game::training("P");
        assert_eq!(g.mt.phase, Phase::Fight);
        assert!(g.f[1].present && g.f[2].present && g.f[3].present);
        let yaw = g.f[0].yaw;
        run(&mut g, 0.3, Input { yaw, ..Default::default() });
        // three jumps finish lesson 1
        for _ in 0..3 {
            g.f[0].jump_buf = 0.0;
            g.tick(&Input { yaw, jump: true, ..Default::default() });
            run(&mut g, 1.2, Input { yaw, ..Default::default() });
        }
        assert_eq!(g.train.as_ref().unwrap().stats.jumps, 3);
        run(&mut g, 2.0, Input { yaw, ..Default::default() });
        assert_eq!(g.train.as_ref().unwrap().lesson, 1, "moved on to the dash lesson");
        // a huge hit doesn't kill a dummy, and it counts toward the combo
        let at = g.f[2].chest();
        g.damage(0, 2, 500.0, V3::ZERO, 0.1, false, at, 0, g.f[0].chest(), v3(0.0, 0.0, 1.0));
        run(&mut g, 0.5, Input { yaw, ..Default::default() });
        assert!(g.f[2].alive && g.f[2].hp >= 1.0);
        assert_eq!(g.train.as_ref().unwrap().best_combo, 1);
        // skipping works both ways
        g.train_skip(1);
        assert_eq!(g.train.as_ref().unwrap().lesson, 2);
        g.train_skip(-5);
        assert_eq!(g.train.as_ref().unwrap().lesson, 0);
    }
}

/// Where a remote fighter should be shown now: between the two updates around (now - delay) on
/// the sender's clock, or carried on a little past the newest one when updates are late.
/// Returns position, yaw, pitch and the sword swing at that moment. Old updates are dropped.
fn playback(ff: &mut Fighter, now: f32) -> Option<(V3, f32, f32, Option<(f32, u8)>)> {
    let off = ff.clock_off?;
    if ff.snaps.len() < 2 {
        return None;
    }
    let rt = now - off - ff.interp;
    while ff.snaps.len() > 2 && ff.snaps[1].t <= rt {
        ff.snaps.pop_front();
    }
    let a = ff.snaps[0];
    let b = ff.snaps[1];
    let last = *ff.snaps.back()?;
    let swing = |s: &Snap, at: f32| {
        let left = s.slash_t - (at - s.t).max(0.0);
        (left.max(0.0), s.slash_idx)
    };
    if rt >= last.t {
        // late: carry on along the last known velocity for a short while
        let ahead = (rt - last.t).min(0.2);
        let mut p = last.pos + ff.vel * ahead;
        if ff.on_ground {
            p.y = last.pos.y;
        }
        return Some((p, last.yaw, last.pitch, Some(swing(&last, rt))));
    }
    if rt <= a.t {
        return Some((a.pos, a.yaw, a.pitch, Some(swing(&a, rt))));
    }
    let u = ((rt - a.t) / (b.t - a.t).max(1e-4)).clamp(0.0, 1.0);
    let pos = a.pos.lerp(b.pos, u);
    let yaw = a.yaw + wrap_angle(b.yaw - a.yaw) * u;
    let pitch = a.pitch + (b.pitch - a.pitch) * u;
    Some((pos, yaw, pitch, Some(swing(&a, rt))))
}

#[cfg(test)]
mod net_smooth_tests {
    use super::*;

    /// Updates arriving with jitter, some lost and some out of order still give smooth movement.
    #[test]
    fn remote_playback_is_smooth_under_bad_network() {
        let mut f = Fighter::new("R", 1, false);
        f.present = true;
        f.alive = true;
        let mut rng = Rng(77);
        let mut arrivals: Vec<(f32, f32)> = Vec::new(); // (arrives at our time, sent at their time)
        for k in 0..240 {
            let sent = k as f32 / 60.0;
            if rng.f() < 0.05 {
                continue; // lost
            }
            let delay = 0.05 + rng.f() * 0.03; // 50..80 ms each way
            arrivals.push((sent + 10.0 + delay, sent)); // our clock runs 10 s ahead
        }
        arrivals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let speed = 7.0;
        let mut now = 10.0;
        let mut i = 0;
        let mut last_pos: Option<V3> = None;
        let mut worst_step: f32 = 0.0;
        let mut steps = Vec::new();
        while now < 13.5 {
            while i < arrivals.len() && arrivals[i].0 <= now {
                let sent = arrivals[i].1;
                let st = crate::net::FState::test_state(v3(sent * speed, 0.0, 0.0), v3(speed, 0.0, 0.0));
                crate::net::apply_remote(&mut f, &st, false, sent, now);
                i += 1;
            }
            if let Some((p, _, _, _)) = playback(&mut f, now) {
                let err = p - f.pos;
                f.pos = if err.len() > 4.0 { p } else { f.pos + err * 0.6 };
            }
            if now > 11.0 {
                if let Some(lp) = last_pos {
                    let step = (f.pos - lp).len();
                    worst_step = worst_step.max(step);
                    steps.push(step);
                    assert!(f.pos.x + 0.01 >= lp.x, "never moves backwards (rubber-banding)");
                }
            }
            last_pos = Some(f.pos);
            now += TICK;
        }
        let ideal = speed * TICK;
        assert!(worst_step < ideal * 2.2, "no big jumps: worst {worst_step} vs ideal {ideal}");
        let mean: f32 = steps.iter().sum::<f32>() / steps.len() as f32;
        assert!((mean - ideal).abs() < ideal * 0.1, "keeps the right speed: {mean} vs {ideal}");
    }
}
/// One pellet's direction: inside the cone of half-angle atan(spread) around `dir`, spread evenly
/// over its area. The sideways axes are made exactly square to `dir`, so no pellet strays wider.
pub fn pellet_dir(dir: V3, right_hint: V3, spread: f32, rng: &mut Rng) -> V3 {
    if spread <= 0.0 {
        return dir;
    }
    let mut right = right_hint - dir * right_hint.dot(dir);
    if right.len() < 1e-4 {
        right = v3(1.0, 0.0, 0.0) - dir * dir.x;
    }
    let right = right.norm();
    let up = dir.cross(right).norm();
    let a = rng.range(0.0, std::f32::consts::TAU);
    let r = spread * rng.f().sqrt();
    (dir + right * (a.cos() * r) + up * (a.sin() * r)).norm()
}
#[cfg(test)]
mod hitbox_tests {
    use super::*;

    #[test]
    fn ray_meets_capsules() {
        let (a, b) = (v3(0.0, 0.0, 5.0), v3(0.0, 2.0, 5.0));
        let t = ray_capsule(V3::ZERO + v3(0.0, 1.0, 0.0), v3(0.0, 0.0, 1.0), 50.0, a, b, 0.5).unwrap();
        assert!((t - 4.5).abs() < 1e-4, "side: {t}");
        let t = ray_capsule(v3(0.0, 5.0, 5.0), v3(0.0, -1.0, 0.0), 50.0, a, b, 0.5).unwrap();
        assert!((t - 2.5).abs() < 1e-4, "round end: {t}");
        assert!(ray_capsule(v3(0.0, 1.0, 0.0), v3(1.0, 0.0, 0.0), 50.0, a, b, 0.5).is_none(), "miss");
        assert!(ray_capsule(v3(0.0, 1.0, 0.0), v3(0.0, 0.0, 1.0), 4.0, a, b, 0.5).is_none(), "out of range");
    }

    fn standing_target() -> Game {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.bots[1] = None;
        g.mt.phase = Phase::Fight;
        g.f[0].pos = v3(0.0, 0.0, -10.0);
        g.f[1].pos = v3(0.0, 0.0, 0.0);
        g.f[1].yaw = std::f32::consts::PI; // facing the shooter
        g.f[0].spawn_prot = 0.0;
        g.f[1].spawn_prot = 0.0;
        g.f[1].loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Revolver, gun2: Weapon::Rifle, item: 0 };
        g.f[1].equip();
        for _ in 0..30 {
            g.f[1].pos = v3(0.0, 0.0, 0.0);
            g.f[1].vel = V3::ZERO;
            g.tick(&Input { yaw: 0.0, ..Default::default() });
        }
        g
    }

    #[test]
    fn hit_shapes_follow_the_skeleton() {
        let g = standing_target();
        let pose = g.hit_poses[1].expect("posed by the animation");
        let head = pose.caps.iter().find(|c| c.part == Part::Head).unwrap();
        let top = head.a.y.max(head.b.y) + head.r;
        assert!(top > 1.6 && top < 2.05, "head on top: {top}");
        for c in &pose.caps {
            assert!(c.a.y.max(c.b.y) <= top + 1e-3, "nothing above the head");
        }
        let o = v3(0.0, 0.0, -10.0);
        let aim = |p: V3| (p - o).norm();
        let hc = (head.a + head.b) * 0.5;
        let head_y = hc.y;
        assert_eq!(g.ray_fighter(1, o + v3(hc.x, head_y, 0.0), v3(0.0, 0.0, 1.0), 50.0).map(|h| h.1), Some(Part::Head), "a shot at the head is a headshot");
        let knee = pose.caps.iter().find(|c| c.part == Part::LegL).unwrap().b;
        assert!(matches!(g.ray_fighter(1, o, aim(knee), 50.0).map(|h| h.1), Some(Part::LegL) | Some(Part::LegR)), "knee height hits a leg");
        assert!(g.ray_fighter(1, o, aim(v3(0.0, top + 0.15, 0.0)), 50.0).is_none(), "just over the head misses");
        assert!(g.ray_fighter(1, o, aim(v3(0.7, 0.6, 0.0)), 50.0).is_none(), "beside the legs misses");
    }

    #[test]
    fn perfect_parry_staggers_and_counters_hurt_more() {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.bots[1] = None;
        g.mt.phase = Phase::Fight;
        for i in 0..2 {
            g.f[i].ap = 0.0; // no armour soaking up the counter
            g.f[i].loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Revolver, gun2: Weapon::Rifle, item: 0 };
            g.f[i].equip();
            g.f[i].cur = 0;
            g.f[i].weapon = Weapon::Katana;
            g.f[i].spawn_prot = 0.0;
        }
        g.f[0].pos = v3(0.0, 0.0, 0.0);
        g.f[0].yaw = 0.0;
        g.f[1].pos = v3(0.0, 0.0, 1.4);
        g.f[1].yaw = std::f32::consts::PI;
        g.update_hit_poses();
        // the defender raised their guard just now
        g.f[1].guard = true;
        g.f[1].guard_t = 0.05;
        g.f[0].slash_idx = 0;
        g.resolve_slash(0);
        assert!(g.f[0].stagger_t > 0.5 && g.f[0].stun_t > 0.5, "the attacker reels");
        assert_eq!(g.f[1].hp, MAX_HP, "nothing got through");
        // guard held for ages: a normal block, no stagger
        let mut g2 = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g2.world = crate::game::lag_comp_tests::flat_world();
        g2.bots[1] = None;
        for i in 0..2 {
            g2.f[i] = g.f[i].clone();
        }
        g2.f[0].stun_t = 0.0;
        g2.f[0].stagger_t = 0.0;
        g2.f[1].guard_t = 1.0;
        g2.update_hit_poses();
        g2.resolve_slash(0);
        assert_eq!(g2.f[0].stagger_t, 0.0, "a late guard only blocks");
        // a counter on the staggered attacker does 1.5x
        let hp_before = g.f[0].hp;
        g.f[1].guard = false;
        g.f[1].slash_idx = 0;
        g.f[1].yaw = std::f32::consts::PI;
        g.update_hit_poses();
        g.resolve_slash(1);
        let lost = hp_before - g.f[0].hp;
        let normal = 18.0 * Weapon::Katana.melee_mult();
        assert!(lost > normal * 1.4, "counter: lost {lost} vs normal {normal}");
    }

    #[test]
    fn hit_ticks_climb_in_pitch() {
        let mut g = standing_target();
        let mut pitches = Vec::new();
        for _ in 0..4 {
            g.sounds.clear();
            let at = g.f[1].chest();
            g.damage(0, 1, 5.0, V3::ZERO, 0.0, false, at, Weapon::Revolver as u8, g.f[0].pos, v3(0.0, 0.0, 1.0));
            pitches.push(g.sounds.iter().find(|s| s.0 == Sfx::Hit).map(|s| s.2).unwrap());
            for _ in 0..20 {
                g.tick(&Input::default());
            }
        }
        assert!(pitches.windows(2).all(|w| w[1] > w[0]), "rising: {pitches:?}");
        for _ in 0..240 {
            g.tick(&Input::default());
        }
        g.sounds.clear();
        let at = g.f[1].chest();
        g.damage(0, 1, 5.0, V3::ZERO, 0.0, false, at, Weapon::Revolver as u8, g.f[0].pos, v3(0.0, 0.0, 1.0));
        assert_eq!(g.sounds.iter().find(|s| s.0 == Sfx::Hit).map(|s| s.2), Some(1.0), "back to the start after a pause");
    }

    #[test]
    fn posing_everyone_is_cheap() {
        let mut g = Game::offline("A", 7, 0.5, Mode::Ffa, 50);
        g.mt.phase = Phase::Fight;
        for i in 0..8 {
            g.f[i].spawn_prot = 0.0;
        }
        let t0 = std::time::Instant::now();
        for _ in 0..120 {
            g.update_hit_poses();
        }
        let per_tick = t0.elapsed().as_secs_f32() / 120.0;
        println!("posing 8 fighters: {:.3} ms per tick", per_tick * 1000.0);
        assert!(per_tick < 0.004, "too slow: {per_tick}");
    }
}

#[cfg(test)]
mod movement_tests {
    use super::*;

    fn arena() -> Game {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.bots[1] = None;
        g.mt.phase = Phase::Fight;
        g.f[0].pos = v3(0.0, 0.0, 0.0);
        g.f[0].vel = V3::ZERO;
        g.f[0].spawn_prot = 0.0;
        g.f[1].pos = v3(30.0, 0.0, 30.0); // out of the way
        g.f[1].spawn_prot = 0.0;
        g.f[0].on_ground = true;
        g
    }
    fn run(g: &mut Game, secs: f32, inp: Input) {
        for _ in 0..(secs / TICK).round() as usize {
            g.tick(&inp);
        }
    }
    fn speed(g: &Game) -> f32 {
        g.f[0].vel.flat().len()
    }
    const FWD: Input = Input { fwd: 1.0, right: 0.0, jump: false, dash: false, fire: false, fire_pressed: false, guard: false, select: None, reload: false, item: false, crouch: false, yaw: 0.0, pitch: 0.0 };

    #[test]
    fn landing_from_a_dash_keeps_speed_then_eases() {
        let mut g = arena();
        run(&mut g, 0.5, FWD);
        g.tick(&Input { dash: true, ..FWD });
        // through the dash hop to the landing
        let mut landed_at = None;
        let mut speeds = Vec::new();
        for k in 0..150 {
            g.tick(&FWD);
            speeds.push(speed(&g));
            if landed_at.is_none() && g.f[0].on_ground && g.f[0].dash_t <= 0.0 {
                landed_at = Some(k);
            }
        }
        let l = landed_at.expect("landed");
        let after = |secs: f32| speeds[l + (secs / TICK) as usize];
        assert!(after(0.3) > RUN_SPEED * 1.25, "still fast 0.3 s after landing: {}", after(0.3));
        // no snapping: speed never drops by much in one tick
        for w in speeds[l..].windows(2) {
            assert!(w[0] - w[1] < 0.6, "eases off smoothly: {} -> {}", w[0], w[1]);
        }
        assert!((speeds[speeds.len() - 1] - RUN_SPEED * g.f[0].flow_mult()).abs() < 0.3, "back to running speed in the end");
    }

    #[test]
    fn chained_moves_build_flow_and_reset() {
        let mut g = arena();
        run(&mut g, 0.4, FWD);
        g.tick(&Input { dash: true, ..FWD }); // ground dash
        assert_eq!(g.f[0].flow, 1);
        // land from the dash hop, then jump straight away
        for _ in 0..120 {
            if g.f[0].on_ground && g.f[0].dash_t <= 0.0 {
                break;
            }
            g.tick(&FWD);
        }
        g.tick(&Input { jump: true, ..FWD });
        assert!(!g.f[0].on_ground, "jumped");
        let before = g.f[0].flow;
        run(&mut g, 0.25, FWD);
        g.tick(&Input { dash: true, ..FWD }); // air dash
        assert_eq!(g.f[0].flow, before + 1, "the air dash chains on");
        assert!(g.f[0].flow >= 2);
        let fast = g.f[0].flow_mult();
        assert!((fast - (1.0 + FLOW_STEP * g.f[0].flow as f32)).abs() < 1e-4);
        // getting hit breaks it
        g.apply_hit(0, Some(1), 5.0, V3::ZERO, 0.0, false, 4, None);
        assert_eq!(g.f[0].flow, 0, "a hit resets the chain");
        // build again and stand still
        g.f[0].flow = 3;
        run(&mut g, 2.0, Input::default());
        assert_eq!(g.f[0].flow, 0, "standing still resets it");
        g.f[0].flow = 9;
        assert!((g.f[0].flow_mult() - 1.3).abs() < 1e-4, "capped at +30%");
    }

    #[test]
    fn perfect_timing_jump_goes_higher() {
        let mut g = arena();
        // drop from a height, jump the instant we land
        g.f[0].pos = v3(0.0, 2.0, 0.0);
        g.f[0].on_ground = false;
        let mut jumped_vy = 0.0;
        for _ in 0..200 {
            let landed_now = g.f[0].on_ground && g.f[0].land_t < 0.05;
            g.tick(&Input { jump: landed_now, ..Default::default() });
            if landed_now {
                jumped_vy = g.f[0].vel.y;
                break;
            }
        }
        assert!(jumped_vy > JUMP_VEL * 1.05, "perfect jump: {jumped_vy}");
        assert!(g.f[0].perfect_t > 0.0);
        // a late jump is a normal one
        let mut g2 = arena();
        run(&mut g2, 0.5, Input::default());
        g2.tick(&Input { jump: true, ..Default::default() });
        assert!(g2.f[0].vel.y <= JUMP_VEL + 0.01, "normal jump: {}", g2.f[0].vel.y);
    }

    #[test]
    fn wall_run_keeps_entry_speed() {
        let mut g = arena();
        // a long wall along +z on our left
        g.world.blocks.push(crate::world::Block { b: Aabb::new(v3(-3.0, 0.0, -20.0), v3(-2.0, 6.0, 40.0)), mat: Mat::Wall });
        g.f[0].pos = v3(-1.5, 0.6, 0.0);
        g.f[0].on_ground = false;
        g.f[0].vel = v3(0.0, 2.0, 15.0);
        g.f[0].yaw = 0.0;
        g.tick(&Input { jump: true, yaw: 0.0, ..Default::default() });
        let w = g.f[0].wall.expect("running on the wall");
        assert!((w.speed - 15.0).abs() < 1.0, "runs as fast as it arrived: {}", w.speed);
        g.tick(&Input { yaw: 0.0, ..Default::default() });
        assert!(g.f[0].vel.flat().len() > 13.0, "and keeps that speed: {}", g.f[0].vel.flat().len());
        // arriving slowly still gives the minimum
        let mut g2 = arena();
        g2.world = g.world.clone_world();
        g2.f[0].pos = v3(-1.5, 0.6, 0.0);
        g2.f[0].on_ground = false;
        g2.f[0].vel = v3(0.0, 2.0, 3.0);
        g2.tick(&Input { jump: true, yaw: 0.0, ..Default::default() });
        assert!((g2.f[0].wall.expect("on the wall").speed - 10.0).abs() < 1e-3);
    }

    #[test]
    fn slide_keeps_speed_and_jump_cancels() {
        let mut g = arena();
        run(&mut g, 0.6, FWD);
        let before = speed(&g);
        g.tick(&Input { crouch: true, ..FWD });
        assert!(g.f[0].sliding > 0.0, "sliding");
        run(&mut g, 0.3, Input { crouch: true, ..FWD });
        assert!(speed(&g) > before, "faster than running: {} vs {}", speed(&g), before);
        let sliding_speed = speed(&g);
        g.tick(&Input { crouch: true, jump: true, ..FWD });
        assert!(!g.f[0].on_ground && g.f[0].sliding == 0.0, "jumped out of the slide");
        assert!(speed(&g) > sliding_speed - 0.5, "keeping the speed");
        // too slow to slide: crouch does nothing
        let mut g2 = arena();
        g2.tick(&Input { crouch: true, ..Default::default() });
        assert_eq!(g2.f[0].sliding, 0.0);
    }

    #[test]
    fn a_kill_gives_health_back() {
        let mut g = arena();
        g.f[0].hp = 40.0;
        g.f[1].last_attacker = Some(0);
        g.f[1].hp = 0.0;
        run(&mut g, 2.5, Input::default());
        assert!(g.f[0].hp >= 40.0 + KILL_HEAL - 0.5 && g.f[0].hp <= 40.0 + KILL_HEAL + 0.5, "healed: {}", g.f[0].hp);
    }
}

#[cfg(test)]
mod weapon_tests {
    use super::*;

    /// A target at the origin with full health and full armour, and a shooter holding `gun`.
    fn range(gun: Weapon) -> Game {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.bots[1] = None;
        g.mt.phase = Phase::Fight;
        g.f[0].loadout = Loadout { melee: Weapon::Katana, gun1: gun, gun2: Weapon::Revolver, item: 0 };
        g.f[0].equip();
        g.f[0].pos = v3(0.0, 0.0, -10.0);
        g.f[1].loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Shotgun, gun2: Weapon::Smg, item: 0 };
        g.f[1].equip();
        for i in 0..2 {
            g.f[i].spawn_prot = 0.0;
        }
        for _ in 0..20 {
            g.f[1].pos = V3::ZERO;
            g.f[1].vel = V3::ZERO;
            g.f[1].yaw = std::f32::consts::PI;
            g.tick(&Input::default());
        }
        g.f[1].hp = MAX_HP;
        g.f[1].ap = MAX_AP;
        g
    }

    /// A shot (origin, direction) whose first hit on the target is capsule `cap`.
    fn shot_at(g: &Game, cap: usize) -> (V3, V3) {
        let pose = g.hit_pose(1);
        let c = pose.caps[cap];
        let centre = (c.a + c.b) * 0.5;
        for tilt in [0.0f32, -0.4, 0.4, -0.8] {
            for k in 0..16 {
                let a = k as f32 / 16.0 * std::f32::consts::TAU;
                let d = v3(a.sin() * tilt.cos(), tilt.sin(), a.cos() * tilt.cos()).norm();
                let o = centre - d * 5.0;
                if pose.ray_cap(o, d, 10.0, 0.0).map(|h| h.1) == Some(cap) {
                    return (o, d);
                }
            }
        }
        panic!("no clear shot at capsule {cap}");
    }

    fn fire(g: &mut Game, w: Weapon, cap: usize) {
        let (o, d) = shot_at(g, cap);
        g.fire_ray(0, o, d, w.gun().dmg, w.gun().range, w);
        g.tick(&Input::default()); // deaths are settled in the tick
    }

    #[test]
    fn hand_cannon_headshot_always_kills() {
        let mut g = range(Weapon::HandCannon);
        fire(&mut g, Weapon::HandCannon, 0);
        assert!(!g.f[1].alive, "one headshot through full armour kills");
        let mut g = range(Weapon::HandCannon);
        fire(&mut g, Weapon::HandCannon, 2);
        assert!(g.f[1].alive, "a body shot doesn''t");
        assert!(g.f[1].hp < MAX_HP && g.f[1].ap < MAX_AP, "but it hurts (armour soaks some)");
    }

    #[test]
    fn sniper_kills_on_the_body_not_hands_or_feet() {
        // head, neck, chest, shoulders, hips, upper arms, thighs
        for cap in [0usize, 2, 4, 5, 7, 9, 11] {
            let mut g = range(Weapon::Sniper);
            fire(&mut g, Weapon::Sniper, cap);
            assert!(!g.f[1].alive, "capsule {cap} ({:?}) is a one-shot kill", g.hit_pose(1).caps[cap].part);
        }
        // forearms/hands and shins/feet
        for cap in [6usize, 8, 10, 12] {
            let mut g = range(Weapon::Sniper);
            fire(&mut g, Weapon::Sniper, cap);
            assert!(g.f[1].alive, "capsule {cap} isn''t a kill");
            let lost = (MAX_HP - g.f[1].hp) + (MAX_AP - g.f[1].ap);
            assert!((lost - SNIPER_LIMB).abs() < 1.0, "a limb hit does 60 (lost {lost})");
        }
    }

    #[test]
    fn sniper_is_dead_on_only_when_scoped() {
        let mut f = Fighter::new("T", 0, true);
        f.weapon = Weapon::Sniper;
        f.scope_t = 0.0;
        assert!(f.spread() > 0.05, "wild from the hip");
        f.scope_t = SCOPE_IN;
        assert_eq!(f.spread(), 0.0, "exact once the scope is up");
    }

    #[test]
    fn rocket_tops_out_at_55() {
        assert_eq!(Weapon::Rocket.max_hit(), 55.0);
        let mut g = range(Weapon::Rocket);
        g.f[1].ap = 0.0;
        let c = g.f[1].chest();
        g.explode(c, 0, 1);
        assert!((MAX_HP - g.f[1].hp - 55.0).abs() < 0.01, "dead centre: {}", MAX_HP - g.f[1].hp);
        let mut g = range(Weapon::Rocket);
        g.f[1].ap = 0.0;
        let c = g.f[1].chest();
        g.explode(c + v3(2.5, 0.0, 0.0), 0, 1);
        let lost = MAX_HP - g.f[1].hp;
        assert!(lost > 0.0 && lost < 20.0, "falls off fast: {lost} at 2.5 m");
        let mut g = range(Weapon::Rocket);
        let c = g.f[1].chest();
        g.explode(c + v3(3.7, 0.0, 0.0), 0, 1);
        assert_eq!(g.f[1].hp, MAX_HP, "nothing past 3.5 m");
    }

    #[test]
    fn balance_numbers() {
        assert_eq!(Weapon::Rifle.mag(), 10);
        assert!((Weapon::Rifle.interval() - 0.45).abs() < 1e-6);
        assert_eq!(Weapon::Rifle.gun().dmg, 34.0);
        assert_eq!(Weapon::Rifle.gun().head, 2.0);
        assert_eq!(Weapon::HandCannon.mag(), 7);
        assert_eq!(Weapon::Sniper.mag(), 5);
        assert!((Weapon::Sniper.interval() - 1.3).abs() < 1e-6 && (Weapon::Sniper.reload_time() - 2.8).abs() < 1e-6);
        assert!((Weapon::Rocket.reload_time() - 2.6).abs() < 1e-6);
        assert_eq!(Weapon::Smg.zoom(), 1.0, "no zoom on the SMG");
        assert_eq!(Weapon::Sniper.zoom(), 3.0);
        assert!(Weapon::Smg.gun().spread_still <= 0.02 * 0.61);
    }
}
