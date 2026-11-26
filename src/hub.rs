//! The Varrow Hub: the town square you load into (map: crate::varrow). Up to 16 players, hosted
//! like a match. Nobody can hurt anybody here, except two duellists in a Court Pit.
//!
//! - Fashionscape: nine slots (head, cape, neck, main hand, body, off hand, legs, hands, feet)
//!   filled from every look you've unlocked (Ledger gear looks, the new outfits), three dye zones
//!   per piece, ten saved outfits; the mirror, the catwalk's podium and photo mode are in main.rs.
//! - Emotes (wave, bow, dance, sit, cheer, salute, clap, laugh), and benches and ledges to sit on.
//! - Challenge Boards: challenge anyone near you (or post an open challenge) to a Duel or a Ledger
//!   Duel; on yes, both are brought into a free Court Pit, fight first to two (or the Ledger
//!   rules' best of), and come back out to the plaza.
//! - Aim rooms (crate::hub_aim): five drills, solo or two side by side on the same targets.
//! - The Rooftop Run: rings in order against the clock, with the ghost of your best run.
//! - The game tables: chess, checkers, Connect 4 (crate::boards) against a player or a bot.
//!
//! The host decides what's shared (seats, tables, pits, challenges, the scoreboards) and tells
//! everyone with HubEv events; each player keeps their own look, emote, aim room and run.

use crate::boards::{self, AnyMove, Board, GameKind, Level, Outcome};
use crate::game::*;
use crate::math::*;
use crate::varrow;

/// How many players a hub holds.
pub const HUB_PLAYERS: usize = 16;

// ===================================================================================== looks

/// The wardrobe's slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Head = 0,
    Cape = 1,
    Neck = 2,
    Main = 3,
    Body = 4,
    Off = 5,
    Legs = 6,
    Hands = 7,
    Feet = 8,
}

pub const SLOT_COUNT: usize = 9;

impl Slot {
    pub const ALL: [Slot; SLOT_COUNT] = [Slot::Head, Slot::Cape, Slot::Neck, Slot::Main, Slot::Body, Slot::Off, Slot::Legs, Slot::Hands, Slot::Feet];
    pub fn name(self) -> &'static str {
        ["Head", "Cape", "Neck", "Main hand", "Body", "Off hand", "Legs", "Hands", "Feet"][self as usize]
    }
}

/// How a look is earned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unlock {
    Always,
    /// Mastery level with any fighter.
    Mastery(u32),
    /// Finish the Rooftop Run.
    Rooftop,
    /// Beat a Hard bot at one of the game tables.
    TableHard,
    /// Win this many duels in the Court Pits.
    PitWins(u32),
    /// Found in the Ashen Vault (a crate::raid::LOOT id).
    Raid(u8),
}

impl Unlock {
    pub fn text(self) -> String {
        match self {
            Unlock::Always => "unlocked".into(),
            Unlock::Mastery(l) => format!("reach mastery {l} with any fighter"),
            Unlock::Rooftop => "finish the Rooftop Run".into(),
            Unlock::TableHard => "beat a Hard bot at a game table".into(),
            Unlock::PitWins(n) => format!("win {n} duels in the Court Pits"),
            Unlock::Raid(_) => "found in the Ashen Vault (the Vault Gate)".into(),
        }
    }
}

/// One look in the wardrobe: its slot, name and set, the model piece it shows (a GEAR_PIECES
/// id in crate::chars, 0 for none) or the held item (a crate::gear item id with a model, for the
/// hands), and how it's earned.
#[derive(Clone, Copy, Debug)]
pub struct LookItem {
    pub id: u8,
    pub slot: Slot,
    pub name: &'static str,
    pub set: &'static str,
    pub piece: u8,
    pub held: u8,
    pub unlock: Unlock,
}

const fn li(id: u8, slot: Slot, name: &'static str, set: &'static str, piece: u8, held: u8, unlock: Unlock) -> LookItem {
    LookItem { id, slot, name, set, piece, held, unlock }
}

pub const RIG: &str = "Bridgewright's Rig";
pub const HERALD: &str = "Court Herald";
pub const SMUGGLER: &str = "River Smuggler";
pub const PILGRIM: &str = "Ash Pilgrim";

/// Every look (id 0 is "nothing").
pub const LOOKS: &[LookItem] = &[
    // Ledger gear looks (the Bridgewright's Rig, and the Rigger's Hook and Snub)
    li(1, Slot::Head, "Riveted Half-Helm", RIG, 51, 0, Unlock::Always),
    li(2, Slot::Body, "Canvas Work Coat", RIG, 52, 0, Unlock::Always),
    li(3, Slot::Legs, "Padded Trousers", RIG, 53, 0, Unlock::Always),
    li(4, Slot::Hands, "Brass-Knuckle Gauntlets", RIG, 54, 0, Unlock::Always),
    li(5, Slot::Feet, "Hobnailed Boots", RIG, 55, 0, Unlock::Always),
    li(6, Slot::Neck, "Rigger's Gear Charm", RIG, 56, 0, Unlock::Always),
    li(7, Slot::Main, "Rigger's Hook", RIG, 0, 57, Unlock::Always),
    li(8, Slot::Off, "Rigger's Snub", RIG, 0, 58, Unlock::Always),
    // the Court Herald: the Ashen Court's crier, all heraldry and brass
    li(20, Slot::Head, "Plumed Herald's Cap", HERALD, 100, 0, Unlock::Mastery(5)),
    li(21, Slot::Cape, "Heraldic Half-Cape", HERALD, 101, 0, Unlock::Mastery(5)),
    li(22, Slot::Neck, "Chain of Office", HERALD, 102, 0, Unlock::Mastery(5)),
    li(23, Slot::Body, "Quartered Tabard", HERALD, 103, 0, Unlock::Mastery(5)),
    li(24, Slot::Legs, "Striped Hose", HERALD, 104, 0, Unlock::Mastery(5)),
    li(25, Slot::Hands, "Gauntleted Cuffs", HERALD, 105, 0, Unlock::Mastery(5)),
    li(26, Slot::Feet, "Buckled Court Shoes", HERALD, 106, 0, Unlock::Mastery(5)),
    // the River Smuggler: oilskin and rope, made for wet nights on the Varrow
    li(30, Slot::Head, "Oilskin Sou'wester", SMUGGLER, 110, 0, Unlock::Rooftop),
    li(31, Slot::Cape, "Oilcloth Short Cloak", SMUGGLER, 111, 0, Unlock::Rooftop),
    li(32, Slot::Neck, "Knotted Neckerchief", SMUGGLER, 112, 0, Unlock::Rooftop),
    li(33, Slot::Body, "Oilskin Waistcoat", SMUGGLER, 113, 0, Unlock::Rooftop),
    li(34, Slot::Legs, "Rolled Breeches", SMUGGLER, 114, 0, Unlock::Rooftop),
    li(35, Slot::Hands, "Fingerless Gloves", SMUGGLER, 115, 0, Unlock::Rooftop),
    li(36, Slot::Feet, "Wading Boots", SMUGGLER, 116, 0, Unlock::Rooftop),
    // the Ash Pilgrim: ash-grey wraps from the burnt chapels upriver
    li(40, Slot::Head, "Pilgrim's Deep Hood", PILGRIM, 120, 0, Unlock::TableHard),
    li(41, Slot::Cape, "Ashen Mantle", PILGRIM, 121, 0, Unlock::TableHard),
    li(42, Slot::Neck, "Censer Beads", PILGRIM, 122, 0, Unlock::TableHard),
    li(43, Slot::Body, "Wrapped Robe", PILGRIM, 123, 0, Unlock::TableHard),
    li(44, Slot::Legs, "Bound Leg-Wraps", PILGRIM, 124, 0, Unlock::TableHard),
    li(45, Slot::Hands, "Bandaged Hands", PILGRIM, 125, 0, Unlock::TableHard),
    li(46, Slot::Feet, "Pilgrim Sandals", PILGRIM, 126, 0, Unlock::TableHard),
    // the fighters' own blades and guns to hold (everyone has these)
    li(60, Slot::Main, "Katana", "Armoury", 0, 0, Unlock::Always),
    li(61, Slot::Main, "Greatsword", "Armoury", 0, 0, Unlock::Always),
    li(62, Slot::Main, "Dagger", "Armoury", 0, 0, Unlock::Always),
    li(63, Slot::Main, "Revolver", "Armoury", 0, 0, Unlock::Always),
    // the Ashen Vault's: the Vault Warden's plate, the Magistrate's robes, weapon finishes
    li(70, Slot::Head, "Vault Warden's Helm", WARDEN, 130, 0, Unlock::Raid(0)),
    li(71, Slot::Cape, "Vault Warden's Mantle", WARDEN, 131, 0, Unlock::Raid(1)),
    li(72, Slot::Neck, "Vault Warden's Gorget", WARDEN, 132, 0, Unlock::Raid(2)),
    li(73, Slot::Body, "Vault Warden's Cuirass", WARDEN, 133, 0, Unlock::Raid(3)),
    li(74, Slot::Legs, "Vault Warden's Greaves", WARDEN, 134, 0, Unlock::Raid(4)),
    li(75, Slot::Hands, "Vault Warden's Gauntlets", WARDEN, 135, 0, Unlock::Raid(5)),
    li(76, Slot::Feet, "Vault Warden's Sabatons", WARDEN, 136, 0, Unlock::Raid(6)),
    li(80, Slot::Head, "Magistrate's Coif", MAGISTRATE, 140, 0, Unlock::Raid(7)),
    li(81, Slot::Cape, "Cape of Verdicts", MAGISTRATE, 141, 0, Unlock::Raid(8)),
    li(82, Slot::Neck, "Ember Stole", MAGISTRATE, 142, 0, Unlock::Raid(9)),
    li(83, Slot::Body, "Robe of Sentence", MAGISTRATE, 143, 0, Unlock::Raid(10)),
    li(90, Slot::Main, "Ember-Gilt Katana", "Vault finishes", 0, 0, Unlock::Raid(11)),
    li(91, Slot::Main, "Ashen Greatsword", "Vault finishes", 0, 0, Unlock::Raid(12)),
    li(92, Slot::Main, "Ledger Quill Dagger", "Vault finishes", 0, 0, Unlock::Raid(13)),
    li(93, Slot::Main, "Verdict Revolver", "Vault finishes", 0, 0, Unlock::Raid(14)),
    // the Ashblood Regent's: his long coat, his glaive (a polearm that fights as a greatsword)
    li(84, Slot::Body, "The Regent's Long Coat", REGENT, 150, 0, Unlock::Raid(25)),
    li(94, Slot::Main, "Ashblood Glaive", "Vault finishes", 0, crate::weapons::GLAIVE, Unlock::Raid(27)),
];

pub const WARDEN: &str = "Vault Warden";
pub const MAGISTRATE: &str = "Magistrate's Robes";
pub const REGENT: &str = "The Ashblood Regent";

/// A weapon finish's colour (multiplies the weapon's own): the vault's main-hand looks.
pub fn look_tint(id: u8) -> Option<[f32; 3]> {
    match id {
        90 => Some([1.35, 0.95, 0.42]),
        91 => Some([0.32, 0.30, 0.31]),
        92 => Some([0.45, 0.55, 1.15]),
        93 => Some([1.3, 0.62, 0.3]),
        _ => None,
    }
}

pub fn look(id: u8) -> Option<&'static LookItem> {
    if id == 0 { None } else { LOOKS.iter().find(|l| l.id == id) }
}

/// The weapon a main-hand look is (the armoury's, held as the fighter's own weapon).
pub fn look_weapon(id: u8) -> Option<Weapon> {
    match id {
        60 => Some(Weapon::Katana),
        61 => Some(Weapon::Greatsword),
        62 => Some(Weapon::Dagger),
        63 => Some(Weapon::Revolver),
        7 => Some(Weapon::Katana),
        90 => Some(Weapon::Katana),
        91 => Some(Weapon::Greatsword),
        92 => Some(Weapon::Dagger),
        93 => Some(Weapon::Revolver),
        94 => Some(Weapon::Greatsword),
        _ => None,
    }
}

/// A set's own colours (coat, trim, accent: crate::gear::DYES).
pub fn set_dyes(set: &str) -> [u8; 3] {
    match set {
        HERALD => [7, 1, 12],
        SMUGGLER => [19, 1, 5],
        PILGRIM => [4, 21, 10],
        WARDEN => [24, 25, 12],
        MAGISTRATE => [24, 25, 5],
        REGENT => [22, 26, 12],
        _ => crate::gear::DEFAULT_DYES,
    }
}

/// The sets with every piece of each (for the wardrobe's "wear the whole set").
pub fn sets() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    for l in LOOKS {
        if !v.contains(&l.set) {
            v.push(l.set);
        }
    }
    v
}

/// What's been earned (kept in the hub save).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Earned {
    pub rooftop: bool,
    pub table_hard: bool,
    pub pit_wins: u32,
    /// The best mastery level of any fighter (from the career, filled in by the menu side).
    pub mastery: u32,
    /// What the Ashen Vault has given.
    pub raid: crate::raid::RaidEarned,
    /// What the bank holds (crate::items): once it's on, the wardrobe goes by what's owned
    /// (a look bit per LOOKS id, a bit per vault loot id), not by what's been earned.
    pub bank_on: bool,
    pub own_looks: u128,
    pub own_loot: u64,
}

impl Earned {
    pub fn has(&self, u: Unlock) -> bool {
        match u {
            Unlock::Always => true,
            Unlock::Mastery(l) => self.mastery >= l,
            Unlock::Rooftop => self.rooftop,
            Unlock::TableHard => self.table_hard,
            Unlock::PitWins(n) => self.pit_wins >= n,
            Unlock::Raid(l) => self.raid.has(l),
        }
    }
    /// A look can be worn: everyone's, or owned.
    pub fn look_owned(&self, l: &LookItem) -> bool {
        if l.unlock == Unlock::Always {
            return true;
        }
        if self.bank_on { l.id < 128 && self.own_looks & (1u128 << l.id) != 0 } else { self.has(l.unlock) }
    }
    /// A vault drop (dye, title, pet) is owned.
    pub fn loot_owned(&self, l: u8) -> bool {
        if self.bank_on { l < 64 && self.own_loot & (1u64 << l) != 0 } else { self.raid.has(l) }
    }
}

/// An outfit: a look in each slot (0 none) and three dyes (crate::gear::DYES) for each; the pet
/// following you (0 none) and the title under your name (a crate::raid::LOOT id, 0 none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outfit {
    pub items: [u8; SLOT_COUNT],
    pub dyes: [[u8; 3]; SLOT_COUNT],
    pub pet: u8,
    pub title: u8,
}

impl Default for Outfit {
    fn default() -> Self {
        Outfit { items: [0; SLOT_COUNT], dyes: [crate::gear::DEFAULT_DYES; SLOT_COUNT], pet: 0, title: 0 }
    }
}

/// The looks and dyes, then the pet and the title.
pub const OUTFIT_BYTES: usize = SLOT_COUNT * 4 + 2;

/// The dyes the vault gives (locked until found).
pub fn dye_locked(d: u8, earned: &Earned) -> bool {
    // (and a season dye not earned yet: crate::season)
    crate::raid::loot_of_dye(d).is_some_and(|l| !earned.loot_owned(l)) || crate::season::dye_locked(d)
}

/// A title's words (a crate::raid::LOOT id).
pub fn title_text(t: u8) -> Option<&'static str> {
    crate::raid::LOOT.get(t as usize).filter(|l| t != 0 && l.kind == crate::raid::LootKind::Title).map(|l| l.name)
}

impl Outfit {
    /// A whole set worn (its pieces in their slots, in the set's own colours; the rest kept).
    pub fn with_set(mut self, set: &str) -> Outfit {
        for l in LOOKS.iter().filter(|l| l.set == set) {
            self.items[l.slot as usize] = l.id;
            self.dyes[l.slot as usize] = set_dyes(set);
        }
        self
    }
    /// Only looks in their own slot and earned (anything else is taken off).
    pub fn checked(mut self, earned: &Earned) -> Outfit {
        for (k, id) in self.items.iter_mut().enumerate() {
            if let Some(l) = look(*id) {
                if l.slot as usize != k || !earned.look_owned(l) {
                    *id = 0;
                }
            } else {
                *id = 0;
            }
        }
        let n = crate::gear::DYES.len() as u8 - 1;
        for (k, zones) in self.dyes.iter_mut().enumerate() {
            for (z, d) in zones.iter_mut().enumerate() {
                *d = (*d).min(n);
                if dye_locked(*d, earned) {
                    *d = crate::gear::DEFAULT_DYES[z];
                }
            }
            let _ = k;
        }
        // (the vault's Cinder Wisp, or a season's pet)
        if self.pet == 1 && !earned.loot_owned(crate::raid::L_PET) || self.pet >= 2 && !crate::season::pet_owned(self.pet) {
            self.pet = 0;
        }
        if title_text(self.title).is_none() || !earned.loot_owned(self.title) {
            self.title = 0;
        }
        self
    }
    /// The model pieces worn (crate::chars::GEAR_PIECES ids) and the slot each is in.
    pub fn pieces(&self) -> Vec<(u8, usize)> {
        self.items.iter().enumerate().filter_map(|(k, &id)| look(id).filter(|l| l.piece != 0).map(|l| (l.piece, k))).collect()
    }
    pub fn held(&self, s: Slot) -> u8 {
        look(self.items[s as usize]).map_or(0, |l| l.held)
    }
    pub fn to_bytes(&self) -> [u8; OUTFIT_BYTES] {
        let mut b = [0u8; OUTFIT_BYTES];
        for k in 0..SLOT_COUNT {
            b[k] = self.items[k];
            b[SLOT_COUNT + k * 3..SLOT_COUNT + k * 3 + 3].copy_from_slice(&self.dyes[k]);
        }
        b[SLOT_COUNT * 4] = self.pet;
        b[SLOT_COUNT * 4 + 1] = self.title;
        b
    }
    pub fn from_bytes(b: &[u8]) -> Option<Outfit> {
        // (outfits saved before pets and titles are two bytes shorter)
        if b.len() < SLOT_COUNT * 4 {
            return None;
        }
        let n = crate::gear::DYES.len() as u8 - 1;
        let mut o = Outfit::default();
        for k in 0..SLOT_COUNT {
            o.items[k] = if look(b[k]).is_some_and(|l| l.slot as usize == k) { b[k] } else { 0 };
            for z in 0..3 {
                o.dyes[k][z] = b[SLOT_COUNT + k * 3 + z].min(n);
            }
        }
        o.pet = b.get(SLOT_COUNT * 4).copied().unwrap_or(0);
        if o.pet >= 2 && crate::season::pet_colour(o.pet).is_none() {
            o.pet = 0;
        }
        o.title = b.get(SLOT_COUNT * 4 + 1).copied().unwrap_or(0).min(crate::raid::LOOT.len() as u8 - 1);
        Some(o)
    }
    pub fn to_text(&self) -> String {
        self.to_bytes().iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")
    }
    pub fn from_text(t: &str) -> Option<Outfit> {
        let b: Vec<u8> = t.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        Outfit::from_bytes(&b)
    }
}

// ==================================================================================== emotes

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emote {
    None = 0,
    Wave = 1,
    Bow = 2,
    Dance = 3,
    Sit = 4,
    Cheer = 5,
    Salute = 6,
    Clap = 7,
    Laugh = 8,
    // season emotes (crate::season: earned on a season's track)
    Kneel = 9,
    Point = 10,
    Shrug = 11,
    MoonBow = 12,
    Toast = 13,
    Brood = 14,
}

impl Emote {
    /// The wheel, clockwise from the top.
    pub const WHEEL: [Emote; 8] = [Emote::Wave, Emote::Bow, Emote::Dance, Emote::Sit, Emote::Cheer, Emote::Salute, Emote::Clap, Emote::Laugh];
    pub fn from_u8(v: u8) -> Emote {
        match v {
            1 => Emote::Wave,
            2 => Emote::Bow,
            3 => Emote::Dance,
            4 => Emote::Sit,
            5 => Emote::Cheer,
            6 => Emote::Salute,
            7 => Emote::Clap,
            8 => Emote::Laugh,
            9 => Emote::Kneel,
            10 => Emote::Point,
            11 => Emote::Shrug,
            12 => Emote::MoonBow,
            13 => Emote::Toast,
            14 => Emote::Brood,
            _ => Emote::None,
        }
    }
    pub fn name(self) -> &'static str {
        ["", "Wave", "Bow", "Dance", "Sit", "Cheer", "Salute", "Clap", "Laugh", "Kneel", "Point", "Shrug", "Moonward Bow", "Raise a Toast", "Brood"][self as usize]
    }
    /// How long it plays (dancing and sitting keep going until you move).
    pub fn length(self) -> Option<f32> {
        match self {
            Emote::Dance | Emote::Sit | Emote::None => None,
            Emote::Wave => Some(2.4),
            Emote::Bow => Some(2.0),
            Emote::Cheer => Some(2.6),
            Emote::Salute => Some(2.2),
            Emote::Clap => Some(2.8),
            Emote::Laugh => Some(2.6),
            Emote::Kneel | Emote::Brood => None,
            Emote::Point => Some(2.0),
            Emote::Shrug => Some(1.8),
            Emote::MoonBow => Some(2.6),
            Emote::Toast => Some(2.4),
        }
    }
    /// The season emotes, in the order the wheel's inner ring shows them.
    pub const SEASON: [Emote; 6] = [Emote::Kneel, Emote::Point, Emote::Shrug, Emote::MoonBow, Emote::Toast, Emote::Brood];
}

/// A fighter's hub state: their look, their emote (and when it began, game time), where they
/// sit (a seat number, see `seat_pose`; SEAT_NONE standing) and which pit they duel in (255 none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FighterHub {
    pub look: Outfit,
    pub emote: u8,
    pub emote_t: f32,
    pub seat: u16,
    pub pit: u8,
    /// One of Varrow's townsfolk (scenery drawn on this computer only: no weapon, no name).
    pub townsfolk: bool,
    /// A raid boss drawn as a fighter (1 the Ashblood Regent: taller, pale, red-eyed; 2 the Ashen
    /// Magistrate).
    pub boss: u8,
}

pub const SEAT_NONE: u16 = u16::MAX;
/// Seats from here up are the tables' chairs: TABLE_SEATS + table * 2 + side.
pub const TABLE_SEATS: u16 = 1000;

impl Default for FighterHub {
    fn default() -> Self {
        FighterHub { look: Outfit::default(), emote: 0, emote_t: 0.0, seat: SEAT_NONE, pit: 255, townsfolk: false, boss: 0 }
    }
}

/// Where a seat is and which way you face sitting on it.
pub fn seat_pose(seat: u16) -> Option<(V3, f32)> {
    if seat >= TABLE_SEATS {
        let k = (seat - TABLE_SEATS) as usize;
        let (x, z, _) = *varrow::TABLES.get(k / 2)?;
        let side = k % 2;
        let dx = if side == 0 { -varrow::CHAIR_OFF } else { varrow::CHAIR_OFF };
        // facing the table
        let yaw = if side == 0 { std::f32::consts::FRAC_PI_2 } else { -std::f32::consts::FRAC_PI_2 };
        return Some((v3(x + dx, varrow::DECK + 0.45, z), yaw));
    }
    varrow::seats().get(seat as usize).copied()
}

/// The seat nearest to `p` within `reach` that nobody else holds.
pub fn free_seat_near(g: &Game, p: V3, reach: f32, me: usize) -> Option<u16> {
    let taken = |s: u16| g.f.iter().enumerate().any(|(k, f)| k != me && f.present && f.hub.seat == s);
    let mut best: Option<(f32, u16)> = None;
    let all = varrow::seats().len() as u16;
    let ids = (0..all).chain((0..varrow::TABLES.len() as u16 * 2).map(|k| TABLE_SEATS + k));
    for s in ids {
        let Some((sp, _)) = seat_pose(s) else { continue };
        let d = (v3(sp.x, p.y, sp.z) - p).len();
        if d < reach && (sp.y - p.y).abs() < 1.6 && !taken(s) && best.map_or(true, |b| d < b.0) {
            best = Some((d, s));
        }
    }
    best.map(|b| b.1)
}

// ==================================================================================== tables

/// Who sits on a side of a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sitter {
    Empty,
    Player(u8),
    Bot(Level),
}

impl Sitter {
    fn to_u8(self) -> u8 {
        match self {
            Sitter::Empty => 0,
            Sitter::Player(s) => 1 + s.min(HUB_PLAYERS as u8 - 1),
            Sitter::Bot(l) => 100 + l as u8,
        }
    }
    fn from_u8(v: u8) -> Sitter {
        match v {
            1..=16 => Sitter::Player(v - 1),
            100..=102 => Sitter::Bot(Level::from_u8(v - 100)),
            _ => Sitter::Empty,
        }
    }
}

pub struct Table {
    pub board: Board,
    pub seats: [Sitter; 2],
    pub outcome: Outcome,
    /// The last move written out, and when the board last changed (game time).
    pub last: String,
    pub moved_t: f32,
    /// Bumped on every change (so an old state never overwrites a newer one).
    pub ver: u32,
    rng: boards::Rng,
    /// A bot's move being worked out on another thread.
    thinking: Option<std::sync::mpsc::Receiver<Option<AnyMove>>>,
}

impl Table {
    fn new(kind: GameKind, seed: u64) -> Table {
        Table { board: Board::new(kind), seats: [Sitter::Empty; 2], outcome: Outcome::Playing, last: String::new(), moved_t: 0.0, ver: 1, rng: boards::Rng(seed), thinking: None }
    }
    pub fn kind(&self) -> GameKind {
        self.board.kind()
    }
    /// Whose move it is: the sitter on that side.
    pub fn to_move(&self) -> Sitter {
        self.seats[self.board.turn() as usize]
    }
    pub fn side_of(&self, slot: usize) -> Option<usize> {
        self.seats.iter().position(|s| *s == Sitter::Player(slot as u8))
    }
    /// A game is on when both sides are taken.
    pub fn playing(&self) -> bool {
        self.seats.iter().all(|s| *s != Sitter::Empty) && self.outcome == Outcome::Playing
    }
}

/// What a player does at a table (sent to the host).
#[derive(Clone, Debug, PartialEq)]
pub enum TableAct {
    /// Sit on a side (0 or 1).
    Sit(u8),
    Stand,
    Move(AnyMove),
    /// Put a bot of this level on the other side.
    Bot(Level),
    /// A new game (once this one is over, or both agree; a player alone may always).
    Reset,
    /// Resign the game in progress.
    Resign,
}

// ===================================================================================== pits

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DuelKind {
    Duel = 0,
    Ledger = 1,
}

impl DuelKind {
    pub fn from_u8(v: u8) -> DuelKind {
        if v == 1 { DuelKind::Ledger } else { DuelKind::Duel }
    }
    pub fn name(self) -> &'static str {
        match self {
            DuelKind::Duel => "1v1 Duel",
            DuelKind::Ledger => "Ledger Duel",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PitPhase {
    /// Being brought in: the countdown.
    Intro = 0,
    Fight = 1,
    /// A round is over: the next one in a moment.
    Break = 2,
    /// The duel is over: back out to the plaza shortly.
    Over = 3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PitDuel {
    pub who: [u8; 2],
    pub kind: DuelKind,
    pub wins: [u8; 2],
    pub first_to: u8,
    pub phase: PitPhase,
    /// Time left in this phase.
    pub t: f32,
}

pub const PIT_INTRO: f32 = 3.5;
pub const PIT_BREAK: f32 = 2.5;
pub const PIT_OVER: f32 = 4.0;

impl PitDuel {
    pub fn side(&self, slot: usize) -> Option<usize> {
        self.who.iter().position(|&w| w as usize == slot)
    }
    pub fn winner(&self) -> Option<usize> {
        (0..2).find(|&k| self.wins[k] >= self.first_to)
    }
}

/// A challenge: from one player to another (or to anyone: `to` 255, posted on the board).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Challenge {
    pub id: u16,
    pub from: u8,
    pub to: u8,
    pub kind: DuelKind,
    /// Seconds it stays up.
    pub t: f32,
}

pub const CHALLENGE_TIME: f32 = 30.0;
pub const OPEN_TIME: f32 = 120.0;

// ============================================================================ the Rooftop Run

/// A moment of a run (for the ghost).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GhostFrame {
    pub t: f32,
    pub pos: V3,
    pub yaw: f32,
    pub ground: bool,
}

pub const GHOST_STEP: f32 = 0.05;

#[derive(Clone, Debug, Default)]
pub struct RunState {
    /// The ring to pass next (0: the start; RUN_RINGS.len(): not running).
    pub next: usize,
    pub t: f32,
    pub running: bool,
    pub frames: Vec<GhostFrame>,
    rec_acc: f32,
    /// The last run's time and whether it was a best (shown a while).
    pub last: Option<(f32, bool)>,
    pub last_t: f32,
}

/// Which ring a point passes through, if any.
pub fn ring_at(p: V3) -> Option<usize> {
    varrow::RUN_RINGS.iter().position(|&(x, y, z)| (p + v3(0.0, 0.9, 0.0) - v3(x, y, z)).len() < varrow::RING_RADIUS)
}

pub fn ghost_to_text(frames: &[GhostFrame]) -> String {
    frames.iter().map(|f| format!("{:.2} {:.2} {:.2} {:.2} {:.2} {}", f.t, f.pos.x, f.pos.y, f.pos.z, f.yaw, f.ground as u8)).collect::<Vec<_>>().join(";")
}

pub fn ghost_from_text(t: &str) -> Vec<GhostFrame> {
    t.split(';')
        .filter_map(|e| {
            let v: Vec<f32> = e.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            (v.len() == 6).then(|| GhostFrame { t: v[0], pos: v3(v[1], v[2], v[3]), yaw: v[4], ground: v[5] > 0.5 })
        })
        .collect()
}

/// Where a ghost is at time `t` (between its two frames).
pub fn ghost_at(frames: &[GhostFrame], t: f32) -> Option<(V3, f32, bool, V3)> {
    let last = frames.last()?;
    if t > last.t {
        return None;
    }
    let k = frames.partition_point(|f| f.t <= t).max(1).min(frames.len() - 1);
    let (a, b) = (frames[k - 1], frames[k]);
    let u = ((t - a.t) / (b.t - a.t).max(1e-4)).clamp(0.0, 1.0);
    let vel = (b.pos - a.pos) * (1.0 / (b.t - a.t).max(1e-4));
    Some((a.pos.lerp(b.pos, u), a.yaw + wrap_angle(b.yaw - a.yaw) * u, a.ground, vel))
}

// ================================================================================ the save

pub const OUTFIT_SAVES: usize = 10;

/// What the hub keeps between sessions (beside the settings).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HubSave {
    pub look: Outfit,
    pub outfits: Vec<(String, Outfit)>,
    pub earned: Earned,
    pub aim_best: [u32; crate::hub_aim::DRILLS],
    /// The Rooftop Run: best time (ms, 0 none) and its ghost.
    pub run_best: u32,
    pub ghost: Vec<GhostFrame>,
}

impl HubSave {
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        s += &format!("look={}\n", self.look.to_text());
        for (n, o) in self.outfits.iter().take(OUTFIT_SAVES) {
            s += &format!("outfit={}|{}\n", n.replace(['|', '\n'], " "), o.to_text());
        }
        s += &format!("earned={},{},{}\n", self.earned.rooftop as u8, self.earned.table_hard as u8, self.earned.pit_wins);
        s += &format!("aim={}\n", self.aim_best.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","));
        s += &format!("run={}\n", self.run_best);
        s += &format!("ghost={}\n", ghost_to_text(&self.ghost));
        s += &format!("raid={}\n", self.earned.raid.to_text());
        s
    }
    pub fn from_text(t: &str) -> HubSave {
        let mut h = HubSave::default();
        for line in t.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k {
                "look" => h.look = Outfit::from_text(v).unwrap_or_default(),
                "outfit" => {
                    if let Some((n, o)) = v.split_once('|') {
                        if let Some(o) = Outfit::from_text(o) {
                            if h.outfits.len() < OUTFIT_SAVES {
                                h.outfits.push((n.chars().take(24).collect(), o));
                            }
                        }
                    }
                }
                "earned" => {
                    let p: Vec<u32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
                    h.earned.rooftop = p.first() == Some(&1);
                    h.earned.table_hard = p.get(1) == Some(&1);
                    h.earned.pit_wins = p.get(2).copied().unwrap_or(0);
                }
                "aim" => {
                    for (k, x) in v.split(',').filter_map(|x| x.parse().ok()).enumerate().take(crate::hub_aim::DRILLS) {
                        h.aim_best[k] = x;
                    }
                }
                "run" => h.run_best = v.parse().unwrap_or(0),
                "ghost" => h.ghost = ghost_from_text(v),
                "raid" => h.earned.raid = crate::raid::RaidEarned::from_text(v),
                _ => {}
            }
        }
        h
    }
}

// ================================================================================== events

/// Hub messages (inside Ev::Hub). From a player to the host, or from the host to everyone.
#[derive(Clone, Debug, PartialEq)]
pub enum HubEv {
    /// (player, relayed) what I'm wearing.
    Look { who: u8, look: Outfit },
    /// (player, relayed) my emote (0 stops it).
    Emote { who: u8, emote: u8 },
    /// (player to host) sit on this seat / stand (SEAT_NONE); (host) who sits where.
    Sit { who: u8, seat: u16 },
    /// (player to host) a challenge to `to` (255: open, on the board).
    Challenge { from: u8, to: u8, kind: u8 },
    /// (player to host) yes or no to a challenge.
    Answer { id: u16, by: u8, accept: bool },
    /// (host) the challenges up now: (id, from, to, kind).
    Challenges { list: Vec<(u16, u8, u8, u8)> },
    /// (host) a pit's duel (None: it's empty).
    Pit { pit: u8, duel: Option<PitDuel> },
    /// (player to host) at a table.
    Table { table: u8, who: u8, act: TableAct },
    /// (host) a table as it stands.
    TableState { table: u8, ver: u32, board: Vec<u8>, seats: [u8; 2], outcome: u8, last: String },
    /// (player to host) a finished aim room or run; (host) the scoreboards.
    Score { who: u8, board: u8, value: u32 },
    Board { board: u8, rows: Vec<(String, u32)> },
    /// (player to host) let's do this drill side by side; (host) go: both lanes, the same seed.
    AimDuo { from: u8, to: u8, drill: u8 },
    AimGo { a: u8, b: u8, drill: u8, seed: u32 },
    /// (host) a line for everyone ("Bram beat Sable 2-1 in the West Court Pit").
    Note { text: String },
    /// The Ashen Vault's messages (crate::raid::RaidMsg).
    Raid(Vec<u8>),
    /// (player to host) an admin command for the host to carry out (crate::commands); (host to
    /// player) its answer.
    Cmd { who: u8, text: String },
    Reply { to: u8, text: String },
    /// (host) the players' Steam IDs as the host sees them (crate::roles: only "reported").
    Ids { list: Vec<(u8, u64)> },
    /// (host) a banner for everyone; the hub's time of day and weather.
    Announce { text: String },
    World { time: u8, weather: u8 },
    /// (host) move to this spot (::tp, ::bring); muted for this long (0: unmuted); removed.
    Move { who: u8, pos: V3 },
    Mute { who: u8, secs: u16 },
    Kicked { who: u8, reason: String },
    /// (player to host) I take no damage now (::god); my session no longer counts for records.
    God { who: u8, on: bool },
    Gated { who: u8 },
    /// Gifts (crate::gifts): an offer (player to host, host to the receiver), the answer, and
    /// the host's word that it's done (or not, and why).
    GiftOffer { gid: u32, from: u8, to: u8, item: [u8; 2], n: u16, create: bool },
    GiftAnswer { gid: u32, by: u8, accept: bool },
    GiftDone { gid: u32, from: u8, to: u8, item: [u8; 2], n: u16, ok: bool, why: String },
    /// (player, relayed) my profile card (crate::season::Card), sent now and then.
    Card { who: u8, card: Vec<u8> },
}

/// Scoreboards: one per aim drill, then the Rooftop Run (lower is better there), then recent duels.
pub const BOARD_RUN: u8 = crate::hub_aim::DRILLS as u8;
/// The Ashen Vault's boards (fastest clears) for parties of 2, 3 and 4: after the run's.
pub const BOARD_RAID: u8 = BOARD_RUN + 1;
pub const BOARD_LAST: u8 = BOARD_RAID + 2;

fn wb_long(b: &mut Vec<u8>, s: &str) {
    let s = s.as_bytes();
    let n = s.len().min(200);
    b.push(n as u8);
    b.extend_from_slice(&s[..n]);
}

fn wb_s(b: &mut Vec<u8>, s: &str) {
    let s = s.as_bytes();
    let n = s.len().min(40);
    b.push(n as u8);
    b.extend_from_slice(&s[..n]);
}

struct Rd<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Rd<'a> {
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
    fn f(&mut self) -> Option<f32> {
        let v = f32::from_bits(self.u32()?);
        Some(if v.is_finite() { v } else { 0.0 })
    }
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.i..self.i + n)?;
        self.i += n;
        Some(s)
    }
    fn s(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        Some(String::from_utf8_lossy(self.bytes(n)?).chars().take(40).collect())
    }
    fn long(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        Some(String::from_utf8_lossy(self.bytes(n)?).chars().take(200).collect())
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.bytes(8)?.try_into().ok()?))
    }
}

fn wb_pit(b: &mut Vec<u8>, d: &PitDuel) {
    b.extend_from_slice(&d.who);
    b.push(d.kind as u8);
    b.extend_from_slice(&d.wins);
    b.push(d.first_to);
    b.push(d.phase as u8);
    b.extend_from_slice(&d.t.to_bits().to_le_bytes());
}

fn rd_pit(r: &mut Rd) -> Option<PitDuel> {
    let who = [r.u8()?.min(HUB_PLAYERS as u8 - 1), r.u8()?.min(HUB_PLAYERS as u8 - 1)];
    let kind = DuelKind::from_u8(r.u8()?);
    let wins = [r.u8()?.min(9), r.u8()?.min(9)];
    let first_to = r.u8()?.clamp(1, 9);
    let phase = match r.u8()? {
        0 => PitPhase::Intro,
        1 => PitPhase::Fight,
        2 => PitPhase::Break,
        _ => PitPhase::Over,
    };
    let t = r.f()?.clamp(0.0, 30.0);
    Some(PitDuel { who, kind, wins, first_to, phase, t })
}

impl HubEv {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(48);
        match self {
            HubEv::Look { who, look } => {
                b.push(1);
                b.push(*who);
                b.extend_from_slice(&look.to_bytes());
            }
            HubEv::Emote { who, emote } => b.extend_from_slice(&[2, *who, *emote]),
            HubEv::Sit { who, seat } => {
                b.extend_from_slice(&[3, *who]);
                b.extend_from_slice(&seat.to_le_bytes());
            }
            HubEv::Challenge { from, to, kind } => b.extend_from_slice(&[4, *from, *to, *kind]),
            HubEv::Answer { id, by, accept } => {
                b.push(5);
                b.extend_from_slice(&id.to_le_bytes());
                b.extend_from_slice(&[*by, *accept as u8]);
            }
            HubEv::Challenges { list } => {
                b.push(6);
                b.push(list.len().min(16) as u8);
                for &(id, f, t, k) in list.iter().take(16) {
                    b.extend_from_slice(&id.to_le_bytes());
                    b.extend_from_slice(&[f, t, k]);
                }
            }
            HubEv::Pit { pit, duel } => {
                b.extend_from_slice(&[7, *pit, duel.is_some() as u8]);
                if let Some(d) = duel {
                    wb_pit(&mut b, d);
                }
            }
            HubEv::Table { table, who, act } => {
                b.extend_from_slice(&[8, *table, *who]);
                match act {
                    TableAct::Sit(s) => b.extend_from_slice(&[0, *s]),
                    TableAct::Stand => b.push(1),
                    TableAct::Move(m) => {
                        b.push(2);
                        b.extend(m.to_bytes());
                    }
                    TableAct::Bot(l) => b.extend_from_slice(&[3, *l as u8]),
                    TableAct::Reset => b.push(4),
                    TableAct::Resign => b.push(5),
                }
            }
            HubEv::TableState { table, ver, board, seats, outcome, last } => {
                b.extend_from_slice(&[9, *table]);
                b.extend_from_slice(&ver.to_le_bytes());
                b.push(board.len().min(255) as u8);
                b.extend(board.iter().take(255));
                b.extend_from_slice(seats);
                b.push(*outcome);
                wb_s(&mut b, last);
            }
            HubEv::Score { who, board, value } => {
                b.extend_from_slice(&[10, *who, *board]);
                b.extend_from_slice(&value.to_le_bytes());
            }
            HubEv::Board { board, rows } => {
                b.extend_from_slice(&[11, *board, rows.len().min(8) as u8]);
                for (n, v) in rows.iter().take(8) {
                    wb_s(&mut b, n);
                    b.extend_from_slice(&v.to_le_bytes());
                }
            }
            HubEv::AimDuo { from, to, drill } => b.extend_from_slice(&[12, *from, *to, *drill]),
            HubEv::AimGo { a, b: bb, drill, seed } => {
                b.extend_from_slice(&[13, *a, *bb, *drill]);
                b.extend_from_slice(&seed.to_le_bytes());
            }
            HubEv::Note { text } => {
                b.push(14);
                wb_s(&mut b, text);
            }
            HubEv::Raid(bytes) => {
                b.push(15);
                b.extend_from_slice(&(bytes.len().min(4000) as u16).to_le_bytes());
                b.extend(bytes.iter().take(4000));
            }
            HubEv::Cmd { who, text } => {
                b.extend_from_slice(&[16, *who]);
                wb_long(&mut b, text);
            }
            HubEv::Reply { to, text } => {
                b.extend_from_slice(&[17, *to]);
                wb_long(&mut b, text);
            }
            HubEv::Ids { list } => {
                b.extend_from_slice(&[18, list.len().min(HUB_PLAYERS) as u8]);
                for (s, id) in list.iter().take(HUB_PLAYERS) {
                    b.push(*s);
                    b.extend_from_slice(&id.to_le_bytes());
                }
            }
            HubEv::Announce { text } => {
                b.push(19);
                wb_long(&mut b, text);
            }
            HubEv::World { time, weather } => b.extend_from_slice(&[20, *time, *weather]),
            HubEv::Move { who, pos } => {
                b.extend_from_slice(&[21, *who]);
                for v in [pos.x, pos.y, pos.z] {
                    b.extend_from_slice(&v.to_bits().to_le_bytes());
                }
            }
            HubEv::Mute { who, secs } => {
                b.extend_from_slice(&[22, *who]);
                b.extend_from_slice(&secs.to_le_bytes());
            }
            HubEv::Kicked { who, reason } => {
                b.extend_from_slice(&[23, *who]);
                wb_long(&mut b, reason);
            }
            HubEv::God { who, on } => b.extend_from_slice(&[24, *who, *on as u8]),
            HubEv::Gated { who } => b.extend_from_slice(&[25, *who]),
            HubEv::GiftOffer { gid, from, to, item, n, create } => {
                b.push(26);
                b.extend_from_slice(&gid.to_le_bytes());
                b.extend_from_slice(&[*from, *to, item[0], item[1]]);
                b.extend_from_slice(&n.to_le_bytes());
                b.push(*create as u8);
            }
            HubEv::GiftAnswer { gid, by, accept } => {
                b.push(27);
                b.extend_from_slice(&gid.to_le_bytes());
                b.extend_from_slice(&[*by, *accept as u8]);
            }
            HubEv::GiftDone { gid, from, to, item, n, ok, why } => {
                b.push(28);
                b.extend_from_slice(&gid.to_le_bytes());
                b.extend_from_slice(&[*from, *to, item[0], item[1]]);
                b.extend_from_slice(&n.to_le_bytes());
                b.push(*ok as u8);
                wb_long(&mut b, why);
            }
            HubEv::Card { who, card } => {
                b.extend_from_slice(&[29, *who, card.len().min(160) as u8]);
                b.extend(card.iter().take(160));
            }
        }
        b
    }

    pub fn from_bytes(b: &[u8]) -> Option<HubEv> {
        let mut r = Rd { b, i: 0 };
        let who = |v: u8| v.min(HUB_PLAYERS as u8 - 1);
        Some(match r.u8()? {
            1 => {
                let w = who(r.u8()?);
                HubEv::Look { who: w, look: Outfit::from_bytes(r.bytes(OUTFIT_BYTES)?)? }
            }
            2 => HubEv::Emote { who: who(r.u8()?), emote: r.u8()?.min(14) },
            3 => HubEv::Sit { who: who(r.u8()?), seat: r.u16()? },
            4 => HubEv::Challenge { from: who(r.u8()?), to: r.u8()?, kind: r.u8()?.min(1) },
            5 => HubEv::Answer { id: r.u16()?, by: who(r.u8()?), accept: r.u8()? == 1 },
            6 => {
                let n = r.u8()?.min(16);
                let mut list = Vec::new();
                for _ in 0..n {
                    list.push((r.u16()?, who(r.u8()?), r.u8()?, r.u8()?.min(1)));
                }
                HubEv::Challenges { list }
            }
            7 => {
                let pit = r.u8()?.min(1);
                let duel = if r.u8()? == 1 { Some(rd_pit(&mut r)?) } else { None };
                HubEv::Pit { pit, duel }
            }
            8 => {
                let table = r.u8()?.min(varrow::TABLES.len() as u8 - 1);
                let w = who(r.u8()?);
                let act = match r.u8()? {
                    0 => TableAct::Sit(r.u8()?.min(1)),
                    1 => TableAct::Stand,
                    2 => TableAct::Move(AnyMove::from_bytes(&b[r.i..])?.0),
                    3 => TableAct::Bot(Level::from_u8(r.u8()?)),
                    4 => TableAct::Reset,
                    _ => TableAct::Resign,
                };
                HubEv::Table { table, who: w, act }
            }
            9 => {
                let table = r.u8()?.min(varrow::TABLES.len() as u8 - 1);
                let ver = r.u32()?;
                let n = r.u8()? as usize;
                let board = r.bytes(n)?.to_vec();
                let seats = [r.u8()?, r.u8()?];
                let outcome = r.u8()?;
                let last = r.s()?;
                HubEv::TableState { table, ver, board, seats, outcome, last }
            }
            10 => HubEv::Score { who: who(r.u8()?), board: r.u8()?.min(BOARD_LAST), value: r.u32()? },
            11 => {
                let board = r.u8()?.min(BOARD_LAST);
                let n = r.u8()?.min(8);
                let mut rows = Vec::new();
                for _ in 0..n {
                    rows.push((r.s()?, r.u32()?));
                }
                HubEv::Board { board, rows }
            }
            12 => HubEv::AimDuo { from: who(r.u8()?), to: who(r.u8()?), drill: r.u8()?.min(crate::hub_aim::DRILLS as u8 - 1) },
            13 => HubEv::AimGo { a: who(r.u8()?), b: who(r.u8()?), drill: r.u8()?.min(crate::hub_aim::DRILLS as u8 - 1), seed: r.u32()? },
            14 => HubEv::Note { text: r.s()? },
            15 => {
                let n = r.u16()? as usize;
                HubEv::Raid(r.bytes(n)?.to_vec())
            }
            16 => HubEv::Cmd { who: who(r.u8()?), text: r.long()? },
            17 => HubEv::Reply { to: who(r.u8()?), text: r.long()? },
            18 => {
                let n = r.u8()?.min(HUB_PLAYERS as u8);
                let mut list = Vec::new();
                for _ in 0..n {
                    list.push((who(r.u8()?), r.u64()?));
                }
                HubEv::Ids { list }
            }
            19 => HubEv::Announce { text: r.long()? },
            20 => HubEv::World { time: r.u8()?.min(2), weather: r.u8()?.min(2) },
            21 => HubEv::Move { who: who(r.u8()?), pos: v3(r.f()?, r.f()?, r.f()?) },
            22 => HubEv::Mute { who: who(r.u8()?), secs: r.u16()? },
            23 => HubEv::Kicked { who: who(r.u8()?), reason: r.long()? },
            24 => HubEv::God { who: who(r.u8()?), on: r.u8()? == 1 },
            25 => HubEv::Gated { who: who(r.u8()?) },
            26 => HubEv::GiftOffer { gid: r.u32()?, from: who(r.u8()?), to: who(r.u8()?), item: [r.u8()?, r.u8()?], n: r.u16()?, create: r.u8()? == 1 },
            27 => HubEv::GiftAnswer { gid: r.u32()?, by: who(r.u8()?), accept: r.u8()? == 1 },
            28 => HubEv::GiftDone { gid: r.u32()?, from: who(r.u8()?), to: who(r.u8()?), item: [r.u8()?, r.u8()?], n: r.u16()?, ok: r.u8()? == 1, why: r.long()? },
            29 => {
                let w = who(r.u8()?);
                let n = r.u8()? as usize;
                HubEv::Card { who: w, card: r.bytes(n)?.to_vec() }
            }
            _ => return None,
        })
    }

    /// Whether the host passes a player's message straight on to everyone.
    pub fn relayed(&self) -> bool {
        matches!(self, HubEv::Look { .. } | HubEv::Emote { .. } | HubEv::Card { .. })
    }
}

// ===================================================================================== the hub

/// A toast on screen (text, time left).
pub type Toast = (String, f32);

pub struct Hub {
    pub tables: Vec<Table>,
    pub pits: [Option<PitDuel>; 2],
    pub challenges: Vec<Challenge>,
    next_id: u16,
    /// Scoreboards (per aim drill, then the run): name and score (the run: milliseconds).
    pub boards: Vec<Vec<(String, u32)>>,
    /// Recent duels' results, newest first.
    pub results: Vec<String>,
    /// (this computer) the aim room being played, the run, the best run's ghost.
    pub aim: Option<crate::hub_aim::AimRun>,
    pub run: RunState,
    pub ghost: Vec<GhostFrame>,
    pub run_best: u32,
    /// Everyone's profile cards as they last sent them (crate::season::Card bytes), by slot.
    pub cards: std::collections::HashMap<usize, Vec<u8>>,
    /// (this computer) a challenge waiting for our answer, and an aim-room invitation.
    pub asked: Option<Challenge>,
    pub duo_ask: Option<(u8, u8)>,
    pub toasts: Vec<Toast>,
    /// (this computer) the aim best scores (saved), so a new best is noticed.
    pub aim_best: [u32; crate::hub_aim::DRILLS],
    /// (this computer) what was earned this session, for the save.
    pub earned: Earned,
    /// (host) the bots' plans.
    pub regulars: Vec<crate::hub_bots::Regular>,
    /// (host) when the shared state goes out again; (everyone) when our look does.
    sync_t: f32,
    look_t: f32,
    /// A transition on screen: entering or leaving a pit (seconds left, text).
    pub fade: Option<(f32, String)>,
    /// (this computer) the last aim room's result, shown a while.
    pub aim_result: Option<(String, f32)>,
    /// (this computer) what this player owns (crate::items), and the gifts in flight.
    pub bank: crate::items::LocalBank,
    pub gifts: crate::gifts::Gifts,
}

impl Hub {
    pub fn new() -> Hub {
        let tables = varrow::TABLES.iter().enumerate().map(|(k, t)| Table::new(t.2, 77 + k as u64 * 1013)).collect();
        Hub {
            tables,
            pits: [None, None],
            challenges: Vec::new(),
            next_id: 1,
            boards: vec![Vec::new(); BOARD_LAST as usize + 1],
            results: Vec::new(),
            aim: None,
            run: RunState { next: 0, ..Default::default() },
            ghost: Vec::new(),
            run_best: 0,
            cards: std::collections::HashMap::new(),
            asked: None,
            duo_ask: None,
            toasts: Vec::new(),
            aim_best: [0; crate::hub_aim::DRILLS],
            earned: Earned::default(),
            regulars: Vec::new(),
            sync_t: 0.0,
            look_t: 0.0,
            fade: None,
            aim_result: None,
            bank: crate::items::LocalBank::default(),
            gifts: crate::gifts::Gifts::default(),
        }
    }

    /// Put anything newly earned in the bank, and let the wardrobe see what's owned.
    pub fn refresh_bank(&mut self) {
        let e = self.earned;
        self.bank.sync(&e);
        self.bank.apply_to(&mut self.earned);
    }

    pub fn toast(&mut self, s: impl Into<String>) {
        self.toasts.push((s.into(), 4.0));
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    /// The pit a fighter duels in.
    pub fn pit_of(&self, slot: usize) -> Option<usize> {
        (0..2).find(|&p| self.pits[p].is_some_and(|d| d.side(slot).is_some()))
    }

    /// Put a score on a board (the run: lower is better), best per name, top 8 kept.
    pub fn post(&mut self, board: u8, name: &str, value: u32) -> bool {
        let Some(rows) = self.boards.get_mut(board as usize) else { return false };
        // (the run and the vault: lower is better)
        let low = board >= BOARD_RUN;
        let better = |a: u32, b: u32| if low { a < b } else { a > b };
        if let Some(r) = rows.iter_mut().find(|r| r.0 == name) {
            if !better(value, r.1) {
                return false;
            }
            r.1 = value;
        } else {
            rows.push((name.to_string(), value));
        }
        rows.sort_by(|a, b| if low { a.1.cmp(&b.1) } else { b.1.cmp(&a.1) });
        rows.truncate(8);
        true
    }
}

impl Default for Hub {
    fn default() -> Self {
        Hub::new()
    }
}

/// The names the hub's regulars go by.
pub const REGULARS: [&str; 15] = ["Wren", "Odo", "Tamsin", "Corlis", "Mabry", "Hesk", "Juno", "Pell", "Isolde", "Garrow", "Fennick", "Sorrel", "Ambrose", "Kesh", "Lark"];

// ============================================================================ the game side

impl Game {
    /// Varrow Centre on this computer (or as its host): you and `bots` regulars.
    pub fn hub(name: &str, bots: usize, kind: Kind) -> Game {
        let mut g = Game::new(kind, 0, Mode::Hub, 0);
        g.set_map(crate::world::MapId::Varrow);
        g.mt.phase = Phase::Fight;
        g.mt.timer = 0.0;
        g.hub = Some(Box::new(Hub::new()));
        g.add_player(0, name, true, None, 0);
        for b in 0..bots.min(HUB_PLAYERS - 1) {
            g.add_regular(b + 1);
        }
        g.banner = Some(("VARROW CENTRE".into(), 2.5));
        g
    }

    /// A hub regular joins in slot `s`: a bot with a look of their own.
    pub fn add_regular(&mut self, s: usize) {
        if s >= self.f.len() {
            return;
        }
        self.add_player(s, REGULARS[(s + 14) % REGULARS.len()], true, Some(0.6), 0);
        self.f[s].ready = true;
        self.f[s].loadout.body = (s % 4) as u8;
        // a look: a set (or parts of two)
        let sets = [RIG, HERALD, SMUGGLER, PILGRIM];
        let mut o = Outfit::default().with_set(sets[s % 4]);
        if s % 3 == 0 {
            let other = Outfit::default().with_set(sets[(s + 1) % 4]);
            o.items[Slot::Head as usize] = other.items[Slot::Head as usize];
            o.items[Slot::Cape as usize] = other.items[Slot::Cape as usize];
        }
        if s % 2 == 1 {
            o.items[Slot::Cape as usize] = 0;
        }
        for k in 0..SLOT_COUNT {
            o.dyes[k] = [(s as u8 * 3) % 12, (s as u8 * 5 + 1) % 12, (s as u8 * 7 + 2) % 12];
        }
        o.items[Slot::Main as usize] = [60, 61, 62, 7][s % 4];
        self.f[s].hub.look = o;
        self.apply_look_weapon(s);
        if let Some(h) = self.hub.as_mut() {
            while h.regulars.len() <= s {
                h.regulars.push(crate::hub_bots::Regular::default());
            }
            h.regulars[s] = crate::hub_bots::Regular::new(s as u64);
        }
    }

    pub fn in_hub(&self) -> bool {
        self.mt.mode == Mode::Hub && self.hub.is_some()
    }

    /// In the hub, two fighters are enemies only while they duel each other in a pit.
    pub fn hub_enemies(&self, a: usize, b: usize) -> bool {
        let Some(h) = self.hub.as_ref() else { return false };
        a != b && h.pits.iter().flatten().any(|d| d.phase == PitPhase::Fight && d.side(a).is_some() && d.side(b).is_some())
    }

    /// Whether Ledger gear rules apply to fighter `i` (a Ledger match, or a Ledger duel in a pit).
    pub fn ledger_for(&self, i: usize) -> bool {
        if self.ledger_on() {
            return true;
        }
        self.hub.as_ref().is_some_and(|h| h.pits.iter().flatten().any(|d| d.kind == DuelKind::Ledger && d.side(i).is_some()))
    }

    /// A pit duellist waiting for the bell can't move.
    pub fn hub_frozen(&self, i: usize) -> bool {
        self.hub.as_ref().is_some_and(|h| h.pits.iter().flatten().any(|d| d.side(i).is_some() && matches!(d.phase, PitPhase::Intro | PitPhase::Break)))
    }

    /// Where someone (re)appears in the hub: their pit's corner when duelling, else the plaza.
    pub fn hub_spawn(&mut self, slot: usize) -> (V3, f32) {
        if let Some(h) = self.hub.as_ref() {
            for (p, d) in h.pits.iter().enumerate() {
                if let Some(side) = d.and_then(|d| d.side(slot)) {
                    let (pos, yaw) = varrow::PITS[p].spawns[side];
                    return (pos, yaw);
                }
            }
        }
        let k = (slot + self.rng.next_u32() as usize) % varrow::ARRIVALS.len();
        let (x, z, yaw) = varrow::ARRIVALS[k];
        (v3(x + self.rng.range(-0.8, 0.8), 0.0, z + self.rng.range(-0.8, 0.8)), yaw)
    }

    /// Queue a hub message (the net side sends it; on this computer alone it's simply handled).
    pub fn hub_send(&mut self, e: HubEv) {
        match self.kind {
            Kind::Client | Kind::Host => self.out_events.push(Ev::Hub(e.clone())),
            _ => {}
        }
        // the host (or this computer alone) acts on its own messages straight away
        if self.is_authority() {
            let me = self.local;
            self.hub_remote(me, e);
        }
    }

    /// The host tells everyone (and acts on it itself).
    pub(crate) fn hub_broadcast(&mut self, e: HubEv) {
        if self.kind == Kind::Host {
            self.out_events.push(Ev::Hub(e.clone()));
        }
        self.hub_apply(e);
    }

    // ---- what the local player does

    /// Start (or stop, with Emote::None) an emote.
    pub fn hub_emote(&mut self, e: Emote) {
        let me = self.local;
        if e == Emote::Sit && self.f[me].hub.seat == SEAT_NONE {
            // sit where you stand (on the ground)
        }
        self.f[me].hub.emote = e as u8;
        self.f[me].hub.emote_t = self.time;
        self.hub_send(HubEv::Emote { who: me as u8, emote: e as u8 });
    }

    /// Wear an outfit (and tell everyone).
    pub fn hub_wear(&mut self, o: Outfit) {
        let me = self.local;
        self.f[me].hub.look = o;
        self.apply_look_weapon(me);
        self.hub_send(HubEv::Look { who: me as u8, look: o });
    }

    /// The main-hand look decides the weapon held in the hub (a blade, carried, when there's
    /// none: nobody walks the square with a gun up).
    pub fn apply_look_weapon(&mut self, i: usize) {
        if !self.in_hub() || self.hub.as_ref().is_some_and(|h| h.pit_of(i).is_some()) || self.in_raid(i) {
            return;
        }
        let o = self.f[i].hub.look;
        if let Some(w) = look_weapon(o.items[Slot::Main as usize]).or(Some(Weapon::Katana)) {
            let f = &mut self.f[i];
            f.loadout.melee = w;
            if w.is_melee() {
                f.weapon = w;
                f.cur = 0;
            } else {
                f.loadout.gun1 = w;
                f.weapon = w;
                f.cur = 1;
            }
        }
    }

    /// Sit on the nearest free seat (or stand up if sitting).
    pub fn hub_sit_toggle(&mut self) -> bool {
        let me = self.local;
        if self.f[me].hub.seat != SEAT_NONE {
            self.hub_send(HubEv::Sit { who: me as u8, seat: SEAT_NONE });
            return true;
        }
        let p = self.f[me].pos;
        match free_seat_near(self, p, 1.6, me) {
            Some(s) => {
                self.hub_send(HubEv::Sit { who: me as u8, seat: s });
                true
            }
            None => false,
        }
    }

    /// Challenge someone (255: post it on the board for anyone).
    pub fn hub_challenge(&mut self, to: u8, kind: DuelKind) {
        let me = self.local as u8;
        self.hub_send(HubEv::Challenge { from: me, to, kind: kind as u8 });
    }

    pub fn hub_answer(&mut self, id: u16, accept: bool) {
        let me = self.local as u8;
        if let Some(h) = self.hub.as_mut() {
            h.asked = None;
        }
        self.hub_send(HubEv::Answer { id, by: me, accept });
    }

    pub fn hub_table(&mut self, table: usize, act: TableAct) {
        let me = self.local as u8;
        self.hub_send(HubEv::Table { table: table as u8, who: me, act });
    }

    /// The challenge from or to someone near you that you could answer.
    pub fn challenge_for(&self, slot: usize) -> Option<Challenge> {
        self.hub.as_ref()?.challenges.iter().copied().find(|c| c.to as usize == slot)
    }

    // ---- the host's side

    /// A hub message from `from` (or our own, on the host).
    pub fn hub_remote(&mut self, from: usize, e: HubEv) {
        if !self.in_hub() {
            return;
        }
        let auth = self.is_authority();
        if let HubEv::Raid(b) = &e {
            crate::raid::on_msg(self, from, b);
            return;
        }
        if matches!(e, HubEv::Cmd { .. } | HubEv::Reply { .. } | HubEv::Ids { .. } | HubEv::Announce { .. } | HubEv::World { .. } | HubEv::Move { .. } | HubEv::Mute { .. } | HubEv::Kicked { .. } | HubEv::God { .. } | HubEv::Gated { .. }) {
            crate::commands::on_hub(self, from, e);
            return;
        }
        if matches!(e, HubEv::GiftOffer { .. } | HubEv::GiftAnswer { .. } | HubEv::GiftDone { .. }) {
            crate::gifts::on_hub(self, from, e);
            return;
        }
        match e {
            HubEv::Look { who, look } => {
                // (a player only speaks for themselves)
                let w = if auth { from } else { who as usize };
                if w < self.f.len() {
                    self.f[w].hub.look = look;
                    self.apply_look_weapon(w);
                }
            }
            HubEv::Card { who, card } => {
                // (a player only speaks for themselves)
                let w = if auth { from } else { who as usize };
                if let Some(h) = self.hub.as_mut() {
                    h.cards.insert(w, card);
                }
            }
            HubEv::Emote { who, emote } => {
                let w = if auth { from } else { who as usize };
                if w < self.f.len() {
                    self.f[w].hub.emote = emote;
                    self.f[w].hub.emote_t = self.time;
                }
            }
            HubEv::Sit { seat, .. } if auth => self.host_sit(from, seat),
            HubEv::Challenge { to, kind, .. } if auth => self.host_challenge(from, to, DuelKind::from_u8(kind)),
            HubEv::Answer { id, accept, .. } if auth => self.host_answer(from, id, accept),
            HubEv::Table { table, act, .. } if auth => self.host_table(from, table as usize, act),
            // (a session with changed stats or unlocks counts for no board: crate::roles)
            HubEv::Score { .. } if auth && !self.counts_for_records(from) => {}
            HubEv::Score { board, value, .. } if auth => {
                let name = self.f.get(from).map(|f| f.name.clone()).unwrap_or_default();
                if self.hub.as_mut().is_some_and(|h| h.post(board, &name, value)) {
                    self.host_send_board(board);
                    if board == BOARD_RUN {
                        let t = value as f32 / 1000.0;
                        self.hub_broadcast(HubEv::Note { text: format!("{name} ran the Rooftop Run in {t:.2} s") });
                    }
                }
            }
            HubEv::AimDuo { to, drill, .. } if auth => {
                let to = to as usize;
                if to < self.f.len() && self.f[to].present {
                    if self.f[to].is_bot {
                        // the regulars don't shoot targets: they cheer from the side instead
                        if let Some(h) = self.hub.as_mut() {
                            h.toast(format!("{} would rather watch", self.f[to].name));
                        }
                    } else {
                        let ask = HubEv::AimDuo { from: from as u8, to: to as u8, drill };
                        if to == self.local {
                            self.hub_apply(ask);
                        } else if self.kind == Kind::Host {
                            self.out_events.push(Ev::Hub(ask));
                        }
                    }
                }
            }
            other => self.hub_apply(other),
        }
    }

    /// What the host says (or what this computer decided itself).
    fn hub_apply(&mut self, e: HubEv) {
        let mut table_won = None;
        self.hub_apply_inner(e, &mut table_won);
        if let Some((kind, hard)) = table_won {
            self.career.push(crate::game::CareerEv::TableWin { kind, hard });
        }
    }

    fn hub_apply_inner(&mut self, e: HubEv, table_won: &mut Option<(u8, bool)>) {
        // (the admin events the host broadcasts act here too: crate::commands)
        if matches!(e, HubEv::Announce { .. } | HubEv::World { .. } | HubEv::Move { .. } | HubEv::Mute { .. } | HubEv::Reply { .. }) {
            let me = self.local;
            crate::commands::on_hub(self, me, e);
            return;
        }
        let me = self.local;
        let now = self.time;
        let Some(h) = self.hub.as_mut() else { return };
        match e {
            HubEv::Sit { who, seat } => {
                if let Some(f) = self.f.get_mut(who as usize) {
                    f.hub.seat = seat;
                    if seat == SEAT_NONE {
                        if f.hub.emote == Emote::Sit as u8 {
                            f.hub.emote = 0;
                        }
                    } else if let Some((p, yaw)) = seat_pose(seat) {
                        f.hub.emote = Emote::Sit as u8;
                        f.hub.emote_t = now;
                        if f.owned {
                            // stand in front of it, facing as it does (the picture sits you down)
                            let fwd = v3(yaw.sin(), 0.0, yaw.cos());
                            let at = v3(p.x, (p.y - 0.45).max(0.0), p.z) + fwd * 0.1;
                            f.pos = at;
                            f.prev_pos = at;
                            f.vel = V3::ZERO;
                            f.yaw = yaw;
                        }
                    }
                }
            }
            HubEv::Challenges { list } => {
                h.challenges = list.iter().map(|&(id, from, to, kind)| Challenge { id, from, to, kind: DuelKind::from_u8(kind), t: CHALLENGE_TIME }).collect();
                if h.asked.is_none() {
                    if let Some(c) = h.challenges.iter().find(|c| c.to as usize == me) {
                        h.asked = Some(*c);
                    }
                }
                if let Some(a) = h.asked {
                    if !h.challenges.iter().any(|c| c.id == a.id) {
                        h.asked = None;
                    }
                }
            }
            HubEv::Pit { pit, duel } => {
                let p = (pit as usize).min(1);
                let was = h.pits[p];
                h.pits[p] = duel;
                let in_now = duel.is_some_and(|d| d.side(me).is_some());
                let in_before = was.is_some_and(|d| d.side(me).is_some());
                if in_now && !in_before {
                    h.fade = Some((1.2, format!("Entering the {}", if p == 0 { "West Court Pit" } else { "East Court Pit" })));
                }
                let won = in_before && !in_now && was.and_then(|d| d.winner()).is_some_and(|w| was.is_some_and(|d| d.who[w] as usize == me));
                if in_before && !in_now {
                    h.fade = Some((1.0, "Back to Varrow Centre".into()));
                    if won {
                        h.earned.pit_wins += 1;
                    }
                }
                self.pit_moves(p, was, duel);
                if won {
                    self.career.push(crate::game::CareerEv::PitWin);
                }
            }
            HubEv::TableState { table, ver, board, seats, outcome, last } => {
                if let Some(t) = h.tables.get_mut(table as usize) {
                    if ver >= t.ver || ver < 3 {
                        if let Some(b) = Board::from_bytes(&board) {
                            t.board = b;
                        }
                        t.seats = [Sitter::from_u8(seats[0]), Sitter::from_u8(seats[1])];
                        let was = t.outcome;
                        t.outcome = Outcome::from_u8(outcome);
                        // (a guest's own win at a table: the host's side counts its own)
                        if self.kind == Kind::Client && was != t.outcome {
                            if let Outcome::Win(s) = t.outcome {
                                if t.seats[s as usize] == Sitter::Player(me as u8) {
                                    *table_won = Some((t.board.kind() as u8, t.seats[1 - s as usize] == Sitter::Bot(Level::Hard)));
                                }
                            }
                        }
                        if t.last != last {
                            t.moved_t = now;
                        }
                        t.last = last;
                        t.ver = ver;
                    }
                }
            }
            HubEv::Board { board, rows } => {
                if let Some(b) = h.boards.get_mut(board as usize) {
                    *b = rows;
                }
            }
            HubEv::Raid(_) => {}
            HubEv::Note { text } => {
                h.results.insert(0, text.clone());
                h.results.truncate(8);
                h.toast(text);
            }
            HubEv::AimDuo { from, to, drill } => {
                if to as usize == me {
                    h.duo_ask = Some((from, drill));
                }
            }
            HubEv::AimGo { a, b, drill, seed } => {
                let (mine, other) = if a as usize == me { (0usize, b) } else if b as usize == me { (1, a) } else { return };
                h.aim = Some(crate::hub_aim::AimRun::new(crate::hub_aim::Drill::from_u8(drill), mine, seed, Some(other)));
                h.duo_ask = None;
            }
            HubEv::Look { who, look } => {
                if let Some(f) = self.f.get_mut(who as usize) {
                    f.hub.look = look;
                }
            }
            HubEv::Emote { who, emote } => {
                if let Some(f) = self.f.get_mut(who as usize) {
                    f.hub.emote = emote;
                    f.hub.emote_t = now;
                }
            }
            HubEv::Score { who, board, value } => {
                let name = self.f.get(who as usize).map(|f| f.name.clone()).unwrap_or_default();
                h.post(board, &name, value);
            }
            _ => {}
        }
    }

    /// (every computer) A pit changed: our own fighters (you, and the host's regulars) go where
    /// it puts them: into their corner when brought in or when a round begins (healed), back
    /// out by the Challenge Boards when it's over.
    fn pit_moves(&mut self, p: usize, was: Option<PitDuel>, now: Option<PitDuel>) {
        for k in 0..self.f.len() {
            let inside = now.is_some_and(|d| d.side(k).is_some());
            let before = was.is_some_and(|d| d.side(k).is_some());
            if inside {
                self.f[k].hub.pit = p as u8;
            } else if self.f[k].hub.pit == p as u8 {
                self.f[k].hub.pit = 255;
            }
            if !self.f[k].owned || !self.f[k].present {
                continue;
            }
            if let Some(d) = now.filter(|_| inside) {
                let side = d.side(k).unwrap_or(0);
                let new_round = d.phase == PitPhase::Fight && was.is_some_and(|w| w.phase != PitPhase::Fight);
                if !before || new_round {
                    let (pos, yaw) = varrow::PITS[p].spawns[side];
                    let f = &mut self.f[k];
                    f.hub.seat = SEAT_NONE;
                    f.hub.emote = 0;
                    f.respawn_at(pos, yaw);
                    f.spawn_prot = 0.0;
                    f.hp = MAX_HP;
                    if !before {
                        f.kit = None;
                        if d.kind == DuelKind::Duel {
                            f.equip();
                        }
                    }
                }
                if matches!(d.phase, PitPhase::Break | PitPhase::Over) && !self.f[k].alive {
                    self.f[k].respawn_t = 1e9;
                }
            } else if before {
                // out by the Challenge Boards, facing the plaza
                let side = was.and_then(|d| d.side(k)).unwrap_or(0);
                let (pos, yaw) = (v3(-3.0 + 6.0 * side as f32, 0.0, 10.5), std::f32::consts::PI);
                let f = &mut self.f[k];
                f.respawn_at(pos, yaw);
                f.kit = None;
                f.equip();
                self.apply_look_weapon(k);
            }
        }
    }

    fn host_sit(&mut self, from: usize, seat: u16) {
        if seat != SEAT_NONE {
            let taken = self.f.iter().enumerate().any(|(k, f)| k != from && f.present && f.hub.seat == seat);
            let near = seat_pose(seat).is_some_and(|(p, _)| (v3(p.x, self.f[from].pos.y, p.z) - self.f[from].pos).len() < 2.5);
            if taken || !near {
                return;
            }
        }
        // standing up from a table's chair leaves the table
        let old = self.f[from].hub.seat;
        if old != SEAT_NONE && old >= TABLE_SEATS && seat != old {
            let t = ((old - TABLE_SEATS) / 2) as usize;
            self.host_table(from, t, TableAct::Stand);
        }
        self.hub_broadcast(HubEv::Sit { who: from as u8, seat });
        if seat >= TABLE_SEATS && seat != SEAT_NONE {
            let k = (seat - TABLE_SEATS) as usize;
            self.host_table(from, k / 2, TableAct::Sit((k % 2) as u8));
        }
    }

    fn host_challenge(&mut self, from: usize, to: u8, kind: DuelKind) {
        let Some(h) = self.hub.as_mut() else { return };
        if h.pit_of(from).is_some() || h.challenges.iter().any(|c| c.from as usize == from) {
            return;
        }
        if to != 255 && (to as usize >= self.f.len() || !self.f[to as usize].present || to as usize == from || h.pit_of(to as usize).is_some()) {
            return;
        }
        let id = h.next_id;
        h.next_id = h.next_id.wrapping_add(1).max(1);
        h.challenges.push(Challenge { id, from: from as u8, to, kind, t: if to == 255 { OPEN_TIME } else { CHALLENGE_TIME } });
        let name = self.f[from].name.clone();
        if to != 255 {
            // a regular thinks it over (most say yes)
            if self.f[to as usize].is_bot {
                if let Some(r) = h.regulars.get_mut(to as usize) {
                    r.consider = Some((id, 1.2 + (id % 3) as f32 * 0.5));
                }
            }
        } else {
            let text = format!("{name} posted an open {} on the Challenge Board", kind.name());
            self.host_send_challenges();
            self.hub_broadcast(HubEv::Note { text });
            return;
        }
        self.host_send_challenges();
    }

    fn host_answer(&mut self, by: usize, id: u16, accept: bool) {
        let Some(h) = self.hub.as_mut() else { return };
        let Some(k) = h.challenges.iter().position(|c| c.id == id) else { return };
        let c = h.challenges[k];
        if c.from as usize == by && !accept {
            // withdrawn
            h.challenges.remove(k);
            self.host_send_challenges();
            return;
        }
        if c.to != 255 && c.to as usize != by {
            return;
        }
        if c.from as usize == by {
            return;
        }
        h.challenges.remove(k);
        let (from_name, by_name) = (self.f[c.from as usize].name.clone(), self.f[by].name.clone());
        if !accept {
            self.host_send_challenges();
            self.hub_broadcast(HubEv::Note { text: format!("{by_name} turned down {from_name}'s challenge") });
            return;
        }
        // the challenges either of them had up go
        if let Some(h) = self.hub.as_mut() {
            h.challenges.retain(|x| x.from != c.from && x.from as usize != by && x.to != c.from && x.to as usize != by);
        }
        self.host_send_challenges();
        let free = self.hub.as_ref().and_then(|h| (0..2).find(|&p| h.pits[p].is_none()));
        match free {
            Some(p) => self.host_start_pit(p, [c.from, by as u8], c.kind),
            None => self.hub_broadcast(HubEv::Note { text: "Both Court Pits are taken: try again in a moment".into() }),
        }
    }

    fn host_send_challenges(&mut self) {
        let list = self.hub.as_ref().map(|h| h.challenges.iter().map(|c| (c.id, c.from, c.to, c.kind as u8)).collect()).unwrap_or_default();
        self.hub_broadcast(HubEv::Challenges { list });
    }

    pub(crate) fn host_send_board(&mut self, board: u8) {
        let rows = self.hub.as_ref().and_then(|h| h.boards.get(board as usize).cloned()).unwrap_or_default();
        self.hub_broadcast(HubEv::Board { board, rows });
    }

    /// Bring two players into pit `p` for a duel.
    pub fn host_start_pit(&mut self, p: usize, who: [u8; 2], kind: DuelKind) {
        let first_to = if kind == DuelKind::Ledger { self.mt.rules.best_of.max(1) / 2 + 1 } else { 2 };
        let d = PitDuel { who, kind, wins: [0, 0], first_to, phase: PitPhase::Intro, t: PIT_INTRO };
        let names = (self.f[who[0] as usize].name.clone(), self.f[who[1] as usize].name.clone());
        // (anyone sitting gets up: the seat is free for others)
        for &w in &who {
            if self.f[w as usize].hub.seat != SEAT_NONE {
                self.hub_broadcast(HubEv::Sit { who: w, seat: SEAT_NONE });
            }
        }
        self.hub_broadcast(HubEv::Pit { pit: p as u8, duel: Some(d) });
        self.hub_broadcast(HubEv::Note { text: format!("{} vs {}: a {} in the {}", names.0, names.1, kind.name(), if p == 0 { "West Court Pit" } else { "East Court Pit" }) });
    }

    /// The pits: countdowns, rounds, the end of a duel.
    fn host_pits(&mut self, dt: f32) {
        for p in 0..2 {
            let Some(mut d) = self.hub.as_ref().and_then(|h| h.pits[p]) else { continue };
            // someone left: the other wins
            let gone: Vec<usize> = (0..2).filter(|&s| !self.f[d.who[s] as usize].present).collect();
            if !gone.is_empty() && d.phase != PitPhase::Over {
                let s = gone[0];
                d.wins[1 - s] = d.first_to;
                d.phase = PitPhase::Over;
                d.t = PIT_OVER;
                self.hub_broadcast(HubEv::Pit { pit: p as u8, duel: Some(d) });
                continue;
            }
            d.t -= dt;
            let mut changed = false;
            match d.phase {
                PitPhase::Intro | PitPhase::Break if d.t <= 0.0 => {
                    d.phase = PitPhase::Fight;
                    d.t = 0.0;
                    changed = true;
                }
                PitPhase::Fight => {
                    let down: Vec<usize> = (0..2).filter(|&s| !self.f[d.who[s] as usize].alive).collect();
                    if !down.is_empty() {
                        // both fell at once: nobody scores the round
                        if down.len() == 1 {
                            d.wins[1 - down[0]] += 1;
                        }
                        if d.winner().is_some() {
                            d.phase = PitPhase::Over;
                            d.t = PIT_OVER;
                        } else {
                            d.phase = PitPhase::Break;
                            d.t = PIT_BREAK;
                        }
                        changed = true;
                    }
                }
                PitPhase::Over if d.t <= 0.0 => {
                    self.host_end_pit(p, d);
                    continue;
                }
                _ => {}
            }
            if changed {
                // (every computer moves its own duellists: see pit_moves)
                self.hub_broadcast(HubEv::Pit { pit: p as u8, duel: Some(d) });
            } else if let Some(h) = self.hub.as_mut() {
                h.pits[p] = Some(d);
            }
        }
    }

    fn host_end_pit(&mut self, p: usize, d: PitDuel) {
        let w = d.winner().unwrap_or(0);
        let (wn, ln) = (self.f[d.who[w] as usize].name.clone(), self.f[d.who[1 - w] as usize].name.clone());
        let text = format!("{wn} beat {ln} {}-{} ({}, {})", d.wins[w], d.wins[1 - w], d.kind.name(), if p == 0 { "West Pit" } else { "East Pit" });
        if let Some(h) = self.hub.as_mut() {
            h.pits[p] = Some(d);
        }
        self.hub_broadcast(HubEv::Pit { pit: p as u8, duel: None });
        self.hub_broadcast(HubEv::Note { text });
    }

    // ---- the tables

    fn host_table(&mut self, from: usize, t: usize, act: TableAct) {
        let now = self.time;
        let Some(h) = self.hub.as_mut() else { return };
        let Some(tb) = h.tables.get_mut(t) else { return };
        let me = Sitter::Player(from as u8);
        let mut changed = false;
        match act {
            TableAct::Sit(side) => {
                let s = (side as usize).min(1);
                if tb.seats[s] == Sitter::Empty || matches!(tb.seats[s], Sitter::Bot(_)) {
                    if let Some(old) = tb.side_of(from) {
                        tb.seats[old] = Sitter::Empty;
                    }
                    tb.seats[s] = me;
                    // a fresh game for a new pair
                    if tb.outcome != Outcome::Playing || tb.board.moves_played_any() == 0 {
                        tb.board = Board::new(tb.kind());
                        tb.outcome = Outcome::Playing;
                        tb.last.clear();
                    }
                    changed = true;
                }
            }
            TableAct::Stand => {
                if let Some(s) = tb.side_of(from) {
                    tb.seats[s] = Sitter::Empty;
                    // leaving a game in progress resigns it
                    if tb.outcome == Outcome::Playing && tb.board.moves_played_any() > 0 && tb.seats[1 - s] != Sitter::Empty {
                        tb.outcome = Outcome::Win((1 - s) as u8);
                    }
                    // nobody human left: the bot gets up too
                    if !tb.seats.iter().any(|s| matches!(s, Sitter::Player(_))) {
                        tb.seats = [Sitter::Empty; 2];
                    }
                    changed = true;
                }
            }
            TableAct::Bot(level) => {
                if let Some(s) = tb.side_of(from) {
                    if tb.seats[1 - s] == Sitter::Empty || matches!(tb.seats[1 - s], Sitter::Bot(_)) {
                        tb.seats[1 - s] = Sitter::Bot(level);
                        tb.board = Board::new(tb.kind());
                        tb.outcome = Outcome::Playing;
                        tb.last.clear();
                        tb.thinking = None;
                        changed = true;
                    }
                }
            }
            TableAct::Reset => {
                let alone = !tb.seats.iter().any(|s| matches!(s, Sitter::Player(p) if *p as usize != from));
                if tb.side_of(from).is_some() && (tb.outcome != Outcome::Playing || alone) {
                    tb.board = Board::new(tb.kind());
                    tb.outcome = Outcome::Playing;
                    tb.last.clear();
                    tb.thinking = None;
                    // swap sides for the next game
                    if tb.seats.iter().all(|s| *s != Sitter::Empty) {
                        tb.seats.swap(0, 1);
                    }
                    changed = true;
                }
            }
            TableAct::Resign => {
                if let Some(s) = tb.side_of(from) {
                    if tb.outcome == Outcome::Playing {
                        tb.outcome = Outcome::Win((1 - s) as u8);
                        changed = true;
                    }
                }
            }
            TableAct::Move(m) => {
                if tb.playing() && tb.to_move() == me {
                    let text = describe(&tb.board, &m);
                    if tb.board.play(&m) {
                        tb.last = text;
                        tb.moved_t = now;
                        tb.outcome = tb.board.outcome();
                        changed = true;
                    }
                }
            }
        }
        if changed {
            tb.ver += 1;
            self.host_table_done(t);
        }
    }

    /// After a table changed: a game just won against a Hard bot is earned; everyone hears.
    fn host_table_done(&mut self, t: usize) {
        let Some(h) = self.hub.as_mut() else { return };
        let tb = &h.tables[t];
        let mut won = None;
        if let Outcome::Win(s) = tb.outcome {
            let hard = tb.seats[1 - s as usize] == Sitter::Bot(Level::Hard);
            if tb.seats[s as usize] == Sitter::Player(self.local as u8) {
                won = Some((tb.board.kind() as u8, hard));
            }
            if hard && tb.seats[s as usize] == Sitter::Player(self.local as u8) {
                h.earned.table_hard = true;
            }
        }
        if let Some((kind, hard)) = won {
            self.career.push(crate::game::CareerEv::TableWin { kind, hard });
        }
        self.host_send_table(t);
    }

    fn host_send_table(&mut self, t: usize) {
        let Some(h) = self.hub.as_ref() else { return };
        let tb = &h.tables[t];
        let e = HubEv::TableState { table: t as u8, ver: tb.ver, board: tb.board.to_bytes(), seats: [tb.seats[0].to_u8(), tb.seats[1].to_u8()], outcome: tb.outcome.to_u8(), last: tb.last.clone() };
        if self.kind == Kind::Host {
            self.out_events.push(Ev::Hub(e));
        }
    }

    /// The tables' bots: think on another thread, move after a short pause.
    fn host_table_bots(&mut self) {
        let now = self.time;
        let mut done = Vec::new();
        let Some(h) = self.hub.as_mut() else { return };
        for (k, tb) in h.tables.iter_mut().enumerate() {
            if !tb.playing() {
                tb.thinking = None;
                continue;
            }
            let Sitter::Bot(level) = tb.to_move() else {
                tb.thinking = None;
                continue;
            };
            match tb.thinking.as_ref() {
                None => {
                    if now - tb.moved_t < 0.9 {
                        continue;
                    }
                    let board = tb.board.clone();
                    let mut rng = tb.rng;
                    tb.rng.next();
                    let (tx, rx) = std::sync::mpsc::channel();
                    std::thread::spawn(move || {
                        let _ = tx.send(board.bot_move(level, &mut rng));
                    });
                    tb.thinking = Some(rx);
                }
                Some(rx) => {
                    if let Ok(m) = rx.try_recv() {
                        tb.thinking = None;
                        if let Some(m) = m {
                            let text = describe(&tb.board, &m);
                            if tb.board.play(&m) {
                                tb.last = text;
                                tb.moved_t = now;
                                tb.outcome = tb.board.outcome();
                                tb.ver += 1;
                                done.push(k);
                            }
                        }
                    }
                }
            }
        }
        for k in done {
            self.host_table_done(k);
        }
    }

    // ---- every tick

    /// The hub's part of a tick (after the fighters moved).
    pub fn hub_tick(&mut self, dt: f32) {
        if !self.in_hub() {
            return;
        }
        crate::commands::tick(self, dt);
        crate::gifts::tick(self, dt);
        let me = self.local;
        // emotes end on their own, or when you move; a seat is left by walking off
        for k in 0..self.f.len() {
            let f = &mut self.f[k];
            if !f.present || f.hub.emote == 0 {
                continue;
            }
            let e = Emote::from_u8(f.hub.emote);
            let moving = f.vel.flat().len() > 0.8 || !f.on_ground;
            let over = e.length().is_some_and(|l| self.time - f.hub.emote_t > l);
            if f.owned && (over || (moving && f.hub.seat == SEAT_NONE)) {
                f.hub.emote = 0;
                if k == me && moving && !over {
                    self.hub_send(HubEv::Emote { who: me as u8, emote: 0 });
                }
            }
        }
        // out of the pits and the aim rooms, a blade is carried (never a gun up)
        let aiming = self.hub.as_ref().is_some_and(|h| h.aim.is_some());
        for k in 0..self.f.len() {
            let f = &self.f[k];
            if f.owned && f.present && f.hub.pit == 255 && !(k == me && aiming) && !f.weapon.is_melee() && !self.in_raid(k) {
                self.apply_look_weapon(k);
            }
        }
        // sitting: kept on the seat; moving gets you up
        for k in 0..self.f.len() {
            if !self.f[k].owned || self.f[k].hub.seat == SEAT_NONE {
                continue;
            }
            if let Some((p, yaw)) = seat_pose(self.f[k].hub.seat) {
                let fwd = v3(yaw.sin(), 0.0, yaw.cos());
                let at = v3(p.x, (p.y - 0.45).max(0.0), p.z) + fwd * 0.1;
                // walking (or jumping) off stands you up; otherwise you stay put on the seat
                // (one tick of walking from a standstill is about 0.45 m/s)
                let leaving = self.f[k].vel.flat().len() > 0.3 || (self.f[k].pos - at).flat().len() > 0.5;
                if leaving {
                    if k == me {
                        self.hub_send(HubEv::Sit { who: me as u8, seat: SEAT_NONE });
                    }
                } else {
                    self.f[k].pos = v3(at.x, self.f[k].pos.y, at.z);
                    self.f[k].vel = V3::ZERO;
                }
            }
        }
        // our look goes out now and then (for anyone who missed it, or joined since)
        let resend = {
            let h = self.hub.as_mut().expect("hub");
            h.look_t -= dt;
            h.toasts.iter_mut().for_each(|t| t.1 -= dt);
            h.toasts.retain(|t| t.1 > 0.0);
            if let Some((t, _)) = h.fade.as_mut() {
                *t -= dt;
            }
            if h.fade.as_ref().is_some_and(|f| f.0 <= 0.0) {
                h.fade = None;
            }
            if let Some((_, t)) = h.aim_result.as_mut() {
                *t -= dt;
            }
            if h.aim_result.as_ref().is_some_and(|a| a.1 <= 0.0) {
                h.aim_result = None;
            }
            if h.look_t <= 0.0 {
                h.look_t = 5.0;
                true
            } else {
                false
            }
        };
        if resend && matches!(self.kind, Kind::Client | Kind::Host) {
            let look = self.f[me].hub.look;
            let emote = self.f[me].hub.emote;
            self.out_events.push(Ev::Hub(HubEv::Look { who: me as u8, look }));
            self.out_events.push(Ev::Hub(HubEv::Emote { who: me as u8, emote }));
        }
        // the Rooftop Run and the aim room (this computer's own)
        self.hub_run(dt);
        crate::hub_aim::tick(self, dt);
        if !self.is_authority() {
            return;
        }
        // ---- the host
        self.host_pits(dt);
        self.host_table_bots();
        // challenges run out
        let expired = {
            let h = self.hub.as_mut().expect("hub");
            let before = h.challenges.len();
            for c in h.challenges.iter_mut() {
                c.t -= dt;
            }
            h.challenges.retain(|c| c.t > 0.0);
            h.challenges.len() != before
        };
        if expired {
            self.host_send_challenges();
        }
        crate::hub_bots::think_all(self, dt);
        // everything shared goes out again every few seconds (for joiners and lost packets)
        let send_all = {
            let h = self.hub.as_mut().expect("hub");
            h.sync_t -= dt;
            if h.sync_t <= 0.0 {
                h.sync_t = 3.0;
                true
            } else {
                false
            }
        };
        if send_all && self.kind == Kind::Host {
            self.host_sync_all();
        }
    }

    /// Everything shared, sent again (for players who joined since, or lost a packet).
    pub fn host_sync_all(&mut self) {
        for t in 0..varrow::TABLES.len() {
            self.host_send_table(t);
        }
        for p in 0..2 {
            let duel = self.hub.as_ref().and_then(|h| h.pits[p]);
            self.out_events.push(Ev::Hub(HubEv::Pit { pit: p as u8, duel }));
        }
        let list = self.hub.as_ref().map(|h| h.challenges.iter().map(|c| (c.id, c.from, c.to, c.kind as u8)).collect()).unwrap_or_default();
        self.out_events.push(Ev::Hub(HubEv::Challenges { list }));
        for b in 0..=BOARD_RUN {
            let rows = self.hub.as_ref().and_then(|h| h.boards.get(b as usize).cloned()).unwrap_or_default();
            if !rows.is_empty() {
                self.out_events.push(Ev::Hub(HubEv::Board { board: b, rows }));
            }
        }
        for k in 0..self.f.len() {
            if self.f[k].present && k != self.local && self.f[k].owned {
                let (look, emote, seat) = (self.f[k].hub.look, self.f[k].hub.emote, self.f[k].hub.seat);
                self.out_events.push(Ev::Hub(HubEv::Look { who: k as u8, look }));
                self.out_events.push(Ev::Hub(HubEv::Emote { who: k as u8, emote }));
                if seat != SEAT_NONE {
                    self.out_events.push(Ev::Hub(HubEv::Sit { who: k as u8, seat }));
                }
            }
        }
        for k in 0..self.f.len() {
            let seat = self.f[k].hub.seat;
            if self.f[k].present && seat != SEAT_NONE && !self.f[k].owned {
                self.out_events.push(Ev::Hub(HubEv::Sit { who: k as u8, seat }));
            }
        }
    }

    /// The Rooftop Run: the rings in order against the clock, recording a ghost as you go.
    fn hub_run(&mut self, dt: f32) {
        let me = self.local;
        let f = &self.f[me];
        let (pos, yaw, ground, alive) = (f.pos, f.yaw, f.on_ground, f.alive);
        let in_pit = f.hub.pit != 255;
        let name = f.name.clone();
        let Some(h) = self.hub.as_mut() else { return };
        let rings = varrow::RUN_RINGS.len();
        if in_pit || !alive {
            h.run.running = false;
            h.run.next = 0;
            return;
        }
        if h.run.last.is_some() {
            h.run.last_t -= dt;
            if h.run.last_t <= 0.0 {
                h.run.last = None;
            }
        }
        let at = ring_at(pos);
        if h.run.running {
            h.run.t += dt;
            h.run.rec_acc += dt;
            if h.run.rec_acc >= GHOST_STEP {
                h.run.rec_acc = 0.0;
                let t = h.run.t;
                h.run.frames.push(GhostFrame { t, pos, yaw, ground });
            }
            // too long, or back on the ground away from the course: it's off
            if h.run.t > 180.0 {
                h.run.running = false;
                h.run.next = 0;
            }
            if at == Some(h.run.next) {
                h.run.next += 1;
                if h.run.next == rings {
                    // finished
                    let t = h.run.t;
                    let ms = (t * 1000.0).round() as u32;
                    let best = h.run_best == 0 || ms < h.run_best;
                    if best {
                        h.run_best = ms;
                        let mut frames = std::mem::take(&mut h.run.frames);
                        frames.push(GhostFrame { t, pos, yaw, ground });
                        h.ghost = frames;
                    }
                    h.run.frames.clear();
                    h.run.running = false;
                    h.run.next = 0;
                    h.run.last = Some((t, best));
                    h.run.last_t = 6.0;
                    h.earned.rooftop = true;
                    h.toast(format!("Rooftop Run: {t:.2} s{}", if best { "  - your best!" } else { "" }));
                    let _ = name;
                    let who = me as u8;
                    self.career.push(crate::game::CareerEv::Rooftop { ms });
                    self.hub_send(HubEv::Score { who, board: BOARD_RUN, value: ms });
                } else {
                    let n = h.run.next;
                    h.toast(format!("Ring {}/{}", n - 1, rings - 2));
                }
            }
        } else if at == Some(0) {
            h.run = RunState { next: 1, t: 0.0, running: true, frames: vec![GhostFrame { t: 0.0, pos, yaw, ground }], rec_acc: 0.0, last: None, last_t: 0.0 };
            h.toast("The Rooftop Run: go!");
        }
    }

    /// Varrow's townsfolk, as fighters to draw (on this computer only: scenery, never players,
    /// never sent): three walking their rounds of the square, three browsing at the stalls and
    /// chatting. Each wears a mix of the wardrobe's pieces in plain market colours; nobody holds
    /// a weapon. Those further than 45 m from the camera are left out.
    pub fn hub_townsfolk(&self, cam: V3) -> Vec<Fighter> {
        if !self.in_hub() {
            return Vec::new();
        }
        let t = self.time;
        // (name, body, head, body, legs, feet, neck, dyes coat/trim/accent)
        const FOLK: [(&str, u8, u8, u8, u8, u8, u8, [u8; 3]); 6] = [
            ("Ada the weaver", 1, 40, 43, 44, 46, 0, [21, 0, 11]),
            ("Old Pell", 0, 30, 33, 34, 36, 32, [16, 1, 5]),
            ("Brin the porter", 3, 0, 2, 3, 5, 0, [14, 1, 11]),
            ("Mags", 2, 20, 23, 24, 26, 0, [19, 1, 12]),
            ("Tam the carter", 1, 0, 43, 34, 36, 32, [22, 21, 10]),
            ("Sel", 0, 40, 33, 3, 5, 0, [5, 1, 11]),
        ];
        let lap = |pts: &[(f32, f32)], s: f32| -> (V3, V3) {
            let n = pts.len();
            let seg: Vec<f32> = (0..n).map(|k| {
                let (a, b) = (pts[k], pts[(k + 1) % n]);
                ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
            }).collect();
            let total: f32 = seg.iter().sum();
            let mut d = s.rem_euclid(total);
            for k in 0..n {
                if d <= seg[k] {
                    let (a, b) = (pts[k], pts[(k + 1) % n]);
                    let f = d / seg[k].max(1e-3);
                    let p = v3(a.0 + (b.0 - a.0) * f, 0.0, a.1 + (b.1 - a.1) * f);
                    return (p, v3(b.0 - a.0, 0.0, b.1 - a.1).norm());
                }
                d -= seg[k];
            }
            (v3(pts[0].0, 0.0, pts[0].1), v3(0.0, 0.0, 1.0))
        };
        let circle = |r: f32, n: usize, dir: f32| -> Vec<(f32, f32)> { (0..n).map(|k| { let a = dir * k as f32 / n as f32 * std::f32::consts::TAU; (a.cos() * r, a.sin() * r) }).collect() };
        let round_a = circle(11.0, 20, 1.0);
        let round_b = circle(13.5, 24, -1.0);
        let stalls_way = [(-12.0, 13.5), (0.0, 12.5), (12.0, 13.5), (16.5, 6.5), (16.5, -6.5), (11.0, -14.5), (-11.0, -14.5), (-15.0, 0.0)];
        let mut out = Vec::new();
        for (k, &(name, body, head, coat, legs, feet, neck, dyes)) in FOLK.iter().enumerate() {
            let (pos, dir, speed) = match k {
                0 => { let (p, d) = lap(&round_a, t * 1.25); (p, d, 1.25) }
                1 => { let (p, d) = lap(&round_b, t * 1.05 + 20.0); (p, d, 1.05) }
                2 => { let (p, d) = lap(&stalls_way, t * 1.4 + 9.0); (p, d, 1.4) }
                _ => {
                    // at a stall, facing its counter, shifting a little
                    let (_, x, z, fx, fz) = crate::varrow::STALLS[(k - 3) % crate::varrow::STALLS.len()];
                    let side = v3(-fz, 0.0, fx) * ((k as f32 * 1.3).sin() * 0.8);
                    let p = v3(x + fx * 1.75, 0.0, z + fz * 1.75) + side;
                    (p, v3(-fx, 0.0, -fz), 0.0)
                }
            };
            if (pos - cam).len() > 45.0 {
                continue;
            }
            let mut f = Fighter::new(name, MAX_PLAYERS + 8 + k, false);
            f.present = true;
            f.alive = true;
            f.is_bot = true;
            f.loadout.body = body;
            f.color_idx = (k % 8) as u8;
            f.pos = pos;
            f.prev_pos = pos;
            f.net_pos = pos;
            f.yaw = dir.x.atan2(dir.z);
            f.vel = dir * speed;
            f.on_ground = true;
            f.cur = 0;
            f.weapon = Weapon::Dagger;
            let mut o = Outfit::default();
            for (slot, id) in [(Slot::Head, head), (Slot::Body, coat), (Slot::Legs, legs), (Slot::Feet, feet), (Slot::Neck, neck)] {
                o.items[slot as usize] = id;
                o.dyes[slot as usize] = dyes;
            }
            f.hub.look = o;
            f.hub.townsfolk = true;
            // browsers chat now and then (and laugh)
            if speed == 0.0 {
                let cycle = (t + k as f32 * 3.7) % 9.0;
                if cycle < 2.6 {
                    f.hub.emote = if k % 2 == 0 { Emote::Laugh as u8 } else { Emote::Wave as u8 };
                    f.hub.emote_t = t - cycle;
                }
            }
            out.push(f);
        }
        // the Banker behind the bank's counter (crate::bank_ui), in green and gold
        let (kx, _, kz) = crate::varrow::BANK;
        let bp = v3(kx + 1.3, 0.0, kz);
        if (bp - cam).len() < 45.0 {
            let mut f = Fighter::new("Banker Odo", MAX_PLAYERS + 15, false);
            f.present = true;
            f.alive = true;
            f.is_bot = true;
            f.loadout.body = 2;
            f.color_idx = 2;
            f.pos = bp;
            f.prev_pos = bp;
            f.net_pos = bp;
            f.yaw = -std::f32::consts::FRAC_PI_2;
            f.on_ground = true;
            f.cur = 0;
            f.weapon = Weapon::Dagger;
            let mut o = Outfit::default();
            for (slot, id) in [(Slot::Head, 20), (Slot::Body, 23), (Slot::Legs, 24), (Slot::Feet, 26), (Slot::Neck, 22)] {
                o.items[slot as usize] = id;
                o.dyes[slot as usize] = [19, 1, 23];
            }
            f.hub.look = o;
            f.hub.townsfolk = true;
            // a nod now and then
            let cycle = t % 11.0;
            if cycle < 2.0 {
                f.hub.emote = Emote::Bow as u8;
                f.hub.emote_t = t - cycle;
            }
            out.push(f);
        }
        out
    }

    /// The ghost of your best run, where it is now (while you run), as a fighter to draw.
    pub fn hub_ghosts(&self) -> Vec<Fighter> {
        let Some(h) = self.hub.as_ref() else { return Vec::new() };
        if !h.run.running || h.ghost.is_empty() {
            return Vec::new();
        }
        let Some((pos, yaw, ground, vel)) = ghost_at(&h.ghost, h.run.t) else { return Vec::new() };
        let mut f = self.f[self.local].clone();
        f.pos = pos;
        f.prev_pos = pos;
        f.yaw = yaw;
        f.pitch = 0.0;
        f.vel = vel;
        f.on_ground = ground;
        f.wall = None;
        f.slash_t = 0.0;
        f.ghost = 0.55;
        f.hub.emote = 0;
        vec![f]
    }

    /// Filter what a fighter can do in the hub: no blades or guns outside a pit or an aim room,
    /// nothing while sitting except standing up.
    pub fn hub_input(&self, i: usize, mut inp: Input) -> Input {
        let Some(h) = self.hub.as_ref() else { return inp };
        // the Ashen Vault: fight freely, but nothing at all while down
        if self.in_raid(i) {
            if self.raid.as_ref().is_some_and(|r| r.is_downed(i)) {
                return Input { yaw: inp.yaw, pitch: inp.pitch, ..Default::default() };
            }
            inp.finish = false;
            // the Regent's ash-blood pools: no block, no abilities while you stand in one
            if crate::raid::in_ash(self, i) {
                inp.guard = false;
                inp.ability = false;
                inp.utility = false;
                inp.spec = false;
            }
            return inp;
        }
        let in_pit = h.pit_of(i).is_some();
        let aiming = i == self.local && h.aim.as_ref().is_some_and(|a| a.live());
        if !in_pit {
            inp.ability = false;
            inp.utility = false;
            inp.kick = false;
            inp.finish = false;
            inp.item = false;
            inp.spec = false;
            inp.inv = None;
            if !aiming {
                inp.fire = false;
                inp.fire_pressed = false;
                inp.guard = inp.guard && false;
            }
        }
        if self.f[i].hub.seat != SEAT_NONE {
            inp.jump = false;
            inp.dash = false;
            inp.fire = false;
            inp.fire_pressed = false;
        }
        inp
    }
}

/// A move written out for the table's line ("e2-e4", "c3xe5", "column 4").
pub fn describe(b: &Board, m: &AnyMove) -> String {
    match (b, m) {
        (Board::Chess(c), AnyMove::Chess(mv)) => c.describe(*mv),
        (_, AnyMove::Checkers(mv)) => mv.0.iter().map(|&s| boards::sq_name(s)).collect::<Vec<_>>().join(if mv.is_jump() { "x" } else { "-" }),
        (_, AnyMove::Drop(c)) => format!("column {}", c + 1),
        _ => String::new(),
    }
}

impl Board {
    /// Moves played in this game (any kind).
    pub fn moves_played_any(&self) -> usize {
        match self {
            // (a board sent over the network has no move list: compare it with a fresh one)
            Board::Chess(c) => c.moves.len().max((c.sq != boards::Chess::new().sq || c.turn != 0) as usize),
            Board::Checkers(c) => c.moves.len().max((c.sq != boards::Checkers::new().sq || c.turn != 0) as usize),
            Board::Connect4(c) => c.cells.iter().flatten().filter(|v| **v != 0).count(),
        }
    }
}
