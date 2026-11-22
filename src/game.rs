use crate::bot::Bot;
use crate::math::*;
use crate::world::World;
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
}

/// How a gun shoots.
pub struct GunStats {
    pub pellets: usize,
    pub spread_still: f32,
    pub spread_moving: f32,
    pub dmg: f32,
    pub range: f32,
    pub head: f32,
    pub knock: f32,
}

impl Weapon {
    pub const MELEE: [Weapon; 3] = [Weapon::Dagger, Weapon::Katana, Weapon::Greatsword];
    pub const GUNS: [Weapon; 5] = [Weapon::Shotgun, Weapon::Revolver, Weapon::Smg, Weapon::Rifle, Weapon::Rocket];

    pub fn from_u8(v: u8) -> Weapon {
        match v {
            0 => Weapon::Dagger,
            1 => Weapon::Katana,
            2 => Weapon::Greatsword,
            3 => Weapon::Shotgun,
            4 => Weapon::Revolver,
            5 => Weapon::Smg,
            6 => Weapon::Rifle,
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
            Weapon::Rifle => "one heavy shot, huge headshots",
            Weapon::Rocket => "explodes, rocket-jump with it",
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
            Weapon::Rifle => 5,
            Weapon::Rocket => 1,
            _ => 0,
        }
    }
    pub fn interval(self) -> f32 {
        match self {
            Weapon::Shotgun => 0.85,
            Weapon::Revolver => 0.30,
            Weapon::Smg => 0.075,
            Weapon::Rifle => 1.0,
            Weapon::Rocket => 0.9,
            _ => 0.0,
        }
    }
    pub fn reload_time(self) -> f32 {
        match self {
            Weapon::Shotgun => 1.9,
            Weapon::Revolver => 1.5,
            Weapon::Smg => 1.8,
            Weapon::Rifle => 2.2,
            Weapon::Rocket => 2.0,
            _ => 0.0,
        }
    }
    pub fn gun(self) -> GunStats {
        match self {
            Weapon::Shotgun => GunStats { pellets: 9, spread_still: 0.075, spread_moving: 0.075, dmg: 7.5, range: 40.0, head: 1.5, knock: 1.1 },
            Weapon::Smg => GunStats { pellets: 1, spread_still: 0.022, spread_moving: 0.045, dmg: 9.0, range: 80.0, head: 1.4, knock: 0.4 },
            Weapon::Rifle => GunStats { pellets: 1, spread_still: 0.0015, spread_moving: 0.03, dmg: 48.0, range: 220.0, head: 2.0, knock: 3.0 },
            _ => GunStats { pellets: 1, spread_still: 0.004, spread_moving: 0.018, dmg: 24.0, range: 150.0, head: 1.75, knock: 1.6 },
        }
    }
    /// The most one hit can do (used by the host to throw out impossible hits).
    pub fn max_hit(self) -> f32 {
        match self {
            Weapon::Dagger => 32.0 * 0.72,
            Weapon::Katana => 32.0,
            Weapon::Greatsword => 32.0 * 1.55,
            Weapon::Rocket => 85.0,
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
        let a = (rng.next_u32() % 5) as usize;
        let b = (a + 1 + (rng.next_u32() % 4) as usize) % 5;
        Loadout { melee: m, gun1: Weapon::GUNS[a], gun2: Weapon::GUNS[b], item: (rng.next_u32() % 3) as u8 }
    }
}

/// What killed someone, for the kill feed (weapons by number, 8 grenade).
pub fn kill_tool(w: u8) -> &'static str {
    if w == 8 { "Grenade" } else if w < 8 { Weapon::from_u8(w).name() } else { "?" }
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
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct WallRun {
    pub normal: V3,
    pub dir: V3,
    pub t: f32,
    pub vertical: bool,
}

#[derive(Clone, Debug)]
pub struct Fighter {
    pub name: String,
    pub slot: usize,
    pub owned: bool,
    pub is_bot: bool,
    pub present: bool,
    pub team: u8,
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
    pub last_attacker: Option<usize>,
    // network smoothing for remote copies
    pub net_pos: V3,
    pub net_age: f32,
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
            last_attacker: None,
            net_pos: V3::ZERO,
            net_age: 0.0,
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
        n.deaths = self.deaths;
        n.color_idx = self.color_idx;
        n.loadout = self.loadout;
        n.ready = self.ready;
        n.vote = self.vote;
        n.ping_ms = self.ping_ms;
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
}

impl Mode {
    pub fn from_u8(v: u8) -> Mode {
        if v == 1 { Mode::Tdm } else { Mode::Ffa }
    }
    pub fn name(self) -> &'static str {
        match self {
            Mode::Ffa => "Free-for-All",
            Mode::Tdm => "Team Deathmatch",
        }
    }
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
    pub color: [u8; 3],
}

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
    Died { victim: u8, killer: u8, w: u8 },
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
    pub grenades: Vec<Grenade>,
    pub particles: Vec<Particle>,
    pub tracers: Vec<Tracer>,
    pub dmg_nums: Vec<DmgNum>,
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
    pub hurt_flash: f32,
    pub shake: f32,
    pub time: f32,
    pub banner: Option<(String, f32)>,
    pub sounds: Vec<Sfx>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sfx {
    Revolver,
    Shotgun,
    Slash,
    Clang,
    Hit,
    Boom,
    Dash,
    Jump,
    Reload,
    Hurt,
    Tick,
}

pub enum Act {
    Gun { w: Weapon },
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
            mt: MatchState { mode, phase, timer, target: target.max(1), team_score: [0, 0], number: 1 },
            grenades: Vec::new(),
            particles: Vec::new(),
            tracers: Vec::new(),
            dmg_nums: Vec::new(),
            rng: Rng(0x9E3779B97F4A7C15 ^ (local_slot as u64 + 1).wrapping_mul(7919)),
            bots,
            out_events: Vec::new(),
            feed: Vec::new(),
            chat: Vec::new(),
            history: (0..MAX_PLAYERS).map(|_| VecDeque::new()).collect(),
            my_loadout: Loadout::default(),
            hitmarker: 0.0,
            hurt_flash: 0.0,
            shake: 0.0,
            time: 0.0,
            banner: match kind {
                Kind::Host | Kind::Client => Some(("WARM-UP".into(), 2.5)),
                _ => Some(("GET READY".into(), 2.0)),
            },
            sounds: Vec::new(),
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
        if owned {
            let (p, y) = self.pick_spawn(slot);
            self.f[slot].respawn_at(p, y);
        }
    }

    pub fn remove_player(&mut self, slot: usize) {
        self.f[slot] = Fighter::new("", slot, true);
        self.bots[slot] = None;
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
        a != b && self.f[b].present && (self.mt.mode == Mode::Ffa || self.f[a].team != self.f[b].team)
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
        let tdm = self.mt.mode == Mode::Tdm;
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
        let tdm = self.mt.mode == Mode::Tdm;
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

    fn sfx(&mut self, s: Sfx, at: V3) {
        // Only play sounds that happen near the local view.
        let d = (at - self.f[self.local].pos).len();
        if d < 60.0 || self.kind == Kind::Demo {
            self.sounds.push(s);
        }
    }

    pub fn say(&mut self, s: String) {
        self.feed.push(FeedLine { killer: None, victim: (String::new(), 0), tool: "", text: Some(s), life: 5.0 });
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
        }

        for i in 0..self.f.len() {
            if !self.f[i].owned || !self.f[i].present {
                continue;
            }
            self.f[i].spawn_prot = (self.f[i].spawn_prot - dt).max(0.0);
            // respawning
            if !self.f[i].alive {
                self.f[i].respawn_t -= dt;
                if self.f[i].respawn_t <= 0.0 && self.mt.phase != Phase::MatchOver {
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

        // deaths are decided here only on the host (or a game on this computer)
        if self.is_authority() {
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
                    let killer = ff.last_attacker;
                    let w = ff.last_weapon;
                    self.on_death(i, killer, w);
                    if self.kind == Kind::Host {
                        self.out_events.push(Ev::Died { victim: i as u8, killer: killer.map_or(255, |k| k as u8), w });
                    }
                }
            }
            // remember where everyone was, to check hits against the past
            let now = self.time;
            for i in 0..self.f.len() {
                if self.f[i].present {
                    let p = self.f[i].pos;
                    let h = &mut self.history[i];
                    h.push_back((now, p));
                    while h.front().map_or(false, |e| now - e.0 > 1.2) {
                        h.pop_front();
                    }
                }
            }
        }

        // remote fighter smoothing
        for ff in self.f.iter_mut() {
            if ff.owned || !ff.present {
                continue;
            }
            ff.net_age += dt;
            let predicted = ff.net_pos + ff.vel * ff.net_age.min(0.15);
            ff.pos = ff.pos.lerp(predicted, 0.25);
            if (ff.pos - predicted).len() > 4.0 {
                ff.pos = predicted;
            }
            if ff.slash_t > 0.0 {
                ff.slash_t -= dt;
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
        }

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
    fn on_death(&mut self, victim: usize, killer: Option<usize>, w: u8) {
        let vname = (self.f[victim].name.clone(), self.f[victim].color_idx);
        let k = killer.filter(|&k| k != victim && k < self.f.len() && self.f[k].present);
        let kname = k.map(|k| (self.f[k].name.clone(), self.f[k].color_idx));
        self.feed.push(FeedLine { killer: kname, victim: vname, tool: kill_tool(w), text: None, life: 6.0 });
        if self.feed.len() > 6 {
            self.feed.remove(0);
        }
        self.sfx(Sfx::Hurt, self.f[victim].pos);
        let p = self.f[victim].chest();
        self.burst(p, [200, 30, 40], 26, 6.0);
        if self.is_authority() && self.scoring() {
            self.f[victim].deaths += 1;
            if let Some(k) = k {
                if self.is_enemy(k, victim) {
                    self.f[k].kills += 1;
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
            Mode::Ffa => self.f.iter().any(|f| f.present && f.kills >= t),
            Mode::Tdm => self.mt.team_score.iter().any(|&s| s as i32 >= t),
        };
        if won {
            self.end_match();
        }
    }

    /// Who's winning, in words.
    pub fn leader_text(&self) -> String {
        match self.mt.mode {
            Mode::Tdm => {
                let [r, b] = self.mt.team_score;
                if r > b {
                    "RED TEAM WINS".into()
                } else if b > r {
                    "BLUE TEAM WINS".into()
                } else {
                    "DRAW".into()
                }
            }
            Mode::Ffa => {
                let mut best: Option<&Fighter> = None;
                for f in self.f.iter().filter(|f| f.present) {
                    if best.map_or(true, |b| f.kills > b.kills) {
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

    fn end_match(&mut self) {
        self.mt.phase = Phase::MatchOver;
        self.mt.timer = if self.kind == Kind::Demo { 5.0 } else { 9999.0 };
        self.banner = Some((self.leader_text(), 9999.0));
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
                self.mt.timer -= dt;
                if self.mt.timer <= 0.0 {
                    self.mt.phase = Phase::Fight;
                    self.mt.timer = MATCH_TIME;
                    self.banner = Some(("FIGHT!".into(), 1.0));
                    self.sfx(Sfx::Tick, self.f[self.local].pos);
                }
            }
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

    /// Start (or restart) the match: scores reset and everyone respawns.
    pub fn start_match(&mut self) {
        self.mt.number += 1;
        self.mt.phase = Phase::Loadout;
        self.mt.timer = if self.kind == Kind::Offline { 120.0 } else { 20.0 };
        self.mt.team_score = [0, 0];
        self.banner = Some(("CHOOSE YOUR LOADOUT".into(), 2.5));
        self.grenades.clear();
        for i in 0..self.f.len() {
            if self.f[i].present {
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
    pub fn apply_match(&mut self, mode: Mode, phase: Phase, timer: f32, target: u16, team_score: [u16; 2], number: u32) {
        self.mt.mode = mode;
        if number != self.mt.number {
            self.mt.number = number;
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
                Phase::MatchOver => self.banner = Some((self.leader_text(), 9999.0)),
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
        ff.recoil = (ff.recoil - dt * 4.0).max(0.0);
        ff.jump_buf = (ff.jump_buf - dt).max(0.0);
        ff.coyote = (ff.coyote - dt).max(0.0);
        ff.dash_buf = (ff.dash_buf - dt).max(0.0);
        ff.ground_dash_t = (ff.ground_dash_t - dt).max(0.0);
        ff.bfly_t = (ff.bfly_t - dt).max(0.0);
        ff.float_t = (ff.float_t - dt).max(0.0);
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

        // weapon switching
        if let Some(s) = inp.select {
            let s = (s as usize).min(2);
            if s != ff.cur && can_act {
                ff.cur = s;
                ff.weapon = ff.slot_weapon(s);
                ff.switch_t = 0.22;
                ff.reload_t = 0.0;
                ff.slash_t = 0.0;
                ff.slash_queued = false;
            }
        }
        // reloading
        let wi = ff.cur.min(2);
        ff.weapon = ff.slot_weapon(wi);
        let melee = ff.weapon.is_melee();
        let dur = ff.weapon.slash_dur();
        if ff.reload_t > 0.0 {
            ff.reload_t -= dt;
            if ff.reload_t <= 0.0 {
                ff.reload_t = 0.0;
                ff.mag[wi] = ff.weapon.mag();
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
            cancel_slash(&mut ff);
            ff.bfly_t = 0.4; // butterfly: guard-cancel, then slash again
        }
        ff.guard = can_act && melee && inp.guard && !ff.slashing() && ff.switch_t <= 0.0;

        // dash / tumble
        if ff.dash_buf > 0.0 && can_act && ff.dash_cd <= 0.0 && ff.wall.is_none() && !slash_locked {
            let dir = if wish.len() > 0.1 { wish.norm() } else { fwd };
            let mut dashed = false;
            if ff.on_ground {
                ff.vel = dir * 18.5 + v3(0.0, 2.2, 0.0);
                ff.on_ground = false;
                ff.dash_t = 0.2;
                ff.dash_cd = 0.38;
                ff.ground_dash_t = 0.32; // a slash now becomes a rising launcher
                dashed = true;
            } else if !ff.air_dash {
                ff.vel = dir * 15.0 + v3(0.0, 3.5, 0.0);
                ff.air_dash = true;
                ff.dash_t = 0.18;
                ff.dash_cd = 0.32;
                dashed = true;
            }
            if dashed {
                ff.dash_buf = 0.0;
                cancel_slash(&mut ff);
                sounds.push(Sfx::Dash);
            }
        }

        let mut speed = RUN_SPEED;
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
                ff.vel = w.normal * 7.5 + v3(0.0, 8.8, 0.0) + along + look_along * 3.0;
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
                    ff.vel = w.dir * 10.0 + v3(0.0, 1.8 - w.t * 3.5, 0.0) - w.normal * 1.0;
                }
                ff.wall = Some(w);
            }
        } else if ff.dash_t > 0.0 {
            ff.dash_t -= dt;
            gravity = !ff.air_dash || ff.dash_t <= 0.0;
            if ff.on_ground && ff.dash_t <= 0.0 {
                // brake out of a ground dash
                ff.vel.x *= 0.5;
                ff.vel.z *= 0.5;
            }
        } else if ff.on_ground {
            let target = wish * speed;
            let cur = ff.vel.flat();
            let diff = target - cur;
            let acc = (if wish.len() > 0.1 { 120.0 } else { 80.0 }) * dt;
            let step = if diff.len() > acc { diff.norm() * acc } else { diff };
            ff.vel.x += step.x;
            ff.vel.z += step.z;
            if ff.jump_buf > 0.0 && can_act && !slash_locked {
                ff.vel.y = JUMP_VEL;
                ff.on_ground = false;
                ff.jump_buf = 0.0;
                ff.coyote = 0.0;
                cancel_slash(&mut ff);
                sounds.push(Sfx::Jump);
            }
        } else if ff.jump_buf > 0.0 && ff.coyote > 0.0 && can_act && ff.vel.y <= 0.5 {
            // jump just after running off a ledge
            ff.vel.y = JUMP_VEL;
            ff.jump_buf = 0.0;
            ff.coyote = 0.0;
            sounds.push(Sfx::Jump);
        } else {
            // air control
            if wish.len() > 0.1 {
                let target = wish * speed.max(ff.vel.flat().len().min(12.0));
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
                    ff.vel = n * 8.0 + v3(0.0, 8.2, 0.0) + (fwd - n * fwd.dot(n)).flat() * 3.0;
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
                    ff.wall = Some(WallRun { normal: n, dir, t: 0.0, vertical });
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
                            ff.recoil = 1.0;
                            acts.push(Act::Gun { w });
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
        const SHOULDER: f32 = 1.05;
        const HEIGHT: f32 = 1.85;
        const BACK: f32 = 5.0;
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
        for (j, o) in self.f.iter().enumerate() {
            if !o.alive || !self.is_enemy(i, j) {
                continue;
            }
            for b in [o.body_box(), o.head_box()] {
                if let Some((ht, _)) = b.ray(start, dir, t) {
                    t = t.min(ht);
                }
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
                    self.out_events.push(Ev::Item { kind });
                }
            }
            Act::Gun { w: Weapon::Rocket } => {
                let (o, dir) = self.shot_ray(i);
                let v = dir * 34.0;
                self.grenades.push(Grenade { pos: o, vel: v, fuse: 4.0, owner: i, kind: 1 });
                if self.kind == Kind::Host || self.kind == Kind::Client {
                    self.out_events.push(Ev::Grenade { p: o, v, kind: 1 });
                }
                self.sfx(Sfx::Shotgun, o);
                for _ in 0..8 {
                    let v = -dir * self.rng.range(2.0, 5.0) + v3(self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0));
                    self.particles.push(Particle { pos: o, vel: v, life: 0.3, max_life: 0.3, size: 0.25, color: [150, 150, 150], gravity: -1.0 });
                }
                if i == self.local {
                    self.shake = self.shake.max(0.5);
                }
            }
            Act::Gun { w } => {
                let (o, dir) = self.shot_ray(i);
                let moving = self.f[i].vel.flat().len() > 1.0 || !self.f[i].on_ground;
                let gs = w.gun();
                let (pellets, dmg, range) = (gs.pellets, gs.dmg, gs.range);
                let spread = if moving { gs.spread_moving } else { gs.spread_still };
                let right = yaw_right(self.f[i].yaw);
                let up = dir.cross(right).norm();
                for _ in 0..pellets {
                    let a = self.rng.range(0.0, std::f32::consts::TAU);
                    let r = spread * self.rng.f().sqrt();
                    let d = (dir + right * (a.cos() * r) + up * (a.sin() * r)).norm();
                    self.fire_ray(i, o, d, dmg, range, w);
                }
                self.sfx(if matches!(w, Weapon::Shotgun | Weapon::Rifle) { Sfx::Shotgun } else { Sfx::Revolver }, o);
                // muzzle flash
                for _ in 0..5 {
                    let v = dir * self.rng.range(2.0, 6.0) + v3(self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0));
                    self.particles.push(Particle { pos: o, vel: v, life: 0.08, max_life: 0.08, size: 0.18, color: [255, 220, 120], gravity: 0.0 });
                }
                if i == self.local {
                    self.shake = self.shake.max(match w {
                        Weapon::Shotgun | Weapon::Rifle => 0.5,
                        Weapon::Smg => 0.1,
                        _ => 0.25,
                    });
                }
            }
            Act::Slash => {
                let p = self.f[i].pos;
                self.sfx(Sfx::Slash, p);
            }
            Act::Throw => {
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
        let mut victim: Option<(usize, bool)> = None;
        for (j, ff) in self.f.iter().enumerate() {
            if !ff.alive || !self.is_enemy(shooter, j) {
                continue;
            }
            if let Some((ht, _)) = ff.head_box().ray(o, d, t) {
                t = ht;
                victim = Some((j, true));
            }
            if let Some((bt, _)) = ff.body_box().ray(o, d, t) {
                if bt < t || victim.map_or(true, |(vj, _)| vj != j) {
                    t = bt;
                    victim = Some((j, false));
                }
            }
        }
        let end = o + d * t;
        let mut spark = 0u8;
        if let Some((j, head)) = victim {
            let gs = w.gun();
            let falloff = match w {
                Weapon::Shotgun => (1.0 - (t - 8.0).max(0.0) / 22.0).max(0.25),
                Weapon::Smg => (1.0 - (t - 20.0).max(0.0) / 60.0).max(0.5),
                _ => 1.0,
            };
            let mut amount = dmg * falloff * if head { gs.head } else { 1.0 };
            let v = &self.f[j];
            let blocked = v.guard && v.fwd().dot(-d) > 0.45;
            if blocked {
                amount *= 0.12;
                spark = 2;
            } else {
                spark = 1;
            }
            let knock = d.flat().norm() * gs.knock;
            self.damage(shooter, j, amount, knock, 0.0, head && !blocked, end, w as u8, o, d);
        } else if hit_world {
            spark = 3;
        }
        let color = if shooter == self.local { [255, 230, 150] } else { [255, 170, 120] };
        self.tracers.push(Tracer { a: o, b: end, life: 0.09, color });
        self.impact_fx(end, spark);
        if self.kind == Kind::Host || self.kind == Kind::Client {
            self.out_events.push(Ev::Shot { o, e: end, w: w as u8, spark });
        }
    }

    pub fn impact_fx(&mut self, p: V3, spark: u8) {
        match spark {
            1 => self.burst(p, [220, 40, 50], 8, 3.0),
            2 => {
                self.burst(p, [255, 255, 200], 10, 5.0);
                self.sfx(Sfx::Clang, p);
            }
            3 => self.burst(p, [200, 190, 160], 5, 2.5),
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
            let hit_at = b.chest();
            if b.guard && facing && !heavy {
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
            let guard_break = b.guard && facing && heavy;
            let dmg = dmg * wpn.melee_mult();
            let kb = kb * if wpn == Weapon::Greatsword { 1.3 } else { 1.0 };
            let knock = d.flat().norm() * kb + v3(0.0, up, 0.0);
            if a.slash_idx == 3 && i == self.local {
                self.say("LAUNCH!".to_string());
            }
            self.damage(i, j, if guard_break { dmg * 0.5 } else { dmg }, knock, if guard_break { 0.9 } else { stun }, false, hit_at, wpn as u8, a.chest(), d.norm());
            self.impact_fx(hit_at, 1);
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
            self.hitmarker = if head { 0.35 } else { 0.22 };
            self.dmg_nums.push(DmgNum { pos: at + v3(0.0, 0.3, 0.0), amount: amount.round() as i32, life: 0.9, head });
            self.sfx(Sfx::Hit, at);
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
            self.apply_hit(victim, attacker, amount, knock, stun, head, w);
        } else {
            if matches!(self.mt.phase, Phase::Countdown | Phase::MatchOver | Phase::Loadout) {
                return;
            }
            let ff = &mut self.f[victim];
            let mut a = amount;
            if ff.ap > 0.0 {
                let absorbed = (a * 0.6).min(ff.ap);
                ff.ap -= absorbed;
                a -= absorbed;
            }
            ff.hp -= a;
            ff.hurt_t = 0.25;
            ff.last_weapon = w;
            if let Some(at) = attacker {
                ff.last_attacker = Some(at);
            }
            self.out_events.push(Ev::Hit { victim: victim as u8, dmg: amount, knock, stun, head, w, o: V3::ZERO, dir: V3::ZERO });
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
    pub fn validate_claim(&self, shooter: usize, victim: usize, w: u8, dmg: f32, o: V3, dir: V3) -> bool {
        if victim >= self.f.len() || !self.f[victim].present || !self.f[victim].alive || !self.is_enemy(shooter, victim) {
            return false;
        }
        let s = &self.f[shooter];
        if !s.alive {
            return false;
        }
        let wpn = Weapon::from_u8(w);
        if w >= 8 || !s.loadout.has(wpn) || wpn == Weapon::Rocket {
            return false; // explosions are worked out by the host itself
        }
        if !(dmg > 0.0) || dmg > wpn.max_hit() * 1.05 {
            return false;
        }
        let rewind = (s.ping_ms as f32 / 1000.0 + 0.06).clamp(0.0, 0.45);
        let t = self.time - rewind;
        let vp = self.pos_at(victim, t);
        let sp_then = self.pos_at(shooter, t);
        let sp_now = s.pos;
        if wpn.is_melee() {
            let reach = wpn.melee_range() + 1.6;
            let ok = |sp: V3| (vp - sp).flat().len() <= reach && (vp.y - sp.y).abs() <= 3.0;
            return ok(sp_then) || ok(sp_now);
        }
        let near = |sp: V3| (o - (sp + v3(0.0, 1.4, 0.0))).len() < 3.5;
        if !(near(sp_then) || near(sp_now)) {
            return false;
        }
        let d = dir.norm();
        if d.len() < 0.5 {
            return false;
        }
        let tol = 0.45;
        let b = Aabb::new(vp + v3(-0.42 - tol, -tol, -0.42 - tol), vp + v3(0.42 + tol, 1.9 + tol, 0.42 + tol));
        b.ray(o, d, wpn.gun().range + 2.0).is_some()
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
        self.sfx(Sfx::Reload, p);
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
    pub fn apply_hit(&mut self, victim: usize, attacker: Option<usize>, amount: f32, knock: V3, stun: f32, _head: bool, w: u8) {
        let ff = &mut self.f[victim];
        if !ff.alive || !ff.present || ff.spawn_prot > 0.0 {
            return;
        }
        if self.mt.phase == Phase::Countdown || self.mt.phase == Phase::MatchOver {
            return;
        }
        ff.last_weapon = w;
        let mut a = amount;
        if ff.ap > 0.0 {
            let absorbed = (a * 0.6).min(ff.ap);
            ff.ap -= absorbed;
            a -= absorbed;
        }
        ff.hp -= a;
        ff.hurt_t = 0.25;
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
            self.hurt_flash = 0.5;
            self.shake = self.shake.max(0.35);
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
        if (lp - p).len() < 14.0 {
            self.shake = self.shake.max(0.8);
        }
        // damage is worked out by the host (or this computer when playing alone)
        if !self.is_authority() {
            return;
        }
        let (radius, base, extra, w) = if kind == 1 { (4.5, 22.0, 58.0, Weapon::Rocket as u8) } else { (4.8, 18.0, 42.0, 8u8) };
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
                let mut dmg = base + extra * k;
                let mut knock = (c - p).norm() * (6.0 + 8.0 * k) + v3(0.0, 4.0 * k + 2.0, 0.0);
                if j == owner {
                    // your own blast hurts less and throws you further (rocket jump)
                    dmg *= 0.35;
                    knock = knock * 1.5 + v3(0.0, 3.0, 0.0);
                }
                if owner == self.local && j != self.local {
                    self.hitmarker = 0.25;
                    self.dmg_nums.push(DmgNum { pos: c + v3(0.0, 0.5, 0.0), amount: dmg.round() as i32, life: 0.9, head: false });
                }
                self.authority_hit(j, Some(owner), dmg, knock, 0.3, false, w);
            }
        }
    }
    fn update_fx(&mut self, dt: f32) {
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
        self.hurt_flash = (self.hurt_flash - dt).max(0.0);
        self.shake = (self.shake - dt * 2.5).max(0.0);
    }

    // ------------------------------------------------------------------ network glue

    pub fn handle_remote_event(&mut self, from: usize, ev: Ev) {
        match ev {
            Ev::Shot { o, e, w, spark } => {
                if w != 0 {
                    self.tracers.push(Tracer { a: o, b: e, life: 0.09, color: [255, 170, 120] });
                    for _ in 0..4 {
                        let v = (e - o).norm() * self.rng.range(2.0, 5.0);
                        self.particles.push(Particle { pos: o, vel: v, life: 0.08, max_life: 0.08, size: 0.18, color: [255, 220, 120], gravity: 0.0 });
                    }
                    self.sfx(if w == 1 { Sfx::Shotgun } else { Sfx::Revolver }, o);
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
                    if from != 0 && self.validate_claim(from, v, w, dmg, o, dir) {
                        self.authority_hit(v, Some(from), dmg, knock, stun, head, w);
                    }
                } else if from == 0 && v == self.local && self.f[v].owned {
                    // the host confirmed we were hit
                    self.apply_hit(v, None, dmg, knock, stun, head, w);
                }
            }
            Ev::Grenade { p, v, kind } => {
                self.grenades.push(Grenade { pos: p, vel: v, fuse: if kind == 1 { 4.0 } else { 1.7 }, owner: from, kind });
            }
            Ev::Died { victim, killer, w } => {
                let v = victim as usize;
                if self.kind == Kind::Client && from == 0 && v < self.f.len() && self.f[v].present {
                    self.f[v].alive = false;
                    self.f[v].death_t = 0.0;
                    if v == self.local {
                        self.f[v].respawn_t = RESPAWN_TIME;
                        self.f[v].wall = None;
                    }
                    let k = if (killer as usize) < self.f.len() { Some(killer as usize) } else { None };
                    self.on_death(v, k, w);
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

    #[test]
    fn host_checks_hits_against_the_past() {
        let mut g = Game::offline("A", 1, 0.5, Mode::Ffa, 50);
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
            g.f[1].pos = v3(k as f32 * 0.1, 0.0, 0.0);
        }
        g.f[0].ping_ms = 200;
        let o = g.f[0].pos + v3(0.0, 1.4, 0.0);
        // where the target was about a quarter second ago (x ~ 3.4), not where it is now (x ~ 5.9)
        let past = g.pos_at(1, g.time - 0.26);
        assert!(past.x < 4.5, "history should remember the past, got {:?}", past);
        let dir_past = (past + v3(0.0, 1.0, 0.0) - o).norm();
        let dir_now = (g.f[1].pos + v3(0.0, 1.0, 0.0) - o).norm();
        assert!(g.validate_claim(0, 1, Weapon::Rifle as u8, 48.0, o, dir_past), "a hit on what the shooter saw counts");
        assert!(!g.validate_claim(0, 1, Weapon::Rifle as u8, 48.0, o, v3(-1.0, 0.0, 0.0)), "a miss is thrown out");
        assert!(!g.validate_claim(0, 1, Weapon::Rifle as u8, 500.0, o, dir_past), "impossible damage is thrown out");
        assert!(!g.validate_claim(0, 1, Weapon::Smg as u8, 9.0, o, dir_past), "a gun you don't carry is thrown out");
        assert!(!g.validate_claim(0, 1, Weapon::Rifle as u8, 48.0, o + v3(30.0, 0.0, 0.0), dir_now), "shots from far away are thrown out");
    }
}