//! Career stats kept on this computer: what you've done across all your matches, the XP it
//! earned and your level; each fighter's mastery (levels from playing them, unlocking their
//! outfit colours, finishers, lore pages and a badge); the daily first-win bonus and three
//! weekly challenges.

use crate::game::{CareerEv, FINISHERS};
use std::path::PathBuf;

/// XP for each thing you do.
pub const XP_KILL: u32 = 10;
pub const XP_HEADSHOT: u32 = 5;
/// On top of the kill.
pub const XP_FINISHER: u32 = 50;
pub const XP_WAVE: u32 = 20;
pub const XP_MATCH: u32 = 25;
/// Taking down someone holding a Writ: this much per kill of their streak.
pub const XP_WRIT_PER_KILL: u32 = 50;
/// The first win of each day.
pub const XP_FIRST_WIN: u32 = 200;
/// XP from one level to the next.
pub const XP_PER_LEVEL: u32 = 500;

/// Fighters with a mastery track (see crate::chars).
pub const FIGHTERS: usize = 4;
/// Mastery XP from one mastery level to the next, and the top level.
pub const MASTERY_XP: u32 = 250;
pub const MASTERY_MAX: u32 = 20;

/// Mastery badges: name, the mastery level it's earned at, colour.
pub const BADGES: [(&str, u32, [u8; 3]); 6] = [
    ("Bronze", 1, [196, 128, 74]),
    ("Silver", 4, [200, 208, 220]),
    ("Gold", 7, [255, 200, 70]),
    ("Platinum", 10, [150, 230, 220]),
    ("Diamond", 14, [130, 180, 255]),
    ("Champion", 20, [255, 110, 90]),
];

/// Outfit colours (the coat or cape): name, mastery level, colour (None: the fighter's own).
pub const OUTFITS: [(&str, u32, Option<[u8; 3]>); 4] = [("Standard", 1, None), ("Ember", 3, Some([150, 42, 30])), ("Midnight", 6, Some([34, 40, 78])), ("Gilded", 10, Some([176, 134, 52]))];

/// Lore pages unlock at these mastery levels.
pub const LORE_LEVELS: [u32; 4] = [2, 5, 9, 13];

/// Each fighter's lore: (title, text) per page.
pub const LORE: [[(&str, &str); 4]; FIGHTERS] = [
    [
        ("The Watch on the Ridge", "The Ranger kept the ridge road for eleven winters. Travellers knew the hooded figure only by the lantern that never went out and the arrows that never missed."),
        ("Old Steel", "The blade at the Ranger's hip was forged for a captain who never came back from the pass. The Ranger carries it the way you carry a promise: quietly, and always."),
        ("Why the Arena", "When the roads closed, the purses moved to the arenas. The Ranger came for the coin and stayed for the only fights left that still had rules."),
        ("Calm", "The crowds call it coldness. Those who have faced the Ranger call it patience: every cut is decided before the blade moves."),
    ],
    [
        ("Forest-Born", "The Huntress learned to stalk before she learned to read, tracking stags through pines so thick the noon sun never touched the ground."),
        ("The Pauldrons", "Her shoulder guards were cut from the hide of the boar that took her father's hunting party. She wears the trophy, not the grief."),
        ("Prey and Predator", "In the arena she circles longer than anyone else, waiting for the moment the other fighter forgets they are being hunted."),
        ("A Name in the Trees", "The forest villages leave carved tokens at the arena gates before her bouts. She never mentions them, and never leaves without touching one."),
    ],
    [
        ("Dock Fighter", "The Brawler grew up on the harbour, settling cargo disputes with fists long before anyone handed him a sword taller than a deckhand."),
        ("The Greatsword", "He won the blade in a wager he says he can't remember. The smith who made it says he remembers it very well."),
        ("Showman", "He rests the steel on his shoulder between rounds because the crowd loves it. He swings it because he does."),
        ("Soft Spot", "Half his winnings go to the harbour orphanage. He'll deny it with a grin, and then buy the next round."),
    ],
    [
        ("Quick Feet", "The Striker was a courier in the hill cities, running messages across rooftops faster than any horse could take the streets."),
        ("First Blood", "A courier who can't fight doesn't stay a courier for long. The Striker learned the blade from the same rooftops, one ambush at a time."),
        ("Never Still", "Crowds count her steps between strikes. Nobody has ever seen her stand flat-footed in the ring."),
        ("The Last Message", "She still carries one sealed letter she was never able to deliver. She says she'll open it the day someone beats her clean."),
    ],
];

/// A weekly challenge: what to do, how many times, what it pays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Challenge {
    pub text: &'static str,
    pub goal: u32,
    pub xp: u32,
    kind: Count,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Count {
    Kills,
    Headshots,
    Finishers,
    Writs,
    Wins,
    MultiKills,
    Matches,
    Waves,
}

const POOL: [Challenge; 8] = [
    Challenge { text: "Get 40 kills", goal: 40, xp: 300, kind: Count::Kills },
    Challenge { text: "Land 15 headshot kills", goal: 15, xp: 300, kind: Count::Headshots },
    Challenge { text: "Perform 3 finishers", goal: 3, xp: 350, kind: Count::Finishers },
    Challenge { text: "Claim 2 Writs", goal: 2, xp: 350, kind: Count::Writs },
    Challenge { text: "Win 3 matches", goal: 3, xp: 400, kind: Count::Wins },
    Challenge { text: "Get 5 multi-kills", goal: 5, xp: 300, kind: Count::MultiKills },
    Challenge { text: "Play 5 matches", goal: 5, xp: 250, kind: Count::Matches },
    Challenge { text: "Clear 15 Gauntlet waves", goal: 15, xp: 300, kind: Count::Waves },
];

/// Days since 1970 (UTC).
pub fn today() -> u32 {
    (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) / 86_400) as u32
}

/// The week a day falls in (weeks start on Monday; 1970-01-01 was a Thursday).
pub fn week_of(day: u32) -> u32 {
    (day + 3) / 7
}

/// The three challenges of a week (the same for everyone that week).
pub fn weekly_set(week: u32) -> [Challenge; 3] {
    let mut h = week.wrapping_mul(2_654_435_761) ^ 0x9e37_79b9;
    let mut picked: Vec<usize> = Vec::new();
    while picked.len() < 3 {
        h ^= h >> 13;
        h = h.wrapping_mul(0x5bd1_e995);
        h ^= h >> 15;
        let k = (h as usize) % POOL.len();
        if !picked.contains(&k) {
            picked.push(k);
        }
    }
    [POOL[picked[0]], POOL[picked[1]], POOL[picked[2]]]
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Career {
    pub kills: u32,
    pub deaths: u32,
    pub headshots: u32,
    pub finishers: u32,
    /// Finishers done with each style (see game::FINISHERS).
    pub fin_by: [u32; FINISHERS.len()],
    pub matches: u32,
    pub waves: u32,
    pub xp: u32,
    /// Writs claimed, and matches won.
    pub writs: u32,
    pub wins: u32,
    /// Mastery XP with each fighter.
    pub mastery: [u32; FIGHTERS],
    /// The last day a first-win bonus was paid.
    pub last_win_day: u32,
    /// The week the challenges below belong to, how far each has got, and which have paid.
    pub week: u32,
    pub weekly: [u32; 3],
    pub weekly_done: [bool; 3],
    /// What just happened worth telling the player (not saved).
    pub notes: Vec<String>,
}

fn path() -> Option<PathBuf> {
    crate::platform::data_file("career.txt")
}

fn list<T: ToString>(v: &[T]) -> String {
    v.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(",")
}

impl Career {
    pub fn load() -> Career {
        path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| Career::from_text(&t)).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(p) = path() {
            let _ = crate::platform::save_file(&p, &self.to_text());
        }
    }

    pub fn to_text(&self) -> String {
        let done: Vec<u8> = self.weekly_done.iter().map(|&d| d as u8).collect();
        format!(
            "kills={}\ndeaths={}\nheadshots={}\nfinishers={}\nfin_by={}\nmatches={}\nwaves={}\nxp={}\nwrits={}\nwins={}\nmastery={}\nlast_win_day={}\nweek={}\nweekly={}\nweekly_done={}\n",
            self.kills,
            self.deaths,
            self.headshots,
            self.finishers,
            list(&self.fin_by),
            self.matches,
            self.waves,
            self.xp,
            self.writs,
            self.wins,
            list(&self.mastery),
            self.last_win_day,
            self.week,
            list(&self.weekly),
            list(&done)
        )
    }

    pub fn from_text(t: &str) -> Career {
        let mut c = Career::default();
        for line in t.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let n = || v.trim().parse::<u32>().unwrap_or(0);
            let nums = || v.split(',').map(|x| x.trim().parse::<u32>().unwrap_or(0)).collect::<Vec<u32>>();
            match k.trim() {
                "kills" => c.kills = n(),
                "deaths" => c.deaths = n(),
                "headshots" => c.headshots = n(),
                "finishers" => c.finishers = n(),
                "matches" => c.matches = n(),
                "waves" => c.waves = n(),
                "xp" => c.xp = n(),
                "writs" => c.writs = n(),
                "wins" => c.wins = n(),
                "last_win_day" => c.last_win_day = n(),
                "week" => c.week = n(),
                "fin_by" => {
                    for (slot, x) in c.fin_by.iter_mut().zip(nums()) {
                        *slot = x;
                    }
                }
                "mastery" => {
                    for (slot, x) in c.mastery.iter_mut().zip(nums()) {
                        *slot = x;
                    }
                }
                "weekly" => {
                    for (slot, x) in c.weekly.iter_mut().zip(nums()) {
                        *slot = x;
                    }
                }
                "weekly_done" => {
                    for (slot, x) in c.weekly_done.iter_mut().zip(nums()) {
                        *slot = x != 0;
                    }
                }
                _ => {}
            }
        }
        c
    }

    /// Count one thing (today, playing no fighter in particular); returns the XP it earned.
    pub fn apply(&mut self, ev: CareerEv) -> u32 {
        self.apply_as(ev, None, today())
    }

    /// Count one thing played as fighter `body` on `day`: the XP it earned (bonuses and
    /// challenges included). It also feeds that fighter's mastery, and leaves notes.
    pub fn apply_as(&mut self, ev: CareerEv, body: Option<u8>, day: u32) -> u32 {
        self.roll_week(day);
        let mut xp = match ev {
            CareerEv::Kill { head, finisher } => {
                self.kills += 1;
                self.count(Count::Kills);
                let mut xp = XP_KILL;
                if head {
                    self.headshots += 1;
                    self.count(Count::Headshots);
                    xp += XP_HEADSHOT;
                }
                if finisher > 0 {
                    self.finishers += 1;
                    self.count(Count::Finishers);
                    if let Some(n) = self.fin_by.get_mut(finisher as usize - 1) {
                        *n += 1;
                    }
                    xp += XP_FINISHER;
                }
                xp
            }
            CareerEv::Death => {
                self.deaths += 1;
                0
            }
            CareerEv::WaveCleared => {
                self.waves += 1;
                self.count(Count::Waves);
                XP_WAVE
            }
            CareerEv::MatchEnd => {
                self.matches += 1;
                self.count(Count::Matches);
                XP_MATCH
            }
            CareerEv::MatchWon => {
                self.wins += 1;
                self.count(Count::Wins);
                if self.last_win_day != day {
                    self.last_win_day = day;
                    self.notes.push(format!("First win of the day! +{XP_FIRST_WIN} XP"));
                    XP_FIRST_WIN
                } else {
                    0
                }
            }
            CareerEv::WritClaim { streak } => {
                self.writs += 1;
                self.count(Count::Writs);
                let xp = XP_WRIT_PER_KILL * streak.max(1);
                self.notes.push(format!("Writ claimed! +{xp} XP"));
                xp
            }
            CareerEv::MultiKill => {
                self.count(Count::MultiKills);
                0
            }
        };
        // challenges finished by this
        let set = weekly_set(self.week);
        for k in 0..3 {
            if !self.weekly_done[k] && self.weekly[k] >= set[k].goal {
                self.weekly_done[k] = true;
                xp += set[k].xp;
                self.notes.push(format!("Weekly challenge done: {} (+{} XP)", set[k].text, set[k].xp));
            }
        }
        self.xp += xp;
        if let Some(b) = body.filter(|&b| (b as usize) < FIGHTERS) {
            let before = self.mastery_level(b);
            self.mastery[b as usize] += xp;
            let after = self.mastery_level(b);
            if after > before {
                let name = crate::chars::name(b);
                let mut note = format!("{name} mastery {after}!");
                let unlocks = unlocks_at(b, after);
                if !unlocks.is_empty() {
                    note += &format!(" Unlocked: {}", unlocks.join(", "));
                }
                self.notes.push(note);
            }
        }
        xp
    }

    /// A new week: fresh challenges.
    fn roll_week(&mut self, day: u32) {
        let w = week_of(day);
        if w != self.week {
            self.week = w;
            self.weekly = [0; 3];
            self.weekly_done = [false; 3];
        }
    }

    fn count(&mut self, kind: Count) {
        let set = weekly_set(self.week);
        for k in 0..3 {
            if set[k].kind == kind && !self.weekly_done[k] {
                self.weekly[k] += 1;
            }
        }
    }

    /// This week's challenges with progress (for the menus).
    pub fn challenges(&self, day: u32) -> Vec<(Challenge, u32, bool)> {
        let w = week_of(day);
        let set = weekly_set(w);
        (0..3).map(|k| if w == self.week { (set[k], self.weekly[k].min(set[k].goal), self.weekly_done[k]) } else { (set[k], 0, false) }).collect()
    }

    /// Is today's first-win bonus still waiting?
    pub fn first_win_ready(&self, day: u32) -> bool {
        self.last_win_day != day
    }

    pub fn level(&self) -> u32 {
        1 + self.xp / XP_PER_LEVEL
    }

    /// Share of the way to the next level.
    pub fn level_progress(&self) -> f32 {
        (self.xp % XP_PER_LEVEL) as f32 / XP_PER_LEVEL as f32
    }

    pub fn mastery_level(&self, body: u8) -> u32 {
        (1 + self.mastery.get(body as usize).copied().unwrap_or(0) / MASTERY_XP).min(MASTERY_MAX)
    }

    pub fn mastery_progress(&self, body: u8) -> f32 {
        if self.mastery_level(body) >= MASTERY_MAX {
            return 1.0;
        }
        (self.mastery.get(body as usize).copied().unwrap_or(0) % MASTERY_XP) as f32 / MASTERY_XP as f32
    }

    /// The fighter's badge: (name, colour).
    pub fn badge(&self, body: u8) -> (&'static str, [u8; 3]) {
        let lv = self.mastery_level(body);
        let b = BADGES.iter().rev().find(|b| lv >= b.1).unwrap_or(&BADGES[0]);
        (b.0, b.2)
    }

    pub fn outfit_unlocked(&self, body: u8, k: usize) -> bool {
        OUTFITS.get(k).is_some_and(|o| self.mastery_level(body) >= o.1)
    }

    pub fn lore_unlocked(&self, body: u8, page: usize) -> bool {
        LORE_LEVELS.get(page).is_some_and(|&l| self.mastery_level(body) >= l)
    }

    /// The finisher style is open to this fighter (by their mastery).
    pub fn finisher_unlocked(&self, body: u8, style: usize) -> bool {
        style < FINISHERS.len() && self.level() >= FINISHERS[style].unlock_level && self.mastery_level(body) >= finisher_level(style)
    }

    /// Finisher styles still locked for this fighter, as a bit mask (for the game).
    pub fn finisher_locks(&self, body: u8) -> u32 {
        (0..FINISHERS.len()).filter(|&k| !self.finisher_unlocked(body, k)).fold(0, |m, k| m | 1 << k)
    }

    /// The finisher style used most (if any yet).
    pub fn favourite_finisher(&self) -> Option<usize> {
        let (k, n) = self.fin_by.iter().enumerate().max_by_key(|(_, n)| **n)?;
        (*n > 0).then_some(k)
    }

    pub fn unlocked(&self, style: usize) -> bool {
        FINISHERS.get(style).is_some_and(|s| self.level() >= s.unlock_level)
    }
}

/// The mastery level a finisher style opens at: each blade's first style from the start, its
/// second at 3, its third at 6; the aerial one at 4.
pub fn finisher_level(style: usize) -> u32 {
    if FINISHERS.get(style).is_some_and(|s| s.weapon.is_none()) {
        4
    } else {
        [1, 3, 6][style % 3]
    }
}

/// What reaching mastery `level` with a fighter unlocks.
pub fn unlocks_at(body: u8, level: u32) -> Vec<String> {
    let mut out = Vec::new();
    for o in OUTFITS.iter().filter(|o| o.1 == level && level > 1) {
        out.push(format!("{} outfit", o.0));
    }
    for (k, s) in FINISHERS.iter().enumerate() {
        if finisher_level(k) == level && level > 1 {
            out.push(format!("{} finisher", s.name));
        }
    }
    for (p, &l) in LORE_LEVELS.iter().enumerate() {
        if l == level {
            out.push(format!("lore: {}", LORE[(body as usize).min(FIGHTERS - 1)][p].0));
        }
    }
    for b in BADGES.iter().filter(|b| b.1 == level && level > 1) {
        out.push(format!("{} badge", b.0));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_earn_xp_and_survive_saving() {
        let mut c = Career::default();
        assert_eq!(c.apply(CareerEv::Kill { head: true, finisher: 0 }), XP_KILL + XP_HEADSHOT);
        assert_eq!(c.apply(CareerEv::Kill { head: false, finisher: 4 }), XP_KILL + XP_FINISHER);
        c.apply(CareerEv::Death);
        c.apply(CareerEv::MatchEnd);
        assert_eq!((c.kills, c.headshots, c.finishers, c.deaths, c.matches), (2, 1, 1, 1, 1));
        assert_eq!(c.fin_by[3], 1);
        assert_eq!(c.favourite_finisher(), Some(3));
        c.notes.clear();
        assert_eq!(Career::from_text(&c.to_text()), c);
        c.xp = XP_PER_LEVEL * 2 + XP_PER_LEVEL / 2;
        assert_eq!(c.level(), 3);
        assert!((c.level_progress() - 0.5).abs() < 1e-6);
        assert!((0..FINISHERS.len()).all(|k| c.unlocked(k)));
    }

    #[test]
    fn first_win_pays_once_a_day() {
        let mut c = Career::default();
        let day = 20_000;
        assert!(c.apply_as(CareerEv::MatchWon, None, day) >= XP_FIRST_WIN);
        assert!(c.apply_as(CareerEv::MatchWon, None, day) < XP_FIRST_WIN, "only the first one");
        assert!(c.apply_as(CareerEv::MatchWon, None, day + 1) >= XP_FIRST_WIN, "and again tomorrow");
    }

    #[test]
    fn writs_pay_per_streak_kill() {
        let mut c = Career::default();
        assert!(c.apply_as(CareerEv::WritClaim { streak: 4 }, None, 20_000) >= 200);
        assert_eq!(c.writs, 1);
    }

    #[test]
    fn weekly_challenges_complete_and_reset() {
        let day = 20_003;
        let set = weekly_set(week_of(day));
        assert!(set[0] != set[1] && set[1] != set[2] && set[0] != set[2], "three different challenges");
        let mut c = Career::default();
        // play until every challenge of the week is done
        for _ in 0..60 {
            for ev in [CareerEv::Kill { head: true, finisher: 1 }, CareerEv::WritClaim { streak: 3 }, CareerEv::MatchWon, CareerEv::MultiKill, CareerEv::MatchEnd, CareerEv::WaveCleared] {
                c.apply_as(ev, Some(0), day);
            }
        }
        assert_eq!(c.weekly_done, [true; 3]);
        // next week: fresh ones
        c.apply_as(CareerEv::Death, None, day + 7);
        assert_eq!(c.weekly_done, [false; 3]);
        assert_eq!(c.week, week_of(day + 7));
    }

    #[test]
    fn mastery_unlocks_outfits_finishers_lore_and_badges() {
        let mut c = Career::default();
        assert_eq!(c.mastery_level(1), 1);
        assert_eq!(c.badge(1).0, "Bronze");
        assert!(c.outfit_unlocked(1, 0) && !c.outfit_unlocked(1, 1));
        assert!(c.finisher_unlocked(1, 3) && !c.finisher_unlocked(1, 4), "the katana's first finisher only");
        assert!(!c.lore_unlocked(1, 0));
        // play the Huntress a long time
        for _ in 0..400 {
            c.apply_as(CareerEv::Kill { head: true, finisher: 0 }, Some(1), 20_000);
        }
        assert!(c.mastery_level(1) >= 20 && c.badge(1).0 == "Champion");
        assert!((0..OUTFITS.len()).all(|k| c.outfit_unlocked(1, k)));
        assert!((0..FINISHERS.len()).all(|k| c.finisher_unlocked(1, k)));
        assert_eq!(c.finisher_locks(1), 0);
        assert!(c.finisher_locks(0) != 0, "other fighters keep their own progress");
        assert!(c.notes.iter().any(|n| n.contains("Sable Rook mastery")));
        c.notes.clear();
        assert_eq!(Career::from_text(&c.to_text()), c);
    }
}
