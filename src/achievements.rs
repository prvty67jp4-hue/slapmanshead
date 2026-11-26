//! Achievements and lifetime stats.
//!
//! Fifty-six achievements across the duels, the four fighters, every execution, Ranked 1v1, the
//! Ashen Vault and Varrow Centre, a few of them hidden until earned. They're kept here on this
//! computer (and in Steam Cloud with the other saves), shown on the Achievements page with
//! progress bars, announced with a toast, and sent to Steam once the game runs under its own App
//! ID (crate::platform::steam_api).
//!
//! What counts is fed in from the game as it happens ([`Record::feed`] and friends) into the
//! record's own copy of the career, never read back from saves an admin command can change: a
//! session that used admin commands (crate::roles) feeds nothing and unlocks nothing.
//!
//! The Steamworks side is written from here too: `steam/achievements.vdf` (the stats and
//! achievements, in the schema's layout, with each achievement's progress stat) and the icons,
//! drawn from the game's own weapon pictures, wardrobe looks and rank colours (`steam/icons`).

use crate::career::{self, Career};
use crate::game::CareerEv;
use crate::platform::steam_api::SteamApi;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The page's sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Duels,
    Fighters,
    Executions,
    Ranked,
    Vault,
    Varrow,
}

impl Group {
    pub const ALL: [Group; 6] = [Group::Duels, Group::Fighters, Group::Executions, Group::Ranked, Group::Vault, Group::Varrow];
    pub fn title(self) -> &'static str {
        match self {
            Group::Duels => "The Duelling Rolls",
            Group::Fighters => "Masters of the Four",
            Group::Executions => "The Headsman's Book",
            Group::Ranked => "The Ranked Rolls",
            Group::Vault => "The Ashen Vault",
            Group::Varrow => "Varrow Centre",
        }
    }
    pub fn colour(self) -> [u8; 3] {
        match self {
            Group::Duels => [255, 140, 60],
            Group::Fighters => [255, 200, 70],
            Group::Executions => [220, 64, 60],
            Group::Ranked => [190, 120, 255],
            Group::Vault => [235, 96, 70],
            Group::Varrow => [90, 200, 190],
        }
    }
}

/// What earns it (and what its progress bar counts).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Wins(u32),
    Kills(u32),
    Headshots(u32),
    /// Headshot kills in a row, without dying or a body-shot kill between.
    HeadStreak(u32),
    Parries(u32),
    Executions(u32),
    Writs(u32),
    /// Kills chained in quick succession.
    Chain(u32),
    QuietWin,
    Mastery(u8),
    /// One execution (crate::exec::EXECUTIONS index).
    Exec(u8),
    Placed,
    /// Reach this rank tier (crate::ranked::TIERS).
    Tier(u8),
    LedgerTop,
    /// A vault boss, by its room (0 the Warden ... 4 the Magistrate).
    Boss(u8),
    VaultClear,
    SealedWrit,
    /// Clear the vault within this many seconds.
    VaultUnder(u32),
    Wipes(u32),
    Rooftop,
    /// The Rooftop Run under this many milliseconds.
    RooftopUnder(u32),
    ChessHard,
    PitWins(u32),
    Cosmetics(u32),
    /// A secret (a bit of Life::secrets).
    Secret(u8),
}

/// What its icon is drawn from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// A weapon's picture (crate::weapons::icon_png).
    Weapon(&'static str),
    /// A wardrobe look, by its place in crate::hub::LOOKS.
    Look(usize),
    /// A vault boss's best drop.
    Boss(u8),
    /// A rank badge of this tier.
    Rank(u8),
    /// A wax seal.
    Seal,
}

pub struct Def {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub group: Group,
    pub hidden: bool,
    pub goal: Goal,
    pub icon: Icon,
}

/// Secrets (bits of Life::secrets).
pub const SECRET_MIRROR: u8 = 0;
pub const SECRET_PHOTO: u8 = 1;
pub const SECRET_STANDS: u8 = 2;

/// The Rooftop Run target time.
pub const ROOFTOP_TARGET_MS: u32 = 45_000;
/// The vault speedrun targets.
pub const VAULT_FAST_S: u32 = 20 * 60;
pub const VAULT_FASTER_S: u32 = 12 * 60;

const fn d(id: &'static str, name: &'static str, desc: &'static str, group: Group, goal: Goal, icon: Icon) -> Def {
    Def { id, name, desc, group, hidden: false, goal, icon }
}
const fn h(id: &'static str, name: &'static str, desc: &'static str, group: Group, goal: Goal, icon: Icon) -> Def {
    Def { id, name, desc, group, hidden: true, goal, icon }
}

use Goal as G;
use Group::*;
use Icon as I;

/// Every achievement, in the page's order.
pub const ALL: [Def; 56] = [
    // ---- the duels
    d("win_first", "First Entry in the Ledger", "Win your first match. The clerks will spell your name wrong, but it's in ink now.", Duels, G::Wins(1), I::Seal),
    d("win_10", "Ten Debts Settled", "Win 10 matches.", Duels, G::Wins(10), I::Seal),
    d("win_100", "A Hundred Writs Answered", "Win 100 matches. Varrow's taverns have started arguing about you.", Duels, G::Wins(100), I::Seal),
    d("win_1000", "The Thousandfold Seal", "Win 1,000 matches. There's no wax left in the city without your name pressed into it.", Duels, G::Wins(1000), I::Seal),
    d("kills_1000", "Ink and Iron", "Take down 1,000 opponents.", Duels, G::Kills(1000), I::Weapon("rifle")),
    d("heads_500", "Crown Collector", "Land 500 headshot kills.", Duels, G::Headshots(500), I::Weapon("sniper")),
    d("head_streak_3", "Three Crowns Running", "Land 3 headshot kills in a row without dying.", Duels, G::HeadStreak(3), I::Weapon("revolver")),
    d("head_streak_7", "The Lanternman's Eye", "Land 7 headshot kills in a row without dying. Nobody on the walls aims like that.", Duels, G::HeadStreak(7), I::Weapon("sniper")),
    d("parry_100", "Steel Says No", "Perfectly parry 100 blows. The Court calls it insolence; the pits call it art.", Duels, G::Parries(100), I::Weapon("katana")),
    d("exec_100", "The Headsman's Tally", "Perform 100 executions.", Duels, G::Executions(100), I::Weapon("greatsword")),
    d("writs_25", "Writ-Breaker", "Claim 25 Writs from fighters on a streak.", Duels, G::Writs(25), I::Seal),
    d("rampage", "Rampage in the Rows", "Chain 5 eliminations in quick succession.", Duels, G::Chain(5), I::Weapon("smg")),
    h("quiet_win", "The Quiet Ledger", "Win a match without firing a single shot.", Duels, G::QuietWin, I::Weapon("dagger")),
    // ---- the four fighters
    d("master_corvin", "Warden of the Border", "Reach mastery 20 with Corvin Vale.", Fighters, G::Mastery(0), I::Weapon("katana")),
    d("master_sable", "Every Debt Collected", "Reach mastery 20 with Sable Rook.", Fighters, G::Mastery(1), I::Weapon("rigger_hook")),
    d("master_bram", "The Anvil Does Not Bend", "Reach mastery 20 with Bram \"Anvil\" Kask.", Fighters, G::Mastery(2), I::Weapon("greatsword")),
    d("master_ilya", "Light Over the Lower Ward", "Reach mastery 20 with Ilya Fen.", Fighters, G::Mastery(3), I::Weapon("dagger")),
    // ---- every execution
    d("exec_wardens_verdict", "The Verdict Stands", "Perform the Warden's Verdict execution.", Executions, G::Exec(0), I::Weapon("katana")),
    d("exec_border_ledger", "Struck from the Border Ledger", "Perform the Border Ledger execution.", Executions, G::Exec(1), I::Weapon("rifle")),
    d("exec_lantern_watch", "Lantern Out at Midnight", "Perform the Lantern Watch execution.", Executions, G::Exec(2), I::Weapon("sniper")),
    d("exec_ridge_fall", "Over the Ridge", "Perform the Ridge Fall execution.", Executions, G::Exec(3), I::Weapon("katana")),
    d("exec_the_collection", "Added to the Collection", "Perform The Collection execution.", Executions, G::Exec(4), I::Weapon("revolver")),
    d("exec_bounty_closed", "Bounty Paid in Full", "Perform the Bounty Closed execution.", Executions, G::Exec(5), I::Weapon("hand_cannon")),
    d("exec_frozen_lake", "Through the Ice", "Perform the Frozen Lake execution.", Executions, G::Exec(6), I::Weapon("shuriken")),
    d("exec_hook_and_line", "Reeled In", "Perform the Hook and Line execution.", Executions, G::Exec(7), I::Weapon("rigger_hook")),
    d("exec_hammer_falls", "When the Hammer Falls", "Perform the Hammer Falls execution.", Executions, G::Exec(8), I::Weapon("greatsword")),
    d("exec_quench", "Quenched", "Perform the Quench execution.", Executions, G::Exec(9), I::Weapon("greatsword")),
    d("exec_foundry_bell", "The Bell Tolls Once", "Perform the Foundry Bell execution.", Executions, G::Exec(10), I::Weapon("scattergun")),
    d("exec_anvil_drop", "Flattened on the Anvil", "Perform the Anvil Drop execution.", Executions, G::Exec(11), I::Weapon("greatsword")),
    d("exec_lights_out", "Lights Out, Varrow", "Perform the Lights Out execution.", Executions, G::Exec(12), I::Weapon("dagger")),
    d("exec_rooftop_run", "Off the Rooftops", "Perform the Rooftop Run execution.", Executions, G::Exec(13), I::Weapon("shuriken")),
    d("exec_lower_ward_lightning", "Lightning in the Lower Ward", "Perform the Lower Ward Lightning execution.", Executions, G::Exec(14), I::Weapon("smg")),
    d("exec_skylight", "Through the Skylight", "Perform the Skylight execution.", Executions, G::Exec(15), I::Weapon("dagger")),
    // ---- Ranked 1v1
    d("rank_placed", "Entered in the Rolls", "Finish your placement matches in Ranked 1v1.", Ranked, G::Placed, I::Rank(0)),
    d("rank_silver", "Sealed in Silver", "Reach Silver Writ in Ranked 1v1.", Ranked, G::Tier(3), I::Rank(3)),
    d("rank_gold", "The Gilded Seal", "Reach Gold Writ in Ranked 1v1.", Ranked, G::Tier(4), I::Rank(4)),
    d("rank_magistrate", "Robes of the Magistrate", "Reach Magistrate in Ranked 1v1.", Ranked, G::Tier(6), I::Rank(6)),
    d("rank_ledger", "Ledger of Champions", "Stand among the top 100 on the Ranked 1v1 board.", Ranked, G::LedgerTop, I::Rank(7)),
    // ---- the Ashen Vault
    d("boss_warden", "The Wax Hall Falls Silent", "Defeat the Warden in the Wax Hall.", Vault, G::Boss(0), I::Boss(0)),
    d("boss_assessors", "Both Judges Overruled", "Defeat the Twin Assessors.", Vault, G::Boss(1), I::Boss(1)),
    d("boss_ledger", "The Chained Ledger Closes", "Defeat the Chained Ledger.", Vault, G::Boss(2), I::Boss(2)),
    d("boss_regent", "Red Moon Setting", "Defeat the Ashblood Regent, Lord Vesper Kael, on the vault's roof.", Vault, G::Boss(3), I::Weapon("ashblood_glaive")),
    d("boss_magistrate", "The Magistrate Is Overruled", "Defeat the Ashen Magistrate.", Vault, G::Boss(4), I::Boss(4)),
    d("vault_clear", "Vaultbreaker", "Clear the Ashen Vault.", Vault, G::VaultClear, I::Seal),
    d("sealed_writ", "Under the Sealed Writ", "Clear the Ashen Vault on Sealed Writ.", Vault, G::SealedWrit, I::Seal),
    d("vault_fast", "Before the Ink Dries", "Clear the Ashen Vault in under 20 minutes.", Vault, G::VaultUnder(VAULT_FAST_S), I::Boss(3)),
    d("vault_faster", "Audit at a Gallop", "Clear the Ashen Vault in under 12 minutes.", Vault, G::VaultUnder(VAULT_FASTER_S), I::Weapon("ashblood_glaive")),
    h("vault_wipes", "Ashes, Ashes", "Fall with your whole party 10 times in the vault. The Court keeps a ledger of that too.", Vault, G::Wipes(10), I::Boss(2)),
    // ---- Varrow Centre
    d("rooftop", "Over the Tiles", "Finish the Rooftop Run.", Varrow, G::Rooftop, I::Look(15)),
    d("rooftop_target", "Gutter Runner", "Finish the Rooftop Run in under 45 seconds.", Varrow, G::RooftopUnder(ROOFTOP_TARGET_MS), I::Look(16)),
    d("chess_hard", "Checkmate the Clerk", "Win a game of chess against the Hard bot at the Varrow tables.", Varrow, G::ChessHard, I::Seal),
    d("pit_10", "Pit Regular", "Win 10 duels in the Court pits.", Varrow, G::PitWins(10), I::Weapon("katana")),
    d("fashion_50", "The Varrow Wardrobe", "Own 50 cosmetics. The tailors on Lantern Row know you by name.", Varrow, G::Cosmetics(50), I::Look(8)),
    h("secret_mirror", "Who Owes Whom", "Admire yourself in the wardrobe mirror.", Varrow, G::Secret(SECRET_MIRROR), I::Look(11)),
    h("secret_photo", "For the Broadsheet", "Take a photo in Varrow Centre.", Varrow, G::Secret(SECRET_PHOTO), I::Look(10)),
    h("secret_stands", "From the Gallery", "Watch a match from the stands.", Varrow, G::Secret(SECRET_STANDS), I::Seal),
];

/// Steam's name for an achievement.
pub fn api_name(id: &str) -> String {
    format!("ACH_{}", id.to_ascii_uppercase())
}

/// Lifetime stats sent to Steam (and shown on the Stats page): API name, what it is.
pub const STATS: [(&str, &str); 12] = [
    ("STAT_WINS", "Matches won"),
    ("STAT_MATCHES", "Matches played"),
    ("STAT_KILLS", "Eliminations"),
    ("STAT_HEADSHOTS", "Headshot kills"),
    ("STAT_PARRIES", "Perfect parries"),
    ("STAT_EXECUTIONS", "Executions"),
    ("STAT_WRITS", "Writs claimed"),
    ("STAT_HEAD_STREAK", "Best headshot streak"),
    ("STAT_BOSSES", "Vault bosses felled"),
    ("STAT_RAID_CLEARS", "Vault clears"),
    ("STAT_ROOFTOP_MS", "Best Rooftop Run (ms)"),
    ("STAT_COSMETICS", "Cosmetics owned"),
];

/// Counters the career doesn't keep.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Life {
    pub parries: u32,
    pub head_run: u32,
    pub head_best: u32,
    pub chain_best: u32,
    pub quiet_wins: u32,
    /// Vault bosses beaten (a bit per room), boss kills in all, clears, hard clears, best clear
    /// (ms), full-party wipes.
    pub bosses: u8,
    pub boss_kills: u32,
    pub clears: u32,
    pub hard_clears: u32,
    pub best_ms: u32,
    pub wipes: u32,
    pub rooftop_runs: u32,
    pub rooftop_ms: u32,
    pub chess_hard: u32,
    pub table_wins: u32,
    pub pit_wins: u32,
    /// Ranked: placement games played (up to 5), the best tier reached + 1 (0 none), on the
    /// Ledger of Champions.
    pub placed: u32,
    pub tier: u8,
    pub ledger: bool,
    pub secrets: u8,
    /// Cosmetics owned (less any an admin session gave), and those given in admin sessions.
    pub cosmetics: u32,
    pub tainted: u32,
}

impl Life {
    fn fields(&mut self) -> [&mut u32; 18] {
        [
            &mut self.parries,
            &mut self.head_run,
            &mut self.head_best,
            &mut self.chain_best,
            &mut self.quiet_wins,
            &mut self.boss_kills,
            &mut self.clears,
            &mut self.hard_clears,
            &mut self.best_ms,
            &mut self.wipes,
            &mut self.rooftop_runs,
            &mut self.rooftop_ms,
            &mut self.chess_hard,
            &mut self.table_wins,
            &mut self.pit_wins,
            &mut self.placed,
            &mut self.cosmetics,
            &mut self.tainted,
        ]
    }
}

/// How long a toast stays up (seconds).
pub const TOAST_TIME: f32 = 5.5;

/// This computer's achievements: what's unlocked (and when), the counters, the toasts to show.
#[derive(Clone, Debug, Default)]
pub struct Record {
    /// The career as counted for achievements (fed only from sessions that count).
    pub clean: Career,
    pub life: Life,
    /// Unlocked: id → when (seconds since 1970).
    pub unlocked: BTreeMap<String, u64>,
    /// Toasts waiting or showing: (index in ALL, seconds shown).
    pub toasts: Vec<(usize, f32)>,
    pub dirty: bool,
    pub stats_dirty: bool,
    /// Cosmetics owned when an admin session began (what it adds is set aside).
    gate_base: Option<u32>,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

pub fn path() -> Option<PathBuf> {
    crate::platform::data_file("achievements.txt")
}

impl Record {
    /// Load the record; the first time, it starts from the career so far.
    pub fn load(career: &Career) -> Record {
        match path().and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(t) => Record::from_text(&t),
            None => Record::seeded(career),
        }
    }

    pub fn seeded(career: &Career) -> Record {
        let mut clean = career.clone();
        clean.notes.clear();
        Record { clean, dirty: true, stats_dirty: true, ..Default::default() }
    }

    pub fn save(&mut self) {
        if let Some(p) = path() {
            let _ = crate::platform::save_file(&p, &self.to_text());
        }
        self.dirty = false;
    }

    pub fn to_text(&self) -> String {
        let mut s = String::from("v=1\n");
        s += &format!("unlocked={}\n", self.unlocked.iter().map(|(k, t)| format!("{k}:{t}")).collect::<Vec<_>>().join(","));
        let mut l = self.life.clone();
        s += &format!("life={}\n", l.fields().iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","));
        s += &format!("flags={},{},{},{}\n", self.life.bosses, self.life.tier, self.life.ledger as u8, self.life.secrets);
        for line in self.clean.to_text().lines() {
            s += &format!("c.{line}\n");
        }
        s
    }

    pub fn from_text(t: &str) -> Record {
        let mut r = Record::default();
        let mut career = String::new();
        for line in t.lines() {
            if let Some(c) = line.strip_prefix("c.") {
                career += c;
                career.push('\n');
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            match k {
                "unlocked" => {
                    for e in v.split(',').filter(|e| !e.is_empty()) {
                        let (id, when) = e.split_once(':').unwrap_or((e, "0"));
                        if ALL.iter().any(|d| d.id == id) {
                            r.unlocked.insert(id.to_string(), when.parse().unwrap_or(0));
                        }
                    }
                }
                "life" => {
                    let nums: Vec<u32> = v.split(',').map(|x| x.trim().parse().unwrap_or(0)).collect();
                    for (slot, n) in r.life.fields().into_iter().zip(nums) {
                        *slot = n;
                    }
                }
                "flags" => {
                    let f: Vec<u8> = v.split(',').map(|x| x.trim().parse().unwrap_or(0)).collect();
                    r.life.bosses = f.first().copied().unwrap_or(0);
                    r.life.tier = f.get(1).copied().unwrap_or(0);
                    r.life.ledger = f.get(2) == Some(&1);
                    r.life.secrets = f.get(3).copied().unwrap_or(0);
                }
                _ => {}
            }
        }
        r.clean = Career::from_text(&career);
        r
    }

    /// Count something that happened (only from a session that counts).
    pub fn feed(&mut self, ev: CareerEv, body: Option<u8>, day: u32) {
        let l = &mut self.life;
        match ev {
            CareerEv::Kill { head, .. } => {
                if head {
                    l.head_run += 1;
                    l.head_best = l.head_best.max(l.head_run);
                } else {
                    l.head_run = 0;
                }
            }
            CareerEv::Death => l.head_run = 0,
            CareerEv::Chain(n) => l.chain_best = l.chain_best.max(n),
            CareerEv::Parry => l.parries += 1,
            CareerEv::Rooftop { ms } => {
                l.rooftop_runs += 1;
                if l.rooftop_ms == 0 || ms < l.rooftop_ms {
                    l.rooftop_ms = ms;
                }
            }
            CareerEv::PitWin => l.pit_wins += 1,
            CareerEv::TableWin { kind, hard } => {
                l.table_wins += 1;
                if hard && kind == crate::boards::GameKind::Chess as u8 {
                    l.chess_hard += 1;
                }
            }
            _ => {}
        }
        self.clean.apply_as(ev, body, day);
        self.clean.notes.clear();
        self.dirty = true;
        self.stats_dirty = true;
    }

    /// A match won without firing a shot.
    pub fn quiet_win(&mut self) {
        self.life.quiet_wins += 1;
        self.dirty = true;
    }
    /// A vault boss beaten (its room).
    pub fn boss_down(&mut self, room: u8) {
        if room < 8 {
            self.life.bosses |= 1 << room;
        }
        self.life.boss_kills += 1;
        self.dirty = true;
        self.stats_dirty = true;
    }
    pub fn vault_cleared(&mut self, ms: u32, hard: bool) {
        let l = &mut self.life;
        l.clears += 1;
        if hard {
            l.hard_clears += 1;
        }
        if ms > 0 && (l.best_ms == 0 || ms < l.best_ms) {
            l.best_ms = ms;
        }
        self.dirty = true;
        self.stats_dirty = true;
    }
    pub fn wipe(&mut self) {
        self.life.wipes += 1;
        self.dirty = true;
    }
    pub fn secret(&mut self, bit: u8) {
        if self.life.secrets & (1 << bit) == 0 {
            self.life.secrets |= 1 << bit;
            self.dirty = true;
        }
    }
    /// The ranked record: placement games, the rank tier now (None in placement), the board place.
    pub fn ranked_seen(&mut self, games: u32, tier: Option<u8>, board: u32) {
        let l = &mut self.life;
        let placed = games.min(crate::ranked::PLACEMENTS);
        let t = tier.map_or(0, |t| t + 1).max(l.tier);
        let ledger = l.ledger || tier == Some(crate::ranked::LEDGER) || (board > 0 && board <= crate::ranked::CHAMPIONS && tier.is_some_and(|t| t >= 6));
        if placed > l.placed || t != l.tier || ledger != l.ledger {
            l.placed = l.placed.max(placed);
            l.tier = t;
            l.ledger = ledger;
            self.dirty = true;
        }
    }
    /// Cosmetics owned now; what a session with admin commands adds doesn't count.
    pub fn cosmetics_seen(&mut self, owned: u32, gated: bool) {
        if gated {
            let base = *self.gate_base.get_or_insert(owned);
            if owned > base {
                self.life.tainted += owned - base;
                self.gate_base = Some(owned);
                self.dirty = true;
            }
            return;
        }
        self.gate_base = None;
        let clean = owned.saturating_sub(self.life.tainted);
        if clean != self.life.cosmetics {
            self.life.cosmetics = clean;
            self.dirty = true;
            self.stats_dirty = true;
        }
    }

    /// How far along an achievement is: (done, needed).
    pub fn progress(&self, g: Goal) -> (u32, u32) {
        let (c, l) = (&self.clean, &self.life);
        let flag = |b: bool| (b as u32, 1);
        let (cur, goal) = match g {
            G::Wins(n) => (c.wins, n),
            G::Kills(n) => (c.kills, n),
            G::Headshots(n) => (c.headshots, n),
            G::HeadStreak(n) => (l.head_best, n),
            G::Parries(n) => (l.parries, n),
            G::Executions(n) => (c.finishers, n),
            G::Writs(n) => (c.writs, n),
            G::Chain(n) => (l.chain_best, n),
            G::QuietWin => flag(l.quiet_wins > 0),
            G::Mastery(b) => (c.mastery_level(b), career::MASTERY_MAX),
            G::Exec(k) => flag(c.fin_by.get(k as usize).is_some_and(|&n| n > 0)),
            G::Placed => (l.placed, crate::ranked::PLACEMENTS),
            G::Tier(t) => flag(l.tier > t),
            G::LedgerTop => flag(l.ledger),
            G::Boss(r) => flag(l.bosses & (1 << r) != 0),
            G::VaultClear => flag(l.clears > 0),
            G::SealedWrit => flag(l.hard_clears > 0),
            G::VaultUnder(s) => flag(l.best_ms != 0 && l.best_ms <= s * 1000),
            G::Wipes(n) => (l.wipes, n),
            G::Rooftop => flag(l.rooftop_runs > 0),
            G::RooftopUnder(ms) => flag(l.rooftop_ms != 0 && l.rooftop_ms <= ms),
            G::ChessHard => flag(l.chess_hard > 0),
            G::PitWins(n) => (l.pit_wins, n),
            G::Cosmetics(n) => (l.cosmetics, n),
            G::Secret(b) => flag(l.secrets & (1 << b) != 0),
        };
        (cur.min(goal), goal)
    }

    pub fn is_unlocked(&self, id: &str) -> bool {
        self.unlocked.contains_key(id)
    }

    /// Unlock what's been earned: the new ones (their place in ALL), each toasted and sent to
    /// Steam. A session that used admin commands unlocks nothing.
    pub fn check(&mut self, api: &mut dyn SteamApi, gated: bool) -> Vec<usize> {
        if gated {
            return Vec::new();
        }
        let mut new = Vec::new();
        for (k, def) in ALL.iter().enumerate() {
            if self.unlocked.contains_key(def.id) {
                continue;
            }
            let (cur, goal) = self.progress(def.goal);
            if cur >= goal {
                self.unlocked.insert(def.id.to_string(), now_secs());
                self.toasts.push((k, 0.0));
                api.ach_set(&api_name(def.id));
                new.push(k);
            }
        }
        if !new.is_empty() {
            self.dirty = true;
            self.push_stats(api);
        }
        new
    }

    /// The lifetime stats' values.
    pub fn stat_values(&self) -> [(&'static str, i32); 12] {
        let (c, l) = (&self.clean, &self.life);
        let v = |n: u32| n.min(i32::MAX as u32) as i32;
        [
            (STATS[0].0, v(c.wins)),
            (STATS[1].0, v(c.matches)),
            (STATS[2].0, v(c.kills)),
            (STATS[3].0, v(c.headshots)),
            (STATS[4].0, v(l.parries)),
            (STATS[5].0, v(c.finishers)),
            (STATS[6].0, v(c.writs)),
            (STATS[7].0, v(l.head_best)),
            (STATS[8].0, v(l.boss_kills)),
            (STATS[9].0, v(l.clears)),
            (STATS[10].0, v(l.rooftop_ms)),
            (STATS[11].0, v(l.cosmetics)),
        ]
    }

    /// Send the stats (and anything unlocked) to Steam; nothing happens where stats are off.
    pub fn push_stats(&mut self, api: &mut dyn SteamApi) -> bool {
        let f = api.features();
        if !f.stats && !f.achievements {
            return false;
        }
        if f.stats {
            for (name, v) in self.stat_values() {
                api.stat_set(name, v);
            }
        }
        self.stats_dirty = false;
        api.store()
    }

    /// Line this computer's record up with Steam's (another computer may have unlocked some; some
    /// here may have been earned before the game had its own App ID): both end up with all.
    pub fn reconcile(&mut self, api: &mut dyn SteamApi) -> (usize, usize) {
        if !api.features().achievements {
            return (0, 0);
        }
        let (mut from_steam, mut to_steam) = (0, 0);
        for def in ALL.iter() {
            let name = api_name(def.id);
            match (api.ach_get(&name), self.unlocked.contains_key(def.id)) {
                (Some(true), false) => {
                    self.unlocked.insert(def.id.to_string(), now_secs());
                    from_steam += 1;
                }
                (Some(false), true) => {
                    api.ach_set(&name);
                    to_steam += 1;
                }
                _ => {}
            }
        }
        if from_steam > 0 {
            self.dirty = true;
        }
        self.push_stats(api);
        (from_steam, to_steam)
    }

    /// Toasts: age them, drop the finished one; the one showing (index in ALL, seconds shown).
    pub fn toast_tick(&mut self, dt: f32) -> Option<(usize, f32)> {
        if let Some(t) = self.toasts.first_mut() {
            t.1 += dt;
            if t.1 >= TOAST_TIME {
                self.toasts.remove(0);
            }
        }
        self.toasts.first().copied()
    }
}

/// Cosmetics owned (looks other than the starter ones, vault drops), from the hub's record.
pub fn cosmetics_owned(e: &crate::hub::Earned) -> u32 {
    let looks = crate::hub::LOOKS.iter().filter(|l| l.unlock != crate::hub::Unlock::Always && e.look_owned(l)).count();
    let loot = (0..crate::raid::LOOT.len() as u8).filter(|&l| !matches!(crate::raid::LOOT[l as usize].kind, crate::raid::LootKind::Look(_)) && e.loot_owned(l)).count();
    (looks + loot) as u32
}

// ================================================================================ Steamworks files

/// steam/achievements.vdf: the stats and achievements in Steam's schema layout (one block of
/// stats, one of achievement bits), with each achievement's progress stat where it counts one.
pub fn vdf(app: u32) -> String {
    let q = |s: &str| s.replace('"', "\\\"");
    let mut s = format!("\"{app}\"\n{{\n\t\"gamename\"\t\t\"Duel Rush\"\n\t\"version\"\t\t\"1\"\n\t\"stats\"\n\t{{\n");
    for (k, (name, title)) in STATS.iter().enumerate() {
        let incr = if *name == "STAT_ROOFTOP_MS" || *name == "STAT_HEAD_STREAK" || *name == "STAT_COSMETICS" { "0" } else { "1" };
        s += &format!(
            "\t\t\"{}\"\n\t\t{{\n\t\t\t\"bits\"\n\t\t\t{{\n\t\t\t}}\n\t\t\t\"type\"\t\t\"INT\"\n\t\t\t\"id\"\t\t\"{}\"\n\t\t\t\"name\"\t\t\"{name}\"\n\t\t\t\"display\"\n\t\t\t{{\n\t\t\t\t\"name\"\t\t\"{title}\"\n\t\t\t}}\n\t\t\t\"min\"\t\t\"0\"\n\t\t\t\"incrementonly\"\t\t\"{incr}\"\n\t\t\t\"permission\"\t\t\"2\"\n\t\t}}\n",
            k + 1,
            k + 1
        );
    }
    s += &format!("\t\t\"{}\"\n\t\t{{\n\t\t\t\"type\"\t\t\"ACHIEVEMENTS\"\n\t\t\t\"id\"\t\t\"{}\"\n\t\t\t\"bits\"\n\t\t\t{{\n", STATS.len() + 1, STATS.len() + 1);
    for (k, def) in ALL.iter().enumerate() {
        let api = api_name(def.id);
        let progress = match progress_stat(def.goal) {
            Some((stat, max)) => format!("\t\t\t\t\t\"progress\"\n\t\t\t\t\t{{\n\t\t\t\t\t\t\"value\"\n\t\t\t\t\t\t{{\n\t\t\t\t\t\t\t\"operation\"\t\t\"statvalue\"\n\t\t\t\t\t\t\t\"operand1\"\t\t\"{stat}\"\n\t\t\t\t\t\t}}\n\t\t\t\t\t\t\"min_val\"\t\t\"0\"\n\t\t\t\t\t\t\"max_val\"\t\t\"{max}\"\n\t\t\t\t\t}}\n"),
            None => String::new(),
        };
        s += &format!(
            "\t\t\t\t\"{k}\"\n\t\t\t\t{{\n\t\t\t\t\t\"name\"\t\t\"{api}\"\n\t\t\t\t\t\"display\"\n\t\t\t\t\t{{\n\t\t\t\t\t\t\"name\"\n\t\t\t\t\t\t{{\n\t\t\t\t\t\t\t\"english\"\t\t\"{}\"\n\t\t\t\t\t\t\t\"token\"\t\t\"{api}_NAME\"\n\t\t\t\t\t\t}}\n\t\t\t\t\t\t\"desc\"\n\t\t\t\t\t\t{{\n\t\t\t\t\t\t\t\"english\"\t\t\"{}\"\n\t\t\t\t\t\t\t\"token\"\t\t\"{api}_DESC\"\n\t\t\t\t\t\t}}\n\t\t\t\t\t\t\"hidden\"\t\t\"{}\"\n\t\t\t\t\t\t\"icon\"\t\t\"icons/{}.png\"\n\t\t\t\t\t\t\"icon_gray\"\t\t\"icons/{}_locked.png\"\n\t\t\t\t\t}}\n{progress}\t\t\t\t}}\n",
            q(def.name),
            q(def.desc),
            def.hidden as u8,
            def.id,
            def.id
        );
    }
    s += "\t\t\t}\n\t\t}\n\t}\n}\n";
    s
}

/// The stat whose value is an achievement's progress (Steam shows its bar), and the goal.
pub fn progress_stat(g: Goal) -> Option<(&'static str, u32)> {
    match g {
        G::Wins(n) if n > 1 => Some(("STAT_WINS", n)),
        G::Kills(n) => Some(("STAT_KILLS", n)),
        G::Headshots(n) => Some(("STAT_HEADSHOTS", n)),
        G::HeadStreak(n) => Some(("STAT_HEAD_STREAK", n)),
        G::Parries(n) => Some(("STAT_PARRIES", n)),
        G::Executions(n) => Some(("STAT_EXECUTIONS", n)),
        G::Writs(n) => Some(("STAT_WRITS", n)),
        G::Cosmetics(n) => Some(("STAT_COSMETICS", n)),
        _ => None,
    }
}

// ================================================================================ icons

fn look_sheet() -> Option<&'static image::RgbaImage> {
    static SHEET: std::sync::OnceLock<Option<image::RgbaImage>> = std::sync::OnceLock::new();
    SHEET.get_or_init(|| image::load_from_memory(crate::hub_ui::LOOK_ICONS).ok().map(|i| i.to_rgba8())).as_ref()
}

fn weapon_pic(name: &str) -> Option<image::RgbaImage> {
    image::load_from_memory(crate::weapons::icon_png(name)?).ok().map(|i| trim(&i.to_rgba8()))
}

/// Cut a picture down to what's drawn in it (its see-through margins go), with a little room.
fn trim(img: &image::RgbaImage) -> image::RgbaImage {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, p) in img.enumerate_pixels() {
        if p.0[3] > 24 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 > x1 || y0 > y1 {
        return img.clone();
    }
    let pad = ((x1 - x0).max(y1 - y0) / 24).max(1);
    let (x0, y0) = (x0.saturating_sub(pad), y0.saturating_sub(pad));
    let (x1, y1) = ((x1 + pad).min(img.width() - 1), (y1 + pad).min(img.height() - 1));
    image::imageops::crop_imm(img, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image()
}

/// A wardrobe look's cell on the icon sheet (8 a row), by its place in LOOKS.
fn look_cell(k: usize) -> Option<image::RgbaImage> {
    let sheet = look_sheet()?;
    let n = crate::hub::LOOKS.len();
    if k >= n {
        return None;
    }
    let rows = n.div_ceil(8) as u32;
    let (cw, ch) = (sheet.width() / 8, sheet.height() / rows);
    let (x, y) = ((k % 8) as u32 * cw, (k / 8) as u32 * ch);
    Some(trim(&image::imageops::crop_imm(sheet, x, y, cw, ch).to_image()))
}

/// A vault boss's look for its icon: its rare drop if that's a look, else the first look it drops.
fn boss_look(room: u8) -> Option<usize> {
    let (common, rare) = crate::raid::DROPS.get(room as usize)?;
    rare.iter().chain(common.iter()).find_map(|&i| match crate::raid::LOOT.get(i as usize)?.kind {
        crate::raid::LootKind::Look(id) => crate::hub::LOOKS.iter().position(|l| l.id == id),
        _ => None,
    })
}

fn sample(src: &image::RgbaImage, u: f32, v: f32) -> [f32; 4] {
    let (w, hh) = (src.width() as f32, src.height() as f32);
    let (x, y) = (u * w - 0.5, v * hh - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let px = |xx: f32, yy: f32| -> [f32; 4] {
        if xx < 0.0 || yy < 0.0 || xx >= w || yy >= hh {
            return [0.0; 4];
        }
        let p = src.get_pixel(xx as u32, yy as u32).0;
        let a = p[3] as f32 / 255.0;
        // premultiplied, so edges blend cleanly
        [p[0] as f32 / 255.0 * a, p[1] as f32 / 255.0 * a, p[2] as f32 / 255.0 * a, a]
    };
    let (a, b, c, e) = (px(x0, y0), px(x0 + 1.0, y0), px(x0, y0 + 1.0), px(x0 + 1.0, y0 + 1.0));
    let mut o = [0.0; 4];
    for k in 0..4 {
        o[k] = (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + e[k] * fx) * fy;
    }
    o
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// An achievement's icon: a medallion in its section's colour with the emblem drawn from the
/// game's own pictures; `locked` is the grey version Steam shows before it's earned.
pub fn icon(def: &Def, size: u32, locked: bool) -> image::RgbaImage {
    let sz = size as f32;
    let col = match def.icon {
        Icon::Rank(t) => crate::ranked::TIERS[t.min(7) as usize].2,
        _ if def.hidden => [170, 120, 240],
        _ => def.group.colour(),
    };
    let colf = [col[0] as f32 / 255.0, col[1] as f32 / 255.0, col[2] as f32 / 255.0];
    // the emblem's picture and how it sits: (picture, turn in radians, how much of the medallion it spans)
    let pic: Option<(image::RgbaImage, f32, f32)> = match def.icon {
        Icon::Weapon(n) => weapon_pic(n).map(|p| (p, -0.62, 1.02)),
        Icon::Look(k) => look_cell(k).map(|p| (p, 0.0, 0.72)),
        Icon::Boss(r) => boss_look(r).and_then(look_cell).map(|p| (p, 0.0, 0.74)),
        _ => None,
    };
    let mut img = image::RgbaImage::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let (px, py) = ((x as f32 + 0.5) / sz * 2.0 - 1.0, (y as f32 + 0.5) / sz * 2.0 - 1.0);
            let r = (px * px + py * py).sqrt();
            let aa = 1.5 / sz * 2.0;
            let inside = 1.0 - smooth(0.96 - aa, 0.96, r);
            if inside <= 0.0 {
                continue;
            }
            // the disc: the colour deep in the middle, dark at the rim, a bright ring
            let glow = (1.0 - r).max(0.0);
            let mut c = [0.0f32; 3];
            for k in 0..3 {
                c[k] = 0.05 + colf[k] * (0.12 + 0.45 * glow * glow);
            }
            let ring = smooth(0.80, 0.83, r) * (1.0 - smooth(0.90, 0.93, r));
            let inner = smooth(0.70, 0.715, r) * (1.0 - smooth(0.725, 0.74, r));
            for k in 0..3 {
                c[k] = c[k] * (1.0 - ring) + (colf[k] * 0.9 + 0.1) * ring;
                c[k] = c[k] * (1.0 - inner * 0.6) + colf[k] * inner * 0.6;
            }
            // the emblem
            let em: [f32; 4] = match (&pic, def.icon) {
                (Some((p, turn, span)), _) => {
                    let (s, co) = turn.sin_cos();
                    let (rx, ry) = (px * co + py * s, -px * s + py * co);
                    let aspect = p.width() as f32 / p.height() as f32;
                    let half_w = span * 0.72;
                    let half_h = half_w / aspect.max(1.0);
                    let half_w = if aspect < 1.0 { half_h * aspect } else { half_w };
                    let (u, v) = (rx / (2.0 * half_w) + 0.5, ry / (2.0 * half_h) + 0.5);
                    if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) { sample(p, u, v) } else { [0.0; 4] }
                }
                (None, Icon::Rank(_)) => {
                    // a badge: a diamond with a bevel and a pip
                    let dd = px.abs() + py.abs() * 1.15;
                    let a = 1.0 - smooth(0.52 - aa, 0.52, dd);
                    let bevel = smooth(0.38, 0.40, dd);
                    let pip = 1.0 - smooth(0.10, 0.12, (px * px + (py + 0.02) * (py + 0.02)).sqrt());
                    let k = 0.55 + 0.45 * (1.0 - bevel) + 0.4 * pip;
                    [colf[0] * k * a, colf[1] * k * a, colf[2] * k * a, a]
                }
                _ => {
                    // a wax seal: a scalloped disc with a star pressed into it
                    let ang = py.atan2(px);
                    let edge = 0.50 + 0.03 * (ang * 12.0).cos();
                    let a = 1.0 - smooth(edge - aa, edge, r);
                    let star = 0.22 + 0.12 * (ang * 5.0 + 1.5708).cos().max(0.0).powf(3.0);
                    let pressed = 1.0 - smooth(star - aa, star, r);
                    let rim = smooth(0.36, 0.38, r) * (1.0 - smooth(0.40, 0.42, r));
                    let wax = [0.62 * colf[0] + 0.25, 0.25 * colf[1] + 0.06, 0.2 * colf[2] + 0.05];
                    let k = 0.9 - 0.35 * pressed + 0.25 * rim + 0.15 * (-py).max(0.0);
                    [wax[0] * k * a, wax[1] * k * a, wax[2] * k * a, a]
                }
            };
            // a soft shadow behind the emblem, then the emblem over the disc
            for k in 0..3 {
                c[k] = c[k] * (1.0 - em[3]) + em[k];
            }
            if locked {
                let g = (0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]) * 0.55 + 0.03;
                c = [g, g, g];
            }
            let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            img.put_pixel(x, y, image::Rgba([to8(c[0]), to8(c[1]), to8(c[2]), to8(inside)]));
        }
    }
    img
}

/// Write steam/achievements.vdf and steam/icons (each icon and its locked version) into `dir`.
pub fn export(dir: &std::path::Path, app: u32) -> std::io::Result<usize> {
    std::fs::create_dir_all(dir.join("icons"))?;
    std::fs::write(dir.join("achievements.vdf"), vdf(app))?;
    let mut n = 0;
    for def in ALL.iter() {
        icon(def, 256, false).save(dir.join("icons").join(format!("{}.png", def.id))).map_err(std::io::Error::other)?;
        icon(def, 256, true).save(dir.join("icons").join(format!("{}_locked.png", def.id))).map_err(std::io::Error::other)?;
        n += 2;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::steam_api::FakeSteam;

    #[test]
    fn about_fifty_achievements_each_named_described_and_unique() {
        assert!((50..=60).contains(&ALL.len()), "{}", ALL.len());
        let mut ids: Vec<&str> = ALL.iter().map(|d| d.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), ALL.len(), "unique ids");
        for d in ALL.iter() {
            assert!(!d.name.is_empty() && d.desc.len() > 10, "{}", d.id);
            assert!(api_name(d.id).len() <= 64 && api_name(d.id).chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'));
        }
        assert!(ALL.iter().filter(|d| d.hidden).count() >= 3, "a few secrets");
        // every execution and every fighter has one; every vault boss too
        for k in 0..crate::exec::COUNT as u8 {
            assert!(ALL.iter().any(|d| d.goal == Goal::Exec(k)), "execution {k}");
        }
        for b in 0..career::FIGHTERS as u8 {
            assert!(ALL.iter().any(|d| d.goal == Goal::Mastery(b)));
        }
        for r in 0..5 {
            assert!(ALL.iter().any(|d| d.goal == Goal::Boss(r)));
        }
        // the brief's milestones
        for g in [Goal::Wins(1), Goal::Wins(10), Goal::Wins(100), Goal::Wins(1000), Goal::Parries(100), Goal::SealedWrit, Goal::ChessHard, Goal::Cosmetics(50), Goal::RooftopUnder(ROOFTOP_TARGET_MS)] {
            assert!(ALL.iter().any(|d| d.goal == g), "{g:?}");
        }
        let exec_names: Vec<&str> = crate::exec::EXECUTIONS.iter().map(|e| e.name).collect();
        for k in 0..crate::exec::COUNT {
            let d = ALL.iter().find(|d| d.goal == Goal::Exec(k as u8)).unwrap();
            assert!(d.desc.contains(exec_names[k]), "{} names {}", d.id, exec_names[k]);
        }
    }

    #[test]
    fn progress_unlocks_toasts_and_reaches_steam() {
        let mut steam = FakeSteam::new(1);
        let mut r = Record::default();
        assert!(r.check(&mut steam, false).is_empty(), "nothing yet");
        let day = career::today();
        r.feed(CareerEv::MatchEnd, Some(0), day);
        r.feed(CareerEv::MatchWon, Some(0), day);
        for _ in 0..3 {
            r.feed(CareerEv::Kill { head: true, finisher: 0 }, Some(0), day);
        }
        let new = r.check(&mut steam, false);
        let ids: Vec<&str> = new.iter().map(|&k| ALL[k].id).collect();
        assert!(ids.contains(&"win_first") && ids.contains(&"head_streak_3"), "{ids:?}");
        assert_eq!(r.toasts.len(), new.len(), "a toast each");
        assert_eq!(steam.achievements.get("ACH_WIN_FIRST"), Some(&true));
        assert_eq!(steam.stats.get("STAT_WINS"), Some(&1));
        assert_eq!(steam.stats.get("STAT_HEAD_STREAK"), Some(&3));
        assert!(steam.stored > 0);
        assert!(r.check(&mut steam, false).is_empty(), "once each");
        // progress bars
        assert_eq!(r.progress(Goal::Wins(10)), (1, 10));
        r.feed(CareerEv::Kill { head: false, finisher: 0 }, Some(0), day);
        r.feed(CareerEv::Kill { head: true, finisher: 0 }, Some(0), day);
        assert_eq!(r.life.head_run, 1, "a body-shot kill breaks the streak");
        assert_eq!(r.progress(Goal::HeadStreak(7)), (3, 7));
        // an execution, a parry, the hub and the vault
        r.feed(CareerEv::Kill { head: false, finisher: 9 }, Some(2), day);
        r.feed(CareerEv::Parry, None, day);
        r.feed(CareerEv::Rooftop { ms: 44_100 }, None, day);
        r.feed(CareerEv::TableWin { kind: crate::boards::GameKind::Chess as u8, hard: true }, None, day);
        r.boss_down(3);
        r.vault_cleared(11 * 60 * 1000, true);
        r.ranked_seen(5, Some(4), 0);
        let ids: Vec<&str> = r.check(&mut steam, false).iter().map(|&k| ALL[k].id).collect();
        for want in ["exec_hammer_falls", "rooftop", "rooftop_target", "chess_hard", "boss_regent", "vault_clear", "sealed_writ", "vault_fast", "vault_faster", "rank_placed", "rank_silver", "rank_gold"] {
            assert!(ids.contains(&want), "{want} in {ids:?}");
        }
        assert!(!r.is_unlocked("rank_magistrate") && !r.is_unlocked("boss_warden"));
        // the toasts play one after another
        let first = r.toast_tick(0.1).unwrap().0;
        r.toast_tick(TOAST_TIME);
        assert_ne!(r.toast_tick(0.1).map(|t| t.0), Some(first));
    }

    #[test]
    fn admin_sessions_never_unlock() {
        let mut steam = FakeSteam::new(1);
        let mut r = Record::default();
        let day = career::today();
        r.feed(CareerEv::MatchWon, Some(0), day);
        assert!(r.check(&mut steam, true).is_empty(), "gated: nothing unlocks");
        assert!(steam.achievements.is_empty() && r.toasts.is_empty());
        // ::max raises the career's mastery, not the record's
        let mut career = Career::default();
        career.max_mastery();
        assert_eq!(r.progress(Goal::Mastery(0)).0, 1);
        // cosmetics an admin session gives (::give) are set aside for good
        r.cosmetics_seen(10, false);
        r.cosmetics_seen(10, true);
        r.cosmetics_seen(60, true);
        r.cosmetics_seen(60, false);
        assert_eq!(r.life.cosmetics, 10);
        assert!(!r.check(&mut steam, false).iter().any(|&k| ALL[k].id == "fashion_50"));
        r.cosmetics_seen(100, false);
        assert_eq!(r.life.cosmetics, 50);
        assert!(r.check(&mut steam, false).iter().any(|&k| ALL[k].id == "fashion_50"));
    }

    #[test]
    fn the_record_round_trips_and_lines_up_with_steam() {
        let mut r = Record::seeded(&Career { wins: 12, kills: 40, ..Default::default() });
        r.life.parries = 7;
        r.life.bosses = 0b10101;
        r.life.tier = 5;
        r.life.secrets = 0b101;
        r.unlocked.insert("win_first".into(), 1_700_000_000);
        let back = Record::from_text(&r.to_text());
        assert_eq!(back.life, r.life);
        assert_eq!(back.unlocked, r.unlocked);
        assert_eq!((back.clean.wins, back.clean.kills), (12, 40));
        // another computer unlocked one on Steam; this one has one Steam hasn't heard of
        let mut steam = FakeSteam::new(1);
        steam.achievements.insert("ACH_WIN_10".into(), true);
        let mut here = back.clone();
        let (from, to) = here.reconcile(&mut steam);
        assert_eq!((from, to), (1, 1));
        assert!(here.is_unlocked("win_10") && steam.achievements.get("ACH_WIN_FIRST") == Some(&true));
        // under the test App ID nothing goes to Steam, and nothing breaks
        let mut test = FakeSteam::new(2).as_test_app();
        assert_eq!(here.reconcile(&mut test), (0, 0));
        let mut fresh = Record::default();
        fresh.feed(CareerEv::MatchWon, None, career::today());
        assert!(!fresh.check(&mut test, false).is_empty(), "unlocked here all the same");
        assert!(test.achievements.is_empty() && test.stats.is_empty());
    }

    #[test]
    fn the_steam_files_and_icons_are_made() {
        let v = vdf(480);
        assert_eq!(v.matches('{').count(), v.matches('}').count(), "balanced");
        for d in ALL.iter() {
            assert!(v.contains(&api_name(d.id)));
        }
        for (s, _) in STATS {
            assert!(v.contains(s));
        }
        assert!(v.contains("\"operand1\"\t\t\"STAT_PARRIES\""), "progress tied to a stat");
        // icons: drawn, with the grey version for before it's earned
        for d in [&ALL[0], &ALL[8], &ALL[17], &ALL[34], &ALL[38], &ALL[50]] {
            let a = icon(d, 64, false);
            let g = icon(d, 64, true);
            let mid = a.get_pixel(32, 32).0;
            assert_eq!(mid[3], 255, "{} is opaque in the middle", d.id);
            assert_eq!(a.get_pixel(0, 0).0[3], 0, "round");
            let gm = g.get_pixel(32, 32).0;
            assert!(gm[0] == gm[1] && gm[1] == gm[2], "locked is grey");
        }
        // the shipped file is current (crate::steam_ui::write_steam_files writes it)
        let shipped = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/steam/achievements.vdf")).unwrap_or_default();
        assert_eq!(shipped.replace("\r\n", "\n"), vdf(480), "steam/achievements.vdf is out of date");
    }
}
