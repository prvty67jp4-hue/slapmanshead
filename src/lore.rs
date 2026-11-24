//! The game's lore and voice lines, read from `assets/lore.txt` while the game runs, so the text
//! can be edited without touching the code (the game notices a saved change within a second).
//!
//! The file: `[section]` headers, then `key = value` lines; `#` starts a comment. A key can repeat
//! (several `kill =` lines): the game picks one of them at random each time.
//!
//! Where the file is looked for: the `DUEL_LORE` switch (a file path), then `assets/lore.txt`
//! beside the game's program, two folders up from it (a `cargo build`: target/release ->
//! app/assets), and in the folder the game runs from. If none is there (or it can't be read)
//! the copy built into the game is used, and anything missing from an edited file falls back
//! to sensible words rather than leaving a blank.

use crate::world::MapId;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Instant, SystemTime};

/// The copy built into the game (used when no file is found next to it).
pub static BUILT_IN: &str = include_str!("../assets/lore.txt");

/// Each fighter's section name, in the game's fighter order (see crate::chars).
pub const FIGHTER_KEYS: [&str; 4] = ["corvin", "sable", "anvil", "ilya"];
/// How many lore entries each fighter has (lore1 .. lore5).
pub const ENTRIES: usize = 5;

/// A parsed lore file: each section's keys, every value in file order.
#[derive(Default, Debug)]
pub struct Lore {
    sections: HashMap<String, Vec<(String, String)>>,
}

impl Lore {
    pub fn parse(text: &str) -> Lore {
        let mut sections: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut cur = String::new();
        for line in text.lines() {
            let t = line.trim().trim_start_matches('\u{feff}');
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            if t.starts_with('[') && t.ends_with(']') {
                cur = t[1..t.len() - 1].trim().to_lowercase();
                sections.entry(cur.clone()).or_default();
                continue;
            }
            if let Some((k, v)) = t.split_once('=') {
                let (k, v) = (k.trim().to_lowercase(), v.trim());
                if !k.is_empty() && !v.is_empty() {
                    sections.entry(cur.clone()).or_default().push((k, v.to_string()));
                }
            }
        }
        Lore { sections }
    }

    /// The first value of `key` in `[section]`.
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections.get(section)?.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// Every value of `key` in `[section]` (a repeated key gives several).
    pub fn all(&self, section: &str, key: &str) -> Vec<&str> {
        self.sections.get(section).map_or_else(Vec::new, |s| s.iter().filter(|(k, _)| k == key).map(|(_, v)| v.as_str()).collect())
    }
}

struct Loaded {
    lore: Arc<Lore>,
    /// The file it came from and when that was last changed (None: the built-in copy).
    from: Option<(PathBuf, Option<SystemTime>)>,
    checked: Instant,
}

fn slot() -> &'static RwLock<Option<Loaded>> {
    static S: OnceLock<RwLock<Option<Loaded>>> = OnceLock::new();
    S.get_or_init(|| RwLock::new(None))
}

/// Where the editable file is (the first that exists), if anywhere.
pub fn file_path() -> Option<PathBuf> {
    let mut tries = Vec::new();
    if let Some(p) = std::env::var_os("DUEL_LORE") {
        tries.push(PathBuf::from(p));
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
        tries.push(dir.join("assets").join("lore.txt"));
        if let Some(up) = dir.parent().and_then(|p| p.parent()) {
            tries.push(up.join("assets").join("lore.txt"));
        }
    }
    tries.push(PathBuf::from("assets").join("lore.txt"));
    tries.into_iter().find(|p| p.is_file())
}

fn load() -> Loaded {
    if let Some(p) = file_path() {
        if let Ok(text) = std::fs::read_to_string(&p) {
            let when = std::fs::metadata(&p).and_then(|m| m.modified()).ok();
            return Loaded { lore: Arc::new(Lore::parse(&text)), from: Some((p, when)), checked: Instant::now() };
        }
    }
    Loaded { lore: Arc::new(Lore::parse(BUILT_IN)), from: None, checked: Instant::now() }
}

/// The lore as it is now: read on first use, and again when the file has been saved since (it
/// is looked at once a second at most).
pub fn current() -> Arc<Lore> {
    {
        let g = slot().read().unwrap_or_else(|e| e.into_inner());
        if let Some(l) = g.as_ref() {
            if l.checked.elapsed().as_secs_f32() < 1.0 {
                return l.lore.clone();
            }
        }
    }
    let mut g = slot().write().unwrap_or_else(|e| e.into_inner());
    let stale = match g.as_ref() {
        None => true,
        Some(l) if l.checked.elapsed().as_secs_f32() < 1.0 => false,
        Some(l) => {
            let now = file_path();
            match (&l.from, now) {
                (Some((p, when)), Some(q)) => *p != q || std::fs::metadata(&q).and_then(|m| m.modified()).ok() != *when,
                (None, Some(_)) => true,
                (Some(_), None) => true,
                (None, None) => false,
            }
        }
    };
    if stale {
        *g = Some(load());
    } else if let Some(l) = g.as_mut() {
        l.checked = Instant::now();
    }
    g.as_ref().map(|l| l.lore.clone()).unwrap_or_default()
}

/// The text kept for the life of the game (each distinct string once), so lines can be handed out
/// as plain `&'static str` like the rest of the game's names.
fn intern(s: &str) -> &'static str {
    static SET: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut set = SET.get_or_init(|| Mutex::new(HashSet::new())).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&k) = set.get(s) {
        return k;
    }
    let k: &'static str = Box::leak(s.to_string().into_boxed_str());
    set.insert(k);
    k
}

/// `key` in `[section]` (the first if it repeats).
pub fn text(section: &str, key: &str) -> Option<&'static str> {
    current().get(section, key).map(intern)
}

/// `key` in `[section]`, or `fallback` when the file doesn't have it.
pub fn text_or(section: &str, key: &str, fallback: &'static str) -> &'static str {
    text(section, key).unwrap_or(fallback)
}

/// One of the values of a repeated key, picked by `seed` (any number: a different one gives a
/// different pick).
pub fn pick(section: &str, key: &str, seed: u64) -> Option<&'static str> {
    let lore = current();
    let all = lore.all(section, key);
    if all.is_empty() {
        return None;
    }
    // a quick integer mix, so neighbouring seeds don't pick neighbouring lines
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    Some(intern(all[(x % all.len() as u64) as usize]))
}

// ------------------------------------------------------------------ fighters

/// A fighter's section name (`fighter.corvin`).
pub fn fighter_section(body: u8) -> String {
    format!("fighter.{}", FIGHTER_KEYS[(body as usize).min(FIGHTER_KEYS.len() - 1)])
}

/// Their name ("Corvin Vale"); `fallback` when the file doesn't say.
pub fn fighter_name(body: u8, fallback: &'static str) -> &'static str {
    text_or(&fighter_section(body), "name", fallback)
}
pub fn fighter_title(body: u8) -> &'static str {
    text_or(&fighter_section(body), "title", "Fighter of the Rush")
}
pub fn fighter_bio(body: u8) -> &'static str {
    text_or(&fighter_section(body), "bio", "A fighter of the Rush. Their story isn't written down yet.")
}
pub fn fighter_why(body: u8) -> &'static str {
    text_or(&fighter_section(body), "why", "For the Writs, like everyone else.")
}
/// Their lore entries (lore1 .., up to ENTRIES; an entry the file leaves out reads as unwritten).
pub fn fighter_entries(body: u8) -> Vec<&'static str> {
    (1..=ENTRIES).map(|n| text_or(&fighter_section(body), &format!("lore{n}"), "This page of the story hasn't been written yet.")).collect()
}
/// Their rival (another fighter), if they have one.
pub fn rival(body: u8) -> Option<u8> {
    let r = text(&fighter_section(body), "rival")?;
    FIGHTER_KEYS.iter().position(|k| k.eq_ignore_ascii_case(r.trim())).map(|i| i as u8).filter(|&i| i != body)
}
/// What `body` says on meeting `other` (`vs_<other>`), if anything.
pub fn vs_line(body: u8, other: u8) -> Option<&'static str> {
    text(&fighter_section(body), &format!("vs_{}", FIGHTER_KEYS[(other as usize).min(FIGHTER_KEYS.len() - 1)]))
}

/// The moments a fighter speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Start,
    Kill,
    Finisher,
    Death,
}

impl Cue {
    pub fn key(self) -> &'static str {
        match self {
            Cue::Start => "start",
            Cue::Kill => "kill",
            Cue::Finisher => "finisher",
            Cue::Death => "death",
        }
    }
    fn fallback(self) -> &'static str {
        match self {
            Cue::Start => "Let's go.",
            Cue::Kill => "Down you go.",
            Cue::Finisher => "It's over.",
            Cue::Death => "Not like this...",
        }
    }
}

/// A fighter's line for a moment (one of theirs at random by `seed`; plain words if the file has none).
pub fn voice(body: u8, cue: Cue, seed: u64) -> &'static str {
    pick(&fighter_section(body), cue.key(), seed).unwrap_or(cue.fallback())
}

// ------------------------------------------------------------------ maps, the announcer, the world

/// A map's section name (`map.sunfall_keep`).
pub fn map_key(m: MapId) -> &'static str {
    match m {
        MapId::Arena => "sunfall_keep",
        MapId::Courtyard => "old_marble_court",
        MapId::CloseQuarters => "undercroft",
        MapId::Bridgeworks => "bridgeworks",
        MapId::Varrow => "varrow_centre",
    }
}
fn map_fallback(m: MapId) -> &'static str {
    match m {
        MapId::Arena => "Sunfall Keep",
        MapId::Courtyard => "Old Marble Court",
        MapId::CloseQuarters => "The Undercroft",
        MapId::Bridgeworks => "Varrow Bridgeworks",
        MapId::Varrow => "Varrow Centre",
    }
}
pub fn map_name(m: MapId) -> &'static str {
    text_or(&format!("map.{}", map_key(m)), "name", map_fallback(m))
}
pub fn map_lore(m: MapId) -> &'static str {
    text_or(&format!("map.{}", map_key(m)), "lore", "Another stretch of old stone where the Rush is fought.")
}

/// The announcer's moments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    MatchStart,
    RoundWin,
    MatchWin,
    WritClaimed,
}

impl Call {
    pub fn key(self) -> &'static str {
        match self {
            Call::MatchStart => "match_start",
            Call::RoundWin => "round_win",
            Call::MatchWin => "match_win",
            Call::WritClaimed => "writ_claimed",
        }
    }
    fn fallback(self) -> &'static str {
        match self {
            Call::MatchStart => "Fight!",
            Call::RoundWin => "The round is over.",
            Call::MatchWin => "The match is over.",
            Call::WritClaimed => "A Writ has been claimed!",
        }
    }
}

pub fn announcer(c: Call, seed: u64) -> &'static str {
    pick("announcer", c.key(), seed).unwrap_or(c.fallback())
}

/// The world's introduction for the Lore page: (heading, text) in reading order.
pub fn world_intro() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Varrow", text_or("world", "intro", "Varrow is a walled city where every quarrel is settled in the Rush.")),
        ("The Rush", text_or("world", "rush", "Two fighters, one bell, and nothing left to lose.")),
        ("The Ashen Court", text_or("world", "court", "Masked judges run the Rush and pay its winners.")),
        ("Writs", text_or("world", "writs", "Sealed parchments that settle debts, free prisoners and hand over houses.")),
    ]
}
pub fn world_title() -> &'static str {
    text_or("world", "title", "Varrow and the Rush")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built_in() -> Lore {
        Lore::parse(BUILT_IN)
    }

    /// Every fighter in the game has a name, title, bio, why, five entries, start / kill /
    /// finisher / death lines, and a rival line for their rival; every map has a name and a line.
    #[test]
    fn every_fighter_and_map_has_its_entries() {
        let l = built_in();
        assert_eq!(FIGHTER_KEYS.len(), crate::chars::COUNT);
        for b in 0..crate::chars::COUNT as u8 {
            let s = fighter_section(b);
            for key in ["name", "title", "bio", "why", "rival", "ability"] {
                assert!(l.get(&s, key).is_some(), "{s}: {key}");
            }
            for n in 1..=ENTRIES {
                assert!(l.get(&s, &format!("lore{n}")).is_some(), "{s}: lore{n}");
            }
            for cue in [Cue::Start, Cue::Kill, Cue::Finisher, Cue::Death] {
                assert!(!l.all(&s, cue.key()).is_empty(), "{s}: {}", cue.key());
            }
            let r = l.get(&s, "rival").unwrap();
            let ri = FIGHTER_KEYS.iter().position(|k| *k == r).unwrap_or_else(|| panic!("{s}: rival {r} is a fighter"));
            assert!(l.get(&s, &format!("vs_{}", FIGHTER_KEYS[ri])).is_some(), "{s}: a line for meeting their rival");
            // the game shows the file's names
            assert_eq!(crate::chars::name(b), l.get(&s, "name").unwrap());
        }
        for m in MapId::ALL {
            let s = format!("map.{}", map_key(m));
            assert!(l.get(&s, "name").is_some() && l.get(&s, "lore").is_some(), "{s}");
            assert_eq!(m.name(), l.get(&s, "name").unwrap());
        }
        for c in [Call::MatchStart, Call::RoundWin, Call::MatchWin, Call::WritClaimed] {
            assert!(l.get("announcer", c.key()).is_some(), "announcer {}", c.key());
        }
        for k in ["title", "intro", "court", "writs", "rush"] {
            assert!(l.get("world", k).is_some(), "world {k}");
        }
    }

    /// The renames: fighters and maps go by the file's names.
    #[test]
    fn fighters_and_maps_go_by_their_lore_names() {
        assert_eq!(crate::chars::name(0), "Corvin Vale");
        assert_eq!(crate::chars::name(1), "Sable Rook");
        assert_eq!(crate::chars::name(2), "Bram \"Anvil\" Kask");
        assert_eq!(crate::chars::name(3), "Ilya Fen");
        assert_eq!(MapId::Arena.name(), "Sunfall Keep");
        assert_eq!(MapId::CloseQuarters.name(), "The Undercroft");
        assert_eq!(MapId::Courtyard.name(), "Old Marble Court");
        assert_eq!(MapId::Bridgeworks.name(), "Varrow Bridgeworks");
        // the rivals pair up both ways
        assert_eq!(rival(0), Some(1));
        assert_eq!(rival(1), Some(0));
        assert_eq!(rival(2), Some(3));
        assert_eq!(rival(3), Some(2));
        assert_eq!(vs_line(0, 1), Some("You still owe me a name."));
    }

    /// Repeated keys are all kept and each gets picked; an unknown key or section gives nothing,
    /// and the game's helpers fall back to plain words instead of a blank.
    #[test]
    fn repeats_are_picked_and_missing_keys_fall_back() {
        let l = Lore::parse("# a comment\n[fighter.x]\nkill = One\nkill = Two\n  name =   Spaced  \nempty =\n[Other]\nk = v = w\n");
        assert_eq!(l.all("fighter.x", "kill"), vec!["One", "Two"]);
        assert_eq!(l.get("fighter.x", "name"), Some("Spaced"));
        assert_eq!(l.get("fighter.x", "empty"), None, "an empty value counts as missing");
        assert_eq!(l.get("other", "k"), Some("v = w"), "sections ignore case; values keep their '='");
        assert_eq!(l.get("fighter.x", "nope"), None);
        assert_eq!(l.get("nowhere", "kill"), None);
        // every start line gets picked by some seed
        let starts = built_in().all("fighter.corvin", "start").len();
        let mut seen = HashSet::new();
        for seed in 0..200 {
            seen.insert(voice(0, Cue::Start, seed));
        }
        assert_eq!(seen.len(), starts);
        // missing: plain fallbacks
        assert_eq!(pick("fighter.nobody", "kill", 1), None);
        assert_eq!(text_or("fighter.nobody", "title", "Fallback"), "Fallback");
        assert_eq!(Cue::Death.fallback(), "Not like this...");
        assert!(!Call::RoundWin.fallback().is_empty());
        assert_eq!(vs_line(0, 2), None, "no line for a fighter who isn't their rival");
    }

    /// An edited file on disk is read as it is (no rebuild needed), and the keys it leaves out
    /// fall back without a blank.
    #[test]
    fn an_edited_file_is_read_and_gaps_fall_back() {
        let dir = std::env::temp_dir().join(format!("duel_lore_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lore.txt");
        std::fs::write(&path, "[fighter.corvin]\nname = Corvin Edited\n[announcer]\nmatch_start = Edited bell!\n").unwrap();
        let lore = Lore::parse(&std::fs::read_to_string(&path).unwrap());
        assert_eq!(lore.get("fighter.corvin", "name"), Some("Corvin Edited"));
        assert_eq!(lore.get("announcer", "round_win"), None);
        // what the game would show for the gaps: fallbacks, never empty
        assert!(lore.get("fighter.corvin", "bio").is_none());
        assert!(!fighter_bio(9).is_empty() && !map_lore(MapId::Arena).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
