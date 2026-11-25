//! Ranked 1v1: a hidden skill rating (Glicko-2: a rating, how sure it is (its deviation) and how
//! steady the player is (volatility)) behind visible ranks with their own names: Ash, Iron, Brass,
//! Silver Writ, Gold Writ, Court and Magistrate (three divisions each below Magistrate), and the
//! Ledger of Champions for the top hundred. Five placement matches come first (the rank shows
//! after them). A ranked match is a Duel under fixed, fair rules (`RULES`): first to three rounds,
//! a set map pool, no Ledger Duels gear, never in a session that used admin commands. Leaving
//! early counts as a loss: a match in play is written down as lost before it starts (`begin`), and
//! only the real result replaces it; a game closed mid-match finds it on the next start.
//!
//! The profile is kept per Steam account on this computer, sealed with a checksum like the bank
//! (crate::items) so it can't be edited by hand. Matchmaking (Steam lobbies by rating and region)
//! is in main.rs's ranked queue; games joined by IP address are never ranked.

use crate::game::Mode;
use crate::world::MapId;

/// Glicko-2's scale factor (rating points per unit of its inner scale).
const SCALE: f64 = 173.7178;
/// The system constant: how much volatility may change (Glickman suggests 0.3 to 1.2).
const TAU: f64 = 0.5;
pub const START_RATING: f64 = 1500.0;
pub const START_RD: f64 = 350.0;
pub const START_VOL: f64 = 0.06;
/// The deviation never drops below this (ratings keep moving a little) or rises above the start.
pub const MIN_RD: f64 = 45.0;
/// Matches before a rank is shown.
pub const PLACEMENTS: u32 = 5;

/// A player's hidden skill rating.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rating {
    pub r: f64,
    pub rd: f64,
    pub vol: f64,
}

impl Default for Rating {
    fn default() -> Rating {
        Rating { r: START_RATING, rd: START_RD, vol: START_VOL }
    }
}

fn g(phi: f64) -> f64 {
    1.0 / (1.0 + 3.0 * phi * phi / (std::f64::consts::PI * std::f64::consts::PI)).sqrt()
}

/// One rating period's update (Glickman's Glicko-2): `games` are (the opponent's rating, the score:
/// 1 a win, 0 a loss, 0.5 a draw). No games: only the deviation grows.
pub fn update(p: Rating, games: &[(Rating, f64)]) -> Rating {
    let mu = (p.r - START_RATING) / SCALE;
    let phi = p.rd / SCALE;
    if games.is_empty() {
        let phi2 = (phi * phi + p.vol * p.vol).sqrt();
        return Rating { r: p.r, rd: (phi2 * SCALE).min(START_RD), vol: p.vol };
    }
    let mut v_inv = 0.0;
    let mut sum = 0.0;
    for (o, s) in games {
        let mu_j = (o.r - START_RATING) / SCALE;
        let phi_j = o.rd / SCALE;
        let gj = g(phi_j);
        let e = 1.0 / (1.0 + (-gj * (mu - mu_j)).exp());
        v_inv += gj * gj * e * (1.0 - e);
        sum += gj * (s - e);
    }
    let v = 1.0 / v_inv.max(1e-12);
    let delta = v * sum;
    // the new volatility (the Illinois algorithm on f(x) = 0)
    let a = (p.vol * p.vol).ln();
    let f = |x: f64| {
        let ex = x.exp();
        let d = phi * phi + v + ex;
        ex * (delta * delta - phi * phi - v - ex) / (2.0 * d * d) - (x - a) / (TAU * TAU)
    };
    let mut big_a = a;
    let mut big_b = if delta * delta > phi * phi + v {
        (delta * delta - phi * phi - v).ln()
    } else {
        let mut k = 1.0;
        while f(a - k * TAU) < 0.0 && k < 100.0 {
            k += 1.0;
        }
        a - k * TAU
    };
    let (mut fa, mut fb) = (f(big_a), f(big_b));
    let mut n = 0;
    while (big_b - big_a).abs() > 1e-6 && n < 100 {
        let c = big_a + (big_a - big_b) * fa / (fb - fa);
        let fc = f(c);
        if fc * fb <= 0.0 {
            big_a = big_b;
            fa = fb;
        } else {
            fa /= 2.0;
        }
        big_b = c;
        fb = fc;
        n += 1;
    }
    let vol = (big_a / 2.0).exp();
    let phi_star = (phi * phi + vol * vol).sqrt();
    let phi_new = 1.0 / (1.0 / (phi_star * phi_star) + 1.0 / v).sqrt();
    let mu_new = mu + phi_new * phi_new * sum;
    Rating { r: mu_new * SCALE + START_RATING, rd: phi_new * SCALE, vol }
}

/// The chance `a` beats `b` (Glicko-2's expectation, both deviations counted).
pub fn win_chance(a: Rating, b: Rating) -> f64 {
    let phi = ((a.rd * a.rd + b.rd * b.rd).sqrt()) / SCALE;
    1.0 / (1.0 + (-g(phi) * (a.r - b.r) / SCALE).exp())
}

// ---------------------------------------------------------------- ranks

/// The visible ranks: name, the rating they start at, their colour.
pub const TIERS: [(&str, f64, [u8; 3]); 8] = [
    ("Ash", 0.0, [140, 134, 128]),
    ("Iron", 1150.0, [120, 140, 160]),
    ("Brass", 1300.0, [205, 150, 70]),
    ("Silver Writ", 1450.0, [205, 215, 230]),
    ("Gold Writ", 1600.0, [255, 205, 80]),
    ("Court", 1750.0, [90, 200, 170]),
    ("Magistrate", 1900.0, [215, 70, 70]),
    ("Ledger of Champions", 1900.0, [190, 120, 255]),
];
/// The Ledger of Champions: the top hundred (on the Steam leaderboard) at Magistrate or above.
pub const LEDGER: u8 = 7;
pub const CHAMPIONS: u32 = 100;

/// A visible rank: its tier (0 Ash .. 7 the Ledger) and division (2 = III, 1 = II, 0 = I; Magistrate
/// and the Ledger have none).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rank {
    pub tier: u8,
    pub div: u8,
}

impl Rank {
    /// The rank for a rating (and a leaderboard place, 0 when unknown).
    pub fn of(r: f64, board: u32) -> Rank {
        let mut tier = 0;
        for (k, t) in TIERS.iter().enumerate().take(7) {
            if r >= t.1 {
                tier = k;
            }
        }
        if tier == 6 {
            let ledger = board > 0 && board <= CHAMPIONS;
            return Rank { tier: if ledger { LEDGER } else { 6 }, div: 0 };
        }
        let lo = TIERS[tier].1;
        let hi = TIERS[tier + 1].1;
        let width = if tier == 0 { 150.0 } else { hi - lo };
        let from = if tier == 0 { hi - width } else { lo };
        let k = ((r - from) / width * 3.0).floor().clamp(0.0, 2.0) as u8;
        Rank { tier: tier as u8, div: 2 - k }
    }
    pub fn name(self) -> String {
        let t = TIERS[self.tier.min(7) as usize].0;
        if self.tier >= 6 {
            t.to_string()
        } else {
            format!("{t} {}", ["I", "II", "III"][self.div.min(2) as usize])
        }
    }
    pub fn colour(self) -> [u8; 3] {
        TIERS[self.tier.min(7) as usize].2
    }
    /// The rank as one byte for the network (0: unranked or in placement).
    pub fn to_byte(r: Option<Rank>) -> u8 {
        r.map_or(0, |k| 1 + k.tier.min(7) * 3 + k.div.min(2))
    }
    pub fn from_byte(b: u8) -> Option<Rank> {
        (b > 0).then(|| Rank { tier: ((b - 1) / 3).min(7), div: (b - 1) % 3 })
    }
    /// The badge's short mark (a letter or two).
    pub fn mark(self) -> &'static str {
        ["A", "Fe", "Br", "SW", "GW", "C", "M", "L"][self.tier.min(7) as usize]
    }
}

// ---------------------------------------------------------------- the rules

/// A ranked match's fixed rules: the mode, rounds to win and the maps it may be played on.
pub struct Rules {
    pub mode: Mode,
    pub target: u16,
    pub maps: &'static [MapId],
}

pub const RULES: Rules = Rules { mode: Mode::Duel, target: 3, maps: &[MapId::Arena, MapId::CloseQuarters, MapId::Courtyard] };

/// The map for a ranked match (the same on both computers: from both players' ids).
pub fn pick_map(a: u64, b: u64) -> MapId {
    let k = (a ^ b).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 33;
    RULES.maps[(k as usize) % RULES.maps.len()]
}

/// Why a match can't count as ranked (None: it can). `admin`: the session used admin commands;
/// `gear`: Ledger Duels gear is worn; `ip`: the game was joined by address.
pub fn void_reason(mode: Mode, admin: bool, gear: bool, ip: bool) -> Option<&'static str> {
    if ip {
        Some("games joined by IP are never ranked")
    } else if admin {
        Some("this session used admin commands")
    } else if gear {
        Some("Ledger Duels gear isn't allowed in ranked")
    } else if mode != RULES.mode {
        Some("ranked is a Duel")
    } else {
        None
    }
}

// ---------------------------------------------------------------- the profile

/// One match's result, kept for the history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Played {
    pub day: u32,
    pub won: bool,
    pub delta: f64,
    pub opp: f64,
    /// It counted as a loss because someone left early (us: true; the opponent left: we won).
    pub left: bool,
}

/// A match that has started and isn't settled yet (the opponent's rating): a loss if we never
/// come back to settle it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pending {
    pub opp: Rating,
}

/// What a result did, for the post-match screen and its animation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Change {
    pub before: Option<Rank>,
    pub after: Option<Rank>,
    /// The hidden rating's change (shown as points), and the rating after.
    pub delta: f64,
    pub rating: f64,
    pub won: bool,
    /// Placement: matches played of five (0 once placed before this one).
    pub placement: u32,
    pub left: bool,
}

impl Change {
    pub fn ranked_up(&self) -> bool {
        matches!((self.before, self.after), (Some(a), Some(b)) if b > a) || (self.before.is_none() && self.after.is_some())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub rating: Rating,
    pub games: u32,
    pub wins: u32,
    pub peak: f64,
    /// The last day a rating period was counted (the deviation grows on days without games).
    pub day: u32,
    pub pending: Option<Pending>,
    /// Place on the Steam leaderboard (0: unknown).
    pub board: u32,
    pub history: Vec<Played>,
}

impl Default for Profile {
    fn default() -> Profile {
        Profile { rating: Rating::default(), games: 0, wins: 0, peak: START_RATING, day: 0, pending: None, board: 0, history: Vec::new() }
    }
}

impl Profile {
    /// The visible rank (None during placement).
    pub fn rank(&self) -> Option<Rank> {
        (self.games >= PLACEMENTS).then(|| Rank::of(self.rating.r, self.board))
    }
    pub fn placed(&self) -> u32 {
        self.games.min(PLACEMENTS)
    }
    /// The deviation grows for each day nobody played (up to its start).
    pub fn age_to(&mut self, today: u32) {
        if self.day == 0 {
            self.day = today;
            return;
        }
        let idle = today.saturating_sub(self.day).min(400);
        for _ in 0..idle {
            self.rating = update(self.rating, &[]);
        }
        self.day = today.max(self.day);
    }
    /// A ranked match is starting: until it's settled it stands as a loss.
    pub fn begin(&mut self, opp: Rating) {
        self.pending = Some(Pending { opp });
    }
    /// Settle a match: won or lost (`left`: someone left early, already decided who won).
    pub fn settle(&mut self, opp: Rating, won: bool, left: bool, today: u32) -> Change {
        self.age_to(today);
        let before = self.rank();
        let placement = if self.games < PLACEMENTS { self.games + 1 } else { 0 };
        let old = self.rating.r;
        let mut new = update(self.rating, &[(opp, if won { 1.0 } else { 0.0 })]);
        new.rd = new.rd.max(MIN_RD);
        self.rating = new;
        self.games += 1;
        if won {
            self.wins += 1;
        }
        self.peak = self.peak.max(new.r);
        self.pending = None;
        self.history.push(Played { day: today, won, delta: new.r - old, opp: opp.r, left });
        if self.history.len() > 50 {
            self.history.remove(0);
        }
        Change { before, after: self.rank(), delta: new.r - old, rating: new.r, won, placement, left }
    }
    /// The game starts: a match left unsettled last time (closed mid-match) counts as a loss now.
    pub fn settle_abandoned(&mut self, today: u32) -> Option<Change> {
        let p = self.pending?;
        Some(self.settle(p.opp, false, true, today))
    }

    pub fn to_text(&self) -> String {
        let r = self.rating;
        let mut s = format!("r={}\nrd={}\nvol={}\ngames={}\nwins={}\npeak={}\nday={}\nboard={}\n", r.r, r.rd, r.vol, self.games, self.wins, self.peak, self.day, self.board);
        if let Some(p) = self.pending {
            s += &format!("pending={},{},{}\n", p.opp.r, p.opp.rd, p.opp.vol);
        }
        for h in &self.history {
            s += &format!("h={},{},{},{},{}\n", h.day, h.won as u8, h.delta, h.opp, h.left as u8);
        }
        s
    }
    pub fn from_text(t: &str) -> Profile {
        let mut p = Profile::default();
        for line in t.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let num = |d: f64| v.trim().parse::<f64>().ok().filter(|x| x.is_finite()).unwrap_or(d);
            match k {
                "r" => p.rating.r = num(START_RATING),
                "rd" => p.rating.rd = num(START_RD).clamp(MIN_RD, START_RD),
                "vol" => p.rating.vol = num(START_VOL).clamp(0.001, 1.0),
                "games" => p.games = v.trim().parse().unwrap_or(0),
                "wins" => p.wins = v.trim().parse().unwrap_or(0),
                "peak" => p.peak = num(START_RATING),
                "day" => p.day = v.trim().parse().unwrap_or(0),
                "board" => p.board = v.trim().parse().unwrap_or(0),
                "pending" => {
                    let x: Vec<f64> = v.split(',').filter_map(|a| a.trim().parse().ok()).collect();
                    if x.len() == 3 {
                        p.pending = Some(Pending { opp: Rating { r: x[0], rd: x[1], vol: x[2] } });
                    }
                }
                "h" => {
                    let x: Vec<&str> = v.split(',').collect();
                    if x.len() == 5 {
                        p.history.push(Played { day: x[0].parse().unwrap_or(0), won: x[1] == "1", delta: x[2].parse().unwrap_or(0.0), opp: x[3].parse().unwrap_or(0.0), left: x[4] == "1" });
                    }
                }
                _ => {}
            }
        }
        p
    }
}

const KEY: &[u8] = b"duel-rush ranked ledger v1";

fn sum(account: u64, body: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(KEY);
    h.update(account.to_le_bytes());
    h.update(body.as_bytes());
    h.update(KEY);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn seal(p: &Profile, account: u64) -> String {
    let body = p.to_text();
    format!("{body}sum={}\n", sum(account, &body))
}

/// A sealed profile read back: None when it was changed outside the game (or is someone else's).
pub fn unseal(text: &str, account: u64) -> Option<Profile> {
    let t = text.replace("\r\n", "\n");
    let at = t.rfind("sum=")?;
    let (body, tail) = t.split_at(at);
    (tail.trim_start_matches("sum=").trim() == sum(account, body)).then(|| Profile::from_text(body))
}

pub fn path(account: u64) -> Option<std::path::PathBuf> {
    crate::platform::data_file(&if account == 0 { "ranked_local.dat".to_string() } else { format!("ranked_{account}.dat") })
}

/// This account's profile (a fresh one when there's none or it was edited by hand: a note says so).
pub fn load(account: u64) -> (Profile, Option<String>) {
    let Some(p) = path(account) else { return (Profile::default(), None) };
    let Ok(t) = std::fs::read_to_string(&p) else { return (Profile::default(), None) };
    match unseal(&t, account) {
        Some(prof) => (prof, None),
        None => {
            let _ = std::fs::rename(&p, p.with_extension("tampered"));
            (Profile::default(), Some("Your ranked record had been changed outside the game: it starts again from placement.".into()))
        }
    }
}

pub fn save(p: &Profile, account: u64) {
    if let Some(f) = path(account) {
        let _ = crate::platform::save_file(&f, &seal(p, account));
    }
}

// ---------------------------------------------------------------- matchmaking

/// How wide a search is after `secs` in the queue: the rating window (points either side) and how
/// far away (Steam's distance: 0 close, 1 the region, 2 far, 3 worldwide).
pub fn search_window(secs: f32) -> (i32, u8) {
    match secs {
        s if s < 15.0 => (150, 0),
        s if s < 35.0 => (250, 1),
        s if s < 60.0 => (400, 2),
        _ => (800, 3),
    }
}

/// Of the open ranked lobbies (lobby id, the host's rating, ping in ms if known), the best match:
/// inside the window, closest in rating, then quickest.
pub fn best_lobby(mine: f64, window: i32, rows: &[(u64, i32, Option<u32>)]) -> Option<u64> {
    rows.iter()
        .filter(|r| (r.1 as f64 - mine).abs() <= window as f64)
        .min_by_key(|r| ((r.1 as f64 - mine).abs() as i64 / 25, r.2.unwrap_or(250)))
        .map(|r| r.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Glickman's own worked example (his Glicko-2 paper): 1500 / 200 / 0.06 against three players.
    #[test]
    fn glicko2_matches_the_worked_example() {
        let p = Rating { r: 1500.0, rd: 200.0, vol: 0.06 };
        let games = [(Rating { r: 1400.0, rd: 30.0, vol: 0.06 }, 1.0), (Rating { r: 1550.0, rd: 100.0, vol: 0.06 }, 0.0), (Rating { r: 1700.0, rd: 300.0, vol: 0.06 }, 0.0)];
        let n = update(p, &games);
        assert!((n.r - 1464.06).abs() < 0.1, "rating {:.2}", n.r);
        assert!((n.rd - 151.52).abs() < 0.1, "deviation {:.2}", n.rd);
        assert!((n.vol - 0.05999).abs() < 0.0001, "volatility {:.5}", n.vol);
        // no games: only the deviation grows
        let idle = update(p, &[]);
        assert_eq!(idle.r, 1500.0);
        assert!(idle.rd > 200.0 && idle.rd < 201.0);
        // beating someone better is worth more than beating someone worse; the favourite is favoured
        let me = Rating::default();
        let strong = Rating { r: 1800.0, rd: 60.0, vol: 0.06 };
        let weak = Rating { r: 1200.0, rd: 60.0, vol: 0.06 };
        assert!(update(me, &[(strong, 1.0)]).r - 1500.0 > update(me, &[(weak, 1.0)]).r - 1500.0);
        assert!(win_chance(strong, weak) > 0.9 && (win_chance(me, me) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn ranks_divisions_and_the_ledger() {
        assert_eq!(Rank::of(900.0, 0).name(), "Ash III");
        assert_eq!(Rank::of(1149.0, 0).name(), "Ash I");
        assert_eq!(Rank::of(1150.0, 0).name(), "Iron III");
        assert_eq!(Rank::of(1520.0, 0).name(), "Silver Writ II");
        assert_eq!(Rank::of(1899.0, 0).name(), "Court I");
        assert_eq!(Rank::of(1950.0, 0).name(), "Magistrate");
        assert_eq!(Rank::of(1950.0, 42).name(), "Ledger of Champions");
        assert_eq!(Rank::of(1700.0, 42).name(), "Gold Writ I", "the top hundred must be Magistrate too");
        assert_eq!(Rank::of(1950.0, 101).tier, 6, "101st isn't in the Ledger");
        for b in 0..=24u8 {
            assert_eq!(Rank::to_byte(Rank::from_byte(b)), b, "byte {b} round trips");
        }
        assert!(Rank::of(1600.0, 0) > Rank::of(1599.0, 0));
    }

    #[test]
    fn placement_then_a_rank() {
        let mut p = Profile::default();
        let opp = Rating::default();
        for k in 1..=PLACEMENTS {
            assert!(p.rank().is_none(), "no rank during placement");
            let c = p.settle(opp, k % 2 == 1, false, 100);
            assert_eq!(c.placement, k);
        }
        assert!(p.rank().is_some(), "ranked after five");
        let c = p.settle(opp, true, false, 100);
        assert_eq!(c.placement, 0);
        // placement moves the rating far more than a settled player's match does
        let mut fresh = Profile::default();
        let early = fresh.settle(opp, true, false, 100).delta;
        let mut settled = Profile { rating: Rating { r: 1500.0, rd: 60.0, vol: 0.06 }, games: 40, ..Profile::default() };
        let late = settled.settle(opp, true, false, 100).delta;
        assert!(early > late * 3.0, "placement {early:.1} vs settled {late:.1}");
        // a rank up shows as one
        let mut near = Profile { rating: Rating { r: 1445.0, rd: 60.0, vol: 0.06 }, games: 20, ..Profile::default() };
        let c = near.settle(Rating { r: 1700.0, rd: 60.0, vol: 0.06 }, true, false, 100);
        assert!(c.ranked_up() && c.after.unwrap().tier == 3, "{c:?}");
    }

    /// Leaving early counts as a loss: the match stands lost from its start; settling it properly
    /// replaces that; a game closed mid-match settles it as lost on the next start.
    #[test]
    fn leaving_early_is_a_loss() {
        let opp = Rating::default();
        let mut p = Profile { games: 10, ..Profile::default() };
        p.begin(opp);
        let text = seal(&p, 7);
        // (the game was closed here)
        let mut back = unseal(&text, 7).expect("reads back");
        assert_eq!(back.pending, Some(Pending { opp }));
        let c = back.settle_abandoned(101).expect("an abandoned match");
        assert!(!c.won && c.left && c.delta < 0.0);
        assert_eq!(back.pending, None);
        assert_eq!(back.games, 11);
        assert!(back.settle_abandoned(101).is_none(), "only once");
        // settled properly: no penalty left behind
        let mut q = Profile { games: 10, ..Profile::default() };
        q.begin(opp);
        let c = q.settle(opp, true, false, 101);
        assert!(c.won && c.delta > 0.0 && q.pending.is_none());
        // the opponent left: a win for us
        let mut r = Profile { games: 10, ..Profile::default() };
        r.begin(opp);
        assert!(r.settle(opp, true, true, 101).won);
    }

    #[test]
    fn the_record_is_sealed() {
        let mut p = Profile::default();
        p.settle(Rating::default(), true, false, 5);
        let t = seal(&p, 99);
        assert_eq!(unseal(&t, 99), Some(p.clone()));
        assert!(unseal(&t.replace("wins=1", "wins=9"), 99).is_none(), "edited by hand");
        assert!(unseal(&t, 98).is_none(), "someone else's");
    }

    #[test]
    fn fair_rules_and_matchmaking() {
        assert!(void_reason(Mode::Duel, false, false, false).is_none());
        assert!(void_reason(Mode::Duel, true, false, false).is_some(), "admin sessions don't count");
        assert!(void_reason(Mode::Duel, false, true, false).is_some(), "no Ledger gear");
        assert!(void_reason(Mode::Duel, false, false, true).is_some(), "IP play stays unranked");
        assert!(void_reason(Mode::Ffa, false, false, false).is_some());
        assert_eq!(pick_map(5, 9), pick_map(9, 5), "both computers pick the same map");
        // the search widens with time; the closest rating wins, then the quickest
        assert!(search_window(5.0).0 < search_window(70.0).0 && search_window(70.0).1 == 3);
        let rows = [(1, 1700, Some(30)), (2, 1520, Some(200)), (3, 1490, Some(40)), (4, 2400, Some(5))];
        assert_eq!(best_lobby(1500.0, 150, &rows), Some(3));
        assert_eq!(best_lobby(1500.0, 150, &rows[..1]), None, "too far apart");
    }
}
