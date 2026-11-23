use crate::bot::Bot;
use crate::math::*;
pub use crate::world::MapId;
use crate::world::{Mat, World};
use std::collections::VecDeque;

pub const RADIUS: f32 = 0.36;
pub const HEIGHT: f32 = 1.8;
pub const GRAVITY: f32 = 25.0;
pub const JUMP_VEL: f32 = 9.4;
/// Base running speed: flow chains and dashes are where the real speed comes from.
pub const RUN_SPEED: f32 = 6.6;
pub const MAX_HP: f32 = 100.0;
pub const MAX_AP: f32 = 50.0;
pub const TICK: f32 = 1.0 / 120.0;
pub const MAX_PLAYERS: usize = 8;
pub const RESPAWN_TIME: f32 = 3.0;
pub const SPAWN_PROT: f32 = 1.5;
/// A match lasts at most six minutes.
pub const MATCH_TIME: f32 = 360.0;
/// Kills in a row (without dying) that earn a Writ (Free-for-All and Team Deathmatch).
pub const WRIT_STREAK: u32 = 3;
/// Play of the Match: how much of the moment is kept before and after it, and how often a frame
/// is kept.
pub const POTM_BEFORE: f32 = 4.0;
pub const POTM_AFTER: f32 = 1.5;
pub const REC_DT: f32 = 1.0 / 30.0;

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
    Shuriken = 10,
}

/// Number of weapons (ids 0..N_WEAPONS); kill-feed ids at or above it are other things.
pub const N_WEAPONS: u8 = 11;
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
/// Throwing stars: flight speed, how long one flies, stars per click (thrown this far apart),
/// and the fan (right click): its spread and the wait after it.
pub const STAR_SPEED: f32 = 62.0;
pub const STAR_LIFE: f32 = 1.4;
pub const STAR_BURST: u8 = 3;
pub const STAR_GAP: f32 = 0.06;
pub const FAN_ANGLE: f32 = 0.12;
pub const FAN_CD: f32 = 0.75;
/// The throwing motion: the arm pulls in across the chest and whips out (a burst), or sweeps
/// wide (the fan).
pub const THROW_TIME: f32 = 0.24;
pub const FAN_THROW_TIME: f32 = 0.28;
/// A thrown star is drawn leaving the hand, easing onto its true path over this long.
pub const STAR_VIS_TIME: f32 = 0.12;

/// The scattergun's pellets, always in this pattern (units of its spread; x right, y up): one
/// in the middle, a tight square of four round it, and a ring of five. The same every shot, so
/// where you aim decides what it does; the crosshair draws it.
pub const SHOTGUN_PATTERN: [(f32, f32); 10] = [
    (0.0, 0.0),
    (0.23, 0.23),
    (-0.23, 0.23),
    (0.23, -0.23),
    (-0.23, -0.23),
    (0.0, 1.0),
    (0.951, 0.309),
    (0.588, -0.809),
    (-0.588, -0.809),
    (-0.951, 0.309),
];
/// One shell loaded into the scattergun (it reloads shell by shell; shooting stops it).
pub const SHELL_TIME: f32 = 0.5;
/// The pump: how long after a shot it racks (fire_cd left when it does, the slide's travel).
pub const PUMP_AT: f32 = 0.58;
/// Single-shot guns: a bullet passing this close to a head counts as a headshot.
pub const HEAD_ASSIST: f32 = 0.05;

/// The scattergun's damage at distance `t`: full to 5 m, 40% by 18 m, 15% from 30 m on.
pub fn shotgun_falloff(t: f32) -> f32 {
    if t <= 5.0 {
        1.0
    } else if t <= 18.0 {
        1.0 - 0.6 * (t - 5.0) / 13.0
    } else {
        (0.4 - 0.25 * (t - 18.0) / 12.0).max(0.15)
    }
}

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
    pub const GUNS: [Weapon; 8] = [Weapon::Shotgun, Weapon::Revolver, Weapon::HandCannon, Weapon::Smg, Weapon::Rifle, Weapon::Sniper, Weapon::Rocket, Weapon::Shuriken];

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
            10 => Weapon::Shuriken,
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
            Weapon::Shuriken => "Shuriken",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Weapon::Dagger => "fast, short reach, light hits",
            Weapon::Katana => "balanced",
            Weapon::Greatsword => "slow, long reach, every hit breaks guards",
            Weapon::Shotgun => "pump: 10 pellets in a fixed pattern, aim decides",
            Weapon::Revolver => "a headshot kills; tap it, don't spam it",
            Weapon::Smg => "hold to spray, 30 rounds",
            Weapon::Rifle => "semi-auto marksman, 10 rounds",
            Weapon::Rocket => "explodes, rocket-jump with it",
            Weapon::HandCannon => "7 heavy rounds, a headshot always kills",
            Weapon::Sniper => "scope up (right click): one hit to the body kills",
            Weapon::Shuriken => "12 throwing stars: click throws 3 fast, right-click fans 3",
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
            Weapon::Shuriken => 12,
            _ => 0,
        }
    }
    pub fn interval(self) -> f32 {
        match self {
            Weapon::Shotgun => 0.9,
            Weapon::Revolver => 0.30,
            Weapon::Smg => 0.075,
            Weapon::Rifle => 0.45,
            Weapon::Rocket => 0.9,
            Weapon::HandCannon => 0.4,
            Weapon::Sniper => 1.3,
            // from one 3-star burst to the next (the burst itself takes 0.12 s)
            Weapon::Shuriken => 0.55,
            _ => 0.0,
        }
    }
    pub fn reload_time(self) -> f32 {
        match self {
            // per shell: it loads one at a time
            Weapon::Shotgun => SHELL_TIME,
            Weapon::Revolver => 1.5,
            Weapon::Smg => 1.8,
            Weapon::Rifle => 2.2,
            Weapon::Rocket => 2.6,
            Weapon::HandCannon => 1.9,
            Weapon::Sniper => 2.8,
            Weapon::Shuriken => 2.0,
            _ => 0.0,
        }
    }
    pub fn gun(self) -> GunStats {
        match self {
            Weapon::Shotgun => GunStats { pellets: 10, spread_still: 0.07, move_cap: 1.0, bloom: 0.0, dmg: 11.0, range: 40.0, head: 2.0, knock: 1.0 },
            Weapon::Smg => GunStats { pellets: 1, spread_still: 0.012, move_cap: 1.5, bloom: 0.25, dmg: 9.0, range: 80.0, head: 1.4, knock: 0.4 },
            Weapon::Rifle => GunStats { pellets: 1, spread_still: 0.003, move_cap: 2.0, bloom: 0.6, dmg: 34.0, range: 200.0, head: 2.0, knock: 2.0 },
            Weapon::HandCannon => GunStats { pellets: 1, spread_still: 0.012, move_cap: 1.5, bloom: 1.5, dmg: 45.0, range: 160.0, head: 1.0, knock: 2.6 },
            // unscoped the sniper sprays wide; scoped it's exact (see Fighter::spread)
            Weapon::Sniper => GunStats { pellets: 1, spread_still: 0.09, move_cap: 1.0, bloom: 0.0, dmg: SNIPER_LIMB, range: 300.0, head: 1.0, knock: 4.0 },
            // throwing stars fly true (no spread) but take a moment to arrive; each hits hard
            Weapon::Shuriken => GunStats { pellets: 1, spread_still: 0.0, move_cap: 1.0, bloom: 0.0, dmg: 32.0, range: STAR_SPEED * STAR_LIFE, head: 2.0, knock: 1.0 },
            // the revolver: a headshot kills anyone without armour; body shots take four
            _ => GunStats { pellets: 1, spread_still: 0.004, move_cap: 1.5, bloom: 0.35, dmg: 28.0, range: 150.0, head: 4.0, knock: 1.6 },
        }
    }
    /// The most one hit can do (used by the host to throw out impossible hits).
    pub fn max_hit(self) -> f32 {
        match self {
            // a charged heavy is the biggest swing; the dagger doubles it from behind
            Weapon::Dagger => 34.0 * 0.72 * 2.0,
            Weapon::Katana => 34.0,
            Weapon::Greatsword => 34.0 * 1.55,
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
            Weapon::Revolver => 0.5,
            Weapon::Rifle => 0.6,
            _ => REST_TIME,
        }
    }
    /// Firing fast stacks up inaccuracy (like tapping versus spamming a pistol in a tactical
    /// shooter): how much each shot adds, and how much spread one unit of it is worth. It drains
    /// at SPAM_RECOVER per second.
    pub fn spam(self) -> (f32, f32) {
        match self {
            Weapon::Revolver => (1.0, 0.045),
            Weapon::HandCannon => (1.0, 0.05),
            _ => (0.0, 0.0),
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

/// What a player brings: a melee weapon, two guns, an item, and which fighter they play as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loadout {
    pub melee: Weapon,
    pub gun1: Weapon,
    pub gun2: Weapon,
    pub item: u8,
    /// The fighter (see crate::chars): 0 Ranger, 1 Huntress, 2 Brawler, 3 Striker.
    pub body: u8,
    /// Their outfit colour (see career::OUTFITS), unlocked through mastery.
    pub outfit: u8,
}

impl Default for Loadout {
    fn default() -> Self {
        Loadout { melee: Weapon::Katana, gun1: Weapon::Shotgun, gun2: Weapon::Revolver, item: 2, body: 0, outfit: 0 }
    }
}

impl Loadout {
    pub fn to_bytes(self) -> [u8; 4] {
        // the fighter rides in the item byte's top half
        // the outfit rides in the blade byte's top half
        [(self.melee as u8) | (self.outfit.min(15) << 4), self.gun1 as u8, self.gun2 as u8, (self.item & 0x0f) | (self.body.min(15) << 4)]
    }
    /// Read and repair (a melee in the melee slot, guns in the gun slots).
    pub fn from_bytes(b: [u8; 4]) -> Loadout {
        let mut l = Loadout { outfit: (b[0] >> 4).min(3), melee: Weapon::from_u8(b[0] & 0x0f), gun1: Weapon::from_u8(b[1]), gun2: Weapon::from_u8(b[2]), item: (b[3] & 0x0f).min(2), body: (b[3] >> 4).min(crate::chars::COUNT as u8 - 1) };
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
        Loadout { melee: m, gun1: Weapon::GUNS[a], gun2: Weapon::GUNS[b], item: (rng.next_u32() % 3) as u8, body: (rng.next_u32() % crate::chars::COUNT as u32) as u8, outfit: 0 }
    }
}

/// What killed someone, for the kill feed (weapons by number, GRENADE_TOOL for grenades).
pub fn kill_tool(w: u8) -> &'static str {
    if w == GRENADE_TOOL {
        "Grenade"
    } else if w == KICK_TOOL {
        "Kick"
    } else if w == COMBO_TOOL {
        "Combo KO"
    } else if w == EXEC_TOOL {
        "EXECUTED"
    } else if w < N_WEAPONS {
        Weapon::from_u8(w).name()
    } else {
        "?"
    }
}

// ---- melee depth: guard meter, kicks, deflects, charged heavies
/// A kick, as a "weapon" number (kill feed, host checks).
pub const KICK_TOOL: u8 = 21;
/// A finisher, as a "weapon" number (kill feed: EXECUTED).
pub const EXEC_TOOL: u8 = 23;
/// Finishers: how close, and how hurt the target must be (and staggered, guard-broken or launched).
pub const FINISH_RANGE: f32 = 2.6;
pub const FINISH_HP: f32 = 25.0;
/// What a finisher gives back.
pub const FINISH_HEAL: f32 = 50.0;

/// One finisher animation. `weapon` None is the aerial one (any blade, target in the air).
/// `unlock_level` is the career level it becomes available at (all 1 for now; they're meant
/// to become cosmetics).
pub struct FinisherStyle {
    pub name: &'static str,
    pub weapon: Option<Weapon>,
    /// Seconds it lasts, and when (share of that) the blow lands.
    pub dur: f32,
    pub hit: f32,
    pub unlock_level: u32,
}

pub const FINISHERS: [FinisherStyle; 10] = [
    FinisherStyle { name: "Throat Cut", weapon: Some(Weapon::Dagger), dur: 0.7, hit: 0.55, unlock_level: 1 },
    FinisherStyle { name: "Twin Stab", weapon: Some(Weapon::Dagger), dur: 0.8, hit: 0.72, unlock_level: 1 },
    FinisherStyle { name: "Spinning Edge", weapon: Some(Weapon::Dagger), dur: 0.75, hit: 0.62, unlock_level: 1 },
    FinisherStyle { name: "Quick Draw", weapon: Some(Weapon::Katana), dur: 0.8, hit: 0.55, unlock_level: 1 },
    FinisherStyle { name: "Cross Cut", weapon: Some(Weapon::Katana), dur: 0.85, hit: 0.75, unlock_level: 1 },
    FinisherStyle { name: "Rising Cleave", weapon: Some(Weapon::Katana), dur: 0.8, hit: 0.6, unlock_level: 1 },
    FinisherStyle { name: "Overhead Crush", weapon: Some(Weapon::Greatsword), dur: 0.95, hit: 0.65, unlock_level: 1 },
    FinisherStyle { name: "Impale", weapon: Some(Weapon::Greatsword), dur: 0.9, hit: 0.6, unlock_level: 1 },
    FinisherStyle { name: "Wheel Cleave", weapon: Some(Weapon::Greatsword), dur: 0.95, hit: 0.7, unlock_level: 1 },
    FinisherStyle { name: "Sky Splitter", weapon: None, dur: 0.9, hit: 0.7, unlock_level: 1 },
];

/// A finisher style (by its number + 1, as fighters carry it; 0 = none).
pub fn finisher(kind: u8) -> &'static FinisherStyle {
    &FINISHERS[(kind.max(1) as usize - 1).min(FINISHERS.len() - 1)]
}

/// Something to count in the career stats (handed to the menu side each frame).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CareerEv {
    /// You got a kill: a headshot? a finisher (its style number + 1)?
    Kill { head: bool, finisher: u8 },
    Death,
    WaveCleared,
    MatchEnd,
    /// You won the match (the first win of the day pays a bonus).
    MatchWon,
    /// You took down someone holding a Writ (their streak).
    WritClaim { streak: u32 },
    /// Two kills in quick succession (or more).
    MultiKill,
}
/// A combo finished: the elimination hit's "weapon" (kill feed).
pub const COMBO_TOOL: u8 = 22;
/// Basic ground swings in a row before you have to mix in another move (or pause).
pub const BASIC_STRING: u8 = 3;
/// Blade hits landed in a row, each while the target is still reeling: an elimination.
pub const COMBO_KO: u32 = 5;

/// One fighter's current combo against them (worked out by the host).
#[derive(Clone, Copy, Debug, Default)]
pub struct ComboTrack {
    pub by: Option<usize>,
    pub n: u32,
    /// When the last hit landed, and how long it keeps them reeling.
    pub last: f32,
    pub window: f32,
}
/// The guard meter: blocking drains it, it refills a moment after you lower your guard.
pub const GUARD_MAX: f32 = 100.0;
pub const GUARD_REGEN: f32 = 45.0;
pub const GUARD_REGEN_DELAY: f32 = 0.6;
/// Stagger when the guard meter runs out (or a charged heavy smashes through).
pub const GUARD_BREAK_TIME: f32 = 0.8;
/// Raise the guard this long before a bullet arrives to send it back.
pub const DEFLECT_WINDOW: f32 = 0.15;
/// The kick: quick, short, unblockable; it staggers a guard, loses to a slash.
pub const KICK_TIME: f32 = 0.34;
/// How long a melee contact's camera jolt lasts.
pub const JOLT_TIME: f32 = 0.13;
pub const KICK_HIT_AT: f32 = 0.1;
pub const KICK_RANGE: f32 = 1.9;
pub const KICK_CD: f32 = 0.75;
pub const KICK_STAGGER: f32 = 0.5;
pub const KICK_DMG: f32 = 10.0;
/// Hold attack this long for a charged heavy slash.
pub const CHARGE_TIME: f32 = 0.5;
/// Slash kinds beyond the basic ones: charged heavy, katana dash-through, greatsword slam.
pub const SLASH_HEAVY: u8 = 4;
pub const SLASH_THROUGH: u8 = 5;
pub const SLASH_SLAM: u8 = 6;
pub const SLAM_RADIUS: f32 = 3.5;

/// How long a blade hit keeps its target reeling (a follow-up landing in that time continues the
/// combo): its stagger, or the time in the air for a launch or a juggle.
pub fn combo_window(stun: f32, knock: V3) -> f32 {
    let air = if knock.y > 7.0 {
        0.9
    } else if knock.y > 4.5 {
        0.55
    } else {
        0.0
    };
    stun.max(air)
}

/// How much a blocked hit of `amount` drains the guard meter.
pub fn guard_drain(amount: f32) -> f32 {
    amount.min(45.0) * 1.2
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
    /// Pressed: kick.
    pub kick: bool,
    /// Pressed: finisher (on a weakened, staggered enemy in reach).
    pub finish: bool,
    /// Pressed: the fighter's ability.
    pub ability: bool,
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
/// Speed added per chained move (5 in a chain: +20%).
pub const FLOW_STEP: f32 = 0.04;

/// Each fighter's active ability (key Q / controller LB), and a passive that comes with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ability {
    /// Corvin Vale: a thrown marker that shows enemies near it through walls.
    Mark = 0,
    /// Sable Rook: a line to a wall, ledge or pillar that pulls you to it.
    Grapple = 1,
    /// Bram "Anvil" Kask: a frontal shield that stops bullets and throws back blades.
    Wall = 2,
    /// Ilya Fen: vanish for a moment while a decoy runs on and fakes an attack.
    Afterimage = 3,
}

impl Ability {
    pub const ALL: [Ability; 4] = [Ability::Mark, Ability::Grapple, Ability::Wall, Ability::Afterimage];
    /// The fighter's own ability.
    pub fn of(body: u8) -> Ability {
        Ability::ALL[(body as usize).min(3)]
    }
    pub fn from_u8(k: u8) -> Ability {
        Ability::ALL[(k as usize).min(3)]
    }
    pub fn name(self) -> &'static str {
        match self {
            Ability::Mark => "Warden's Mark",
            Ability::Grapple => "Grapple Line",
            Ability::Wall => "Iron Wall",
            Ability::Afterimage => "Afterimage",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Ability::Mark => "throw a marker: enemies within 12 m show through walls for 4 s",
            Ability::Grapple => "pull yourself to a wall, ledge or pillar within 20 m (2 charges; jump lets go)",
            Ability::Wall => "a 2.5 s frontal shield: stops bullets, throws back blades",
            Ability::Afterimage => "vanish for 0.6 s while a decoy runs on and fakes an attack",
        }
    }
    pub fn passive(self) -> &'static str {
        match self {
            Ability::Mark => "instant reload after a kill",
            Ability::Grapple => "your sniper glint stays hidden for the first second scoped",
            Ability::Wall => "+30% guard meter",
            Ability::Afterimage => "flow chains fade 50% slower",
        }
    }
    /// Seconds for a spent charge to come back, and how many charges it holds.
    pub fn cooldown(self, tn: &Tuning) -> f32 {
        match self {
            Ability::Mark => tn.mark_cd,
            Ability::Grapple => tn.grapple_cd,
            Ability::Wall => tn.shield_cd,
            Ability::Afterimage => tn.after_cd,
        }
    }
    pub fn charges(self, tn: &Tuning) -> f32 {
        if self == Ability::Grapple { tn.grapple_charges.round().max(1.0) } else { 1.0 }
    }
}

/// A Warden's Mark: flying, then stuck where it landed, showing enemies near it.
#[derive(Clone, Debug)]
pub struct Mark {
    pub pos: V3,
    pub vel: V3,
    pub owner: usize,
    pub landed: bool,
    /// Seconds flying, or seconds left once landed.
    pub t: f32,
}

/// An Afterimage decoy: a copy of its owner running on, faking an attack.
#[derive(Clone, Debug)]
pub struct Decoy {
    pub pos: V3,
    pub vel: V3,
    pub yaw: f32,
    pub owner: usize,
    pub t: f32,
}

/// Feel settings the developer tuning panel (F4) changes live. The defaults are the game's own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    pub run_speed: f32,
    /// Seconds from standing to full run speed.
    pub accel_time: f32,
    /// Seconds from full run speed to a stop.
    pub stop_time: f32,
    /// Steering in the air (m/s²).
    pub air_control: f32,
    pub gravity: f32,
    /// Metres a jump rises.
    pub jump_height: f32,
    pub dash_speed: f32,
    /// Seconds a dash lasts.
    pub dash_time: f32,
    /// Dashes you can do back to back, and seconds for each spent one to come back (they
    /// refill one at a time).
    pub dash_charges: f32,
    pub dash_recharge: f32,
    /// Slowest wall run (you keep any faster entry speed).
    pub wall_speed: f32,
    /// Longest wall run (seconds; running up a wall lasts about half).
    pub wall_time: f32,
    /// Speed added per chained move.
    pub flow_step: f32,
    pub cam_dist: f32,
    pub cam_height: f32,
    pub shoulder: f32,
    /// Seconds the camera takes to catch up (0 = locked on).
    pub cam_smooth: f32,
    /// Size of the characters (and their hit shapes).
    pub char_scale: f32,
    // ---- feel
    /// Share of speed lost on a sharp turn above 80% speed.
    pub turn_slow: f32,
    /// Skid dust and planted feet when stopping (0 = off).
    pub skid: f32,
    /// How much characters lean into turns and speeding up / slowing down.
    pub lean: f32,
    /// How much a landing squashes the character (heavier from higher).
    pub land_squash: f32,
    /// The crouch before a ground dash (seconds; 0 = instant).
    pub dash_windup: f32,
    /// Streaks at the screen edges during a dash.
    pub dash_blur: f32,
    /// Camera bob while running (metres).
    pub cam_bob: f32,
    /// Camera roll on wall-runs (degrees).
    pub cam_roll: f32,
    /// Camera dip on hard landings (metres).
    pub land_dip: f32,
    /// Extra field of view at high speed (degrees).
    pub fov_speed: f32,
    /// Team-coloured rim light on players, and how thin it is.
    pub rim: f32,
    pub rim_width: f32,
    // ---- abilities
    pub mark_cd: f32,
    pub mark_radius: f32,
    pub mark_time: f32,
    pub grapple_cd: f32,
    pub grapple_charges: f32,
    pub grapple_range: f32,
    pub grapple_pull: f32,
    pub shield_cd: f32,
    pub shield_time: f32,
    pub after_cd: f32,
    pub invis_time: f32,
    pub decoy_time: f32,
    pub glint_hide: f32,
    pub guard_bonus: f32,
    pub flow_slow: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            run_speed: RUN_SPEED,
            accel_time: 0.12,
            stop_time: 0.1,
            air_control: 22.0,
            // a touch lighter than thrown things fall, so jumps hang a little and read easily
            gravity: 23.0,
            jump_height: JUMP_VEL * JUMP_VEL / (2.0 * GRAVITY),
            dash_speed: 16.0,
            dash_time: 0.2,
            dash_charges: 2.0,
            dash_recharge: 1.9,
            wall_speed: 9.0,
            wall_time: 1.15,
            flow_step: FLOW_STEP,
            cam_dist: 3.2,
            cam_height: 1.98,
            shoulder: 0.78,
            cam_smooth: 0.04,
            char_scale: 1.0,
            turn_slow: 0.3,
            skid: 1.0,
            lean: 1.0,
            land_squash: 1.0,
            dash_windup: 0.04,
            dash_blur: 0.7,
            cam_bob: 0.025,
            cam_roll: 6.0,
            land_dip: 0.18,
            fov_speed: 4.0,
            // a thin outline: the fighters wear their own colours now
            rim: 0.5,
            rim_width: 4.0,
            mark_cd: 20.0,
            mark_radius: 12.0,
            mark_time: 4.0,
            grapple_cd: 12.0,
            grapple_charges: 2.0,
            grapple_range: 20.0,
            grapple_pull: 24.0,
            shield_cd: 22.0,
            shield_time: 2.5,
            after_cd: 18.0,
            invis_time: 0.6,
            decoy_time: 1.6,
            glint_hide: 1.0,
            guard_bonus: 0.3,
            flow_slow: 0.5,
        }
    }
}

impl Tuning {
    /// Take-off speed for the jump height under this gravity.
    pub fn jump_vel(&self) -> f32 {
        (2.0 * self.gravity * self.jump_height).max(0.0).sqrt()
    }
    pub fn accel(&self) -> f32 {
        self.run_speed / self.accel_time.max(0.005)
    }
    pub fn decel(&self) -> f32 {
        self.run_speed / self.stop_time.max(0.005)
    }
    /// `key=value` lines (for preset files).
    pub fn to_text(&self) -> String {
        TUNE_FIELDS.iter().map(|f| format!("{}={}\n", f.key, (f.get)(self))).collect()
    }
    /// Read `key=value` lines over the defaults (unknown keys are ignored, values kept in range).
    pub fn from_text(text: &str) -> Tuning {
        let mut t = Tuning::default();
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=') {
                if let (Some(f), Ok(v)) = (TUNE_FIELDS.iter().find(|f| f.key == k.trim()), v.trim().parse::<f32>()) {
                    if v.is_finite() {
                        (f.set)(&mut t, v.clamp(f.min, f.max));
                    }
                }
            }
        }
        t
    }
}

/// One slider of the tuning panel.
pub struct TuneField {
    pub key: &'static str,
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub unit: &'static str,
    pub group: &'static str,
    pub get: fn(&Tuning) -> f32,
    pub set: fn(&mut Tuning, f32),
}

pub const TUNE_FIELDS: [TuneField; 45] = [
    TuneField { key: "run_speed", label: "Run speed", min: 4.0, max: 16.0, unit: "m/s", group: "Movement", get: |t| t.run_speed, set: |t, v| t.run_speed = v },
    TuneField { key: "accel_time", label: "Ground acceleration time", min: 0.01, max: 0.6, unit: "s", group: "Movement", get: |t| t.accel_time, set: |t, v| t.accel_time = v },
    TuneField { key: "stop_time", label: "Stopping time", min: 0.01, max: 0.8, unit: "s", group: "Movement", get: |t| t.stop_time, set: |t, v| t.stop_time = v },
    TuneField { key: "air_control", label: "Air control", min: 0.0, max: 80.0, unit: "m/s²", group: "Movement", get: |t| t.air_control, set: |t, v| t.air_control = v },
    TuneField { key: "gravity", label: "Gravity", min: 5.0, max: 60.0, unit: "m/s²", group: "Movement", get: |t| t.gravity, set: |t, v| t.gravity = v },
    TuneField { key: "jump_height", label: "Jump height", min: 0.3, max: 5.0, unit: "m", group: "Movement", get: |t| t.jump_height, set: |t, v| t.jump_height = v },
    TuneField { key: "dash_speed", label: "Dash speed", min: 6.0, max: 40.0, unit: "m/s", group: "Moves", get: |t| t.dash_speed, set: |t, v| t.dash_speed = v },
    TuneField { key: "dash_time", label: "Dash length", min: 0.05, max: 0.6, unit: "s", group: "Moves", get: |t| t.dash_time, set: |t, v| t.dash_time = v },
    TuneField { key: "dash_charges", label: "Dashes in a row", min: 1.0, max: 6.0, unit: "", group: "Moves", get: |t| t.dash_charges, set: |t, v| t.dash_charges = v.round() },
    TuneField { key: "dash_recharge", label: "Dash refill time (each)", min: 0.3, max: 4.0, unit: "s", group: "Moves", get: |t| t.dash_recharge, set: |t, v| t.dash_recharge = v },
    TuneField { key: "wall_speed", label: "Wall-run speed (min)", min: 4.0, max: 24.0, unit: "m/s", group: "Moves", get: |t| t.wall_speed, set: |t, v| t.wall_speed = v },
    TuneField { key: "wall_time", label: "Wall-run duration", min: 0.2, max: 3.0, unit: "s", group: "Moves", get: |t| t.wall_time, set: |t, v| t.wall_time = v },
    TuneField { key: "flow_step", label: "Flow chain bonus (per move)", min: 0.0, max: 0.2, unit: "×", group: "Moves", get: |t| t.flow_step, set: |t, v| t.flow_step = v },
    TuneField { key: "cam_dist", label: "Camera distance", min: 1.0, max: 8.0, unit: "m", group: "Camera", get: |t| t.cam_dist, set: |t, v| t.cam_dist = v },
    TuneField { key: "cam_height", label: "Camera height", min: 1.2, max: 3.0, unit: "m", group: "Camera", get: |t| t.cam_height, set: |t, v| t.cam_height = v },
    TuneField { key: "shoulder", label: "Shoulder offset", min: -1.5, max: 1.5, unit: "m", group: "Camera", get: |t| t.shoulder, set: |t, v| t.shoulder = v },
    TuneField { key: "cam_smooth", label: "Camera smoothing", min: 0.0, max: 0.4, unit: "s", group: "Camera", get: |t| t.cam_smooth, set: |t, v| t.cam_smooth = v },
    TuneField { key: "char_scale", label: "Character scale", min: 0.5, max: 2.0, unit: "×", group: "Characters", get: |t| t.char_scale, set: |t, v| t.char_scale = v },
    TuneField { key: "turn_slow", label: "Sharp-turn slowdown", min: 0.0, max: 0.8, unit: "×", group: "Movement feel", get: |t| t.turn_slow, set: |t, v| t.turn_slow = v },
    TuneField { key: "skid", label: "Stop skid (dust, planted feet)", min: 0.0, max: 2.0, unit: "×", group: "Movement feel", get: |t| t.skid, set: |t, v| t.skid = v },
    TuneField { key: "dash_windup", label: "Dash anticipation crouch", min: 0.0, max: 0.12, unit: "s", group: "Movement feel", get: |t| t.dash_windup, set: |t, v| t.dash_windup = v },
    TuneField { key: "dash_blur", label: "Dash edge blur", min: 0.0, max: 1.5, unit: "×", group: "Movement feel", get: |t| t.dash_blur, set: |t, v| t.dash_blur = v },
    TuneField { key: "lean", label: "Lean into turns / speed", min: 0.0, max: 2.5, unit: "×", group: "Animation", get: |t| t.lean, set: |t, v| t.lean = v },
    TuneField { key: "land_squash", label: "Landing squash", min: 0.0, max: 2.5, unit: "×", group: "Animation", get: |t| t.land_squash, set: |t, v| t.land_squash = v },
    TuneField { key: "cam_bob", label: "Running bob", min: 0.0, max: 0.1, unit: "m", group: "Camera feel", get: |t| t.cam_bob, set: |t, v| t.cam_bob = v },
    TuneField { key: "cam_roll", label: "Wall-run roll", min: 0.0, max: 15.0, unit: "°", group: "Camera feel", get: |t| t.cam_roll, set: |t, v| t.cam_roll = v },
    TuneField { key: "land_dip", label: "Hard-landing dip", min: 0.0, max: 0.5, unit: "m", group: "Camera feel", get: |t| t.land_dip, set: |t, v| t.land_dip = v },
    TuneField { key: "fov_speed", label: "FOV widening with speed", min: 0.0, max: 20.0, unit: "°", group: "Camera feel", get: |t| t.fov_speed, set: |t, v| t.fov_speed = v },
    TuneField { key: "rim", label: "Player rim light", min: 0.0, max: 2.0, unit: "×", group: "Players", get: |t| t.rim, set: |t, v| t.rim = v },
    TuneField { key: "rim_width", label: "Rim thinness", min: 1.0, max: 6.0, unit: "", group: "Players", get: |t| t.rim_width, set: |t, v| t.rim_width = v },
    TuneField { key: "mark_cd", label: "Warden's Mark cooldown", min: 2.0, max: 60.0, unit: "s", group: "Abilities", get: |t| t.mark_cd, set: |t, v| t.mark_cd = v },
    TuneField { key: "mark_radius", label: "Warden's Mark reveal radius", min: 3.0, max: 30.0, unit: "m", group: "Abilities", get: |t| t.mark_radius, set: |t, v| t.mark_radius = v },
    TuneField { key: "mark_time", label: "Warden's Mark reveal time", min: 0.5, max: 10.0, unit: "s", group: "Abilities", get: |t| t.mark_time, set: |t, v| t.mark_time = v },
    TuneField { key: "grapple_cd", label: "Grapple Line recharge (each)", min: 1.0, max: 40.0, unit: "s", group: "Abilities", get: |t| t.grapple_cd, set: |t, v| t.grapple_cd = v },
    TuneField { key: "grapple_charges", label: "Grapple Line charges", min: 1.0, max: 4.0, unit: "", group: "Abilities", get: |t| t.grapple_charges, set: |t, v| t.grapple_charges = v.round() },
    TuneField { key: "grapple_range", label: "Grapple Line range", min: 5.0, max: 40.0, unit: "m", group: "Abilities", get: |t| t.grapple_range, set: |t, v| t.grapple_range = v },
    TuneField { key: "grapple_pull", label: "Grapple Line pull speed", min: 8.0, max: 45.0, unit: "m/s", group: "Abilities", get: |t| t.grapple_pull, set: |t, v| t.grapple_pull = v },
    TuneField { key: "shield_cd", label: "Iron Wall cooldown", min: 2.0, max: 60.0, unit: "s", group: "Abilities", get: |t| t.shield_cd, set: |t, v| t.shield_cd = v },
    TuneField { key: "shield_time", label: "Iron Wall duration", min: 0.5, max: 8.0, unit: "s", group: "Abilities", get: |t| t.shield_time, set: |t, v| t.shield_time = v },
    TuneField { key: "after_cd", label: "Afterimage cooldown", min: 2.0, max: 60.0, unit: "s", group: "Abilities", get: |t| t.after_cd, set: |t, v| t.after_cd = v },
    TuneField { key: "invis_time", label: "Afterimage invisibility", min: 0.1, max: 3.0, unit: "s", group: "Abilities", get: |t| t.invis_time, set: |t, v| t.invis_time = v },
    TuneField { key: "decoy_time", label: "Afterimage decoy life", min: 0.3, max: 4.0, unit: "s", group: "Abilities", get: |t| t.decoy_time, set: |t, v| t.decoy_time = v },
    TuneField { key: "glint_hide", label: "Sable: hidden sniper glint", min: 0.0, max: 3.0, unit: "s", group: "Passives", get: |t| t.glint_hide, set: |t, v| t.glint_hide = v },
    TuneField { key: "guard_bonus", label: "Bram: extra guard meter", min: 0.0, max: 1.0, unit: "×", group: "Passives", get: |t| t.guard_bonus, set: |t, v| t.guard_bonus = v },
    TuneField { key: "flow_slow", label: "Ilya: slower flow fade", min: 0.0, max: 0.9, unit: "×", group: "Passives", get: |t| t.flow_slow, set: |t, v| t.flow_slow = v },
];
/// ...up to this many steps (+20%).
pub const FLOW_MAX: u32 = 5;
/// Moves count as chained if you weren't on the ground longer than this before them.
pub const CHAIN_GROUND: f32 = 0.3;
/// Landing from a dash, wall-jump or air dash keeps your speed this long.
pub const MOMENTUM_HOLD: f32 = 0.4;
/// Jump or dash this soon after landing for a perfect-timing boost.
pub const PERFECT_WINDOW: f32 = 0.1;
/// Slides start at this speed and last at most this long.
/// A slide starts from about a full run (a little under run speed).
pub const SLIDE_MIN_SPEED: f32 = 6.4;
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
                c(at(0.0, 1.63), at(0.0, 1.73), 0.15, Part::Head),
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
    /// Dashes ready to use (a fraction is one refilling).
    pub dash_charges: f32,
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
    /// Inaccuracy stacked up by firing a pistol quickly (see Weapon::spam).
    pub spam: f32,
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
    /// Moves chained (each +4% speed), its little pop on screen, and the slow fade on foot.
    pub flow: u32,
    pub flow_pop: f32,
    /// The ability: charges ready (fractions recharging), a grapple's anchor and how long it's
    /// pulled, the Iron Wall's time left, invisibility left (Afterimage).
    pub ab_charge: f32,
    pub grapple: Option<V3>,
    pub grapple_t: f32,
    pub shield_t: f32,
    pub invis_t: f32,
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
    /// Crouching before a ground dash (time left), and the dash waiting to go.
    pub dash_wind: f32,
    dash_dir: Option<V3>,
    dash_perfect: bool,
    /// Guard meter (0..GUARD_MAX), time since it last took a hit, and a flash after it breaks.
    pub guard_hp: f32,
    guard_rest: f32,
    pub guard_broke: f32,
    /// Kicking (time left), cooldown, and whether this kick has landed its blow.
    pub kick_t: f32,
    kick_cd: f32,
    kick_done: bool,
    /// Holding attack: how long, and whether the heavy is ready.
    pub charge_t: f32,
    pub charged: bool,
    /// Katana dash-through: time left and who it has already cut.
    pub through_t: f32,
    through_hit: u32,
    /// Greatsword: plunging down for a ground slam.
    pub slam: bool,
    /// Doing a finisher: its style (number + 1, 0 = none), how far in, on whom, and whether the
    /// blow has landed.
    pub fin: u8,
    pub fin_t: f32,
    pub fin_target: usize,
    fin_hit: bool,
    /// Caught in someone's finisher: by whom (and for how much longer at most).
    pub held_by: Option<usize>,
    pub held_t: f32,
    pub held_kind: u8,
    /// Basic ground swings in this string, and a short breather after the string runs out.
    pub basic_n: u8,
    pub slash_lock: f32,
    /// Throwing stars: stars left in this burst, time to the next, right button last tick.
    pub burst_left: u8,
    burst_t: f32,
    alt_was: bool,
    /// The throwing motion (time left), and whether it's the wide fan sweep.
    pub throw_t: f32,
    pub throw_fan: bool,
    /// No second sharp-turn slowdown until this runs out.
    turn_cd: f32,
    /// How hard the last landing was (0..1, fades), for the camera dip.
    pub land_impact: f32,
    /// Speed on the last tick (skid dust when it drops fast).
    prev_speed: f32,
    skid_dust: f32,
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
            dash_charges: 2.0,
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
            spam: 0.0,
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
            dash_wind: 0.0,
            dash_dir: None,
            dash_perfect: false,
            guard_hp: GUARD_MAX,
            guard_rest: 10.0,
            guard_broke: 0.0,
            kick_t: 0.0,
            kick_cd: 0.0,
            kick_done: true,
            charge_t: 0.0,
            charged: false,
            through_t: 0.0,
            through_hit: 0,
            slam: false,
            fin: 0,
            fin_t: 0.0,
            fin_target: 0,
            fin_hit: false,
            held_by: None,
            held_t: 0.0,
            held_kind: 0,
            basic_n: 0,
            slash_lock: 0.0,
            burst_left: 0,
            burst_t: 0.0,
            alt_was: false,
            throw_t: 0.0,
            throw_fan: false,
            turn_cd: 0.0,
            land_impact: 0.0,
            prev_speed: 0.0,
            skid_dust: 0.0,
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
            ab_charge: 8.0,
            grapple: None,
            grapple_t: 0.0,
            shield_t: 0.0,
            invis_t: 0.0,
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
        self.flow_mult_by(FLOW_STEP)
    }
    pub fn flow_mult_by(&self, step: f32) -> f32 {
        1.0 + step * self.flow.min(FLOW_MAX) as f32
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
            return gs.spread_still; // the scattergun's pattern never changes
        }
        if self.weapon == Weapon::Sniper {
            // wild from the hip, dead-on the moment the scope is up (quickscopes welcome)
            return if self.scope_t >= SCOPE_IN { 0.0 } else { gs.spread_still };
        }
        if self.since_shot >= self.weapon.rest_time() {
            return 0.0; // a rested first shot is perfectly accurate
        }
        let (_, per) = self.weapon.spam();
        gs.spread_still * self.move_spread * (1.0 + gs.bloom * self.recoil) + per * self.spam * self.move_spread
    }
    pub fn chest(&self) -> V3 {
        self.pos + v3(0.0, 1.2, 0.0)
    }
    /// The scattergun's pump racking back and forward after a shot (0 at rest, 1 fully back).
    pub fn pump_k(&self) -> f32 {
        if self.weapon != Weapon::Shotgun || self.fire_cd <= 0.0 {
            return 0.0;
        }
        let e = Weapon::Shotgun.interval() - self.fire_cd;
        let p = ((e - 0.26) / 0.36).clamp(0.0, 1.0);
        (p * std::f32::consts::PI).sin()
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
    /// Solo: survive waves of ever better bots (Practice only).
    Gauntlet = 6,
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
            6 => Mode::Gauntlet,
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
            Mode::Gauntlet => "Gauntlet",
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
            Mode::Gauntlet => "solo: survive waves of ever better bots for a high score",
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
            Mode::Gauntlet => "score",
            _ => "kills",
        }
    }
}

/// Gauntlet: the wave you're on, your score, and how it went.
#[derive(Clone, Debug, Default)]
pub struct Gauntlet {
    pub wave: u32,
    pub score: u32,
    pub kills: u32,
    /// The pause before the next wave.
    pub break_t: f32,
    /// You fell: the run is over.
    pub over: bool,
    /// Your best score on this computer (from settings), and whether this run beat it.
    pub best: u32,
    pub new_best: bool,
    /// The score was handed to the leaderboard.
    pub submitted: bool,
    /// Steam leaderboard: your rank (0 = none yet) and the top entries (name, score, rank).
    pub rank: i32,
    pub board: Vec<(String, i32, i32)>,
}

/// Bots in a Gauntlet wave and how good they are.
pub fn gauntlet_wave(wave: u32) -> (usize, f32) {
    let n = (1 + wave as usize / 2).min(6);
    let skill = (0.25 + 0.075 * (wave.max(1) - 1) as f32).min(1.0);
    (n, skill)
}

/// Points for a Gauntlet kill on `wave`.
pub fn gauntlet_kill_points(wave: u32, head: bool) -> u32 {
    100 + 25 * wave + if head { 50 } else { 0 }
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
    /// Classic: no abilities or passives (a lobby setting).
    pub classic: bool,
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
    /// 0 grenade, 1 rocket, 2 throwing star
    pub kind: u8,
    /// Drawn this far from where it really is, shrinking to nothing (a star leaving the hand).
    pub vis: V3,
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

/// One kept moment of the match: everyone as they were, with the shots and sparks in the air.
#[derive(Clone, Debug)]
pub struct RecFrame {
    pub t: f32,
    pub f: Vec<Fighter>,
    pub tracers: Vec<Tracer>,
    pub particles: Vec<Particle>,
}

/// The Play of the Match: whose, what it was, how good, and the frames to replay.
#[derive(Clone, Debug)]
pub struct Highlight {
    pub player: usize,
    pub name: String,
    pub color: u8,
    pub what: String,
    pub score: f32,
    pub frames: Vec<RecFrame>,
}

impl Highlight {
    pub fn duration(&self) -> f32 {
        match (self.frames.first(), self.frames.last()) {
            (Some(a), Some(b)) => (b.t - a.t).max(0.0),
            _ => 0.0,
        }
    }
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
    /// Who was hit (255: nobody in particular); a volley's pellets add up into one number.
    pub victim: u8,
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
    /// Someone used their ability: what (Ability), where from / to, which way. From a player
    /// about themselves; from the host, passed on once it checked out.
    Ability { who: u8, kind: u8, p: V3, d: V3 },
    /// From a player: "I hit them" (the host checks it). From the host: "you were hit".
    /// `guard` drains the victim's guard meter (a blocked hit).
    Hit { victim: u8, dmg: f32, knock: V3, stun: f32, head: bool, w: u8, o: V3, dir: V3, guard: f32 },
    /// From a player: "my bullet was sent back at me by `by`" (the host trusts self-inflicted hurt).
    Deflect { by: u8, dmg: f32 },
    /// "I've started a finisher on `target`": their computer holds them for it.
    Finisher { target: u8, kind: u8 },
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
    /// Which slot the host plays in (0 unless the host changed mid-match).
    pub host_slot: usize,
    /// Gauntlet (solo waves), when that's the mode.
    pub gauntlet: Option<Gauntlet>,
    /// Things for the career stats since the menu side last looked.
    pub career: Vec<CareerEv>,
    /// Our last finisher's style (the kill can be confirmed after it has finished playing).
    pub last_finisher: u8,
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
    pub last_kill: Option<(usize, Option<usize>, u8, bool)>,
    pub rng: Rng,
    pub bots: Vec<Option<Bot>>,
    pub out_events: Vec<Ev>,
    pub feed: Vec<FeedLine>,
    pub chat: Vec<ChatLine>,
    /// Where everyone was recently (for checking hits against the past).
    pub history: Vec<VecDeque<(f32, V3)>>,
    /// The loadout you'll respawn with.
    pub my_loadout: Loadout,
    /// The menu's showcased fighter: posed in front of the camera, outside the match (nobody
    /// targets, hits or bumps into it).
    pub showcase: Option<usize>,
    /// Warden's Marks and Afterimage decoys in the arena.
    pub marks: Vec<Mark>,
    pub decoys: Vec<Decoy>,
    /// (host) when each player last used their ability (to throw out uses still on cooldown).
    ab_uses: Vec<Vec<f32>>,
    /// When each fighter's scope came up (-1: not scoped), for the sniper glint.
    pub scoped_since: Vec<f32>,
    /// Kills since their last death, for everyone (a Writ at WRIT_STREAK).
    pub streaks: Vec<u32>,
    /// Quick kills in a row for everyone (count, time of the last), for multi-kills.
    pub kill_chain: Vec<(u32, f32)>,
    /// Finisher styles the local player hasn't unlocked yet (bit per style; from their mastery).
    pub fin_locks: u32,
    /// Play of the Match: the last few seconds kept (frames), the best moment so far waiting for
    /// its tail (score, who, what, when), the one picked, and its playback after the match.
    pub rec: VecDeque<RecFrame>,
    rec_acc: f32,
    hl_pending: Option<(f32, usize, String, f32)>,
    pub potm: Option<Highlight>,
    pub potm_t: Option<f32>,
    pub potm_done: bool,
    potm_stash: Option<(Vec<Fighter>, Vec<Tracer>, Vec<Particle>, f32)>,
    flow_noted: Vec<bool>,
    /// The phase last tick (to notice a match ending on every computer).
    seen_phase: Phase,    pub hitmarker: f32,
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
    /// Melee contact: a short sharp jolt of the camera along the blow (direction, strength, time
    /// left). Looks only: it never moves the aim.
    pub jolt_dir: V3,
    pub jolt_k: f32,
    pub jolt_t: f32,
    /// "+35 HP" shown after a kill.
    pub heal_pop: f32,
    /// Upward aim kick from the last shot, for the screen to apply to the mouse aim.
    pub aim_kick: f32,
    /// Feel settings (the developer tuning panel changes them live).
    pub tune: Tuning,
    /// (host / this computer) each fighter's current combo against them.
    pub combos: Vec<ComboTrack>,
    /// Our own combo as we see it (for the HUD): hits in a row and when the last landed.
    pub my_combo: ComboTrack,
    /// Where the camera actually sits relative to the plain over-the-shoulder spot (its spring,
    /// bob and landing dip), so shots leave along the line through the crosshair.
    pub view_offset: V3,
    /// How far between the last two ticks the last picture was drawn; hit shapes are placed
    /// where everyone was shown.
    pub view_alpha: f32,
    seen_pos: Vec<V3>,
    /// Where a shot's tracer and flash start (the gun), when that isn't where the shot starts.
    tracer_from: Option<V3>,
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
    pub heavies: u32,
    pub kicks: u32,
    pub guard_kicks: u32,
    pub guard_breaks: u32,
    pub deflects: u32,
    pub backstabs: u32,
    pub dash_throughs: u32,
    pub slams: u32,
    pub combo_kos: u32,
    pub star_hits: u32,
    pub marks: u32,
    pub grapples: u32,
    pub wall_blocks: u32,
    pub afterimages: u32,
}

pub struct Lesson {
    pub title: &'static str,
    pub how: &'static str,
    pub count: fn(&TrainStats) -> u32,
    pub need: u32,
    /// A gun to hand over in slot 3 for this lesson.
    pub gun: Option<Weapon>,
    /// The blade for this lesson (the katana when not set).
    pub melee: Option<Weapon>,
    /// What the dummies do: 0 stand facing you, 1 hold their guard up, 2 shoot at you now and
    /// then, 3 stand with their backs turned.
    pub dummy: u8,
    /// The fighter to play for this lesson (their ability).
    pub body: Option<u8>,
}

pub const LESSONS: [Lesson; 31] = [
    Lesson { title: "Move and jump", how: "Move with W A S D and look with the mouse. Press SPACE to jump.", count: |s| s.jumps, need: 3, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Dash", how: "Press SHIFT (or double-tap a direction key) to dash. Dash in different directions.", count: |s| s.dashes, need: 3, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Air dash", how: "Jump with SPACE, then press SHIFT in the air for a second burst of speed.", count: |s| s.air_dashes, need: 2, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Wall-run", how: "Run along a wall, jump, then press SPACE again next to the wall while moving along it.", count: |s| s.wallruns, need: 2, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Wall climb", how: "Face a wall, jump, then press SPACE again while looking straight at it to run up.", count: |s| s.wallclimbs, need: 1, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Wall-jump", how: "While on a wall, press SPACE again to kick off it. Chain wall to wall!", count: |s| s.walljumps, need: 2, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Slide", how: "Run, then hold C to slide along the ground. Press SPACE mid-slide to jump out of it with your speed.", count: |s| s.slides.min(1) + s.slide_jumps.min(1), need: 2, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Momentum", how: "Land from a dash, air dash or wall-jump: you keep your speed for a moment instead of slowing to a run.", count: |s| s.momentum, need: 3, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Perfect timing", how: "Jump or dash the instant you land (within a tenth of a second) for an extra boost and a whoosh.", count: |s| s.perfects, need: 2, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Flow chain", how: "Chain moves without staying on the ground: dash, wall-run, wall-jump, air dash. Each adds +4% speed. Reach x4!", count: |s| (s.best_flow >= 4) as u32, need: 1, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Sword slash", how: "Press 1 for your katana and click to slash a dummy. Click again to chain swings.", count: |s| s.slash_hits, need: 4, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Slash cancel", how: "Right after a slash hits, press SHIFT, SPACE or hold right-click to cut its recovery short.", count: |s| s.cancels, need: 3, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Butterfly", how: "Slash, hold right-click to guard, release and slash again. Keep the rhythm going!", count: |s| s.butterflies, need: 4, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Launcher", how: "Dash at a dummy and slash during the dash (or land the 3rd hit of a ground combo) to launch it.", count: |s| s.launches, need: 2, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Air combo", how: "Launch a dummy, jump after it and slash it in mid-air before it lands.", count: |s| s.air_hits, need: 2, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Combo KO", how: "Land 5 blade hits in a row, each while the dummy is still reeling (mashing won't do: mix slashes, cancels, a launcher, air hits, kicks). The fifth knocks it out.", count: |s| s.combo_kos, need: 1, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Charged heavy", how: "Hold the attack button: after half a second your blade flashes. Let go for a heavy slash that smashes through guards and launches. (SHIFT cancels it.)", count: |s| s.heavies, need: 2, gun: None, melee: None, dummy: 1, body: None },
    Lesson { title: "Kick", how: "Press F to kick. It's quick and can't be blocked: kick a guarding dummy to stagger it. (A slash beats a kick, a kick beats a guard, a guard beats a slash.)", count: |s| s.guard_kicks, need: 3, gun: None, melee: None, dummy: 1, body: None },
    Lesson { title: "Guard break", how: "Blocking drains the guard meter under the crosshair. Keep slashing a guarding dummy until its guard shatters.", count: |s| s.guard_breaks, need: 1, gun: None, melee: None, dummy: 1, body: None },
    Lesson { title: "Bullet deflect", how: "Hold your blade. A dummy shoots at you now and then: raise your guard (right-click) just as it fires to send the bullet back.", count: |s| s.deflects, need: 2, gun: None, melee: None, dummy: 2, body: None },
    Lesson { title: "Dagger backstab", how: "You have a dagger now. Its hits do double damage from behind: get around a dummy and stab it in the back.", count: |s| s.backstabs, need: 3, gun: None, melee: Some(Weapon::Dagger), dummy: 3, body: None },
    Lesson { title: "Katana dash-through", how: "Dash at a dummy and slash during the dash: you cut straight through it and come out the other side.", count: |s| s.dash_throughs, need: 2, gun: None, melee: None, dummy: 0, body: None },
    Lesson { title: "Greatsword slam", how: "You have a greatsword now. Jump high (or off a ledge) and slash in the air to plunge down with a shockwave.", count: |s| s.slams, need: 2, gun: None, melee: Some(Weapon::Greatsword), dummy: 0, body: None },
    Lesson { title: "Hand cannon", how: "Press 3 for the hand cannon. A headshot always kills, even through armour. Land 2 headshots on a dummy (wait a beat between shots: the first is dead-on).", count: |s| s.cannon_heads, need: 2, gun: Some(Weapon::HandCannon), melee: None, dummy: 0, body: None },
    Lesson { title: "Sniper", how: "Press 3 for the sniper and hold right click to scope: it's dead-on once the scope is up. Hit a dummy twice (one body hit kills a real player).", count: |s| s.sniper_hits, need: 2, gun: Some(Weapon::Sniper), melee: None, dummy: 0, body: None },
    Lesson { title: "Shuriken", how: "Press 3 for the throwing stars. Click throws three in a quick burst, right-click fans three at once. They fly fast but not instantly: lead a moving target. Hit a dummy 6 times.", count: |s| s.star_hits, need: 6, gun: Some(Weapon::Shuriken), melee: None, dummy: 0, body: None },
    Lesson { title: "Warden's Mark (Corvin Vale)", how: "You're Corvin Vale. Press Q (controller LB) to throw a marker near the dummies: enemies within 12 m of it show through walls for 4 s. Passive: a kill reloads every gun.", count: |s| s.marks, need: 2, gun: None, melee: None, dummy: 0, body: Some(0) },
    Lesson { title: "Grapple Line (Sable Rook)", how: "You're Sable Rook. Aim at a wall, ledge or pillar within 20 m and press Q to be pulled there (2 charges). Press SPACE to let go early and keep your speed. Passive: your sniper glint stays hidden for the first second scoped.", count: |s| s.grapples, need: 3, gun: None, melee: None, dummy: 0, body: Some(1) },
    Lesson { title: "Iron Wall (Bram Kask)", how: "You're Bram \"Anvil\" Kask. The dummies shoot at you: face them and press Q to raise a shield that stops bullets (and throws back blades). Block 3 shots. Passive: a 30% bigger guard meter.", count: |s| s.wall_blocks, need: 3, gun: None, melee: None, dummy: 2, body: Some(2) },
    Lesson { title: "Afterimage (Ilya Fen)", how: "You're Ilya Fen. Press Q to vanish for a moment while a decoy of you runs on and fakes an attack. Passive: your flow chain fades half as fast.", count: |s| s.afterimages, need: 2, gun: None, melee: None, dummy: 0, body: Some(3) },
    Lesson { title: "Rocket jump", how: "Press 2 for the rocket launcher. Look down at your feet, jump and fire at the same moment.", count: |s| s.rocket_jumps, need: 1, gun: None, melee: None, dummy: 0, body: None },
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
    Kick,
    GuardBreak,
    Deflect,
    ChargeReady,
    Slam,
    Shuriken,
    /// The scattergun racking its pump, and a shell pushed into it.
    Pump,
    ShellIn,
    /// Abilities: a mark pinging, a grapple firing, a shield going up, vanishing.
    MarkPing,
    GrappleFire,
    ShieldUp,
    Vanish,
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
            Weapon::Shuriken => Sfx::Shuriken,
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
/// Pistol spam inaccuracy drains this fast (units per second), and stacks up to SPAM_MAX.
pub const SPAM_RECOVER: f32 = 1.6;
pub const SPAM_MAX: f32 = 3.5;

/// What the host could confirm about a player's hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClaimCheck {
    pub head_ok: bool,
    pub core_ok: bool,
}

pub enum Act {
    /// A gunshot with the spread it had the moment it was fired.
    Gun { w: Weapon, spread: f32 },
    /// The scattergun's pump racks: out flies the spent shell.
    Pump,
    /// The fighter's ability: where from / to, which way.
    Ability { kind: Ability, p: V3, d: V3 },
    Slash,
    Throw,
    Item { kind: u8 },
    /// A greatsword plunge hit the ground: shockwave.
    Slam,
    /// A finisher starts (the victim is held for it).
    FinishStart,
    /// The finisher's blow lands.
    Execute,
    /// A throwing star, turned this far (radians) from where you aim.
    Star { off: f32 },
}

pub const BOT_NAMES: [&str; 7] = ["Ash", "Vex", "Kiro", "Nyx", "Rook", "Sable", "Juno"];

impl Game {
    pub fn new(kind: Kind, local_slot: usize, mode: Mode, target: u16) -> Game {
        let world = World::arena();
        let flag_homes = world.flags;
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
                classic: false,
                phase,
                timer,
                target: target.max(1),
                team_score: [0, 0],
                number: 1,
                round: 0,
                round_end: 0.0,
                duel: [255, 255],
                flags: [0, 1].map(|t| Flag { state: 0, carrier: 255, pos: flag_homes[t], timer: 0.0 }),
            },
            queue: VecDeque::new(),
            host_slot: 0,
            gauntlet: None,
            career: Vec::new(),
            last_finisher: 0,
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
            showcase: None,
            marks: Vec::new(),
            decoys: Vec::new(),
            ab_uses: vec![Vec::new(); MAX_PLAYERS],
            scoped_since: vec![-1.0; MAX_PLAYERS],
            streaks: vec![0; MAX_PLAYERS],
            kill_chain: vec![(0, -10.0); MAX_PLAYERS],
            fin_locks: 0,
            rec: VecDeque::new(),
            rec_acc: 0.0,
            hl_pending: None,
            potm: None,
            potm_t: None,
            potm_done: false,
            potm_stash: None,
            flow_noted: vec![false; MAX_PLAYERS],
            seen_phase: Phase::Warmup,
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
            jolt_dir: V3::ZERO,
            jolt_k: 0.0,
            jolt_t: 0.0,
            heal_pop: 0.0,
            aim_kick: 0.0,
            tune: Tuning::default(),
            combos: vec![ComboTrack::default(); MAX_PLAYERS],
            my_combo: ComboTrack::default(),
            view_offset: V3::ZERO,
            view_alpha: 1.0,
            seen_pos: vec![V3::ZERO; MAX_PLAYERS],
            tracer_from: None,
        }
    }

    /// Play on another map: the level is rebuilt, the flags go home and everyone this computer
    /// runs starts again at a spawn point there.
    pub fn set_map(&mut self, map: MapId) {
        if self.world.map == map {
            return;
        }
        self.world = World::build(map);
        self.mt.flags = [0, 1].map(|t| Flag { state: 0, carrier: 255, pos: self.world.flag_base(t), timer: 0.0 });
        self.grenades.clear();
        for i in 0..self.f.len() {
            if self.f[i].present && self.f[i].owned && self.f[i].alive {
                let (p, y) = self.pick_spawn(i);
                self.f[i].respawn_at(p, y);
            }
        }
        for h in self.history.iter_mut() {
            h.clear();
        }
    }

    /// The training room: you, a katana and a rocket launcher, and three dummies that can't die.
    /// It's laid out on the open courtyard (the long wall behind you is for wall-runs).
    pub fn training(name: &str) -> Game {
        let mut g = Game::new(Kind::Offline, 0, Mode::Ffa, 9999);
        g.set_map(MapId::Courtyard);
        g.my_loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Rocket, gun2: Weapon::Revolver, item: 2, body: 0, outfit: 0 };
        g.add_player(0, name, true, None, 0);
        let homes = [v3(-3.5, 0.0, -6.0), v3(0.0, 0.0, -6.2), v3(3.5, 0.0, -6.0)];
        for (k, &h) in homes.iter().enumerate() {
            let s = k + 1;
            g.add_player(s, &format!("Dummy {}", s), true, Some(0.0), 0);
            g.bots[s] = None; // stands still
            g.f[s].loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Revolver, gun2: Weapon::Shotgun, item: 0, body: 0, outfit: 0 };
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

    /// Developer tool: a still target `dist` metres straight ahead of the local player (games on
    /// this computer only). Uses a free slot, or replaces the last such dummy.
    pub fn spawn_dummy(&mut self, dist: f32) -> Result<String, String> {
        if self.kind != Kind::Offline {
            return Err("Dummies can only be added to games on this computer (Practice or Training).".into());
        }
        let me = self.local;
        let slot = (0..MAX_PLAYERS)
            .find(|&s| !self.f[s].present)
            .or_else(|| (0..MAX_PLAYERS).rev().find(|&s| s != me && self.f[s].name.starts_with("Dummy")))
            .ok_or("No room: the game is full.")?;
        let fwd = yaw_fwd(self.f[me].yaw);
        let mut at = self.f[me].pos + fwd * dist;
        // stand it on whatever is there
        at.y = self.world.ground_below(at + v3(0.0, 6.0, 0.0)).max(0.0);
        let team = if self.mt.mode.teams() { 1 - self.f[me].team.min(1) } else { 0 };
        let name = format!("Dummy {:.0}m", dist);
        self.add_player(slot, &name, true, Some(0.0), team);
        self.bots[slot] = None; // it just stands there
        let yaw = (-fwd.x).atan2(-fwd.z);
        self.f[slot].respawn_at(at, yaw);
        self.f[slot].spawn_prot = 0.0;
        self.f[slot].ready = true;
        Ok(format!("{name} placed"))
    }

    /// Remove every dummy added with `spawn_dummy`.
    pub fn remove_dummies(&mut self) -> usize {
        let ids: Vec<usize> = (0..MAX_PLAYERS).filter(|&s| s != self.local && self.f[s].present && self.f[s].name.starts_with("Dummy ") && self.f[s].name.ends_with('m')).collect();
        for &s in &ids {
            self.remove_player(s);
        }
        ids.len()
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
            if i == me {
                // you can't die in training
                ff.alive = true;
                if ff.hp < 1.0 {
                    ff.hp = 1.0;
                }
            } else if !ff.alive {
                // a dummy that went down gets back up quickly
                ff.respawn_t = ff.respawn_t.min(1.5);
                continue;
            } else if ff.spawn_prot > 0.0 {
                // just back up: at its own spot, ready to be hit
                ff.spawn_prot = 0.0;
                if (1..=3).contains(&i) && !ff.name.ends_with('m') {
                    let home = t.homes[(i - 1).min(2)];
                    ff.pos = home;
                    ff.prev_pos = home;
                    ff.vel = V3::ZERO;
                }
            }
            if ff.hurt_t <= 0.0 {
                ff.hp = (ff.hp + 60.0 * dt).min(100.0);
            }
            if i != me {
                ff.ap = 0.0;
                // face the player (or turn their backs for the backstab lesson)
                let d = self.f[me].pos - self.f[i].pos;
                let away = LESSONS.get(t.lesson).is_some_and(|l| l.dummy == 3);
                self.f[i].yaw = if away { 0.0 } else { d.x.atan2(d.z) };
                // the three training dummies go back to their spots (measuring dummies stay put)
                let home = t.homes[(i - 1).min(2)];
                let own = (1..=3).contains(&i) && !self.f[i].name.ends_with('m');
                let ff = &mut self.f[i];
                if own && ff.on_ground && ff.float_t <= 0.0 && ff.stun_t <= 0.0 && (ff.pos - home).flat().len() > 4.0 {
                    ff.pos = home;
                    ff.prev_pos = home;
                    ff.vel = V3::ZERO;
                }
            }
        }
        // refill ammo and grenades so practice never runs dry; hand over this lesson's gun
        if let Some(f) = self.f.get_mut(me) {
            f.items = f.items.max(1);
            // an ability lesson: play its fighter
            if let Some(b) = LESSONS.get(t.lesson).and_then(|l| l.body) {
                if f.loadout.body != b {
                    f.loadout.body = b;
                    f.ab_charge = 9.0;
                    self.my_loadout.body = b;
                }
            }
            let blade = LESSONS.get(t.lesson).and_then(|l| l.melee).unwrap_or(Weapon::Katana);
            if f.loadout.melee != blade {
                f.loadout.melee = blade;
                if f.cur == 0 {
                    f.weapon = blade;
                    f.slash_t = 0.0;
                }
                self.my_loadout.melee = blade;
            }
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

    /// Gauntlet: just you; the waves come once you've picked your loadout. `best` is your best
    /// score so far.
    pub fn gauntlet(name: &str, best: u32) -> Game {
        let mut g = Game::new(Kind::Offline, 0, Mode::Gauntlet, 0);
        g.add_player(0, name, true, None, 0);
        g.gauntlet = Some(Gauntlet { best, ..Default::default() });
        g.banner = Some(("GAUNTLET".into(), 2.5));
        g
    }

    /// Gauntlet during the fight: send the waves, give a breather between them, end the run
    /// when you fall.
    fn update_gauntlet(&mut self, dt: f32) {
        let Some(mut gt) = self.gauntlet.take() else { return };
        let me = self.local;
        if !gt.over {
            // the fallen stay down until the wave is over
            for s in 0..self.f.len() {
                if s != me && self.f[s].present && !self.f[s].alive {
                    self.f[s].respawn_t = 1e9;
                }
            }
            if !self.f[me].alive {
                gt.over = true;
                gt.new_best = gt.score > gt.best;
                gt.best = gt.best.max(gt.score);
                self.start_killcam();
                self.mt.phase = Phase::MatchOver;
                self.mt.timer = 9999.0;
                self.banner = Some((format!("GAUNTLET OVER - SCORE {}", gt.score), 9999.0));
                self.sfx2d(Sfx::MatchOver);
            } else if gt.break_t > 0.0 {
                gt.break_t -= dt;
                if gt.break_t <= 0.0 {
                    self.spawn_wave(&mut gt);
                }
            } else {
                let left = (0..self.f.len()).filter(|&s| s != me && self.f[s].present && self.f[s].alive).count();
                if left == 0 {
                    if gt.wave > 0 {
                        // wave cleared: points, a breather, some health, armour and ammo back
                        let bonus = 250 * gt.wave;
                        gt.score += bonus;
                        self.say(format!("Wave {} cleared! +{bonus}", gt.wave));
                        self.career.push(CareerEv::WaveCleared);
                        self.sfx2d(Sfx::Kill);
                        let f = &mut self.f[me];
                        f.hp = (f.hp + 40.0).min(MAX_HP);
                        f.ap = (f.ap + 25.0).min(MAX_AP);
                        f.items = f.items.max(1);
                        for k in 1..3 {
                            let w = f.slot_weapon(k);
                            f.mag[k] = w.mag();
                        }
                    }
                    gt.break_t = if gt.wave == 0 { 0.6 } else { 4.0 };
                    self.banner = Some((format!("WAVE {}", gt.wave + 1), gt.break_t.max(1.5)));
                }
            }
        }
        self.gauntlet = Some(gt);
    }

    /// The next Gauntlet wave: clear away the last one, bring in more (and better) bots.
    fn spawn_wave(&mut self, gt: &mut Gauntlet) {
        let me = self.local;
        for s in 0..self.f.len() {
            if s != me && self.f[s].present {
                self.remove_player(s);
            }
        }
        gt.wave += 1;
        let (n, skill) = gauntlet_wave(gt.wave);
        let mut slot = 0;
        for k in 0..n {
            slot = (slot + 1..MAX_PLAYERS).find(|&s| s != me).unwrap_or(MAX_PLAYERS - 1);
            self.add_player(slot, BOT_NAMES[(k + gt.wave as usize) % BOT_NAMES.len()], true, Some(skill), 1);
            let f = &mut self.f[slot];
            f.ready = true;
            // later waves come armoured
            f.ap = (10.0 * gt.wave as f32).min(MAX_AP);
        }
        self.sfx2d(Sfx::MatchStart);
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

    /// Who fighter `i` could finish right now: an enemy in reach, below a quarter health, and
    /// staggered, guard-broken or launched (the nearest if several).
    pub fn finish_target(&self, i: usize) -> Option<usize> {
        let a = &self.f[i];
        if !a.alive || a.fin > 0 || a.held_by.is_some() || a.stun_t > 0.0 {
            return None;
        }
        let mut best: Option<(f32, usize)> = None;
        for j in 0..self.f.len() {
            let b = &self.f[j];
            if !self.is_enemy(i, j) || !b.alive || b.spawn_prot > 0.0 || b.hp >= FINISH_HP || b.held_by.is_some() {
                continue;
            }
            let launched = !b.on_ground && (b.stun_t > 0.0 || b.float_t > 0.0);
            if !(b.stagger_t > 0.0 || b.guard_broke > 0.0 || launched) {
                continue;
            }
            let d = b.pos - a.pos;
            let dist = d.flat().len();
            if dist > FINISH_RANGE || d.y.abs() > 3.0 || !self.world.line_clear(a.chest(), b.chest()) {
                continue;
            }
            if best.map_or(true, |(bd, _)| dist < bd) {
                best = Some((dist, j));
            }
        }
        best.map(|(_, j)| j)
    }

    /// Hold fighter `t` for `attacker`'s finisher (on the computer that runs `t`).
    fn hold(&mut self, t: usize, attacker: usize, kind: u8) {
        let dur = finisher(kind).dur;
        let f = &mut self.f[t];
        if !f.alive {
            return;
        }
        f.held_by = Some(attacker);
        f.held_t = dur + 0.25;
        f.held_kind = kind;
        f.stun_t = f.stun_t.max(dur + 0.1);
        f.slash_t = 0.0;
        f.kick_t = 0.0;
        f.guard = false;
        f.wall = None;
        f.vel.x = 0.0;
        f.vel.z = 0.0;
    }

    /// (host) Keep the lobby at `target` fighters with bots: add one when there's room, and send
    /// one away when there are more than that (a real player came in). Returns what happened.
    pub fn fill_bots(&mut self, target: usize, skill: f32) -> Option<String> {
        let present = self.present();
        let bots: Vec<usize> = present.iter().copied().filter(|&s| s != self.local && self.f[s].is_bot && self.f[s].owned).collect();
        if present.len() > target {
            let s = *bots.last()?;
            let nm = self.f[s].name.clone();
            self.remove_player(s);
            return Some(format!("{nm} (bot) left to make room"));
        }
        if present.len() < target.min(MAX_PLAYERS) {
            let slot = (0..MAX_PLAYERS).find(|&s| !self.f[s].present && s != self.local)?;
            let used: Vec<String> = present.iter().map(|&s| self.f[s].name.clone()).collect();
            let name = BOT_NAMES.iter().find(|n| !used.iter().any(|u| u == *n)).copied().unwrap_or("Bot");
            let team = self.balance_team();
            self.add_player(slot, name, true, Some(skill), team);
            self.f[slot].ready = true;
            return Some(format!("{name} (bot) joined"));
        }
        None
    }

    /// (player) The host left and this computer takes over: the old host's fighter goes, its
    /// bots become ours, and from now on health, deaths and the match are decided here. Everyone
    /// else (`humans`) keeps playing from their own computer.
    pub fn become_host(&mut self, old_host: usize, humans: &[usize]) {
        self.kind = Kind::Host;
        if old_host < self.f.len() && old_host != self.local && self.f[old_host].present {
            self.remove_player(old_host);
        }
        for s in 0..self.f.len() {
            let f = &mut self.f[s];
            // their updates now come on other clocks: start their playback afresh
            f.snaps.clear();
            f.clock_off = None;
            if s != self.local && f.present && !humans.contains(&s) {
                // one of the old host's bots: ours to run now
                f.owned = true;
                f.is_bot = true;
                f.ready = true;
                self.bots[s] = Some(Bot::new(0.6, 977 + s as u64 * 31));
            }
        }
        for h in self.history.iter_mut() {
            h.clear();
        }
        for h in self.pose_history.iter_mut() {
            h.clear();
        }
        // whoever is waiting for a duel, in slot order
        self.queue = (0..self.f.len()).filter(|&s| self.f[s].present && !self.f[s].spectate && !self.mt.duel.contains(&(s as u8))).collect();
        self.host_slot = self.local;
        self.banner = Some(("Host has been usurped".into(), 2.5));
        self.say("The host left: you are hosting now.".to_string());
    }

    /// (player) The host left and `new_host` takes over: their computer runs the match now.
    pub fn follow_new_host(&mut self, old_host: usize, new_host: usize) {
        if old_host < self.f.len() && old_host != self.local && self.f[old_host].present {
            self.remove_player(old_host);
        }
        for f in self.f.iter_mut() {
            f.snaps.clear();
            f.clock_off = None;
        }
        self.host_slot = new_host;
        self.banner = Some(("Host has been usurped".into(), 2.5));
        if new_host < self.f.len() {
            let nm = self.f[new_host].name.clone();
            self.say(format!("The host left: {nm} is hosting now."));
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
            return Some(self.world.flag_base(my));
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
        if self.showcase.is_some_and(|s| s == a || s == b) {
            return false;
        }
        if self.mt.mode == Mode::Gauntlet {
            // the waves are all against you, never each other
            return a != b && self.f[b].present && !self.f[b].spectate && ((a == self.local) != (b == self.local));
        }
        a != b && self.f[b].present && !self.f[b].spectate && (!self.mt.mode.teams() || self.f[a].team != self.f[b].team)
    }

    /// Closest living enemy, preferring ones in sight.
    pub fn nearest_enemy(&self, i: usize) -> Option<usize> {
        let me = self.f[i].eye();
        let mut best: Option<(f32, usize)> = None;
        for j in 0..self.f.len() {
            if !self.is_enemy(i, j) || !self.f[j].alive || self.f[j].invis_t > 0.0 {
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
        let skid_k = self.tune.skid;
        let mut dust: Vec<(V3, V3)> = Vec::new();
        for i in 0..self.f.len() {
            let f = &mut self.f[i];
            if !f.present || !f.alive || f.spectate {
                f.was_ground = true;
                f.prev_speed = 0.0;
                continue;
            }
            // a hard stop scuffs up a little dust at the feet
            let sp = f.vel.flat().len();
            let braking = f.on_ground && f.wall.is_none() && f.sliding <= 0.0 && f.dash_t <= 0.0 && f.prev_speed > 4.0 && (f.prev_speed - sp) > 20.0 * dt;
            if braking && skid_k > 0.0 {
                f.skid_dust += dt * skid_k;
                while f.skid_dust > 0.02 {
                    f.skid_dust -= 0.02;
                    let back = if f.prev_speed > 0.1 { f.vel.flat() * (1.0 / sp.max(0.1)) } else { V3::ZERO };
                    dust.push((f.pos + v3(0.0, 0.05, 0.0), back));
                }
            } else {
                f.skid_dust = 0.0;
            }
            f.prev_speed = sp;
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
        for (p, d) in dust {
            for _ in 0..2 {
                let v = d * self.rng.range(0.5, 1.8) + v3(self.rng.range(-0.6, 0.6), self.rng.range(0.2, 0.9), self.rng.range(-0.6, 0.6));
                let life = self.rng.range(0.3, 0.55);
                let size = self.rng.range(0.10, 0.2);
                self.particles.push(Particle { pos: p + v3(self.rng.range(-0.15, 0.15), 0.0, self.rng.range(-0.15, 0.15)), vel: v, life, max_life: life, size, color: [150, 140, 122], gravity: 1.2 });
            }
        }
    }

    /// A sound that isn't in the world (hit markers, countdown, jingles).
    fn sfx2d(&mut self, s: Sfx) {
        self.sounds.push((s, None, 1.0));
    }

    /// A melee blow connected near you (you swung it or took it): jolt the camera along it.
    pub fn jolt(&mut self, dir: V3, k: f32) {
        if k >= self.jolt_k * (self.jolt_t / JOLT_TIME) {
            self.jolt_dir = dir.flat().norm();
            self.jolt_k = k;
            self.jolt_t = JOLT_TIME;
        }
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
        let va = self.view_alpha.clamp(0.0, 1.0);
        for (k, ff) in self.f.iter_mut().enumerate() {
            // where the last picture showed them (between the last two ticks)
            let seen = ff.prev_pos.lerp(ff.pos, va);
            if let Some(s) = self.seen_pos.get_mut(k) {
                *s = if (seen - ff.pos).len() < 2.0 { seen } else { ff.pos };
            }
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
                    None if self.train.is_some() => self.dummy_input(i),
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
            if self.f[i].kick_t > 0.0 && !self.f[i].kick_done && self.f[i].kick_t <= KICK_TIME - KICK_HIT_AT {
                self.f[i].kick_done = true;
                self.resolve_kick(i);
            }
            if self.f[i].through_t > 0.0 && self.f[i].alive {
                self.resolve_through(i);
            }
        }
        // enemies are solid to each other; teammates walk through one another
        self.separate_bodies();

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
            ff.throw_t = (ff.throw_t - dt).max(0.0);
            if ff.fin > 0 {
                ff.fin_t += dt;
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
        self.record(dt);
        if self.mt.phase == Phase::MatchOver && self.seen_phase != Phase::MatchOver {
            self.on_match_over();
        }
        if self.mt.phase != Phase::MatchOver && self.seen_phase == Phase::MatchOver {
            // a new match: a new Play of the Match
            self.potm = None;
            self.potm_t = None;
            self.potm_done = false;
            self.hl_pending = None;
            self.rec.clear();
        }
        self.seen_phase = self.mt.phase;
        if let Some((_, t)) = &mut self.banner {
            *t -= dt;
            if *t <= 0.0 {
                self.banner = None;
            }
        }
    }

    /// One drawn frame of play, `dt` seconds long: the kill cam's clock, slow motion, the hit
    /// freeze, then as many fixed ticks as the time adds up to (at most 12, so a slow frame
    /// never snowballs). `acc` carries the leftover time between frames; `inp`'s one-shot
    /// presses are used up by the first tick. Returns how many ticks ran.
    pub fn advance(&mut self, acc: &mut f32, dt: f32, inp: &mut Input) -> usize {
        let dt = if dt.is_finite() { dt.clamp(0.0, 0.1) } else { 0.0 };
        if !acc.is_finite() {
            *acc = 0.0;
        }
        if let Some(kc) = self.killcam.as_mut() {
            kc.t += dt;
        }
        // the Play of the Match plays once the final kill cam is over
        if self.mt.phase == Phase::MatchOver && !self.potm_done {
            let cam_over = self.killcam.as_ref().map_or(true, |k| k.t >= KILLCAM_TIME);
            if let Some(h) = self.potm.as_ref().filter(|_| cam_over) {
                let len = h.duration();
                let t = self.potm_t.map_or(0.0, |t| t + dt);
                if t >= len {
                    self.potm_t = None;
                    self.potm_done = true;
                } else {
                    self.potm_t = Some(t);
                }
            }
        }
        let mut scale = 1.0;
        if self.slowmo > 0.0 {
            self.slowmo -= dt;
            scale = 0.25;
        }
        if self.hitstop > 0.0 {
            self.hitstop -= dt;
        } else {
            *acc += dt * scale;
        }
        let mut steps = 0;
        while *acc >= TICK && steps < 12 {
            self.tick(inp);
            self.view_alpha = 1.0;
            // presses count once
            inp.jump = false;
            inp.dash = false;
            inp.ability = false;
            inp.fire_pressed = false;
            inp.reload = false;
            inp.item = false;
            inp.kick = false;
            inp.finish = false;
            inp.select = None;
            *acc -= TICK;
            steps += 1;
        }
        if steps == 12 {
            *acc = 0.0;
        }
        steps
    }

    /// Test switch (DUEL_FINAL_SHOT): set up a bot's gunshot as the match-winning kill on you.
    /// You're on 1 HP, the first enemy bot is one kill (or round) from winning, 9 m away with a
    /// clear view of you and `gun` in hand.
    pub fn stage_final_shot(&mut self, gun: Weapon) {
        if self.mt.phase != Phase::Fight {
            return;
        }
        let me = self.local;
        let Some(sh) = (0..self.f.len()).find(|&s| s != me && self.f[s].present && self.f[s].alive && !self.f[s].spectate && self.f[s].is_bot && self.is_enemy(s, me)) else { return };
        let t = self.mt.target as i32;
        match self.mt.mode {
            Mode::Ffa | Mode::Gladiator => self.f[sh].kills = self.f[sh].kills.max(t - 1),
            Mode::Tdm => {
                let team = (self.f[sh].team as usize).min(1);
                self.mt.team_score[team] = self.mt.team_score[team].max((t - 1).max(0) as _);
            }
            Mode::Duel | Mode::Lms => self.f[sh].wins = self.f[sh].wins.max(t - 1),
            _ => {}
        }
        let here = self.f[me].pos;
        let there = self.f[sh].pos;
        let d = (there - here).flat().len();
        let clear = |p: V3| self.world.line_clear(here + v3(0.0, 1.2, 0.0), p + v3(0.0, 1.5, 0.0));
        if d < 6.0 || d > 12.0 || !clear(there) {
            let ground = if self.f[me].on_ground { here.y } else { there.y.min(here.y) };
            for j in 0..16 {
                let a = j as f32 * std::f32::consts::PI / 8.0;
                let p = v3(here.x + a.sin() * 9.0, ground, here.z + a.cos() * 9.0);
                if p.x.abs() < self.world.half - 1.0 && p.z.abs() < self.world.half - 1.0 && clear(p) && self.world.line_clear(p + v3(0.0, 0.3, 0.0), p + v3(0.0, 1.9, 0.0)) {
                    self.f[sh].pos = p;
                    self.f[sh].prev_pos = p;
                    self.f[sh].vel = V3::ZERO;
                    break;
                }
            }
        }
        for s in 0..self.f.len() {
            if s != me && s != sh && self.f[s].present && self.f[s].alive {
                self.f[s].pos = v3(-14.0 + s as f32 * 2.0, self.f[s].pos.y, -14.0);
                self.f[s].vel = V3::ZERO;
            }
        }
        let f = &mut self.f[me];
        f.hp = f.hp.min(1.0);
        f.spawn_prot = 0.0;
        f.regen_left = 0.0;
        let b = &mut self.f[sh];
        b.spawn_prot = 0.0;
        b.items = 0;
        b.loadout.gun1 = gun;
        b.loadout.gun2 = gun;
        if b.cur == 0 {
            b.cur = 1;
        }
        b.weapon = gun;
        b.mag = [b.mag[0], 30, 30];
        b.reload_t = 0.0;
    }

    /// The final kill cam's view while it runs: circling the fallen player in slow motion.
    /// (camera position, the point it looks at)
    pub fn killcam_view(&self, alpha: f32) -> Option<(V3, V3)> {
        let kc = self.killcam.as_ref()?;
        if !(kc.t < KILLCAM_TIME) || kc.victim >= self.f.len() || !self.f[kc.victim].present {
            return None;
        }
        let v = &self.f[kc.victim];
        let target = v.prev_pos.lerp(v.pos, alpha.clamp(0.0, 1.0)) + v3(0.0, 0.9, 0.0);
        let a = v.yaw + 2.2 + kc.t * 0.55;
        let mut out = v3(a.sin(), 0.0, a.cos());
        let mut dist = (4.4 - kc.t * 0.35).max(1.0);
        if let Some((t, _)) = self.world.raycast(target, (out + v3(0.0, 0.35, 0.0)).norm(), dist + 0.3) {
            dist = (t - 0.3).max(1.0);
        }
        out = (out + v3(0.0, 0.35, 0.0)).norm();
        let pos = target + out * dist;
        let ok = |p: V3| p.x.is_finite() && p.y.is_finite() && p.z.is_finite();
        if !ok(pos) || !ok(target) {
            return None;
        }
        Some((pos, target))
    }

    /// Does this fighter's body block enemies right now? (Not while dead, watching, cutting
    /// through with a katana dash, doing or caught in a finisher.)
    fn solid_body(&self, i: usize) -> bool {
        let f = &self.f[i];
        f.present && f.alive && !f.spectate && f.through_t <= 0.0 && f.fin == 0 && f.held_by.is_none()
    }

    /// Enemies can't run through each other: a fighter of ours overlapping an enemy's body is
    /// pushed back out (half each when both are ours, all of it when the enemy is someone else's,
    /// whose own computer does the same). Teammates pass through freely.
    fn separate_bodies(&mut self) {
        let n = self.f.len();
        let reach = RADIUS * 2.0;
        for i in 0..n {
            if !self.f[i].owned || !self.solid_body(i) {
                continue;
            }
            for j in 0..n {
                if i == j || !self.solid_body(j) || !self.is_enemy(i, j) {
                    continue;
                }
                let (a, b) = (self.f[i].pos, self.f[j].pos);
                // bodies overlap only at about the same height (you can jump over someone)
                if (a.y - b.y).abs() > HEIGHT * 0.8 {
                    continue;
                }
                let d = (a - b).flat();
                let dist = d.len();
                if dist >= reach {
                    continue;
                }
                let dir = if dist > 1e-4 { d * (1.0 / dist) } else { yaw_fwd(self.f[i].yaw + i as f32 * 2.1) * -1.0 };
                let share = if self.f[j].owned { 0.5 } else { 1.0 };
                let to = a + dir * ((reach - dist) * share);
                // never pushed into a wall
                let clear = [0.3f32, 1.2].iter().all(|h| self.world.line_clear(a + v3(0.0, *h, 0.0), to + v3(0.0, *h, 0.0) + dir * RADIUS));
                if !clear {
                    continue;
                }
                let f = &mut self.f[i];
                f.pos = to;
                // stop running into them (sliding along is fine)
                let into = -f.vel.flat().dot(dir);
                if into > 0.0 {
                    f.vel = f.vel + dir * into;
                }
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
        // Writs: a streak of WRIT_STREAK kills marks you; whoever takes you down claims it
        let writ_modes = matches!(self.mt.mode, Mode::Ffa | Mode::Tdm) && self.kind != Kind::Demo && self.train.is_none();
        let held = self.streaks.get(victim).copied().unwrap_or(0);
        let claimed = writ_modes && held >= WRIT_STREAK;
        if let Some(s) = self.streaks.get_mut(victim) {
            *s = 0;
        }
        if let Some(s) = self.kill_chain.get_mut(victim) {
            *s = (0, -10.0);
        }
        if let Some(kk) = k.filter(|&kk| kk != victim && self.is_enemy(kk, victim)) {
            // Corvin Vale's passive: a kill reloads every gun at once
            if self.abilities_on() && self.f[kk].owned && Ability::of(self.f[kk].loadout.body) == Ability::Mark {
                let f = &mut self.f[kk];
                for s in 1..3 {
                    f.mag[s] = f.slot_weapon(s).mag();
                }
                f.reload_t = 0.0;
                if kk == self.local {
                    self.sfx2d(Sfx::ReloadEnd);
                }
            }
            let now = self.time;
            let n = {
                let s = &mut self.streaks[kk];
                *s += 1;
                *s
            };
            let chain = {
                let c = &mut self.kill_chain[kk];
                *c = if now - c.1 < 4.0 { (c.0 + 1, now) } else { (1, now) };
                c.0
            };
            let kn = self.f[kk].name.clone();
            let vn = self.f[victim].name.clone();
            if claimed {
                self.feed.push(FeedLine { killer: None, victim: (String::new(), 0), tool: "", head: false, text: Some(format!("WRIT CLAIMED  {kn} took {vn}'s Writ ({held} kills)")), life: 6.0 });
                if kk == self.local {
                    self.career.push(CareerEv::WritClaim { streak: held });
                    self.banner = Some(("WRIT CLAIMED".into(), 1.6));
                }
                self.highlight(kk, 180.0 + 30.0 * held as f32, "WRIT CLAIMED".into());
            }
            if writ_modes && n == WRIT_STREAK {
                self.feed.push(FeedLine { killer: None, victim: (String::new(), 0), tool: "", head: false, text: Some(format!("WRIT  {kn} is on a {n}-kill streak: take them down for XP")), life: 6.0 });
                if kk == self.local {
                    self.banner = Some(("YOU HOLD A WRIT".into(), 2.0));
                }
            }
            // the Play of the Match: multi-kills, finishers, or at least a good kill
            let what = match chain {
                2 => "DOUBLE KILL",
                3 => "TRIPLE KILL",
                4 => "QUAD KILL",
                c if c >= 5 => "RAMPAGE",
                _ if w == EXEC_TOOL => "EXECUTION",
                _ if head => "HEADSHOT",
                _ => "ELIMINATION",
            };
            let score = if chain >= 2 { 100.0 * chain as f32 } else if w == EXEC_TOOL { 220.0 } else if head { 70.0 } else { 50.0 };
            self.highlight(kk, score + if w == EXEC_TOOL && chain >= 2 { 60.0 } else { 0.0 }, what.into());
            if kk == self.local && chain == 2 {
                self.career.push(CareerEv::MultiKill);
            }
        }
        if k == Some(self.local) && victim != self.local && self.is_enemy(self.local, victim) {
            self.streak = if self.time - self.last_kill_time < 4.0 { self.streak + 1 } else { 1 };
            self.last_kill_time = self.time;
            self.kill_t = 1.6;
            self.heal_pop = 1.6;
            self.hitmarker = self.hitmarker.max(0.35); // the hit confirm turns red
            self.kill_name = self.f[victim].name.clone();
            self.sfx2d(Sfx::Kill);
        }
        if k == Some(self.local) && victim != self.local {
            let fin = if w == EXEC_TOOL { self.f[self.local].fin.max(self.last_finisher) } else { 0 };
            self.career.push(CareerEv::Kill { head, finisher: fin });
        }
        if victim == self.local {
            self.career.push(CareerEv::Death);
        }
        if let Some(gt) = self.gauntlet.as_mut() {
            if k == Some(self.local) && victim != self.local && !gt.over {
                gt.score += gauntlet_kill_points(gt.wave, head);
                gt.kills += 1;
            }
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
            Mode::Gauntlet => false, // it ends when you fall
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

    /// Abilities and passives are in play (any mode, Duels too, unless the lobby is Classic).
    pub fn abilities_on(&self) -> bool {
        !self.mt.classic
    }

    /// The guard meter's top for this fighter (Bram's passive raises it).
    pub fn guard_max_of(&self, f: &Fighter) -> f32 {
        if self.abilities_on() && Ability::of(f.loadout.body) == Ability::Wall { GUARD_MAX * (1.0 + self.tune.guard_bonus) } else { GUARD_MAX }
    }

    /// (host) a player's ability use holds up: their fighter's own, abilities on, alive, a
    /// charge free (with a little slack for lag), and where it happened in reach.
    pub fn ability_ok(&self, w: usize, kind: Ability, p: V3) -> bool {
        let f = &self.f[w];
        if !self.abilities_on() || !f.alive || Ability::of(f.loadout.body) != kind {
            return false;
        }
        let cd = kind.cooldown(&self.tune) * 0.85;
        let recent = self.ab_uses[w].iter().filter(|&&t| self.time - t < cd).count();
        if recent as f32 >= kind.charges(&self.tune) {
            return false;
        }
        let reach = if kind == Ability::Grapple { self.tune.grapple_range + 4.0 } else { 4.0 };
        (p - f.pos).len() <= reach
    }

    /// What an ability use looks and sounds like (and leaves in the arena), on every computer.
    fn ability_fx(&mut self, i: usize, kind: Ability, p: V3, d: V3) {
        let owned = self.f[i].owned;
        let at = self.f[i].chest();
        match kind {
            Ability::Mark => {
                self.marks.push(Mark { pos: p, vel: clamp_len(d, 40.0), owner: i, landed: false, t: 0.0 });
                self.sfx(Sfx::Throw, p);
            }
            Ability::Grapple => {
                if !owned {
                    self.f[i].grapple = Some(p);
                    self.f[i].grapple_t = 0.0;
                }
                self.sfx(Sfx::GrappleFire, at);
                self.burst(p, [230, 220, 200], 8, 3.0);
                self.note(i, |s| s.grapples += 1);
            }
            Ability::Wall => {
                if !owned {
                    self.f[i].shield_t = self.tune.shield_time;
                }
                self.sfx(Sfx::ShieldUp, at);
                self.burst(at + self.f[i].fwd() * 0.6, [120, 190, 255], 18, 4.0);
            }
            Ability::Afterimage => {
                if !owned {
                    self.f[i].invis_t = self.tune.invis_time;
                }
                let dd = clamp_len(d, 15.0);
                self.decoys.push(Decoy { pos: p, vel: v3(dd.x, 0.0, dd.z), yaw: dd.x.atan2(dd.z), owner: i, t: 0.0 });
                self.sfx(Sfx::Vanish, at);
                for _ in 0..14 {
                    let v = v3(self.rng.range(-1.2, 1.2), self.rng.range(0.2, 1.4), self.rng.range(-1.2, 1.2));
                    self.particles.push(Particle { pos: at, vel: v, life: 0.6, max_life: 0.6, size: 0.35, color: [150, 140, 170], gravity: -0.4 });
                }
                self.note(i, |s| s.afterimages += 1);
            }
        }
    }

    /// Marks fly and stick, decoys run and fade, scopes are timed (for the sniper glint).
    fn update_abilities(&mut self, dt: f32) {
        let gravity = self.tune.gravity;
        let mut landed = Vec::new();
        for (k, m) in self.marks.iter_mut().enumerate() {
            if m.landed {
                m.t -= dt;
                continue;
            }
            m.t += dt;
            let step = m.vel * dt;
            let len = step.len();
            let hit = if len > 1e-5 { self.world.raycast(m.pos, step * (1.0 / len), len + 0.05) } else { None };
            match hit {
                Some((ht, _)) => {
                    m.pos = m.pos + step * ((ht - 0.05).max(0.0) / len);
                    m.landed = true;
                    m.t = self.tune.mark_time;
                    landed.push(k);
                }
                None => {
                    m.pos = m.pos + step;
                    m.vel.y -= gravity * dt;
                    if m.t > 2.0 {
                        m.landed = true;
                        m.t = self.tune.mark_time;
                        landed.push(k);
                    }
                }
            }
        }
        for k in landed {
            let (p, owner) = (self.marks[k].pos, self.marks[k].owner);
            self.sfx(Sfx::MarkPing, p);
            self.burst(p, [255, 90, 70], 16, 5.0);
            let r = self.tune.mark_radius;
            if (0..self.f.len()).any(|j| self.f[j].alive && self.is_enemy(owner, j) && (self.f[j].pos - p).len() <= r) {
                self.note(owner, |s| s.marks += 1);
            }
        }
        self.marks.retain(|m| !m.landed || m.t > 0.0);
        let life = self.tune.decoy_time;
        for d in self.decoys.iter_mut() {
            d.t += dt;
            let step = d.vel * dt;
            let len = step.len();
            if len > 1e-5 && self.world.raycast(d.pos + v3(0.0, 1.0, 0.0), step * (1.0 / len), len + 0.4).is_some() {
                d.vel = V3::ZERO; // ran into a wall: stops there
            } else {
                d.pos = d.pos + step;
            }
            let ground = self.world.ground_below(d.pos + v3(0.0, 0.8, 0.0));
            if (d.pos.y - ground).abs() < 1.5 {
                d.pos.y = ground;
            }
        }
        self.decoys.retain(|d| d.t < life);
        let now = self.time;
        for (i, f) in self.f.iter().enumerate() {
            if let Some(s) = self.scoped_since.get_mut(i) {
                *s = if f.scope_t > 0.0 && f.alive { if *s < 0.0 { now } else { *s } } else { -1.0 };
            }
        }
        for u in self.ab_uses.iter_mut() {
            u.retain(|&t| now - t < 120.0);
        }
    }

    /// Does `viewer` see fighter `j` through walls (a Warden's Mark of theirs or a teammate's)?
    pub fn revealed_to(&self, viewer: usize, j: usize) -> bool {
        let r = self.tune.mark_radius;
        self.f.get(j).is_some_and(|f| f.alive) && self.marks.iter().any(|m| m.landed && m.owner < self.f.len() && !self.is_enemy(viewer, m.owner) && self.is_enemy(m.owner, j) && (self.f[j].pos - m.pos).len() <= r)
    }

    /// A decoy drawn as its owner: running, then faking a swing.
    pub fn decoy_fighter(&self, d: &Decoy) -> Option<Fighter> {
        let mut f = self.f.get(d.owner)?.clone();
        f.pos = d.pos;
        f.prev_pos = d.pos;
        f.vel = d.vel;
        f.yaw = d.yaw;
        f.alive = true;
        f.on_ground = true;
        f.invis_t = 0.0;
        f.shield_t = 0.0;
        f.grapple = None;
        f.guard = false;
        f.fin = 0;
        f.cur = 0;
        f.weapon = f.loadout.melee;
        let fake = d.t - 0.55;
        f.slash_t = if (0.0..f.weapon.slash_dur()).contains(&fake) { f.weapon.slash_dur() - fake } else { 0.0 };
        f.slash_idx = 0;
        Some(f)
    }

    /// Someone holds a Writ right now.
    pub fn has_writ(&self, j: usize) -> bool {
        matches!(self.mt.mode, Mode::Ffa | Mode::Tdm) && self.kind != Kind::Demo && self.train.is_none() && self.f.get(j).is_some_and(|f| f.present && f.alive) && self.streaks.get(j).is_some_and(|&s| s >= WRIT_STREAK)
    }

    /// A moment worth replaying: kept if it beats the best so far (it's saved once its tail
    /// has been recorded).
    fn highlight(&mut self, player: usize, score: f32, what: String) {
        if self.kind == Kind::Demo || self.train.is_some() || self.mt.phase != Phase::Fight {
            return;
        }
        let best = self.potm.as_ref().map_or(0.0, |h| h.score).max(self.hl_pending.as_ref().map_or(0.0, |p| p.0));
        if score > best {
            self.hl_pending = Some((score, player, what, self.time));
        }
    }

    /// Keep the last few seconds (during a fight), and seal a waiting highlight once its tail
    /// is in.
    fn record(&mut self, dt: f32) {
        if self.kind == Kind::Demo || self.train.is_some() || self.mt.phase != Phase::Fight {
            return;
        }
        // a long flow chain is a moment too
        for i in 0..self.f.len().min(self.flow_noted.len()) {
            let f = &self.f[i];
            if f.present && f.alive && f.flow >= FLOW_MAX && !self.flow_noted[i] {
                self.flow_noted[i] = true;
                self.highlight(i, 90.0 + 10.0 * f.flow as f32, "FLOW CHAIN".into());
            } else if self.f[i].flow == 0 {
                self.flow_noted[i] = false;
            }
        }
        self.rec_acc += dt;
        if self.rec_acc >= REC_DT {
            self.rec_acc -= REC_DT;
            self.rec.push_back(RecFrame { t: self.time, f: self.f.clone(), tracers: self.tracers.clone(), particles: self.particles.clone() });
            let keep = POTM_BEFORE + POTM_AFTER + 0.5;
            while self.rec.front().is_some_and(|fr| fr.t < self.time - keep) {
                self.rec.pop_front();
            }
        }
        if self.hl_pending.as_ref().is_some_and(|p| self.time >= p.3 + POTM_AFTER) {
            self.seal_highlight();
        }
    }

    fn seal_highlight(&mut self) {
        let Some((score, player, what, at)) = self.hl_pending.take() else { return };
        let frames: Vec<RecFrame> = self.rec.iter().filter(|fr| fr.t >= at - POTM_BEFORE && fr.t <= at + POTM_AFTER).cloned().collect();
        if frames.len() < 4 || player >= self.f.len() {
            return;
        }
        let (name, color) = (self.f[player].name.clone(), self.f[player].color_idx);
        self.potm = Some(Highlight { player, name, color, what, score, frames });
    }

    /// The match just ended (here, or the host said so): count it, and pick the Play of the Match.
    fn on_match_over(&mut self) {
        if self.kind == Kind::Demo {
            return;
        }
        self.seal_highlight();
        self.potm_t = None;
        self.potm_done = self.potm.is_none();
        if self.train.is_none() {
            self.career.push(CareerEv::MatchEnd);
            if self.local_won() {
                self.career.push(CareerEv::MatchWon);
            }
        }
    }

    /// Did the local player (or their team) win the match that just ended?
    pub fn local_won(&self) -> bool {
        let me = &self.f[self.local];
        if !me.present {
            return false;
        }
        match self.mt.mode {
            Mode::Tdm | Mode::Ctf => {
                let t = (me.team as usize).min(1);
                self.mt.team_score[t] > self.mt.team_score[1 - t]
            }
            Mode::Duel | Mode::Lms => self.f.iter().filter(|f| f.present && f.slot != self.local).all(|f| me.wins > f.wins),
            Mode::Gauntlet => false,
            _ => self.f.iter().filter(|f| f.present && f.slot != self.local).all(|f| me.kills > f.kills),
        }
    }

    /// While the Play of the Match plays: put its moment in place of the live one (fighters
    /// between two kept frames, the shots and sparks of the nearest) so it can be drawn; true
    /// if it did. Undo with `potm_view_end`.
    pub fn potm_view_begin(&mut self) -> bool {
        let (Some(t), Some(h)) = (self.potm_t, self.potm.as_ref()) else { return false };
        if self.potm_stash.is_some() || h.frames.is_empty() {
            return false;
        }
        let t0 = h.frames[0].t + t;
        let k = h.frames.iter().position(|fr| fr.t > t0).unwrap_or(h.frames.len() - 1).max(1).min(h.frames.len() - 1);
        let (a, b) = (&h.frames[k - 1], &h.frames[k]);
        let w = ((t0 - a.t) / (b.t - a.t).max(1e-4)).clamp(0.0, 1.0);
        let mut f = a.f.clone();
        for (x, y) in f.iter_mut().zip(b.f.iter()) {
            if (y.pos - x.pos).len() < 3.0 {
                x.pos = x.pos.lerp(y.pos, w);
                x.yaw += crate::math::wrap_angle(y.yaw - x.yaw) * w;
                x.pitch += (y.pitch - x.pitch) * w;
                x.vel = x.vel.lerp(y.vel, w);
            }
            x.prev_pos = x.pos;
        }
        let fr = if w < 0.5 { a } else { b };
        let (tracers, particles) = (fr.tracers.clone(), fr.particles.clone());
        let time = a.t + (b.t - a.t) * w;
        let live_f = std::mem::replace(&mut self.f, f);
        let live_tr = std::mem::replace(&mut self.tracers, tracers);
        let live_pa = std::mem::replace(&mut self.particles, particles);
        let live_time = std::mem::replace(&mut self.time, time);
        self.potm_stash = Some((live_f, live_tr, live_pa, live_time));
        true
    }

    pub fn potm_view_end(&mut self) {
        if let Some((f, tr, pa, time)) = self.potm_stash.take() {
            self.f = f;
            self.tracers = tr;
            self.particles = pa;
            self.time = time;
        }
    }

    /// Skip the Play of the Match.
    pub fn skip_potm(&mut self) {
        self.potm_t = None;
        self.potm_done = true;
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
            Phase::Fight if self.mt.mode == Mode::Gauntlet => self.update_gauntlet(dt),
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
                        fl.pos = if c < self.f.len() { self.f[c].pos } else { self.world.flag_base(t) };
                        fl.carrier = 255;
                        if c < self.f.len() {
                            let n = self.f[c].name.clone();
                            self.say(format!("{n} dropped the {tname} flag"));
                        }
                    } else {
                        fl.pos = self.f[c].pos;
                        let home = (self.f[c].team as usize).min(1);
                        let near_home = (self.f[c].pos - self.world.flag_base(home)).flat().len() < 1.8;
                        if near_home && self.mt.flags[home].state == 0 {
                            self.mt.team_score[home] += 1;
                            let n = self.f[c].name.clone();
                            self.say(format!("{n} captured the {tname} flag!"));
                            self.banner = Some((format!("{} TEAM SCORES", TEAM_NAMES[home].to_uppercase()), 2.0));
                            fl = Flag { state: 0, carrier: 255, pos: self.world.flag_base(t), timer: 0.0 };
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
                            fl = Flag { state: 0, carrier: 255, pos: self.world.flag_base(t), timer: 0.0 };
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
                                fl = Flag { state: 0, carrier: 255, pos: self.world.flag_base(t), timer: 0.0 };
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
        if self.mt.mode == Mode::Gauntlet {
            // a new run: the bots go, the score starts again (the best is kept)
            let best = self.gauntlet.as_ref().map_or(0, |g| g.best);
            let me = self.local;
            for s in 0..self.f.len() {
                if s != me && self.f[s].present {
                    self.remove_player(s);
                }
            }
            self.gauntlet = Some(Gauntlet { best, ..Default::default() });
        }
        self.mt.number += 1;
        self.streaks = vec![0; MAX_PLAYERS];
        self.kill_chain = vec![(0, -10.0); MAX_PLAYERS];
        self.killcam = None;
        self.slowmo = 0.0;
        self.last_kill = None;
        self.mt.phase = Phase::Loadout;
        self.mt.timer = if self.kind == Kind::Offline { 120.0 } else { 20.0 };
        self.mt.team_score = [0, 0];
        self.mt.round = 0;
        self.mt.round_end = 0.0;
        self.mt.duel = [255, 255];
        self.mt.flags = [0, 1].map(|t| Flag { state: 0, carrier: 255, pos: self.world.flag_base(t), timer: 0.0 });
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
        let tn = self.tune;
        let run = tn.run_speed;

        let cd_before = ff.fire_cd;
        ff.fire_cd = (ff.fire_cd - dt).max(0.0);
        if ff.weapon == Weapon::Shotgun && cd_before > PUMP_AT && ff.fire_cd <= PUMP_AT && ff.alive {
            acts.push(Act::Pump);
        }
        ff.switch_t = (ff.switch_t - dt).max(0.0);
        ff.dash_cd = (ff.dash_cd - dt).max(0.0);
        // spent dashes come back one at a time
        let most = tn.dash_charges.round().max(1.0);
        ff.dash_charges = (ff.dash_charges + dt / tn.dash_recharge.max(0.05)).min(most);
        ff.item_cd = (ff.item_cd - dt).max(0.0);
        ff.hurt_t = (ff.hurt_t - dt).max(0.0);
        ff.stun_t = (ff.stun_t - dt).max(0.0);
        ff.combo_window = (ff.combo_window - dt).max(0.0);
        ff.recoil = (ff.recoil - dt * 8.0).max(0.0);
        ff.since_shot += dt;
        ff.spam = (ff.spam - SPAM_RECOVER * dt).max(0.0);
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
        ff.turn_cd = (ff.turn_cd - dt).max(0.0);
        ff.kick_cd = (ff.kick_cd - dt).max(0.0);
        ff.throw_t = (ff.throw_t - dt).max(0.0);
        ff.slash_lock = (ff.slash_lock - dt).max(0.0);
        if ff.slash_t <= 0.0 && ff.combo_window <= 0.0 {
            ff.basic_n = 0; // the string ended on its own
        }
        ff.guard_broke = (ff.guard_broke - dt).max(0.0);
        ff.through_t = (ff.through_t - dt).max(0.0);
        if ff.kick_t > 0.0 {
            ff.kick_t = (ff.kick_t - dt).max(0.0);
        }
        if ff.stun_t > 0.0 {
            // staggered: no kick, no charge
            ff.kick_t = 0.0;
            ff.charge_t = 0.0;
            ff.charged = false;
        }
        // the guard meter refills a moment after the guard comes down
        if !ff.guard {
            ff.guard_rest += dt;
            if ff.guard_rest > GUARD_REGEN_DELAY {
                ff.guard_hp = (ff.guard_hp + GUARD_REGEN * dt).min(self.guard_max_of(&ff));
            }
        }
        // carried speed: held for a moment after landing, then eased off
        if ff.carry_t > 0.0 {
            ff.carry_t -= dt;
        } else if ff.carry > 0.0 {
            ff.carry -= 14.0 * dt;
            if ff.carry <= run * 0.5 {
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
            ff.vel.y -= tn.gravity * dt;
            self.collide(&mut ff, dt);
            self.f[i] = ff;
            return acts;
        }

        ff.yaw = wrap_angle(inp.yaw);
        ff.pitch = inp.pitch.clamp(-1.45, 1.45);
        // caught in a finisher: held still, facing whoever is doing it
        if let Some(a) = ff.held_by {
            ff.held_t -= dt;
            let attacker_done = a < self.f.len() && self.f[a].fin == 0 && ff.held_t < finisher(ff.held_kind).dur - 0.3;
            if ff.held_t <= 0.0 || attacker_done || !ff.alive {
                ff.held_by = None;
            } else {
                ff.stun_t = ff.stun_t.max(0.1);
                ff.vel.x *= 0.5;
                ff.vel.z *= 0.5;
                if a < self.f.len() {
                    let d = self.f[a].pos - ff.pos;
                    if d.flat().len() > 0.05 {
                        ff.yaw = d.x.atan2(d.z);
                    }
                }
            }
        }
        // doing a finisher: it plays out, nothing else until it's done
        if ff.fin > 0 {
            let style = finisher(ff.fin);
            ff.fin_t += dt;
            let t = ff.fin_target.min(self.f.len() - 1);
            let tp = self.f[t].pos;
            let d = (tp - ff.pos).flat();
            let dir = if d.len() > 0.05 { d.norm() } else { ff.fwd() };
            // step in close, at their height (an aerial one rises to meet them)
            let spot = tp - dir * 1.05;
            ff.pos = ff.pos.lerp(v3(spot.x, if style.weapon.is_none() { tp.y } else { ff.pos.y }, spot.z), 0.3);
            ff.vel = V3::ZERO;
            ff.yaw = dir.x.atan2(dir.z);
            ff.guard = false;
            ff.slash_t = 0.0;
            if !ff.fin_hit && ff.fin_t >= style.dur * style.hit {
                ff.fin_hit = true;
                acts.push(Act::Execute);
            }
            if ff.fin_t >= style.dur || !ff.alive {
                ff.fin = 0;
            }
            if style.weapon.is_none() && ff.fin == 0 {
                ff.on_ground = false; // drop back down after an aerial one
            }
            self.f[i] = ff;
            return acts;
        }
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
        let flow_k = ff.flow_mult_by(tn.flow_step);
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
                if ff.weapon == Weapon::Shotgun {
                    // one shell at a time, until it's full (or you shoot)
                    ff.mag[wi] = (ff.mag[wi] + 1).min(ff.weapon.mag());
                    sounds.push(Sfx::ShellIn);
                    if ff.mag[wi] < ff.weapon.mag() {
                        ff.reload_t = SHELL_TIME;
                    } else {
                        sounds.push(Sfx::Pump);
                    }
                } else {
                    ff.mag[wi] = ff.weapon.mag();
                    sounds.push(Sfx::ReloadEnd);
                }
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
        ff.guard = can_act && melee && inp.guard && !ff.slashing() && ff.switch_t <= 0.0 && ff.kick_t <= 0.0 && ff.guard_hp > 0.0 && ff.charge_t < 0.2;
        ff.guard_t = if ff.guard { ff.guard_t + dt } else { 0.0 };
        ff.stagger_t = (ff.stagger_t - dt).max(0.0);
        let scoping = can_act && !melee && inp.guard && ff.weapon.zoom() > 1.0 && ff.reload_t <= 0.0 && ff.switch_t <= 0.0;
        if scoping && ff.scope_t <= 0.0 && ff.weapon == Weapon::Sniper {
            sounds.push(Sfx::ScopeIn);
        }
        ff.scope_t = if scoping { ff.scope_t + dt } else { 0.0 };

        // ---- the ability (Q / LB): charges come back one at a time
        let ab = Ability::of(ff.loadout.body);
        let ab_on = self.abilities_on();
        let most = ab.charges(&tn);
        ff.ab_charge = if ab_on { (ff.ab_charge + dt / ab.cooldown(&tn).max(0.5)).min(most) } else { most };
        ff.shield_t = (ff.shield_t - dt).max(0.0);
        ff.invis_t = (ff.invis_t - dt).max(0.0);
        if inp.ability && ab_on && can_act && ff.alive && ff.ab_charge >= 1.0 && ff.fin == 0 && ff.held_by.is_none() {
            let eye = ff.eye();
            match ab {
                Ability::Mark => {
                    let (_, dir) = self.shot_ray(i);
                    acts.push(Act::Ability { kind: ab, p: eye + ff.fwd() * 0.4, d: dir * 22.0 + v3(0.0, 4.0, 0.0) });
                    ff.ab_charge -= 1.0;
                }
                Ability::Grapple => {
                    // the line goes where the crosshair is, if there's something to hook within range
                    let (o, dir) = self.shot_ray(i);
                    let reach = tn.grapple_range + (o - eye).len();
                    if let Some((t, _)) = self.world.raycast(o, dir, reach) {
                        let anchor = o + dir * (t - 0.05);
                        let d = (anchor - eye).len();
                        if d <= tn.grapple_range + 0.5 && d > 2.0 {
                            acts.push(Act::Ability { kind: ab, p: anchor, d: dir });
                            ff.ab_charge -= 1.0;
                            ff.grapple = Some(anchor);
                            ff.grapple_t = 0.0;
                            // off the ground and on the way, keeping the speed you had
                            let to = (anchor - ff.chest()).norm();
                            ff.vel = ff.vel * 0.7 + to * (tn.grapple_pull * 0.55) + v3(0.0, 2.5, 0.0);
                            ff.on_ground = false;
                            ff.wall = None;
                            ff.sliding = 0.0;
                        }
                    }
                }
                Ability::Wall => {
                    ff.shield_t = tn.shield_time;
                    ff.guard = false;
                    acts.push(Act::Ability { kind: ab, p: ff.pos, d: ff.fwd() });
                    ff.ab_charge -= 1.0;
                }
                Ability::Afterimage => {
                    ff.invis_t = tn.invis_time;
                    let run = ff.fwd() * (tn.run_speed.max(6.0) * 1.1);
                    acts.push(Act::Ability { kind: ab, p: ff.pos, d: run });
                    ff.ab_charge -= 1.0;
                }
            }
        }
        // the grapple pulls (on top of what you were doing); jump lets go, keeping the speed
        if let Some(a) = ff.grapple {
            ff.grapple_t += dt;
            let to = a - ff.chest();
            let dist = to.len();
            if !ff.alive || inp.jump || dist < 1.4 || ff.grapple_t > 1.5 || ff.stun_t > 0.0 {
                if inp.jump && ff.alive {
                    ff.vel.y = ff.vel.y.max(5.5);
                }
                ff.grapple = None;
            } else {
                let dir = to * (1.0 / dist);
                let along = ff.vel.dot(dir);
                if along < tn.grapple_pull {
                    ff.vel = ff.vel + dir * (tn.grapple_pull - along).min(70.0 * dt);
                }
                ff.vel.y += tn.gravity * 0.85 * dt; // the line carries your weight
                ff.on_ground = false;
            }
        }

        // finisher: on a weakened, staggered enemy in reach
        if inp.finish && can_act && ff.fin == 0 && ff.held_by.is_none() {
            self.f[i] = ff.clone();
            if let Some(t) = self.finish_target(i) {
                let b = &self.f[t];
                let airborne = !b.on_ground && b.pos.y - self.world.ground_below(b.pos + v3(0.0, 0.2, 0.0)) > 0.8;
                let blade = ff.loadout.melee;
                let fits = |k: usize| if airborne { FINISHERS[k].weapon.is_none() } else { FINISHERS[k].weapon == Some(blade) };
                // your own finishers: the ones your fighter's mastery has opened
                let open = |k: usize| i != self.local || self.fin_locks & (1 << k) == 0;
                let mut choices: Vec<usize> = (0..FINISHERS.len()).filter(|&k| fits(k) && open(k)).collect();
                if choices.is_empty() {
                    choices = (0..FINISHERS.len()).filter(|&k| fits(k)).collect();
                }
                let pick = choices[(self.rng.next_u32() as usize) % choices.len().max(1)];
                ff.fin = pick as u8 + 1;
                self.last_finisher = ff.fin;
                ff.fin_t = 0.0;
                ff.fin_target = t;
                ff.fin_hit = false;
                ff.cur = 0;
                ff.weapon = blade;
                ff.switch_t = 0.0;
                ff.reload_t = 0.0;
                cancel_slash(&mut ff);
                ff.guard = false;
                ff.charge_t = 0.0;
                ff.charged = false;
                acts.push(Act::FinishStart);
            }
        }

        // kick: quick and short, any weapon in hand
        if inp.kick && can_act && ff.kick_t <= 0.0 && ff.kick_cd <= 0.0 && !slash_locked && ff.dash_wind <= 0.0 && ff.switch_t <= 0.0 {
            cancel_slash(&mut ff);
            ff.kick_t = KICK_TIME;
            ff.kick_cd = KICK_CD;
            ff.basic_n = 0;
            ff.kick_done = false;
            ff.guard = false;
            ff.charge_t = 0.0;
            ff.charged = false;
            ff.vel.x = ff.vel.x * 0.4 + fwd.x * 3.0;
            ff.vel.z = ff.vel.z * 0.4 + fwd.z * 3.0;
            self.note(i, |s| s.kicks += 1);
            sounds.push(Sfx::Slash);
        }

        // dash / tumble. A ground dash crouches for a split second first (the anticipation),
        // then bursts out.
        let mut fire_dash: Option<V3> = None;
        if ff.dash_wind > 0.0 {
            ff.dash_wind -= dt;
            if !can_act || ff.wall.is_some() {
                ff.dash_wind = 0.0;
                ff.dash_dir = None;
            } else if ff.dash_wind <= 0.0 {
                ff.dash_wind = 0.0;
                fire_dash = ff.dash_dir.take();
            }
        }
        let wants_dash = ff.dash_buf > 0.0 && can_act && ff.dash_cd <= 0.0 && ff.dash_charges >= 1.0 && ff.wall.is_none() && !slash_locked && ff.dash_wind <= 0.0;
        if wants_dash && ff.on_ground && tn.dash_windup > 0.0 && fire_dash.is_none() {
            let dir = if wish.len() > 0.1 { wish.norm() } else { fwd };
            ff.dash_wind = tn.dash_windup;
            ff.dash_dir = Some(dir);
            ff.dash_perfect = perfect_window;
            ff.dash_buf = 0.0;
            cancel_slash(&mut ff);
            // sink into the crouch: brake a little
            ff.vel.x *= 0.85;
            ff.vel.z *= 0.85;
        } else if wants_dash || fire_dash.is_some() {
            let perfect_window = if fire_dash.is_some() { ff.dash_perfect } else { perfect_window };
            let dir = fire_dash.unwrap_or(if wish.len() > 0.1 { wish.norm() } else { fwd });
            let mut dashed = false;
            if ff.on_ground {
                let boost = if perfect_window { 1.12 } else { 1.0 };
                ff.vel = dir * (tn.dash_speed * flow_k * boost) + v3(0.0, 2.2, 0.0);
                ff.on_ground = false;
                ff.dash_t = tn.dash_time;
                ff.dash_cd = 0.38;
                ff.ground_dash_t = 0.45; // a slash now becomes a rising launcher (katana: during the burst, a dash-through)
                ff.sliding = 0.0;
                if perfect_window {
                    ff.perfect_t = 0.6;
                    sounds.push(Sfx::Perfect);
                    self.note(i, |s| s.perfects += 1);
                }
                dashed = true;
            } else if !ff.air_dash {
                ff.vel = dir * (tn.dash_speed * 0.81 * flow_k) + v3(0.0, 3.5, 0.0);
                ff.air_dash = true;
                ff.dash_t = tn.dash_time * 0.9;
                ff.dash_cd = 0.32;
                dashed = true;
            }
            if dashed {
                ff.dash_charges = (ff.dash_charges - 1.0).max(0.0);
                // a dash cancels a charged heavy, and starts a fresh string
                ff.basic_n = 0;
                ff.charge_t = 0.0;
                ff.charged = false;
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

        let mut speed = run * flow_k;
        if ff.guard {
            speed = 4.5;
        }
        if ff.slashing() {
            speed = 5.5;
        }
        if ff.kick_t > 0.0 {
            speed = 3.0;
        }
        if ff.charge_t > 0.2 && melee {
            speed = speed.min(5.0); // winding up a heavy: a little slower
        }

        let mut gravity = true;
        if let Some(mut w) = ff.wall {
            w.t += dt;
            let touching = self.touching_wall(ff.pos).map_or(false, |n| n.dot(w.normal) > 0.7);
            let max_t = if w.vertical { tn.wall_time * 0.48 } else { tn.wall_time };
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
                ff.carry = flat.min(run * 1.7 * flow_k);
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
                    ff.carry = sp.min(run * 1.7 * flow_k);
                    ff.carry_t = 0.15;
                }
            } else {
                // a sharp turn at speed costs momentum for a moment
                let sp = cur.len();
                if tn.turn_slow > 0.0 && ff.turn_cd <= 0.0 && sp > run * 0.8 && wish.len() > 0.5 && cur.norm().dot(wish.norm()) < 0.2 {
                    ff.vel.x *= 1.0 - tn.turn_slow;
                    ff.vel.z *= 1.0 - tn.turn_slow;
                    ff.carry = 0.0;
                    ff.turn_cd = 0.3;
                }
                let cur = ff.vel.flat();
                let speed = speed * (1.0 - tn.turn_slow * 0.5 * (ff.turn_cd / 0.3));
                // run, holding on to carried speed (it eases off rather than snapping back)
                let cap = speed.max(ff.carry);
                let target = wish * cap;
                let diff = target - cur;
                let carrying = ff.carry > speed;
                let slowing = target.len() + 0.01 < cur.len();
                let acc = if wish.len() > 0.1 {
                    if carrying && slowing { 30.0 } else { tn.accel() }
                } else if carrying {
                    25.0
                } else {
                    tn.decel()
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
                ff.vel.y = tn.jump_vel();
                ff.basic_n = 0;
                if perfect_window {
                    // a perfectly timed bounce
                    ff.vel.y = tn.jump_vel() * 1.12;
                    let flat = ff.vel.flat();
                    let boosted = (flat.len() * 1.08).min(run * 2.2);
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
            ff.vel.y = tn.jump_vel();
            ff.jump_buf = 0.0;
            ff.coyote = 0.0;
            sounds.push(Sfx::Jump);
        } else {
            // air control
            if wish.len() > 0.1 {
                let target = wish * speed.max(ff.vel.flat().len().min(12.0 * flow_k));
                let cur = ff.vel.flat();
                let diff = target - cur;
                let acc = tn.air_control * dt;
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
                    let entry = ff.vel.flat().len().clamp(tn.wall_speed, tn.wall_speed.max(22.0));
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
            ff.vel.y -= tn.gravity * k * dt;
        }
        ff.vel.y = ff.vel.y.max(-40.0);

        let vy_before = ff.vel.y;
        self.collide(&mut ff, dt);
        if ff.on_ground && !grounded_before && ff.slam {
            ff.slam = false;
            ff.slash_t = ff.slash_t.min(ff.weapon.slash_dur() * 0.3);
            acts.push(Act::Slam);
        }
        if ff.on_ground && !grounded_before {
            // how hard: nothing for a hop, full for a long fall
            ff.land_impact = ((-vy_before - 5.0) / 15.0).clamp(0.0, 1.0);
            // landed: from a dash, wall-jump or air dash you keep your speed a moment
            ff.last_air = ff.air_t;
            ff.land_t = 0.0;
            let flat = ff.vel.flat().len();
            if ff.from_move && flat > run * flow_k * 0.95 {
                ff.carry = flat.min(run * 1.7 * flow_k);
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
        // Ilya Fen's passive: the chain fades slower
        let flow_keep = if self.abilities_on() && Ability::of(ff.loadout.body) == Ability::Afterimage { 1.0 / (1.0 - tn.flow_slow.clamp(0.0, 0.9)) } else { 1.0 };
        if ff.on_ground && ff.vel.flat().len() < 1.0 && wish.len() < 0.1 {
            ff.still_t += dt;
            if ff.still_t > 0.2 * flow_keep {
                ff.flow = 0;
            }
        } else {
            ff.still_t = 0.0;
        }
        if ff.ground_t > 1.0 && ff.flow > 0 && ff.sliding <= 0.0 {
            ff.flow_fade += dt;
            if ff.flow_fade > 0.6 * flow_keep {
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
                    // hold attack to charge a heavy; let go to swing it
                    let mut heavy_now = false;
                    if inp.fire && ff.kick_t <= 0.0 && ff.dash_wind <= 0.0 {
                        ff.charge_t += dt;
                        if ff.charge_t >= CHARGE_TIME && !ff.charged {
                            ff.charged = true;
                            sounds.push(Sfx::ChargeReady);
                        }
                    } else {
                        if ff.charged && !inp.fire && !slash_locked {
                            heavy_now = true;
                        }
                        ff.charge_t = 0.0;
                        ff.charged = false;
                    }
                    if heavy_now {
                        ff.slash_queued = false;
                        ff.guard = false;
                        ff.slash_idx = SLASH_HEAVY;
                        // a longer wind-up before the blow lands
                        ff.slash_t = dur * 1.25;
                        ff.slash_hit_done = false;
                        ff.vel.x = fwd.x * 6.0;
                        ff.vel.z = fwd.z * 6.0;
                        acts.push(Act::Slash);
                    }
                    // a slash pressed in the dash's crouch waits for the burst (then it's the launcher)
                    if ff.slash_queued && ff.slash_t < dur * 0.36 && ff.dash_wind <= 0.0 && ff.kick_t <= 0.0 && !heavy_now && ff.slash_lock <= 0.0 {
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
                        } else if ff.weapon == Weapon::Katana && ff.dash_t > 0.0 && ff.ground_dash_t > 0.0 {
                            SLASH_THROUGH
                        } else if ff.ground_dash_t > 0.0 {
                            3
                        } else if airborne && ff.weapon == Weapon::Greatsword && ff.pos.y - self.world.ground_below(ff.pos + v3(0.0, 0.2, 0.0)) > 1.2 {
                            SLASH_SLAM
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
                        // mashing: after three basic swings the string is over; mix in a kick, a
                        // dash, a jump or a heavy (or pause a beat) to keep going
                        let basic = !airborne && (idx == 0 || idx == 1);
                        let winded = basic && ff.basic_n >= BASIC_STRING;
                        if winded {
                            ff.slash_lock = 0.3;
                            ff.basic_n = 0;
                            ff.combo_window = 0.0;
                        } else {
                        ff.basic_n = if basic { ff.basic_n + 1 } else { 0 };
                        if butterfly {
                            self.note(i, |s| s.butterflies += 1);
                        }
                        ff.guard = false;
                        ff.bfly_t = 0.0;
                        ff.slash_idx = idx;
                        ff.slash_t = start;
                        ff.slash_hit_done = false;
                        match idx {
                            SLASH_THROUGH => {
                                // cut straight through: a fast burst forward, no stopping
                                let sp = (ff.vel.flat().len()).max(tn.dash_speed) * 1.1;
                                ff.vel.x = fwd.x * sp;
                                ff.vel.z = fwd.z * sp;
                                ff.dash_t = ff.dash_t.max(0.24);
                                ff.ground_dash_t = 0.0;
                                ff.through_t = 0.26;
                                ff.through_hit = 0;
                                ff.slash_hit_done = true; // hits along the way instead
                            }
                            SLASH_SLAM => {
                                // plunge to the ground
                                ff.vel.x = fwd.x * 2.0;
                                ff.vel.z = fwd.z * 2.0;
                                ff.vel.y = -30.0;
                                ff.float_t = 0.0;
                                ff.slam = true;
                                ff.slash_hit_done = true; // the shockwave does the hitting
                            }
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
                }
                Weapon::Shuriken => {
                    // click: a quick burst of three stars
                    if ff.burst_left == 0 && inp.fire && ff.fire_cd <= 0.0 && ff.reload_t <= 0.0 {
                        if ff.mag[wi] > 0 {
                            ff.burst_left = STAR_BURST;
                            ff.burst_t = 0.0;
                            ff.fire_cd = Weapon::Shuriken.interval();
                            ff.throw_t = THROW_TIME;
                            ff.throw_fan = false;
                        } else {
                            ff.reload_t = Weapon::Shuriken.reload_time();
                            sounds.push(Sfx::Reload);
                        }
                    }
                    if ff.burst_left > 0 {
                        ff.burst_t -= dt;
                        if ff.burst_t <= 0.0 {
                            if ff.mag[wi] > 0 && ff.reload_t <= 0.0 {
                                ff.mag[wi] -= 1;
                                ff.burst_left -= 1;
                                ff.burst_t = STAR_GAP;
                                ff.recoil = 0.6;
                                ff.since_shot = 0.0;
                                acts.push(Act::Star { off: 0.0 });
                            } else {
                                ff.burst_left = 0;
                            }
                        }
                    }
                    // right click: a fan of three at once
                    let alt = inp.guard;
                    if alt && !ff.alt_was && ff.burst_left == 0 && ff.fire_cd <= 0.0 && ff.reload_t <= 0.0 && ff.mag[wi] > 0 {
                        let n = ff.mag[wi].min(3);
                        ff.mag[wi] -= n;
                        ff.fire_cd = FAN_CD;
                        ff.throw_t = FAN_THROW_TIME;
                        ff.throw_fan = true;
                        ff.recoil = 1.0;
                        ff.since_shot = 0.0;
                        let offs: &[f32] = match n {
                            1 => &[0.0],
                            2 => &[-FAN_ANGLE * 0.5, FAN_ANGLE * 0.5],
                            _ => &[-FAN_ANGLE, 0.0, FAN_ANGLE],
                        };
                        for &off in offs {
                            acts.push(Act::Star { off });
                        }
                    }
                    ff.alt_was = alt;
                    if ff.mag[wi] == 0 && ff.reload_t <= 0.0 && ff.burst_left == 0 {
                        ff.reload_t = Weapon::Shuriken.reload_time();
                        sounds.push(Sfx::Reload);
                    }
                }
                w => {
                    // the scattergun can cut its shell-by-shell reload short with a shot
                    let shell_cancel = w == Weapon::Shotgun && ff.mag[wi] > 0;
                    if inp.fire && ff.fire_cd <= 0.0 && (ff.reload_t <= 0.0 || shell_cancel) {
                        if ff.mag[wi] > 0 {
                            ff.reload_t = 0.0;
                            ff.mag[wi] -= 1;
                            ff.fire_cd = w.interval();
                            let spread = ff.spread(); // what the crosshair showed
                            ff.spam = (ff.spam + w.spam().0).min(SPAM_MAX);
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
        let (shoulder, height, back_dist) = (self.tune.shoulder, self.tune.cam_height, self.tune.cam_dist.max(0.5));
        const PAD: f32 = 0.35;
        let dir = look_dir(yaw, pitch);
        let right = yaw_right(yaw);
        let head = pos + v3(0.0, 1.6, 0.0);
        // keep the shoulder point out of walls on that side (a negative offset is the left shoulder)
        let side_dir = if shoulder >= 0.0 { right } else { -right };
        let mut side = shoulder.abs();
        if let Some((t, _)) = self.world.raycast(head, side_dir, side + PAD) {
            side = (t - PAD).max(0.0);
        }
        let mut pivot = head + side_dir * side;
        let lift = height - 1.6;
        if lift > 0.0 {
            if let Some((t, _)) = self.world.raycast(pivot, V3::UP, lift + PAD) {
                pivot.y += (t - PAD).max(0.0).min(lift);
            } else {
                pivot.y += lift;
            }
        } else {
            pivot.y += lift;
        }
        // pull the camera in when anything is behind it (a few rays, so the edge of the
        // view doesn't poke through walls either)
        let back = -dir;
        let cam_up = dir.cross(right).norm();
        let mut dist = back_dist;
        for off in [V3::ZERO, right * PAD, -right * PAD, cam_up * PAD, -cam_up * PAD] {
            if let Some((t, _)) = self.world.raycast(pivot + off, back, back_dist + PAD) {
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
            // the shapes stand where the player saw the body, not a fraction of a tick ahead
            let at = self.seen_pos.get(i).copied().filter(|s| (*s - f.pos).len() < 2.0 && *s != V3::ZERO).unwrap_or(f.pos);
            let posed = rig.and_then(|r| crate::character::animate(r, &mut self.anim[i], f, at, self.time, &self.tune));
            self.hit_poses[i] = Some(match posed {
                Some(p) => p.hit,
                None => HitPose::standing(at, f.yaw),
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
        if i == self.local {
            // what the screen shows: the camera (with its spring, bob and dip) looking at the
            // point the plain camera line meets. The shot goes down exactly that line, from about
            // the shoulder, so the crosshair is where it hits (edges of a head included).
            let off = self.view_offset;
            let off = if off.len() > 2.0 { off.norm() * 2.0 } else { off };
            let aim_at = cam + dir * self.world.raycast(cam, dir, 300.0).map_or(300.0, |(t, _)| t.max(1.0));
            let eye = cam + off;
            let vdir = (aim_at - eye).norm();
            let start = eye + vdir * (pivot - eye).dot(vdir).max(0.0);
            return (start, vdir);
        }
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
                self.grenades.push(Grenade { pos: o, vel: v, fuse: 5.0, owner: i, kind: 1, vis: V3::ZERO });
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
                // the shot follows the crosshair line; the tracer and flash come from the gun
                let ff = &self.f[i];
                let muzzle = ff.pos + v3(0.0, 1.38, 0.0) + yaw_right(ff.yaw) * 0.28 + ff.fwd() * 0.45;
                if !ff.is_bot {
                    self.tracer_from = Some(muzzle);
                }
                if w == Weapon::Shotgun {
                    self.fire_pattern(i, o, dir, spread);
                } else {
                    for _ in 0..pellets {
                        let d = pellet_dir(dir, right, spread, &mut self.rng);
                        self.fire_ray(i, o, d, dmg, range, w);
                    }
                }
                self.tracer_from = None;
                let o = if self.f[i].is_bot { o } else { muzzle };
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
                // spent shells fly out to the right (the scattergun's when its pump racks)
                if w != Weapon::Revolver && w != Weapon::Shotgun {
                    let color = if w == Weapon::Shotgun { [170, 30, 30] } else { [214, 172, 70] };
                    let v = right * self.rng.range(2.0, 3.2) + v3(0.0, self.rng.range(2.0, 3.0), 0.0) - dir * 0.5;
                    self.casings.push(Casing { pos: o - dir * 0.35 + right * 0.05, vel: v, life: 2.0, spin: self.rng.range(0.0, 6.0), color });
                    if self.casings.len() > 80 {
                        self.casings.remove(0);
                    }
                }
                if i == self.local && !self.f[i].is_bot {
                    // recoil: the hand cannon, scattergun and sniper kick the aim up (it settles back)
                    self.aim_kick += match w {
                        Weapon::Shotgun => 0.07,
                        Weapon::HandCannon => 0.075,
                        Weapon::Sniper => 0.05,
                        Weapon::Rifle => 0.018,
                        Weapon::Revolver => 0.034,
                        _ => 0.0,
                    };
                }
                if i == self.local {
                    // only a nudge: a shaking view would throw off the aim
                    self.shake = self.shake.max(match w {
                        // the scattergun's boom punches the camera
                        Weapon::Shotgun => 0.4,
                        Weapon::Rifle => 0.2,
                        Weapon::Smg => 0.05,
                        _ => 0.12,
                    });
                }
            }
            Act::Ability { kind, p, d } => {
                self.ability_fx(i, kind, p, d);
                if self.kind == Kind::Host || self.kind == Kind::Client {
                    self.out_events.push(Ev::Ability { who: i as u8, kind: kind as u8, p, d });
                }
            }
            Act::Pump => {
                let ff = &self.f[i];
                let right = yaw_right(ff.yaw);
                let at = ff.pos + v3(0.0, 1.3, 0.0) + right * 0.3 + ff.fwd() * 0.3;
                let v = right * self.rng.range(2.2, 3.4) + v3(0.0, self.rng.range(2.2, 3.2), 0.0) - ff.fwd() * 0.6;
                self.sfx(Sfx::Pump, at);
                self.casings.push(Casing { pos: at, vel: v, life: 2.0, spin: self.rng.range(0.0, 6.0), color: [170, 30, 30] });
                if self.casings.len() > 80 {
                    self.casings.remove(0);
                }
            }
            Act::Slash => {
                let p = self.f[i].pos;
                self.sfx(Sfx::Slash, p);
            }
            Act::Slam => self.resolve_slam(i),
            Act::FinishStart => {
                let (t, kind) = (self.f[i].fin_target, self.f[i].fin);
                if self.f[t].owned {
                    self.hold(t, i, kind);
                }
                if self.kind == Kind::Host || self.kind == Kind::Client {
                    self.out_events.push(Ev::Finisher { target: t as u8, kind });
                }
                let p = self.f[i].pos;
                self.sfx(Sfx::ChargeReady, p);
            }
            Act::Execute => self.execute(i),
            Act::Star { off } => self.throw_star(i, off),
            Act::Throw => {
                let tp = self.f[i].pos + v3(0.0, 1.4, 0.0);
                self.sfx(Sfx::Throw, tp);
                let ff = &self.f[i];
                let dir = look_dir(ff.yaw, ff.pitch + 0.15);
                let p = ff.eye() + ff.fwd() * 0.5;
                let v = dir * 17.0 + v3(0.0, 3.0, 0.0) + ff.vel * 0.4;
                self.grenades.push(Grenade { pos: p, vel: v, fuse: 1.7, owner: i, kind: 0, vis: V3::ZERO });
                if self.kind == Kind::Host || self.kind == Kind::Client {
                    self.out_events.push(Ev::Grenade { p, v, kind: 0 });
                }
            }
        }
    }

    /// The scattergun's shot: its ten pellets in their fixed pattern around `dir`.
    pub fn fire_pattern(&mut self, i: usize, o: V3, dir: V3, spread: f32) {
        let w = Weapon::Shotgun;
        let gs = w.gun();
        let mut right = yaw_right(self.f[i].yaw);
        right = right - dir * right.dot(dir);
        if right.len() < 1e-3 {
            right = v3(dir.z, 0.0, -dir.x);
        }
        let right = right.norm();
        let up = dir.cross(right).norm();
        for (x, y) in SHOTGUN_PATTERN {
            let d = (dir + right * (x * spread) + up * (y * spread)).norm();
            self.fire_ray(i, o, d, gs.dmg, gs.range, w);
        }
    }

    /// Where a ray meets fighter `j`'s head grown by `pad`, if it does.
    pub fn ray_head(&self, j: usize, o: V3, d: V3, max_t: f32, pad: f32) -> Option<f32> {
        let pose = self.hit_pose(j);
        pose.caps.iter().filter(|c| c.part == Part::Head).filter_map(|c| ray_capsule(o, d, max_t, c.a, c.b, c.r + pad)).reduce(f32::min)
    }

    fn fire_ray(&mut self, shooter: usize, o: V3, d: V3, dmg: f32, range: f32, w: Weapon) {
        let mut t = range;
        let mut hit_world = false;
        if let Some((wt, _)) = self.world.raycast(o, d, range) {
            t = wt;
            hit_world = true;
        }
        let t_world = t;
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
        // single-shot guns: a bullet that just grazes a head (within HEAD_ASSIST) is a headshot
        if matches!(w, Weapon::Revolver | Weapon::HandCannon | Weapon::Rifle | Weapon::Sniper) && !matches!(victim, Some((_, Part::Head, _))) {
            for j in 0..self.f.len() {
                if !self.f[j].alive || !self.is_enemy(shooter, j) {
                    continue;
                }
                let Some(ht) = self.ray_head(j, o, d, t_world, HEAD_ASSIST) else { continue };
                // in front of whatever else it met, or the same fighter's head just behind the part it hit
                let ok = match victim {
                    None => true,
                    Some((vj, _, _)) if vj == j => ht <= t + 0.3,
                    Some(_) => ht < t,
                };
                if ok {
                    t = ht.min(t_world);
                    victim = Some((j, Part::Head, false));
                }
            }
        }
        // the Iron Wall stops bullets coming at its front
        let mut shielded = false;
        if let Some((j, _, _)) = victim {
            if self.f[j].shield_t > 0.0 && self.f[j].fwd().dot(-d) > 0.25 {
                shielded = true;
                t = (t - 0.55).max(0.05);
                victim = None;
                self.note(j, |s| s.wall_blocks += 1);
            }
        }
        let end = o + d * t;
        let mut spark = 0u8;
        if let Some((j, part, far)) = victim {
            let head = part == Part::Head;
            let gs = w.gun();
            let falloff = match w {
                Weapon::Shotgun => shotgun_falloff(t),
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
            // a guard raised just in time sends the bullet back
            let deflect = blocked && v.weapon.is_melee() && v.guard_t <= DEFLECT_WINDOW;
            let mut drain = 0.0;
            if deflect {
                spark = 6;
            } else if blocked {
                let raw = amount.min(gs.dmg.max(45.0));
                drain = guard_drain(raw);
                amount = raw * 0.12; // a sword guard stops even the big ones
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
            if deflect {
                let back = amount.min(gs.dmg.max(24.0)).min(60.0) * 0.75;
                self.deflect_back(shooter, j, end, back, w);
            } else {
                self.damage_g(shooter, j, amount, knock, 0.0, head && !blocked, end, w as u8, o, d, drain);
            }
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
        if shielded {
            spark = 2;
        }
        let color = if shooter == self.local { [255, 230, 150] } else { [255, 170, 120] };
        let vis = self.tracer_from.unwrap_or(o);
        self.tracers.push(Tracer::shot(vis, end, w, color));
        self.impact_fx(end, spark);
        if spark == 1 || spark == 5 {
            self.sfx(Sfx::BulletFlesh, end);
        }
        if self.kind == Kind::Host || self.kind == Kind::Client {
            self.out_events.push(Ev::Shot { o: vis, e: end, w: w as u8, spark });
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
            6 => {
                // a deflected bullet: a bright blue-white flash and a ring
                self.burst(p, [210, 240, 255], 22, 9.0);
                self.burst(p, [255, 255, 255], 8, 4.0);
                self.sfx(Sfx::Deflect, p);
            }
            7 => {
                // guard break: a shower of sparks and a big clang
                self.burst(p, [255, 220, 150], 30, 8.0);
                self.burst(p, [255, 255, 255], 12, 5.0);
                self.sfx(Sfx::GuardBreak, p);
                if (p - self.f[self.local].pos).len() < 8.0 {
                    self.shake = self.shake.max(0.45);
                }
            }
            8 => {
                // greatsword slam: a ring of dust and grit
                for k in 0..40 {
                    let ang = k as f32 / 40.0 * std::f32::consts::TAU + self.rng.range(0.0, 0.15);
                    let dir = v3(ang.sin(), 0.0, ang.cos());
                    let life = self.rng.range(0.4, 0.7);
                    let v = dir * self.rng.range(6.0, 9.0) + v3(0.0, self.rng.range(0.5, 2.0), 0.0);
                    self.particles.push(Particle { pos: p + dir * 0.4 + v3(0.0, 0.1, 0.0), vel: v, life, max_life: life, size: self.rng.range(0.18, 0.32), color: [160, 148, 128], gravity: 2.0 });
                }
                self.burst(p + v3(0.0, 0.2, 0.0), [255, 210, 140], 14, 6.0);
                self.sfx(Sfx::Slam, p);
                if (p - self.f[self.local].pos).len() < 10.0 {
                    self.shake = self.shake.max(0.55);
                }
            }
            9 => {
                // a kick lands
                self.burst(p, [215, 205, 185], 10, 3.5);
                self.sfx(Sfx::Kick, p);
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
            let b = self.f[j].clone();
            let b = &b;
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
            if b.shield_t > 0.0 && facing {
                // the Iron Wall throws the attacker back
                let at = &mut self.f[i];
                at.vel = -d.flat().norm() * 9.0 + v3(0.0, 3.5, 0.0);
                at.stun_t = at.stun_t.max(0.45);
                at.stagger_t = at.stagger_t.max(0.45);
                at.slash_t = 0.0;
                at.slash_queued = false;
                self.impact_fx(hit_at, 2);
                if i == self.local || j == self.local {
                    self.shake = self.shake.max(0.3);
                }
                if self.kind == Kind::Host || self.kind == Kind::Client {
                    self.out_events.push(Ev::Shot { o: hit_at, e: hit_at, w: 0, spark: 2 });
                }
                continue;
            }
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
                    self.jolt(d, 0.8);
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
            let heavy = heavy || a.slash_idx == SLASH_HEAVY;
            if b.guard && facing && !heavy && !counter {
                // blocked: the guard meter takes it
                self.impact_fx(hit_at, 2);
                if j == self.local || i == self.local {
                    self.shake = self.shake.max(0.2);
                    self.jolt(d, 0.5);
                }
                if self.kind == Kind::Host || self.kind == Kind::Client {
                    self.out_events.push(Ev::Shot { o: hit_at, e: hit_at, w: 0, spark: 2 });
                }
                let drain = guard_drain(18.0 * wpn.melee_mult() + 8.0);
                self.damage_g(i, j, 0.0, d.flat().norm() * 1.5, 0.0, false, hit_at, wpn as u8, a.chest(), d.norm(), drain);
                continue;
            }
            let air = !a.on_ground && a.wall.is_none();
            let (dmg, kb, up, stun) = match a.slash_idx {
                SLASH_HEAVY => (34.0, 5.0, 10.5, 0.7), // charged heavy: smashes through and launches
                3 => (22.0, 2.5, 11.5, 0.7), // launcher: pops them into the air
                2 => (26.0, 4.0, -14.0, 0.5), // air smash: slams them down
                _ if air => (15.0, 2.0, 5.5, 0.35), // juggle hit: keeps them up
                0 => (18.0, 3.5, 1.5, 0.12),
                _ => (18.0, 4.0, 2.0, 0.12),
            };
            let guard_break = b.guard && facing && heavy && !counter;
            let mut dmg = dmg * wpn.melee_mult() * if counter { 1.5 } else { 1.0 };
            // the dagger: double damage in the back
            let behind = b.fwd().dot(d.flat().norm()) > 0.4;
            if wpn == Weapon::Dagger && behind {
                dmg *= 2.0;
                self.note(i, |s| s.backstabs += 1);
                if i == self.local {
                    self.say("BACKSTAB x2".to_string());
                }
            }
            // a slash beats a kick: it cuts the kick short and staggers longer
            let stun = if b.kick_t > 0.0 {
                if i == self.local {
                    self.say("Kick interrupted!".to_string());
                }
                f32::max(stun, 0.45)
            } else {
                stun
            };
            if a.slash_idx == SLASH_HEAVY {
                self.note(i, |s| s.heavies += 1);
                if i == self.local {
                    self.say(if guard_break { "HEAVY - guard smashed!".to_string() } else { "HEAVY!".to_string() });
                }
            }

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
            self.damage_g(i, j, if guard_break { dmg * 0.5 } else { dmg }, knock, if guard_break { 0.0 } else { stun }, false, hit_at, wpn as u8, a.chest(), d.norm(), if guard_break { GUARD_MAX } else { 0.0 });
            self.impact_fx(hit_at, 1);
            self.sfx(if guard_break { Sfx::Clang } else { Sfx::Flesh }, hit_at);
            if i == self.local || j == self.local {
                let big = heavy || a.slash_idx >= 2;
                self.hitstop = self.hitstop.max(if big { 0.11 } else { 0.06 });
                self.shake = self.shake.max(if big { 0.55 } else { 0.22 });
                self.jolt(d, if big { 1.0 } else { 0.6 });
            }
            if self.kind == Kind::Host || self.kind == Kind::Client {
                self.out_events.push(Ev::Shot { o: hit_at, e: hit_at, w: 0, spark: 1 });
            }
        }
    }

    /// The kick lands (on the kicker's computer): it can't be blocked and staggers a guard, but a
    /// slash in full swing beats it.
    fn resolve_kick(&mut self, i: usize) {
        let a = self.f[i].clone();
        let fwd = a.fwd();
        for j in 0..self.f.len() {
            if j == i || !self.f[j].alive || !self.is_enemy(i, j) {
                continue;
            }
            let b = self.f[j].clone();
            let d = b.pos - a.pos;
            let dist = d.flat().len();
            if dist > KICK_RANGE || d.y.abs() > 1.6 || (dist > 0.3 && fwd.dot(d.flat().norm()) < 0.3) {
                continue;
            }
            if !self.world.line_clear(a.chest(), b.chest()) {
                continue;
            }
            let dir = if dist > 0.05 { d.flat().norm() } else { fwd };
            let at = b.chest() - dir * 0.3 - v3(0.0, 0.35, 0.0);
            if b.weapon.is_melee() && b.slashing() && b.slash_t > b.weapon.slash_dur() * 0.45 {
                // their slash cuts through our kick
                let f = &mut self.f[i];
                f.kick_t = 0.0;
                f.stun_t = f.stun_t.max(0.35);
                if i == self.local {
                    self.say("Your kick was cut short by a slash!".to_string());
                }
                return;
            }
            let facing = b.fwd().dot(-dir) > 0.3;
            if b.guard && facing {
                // a kick beats a guard: knocked off balance
                self.damage_g(i, j, 6.0, dir * 4.0, KICK_STAGGER, false, at, KICK_TOOL, a.chest(), dir, 30.0);
                self.note(i, |s| s.guard_kicks += 1);
                if i == self.local {
                    self.say("GUARD KICKED!".to_string());
                }
            } else {
                self.damage_g(i, j, KICK_DMG, dir * 6.5 + v3(0.0, 2.5, 0.0), 0.25, false, at, KICK_TOOL, a.chest(), dir, 0.0);
            }
            self.impact_fx(at, 9);
            if i == self.local || j == self.local {
                self.hitstop = self.hitstop.max(0.06);
                self.shake = self.shake.max(0.3);
                self.jolt(dir, 0.7);
            }
            if self.kind == Kind::Host || self.kind == Kind::Client {
                self.out_events.push(Ev::Shot { o: at, e: at, w: 0, spark: 9 });
            }
        }
    }

    /// Katana dash-through: every enemy along the path of the burst is cut once.
    fn resolve_through(&mut self, i: usize) {
        let a = self.f[i].clone();
        let (p0, p1) = (a.prev_pos, a.pos + a.fwd() * 0.8);
        let seg = (p1 - p0).flat();
        for j in 0..self.f.len().min(32) {
            if j == i || !self.f[j].alive || !self.is_enemy(i, j) || a.through_hit & (1 << j) != 0 {
                continue;
            }
            let b = self.f[j].clone();
            let rel = (b.pos - p0).flat();
            let t = if seg.dot(seg) > 1e-6 { (rel.dot(seg) / seg.dot(seg)).clamp(0.0, 1.0) } else { 0.0 };
            let closest = p0 + seg * t;
            let off = (b.pos - closest).flat();
            if off.len() > 1.2 || (b.pos.y - a.pos.y).abs() > 1.8 {
                continue;
            }
            self.f[i].through_hit |= 1 << j;
            let dir = if seg.len() > 0.01 { seg.norm() } else { a.fwd() };
            let hit_at = b.chest();
            if b.guard && b.fwd().dot(-dir) > 0.3 {
                self.impact_fx(hit_at, 2);
                if self.kind == Kind::Host || self.kind == Kind::Client {
                    self.out_events.push(Ev::Shot { o: hit_at, e: hit_at, w: 0, spark: 2 });
                }
                self.damage_g(i, j, 0.0, dir * 2.0, 0.0, false, hit_at, a.weapon as u8, a.chest(), dir, guard_drain(30.0));
                continue;
            }
            let side = if off.len() > 0.05 { off.norm() } else { yaw_right(a.yaw) };
            let dmg = 26.0 * a.weapon.melee_mult();
            {
                let f = &mut self.f[j];
                f.hit_part = Part::Chest as u8;
                f.flash_t = 0.12;
                f.hurt_t = f.hurt_t.max(0.25);
            }
            self.damage_g(i, j, dmg, side * 3.0 + dir * 1.5 + v3(0.0, 2.0, 0.0), 0.35, false, hit_at, a.weapon as u8, a.chest(), dir, 0.0);
            self.impact_fx(hit_at, 1);
            self.sfx(Sfx::Flesh, hit_at);
            self.note(i, |s| {
                s.dash_throughs += 1;
                s.slash_hits += 1;
            });
            if i == self.local {
                self.say("DASH THROUGH!".to_string());
                self.hitstop = self.hitstop.max(0.05);
                self.shake = self.shake.max(0.3);
                self.jolt(dir, 0.7);
            }
            if self.kind == Kind::Host || self.kind == Kind::Client {
                self.out_events.push(Ev::Shot { o: hit_at, e: hit_at, w: 0, spark: 1 });
            }
        }
    }

    /// Greatsword slam: a shockwave where the plunge hits the ground.
    fn resolve_slam(&mut self, i: usize) {
        let a = self.f[i].clone();
        let c = a.pos;
        self.impact_fx(c, 8);
        if self.kind == Kind::Host || self.kind == Kind::Client {
            self.out_events.push(Ev::Shot { o: c, e: c, w: 0, spark: 8 });
        }
        let mut hits = 0;
        for j in 0..self.f.len() {
            if j == i || !self.f[j].alive || !self.is_enemy(i, j) {
                continue;
            }
            let b = self.f[j].clone();
            let d = (b.pos - c).flat();
            let dist = d.len();
            if dist > SLAM_RADIUS || (b.pos.y - c.y).abs() > 2.2 || !self.world.line_clear(c + v3(0.0, 0.5, 0.0), b.chest()) {
                continue;
            }
            hits += 1;
            let dir = if dist > 0.05 { d.norm() } else { a.fwd() };
            let k = 1.0 - dist / SLAM_RADIUS * 0.6;
            if b.guard && b.fwd().dot(-dir) > 0.3 {
                self.damage_g(i, j, 0.0, dir * 5.0, 0.0, false, b.chest(), a.weapon as u8, a.chest(), dir, 50.0);
                continue;
            }
            {
                let f = &mut self.f[j];
                f.hit_part = Part::LegL as u8;
                f.flash_t = 0.12;
                f.hurt_t = f.hurt_t.max(0.25);
            }
            self.damage_g(i, j, 30.0 * k, dir * (6.0 * k) + v3(0.0, 6.5 * k, 0.0), 0.45, false, b.chest(), a.weapon as u8, a.chest(), dir, 0.0);
        }
        if hits > 0 {
            self.note(i, |s| s.slams += 1);
            if i == self.local {
                self.say("SLAM!".to_string());
            }
        }
    }

    /// A bullet sent back by a well-timed guard: it flies at the shooter.
    fn deflect_back(&mut self, shooter: usize, by: usize, at: V3, dmg: f32, w: Weapon) {
        let target = self.f[shooter].chest();
        let dir = (target - at).norm();
        let clear = self.world.line_clear(at, target);
        let end = if clear { target } else { at + dir * self.world.raycast(at, dir, 200.0).map_or(30.0, |(t, _)| t) };
        self.tracers.push(Tracer::shot(at, end, w, [150, 220, 255]));
        self.note(by, |s| s.deflects += 1);
        if by == self.local {
            self.say("DEFLECTED!".to_string());
        } else if shooter == self.local {
            self.say("Your shot was deflected!".to_string());
        }
        if self.kind == Kind::Host || self.kind == Kind::Client {
            self.out_events.push(Ev::Shot { o: at, e: end, w: 0, spark: 10 });
        }
        if clear && self.f[shooter].alive {
            if self.is_authority() {
                self.authority_hit(shooter, Some(by), dmg, dir * 1.5, 0.0, false, w as u8);
            } else if shooter == self.local {
                self.out_events.push(Ev::Deflect { by: by as u8, dmg });
            }
        }
    }

    /// A training dummy's "input": stand, guard or take a shot now and then, by lesson.
    fn dummy_input(&self, i: usize) -> Input {
        let f = &self.f[i];
        let mut inp = Input { yaw: f.yaw, ..Default::default() };
        let mode = self.train.as_ref().and_then(|t| LESSONS.get(t.lesson)).map_or(0, |l| l.dummy);
        let me = &self.f[self.local];
        match mode {
            1 => {
                if f.cur != 0 {
                    inp.select = Some(0);
                }
                inp.guard = true;
            }
            2 => {
                if f.cur != 1 {
                    inp.select = Some(1);
                }
                let to = me.chest() - f.eye();
                let flat = to.flat().len().max(0.01);
                inp.yaw = to.x.atan2(to.z);
                inp.pitch = to.y.atan2(flat);
                let ph = (self.time + i as f32 * 0.53) % 1.7;
                inp.fire = ph < 0.05 && to.len() < 30.0 && me.alive;
            }
            _ => {
                if f.cur != 0 {
                    inp.select = Some(0);
                }
            }
        }
        inp
    }

    /// A hit seen on this computer. The host (or a game on this computer) applies it right away;
    /// a player sends it to the host, which checks it against where the target was a moment ago.
    #[allow(clippy::too_many_arguments)]
    fn damage(&mut self, attacker: usize, victim: usize, amount: f32, knock: V3, stun: f32, head: bool, at: V3, w: u8, o: V3, dir: V3) {
        self.damage_g(attacker, victim, amount, knock, stun, head, at, w, o, dir, 0.0);
    }

    /// `damage` that also drains the victim's guard meter by `guard` (a blocked hit).
    #[allow(clippy::too_many_arguments)]
    fn damage_g(&mut self, attacker: usize, victim: usize, amount: f32, knock: V3, stun: f32, head: bool, at: V3, w: u8, o: V3, dir: V3, guard: f32) {
        if self.f[victim].spawn_prot > 0.0 || self.showcase == Some(victim) {
            return;
        }
        if attacker == self.local && victim != self.local && (w < 3 || w == KICK_TOOL) {
            let slack = if self.kind == Kind::Offline { 0.1 } else { 0.2 };
            let c = &mut self.my_combo;
            if amount <= 0.0 {
                *c = ComboTrack::default();
            } else {
                let cont = c.by == Some(victim) && self.time - c.last <= c.window + slack;
                c.n = if cont { c.n + 1 } else { 1 };
                c.by = Some(victim);
                c.last = self.time;
                c.window = combo_window(stun, knock);
            }
        }
        if attacker == self.local && victim != self.local && amount > 0.0 {
            self.combo_hit(amount);
            self.hitmarker = if head { 0.35 } else { 0.25 };
            self.hit_head = head || (self.hit_head && self.time - self.hit_chain_t < 0.02);
            let shown = if amount >= LETHAL { (self.f[victim].hp + self.f[victim].ap).max(1.0) } else { amount };
            // one crisp tick per shot (a shotgun's pellets count once), climbing in pitch while
            // the hits keep coming
            let volley = self.time - self.hit_chain_t < 0.02;
            // and one number per shot: a volley's pellets on the same target add up (gold if any
            // of them was a headshot)
            let gold = head || amount >= LETHAL;
            match self.dmg_nums.last_mut().filter(|d| volley && d.victim == victim as u8 && d.life > 0.85) {
                Some(d) => {
                    d.amount += shown.round() as i32;
                    d.head |= gold;
                    d.life = 0.9;
                }
                None => self.dmg_nums.push(DmgNum { pos: at + v3(0.0, 0.3, 0.0), amount: shown.round() as i32, life: 0.9, head: gold, victim: victim as u8 }),
            }
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
            self.authority_hit_g(victim, Some(attacker), amount, knock, stun, head, w, guard);
        } else {
            self.out_events.push(Ev::Hit { victim: victim as u8, dmg: amount, knock, stun, head, w, o, dir, guard });
        }
    }

    /// The host's (or this computer's) final word on a hit: health goes down here, and the
    /// victim's computer is told so it can stagger.
    #[allow(clippy::too_many_arguments)]
    pub fn authority_hit(&mut self, victim: usize, attacker: Option<usize>, amount: f32, knock: V3, stun: f32, head: bool, w: u8) {
        self.authority_hit_g(victim, attacker, amount, knock, stun, head, w, 0.0);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn authority_hit_g(&mut self, victim: usize, attacker: Option<usize>, amount: f32, knock: V3, stun: f32, head: bool, w: u8, guard: f32) {
        if !self.f[victim].present || !self.f[victim].alive || self.f[victim].spawn_prot > 0.0 {
            return;
        }
        // an execution: the one doing it gets health back and a full item stock
        if w == EXEC_TOOL && amount >= LETHAL {
            if let Some(a) = attacker.filter(|&a| a < self.f.len() && self.f[a].alive) {
                let f = &mut self.f[a];
                f.hp = (f.hp + FINISH_HEAL).min(MAX_HP);
                f.items = f.items.max(2);
            }
        }
        // blade combos: hits in a row, each landing while the target still reels from the last.
        // Each one staggers a little less (so a combo can't go on forever); the fifth is a KO.
        let (mut amount, mut stun, mut w) = (amount, stun, w);
        if let (Some(a), true) = (attacker, w < 3 || w == KICK_TOOL) {
            let slack = if self.kind == Kind::Offline { 0.1 } else { 0.2 };
            let now = self.time;
            if let Some(ct) = self.combos.get_mut(victim) {
                if amount <= 0.0 {
                    *ct = ComboTrack::default(); // blocked: the combo is broken
                } else {
                    let cont = ct.by == Some(a) && now - ct.last <= ct.window + slack;
                    ct.n = if cont { ct.n + 1 } else { 1 };
                    ct.by = Some(a);
                    ct.last = now;
                    stun *= (1.0 - 0.12 * (ct.n - 1) as f32).max(0.5);
                    ct.window = combo_window(stun, knock);
                    if ct.n >= COMBO_KO {
                        *ct = ComboTrack::default();
                        amount = LETHAL;
                        w = COMBO_TOOL;
                        if a == self.local {
                            self.say("COMBO KO!".to_string());
                        }
                        if victim == self.local {
                            self.say("Combo KO'd!".to_string());
                        }
                        self.note(a, |s| s.combo_kos += 1);
                    }
                }
            }
        }
        if self.f[victim].owned {
            let src = attacker.filter(|&a| a < self.f.len()).map(|a| self.f[a].pos);
            self.apply_hit_g(victim, attacker, amount, knock, stun, head, w, src, guard);
        } else {
            if matches!(self.mt.phase, Phase::Countdown | Phase::MatchOver | Phase::Loadout) {
                return;
            }
            if amount <= 0.0 {
                // a blocked hit: only their guard meter (on their computer) takes it
                let o = attacker.filter(|&a| a < self.f.len()).map_or(V3::ZERO, |a| self.f[a].pos);
                self.out_events.push(Ev::Hit { victim: victim as u8, dmg: 0.0, knock, stun, head: false, w, o, dir: V3::ZERO, guard });
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
            self.out_events.push(Ev::Hit { victim: victim as u8, dmg: amount, knock, stun, head, w, o, dir: V3::ZERO, guard });
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
        if w == EXEC_TOOL {
            // a finisher: they must have been close, and the target nearly beaten
            if self.f[victim].hp > FINISH_HP + 10.0 {
                return None;
            }
            let ping = s.ping_ms as f32 / 1000.0;
            for back in [INTERP_DELAY, MAX_INTERP] {
                let t = self.time - (ping + back + 0.015).clamp(0.0, 0.7);
                let vp = self.pos_at(victim, t);
                for sp in [self.pos_at(shooter, t), s.pos] {
                    if (vp - sp).flat().len() <= FINISH_RANGE + 1.6 && (vp.y - sp.y).abs() <= 4.0 {
                        return Some(ClaimCheck { head_ok: false, core_ok: true });
                    }
                }
            }
            return None;
        }
        if w == KICK_TOOL {
            // anyone can kick: just check the damage and that they were close enough
            if !(dmg >= 0.0) || dmg > KICK_DMG * 1.5 * 1.05 {
                return None;
            }
            let ping = s.ping_ms as f32 / 1000.0;
            for back in [INTERP_DELAY, MAX_INTERP] {
                let t = self.time - (ping + back + 0.015).clamp(0.0, 0.7);
                let vp = self.pos_at(victim, t);
                for sp in [self.pos_at(shooter, t), s.pos] {
                    if (vp - sp).flat().len() <= KICK_RANGE + 1.6 && (vp.y - sp.y).abs() <= 3.0 {
                        return Some(ClaimCheck { head_ok: false, core_ok: true });
                    }
                }
            }
            return None;
        }
        let wpn = Weapon::from_u8(w);
        if w >= N_WEAPONS || !s.loadout.has(wpn) || wpn == Weapon::Rocket {
            return None; // explosions are worked out by the host itself
        }
        // an Iron Wall facing the attacker stops it
        let vf = &self.f[victim];
        if vf.shield_t > 0.0 {
            let toward = if wpn.is_melee() { (s.pos - vf.pos).flat().norm() } else { -dir.norm() };
            if vf.fwd().dot(toward) > 0.25 {
                return None;
            }
        }
        if !(dmg >= 0.0) || dmg > wpn.max_hit() * 1.5 * 1.05 {
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
            let near = |sp: V3| {
                if wpn == Weapon::Shuriken {
                    // a star: claimed from just before where it struck, within its flight range
                    (o - sp).len() < STAR_SPEED * STAR_LIFE + 4.0
                } else {
                    (o - (sp + v3(0.0, 1.4, 0.0))).len() < 3.5
                }
            };
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
        self.apply_hit_g(victim, attacker, amount, knock, stun, head, w, src, 0.0);
    }

    /// `apply_hit` with a guard-meter drain: when the meter runs out the guard breaks (a long
    /// stagger and a big clang).
    #[allow(clippy::too_many_arguments)]
    pub fn apply_hit_g(&mut self, victim: usize, attacker: Option<usize>, amount: f32, knock: V3, stun: f32, head: bool, w: u8, src: Option<V3>, guard: f32) {
        {
            let ff = &self.f[victim];
            if !ff.alive || !ff.present || ff.spawn_prot > 0.0 || self.mt.phase == Phase::Countdown || self.mt.phase == Phase::MatchOver {
                return;
            }
        }
        let mut stun = stun;
        // hit by someone else in the middle of a finisher: it's cut short (fair in team fights)
        if amount > 0.0 && self.f[victim].fin > 0 && attacker != Some(self.f[victim].fin_target) {
            self.f[victim].fin = 0;
            if victim == self.local {
                self.say("Finisher interrupted!".to_string());
            }
        }
        if guard > 0.0 {
            let ff = &mut self.f[victim];
            ff.guard_hp -= guard.min(GUARD_MAX);
            ff.guard_rest = 0.0;
            if ff.guard_hp <= 0.0 {
                ff.guard_hp = 0.0;
                ff.guard = false;
                ff.guard_broke = 1.0;
                ff.stagger_t = GUARD_BREAK_TIME;
                stun = stun.max(GUARD_BREAK_TIME);
                let p = ff.chest();
                self.impact_fx(p, 7);
                if self.kind == Kind::Host || self.kind == Kind::Client {
                    self.out_events.push(Ev::Shot { o: p, e: p, w: 0, spark: 7 });
                }
                if let Some(a) = attacker {
                    self.note(a, |s| s.guard_breaks += 1);
                    if a == self.local {
                        self.say("GUARD BREAK!".to_string());
                    }
                }
                if victim == self.local {
                    self.say("Your guard broke!".to_string());
                    self.shake = self.shake.max(0.6);
                }
            }
        }
        if w == KICK_TOOL && stun >= KICK_STAGGER * 0.9 {
            let ff = &mut self.f[victim];
            ff.stagger_t = ff.stagger_t.max(stun);
            ff.guard = false;
        }
        if amount <= 0.0 {
            // blocked: a shove, and the stagger when the guard gave way
            let ff = &mut self.f[victim];
            ff.vel.x += knock.x;
            ff.vel.z += knock.z;
            if stun > 0.0 {
                ff.stun_t = ff.stun_t.max(stun);
                ff.slash_t = 0.0;
                ff.kick_t = 0.0;
                ff.charge_t = 0.0;
                ff.charged = false;
                ff.wall = None;
            }
            return;
        }
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
            ff.kick_t = 0.0;
            ff.charge_t = 0.0;
            ff.charged = false;
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
            if w < 3 || w == KICK_TOOL || w == EXEC_TOOL {
                // a blade (or a kick) on you: brief freeze, and the camera jolts with the blow
                if w < 3 {
                    self.hitstop = self.hitstop.max(if heavy { 0.1 } else { 0.05 });
                }
                self.jolt(me - from, if heavy { 0.9 } else { 0.55 });
            }
            let p = self.f[victim].pos;
            self.sfx(Sfx::Hurt, p);
        }
    }

    fn update_grenades(&mut self, dt: f32) {
        let mut explode = Vec::new();
        self.update_stars(dt);
        for (gi, g) in self.grenades.iter_mut().enumerate() {
            if g.kind == 2 {
                continue;
            }
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

    /// The finisher's blow: the kill (checked by the host like any hit), a big hit-stop and a
    /// flash, and the finisher's rewards on our side (guard and item back; the host gives the
    /// health).
    fn execute(&mut self, i: usize) {
        let t = self.f[i].fin_target;
        if t >= self.f.len() || !self.f[t].alive {
            return;
        }
        let style = finisher(self.f[i].fin);
        let a = self.f[i].chest();
        let at = self.f[t].chest();
        let dir = (at - a).flat().norm();
        let knock = if style.weapon.is_none() { v3(0.0, -14.0, 0.0) } else { dir * 4.0 + v3(0.0, 2.5, 0.0) };
        self.damage(i, t, LETHAL, knock, 0.8, false, at, EXEC_TOOL, a, dir);
        self.burst(at, [200, 25, 35], 34, 6.0);
        self.burst(at, [255, 235, 200], 14, 8.0);
        self.sfx(Sfx::Flesh, at);
        self.sfx(Sfx::GuardBreak, at);
        if i == self.local {
            self.f[i].guard_hp = GUARD_MAX;
            self.f[i].items = self.f[i].items.max(2);
            self.say(format!("EXECUTED! ({})", style.name));
        }
        if i == self.local || t == self.local {
            self.hitstop = self.hitstop.max(0.14);
            self.shake = self.shake.max(0.5);
            self.jolt(dir, 1.0);
            self.parry_flash = 0.22;
            if self.kind == Kind::Offline {
                self.slowmo = self.slowmo.max(0.3);
            }
        }
    }

    /// Throw one star from where you aim (turned `off` radians to the side, for the fan).
    fn throw_star(&mut self, i: usize, off: f32) {
        let (o, dir) = self.shot_ray(i);
        let yaw = dir.x.atan2(dir.z);
        let pitch = dir.y.clamp(-1.0, 1.0).asin();
        let d = look_dir(yaw + off, pitch);
        let p = o + d * 0.2;
        let v = d * STAR_SPEED;
        let vis = self.star_from_hand(i, p);
        self.grenades.push(Grenade { pos: p, vel: v, fuse: STAR_LIFE, owner: i, kind: 2, vis });
        if self.kind == Kind::Host || self.kind == Kind::Client {
            self.out_events.push(Ev::Grenade { p, v, kind: 2 });
        }
        self.sfx(Sfx::Shuriken, p);
    }

    /// How far a new star at `p` is from the thrower's hand (drawn from there, easing onto its path).
    fn star_from_hand(&self, i: usize, p: V3) -> V3 {
        let hand = self.hit_poses.get(i).copied().flatten().map(|h| h.caps[8].b);
        match hand {
            Some(h) if (h - p).len() < 2.5 => h - p,
            _ => V3::ZERO,
        }
    }

    /// Stars fly straight. The computer that threw one decides what it hits; everyone else just
    /// watches it fly and vanish.
    fn update_stars(&mut self, dt: f32) {
        let mut k = 0;
        while k < self.grenades.len() {
            if self.grenades[k].kind != 2 {
                k += 1;
                continue;
            }
            let g = self.grenades[k].clone();
            let step = g.vel * dt;
            let len = step.len().max(1e-4);
            let d = step * (1.0 / len);
            let mut t = len;
            let mut wall = false;
            if let Some((wt, _)) = self.world.raycast(g.pos, d, len) {
                t = wt;
                wall = true;
            }
            let mut hit: Option<(usize, Part)> = None;
            for j in 0..self.f.len() {
                let f = &self.f[j];
                if j == g.owner || !f.present || !f.alive || g.owner >= self.f.len() || !self.is_enemy(g.owner, j) {
                    continue;
                }
                let pose = self.hit_pose(j);
                if pose.bounds().ray(g.pos, d, t + 0.1).is_none() {
                    continue;
                }
                if let Some((ht, ci)) = pose.ray_cap(g.pos, d, t, 0.03) {
                    t = ht;
                    hit = Some((j, pose.caps[ci].part));
                    wall = false;
                }
            }
            let at = g.pos + d * t;
            let fuse = g.fuse - dt;
            if hit.is_some() || wall || fuse <= 0.0 {
                self.grenades.remove(k);
            } else {
                let s = &mut self.grenades[k];
                s.pos = at;
                s.fuse = fuse;
                k += 1;
            }
            if let Some((j, part)) = hit {
                self.star_hit(g.owner, j, part, at, d);
            } else if wall {
                self.burst(at, [255, 220, 170], 5, 4.0);
                self.sfx(Sfx::ImpactStone, at);
            }
        }
    }

    /// A star meets a body. Guards block it (and a guard raised just in time sends it back).
    fn star_hit(&mut self, owner: usize, j: usize, part: Part, at: V3, d: V3) {
        let ours = owner < self.f.len() && self.f[owner].owned;
        let v = self.f[j].clone();
        let blocked = v.guard && v.fwd().dot(-d) > 0.45;
        let deflect = blocked && v.weapon.is_melee() && v.guard_t <= DEFLECT_WINDOW;
        let gs = Weapon::Shuriken.gun();
        let head = part == Part::Head;
        if deflect {
            self.impact_fx(at, 6);
            if ours {
                self.deflect_back(owner, j, at, gs.dmg * 0.75, Weapon::Shuriken);
            }
            return;
        }
        if blocked {
            self.impact_fx(at, 2);
        } else {
            self.impact_fx(at, if v.ap > 0.0 { 5 } else { 1 });
            self.sfx(Sfx::BulletFlesh, at);
        }
        if !ours {
            return; // the thrower's computer counts the damage
        }
        let mut amount = gs.dmg * if head { gs.head } else { part.mult() };
        let mut drain = 0.0;
        if blocked {
            drain = guard_drain(amount);
            amount *= 0.12;
        } else {
            let f = &mut self.f[j];
            f.hit_part = part as u8;
            f.flash_t = 0.12;
            f.hurt_t = f.hurt_t.max(0.25);
            self.note(owner, |s| s.star_hits += 1);
        }
        if self.kind == Kind::Host || self.kind == Kind::Client {
            self.out_events.push(Ev::Shot { o: at, e: at, w: 0, spark: if blocked { 2 } else { 1 } });
        }
        self.damage_g(owner, j, amount, d.flat().norm() * gs.knock, 0.0, head && !blocked, at, Weapon::Shuriken as u8, at - d, d, drain);
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
                    self.dmg_nums.push(DmgNum { pos: c + v3(0.0, 0.5, 0.0), amount: dmg.round() as i32, life: 0.9, head: false, victim: 255 });
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
        self.update_abilities(dt);
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
        self.jolt_t = (self.jolt_t - dt).max(0.0);
    }

    // ------------------------------------------------------------------ network glue

    pub fn handle_remote_event(&mut self, from: usize, ev: Ev) {
        match ev {
            Ev::Shot { o, e, spark, .. } if spark == 10 => {
                // a deflected bullet flying back
                self.tracers.push(Tracer::shot(o, e, Weapon::Revolver, [150, 220, 255]));
            }
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
            Ev::Hit { victim, dmg, knock, stun, head, w, o, dir, guard } => {
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
                            self.authority_hit_g(v, Some(from), dmg, knock, stun, head && chk.head_ok, w, guard.clamp(0.0, GUARD_MAX));
                        }
                    }
                } else if from == 0 && v == self.local && self.f[v].owned {
                    // the host confirmed we were hit
                    self.apply_hit_g(v, None, dmg, knock, stun, head, w, Some(o), guard.clamp(0.0, GUARD_MAX));
                }
            }
            Ev::Ability { who, kind, p, d } => {
                let w = who as usize;
                if w >= self.f.len() || !self.f[w].present || !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite()) {
                    return;
                }
                let kind = Ability::from_u8(kind);
                if self.kind == Kind::Host {
                    // a player used their ability: only their own, the right one, off cooldown, in reach
                    if from != w || !self.ability_ok(w, kind, p) {
                        return;
                    }
                    self.ab_uses[w].push(self.time);
                    self.ability_fx(w, kind, p, d);
                    self.out_events.push(Ev::Ability { who, kind: kind as u8, p, d }); // passed on
                } else if from == 0 && w != self.local {
                    self.ability_fx(w, kind, p, d);
                }
            }
            Ev::Finisher { target, kind } => {
                let t = target as usize;
                if t < self.f.len() && from < self.f.len() && self.f[t].owned && self.f[t].alive {
                    self.hold(t, from, kind.clamp(1, FINISHERS.len() as u8));
                }
            }
            Ev::Deflect { by, dmg } => {
                // a player says their own bullet came back at them: hurting yourself needs no proof
                let b = by as usize;
                if self.kind == Kind::Host && from < self.f.len() && b < self.f.len() && b != from && self.f[from].alive && self.f[b].present && self.is_enemy(from, b) {
                    let w = self.f[from].weapon as u8;
                    self.authority_hit(from, Some(b), dmg.clamp(0.0, 60.0), V3::ZERO, 0.0, false, w);
                }
            }
            Ev::Grenade { p, v, kind } => {
                let fuse = match kind {
                    1 => 5.0,
                    2 => STAR_LIFE,
                    _ => 1.7,
                };
                let vis = if kind == 2 && from < self.f.len() { self.star_from_hand(from, p) } else { V3::ZERO };
                self.grenades.push(Grenade { pos: p, vel: v, fuse, owner: from, kind, vis });
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
        // (the katana's own dash + slash cuts through instead; the launcher is for the others)
        g.f[0].loadout.melee = Weapon::Dagger;
        g.f[0].weapon = Weapon::Dagger;
        g.f[0].pos = v3(-11.5, 0.0, 0.0);
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
            props: Vec::new(),
            vines: Vec::new(),
            torches: Vec::new(),
            map: MapId::Arena,
            flags: [v3(0.0, 0.0, -21.5), v3(0.0, 0.0, 21.5)],
        }
    }

    #[test]
    fn host_checks_hits_against_the_past() {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g.world = flat_world();
        g.bots[1] = None;
        g.mt.phase = Phase::Fight;
        g.f[0].loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Rifle, gun2: Weapon::Revolver, item: 0, body: 0, outfit: 0 };
        g.f[0].pos = v3(0.0, 0.0, -10.0);
        g.f[1].pos = v3(0.0, 0.0, 0.0);
        g.f[0].spawn_prot = 0.0;
        g.f[1].spawn_prot = 0.0;
        // the target runs sideways for half a second
        for k in 0..60 {
            g.f[1].pos = v3(k as f32 * 0.1, 0.0, 0.0);
            g.f[1].vel = V3::ZERO;
            g.tick(&Input { yaw: 0.0, ..Default::default() });
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
        g.f[red].pos = g.world.flag_base(1);
        g.tick(&Input::default());
        assert_eq!(g.mt.flags[1].state, 1, "touching the enemy flag picks it up");
        g.f[red].pos = g.world.flag_base(0);
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
        // a big enough hit drops a dummy (it counts toward the combo), and it gets back up at
        // its own spot
        let home = g.f[2].pos;
        let at = g.f[2].chest();
        g.damage(0, 2, 500.0, V3::ZERO, 0.1, false, at, 0, g.f[0].chest(), v3(0.0, 0.0, 1.0));
        run(&mut g, 0.5, Input { yaw, ..Default::default() });
        assert!(!g.f[2].alive, "the dummy went down");
        assert_eq!(g.train.as_ref().unwrap().best_combo, 1);
        run(&mut g, 2.0, Input { yaw, ..Default::default() });
        assert!(g.f[2].alive && g.f[2].hp >= MAX_HP - 1.0 && (g.f[2].pos - home).flat().len() < 1.0, "back up at home");
        // the player never dies here
        let at = g.f[0].chest();
        g.damage(2, 0, 500.0, V3::ZERO, 0.1, false, at, 0, g.f[2].chest(), v3(0.0, 0.0, -1.0));
        run(&mut g, 0.5, Input { yaw, ..Default::default() });
        assert!(g.f[0].alive);
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
fn clamp_len(v: V3, max: f32) -> V3 {
    let l = v.len();
    if l > max && l.is_finite() { v * (max / l) } else if l.is_finite() { v } else { V3::ZERO }
}

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
        g.f[1].loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Revolver, gun2: Weapon::Rifle, item: 0, body: 0, outfit: 0 };
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
            g.f[i].loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Revolver, gun2: Weapon::Rifle, item: 0, body: 0, outfit: 0 };
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
    const FWD: Input = Input { fwd: 1.0, right: 0.0, jump: false, dash: false, fire: false, fire_pressed: false, guard: false, select: None, reload: false, item: false, crouch: false, kick: false, finish: false, ability: false, yaw: 0.0, pitch: 0.0 };

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
    fn ground_has_weight_and_turns_cost_speed() {
        let mut g = arena();
        let tn = g.tune;
        // about 0.12 s to reach full speed
        let mut t_full = None;
        for k in 0..60 {
            g.tick(&FWD);
            if t_full.is_none() && g.f[0].vel.flat().len() > tn.run_speed * 0.97 {
                t_full = Some((k + 1) as f32 * TICK);
            }
        }
        let t_full = t_full.expect("reaches full speed");
        assert!((0.08..=0.17).contains(&t_full), "full speed after {t_full}s");
        assert!((g.f[0].vel.flat().len() - RUN_SPEED).abs() < 0.2, "base run speed 7.2");
        // about 0.1 s to stop
        let mut t_stop = None;
        for k in 0..60 {
            g.tick(&Input { fwd: 0.0, ..FWD });
            if t_stop.is_none() && g.f[0].vel.flat().len() < 0.2 {
                t_stop = Some((k + 1) as f32 * TICK);
            }
        }
        let t_stop = t_stop.expect("stops");
        assert!((0.06..=0.14).contains(&t_stop), "stopped after {t_stop}s");
        // a sharp turn at full speed bleeds speed for a moment
        run(&mut g, 0.4, FWD);
        let before = g.f[0].vel.flat().len();
        g.tick(&Input { fwd: -1.0, ..FWD });
        let after = g.f[0].vel.flat().len();
        assert!(after < before * (1.0 - tn.turn_slow * 0.8), "sharp turn slows: {before} -> {after}");
        // the dash crouches for a split second, then bursts well beyond running speed
        let mut g = arena();
        run(&mut g, 0.4, FWD);
        g.tick(&Input { dash: true, ..FWD });
        assert!(g.f[0].dash_wind > 0.0 && g.f[0].dash_t <= 0.0, "anticipation first");
        let mut peak: f32 = 0.0;
        for _ in 0..20 {
            g.tick(&FWD);
            peak = peak.max(g.f[0].vel.flat().len());
        }
        assert!(peak > RUN_SPEED * 1.8, "dash bursts out: {peak}");
        // landing from higher hits harder
        let mut g = arena();
        g.f[0].pos.y = 1.5;
        g.f[0].on_ground = false;
        for _ in 0..120 {
            g.tick(&Input { fwd: 0.0, ..FWD });
        }
        let low = g.f[0].land_impact;
        g.f[0].pos.y = 9.0;
        g.f[0].on_ground = false;
        for _ in 0..240 {
            g.tick(&Input { fwd: 0.0, ..FWD });
        }
        assert!(g.f[0].land_impact > low + 0.3, "higher fall, harder landing: {low} vs {}", g.f[0].land_impact);
    }

    #[test]
    fn chained_moves_build_flow_and_reset() {
        let mut g = arena();
        run(&mut g, 0.4, FWD);
        g.tick(&Input { dash: true, ..FWD }); // ground dash (after its short crouch)
        for _ in 0..30 {
            if g.f[0].dash_t > 0.0 {
                break;
            }
            g.tick(&FWD);
        }
        assert!(g.f[0].dash_t > 0.0, "the dash bursts out after its wind-up");
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
        assert!((g.f[0].flow_mult() - 1.2).abs() < 1e-4, "capped at +20%");
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
        assert!((g2.f[0].wall.expect("on the wall").speed - g2.tune.wall_speed).abs() < 1e-3);
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
        g.f[0].loadout = Loadout { melee: Weapon::Katana, gun1: gun, gun2: Weapon::Revolver, item: 0, body: 0, outfit: 0 };
        g.f[0].equip();
        g.f[0].pos = v3(0.0, 0.0, -10.0);
        g.f[1].loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Shotgun, gun2: Weapon::Smg, item: 0, body: 0, outfit: 0 };
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

    /// Fire the scattergun's whole pattern from `dist` metres straight at capsule `cap`'s middle.
    fn pump_at(g: &mut Game, cap: usize, dist: f32) {
        let c = g.hit_pose(1).caps[cap];
        let d = v3(0.0, 0.0, 1.0);
        let o = (c.a + c.b) * 0.5 - d * dist;
        g.f[0].yaw = 0.0;
        g.fire_pattern(0, o, d, Weapon::Shotgun.gun().spread_still);
        g.tick(&Input::default()); // deaths are settled in the tick
    }

    #[test]
    fn point_blank_shotgun_body_does_100() {
        let mut g = range(Weapon::Shotgun);
        pump_at(&mut g, 2, 1.5);
        let lost = (MAX_HP - g.f[1].hp) + (MAX_AP - g.f[1].ap);
        println!("point-blank body: {lost}");
        assert!(lost >= 100.0, "a point-blank body shot does 100+ (did {lost})");
        // shown as one number for the whole shot
        let nums: Vec<&DmgNum> = g.dmg_nums.iter().filter(|n| n.victim == 1).collect();
        assert_eq!(nums.len(), 1, "one damage number per shot");
        assert!((nums[0].amount as f32 - lost).abs() <= 2.0, "the number is the shot's total: {} vs {lost}", nums[0].amount);
        assert!(!nums[0].head);
    }

    #[test]
    fn point_blank_shotgun_headshot_kills_through_full_armour() {
        let mut g = range(Weapon::Shotgun);
        assert!(g.f[1].hp >= MAX_HP && g.f[1].ap >= MAX_AP);
        pump_at(&mut g, 0, 1.5);
        assert!(!g.f[1].alive, "a point-blank headshot kills through full armour");
        let n = g.dmg_nums.iter().find(|n| n.victim == 1).expect("a number");
        println!("point-blank head: {}", n.amount);
        assert!(n.head && n.amount >= 160, "a gold 160+: {} (head {})", n.amount, n.head);
    }

    #[test]
    fn shotgun_pattern_is_fixed_and_falls_off() {
        assert_eq!(SHOTGUN_PATTERN.len(), Weapon::Shotgun.gun().pellets);
        assert!((shotgun_falloff(4.0) - 1.0).abs() < 1e-6);
        assert!((shotgun_falloff(18.0) - 0.4).abs() < 1e-4);
        assert!((shotgun_falloff(60.0) - 0.15).abs() < 1e-6);
        // the same shot from the same place does exactly the same every time
        let mut a = range(Weapon::Shotgun);
        pump_at(&mut a, 2, 6.0);
        let mut b = range(Weapon::Shotgun);
        pump_at(&mut b, 2, 6.0);
        assert_eq!((a.f[1].hp, a.f[1].ap), (b.f[1].hp, b.f[1].ap));
        assert!(a.f[1].hp < MAX_HP || a.f[1].ap < MAX_AP);
    }

    #[test]
    fn a_shot_clipping_the_hood_is_a_headshot() {
        // a level shot just over the top of the head: `above` metres from its axis's top end
        let shot = |w: Weapon, above: f32| -> (bool, f32) {
            let mut g = range(w);
            let c = g.hit_pose(1).caps[0];
            let top = c.b + v3(0.0, above, 0.0);
            let d = v3(0.0, 0.0, 1.0);
            g.fire_ray(0, top - d * 5.0, d, w.gun().dmg, w.gun().range, w);
            g.tick(&Input::default());
            let lost = (MAX_HP - g.f[1].hp) + (MAX_AP - g.f[1].ap);
            (g.dmg_nums.iter().any(|n| n.victim == 1 && n.head), lost)
        };
        // over the hood: inside the bigger head (0.15) though outside the old one (0.115)
        let (head, lost) = shot(Weapon::Smg, 0.13);
        assert!(head && lost > 0.0, "the hood is the head");
        // a single-shot gun grazing past within 5 cm of it
        for w in [Weapon::Revolver, Weapon::HandCannon, Weapon::Rifle] {
            let (head, lost) = shot(w, 0.15 + 0.035);
            assert!(head && lost > 0.0, "{w:?}: a graze counts as a headshot");
        }
        // but not further out, and not for a spray gun
        assert!(shot(Weapon::Revolver, 0.15 + 0.08).1 == 0.0, "a clean miss");
        assert!(shot(Weapon::Smg, 0.15 + 0.035).1 == 0.0, "no assist for the SMG");
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

    #[test]
    fn revolver_rewards_tapping_not_spamming() {
        // a headshot kills an unarmoured target outright
        let r = Weapon::Revolver.gun();
        assert!(r.dmg * r.head >= MAX_HP, "revolver headshot one-taps");
        // tapping (half a second apart): every shot dead-on
        let mut f = Fighter::new("x", 0, true);
        f.loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Revolver, gun2: Weapon::Rifle, item: 0, body: 0, outfit: 0 };
        f.equip();
        f.cur = 1;
        f.weapon = Weapon::Revolver;
        f.move_spread = 1.0;
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.bots[1] = None;
        g.mt.phase = Phase::Fight;
        g.f[1].pos = v3(30.0, 0.0, 30.0);
        g.f[0].loadout = f.loadout;
        g.f[0].equip();
        g.f[0].pos = V3::ZERO;
        g.f[0].spawn_prot = 0.0;
        let pick = Input { select: Some(1), ..Default::default() };
        for _ in 0..60 {
            g.tick(&pick);
        }
        assert_eq!(g.f[0].weapon, Weapon::Revolver);
        let mut tapped = Vec::new();
        for _ in 0..4 {
            tapped.push(g.f[0].spread());
            g.tick(&Input { fire: true, ..Default::default() });
            for _ in 0..(0.55 / TICK) as usize {
                g.tick(&Input::default());
            }
        }
        assert!(tapped.iter().all(|&s| s == 0.0), "tapped shots are dead-on: {tapped:?}");
        // spamming as fast as it fires: each shot wilder than the last (a full cylinder first)
        g.f[0].mag[1] = Weapon::Revolver.mag();
        g.f[0].reload_t = 0.0;
        let mut spam = Vec::new();
        for _ in 0..5 {
            for _ in 0..40 {
                if g.f[0].fire_cd <= 0.0 && g.f[0].reload_t <= 0.0 {
                    break;
                }
                g.tick(&Input::default());
            }
            spam.push(g.f[0].spread());
            g.tick(&Input { fire: true, ..Default::default() });
        }
        assert!(spam.windows(2).skip(1).all(|w| w[1] > w[0]), "spread grows while spamming: {spam:?}");
        // by the 4th spammed shot a head at 10 m (0.12 m wide) is mostly a miss
        assert!(spam[3] * 10.0 > 0.25, "spammed shots fly wide: {spam:?}");
    }
}

#[cfg(test)]
mod tuning_tests {
    use super::*;

    fn flat() -> Game {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.bots[1] = None;
        g.mt.phase = Phase::Fight;
        g.f[0].pos = V3::ZERO;
        g.f[0].spawn_prot = 0.0;
        g.f[1].pos = v3(30.0, 0.0, 30.0);
        g
    }

    #[test]
    fn presets_round_trip_and_stay_in_range() {
        let mut t = Tuning::default();
        t.run_speed = 11.5;
        t.cam_dist = 4.25;
        t.char_scale = 1.3;
        assert_eq!(Tuning::from_text(&t.to_text()), t);
        let wild = Tuning::from_text("run_speed=999\ngravity=-5\nnonsense=1\njump_height=abc");
        assert_eq!(wild.run_speed, 16.0, "clamped to the slider range");
        assert_eq!(wild.gravity, 5.0);
        assert_eq!(wild.jump_height, Tuning::default().jump_height, "bad values are ignored");
        // the defaults are the game''s own numbers
        let d = Tuning::default();
        assert!((d.jump_vel() - (2.0 * d.gravity * d.jump_height).sqrt()).abs() < 1e-3, "jump speed follows gravity and height");
        // full speed in 0.12 s, a stop in 0.1 s, whatever the run speed
        assert!((d.accel() - d.run_speed / 0.12).abs() < 1e-3 && (d.decel() - d.run_speed / 0.1).abs() < 1e-3);
    }

    #[test]
    fn run_speed_and_jump_height_take_effect() {
        let fwd = Input { fwd: 1.0, ..Default::default() };
        let mut g = flat();
        g.tune.run_speed = 12.0;
        for _ in 0..90 {
            g.tick(&fwd);
        }
        assert!((g.f[0].vel.flat().len() - 12.0).abs() < 0.2, "runs at the new speed: {}", g.f[0].vel.flat().len());
        let mut g = flat();
        g.tune.jump_height = 3.0;
        for _ in 0..30 {
            g.tick(&Input::default());
        }
        g.tick(&Input { jump: true, ..Default::default() });
        let mut top: f32 = 0.0;
        for _ in 0..200 {
            g.tick(&Input::default());
            top = top.max(g.f[0].pos.y);
        }
        assert!((top - 3.0).abs() < 0.15, "jumps 3 m high: {top}");
    }

    #[test]
    fn camera_distance_takes_effect() {
        let mut g = flat();
        let (_, _, near) = g.camera_rig(V3::ZERO, 0.0, 0.0);
        g.tune.cam_dist = 6.0;
        let (_, _, far) = g.camera_rig(V3::ZERO, 0.0, 0.0);
        assert!((near - 3.2).abs() < 1e-3 && (far - 6.0).abs() < 1e-3, "{near} -> {far}");
    }

    #[test]
    fn dummies_stand_where_asked() {
        let mut g = flat();
        g.f[0].yaw = 0.0;
        g.spawn_dummy(15.0).unwrap();
        let d = g.f.iter().find(|f| f.present && f.name == "Dummy 15m").expect("placed");
        assert!((d.pos - v3(0.0, 0.0, 15.0)).len() < 0.01, "15 m straight ahead: {:?}", d.pos);
        assert!(d.fwd().dot(v3(0.0, 0.0, -1.0)) > 0.99, "facing us");
        assert_eq!(g.remove_dummies(), 1);
        assert!(!g.f.iter().any(|f| f.present && f.name.starts_with("Dummy 15")));
    }
}

#[cfg(test)]
mod melee_depth_tests {
    use super::*;

    /// Two fighters on flat ground, face to face 1.4 m apart: 0 at the origin looking +z, 1 in
    /// front looking back. Both carry `blade`, no armour, no brains.
    fn duel(blade: Weapon) -> Game {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.bots[1] = None;
        g.mt.phase = Phase::Fight;
        g.mt.timer = 99.0;
        for i in 0..2 {
            g.f[i].loadout = Loadout { melee: blade, gun1: Weapon::Revolver, gun2: Weapon::Rifle, item: 0, body: 0, outfit: 0 };
            g.f[i].equip();
            g.f[i].cur = 0;
            g.f[i].weapon = blade;
            g.f[i].ap = 0.0;
            g.f[i].spawn_prot = 0.0;
            g.f[i].on_ground = true;
        }
        g.f[0].pos = V3::ZERO;
        g.f[0].yaw = 0.0;
        g.f[1].pos = v3(0.0, 0.0, 1.4);
        g.f[1].yaw = std::f32::consts::PI;
        for i in 0..2 {
            g.f[i].prev_pos = g.f[i].pos;
        }
        g.update_hit_poses();
        g
    }

    fn hold(g: &mut Game, secs: f32, inp: Input) {
        for _ in 0..(secs / TICK).round() as usize {
            g.tick(&inp);
        }
    }

    #[test]
    fn blocking_drains_the_guard_until_it_breaks_then_it_refills() {
        let mut g = duel(Weapon::Katana);
        g.f[1].guard = true;
        g.f[1].guard_t = 1.0; // up for a while: blocks, no perfect parry
        let mut broke_at = None;
        for k in 0..8 {
            let before = g.f[1].guard_hp;
            g.f[1].guard = g.f[1].stun_t <= 0.0;
            g.f[0].slash_idx = 0;
            g.resolve_slash(0);
            assert_eq!(g.f[1].hp, MAX_HP, "a block lets nothing through");
            if g.f[1].guard_hp <= 0.0 {
                broke_at = Some(k + 1);
                break;
            }
            assert!(g.f[1].guard_hp < before, "each block drains the meter");
        }
        let n = broke_at.expect("the guard breaks under a flurry");
        assert!((3..=5).contains(&n), "broke after {n} blocks");
        assert!(g.f[1].stun_t >= GUARD_BREAK_TIME - 0.01 && g.f[1].stagger_t >= GUARD_BREAK_TIME - 0.01, "guard break staggers for 0.8 s");
        assert!(!g.f[1].guard && g.f[1].guard_broke > 0.0);
        // then it refills once the guard is down
        g.f[1].stun_t = 0.0;
        hold(&mut g, 2.0, Input { yaw: 0.0, ..Default::default() });
        assert!(g.f[1].guard_hp > GUARD_MAX * 0.5, "refills when not guarding: {}", g.f[1].guard_hp);
    }

    #[test]
    fn kick_beats_guard_slash_beats_kick() {
        // a kick staggers a guard (and can't be blocked)
        let mut g = duel(Weapon::Katana);
        g.f[1].guard = true;
        g.f[1].guard_t = 1.0;
        g.resolve_kick(0);
        assert!(g.f[1].stun_t >= KICK_STAGGER - 0.01 && g.f[1].stagger_t >= KICK_STAGGER - 0.01, "kicked out of the guard");
        assert!(g.f[1].hp < MAX_HP, "unblockable: a little gets through");
        // a slash in full swing beats the kick
        let mut g = duel(Weapon::Katana);
        g.f[1].slash_idx = 0;
        g.f[1].slash_t = Weapon::Katana.slash_dur() * 0.9;
        g.resolve_kick(0);
        assert!(g.f[0].stun_t > 0.3, "the kicker is cut short");
        assert_eq!(g.f[1].hp, MAX_HP);
        // and a slash on someone mid-kick staggers them longer
        let mut g = duel(Weapon::Katana);
        g.f[1].kick_t = KICK_TIME * 0.8;
        g.f[0].slash_idx = 0;
        g.resolve_slash(0);
        assert!(g.f[1].stun_t >= 0.44 && g.f[1].kick_t <= 0.0, "kick interrupted");
        // the F key does it in play: quick, short range
        let mut g = duel(Weapon::Katana);
        g.tick(&Input { kick: true, ..Default::default() });
        assert!(g.f[0].kick_t > 0.0);
        hold(&mut g, 0.3, Input::default());
        assert!(g.f[1].hp <= MAX_HP - KICK_DMG + 0.01, "the kick lands: {}", g.f[1].hp);
        let mut far = duel(Weapon::Katana);
        far.f[1].pos = v3(0.0, 0.0, 3.5);
        far.f[1].prev_pos = far.f[1].pos;
        far.tick(&Input { kick: true, ..Default::default() });
        hold(&mut far, 0.3, Input::default());
        assert_eq!(far.f[1].hp, MAX_HP, "short range only");
    }

    #[test]
    fn guard_just_in_time_deflects_bullets() {
        let mut g = duel(Weapon::Katana);
        g.f[1].pos = v3(0.0, 0.0, 8.0);
        g.f[1].prev_pos = g.f[1].pos;
        g.update_hit_poses();
        g.f[1].guard = true;
        g.f[1].guard_t = 0.05;
        let o = g.f[0].eye();
        let d = (g.f[1].chest() - o).norm();
        g.fire_ray(0, o, d, 24.0, 150.0, Weapon::Revolver);
        assert_eq!(g.f[1].hp, MAX_HP, "the deflector is untouched");
        assert!(g.f[0].hp < MAX_HP, "the bullet came back at the shooter");
        // a guard held too long only blocks (and the meter pays)
        let mut g = duel(Weapon::Katana);
        g.f[1].pos = v3(0.0, 0.0, 8.0);
        g.f[1].prev_pos = g.f[1].pos;
        g.update_hit_poses();
        g.f[1].guard = true;
        g.f[1].guard_t = 1.0;
        let o = g.f[0].eye();
        let d = (g.f[1].chest() - o).norm();
        g.fire_ray(0, o, d, 24.0, 150.0, Weapon::Revolver);
        assert_eq!(g.f[0].hp, MAX_HP, "no deflect");
        assert!(g.f[1].guard_hp < GUARD_MAX && g.f[1].hp > MAX_HP - 5.0, "blocked: chip damage, guard drained");
    }

    #[test]
    fn charged_heavy_breaks_guards_and_launches_and_a_dash_cancels_it() {
        let mut g = duel(Weapon::Katana);
        hold(&mut g, 0.6, Input { fire: true, ..Default::default() });
        assert!(g.f[0].charged, "fully charged after half a second");
        assert!(!g.f[0].slashing(), "holding alone doesn't swing");
        g.tick(&Input::default()); // let go
        assert_eq!(g.f[0].slash_idx, SLASH_HEAVY);
        let mut peak: f32 = 0.0;
        for _ in 0..60 {
            g.tick(&Input::default());
            peak = peak.max(g.f[1].vel.y);
        }
        assert!(g.f[1].hp < MAX_HP - 30.0 && peak > 7.0, "a heavy hit that launches: hp {} vy {peak}", g.f[1].hp);
        // straight through a full guard
        let mut g = duel(Weapon::Katana);
        g.f[1].guard = true;
        g.f[1].guard_t = 1.0;
        g.f[0].slash_idx = SLASH_HEAVY;
        g.resolve_slash(0);
        assert!(g.f[1].guard_hp <= 0.0 && g.f[1].stun_t >= GUARD_BREAK_TIME - 0.01, "guard smashed");
        assert!(g.f[1].vel.y > 7.0, "and launched");
        // a dash cancels the charge
        let mut g = duel(Weapon::Katana);
        hold(&mut g, 0.6, Input { fire: true, ..Default::default() });
        assert!(g.f[0].charged);
        for _ in 0..12 {
            g.tick(&Input { fire: true, dash: true, ..Default::default() });
        }
        assert!(!g.f[0].charged, "dashing drops the charge");
        g.tick(&Input::default());
        assert_ne!(g.f[0].slash_idx, SLASH_HEAVY);
    }

    #[test]
    fn dagger_backstab_doubles() {
        let mut g = duel(Weapon::Dagger);
        g.f[0].slash_idx = 0;
        g.resolve_slash(0);
        let front = MAX_HP - g.f[1].hp;
        let mut g = duel(Weapon::Dagger);
        g.f[1].yaw = 0.0; // back turned
        g.update_hit_poses();
        g.f[0].slash_idx = 0;
        g.resolve_slash(0);
        let back = MAX_HP - g.f[1].hp;
        assert!(front > 1.0 && (back / front - 2.0).abs() < 0.05, "front {front}, back {back}");
    }

    #[test]
    fn katana_dashes_through_its_target() {
        let mut g = duel(Weapon::Katana);
        g.f[1].pos = v3(0.0, 0.0, 3.0);
        g.f[1].prev_pos = g.f[1].pos;
        let fwd = Input { fwd: 1.0, ..Default::default() };
        g.tick(&Input { dash: true, ..fwd });
        for _ in 0..30 {
            if g.f[0].dash_t > 0.0 {
                break;
            }
            g.tick(&fwd);
        }
        g.tick(&Input { fire_pressed: true, ..fwd });
        assert_eq!(g.f[0].slash_idx, SLASH_THROUGH);
        hold(&mut g, 0.4, Input::default());
        assert!(g.f[1].hp < MAX_HP - 20.0, "cut on the way past: {}", g.f[1].hp);
        assert!(g.f[0].pos.z > g.f[1].pos.z + 0.5, "came out the other side: {} vs {}", g.f[0].pos.z, g.f[1].pos.z);
    }

    #[test]
    fn greatsword_slams_from_the_air() {
        let mut g = duel(Weapon::Greatsword);
        g.f[1].pos = v3(0.0, 0.0, 2.2);
        g.f[1].prev_pos = g.f[1].pos;
        g.f[0].pos = v3(0.0, 4.0, 0.0);
        g.f[0].prev_pos = g.f[0].pos;
        g.f[0].on_ground = false;
        g.tick(&Input { fire_pressed: true, ..Default::default() });
        assert_eq!(g.f[0].slash_idx, SLASH_SLAM);
        assert!(g.f[0].slam && g.f[0].vel.y < -20.0, "plunging");
        let mut peak: f32 = 0.0;
        for _ in 0..60 {
            g.tick(&Input::default());
            peak = peak.max(g.f[1].vel.y);
        }
        assert!(g.f[0].on_ground && !g.f[0].slam);
        assert!(g.f[1].hp < MAX_HP - 15.0 && peak > 3.0, "shockwave hit and popped them up: hp {} vy {peak}", g.f[1].hp);
        // out of reach: untouched
        let mut g = duel(Weapon::Greatsword);
        g.f[1].pos = v3(0.0, 0.0, 6.0);
        g.f[0].pos = v3(0.0, 4.0, 0.0);
        g.f[0].on_ground = false;
        g.tick(&Input { fire_pressed: true, ..Default::default() });
        hold(&mut g, 0.5, Input::default());
        assert_eq!(g.f[1].hp, MAX_HP);
    }

    #[test]
    fn host_checks_blocks_and_kicks() {
        let g = duel(Weapon::Katana);
        let (o, d) = (g.f[0].chest(), v3(0.0, 0.0, 1.0));
        // a blocked slash claims no damage, only guard drain: still a real hit to check
        assert!(g.validate_claim(0, 1, Weapon::Katana as u8, 0.0, o, d, false).is_some());
        assert!(g.validate_claim(0, 1, KICK_TOOL, KICK_DMG, o, d, false).is_some(), "a kick in reach");
        assert!(g.validate_claim(0, 1, KICK_TOOL, 50.0, o, d, false).is_none(), "no kick does 50");
        let mut far = duel(Weapon::Katana);
        far.f[1].pos = v3(0.0, 0.0, 9.0);
        assert!(far.validate_claim(0, 1, KICK_TOOL, KICK_DMG, o, d, false).is_none(), "out of reach");
        // a charged heavy and a dagger backstab are within what the host allows
        assert!(g.validate_claim(0, 1, Weapon::Katana as u8, 34.0 * 1.5, o, d, false).is_some());
    }
}

#[cfg(test)]
mod fairness_tests {
    use super::*;

    fn duel() -> Game {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.bots[1] = None;
        g.mt.phase = Phase::Fight;
        g.mt.timer = 99.0;
        for i in 0..2 {
            g.f[i].loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Revolver, gun2: Weapon::Rifle, item: 0, body: 0, outfit: 0 };
            g.f[i].equip();
            g.f[i].cur = 0;
            g.f[i].weapon = Weapon::Katana;
            g.f[i].ap = 0.0;
            g.f[i].spawn_prot = 0.0;
            g.f[i].is_bot = i != 0; // the target stands still (no brain), not copying our keys
        }
        g.f[0].pos = V3::ZERO;
        g.f[0].yaw = 0.0;
        g.f[1].pos = v3(0.0, 0.0, 1.6);
        g.f[1].yaw = std::f32::consts::PI;
        for i in 0..2 {
            g.f[i].prev_pos = g.f[i].pos;
        }
        g
    }

    #[test]
    fn mashing_a_blade_leaves_openings() {
        let mut g = duel();
        g.f[1].hp = 10_000.0; // just measuring
        let mut free = 0.0f32;
        let mut longest = 0.0f32;
        let mut swings = 0;
        let mut was = 0.0;
        for _ in 0..(3.0 / TICK) as usize {
            g.tick(&Input { fire_pressed: true, ..Default::default() });
            if g.f[0].slash_t > was + 0.05 {
                swings += 1;
            }
            was = g.f[0].slash_t;
            if g.f[1].stun_t <= 0.0 {
                free += TICK;
                longest = longest.max(free);
            } else {
                free = 0.0;
            }
            assert!(g.f[1].alive, "mashing never knocks anyone out");
        }
        assert!(swings >= 4, "still swings: {swings}");
        assert!(longest >= 0.25, "the target gets a real opening to guard or kick back: {longest}");
    }

    #[test]
    fn five_hits_in_a_row_is_a_ko() {
        let hit = |g: &mut Game| {
            let at = g.f[1].chest();
            g.damage(0, 1, 8.0, V3::ZERO, 0.35, false, at, Weapon::Katana as u8, g.f[0].chest(), v3(0.0, 0.0, 1.0));
        };
        // each hit lands while they still reel from the last: the fifth knocks them out
        let mut g = duel();
        for k in 0..5 {
            hit(&mut g);
            for _ in 0..(0.2 / TICK) as usize {
                g.tick(&Input::default());
            }
            if k < 4 {
                assert!(g.f[1].alive, "still up after {} hits", k + 1);
            }
        }
        assert!(!g.f[1].alive, "five in a row: KO");
        assert_eq!(g.f[1].last_weapon, COMBO_TOOL);
        // the same hits with gaps they could have escaped in: no KO
        let mut g = duel();
        for _ in 0..5 {
            hit(&mut g);
            for _ in 0..(0.7 / TICK) as usize {
                g.tick(&Input::default());
            }
        }
        assert!(g.f[1].alive && g.f[1].hp > 50.0, "no combo, no KO: {}", g.f[1].hp);
        // a blocked hit breaks the combo
        let mut g = duel();
        for _ in 0..3 {
            hit(&mut g);
            g.tick(&Input::default());
        }
        let at = g.f[1].chest();
        g.damage_g(0, 1, 0.0, V3::ZERO, 0.0, false, at, Weapon::Katana as u8, g.f[0].chest(), v3(0.0, 0.0, 1.0), 10.0);
        assert_eq!(g.combos[1].n, 0);
    }

    /// Aim the local player's camera line through `p` (the rig moves with the aim, so settle it).
    fn aim_through(g: &mut Game, p: V3) {
        for _ in 0..12 {
            let (pivot, dir, dist) = g.camera_rig(g.f[0].pos, g.f[0].yaw, g.f[0].pitch);
            let cam = pivot - dir * dist;
            let d = (p - cam).norm();
            g.f[0].yaw = d.x.atan2(d.z);
            g.f[0].pitch = d.y.atan2(d.flat().len());
        }
    }

    #[test]
    fn shots_at_the_edge_of_a_head_are_headshots() {
        let mut g = duel();
        g.f[1].pos = v3(0.4, 0.0, 12.0);
        g.f[1].prev_pos = g.f[1].pos;
        g.update_hit_poses();
        let pose = g.hit_pose(1);
        let head = *pose.caps.iter().find(|c| c.part == Part::Head).unwrap();
        let mid = (head.a + head.b) * 0.5;
        let top = head.a.y.max(head.b.y);
        let r = head.r * 0.85;
        for (name, p) in [("top", v3(mid.x, top + r, mid.z)), ("left", mid + v3(-r, 0.0, 0.0)), ("right", mid + v3(r, 0.0, 0.0)), ("middle", mid)] {
            aim_through(&mut g, p);
            let (o, d) = g.shot_ray(0);
            let hit = g.ray_fighter(1, o, d, 100.0).map(|h| h.1);
            assert_eq!(hit, Some(Part::Head), "{name} of the head");
        }
        // just above the head goes over (it isn't fattened up)
        aim_through(&mut g, v3(mid.x, top + head.r + 0.08, mid.z));
        let (o, d) = g.shot_ray(0);
        assert!(g.ray_fighter(1, o, d, 100.0).is_none());
    }

    #[test]
    fn hit_shapes_stand_where_the_body_was_drawn() {
        let mut g = duel();
        g.f[1].prev_pos = v3(0.0, 0.0, 10.0);
        g.f[1].pos = v3(0.3, 0.0, 10.0);
        g.view_alpha = 0.0; // the last picture showed them at the older spot
        g.tick(&Input::default());
        let pose = g.hit_pose(1);
        let head = pose.caps.iter().find(|c| c.part == Part::Head).unwrap();
        assert!(head.a.x.abs() < 0.12, "shapes where they were drawn: {}", head.a.x);
    }
}

#[cfg(test)]
mod shuriken_tests {
    use super::*;

    fn range() -> Game {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.bots[1] = None;
        g.mt.phase = Phase::Fight;
        g.mt.timer = 99.0;
        for i in 0..2 {
            g.f[i].loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Shuriken, gun2: Weapon::Rifle, item: 0, body: 0, outfit: 0 };
            g.f[i].equip();
            g.f[i].ap = 0.0;
            g.f[i].spawn_prot = 0.0;
            g.f[i].is_bot = i != 0; // the target stands still (no brain), not copying our keys
        }
        g.f[0].pos = V3::ZERO;
        g.f[1].pos = v3(0.0, 0.0, 12.4);
        g.f[1].yaw = std::f32::consts::PI;
        g.f[1].hp = 10_000.0; // just measuring
        for i in 0..2 {
            g.f[i].prev_pos = g.f[i].pos;
        }
        for _ in 0..40 {
            g.tick(&Input { select: Some(1), ..Default::default() });
        }
        g
    }

    /// Aim the camera line at the target's chest.
    fn aim(g: &mut Game) -> Input {
        let p = g.f[1].chest();
        for _ in 0..12 {
            let (pivot, dir, dist) = g.camera_rig(g.f[0].pos, g.f[0].yaw, g.f[0].pitch);
            let d = (p - (pivot - dir * dist)).norm();
            g.f[0].yaw = d.x.atan2(d.z);
            g.f[0].pitch = d.y.atan2(d.flat().len());
        }
        Input { yaw: g.f[0].yaw, pitch: g.f[0].pitch, ..Default::default() }
    }

    #[test]
    fn stars_burst_fly_and_hit_hard_with_a_small_supply() {
        let mut g = range();
        assert_eq!(g.f[0].weapon, Weapon::Shuriken);
        assert_eq!(g.f[0].mag[1], 12, "12 stars");
        let base = aim(&mut g);
        g.tick(&Input { fire: true, ..base });
        // not instant: nothing lands on the first tick
        assert_eq!(g.f[1].hp, 10_000.0);
        for _ in 0..60 {
            g.tick(&base);
        }
        assert_eq!(g.f[0].mag[1], 9, "one click throws a burst of three");
        let lost = 10_000.0 - g.f[1].hp;
        assert!(lost >= 3.0 * 32.0 * 0.6 && lost <= 3.0 * 32.0 * 1.01, "three body hits land: {lost}");
        // right click: a fan of three at once
        g.f[0].fire_cd = 0.0;
        g.tick(&Input { guard: true, ..base });
        assert_eq!(g.f[0].mag[1], 6, "the fan throws three at once");
        assert_eq!(g.grenades.iter().filter(|s| s.kind == 2).count(), 3);
        // empty: a longer reload than the pistols
        for _ in 0..80 {
            g.f[0].fire_cd = 0.0;
            g.tick(&Input { fire: true, ..base });
        }
        assert!(Weapon::Shuriken.reload_time() > Weapon::Revolver.reload_time());
        let mut reloaded = false;
        for _ in 0..(2.2 / TICK) as usize {
            g.tick(&base);
            reloaded |= g.f[0].mag[1] == 12;
        }
        assert!(reloaded, "reloads to 12");
    }

    #[test]
    fn balance_and_host_checks() {
        let s = Weapon::Shuriken.gun();
        // a star out-hits a revolver bullet on the body; a burst nearly kills
        assert!(s.dmg > Weapon::Revolver.gun().dmg);
        assert!(s.dmg * 3.0 >= 90.0 && s.dmg * 3.0 < MAX_HP, "a full body burst leaves them on a sliver");
        assert!(s.dmg * s.head < MAX_HP, "no one-star headshot kill");
        assert!(Weapon::Shuriken.mag() < Weapon::Smg.mag() && Weapon::Shuriken.mag() >= 9);
        // the host checks a star where it landed, even far from the thrower
        let g = range();
        let d = v3(0.0, 0.0, 1.0);
        let at = g.f[1].chest();
        assert!(g.validate_claim(0, 1, Weapon::Shuriken as u8, s.dmg, at - d, d, false).is_some(), "a star that landed");
        assert!(g.validate_claim(0, 1, Weapon::Shuriken as u8, s.dmg * 4.0, at - d, d, false).is_none(), "no star does four times its damage");
        assert!(g.validate_claim(0, 1, Weapon::Shuriken as u8, s.dmg, at + v3(3.0, 0.0, 0.0) - d, d, false).is_none(), "a star that missed");
    }
}

#[cfg(test)]
mod map_tests {
    use super::*;

    /// Distance in the ground plane from a point to a box's footprint.
    fn foot_dist(p: V3, b: &Aabb) -> f32 {
        let dx = (b.min.x - p.x).max(p.x - b.max.x).max(0.0);
        let dz = (b.min.z - p.z).max(p.z - b.max.z).max(0.0);
        (dx * dx + dz * dz).sqrt()
    }

    /// Spawns stand on something and inside nothing.
    fn spawns_are_clear(w: &World) {
        for &(p, _) in &w.spawn_points {
            let body = Aabb::new(p + v3(-RADIUS, 0.05, -RADIUS), p + v3(RADIUS, HEIGHT, RADIUS));
            assert!(!w.blocks.iter().any(|b| b.b.overlaps(&body)), "spawn {p:?} is clear");
            assert!((w.ground_below(p + v3(0.0, 0.1, 0.0)) - p.y).abs() < 0.05, "spawn {p:?} stands on something");
            assert!(p.x.abs() < w.half && p.z.abs() < w.half);
        }
    }

    /// Three levels: the ground, about 3 m, and 5 m or more.
    fn three_levels(w: &World) {
        let tops: Vec<f32> = w.blocks.iter().filter(|b| (b.b.max.x - b.b.min.x) * (b.b.max.z - b.b.min.z) >= 1.0).map(|b| b.b.max.y).collect();
        assert!(tops.iter().any(|&t| (t - 3.0).abs() < 0.3), "a middle level");
        assert!(tops.iter().any(|&t| (5.0..6.0).contains(&t)), "a top level");
    }

    /// Things on the ground you can't just see over.
    fn blockers(w: &World) -> Vec<Aabb> {
        w.blocks.iter().map(|b| b.b).filter(|b| b.min.y < 2.0 && b.max.y > 1.0).collect()
    }

    #[test]
    fn close_quarters_is_small_dense_and_vertical() {
        let w = World::build(MapId::CloseQuarters);
        assert_eq!(w.map, MapId::CloseQuarters);
        assert_eq!(MapId::CloseQuarters.name(), "Close Quarters");
        assert!((w.half * 2.0 - 30.0).abs() < 0.01, "30 x 30 m");
        let bl = blockers(&w);
        let mut widest: f32 = 0.0;
        let mut x = -w.half + 0.25;
        while x < w.half {
            let mut z = -w.half + 0.25;
            while z < w.half {
                let p = v3(x, 0.0, z);
                if !bl.iter().any(|b| foot_dist(p, b) <= 0.0) {
                    widest = widest.max(bl.iter().map(|b| foot_dist(p, b)).fold(f32::MAX, f32::min) * 2.0);
                }
                z += 0.5;
            }
            x += 0.5;
        }
        assert!(widest <= 8.5, "no open area more than ~8 m across: {widest:.1} m");
        three_levels(&w);
        spawns_are_clear(&w);
        let cy = World::build(MapId::Courtyard);
        assert_eq!(cy.map, MapId::Courtyard);
        assert!(cy.half > 20.0);
    }

    #[test]
    fn arena_is_open_with_a_clear_centre_and_three_lanes() {
        let w = World::arena();
        assert_eq!(w.map, MapId::Arena);
        assert!((w.half * 2.0 - 38.0).abs() < 0.01, "38 x 38 m");
        let bl = blockers(&w);
        // the middle 14 m: only the tower (with its ledges) and the pillar pair
        let centre: Vec<&Aabb> = bl.iter().filter(|b| foot_dist(V3::ZERO, b) < 7.0).collect();
        for b in &centre {
            let mid = (b.min + b.max) * 0.5;
            let tower = foot_dist(V3::ZERO, b) < 0.01 || (mid.x.abs() < 0.1 && mid.z.abs() < 2.5);
            let pillar = (mid.x.abs() - 4.5).abs() < 0.1 && mid.z.abs() < 0.1 && b.max.y > 6.0;
            assert!(tower || pillar, "only the tower and the pillar pair in the centre, found {b:?}");
        }
        assert!(centre.len() <= 5);
        // the side corridors are at least 5 m wide all along
        for side in [-1.0f32, 1.0] {
            let mut z = -15.0;
            while z <= 15.0 {
                let p = v3(side * 16.1, 0.0, z);
                let d = bl.iter().map(|b| foot_dist(p, b)).fold(f32::MAX, f32::min);
                assert!(d >= 2.49, "corridor at {p:?} only {:.2} m to a wall", d);
                z += 0.5;
            }
        }
        // three clear lanes from each spawn to the centre, each at least 5 m wide
        for x in [-10.5f32, 0.0, 10.5] {
            for off in [-0.45f32, 0.0, 0.45] {
                for sgn in [-1.0f32, 1.0] {
                    let (a, b) = (v3((x + off) * sgn, 1.0, -15.2 * sgn), v3((x + off) * sgn, 1.0, -7.0 * sgn));
                    assert!(w.line_clear(a, b), "lane at x {x} is open from spawn to the centre");
                }
            }
            let p = v3(x, 0.0, -10.0);
            let d = bl.iter().map(|b| foot_dist(p, b)).fold(f32::MAX, f32::min);
            assert!(d >= 2.37, "lane at x {x} is about 5 m wide ({:.2} m to the side)", d);
        }
        three_levels(&w);
        spawns_are_clear(&w);
        // the two sides start well apart
        let (red, blue): (Vec<V3>, Vec<V3>) = (w.spawn_points.iter().map(|s| s.0).filter(|p| p.z < 0.0).collect(), w.spawn_points.iter().map(|s| s.0).filter(|p| p.z > 0.0).collect());
        for r in &red {
            for b in &blue {
                assert!((*r - *b).len() > 15.0);
            }
        }
    }

    /// Run off one platform towards the other (`gap` metres apart, both 3 m up), jumping at the
    /// edge: `x0` start, `edge` where to jump, `far` the far platform's near edge, `z` the line.
    fn leap(map: MapId, x0: f32, edge: f32, far: f32, z: f32, dash: bool) -> V3 {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g.set_map(map);
        g.bots[1] = None;
        g.f[1].pos = v3(16.0, 0.0, 16.0);
        g.mt.phase = Phase::Fight;
        g.mt.timer = 99.0;
        let yaw = -std::f32::consts::FRAC_PI_2; // facing -x
        g.f[0].pos = v3(x0, 3.0, z);
        g.f[0].prev_pos = g.f[0].pos;
        g.f[0].yaw = yaw;
        g.f[0].on_ground = true;
        g.f[0].spawn_prot = 0.0;
        let run = Input { fwd: 1.0, yaw, ..Default::default() };
        for _ in 0..240 {
            if g.f[0].pos.x < edge {
                break;
            }
            g.tick(&run);
        }
        g.tick(&Input { jump: true, ..run });
        let mut dashed = false;
        for _ in 0..240 {
            let apex = g.f[0].vel.y <= 0.5;
            if dash && !dashed && apex && !g.f[0].on_ground {
                g.tick(&Input { dash: true, ..run });
                dashed = true;
                continue;
            }
            // over the far platform: pull back so you don't sail past it
            let brake = dashed && g.f[0].pos.x < far - 0.8;
            g.tick(&Input { fwd: if brake { -1.0 } else { 1.0 }, ..run });
            if g.f[0].on_ground && g.f[0].air_t == 0.0 && g.f[0].pos.x < edge - 0.75 {
                break;
            }
        }
        g.f[0].pos
    }

    #[test]
    fn the_platform_gaps_need_a_dash() {
        for (map, x0, edge, far, z) in [(MapId::CloseQuarters, 5.6, 2.25, -4.5, -6.75), (MapId::Arena, 7.6, 3.5, -3.25, -10.25)] {
            let plain = leap(map, x0, edge, far, z, false);
            assert!(plain.y < 0.5, "{map:?}: a plain running jump falls short: landed at {plain:?}");
            let dashed = leap(map, x0, edge, far, z, true);
            assert!((dashed.y - 3.0).abs() < 0.2 && dashed.x <= far + 0.05, "{map:?}: a jump and an air dash make it: landed at {dashed:?}");
        }
    }

    #[test]
    fn stairs_lead_up_to_the_balconies() {
        for (map, start, past) in [(MapId::CloseQuarters, v3(-9.6, 0.0, -7.5), -12.4), (MapId::Arena, v3(-12.2, 0.0, -10.5), -15.5)] {
            let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
            g.set_map(map);
            g.bots[1] = None;
            g.f[1].pos = v3(16.0, 0.0, 16.0);
            g.mt.phase = Phase::Fight;
            // walk up the stairs by the west corridor wall (facing -z)
            g.f[0].pos = start;
            g.f[0].prev_pos = start;
            g.f[0].spawn_prot = 0.0;
            let up = Input { fwd: 1.0, yaw: std::f32::consts::PI, ..Default::default() };
            for _ in 0..(2.5 / TICK) as usize {
                g.tick(&up);
            }
            assert!((g.f[0].pos.y - 3.0).abs() < 0.1 && g.f[0].pos.z < past, "{map:?}: on the balcony: {:?}", g.f[0].pos);
        }
    }
}
#[cfg(test)]
mod dash_charge_tests {
    use super::*;

    #[test]
    fn three_dashes_then_they_refill_one_at_a_time() {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.bots[1] = None;
        g.f[1].pos = v3(40.0, 0.0, 40.0);
        g.mt.phase = Phase::Fight;
        g.mt.timer = 99.0;
        g.f[0].pos = V3::ZERO;
        g.f[0].spawn_prot = 0.0;
        let fwd = Input { fwd: 1.0, ..Default::default() };
        // dash as soon as each one is allowed
        let mut dashes = 0;
        let mut was = false;
        for _ in 0..(1.4 / TICK) as usize {
            g.tick(&Input { dash: true, ..fwd });
            let now = g.f[0].dash_t > 0.0;
            if now && !was {
                dashes += 1;
            }
            was = now;
        }
        assert_eq!(dashes, g.tune.dash_charges as usize, "the full set in a row, then no more");
        assert!(g.f[0].dash_charges < 1.0);
        // each spent dash comes back after its refill time (one at a time)
        let refill = g.tune.dash_recharge;
        for _ in 0..((refill + 0.1) / TICK) as usize {
            g.tick(&fwd);
        }
        assert!(g.f[0].dash_charges >= 1.0 && g.f[0].dash_charges < 2.0, "one back: {}", g.f[0].dash_charges);
        for _ in 0..((refill * 2.0) / TICK) as usize {
            g.tick(&fwd);
        }
        assert!((g.f[0].dash_charges - g.tune.dash_charges).abs() < 1e-3, "all back: {}", g.f[0].dash_charges);
    }
}

#[cfg(test)]
mod gauntlet_tests {
    use super::*;

    fn kill(g: &mut Game, victim: usize, head: bool) {
        let at = g.f[victim].chest();
        let o = g.f[0].chest();
        g.f[victim].spawn_prot = 0.0;
        g.damage(0, victim, LETHAL, V3::ZERO, 0.0, head, at, Weapon::Revolver as u8, o, v3(0.0, 0.0, 1.0));
        g.tick(&Input::default());
    }

    #[test]
    fn waves_get_bigger_and_better_and_the_run_ends_when_you_fall() {
        let mut g = Game::gauntlet("P", 1000);
        g.mt.phase = Phase::Fight;
        g.f[0].spawn_prot = 0.0;
        for _ in 0..(1.0 / TICK) as usize {
            g.tick(&Input::default());
        }
        let gt = g.gauntlet.clone().unwrap();
        assert_eq!(gt.wave, 1);
        let bots: Vec<usize> = (1..g.f.len()).filter(|&s| g.f[s].present).collect();
        assert_eq!(bots.len(), gauntlet_wave(1).0);
        // the waves only ever fight you
        assert!(g.is_enemy(bots[0], 0) && g.is_enemy(0, bots[0]));
        // a headshot kill scores, clearing the wave scores more and gives a breather
        kill(&mut g, bots[0], true);
        let gt = g.gauntlet.clone().unwrap();
        assert_eq!(gt.score, gauntlet_kill_points(1, true) + 250);
        assert!(gt.break_t > 3.0, "a breather before the next wave");
        for _ in 0..(4.5 / TICK) as usize {
            g.tick(&Input::default());
        }
        let gt = g.gauntlet.clone().unwrap();
        assert_eq!(gt.wave, 2);
        let wave2: Vec<usize> = (1..g.f.len()).filter(|&s| g.f[s].present).collect();
        assert_eq!(wave2.len(), gauntlet_wave(2).0);
        assert!(gauntlet_wave(2).1 > gauntlet_wave(1).1 && gauntlet_wave(9).0 > gauntlet_wave(2).0, "bigger and better waves");
        if wave2.len() >= 2 {
            assert!(!g.is_enemy(wave2[0], wave2[1]), "bots don't fight each other");
        }
        // you fall: the run is over (no respawn), and the best is updated
        g.f[0].hp = 1.0;
        g.f[0].ap = 0.0;
        let at = g.f[0].chest();
        g.damage(wave2[0], 0, LETHAL, V3::ZERO, 0.0, false, at, Weapon::Revolver as u8, at, v3(0.0, 0.0, 1.0));
        for _ in 0..(5.0 / TICK) as usize {
            g.tick(&Input::default());
        }
        let gt = g.gauntlet.clone().unwrap();
        assert!(gt.over && g.mt.phase == Phase::MatchOver && !g.f[0].alive);
        assert_eq!(gt.best, 1000.max(gt.score));
        // try again: a fresh run, the best kept
        g.start_match();
        let gt = g.gauntlet.clone().unwrap();
        assert!(gt.wave == 0 && gt.score == 0 && !gt.over && gt.best == 1000.max(gauntlet_kill_points(1, true) + 250));
        assert!((1..g.f.len()).all(|s| !g.f[s].present), "the old wave is gone");
    }

    #[test]
    fn lobby_bots_fill_up_and_make_room() {
        let mut g = Game::new(Kind::Host, 0, Mode::Ffa, 20);
        g.add_player(0, "Host", true, None, 0);
        for _ in 0..10 {
            g.fill_bots(4, 0.5);
        }
        assert_eq!(g.present().len(), 4, "filled to four with bots");
        // a real player arrives: one bot steps aside
        let free = (0..MAX_PLAYERS).find(|&s| !g.f[s].present).unwrap();
        g.add_player(free, "Friend", false, None, 0);
        assert_eq!(g.present().len(), 5);
        let msg = g.fill_bots(4, 0.5).expect("a bot leaves");
        assert!(msg.contains("left"));
        assert_eq!(g.present().len(), 4);
        assert!(g.f[free].present && !g.f[free].is_bot, "the player stays");
        // nobody to send away but people: nothing changes
        let mut solo = Game::new(Kind::Host, 0, Mode::Ffa, 20);
        solo.add_player(0, "Host", true, None, 0);
        solo.add_player(1, "Friend", false, None, 0);
        assert!(solo.fill_bots(1, 0.5).is_none());
    }
}

#[cfg(test)]
mod finisher_tests {
    use super::*;

    /// You (katana) facing a beaten enemy 1.6 m away, plus a third fighter off to the side.
    fn setup() -> Game {
        let mut g = Game::offline("A", 2, 0.5, Mode::Ffa, 50);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.mt.phase = Phase::Fight;
        g.mt.timer = 99.0;
        for i in 0..3 {
            g.bots[i] = None;
            g.f[i].is_bot = i != 0;
            g.f[i].spawn_prot = 0.0;
            g.f[i].ap = 0.0;
            g.f[i].loadout.melee = Weapon::Katana;
        }
        g.f[0].pos = V3::ZERO;
        g.f[0].yaw = 0.0;
        g.f[0].hp = 40.0;
        g.f[1].pos = v3(0.0, 0.0, 1.6);
        g.f[1].yaw = std::f32::consts::PI;
        g.f[2].pos = v3(6.0, 0.0, 0.0);
        for i in 0..3 {
            g.f[i].prev_pos = g.f[i].pos;
            g.f[i].on_ground = true;
        }
        g.f[1].hp = 20.0;
        g.f[1].stagger_t = 0.6;
        g.f[1].stun_t = 0.6;
        g
    }

    #[test]
    fn only_beaten_staggered_enemies_in_reach_can_be_finished() {
        let g = setup();
        assert_eq!(g.finish_target(0), Some(1));
        let mut h = setup();
        h.f[1].hp = 30.0;
        assert_eq!(h.finish_target(0), None, "not hurt enough");
        let mut h = setup();
        h.f[1].stagger_t = 0.0;
        h.f[1].stun_t = 0.0;
        assert_eq!(h.finish_target(0), None, "not staggered");
        let mut h = setup();
        h.f[1].stagger_t = 0.0;
        h.f[1].guard_broke = 0.5;
        assert_eq!(h.finish_target(0), Some(1), "guard-broken counts");
        let mut h = setup();
        h.f[1].pos = v3(0.0, 0.0, 4.0);
        assert_eq!(h.finish_target(0), None, "out of reach");
    }

    #[test]
    fn a_finisher_executes_heals_and_is_counted() {
        let mut g = setup();
        g.tick(&Input { finish: true, ..Default::default() });
        let kind = g.f[0].fin;
        assert!(kind > 0, "the finisher starts");
        let style = finisher(kind);
        assert_eq!(style.weapon, Some(Weapon::Katana), "a katana finisher");
        assert!(style.dur < 1.0, "under a second");
        assert_eq!(g.f[1].held_by, Some(0), "the victim is held");
        let mut ticks = 0;
        while g.f[0].fin > 0 && ticks < 200 {
            g.tick(&Input::default());
            ticks += 1;
        }
        assert!((ticks as f32 * TICK) < 1.0);
        for _ in 0..30 {
            g.tick(&Input::default());
        }
        assert!(!g.f[1].alive, "executed");
        assert!(g.f[0].hp >= 40.0 + FINISH_HEAL - 1.0, "50 health back: {}", g.f[0].hp);
        assert_eq!(g.f[0].guard_hp, GUARD_MAX);
        assert!(g.feed.iter().any(|l| l.tool == "EXECUTED"));
        assert!(g.career.iter().any(|e| matches!(e, CareerEv::Kill { finisher, .. } if *finisher == kind)));
    }

    #[test]
    fn a_launched_target_gets_the_aerial_finisher() {
        let mut g = setup();
        g.f[1].pos = v3(0.0, 2.0, 1.6);
        g.f[1].prev_pos = g.f[1].pos;
        g.f[1].on_ground = false;
        g.f[1].float_t = 0.8;
        g.tick(&Input { finish: true, ..Default::default() });
        assert!(g.f[0].fin > 0);
        assert!(finisher(g.f[0].fin).weapon.is_none(), "the aerial one");
    }

    #[test]
    fn being_hit_cuts_a_finisher_short() {
        let mut g = setup();
        g.tick(&Input { finish: true, ..Default::default() });
        assert!(g.f[0].fin > 0);
        // someone else hits you before the blow lands
        let at = g.f[0].chest();
        let o = g.f[2].chest();
        g.damage(2, 0, 8.0, V3::ZERO, 0.0, false, at, Weapon::Revolver as u8, o, v3(-1.0, 0.0, 0.0));
        assert_eq!(g.f[0].fin, 0, "interrupted");
        for _ in 0..120 {
            g.tick(&Input::default());
        }
        assert!(g.f[1].alive, "the target lives");
    }

    #[test]
    fn the_host_checks_finisher_kills() {
        let g = setup();
        let (o, d) = (g.f[0].chest(), v3(0.0, 0.0, 1.0));
        assert!(g.validate_claim(0, 1, EXEC_TOOL, LETHAL, o, d, false).is_some());
        let mut healthy = setup();
        healthy.f[1].hp = 90.0;
        assert!(healthy.validate_claim(0, 1, EXEC_TOOL, LETHAL, o, d, false).is_none(), "no executing a healthy player");
    }
}

#[cfg(test)]
mod body_tests {
    use super::*;

    /// Two fighters 3 m apart run straight at each other for a second.
    fn run_at_each_other(mode: Mode, same_team: bool) -> f32 {
        let mut g = Game::offline("P", 1, 0.5, mode, 50);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.bots[1] = None;
        g.f[1].is_bot = true;
        g.mt.phase = Phase::Fight;
        g.f[1].team = if same_team { g.f[0].team } else { 1 - g.f[0].team };
        for k in 0..2 {
            g.f[k].spawn_prot = 0.0;
            g.f[k].vel = V3::ZERO;
            g.f[k].on_ground = true;
        }
        g.f[0].pos = v3(0.0, 0.0, -1.5);
        g.f[0].yaw = 0.0;
        g.f[1].pos = v3(0.0, 0.0, 1.5);
        g.f[1].yaw = std::f32::consts::PI;
        for _ in 0..(1.0 / TICK) as usize {
            // both push forward: the bot through its own input, us through ours
            g.f[1].yaw = std::f32::consts::PI;
            g.tick(&Input { fwd: 1.0, yaw: 0.0, ..Default::default() });
        }
        g.f[1].pos.z - g.f[0].pos.z
    }

    #[test]
    fn enemies_block_each_other_and_teammates_pass_through() {
        let gap = run_at_each_other(Mode::Ffa, false);
        assert!(gap >= RADIUS * 2.0 - 0.05, "enemies stop body to body (gap {gap})");
        let gap = run_at_each_other(Mode::Tdm, true);
        assert!(gap < 0.0, "teammates walk through each other (gap {gap})");
    }
}

#[cfg(test)]
mod engagement_tests {
    use super::*;

    fn fight(bots: usize) -> Game {
        let mut g = Game::offline("A", bots, 0.5, Mode::Ffa, 50);
        g.mt.phase = Phase::Fight;
        g.mt.timer = MATCH_TIME;
        for i in 1..=bots {
            g.bots[i] = None; // they stand still: the test decides who dies
        }
        for _ in 0..10 {
            g.tick(&Input::default());
        }
        g
    }

    #[test]
    fn matches_last_six_minutes() {
        assert!((300.0..=420.0).contains(&MATCH_TIME));
    }

    #[test]
    fn a_three_kill_streak_earns_a_writ_and_claiming_it_pays() {
        let mut g = fight(2);
        for _ in 0..WRIT_STREAK {
            g.on_death(2, Some(1), Weapon::Revolver as u8, false);
        }
        assert!(g.has_writ(1), "three kills in a row: a Writ");
        assert!(g.feed.iter().any(|l| l.text.as_deref().is_some_and(|t| t.starts_with("WRIT"))));
        g.career.clear();
        g.on_death(1, Some(0), Weapon::Revolver as u8, false);
        assert!(!g.has_writ(1), "claimed");
        assert!(g.career.contains(&CareerEv::WritClaim { streak: WRIT_STREAK }), "the claim pays XP: {:?}", g.career);
        assert!(g.feed.iter().any(|l| l.text.as_deref().is_some_and(|t| t.starts_with("WRIT CLAIMED"))));
        // dying ends a streak
        assert_eq!(g.streaks[1], 0);
    }

    #[test]
    fn play_of_the_match_replays_the_best_moment() {
        let mut g = fight(2);
        for _ in 0..240 {
            g.tick(&Input::default());
        }
        // a lone kill, then a double kill: the double is the better moment
        g.on_death(2, Some(1), Weapon::Revolver as u8, false);
        for _ in 0..240 {
            g.tick(&Input::default());
        }
        g.on_death(1, Some(0), Weapon::Shotgun as u8, true);
        g.on_death(2, Some(0), Weapon::Shotgun as u8, false);
        for _ in 0..(POTM_AFTER / TICK) as usize + 20 {
            g.tick(&Input::default());
        }
        g.mt.timer = 0.0;
        for _ in 0..4 {
            g.tick(&Input::default());
        }
        assert_eq!(g.mt.phase, Phase::MatchOver);
        let h = g.potm.clone().expect("a Play of the Match");
        assert_eq!(h.player, 0);
        assert_eq!(h.what, "DOUBLE KILL");
        assert!(h.duration() > POTM_BEFORE, "the moment and what led up to it: {}", h.duration());
        assert!(g.career.contains(&CareerEv::MatchEnd));
        // it plays after the kill cam, then it's done
        let live = g.f[1].pos;
        let mut acc = 0.0;
        let mut seen = false;
        for _ in 0..600 {
            let mut inp = Input::default();
            g.advance(&mut acc, 1.0 / 60.0, &mut inp);
            if g.potm_t.is_some() {
                seen = true;
                assert!(g.potm_view_begin());
                g.potm_view_end();
                assert_eq!(g.f[1].pos, live, "the live match comes back after drawing");
            }
            if g.potm_done {
                break;
            }
        }
        assert!(seen && g.potm_done, "played through");
    }
}

#[cfg(test)]
mod ability_tests {
    use super::*;
    use crate::world::{Block, Mat};

    /// You (slot 0, playing `body`) at the origin facing +z on open ground, still targets at
    /// 10 m and 30 m ahead.
    fn arena(body: u8) -> Game {
        let mut g = Game::offline("A", 2, 0.5, Mode::Ffa, 50);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.mt.phase = Phase::Fight;
        g.mt.timer = MATCH_TIME;
        for i in 1..=2 {
            g.bots[i] = None;
        }
        g.f[0].loadout.body = body;
        for (i, z) in [(0usize, 0.0f32), (1, 10.0), (2, 30.0)] {
            g.f[i].pos = v3(0.0, 0.0, z);
            g.f[i].prev_pos = g.f[i].pos;
            g.f[i].vel = V3::ZERO;
            g.f[i].yaw = if i == 0 { 0.0 } else { std::f32::consts::PI };
            g.f[i].spawn_prot = 0.0;
        }
        g.tick(&Input::default());
        g
    }

    fn run(g: &mut Game, secs: f32, inp: Input) {
        for _ in 0..(secs / TICK) as usize {
            g.tick(&inp);
        }
    }

    fn press(g: &mut Game, pitch: f32) {
        g.tick(&Input { ability: true, pitch, ..Default::default() });
    }

    #[test]
    fn wardens_mark_shows_enemies_near_it_then_fades() {
        let mut g = arena(0);
        press(&mut g, 0.0);
        assert_eq!(g.marks.len(), 1, "thrown");
        assert!(g.f[0].ab_charge < 1.0, "on cooldown");
        press(&mut g, 0.0);
        assert_eq!(g.marks.len(), 1, "one at a time");
        run(&mut g, 1.5, Input::default());
        assert!(g.marks[0].landed);
        assert!(g.revealed_to(0, 1), "the near enemy shows");
        assert!(!g.revealed_to(0, 2), "the far one doesn't");
        assert!(!g.revealed_to(1, 2), "only for the one who threw it (and their team)");
        let fade = g.tune.mark_time + 0.5;
        run(&mut g, fade, Input::default());
        assert!(g.marks.is_empty() && !g.revealed_to(0, 1), "it fades");
    }

    #[test]
    fn grapple_pulls_you_to_a_wall_and_jump_lets_go() {
        let mut g = arena(1);
        g.world.blocks.push(Block { b: Aabb::new(v3(-6.0, 0.0, 15.0), v3(6.0, 8.0, 16.0)), mat: Mat::Wall });
        for i in 1..=2 {
            g.f[i].pos = v3(20.0, 0.0, 0.0);
        }
        assert!(g.f[0].ab_charge >= 2.0, "two charges");
        press(&mut g, 0.08);
        assert!(g.f[0].grapple.is_some(), "hooked the wall");
        run(&mut g, 0.45, Input { pitch: 0.08, ..Default::default() });
        assert!(g.f[0].pos.z > 4.0, "pulled towards it: z {}", g.f[0].pos.z);
        let speed = g.f[0].vel.flat().len();
        g.tick(&Input { jump: true, pitch: 0.08, ..Default::default() });
        assert!(g.f[0].grapple.is_none(), "jump lets go");
        assert!(g.f[0].vel.flat().len() > speed * 0.6, "keeping the momentum");
        // the second charge (from back in range), then none left
        run(&mut g, 1.0, Input::default());
        g.f[0].pos = V3::ZERO;
        g.f[0].prev_pos = V3::ZERO;
        g.f[0].vel = V3::ZERO;
        run(&mut g, 0.3, Input { pitch: 0.08, ..Default::default() });
        press(&mut g, 0.08);
        assert!(g.f[0].ab_charge < 1.0);
        let before = g.f[0].ab_charge;
        press(&mut g, 0.08);
        assert!(g.f[0].ab_charge >= before, "no third pull");
        // nothing to hook in range: nothing happens and no charge is spent
        let mut g = arena(1);
        press(&mut g, 0.5);
        assert!(g.f[0].grapple.is_none() && g.f[0].ab_charge >= 2.0, "open sky: no pull, no charge spent");
    }

    #[test]
    fn iron_wall_stops_bullets_from_the_front_and_throws_back_blades() {
        let mut g = arena(2);
        // the wall is up on fighter 0, who faces +z; fighter 1 shoots from the front
        press(&mut g, 0.0);
        assert!(g.f[0].shield_t > 0.0);
        let chest = g.f[0].chest();
        let front = v3(0.0, 0.0, -1.0);
        g.fire_ray(1, chest - front * 6.0, front, 28.0, 150.0, Weapon::Revolver);
        g.tick(&Input::default());
        assert_eq!((g.f[0].hp, g.f[0].ap), (MAX_HP, MAX_AP), "stopped by the shield");
        assert!(g.validate_claim(1, 0, Weapon::Revolver as u8, 28.0, chest - front * 6.0, front, false).is_none(), "the host agrees");
        // from behind it's no help
        let back = v3(0.0, 0.0, 1.0);
        g.fire_ray(1, chest - back * 6.0, back, 28.0, 150.0, Weapon::Revolver);
        g.tick(&Input::default());
        assert!(g.f[0].hp < MAX_HP || g.f[0].ap < MAX_AP, "a shot in the back lands");
        // a blade from the front: the attacker is thrown back and staggered, no damage
        let (hp, ap) = (g.f[0].hp, g.f[0].ap);
        g.f[1].pos = v3(0.0, 0.0, 1.5);
        g.f[1].yaw = std::f32::consts::PI;
        g.f[1].cur = 0;
        g.f[1].weapon = g.f[1].loadout.melee;
        g.f[1].slash_idx = 0;
        g.resolve_slash(1);
        assert_eq!((g.f[0].hp, g.f[0].ap), (hp, ap));
        assert!(g.f[1].stun_t > 0.0 && g.f[1].vel.z > 3.0, "thrown back: {:?}", g.f[1].vel);
    }

    #[test]
    fn afterimage_vanishes_and_sends_a_decoy() {
        let mut g = arena(3);
        press(&mut g, 0.0);
        assert!(g.f[0].invis_t > 0.0, "vanished");
        assert_eq!(g.decoys.len(), 1, "a decoy runs on");
        assert_ne!(g.nearest_enemy(1), Some(0), "bots lose sight of you");
        let start = g.decoys[0].pos;
        run(&mut g, 0.4, Input::default());
        assert!((g.decoys[0].pos - start).len() > 2.0, "the decoy runs");
        let fake = g.decoy_fighter(&g.decoys[0]).expect("drawn as you");
        assert_eq!(fake.loadout.body, 3);
        run(&mut g, 0.4, Input::default());
        assert_eq!(g.f[0].invis_t, 0.0, "back in sight");
        let life = g.tune.decoy_time;
        run(&mut g, life, Input::default());
        assert!(g.decoys.is_empty(), "the decoy fades");
    }

    #[test]
    fn passives() {
        // Corvin Vale: a kill reloads every gun
        let mut g = arena(0);
        g.f[0].mag = [0, 0, 0];
        g.on_death(1, Some(0), Weapon::Revolver as u8, false);
        assert_eq!(g.f[0].mag[1], g.f[0].slot_weapon(1).mag());
        assert_eq!(g.f[0].mag[2], g.f[0].slot_weapon(2).mag());
        // Bram Kask: a bigger guard meter
        let g = arena(2);
        assert!((g.guard_max_of(&g.f[0]) - GUARD_MAX * 1.3).abs() < 1e-3);
        assert_eq!(g.guard_max_of(&g.f[1]), GUARD_MAX);
        // Ilya Fen: the flow chain fades slower when you stop
        let mut a = arena(3);
        let mut b = arena(0);
        for g in [&mut a, &mut b] {
            g.f[0].flow = 3;
            run(g, 0.3, Input::default());
        }
        assert!(a.f[0].flow > 0 && b.f[0].flow == 0, "Ilya keeps it a little longer ({} vs {})", a.f[0].flow, b.f[0].flow);
        // Sable Rook: her scope is timed (her glint hides for its first moment)
        let mut g = arena(1);
        g.f[0].loadout.gun1 = Weapon::Sniper;
        g.f[0].cur = 1;
        g.f[0].weapon = Weapon::Sniper;
        g.f[0].switch_t = 0.0;
        g.f[0].reload_t = 0.0;
        run(&mut g, 0.3, Input { guard: true, ..Default::default() });
        assert!(g.f[0].scope_t > 0.0 && g.scoped_since[0] >= 0.0, "scoped, and timed");
    }

    #[test]
    fn classic_lobbies_have_no_abilities_and_duels_do() {
        let mut g = arena(3);
        g.mt.classic = true;
        press(&mut g, 0.0);
        assert!(g.decoys.is_empty() && g.f[0].invis_t == 0.0);
        assert_eq!(g.guard_max_of(&g.f[0]), GUARD_MAX);
        // a Duel follows the setting like every other mode
        let mut d = arena(3);
        d.mt.mode = Mode::Duel;
        assert!(d.abilities_on());
        press(&mut d, 0.0);
        assert_eq!(d.decoys.len(), 1, "abilities work in a Duel");
    }

    #[test]
    fn the_host_checks_ability_uses() {
        let mut g = Game::new(Kind::Host, 0, Mode::Ffa, 15);
        g.world = crate::game::lag_comp_tests::flat_world();
        g.add_player(0, "Host", true, None, 0);
        g.add_player(1, "Guest", false, None, 0);
        g.mt.phase = Phase::Fight;
        g.f[1].alive = true;
        g.f[1].loadout.body = 3;
        g.f[1].pos = v3(0.0, 0.0, 5.0);
        let p = g.f[1].pos;
        let ev = |kind: Ability, p: V3| Ev::Ability { who: 1, kind: kind as u8, p, d: v3(0.0, 0.0, 7.0) };
        // the wrong ability for their fighter, or someone else's: thrown out
        g.handle_remote_event(1, ev(Ability::Wall, p));
        assert_eq!(g.f[1].shield_t, 0.0);
        g.handle_remote_event(2, ev(Ability::Afterimage, p));
        assert!(g.decoys.is_empty());
        // their own: it happens and is passed on
        g.out_events.clear();
        g.handle_remote_event(1, ev(Ability::Afterimage, p));
        assert_eq!(g.decoys.len(), 1);
        assert!(g.out_events.iter().any(|e| matches!(e, Ev::Ability { who: 1, .. })), "passed on to the others");
        // again right away: still on cooldown
        g.handle_remote_event(1, ev(Ability::Afterimage, p));
        assert_eq!(g.decoys.len(), 1, "on cooldown: thrown out");
        // far from where they stand: thrown out
        assert!(!g.ability_ok(1, Ability::Afterimage, p + v3(30.0, 0.0, 0.0)));
        // grapples reach 20 m, not 40
        g.f[1].loadout.body = 1;
        assert!(g.ability_ok(1, Ability::Grapple, p + v3(0.0, 2.0, 15.0)));
        assert!(!g.ability_ok(1, Ability::Grapple, p + v3(0.0, 0.0, 40.0)));
        // a Classic lobby: none at all
        g.mt.classic = true;
        assert!(!g.ability_ok(1, Ability::Grapple, p + v3(0.0, 2.0, 15.0)));
    }

    #[test]
    fn bots_use_their_abilities() {
        let mut g = Game::offline("A", 3, 0.9, Mode::Ffa, 50);
        g.mt.phase = Phase::Fight;
        g.mt.timer = MATCH_TIME;
        g.f[0].is_bot = true;
        g.bots[0] = Some(crate::bot::Bot::new(0.9, 5));
        for i in 0..4 {
            g.f[i].loadout.body = i as u8;
        }
        let mut used = [false; 4];
        for _ in 0..(40.0 / TICK) as usize {
            g.tick(&Input::default());
            for i in 0..4 {
                if g.f[i].ab_charge < Ability::of(i as u8).charges(&g.tune) - 0.01 {
                    used[i] = true;
                }
            }
        }
        assert!(used.iter().filter(|&&u| u).count() >= 2, "bots use abilities: {used:?}");
    }
}

#[cfg(test)]
mod ability_mode_tests {
    use super::*;

    #[test]
    fn every_practice_mode_has_abilities() {
        for mode in Mode::ALL.into_iter().chain([Mode::Gauntlet]) {
            let mut g = if mode == Mode::Gauntlet { Game::gauntlet("A", 0) } else { Game::offline("A", 1, 0.5, mode, 5) };
            g.mt.classic = false; // the menu's "Abilities" setting, on (the default)
            g.f[0].ready = true;
            g.f[0].loadout.body = 3;
            g.my_loadout.body = 3;
            // play until the fight starts and you're in it
            let mut n = 0;
            while !(g.mt.phase == Phase::Fight && g.f[0].alive && !g.f[0].spectate) && n < 60 * 120 {
                g.tick(&Input::default());
                n += 1;
            }
            assert!(g.abilities_on(), "{mode:?}: abilities on");
            g.tick(&Input { ability: true, ..Default::default() });
            assert!(g.f[0].invis_t > 0.0, "{mode:?}: Q works (phase {:?})", g.mt.phase);
        }
    }
}