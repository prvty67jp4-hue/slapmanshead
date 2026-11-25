//! THE ASHEN VAULT: a co-op raid for 2-4 (players, or a player with 1-3 bots), launched from
//! the Vault Gate in Varrow Centre (the vault lies under the hub: crate::vault). Five rooms in a
//! row, each a boss with its own mechanic; short corridors between them, each with a line of the
//! vault's story.
//!
//! - The Warden of Wax: a giant candle knight. Pools of molten wax spread over the floor; the
//!   pressure plates must be kept lit (someone standing on each) or the hall floods. It hits
//!   whoever has hurt it most (the tank keeps its attention).
//! - The Twin Assessors: two linked judges (Tally throws coins, Seal stamps the floor). Close to
//!   each other they share a shield; once one falls the other must fall within 10 s or the first
//!   rises again at half its health.
//! - The Chained Ledger: a huge book. It chains two of you together (stay close or the chain
//!   burns), and calls ink thralls that crawl to it to heal it.
//! - The Ashblood Regent, Lord Vesper Kael: a duel on the vault's open roof under a red moon,
//!   all reading telegraphs and moving precisely (see `regent`).
//! - The Ashen Magistrate: three phases. A duel (telegraphed specials); the floor breaks into five
//!   islands over fire (wall-run between them; he condemns an island at a time); then he divides
//!   his sentence: SPREAD (stand apart) or STACK (stand together), called by a big sign over him.
//!
//! A full wipe restarts the room, not the raid; each room allows 3 team revives (hold E on a
//! downed ally for 3 s). Health and mechanics scale with the party's size. Hard mode ("Sealed
//! Writ", after a normal clear): more health, harsher mechanics, 2 revives, an extra move per
//! boss. Rewards are cosmetic only (crate::raid::LOOT). The host runs the raid and tells the party
//! (RaidMsg over HubEv::Raid); everyone moves their own fighter.

use crate::game::*;
use crate::math::*;
use crate::vault::{self, ROOMS, VY};

pub const PARTY_MAX: usize = 4;
pub const REVIVE_TIME: f32 = 3.0;
pub const REVIVE_RANGE: f32 = 1.8;
/// Unhurt this long, health comes back at this many points a second (normal only).
pub const REGEN_AFTER: f32 = 4.0;
pub const REGEN_RATE: f32 = 6.0;
pub const TWIN_WINDOW: f32 = 10.0;
pub const TWIN_WINDOW_HARD: f32 = 6.0;
/// How far apart the Assessors must be to be hurt.
pub const TWIN_BOND: f32 = 8.0;
pub const TETHER_LEN: f32 = 6.0;
/// The Regent's moves (RegentState::mv).
pub const MV_JAB: u8 = 0;
pub const MV_RING: u8 = 1;
pub const MV_THRUST: u8 = 2;
pub const MV_ARC: u8 = 3;
pub const MV_CHARGE: u8 = 4;
pub const NO_MOVE: u8 = 255;
/// What the Regent is doing (RegentState::mode).
pub const RM_MOVES: u8 = 0;
pub const RM_WAVES: u8 = 1;
pub const RM_SPIKES: u8 = 2;
pub const RM_JABS: u8 = 3;
pub const RM_STAGGER: u8 = 4;
pub const RM_STEPS: u8 = 5;
pub const RM_SPELL: u8 = 6;
/// A blood wave: half its thickness, half a gap's width.
pub const WAVE_HALF: f32 = 0.45;
pub const GAP_HALF: f32 = 1.2;

/// The Ashblood Regent's state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RegentState {
    pub mode: u8,
    /// A clock for the mode (to the next move, the next wave...).
    pub t: f32,
    /// The marked player (255: none).
    pub mark: u8,
    /// The move winding up (NO_MOVE: none) and the seconds until it lands.
    pub mv: u8,
    pub wind: f32,
    pub last_mv: u8,
    pub chain_left: u8,
    pub chain_ok: bool,
    pub waves_left: u8,
    /// Who the spikes are on (a bit per slot) and the seconds to the cue.
    pub spikes: u16,
    pub spike_t: f32,
    /// Where a charge comes from (0-3: west, east, south, north) and where it starts.
    pub side: u8,
    pub stagger: f32,
    pub vanished: bool,
}

/// A wall of blood crossing the roof: which way (0 +x, 1 -x, 2 +z, 3 -z), where it is along its
/// way, the gaps in it (along the other axis), who it has caught.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wave {
    pub axis: u8,
    pub pos: f32,
    pub gaps: [f32; 2],
    pub ngaps: u8,
    pub hit: u16,
}

impl Wave {
    /// Whether a point is caught by the wave (in the wall, not in a gap).
    pub fn catches(&self, p: V3) -> bool {
        let (along, across) = if self.axis < 2 { (p.x, p.z) } else { (p.z, p.x) };
        (along - self.pos).abs() < WAVE_HALF && !self.gaps.iter().take(self.ngaps as usize).any(|g| (across - g).abs() < GAP_HALF)
    }
}
/// The Magistrate's Divided Sentence: how close counts as together.
pub const SENTENCE_R: f32 = 5.0;
const INTRO: f32 = 3.5;
/// The Blood Ring's safe hug: this close to the Regent it can't reach you.
pub const DONUT_SAFE: f32 = 1.8;
const WIPE_T: f32 = 4.5;

// ============================================================================== the loot

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LootKind {
    /// A wardrobe look (crate::hub::LOOKS id).
    Look(u8),
    /// A dye (crate::gear::DYES index).
    Dye(u8),
    Pet,
    Title,
}

#[derive(Clone, Copy, Debug)]
pub struct LootItem {
    pub kind: LootKind,
    pub name: &'static str,
    pub rare: bool,
}

const fn li(kind: LootKind, name: &'static str, rare: bool) -> LootItem {
    LootItem { kind, name, rare }
}

/// Everything the vault can give (cosmetic only). Their place in this list is their id (saved).
pub const LOOT: [LootItem; 28] = [
    li(LootKind::Look(70), "Vault Warden's Helm", false),
    li(LootKind::Look(71), "Vault Warden's Mantle", false),
    li(LootKind::Look(72), "Vault Warden's Gorget", false),
    li(LootKind::Look(73), "Vault Warden's Cuirass", false),
    li(LootKind::Look(74), "Vault Warden's Greaves", false),
    li(LootKind::Look(75), "Vault Warden's Gauntlets", false),
    li(LootKind::Look(76), "Vault Warden's Sabatons", false),
    li(LootKind::Look(80), "Magistrate's Coif", false),
    li(LootKind::Look(81), "Cape of Verdicts", false),
    li(LootKind::Look(82), "Ember Stole", false),
    li(LootKind::Look(83), "Robe of Sentence", false),
    li(LootKind::Look(90), "Ember-Gilt Katana", false),
    li(LootKind::Look(91), "Ashen Greatsword", false),
    li(LootKind::Look(92), "Ledger Quill Dagger", false),
    li(LootKind::Look(93), "Verdict Revolver", false),
    li(LootKind::Dye(24), "Ash Black dye", false),
    li(LootKind::Dye(25), "Ember Gold dye", false),
    li(LootKind::Pet, "Cinder Wisp (pet)", true),
    li(LootKind::Title, "Wax-Tamer", true),
    li(LootKind::Title, "Twice-Weighed", true),
    li(LootKind::Title, "Ledger-Burner", true),
    li(LootKind::Title, "Moon-Defier", true),
    li(LootKind::Title, "Unsentenced", true),
    li(LootKind::Title, "Vaultbreaker", false),
    li(LootKind::Title, "of the Sealed Writ", false),
    li(LootKind::Look(84), "The Regent's Long Coat", false),
    li(LootKind::Dye(26), "Red Moon dye", false),
    li(LootKind::Look(94), "Ashblood Glaive", false),
];
pub const L_PET: u8 = 17;
pub const L_VAULTBREAKER: u8 = 23;
pub const L_SEALED_WRIT: u8 = 24;

/// Each boss's drop table: the common pieces, the rare ones.
pub const DROPS: [(&[u8], &[u8]); 5] = [
    (&[0, 5, 6, 15], &[18]),
    (&[2, 1, 16, 12, 11], &[19]),
    (&[4, 13, 15], &[20]),
    (&[25, 26, 27, 3], &[21]),
    (&[7, 8, 9, 10, 14], &[17, 22]),
];

/// The loot id of a look (None if the vault doesn't give it).
pub fn loot_of_look(id: u8) -> Option<u8> {
    LOOT.iter().position(|l| l.kind == LootKind::Look(id)).map(|k| k as u8)
}
pub fn loot_of_dye(d: u8) -> Option<u8> {
    LOOT.iter().position(|l| l.kind == LootKind::Dye(d)).map(|k| k as u8)
}

/// What the vault has given this player, and how they've done (kept in the hub save).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RaidEarned {
    /// LOOT ids owned (a bit each).
    pub items: u64,
    /// Cleared normal, cleared hard.
    pub cleared: bool,
    pub cleared_hard: bool,
    /// Best total times (ms) by party size 2, 3, 4 (0: none).
    pub best: [u32; 3],
}

impl RaidEarned {
    pub fn has(&self, loot: u8) -> bool {
        loot < 64 && (self.items >> loot) & 1 == 1
    }
    pub fn give(&mut self, loot: u8) {
        if loot < 64 {
            self.items |= 1 << loot;
        }
    }
    pub fn to_text(&self) -> String {
        format!("{},{},{},{},{},{}", self.items, self.cleared as u8, self.cleared_hard as u8, self.best[0], self.best[1], self.best[2])
    }
    pub fn from_text(t: &str) -> RaidEarned {
        let v: Vec<u64> = t.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        let g = |k: usize| v.get(k).copied().unwrap_or(0);
        RaidEarned { items: g(0), cleared: g(1) == 1, cleared_hard: g(2) == 1, best: [g(3) as u32, g(4) as u32, g(5) as u32] }
    }
}

// ============================================================================== the pieces

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoeKind {
    Warden = 0,
    Tally = 1,
    Seal = 2,
    Ledger = 3,
    Thrall = 4,
    /// (unused: kept so the numbers stay put)
    Rift = 5,
    Regent = 6,
    Magistrate = 7,
}

impl FoeKind {
    pub fn from_u8(v: u8) -> FoeKind {
        [FoeKind::Warden, FoeKind::Tally, FoeKind::Seal, FoeKind::Ledger, FoeKind::Thrall, FoeKind::Rift, FoeKind::Regent, FoeKind::Magistrate][(v as usize).min(7)]
    }
    pub fn name(self) -> &'static str {
        ["The Warden of Wax", "Tally, the Gilded Assessor", "Seal, the Grey Assessor", "The Chained Ledger", "Ink Thrall", "Blood Rift", "Lord Vesper Kael, the Ashblood Regent", "The Ashen Magistrate"][self as usize]
    }
    /// (radius, height) of its hit shape; the Ledger floats (its shape starts 2 m up).
    pub fn size(self) -> (f32, f32) {
        match self {
            FoeKind::Warden => (1.3, 4.4),
            FoeKind::Tally | FoeKind::Seal => (0.85, 3.0),
            FoeKind::Ledger => (1.9, 2.2),
            FoeKind::Thrall => (0.5, 1.2),
            FoeKind::Rift => (0.5, 0.9),
            FoeKind::Regent => (0.6, 2.4),
            FoeKind::Magistrate => (0.95, 3.5),
        }
    }
    pub fn base_hp(self) -> f32 {
        match self {
            FoeKind::Warden => 30000.0,
            FoeKind::Tally | FoeKind::Seal => 12500.0,
            FoeKind::Ledger => 21000.0,
            FoeKind::Thrall => 260.0,
            FoeKind::Rift => 1.0,
            FoeKind::Regent => 26000.0,
            FoeKind::Magistrate => 32000.0,
        }
    }
    pub fn lift(self) -> f32 {
        match self {
            FoeKind::Ledger => 2.0,
            _ => 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Foe {
    pub kind: FoeKind,
    pub pos: V3,
    pub yaw: f32,
    pub hp: f32,
    pub max: f32,
    pub alive: bool,
    /// What it's doing (each kind its own: 0 idle, 1 winding up, 2 striking...), and a clock.
    pub state: u8,
    pub t: f32,
    pub vel: V3,
    /// (clients) where the host last put it, for smoothing.
    pub shown: V3,
}

impl Foe {
    fn new(kind: FoeKind, pos: V3, hp: f32) -> Foe {
        Foe { kind, pos, yaw: 0.0, hp, max: hp, alive: true, state: 0, t: 0.0, vel: V3::ZERO, shown: pos }
    }
    /// Its hit shape: a capsule (bottom, top, radius).
    pub fn capsule(&self) -> (V3, V3, f32) {
        let (r, h) = self.kind.size();
        let b = self.pos + v3(0.0, self.kind.lift() + r.min(h * 0.5), 0.0);
        let t = self.pos + v3(0.0, self.kind.lift() + (h - r).max(r.min(h * 0.5)), 0.0);
        (b, t, r)
    }
    pub fn centre(&self) -> V3 {
        let (a, b, _) = self.capsule();
        (a + b) * 0.5
    }
}

/// What hurt you (the death recap names it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    Cleave = 0,
    Slam,
    WaxPool,
    Flood,
    WickBurst,
    Coins,
    Seal,
    InkBeam,
    Chain,
    PageStorm,
    Blades,
    ShadowCut,
    Gavel,
    Verdict,
    Sweep,
    Fire,
    Condemned,
    Spread,
    Stack,
    Jab,
    BloodRing,
    Thrust,
    Arc,
    Wave,
    Spike,
    AshPool,
    Charge,
    Spell,
}

impl Cause {
    pub fn from_u8(v: u8) -> Cause {
        use Cause::*;
        [Cleave, Slam, WaxPool, Flood, WickBurst, Coins, Seal, InkBeam, Chain, PageStorm, Blades, ShadowCut, Gavel, Verdict, Sweep, Fire, Condemned, Spread, Stack, Jab, BloodRing, Thrust, Arc, Wave, Spike, AshPool, Charge, Spell][(v as usize).min(27)]
    }
    pub fn name(self) -> &'static str {
        [
            "the Warden's cleave",
            "the Warden's slam",
            "molten wax",
            "the flooded hall",
            "the Wick Burst",
            "Tally's coins",
            "Seal's stamp",
            "the ink beam",
            "the burning chain",
            "the page storm",
            "the wall blades",
            "the shadow's cut",
            "the gavel",
            "a Verdict shot",
            "the Sentence sweep",
            "the fire below",
            "a condemned island",
            "the Sentence (spread out!)",
            "the Sentence (stack up!)",
            "the Red Flash Jab (step back)",
            "the Blood Ring (hug him)",
            "the Lance Thrust (sidestep)",
            "the Sweeping Arc (get behind him)",
            "a blood wave (go through a gap, straight)",
            "a giant spike (block on the cue, or hide)",
            "an ash-blood pool",
            "the Regent's charge",
            "the Regent's spell (block it)",
        ][self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeleKind {
    Circle = 0,
    Cone = 1,
    Line = 2,
    Ring = 3,
    /// A blast all round except right up against its centre (hug it).
    Donut = 4,
    /// A sweep all round except a wedge straight behind (`len`: the safe half-angle).
    Arc = 5,
}

/// A telegraphed attack on the floor: it lands when its clock runs out (a ring sweeps out while
/// it runs: jump it).
#[derive(Clone, Copy, Debug)]
pub struct Tele {
    pub kind: TeleKind,
    pub pos: V3,
    /// Facing (yaw: the direction of a cone or a line).
    pub dir: f32,
    /// Radius (a line: half its width).
    pub r: f32,
    /// A line's length; a cone's half angle.
    pub len: f32,
    pub t: f32,
    pub total: f32,
    pub dmg: f32,
    pub cause: Cause,
    /// (a ring) who it has already caught.
    pub hit: u16,
}

impl Tele {
    pub fn covers(&self, p: V3) -> bool {
        let d = (p - self.pos).flat();
        let fwd = v3(self.dir.sin(), 0.0, self.dir.cos());
        match self.kind {
            TeleKind::Circle => d.len() < self.r,
            TeleKind::Cone => d.len() < self.r && (d.len() < 0.4 || fwd.dot(d.norm()) > self.len.cos()),
            TeleKind::Line => {
                let along = d.dot(fwd);
                let side = (d - fwd * along).len();
                along > -0.5 && along < self.len && side < self.r
            }
            TeleKind::Ring => {
                let cur = self.r * (1.0 - (self.t / self.total.max(0.01)).clamp(0.0, 1.0));
                (d.len() - cur).abs() < 0.9
            }
            TeleKind::Donut => d.len() < self.r && d.len() > DONUT_SAFE,
            TeleKind::Arc => d.len() < self.r && !(d.len() > 0.3 && (-fwd).dot(d.norm()) > self.len.cos()),
        }
    }
    /// 0 at the start, 1 as it lands.
    pub fn k(&self) -> f32 {
        1.0 - (self.t / self.total.max(0.01)).clamp(0.0, 1.0)
    }
}

/// Something lying on the floor that hurts: molten wax, a condemned island's fire.
#[derive(Clone, Copy, Debug)]
pub struct Pool {
    pub kind: u8,
    pub pos: V3,
    pub r: f32,
    pub max: f32,
    pub life: f32,
}

/// A thrown thing: a coin, a wall blade, a Verdict bullet.
#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub kind: u8,
    pub pos: V3,
    pub vel: V3,
    pub dmg: f32,
    pub life: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// The party has come in; the boss wakes after a moment.
    Intro = 0,
    Fight = 1,
    /// The boss is beaten: the chest, the door on is open.
    Cleared = 2,
    /// Everyone is down: the room starts again shortly.
    Wipe = 3,
    /// The Magistrate is beaten: the results.
    Done = 4,
}

impl Stage {
    fn from_u8(v: u8) -> Stage {
        [Stage::Intro, Stage::Fight, Stage::Cleared, Stage::Wipe, Stage::Done][(v as usize).min(4)]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Spread = 1,
    Stack = 2,
}

#[derive(Clone, Debug)]
pub struct Member {
    pub slot: u8,
    pub name: String,
    pub bot: bool,
    pub downed: bool,
    /// Revive progress (seconds held), and who is reviving them.
    pub revive: f32,
    pub reviver: u8,
    /// (host) threat on the Warden; damage dealt this run.
    pub threat: f32,
    pub dealt: f32,
    pub connected: bool,
    /// (host) holding E to revive (players tell the host; bots decide).
    pub holding: bool,
    /// (host) seconds since they were last hurt (health comes back after a while unhurt).
    pub calm: f32,
}

/// A chest left where a boss fell: what each player got from it, whether something rare.
#[derive(Clone, Debug)]
pub struct Chest {
    pub pos: V3,
    pub rare: bool,
    pub open: bool,
    pub items: Vec<(u8, u8)>,
}

#[derive(Clone, Debug)]
pub struct Raid {
    pub members: Vec<Member>,
    pub hard: bool,
    pub room: usize,
    pub stage: Stage,
    pub stage_t: f32,
    pub room_time: f32,
    pub total_time: f32,
    pub splits: [f32; 5],
    pub revives_left: u8,
    pub wipes: u32,
    /// The death recap: (slot, what, room).
    pub recap: Vec<(u8, Cause, u8)>,
    pub foes: Vec<Foe>,
    pub teles: Vec<Tele>,
    pub pools: Vec<Pool>,
    pub shots: Vec<Shot>,
    // ---- the mechanics
    /// The Warden: each plate's light (0-1), the flood (0-1).
    pub plates: [f32; 4],
    pub flood: f32,
    /// The Assessors: seconds since each fell (negative: standing).
    pub twin_dead: [f32; 2],
    /// The Ledger's chains: (a, b, seconds left, seconds stretched).
    pub tethers: Vec<(u8, u8, f32, f32)>,
    /// The Ashblood Regent: what he's doing (see `regent`), and the blood waves crossing the roof.
    pub regent: RegentState,
    pub waves: Vec<Wave>,
    /// The Magistrate: phase (1-3), whether the floor is down, the island he's condemning
    /// (index, seconds to go), his sentence (call, seconds to go, the one marked).
    pub phase: u8,
    pub floor_down: bool,
    pub condemned: Option<(u8, f32)>,
    pub call: Option<(Call, f32, u8)>,
    /// Cooldowns (each boss uses its own).
    pub cd: [f32; 8],
    pub rng: u32,
    pub seq: u32,
    /// Bumped whenever the party is moved (into a room, after a wipe): everyone puts their own
    /// fighter at the room's entry.
    pub place: u16,
    pub chest: Option<Chest>,
    /// (this computer) lines to show: (text, seconds left); the loot that came to me (loot id,
    /// rare) since the chest last opened.
    pub notes: Vec<(String, f32)>,
    pub mine: Vec<(u8, bool)>,
    /// (this computer) the last place count acted on, my last E state sent.
    seen_place: u16,
    sent_hold: bool,
    snap_t: f32,
    /// (host) the results once done: total ms; whether the leaderboard took it.
    pub result_ms: u32,
}

/// How much health scales with the party's size (2, 3, 4).
pub fn scale(n: usize) -> f32 {
    [0.55, 0.78, 1.0][n.clamp(2, 4) - 2]
}

impl Raid {
    pub fn new(members: Vec<Member>, hard: bool, seed: u32) -> Raid {
        let mut r = Raid {
            members,
            hard,
            room: 0,
            stage: Stage::Intro,
            stage_t: INTRO,
            room_time: 0.0,
            total_time: 0.0,
            splits: [0.0; 5],
            revives_left: 0,
            wipes: 0,
            recap: Vec::new(),
            foes: Vec::new(),
            teles: Vec::new(),
            pools: Vec::new(),
            shots: Vec::new(),
            plates: [0.0; 4],
            flood: 0.0,
            twin_dead: [-1.0; 2],
            tethers: Vec::new(),
            regent: RegentState::default(),
            waves: Vec::new(),
            phase: 1,
            floor_down: false,
            condemned: None,
            call: None,
            cd: [0.0; 8],
            rng: seed | 1,
            seq: 0,
            place: 1,
            chest: None,
            notes: Vec::new(),
            mine: Vec::new(),
            seen_place: 0,
            sent_hold: false,
            snap_t: 0.0,
            result_ms: 0,
        };
        r.setup_room();
        r
    }
    pub fn size(&self) -> usize {
        self.members.len().clamp(2, 4)
    }
    pub fn hp_mult(&self) -> f32 {
        scale(self.size()) * if self.hard { 1.3 } else { 1.0 }
    }
    pub fn member(&self, slot: usize) -> Option<&Member> {
        self.members.iter().find(|m| m.slot as usize == slot)
    }
    pub fn is_downed(&self, slot: usize) -> bool {
        self.member(slot).is_some_and(|m| m.downed)
    }
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }
    pub fn boss(&self) -> Option<&Foe> {
        self.foes.iter().find(|f| !matches!(f.kind, FoeKind::Thrall | FoeKind::Rift))
    }
    /// Lay out the room's boss and mechanics from the start.
    pub fn setup_room(&mut self) {
        let m = self.hp_mult();
        let r = ROOMS[self.room];
        let c = r.centre();
        self.foes.clear();
        self.teles.clear();
        self.pools.clear();
        self.shots.clear();
        self.tethers.clear();
        self.plates = [0.0; 4];
        self.flood = 0.0;
        self.twin_dead = [-1.0; 2];
        self.regent = RegentState { mv: NO_MOVE, mark: 255, t: 1.5, ..Default::default() };
        self.waves.clear();
        self.phase = 1;
        self.floor_down = false;
        self.condemned = None;
        self.call = None;
        self.cd = [3.0, 6.0, 9.0, 12.0, 15.0, 18.0, 21.0, 24.0];
        self.revives_left = if self.hard { 2 } else { 3 };
        for mb in &mut self.members {
            mb.threat = 0.0;
            mb.downed = false;
            mb.revive = 0.0;
        }
        match self.room {
            0 => self.foes.push(Foe::new(FoeKind::Warden, c + v3(2.0, 0.0, 0.0), FoeKind::Warden.base_hp() * m)),
            1 => {
                for (k, (x, z)) in vault::DAISES.iter().enumerate() {
                    let kind = if k == 0 { FoeKind::Tally } else { FoeKind::Seal };
                    self.foes.push(Foe::new(kind, v3(*x, VY + 0.3, *z), kind.base_hp() * m));
                }
            }
            2 => self.foes.push(Foe::new(FoeKind::Ledger, c, FoeKind::Ledger.base_hp() * m)),
            3 => {
                let mut b = Foe::new(FoeKind::Regent, v3(vault::ROOF_CENTRE.0, VY, vault::ROOF_CENTRE.1 + 4.0), FoeKind::Regent.base_hp() * m);
                b.yaw = std::f32::consts::PI;
                self.foes.push(b);
            }
            _ => {
                let (x, z) = vault::ISLANDS[0];
                self.foes.push(Foe::new(FoeKind::Magistrate, v3(x, VY, z), FoeKind::Magistrate.base_hp() * m));
            }
        }
        for f in &mut self.foes {
            if f.kind != FoeKind::Regent {
                f.yaw = std::f32::consts::PI * 1.5;
            }
        }
    }
    fn note(&mut self, t: impl Into<String>) {
        self.notes.push((t.into(), 4.0));
    }
}

/// The plates in use: one fewer than the party (the tank is busy), at least one.
pub fn plates_needed(n: usize) -> usize {
    (n.saturating_sub(1)).clamp(1, 4)
}

// ============================================================================== into the game

impl Game {
    pub fn in_raid(&self, i: usize) -> bool {
        self.raid.as_ref().is_some_and(|r| r.member(i).is_some())
    }
    /// The local player's raid (if they're in it).
    pub fn my_raid(&self) -> Option<&Raid> {
        self.raid.as_deref().filter(|r| r.member(self.local).is_some())
    }

    /// (host) Start the vault for these fighters (bots fill up to `bots` more, from the
    /// regulars nearest the gate).
    pub fn raid_start(&mut self, mut party: Vec<usize>, bots: usize, hard: bool) {
        if !self.is_authority() || self.raid.is_some() {
            return;
        }
        let (gx, gz, _) = vault::GATE;
        let gate = v3(gx, 0.0, gz);
        let mut spare: Vec<usize> = (0..self.f.len()).filter(|&k| self.f[k].present && self.f[k].is_bot && !party.contains(&k) && self.hub.as_ref().is_some_and(|h| h.pit_of(k).is_none())).collect();
        spare.sort_by(|a, b| (self.f[*a].pos - gate).len().total_cmp(&(self.f[*b].pos - gate).len()));
        for k in spare.into_iter().take(bots) {
            if party.len() < PARTY_MAX {
                party.push(k);
            }
        }
        party.truncate(PARTY_MAX);
        if party.len() < 2 {
            return;
        }
        let members: Vec<Member> = party.iter().map(|&k| Member { slot: k as u8, name: self.f[k].name.clone(), bot: self.f[k].is_bot, downed: false, revive: 0.0, reviver: 255, threat: 0.0, dealt: 0.0, connected: true, holding: false, calm: 0.0 }).collect();
        let seed = (self.time * 977.0) as u32 ^ 0xA5E7;
        let mut r = Raid::new(members, hard, seed);
        r.note(format!("THE ASHEN VAULT{}", if hard { "  ·  SEALED WRIT" } else { "" }));
        self.raid = Some(Box::new(r));
        for k in party {
            let f = &mut self.f[k];
            f.hub.seat = crate::hub::SEAT_NONE;
            f.hub.emote = 0;
            f.hp = MAX_HP;
            f.equip();
        }
        self.raid_place_all();
        raid_send_snap(self);
    }

    /// Put my own fighters at the current room's entry (each a little apart).
    fn raid_place_all(&mut self) {
        let Some(r) = self.raid.as_ref() else { return };
        let room = ROOMS[r.room];
        let slots: Vec<usize> = r.members.iter().map(|m| m.slot as usize).collect();
        for (k, s) in slots.into_iter().enumerate() {
            if s < self.f.len() && self.f[s].owned {
                place(self, s, room, k);
            }
        }
    }

    /// Leave the raid (the local player): back to the gate.
    pub fn raid_leave(&mut self) {
        let me = self.local;
        if self.is_authority() {
            host_leave(self, me);
        } else {
            raid_msg(self, &RaidMsg::Leave);
        }
        to_gate(self, me);
    }

    /// Hold (or let go of) E: revive whoever is down within reach; press it: open the chest.
    pub fn raid_use(&mut self, held: bool, pressed: bool) {
        let me = self.local;
        if !self.in_raid(me) {
            return;
        }
        if self.is_authority() {
            if let Some(r) = self.raid.as_mut() {
                if let Some(m) = r.members.iter_mut().find(|m| m.slot as usize == me) {
                    m.holding = held;
                }
            }
            if pressed {
                host_use(self, me);
            }
        } else {
            let sent = self.raid.as_ref().is_some_and(|r| r.sent_hold);
            if sent != held {
                raid_msg(self, &RaidMsg::Hold { on: held });
                if let Some(r) = self.raid.as_mut() {
                    r.sent_hold = held;
                }
            }
            if pressed {
                raid_msg(self, &RaidMsg::Use);
            }
        }
    }
}

/// Put a fighter at a room's entry (the k-th of the party).
fn place(g: &mut Game, s: usize, room: vault::Room, k: usize) {
    let fwd = v3(room.entry.2.sin(), 0.0, room.entry.2.cos());
    let side = v3(fwd.z, 0.0, -fwd.x);
    let p = room.entry_pos() + side * ((k as f32 - 1.5) * 1.3);
    g.f[s].respawn_at(p, room.entry.2);
    g.f[s].hp = MAX_HP;
}

/// Back to the Vault Gate in the hub.
fn to_gate(g: &mut Game, s: usize) {
    let (gx, gz, _) = vault::GATE;
    if s < g.f.len() {
        g.f[s].respawn_at(v3(gx + (s % 4) as f32 * 0.8 - 1.2, 0.0, gz + 1.5), std::f32::consts::PI);
        g.f[s].hp = MAX_HP;
        g.f[s].hub.emote = 0;
        g.apply_look_weapon(s);
    }
}

// ============================================================================== the network

pub enum RaidMsg {
    /// (player to host) start from the gate: hard mode, how many bots.
    Gate { hard: bool, bots: u8 },
    /// (host) the raid as it stands.
    Snap(Vec<u8>),
    /// (player to host) I hit this foe this hard.
    Hit { foe: u8, dmg: u16 },
    /// (player to host) holding E (revive) or not.
    Hold { on: bool },
    /// (player to host) E pressed: open the chest.
    Use,
    /// (player to host) I fell into the fire.
    Burn,
    /// (player to host) I'm leaving the raid.
    Leave,
    /// (host) loot for a player: (slot, loot id, rare).
    Loot { slot: u8, item: u8, rare: bool },
    /// (host) the raid is over: the leaderboard row (party size, ms).
    End { size: u8, ms: u32, hard: bool },
    /// (host) the raid is over for everyone (nobody left in it).
    Gone,
}

impl RaidMsg {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        match self {
            RaidMsg::Gate { hard, bots } => b.extend_from_slice(&[1, *hard as u8, *bots]),
            RaidMsg::Snap(s) => {
                b.push(2);
                b.extend_from_slice(s);
            }
            RaidMsg::Hit { foe, dmg } => {
                b.extend_from_slice(&[3, *foe]);
                b.extend_from_slice(&dmg.to_le_bytes());
            }
            RaidMsg::Hold { on } => b.extend_from_slice(&[4, *on as u8]),
            RaidMsg::Use => b.push(5),
            RaidMsg::Burn => b.push(6),
            RaidMsg::Leave => b.push(7),
            RaidMsg::Loot { slot, item, rare } => b.extend_from_slice(&[8, *slot, *item, *rare as u8]),
            RaidMsg::End { size, ms, hard } => {
                b.extend_from_slice(&[9, *size, *hard as u8]);
                b.extend_from_slice(&ms.to_le_bytes());
            }
            RaidMsg::Gone => b.push(10),
        }
        b
    }
    pub fn from_bytes(b: &[u8]) -> Option<RaidMsg> {
        let g = |k: usize| b.get(k).copied();
        Some(match g(0)? {
            1 => RaidMsg::Gate { hard: g(1)? == 1, bots: g(2)?.min(3) },
            2 => RaidMsg::Snap(b[1..].to_vec()),
            3 => RaidMsg::Hit { foe: g(1)?, dmg: u16::from_le_bytes([g(2)?, g(3)?]) },
            4 => RaidMsg::Hold { on: g(1)? == 1 },
            5 => RaidMsg::Use,
            6 => RaidMsg::Burn,
            7 => RaidMsg::Leave,
            8 => RaidMsg::Loot { slot: g(1)?, item: g(2)?.min(LOOT.len() as u8 - 1), rare: g(3)? == 1 },
            9 => RaidMsg::End { size: g(1)?.clamp(2, 4), hard: g(2)? == 1, ms: u32::from_le_bytes([g(3)?, g(4)?, g(5)?, g(6)?]) },
            10 => RaidMsg::Gone,
            _ => return None,
        })
    }
}

/// Send a raid message (players to the host; the host to everyone).
pub fn raid_msg(g: &mut Game, m: &RaidMsg) {
    let e = crate::hub::HubEv::Raid(m.to_bytes());
    match g.kind {
        Kind::Client | Kind::Host => g.out_events.push(Ev::Hub(e)),
        _ => {}
    }
}

struct Wr(Vec<u8>);
impl Wr {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i16(&mut self, v: f32, k: f32) {
        self.0.extend_from_slice(&((v * k).round().clamp(-32767.0, 32767.0) as i16).to_le_bytes());
    }
    fn p(&mut self, v: V3) {
        self.i16(v.x, 100.0);
        self.i16(v.y, 100.0);
        self.i16(v.z, 100.0);
    }
    fn q(&mut self, v: f32, k: f32) {
        self.u8((v * k).round().clamp(0.0, 255.0) as u8);
    }
    fn ang(&mut self, a: f32) {
        self.u8(((a.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU) * 256.0) as u8);
    }
}

struct Rd<'a> {
    b: &'a [u8],
    i: usize,
}
impl Rd<'_> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.i)?;
        self.i += 1;
        Some(v)
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes([self.u8()?, self.u8()?]))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes([self.u8()?, self.u8()?, self.u8()?, self.u8()?]))
    }
    fn i16(&mut self, k: f32) -> Option<f32> {
        Some(i16::from_le_bytes([self.u8()?, self.u8()?]) as f32 / k)
    }
    fn p(&mut self) -> Option<V3> {
        Some(v3(self.i16(100.0)?, self.i16(100.0)?, self.i16(100.0)?))
    }
    fn q(&mut self, k: f32) -> Option<f32> {
        Some(self.u8()? as f32 / k)
    }
    fn ang(&mut self) -> Option<f32> {
        Some(self.u8()? as f32 / 256.0 * std::f32::consts::TAU)
    }
}

/// The raid as it stands, packed small (about 8 times a second to the party).
pub fn encode(r: &Raid) -> Vec<u8> {
    let mut w = Wr(Vec::with_capacity(512));
    w.u32(r.seq);
    w.u8(r.room as u8);
    w.u8(r.stage as u8);
    w.u16((r.stage_t.max(0.0) * 10.0) as u16);
    w.u32((r.room_time * 1000.0) as u32);
    w.u32((r.total_time * 1000.0) as u32);
    for s in r.splits {
        w.u32((s * 1000.0) as u32);
    }
    w.u8(r.revives_left);
    w.u16(r.wipes.min(65535) as u16);
    w.u8(r.hard as u8 | (r.floor_down as u8) << 1);
    w.u8(r.phase);
    w.q(r.flood, 255.0);
    for p in r.plates {
        w.q(p, 255.0);
    }
    for d in r.twin_dead {
        w.i16(d, 10.0);
    }
    let rg = &r.regent;
    w.u8(rg.mode);
    w.u8(rg.mark);
    w.u8(rg.mv);
    w.q(rg.wind, 40.0);
    w.u8(rg.vanished as u8 | (rg.chain_ok as u8) << 1);
    w.u8(rg.chain_left);
    w.u16(rg.spikes);
    w.q(rg.spike_t, 40.0);
    w.q(rg.stagger, 20.0);
    w.u8(rg.side);
    w.q(rg.t, 20.0);
    w.u8(rg.waves_left);
    w.u8(r.waves.len().min(6) as u8);
    for wv in r.waves.iter().take(6) {
        w.u8(wv.axis | wv.ngaps << 4);
        w.i16(wv.pos, 100.0);
        w.i16(wv.gaps[0], 100.0);
        w.i16(wv.gaps[1], 100.0);
    }
    match r.condemned {
        Some((i, t)) => {
            w.u8(i);
            w.q(t, 20.0);
        }
        None => {
            w.u8(255);
            w.u8(0);
        }
    }
    match r.call {
        Some((c, t, m)) => {
            w.u8(c as u8);
            w.q(t, 20.0);
            w.u8(m);
        }
        None => {
            w.u8(0);
            w.u8(0);
            w.u8(0);
        }
    }
    w.u16(r.place);
    w.u8(r.members.len() as u8);
    for m in &r.members {
        w.u8(m.slot);
        w.u8(m.bot as u8 | (m.downed as u8) << 1 | (m.connected as u8) << 2);
        w.q(m.revive, 80.0);
    }
    w.u8(r.tethers.len().min(2) as u8);
    for &(a, b, t, s) in r.tethers.iter().take(2) {
        w.u8(a);
        w.u8(b);
        w.q(t, 20.0);
        w.q(s, 20.0);
    }
    w.u8(r.foes.len().min(14) as u8);
    for f in r.foes.iter().take(14) {
        w.u8(f.kind as u8 | (f.alive as u8) << 4);
        w.p(f.pos);
        w.ang(f.yaw);
        w.u16(((f.hp / f.max.max(1.0)).clamp(0.0, 1.0) * 65535.0) as u16);
        w.u8(f.state);
        w.q(f.t.max(0.0), 20.0);
    }
    w.u8(r.teles.len().min(14) as u8);
    for t in r.teles.iter().take(14) {
        w.u8(t.kind as u8 | (t.cause as u8) << 3);
        w.p(t.pos);
        w.ang(t.dir);
        w.q(t.r, 10.0);
        w.q(t.len, 10.0);
        w.q(t.t, 40.0);
        w.q(t.total, 40.0);
    }
    w.u8(r.pools.len().min(14) as u8);
    for p in r.pools.iter().take(14) {
        w.u8(p.kind);
        w.i16(p.pos.x, 100.0);
        w.i16(p.pos.z, 100.0);
        w.i16(p.pos.y, 100.0);
        w.q(p.r, 40.0);
    }
    w.u8(r.shots.len().min(24) as u8);
    for s in r.shots.iter().take(24) {
        w.u8(s.kind);
        w.p(s.pos);
        for c in [s.vel.x, s.vel.y, s.vel.z] {
            w.u8(((c * 4.0).round().clamp(-127.0, 127.0) as i8) as u8);
        }
    }
    match &r.chest {
        Some(c) => {
            w.u8(1 | (c.rare as u8) << 1 | (c.open as u8) << 2);
            w.p(c.pos);
        }
        None => w.u8(0),
    }
    w.0
}

/// Read a snapshot into a raid (a player's copy). None if it doesn't read.
pub fn decode(b: &[u8], old: Option<&Raid>) -> Option<Raid> {
    let mut r = Rd { b, i: 0 };
    let seq = r.u32()?;
    let room = (r.u8()? as usize).min(4);
    let stage = Stage::from_u8(r.u8()?);
    let stage_t = r.u16()? as f32 / 10.0;
    let room_time = r.u32()? as f32 / 1000.0;
    let total_time = r.u32()? as f32 / 1000.0;
    let mut splits = [0.0; 5];
    for s in &mut splits {
        *s = r.u32()? as f32 / 1000.0;
    }
    let revives_left = r.u8()?.min(3);
    let wipes = r.u16()? as u32;
    let flags = r.u8()?;
    let phase = r.u8()?.clamp(1, 3);
    let flood = r.q(255.0)?;
    let mut plates = [0.0; 4];
    for p in &mut plates {
        *p = r.q(255.0)?;
    }
    let twin_dead = [r.i16(10.0)?, r.i16(10.0)?];
    let mode = r.u8()?;
    let mark = r.u8()?;
    let mv = r.u8()?;
    let wind = r.q(40.0)?;
    let rf = r.u8()?;
    let chain_left = r.u8()?;
    let spikes = r.u16()?;
    let spike_t = r.q(40.0)?;
    let stagger = r.q(20.0)?;
    let side = r.u8()?;
    let rt = r.q(20.0)?;
    let waves_left = r.u8()?;
    let regent = RegentState { mode, t: rt, mark, mv, wind, last_mv: NO_MOVE, chain_left, chain_ok: rf & 2 == 2, waves_left, spikes, spike_t, side, stagger, vanished: rf & 1 == 1 };
    let nw = (r.u8()? as usize).min(6);
    let mut waves = Vec::new();
    for _ in 0..nw {
        let a = r.u8()?;
        waves.push(Wave { axis: a & 3, ngaps: (a >> 4).min(2), pos: r.i16(100.0)?, gaps: [r.i16(100.0)?, r.i16(100.0)?], hit: 0 });
    }
    let ci = r.u8()?;
    let ct = r.q(20.0)?;
    let condemned = (ci < 5).then_some((ci, ct));
    let call_k = r.u8()?;
    let call_t = r.q(20.0)?;
    let call_m = r.u8()?;
    let call = match call_k {
        1 => Some((Call::Spread, call_t, call_m)),
        2 => Some((Call::Stack, call_t, call_m)),
        _ => None,
    };
    let place = r.u16()?;
    let n = (r.u8()? as usize).min(PARTY_MAX);
    let mut members = Vec::new();
    for _ in 0..n {
        let slot = r.u8()?.min(crate::game::MAX_PLAYERS as u8 - 1);
        let fl = r.u8()?;
        let revive = r.q(80.0)?;
        let name = old.and_then(|o| o.member(slot as usize)).map(|m| m.name.clone()).unwrap_or_default();
        members.push(Member { slot, name, bot: fl & 1 == 1, downed: fl & 2 == 2, revive, reviver: 255, threat: 0.0, dealt: 0.0, connected: fl & 4 == 4, holding: false, calm: 0.0 });
    }
    let nt = (r.u8()? as usize).min(2);
    let mut tethers = Vec::new();
    for _ in 0..nt {
        tethers.push((r.u8()?, r.u8()?, r.q(20.0)?, r.q(20.0)?));
    }
    let nf = (r.u8()? as usize).min(14);
    let mut foes = Vec::new();
    for k in 0..nf {
        let kb = r.u8()?;
        let kind = FoeKind::from_u8(kb & 15);
        let pos = r.p()?;
        let yaw = r.ang()?;
        let frac = r.u16()? as f32 / 65535.0;
        let state = r.u8()?;
        let t = r.q(20.0)?;
        let max = kind.base_hp();
        let mut f = Foe { kind, pos, yaw, hp: frac * max, max, alive: kb & 16 == 16, state, t, vel: V3::ZERO, shown: pos };
        // smooth from where it was shown
        if let Some(o) = old.and_then(|o| o.foes.get(k)).filter(|o| o.kind == kind) {
            f.shown = o.shown;
            f.vel = (pos - o.pos) * 8.0;
        }
        foes.push(f);
    }
    let ntl = (r.u8()? as usize).min(14);
    let mut teles = Vec::new();
    for _ in 0..ntl {
        let kb = r.u8()?;
        let kind = match kb & 7 {
            0 => TeleKind::Circle,
            1 => TeleKind::Cone,
            2 => TeleKind::Line,
            3 => TeleKind::Ring,
            4 => TeleKind::Donut,
            _ => TeleKind::Arc,
        };
        teles.push(Tele { kind, cause: Cause::from_u8(kb >> 3), pos: r.p()?, dir: r.ang()?, r: r.q(10.0)?, len: r.q(10.0)?, t: r.q(40.0)?, total: r.q(40.0)?, dmg: 0.0, hit: 0 });
    }
    let np = (r.u8()? as usize).min(14);
    let mut pools = Vec::new();
    for _ in 0..np {
        let kind = r.u8()?;
        let x = r.i16(100.0)?;
        let z = r.i16(100.0)?;
        let y = r.i16(100.0)?;
        let rr = r.q(40.0)?;
        pools.push(Pool { kind, pos: v3(x, y, z), r: rr, max: rr, life: 1.0 });
    }
    let ns = (r.u8()? as usize).min(24);
    let mut shots = Vec::new();
    for _ in 0..ns {
        let kind = r.u8()?;
        let pos = r.p()?;
        let vel = v3(r.u8()? as i8 as f32 / 4.0, r.u8()? as i8 as f32 / 4.0, r.u8()? as i8 as f32 / 4.0);
        shots.push(Shot { kind, pos, vel, dmg: 0.0, life: 1.0 });
    }
    let cf = r.u8()?;
    let chest = if cf & 1 == 1 {
        let pos = r.p()?;
        let items = old.and_then(|o| o.chest.as_ref()).map(|c| c.items.clone()).unwrap_or_default();
        Some(Chest { pos, rare: cf & 2 == 2, open: cf & 4 == 4, items })
    } else {
        None
    };
    let (notes, mine, seen_place, sent_hold) = old.map(|o| (o.notes.clone(), o.mine.clone(), o.seen_place, o.sent_hold)).unwrap_or_default();
    let hard = flags & 1 == 1;
    Some(Raid {
        members,
        hard,
        room,
        stage,
        stage_t,
        room_time,
        total_time,
        splits,
        revives_left,
        wipes,
        recap: old.map(|o| o.recap.clone()).unwrap_or_default(),
        foes,
        teles,
        pools,
        shots,
        plates,
        flood,
        twin_dead,
        tethers,
        regent,
        waves,
        phase,
        floor_down: flags & 2 == 2,
        condemned,
        call,
        cd: [0.0; 8],
        rng: 1,
        seq,
        place,
        chest,
        notes,
        mine,
        seen_place,
        sent_hold,
        snap_t: 0.0,
        result_ms: old.map_or(0, |o| o.result_ms),
    })
}

fn raid_send_snap(g: &mut Game) {
    let Some(r) = g.raid.as_mut() else { return };
    r.seq = r.seq.wrapping_add(1);
    let b = encode(r);
    raid_msg(g, &RaidMsg::Snap(b));
}

/// A raid message arrived (`from`: the sender's slot).
pub fn on_msg(g: &mut Game, from: usize, bytes: &[u8]) {
    let Some(m) = RaidMsg::from_bytes(bytes) else { return };
    let auth = g.is_authority();
    match m {
        RaidMsg::Gate { hard, bots } if auth => gate_start(g, from, hard, bots as usize),
        RaidMsg::Snap(b) if !auth => {
            let old = g.raid.take();
            let mut new = decode(&b, old.as_deref());
            // names from the fighters themselves
            if let Some(r) = new.as_mut() {
                for mb in &mut r.members {
                    if mb.name.is_empty() {
                        mb.name = g.f.get(mb.slot as usize).map(|f| f.name.clone()).unwrap_or_default();
                    }
                }
                // a newer recap line when someone went down
                if let Some(o) = old.as_ref() {
                    for mb in &r.members {
                        if mb.downed && !o.is_downed(mb.slot as usize) {
                            r.notes.push((format!("{} is down!", mb.name), 3.0));
                        }
                    }
                    if r.stage != o.stage || r.room != o.room {
                        stage_notes(r, o.stage, o.room);
                    }
                } else {
                    r.note(format!("THE ASHEN VAULT{}", if r.hard { "  ·  SEALED WRIT" } else { "" }));
                }
            }
            g.raid = new.map(Box::new).or(old);
        }
        RaidMsg::Hit { foe, dmg } if auth => host_damage(g, from, foe as usize, dmg as f32),
        RaidMsg::Hold { on } if auth => {
            if let Some(r) = g.raid.as_mut() {
                if let Some(mb) = r.members.iter_mut().find(|m| m.slot as usize == from) {
                    mb.holding = on;
                }
            }
        }
        RaidMsg::Use if auth => host_use(g, from),
        RaidMsg::Burn if auth => hurt(g, from, if g.raid.as_ref().is_some_and(|r| r.hard) { 25.0 } else { 15.0 }, Cause::Fire),
        RaidMsg::Leave if auth => host_leave(g, from),
        RaidMsg::Loot { slot, item, rare } if !auth => got_loot(g, slot as usize, item, rare),
        RaidMsg::End { size, ms, hard } if !auth => finished_here(g, size as usize, ms, hard),
        RaidMsg::Gone if !auth => {
            let me = g.local;
            if g.in_raid(me) {
                to_gate(g, me);
            }
            g.raid = None;
        }
        _ => {}
    }
}

/// (host) Someone pressed E in the gate's circle: everyone standing in it goes in.
pub fn gate_start(g: &mut Game, from: usize, hard: bool, bots: usize) {
    if g.raid.is_some() {
        if let Some(h) = g.hub.as_mut() {
            h.toast("A party is already in the vault");
        }
        return;
    }
    let (gx, gz, gr) = vault::GATE;
    let mut party: Vec<usize> = (0..g.f.len()).filter(|&k| g.f[k].present && !g.f[k].is_bot && (g.f[k].pos - v3(gx, 0.0, gz)).flat().len() < gr && g.f[k].pos.y > -5.0).collect();
    if !party.contains(&from) {
        party.insert(0, from);
    }
    party.truncate(PARTY_MAX);
    let room_for_bots = PARTY_MAX - party.len();
    let want = if party.len() == 1 { bots.max(1) } else { bots };
    g.raid_start(party, want.min(room_for_bots), hard);
}

// ============================================================================== damage

/// (host) A fighter takes a hit from the vault.
pub fn hurt(g: &mut Game, slot: usize, dmg: f32, cause: Cause) {
    let Some(r) = g.raid.as_mut() else { return };
    if r.stage != Stage::Fight {
        return;
    }
    let room = r.room as u8;
    let hard = r.hard;
    let Some(m) = r.members.iter_mut().find(|m| m.slot as usize == slot) else { return };
    if m.downed || slot >= g.f.len() || g.admin.is_god(slot) {
        return;
    }
    m.calm = 0.0;
    let f = &mut g.f[slot];
    f.hp -= dmg * if hard { 1.15 } else { 1.0 };
    f.hurt_t = 0.35;
    if f.hp <= 0.0 {
        f.hp = 1.0;
        m.downed = true;
        m.revive = 0.0;
        let name = m.name.clone();
        r.recap.push((slot as u8, cause, room));
        r.note(format!("{name} is down ({})!", cause.name()));
    }
}

/// The nearest foe along a ray within `t_max` (distance, foe index).
pub fn ray_hit(g: &Game, o: V3, d: V3, t_max: f32) -> Option<(f32, usize)> {
    let r = g.raid.as_ref()?;
    let mut best: Option<(f32, usize)> = None;
    for (k, f) in r.foes.iter().enumerate() {
        if !f.alive {
            continue;
        }
        let (a, b, rad) = f.capsule();
        if let Some(t) = ray_capsule(o, d, a, b, rad) {
            if t < t_max && best.is_none_or(|x| t < x.0) {
                best = Some((t, k));
            }
        }
    }
    best
}

/// A ray against a capsule: the distance along the ray where it enters (None: it misses).
pub fn ray_capsule(o: V3, d: V3, a: V3, b: V3, r: f32) -> Option<f32> {
    // closest approach between the ray and the segment, sampled finely enough for a hit test
    let ab = b - a;
    let mut best: Option<f32> = None;
    let steps = 6;
    for s in 0..=steps {
        let c = a + ab * (s as f32 / steps as f32);
        let oc = o - c;
        let bq = oc.dot(d);
        let cq = oc.dot(oc) - r * r;
        let disc = bq * bq - cq;
        if disc >= 0.0 {
            let t = -bq - disc.sqrt();
            let t = if t < 0.0 { -bq + disc.sqrt() } else { t };
            if t >= 0.0 && best.is_none_or(|x| t < x) {
                best = Some(t);
            }
        }
    }
    best
}

/// A shooter hit a foe (on the host it lands; a player tells the host).
pub fn hit_foe(g: &mut Game, shooter: usize, foe: usize, dmg: f32) {
    if g.is_authority() {
        host_damage(g, shooter, foe, dmg);
    } else {
        raid_msg(g, &RaidMsg::Hit { foe: foe as u8, dmg: dmg.clamp(0.0, 65535.0) as u16 });
        // (a little feedback straight away)
        if let Some(f) = g.raid.as_mut().and_then(|r| r.foes.get_mut(foe)) {
            f.t = f.t.max(0.0);
        }
    }
}

/// Whether a foe can be hurt right now (and why not).
pub fn vulnerable(r: &Raid, k: usize) -> Result<(), &'static str> {
    let Some(f) = r.foes.get(k) else { return Err("") };
    if !f.alive || r.stage != Stage::Fight {
        return Err("");
    }
    match f.kind {
        FoeKind::Tally | FoeKind::Seal => {
            let other = r.foes.iter().find(|o| o.alive && o.kind != f.kind && matches!(o.kind, FoeKind::Tally | FoeKind::Seal));
            if other.is_some_and(|o| (o.pos - f.pos).flat().len() < TWIN_BOND) {
                return Err("BOUND (pull them apart)");
            }
            Ok(())
        }
        FoeKind::Regent if r.regent.vanished => Err("VANISHED"),
        _ => Ok(()),
    }
}

fn host_damage(g: &mut Game, shooter: usize, k: usize, dmg: f32) {
    let Some(r) = g.raid.as_mut() else { return };
    if vulnerable(r, k).is_err() {
        return;
    }
    // (staggered after a perfect jab chain, the Regent takes double)
    let dmg = if r.foes[k].kind == FoeKind::Regent && r.regent.mode == RM_STAGGER { dmg * 2.0 } else { dmg };
    let f = &mut r.foes[k];
    f.hp -= dmg;
    if let Some(m) = r.members.iter_mut().find(|m| m.slot as usize == shooter) {
        m.threat += dmg;
        m.dealt += dmg;
    }
    if f.hp <= 0.0 {
        f.hp = 0.0;
        f.alive = false;
        let kind = f.kind;
        match kind {
            FoeKind::Tally => r.twin_dead[0] = 0.0,
            FoeKind::Seal => r.twin_dead[1] = 0.0,
            _ => {}
        }
    }
}

/// A blade swung by fighter `i`: whatever foe is in front of it and in reach takes it.
pub fn blade(g: &mut Game, i: usize) {
    let Some(r) = g.raid.as_ref() else { return };
    if r.member(i).is_none() {
        return;
    }
    let a = &g.f[i];
    let fwd = a.fwd();
    let reach = a.reach() + 0.4;
    let dmg = 55.0 * a.weapon.melee_mult() * if a.slash_idx >= 2 { 1.4 } else { 1.0 };
    let mut hits = Vec::new();
    for (k, f) in r.foes.iter().enumerate() {
        if !f.alive {
            continue;
        }
        let (rad, h) = f.kind.size();
        let d = f.pos - a.pos;
        let dist = d.flat().len() - rad;
        let dy = a.pos.y - (f.pos.y + f.kind.lift());
        if dist > reach || dy < -1.0 || dy > h + 0.5 {
            continue;
        }
        if dist > 0.3 && fwd.dot(d.flat().norm()) < 0.2 {
            continue;
        }
        hits.push(k);
    }
    for k in hits {
        hit_foe(g, i, k, dmg);
    }
}

// ============================================================================== the tick

/// The raid's tick: the host runs it; everyone keeps their own fighters where they belong.
pub fn tick(g: &mut Game, dt: f32) {
    if g.raid.is_none() {
        return;
    }
    let auth = g.is_authority();
    // fighters that have left (or dropped): a player who's gone is marked; one who comes back
    // under the same name is put back in
    if auth {
        let present: Vec<(usize, bool, String)> = g.f.iter().enumerate().map(|(k, f)| (k, f.present, f.name.clone())).collect();
        if let Some(r) = g.raid.as_mut() {
            for m in &mut r.members {
                let here = present.get(m.slot as usize).is_some_and(|p| p.1 && p.2 == m.name);
                if !here && m.connected {
                    m.connected = false;
                    let name = m.name.clone();
                    r.notes.push((format!("{name} lost the connection: they can come back in"), 5.0));
                } else if !here {
                    // came back in another slot?
                    if let Some((k, _, _)) = present.iter().find(|p| p.1 && p.2 == m.name) {
                        m.slot = *k as u8;
                        m.connected = true;
                        r.place = r.place.wrapping_add(1);
                    }
                } else if !m.connected {
                    m.connected = true;
                    r.place = r.place.wrapping_add(1);
                }
            }
        }
        crate::raid_bots::host_hands(g);
        host_tick(g, dt);
        let send = g.raid.as_mut().map(|r| {
            r.snap_t -= dt;
            if r.snap_t <= 0.0 {
                r.snap_t = 0.125;
                true
            } else {
                false
            }
        });
        if send == Some(true) && matches!(g.kind, Kind::Host) {
            raid_send_snap(g);
        }
    } else if let Some(r) = g.raid.as_mut() {
        // (a player's copy) foes glide to where the host has them; clocks run on
        for f in &mut r.foes {
            f.shown = f.shown + (f.pos - f.shown) * (dt * 10.0).min(1.0);
        }
        for t in &mut r.teles {
            t.t = (t.t - dt).max(0.0);
        }
        for s in &mut r.shots {
            s.pos = s.pos + s.vel * dt;
        }
        r.stage_t = (r.stage_t - dt).max(0.0);
        if r.stage == Stage::Fight {
            r.room_time += dt;
            r.total_time += dt;
        }
    }
    local_tick(g, dt);
}

/// Every computer: its own fighters stay in the part of the vault that's open, are placed when
/// the party is moved, burn in the fire (and climb back out onto an island).
fn local_tick(g: &mut Game, dt: f32) {
    // the Magistrate's floor where the raid says it is (broken in his second phase; whole again
    // after a wipe), on every computer
    let want_down = g.raid.as_ref().is_some_and(|r| r.room == 4 && r.floor_down);
    let n = vault::FLOOR_TILES.len();
    let is_down = g.world.blocks.len() >= n && g.world.blocks[g.world.blocks.len() - n].b.min.y < VY - 50.0;
    if want_down != is_down {
        set_floor(g, want_down);
    }
    let Some(r) = g.raid.as_mut() else { return };
    for (_, life) in &mut r.notes {
        *life -= dt;
    }
    r.notes.retain(|n| n.1 > 0.0);
    let place_now = r.seen_place != r.place;
    r.seen_place = r.place;
    let room = r.room;
    let stage = r.stage;
    let members: Vec<(usize, usize, bool)> = r.members.iter().enumerate().map(|(k, m)| (m.slot as usize, k, m.downed)).collect();
    let mut burned = Vec::new();
    for (s, k, downed) in members {
        if s >= g.f.len() || !g.f[s].owned {
            continue;
        }
        if place_now {
            place(g, s, ROOMS[room], k);
            continue;
        }
        // downed: kneel where you fell
        if downed {
            if g.f[s].hub.emote != crate::hub::Emote::Sit as u8 {
                g.f[s].hub.emote = crate::hub::Emote::Sit as u8;
                g.f[s].hub.emote_t = g.time;
            }
        } else if g.f[s].hub.emote == crate::hub::Emote::Sit as u8 && g.f[s].hub.seat == crate::hub::SEAT_NONE {
            g.f[s].hub.emote = 0;
        }
        let p = g.f[s].pos;
        if !vault::in_vault(p) {
            // (out of the vault somehow, a fall through the world: back to the party's room)
            if stage != Stage::Done {
                place(g, s, ROOMS[room], k);
            }
            continue;
        }
        // the fire under the Magistrate's floor
        if p.y < vault::FIRE_Y && room == 4 {
            let (ix, iz) = nearest_island(p);
            g.f[s].pos = v3(ix, VY + 0.05, iz);
            g.f[s].prev_pos = g.f[s].pos;
            g.f[s].vel = V3::ZERO;
            burned.push(s);
            continue;
        }
        // the open part of the vault
        let mut boxes = vec![(ROOMS[room].x, ROOMS[room].z)];
        if matches!(stage, Stage::Cleared) && room < 4 {
            if let Some((x, z)) = ROOMS[room].out {
                boxes.push((x, z));
                boxes.push((ROOMS[room + 1].x, ROOMS[room + 1].z));
            }
        }
        let inside = boxes.iter().any(|(x, z)| p.x >= x.0 + 0.35 && p.x <= x.1 - 0.35 && p.z >= z.0 + 0.35 && p.z <= z.1 - 0.35);
        if !inside {
            let mut best = (f32::MAX, p);
            for (x, z) in &boxes {
                let q = v3(p.x.clamp(x.0 + 0.4, x.1 - 0.4), p.y, p.z.clamp(z.0 + 0.4, z.1 - 0.4));
                let d = (q - p).flat().len();
                if d < best.0 {
                    best = (d, q);
                }
            }
            g.f[s].pos = best.1;
        }
    }
    for s in burned {
        if g.is_authority() {
            hurt(g, s, if g.raid.as_ref().is_some_and(|r| r.hard) { 25.0 } else { 15.0 }, Cause::Fire);
        } else {
            raid_msg(g, &RaidMsg::Burn);
        }
    }
}

pub fn nearest_island(p: V3) -> (f32, f32) {
    let mut best = vault::ISLANDS[0];
    for &(x, z) in &vault::ISLANDS {
        if (v3(x, 0.0, z) - p).flat().len() < (v3(best.0, 0.0, best.1) - p).flat().len() {
            best = (x, z);
        }
    }
    best
}

/// Positions and states the host's mechanics need: (slot, position, alive and up).
fn party_view(g: &Game) -> Vec<(usize, V3, bool)> {
    let Some(r) = g.raid.as_ref() else { return Vec::new() };
    r.members.iter().filter(|m| (m.slot as usize) < g.f.len()).map(|m| {
        let f = &g.f[m.slot as usize];
        (m.slot as usize, f.pos, !m.downed && m.connected && f.present)
    }).collect()
}

fn host_tick(g: &mut Game, dt: f32) {
    let view = party_view(g);
    let Some(r) = g.raid.as_mut() else { return };
    r.stage_t -= dt;
    match r.stage {
        Stage::Intro => {
            if r.stage_t <= 0.0 {
                r.stage = Stage::Fight;
                let name = ROOMS[r.room].name;
                let boss = r.boss().map(|b| b.kind.name()).unwrap_or("");
                r.note(format!("{boss}  ·  {name}"));
            }
            return;
        }
        Stage::Wipe => {
            if r.stage_t <= 0.0 {
                r.wipes += 1;
                r.setup_room();
                r.stage = Stage::Intro;
                r.stage_t = INTRO;
                r.place = r.place.wrapping_add(1);
                r.note("The room resets. Again.");
                let slots: Vec<usize> = r.members.iter().map(|m| m.slot as usize).collect();
                for s in slots {
                    if s < g.f.len() {
                        g.f[s].hp = MAX_HP;
                    }
                }
            }
            return;
        }
        Stage::Cleared => {
            // anyone stepping into the next room starts it
            let next = r.room + 1;
            if next < 5 && view.iter().any(|v| v.2 && ROOMS[next].contains(v.1, -2.5)) {
                r.room = next;
                r.setup_room();
                r.stage = Stage::Intro;
                r.stage_t = INTRO;
                r.chest = None;
                r.room_time = 0.0;
                r.place = r.place.wrapping_add(1);
            }
            return;
        }
        Stage::Done => {
            if r.stage_t <= 0.0 {
                end_raid(g);
            }
            return;
        }
        Stage::Fight => {}
    }
    r.room_time += dt;
    r.total_time += dt;
    // ---- revives
    let revives_left = r.revives_left;
    let mut revived = Vec::new();
    let holders: Vec<(usize, V3)> = r.members.iter().filter(|m| m.holding && !m.downed).filter_map(|m| view.iter().find(|v| v.0 == m.slot as usize && v.2).map(|v| (v.0, v.1))).collect();
    for m in &mut r.members {
        if !m.downed {
            continue;
        }
        let at = view.iter().find(|v| v.0 == m.slot as usize).map(|v| v.1);
        let who = at.and_then(|p| holders.iter().find(|h| (h.1 - p).len() < REVIVE_RANGE));
        if let (Some(h), true) = (who, revives_left > 0) {
            m.revive += dt;
            m.reviver = h.0 as u8;
            if m.revive >= REVIVE_TIME {
                revived.push(m.slot as usize);
            }
        } else {
            m.revive = (m.revive - dt * 2.0).max(0.0);
            m.reviver = 255;
        }
    }
    for s in &revived {
        if let Some(m) = r.members.iter_mut().find(|m| m.slot as usize == *s) {
            m.downed = false;
            m.revive = 0.0;
        }
        r.revives_left = r.revives_left.saturating_sub(1);
    }
    let names: Vec<String> = revived.iter().filter_map(|s| r.member(*s).map(|m| m.name.clone())).collect();
    for n in names {
        let left = r.revives_left;
        r.note(format!("{n} is back up  ·  {left} revive{} left", if left == 1 { "" } else { "s" }));
    }
    for s in revived {
        if s < g.f.len() {
            g.f[s].hp = 40.0;
        }
    }
    // ---- a wipe?
    let Some(r) = g.raid.as_mut() else { return };
    if r.members.iter().filter(|m| m.connected).all(|m| m.downed) {
        r.stage = Stage::Wipe;
        r.stage_t = WIPE_T;
        r.teles.clear();
        r.shots.clear();
        r.note("THE PARTY FALLS");
        return;
    }
    // health comes back slowly after a while without a hit (not in Sealed Writ)
    let hard = r.hard;
    let mut regen = Vec::new();
    for m in &mut r.members {
        m.threat *= (1.0 - 0.08 * dt).max(0.0);
        m.calm += dt;
        if !hard && !m.downed && m.calm > REGEN_AFTER {
            regen.push(m.slot as usize);
        }
    }
    for s in regen {
        if s < g.f.len() {
            g.f[s].hp = (g.f[s].hp + REGEN_RATE * dt).min(MAX_HP);
        }
    }
    let Some(r) = g.raid.as_mut() else { return };
    // ---- the boss and its mechanics
    match r.room {
        0 => warden(g, dt, &view),
        1 => twins(g, dt, &view),
        2 => ledger(g, dt, &view),
        3 => regent(g, dt, &view),
        _ => magistrate(g, dt, &view),
    }
    hazards(g, dt, &view);
    // ---- beaten?
    let Some(r) = g.raid.as_mut() else { return };
    let beaten = match r.room {
        1 => r.foes.iter().filter(|f| matches!(f.kind, FoeKind::Tally | FoeKind::Seal)).all(|f| !f.alive),
        _ => r.boss().is_some_and(|b| !b.alive),
    };
    if beaten {
        room_cleared(g);
    }
}

/// The party members still up: (slot, position).
fn up(view: &[(usize, V3, bool)]) -> Vec<(usize, V3)> {
    view.iter().filter(|v| v.2).map(|v| (v.0, v.1)).collect()
}

fn nearest(p: V3, ups: &[(usize, V3)]) -> Option<(usize, V3)> {
    ups.iter().copied().min_by(|a, b| (a.1 - p).flat().len().total_cmp(&(b.1 - p).flat().len()))
}

fn face(f: &mut Foe, to: V3, dt: f32, turn: f32) {
    let d = (to - f.pos).flat();
    if d.len() < 0.01 {
        return;
    }
    let want = d.x.atan2(d.z);
    let mut diff = (want - f.yaw).rem_euclid(std::f32::consts::TAU);
    if diff > std::f32::consts::PI {
        diff -= std::f32::consts::TAU;
    }
    f.yaw += diff.clamp(-turn * dt, turn * dt);
}

fn walk(f: &mut Foe, to: V3, speed: f32, keep: f32, dt: f32, room: usize) {
    let d = (to - f.pos).flat();
    let r = ROOMS[room];
    if d.len() > keep {
        f.vel = d.norm() * speed;
        f.pos = f.pos + f.vel * dt;
    } else {
        f.vel = V3::ZERO;
    }
    let (rad, _) = f.kind.size();
    f.pos.x = f.pos.x.clamp(r.x.0 + rad + 0.3, r.x.1 - rad - 0.3);
    f.pos.z = f.pos.z.clamp(r.z.0 + rad + 0.3, r.z.1 - rad - 0.3);
}

fn tele(r: &mut Raid, kind: TeleKind, pos: V3, dir: f32, rad: f32, len: f32, t: f32, dmg: f32, cause: Cause) {
    let t = if r.hard { t * 0.85 } else { t };
    r.teles.push(Tele { kind, pos: v3(pos.x, VY, pos.z), dir, r: rad, len, t, total: t, dmg, cause, hit: 0 });
}

fn yaw_to(a: V3, b: V3) -> f32 {
    let d = (b - a).flat();
    d.x.atan2(d.z)
}

// ---------------------------------------------------------------- the Warden of Wax

fn warden(g: &mut Game, dt: f32, view: &[(usize, V3, bool)]) {
    let ups = up(view);
    let Some(r) = g.raid.as_mut() else { return };
    let hard = r.hard;
    let n = r.members.len();
    // the plates: lit while someone stands on one, fading when nobody does
    let need = plates_needed(n);
    for (k, (x, z)) in vault::PLATES.iter().enumerate() {
        let on = ups.iter().any(|(_, p)| (v3(*x, VY, *z) - *p).flat().len() < vault::PLATE_R);
        let fade = if hard { 1.0 / 4.0 } else { 1.0 / 6.0 };
        r.plates[k] = if on { (r.plates[k] + dt * 1.2).min(1.0) } else { (r.plates[k] - dt * fade).max(0.0) };
    }
    let dark = r.plates.iter().take(need).filter(|l| **l < 0.5).count();
    let rate = if hard { 0.05 } else { 0.033 };
    r.flood = if dark > 0 { r.flood + dt * rate * dark as f32 } else { (r.flood - dt * 0.03).max(0.0) };
    let flooded = r.flood >= 1.0;
    if flooded {
        r.flood = 0.35;
        r.note("THE HALL FLOODS WITH WAX  ·  keep the plates lit!");
    }
    // whom it hates most (the tank keeps its attention by hurting it)
    let tank = r.members.iter().filter(|m| !m.downed && ups.iter().any(|u| u.0 == m.slot as usize)).max_by(|a, b| a.threat.total_cmp(&b.threat)).map(|m| m.slot as usize);
    let room = r.room;
    let Some(b) = r.foes.iter_mut().find(|f| f.kind == FoeKind::Warden && f.alive) else { return };
    let target = tank.and_then(|s| ups.iter().find(|u| u.0 == s).copied()).or_else(|| nearest(b.pos, &ups));
    let mut new_teles = Vec::new();
    let mut new_pools = Vec::new();
    if let Some((_, tp)) = target {
        face(b, tp, dt, 1.6);
        if b.state == 0 {
            walk(b, tp, 2.3, 3.6, dt, room);
        }
        let d = (tp - b.pos).flat().len();
        // the cleave: a wide arc in front of it, after a slow wind-up
        if r.cd[0] <= 0.0 && d < 6.0 && b.state == 0 {
            b.state = 1;
            b.t = 1.3;
            new_teles.push((TeleKind::Cone, b.pos, b.yaw, 6.0, 1.0, 1.3, 38.0, Cause::Cleave));
            r.cd[0] = 3.8;
        }
    }
    // the slam round itself
    if r.cd[1] <= 0.0 {
        new_teles.push((TeleKind::Circle, b.pos, 0.0, 4.6, 0.0, 1.7, 32.0, Cause::Slam));
        b.state = 2;
        b.t = 1.7;
        r.cd[1] = 12.0;
    }
    // molten wax under someone
    if r.cd[2] <= 0.0 && !ups.is_empty() {
        let pick = (r.rng as usize) % ups.len();
        let mut at = v3(ups[pick].1.x, VY, ups[pick].1.z);
        // (never on a plate: it lands beside it, towards the hall's middle)
        let max = if hard { 3.8 } else { 3.2 };
        let c = ROOMS[0].centre();
        for (x, z) in vault::PLATES {
            let pl = v3(x, VY, z);
            if (at - pl).flat().len() < vault::PLATE_R + max {
                at = pl + (v3(c.x, VY, c.z) - pl).flat().norm() * (vault::PLATE_R + max + 0.3);
            }
        }
        new_pools.push(Pool { kind: 0, pos: at, r: 0.6, max, life: 22.0 });
        r.cd[2] = if hard { 4.0 } else { 6.0 };
    }
    // (hard) the Wick Burst: a ring of fire rolling out from it (jump it)
    if hard && r.cd[3] <= 0.0 {
        new_teles.push((TeleKind::Ring, b.pos, 0.0, 16.0, 0.0, 3.2, 26.0, Cause::WickBurst));
        r.cd[3] = 18.0;
    }
    if b.t > 0.0 {
        b.t -= dt;
        if b.t <= 0.0 {
            b.state = 0;
        }
    }
    for c in r.cd.iter_mut() {
        *c -= dt;
    }
    let _ = r.rand();
    for (k, p, d, rad, len, t, dmg, c) in new_teles {
        tele(r, k, p, d, rad, len, t, dmg, c);
    }
    r.pools.extend(new_pools);
    if flooded {
        let slots: Vec<usize> = ups.iter().map(|u| u.0).collect();
        for s in slots {
            hurt(g, s, 60.0, Cause::Flood);
        }
    }
}

// ---------------------------------------------------------------- the Twin Assessors

fn twins(g: &mut Game, dt: f32, view: &[(usize, V3, bool)]) {
    let ups = up(view);
    let Some(r) = g.raid.as_mut() else { return };
    let hard = r.hard;
    let window = if hard { TWIN_WINDOW_HARD } else { TWIN_WINDOW };
    // one fallen and the other standing too long: it rises again
    for k in 0..2 {
        if r.twin_dead[k] >= 0.0 {
            let other_dead = r.twin_dead[1 - k] >= 0.0;
            if !other_dead {
                r.twin_dead[k] += dt;
                if r.twin_dead[k] > window {
                    r.twin_dead[k] = -1.0;
                    let kind = if k == 0 { FoeKind::Tally } else { FoeKind::Seal };
                    if let Some(f) = r.foes.iter_mut().find(|f| f.kind == kind) {
                        f.alive = true;
                        f.hp = f.max * 0.5;
                    }
                    r.note(format!("{} rises again  ·  they must fall within {window:.0} s of each other", kind.name()));
                }
            }
        }
    }
    let positions: Vec<(FoeKind, V3)> = r.foes.iter().filter(|f| f.alive).map(|f| (f.kind, f.pos)).collect();
    let mut new_teles = Vec::new();
    let mut new_shots = Vec::new();
    let room = r.room;
    let (cd0, cd1) = (r.cd[0], r.cd[1]);
    let (mut reset0, mut reset1) = (false, false);
    let rnd = r.rand();
    for f in r.foes.iter_mut().filter(|f| f.alive) {
        let other = positions.iter().find(|p| p.0 != f.kind).map(|p| p.1);
        let Some((_, tp)) = nearest(f.pos, &ups) else { continue };
        face(f, tp, dt, 2.0);
        // drawn to its twin a little: the party has to keep them apart
        let goal = match other {
            Some(o) => tp * 0.8 + o * 0.2,
            None => tp,
        };
        // (each keeps its own distance from whoever it's after: Tally throws from afar)
        let keep = if f.kind == FoeKind::Tally { 7.0 } else { 2.5 };
        if (tp - f.pos).flat().len() > keep {
            walk(f, goal, if f.kind == FoeKind::Seal { 2.2 } else { 1.6 }, 0.5, dt, room);
        } else {
            f.vel = V3::ZERO;
        }
        match f.kind {
            FoeKind::Tally if cd0 <= 0.0 => {
                // a fan of coins
                let base = yaw_to(f.pos, tp);
                for c in -2..=2 {
                    let a = base + c as f32 * 0.16;
                    new_shots.push(Shot { kind: 0, pos: f.pos + v3(0.0, 1.8, 0.0), vel: v3(a.sin(), -0.03, a.cos()) * 15.0, dmg: 9.0, life: 3.0 });
                }
                f.state = 1;
                f.t = 0.5;
                reset0 = true;
            }
            FoeKind::Seal if cd1 <= 0.0 => {
                // stamps under two of you
                for (k, (_, p)) in ups.iter().enumerate().take(2) {
                    new_teles.push((TeleKind::Circle, *p + v3(rnd - 0.5, 0.0, 0.0) * k as f32, 0.0, 2.6, 0.0, 1.8, 34.0, Cause::Seal));
                }
                f.state = 1;
                f.t = 1.0;
                reset1 = true;
            }
            _ => {}
        }
        if f.t > 0.0 {
            f.t -= dt;
            if f.t <= 0.0 {
                f.state = 0;
            }
        }
    }
    if reset0 {
        r.cd[0] = if hard { 2.8 } else { 3.6 };
    }
    if reset1 {
        r.cd[1] = if hard { 4.8 } else { 6.2 };
    }
    // (hard) they swap places
    if hard && r.cd[4] <= 0.0 {
        let alive: Vec<usize> = (0..r.foes.len()).filter(|&k| r.foes[k].alive).collect();
        if alive.len() == 2 {
            let (a, b) = (r.foes[alive[0]].pos, r.foes[alive[1]].pos);
            r.foes[alive[0]].pos = b;
            r.foes[alive[1]].pos = a;
            r.note("The Assessors trade places!");
        }
        r.cd[4] = 25.0;
    }
    for c in r.cd.iter_mut() {
        *c -= dt;
    }
    for (k, p, d, rad, len, t, dmg, c) in new_teles {
        tele(r, k, p, d, rad, len, t, dmg, c);
    }
    r.shots.extend(new_shots);
}

// ---------------------------------------------------------------- the Chained Ledger

fn ledger(g: &mut Game, dt: f32, view: &[(usize, V3, bool)]) {
    let ups = up(view);
    let Some(r) = g.raid.as_mut() else { return };
    let hard = r.hard;
    let n = r.members.len();
    let room = r.room;
    let c = ROOMS[room].centre();
    // the chains: two of you bound (hard, four: two chains); too far apart, it burns, more and more
    let mut chain_hurt = Vec::new();
    for t in &mut r.tethers {
        t.2 -= dt;
        let (pa, pb) = (view.iter().find(|v| v.0 == t.0 as usize), view.iter().find(|v| v.0 == t.1 as usize));
        if let (Some(a), Some(b)) = (pa, pb) {
            if a.2 && b.2 && (a.1 - b.1).flat().len() > TETHER_LEN {
                t.3 += dt;
                let dps = 6.0 + 3.0 * t.3;
                chain_hurt.push((t.0 as usize, dps * dt));
                chain_hurt.push((t.1 as usize, dps * dt));
            } else {
                t.3 = (t.3 - dt).max(0.0);
            }
        }
    }
    r.tethers.retain(|t| t.2 > 0.0);
    if r.cd[3] <= 0.0 && ups.len() >= 2 {
        let mut pool: Vec<usize> = ups.iter().map(|u| u.0).collect();
        let chains = if hard && pool.len() >= 4 { 2 } else { 1 };
        for _ in 0..chains {
            if pool.len() < 2 {
                break;
            }
            let a = pool.remove((r.rand() * pool.len() as f32) as usize % pool.len());
            let b = pool.remove((r.rand() * pool.len() as f32) as usize % pool.len());
            r.tethers.push((a as u8, b as u8, 12.0, 0.0));
            let (na, nb) = (r.member(a).map(|m| m.name.clone()).unwrap_or_default(), r.member(b).map(|m| m.name.clone()).unwrap_or_default());
            r.note(format!("{na} and {nb} are CHAINED  ·  stay within {TETHER_LEN:.0} m"));
        }
        r.cd[3] = if hard { 17.0 } else { 23.0 };
    }
    // the thralls: from the room's ends to the book, healing it if they get there
    if r.cd[4] <= 0.0 {
        let count = n.max(2) + hard as usize;
        let rr = ROOMS[room];
        for k in 0..count {
            let x = if k % 2 == 0 { rr.x.0 + 1.2 } else { rr.x.1 - 1.2 };
            let z = rr.z.0 + 3.0 + (k as f32 * 3.1) % (rr.z.1 - rr.z.0 - 6.0);
            let hp = FoeKind::Thrall.base_hp() * r.hp_mult();
            r.foes.push(Foe::new(FoeKind::Thrall, v3(x, VY, z), hp));
        }
        r.note("Ink thralls crawl from the stacks  ·  stop them reaching the Ledger!");
        r.cd[4] = if hard { 24.0 } else { 28.0 };
    }
    let speed = if hard { 1.3 } else { 1.0 };
    let heal = if hard { 0.05 } else { 0.03 };
    let mut healed = 0.0;
    for f in r.foes.iter_mut().filter(|f| f.alive && f.kind == FoeKind::Thrall) {
        face(f, c, dt, 4.0);
        walk(f, c, speed, 0.0, dt, room);
        if (f.pos - c).flat().len() < 2.6 {
            f.alive = false;
            healed += heal;
        }
    }
    r.foes.retain(|f| f.kind != FoeKind::Thrall || f.alive);
    let mut new_teles = Vec::new();
    if let Some(b) = r.foes.iter_mut().find(|f| f.kind == FoeKind::Ledger && f.alive) {
        if healed > 0.0 {
            b.hp = (b.hp + b.max * healed).min(b.max);
        }
        b.pos = c + v3((g.time * 0.4).sin() * 1.2, 0.0, (g.time * 0.3).cos() * 1.2);
        if let Some((_, tp)) = nearest(b.pos, &ups) {
            face(b, tp, dt, 1.2);
            // the ink beam: a long line at someone
            if r.cd[0] <= 0.0 {
                let pick = ups[(r.rng as usize >> 3) % ups.len()].1;
                new_teles.push((TeleKind::Line, b.pos, yaw_to(b.pos, pick), 1.3, 20.0, 1.5, 36.0, Cause::InkBeam));
                b.state = 1;
                b.t = 1.5;
                r.cd[0] = if hard { 4.0 } else { 5.2 };
            }
        }
        // the page storm round it
        if r.cd[1] <= 0.0 {
            new_teles.push((TeleKind::Circle, b.pos, 0.0, 6.0, 0.0, 2.1, 26.0, Cause::PageStorm));
            r.cd[1] = 14.0;
        }
        if b.t > 0.0 {
            b.t -= dt;
            if b.t <= 0.0 {
                b.state = 0;
            }
        }
    }
    if healed > 0.0 {
        r.note(format!("A thrall reached the Ledger: it heals {:.0}%", healed * 100.0));
    }
    let _ = r.rand();
    for cd in r.cd.iter_mut() {
        *cd -= dt;
    }
    for (k, p, d, rad, len, t, dmg, c) in new_teles {
        tele(r, k, p, d, rad, len, t, dmg, c);
    }
    for (s, d) in chain_hurt {
        hurt(g, s, d, Cause::Chain);
    }
}

// ---------------------------------------------------------------- the Ashblood Regent

/// Lord Vesper Kael, the Ashblood Regent, on the vault's roof. Every move is aimed at one marked
/// player (the others get free hits) and each reads differently:
///
/// Phase 1 (100-70%): RED FLASH JAB (he flashes red with a pop: step back out of reach), BLOOD
/// RING (he draws red energy in, then a ring blasts out: hug him), LANCE THRUST (a line in front
/// of him and behind: sidestep), SWEEPING ARC (the spear curls all round: get behind him). A move
/// that lands on its mark feeds him (he heals a little: everyone pays).
///
/// Phase 2 (at 70%): he vanishes and blood waves cross the roof from its edges, each with gaps
/// (go through a gap moving straight). Then FOUR GIANT SPIKES, one on each of you: the only
/// one-shot; block on the cue (sword guard raised just in time) or stand behind a chimney. Then a
/// fast JAB CHAIN on one player: dodge or parry every jab and he's staggered (double damage for
/// everyone, 8 s); miss one and it's the waves again. Below 40% he moves on.
///
/// Phase 3 (below 40%): lightning strikes the roof and leaves ash-blood pools (they drain you
/// and switch off your guard and abilities: stay out); between his moves he leaves the roof and
/// charges back across it from one side. His footsteps are the cue: when they stop, get out of
/// his path, then block the spell he throws after it. Final 10%: everything faster, all mixed.
fn regent(g: &mut Game, dt: f32, view: &[(usize, V3, bool)]) {
    let ups = up(view);
    let guard: Vec<(usize, bool, f32)> = ups.iter().map(|(s, _)| (*s, g.f[*s].guard, g.f[*s].guard_t)).collect();
    let mut hits: Vec<(usize, f32, Cause)> = Vec::new();
    let mut cues: Vec<(crate::game::Sfx, V3)> = Vec::new();
    {
        let world = &g.world;
        let Some(r) = g.raid.as_mut() else { return };
        let hard = r.hard;
        let Some(bk) = r.foes.iter().position(|f| f.kind == FoeKind::Regent && f.alive) else { return };
        let frac = r.foes[bk].hp / r.foes[bk].max.max(1.0);
        let fin = frac < 0.10;
        let pace = if hard { 0.78 } else { 1.0 } * if fin { 0.75 } else { 1.0 };
        let roof = ROOMS[3];
        let centre = v3(vault::ROOF_CENTRE.0, VY, vault::ROOF_CENTRE.1);
        let guarded = |s: usize, window: f32| guard.iter().any(|g| g.0 == s && g.1 && g.2 <= window);
        let pos_of = |s: u8| ups.iter().find(|u| u.0 == s as usize).map(|u| u.1);
        // the next mark: round the party, one at a time
        let next_mark = |cur: u8| -> u8 {
            if ups.is_empty() {
                return 255;
            }
            let k = ups.iter().position(|u| u.0 == cur as usize).map_or(0, |k| (k + 1) % ups.len());
            ups[k].0 as u8
        };
        // phase 1 to 2 at 70% (between moves)
        if r.phase == 1 && frac <= 0.70 && r.regent.mv == NO_MOVE {
            r.phase = 2;
            r.regent.mode = RM_WAVES;
            r.regent.vanished = true;
            r.regent.waves_left = 4 + 2 * hard as u8;
            r.regent.t = 1.2;
            r.note("He vanishes  ·  blood rifts open at the edges: go through the gaps, moving straight");
        }
        let rg = &mut r.regent;
        rg.stagger = (rg.stagger - dt).max(0.0);
        match rg.mode {
            RM_WAVES => {
                rg.t -= dt;
                if rg.waves_left > 0 && rg.t <= 0.0 {
                    let rnd = (r.rng >> 4) as f32 / (1u32 << 28) as f32;
                    r.rng ^= r.rng << 13;
                    r.rng ^= r.rng >> 17;
                    r.rng ^= r.rng << 5;
                    let axes: Vec<u8> = if hard && r.regent.waves_left % 3 == 0 { vec![(rnd * 2.0) as u8 % 2, 2 + (rnd * 3.0) as u8 % 2] } else { vec![(rnd * 4.0) as u8 % 4] };
                    for (k, axis) in axes.into_iter().enumerate() {
                        let (lo, hi) = if axis < 2 { roof.z } else { roof.x };
                        let span = hi - lo - 3.0;
                        let g1 = lo + 1.5 + ((rnd * 7.3 + k as f32 * 0.41).fract()) * span;
                        let g2 = lo + 1.5 + ((rnd * 3.1 + 0.5 + k as f32 * 0.29).fract()) * span;
                        let two = (rnd * 5.0).fract() > 0.45 && (g1 - g2).abs() > 3.0;
                        // (from a rift just beyond the parapet: a moment to see it coming)
                        let start = match axis {
                            0 => roof.x.0 - 3.0,
                            1 => roof.x.1 + 3.0,
                            2 => roof.z.0 - 3.0,
                            _ => roof.z.1 + 3.0,
                        };
                        r.waves.push(Wave { axis, pos: start, gaps: [g1, g2], ngaps: if two { 2 } else { 1 }, hit: 0 });
                    }
                    r.regent.waves_left -= 1;
                    r.regent.t = 2.0 * pace;
                    cues.push((crate::game::Sfx::Boom, centre));
                }
                if r.regent.waves_left == 0 && r.waves.is_empty() {
                    // he comes back: four giant spikes
                    let mut mask = 0u16;
                    for (s, _) in ups.iter().take(4) {
                        mask |= 1 << s;
                    }
                    let b = &mut r.foes[bk];
                    b.pos = centre;
                    b.state = 5;
                    let rg = &mut r.regent;
                    rg.vanished = false;
                    rg.mode = RM_SPIKES;
                    rg.spikes = mask;
                    rg.spike_t = 3.2 * pace;
                    cues.push((crate::game::Sfx::ExecRing, centre));
                    r.note("FOUR GIANT SPIKES  ·  block on the cue (blade out, guard up) or get behind a chimney");
                }
            }
            RM_SPIKES => {
                rg.spike_t -= dt;
                if rg.spike_t <= 0.0 {
                    let from = r.foes[bk].pos + v3(0.0, 2.2, 0.0);
                    for (s, p) in &ups {
                        if r.regent.spikes & (1 << s) == 0 {
                            continue;
                        }
                        let blocked = guarded(*s, 0.9);
                        let cover = !world.line_clear(from, *p + v3(0.0, 1.2, 0.0));
                        if !(blocked || cover) {
                            hits.push((*s, 999.0, Cause::Spike));
                        }
                    }
                    cues.push((crate::game::Sfx::Boom, centre));
                    let rg = &mut r.regent;
                    rg.spikes = 0;
                    rg.mode = RM_JABS;
                    rg.chain_left = 5;
                    rg.chain_ok = true;
                    rg.mark = next_mark(rg.mark);
                    rg.mv = NO_MOVE;
                    rg.t = 0.6;
                    let who = r.member(r.regent.mark as usize).map(|m| m.name.clone()).unwrap_or_default();
                    r.note(format!("JAB CHAIN on {who}  ·  dodge or parry all five"));
                }
            }
            RM_JABS => {
                if rg.mv == NO_MOVE {
                    rg.t -= dt;
                    if rg.t <= 0.0 && rg.chain_left > 0 {
                        if let Some(mp) = pos_of(rg.mark) {
                            let b = &mut r.foes[bk];
                            b.yaw = yaw_to(b.pos, mp);
                            b.state = 1;
                            let w = 0.6 * pace;
                            let (pos, yaw) = (b.pos, b.yaw);
                            r.regent.mv = MV_JAB;
                            r.regent.wind = w;
                            r.teles.push(Tele { kind: TeleKind::Cone, pos, dir: yaw, r: 3.6, len: 0.5, t: w, total: w, dmg: 0.0, cause: Cause::Jab, hit: 0 });
                            cues.push((crate::game::Sfx::Hit, pos));
                        } else {
                            r.regent.chain_left = 0;
                        }
                    }
                    if r.regent.chain_left == 0 && r.regent.mv == NO_MOVE {
                        if r.regent.chain_ok {
                            r.regent.mode = RM_STAGGER;
                            r.regent.stagger = 8.0;
                            r.regent.t = 8.0;
                            r.foes[bk].state = 6;
                            r.note("STAGGERED  ·  everyone in: double damage!");
                        } else {
                            r.regent.mode = RM_WAVES;
                            r.regent.vanished = true;
                            r.regent.waves_left = 4 + 2 * hard as u8;
                            r.regent.t = 1.0;
                            r.note("He shrugs it off  ·  the waves return");
                        }
                    }
                } else {
                    rg.wind -= dt;
                    if let Some(b) = r.foes.get_mut(bk) {
                        b.t = rg.wind;
                    }
                    if rg.wind <= 0.0 {
                        let mark = rg.mark as usize;
                        if let Some(mp) = pos_of(rg.mark) {
                            let b = &r.foes[bk];
                            let t = Tele { kind: TeleKind::Cone, pos: b.pos, dir: b.yaw, r: 3.6, len: 0.5, t: 0.0, total: 1.0, dmg: 0.0, cause: Cause::Jab, hit: 0 };
                            if t.covers(mp) && !guarded(mark, 0.45) {
                                hits.push((mark, 24.0, Cause::Jab));
                                r.regent.chain_ok = false;
                            }
                        }
                        r.regent.chain_left = r.regent.chain_left.saturating_sub(1);
                        r.regent.mv = NO_MOVE;
                        r.regent.t = 0.28 * pace;
                        r.foes[bk].state = 0;
                    }
                }
            }
            RM_STAGGER => {
                rg.t -= dt;
                if rg.t <= 0.0 {
                    r.foes[bk].state = 0;
                    if frac <= 0.40 {
                        r.phase = 3;
                        r.regent.mode = RM_MOVES;
                        r.regent.t = 1.0;
                        r.cd[2] = 1.0;
                        r.cd[3] = 9.0;
                        r.note("Lightning splits the red sky  ·  stay out of the ash-blood pools");
                    } else {
                        r.regent.mode = RM_WAVES;
                        r.regent.vanished = true;
                        r.regent.waves_left = 4 + 2 * hard as u8;
                        r.regent.t = 1.0;
                    }
                }
            }
            RM_STEPS => {
                // off the roof: his footsteps come closer from one side, then stop
                rg.t -= dt;
                let (x, z) = side_point(rg.side, roof, 0.5);
                if ((rg.t * 3.0).fract() < dt * 3.0) && rg.t > 0.0 {
                    cues.push((crate::game::Sfx::StepStone, v3(x, VY, z)));
                }
                if rg.t <= 0.0 {
                    if let Some(mp) = pos_of(rg.mark) {
                        let from = v3(x, VY, z);
                        let yaw = yaw_to(from, mp);
                        let w = 0.55 * pace;
                        rg.mv = MV_CHARGE;
                        rg.wind = w;
                        rg.mode = RM_MOVES;
                        let b = &mut r.foes[bk];
                        b.pos = from;
                        b.yaw = yaw;
                        b.state = 4;
                        r.teles.push(Tele { kind: TeleKind::Line, pos: from, dir: yaw, r: 1.3, len: 24.0, t: w, total: w, dmg: 0.0, cause: Cause::Charge, hit: 0 });
                    } else {
                        rg.mode = RM_MOVES;
                        rg.vanished = false;
                    }
                }
            }
            RM_SPELL => {
                rg.t -= dt;
                if rg.t <= 0.0 {
                    if let Some(mp) = pos_of(rg.mark) {
                        let b = &r.foes[bk];
                        let from = b.pos + v3(0.0, 1.6, 0.0);
                        let dir = (mp + v3(0.0, 1.2, 0.0) - from).norm();
                        r.shots.push(Shot { kind: 3, pos: from, vel: dir * 13.0, dmg: 30.0, life: 4.0 });
                        cues.push((crate::game::Sfx::Vanish, from));
                    }
                    r.regent.mode = RM_MOVES;
                    r.regent.t = 1.2 * pace;
                    r.foes[bk].state = 0;
                }
            }
            _ => {
                // ---- his four moves (phases 1 and 3), a charge now and then in phase 3
                if rg.mv == NO_MOVE {
                    rg.t -= dt;
                    let mark_pos = pos_of(rg.mark).or_else(|| ups.first().map(|u| u.1));
                    if let Some(mp) = mark_pos {
                        let b = &mut r.foes[bk];
                        face(b, mp, dt, 3.0);
                        walk(b, mp, 3.0, 3.2, dt, 3);
                    }
                    if r.phase == 3 && r.cd[3] <= 0.0 {
                        let rg = &mut r.regent;
                        rg.mode = RM_STEPS;
                        rg.vanished = true;
                        rg.side = ((r.rng >> 7) % 4) as u8;
                        rg.mark = next_mark(rg.mark);
                        rg.t = 2.4 * pace;
                        r.cd[3] = (if fin { 10.0 } else { 15.0 }) * pace;
                        r.note("He leaves the roof  ·  listen: when his footsteps stop, get out of his path");
                    } else if r.regent.t <= 0.0 {
                        let rnd = (r.rng >> 9) % 4;
                        r.rng ^= r.rng << 13;
                        r.rng ^= r.rng >> 17;
                        r.rng ^= r.rng << 5;
                        let mut mv = rnd as u8;
                        if mv == r.regent.last_mv {
                            mv = (mv + 1) % 4;
                        }
                        let mark = next_mark(r.regent.mark);
                        if let Some(mp) = pos_of(mark) {
                            let b = &mut r.foes[bk];
                            b.yaw = yaw_to(b.pos, mp);
                            b.state = 1 + mv;
                            let (bp, yaw) = (b.pos, b.yaw);
                            let fwd = v3(yaw.sin(), 0.0, yaw.cos());
                            let (w, tl, sfx) = match mv {
                                MV_JAB => (0.95, Tele { kind: TeleKind::Cone, pos: bp, dir: yaw, r: 4.2, len: 0.55, t: 0.0, total: 0.0, dmg: 0.0, cause: Cause::Jab, hit: 0 }, crate::game::Sfx::Hit),
                                MV_RING => (1.45, Tele { kind: TeleKind::Donut, pos: bp, dir: yaw, r: 9.0, len: 0.0, t: 0.0, total: 0.0, dmg: 0.0, cause: Cause::BloodRing, hit: 0 }, crate::game::Sfx::ChargeReady),
                                MV_THRUST => (1.15, Tele { kind: TeleKind::Line, pos: bp - fwd * 9.0, dir: yaw, r: 1.1, len: 18.0, t: 0.0, total: 0.0, dmg: 0.0, cause: Cause::Thrust, hit: 0 }, crate::game::Sfx::GrappleFire),
                                _ => (1.3, Tele { kind: TeleKind::Arc, pos: bp, dir: yaw, r: 5.8, len: 0.6, t: 0.0, total: 0.0, dmg: 0.0, cause: Cause::Arc, hit: 0 }, crate::game::Sfx::Slash),
                            };
                            let w = w * pace;
                            r.teles.push(Tele { t: w, total: w, ..tl });
                            cues.push((sfx, bp));
                            let rg = &mut r.regent;
                            rg.mv = mv;
                            rg.last_mv = mv;
                            rg.wind = w;
                            rg.mark = mark;
                        }
                    }
                } else {
                    rg.wind -= dt;
                    r.foes[bk].t = r.regent.wind;
                    if r.regent.wind <= 0.0 {
                        let mv = r.regent.mv;
                        let mark = r.regent.mark as usize;
                        let b = r.foes[bk];
                        let fwd = v3(b.yaw.sin(), 0.0, b.yaw.cos());
                        let (tl, dmg, cause) = match mv {
                            MV_JAB => (Tele { kind: TeleKind::Cone, pos: b.pos, dir: b.yaw, r: 4.2, len: 0.55, t: 0.0, total: 1.0, dmg: 0.0, cause: Cause::Jab, hit: 0 }, 30.0, Cause::Jab),
                            MV_RING => (Tele { kind: TeleKind::Donut, pos: b.pos, dir: b.yaw, r: 9.0, len: 0.0, t: 0.0, total: 1.0, dmg: 0.0, cause: Cause::BloodRing, hit: 0 }, 34.0, Cause::BloodRing),
                            MV_THRUST => (Tele { kind: TeleKind::Line, pos: b.pos - fwd * 9.0, dir: b.yaw, r: 1.1, len: 18.0, t: 0.0, total: 1.0, dmg: 0.0, cause: Cause::Thrust, hit: 0 }, 36.0, Cause::Thrust),
                            MV_CHARGE => (Tele { kind: TeleKind::Line, pos: b.pos, dir: b.yaw, r: 1.3, len: 24.0, t: 0.0, total: 1.0, dmg: 0.0, cause: Cause::Charge, hit: 0 }, 40.0, Cause::Charge),
                            _ => (Tele { kind: TeleKind::Arc, pos: b.pos, dir: b.yaw, r: 5.8, len: 0.6, t: 0.0, total: 1.0, dmg: 0.0, cause: Cause::Arc, hit: 0 }, 34.0, Cause::Arc),
                        };
                        // the charge runs over everyone in its path; the rest only find their mark
                        let mut landed = false;
                        for (s, p) in &ups {
                            if (mv == MV_CHARGE || *s == mark) && tl.covers(*p) {
                                hits.push((*s, dmg, cause));
                                landed |= *s == mark;
                            }
                        }
                        if landed && mv != MV_CHARGE {
                            // a move that finds its mark feeds him
                            let f = &mut r.foes[bk];
                            f.hp = (f.hp + f.max * 0.015).min(f.max);
                        }
                        if mv == MV_CHARGE {
                            // he comes to a halt across the roof, then throws his spell
                            let end = b.pos + fwd * 16.0;
                            let f = &mut r.foes[bk];
                            f.pos = v3(end.x.clamp(roof.x.0 + 1.0, roof.x.1 - 1.0), VY, end.z.clamp(roof.z.0 + 1.0, roof.z.1 - 1.0));
                            f.state = 7;
                            r.regent.vanished = false;
                            r.regent.mode = RM_SPELL;
                            r.regent.t = 0.7 * pace;
                        } else {
                            r.foes[bk].state = 0;
                            r.regent.t = (if r.phase == 3 { 1.3 } else { 1.05 }) * pace;
                        }
                        r.regent.mv = NO_MOVE;
                    }
                }
                // phase 3: lightning and its pools
                if r.phase == 3 && r.cd[2] <= 0.0 && !ups.is_empty() {
                    let pick = ups[(r.rng as usize >> 5) % ups.len()].1;
                    let off = v3(((r.rng >> 11) % 100) as f32 / 50.0 - 1.0, 0.0, ((r.rng >> 17) % 100) as f32 / 50.0 - 1.0) * 2.0;
                    r.rng ^= r.rng << 13;
                    r.rng ^= r.rng >> 17;
                    r.rng ^= r.rng << 5;
                    let at = v3((pick.x + off.x).clamp(roof.x.0 + 1.0, roof.x.1 - 1.0), VY, (pick.z + off.z).clamp(roof.z.0 + 1.0, roof.z.1 - 1.0));
                    let w = 0.9 * pace;
                    r.teles.push(Tele { kind: TeleKind::Circle, pos: at, dir: 0.0, r: 1.6, len: 0.0, t: w, total: w, dmg: 18.0, cause: Cause::AshPool, hit: 0 });
                    r.pools.push(Pool { kind: 3, pos: at, r: -w * 2.0, max: 1.6, life: if fin { 60.0 } else { 40.0 } });
                    r.cd[2] = (if fin { 2.2 } else { 3.6 }) * pace;
                }
            }
        }
        // the waves roll on
        let speed = if hard { 7.5 } else { 6.0 };
        for w in &mut r.waves {
            w.pos += if w.axis % 2 == 0 { speed * dt } else { -speed * dt };
            for (s, p) in &ups {
                if w.hit & (1 << s) == 0 && w.catches(*p) {
                    w.hit |= 1 << s;
                    hits.push((*s, 20.0, Cause::Wave));
                }
            }
        }
        r.waves.retain(|w| match w.axis {
            0 => w.pos < roof.x.1 + 0.5,
            1 => w.pos > roof.x.0 - 0.5,
            2 => w.pos < roof.z.1 + 0.5,
            _ => w.pos > roof.z.0 - 0.5,
        });
        for cd in r.cd.iter_mut() {
            *cd -= dt;
        }
    }
    for (s, p) in cues {
        g.sound(s, p);
    }
    for (s, d, c) in hits {
        hurt(g, s, d, c);
    }
}

/// A point on the roof's edge (0 west, 1 east, 2 south, 3 north), `inset` metres in.
pub fn side_point(side: u8, roof: vault::Room, inset: f32) -> (f32, f32) {
    let (cx, cz) = ((roof.x.0 + roof.x.1) * 0.5, (roof.z.0 + roof.z.1) * 0.5);
    match side % 4 {
        0 => (roof.x.0 + inset, cz),
        1 => (roof.x.1 - inset, cz),
        2 => (cx, roof.z.0 + inset),
        _ => (cx, roof.z.1 - inset),
    }
}

/// (host) E pressed in the raid: open the chest when near it.
fn host_use(g: &mut Game, s: usize) {
    let near = g.raid.as_ref().and_then(|r| r.chest.as_ref()).is_some_and(|c| s < g.f.len() && (c.pos - g.f[s].pos).flat().len() < 3.0);
    if near {
        open_chest(g);
    }
}

/// Whether a fighter stands in an ash-blood pool (its guard and abilities are off).
pub fn in_ash(g: &Game, i: usize) -> bool {
    let Some(r) = g.raid.as_ref() else { return false };
    let p = g.f[i].pos;
    r.room == 3 && r.pools.iter().any(|q| q.kind == 3 && q.r > 0.3 && (q.pos - p).flat().len() < q.r)
}

// ---------------------------------------------------------------- the Ashen Magistrate

/// Move the Magistrate's floor (its blocks are the world's last ones): down into the dark below
/// the vault, or back.
pub fn set_floor(g: &mut Game, down: bool) {
    let n = vault::FLOOR_TILES.len();
    let len = g.world.blocks.len();
    if len < n {
        return;
    }
    for (k, (x0, z0, x1, z1)) in vault::FLOOR_TILES.iter().enumerate() {
        let b = &mut g.world.blocks[len - n + k];
        let y = if down { vault::TILE_AWAY } else { VY - 1.0 };
        b.b = Aabb::new(v3(*x0, y, *z0), v3(*x1, y + 1.0, *z1));
    }
}

fn magistrate(g: &mut Game, dt: f32, view: &[(usize, V3, bool)]) {
    let ups = up(view);
    let Some(r) = g.raid.as_mut() else { return };
    let hard = r.hard;
    let room = r.room;
    let mut floor_change = None;
    let mut new_teles = Vec::new();
    let mut sentence: Option<(Call, u8)> = None;
    let Some(bk) = r.foes.iter().position(|f| f.kind == FoeKind::Magistrate) else { return };
    let frac = r.foes[bk].hp / r.foes[bk].max.max(1.0);
    // the phases
    let want = if frac > 0.66 { 1 } else if frac > 0.42 { 2 } else { 3 };
    if want != r.phase {
        r.phase = want;
        match want {
            2 => {
                r.floor_down = true;
                floor_change = Some(true);
                let (x, z) = vault::ISLANDS[0];
                r.foes[bk].pos = v3(x, VY, z);
                r.note("THE FLOOR BREAKS  ·  wall-run between the islands, stay off the fire");
                r.cd[2] = 4.0;
            }
            _ => {
                r.floor_down = false;
                r.condemned = None;
                r.pools.retain(|p| p.kind != 2);
                floor_change = Some(false);
                r.note("THE ASH SETTLES  ·  watch the sign over him: SPREAD or STACK");
                r.cd[3] = 3.0;
            }
        }
    }
    let rnd = r.rand();
    let b = &mut r.foes[bk];
    if let Some((_, tp)) = nearest(b.pos, &ups) {
        face(b, tp, dt, 2.4);
        if r.phase != 2 && b.state == 0 {
            walk(b, tp, 2.8, 2.6, dt, room);
        }
        let d = (tp - b.pos).flat().len();
        // the gavel slam where you stand
        if r.cd[0] <= 0.0 && r.phase != 2 {
            new_teles.push((TeleKind::Circle, tp, 0.0, 3.6, 0.0, 1.5, 36.0, Cause::Gavel));
            b.state = 1;
            b.t = 1.5;
            r.cd[0] = if r.phase == 3 { 9.0 } else { 6.0 };
        }
        // a Verdict shot: a straight line at someone
        if r.cd[1] <= 0.0 {
            let pick = ups[(rnd * ups.len() as f32) as usize % ups.len()].1;
            new_teles.push((TeleKind::Line, b.pos, yaw_to(b.pos, pick), 0.75, 30.0, 1.15, if hard { 42.0 } else { 34.0 }, Cause::Verdict));
            b.state = 2;
            b.t = 1.15;
            r.cd[1] = if hard { 5.0 } else { 6.8 };
        }
        // the sentence sweep when you're close
        if r.cd[4] <= 0.0 && d < 5.0 && r.phase == 1 {
            new_teles.push((TeleKind::Cone, b.pos, b.yaw, 6.0, 1.05, 1.2, 30.0, Cause::Sweep));
            b.state = 3;
            b.t = 1.2;
            r.cd[4] = 9.0;
        }
    }
    if b.t > 0.0 {
        b.t -= dt;
        if b.t <= 0.0 {
            b.state = 0;
        }
    }
    // phase 2: an island at a time condemned to the fire
    if r.phase == 2 {
        match r.condemned {
            Some((i, t)) => {
                let t = t - dt;
                if t <= 0.0 {
                    let (x, z) = vault::ISLANDS[i as usize];
                    r.pools.push(Pool { kind: 2, pos: v3(x, VY, z), r: vault::ISLAND_HALF + 0.6, max: vault::ISLAND_HALF + 0.6, life: if hard { 9.0 } else { 7.0 } });
                    r.condemned = None;
                    r.cd[2] = if hard { 6.0 } else { 11.0 };
                } else {
                    r.condemned = Some((i, t));
                }
            }
            None if r.cd[2] <= 0.0 => {
                // the outer island with most of you on it
                let mut best = (0usize, 1u8);
                for k in 1..5u8 {
                    let (x, z) = vault::ISLANDS[k as usize];
                    let c = ups.iter().filter(|u| (u.1 - v3(x, 0.0, z)).flat().len() < vault::ISLAND_HALF + 0.5).count();
                    if c > best.0 {
                        best = (c, k);
                    }
                }
                let k = if best.0 == 0 { 1 + (rnd * 4.0) as u8 % 4 } else { best.1 };
                r.condemned = Some((k, 3.0));
                r.note("An island is CONDEMNED  ·  get off it!");
            }
            None => {}
        }
    }
    // phase 3: the Divided Sentence
    if r.phase == 3 {
        match r.call {
            Some((c, t, m)) => {
                let t = t - dt;
                if t <= 0.0 {
                    sentence = Some((c, m));
                    r.call = None;
                    r.cd[3] = if hard { 6.0 } else { 8.0 };
                } else {
                    r.call = Some((c, t, m));
                }
            }
            None if r.cd[3] <= 0.0 && !ups.is_empty() => {
                let c = if rnd < 0.5 { Call::Spread } else { Call::Stack };
                let m = ups[(rnd * 7.0) as usize % ups.len()].0 as u8;
                r.call = Some((c, if hard { 3.2 } else { 4.5 }, m));
            }
            None => {}
        }
    }
    for cd in r.cd.iter_mut() {
        *cd -= dt;
    }
    for (k, p, d, rad, len, t, dmg, c) in new_teles {
        tele(r, k, p, d, rad, len, t, dmg, c);
    }
    if let Some(down) = floor_change {
        set_floor(g, down);
    }
    // the sentence lands: shared out by how you stand
    if let Some((c, m)) = sentence {
        let pos: Vec<(usize, V3)> = ups.clone();
        match c {
            Call::Spread => {
                for (s, p) in &pos {
                    let near = pos.iter().filter(|(o, q)| o != s && (*q - *p).flat().len() < SENTENCE_R).count();
                    hurt(g, *s, 18.0 + 32.0 * near as f32, Cause::Spread);
                }
            }
            Call::Stack => {
                let at = pos.iter().find(|p| p.0 == m as usize).map(|p| p.1).or(pos.first().map(|p| p.1));
                if let Some(at) = at {
                    let inside: Vec<usize> = pos.iter().filter(|(_, q)| (*q - at).flat().len() < SENTENCE_R).map(|p| p.0).collect();
                    // (as much as the whole party, all in, takes 32 each of; fewer in, more each)
                    let party = g.raid.as_ref().map_or(4, |r| r.members.len()) as f32;
                    let share = party * if hard { 42.0 } else { 32.0 } / inside.len().max(1) as f32;
                    for s in inside {
                        hurt(g, s, share, Cause::Stack);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------- what lands

fn hazards(g: &mut Game, dt: f32, view: &[(usize, V3, bool)]) {
    let ups = up(view);
    let ground: Vec<(usize, bool, V3)> = ups.iter().map(|(s, _p)| (*s, g.f[*s].on_ground, g.f[*s].chest())).collect();
    let guarding: Vec<usize> = ups.iter().filter(|(s, _)| g.f[*s].guard).map(|(s, _)| *s).collect();
    let Some(r) = g.raid.as_mut() else { return };
    let mut hits: Vec<(usize, f32, Cause)> = Vec::new();
    // telegraphs
    for t in &mut r.teles {
        t.t -= dt;
        if t.kind == TeleKind::Ring {
            for (k, (s, on_ground, _)) in ground.iter().enumerate() {
                let p = ups[k].1;
                if *on_ground && t.hit & (1 << s) == 0 && t.covers(p) {
                    t.hit |= 1 << s;
                    hits.push((*s, t.dmg, t.cause));
                }
            }
        } else if t.t <= 0.0 {
            for (s, p) in &ups {
                if t.covers(*p) {
                    hits.push((*s, t.dmg, t.cause));
                }
            }
        }
    }
    r.teles.retain(|t| t.t > 0.0);
    // pools: wax spreads; fire burns
    for p in &mut r.pools {
        p.life -= dt;
        p.r = (p.r + dt * 0.35).min(p.max);
        for (s, q) in &ups {
            if (*q - p.pos).flat().len() < p.r && (q.y - p.pos.y).abs() < 1.2 {
                let (dps, c) = match p.kind {
                    2 => (16.0, Cause::Condemned),
                    3 => (11.0, Cause::AshPool),
                    _ => (9.0, Cause::WaxPool),
                };
                hits.push((*s, dps * dt, c));
            }
        }
    }
    r.pools.retain(|p| p.life > 0.0);
    // thrown things
    let world = &g.world;
    for s in &mut r.shots {
        let next = s.pos + s.vel * dt;
        s.life -= dt;
        if !world.line_clear(s.pos, next) {
            s.life = 0.0;
        }
        s.pos = next;
        for (slot, _, chest) in &ground {
            if s.life > 0.0 && (*chest - s.pos).len() < 0.6 {
                // (the Regent's spell is blocked by a raised guard)
                if s.kind == 3 && guarding.contains(slot) {
                    s.life = 0.0;
                    continue;
                }
                let c = match s.kind {
                    0 => Cause::Coins,
                    3 => Cause::Spell,
                    _ => Cause::Blades,
                };
                hits.push((*slot, s.dmg, c));
                s.life = 0.0;
            }
        }
    }
    r.shots.retain(|s| s.life > 0.0);
    for (s, d, c) in hits {
        hurt(g, s, d, c);
    }
}

// ============================================================================== clears and loot

fn stage_notes(r: &mut Raid, was: Stage, was_room: usize) {
    if r.room != was_room {
        r.notes.push((format!("{}", ROOMS[r.room].name), 4.0));
    }
    match r.stage {
        Stage::Cleared if was != Stage::Cleared => {
            r.notes.push((format!("VANQUISHED  ·  {}", fmt_time(r.splits[r.room])), 5.0));
            if r.room < 4 {
                r.notes.push((ROOMS[r.room].lore.to_string(), 8.0));
            }
        }
        Stage::Wipe if was != Stage::Wipe => r.notes.push(("THE PARTY FALLS  ·  the room resets".into(), 4.0)),
        Stage::Fight if was == Stage::Intro => {
            let boss = r.boss().map(|b| b.kind.name()).unwrap_or("");
            let name = ROOMS[r.room].name;
            r.notes.push((format!("{boss}  ·  {name}"), 4.0));
        }
        _ => {}
    }
}

pub fn fmt_time(s: f32) -> String {
    let ms = (s * 1000.0) as u32;
    format!("{}:{:02}.{}", ms / 60000, (ms / 1000) % 60, (ms % 1000) / 100)
}

fn room_cleared(g: &mut Game) {
    let Some(r) = g.raid.as_mut() else { return };
    let room = r.room;
    r.splits[room] = r.room_time;
    r.teles.clear();
    r.shots.clear();
    r.pools.clear();
    r.tethers.clear();
    r.call = None;
    r.condemned = None;
    for m in &mut r.members {
        m.downed = false;
        m.revive = 0.0;
    }
    let pos = ROOMS[room].centre() + v3(0.0, 0.0, -2.0);
    // the loot: one piece each from the boss's table (what you don't have yet first), a chance
    // of something rare (better in Sealed Writ, which also gives a second roll)
    let (common, rare) = DROPS[room];
    let hard = r.hard;
    let mut rolls: Vec<(u8, u8, bool)> = Vec::new();
    let members: Vec<(u8, bool)> = r.members.iter().map(|m| (m.slot, m.bot)).collect();
    for (slot, bot) in members {
        if bot {
            continue;
        }
        let own = owned_of(g, slot as usize);
        let Some(r) = g.raid.as_mut() else { return };
        for _ in 0..(1 + hard as usize) {
            let fresh: Vec<u8> = common.iter().copied().filter(|&i| !own.has(i)).collect();
            let pool = if fresh.is_empty() { common.to_vec() } else { fresh };
            let pick = pool[(r.rand() * pool.len() as f32) as usize % pool.len()];
            rolls.push((slot, pick, false));
        }
        let chance = if hard { 0.14 } else { 0.08 };
        if r.rand() < chance {
            let pick = rare[(r.rand() * rare.len() as f32) as usize % rare.len()];
            rolls.push((slot, pick, true));
        }
    }
    let Some(r) = g.raid.as_mut() else { return };
    let any_rare = rolls.iter().any(|x| x.2);
    r.chest = Some(Chest { pos, rare: any_rare, open: false, items: rolls.iter().map(|x| (x.0, x.1)).collect() });
    if room == 4 {
        r.stage = Stage::Done;
        r.stage_t = 25.0;
        r.result_ms = (r.total_time * 1000.0) as u32;
    } else {
        r.stage = Stage::Cleared;
        r.stage_t = 0.0;
    }
    let lore = ROOMS[room].lore;
    let split = fmt_time(r.room_time);
    r.note(format!("VANQUISHED  ·  {split}"));
    if room < 4 {
        r.note(lore);
    }
    let slots: Vec<usize> = r.members.iter().map(|m| m.slot as usize).collect();
    for s in slots {
        if s < g.f.len() {
            g.f[s].hp = MAX_HP;
        }
    }
    for (slot, item, rare) in rolls {
        raid_msg(g, &RaidMsg::Loot { slot, item, rare });
        got_loot(g, slot as usize, item, rare);
    }
    if room == 4 {
        let (size, ms, hard) = {
            let r = g.raid.as_ref().unwrap();
            (r.members.len().clamp(2, 4) as u8, r.result_ms, r.hard)
        };
        raid_msg(g, &RaidMsg::End { size, ms, hard });
        finished_here(g, size as usize, ms, hard);
        post_board(g, size as usize, ms);
    }
}

fn owned_of(g: &Game, slot: usize) -> RaidEarned {
    if slot == g.local {
        g.hub.as_ref().map(|h| h.earned.raid).unwrap_or_default()
    } else {
        RaidEarned::default()
    }
}

/// Loot came to someone: mine is kept (and shown in the chest).
fn got_loot(g: &mut Game, slot: usize, item: u8, rare: bool) {
    if slot != g.local {
        return;
    }
    if let Some(h) = g.hub.as_mut() {
        h.earned.raid.give(item);
        h.refresh_bank();
    }
    if let Some(r) = g.raid.as_mut() {
        r.mine.push((item, rare));
        if let Some(c) = r.chest.as_mut() {
            c.rare |= rare;
        }
    }
}

/// The run is over here: what's earned for it (titles, best times).
fn finished_here(g: &mut Game, size: usize, ms: u32, hard: bool) {
    let me = g.local;
    if !g.in_raid(me) {
        return;
    }
    // (a session with changed stats or unlocks sets no records: crate::roles)
    if !g.counts_for_records(me) {
        if let Some(r) = g.raid.as_mut() {
            r.result_ms = ms;
            r.note("THE VAULT IS BROKEN  ·  not recorded (this session used admin commands)");
        }
        return;
    }
    if let Some(h) = g.hub.as_mut() {
        let e = &mut h.earned.raid;
        e.cleared = true;
        e.give(L_VAULTBREAKER);
        if hard {
            e.cleared_hard = true;
            e.give(L_SEALED_WRIT);
        }
        let k = size.clamp(2, 4) - 2;
        if e.best[k] == 0 || ms < e.best[k] {
            e.best[k] = ms;
        }
    }
    if let Some(r) = g.raid.as_mut() {
        r.result_ms = ms;
        r.note(format!("THE VAULT IS BROKEN  ·  {}", fmt_time(ms as f32 / 1000.0)));
    }
}

/// (host) The leaderboard for this party size: the names, the time.
fn post_board(g: &mut Game, size: usize, ms: u32) {
    // (not when anyone in the party used admin commands this session)
    if g.raid.as_ref().is_some_and(|r| r.members.iter().any(|m| !m.bot && !g.counts_for_records(m.slot as usize))) {
        return;
    }
    let names: Vec<String> = g.raid.as_ref().map(|r| r.members.iter().map(|m| m.name.clone()).collect()).unwrap_or_default();
    let row = names.join(", ");
    let board = crate::hub::BOARD_RAID + (size.clamp(2, 4) - 2) as u8;
    if g.hub.as_mut().is_some_and(|h| h.post(board, &row, ms)) {
        g.host_send_board(board);
    }
}

/// (host) Someone leaves: they're out; once no player is left, the raid is over.
fn host_leave(g: &mut Game, s: usize) {
    let Some(r) = g.raid.as_mut() else { return };
    r.members.retain(|m| m.slot as usize != s);
    let players_left = r.members.iter().any(|m| !m.bot);
    if !players_left || r.members.len() < 1 {
        end_raid(g);
    }
}

/// (host) The raid is over: everyone back to the gate.
pub fn end_raid(g: &mut Game) {
    let Some(r) = g.raid.take() else { return };
    for m in &r.members {
        let s = m.slot as usize;
        if s < g.f.len() && g.f[s].owned {
            to_gate(g, s);
        }
    }
    set_floor(g, false);
    raid_msg(g, &RaidMsg::Gone);
}

/// (host) Open the chest (anyone near it pressing E opens it for everyone).
pub fn open_chest(g: &mut Game) {
    if let Some(c) = g.raid.as_mut().and_then(|r| r.chest.as_mut()) {
        c.open = true;
    }
}

// ============================================================================== for drawing

/// Lights the vault adds: (position, reach, colour linear).
pub fn lights(g: &Game) -> Vec<(V3, f32, [f32; 3])> {
    let Some(r) = g.raid.as_ref() else { return Vec::new() };
    let mut out = Vec::new();
    let flick = 0.85 + 0.15 * (g.time * 13.0).sin() * (g.time * 7.3).cos();
    // each boss lights itself: warm fire on the Warden and the Magistrate, gold on Tally, cold on
    // Seal, violet on the Ledger (brighter as they wind up, so the move reads)
    for (k, f) in r.foes.iter().enumerate().filter(|(_, f)| f.alive) {
        let p = if g.is_authority() { f.pos } else { f.shown };
        let up = if f.state != 0 { 1.35 } else { 1.0 };
        let fwd = yaw_fwd(f.yaw);
        // a cool key light from the front and above, so its shape reads against the dark hall
        let (key_h, key) = match f.kind {
            FoeKind::Warden => (6.5, 1.2),
            FoeKind::Ledger => (6.0, 1.0),
            FoeKind::Magistrate => (5.5, 1.3),
            FoeKind::Tally | FoeKind::Seal => (4.5, 0.8),
            _ => (0.0, 0.0),
        };
        if key > 0.0 {
            out.push((p + fwd * 3.5 + v3(0.0, key_h, 0.0), 10.0, [0.85 * key, 0.95 * key, 1.45 * key]));
        }
        match f.kind {
            FoeKind::Warden => {
                out.push((p + v3(0.0, 5.4, 0.0), 11.0, [3.4 * flick * up, 2.0 * flick * up, 0.7]));
                out.push((p + v3(0.0, 3.0, 0.0) + fwd * 1.2, 4.0, [2.0 * flick, 0.7, 0.2]));
            }
            FoeKind::Tally => out.push((p + v3(0.0, 3.3, 0.0) + fwd * 0.6, 6.5, [2.3 * up, 1.75 * up, 0.7])),
            FoeKind::Seal => out.push((p + v3(0.0, 3.6, 0.0) + fwd * 0.6, 6.5, [0.9 * up, 1.1 * up, 1.7 * up])),
            FoeKind::Ledger => out.push((p + v3(0.0, 3.4, 0.0) + fwd * 0.8, 11.0, [1.7 * up, 0.75 * up, 2.8 * up])),
            FoeKind::Magistrate => {
                let gavel = p + yaw_right(f.yaw) * 1.0 + fwd * 0.7 + v3(0.0, if f.state == 1 { 5.0 } else { 3.1 }, 0.0);
                out.push((gavel, 8.5, [3.2 * flick * up, 1.3 * flick * up, 0.35]));
            }
            FoeKind::Regent if !r.regent.vanished => out.push((p + v3(0.0, 2.6, 0.0), 4.0, [1.6, 0.15, 0.12])),
            _ => {}
        }
        // the Assessors bound: their chain glows between them
        if f.kind == FoeKind::Tally && matches!(vulnerable(r, k), Err(s) if s.starts_with("BOUND")) {
            if let Some(o) = r.foes.iter().find(|o| o.alive && o.kind == FoeKind::Seal) {
                let op = if g.is_authority() { o.pos } else { o.shown };
                out.push(((p + op) * 0.5 + v3(0.0, 1.6, 0.0), 7.0, [2.6, 1.9, 0.6]));
            }
        }
    }
    // each hall's own light: high, cold shafts from its windows against the bosses' warm fire, so
    // they stand out from the dark
    let rm = ROOMS[r.room.min(ROOMS.len() - 1)];
    let c = rm.centre();
    let top = VY + vault::ROOM_H - 1.5;
    match r.room {
        0 => {
            out.push((v3(rm.x.0 + 2.5, top, rm.z.0 + 2.5), 16.0, [0.5, 0.75, 1.7]));
            out.push((v3(rm.x.1 - 2.5, top, rm.z.1 - 2.5), 16.0, [0.5, 0.75, 1.7]));
            for (k, (x, z)) in vault::PLATES.iter().enumerate() {
                let lit = r.plates.get(k).copied().unwrap_or(0.0);
                if lit > 0.2 {
                    out.push((v3(*x, VY + 1.0, *z), 4.5, [2.2 * lit, 1.5 * lit, 0.5 * lit]));
                }
            }
        }
        1 => {
            out.push((v3(c.x - 5.0, top - 0.5, c.z), 10.0, [2.0, 1.5, 0.65]));
            out.push((v3(c.x + 5.0, top - 0.5, c.z), 10.0, [2.0, 1.5, 0.65]));
            out.push((v3(c.x, top, rm.z.0 + 1.5), 15.0, [0.55, 0.75, 1.6]));
        }
        2 => {
            out.push((v3(c.x, top - 1.0, rm.z.0 + 2.0), 11.0, [0.55, 0.6, 1.4]));
            out.push((v3(c.x, top - 1.0, rm.z.1 - 2.0), 11.0, [0.55, 0.6, 1.4]));
        }
        4 if !r.floor_down => {
            out.push((v3(c.x - 8.0, top + 1.0, c.z - 6.0), 18.0, [0.7, 0.85, 1.8]));
            out.push((v3(c.x + 8.0, top + 1.0, c.z + 8.0), 18.0, [0.7, 0.85, 1.8]));
        }
        _ => {}
    }
    // the roof's red moonlight, and its corner lanterns burning red
    if r.room == 3 {
        let (x, z) = vault::ROOF_CENTRE;
        out.push((v3(x + 6.0, VY + 9.0, z + 14.0), 28.0, [1.4, 0.35, 0.3]));
        let rf = ROOMS[3];
        for (cx, cz) in [(rf.x.0 + 0.6, rf.z.0 + 0.6), (rf.x.1 - 0.6, rf.z.0 + 0.6), (rf.x.0 + 0.6, rf.z.1 - 0.6), (rf.x.1 - 0.6, rf.z.1 - 0.6)] {
            out.push((v3(cx, VY + 2.3, cz), 6.0, [2.2 * flick, 0.5, 0.3]));
        }
        // lightning flashes over the roof as it strikes
        for t in r.teles.iter().filter(|t| t.cause == Cause::AshPool && t.t < 0.12) {
            out.push((t.pos + v3(0.0, 3.0, 0.0), 12.0, [4.0, 3.2, 3.6]));
        }
    }
    if r.room == 4 && r.floor_down {
        let (x, z) = vault::PIT_CENTRE;
        for (dx, dz) in [(-12.0f32, -8.0f32), (10.0, -8.0), (-12.0, 10.0), (10.0, 10.0)] {
            out.push((v3(x + dx, VY - 3.0, z + dz), 14.0, [3.0 * flick, 1.1, 0.25]));
        }
    }
    out
}

/// Night on the Regent's roof where the camera is (1: the red-moon night; 0: elsewhere).
pub fn night(g: &Game, cam: V3) -> f32 {
    if g.raid.is_none() {
        return 0.0;
    }
    if ROOMS[3].contains(cam, 1.0) || (cam.y > VY - 2.0 && cam.y < VY + 14.0 && cam.x > 14.0 && cam.z > -10.0 && cam.z < 12.0) { 1.0 } else { 0.0 }
}

/// Whether the camera is down in one of the vault's halls (not the Regent's roof): its light is
/// low there, so the bosses' fires and the halls' cold windows carry the scene.
pub fn indoors(g: &Game, cam: V3) -> bool {
    g.raid.is_some() && cam.y < VY + vault::ROOM_H + 1.0 && ROOMS.iter().enumerate().any(|(k, rm)| k != 3 && rm.contains(cam, 1.0))
}

/// The foes drawn as fighters (the Ashblood Regent), dressed for it.
pub fn fighters(g: &Game) -> Vec<Fighter> {
    let Some(r) = g.raid.as_ref() else { return Vec::new() };
    let mut out = Vec::new();
    for (k, f) in r.foes.iter().enumerate() {
        // (the Magistrate has its own model now: drawn with the other bosses)
        if !f.alive || f.kind != FoeKind::Regent || r.regent.vanished {
            continue;
        }
        let mag = f.kind == FoeKind::Magistrate;
        let mut x = Fighter::new(f.kind.name(), crate::game::MAX_PLAYERS + 2 + k, false);
        x.present = true;
        x.alive = true;
        x.is_bot = true;
        x.loadout.body = if mag { 2 } else { 3 };
        let pos = if g.is_authority() { f.pos } else { f.shown };
        x.pos = pos;
        x.prev_pos = pos;
        x.net_pos = pos;
        x.yaw = f.yaw;
        x.vel = f.vel;
        x.on_ground = true;
        x.color_idx = if mag { 0 } else { 7 };
        // swinging while it strikes; a pistol for the Verdict
        if f.state == 2 && mag {
            x.cur = 1;
            x.weapon = Weapon::HandCannon;
            x.since_shot = 0.05;
        } else {
            x.cur = 0;
            x.weapon = if mag { Weapon::Greatsword } else { Weapon::Katana };
            x.loadout.melee = x.weapon;
            if f.state != 0 && (mag || f.t < 0.22) {
                x.slash_t = 0.12;
                x.slash_idx = (f.t * 3.0) as u8 % 3;
            } else if f.state != 0 {
                // (the Regent winding up: his glaive raised on guard, then the strike)
                x.guard = true;
                x.guard_t = 0.3;
            }
        }
        let mut o = crate::hub::Outfit::default();
        if mag {
            for (slot, id) in [(crate::hub::Slot::Head, 80u8), (crate::hub::Slot::Cape, 81), (crate::hub::Slot::Neck, 82), (crate::hub::Slot::Body, 83), (crate::hub::Slot::Legs, 74), (crate::hub::Slot::Hands, 75), (crate::hub::Slot::Feet, 76)] {
                o.items[slot as usize] = id;
                o.dyes[slot as usize] = [24, 25, 12];
            }
        } else {
            // the Regent: his long coat, black hose and court shoes, the Ashblood Glaive
            for (slot, id, dyes) in [(crate::hub::Slot::Body, 84u8, [22u8, 26, 12]), (crate::hub::Slot::Legs, 24, [22, 22, 22]), (crate::hub::Slot::Feet, 26, [22, 26, 12]), (crate::hub::Slot::Main, 94, [0, 1, 2])] {
                o.items[slot as usize] = id;
                o.dyes[slot as usize] = dyes;
            }
            x.loadout.body = 0;
            if x.cur == 0 {
                x.weapon = Weapon::Greatsword;
                x.loadout.melee = Weapon::Greatsword;
            }
        }
        x.hub.look = o;
        x.hub.townsfolk = false;
        x.hub.boss = if mag { 2 } else { 1 };
        x.ghost = 0.0;
        out.push(x);
    }
    out
}
