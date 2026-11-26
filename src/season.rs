//! Seasons and challenges (all rewards cosmetic, none for sale).
//!
//! - **Seasons** come from text files (`seasons/season1.txt`...: see the notes in that file),
//!   read when the game starts, so a new season needs no code. Each has a name, a lore theme,
//!   a start date and length (ten weeks), a free 50-tier track earned with XP from matches, raids,
//!   challenges and Varrow's games, prestige levels after tier 50 (a coloured border on the profile
//!   card), season-end rewards by peak Ranked 1v1 rank, and ten season-long challenges.
//! - **Rewards**: coat colours (any fighter, seen by everyone in every match), dyes (the season
//!   dyes in the wardrobe stay locked until earned), weapon finishes (seen by everyone), emotes,
//!   titles, banners, nameplates (the profile card and the name over your head in Varrow Centre)
//!   and a season pet. Earned rewards are kept for good, across seasons.
//! - **Challenges**: three daily and five weekly per player, drawn from a pool by the player's own
//!   seed, resetting at 00:00 UTC (weekly on Monday), with one free reroll a day; plus the season's
//!   ten. Nothing counts from a session that used admin commands.
//! - **The profile card** (banner, title, nameplate, border, rank, favourite fighter, top stats,
//!   three showcased rewards) others see in the hub, and the match history.

use crate::game::CareerEv;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

pub type Rgb = [u8; 3];

/// A banner's pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Plain,
    Moon,
    Stripes,
    Chevron,
}

/// What a reward is.
#[derive(Clone, Debug, PartialEq)]
pub enum Reward {
    /// A coat or cape colour, and the hem's.
    Outfit { coat: Rgb, hem: Rgb },
    /// A dye (crate::gear::DYES index).
    Dye(u8),
    Finish { col: Rgb, shine: f32 },
    Emote(crate::hub::Emote),
    Title,
    Banner { top: Rgb, bottom: Rgb, pattern: Pattern },
    Nameplate { fill: Rgb, text: Rgb },
    Pet { col: Rgb },
}

impl Reward {
    pub fn kind(&self) -> &'static str {
        match self {
            Reward::Outfit { .. } => "Outfit",
            Reward::Dye(_) => "Dye",
            Reward::Finish { .. } => "Weapon finish",
            Reward::Emote(_) => "Emote",
            Reward::Title => "Title",
            Reward::Banner { .. } => "Banner",
            Reward::Nameplate { .. } => "Nameplate",
            Reward::Pet { .. } => "Pet",
        }
    }
    /// Its main colour (for swatches and icons).
    pub fn colour(&self) -> Rgb {
        match self {
            Reward::Outfit { coat, .. } => *coat,
            Reward::Dye(k) => crate::gear::DYES.get(*k as usize).map_or([128; 3], |d| {
                let c = |v: f32| (v.max(0.0).powf(1.0 / 2.2) * 255.0).min(255.0) as u8;
                [c(d.1[0]), c(d.1[1]), c(d.1[2])]
            }),
            Reward::Finish { col, .. } => *col,
            Reward::Emote(_) => [255, 200, 140],
            Reward::Title => [235, 220, 200],
            Reward::Banner { top, .. } => *top,
            Reward::Nameplate { fill, .. } => *fill,
            Reward::Pet { col } => *col,
        }
    }
}

/// One reward of a season, by its key ("s1.t12": season 1, tier 12; ".r3" a rank reward; ".c5"
/// a season challenge's).
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub key: String,
    pub name: String,
    pub reward: Reward,
    pub season: u32,
}

/// What a challenge counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    Wins,
    WinsAs(u8),
    DuelWins,
    Kills,
    Headshots,
    Parries,
    Executions,
    Writs,
    Matches,
    Waves,
    Boss(u8),
    Vault,
    VaultHard,
    Rooftop,
    RooftopUnder(u32),
    Table(u8),
    Pit,
    MultiKills,
    RankedWins,
    HubGames,
}

impl What {
    pub fn parse(s: &str) -> Option<What> {
        let (k, arg) = s.split_once(':').map_or((s, ""), |(a, b)| (a, b));
        let n = || arg.trim().parse::<u32>().ok();
        Some(match k.trim() {
            "wins" => What::Wins,
            "wins_as" => What::WinsAs(n()?.min(3) as u8),
            "duel_wins" => What::DuelWins,
            "kills" => What::Kills,
            "headshots" => What::Headshots,
            "parries" => What::Parries,
            "executions" => What::Executions,
            "writs" => What::Writs,
            "matches" => What::Matches,
            "waves" => What::Waves,
            "boss" => What::Boss(n()?.min(4) as u8),
            "vault" => What::Vault,
            "vault_hard" => What::VaultHard,
            "rooftop" => What::Rooftop,
            "rooftop_under" => What::RooftopUnder(n()?),
            "table" => What::Table(match arg.trim() {
                "chess" => 0,
                "checkers" => 1,
                "connect4" => 2,
                _ => return None,
            }),
            "pit" => What::Pit,
            "multikills" => What::MultiKills,
            "ranked_wins" => What::RankedWins,
            "hub_games" => What::HubGames,
            _ => return None,
        })
    }
}

/// What a challenge says.
pub fn describe(w: What, goal: u32) -> String {
    let s = |one: &str, many: &str| if goal == 1 { one.to_string() } else { format!("{goal} {many}") };
    match w {
        What::Wins => format!("Win {}", s("a match", "matches")),
        What::WinsAs(b) => format!("Win {} as {}", s("a match", "matches"), crate::chars::name(b)),
        What::DuelWins => format!("Win {}", s("a Duel", "Duels")),
        What::Kills => format!("Get {}", s("a kill", "kills")),
        What::Headshots => format!("Land {}", s("a headshot kill", "headshot kills")),
        What::Parries => format!("Parry {}", s("an attack", "attacks")),
        What::Executions => format!("Perform {}", s("an execution", "executions")),
        What::Writs => format!("Claim {}", s("a Writ", "Writs")),
        What::Matches => format!("Play {}", s("a match", "matches")),
        What::Waves => format!("Clear {}", s("a Gauntlet wave", "Gauntlet waves")),
        What::Boss(r) => {
            let name = ["the Warden", "the Twin Assessors", "the Chained Ledger", "the Ashblood Regent", "the Ashen Magistrate"][r.min(4) as usize];
            if goal == 1 { format!("Clear {name}") } else { format!("Clear {name} {goal} times") }
        }
        What::Vault => s("Clear the Ashen Vault", "Ashen Vault clears"),
        What::VaultHard => "Clear the Ashen Vault on Sealed Writ".into(),
        What::Rooftop => format!("Finish the Rooftop Run{}", if goal > 1 { format!(" {goal} times") } else { String::new() }),
        What::RooftopUnder(sec) => format!("Finish the Rooftop Run under {sec} s"),
        What::Table(k) => format!("Win {} at the Varrow tables", s(&format!("a {} game", ["chess", "checkers", "Connect Four"][k.min(2) as usize]), &format!("{} games", ["chess", "checkers", "Connect Four"][k.min(2) as usize]))),
        What::Pit => format!("Win {} in the Court pits", s("a duel", "duels")),
        What::MultiKills => format!("Get {}", s("a multi-kill", "multi-kills")),
        What::RankedWins => format!("Win {}", s("a Ranked 1v1 match", "Ranked 1v1 matches")),
        What::HubGames => format!("Win {} in Varrow Centre (tables or pits)", s("a game", "games")),
    }
}

/// The daily pool (what, goal), and the weekly one.
pub const DAILY: [(What, u32); 18] = [
    (What::Wins, 2),
    (What::Headshots, 10),
    (What::Parries, 15),
    (What::Kills, 25),
    (What::Executions, 2),
    (What::Matches, 3),
    (What::Table(0), 1),
    (What::Rooftop, 1),
    (What::RooftopUnder(50), 1),
    (What::Pit, 2),
    (What::Waves, 10),
    (What::WinsAs(0), 1),
    (What::WinsAs(1), 1),
    (What::WinsAs(2), 1),
    (What::WinsAs(3), 1),
    (What::Writs, 1),
    (What::MultiKills, 2),
    (What::DuelWins, 1),
];
pub const WEEKLY: [(What, u32); 16] = [
    (What::WinsAs(1), 3),
    (What::WinsAs(0), 3),
    (What::WinsAs(2), 3),
    (What::WinsAs(3), 3),
    (What::Boss(1), 1),
    (What::Boss(0), 1),
    (What::Vault, 1),
    (What::Headshots, 60),
    (What::Parries, 60),
    (What::Wins, 10),
    (What::RankedWins, 3),
    (What::Executions, 10),
    (What::HubGames, 5),
    (What::Pit, 3),
    (What::RooftopUnder(45), 1),
    (What::DuelWins, 5),
];
pub const DAILY_XP: u32 = 600;
pub const WEEKLY_XP: u32 = 2500;
pub const DAILIES: usize = 3;
pub const WEEKLIES: usize = 5;

/// XP from what isn't a match (matches give the career's XP).
pub const XP_BOSS: u32 = 400;
pub const XP_VAULT: u32 = 1500;
pub const XP_VAULT_HARD: u32 = 1000;
pub const XP_TABLE: u32 = 150;
pub const XP_TABLE_HARD: u32 = 300;
pub const XP_PIT: u32 = 200;
pub const XP_ROOFTOP: u32 = 150;
pub const TIERS: u32 = 50;

/// A season challenge.
#[derive(Clone, Debug, PartialEq)]
pub struct SeasonChallenge {
    pub what: What,
    pub goal: u32,
    pub text: String,
    pub xp: u32,
    /// Its reward too (an index in Registry::items).
    pub reward: Option<usize>,
}

/// A season as its file describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Season {
    pub id: u32,
    pub name: String,
    pub theme: String,
    /// Days since 1970 (UTC) it starts; how many weeks it runs.
    pub start: u32,
    pub weeks: u32,
    pub tier_xp: u32,
    pub prestige_xp: u32,
    pub colour: Rgb,
    /// Each tier's reward (Registry::items index), tier 1 first.
    pub tiers: Vec<Option<usize>>,
    /// Season-end rewards: (rank tier, item).
    pub ranks: Vec<(u8, usize)>,
    pub prestige: Vec<Rgb>,
    pub challenges: Vec<SeasonChallenge>,
}

impl Season {
    pub fn end(&self) -> u32 {
        self.start + self.weeks * 7
    }
    pub fn active(&self, day: u32) -> bool {
        day >= self.start && day < self.end()
    }
}

/// Every season the game knows and all their rewards.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub seasons: Vec<Season>,
    pub items: Vec<Item>,
    /// Lines in the files that couldn't be read (shown on the season page, for whoever wrote them).
    pub problems: Vec<String>,
}

/// Days since 1970 for a date "YYYY-MM-DD" (UTC).
pub fn day_of(date: &str) -> Option<u32> {
    let mut it = date.trim().split('-').map(|x| x.parse::<i64>());
    let (y, m, d) = (it.next()?.ok()?, it.next()?.ok()?, it.next()?.ok()?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // (days from civil: Howard Hinnant's algorithm)
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    u32::try_from(era * 146_097 + doe - 719_468).ok()
}

/// The date for a day number ("2026-11-23").
pub fn date_of(day: u32) -> String {
    let z = day as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

fn rgb(s: &str) -> Option<Rgb> {
    let v: Vec<u8> = s.split(',').filter_map(|x| x.trim().parse::<u8>().ok()).collect();
    (v.len() == 3).then(|| [v[0], v[1], v[2]])
}

/// The emote a season file names.
pub fn emote_of(key: &str) -> Option<crate::hub::Emote> {
    use crate::hub::Emote;
    Some(match key.trim() {
        "kneel" => Emote::Kneel,
        "point" => Emote::Point,
        "shrug" => Emote::Shrug,
        "moonbow" => Emote::MoonBow,
        "toast" => Emote::Toast,
        "brood" => Emote::Brood,
        _ => return None,
    })
}

/// A reward from its parts ("dye | Blood Moon", "finish | name | r,g,b | 0.5"...).
fn reward_of(parts: &[&str]) -> Result<(String, Reward), String> {
    let kind = parts.first().map(|s| s.trim()).unwrap_or("");
    let name = parts.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
    let col = |k: usize| parts.get(k).and_then(|s| rgb(s)).ok_or_else(|| format!("{kind}: colour {k} isn't r,g,b"));
    if name.is_empty() {
        return Err(format!("{kind}: no name"));
    }
    let r = match kind {
        "outfit" => Reward::Outfit { coat: col(2)?, hem: col(3)? },
        "dye" => {
            let k = crate::gear::DYES.iter().position(|d| d.0.eq_ignore_ascii_case(&name)).ok_or_else(|| format!("no dye called {name}"))?;
            Reward::Dye(k as u8)
        }
        "finish" => Reward::Finish { col: col(2)?, shine: parts.get(3).and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(0.3).clamp(0.0, 1.0) },
        "emote" => Reward::Emote(parts.get(2).and_then(|s| emote_of(s)).ok_or_else(|| format!("emote {name}: kneel, point, shrug, moonbow, toast or brood"))?),
        "title" => Reward::Title,
        "banner" => Reward::Banner {
            top: col(2)?,
            bottom: col(3)?,
            pattern: match parts.get(4).map(|s| s.trim()) {
                Some("moon") => Pattern::Moon,
                Some("stripes") => Pattern::Stripes,
                Some("chevron") => Pattern::Chevron,
                _ => Pattern::Plain,
            },
        },
        "nameplate" => Reward::Nameplate { fill: col(2)?, text: col(3)? },
        "pet" => Reward::Pet { col: col(2)? },
        _ => return Err(format!("no reward kind called {kind}")),
    };
    Ok((name, r))
}

impl Registry {
    /// Read one season file into the registry.
    pub fn add_file(&mut self, text: &str, source: &str) {
        let mut s = Season { id: 0, name: String::new(), theme: String::new(), start: 0, weeks: 10, tier_xp: 3000, prestige_xp: 4000, colour: [205, 46, 52], tiers: vec![None; TIERS as usize], ranks: Vec::new(), prestige: Vec::new(), challenges: Vec::new() };
        let mut items: Vec<Item> = Vec::new();
        let mut problems = Vec::new();
        // the id first (the item keys carry it)
        for line in text.lines() {
            if let Some((k, v)) = line.split('#').next().unwrap_or("").split_once('=') {
                if k.trim() == "id" {
                    s.id = v.trim().parse().unwrap_or(0);
                }
            }
        }
        if s.id == 0 || self.seasons.iter().any(|o| o.id == s.id) {
            if s.id == 0 {
                self.problems.push(format!("{source}: no \"id = <number>\" line"));
            }
            return;
        }
        let base = self.items.len();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let mut bad = |why: String| problems.push(format!("{source} line {}: {why}", n + 1));
            match k {
                "id" => {}
                "name" => s.name = v.to_string(),
                "start" => match day_of(v) {
                    Some(d) => s.start = d,
                    None => bad("start isn't a date like 2026-11-23".into()),
                },
                "weeks" => s.weeks = v.parse().unwrap_or(10).clamp(1, 52),
                "tier_xp" => s.tier_xp = v.parse().unwrap_or(3000).max(100),
                "prestige_xp" => s.prestige_xp = v.parse().unwrap_or(4000).max(100),
                "colour" => s.colour = rgb(v).unwrap_or(s.colour),
                "theme" => {
                    if !s.theme.is_empty() {
                        s.theme.push('\n');
                    }
                    s.theme.push_str(v);
                }
                "prestige" => s.prestige = v.split('|').filter_map(rgb).collect(),
                _ if k.starts_with("tier ") || k.starts_with("rank ") => {
                    let num: Option<u32> = k[5..].trim().parse().ok();
                    let parts: Vec<&str> = v.split('|').collect();
                    match (num, reward_of(&parts)) {
                        (Some(t), Ok((name, reward))) if k.starts_with("tier ") && (1..=TIERS).contains(&t) => {
                            s.tiers[t as usize - 1] = Some(base + items.len());
                            items.push(Item { key: format!("s{}.t{t}", s.id), name, reward, season: s.id });
                        }
                        (Some(t), Ok((name, reward))) if k.starts_with("rank ") && t <= 7 => {
                            s.ranks.push((t as u8, base + items.len()));
                            items.push(Item { key: format!("s{}.r{t}", s.id), name, reward, season: s.id });
                        }
                        (_, Err(e)) => bad(e),
                        _ => bad(format!("{k}: tiers run 1-{TIERS}, ranks 0-7")),
                    }
                }
                "challenge" => {
                    let parts: Vec<&str> = v.split('|').map(str::trim).collect();
                    let mut head = parts.first().copied().unwrap_or("").split_whitespace();
                    let what = head.next().and_then(What::parse);
                    let goal = head.next().and_then(|g| g.parse::<u32>().ok()).unwrap_or(1).max(1);
                    let xp = parts.get(2).and_then(|x| x.parse::<u32>().ok()).unwrap_or(5000);
                    let Some(what) = what else {
                        bad(format!("challenge: \"{}\" isn't something to count", parts.first().unwrap_or(&"")));
                        continue;
                    };
                    let text = parts.get(1).map(|t| t.to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| describe(what, goal));
                    let mut reward = None;
                    if parts.len() > 3 {
                        match reward_of(&parts[3..]) {
                            Ok((name, r)) => {
                                reward = Some(base + items.len());
                                items.push(Item { key: format!("s{}.c{}", s.id, s.challenges.len() + 1), name, reward: r, season: s.id });
                            }
                            Err(e) => bad(e),
                        }
                    }
                    s.challenges.push(SeasonChallenge { what, goal, text, xp, reward });
                }
                _ => bad(format!("\"{k}\" isn't a season setting")),
            }
        }
        if s.start == 0 {
            problems.push(format!("{source}: no start date"));
            self.problems.extend(problems);
            return;
        }
        if s.prestige.is_empty() {
            s.prestige = vec![[196, 128, 74], [200, 208, 220], [255, 200, 70]];
        }
        s.challenges.truncate(10);
        self.items.extend(items);
        self.seasons.push(s);
        self.seasons.sort_by_key(|x| x.id);
        self.problems.extend(problems);
    }

    /// The season on now; between seasons, the last one that started (or the first to come).
    pub fn current(&self, day: u32) -> Option<&Season> {
        self.seasons.iter().find(|s| s.active(day)).or_else(|| self.seasons.iter().filter(|s| s.start <= day).max_by_key(|s| s.start)).or_else(|| self.seasons.iter().min_by_key(|s| s.start))
    }
    pub fn season(&self, id: u32) -> Option<&Season> {
        self.seasons.iter().find(|s| s.id == id)
    }
    pub fn item(&self, key: &str) -> Option<(usize, &Item)> {
        self.items.iter().enumerate().find(|(_, i)| i.key == key)
    }
    /// An item's number among those of its kind (what goes over the network and into loadouts).
    fn nth_of(&self, idx: usize, pick: fn(&Reward) -> bool) -> Option<usize> {
        if !pick(&self.items.get(idx)?.reward) {
            return None;
        }
        Some(self.items[..idx].iter().filter(|i| pick(&i.reward)).count())
    }
    fn by_nth(&self, n: usize, pick: fn(&Reward) -> bool) -> Option<&Item> {
        self.items.iter().filter(|i| pick(&i.reward)).nth(n)
    }
}

fn is_outfit(r: &Reward) -> bool {
    matches!(r, Reward::Outfit { .. })
}
fn is_finish(r: &Reward) -> bool {
    matches!(r, Reward::Finish { .. })
}
fn is_pet(r: &Reward) -> bool {
    matches!(r, Reward::Pet { .. })
}

/// The built-in first season (used when the seasons folder isn't beside the game).
pub const BUILT_IN: &str = include_str!("../seasons/season1.txt");

/// Where season files are looked for: the save folder's seasons/, beside the game, and (while
/// developing) the project's own folder.
fn season_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = crate::platform::data_file("seasons") {
        v.push(d);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            v.push(d.join("seasons"));
            v.push(d.join("../../seasons"));
        }
    }
    v
}

/// Read every season file (and the built-in one if season 1 isn't among them).
pub fn load_registry() -> Registry {
    let mut r = Registry::default();
    if std::env::var_os("DUEL_SEASONS_BUILT_IN").is_none() {
        for d in season_dirs() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "txt") && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("season"))).collect();
            files.sort();
            for f in files {
                if let Ok(t) = std::fs::read_to_string(&f) {
                    r.add_file(&t, &f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
                }
            }
        }
    }
    if r.season(1).is_none() {
        r.add_file(BUILT_IN, "season1.txt (built in)");
    }
    r
}

static REG: OnceLock<Registry> = OnceLock::new();

/// The seasons (read once, when first asked).
pub fn reg() -> &'static Registry {
    REG.get_or_init(load_registry)
}

// ---- what other parts of the game ask (the renderer, the wardrobe, the network)

/// A season coat colour by its outfit number (crate::career::OUTFITS come first): (coat, hem).
pub fn outfit_colours(outfit: u8) -> Option<(Rgb, Rgb)> {
    let n = (outfit as usize).checked_sub(crate::career::OUTFITS.len())?;
    match reg().by_nth(n, is_outfit)?.reward {
        Reward::Outfit { coat, hem } => Some((coat, hem)),
        _ => None,
    }
}
/// The loadout's outfit number for a season outfit item.
pub fn outfit_number(idx: usize) -> Option<u8> {
    let n = reg().nth_of(idx, is_outfit)? + crate::career::OUTFITS.len();
    (n <= 15).then_some(n as u8)
}
/// A weapon finish by its number (1..; 0 none): colour and shine.
pub fn finish(n: u8) -> Option<(Rgb, f32)> {
    match reg().by_nth((n as usize).checked_sub(1)?, is_finish)?.reward {
        Reward::Finish { col, shine } => Some((col, shine)),
        _ => None,
    }
}
pub fn finish_number(idx: usize) -> Option<u8> {
    let n = reg().nth_of(idx, is_finish)? + 1;
    (n <= 255).then_some(n as u8)
}
/// A season pet by the outfit's pet value (1 is the vault's Cinder Wisp, 2.. the season pets).
pub fn pet_colour(v: u8) -> Option<Rgb> {
    match reg().by_nth((v as usize).checked_sub(2)?, is_pet)?.reward {
        Reward::Pet { col } => Some(col),
        _ => None,
    }
}
pub fn pet_value(idx: usize) -> Option<u8> {
    let n = reg().nth_of(idx, is_pet)? + 2;
    (n <= 255).then_some(n as u8)
}

/// The season dyes and pets this player owns (bits), kept where the wardrobe and outfit checks
/// can see them.
static DYES_OWNED: AtomicU32 = AtomicU32::new(0);
static PETS_OWNED: AtomicU32 = AtomicU32::new(0);

/// A season dye not earned yet (the wardrobe leaves it out).
pub fn dye_locked(k: u8) -> bool {
    k >= crate::gear::SEASON_DYES && DYES_OWNED.load(Ordering::Relaxed) & (1 << (k - crate::gear::SEASON_DYES).min(31)) == 0
}
/// A season pet that may be worn.
pub fn pet_owned(v: u8) -> bool {
    v >= 2 && PETS_OWNED.load(Ordering::Relaxed) & (1 << (v - 2).min(31)) != 0
}
/// The extra emotes earned (bits by Emote number).
static EMOTES_OWNED: AtomicU32 = AtomicU32::new(0);
pub fn emote_owned(e: crate::hub::Emote) -> bool {
    (e as u8) < 9 || EMOTES_OWNED.load(Ordering::Relaxed) & (1 << (e as u8)) != 0
}

// ================================================================================ progress

/// One challenge in play: which (pool or season index), how far, whether it's done.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    pub idx: u16,
    pub prog: u32,
    pub done: bool,
}

/// What's worn from the season locker (item keys).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Equip {
    pub outfit: Option<String>,
    pub finish: Option<String>,
    pub title: Option<String>,
    pub banner: Option<String>,
    pub nameplate: Option<String>,
    pub pet: Option<String>,
    pub showcase: [Option<String>; 3],
}

/// A match in the history.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Played {
    pub when: u64,
    pub mode: String,
    pub map: String,
    pub won: bool,
    pub kills: u32,
    pub deaths: u32,
    pub xp: u32,
    pub ranked: bool,
}

pub const HISTORY: usize = 40;

/// Where a challenge or XP came from (for the game's context when counting).
#[derive(Clone, Copy, Debug, Default)]
pub struct Ctx {
    pub body: u8,
    pub duel: bool,
    pub ranked: bool,
}

/// This player's season progress, challenges, locker and history.
#[derive(Clone, Debug, Default)]
pub struct Progress {
    /// The player's own seed (their challenges differ from everyone else's).
    pub seed: u64,
    /// The season the XP below belongs to, the XP, and the best rank tier + 1 reached in it.
    pub season: u32,
    pub xp: u32,
    pub peak: u8,
    /// Rewards earned (kept across seasons), and the ones not looked at yet.
    pub owned: BTreeSet<String>,
    pub fresh: BTreeSet<String>,
    pub equip: Equip,
    /// The day and week the challenges are for; the day the free reroll went.
    pub day: u32,
    pub daily: Vec<Slot>,
    pub reroll_day: u32,
    pub week: u32,
    pub weekly: Vec<Slot>,
    /// The season's challenges.
    pub sprog: Vec<Slot>,
    /// The season start screen was seen for this season; ranked was soft-reset for this season.
    pub seen: u32,
    pub ranked_reset: u32,
    pub history: Vec<Played>,
    /// Lines to tell the player (not saved).
    pub notes: Vec<String>,
    pub dirty: bool,
}

fn mix(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^ (h >> 33)
}

/// `n` different picks from a pool of `len`, from a seed.
pub fn draw(seed: u64, len: usize, n: usize) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    let mut h = seed;
    while out.len() < n.min(len) {
        h = mix(h.wrapping_add(0x9e37_79b9_7f4a_7c15));
        let k = (h % len as u64) as u16;
        if !out.contains(&k) {
            out.push(k);
        }
    }
    out
}

pub fn day_now() -> u32 {
    (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) / 86_400) as u32
}
pub fn secs_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}
/// Seconds to the next daily reset (00:00 UTC) and the next weekly one (Monday 00:00 UTC).
pub fn resets_in(now: u64) -> (u64, u64) {
    let day = now / 86_400;
    let daily = 86_400 - now % 86_400;
    let next_week_day = (crate::career::week_of(day as u32) as u64 + 1) * 7 - 3;
    (daily, next_week_day * 86_400 - now)
}

/// "3d 4h", "5h 12m", "12m".
pub fn span(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs / 3600 % 24, secs / 60 % 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{}m", m.max(1))
    }
}

/// What the start of a new season settled.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rollover {
    pub from: u32,
    pub to: u32,
    /// Season-end rewards given for the last season's peak rank.
    pub rank_rewards: Vec<String>,
}

impl Progress {
    pub fn new(seed: u64) -> Progress {
        Progress { seed: seed.max(1), dirty: true, ..Default::default() }
    }

    /// The tier reached (0..=50), XP into the next one, what it needs, and the prestige level.
    pub fn tier(&self, s: &Season) -> (u32, u32, u32, u32) {
        let track = s.tier_xp * TIERS;
        if self.xp < track {
            (self.xp / s.tier_xp, self.xp % s.tier_xp, s.tier_xp, 0)
        } else {
            let over = self.xp - track;
            (TIERS, over % s.prestige_xp, s.prestige_xp, over / s.prestige_xp)
        }
    }

    /// The prestige border's colour (None before prestige).
    pub fn border(&self, s: &Season) -> Option<Rgb> {
        let p = self.tier(s).3;
        (p > 0).then(|| s.prestige[(p as usize - 1).min(s.prestige.len() - 1)])
    }

    fn give(&mut self, r: &Registry, idx: usize) {
        if let Some(it) = r.items.get(idx) {
            if self.owned.insert(it.key.clone()) {
                self.fresh.insert(it.key.clone());
                self.notes.push(format!("Season reward: {} ({})", it.name, it.reward.kind()));
                self.dirty = true;
            }
        }
    }

    /// Bring the record up to today: a new season (season-end rank rewards, a fresh track and
    /// season challenges), new daily and weekly challenges.
    pub fn roll(&mut self, r: &Registry, day: u32) -> Option<Rollover> {
        let mut out = None;
        if let Some(s) = r.current(day).filter(|s| s.active(day)) {
            if self.season != s.id {
                let mut ro = Rollover { from: self.season, to: s.id, rank_rewards: Vec::new() };
                if let Some(old) = r.season(self.season) {
                    // every rank reward up to the peak (peak is the tier + 1)
                    let peak = self.peak;
                    for &(_, idx) in old.ranks.iter().filter(|(t, _)| *t < peak) {
                        self.give(r, idx);
                        ro.rank_rewards.push(r.items[idx].name.clone());
                    }
                }
                self.season = s.id;
                self.xp = 0;
                self.peak = 0;
                self.sprog = (0..s.challenges.len()).map(|k| Slot { idx: k as u16, ..Default::default() }).collect();
                self.dirty = true;
                out = Some(ro);
            }
        }
        if self.day != day || self.daily.len() != DAILIES {
            self.day = day;
            self.daily = draw(mix(self.seed ^ (day as u64 * 0x1_0003)), DAILY.len(), DAILIES).into_iter().map(|idx| Slot { idx, ..Default::default() }).collect();
            self.dirty = true;
        }
        let week = crate::career::week_of(day);
        if self.week != week || self.weekly.len() != WEEKLIES {
            self.week = week;
            self.weekly = draw(mix(self.seed ^ (week as u64 * 0x7_0001) ^ 0xabcd), WEEKLY.len(), WEEKLIES).into_iter().map(|idx| Slot { idx, ..Default::default() }).collect();
            self.dirty = true;
        }
        out
    }

    /// Add XP to the track (only while a season is on): tiers crossed give their rewards.
    pub fn add_xp(&mut self, r: &Registry, day: u32, n: u32) {
        let Some(s) = r.current(day).filter(|s| s.active(day) && s.id == self.season) else { return };
        let (before, _, _, was) = self.tier(s);
        self.xp = self.xp.saturating_add(n);
        let (after, _, _, prestige) = self.tier(s);
        for t in before + 1..=after {
            if let Some(Some(idx)) = s.tiers.get(t as usize - 1) {
                self.give(r, *idx);
            }
        }
        if after > before {
            self.notes.push(format!("Season tier {after}!"));
        }
        if prestige > was {
            self.notes.push(format!("Prestige {prestige}: a new border for your profile card"));
        }
        self.dirty = true;
    }

    /// Count something toward the challenges; returns the XP the finished challenges paid (also
    /// added to the track).
    fn count(&mut self, r: &Registry, day: u32, test: impl Fn(What) -> u32) -> u32 {
        let mut paid = 0;
        let mut notes = Vec::new();
        for s in self.daily.iter_mut() {
            let (w, goal) = DAILY[s.idx as usize % DAILY.len()];
            if !s.done {
                s.prog = (s.prog + test(w)).min(goal);
                if s.prog >= goal {
                    s.done = true;
                    paid += DAILY_XP;
                    notes.push(format!("Daily challenge done: {} (+{DAILY_XP} XP)", describe(w, goal)));
                }
            }
        }
        for s in self.weekly.iter_mut() {
            let (w, goal) = WEEKLY[s.idx as usize % WEEKLY.len()];
            if !s.done {
                s.prog = (s.prog + test(w)).min(goal);
                if s.prog >= goal {
                    s.done = true;
                    paid += WEEKLY_XP;
                    notes.push(format!("Weekly challenge done: {} (+{WEEKLY_XP} XP)", describe(w, goal)));
                }
            }
        }
        let mut gifts = Vec::new();
        if let Some(season) = r.season(self.season).filter(|s| s.active(day)) {
            for s in self.sprog.iter_mut() {
                let Some(c) = season.challenges.get(s.idx as usize) else { continue };
                if !s.done {
                    s.prog = (s.prog + test(c.what)).min(c.goal);
                    if s.prog >= c.goal {
                        s.done = true;
                        paid += c.xp;
                        notes.push(format!("Season challenge done: {} (+{} XP)", c.text, c.xp));
                        gifts.extend(c.reward);
                    }
                }
            }
        }
        self.notes.extend(notes);
        for g in gifts {
            self.give(r, g);
        }
        if paid > 0 {
            self.add_xp(r, day, paid);
        }
        paid
    }

    /// Count what happened in a match or in Varrow Centre: `xp` is what the career paid for it
    /// (the track gets the same). A session that used admin commands counts nothing.
    pub fn feed(&mut self, r: &Registry, day: u32, ev: CareerEv, ctx: Ctx, xp: u32, gated: bool) {
        if gated {
            return;
        }
        let extra = match ev {
            CareerEv::TableWin { hard, .. } => if hard { XP_TABLE_HARD } else { XP_TABLE },
            CareerEv::PitWin => XP_PIT,
            CareerEv::Rooftop { .. } => XP_ROOFTOP,
            _ => 0,
        };
        self.add_xp(r, day, xp + extra);
        self.count(r, day, |w| match (w, ev) {
            (What::Wins, CareerEv::MatchWon) => 1,
            (What::WinsAs(b), CareerEv::MatchWon) if b == ctx.body => 1,
            (What::DuelWins, CareerEv::MatchWon) if ctx.duel => 1,
            (What::RankedWins, CareerEv::MatchWon) if ctx.ranked => 1,
            (What::Matches, CareerEv::MatchEnd) => 1,
            (What::Kills, CareerEv::Kill { .. }) => 1,
            (What::Headshots, CareerEv::Kill { head: true, .. }) => 1,
            (What::Executions, CareerEv::Kill { finisher, .. }) if finisher > 0 => 1,
            (What::Writs, CareerEv::WritClaim { .. }) => 1,
            (What::MultiKills, CareerEv::MultiKill) => 1,
            (What::Waves, CareerEv::WaveCleared) => 1,
            (What::Parries, CareerEv::Parry) => 1,
            (What::Rooftop, CareerEv::Rooftop { .. }) => 1,
            (What::RooftopUnder(s), CareerEv::Rooftop { ms }) if ms <= s * 1000 => 1,
            (What::Pit | What::HubGames, CareerEv::PitWin) => 1,
            (What::Table(k), CareerEv::TableWin { kind, .. }) if k == kind => 1,
            (What::HubGames, CareerEv::TableWin { .. }) => 1,
            _ => 0,
        });
    }

    /// A vault boss beaten (its room), and a clear.
    pub fn boss_down(&mut self, r: &Registry, day: u32, room: u8, gated: bool) {
        if gated {
            return;
        }
        self.add_xp(r, day, XP_BOSS);
        self.count(r, day, |w| (w == What::Boss(room)) as u32);
    }
    pub fn vault_cleared(&mut self, r: &Registry, day: u32, hard: bool, gated: bool) {
        if gated {
            return;
        }
        self.add_xp(r, day, XP_VAULT + if hard { XP_VAULT_HARD } else { 0 });
        self.count(r, day, |w| (w == What::Vault || (hard && w == What::VaultHard)) as u32);
    }

    /// The ranked rank tier seen now (for the season-end rewards).
    pub fn rank_seen(&mut self, tier: Option<u8>, gated: bool) {
        if let Some(t) = tier.filter(|_| !gated) {
            if t + 1 > self.peak {
                self.peak = t + 1;
                self.dirty = true;
            }
        }
    }

    /// Today's free reroll of a daily challenge not yet done: a different one from the pool.
    pub fn reroll(&mut self, k: usize, day: u32) -> bool {
        if self.reroll_day == day || self.daily.get(k).is_none_or(|s| s.done) {
            return false;
        }
        let taken: Vec<u16> = self.daily.iter().map(|s| s.idx).collect();
        let mut h = mix(self.seed ^ (day as u64) << 20 ^ 0x5eed);
        let pick = loop {
            h = mix(h.wrapping_add(1));
            let c = (h % DAILY.len() as u64) as u16;
            if !taken.contains(&c) {
                break c;
            }
        };
        self.daily[k] = Slot { idx: pick, ..Default::default() };
        self.reroll_day = day;
        self.dirty = true;
        true
    }

    /// A match for the history (newest first).
    pub fn played(&mut self, p: Played) {
        self.history.insert(0, p);
        self.history.truncate(HISTORY);
        self.dirty = true;
    }

    /// Wear an owned reward (or take it off when it's on).
    pub fn wear(&mut self, r: &Registry, key: &str) -> bool {
        if !self.owned.contains(key) {
            return false;
        }
        let Some((_, it)) = r.item(key) else { return false };
        let slot = match it.reward {
            Reward::Outfit { .. } => &mut self.equip.outfit,
            Reward::Finish { .. } => &mut self.equip.finish,
            Reward::Title => &mut self.equip.title,
            Reward::Banner { .. } => &mut self.equip.banner,
            Reward::Nameplate { .. } => &mut self.equip.nameplate,
            Reward::Pet { .. } => &mut self.equip.pet,
            Reward::Dye(_) | Reward::Emote(_) => return false,
        };
        *slot = if slot.as_deref() == Some(key) { None } else { Some(key.to_string()) };
        self.dirty = true;
        true
    }

    /// Show (or stop showing) an owned reward on the profile card (three at most).
    pub fn showcase(&mut self, key: &str) {
        if !self.owned.contains(key) {
            return;
        }
        let sc = &mut self.equip.showcase;
        if let Some(k) = sc.iter().position(|s| s.as_deref() == Some(key)) {
            sc[k] = None;
        } else if let Some(k) = sc.iter().position(|s| s.is_none()) {
            sc[k] = Some(key.to_string());
        } else {
            sc.rotate_left(1);
            sc[2] = Some(key.to_string());
        }
        self.dirty = true;
    }

    /// Tell the wardrobe and outfit checks which season dyes, pets and emotes are owned.
    pub fn publish(&self, r: &Registry) {
        let (mut dyes, mut pets, mut emotes) = (0u32, 0u32, 0u32);
        for (idx, it) in r.items.iter().enumerate() {
            if !self.owned.contains(&it.key) {
                continue;
            }
            match it.reward {
                Reward::Dye(k) if k >= crate::gear::SEASON_DYES => dyes |= 1 << (k - crate::gear::SEASON_DYES).min(31),
                Reward::Pet { .. } => {
                    if let Some(v) = pet_value_in(r, idx) {
                        pets |= 1 << (v - 2).min(31);
                    }
                }
                Reward::Emote(e) => emotes |= 1 << (e as u8).min(31),
                _ => {}
            }
        }
        DYES_OWNED.store(dyes, Ordering::Relaxed);
        PETS_OWNED.store(pets, Ordering::Relaxed);
        EMOTES_OWNED.store(emotes, Ordering::Relaxed);
    }

    // ---- the file
    pub fn to_text(&self) -> String {
        let slots = |v: &[Slot]| v.iter().map(|s| format!("{}:{}:{}", s.idx, s.prog, s.done as u8)).collect::<Vec<_>>().join(",");
        let o = |s: &Option<String>| s.clone().unwrap_or_default();
        let e = &self.equip;
        let mut t = format!(
            "v=1\nseed={}\nseason={}\nxp={}\npeak={}\nseen={}\nranked_reset={}\nday={}\nreroll_day={}\nweek={}\ndaily={}\nweekly={}\nsprog={}\nowned={}\nfresh={}\nequip={}|{}|{}|{}|{}|{}|{}|{}|{}\n",
            self.seed,
            self.season,
            self.xp,
            self.peak,
            self.seen,
            self.ranked_reset,
            self.day,
            self.reroll_day,
            self.week,
            slots(&self.daily),
            slots(&self.weekly),
            slots(&self.sprog),
            self.owned.iter().cloned().collect::<Vec<_>>().join(","),
            self.fresh.iter().cloned().collect::<Vec<_>>().join(","),
            o(&e.outfit),
            o(&e.finish),
            o(&e.title),
            o(&e.banner),
            o(&e.nameplate),
            o(&e.pet),
            o(&e.showcase[0]),
            o(&e.showcase[1]),
            o(&e.showcase[2])
        );
        for p in &self.history {
            t += &format!("match={}|{}|{}|{}|{}|{}|{}|{}\n", p.when, p.mode.replace('|', "/"), p.map.replace('|', "/"), p.won as u8, p.kills, p.deaths, p.xp, p.ranked as u8);
        }
        t
    }

    pub fn from_text(t: &str) -> Progress {
        let mut p = Progress::default();
        let slots = |v: &str| -> Vec<Slot> {
            v.split(',').filter(|x| !x.is_empty()).filter_map(|x| {
                let mut it = x.split(':');
                Some(Slot { idx: it.next()?.parse().ok()?, prog: it.next()?.parse().ok()?, done: it.next()? == "1" })
            }).collect()
        };
        for line in t.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let n = || v.trim().parse::<u32>().unwrap_or(0);
            match k {
                "seed" => p.seed = v.trim().parse().unwrap_or(1),
                "season" => p.season = n(),
                "xp" => p.xp = n(),
                "peak" => p.peak = n().min(8) as u8,
                "seen" => p.seen = n(),
                "ranked_reset" => p.ranked_reset = n(),
                "day" => p.day = n(),
                "reroll_day" => p.reroll_day = n(),
                "week" => p.week = n(),
                "daily" => p.daily = slots(v),
                "weekly" => p.weekly = slots(v),
                "sprog" => p.sprog = slots(v),
                "owned" => p.owned = v.split(',').filter(|x| !x.is_empty()).map(str::to_string).collect(),
                "fresh" => p.fresh = v.split(',').filter(|x| !x.is_empty()).map(str::to_string).collect(),
                "equip" => {
                    let f: Vec<Option<String>> = v.split('|').map(|x| (!x.is_empty()).then(|| x.to_string())).collect();
                    let g = |k: usize| f.get(k).cloned().flatten();
                    p.equip = Equip { outfit: g(0), finish: g(1), title: g(2), banner: g(3), nameplate: g(4), pet: g(5), showcase: [g(6), g(7), g(8)] };
                }
                "match" => {
                    let f: Vec<&str> = v.split('|').collect();
                    if f.len() >= 8 {
                        let u = |k: usize| f[k].parse::<u32>().unwrap_or(0);
                        p.history.push(Played { when: f[0].parse().unwrap_or(0), mode: f[1].into(), map: f[2].into(), won: f[3] == "1", kills: u(4), deaths: u(5), xp: u(6), ranked: f[7] == "1" });
                    }
                }
                _ => {}
            }
        }
        p.history.truncate(HISTORY);
        if p.seed == 0 {
            p.seed = 1;
        }
        p
    }
}

fn pet_value_in(r: &Registry, idx: usize) -> Option<u8> {
    let n = r.nth_of(idx, is_pet)? + 2;
    (n <= 255).then_some(n as u8)
}

pub fn path() -> Option<PathBuf> {
    crate::platform::data_file("season.txt")
}

/// Load this computer's season record (a fresh one, with a new seed, the first time).
pub fn load() -> Progress {
    match path().and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(t) => Progress::from_text(&t),
        None => Progress::new(secs_now() ^ (std::process::id() as u64).rotate_left(32) ^ 0x5ea5_0000),
    }
}

pub fn save(p: &mut Progress) {
    if let Some(f) = path() {
        let _ = crate::platform::save_file(&f, &p.to_text());
    }
    p.dirty = false;
}

/// The ranked soft reset at a season's start: the rating halfway back to 1500, and the
/// uncertainty raised so the first matches move it quickly.
pub fn soft_reset(r: &mut crate::ranked::Rating) {
    r.r = (r.r + 1500.0) * 0.5;
    r.rd = r.rd.max(250.0);
}

// ================================================================================ the profile card

/// A profile card as others see it (sent round Varrow Centre).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Card {
    pub name: String,
    /// Registry::items indices (u16::MAX: none).
    pub banner: u16,
    pub title: u16,
    pub nameplate: u16,
    pub showcase: [u16; 3],
    /// The season (id), its tier, the prestige level.
    pub season: u8,
    pub tier: u8,
    pub prestige: u8,
    /// The ranked badge byte (crate::ranked::Rank::to_byte), the favourite fighter.
    pub rank: u8,
    pub fav: u8,
    /// Matches won, kills, headshots, parries, executions, vault clears.
    pub stats: [u32; 6],
}

pub const NONE: u16 = u16::MAX;
pub const CARD_STATS: [&str; 6] = ["Wins", "Kills", "Headshots", "Parries", "Executions", "Vault clears"];

impl Card {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(64);
        let n = self.name.as_bytes();
        b.push(n.len().min(32) as u8);
        b.extend_from_slice(&n[..n.len().min(32)]);
        for v in [self.banner, self.title, self.nameplate, self.showcase[0], self.showcase[1], self.showcase[2]] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&[self.season, self.tier, self.prestige, self.rank, self.fav]);
        for s in self.stats {
            b.extend_from_slice(&s.to_le_bytes());
        }
        b
    }
    pub fn from_bytes(b: &[u8]) -> Option<Card> {
        let n = *b.first()? as usize;
        let name = String::from_utf8_lossy(b.get(1..1 + n)?).chars().take(32).collect();
        let mut i = 1 + n;
        let mut u16v = || -> Option<u16> {
            let v = u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]);
            i += 2;
            Some(v)
        };
        let (banner, title, nameplate) = (u16v()?, u16v()?, u16v()?);
        let showcase = [u16v()?, u16v()?, u16v()?];
        let f = b.get(i..i + 5)?;
        let (season, tier, prestige, rank, fav) = (f[0], f[1].min(TIERS as u8), f[2], f[3], f[4].min(3));
        i += 5;
        let mut stats = [0u32; 6];
        for s in stats.iter_mut() {
            *s = u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?);
            i += 4;
        }
        let items = reg().items.len() as u16;
        let ok = |v: u16| if v < items { v } else { NONE };
        Some(Card { name, banner: ok(banner), title: ok(title), nameplate: ok(nameplate), showcase: showcase.map(ok), season, tier, prestige, rank, fav, stats })
    }
    /// The words under the name.
    pub fn title_text(&self) -> Option<&'static str> {
        reg().items.get(self.title as usize).map(|i| i.name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> Registry {
        let mut r = Registry::default();
        r.add_file(BUILT_IN, "season1.txt");
        r
    }

    #[test]
    fn the_season_file_reads_whole() {
        let r = registry();
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        let s = r.season(1).unwrap();
        assert_eq!(s.name, "The Red Moon");
        assert!(s.theme.contains("Ashblood Regent"));
        assert_eq!((s.weeks, date_of(s.start).as_str()), (10, "2026-11-23"));
        assert_eq!(date_of(s.end()), "2027-02-01");
        assert!(s.tiers.iter().all(|t| t.is_some()), "a reward at every tier");
        assert!(matches!(r.items[s.tiers[49].unwrap()].reward, Reward::Pet { .. }), "the pet at tier 50");
        assert_eq!(s.challenges.len(), 10);
        assert!(s.ranks.len() >= 5 && s.prestige.len() >= 3);
        // every kind the brief names is on the track
        for kind in ["Outfit", "Dye", "Weapon finish", "Emote", "Title", "Banner", "Nameplate", "Pet"] {
            assert!(s.tiers.iter().flatten().any(|&i| r.items[i].reward.kind() == kind), "{kind}");
        }
        // a broken line is reported, the rest still read
        let mut bad = Registry::default();
        bad.add_file("id = 9\nname = X\nstart = 2027-03-01\ntier 3 = dye | Not A Dye\ntier 4 = title | Fine\nwhat = ever\n", "season9.txt");
        assert_eq!(bad.problems.len(), 2, "{:?}", bad.problems);
        assert!(bad.season(9).unwrap().tiers[3].is_some());
        assert_eq!(day_of("2026-11-23"), Some(20_780));
        assert_eq!(date_of(20_780), "2026-11-23");
        assert_eq!((day_of("1970-01-01"), date_of(0).as_str()), (Some(0), "1970-01-01"));
        assert_eq!(20_780 % 7, 4, "a Monday");
    }

    #[test]
    fn xp_adds_up_and_tiers_unlock_their_rewards() {
        let r = registry();
        let s = r.season(1).unwrap();
        let day = s.start + 3;
        let mut p = Progress::new(7);
        assert!(p.roll(&r, day).is_some(), "the season begins");
        // a match: the career's XP, then a vault boss, a clear, a table win against the Hard bot
        let ctx = Ctx { body: 1, duel: true, ranked: false };
        p.feed(&r, day, CareerEv::Kill { head: true, finisher: 0 }, ctx, 15, false);
        p.feed(&r, day, CareerEv::MatchEnd, ctx, 25, false);
        p.boss_down(&r, day, 1, false);
        p.vault_cleared(&r, day, false, false);
        p.feed(&r, day, CareerEv::TableWin { kind: 0, hard: true }, ctx, 0, false);
        let challenge_xp: u32 = p.notes.iter().filter_map(|n| n.split("(+").nth(1)).filter_map(|x| x.trim_end_matches(" XP)").parse::<u32>().ok()).sum();
        assert_eq!(p.xp, 15 + 25 + XP_BOSS + XP_VAULT + XP_TABLE_HARD + challenge_xp);
        // tiers: each threshold gives its reward once
        let mut q = Progress::new(1);
        q.roll(&r, day);
        q.add_xp(&r, day, s.tier_xp * 3 - 1);
        assert_eq!(q.tier(s).0, 2);
        assert_eq!(q.owned.len(), 2);
        q.add_xp(&r, day, 1);
        assert_eq!(q.tier(s).0, 3);
        assert!(q.owned.contains("s1.t3"));
        // to the end of the track: the pet, then prestige and its border
        q.add_xp(&r, day, s.tier_xp * TIERS);
        assert_eq!(q.tier(s).0, TIERS);
        assert!(q.owned.contains("s1.t50"));
        assert!(q.tier(s).3 >= 1 && q.border(s).is_some());
        assert_eq!(q.owned.iter().filter(|k| k.contains(".t")).count(), 50);
        // no season on: no XP
        let mut off = Progress::new(1);
        off.roll(&r, s.start - 1);
        off.add_xp(&r, s.start - 1, 999);
        assert_eq!(off.xp, 0);
    }

    #[test]
    fn challenges_roll_by_utc_day_and_week_with_one_reroll() {
        let r = registry();
        let s = r.season(1).unwrap();
        let day = s.start + 2;
        let mut a = Progress::new(11);
        a.roll(&r, day);
        assert_eq!((a.daily.len(), a.weekly.len(), a.sprog.len()), (DAILIES, WEEKLIES, 10));
        let mut dd: Vec<u16> = a.daily.iter().map(|x| x.idx).collect();
        dd.dedup();
        assert_eq!(dd.len(), DAILIES, "three different dailies");
        // the same player gets the same set all day; another player another set
        let before = a.daily.clone();
        a.roll(&r, day);
        assert_eq!(a.daily, before);
        let mut b = Progress::new(12);
        b.roll(&r, day);
        let differ = (0..20).any(|k| {
            let mut x = Progress::new(100 + k);
            x.roll(&r, day);
            x.daily != a.daily
        });
        assert!(differ);
        // the next UTC day: new dailies, same weeklies (same week)
        let weekly = a.weekly.clone();
        a.roll(&r, day + 1);
        assert_eq!(a.weekly, weekly, "weeklies keep until Monday");
        // Monday 00:00 UTC: new weeklies
        let monday = (crate::career::week_of(day) + 1) * 7 - 3;
        assert_eq!(monday % 7, 4, "1970-01-05 was a Monday: day % 7 == 4 on Mondays");
        a.roll(&r, monday);
        assert_ne!(a.weekly, weekly);
        // the reset clocks
        let now = monday as u64 * 86_400 + 3600;
        let (d, w) = resets_in(now);
        assert_eq!(d, 23 * 3600);
        assert_eq!(w, 7 * 86_400 - 3600);
        // one free reroll a day, never into a challenge already showing
        assert!(a.reroll(0, monday));
        let set: Vec<u16> = a.daily.iter().map(|x| x.idx).collect();
        assert_eq!(set.iter().collect::<std::collections::BTreeSet<_>>().len(), DAILIES);
        assert!(!a.reroll(1, monday), "one a day");
        a.roll(&r, monday + 1);
        assert!(a.reroll(1, monday + 1), "tomorrow again");
    }

    #[test]
    fn challenges_count_the_right_things_and_admin_sessions_count_nothing() {
        let r = registry();
        let s = r.season(1).unwrap();
        let day = s.start + 5;
        let mut p = Progress::new(3);
        p.roll(&r, day);
        p.daily = vec![Slot { idx: DAILY.iter().position(|d| d.0 == What::Parries).unwrap() as u16, ..Default::default() }, Slot { idx: DAILY.iter().position(|d| d.0 == What::RooftopUnder(50)).unwrap() as u16, ..Default::default() }, Slot { idx: DAILY.iter().position(|d| d.0 == What::Table(0)).unwrap() as u16, ..Default::default() }];
        p.weekly = vec![Slot { idx: WEEKLY.iter().position(|d| d == &(What::WinsAs(1), 3)).unwrap() as u16, ..Default::default() }, Slot { idx: WEEKLY.iter().position(|d| d.0 == What::Boss(1)).unwrap() as u16, ..Default::default() }];
        let sable = Ctx { body: 1, duel: true, ranked: false };
        // an admin session: nothing at all
        for _ in 0..20 {
            p.feed(&r, day, CareerEv::Parry, sable, 0, true);
        }
        p.feed(&r, day, CareerEv::MatchWon, sable, 300, true);
        p.boss_down(&r, day, 1, true);
        p.vault_cleared(&r, day, true, true);
        assert_eq!((p.xp, p.daily[0].prog, p.weekly[1].prog), (0, 0, 0));
        // the real thing
        for _ in 0..15 {
            p.feed(&r, day, CareerEv::Parry, sable, 0, false);
        }
        assert!(p.daily[0].done);
        p.feed(&r, day, CareerEv::Rooftop { ms: 51_000 }, sable, 0, false);
        assert!(!p.daily[1].done, "51 s isn't under 50");
        p.feed(&r, day, CareerEv::Rooftop { ms: 49_500 }, sable, 0, false);
        assert!(p.daily[1].done);
        p.feed(&r, day, CareerEv::TableWin { kind: 1, hard: false }, sable, 0, false);
        assert!(!p.daily[2].done, "checkers isn't chess");
        p.feed(&r, day, CareerEv::TableWin { kind: 0, hard: false }, sable, 0, false);
        assert!(p.daily[2].done);
        for _ in 0..3 {
            p.feed(&r, day, CareerEv::MatchWon, Ctx { body: 0, ..sable }, 0, false);
        }
        assert_eq!(p.weekly[0].prog, 0, "wins as Corvin don't count for Sable");
        for _ in 0..3 {
            p.feed(&r, day, CareerEv::MatchWon, sable, 0, false);
        }
        assert!(p.weekly[0].done);
        p.boss_down(&r, day, 1, false);
        assert!(p.weekly[1].done, "the Twin Assessors");
        // a season challenge with a reward of its own
        let k = s.challenges.iter().position(|c| c.what == What::VaultHard).unwrap();
        p.vault_cleared(&r, day, true, false);
        assert!(p.sprog[k].done);
        assert!(p.owned.contains(&format!("s1.c{}", k + 1)));
        // done challenges paid their XP once
        let xp = p.xp;
        p.feed(&r, day, CareerEv::Parry, sable, 0, false);
        assert_eq!(p.xp, xp);
    }

    #[test]
    fn a_new_season_resets_the_track_and_pays_the_peak_rank() {
        let mut r = registry();
        r.add_file("id = 2\nname = Second\nstart = 2027-02-01\ntier 1 = title | Two\n", "season2.txt");
        let s1 = r.season(1).unwrap().clone();
        let mut p = Progress::new(5);
        p.roll(&r, s1.start);
        p.add_xp(&r, s1.start, 10_000);
        p.rank_seen(Some(4), false);
        p.rank_seen(Some(6), true);
        assert_eq!(p.peak, 5, "admin sessions don't raise the peak");
        let ro = p.roll(&r, s1.end()).expect("season 2 begins");
        assert_eq!((ro.from, ro.to, p.season, p.xp), (1, 2, 2, 0));
        // Gold Writ peak: the rewards for Iron, Brass, Silver and Gold
        for t in 1..=4 {
            assert!(p.owned.contains(&format!("s1.r{t}")), "rank {t}");
        }
        assert!(!p.owned.contains("s1.r5"));
        assert!(p.owned.contains("s1.t3"), "what was earned is kept");
        let mut rating = crate::ranked::Rating { r: 1900.0, rd: 60.0, ..Default::default() };
        soft_reset(&mut rating);
        assert_eq!((rating.r, rating.rd), (1700.0, 250.0));
    }

    #[test]
    fn the_record_and_card_round_trip() {
        let r = registry();
        let day = r.season(1).unwrap().start;
        let mut p = Progress::new(99);
        p.roll(&r, day);
        p.add_xp(&r, day, 30_000);
        assert!(p.wear(&r, "s1.t3"));
        assert!(p.wear(&r, "s1.t1"));
        assert!(!p.wear(&r, "s1.t40"), "not earned");
        p.showcase("s1.t4");
        p.played(Played { when: 5, mode: "Duel".into(), map: "Sunfall Keep".into(), won: true, kills: 3, deaths: 1, xp: 120, ranked: true });
        let back = Progress::from_text(&p.to_text());
        assert_eq!(back.owned, p.owned);
        assert_eq!(back.equip, p.equip);
        assert_eq!((back.xp, back.seed, back.history.len()), (p.xp, 99, 1));
        assert_eq!(back.daily, p.daily);
        let c = Card { name: "Wren".into(), banner: 3, title: 0, nameplate: NONE, showcase: [1, NONE, 2], season: 1, tier: 12, prestige: 0, rank: 0x23, fav: 1, stats: [1, 2, 3, 4, 5, 6] };
        assert_eq!(Card::from_bytes(&c.to_bytes()), Some(c.clone()));
        assert!(c.to_bytes().len() < 80);
        assert_eq!(Card::from_bytes(&c.to_bytes()[..10]), None);
    }

    #[test]
    fn rewards_map_to_loadout_numbers() {
        let r = reg();
        let s = r.season(1).unwrap();
        let outfit = s.tiers.iter().flatten().copied().find(|&i| matches!(r.items[i].reward, Reward::Outfit { .. })).unwrap();
        let n = outfit_number(outfit).unwrap();
        assert!(n as usize >= crate::career::OUTFITS.len() && n <= 15);
        assert!(outfit_colours(n).is_some());
        let fin = s.tiers.iter().flatten().copied().find(|&i| matches!(r.items[i].reward, Reward::Finish { .. })).unwrap();
        assert_eq!(finish(finish_number(fin).unwrap()).map(|f| f.0), Some([150, 46, 40]));
        let pet = s.tiers[49].unwrap();
        assert_eq!(pet_value(pet), Some(2));
        assert!(pet_colour(2).is_some());
        assert!(dye_locked(crate::gear::SEASON_DYES), "season dyes locked by default");
        assert!(!dye_locked(0));
    }
}
