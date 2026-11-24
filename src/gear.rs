//! Ledger Duels: the sanctioned duels of the Ashen Court (crate::game::Mode::Ledger). One on one,
//! with gear: eight equipment slots (head, body, legs, hands, feet, main hand, off hand, charm)
//! and a 12-slot inventory of switch weapons, food and tonics. Every item states what it does
//! (accuracy, damage, defence against blades / bullets / blasts, move speed, weight); every item
//! is open to everyone in this mode (no grinding or paying for power: mastery only changes how an
//! item's card looks).
//!
//! Aim and timing still decide every hit: gear changes how much a hit does (with a small spread
//! of +-6%), how much is shrugged off, how fast you move and how quickly you can swap. Four sets
//! give a bonus for wearing all four pieces, each a real trade-off; each weapon has a special
//! attack (its "spec": 25, 50 or 100% of a slowly refilling bar) with a wind-up the other side can
//! see and answer. Food heals but takes time to eat (no attacking meanwhile); a light bite or a
//! tonic can be eaten straight after, for a little extra delay.
//!
//! The challenger sets the duel's rules (no guns, no blades, no food, no specs, no movement, no
//! gear switching, no sets; best of 1, 3 or 5) and both sides see them before accepting. Nothing
//! is ever staked: the honour ledger keeps wins and streaks per loadout instead.

use crate::game::Weapon;

pub const SLOTS: usize = 8;
pub const INV: usize = 12;
/// Inventory slots on the number keys (1-6); all 12 are on the radial.
pub const HOTKEYS: usize = 6;
/// Saved loadouts.
pub const SAVED: usize = 5;

/// Hits outside a special can't do more than this before gear (sniper body shots, hand cannon
/// and revolver headshots included), nor more than NORMAL_MAX after it.
pub const BASE_CAP: f32 = 45.0;
pub const NORMAL_MAX: f32 = 55.0;
/// Damage spread either way (a small one: aim decides hits, not dice).
pub const VARIANCE: f32 = 0.06;
/// Most damage any one defence takes off.
pub const DEF_CAP: f32 = 40.0;
/// The special bar (0-100) refills this much a second (before regen bonuses).
pub const SPEC_REGEN: f32 = 1.6;
/// Eating: the extra a combo bite adds to a meal's lock, and a bite's lock on its own.
pub const COMBO_EXTRA: f32 = 0.25;
pub const BITE_LOCK: f32 = 0.6;
/// Walking speed while eating and while winding up a special.
pub const EAT_SLOW: f32 = 0.7;
pub const WIND_SLOW: f32 = 0.6;
/// A strike special waits this long for its attack (then it's spent).
pub const ARMED_TIME: f32 = 0.8;
/// Haul: the chain rattles and the hook glows for this long, then it flies this far, and a caught
/// fighter is staggered this long.
pub const HAUL_WINDUP: f32 = 0.4;
pub const HAUL_RANGE: f32 = 8.0;
pub const HAUL_STAGGER: f32 = 0.5;
/// Change of Tools: swaps this much quicker; a blade hit and a gun hit within TOOLS_WINDOW
/// seconds give TOOLS_SPEC bar, at most once every TOOLS_COOLDOWN.
pub const TOOLS_SWAP: f32 = 0.7;
pub const TOOLS_WINDOW: f32 = 3.0;
pub const TOOLS_SPEC: f32 = 10.0;
pub const TOOLS_COOLDOWN: f32 = 6.0;
/// The Rigger's Snub (an off-hand pistol): rounds, time between shots, reload, damage of a round.
pub const SNUB: u8 = 58;
pub const SNUB_ROUNDS: u8 = 3;
pub const SNUB_GAP: f32 = 0.5;
pub const SNUB_RELOAD: f32 = 2.2;
pub const SNUB_DMG: f32 = 14.0;
/// The Rigger's Hook and its set's charm (glows teal while the set's bonus is on).
pub const HOOK: u8 = 57;
pub const GEAR_CHARM: u8 = 56;
/// After a strike special's attack, hits within this long carry it (a blade's swing lands a moment
/// after it starts; a hitscan gun's hits are the same instant).
pub const STRIKE_MELEE: f32 = 0.7;
pub const STRIKE_GUN: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Head = 0,
    Body = 1,
    Legs = 2,
    Hands = 3,
    Feet = 4,
    Main = 5,
    Off = 6,
    Charm = 7,
}

impl Slot {
    pub const ALL: [Slot; SLOTS] = [Slot::Head, Slot::Body, Slot::Legs, Slot::Hands, Slot::Feet, Slot::Main, Slot::Off, Slot::Charm];
    pub fn name(self) -> &'static str {
        match self {
            Slot::Head => "Head",
            Slot::Body => "Body",
            Slot::Legs => "Legs",
            Slot::Hands => "Hands",
            Slot::Feet => "Feet",
            Slot::Main => "Main hand",
            Slot::Off => "Off hand",
            Slot::Charm => "Charm",
        }
    }
}

/// What an item is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Gear(Slot),
    /// A meal: heals, then a lock while it's eaten.
    Food,
    /// A light bite: heals a little, and can be eaten straight after a meal.
    Bite,
    /// A drink with an effect; like a bite, it can follow a meal.
    Tonic(Tonic),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tonic {
    /// +12% damage for 20 s.
    Fury = 0,
    /// +10% to every defence for 20 s.
    Iron = 1,
    /// +8% move speed for 15 s.
    Haste = 2,
    /// +20 special bar, at once.
    Clarity = 3,
}

impl Tonic {
    pub fn secs(self) -> f32 {
        match self {
            Tonic::Fury | Tonic::Iron => 20.0,
            Tonic::Haste => 15.0,
            Tonic::Clarity => 0.0,
        }
    }
}

/// The five sets: all four pieces (head, body, legs, hands) give the bonus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Set {
    /// Heavy plate of the Court's gravekeepers: more damage the lower your own health.
    Gravewarden = 1,
    /// A tower of riveted iron: tougher against everything, but it hits softer.
    Bastion = 2,
    /// Courier's silks: faster on foot and quicker swaps, little protection.
    Swiftshade = 3,
    /// A marksman's coat: guns steadier and harder-hitting, blades weaker.
    Longsight = 4,
    /// The Bridgeworks crane-riggers' rig: the switcher's set, medium at everything; quicker swaps
    /// and special bar for landing hits with a blade and a gun close together.
    Bridgewright = 5,
}

impl Set {
    pub const ALL: [Set; 5] = [Set::Gravewarden, Set::Bastion, Set::Swiftshade, Set::Longsight, Set::Bridgewright];
    pub fn name(self) -> &'static str {
        match self {
            Set::Gravewarden => "Gravewarden",
            Set::Bastion => "Bastion",
            Set::Swiftshade => "Swiftshade",
            Set::Longsight => "Longsight",
            Set::Bridgewright => "Bridgewright's Rig",
        }
    }
    pub fn bonus(self) -> &'static str {
        match self {
            Set::Gravewarden => "up to +30% damage as your own health falls (none at full health): high risk",
            Set::Bastion => "+5% to every defence, but -25% damage",
            Set::Swiftshade => "+12% move speed and gear swaps 0.1 s quicker",
            Set::Longsight => "guns: +20% accuracy and +10% damage; blades: -12% damage",
            Set::Bridgewright => "Change of Tools: gear swaps 30% quicker; landing a blade hit and a gun hit within 3 s gives +10 special bar (at most once every 6 s)",
        }
    }
}

/// Each weapon's special attack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spec {
    RiposteLine = 1,
    HeadsmansDrop = 2,
    FangFlurry = 3,
    QuickVerdict = 4,
    Toll = 5,
    SealBreaker = 6,
    SlagBurst = 7,
    Rattle = 8,
    FinalEntry = 9,
    RuinShell = 10,
    Starfall = 11,
    /// The Rigger's Hook: the hook thrown on a chain, yanking them in.
    Haul = 12,
}

/// What a special does.
#[derive(Clone, Copy, Debug)]
pub struct SpecInfo {
    pub name: &'static str,
    /// Share of the bar it costs (25, 50 or 100).
    pub cost: f32,
    /// The wind-up (seen and heard by the other side) before it goes.
    pub windup: f32,
    /// Damage times this.
    pub mult: f32,
    /// Share of the target's defence it ignores (1: armour-piercing).
    pub pierce: f32,
    /// A blocked blow still smashes the guard.
    pub guard_break: bool,
    /// 0: the next attack is the special. Otherwise every hit for this long is.
    pub timed: f32,
    /// Most one hit of it can do.
    pub cap: f32,
    pub blurb: &'static str,
}

impl Spec {
    pub const ALL: [Spec; 12] = [
        Spec::RiposteLine,
        Spec::HeadsmansDrop,
        Spec::FangFlurry,
        Spec::QuickVerdict,
        Spec::Toll,
        Spec::SealBreaker,
        Spec::SlagBurst,
        Spec::Rattle,
        Spec::FinalEntry,
        Spec::RuinShell,
        Spec::Starfall,
        Spec::Haul,
    ];
    pub fn from_u8(v: u8) -> Option<Spec> {
        Spec::ALL.iter().copied().find(|s| *s as u8 == v)
    }
    pub fn info(self) -> SpecInfo {
        #[allow(clippy::too_many_arguments)]
        fn s(name: &'static str, cost: f32, windup: f32, mult: f32, pierce: f32, guard_break: bool, timed: f32, cap: f32, blurb: &'static str) -> SpecInfo {
            SpecInfo { name, cost, windup, mult, pierce, guard_break, timed, cap, blurb }
        }
        match self {
            Spec::RiposteLine => s("Riposte Line", 25.0, 0.30, 1.45, 0.25, false, 0.0, 70.0, "a lunging thrust that slips past a quarter of their defence"),
            Spec::HeadsmansDrop => s("Headsman's Drop", 50.0, 0.55, 2.0, 0.0, true, 0.0, 85.0, "a crushing overhead: doubles the blow, and smashes a guard that tries to stop it"),
            Spec::FangFlurry => s("Fang Flurry", 50.0, 0.35, 1.5, 0.0, false, 1.2, 60.0, "for 1.2 s every dagger cut hits half again as hard"),
            Spec::QuickVerdict => s("Quick Verdict", 25.0, 0.30, 1.5, 0.2, false, 0.0, 70.0, "the next shot hits half again as hard"),
            Spec::Toll => s("Toll", 50.0, 0.45, 1.6, 0.4, false, 0.0, 80.0, "a heavy round that ignores 40% of their defence"),
            Spec::SealBreaker => s("Seal Breaker", 50.0, 0.45, 1.6, 1.0, false, 0.0, 80.0, "an armour-piercing shot: no defence stops it"),
            Spec::SlagBurst => s("Slag Burst", 50.0, 0.40, 1.5, 0.0, false, 0.0, 85.0, "the next blast of slag hits half again as hard"),
            Spec::Rattle => s("Rattle", 50.0, 0.35, 1.4, 0.0, false, 1.6, 40.0, "for 1.6 s every bullet hits 40% harder"),
            Spec::FinalEntry => s("Final Entry", 100.0, 0.60, 1.0, 0.5, false, 0.0, 95.0, "a scoped shot that keeps its full weight (up to 95) and ignores half their defence"),
            Spec::RuinShell => s("Ruin Shell", 100.0, 0.50, 1.7, 0.5, false, 3.0, 85.0, "for 3 s your shells hit 70% harder and ignore half their defence"),
            Spec::Starfall => s("Starfall", 50.0, 0.35, 1.45, 0.0, false, 1.5, 45.0, "for 1.5 s every star hits 45% harder"),
            Spec::Haul => s("Haul", 50.0, HAUL_WINDUP, 1.0, 0.0, false, 0.0, 12.0, "throws the hook on a chain up to 8 m: yanks them to you and staggers them for 0.5 s (dodge it, or parry it)"),
        }
    }
    /// The wind-up's colour on screen (and over the fighter's head): the dearer, the redder.
    pub fn color(self) -> [u8; 3] {
        let c = self.info().cost;
        if c >= 100.0 {
            [255, 70, 60]
        } else if c >= 50.0 {
            [255, 150, 50]
        } else {
            [255, 220, 90]
        }
    }
}

/// One item. Stats are percentages (accuracy, damage, defences, speed) and kilograms (weight).
#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub id: u8,
    pub name: &'static str,
    pub kind: Kind,
    /// A main-hand weapon's kind (how it swings or shoots).
    pub base: Option<Weapon>,
    /// Takes both hands (no off-hand).
    pub two_handed: bool,
    pub acc: i8,
    pub dmg: i8,
    /// Defence against blades, bullets, blasts.
    pub def: [i8; 3],
    pub speed: i8,
    /// Special bar refill (percent more).
    pub regen: i8,
    pub weight: u8,
    pub set: Option<Set>,
    pub spec: Option<Spec>,
    /// Food: health it gives back.
    pub heal: u8,
    pub blurb: &'static str,
}

#[allow(clippy::too_many_arguments)]
const fn armour(id: u8, name: &'static str, slot: Slot, def: [i8; 3], acc: i8, dmg: i8, speed: i8, weight: u8, set: Option<Set>, blurb: &'static str) -> Item {
    Item { id, name, kind: Kind::Gear(slot), base: None, two_handed: false, acc, dmg, def, speed, regen: 0, weight, set, spec: None, heal: 0, blurb }
}
#[allow(clippy::too_many_arguments)]
const fn weapon(id: u8, name: &'static str, base: Weapon, two_handed: bool, acc: i8, dmg: i8, speed: i8, weight: u8, spec: Spec, blurb: &'static str) -> Item {
    Item { id, name, kind: Kind::Gear(Slot::Main), base: Some(base), two_handed, acc, dmg, def: [0; 3], speed, regen: 0, weight, set: None, spec: Some(spec), heal: 0, blurb }
}
#[allow(clippy::too_many_arguments)]
const fn trinket(id: u8, name: &'static str, slot: Slot, def: [i8; 3], acc: i8, dmg: i8, speed: i8, regen: i8, weight: u8, blurb: &'static str) -> Item {
    Item { id, name, kind: Kind::Gear(slot), base: None, two_handed: false, acc, dmg, def, speed, regen, weight, set: None, spec: None, heal: 0, blurb }
}
const fn eat(id: u8, name: &'static str, kind: Kind, heal: u8, blurb: &'static str) -> Item {
    Item { id, name, kind, base: None, two_handed: false, acc: 0, dmg: 0, def: [0; 3], speed: 0, regen: 0, weight: 0, set: None, spec: None, heal, blurb }
}

const GW: Option<Set> = Some(Set::Gravewarden);
const BA: Option<Set> = Some(Set::Bastion);
const SW: Option<Set> = Some(Set::Swiftshade);
const LS: Option<Set> = Some(Set::Longsight);
const BW: Option<Set> = Some(Set::Bridgewright);

/// Every item (ids from 1; 0 is an empty slot).
pub const ITEMS: &[Item] = &[
    // ---- Gravewarden: heavy, strong against blades, weak against bullets
    armour(1, "Gravewarden Helm", Slot::Head, [6, 2, 3], 0, 0, -2, 4, GW, "a barred helm of blackened iron"),
    armour(2, "Gravewarden Hauberk", Slot::Body, [10, 3, 5], 0, 0, -3, 7, GW, "chain under plate, cold to the touch"),
    armour(3, "Gravewarden Greaves", Slot::Legs, [7, 2, 4], 0, 0, -2, 5, GW, "plated shins that ring on the stones"),
    armour(4, "Gravewarden Gauntlets", Slot::Hands, [3, 1, 1], 0, 1, 0, 2, GW, "spiked knuckles: a little more bite"),
    // ---- Bastion: tough against everything, heavy
    armour(5, "Bastion Visor", Slot::Head, [6, 4, 6], 0, 0, -2, 4, BA, "a riveted visor with a narrow slit"),
    armour(6, "Bastion Cuirass", Slot::Body, [9, 6, 9], 0, 0, -3, 8, BA, "a breastplate thick enough to stop a ball"),
    armour(7, "Bastion Tassets", Slot::Legs, [6, 4, 6], 0, 0, -2, 5, BA, "overlapping plates over the thighs"),
    armour(8, "Bastion Bracers", Slot::Hands, [3, 2, 3], 0, 0, -1, 2, BA, "iron bracers with a leather lining"),
    // ---- Swiftshade: light and quick
    armour(9, "Swiftshade Cowl", Slot::Head, [2, 2, 1], 0, 0, 1, 1, SW, "a courier's silk cowl"),
    armour(10, "Swiftshade Vest", Slot::Body, [4, 4, 2], 0, 0, 2, 2, SW, "quilted silk over a mail lining"),
    armour(11, "Swiftshade Leggings", Slot::Legs, [3, 3, 1], 0, 0, 2, 1, SW, "close-fitting, made for rooftops"),
    armour(12, "Swiftshade Wraps", Slot::Hands, [1, 1, 0], 4, 0, 0, 1, SW, "grip wraps for quick hands"),
    // ---- Longsight: the marksman's coat
    armour(13, "Longsight Hat", Slot::Head, [3, 4, 2], 5, 0, 0, 2, LS, "a wide brim against the glare"),
    armour(14, "Longsight Greatcoat", Slot::Body, [5, 7, 3], 0, 0, 0, 4, LS, "a long coat lined with tin plates"),
    armour(15, "Longsight Breeches", Slot::Legs, [4, 5, 2], 0, 0, 0, 3, LS, "reinforced riding breeches"),
    armour(16, "Longsight Gloves", Slot::Hands, [1, 2, 1], 6, 0, 0, 1, LS, "thin gloves for a light trigger"),
    // ---- plain pieces
    armour(17, "Plain Hood", Slot::Head, [2, 2, 2], 0, 0, 0, 1, None, "wool, nothing more"),
    armour(18, "Padded Jerkin", Slot::Body, [4, 4, 4], 0, 0, 0, 3, None, "layers of stitched linen"),
    armour(19, "Wool Trousers", Slot::Legs, [3, 3, 3], 0, 0, 0, 2, None, "warm, and that's all"),
    armour(20, "Leather Gloves", Slot::Hands, [1, 1, 1], 2, 0, 0, 1, None, "good grip"),
    // ---- feet
    armour(21, "Courier Boots", Slot::Feet, [1, 1, 1], 0, 0, 5, 1, None, "soft soles, fast feet"),
    armour(22, "Ironshod Boots", Slot::Feet, [4, 4, 3], 0, 0, -2, 3, None, "heavy boots with iron toes"),
    armour(23, "Ashwalk Sandals", Slot::Feet, [0, 0, 6], 0, 0, 2, 1, None, "fireproof straps from the foundries"),
    // ---- main hands
    weapon(24, "Courtly Sabre", Weapon::Katana, false, 4, 20, 0, 3, Spec::RiposteLine, "the Court's standard duelling blade"),
    weapon(25, "Ashfall Cleaver", Weapon::Greatsword, true, 0, 8, -4, 8, Spec::HeadsmansDrop, "a headsman's blade: slow, two hands, brutal"),
    weapon(26, "Twin Fangs", Weapon::Dagger, true, 6, 60, 6, 2, Spec::FangFlurry, "a pair of thin knives, one in each hand"),
    weapon(27, "Court Revolver", Weapon::Revolver, false, 3, 0, 0, 2, Spec::QuickVerdict, "six chambers and a ledger seal on the grip"),
    weapon(28, "Bellmouth Cannon", Weapon::HandCannon, false, -3, -10, 0, 4, Spec::Toll, "a short brass barrel that kicks like a mule"),
    weapon(29, "Ledger Repeater", Weapon::Rifle, true, 0, -58, 0, 5, Spec::SealBreaker, "a lever rifle for long Court bouts"),
    weapon(30, "Foundry Scattergun", Weapon::Shotgun, true, 0, -60, 0, 6, Spec::SlagBurst, "a foundry gun loaded with slag"),
    weapon(31, "Lowtown Rattler", Weapon::Smg, false, -2, 10, 0, 3, Spec::Rattle, "a cheap repeater from the Lower Wards"),
    weapon(32, "Warden Longbarrel", Weapon::Sniper, true, 8, -15, -3, 7, Spec::FinalEntry, "a border warden's long gun"),
    weapon(33, "Ember Mortar", Weapon::Rocket, true, 0, 0, -4, 8, Spec::RuinShell, "a tube that throws burning shells"),
    weapon(34, "Gutter Stars", Weapon::Shuriken, false, 2, -10, 0, 1, Spec::Starfall, "sharpened tin stars from the gutters"),
    // ---- off hands (one-handed main hands only)
    trinket(35, "Brass Buckler", Slot::Off, [8, 3, 2], 0, 0, -1, 0, 3, "a small round shield"),
    trinket(36, "Parrying Hook", Slot::Off, [10, 0, 0], 3, 0, 0, 0, 1, "a hooked knife for catching blades"),
    trinket(37, "Ward Lantern", Slot::Off, [0, 2, 10], 0, 0, 0, 0, 2, "a hooded lantern that takes the blast"),
    trinket(38, "Clerk's Ledger", Slot::Off, [0, 0, 0], 0, 0, 0, 25, 1, "keeping count steadies the hand: the special bar refills faster"),
    // ---- charms
    trinket(39, "Ash Seal", Slot::Charm, [0, 0, 0], 0, 5, 0, 0, 0, "a grey wax seal: +5% damage"),
    trinket(40, "Pilgrim Knot", Slot::Charm, [0, 0, 0], 0, 0, 4, 0, 0, "a knotted cord: +4% speed"),
    trinket(41, "Wax Writ", Slot::Charm, [0, 0, 0], 0, 0, 0, 25, 0, "an unsealed Writ: the special bar refills faster"),
    trinket(42, "Iron Thimble", Slot::Charm, [3, 3, 3], 0, 0, 0, 0, 0, "+3% to every defence"),
    trinket(43, "Ember Locket", Slot::Charm, [0, 0, 10], 0, 0, 0, 0, 0, "a locket of foundry ash"),
    // ---- food and tonics
    eat(44, "Hard Bread", Kind::Food, 16, "a meal: +16 health, then 1 s of eating"),
    eat(45, "Smoked Eel", Kind::Food, 24, "a meal: +24 health, then 1.2 s of eating"),
    eat(46, "Honey Cake", Kind::Bite, 10, "a bite: +10 health; can follow a meal straight away"),
    eat(47, "Ember Tonic", Kind::Tonic(Tonic::Fury), 0, "+12% damage for 20 s; can follow a meal"),
    eat(48, "Iron Draught", Kind::Tonic(Tonic::Iron), 0, "+10% to every defence for 20 s; can follow a meal"),
    eat(49, "Quickwater", Kind::Tonic(Tonic::Haste), 0, "+8% move speed for 15 s; can follow a meal"),
    eat(50, "Clarity Draught", Kind::Tonic(Tonic::Clarity), 0, "+20 special bar at once; can follow a meal"),
    // ---- Bridgewright's Rig: the crane-riggers' switcher set, medium at everything
    armour(51, "Bridgewright Half-Helm", Slot::Head, [4, 4, 4], 2, 0, 0, 3, BW, "a riveted iron half-helm, a brass lens over one eye, a rolled leather brim"),
    armour(52, "Bridgewright Work Coat", Slot::Body, [5, 5, 5], 0, 0, -1, 5, BW, "thick canvas over an iron chest plate; a tool harness and a coil of rope on the back"),
    armour(53, "Bridgewright Trousers", Slot::Legs, [4, 4, 4], 0, 0, 0, 3, BW, "padded canvas with iron knee caps"),
    armour(54, "Bridgewright Gauntlets", Slot::Hands, [2, 2, 2], 2, 0, 0, 2, BW, "oversized leather gauntlets with brass knuckle plates"),
    armour(55, "Hobnailed Boots", Slot::Feet, [3, 3, 3], 0, 0, 1, 2, None, "heavy hobnailed boots with ankle guards"),
    trinket(56, "Rigger's Gear Charm", Slot::Charm, [1, 1, 1], 0, 0, 0, 0, 0, "a small brass gear on a chain: it glows teal while the Rig's bonus is on"),
    weapon(57, "Rigger's Hook", Weapon::Katana, false, 2, -5, 0, 3, Spec::Haul, "a one-handed hooked axe on a chain: pairs with an off-hand pistol"),
    trinket(58, "Rigger's Snub", Slot::Off, [0, 0, 0], 0, 0, 0, 0, 2, "an off-hand pistol: R fires it while a one-handed blade is in hand (3 rounds)"),
];

pub fn item(id: u8) -> Option<&'static Item> {
    if id == 0 {
        return None;
    }
    ITEMS.get(id as usize - 1).filter(|it| it.id == id)
}

/// The lock a meal gives (seconds).
pub fn meal_lock(it: &Item) -> f32 {
    if it.heal >= 20 {
        1.2
    } else {
        1.0
    }
}

/// The damage kinds defence is against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dmg {
    Blade = 0,
    Bullet = 1,
    Blast = 2,
}

impl Dmg {
    /// What a kill-feed weapon id is (None: things gear doesn't touch, like an execution).
    pub fn of(w: u8) -> Option<Dmg> {
        use crate::game::{GRENADE_TOOL, KICK_TOOL, N_WEAPONS};
        if w < 3 || w == KICK_TOOL {
            Some(Dmg::Blade)
        } else if w == Weapon::Rocket as u8 || w == GRENADE_TOOL {
            Some(Dmg::Blast)
        } else if w < N_WEAPONS {
            Some(Dmg::Bullet)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------- rules

/// The duel's rules (set by the challenger, the host).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    pub no_guns: bool,
    pub no_blades: bool,
    pub no_food: bool,
    pub no_specs: bool,
    pub no_move: bool,
    pub no_switch: bool,
    pub no_sets: bool,
    /// 1, 3 or 5.
    pub best_of: u8,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { no_guns: false, no_blades: false, no_food: false, no_specs: false, no_move: false, no_switch: false, no_sets: false, best_of: 3 }
    }
}

impl Rules {
    pub fn bits(&self) -> u8 {
        self.no_guns as u8 | (self.no_blades as u8) << 1 | (self.no_food as u8) << 2 | (self.no_specs as u8) << 3 | (self.no_move as u8) << 4 | (self.no_switch as u8) << 5 | (self.no_sets as u8) << 6
    }
    pub fn from_bits(b: u8, best_of: u8) -> Rules {
        let mut r = Rules {
            no_guns: b & 1 != 0,
            no_blades: b & 2 != 0,
            no_food: b & 4 != 0,
            no_specs: b & 8 != 0,
            no_move: b & 16 != 0,
            no_switch: b & 32 != 0,
            no_sets: b & 64 != 0,
            best_of: if matches!(best_of, 1 | 3 | 5) { best_of } else { 3 },
        };
        if r.no_guns && r.no_blades {
            r.no_blades = false; // something has to be allowed in the hand
        }
        r
    }
    /// Round wins to take the duel.
    pub fn target(&self) -> u16 {
        (self.best_of as u16 + 1) / 2
    }
    /// The rules as players read them (empty: standard rules).
    pub fn lines(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.no_guns {
            v.push("No guns");
        }
        if self.no_blades {
            v.push("No blades");
        }
        if self.no_food {
            v.push("No food or tonics");
        }
        if self.no_specs {
            v.push("No special attacks");
        }
        if self.no_move {
            v.push("No movement (stand and trade)");
        }
        if self.no_switch {
            v.push("No gear switching");
        }
        if self.no_sets {
            v.push("No set bonuses");
        }
        v
    }
    pub fn summary(&self) -> String {
        let l = self.lines();
        let rules = if l.is_empty() { "standard rules".to_string() } else { l.join(", ").to_lowercase() };
        format!("Best of {}: {rules}", self.best_of)
    }
    /// May this item be used under these rules?
    pub fn allows(&self, it: &Item) -> bool {
        match it.kind {
            Kind::Gear(Slot::Main) => match it.base {
                Some(w) if w.is_melee() => !self.no_blades,
                Some(_) => !self.no_guns,
                None => true,
            },
            Kind::Food | Kind::Bite | Kind::Tonic(_) => !self.no_food,
            _ => true,
        }
    }
}

// ---------------------------------------------------------------- loadouts

/// Dyes for gear with dye zones (the coat, its trim, an accent): name and colour (linear).
pub const DYES: [(&str, [f32; 3]); 12] = [
    ("Oiled Brown", [0.20, 0.11, 0.05]),
    ("Brass", [0.62, 0.42, 0.14]),
    ("Teal", [0.03, 0.42, 0.40]),
    ("Soot Black", [0.03, 0.03, 0.035]),
    ("Bridge Grey", [0.22, 0.23, 0.25]),
    ("Rust Red", [0.40, 0.07, 0.04]),
    ("Moss Green", [0.10, 0.20, 0.06]),
    ("Court Blue", [0.05, 0.10, 0.35]),
    ("Copper", [0.55, 0.22, 0.08]),
    ("Silver", [0.60, 0.62, 0.66]),
    ("Ember Orange", [0.80, 0.30, 0.05]),
    ("Ivory", [0.75, 0.70, 0.58]),
];
/// The dye zones, and the Rig's own colours: oiled brown canvas, brass trim, a teal accent.
pub const DYE_ZONES: [&str; 3] = ["Coat", "Trim", "Accent"];
pub const DEFAULT_DYES: [u8; 3] = [0, 1, 2];

pub fn dye(k: u8) -> [f32; 3] {
    DYES[(k as usize).min(DYES.len() - 1)].1
}

/// A named loadout: what's worn and what's carried, and the dyes on its gear.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GearLoadout {
    pub name: String,
    pub equip: [u8; SLOTS],
    pub inv: [u8; INV],
    pub dyes: [u8; 3],
}

impl Default for GearLoadout {
    fn default() -> Self {
        presets().remove(4)
    }
}

impl GearLoadout {
    pub fn new(name: &str, equip: [u8; SLOTS], inv: [u8; INV]) -> GearLoadout {
        GearLoadout { name: name.to_string(), equip, inv, dyes: DEFAULT_DYES }
    }
    pub fn main(&self) -> Option<&'static Item> {
        item(self.equip[Slot::Main as usize])
    }
    /// One line of text (for saving): name|equip ids|inventory ids.
    pub fn to_text(&self) -> String {
        let l = |v: &[u8]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
        format!("{}|{}|{}|{}", self.name.replace(['|', '\n', '\r'], " "), l(&self.equip), l(&self.inv), l(&self.dyes))
    }
    pub fn from_text(s: &str) -> Option<GearLoadout> {
        let mut parts = s.split('|');
        let name = parts.next()?.trim().chars().take(24).collect::<String>();
        let read = |p: Option<&str>, n: usize| -> Vec<u8> {
            let mut v: Vec<u8> = p.unwrap_or("").split(',').map(|x| x.trim().parse().unwrap_or(0)).collect();
            v.resize(n, 0);
            v
        };
        let e = read(parts.next(), SLOTS);
        let i = read(parts.next(), INV);
        let d = parts.next().map(|p| read(Some(p), 3));
        let mut g = GearLoadout { name: if name.is_empty() { "Loadout".into() } else { name }, equip: [0; SLOTS], inv: [0; INV], dyes: DEFAULT_DYES };
        g.equip.copy_from_slice(&e[..SLOTS]);
        g.inv.copy_from_slice(&i[..INV]);
        if let Some(d) = d {
            for k in 0..3 {
                g.dyes[k] = d[k].min(DYES.len() as u8 - 1);
            }
        }
        g.fix();
        Some(g)
    }
    /// Unknown items, items in the wrong slot, and an off hand with a two-handed weapon are
    /// taken out (to the pack when there's room); something is always in hand.
    pub fn fix(&mut self) {
        for (k, id) in self.equip.iter_mut().enumerate() {
            if item(*id).map_or(true, |it| it.kind != Kind::Gear(Slot::ALL[k])) {
                *id = 0;
            }
        }
        for id in self.inv.iter_mut() {
            if item(*id).is_none() {
                *id = 0;
            }
        }
        if self.main().is_some_and(|m| m.two_handed) && self.equip[Slot::Off as usize] != 0 {
            let off = std::mem::take(&mut self.equip[Slot::Off as usize]);
            if let Some(free) = self.inv.iter_mut().find(|x| **x == 0) {
                *free = off;
            }
        }
        if self.equip[Slot::Main as usize] == 0 {
            self.equip[Slot::Main as usize] = 24; // a Courtly Sabre: everyone has something in hand
        }
    }
    /// From the armoury: wear `id` in its slot (a two-handed weapon sends the off hand to the
    /// pack, or off if the pack is full); an off hand with a two-handed weapon is refused.
    pub fn wear(&mut self, id: u8) -> Result<(), &'static str> {
        let it = item(id).ok_or("No such item")?;
        let Kind::Gear(slot) = it.kind else { return Err("That goes in the pack") };
        if slot == Slot::Off && self.main().is_some_and(|m| m.two_handed) {
            return Err("Both hands are full");
        }
        self.equip[slot as usize] = id;
        if slot == Slot::Main && it.two_handed && self.equip[Slot::Off as usize] != 0 {
            let off = std::mem::take(&mut self.equip[Slot::Off as usize]);
            let _ = self.pack_add(off);
        }
        Ok(())
    }
    /// Take off what's worn in `slot` (into the pack when there's room); the main hand stays.
    pub fn take_off(&mut self, slot: Slot) {
        if slot == Slot::Main {
            return;
        }
        let id = std::mem::take(&mut self.equip[slot as usize]);
        if id != 0 {
            let _ = self.pack_add(id);
        }
    }
    /// Put `id` in the first free pack slot.
    pub fn pack_add(&mut self, id: u8) -> Result<usize, &'static str> {
        item(id).ok_or("No such item")?;
        let k = self.inv.iter().position(|&x| x == 0).ok_or("The pack is full")?;
        self.inv[k] = id;
        Ok(k)
    }
    /// Wear the gear in pack slot `k`; what it replaces goes into that slot.
    pub fn pack_wear(&mut self, k: usize) -> Result<(), &'static str> {
        let id = *self.inv.get(k).ok_or("No such slot")?;
        let it = item(id).ok_or("Empty slot")?;
        let Kind::Gear(slot) = it.kind else { return Err("Food is eaten in the duel") };
        if slot == Slot::Off && self.main().is_some_and(|m| m.two_handed) {
            return Err("Both hands are full");
        }
        self.inv[k] = self.equip[slot as usize];
        self.equip[slot as usize] = id;
        if slot == Slot::Main && it.two_handed && self.equip[Slot::Off as usize] != 0 {
            let off = std::mem::take(&mut self.equip[Slot::Off as usize]);
            let _ = self.pack_add(off);
        }
        Ok(())
    }

    /// This loadout fitted to the rules: a main hand they forbid is swapped for one they allow
    /// (from the pack if it carries one, else a default).
    pub fn for_rules(&self, r: &Rules) -> GearLoadout {
        let mut g = self.clone();
        if !g.main().is_some_and(|m| r.allows(m)) {
            let pick = g.inv.iter().position(|&id| item(id).is_some_and(|it| it.kind == Kind::Gear(Slot::Main) && r.allows(it)));
            let old = g.equip[Slot::Main as usize];
            match pick {
                Some(k) => {
                    g.equip[Slot::Main as usize] = g.inv[k];
                    g.inv[k] = old;
                }
                None => g.equip[Slot::Main as usize] = if r.no_blades { 27 } else { 24 },
            }
            g.fix();
        }
        g
    }
}

/// The set all four armour pieces (head, body, legs, hands) belong to.
pub fn full_set(equip: &[u8; SLOTS]) -> Option<Set> {
    let first = item(equip[0])?.set?;
    (1..4).all(|k| item(equip[k]).and_then(|it| it.set) == Some(first)).then_some(first)
}

/// The loadouts everyone starts with (and the bots' choices): one per set and a few mixed.
pub fn presets() -> Vec<GearLoadout> {
    vec![
        // a heavy blade that grows deadlier as you bleed; a revolver for when they back off
        GearLoadout::new("Gravewarden Headsman", [1, 2, 3, 4, 22, 25, 0, 39], [27, 45, 45, 46, 46, 47, 0, 0, 0, 0, 0, 0]),
        // sabre and buckler behind a wall of iron; the cleaver for the finish
        GearLoadout::new("Bastion Wall", [5, 6, 7, 8, 22, 24, 35, 42], [25, 27, 44, 46, 46, 48, 0, 0, 0, 0, 0, 0]),
        // daggers and speed; stars to chase
        GearLoadout::new("Swiftshade Fangs", [9, 10, 11, 12, 21, 26, 0, 40], [34, 44, 44, 46, 46, 49, 0, 0, 0, 0, 0, 0]),
        // a rifle from range; the sabre if they close in
        GearLoadout::new("Longsight Marksman", [13, 14, 15, 16, 21, 29, 0, 41], [24, 44, 44, 46, 46, 50, 0, 0, 0, 0, 0, 0]),
        // a balanced Court duellist: sabre and hook, the revolver, the cleaver for a finish
        GearLoadout::new("Court Duellist", [17, 18, 19, 20, 21, 24, 36, 39], [27, 25, 45, 45, 46, 47, 0, 0, 0, 0, 0, 0]),
        // a slag gun up close, the cannon for range
        GearLoadout::new("Foundry Brawler", [17, 18, 19, 20, 23, 30, 0, 43], [28, 45, 45, 46, 46, 48, 0, 0, 0, 0, 0, 0]),
        // a Lower Wards rattler with a clerk's ledger
        GearLoadout::new("Lowtown Rattler", [9, 10, 11, 12, 21, 31, 38, 39], [26, 44, 44, 46, 46, 47, 0, 0, 0, 0, 0, 0]),
        // the long gun and a cannon
        GearLoadout::new("Border Warden", [13, 14, 15, 16, 22, 32, 0, 41], [28, 44, 44, 46, 46, 48, 0, 0, 0, 0, 0, 0]),
        // the crane-rigger: hook and snub pistol, haul them in, swap to the cleaver for the finish
        GearLoadout::new("Bridgewright Rigger", [51, 52, 53, 54, 55, 57, 58, 56], [25, 27, 44, 46, 46, 47, 0, 0, 0, 0, 0, 0]),
    ]
}

// ---------------------------------------------------------------- stats

/// What a fighter's gear adds up to.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub acc: f32,
    pub dmg: f32,
    pub def: [f32; 3],
    pub speed: f32,
    pub regen: f32,
    pub weight: f32,
    pub set: Option<Set>,
}

impl Stats {
    /// Move speed: the items' speed, less 0.5% for every kilogram over 15.
    pub fn move_mult(&self) -> f32 {
        let pct = self.speed - 0.5 * (self.weight - 15.0).max(0.0);
        (1.0 + pct / 100.0).clamp(0.75, 1.2)
    }
}

/// The gear's totals (a set's bonus included unless `no_sets`), before tonics.
pub fn stats(equip: &[u8; SLOTS], no_sets: bool) -> Stats {
    let mut s = Stats::default();
    for it in equip.iter().filter_map(|&id| item(id)) {
        s.acc += it.acc as f32;
        s.dmg += it.dmg as f32;
        for k in 0..3 {
            s.def[k] += it.def[k] as f32;
        }
        s.speed += it.speed as f32;
        s.regen += it.regen as f32;
        s.weight += it.weight as f32;
    }
    if !no_sets {
        s.set = full_set(equip);
        let gun = item(equip[Slot::Main as usize]).and_then(|m| m.base).is_some_and(|w| !w.is_melee());
        match s.set {
            Some(Set::Bastion) => {
                for d in s.def.iter_mut() {
                    *d += 5.0;
                }
                s.dmg -= 25.0;
            }
            Some(Set::Swiftshade) => s.speed += 12.0,
            Some(Set::Longsight) if gun => {
                s.acc += 15.0;
                s.dmg += 5.0;
            }
            Some(Set::Longsight) => s.dmg -= 12.0,
            _ => {}
        }
    }
    for d in s.def.iter_mut() {
        *d = d.clamp(0.0, DEF_CAP);
    }
    s
}

/// Gravewarden's bonus at this much health (percent damage).
pub fn grave_bonus(hp: f32) -> f32 {
    30.0 * (1.0 - (hp / 100.0).clamp(0.0, 1.0))
}

/// How long equipping an item takes (0.3-0.6 s by its weight; Swiftshade 0.1 s quicker, the
/// Bridgewright's Rig 30% quicker).
pub fn swap_time(it: &Item, set: Option<Set>) -> f32 {
    let t = (0.3 + 0.0375 * it.weight as f32).clamp(0.3, 0.6);
    match set {
        Some(Set::Swiftshade) => (t - 0.1).max(0.25),
        Some(Set::Bridgewright) => t * TOOLS_SWAP,
        _ => t,
    }
}

// ---------------------------------------------------------------- a fighter's gear in a duel

/// Bits of a fighter's state others see (sent with the fighter).
pub const B_WIND: u8 = 1;
pub const B_STRIKE: u8 = 2;
pub const B_EAT: u8 = 4;
pub const B_SWAP: u8 = 8;
pub const B_FURY: u8 = 16;
pub const B_IRON: u8 = 32;
pub const B_HASTE: u8 = 64;

/// A fighter's gear during a duel.
#[derive(Clone, Debug, PartialEq)]
pub struct Kit {
    /// What they started the round with (a new round starts from it again).
    pub base: GearLoadout,
    pub equip: [u8; SLOTS],
    pub inv: [u8; INV],
    /// The special bar (0-100).
    pub spec: f32,
    /// An item being equipped: pack slot, time left, its full time.
    pub swap: Option<(usize, f32, f32)>,
    /// Eating: lock left, and whether a bite or tonic already followed this meal.
    pub eat_t: f32,
    pub combo_used: bool,
    /// A special winding up: which, time left.
    pub wind: Option<(Spec, f32)>,
    /// A strike special ready for the next attack: which, time left.
    pub armed: Option<(Spec, f32)>,
    /// Hits carry this special for this long.
    pub strike: Option<(Spec, f32)>,
    /// Tonic effects: time left (Fury, Iron, Haste).
    pub buffs: [f32; 3],
    /// Magazines of guns swapped out (item id, rounds).
    pub mags: Vec<(u8, u8)>,
    /// A copy shown for someone on another computer (only what's sent is right).
    pub remote: bool,
    /// What they last tried that didn't work (for the HUD), and how long it shows.
    pub note: Option<(&'static str, f32)>,
    /// The duel's "no sets" rule (for what the fighter works out alone: accuracy and reach).
    pub no_sets: bool,
    /// The gear's dyes (coat, trim, accent).
    pub dyes: [u8; 3],
    /// Change of Tools: the last hit landed (a blade's?, when) and time until it can pay again.
    pub tools_last: Option<(bool, f32)>,
    pub tools_cd: f32,
    /// The off-hand pistol: rounds, time to the next shot, reload left.
    pub snub: u8,
    pub snub_cd: f32,
    pub snub_reload: f32,
}

impl Kit {
    pub fn new(base: &GearLoadout) -> Kit {
        Kit {
            base: base.clone(),
            equip: base.equip,
            inv: base.inv,
            spec: 100.0,
            swap: None,
            eat_t: 0.0,
            combo_used: false,
            wind: None,
            armed: None,
            strike: None,
            buffs: [0.0; 3],
            mags: Vec::new(),
            remote: false,
            note: None,
            no_sets: false,
            dyes: base.dyes,
            tools_last: None,
            tools_cd: 0.0,
            snub: SNUB_ROUNDS,
            snub_cd: 0.0,
            snub_reload: 0.0,
        }
    }
    /// A hit landed with a blade (or not): the Rig's Change of Tools pays +10 special bar for a
    /// blade hit and a gun hit within 3 s (at most once every 6 s). True when it paid.
    pub fn landed(&mut self, blade: bool, now: f32, rules: &Rules) -> bool {
        if self.stats(rules).set != Some(Set::Bridgewright) {
            return false;
        }
        let paid = match self.tools_last {
            Some((b, t)) if b != blade && now - t <= TOOLS_WINDOW && self.tools_cd <= 0.0 => {
                self.spec = (self.spec + TOOLS_SPEC).min(100.0);
                self.tools_cd = TOOLS_COOLDOWN;
                true
            }
            _ => false,
        };
        self.tools_last = Some((blade, now));
        paid
    }
    /// The Rig's charm glows: it's worn and the set's bonus is on.
    pub fn charm_glows(&self) -> bool {
        self.equip[Slot::Charm as usize] == GEAR_CHARM && stats(&self.equip, self.no_sets).set == Some(Set::Bridgewright)
    }
    /// The off-hand pistol can fire now (worn with a one-handed blade, loaded, not busy).
    pub fn snub_ready(&self) -> bool {
        self.equip[Slot::Off as usize] == SNUB && self.main().is_some_and(|m| !m.two_handed && m.base.is_some_and(|b| b.is_melee())) && !self.busy() && self.snub > 0 && self.snub_cd <= 0.0 && self.snub_reload <= 0.0
    }
    pub fn fire_snub(&mut self) {
        self.snub = self.snub.saturating_sub(1);
        self.snub_cd = SNUB_GAP;
        if self.snub == 0 {
            self.snub_reload = SNUB_RELOAD;
        }
    }
    /// Spread times this (better accuracy: tighter; worse: wider).
    pub fn spread_mult(&self) -> f32 {
        (1.0 - stats(&self.equip, self.no_sets).acc / 100.0).clamp(0.6, 1.15)
    }
    /// A blade's reach times this (accuracy lengthens it a little: +1% per 2% accuracy, at most 10%).
    pub fn reach_mult(&self) -> f32 {
        (1.0 + stats(&self.equip, self.no_sets).acc.max(0.0) / 200.0).min(1.1)
    }
    /// A copy for a fighter on another computer, from what it sends.
    pub fn remote(equip: [u8; SLOTS], spec: u8, bits: u8) -> Kit {
        let mut k = Kit::new(&GearLoadout { name: String::new(), equip, inv: [0; INV], dyes: DEFAULT_DYES });
        k.remote = true;
        k.set_remote(equip, spec, bits);
        k
    }
    pub fn set_remote(&mut self, equip: [u8; SLOTS], spec: u8, bits: u8) {
        self.equip = equip;
        self.spec = spec.min(100) as f32;
        let s = self.main().and_then(|m| m.spec);
        self.wind = if bits & B_WIND != 0 { s.map(|s| (s, self.wind.map_or(s.info().windup, |w| w.1))) } else { None };
        self.strike = if bits & B_STRIKE != 0 { s.map(|s| (s, 0.2)) } else { None };
        self.eat_t = if bits & B_EAT != 0 { 0.3 } else { 0.0 };
        self.swap = if bits & B_SWAP != 0 { Some((0, 0.2, 0.4)) } else { None };
        self.buffs = [if bits & B_FURY != 0 { 1.0 } else { 0.0 }, if bits & B_IRON != 0 { 1.0 } else { 0.0 }, if bits & B_HASTE != 0 { 1.0 } else { 0.0 }];
    }
    /// The state bits others see.
    pub fn bits(&self) -> u8 {
        let mut b = 0;
        if self.wind.is_some() {
            b |= B_WIND;
        }
        if self.strike.is_some() || self.armed.is_some() {
            b |= B_STRIKE;
        }
        if self.eat_t > 0.0 {
            b |= B_EAT;
        }
        if self.swap.is_some() {
            b |= B_SWAP;
        }
        if self.buffs[0] > 0.0 {
            b |= B_FURY;
        }
        if self.buffs[1] > 0.0 {
            b |= B_IRON;
        }
        if self.buffs[2] > 0.0 {
            b |= B_HASTE;
        }
        b
    }
    pub fn main(&self) -> Option<&'static Item> {
        item(self.equip[Slot::Main as usize])
    }
    /// The weapon in hand.
    pub fn weapon(&self) -> Weapon {
        self.main().and_then(|m| m.base).unwrap_or(Weapon::Katana)
    }
    /// What's being equipped (for the HUD and the tell over their head).
    pub fn swapping_to(&self) -> Option<&'static Item> {
        self.swap.and_then(|(k, _, _)| self.inv.get(k).and_then(|&id| item(id)))
    }
    pub fn stats(&self, rules: &Rules) -> Stats {
        let mut s = stats(&self.equip, rules.no_sets);
        if self.buffs[1] > 0.0 {
            for d in s.def.iter_mut() {
                *d = (*d + 10.0).min(DEF_CAP);
            }
        }
        if self.buffs[2] > 0.0 {
            s.speed += 8.0;
        }
        if self.buffs[0] > 0.0 {
            s.dmg += 12.0;
        }
        s
    }
    /// Busy: winding up a special, eating or swapping (no attacking).
    pub fn busy(&self) -> bool {
        self.wind.is_some() || self.eat_t > 0.0 || self.swap.is_some()
    }
    /// Damage multiplier for a hit with weapon id `w` (gear, set, tonic; the special separately).
    pub fn attack_mult(&self, rules: &Rules, hp: f32, w: u8) -> f32 {
        let s = self.stats(rules);
        let mut pct = s.dmg;
        // the weapon's own damage only counts for its own hits (a kick is just a kick)
        if let Some(m) = self.main() {
            if m.base.map(|b| b as u8) != Some(w) {
                pct -= m.dmg as f32;
            }
        }
        if s.set == Some(Set::Gravewarden) {
            pct += grave_bonus(hp);
        }
        (1.0 + pct / 100.0).max(0.5)
    }
    /// Share of a hit of this kind that gets through (after `pierce` of the defence is ignored).
    pub fn defend(&self, rules: &Rules, kind: Dmg, pierce: f32) -> f32 {
        let d = self.stats(rules).def[kind as usize] * (1.0 - pierce.clamp(0.0, 1.0));
        1.0 - d / 100.0
    }
    /// The special carried by a hit landing now (if any).
    pub fn hit_spec(&self) -> Option<Spec> {
        self.strike.map(|s| s.0)
    }
    /// Can a special start now?
    pub fn spec_ready(&self, rules: &Rules) -> Result<Spec, &'static str> {
        if rules.no_specs {
            return Err("Special attacks are off in this duel");
        }
        let s = self.main().and_then(|m| m.spec).ok_or("No special for this weapon")?;
        if self.busy() || self.armed.is_some() || self.strike.is_some() {
            return Err("Busy");
        }
        if self.spec < s.info().cost - 0.01 {
            return Err("Not enough special bar");
        }
        Ok(s)
    }
    /// Start a special: the bar pays, the wind-up begins.
    pub fn start_spec(&mut self, rules: &Rules) -> Result<Spec, &'static str> {
        let s = self.spec_ready(rules)?;
        self.spec -= s.info().cost;
        self.wind = Some((s, s.info().windup));
        Ok(s)
    }
    /// What using pack slot `k` would do (equip a piece of gear, or eat), or why it can't.
    pub fn can_use(&self, k: usize, rules: &Rules) -> Result<&'static Item, &'static str> {
        let it = self.inv.get(k).and_then(|&id| item(id)).ok_or("Empty slot")?;
        if !rules.allows(it) {
            return Err(match it.kind {
                Kind::Gear(_) => "Not allowed in this duel",
                _ => "No food in this duel",
            });
        }
        match it.kind {
            Kind::Gear(slot) => {
                if rules.no_switch {
                    return Err("Gear switching is off in this duel");
                }
                if self.swap.is_some() || self.wind.is_some() || self.eat_t > 0.0 {
                    return Err("Busy");
                }
                if slot == Slot::Off && self.main().is_some_and(|m| m.two_handed) {
                    return Err("Both hands are full");
                }
                if slot == Slot::Main && it.two_handed && self.equip[Slot::Off as usize] != 0 && !self.inv.iter().enumerate().any(|(j, &x)| x == 0 && j != k) {
                    return Err("No room in the pack for the off hand");
                }
                Ok(it)
            }
            Kind::Food => {
                if self.eat_t > 0.0 || self.wind.is_some() || self.swap.is_some() {
                    return Err("Still eating");
                }
                Ok(it)
            }
            Kind::Bite | Kind::Tonic(_) => {
                if (self.eat_t > 0.0 && self.combo_used) || self.wind.is_some() || self.swap.is_some() {
                    return Err("Still eating");
                }
                Ok(it)
            }
        }
    }
    /// Begin equipping pack slot `k` (it goes on when the swap time has passed).
    pub fn begin_swap(&mut self, k: usize, rules: &Rules) -> Result<f32, &'static str> {
        let it = self.can_use(k, rules)?;
        if !matches!(it.kind, Kind::Gear(_)) {
            return Err("Not gear");
        }
        let t = swap_time(it, self.stats(rules).set);
        self.swap = Some((k, t, t));
        self.armed = None;
        Ok(t)
    }
    /// The swap is done: the item goes on, what it replaces goes into its slot of the pack.
    pub fn finish_swap(&mut self, k: usize) {
        let Some(it) = self.inv.get(k).and_then(|&id| item(id)) else { return };
        let Kind::Gear(slot) = it.kind else { return };
        let s = slot as usize;
        let old = self.equip[s];
        self.equip[s] = it.id;
        self.inv[k] = old;
        if slot == Slot::Main && it.two_handed && self.equip[Slot::Off as usize] != 0 {
            let off = std::mem::take(&mut self.equip[Slot::Off as usize]);
            if let Some(free) = self.inv.iter_mut().find(|x| **x == 0) {
                *free = off;
            }
        }
    }
    /// Eat or drink pack slot `k`: returns the health it gives back (the caller heals).
    pub fn eat(&mut self, k: usize, rules: &Rules) -> Result<(f32, &'static Item), &'static str> {
        let it = self.can_use(k, rules)?;
        match it.kind {
            Kind::Food => {
                self.eat_t = meal_lock(it);
                self.combo_used = false;
            }
            Kind::Bite | Kind::Tonic(_) => {
                self.eat_t = if self.eat_t > 0.0 { self.eat_t + COMBO_EXTRA } else { BITE_LOCK };
                self.combo_used = true;
                if let Kind::Tonic(t) = it.kind {
                    match t {
                        Tonic::Clarity => self.spec = (self.spec + 20.0).min(100.0),
                        _ => self.buffs[t as usize] = t.secs(),
                    }
                }
            }
            Kind::Gear(_) => return Err("Not food"),
        }
        self.inv[k] = 0;
        self.armed = None;
        Ok((it.heal as f32, it))
    }
    /// Time passes: the bar refills, locks and effects run down. Returns a swap that just finished
    /// (its pack slot) and a special whose wind-up just ended.
    pub fn tick(&mut self, dt: f32, rules: &Rules) -> (Option<usize>, Option<Spec>) {
        if !self.remote {
            let regen = SPEC_REGEN * (1.0 + self.stats(rules).regen / 100.0);
            self.spec = (self.spec + regen * dt).min(100.0);
        }
        self.eat_t = (self.eat_t - dt).max(0.0);
        if self.eat_t <= 0.0 {
            self.combo_used = false;
        }
        for b in self.buffs.iter_mut() {
            *b = (*b - dt).max(0.0);
        }
        self.tools_cd = (self.tools_cd - dt).max(0.0);
        self.snub_cd = (self.snub_cd - dt).max(0.0);
        if self.snub_reload > 0.0 {
            self.snub_reload -= dt;
            if self.snub_reload <= 0.0 {
                self.snub_reload = 0.0;
                self.snub = SNUB_ROUNDS;
            }
        }
        if let Some((_, t)) = &mut self.note {
            *t -= dt;
        }
        if self.note.is_some_and(|n| n.1 <= 0.0) {
            self.note = None;
        }
        let mut swapped = None;
        if let Some((k, t, full)) = self.swap {
            let t = t - dt;
            if t <= 0.0 {
                self.swap = None;
                if !self.remote {
                    self.finish_swap(k);
                }
                swapped = Some(k);
            } else {
                self.swap = Some((k, t, full));
            }
        }
        let mut fired = None;
        if let Some((s, t)) = self.wind {
            let t = t - dt;
            if t <= 0.0 {
                self.wind = None;
                fired = Some(s);
                if !self.remote {
                    if s.info().timed > 0.0 {
                        self.strike = Some((s, s.info().timed));
                    } else {
                        self.armed = Some((s, ARMED_TIME));
                    }
                }
            } else {
                self.wind = Some((s, t));
            }
        }
        if let Some((s, t)) = self.armed {
            self.armed = if t - dt > 0.0 { Some((s, t - dt)) } else { None };
        }
        if let Some((s, t)) = self.strike {
            self.strike = if t - dt > 0.0 { Some((s, t - dt)) } else { None };
        }
        (swapped, fired)
    }
    /// The attack a strike special was waiting for just happened: its hits carry it for a moment.
    pub fn attacked(&mut self, melee: bool) {
        if let Some((s, _)) = self.armed.take() {
            self.strike = Some((s, if melee { STRIKE_MELEE } else { STRIKE_GUN }));
        }
    }
    pub fn say(&mut self, what: &'static str) {
        self.note = Some((what, 1.5));
    }
}

// ---------------------------------------------------------------- saved loadouts and honour

/// One loadout's record in the honour ledger (no stakes, only the record).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Honour {
    pub wins: u32,
    pub losses: u32,
    pub streak: u32,
    pub best: u32,
}

/// What's kept on this computer for Ledger Duels: five named loadouts, the one picked, the rules
/// last set as challenger, and the honour ledger (by loadout name).
#[derive(Clone, Debug, PartialEq)]
pub struct LedgerSave {
    pub kits: Vec<GearLoadout>,
    pub pick: usize,
    pub rules: Rules,
    pub honour: Vec<(String, Honour)>,
}

impl Default for LedgerSave {
    fn default() -> Self {
        let p = presets();
        LedgerSave { kits: vec![p[4].clone(), p[0].clone(), p[1].clone(), p[2].clone(), p[3].clone()], pick: 0, rules: Rules::default(), honour: Vec::new() }
    }
}

impl LedgerSave {
    pub fn path() -> Option<std::path::PathBuf> {
        crate::platform::data_file("ledger.txt")
    }
    pub fn load() -> LedgerSave {
        Self::path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| LedgerSave::from_text(&t)).unwrap_or_default()
    }
    pub fn save(&self) {
        if let Some(p) = Self::path() {
            let _ = crate::platform::save_file(&p, &self.to_text());
        }
    }
    pub fn current(&self) -> &GearLoadout {
        &self.kits[self.pick.min(self.kits.len() - 1)]
    }
    pub fn to_text(&self) -> String {
        let mut s = format!("pick={}\nrules={},{}\n", self.pick, self.rules.bits(), self.rules.best_of);
        for k in &self.kits {
            s += &format!("kit={}\n", k.to_text());
        }
        for (name, h) in &self.honour {
            s += &format!("honour={}|{}|{}|{}|{}\n", name.replace(['|', '\n'], " "), h.wins, h.losses, h.streak, h.best);
        }
        s
    }
    pub fn from_text(t: &str) -> LedgerSave {
        let mut out = LedgerSave { kits: Vec::new(), ..Default::default() };
        for line in t.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "pick" => out.pick = v.trim().parse().unwrap_or(0),
                "rules" => {
                    let mut it = v.split(',').filter_map(|x| x.trim().parse::<u8>().ok());
                    out.rules = Rules::from_bits(it.next().unwrap_or(0), it.next().unwrap_or(3));
                }
                "kit" => {
                    if let Some(g) = GearLoadout::from_text(v) {
                        if out.kits.len() < SAVED {
                            out.kits.push(g);
                        }
                    }
                }
                "honour" => {
                    let p: Vec<&str> = v.split('|').collect();
                    if p.len() >= 5 {
                        let n = |s: &str| s.trim().parse().unwrap_or(0);
                        out.honour.push((p[0].to_string(), Honour { wins: n(p[1]), losses: n(p[2]), streak: n(p[3]), best: n(p[4]) }));
                    }
                }
                _ => {}
            }
        }
        let d = LedgerSave::default();
        while out.kits.len() < SAVED {
            out.kits.push(d.kits[out.kits.len()].clone());
        }
        out.pick = out.pick.min(SAVED - 1);
        out
    }
    pub fn honour_of(&self, name: &str) -> Honour {
        self.honour.iter().find(|(n, _)| n == name).map(|(_, h)| h.clone()).unwrap_or_default()
    }
    /// A duel won or lost with the loadout called `name`.
    pub fn record(&mut self, name: &str, won: bool) {
        if !self.honour.iter().any(|(n, _)| n == name) {
            self.honour.push((name.to_string(), Honour::default()));
        }
        if let Some((_, h)) = self.honour.iter_mut().find(|(n, _)| n == name) {
            if won {
                h.wins += 1;
                h.streak += 1;
                h.best = h.best.max(h.streak);
            } else {
                h.losses += 1;
                h.streak = 0;
            }
        }
    }
}

/// A cosmetic variant of an item card (its frame and name), opened by the fighter's mastery:
/// purely a look, never a stat.
pub fn variant(mastery: u32) -> (&'static str, [u8; 3]) {
    match mastery {
        0..=2 => ("Court", [150, 140, 128]),
        3..=5 => ("Bronze", [196, 128, 74]),
        6..=9 => ("Silvered", [200, 208, 220]),
        10..=14 => ("Gilded", [255, 200, 70]),
        _ => ("Ashen", [255, 110, 90]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every item says what it is and does; names are unique and our own; every weapon has a
    /// special costing 25, 50 or 100; each set has its four pieces.
    #[test]
    fn the_catalogue_is_complete_and_original() {
        let mut names = std::collections::HashSet::new();
        for (k, it) in ITEMS.iter().enumerate() {
            assert_eq!(it.id as usize, k + 1, "ids run in order");
            assert!(item(it.id).is_some());
            assert!(names.insert(it.name), "{} twice", it.name);
            assert!(!it.blurb.is_empty());
            match it.kind {
                Kind::Gear(Slot::Main) => {
                    assert!(it.base.is_some() && it.spec.is_some(), "{}: a weapon kind and a special", it.name);
                    let c = it.spec.unwrap().info().cost;
                    assert!([25.0, 50.0, 100.0].contains(&c), "{}: costs {c}", it.name);
                }
                Kind::Food | Kind::Bite => assert!(it.heal > 0, "{} heals", it.name),
                _ => {}
            }
        }
        // words from classic MMO duel arenas aren't used
        for w in ["dragon", "abyssal", "whip", "godsword", "granite", "rune", "karambwan", "shark", "barrows", "dharok", "stake", "brew"] {
            assert!(ITEMS.iter().all(|it| !it.name.to_lowercase().contains(w)), "{w}");
        }
        for set in Set::ALL {
            for slot in [Slot::Head, Slot::Body, Slot::Legs, Slot::Hands] {
                assert_eq!(ITEMS.iter().filter(|it| it.set == Some(set) && it.kind == Kind::Gear(slot)).count(), 1, "{:?} {:?}", set, slot);
            }
        }
        for s in Spec::ALL {
            let i = s.info();
            assert!(i.windup >= 0.3 && i.windup <= 0.6, "{}: a readable wind-up", i.name);
            assert!(ITEMS.iter().any(|it| it.spec == Some(s)), "{} belongs to a weapon", i.name);
            assert_eq!(Spec::from_u8(s as u8), Some(s));
        }
        for p in presets() {
            let mut q = p.clone();
            q.fix();
            assert_eq!(q, p, "{}: a valid loadout", p.name);
        }
    }

    /// Stats add up; a full set gives its bonus (unless sets are off); weight slows you.
    #[test]
    fn stats_sets_and_weight() {
        let p = presets();
        let grave = stats(&p[0].equip, false);
        assert_eq!(grave.set, Some(Set::Gravewarden));
        assert!(grave.def[0] > grave.def[1], "strong against blades, weak against bullets");
        assert!(grave.move_mult() < 1.0, "heavy");
        assert_eq!(stats(&p[0].equip, true).set, None, "no sets: no bonus");
        let bastion = stats(&p[1].equip, false);
        assert!(bastion.dmg < 0.0 && bastion.def.iter().all(|&d| d >= 30.0), "{bastion:?}");
        let swift = stats(&p[2].equip, false);
        assert!(swift.move_mult() > 1.1, "{}", swift.move_mult());
        assert!(swap_time(item(25).unwrap(), Some(Set::Swiftshade)) < swap_time(item(25).unwrap(), None));
        for it in ITEMS.iter().filter(|it| matches!(it.kind, Kind::Gear(_))) {
            let t = swap_time(it, None);
            assert!((0.3..=0.6).contains(&t), "{}: {t}", it.name);
        }
        assert_eq!(grave_bonus(100.0), 0.0);
        assert!((grave_bonus(25.0) - 22.5).abs() < 1e-4);
        let mut heavy = [0u8; SLOTS];
        heavy[..4].copy_from_slice(&[5, 6, 7, 8]);
        heavy[Slot::Feet as usize] = 22;
        heavy[Slot::Off as usize] = 35;
        heavy[Slot::Charm as usize] = 42;
        assert!(stats(&heavy, false).def.iter().all(|&d| d <= DEF_CAP));
    }

    /// Swapping takes its time and puts the old piece in the pack; two hands push the off hand
    /// out; rules can forbid it.
    #[test]
    fn swaps_take_time_and_follow_the_rules() {
        let r = Rules::default();
        let mut k = Kit::new(&presets()[1]); // sabre and buckler, cleaver in the pack
        assert_eq!(k.weapon(), Weapon::Katana);
        let t = k.begin_swap(0, &r).unwrap();
        assert!((0.3..=0.6).contains(&t));
        assert!(k.busy());
        assert_eq!(k.swapping_to().unwrap().name, "Ashfall Cleaver");
        let mut done = None;
        let mut el = 0.0;
        while done.is_none() {
            done = k.tick(0.01, &r).0;
            el += 0.01;
        }
        assert!((el - t).abs() < 0.02);
        assert_eq!(k.weapon(), Weapon::Greatsword);
        assert_eq!(k.equip[Slot::Off as usize], 0, "two hands: the buckler comes off");
        assert!(k.inv.contains(&24) && k.inv.contains(&35), "the sabre and the buckler are in the pack");
        let no = Rules { no_switch: true, ..r };
        assert!(Kit::new(&presets()[1]).begin_swap(0, &no).is_err());
    }

    /// Eating heals and locks attacks; a bite can follow a meal for a little extra; no food rule.
    #[test]
    fn eating_and_combo_eating() {
        let r = Rules::default();
        let mut k = Kit::new(&presets()[1]);
        let slot = |k: &Kit, id: u8| k.inv.iter().position(|&x| x == id).unwrap();
        let bread = slot(&k, 44);
        let (heal, it) = k.eat(bread, &r).unwrap();
        assert_eq!(heal, 16.0);
        assert_eq!(it.name, "Hard Bread");
        let lock = k.eat_t;
        assert!(k.busy());
        let cake = slot(&k, 46);
        let (bite, _) = k.eat(cake, &r).unwrap();
        assert_eq!(bite, 10.0);
        assert!((k.eat_t - lock - COMBO_EXTRA).abs() < 1e-5, "a combo bite adds a little");
        let cake2 = slot(&k, 46);
        assert!(k.eat(cake2, &r).is_err(), "only one bite per meal");
        let mut two = Kit::new(&presets()[0]); // two eels
        let eel = slot(&two, 45);
        two.eat(eel, &r).unwrap();
        let eel2 = slot(&two, 45);
        assert!(two.eat(eel2, &r).is_err(), "no second meal while eating");
        let no = Rules { no_food: true, ..r };
        let k2 = Kit::new(&presets()[1]);
        assert!(k2.clone().eat(slot(&k2, 44), &no).is_err());
    }

    /// A special: costs its share, winds up, then is armed for the next attack.
    #[test]
    fn specials_cost_wind_up_and_strike() {
        let r = Rules::default();
        let mut k = Kit::new(&presets()[0]); // the cleaver: Headsman's Drop, 50
        let s = k.start_spec(&r).unwrap();
        assert_eq!(s, Spec::HeadsmansDrop);
        assert_eq!(k.spec, 50.0);
        assert!(k.start_spec(&r).is_err(), "one at a time");
        let mut fired = None;
        while fired.is_none() {
            fired = k.tick(0.01, &r).1;
        }
        assert!(k.armed.is_some() && k.hit_spec().is_none());
        k.attacked(true);
        assert_eq!(k.hit_spec(), Some(Spec::HeadsmansDrop));
        k.spec = 10.0;
        k.strike = None;
        assert!(k.start_spec(&r).is_err(), "not enough bar");
        assert!(Kit::new(&presets()[0]).start_spec(&Rules { no_specs: true, ..r }).is_err());
    }

    /// Rules round-trip; best of; a forbidden main hand is swapped for an allowed one.
    #[test]
    fn rules_and_fitting_a_loadout() {
        let r = Rules { no_guns: true, no_move: true, best_of: 5, ..Default::default() };
        assert_eq!(Rules::from_bits(r.bits(), r.best_of), r);
        assert_eq!(r.target(), 3);
        assert_eq!(Rules::default().target(), 2);
        assert_eq!(Rules::from_bits(0, 1).target(), 1);
        assert!(!Rules::from_bits(3, 3).no_blades, "never both");
        let marks = presets()[3].clone(); // rifle in hand, sabre in the pack
        let fitted = marks.for_rules(&r);
        assert_eq!(fitted.main().unwrap().name, "Courtly Sabre");
        assert!(fitted.inv.contains(&29), "the rifle goes into the pack");
        let blades_off = presets()[0].for_rules(&Rules { no_blades: true, ..Default::default() });
        assert!(!blades_off.main().unwrap().base.unwrap().is_melee());
        assert!(r.summary().contains("Best of 5") && r.summary().contains("no guns"));
    }

    /// Building a loadout: wearing from the armoury, taking off into the pack, wearing from it.
    #[test]
    fn building_a_loadout() {
        let mut g = presets()[4].clone(); // sabre and hook
        g.wear(25).unwrap(); // the cleaver: two hands
        assert_eq!(g.equip[Slot::Off as usize], 0);
        assert!(g.inv.contains(&36), "the hook went to the pack");
        assert!(g.wear(35).is_err(), "no buckler with two hands full");
        g.take_off(Slot::Head);
        assert_eq!(g.equip[Slot::Head as usize], 0);
        assert!(g.inv.contains(&17));
        let k = g.inv.iter().position(|&x| x == 17).unwrap();
        g.pack_wear(k).unwrap();
        assert_eq!(g.equip[Slot::Head as usize], 17);
        g.take_off(Slot::Main);
        assert_ne!(g.equip[Slot::Main as usize], 0, "something stays in hand");
        while g.pack_add(44).is_ok() {}
        assert!(g.inv.iter().all(|&x| x != 0), "filled up");
        assert!(g.pack_add(44).is_err());
    }

    /// Five named loadouts, the rules and the honour ledger survive saving.
    #[test]
    fn loadouts_and_honour_are_saved() {
        let mut s = LedgerSave::default();
        assert_eq!(s.kits.len(), SAVED);
        s.kits[2].name = "My Fangs".into();
        s.kits[2].inv[11] = 50;
        s.pick = 2;
        s.rules = Rules { no_food: true, best_of: 1, ..Default::default() };
        s.record("My Fangs", true);
        s.record("My Fangs", true);
        s.record("My Fangs", false);
        s.record("My Fangs", true);
        let h = s.honour_of("My Fangs");
        assert_eq!((h.wins, h.losses, h.streak, h.best), (3, 1, 1, 2));
        let back = LedgerSave::from_text(&s.to_text());
        assert_eq!(back, s);
        // a damaged file still gives five usable loadouts
        let bad = LedgerSave::from_text("kit=Odd|999,1|x\npick=9\n");
        assert_eq!(bad.kits.len(), SAVED);
        assert_eq!(bad.pick, SAVED - 1);
        assert_eq!(bad.kits[0].equip[Slot::Main as usize], 24, "something in hand");
    }
}
