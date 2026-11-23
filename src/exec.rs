//! EXECUTIONS: each fighter's signature finishing moves (three on the ground, one in the air).
//!
//! Everything about one is data here: how long it lasts and when the killing blow lands, the
//! animation (pieces of the Quaternius clips laid on a timeline, plus hand-keyed body motion and
//! poses on our skeleton), the moments that make a sound or an effect (cues), the camera's shots,
//! how the victim is moved, the mastery level that unlocks it and its announcer line (read from
//! assets/lore.txt, `[executions]`). The game, the renderer and the camera all read the same
//! tables, so every computer plays the same execution the same way.
//!
//! All names, moves and lines are this game's own.

use crate::math::*;

/// Three signature executions and one aerial per fighter.
pub const PER_FIGHTER: usize = 4;
pub const COUNT: usize = 16;

/// Something that happens at a moment of an execution (a sound, an effect, the camera).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// A blade or body cutting the air.
    Whoosh,
    /// A blow that hurts but doesn't kill (the victim flinches).
    Strike,
    /// The victim's weapon knocked aside: sparks and a clang.
    Disarm,
    /// The killing blow (the kill itself is decided by the game; this is how it looks).
    Lethal,
    /// The blade hits the ground: a cracked floor, dust and debris.
    Slam,
    /// A point-blank shot from the gun in hand.
    Shot,
    /// A hunter's line fired at the victim, yanking them in.
    Yank,
    /// Gone in a blur: afterimages and a hollow whoosh.
    Blur,
    /// Forge embers off a hot blade.
    Embers,
    /// A glint off the warden's badge.
    Glint,
    /// A flip or a leap: air and dust.
    Flip,
    /// The ring-out after the blow.
    Ring,
    /// The crowd roars and the announcer speaks.
    Crowd,
}

/// A piece of an animation clip played over part of the execution.
#[derive(Clone, Copy, Debug)]
pub struct Seg {
    /// Share of the execution it covers.
    pub from: f32,
    pub to: f32,
    pub clip: &'static str,
    /// Clip time at its start and end (seconds); past the clip's end it wraps when `wrap`.
    pub c0: f32,
    pub c1: f32,
    pub wrap: bool,
}

const fn seg(from: f32, to: f32, clip: &'static str, c0: f32, c1: f32) -> Seg {
    Seg { from, to, clip, c0, c1, wrap: false }
}
const fn segw(from: f32, to: f32, clip: &'static str, c0: f32, c1: f32) -> Seg {
    Seg { from, to, clip, c0, c1, wrap: true }
}

/// What the camera looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Attacker,
    Victim,
    /// Between them.
    Mid,
}

/// One camera shot, from `at` until the next one starts (a cut). Within it the camera moves from
/// the first to the second value of `yaw` and `dist` (an orbit or a push-in).
///
/// `yaw` is measured round the focus from the side: 0 is to the attacker's right of the line
/// between them, +pi/2 behind the attacker, -pi/2 behind the victim.
#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub at: f32,
    pub focus: Focus,
    pub yaw: (f32, f32),
    pub dist: (f32, f32),
    /// Height of the camera above the point it looks at (negative: a low angle looking up).
    pub h: f32,
    /// Height of the point looked at above the focus's feet.
    pub look_h: f32,
    pub fov: f32,
}

const fn shot(at: f32, focus: Focus, yaw: (f32, f32), dist: (f32, f32), h: f32, look_h: f32, fov: f32) -> Shot {
    Shot { at, focus, yaw, dist, h, look_h, fov }
}

/// Which way an afterimage copy of the attacker appears.
#[derive(Clone, Copy, Debug)]
pub enum Ghosts {
    None,
    /// Copies striking from round the victim: (share it strikes at, angle round the victim).
    Ring(&'static [(f32, f32)]),
    /// A trail of copies behind the attacker as they move.
    Trail,
}

pub struct Execution {
    /// Key of its announcer line in assets/lore.txt.
    pub id: &'static str,
    pub name: &'static str,
    /// The fighter it belongs to (crate::chars): 0 Corvin, 1 Sable, 2 Anvil, 3 Ilya.
    pub fighter: u8,
    /// Done on a target in the air.
    pub aerial: bool,
    /// Seconds it lasts, and when (share of that) the killing blow lands.
    pub dur: f32,
    pub hit: f32,
    /// The fighter's mastery level it unlocks at (1: from the start).
    pub mastery: u32,
    pub segs: &'static [Seg],
    pub cues: &'static [(f32, Cue)],
    pub shots: &'static [Shot],
    /// When the gun is in hand instead of the blade.
    pub gun: Option<(f32, f32)>,
    /// When the victim is yanked in (to arm's reach).
    pub pull: Option<(f32, f32)>,
    /// When the victim is beaten down to their knees.
    pub kneel: Option<(f32, f32)>,
    /// The slow-motion stretch round the blow (the animation is authored slowed down here).
    pub slow: (f32, f32),
    /// When the attacker is only a blur (not drawn).
    pub hidden: Option<(f32, f32)>,
    pub ghosts: Ghosts,
}

use Cue::*;
use Focus::*;

const PI: f32 = std::f32::consts::PI;
const TAU: f32 = std::f32::consts::TAU;

// contact frames of the clips (where the hand moves fastest), measured from the clips
// (anim::rig_tests::probe_clip_contacts)
const A_HIT: f32 = 0.25; // Sword_Regular_A
const B_HIT: f32 = 0.27; // Sword_Regular_B
const C_HIT: f32 = 0.65; // Sword_Regular_C (the spinning cut)
const ATTACK_HIT: f32 = 0.38; // Sword_Attack
const DASH_HIT: f32 = 0.33; // Sword_Dash
const HOOK_HIT: f32 = 0.25; // Melee_Hook
const SLAM_HIT: f32 = 2.55; // Sword_Heavy_Combo, its last blow
const CHOP_HIT: f32 = 0.97 + 0.13; // TreeChopping_Loop, wrapped once

pub const EXECUTIONS: [Execution; COUNT] = [
    // ---------------------------------------------------------------- Corvin Vale: the warden
    Execution {
        id: "wardens_verdict",
        name: "Warden's Verdict",
        fighter: 0,
        aerial: false,
        dur: 2.1,
        hit: 0.64,
        mastery: 1,
        segs: &[
            seg(0.0, 0.14, "Melee_Hook", 0.0, HOOK_HIT),
            seg(0.14, 0.22, "Melee_Hook", HOOK_HIT, 0.42),
            seg(0.22, 0.46, "Sword_Regular_C", 0.3, 0.6),
            seg(0.46, 0.64, "Sword_Attack", 0.05, ATTACK_HIT),
            seg(0.64, 0.82, "Sword_Attack", ATTACK_HIT, 0.47),
            seg(0.82, 1.0, "Sword_Attack", 0.47, 1.25),
        ],
        cues: &[(0.08, Whoosh), (0.14, Disarm), (0.26, Whoosh), (0.48, Glint), (0.58, Whoosh), (0.64, Lethal), (0.7, Ring), (0.8, Crowd)],
        shots: &[
            shot(0.0, Mid, (0.4, 0.9), (3.4, 3.0), 0.4, 1.1, 58.0),
            shot(0.44, Attacker, (-0.75, -0.6), (1.7, 1.5), 0.05, 1.45, 42.0),
            shot(0.6, Mid, (0.15, -0.05), (3.0, 3.5), -0.65, 1.0, 64.0),
            shot(0.82, Attacker, (1.35, 1.65), (3.6, 4.6), 1.5, 1.2, 56.0),
        ],
        gun: None,
        pull: None,
        kneel: Some((0.16, 0.64)),
        slow: (0.62, 0.82),
        hidden: None,
        ghosts: Ghosts::None,
    },
    Execution {
        id: "border_ledger",
        name: "Border Ledger",
        fighter: 0,
        aerial: false,
        dur: 2.3,
        hit: 0.72,
        mastery: 3,
        segs: &[
            seg(0.0, 0.14, "Sword_Regular_Combo", 0.0, 0.25),
            seg(0.14, 0.34, "Sword_Regular_Combo", 0.25, 0.73),
            seg(0.34, 0.52, "Sword_Regular_Combo", 0.73, 1.2),
            seg(0.52, 0.72, "Sword_Attack", 0.1, ATTACK_HIT),
            seg(0.72, 0.86, "Sword_Attack", ATTACK_HIT, 0.46),
            seg(0.86, 1.0, "Sword_Attack", 0.46, 1.1),
        ],
        cues: &[(0.08, Whoosh), (0.14, Strike), (0.28, Whoosh), (0.34, Strike), (0.5, Glint), (0.64, Whoosh), (0.72, Lethal), (0.77, Ring), (0.85, Crowd)],
        shots: &[
            shot(0.0, Mid, (-0.5, -0.2), (3.1, 2.9), 0.3, 1.1, 58.0),
            shot(0.34, Attacker, (-0.85, -0.75), (2.2, 2.0), 0.25, 1.4, 48.0),
            shot(0.62, Victim, (0.9, 0.8), (1.8, 1.6), 0.1, 1.2, 44.0),
            shot(0.8, Mid, (0.3, 1.0), (3.8, 4.4), 0.8, 1.0, 60.0),
        ],
        gun: None,
        pull: None,
        kneel: Some((0.36, 0.72)),
        slow: (0.7, 0.86),
        hidden: None,
        ghosts: Ghosts::None,
    },
    Execution {
        id: "lantern_watch",
        name: "Lantern Watch",
        fighter: 0,
        aerial: false,
        dur: 2.0,
        hit: 0.32,
        mastery: 6,
        segs: &[
            seg(0.0, 0.14, "Sword_Dash", 0.0, 0.06),
            seg(0.14, 0.32, "Sword_Dash", 0.06, DASH_HIT),
            seg(0.32, 0.5, "Sword_Dash", DASH_HIT, 0.45),
            seg(0.5, 1.0, "Sword_Dash", 0.45, 1.5),
        ],
        cues: &[(0.08, Glint), (0.24, Whoosh), (0.32, Lethal), (0.56, Ring), (0.7, Crowd)],
        shots: &[
            shot(0.0, Attacker, (-0.8, -0.7), (2.1, 1.8), -0.3, 1.3, 50.0),
            shot(0.22, Mid, (0.0, 0.05), (4.4, 4.4), 0.5, 1.0, 62.0),
            shot(0.42, Victim, (0.9, 0.55), (2.6, 2.4), 0.2, 1.2, 50.0),
            shot(0.72, Attacker, (0.6, 1.0), (3.4, 4.2), 1.2, 1.2, 58.0),
        ],
        gun: None,
        pull: None,
        kneel: None,
        slow: (0.3, 0.5),
        hidden: None,
        ghosts: Ghosts::None,
    },
    Execution {
        id: "ridge_fall",
        name: "Ridge Fall",
        fighter: 0,
        aerial: true,
        dur: 1.9,
        hit: 0.58,
        mastery: 4,
        segs: &[
            seg(0.0, 0.3, "NinjaJump_Start", 0.0, 0.45),
            seg(0.3, 0.58, "Sword_Heavy_Combo", 2.25, SLAM_HIT),
            seg(0.58, 0.76, "Sword_Heavy_Combo", SLAM_HIT, 2.66),
            seg(0.76, 1.0, "NinjaJump_Land", 0.0, 0.85),
        ],
        cues: &[(0.08, Flip), (0.36, Glint), (0.5, Whoosh), (0.58, Lethal), (0.62, Ring), (0.8, Slam), (0.84, Crowd)],
        shots: &[
            shot(0.0, Mid, (0.6, 0.8), (3.2, 3.0), -0.9, 1.4, 60.0),
            shot(0.48, Attacker, (1.1, 1.0), (2.0, 1.8), 0.2, 1.3, 46.0),
            shot(0.7, Mid, (0.5, 0.9), (4.4, 5.0), 2.0, 0.4, 60.0),
        ],
        gun: None,
        pull: None,
        kneel: None,
        slow: (0.56, 0.76),
        hidden: None,
        ghosts: Ghosts::None,
    },
    // ---------------------------------------------------------------- Sable Rook: the collector
    Execution {
        id: "the_collection",
        name: "The Collection",
        fighter: 1,
        aerial: false,
        dur: 2.4,
        hit: 0.6,
        mastery: 1,
        segs: &[
            seg(0.0, 0.12, "OverhandThrow", 0.0, 0.33),
            seg(0.12, 0.3, "OverhandThrow", 0.33, 0.9),
            seg(0.3, 0.4, "Melee_Hook", 0.0, HOOK_HIT),
            seg(0.4, 0.44, "Melee_Hook", HOOK_HIT, 0.4),
            seg(0.44, 1.0, "Sword_Idle", 0.0, 1.0),
        ],
        cues: &[(0.06, Yank), (0.3, Whoosh), (0.4, Strike), (0.6, Shot), (0.6, Lethal), (0.66, Ring), (0.8, Crowd)],
        shots: &[
            shot(0.0, Mid, (0.2, 0.35), (4.0, 3.6), 0.4, 1.0, 60.0),
            shot(0.3, Attacker, (-0.8, -0.7), (1.8, 1.6), 0.15, 1.45, 40.0),
            shot(0.55, Mid, (0.6, 0.5), (2.1, 1.9), 0.05, 1.1, 46.0),
            shot(0.68, Attacker, (1.0, 1.45), (3.0, 3.2), -0.45, 1.3, 56.0),
        ],
        gun: Some((0.44, 1.0)),
        pull: Some((0.08, 0.28)),
        kneel: Some((0.42, 0.6)),
        slow: (0.58, 0.72),
        hidden: None,
        ghosts: Ghosts::None,
    },
    Execution {
        id: "bounty_closed",
        name: "Bounty Closed",
        fighter: 1,
        aerial: false,
        dur: 2.2,
        hit: 0.62,
        mastery: 3,
        segs: &[
            seg(0.0, 0.22, "Slide_Start", 0.0, 0.5),
            seg(0.22, 0.36, "Melee_Hook", 0.0, HOOK_HIT),
            seg(0.36, 0.4, "Melee_Hook", HOOK_HIT, 0.42),
            seg(0.4, 1.0, "Sword_Idle", 0.0, 1.0),
        ],
        cues: &[(0.1, Whoosh), (0.18, Strike), (0.3, Whoosh), (0.36, Strike), (0.62, Shot), (0.62, Lethal), (0.68, Ring), (0.82, Crowd)],
        shots: &[
            shot(0.0, Mid, (0.4, 0.55), (3.2, 3.0), -0.1, 0.9, 58.0),
            shot(0.36, Victim, (1.2, 1.15), (2.0, 1.9), 1.3, 0.8, 50.0),
            shot(0.56, Mid, (0.5, 0.45), (2.4, 2.2), 0.3, 1.0, 50.0),
            shot(0.7, Mid, (0.7, 1.1), (3.8, 4.4), 1.0, 0.8, 58.0),
        ],
        gun: Some((0.4, 1.0)),
        pull: None,
        kneel: Some((0.2, 0.62)),
        slow: (0.6, 0.74),
        hidden: None,
        ghosts: Ghosts::None,
    },
    Execution {
        id: "frozen_lake",
        name: "Frozen Lake",
        fighter: 1,
        aerial: false,
        dur: 2.3,
        hit: 0.66,
        mastery: 6,
        segs: &[
            seg(0.0, 0.16, "Melee_Hook", 0.0, HOOK_HIT),
            seg(0.16, 0.42, "Sword_Regular_C", 0.3, C_HIT),
            seg(0.42, 1.0, "Sword_Idle", 0.0, 1.0),
        ],
        cues: &[(0.1, Whoosh), (0.16, Strike), (0.3, Whoosh), (0.66, Shot), (0.66, Lethal), (0.72, Ring), (0.84, Crowd)],
        shots: &[
            shot(0.0, Mid, (-0.4, -0.2), (3.0, 2.8), 0.3, 1.0, 58.0),
            shot(0.2, Victim, (0.0, 0.6), (3.0, 3.2), 0.6, 1.1, 56.0),
            shot(0.6, Mid, (0.55, 0.5), (2.4, 2.2), 0.3, 1.0, 50.0),
            shot(0.74, Mid, (0.4, 0.9), (4.0, 4.6), 1.4, 0.7, 58.0),
        ],
        gun: Some((0.44, 1.0)),
        pull: None,
        kneel: Some((0.2, 0.66)),
        slow: (0.64, 0.78),
        hidden: None,
        ghosts: Ghosts::None,
    },
    Execution {
        id: "hook_and_line",
        name: "Hook and Line",
        fighter: 1,
        aerial: true,
        dur: 1.8,
        hit: 0.6,
        mastery: 4,
        segs: &[
            seg(0.0, 0.14, "OverhandThrow", 0.0, 0.33),
            seg(0.14, 0.32, "OverhandThrow", 0.33, 0.9),
            seg(0.32, 0.8, "Sword_Idle", 0.0, 1.0),
            seg(0.8, 1.0, "NinjaJump_Land", 0.0, 0.8),
        ],
        cues: &[(0.06, Yank), (0.3, Whoosh), (0.6, Shot), (0.6, Lethal), (0.66, Ring), (0.84, Crowd)],
        shots: &[
            shot(0.0, Mid, (0.3, 0.5), (3.8, 3.4), -0.6, 1.3, 60.0),
            shot(0.5, Mid, (0.6, 0.5), (2.6, 2.4), 0.0, 1.2, 50.0),
            shot(0.68, Mid, (0.6, 1.0), (4.2, 4.8), 1.6, 0.6, 60.0),
        ],
        gun: Some((0.32, 0.8)),
        pull: Some((0.08, 0.3)),
        kneel: None,
        slow: (0.58, 0.72),
        hidden: None,
        ghosts: Ghosts::None,
    },
    // ---------------------------------------------------------------- Bram "Anvil" Kask: the smith
    Execution {
        id: "hammer_falls",
        name: "Hammer Falls",
        fighter: 2,
        aerial: false,
        dur: 2.3,
        hit: 0.62,
        mastery: 1,
        segs: &[
            seg(0.0, 0.4, "TreeChopping_Loop", 0.45, 0.9),
            seg(0.4, 0.5, "TreeChopping_Loop", 0.9, 0.97),
            segw(0.5, 0.62, "TreeChopping_Loop", 0.97, CHOP_HIT),
            segw(0.62, 0.8, "TreeChopping_Loop", CHOP_HIT, CHOP_HIT + 0.09),
            segw(0.8, 1.0, "TreeChopping_Loop", CHOP_HIT + 0.09, CHOP_HIT + 0.35),
        ],
        cues: &[(0.2, Embers), (0.52, Whoosh), (0.62, Lethal), (0.62, Slam), (0.64, Embers), (0.68, Ring), (0.8, Crowd)],
        shots: &[
            shot(0.0, Attacker, (-0.8, -0.65), (2.6, 2.3), -0.75, 1.6, 50.0),
            shot(0.46, Victim, (0.3, 0.25), (3.0, 2.8), 2.5, 0.3, 54.0),
            shot(0.6, Mid, (0.8, 0.85), (3.6, 3.8), -0.1, 0.8, 64.0),
            shot(0.8, Mid, (0.85, 1.4), (4.0, 4.8), 1.2, 0.6, 58.0),
        ],
        gun: None,
        pull: None,
        kneel: Some((0.12, 0.62)),
        slow: (0.6, 0.8),
        hidden: None,
        ghosts: Ghosts::None,
    },
    Execution {
        id: "quench",
        name: "Quench",
        fighter: 2,
        aerial: false,
        dur: 2.4,
        hit: 0.7,
        mastery: 3,
        segs: &[
            seg(0.0, 0.14, "Shield_Dash", 0.0, 0.3),
            seg(0.14, 0.34, "Sword_Heavy_Combo", 0.9, 1.17),
            seg(0.34, 0.52, "Sword_Heavy_Combo", 1.5, 1.8),
            seg(0.52, 0.7, "Sword_Heavy_Combo", 2.25, SLAM_HIT),
            seg(0.7, 0.84, "Sword_Heavy_Combo", SLAM_HIT, 2.66),
            seg(0.84, 1.0, "Sword_Heavy_Combo", 2.66, 3.1),
        ],
        cues: &[(0.06, Whoosh), (0.1, Strike), (0.1, Embers), (0.28, Whoosh), (0.34, Strike), (0.46, Whoosh), (0.52, Strike), (0.52, Embers), (0.7, Lethal), (0.7, Slam), (0.75, Ring), (0.86, Crowd)],
        shots: &[
            shot(0.0, Mid, (0.6, 0.4), (3.6, 3.2), 0.2, 1.0, 60.0),
            shot(0.34, Victim, (-1.3, -1.2), (2.0, 1.9), 0.0, 1.5, 46.0),
            shot(0.6, Mid, (0.9, 0.8), (3.0, 2.8), 1.1, 0.9, 54.0),
            shot(0.78, Mid, (0.0, 0.6), (4.0, 4.6), 0.3, 0.8, 62.0),
        ],
        gun: None,
        pull: None,
        kneel: Some((0.36, 0.7)),
        slow: (0.68, 0.84),
        hidden: None,
        ghosts: Ghosts::None,
    },
    Execution {
        id: "foundry_bell",
        name: "Foundry Bell",
        fighter: 2,
        aerial: false,
        dur: 2.5,
        hit: 0.74,
        mastery: 6,
        segs: &[
            seg(0.0, 0.2, "Sword_Regular_C", 0.3, C_HIT),
            seg(0.2, 0.4, "Sword_Regular_C", 0.3, C_HIT),
            seg(0.4, 0.56, "Sword_Heavy_Combo", 0.05, 0.38),
            seg(0.56, 0.74, "Sword_Heavy_Combo", 2.25, SLAM_HIT),
            seg(0.74, 0.88, "Sword_Heavy_Combo", SLAM_HIT, 2.66),
            seg(0.88, 1.0, "Sword_Heavy_Combo", 2.66, 3.1),
        ],
        cues: &[(0.12, Whoosh), (0.2, Strike), (0.32, Whoosh), (0.4, Strike), (0.42, Embers), (0.56, Strike), (0.66, Whoosh), (0.74, Lethal), (0.74, Slam), (0.76, Embers), (0.8, Ring), (0.9, Crowd)],
        shots: &[
            shot(0.0, Mid, (0.2, 1.4), (3.8, 3.8), 0.6, 1.0, 62.0),
            shot(0.42, Attacker, (-0.8, -0.7), (2.0, 1.8), -0.5, 1.5, 46.0),
            shot(0.64, Victim, (0.6, 0.5), (2.8, 2.6), 2.2, 0.3, 54.0),
            shot(0.8, Mid, (1.0, 1.6), (4.2, 5.0), 0.8, 0.7, 60.0),
        ],
        gun: None,
        pull: None,
        kneel: Some((0.42, 0.74)),
        slow: (0.72, 0.88),
        hidden: None,
        ghosts: Ghosts::None,
    },
    Execution {
        id: "anvil_drop",
        name: "Anvil Drop",
        fighter: 2,
        aerial: true,
        dur: 2.0,
        hit: 0.62,
        mastery: 4,
        segs: &[
            seg(0.0, 0.2, "NinjaJump_Start", 0.0, 0.45),
            seg(0.2, 0.44, "Sword_Heavy_Combo", 1.95, 2.25),
            seg(0.44, 0.62, "Sword_Heavy_Combo", 2.25, SLAM_HIT),
            seg(0.62, 0.78, "Sword_Heavy_Combo", SLAM_HIT, 2.64),
            seg(0.78, 1.0, "NinjaJump_Land", 0.0, 0.85),
        ],
        cues: &[(0.08, Flip), (0.3, Embers), (0.52, Whoosh), (0.62, Lethal), (0.64, Embers), (0.68, Ring), (0.8, Slam), (0.86, Crowd)],
        shots: &[
            shot(0.0, Mid, (0.7, 0.9), (3.4, 3.2), -1.0, 1.5, 60.0),
            shot(0.44, Mid, (0.9, 0.8), (3.2, 3.0), 0.6, 1.0, 54.0),
            shot(0.7, Mid, (0.4, 0.9), (4.4, 5.2), 0.1, 0.6, 64.0),
        ],
        gun: None,
        pull: None,
        kneel: None,
        slow: (0.6, 0.78),
        hidden: None,
        ghosts: Ghosts::None,
    },
    // ---------------------------------------------------------------- Ilya Fen: the spark
    Execution {
        id: "lights_out",
        name: "Lights Out",
        fighter: 3,
        aerial: false,
        dur: 2.0,
        hit: 0.7,
        mastery: 1,
        segs: &[
            seg(0.0, 0.08, "Sword_Dash", 0.0, 0.06),
            seg(0.08, 0.56, "Sword_Idle", 0.0, 1.0),
            seg(0.56, 0.7, "Sword_Dash", 0.05, DASH_HIT),
            seg(0.7, 0.86, "Sword_Dash", DASH_HIT, 0.45),
            seg(0.86, 1.0, "Sword_Dash", 0.45, 1.2),
        ],
        cues: &[(0.04, Blur), (0.14, Strike), (0.26, Strike), (0.38, Strike), (0.5, Strike), (0.62, Whoosh), (0.7, Lethal), (0.76, Ring), (0.86, Crowd)],
        shots: &[
            shot(0.0, Mid, (0.5, 0.6), (3.2, 3.0), 0.3, 1.0, 58.0),
            shot(0.1, Victim, (0.0, 3.1), (2.6, 2.6), 0.5, 1.1, 60.0),
            shot(0.54, Attacker, (-0.85, -0.75), (1.8, 1.6), 0.1, 1.35, 44.0),
            shot(0.74, Mid, (1.0, 1.3), (3.6, 4.2), -0.3, 1.0, 60.0),
        ],
        gun: None,
        pull: None,
        kneel: Some((0.3, 0.7)),
        slow: (0.68, 0.86),
        hidden: Some((0.08, 0.56)),
        ghosts: Ghosts::Ring(&[(0.14, 1.6), (0.26, 3.5), (0.38, 5.3), (0.5, 0.5)]),
    },
    Execution {
        id: "rooftop_run",
        name: "Rooftop Run",
        fighter: 3,
        aerial: false,
        dur: 2.2,
        hit: 0.74,
        mastery: 3,
        segs: &[
            seg(0.0, 0.14, "Sword_Regular_A", 0.0, A_HIT),
            seg(0.14, 0.3, "Sword_Regular_B", 0.0, B_HIT),
            seg(0.3, 0.4, "Sword_Regular_B", B_HIT, 0.5),
            seg(0.4, 0.62, "NinjaJump_Start", 0.2, 0.6),
            seg(0.62, 0.74, "Sword_Regular_A", 0.0, A_HIT),
            seg(0.74, 0.88, "Sword_Regular_A", A_HIT, 0.33),
            seg(0.88, 1.0, "Sword_Regular_A", 0.33, 0.43),
        ],
        cues: &[(0.08, Whoosh), (0.14, Strike), (0.24, Whoosh), (0.3, Strike), (0.38, Flip), (0.66, Whoosh), (0.74, Lethal), (0.8, Ring), (0.88, Crowd)],
        shots: &[
            shot(0.0, Mid, (0.3, 0.6), (3.0, 2.9), 0.3, 1.0, 58.0),
            shot(0.34, Mid, (0.0, 0.1), (4.2, 4.4), -0.5, 1.4, 64.0),
            shot(0.62, Victim, (-0.9, -0.8), (1.8, 1.7), 0.1, 1.3, 44.0),
            shot(0.8, Mid, (0.8, 1.3), (3.8, 4.4), 0.9, 1.0, 58.0),
        ],
        gun: None,
        pull: None,
        kneel: Some((0.32, 0.74)),
        slow: (0.72, 0.88),
        hidden: None,
        ghosts: Ghosts::Trail,
    },
    Execution {
        id: "lower_ward_lightning",
        name: "Lower Ward Lightning",
        fighter: 3,
        aerial: false,
        dur: 2.0,
        hit: 0.72,
        mastery: 6,
        segs: &[
            seg(0.0, 0.1, "Sword_Dash", 0.05, DASH_HIT),
            seg(0.1, 0.18, "Sword_Dash", DASH_HIT, 0.5),
            seg(0.18, 0.3, "Sword_Dash", 0.05, DASH_HIT),
            seg(0.3, 0.38, "Sword_Dash", DASH_HIT, 0.5),
            seg(0.38, 0.58, "Sword_Idle", 0.0, 1.0),
            seg(0.58, 0.72, "Sword_Regular_B", 0.0, B_HIT),
            seg(0.72, 0.86, "Sword_Regular_B", B_HIT, 0.36),
            seg(0.86, 1.0, "Sword_Regular_B", 0.36, 0.53),
        ],
        cues: &[(0.02, Blur), (0.1, Strike), (0.22, Blur), (0.3, Strike), (0.48, Blur), (0.64, Whoosh), (0.72, Lethal), (0.78, Ring), (0.86, Crowd)],
        shots: &[
            shot(0.0, Mid, (0.0, 0.05), (4.4, 4.4), 0.4, 1.0, 64.0),
            shot(0.4, Victim, (0.9, 1.3), (2.4, 2.2), 0.3, 1.2, 54.0),
            shot(0.62, Attacker, (-0.8, -0.75), (1.7, 1.6), 0.0, 1.35, 42.0),
            shot(0.78, Mid, (0.6, 1.1), (3.8, 4.4), 1.1, 0.9, 58.0),
        ],
        gun: None,
        pull: None,
        kneel: Some((0.32, 0.72)),
        slow: (0.7, 0.86),
        hidden: None,
        ghosts: Ghosts::Trail,
    },
    Execution {
        id: "skylight",
        name: "Skylight",
        fighter: 3,
        aerial: true,
        dur: 1.7,
        hit: 0.62,
        mastery: 4,
        segs: &[
            seg(0.0, 0.2, "NinjaJump_Start", 0.0, 0.45),
            seg(0.2, 0.62, "Sword_Regular_C", 0.3, C_HIT),
            seg(0.62, 0.78, "Sword_Regular_C", C_HIT, 0.72),
            seg(0.78, 1.0, "NinjaJump_Land", 0.0, 0.8),
        ],
        cues: &[(0.06, Flip), (0.2, Blur), (0.5, Whoosh), (0.62, Lethal), (0.68, Ring), (0.84, Crowd)],
        shots: &[
            shot(0.0, Mid, (0.6, 0.9), (3.4, 3.2), -0.9, 1.5, 60.0),
            shot(0.4, Mid, (0.9, 2.0), (2.8, 2.6), 0.2, 1.3, 56.0),
            shot(0.7, Mid, (0.5, 0.8), (4.4, 5.0), 1.8, 0.5, 60.0),
        ],
        gun: None,
        pull: None,
        kneel: None,
        slow: (0.6, 0.78),
        hidden: None,
        ghosts: Ghosts::Trail,
    },
];

/// An execution by its number + 1 (as fighters carry it; 0 = none).
pub fn execution(kind: u8) -> &'static Execution {
    &EXECUTIONS[(kind.max(1) as usize - 1).min(COUNT - 1)]
}

/// The executions of fighter `body`: its three signature ones and its aerial one (style numbers).
pub fn of_fighter(body: u8) -> [usize; PER_FIGHTER] {
    let b = (body as usize).min(COUNT / PER_FIGHTER - 1);
    [b * 4, b * 4 + 1, b * 4 + 2, b * 4 + 3]
}

/// The executions fighter `body` can choose from (`open`: bits of the four, see `of_fighter`), on
/// a target in the air or not. With the aerial one still locked, a target in the air gets a
/// signature one instead (and the other way round), so the trigger rules stay the same for all.
pub fn choices(body: u8, airborne: bool, open: u8) -> Vec<usize> {
    let mine = of_fighter(body);
    let open = open | 1; // the first is always open
    let fits: Vec<usize> = mine.iter().enumerate().filter(|(k, s)| open & (1 << k) != 0 && EXECUTIONS[**s].aerial == airborne).map(|(_, s)| *s).collect();
    if !fits.is_empty() {
        return fits;
    }
    mine.iter().enumerate().filter(|(k, _)| open & (1 << k) != 0).map(|(_, s)| *s).collect()
}

/// How far into `seg` progress `p` is (0..1).
fn within(p: f32, a: f32, b: f32) -> f32 {
    ((p - a) / (b - a).max(1e-4)).clamp(0.0, 1.0)
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The clip pose to show at progress `p`: (clip, time, wrap), and while a new piece starts, the
/// last piece's end to blend from (clip, time, wrap, its weight).
pub type ClipAt = (&'static str, f32, bool);
pub fn clip_at(style: usize, p: f32) -> (ClipAt, Option<(ClipAt, f32)>) {
    let e = &EXECUTIONS[style.min(COUNT - 1)];
    let k = e.segs.iter().rposition(|s| p >= s.from).unwrap_or(0);
    let s = &e.segs[k];
    let u = within(p, s.from, s.to);
    let now = (s.clip, s.c0 + (s.c1 - s.c0) * u, s.wrap);
    // a short blend from the previous piece when this one starts on another clip or time
    if k > 0 {
        let prev = &e.segs[k - 1];
        let gap = (prev.clip != s.clip) || (prev.c1 - s.c0).abs() > 0.02;
        let blend = 0.07 / e.dur; // seconds of blend, as a share
        if gap && p - s.from < blend {
            let w = 1.0 - smooth((p - s.from) / blend);
            return (now, Some(((prev.clip, prev.c1, prev.wrap), w)));
        }
    }
    (now, None)
}

/// The whole body's motion at progress `p`: (extra turn about the vertical, tip forward (a full
/// turn negative is a backflip), rise in metres).
pub fn body(style: usize, p: f32) -> (f32, f32, f32) {
    let arc = |a: f32, b: f32, h: f32| (within(p, a, b) * PI).sin() * h;
    match style {
        // Warden's Verdict: a full turn into the judgement stroke, leaning into it
        0 => (TAU * smooth(within(p, 0.24, 0.46)), 0.25 * within(p, 0.5, 0.64) * (1.0 - within(p, 0.82, 1.0)), 0.0),
        1 => (0.0, 0.12 * (p * 3.0 * PI).sin().abs() + 0.2 * within(p, 0.6, 0.72) * (1.0 - within(p, 0.86, 1.0)), 0.0),
        // Lantern Watch: low and coiled, then the dash
        2 => (0.0, 0.35 * within(p, 0.0, 0.14) * (1.0 - within(p, 0.36, 0.6)), 0.0),
        // Ridge Fall: up with them, the blade over the head, down
        3 => (0.0, 0.35 * within(p, 0.4, 0.58), arc(0.0, 0.76, 0.4)),
        // The Collection: leaning back hauling the line, then easy and upright
        4 => (0.0, -0.25 * arc(0.08, 0.3, 1.0) + 0.1 * within(p, 0.3, 0.4) * (1.0 - within(p, 0.4, 0.5)), 0.0),
        // Bounty Closed: a low sweep, then standing over them
        5 => (PI * 0.5 * arc(0.0, 0.22, 1.0), 0.15 * arc(0.0, 0.22, 1.0), -0.35 * arc(0.0, 0.22, 1.0)),
        6 => (0.0, 0.1 * arc(0.0, 0.16, 1.0), 0.0),
        7 => (0.0, -0.2 * arc(0.06, 0.3, 1.0), arc(0.0, 0.8, 0.3)),
        // Hammer Falls: leaning back hoisting the blade, then all the weight forward into the floor
        8 => (0.0, -0.18 * within(p, 0.0, 0.5) + 0.5 * within(p, 0.5, 0.62) * (1.0 - within(p, 0.8, 1.0)), 0.0),
        9 => (0.0, 0.3 * arc(0.0, 0.14, 1.0) + 0.4 * within(p, 0.56, 0.7) * (1.0 - within(p, 0.84, 1.0)), 0.0),
        // Foundry Bell: two full wheels of the greatsword, then up and down like a hammer on a bell
        10 => (TAU * 2.0 * smooth(within(p, 0.0, 0.4)), 0.45 * within(p, 0.6, 0.74) * (1.0 - within(p, 0.88, 1.0)), 0.0),
        11 => (0.0, 0.5 * within(p, 0.4, 0.62), arc(0.0, 0.78, 0.6)),
        // Lights Out: crouched to spring
        12 => (0.0, 0.3 * within(p, 0.56, 0.7) * (1.0 - within(p, 0.86, 1.0)), 0.0),
        // Rooftop Run: the flip over them (a full forward turn in the air)
        13 => (0.0, TAU * smooth(within(p, 0.4, 0.62)), arc(0.4, 0.62, 1.7)),
        14 => (0.0, 0.25 * arc(0.0, 0.18, 1.0) + 0.25 * arc(0.18, 0.38, 1.0), 0.0),
        // Skylight: a spinning backflip slash in the air
        15 => (TAU * smooth(within(p, 0.2, 0.62)), -TAU * smooth(within(p, 0.0, 0.3)), arc(0.0, 0.78, 0.5)),
        _ => (0.0, 0.0, 0.0),
    }
}

/// How far along the line to the victim (as it was when the execution began) the attacker has
/// moved at `p`, in metres: dashing through and past, running round, flipping over.
pub fn advance(style: usize, p: f32) -> f32 {
    match style {
        // Lantern Watch: the cut as they meet, then on past them
        2 => 2.4 * smooth(within(p, 0.3, 0.46)),
        6 => 2.2 * smooth(within(p, 0.18, 0.42)),
        13 => 2.2 * smooth(within(p, 0.4, 0.62)),
        // Lower Ward Lightning: through them and back again
        14 => 2.3 * smooth(within(p, 0.0, 0.12)) - 2.3 * smooth(within(p, 0.18, 0.3)),
        _ => 0.0,
    }
}

/// How far the attacker stands from the victim (metres), by blade.
pub fn stand_off(style: usize, blade: crate::game::Weapon) -> f32 {
    use crate::game::Weapon;
    let e = &EXECUTIONS[style.min(COUNT - 1)];
    if e.gun.is_some() {
        return 1.15;
    }
    match blade {
        Weapon::Dagger => 0.85,
        Weapon::Greatsword => 1.3,
        _ => 1.05,
    }
}

/// The killing blow's hand-keyed contact: how strongly (0..1) the sword arm is guided onto the
/// one being executed at `p` (the clips swing where they swing; this puts the blade on them for
/// the blow, whoever is kneeling, standing or in the air), and the height it lands at above their
/// feet (lower when they kneel).
pub fn strike(style: usize, p: f32) -> (f32, f32) {
    let e = &EXECUTIONS[style.min(COUNT - 1)];
    if gun_out(style, e.hit) {
        return (0.0, 0.0);
    }
    let w = if p < e.hit { smooth(within(p, e.hit - 0.07, e.hit - 0.01)) } else { 1.0 - smooth(within(p, e.hit + 0.03, e.hit + 0.1)) };
    (w, 1.25 - 0.5 * kneel(style, p))
}
/// Is the gun in hand at `p`?
pub fn gun_out(style: usize, p: f32) -> bool {
    EXECUTIONS[style.min(COUNT - 1)].gun.is_some_and(|(a, b)| p >= a && p < b)
}

/// Is the attacker only a blur at `p` (not drawn)?
pub fn hidden(style: usize, p: f32) -> bool {
    EXECUTIONS[style.min(COUNT - 1)].hidden.is_some_and(|(a, b)| p >= a && p < b)
}

/// The victim yanked in at `p`: the share of the way to arm's reach (None: not now).
pub fn pull(style: usize, p: f32) -> Option<f32> {
    EXECUTIONS[style.min(COUNT - 1)].pull.filter(|(a, _)| p >= *a).map(|(a, b)| smooth(within(p, a, b)))
}

/// How far the victim is beaten down to their knees at `p` (0..1).
pub fn kneel(style: usize, p: f32) -> f32 {
    match EXECUTIONS[style.min(COUNT - 1)].kneel {
        Some((a, b)) => smooth(within(p, a, a + 0.08)) * (1.0 - smooth(within(p, b, b + 0.06))),
        None => 0.0,
    }
}

/// A hand-keyed pose on top of the clip: the left hand's target (our body space: +x right,
/// +y up, +z forward, metres) and a turn of the head (radians, up) and how strong it is.
pub fn left_hand(style: usize, p: f32) -> Option<(V3, f32)> {
    match style {
        // The Collection: a collector's flourish over the fallen, the free hand raised high
        4 => {
            let w = smooth(within(p, 0.72, 0.82));
            (w > 0.0).then_some((v3(-0.22, 1.98, 0.18), w))
        }
        // Warden's Verdict: the free hand flat on the badge as it glints
        0 => {
            let w = smooth(within(p, 0.44, 0.48)) * (1.0 - smooth(within(p, 0.54, 0.58)));
            (w > 0.0).then_some((v3(-0.12, 1.32, 0.16), w))
        }
        // Hammer Falls and Foundry Bell: both hands on the hilt for the big blow
        8 | 10 => {
            let w = smooth(within(p, 0.3, 0.4)) * (1.0 - smooth(within(p, 0.82, 0.92)));
            (w > 0.0).then_some((v3(0.05, 1.25, 0.42), w))
        }
        _ => None,
    }
}

/// The head tipped up (+) or down (-) at `p` (radians).
pub fn head_tilt(style: usize, p: f32) -> f32 {
    match style {
        4 => -0.35 * smooth(within(p, 0.72, 0.84)), // looking down at the collected
        2 => -0.25 * smooth(within(p, 0.4, 0.55)),  // eyes down after the cut
        8 => 0.3 * within(p, 0.2, 0.5) * (1.0 - within(p, 0.5, 0.56)),
        _ => 0.0,
    }
}

/// Afterimages of the attacker at `p`: (where it stands relative to the attacker's spot as an
/// offset along/across the line (metres), how far it is into a strike (0..1, or None for a
/// copy of the attacker's own pose a moment ago and the delay), its strength (0..1)).
#[derive(Clone, Copy, Debug)]
pub struct Ghost {
    /// Round the victim: angle (radians, 0 = where the attacker stands) at 1.05 m.
    pub angle: Option<f32>,
    /// A copy of the attacker this much earlier (shares).
    pub delay: f32,
    /// How far into its strike (0..1).
    pub strike: f32,
    pub strength: f32,
}

pub fn ghosts(style: usize, p: f32) -> Vec<Ghost> {
    let e = &EXECUTIONS[style.min(COUNT - 1)];
    let mut out = Vec::new();
    match e.ghosts {
        Ghosts::None => {}
        Ghosts::Ring(list) => {
            for &(at, ang) in list {
                // each appears a moment before its strike and fades after
                let pre = 0.07;
                let post = 0.09;
                if p >= at - pre && p < at + post {
                    let s = if p < at { within(p, at - pre, at) * 0.6 } else { 0.6 + 0.4 * within(p, at, at + post) };
                    let strength = smooth(within(p, at - pre, at - pre + 0.02)) * (1.0 - smooth(within(p, at + post * 0.4, at + post)));
                    out.push(Ghost { angle: Some(ang), delay: 0.0, strike: s, strength });
                }
            }
        }
        Ghosts::Trail => {
            // only while moving fast (the flip, the dashes, the spin)
            let moving = match style {
                13 => within(p, 0.36, 0.66) > 0.0 && p < 0.66,
                14 => p < 0.4,
                15 => (0.18..0.66).contains(&p),
                _ => false,
            };
            if moving {
                for k in 1..=3 {
                    let d = 0.025 * k as f32;
                    if p - d > 0.0 {
                        out.push(Ghost { angle: None, delay: d, strike: 0.0, strength: 0.55 - 0.15 * k as f32 });
                    }
                }
            }
        }
    }
    out
}

/// Where the camera is and what it looks at, `p` into execution `style`, for the attacker (or,
/// with `victim_view`, the one being executed): (camera, looked-at point, field of view).
pub fn camera(style: usize, p: f32, attacker: V3, victim: V3, victim_view: bool) -> (V3, V3, f32) {
    let e = &EXECUTIONS[style.min(COUNT - 1)];
    let line = (victim - attacker).flat();
    let line = if line.len() > 0.05 { line.norm() } else { v3(0.0, 0.0, 1.0) };
    let side = v3(line.z, 0.0, -line.x);
    let victim_shots = [
        // the victim: a low angle up at the one doing it, a close-up at the blow, then a pull-out
        shot(0.0, Attacker, (-1.25, -1.05), (1.9, 1.5), -0.55, 1.5, 60.0),
        shot((e.hit - 0.05).max(0.0), Attacker, (-1.45, -1.4), (1.25, 1.15), -0.2, 1.45, 44.0),
        shot((e.hit + 0.08).min(1.0), Mid, (0.4, 0.8), (3.8, 4.4), 1.2, 0.9, 60.0),
    ];
    let shots: &[Shot] = if victim_view { &victim_shots } else { e.shots };
    let k = shots.iter().rposition(|s| p >= s.at).unwrap_or(0);
    let s = &shots[k];
    let end = shots.get(k + 1).map_or(1.0, |n| n.at);
    let u = smooth(within(p, s.at, end));
    let yaw = s.yaw.0 + (s.yaw.1 - s.yaw.0) * u;
    let dist = s.dist.0 + (s.dist.1 - s.dist.0) * u;
    let focus = match s.focus {
        Attacker => attacker,
        Victim => victim,
        Mid => (attacker + victim) * 0.5,
    };
    let look = focus + v3(0.0, s.look_h, 0.0);
    let dir = side * yaw.cos() - line * yaw.sin();
    let pos = look + dir * dist + v3(0.0, s.h, 0.0);
    (pos, look, s.fov)
}

/// The points an execution's camera must show, `p` in: the attacker's head, the victim's head
/// (lower when kneeling) and the weapon (the blade hand, or the gun).
pub fn key_points(style: usize, p: f32, attacker: V3, victim: V3) -> [V3; 3] {
    let (_, _, lift) = body(style, p);
    let line = (victim - attacker).flat();
    let line = if line.len() > 0.05 { line.norm() } else { v3(0.0, 0.0, 1.0) };
    let a_head = attacker + v3(0.0, 1.62 + lift, 0.0);
    let v_head = victim + v3(0.0, 1.62 - 0.6 * kneel(style, p), 0.0);
    let weapon = attacker + line * 0.6 + v3(0.0, 1.2 + lift, 0.0);
    [a_head, v_head, weapon]
}

/// Picture shape the framing check assumes (16:9, the narrowest the game is played at) and the
/// share of the height the execution's letterbox bars leave.
const ASPECT: f32 = 16.0 / 9.0;
const LETTERBOX_LEFT: f32 = 0.76;

/// Does a camera at `pos` looking at `look` (horizontal field of view `fov` degrees) show every
/// point in `pts` inside the picture (within the letterbox, a margin from the edges) with nothing
/// solid between it and the camera (a body-wide line: the middle and either side), and is the
/// camera itself clear of walls and floors?
pub fn frames(world: &crate::world::World, pos: V3, look: V3, fov: f32, pts: &[V3]) -> bool {
    let r = 0.22;
    let cam_box = Aabb::new(pos - v3(r, r, r), pos + v3(r, r, r));
    if world.blocks.iter().any(|b| b.b.overlaps(&cam_box)) {
        return false;
    }
    let f = look - pos;
    if f.len() < 0.2 {
        return false;
    }
    let f = f.norm();
    let rt = v3(f.z, 0.0, -f.x);
    if rt.len() < 1e-3 {
        return false;
    }
    let rt = rt.norm();
    let up = rt.cross(f).norm();
    let tx = (fov.to_radians() * 0.5).tan();
    let ty = tx / ASPECT * LETTERBOX_LEFT;
    for &q in pts {
        let d = q - pos;
        let z = d.dot(f);
        if z < 0.35 {
            return false;
        }
        let (x, y) = (d.dot(rt) / z, d.dot(up) / z);
        if x.abs() > tx * 0.9 || y.abs() > ty * 0.9 {
            return false;
        }
        // seen: a line from the camera to it, and to either side of it, all clear
        let side = rt * 0.12;
        if !(world.line_clear(pos, q) && world.line_clear(pos + side, q + side) && world.line_clear(pos - side, q - side)) {
            return false;
        }
    }
    true
}

/// The camera an execution actually uses: the authored shot (`camera`) when it frames both
/// fighters' heads and the weapon with nothing in the way; otherwise the same shot turned round
/// them or pulled in, and failing those a safe over-the-shoulder shot. `prefer` is the choice made
/// last frame (kept while it still works, so the camera doesn't hop between angles). Returns
/// (camera, looked-at point, field of view, the choice made).
pub fn camera_safe(world: &crate::world::World, style: usize, p: f32, attacker: V3, victim: V3, victim_view: bool, prefer: Option<usize>) -> (V3, V3, f32, usize) {
    let pts = key_points(style, p, attacker, victim);
    let (pos0, look0, fov0) = camera(style, p, attacker, victim, victim_view);
    let off = pos0 - look0;
    let flat = off.flat();
    let base_yaw = flat.x.atan2(flat.z);
    let flat_len = flat.len();
    // the candidates: turned round the point looked at, and pulled in
    const TURNS: [f32; 8] = [0.0, 0.5, -0.5, 1.0, -1.0, 1.6, -1.6, std::f32::consts::PI];
    const PULLS: [f32; 3] = [1.0, 0.78, 0.6];
    let n = TURNS.len() * PULLS.len();
    // the second family: aimed between them, far enough back that both fit, from round about
    // them, low and higher; then from high above (a narrow corridor), then over the shoulder
    let mid = (pts[0] + pts[1] + pts[2]) * (1.0 / 3.0);
    let line = (victim - attacker).flat();
    let line = if line.len() > 0.05 { line.norm() } else { v3(0.0, 0.0, 1.0) };
    let sidev = v3(line.z, 0.0, -line.x);
    let spread = pts.iter().map(|q| (*q - mid).len()).fold(0.0f32, f32::max);
    let fit_fov = 66.0f32;
    let need = (spread + 0.45) / ((fit_fov.to_radians() * 0.5).tan() / ASPECT * LETTERBOX_LEFT * 0.85);
    const AROUND: [f32; 10] = [0.0, std::f32::consts::PI, 0.6, -0.6, 2.54, -2.54, 1.2, -1.2, -1.5708, 1.5708];
    const RISE: [f32; 2] = [0.35, 0.75];
    const BACK: [f32; 2] = [1.0, 1.35];
    let m = AROUND.len() * RISE.len() * BACK.len();
    let cand = |k: usize| -> (V3, V3, f32) {
        if k >= n + m + 4 {
            return over_shoulder(world, style, p, attacker, victim, victim_view, k - n - m - 4);
        }
        if k >= n + m {
            // from high above, a little to one side (a corridor, a tight corner)
            let j = k - n - m;
            let s = if j % 2 == 0 { 1.0 } else { -1.0 };
            let along = if j < 2 { 0.0 } else { -0.9 };
            return (mid + sidev * (0.5 * s) + line * along + v3(0.0, need.max(2.6) * 0.95, 0.0), mid, fit_fov + 6.0);
        }
        if k >= n {
            let j = k - n;
            let a = AROUND[j / (RISE.len() * BACK.len())];
            let rise = RISE[(j / BACK.len()) % RISE.len()];
            let back = BACK[j % BACK.len()];
            let dir = (sidev * a.cos() - line * a.sin()) * (1.0 - rise * rise).sqrt() + v3(0.0, rise, 0.0);
            return (mid + dir * (need * back), mid, fit_fov);
        }
        let (turn, pull) = (TURNS[k / PULLS.len()], PULLS[k % PULLS.len()]);
        let yaw = base_yaw + turn;
        let pos = look0 + v3(yaw.sin() * flat_len * pull, off.y * pull.max(0.8), yaw.cos() * flat_len * pull);
        // pulled in close: widen a little so both still fit
        (pos, look0, fov0 + (1.0 - pull) * 30.0)
    };
    let total = n + m + 8;
    if let Some(k) = prefer.filter(|&k| k < total) {
        let (pos, look, fov) = cand(k);
        if frames(world, pos, look, fov, &pts) {
            return (pos, look, fov, k);
        }
    }
    for k in 0..total {
        let (pos, look, fov) = cand(k);
        if frames(world, pos, look, fov, &pts) {
            return (pos, look, fov, k);
        }
    }
    // nothing frames everything (a cramped corner): the over-the-shoulder shot, kept out of walls
    let (pos, look, fov) = cand(n + m + 4);
    (pos, look, fov, n + m + 4)
}

/// The safe fallback: over the shoulder of the one whose view it is (the attacker's, or the
/// victim's), looking at the other, wide enough for both; `k` picks the right or left shoulder and
/// how far back (pulled in front of any wall behind).
fn over_shoulder(world: &crate::world::World, style: usize, p: f32, attacker: V3, victim: V3, victim_view: bool, k: usize) -> (V3, V3, f32) {
    let (me, other) = if victim_view { (victim, attacker) } else { (attacker, victim) };
    let line = (other - me).flat();
    let line = if line.len() > 0.05 { line.norm() } else { v3(0.0, 0.0, 1.0) };
    let side = v3(line.z, 0.0, -line.x) * if k % 2 == 0 { 1.0 } else { -1.0 };
    let back = if k < 2 { 1.9 } else { 1.2 };
    let (_, _, lift) = body(style, p);
    let head = me + v3(0.0, 1.7 + lift, 0.0);
    let mid = (attacker + victim) * 0.5 + v3(0.0, 1.15, 0.0);
    let out = (-line * back + side * 0.75 + v3(0.0, 0.45, 0.0)).norm();
    let mut dist = (back * back + 0.75 * 0.75 + 0.45 * 0.45).sqrt();
    if let Some((hit, _)) = world.raycast(head, out, dist + 0.3) {
        dist = (hit - 0.3).max(0.35);
    }
    (head + out * dist, mid, 74.0)
}

/// Which shot of the camera is showing (a new number is a cut).
pub fn shot_index(style: usize, p: f32, victim_view: bool) -> usize {
    if victim_view {
        let e = &EXECUTIONS[style.min(COUNT - 1)];
        return [0.0, e.hit - 0.05, e.hit + 0.08].iter().rposition(|&a| p >= a).unwrap_or(0);
    }
    EXECUTIONS[style.min(COUNT - 1)].shots.iter().rposition(|s| p >= s.at).unwrap_or(0)
}

// ---------------------------------------------------------------------------- violence

/// Settings > Game > Violence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Violence {
    /// Stylised blood spray and blood on the floor (nothing is cut off).
    Full,
    /// Sparks and dark particles instead of blood.
    #[default]
    Reduced,
    /// No blood: white flashes where blows land.
    Off,
}

impl Violence {
    pub const ALL: [Violence; 3] = [Violence::Full, Violence::Reduced, Violence::Off];
    pub fn from_u8(v: u8) -> Violence {
        Violence::ALL[(v as usize).min(2)]
    }
    pub fn name(self) -> &'static str {
        match self {
            Violence::Full => "Full",
            Violence::Reduced => "Reduced",
            Violence::Off => "Off",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Violence::Full => "Stylised blood spray and stains (no dismemberment)",
            Violence::Reduced => "Sparks and dark particles instead of blood",
            Violence::Off => "No blood: white flashes where blows land",
        }
    }
}

/// How a wound looks at a violence setting: the main spray's colour, a second darker or softer
/// one, how much it falls, and whether it leaves a stain on the floor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wound {
    pub main: [u8; 3],
    pub second: [u8; 3],
    pub gravity: f32,
    pub stain: bool,
}

pub fn wound(v: Violence) -> Wound {
    match v {
        Violence::Full => Wound { main: [170, 14, 22], second: [105, 8, 14], gravity: 9.0, stain: true },
        Violence::Reduced => Wound { main: [255, 196, 120], second: [38, 34, 38], gravity: 6.0, stain: false },
        Violence::Off => Wound { main: [255, 255, 255], second: [225, 232, 255], gravity: 2.0, stain: false },
    }
}

/// Is this colour blood (for the tests: nothing red may show unless Violence is Full)?
#[cfg(test)]
pub fn is_blood(c: [u8; 3]) -> bool {
    c[0] > 90 && (c[1] as u32) * 3 < c[0] as u32 && (c[2] as u32) * 3 < c[0] as u32
}

// ---------------------------------------------------------------------------- announcer

static LORE: &str = include_str!("../assets/lore.txt");

/// A value from assets/lore.txt: `key` in `[section]` (the first if it repeats).
pub fn lore(section: &str, key: &str) -> Option<&'static str> {
    let mut in_section = false;
    for line in LORE.lines() {
        let t = line.trim();
        if t.starts_with('#') || t.is_empty() {
            continue;
        }
        if t.starts_with('[') && t.ends_with(']') {
            in_section = &t[1..t.len() - 1] == section;
            continue;
        }
        if in_section {
            if let Some((k, v)) = t.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim());
                }
            }
        }
    }
    None
}

/// The announcer's line for an execution (from assets/lore.txt `[executions]`).
pub fn announcer_line(style: usize) -> &'static str {
    lore("executions", EXECUTIONS[style.min(COUNT - 1)].id).unwrap_or("The Court has seen it done.")
}

/// The fighter's own line as they finish someone (lore.txt `finisher` under the fighter).
pub fn fighter_line(body: u8) -> &'static str {
    let key = ["corvin", "sable", "anvil", "ilya"][(body as usize).min(3)];
    lore(&format!("fighter.{key}"), "finisher").unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Words, names and lines from other games' finishing moves are not used.
    const BORROWED: [&str; 18] = [
        "fatal", "brutal", "animal", "babal", "friendship", "finish him", "finish her", "flawless", "toasty", "get over here", "mortal", "kombat", "test your might", "krypt", "outworld", "netherrealm", "spine rip", "kiss of death",
    ];

    #[test]
    fn every_fighter_has_three_signature_executions_and_an_aerial_one() {
        assert_eq!(EXECUTIONS.len(), COUNT);
        for body in 0..4u8 {
            let mine = of_fighter(body);
            for (k, &s) in mine.iter().enumerate() {
                let e = &EXECUTIONS[s];
                assert_eq!(e.fighter, body, "{} belongs to fighter {body}", e.name);
                assert_eq!(e.aerial, k == 3, "{}: the fourth is the aerial one", e.name);
            }
            // one open from the start, the rest through mastery
            let at_start: Vec<_> = mine.iter().filter(|&&s| EXECUTIONS[s].mastery <= 1).collect();
            assert_eq!(at_start.len(), 1, "fighter {body}: one execution at the start");
            assert!(mine.iter().all(|&s| (1..=crate::career::MASTERY_MAX).contains(&EXECUTIONS[s].mastery)));
        }
    }

    #[test]
    fn executions_are_cinematic_lengths_and_well_formed() {
        let mut names = Vec::new();
        for (k, e) in EXECUTIONS.iter().enumerate() {
            assert!((1.5..=2.5).contains(&e.dur), "{}: lasts 1.5 to 2.5 s ({})", e.name, e.dur);
            assert!(e.hit > 0.2 && e.hit < 0.9, "{}: the blow lands in the middle", e.name);
            // the animation covers it all, in order
            assert_eq!(e.segs.first().map(|s| s.from), Some(0.0), "{}", e.name);
            assert_eq!(e.segs.last().map(|s| s.to), Some(1.0), "{}", e.name);
            for w in e.segs.windows(2) {
                assert!((w[0].to - w[1].from).abs() < 1e-5, "{}: pieces meet", e.name);
            }
            // cues in order, one killing blow at the hit, then the ring-out and the crowd
            assert!(e.cues.windows(2).all(|w| w[0].0 <= w[1].0), "{}: cues in order", e.name);
            assert_eq!(e.cues.iter().filter(|c| c.1 == Lethal).count(), 1, "{}", e.name);
            assert!(e.cues.iter().any(|c| c.1 == Lethal && (c.0 - e.hit).abs() < 1e-4), "{}: the killing blow's cue is at the hit", e.name);
            for need in [Whoosh, Ring, Crowd] {
                assert!(e.cues.iter().any(|c| c.1 == need), "{}: has {need:?}", e.name);
            }
            // a camera with cuts (at least three shots), the slow motion round the blow
            assert!(e.shots.len() >= 3 && e.shots[0].at == 0.0, "{}: several shots", e.name);
            assert!(e.shots.windows(2).all(|w| w[0].at < w[1].at));
            assert!(e.slow.0 <= e.hit && e.slow.1 > e.hit, "{}: slow motion at the blow", e.name);
            // the killing piece of animation is slowed down after the blow (the slow motion)
            let speed = |p: f32| {
                let (a, _) = clip_at(k, p);
                let (b, _) = clip_at(k, p + 0.01);
                (b.1 - a.1) / (0.01 * e.dur)
            };
            if e.gun.map_or(true, |(a, b)| !(a..b).contains(&e.hit)) {
                assert!(speed(e.hit + 0.02) < speed(e.hit - 0.03) * 0.8, "{}: slower after the blow ({} vs {})", e.name, speed(e.hit + 0.02), speed(e.hit - 0.03));
            }
            // a distinct name, and nothing borrowed
            assert!(!names.contains(&e.name), "{} twice", e.name);
            names.push(e.name);
            let text = format!("{} {} {}", e.name, e.id, announcer_line(k)).to_lowercase();
            for w in BORROWED {
                assert!(!text.contains(w), "{}: uses '{w}'", e.name);
            }
        }
    }

    #[test]
    fn every_execution_has_an_announcer_line_in_the_lore_file() {
        for e in EXECUTIONS.iter() {
            let line = lore("executions", e.id).unwrap_or_else(|| panic!("lore.txt [executions] {} =", e.id));
            assert!(line.len() > 12, "{}: a real line", e.id);
        }
        for b in 0..4 {
            assert!(!fighter_line(b).is_empty());
        }
    }

    #[test]
    fn the_aerial_one_is_for_targets_in_the_air_and_locked_ones_are_never_chosen() {
        for body in 0..4u8 {
            let mine = of_fighter(body);
            // only the first open: it's used on the ground and in the air
            assert_eq!(choices(body, false, 0b0001), vec![mine[0]]);
            assert_eq!(choices(body, true, 0b0001), vec![mine[0]]);
            // all open: the aerial one in the air, the signature ones on the ground
            assert_eq!(choices(body, true, 0b1111), vec![mine[3]]);
            assert_eq!(choices(body, false, 0b1111), vec![mine[0], mine[1], mine[2]]);
            // the second still locked
            assert!(!choices(body, false, 0b1101).contains(&mine[1]));
        }
    }

    #[test]
    fn the_victim_sees_the_one_doing_it_from_below() {
        let a = v3(0.0, 0.0, 0.0);
        let v = v3(0.0, 0.0, 1.05);
        for k in 0..COUNT {
            let (cam, look, fov) = camera(k, 0.2, a, v, true);
            // the camera is behind the victim, low, looking at the attacker's head
            assert!(cam.z > v.z - 0.2, "{k}: behind the victim ({cam:?})");
            assert!(look.y > cam.y, "{k}: looking up");
            assert!((look - (a + v3(0.0, 1.5, 0.0))).len() < 0.01 && fov > 30.0);
            // and the attacker's camera cuts between shots
            let cuts = (1..100).filter(|&i| shot_index(k, i as f32 / 100.0, false) != shot_index(k, (i - 1) as f32 / 100.0, false)).count();
            assert!(cuts >= 2, "{k}: cuts");
        }
    }

    /// Places to stand for the camera test: open ground, and with walls, corners and pillars
    /// round the two fighters (the attacker at the origin facing +z, the victim ahead of them).
    fn rooms() -> Vec<(&'static str, crate::world::World)> {
        use crate::math::Aabb;
        use crate::world::{Block, Mat};
        let wall = |w: &mut crate::world::World, a: V3, b: V3| w.blocks.push(Block { b: Aabb::new(a, b), mat: Mat::Wall });
        let mut out = vec![("open ground", crate::game::lag_comp_tests::flat_world())];
        let mut w = crate::game::lag_comp_tests::flat_world();
        wall(&mut w, v3(-10.0, 0.0, -1.5), v3(10.0, 6.0, -1.0));
        out.push(("wall behind the attacker", w));
        let mut w = crate::game::lag_comp_tests::flat_world();
        wall(&mut w, v3(1.2, 0.0, -10.0), v3(1.7, 6.0, 10.0));
        out.push(("wall on the right", w));
        let mut w = crate::game::lag_comp_tests::flat_world();
        wall(&mut w, v3(-1.7, 0.0, -10.0), v3(-1.2, 6.0, 10.0));
        out.push(("wall on the left", w));
        let mut w = crate::game::lag_comp_tests::flat_world();
        wall(&mut w, v3(-10.0, 0.0, 2.8), v3(10.0, 6.0, 3.3));
        out.push(("wall behind the victim", w));
        let mut w = crate::game::lag_comp_tests::flat_world();
        wall(&mut w, v3(-10.0, 0.0, -1.5), v3(10.0, 6.0, -1.0));
        wall(&mut w, v3(1.2, 0.0, -10.0), v3(1.7, 6.0, 10.0));
        out.push(("corner", w));
        let mut w = crate::game::lag_comp_tests::flat_world();
        wall(&mut w, v3(-1.6, 0.0, -1.5), v3(1.6, 6.0, -1.0));
        wall(&mut w, v3(1.2, 0.0, -1.5), v3(1.7, 6.0, 3.5));
        wall(&mut w, v3(-1.7, 0.0, -1.5), v3(-1.2, 6.0, 3.5));
        out.push(("narrow corridor", w));
        out
    }

    /// The execution cameras never lose the fight: at each execution's key frames (the wind-up,
    /// the blow, and along the way), from the attacker's and the victim's view, in open ground and
    /// against walls, corners, a corridor and a pillar dropped where the authored camera would be,
    /// both fighters' heads and the weapon are on screen inside the letterbox with nothing solid
    /// in front of them, and the camera isn't inside anything.
    #[test]
    fn the_execution_camera_always_frames_both_fighters() {
        use crate::game::Weapon;
        let mut fails = Vec::new();
        let mut checked = 0;
        for (k, e) in EXECUTIONS.iter().enumerate() {
            let y = if e.aerial { 1.6 } else { 0.0 };
            let stand = stand_off(k, Weapon::Katana);
            let mut moments = vec![(e.hit - 0.2).max(0.02), e.hit + 0.012];
            moments.extend((1..10).map(|i| i as f32 / 10.0));
            for &p in &moments {
                // where the game stands them: the victim ahead, the attacker moving along the line
                let victim = v3(0.0, y, stand);
                let attacker = victim - v3(0.0, 0.0, stand) + v3(0.0, 0.0, advance(k, p));
                let attacker = v3(attacker.x, y, attacker.z);
                let mut places = rooms();
                // a pillar right where the authored shot puts the camera
                let (raw, _, _) = camera(k, p, attacker, victim, false);
                let mut w = crate::game::lag_comp_tests::flat_world();
                w.blocks.push(crate::world::Block { b: crate::math::Aabb::new(v3(raw.x - 0.5, 0.0, raw.z - 0.5), v3(raw.x + 0.5, 6.0, raw.z + 0.5)), mat: crate::world::Mat::Wall });
                places.push(("pillar in the way", w));
                for (room, world) in &places {
                    for victim_view in [false, true] {
                        checked += 1;
                        let (pos, look, fov, _) = camera_safe(world, k, p, attacker, victim, victim_view, None);
                        let pts = key_points(k, p, attacker, victim);
                        if !frames(world, pos, look, fov, &pts) {
                            fails.push(format!("{} at {p:.2}, {room}, {}", e.name, if victim_view { "victim's view" } else { "attacker's view" }));
                        }
                    }
                }
            }
        }
        assert!(fails.len() * 100 <= checked, "{} of {checked} framings lose a fighter:\n{}", fails.len(), fails.join("\n"));
        assert!(fails.is_empty(), "{} framings lose a fighter:\n{}", fails.len(), fails.join("\n"));
    }

    /// The check itself: a fighter behind a wall, off the side of the picture, under the
    /// letterbox or behind the camera is caught; a camera inside a wall is caught.
    #[test]
    fn the_framing_check_catches_lost_fighters() {
        use crate::math::Aabb;
        use crate::world::{Block, Mat};
        let mut w = crate::game::lag_comp_tests::flat_world();
        let pts = [v3(0.0, 1.6, 0.0), v3(0.0, 1.6, 1.0)];
        let cam = v3(3.0, 1.6, 0.5);
        let look = v3(0.0, 1.4, 0.5);
        assert!(frames(&w, cam, look, 60.0, &pts), "in view");
        assert!(!frames(&w, cam, v3(0.0, 1.4, 6.0), 60.0, &pts), "looking away");
        assert!(!frames(&w, cam, look, 60.0, &[v3(0.0, 4.0, 0.5)]), "above the letterbox");
        assert!(!frames(&w, cam, look, 60.0, &[v3(5.0, 1.6, 0.5)]), "behind the camera");
        w.blocks.push(Block { b: Aabb::new(v3(1.4, 0.0, -2.0), v3(1.6, 4.0, 3.0)), mat: Mat::Wall });
        assert!(!frames(&w, cam, look, 60.0, &pts), "behind a wall");
        assert!(!frames(&w, v3(1.5, 1.6, 0.5), look, 60.0, &pts), "camera inside the wall");
    }

    #[test]
    fn violence_settings_change_how_wounds_look() {
        assert_eq!(Violence::default(), Violence::Reduced);
        assert!(is_blood(wound(Violence::Full).main) && wound(Violence::Full).stain);
        for v in [Violence::Reduced, Violence::Off] {
            let w = wound(v);
            assert!(!is_blood(w.main) && !is_blood(w.second) && !w.stain, "{v:?}: no blood");
        }
        assert_eq!(wound(Violence::Off).main, [255, 255, 255], "Off: white flashes");
        for (k, v) in Violence::ALL.iter().enumerate() {
            assert_eq!(Violence::from_u8(k as u8), *v);
        }
    }
}
