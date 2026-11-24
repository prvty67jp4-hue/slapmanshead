//! Named areas of each map: the callouts players use ("two at Crane, Blue side"), shown on the
//! minimap and the HUD and used by pings, and each area's own look on the team map (a warm brick
//! red base, a cool slate blue base, wet dark river banks, wooden and iron bridges, sooty timber
//! workshops) so every lane reads at a glance.

use crate::env::{self, Surface};
use crate::math::*;
use crate::world::{Mat, MapId};

/// How an area looks (its materials' layer, colour and shine), when it differs from the map's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// The map's own palette.
    Map,
    RedBase,
    BlueBase,
    /// Wet, dark stone along the water.
    Riverbank,
    WoodBridge,
    IronBridge,
    StoneBridge,
    /// Timber, soot and forge-blackened brick.
    Workshop,
    /// Weathered, greenish quay stone.
    Quay,
}

/// A named area: a box on the ground (x and z), the heights it covers, and its look.
#[derive(Clone, Copy, Debug)]
pub struct Zone {
    pub name: &'static str,
    pub x: (f32, f32),
    pub z: (f32, f32),
    pub y: (f32, f32),
    pub look: Look,
}

const fn zone(name: &'static str, x: (f32, f32), z: (f32, f32), look: Look) -> Zone {
    Zone { name, x, z, y: (-99.0, 99.0), look }
}
const fn zone_y(name: &'static str, x: (f32, f32), z: (f32, f32), y: (f32, f32), look: Look) -> Zone {
    Zone { name, x, z, y, look }
}

use Look::*;

/// Varrow Bridgeworks: the red team's half is south (z < 0), the blue team's north. Listed from
/// the most specific to the broadest (the first that holds a point names it).
const BRIDGEWORKS: [Zone; 28] = [
    zone("Red Flag Room", (-5.5, 6.0), (-30.0, -23.5), RedBase),
    zone("Blue Flag Room", (-5.5, 6.0), (23.5, 30.0), BlueBase),
    zone("Red Barracks", (-30.0, -5.5), (-30.0, -19.0), RedBase),
    zone("Blue Barracks", (-30.0, -5.5), (19.0, 30.0), BlueBase),
    zone("Red Yard", (6.0, 30.0), (-30.0, -19.0), RedBase),
    zone("Blue Yard", (6.0, 30.0), (19.0, 30.0), BlueBase),
    zone("Red Gate", (-30.0, 30.0), (-19.0, -16.5), RedBase),
    zone("Blue Gate", (-30.0, 30.0), (16.5, 19.0), BlueBase),
    zone_y("Footbridge", (-26.5, -21.5), (-4.6, 4.6), (-0.6, 99.0), WoodBridge),
    zone_y("Stone Bridge", (-8.5, 8.5), (-4.6, 4.6), (-0.6, 99.0), StoneBridge),
    zone_y("Covered Bridge", (17.5, 25.5), (-4.6, 4.6), (-0.6, 99.0), IronBridge),
    zone("Under the Bridge", (-8.5, 8.5), (-4.6, 4.6), Riverbank),
    zone("River", (-30.0, 30.0), (-4.6, 4.6), Riverbank),
    zone("Red Crane", (-30.0, -25.0), (-13.0, -7.0), Quay),
    zone("Blue Crane", (-30.0, -25.0), (7.0, 13.0), Quay),
    zone_y("Red Warehouse Roof", (-17.5, -11.5), (-13.5, -5.5), (6.5, 99.0), Map),
    zone_y("Blue Warehouse Roof", (-17.5, -11.5), (5.5, 13.5), (6.5, 99.0), Map),
    zone("Red Warehouse", (-20.0, -11.5), (-13.5, -5.5), Map),
    zone("Blue Warehouse", (-20.0, -11.5), (5.5, 13.5), Map),
    zone("Red Tower", (-11.5, -8.0), (-8.0, -4.6), Map),
    zone("Blue Tower", (-11.5, -8.0), (4.6, 8.0), Map),
    zone("Red Forge", (18.5, 24.0), (-14.5, -9.5), Workshop),
    zone("Blue Forge", (18.5, 24.0), (9.5, 14.5), Workshop),
    zone("Red Workshops", (13.0, 30.0), (-16.5, -4.6), Workshop),
    zone("Blue Workshops", (13.0, 30.0), (4.6, 16.5), Workshop),
    zone("Red Quay", (-30.0, -20.0), (-16.5, -4.6), Quay),
    zone("Blue Quay", (-30.0, -20.0), (4.6, 16.5), Quay),
    zone("Plaza", (-20.0, 13.0), (-16.5, 16.5), Map),
];

/// Varrow Centre (the hub): its places (see crate::varrow).
const VARROW: [Zone; 9] = [
    zone("West Court Pit", (-22.0, -6.0), (22.6, 36.0), Map),
    zone("East Court Pit", (6.0, 22.0), (22.6, 36.0), Map),
    zone("The Gallery", (24.0, 36.0), (-36.0, 36.0), Map),
    zone("Tables Pavilion", (-36.0, -20.5), (-12.5, 8.5), Map),
    zone("The Tailor's", (-14.5, -1.5), (-36.0, -28.0), Map),
    zone("The Catwalk", (2.5, 9.5), (-36.0, -17.5), Map),
    zone("Challenge Boards", (-24.0, 24.0), (14.0, 36.0), Map),
    zone("The Fountain", (-5.0, 5.0), (-5.0, 5.0), Map),
    zone("Varrow Centre", (-36.0, 24.0), (-36.0, 36.0), Map),
];

/// The Arena: point-symmetric, a spawn balcony at each end.
const ARENA: [Zone; 10] = [
    zone("Tower", (-2.5, 2.5), (-2.5, 2.5), Map),
    zone("South Spawn", (-19.0, 19.0), (-19.0, -15.5), Map),
    zone("North Spawn", (-19.0, 19.0), (15.5, 19.0), Map),
    zone("West Corridor", (-19.0, -13.0), (-15.5, 15.5), Map),
    zone("East Corridor", (13.0, 19.0), (-15.5, 15.5), Map),
    zone("South-West Platform", (-8.5, -3.0), (-13.5, -7.0), Map),
    zone("South-East Platform", (3.0, 8.5), (-13.5, -7.0), Map),
    zone("North-West Platform", (-8.5, -3.0), (7.0, 13.5), Map),
    zone("North-East Platform", (3.0, 8.5), (7.0, 13.5), Map),
    zone("Centre", (-13.0, 13.0), (-15.5, 15.5), Map),
];

/// The Open Courtyard: the dais in the middle, four pillars round it, long walls at each end.
const COURTYARD: [Zone; 11] = [
    zone("Dais", (-5.0, 5.0), (-5.0, 5.0), Map),
    zone("South-West Pillar", (-10.5, -5.5), (-10.5, -5.5), Map),
    zone("South-East Pillar", (5.5, 10.5), (-10.5, -5.5), Map),
    zone("North-West Pillar", (-10.5, -5.5), (5.5, 10.5), Map),
    zone("North-East Pillar", (5.5, 10.5), (5.5, 10.5), Map),
    zone("South Towers", (-24.0, 24.0), (-24.0, -15.0), Map),
    zone("North Towers", (-24.0, 24.0), (15.0, 24.0), Map),
    zone("West Wall", (-24.0, -15.0), (-15.0, 15.0), Map),
    zone("East Wall", (15.0, 24.0), (-15.0, 15.0), Map),
    zone("South Court", (-15.0, 15.0), (-15.0, 0.0), Map),
    zone("North Court", (-15.0, 15.0), (0.0, 15.0), Map),
];

/// Close Quarters: a tight square of halls round a middle.
const CLOSE_QUARTERS: [Zone; 9] = [
    zone("Middle", (-4.0, 4.0), (-4.0, 4.0), Map),
    zone("South-West Corner", (-15.0, -7.0), (-15.0, -7.0), Map),
    zone("South-East Corner", (7.0, 15.0), (-15.0, -7.0), Map),
    zone("North-West Corner", (-15.0, -7.0), (7.0, 15.0), Map),
    zone("North-East Corner", (7.0, 15.0), (7.0, 15.0), Map),
    zone("South Hall", (-7.0, 7.0), (-15.0, -4.0), Map),
    zone("North Hall", (-7.0, 7.0), (4.0, 15.0), Map),
    zone("West Hall", (-15.0, -4.0), (-7.0, 7.0), Map),
    zone("East Hall", (4.0, 15.0), (-7.0, 7.0), Map),
];

pub fn zones(map: MapId) -> &'static [Zone] {
    match map {
        MapId::Bridgeworks => &BRIDGEWORKS,
        MapId::Arena => &ARENA,
        MapId::Courtyard => &COURTYARD,
        MapId::CloseQuarters => &CLOSE_QUARTERS,
        MapId::Varrow => &VARROW,
    }
}

impl Zone {
    pub fn holds(&self, p: V3) -> bool {
        p.x >= self.x.0 && p.x <= self.x.1 && p.z >= self.z.0 && p.z <= self.z.1 && p.y >= self.y.0 && p.y <= self.y.1
    }
}

/// The zone at `p` (the first that holds it).
pub fn zone_at(map: MapId, p: V3) -> Option<&'static Zone> {
    zones(map).iter().find(|z| z.holds(p))
}

/// The callout for a spot: its area's name ("Blue Crane"), or "Open ground" between areas.
pub fn callout(map: MapId, p: V3) -> &'static str {
    // (down in the Ashen Vault: its rooms)
    if map == MapId::Varrow && crate::vault::in_vault(p) {
        return crate::vault::room_at(p).map_or("The Ashen Vault", |k| crate::vault::ROOMS[k].name);
    }
    zone_at(map, p).map_or("Open ground", |z| z.name)
}

/// A material's look in an area (the map's own when the area doesn't change it).
pub fn surface(map: MapId, m: Mat, p: V3) -> Surface {
    // the Ashen Vault, under Varrow Centre: soot-dark stone, iron and old timber
    if map == MapId::Varrow && crate::vault::in_vault(p) {
        let s = |layer, tint, shine| Surface { layer, tint, shine };
        return match m {
            Mat::Floor | Mat::Stone => s(env::L_SLATE, [96, 88, 84], 0.15),
            Mat::Wall => s(env::L_BLOCKS, [104, 96, 92], 0.05),
            Mat::Metal => s(env::L_IRON, [70, 62, 58], 0.4),
            Mat::Trim | Mat::Crate | Mat::Hidden => s(env::L_WOOD, [82, 60, 46], 0.1),
        };
    }
    let look = zone_at(map, p).map_or(Map, |z| z.look);
    look_surface(look, m).unwrap_or_else(|| env::surface(map, m))
}

fn look_surface(look: Look, m: Mat) -> Option<Surface> {
    let s = |layer, tint, shine| Some(Surface { layer, tint, shine });
    match (look, m) {
        (_, Mat::Hidden) | (Map, _) => None,
        // warm brick and red-brown timber
        (RedBase, Mat::Wall) => s(env::L_BRICK, [158, 88, 68], 0.05),
        (RedBase, Mat::Floor) => s(env::L_BLOCKS, [138, 108, 92], 0.2),
        (RedBase, Mat::Stone) => s(env::L_BRICK, [146, 98, 80], 0.06),
        (RedBase, Mat::Trim | Mat::Crate) => s(env::L_WOOD, [118, 66, 46], 0.1),
        // cool slate and blue-grey timber
        (BlueBase, Mat::Wall) => s(env::L_SLATE, [100, 114, 136], 0.08),
        (BlueBase, Mat::Floor) => s(env::L_SLATE, [94, 104, 120], 0.22),
        (BlueBase, Mat::Stone) => s(env::L_BLOCKS, [110, 122, 142], 0.08),
        (BlueBase, Mat::Trim | Mat::Crate) => s(env::L_WOOD, [66, 76, 98], 0.12),
        // wet, dark river stone (shiny)
        (Riverbank, Mat::Wall | Mat::Stone | Mat::Floor) => s(env::L_RUBBLE_STONE, [70, 74, 76], 0.55),
        (Riverbank, Mat::Trim | Mat::Crate) => s(env::L_WOOD, [62, 50, 42], 0.4),
        // planks and tarred timber
        (WoodBridge, Mat::Floor | Mat::Stone | Mat::Trim | Mat::Crate) => s(env::L_WOOD, [132, 96, 64], 0.12),
        (WoodBridge, Mat::Wall) => s(env::L_WOOD, [96, 70, 50], 0.1),
        // riveted iron on a plank deck
        (IronBridge, Mat::Floor) => s(env::L_WOOD, [116, 86, 60], 0.12),
        (IronBridge, Mat::Wall | Mat::Stone | Mat::Metal) => s(env::L_IRON, [92, 98, 108], 0.5),
        (IronBridge, Mat::Trim | Mat::Crate) => s(env::L_IRON, [74, 78, 86], 0.5),
        // grey dressed stone, darker than the plazas round it
        (StoneBridge, Mat::Floor) => s(env::L_BLOCKS, [120, 116, 110], 0.22),
        (StoneBridge, Mat::Stone | Mat::Wall) => s(env::L_SANDSTONE, [128, 120, 110], 0.08),
        // timber walls, soot-black brick, a dark floor
        (Workshop, Mat::Wall) => s(env::L_WOOD, [112, 80, 56], 0.08),
        (Workshop, Mat::Stone) => s(env::L_BRICK, [82, 70, 64], 0.05),
        (Workshop, Mat::Floor) => s(env::L_BLOCKS, [86, 78, 70], 0.15),
        (Workshop, Mat::Trim | Mat::Crate) => s(env::L_WOOD, [70, 50, 38], 0.1),
        // weathered quay stone, a little green
        (Quay, Mat::Wall | Mat::Stone) => s(env::L_RUBBLE_STONE, [118, 124, 112], 0.08),
        (Quay, Mat::Floor) => s(env::L_BLOCKS, [116, 118, 110], 0.25),
        _ => None,
    }
}

/// The x and z lines where a box crosses from one area into another (so the renderer can cut a
/// big floor or wall into pieces that each take their own area's look).
pub fn cuts(map: MapId, min: V3, max: V3) -> (Vec<f32>, Vec<f32>) {
    let mut xs = vec![min.x, max.x];
    let mut zs = vec![min.z, max.z];
    for z in zones(map).iter().filter(|z| z.look != Map || map == MapId::Bridgeworks) {
        for x in [z.x.0, z.x.1] {
            if x > min.x + 0.05 && x < max.x - 0.05 {
                xs.push(x);
            }
        }
        for c in [z.z.0, z.z.1] {
            if c > min.z + 0.05 && c < max.z - 0.05 {
                zs.push(c);
            }
        }
    }
    xs.sort_by(f32::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    zs.sort_by(f32::total_cmp);
    zs.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    (xs, zs)
}

/// Glowing landmarks drawn into the level (unlit, they bloom): (centre, facing, radius, colour as
/// linear light). The team map's clock tower has three lit faces; the forges glow from their
/// mouths.
pub fn glows(map: MapId) -> Vec<(V3, V3, f32, [f32; 3])> {
    match map {
        MapId::Bridgeworks => {
            let face = [2.2f32, 1.9, 1.35];
            let fire = [3.2f32, 1.3, 0.35];
            vec![
                (v3(CLOCK.0 + 2.05, 16.0, CLOCK.1), v3(1.0, 0.0, 0.0), 1.35, face),
                (v3(CLOCK.0, 16.0, CLOCK.1 + 2.05), v3(0.0, 0.0, 1.0), 1.35, face),
                (v3(CLOCK.0, 16.0, CLOCK.1 - 2.05), v3(0.0, 0.0, -1.0), 1.35, face),
                (v3(21.25, 0.75, 13.02), v3(0.0, 0.0, 1.0), 0.55, fire),
                (v3(21.25, 0.75, -13.02), v3(0.0, 0.0, -1.0), 0.55, fire),
            ]
        }
        _ => Vec::new(),
    }
}

/// Where the team map's clock tower stands (outside the west wall, on the river's line).
pub const CLOCK: (f32, f32) = (-36.0, 0.0);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    /// Every map has callouts covering where players stand, every name is unique, and the team
    /// map's two halves mirror each other (each Red area has its Blue twin).
    #[test]
    fn every_map_has_callouts_where_players_go() {
        for map in MapId::ALL {
            let w = World::build(map);
            let names: Vec<&str> = zones(map).iter().map(|z| z.name).collect();
            for (k, n) in names.iter().enumerate() {
                assert!(!names[..k].contains(n), "{map:?}: {n} twice");
            }
            // every spawn and both flags have a name
            for (p, _) in &w.spawn_points {
                assert_ne!(callout(map, *p + v3(0.0, 0.2, 0.0)), "Open ground", "{map:?}: spawn {p:?} named");
            }
            for f in w.flags {
                assert_ne!(callout(map, f + v3(0.0, 0.2, 0.0)), "Open ground", "{map:?}: flag named");
            }
            // most of the floor is named
            let h = w.half - 1.0;
            let (mut named, mut all) = (0, 0);
            for i in 0..20 {
                for j in 0..20 {
                    let p = v3(-h + 2.0 * h * i as f32 / 19.0, 0.2, -h + 2.0 * h * j as f32 / 19.0);
                    all += 1;
                    if zone_at(map, p).is_some() {
                        named += 1;
                    }
                }
            }
            assert!(named * 10 >= all * 9, "{map:?}: {named} of {all} spots named");
        }
        for z in zones(MapId::Bridgeworks) {
            if let Some(rest) = z.name.strip_prefix("Red ") {
                assert!(zones(MapId::Bridgeworks).iter().any(|o| o.name == format!("Blue {rest}")), "{} has a Blue twin", z.name);
            }
        }
    }

    /// On the team map the two bases, the river, the bridges and the workshops each look different:
    /// no two of them share both the wall material and colour, and the river banks are darker.
    #[test]
    fn team_map_areas_each_have_their_own_look() {
        let map = MapId::Bridgeworks;
        let at = |x: f32, y: f32, z: f32, m: Mat| surface(map, m, v3(x, y, z));
        let red = at(-15.0, 0.0, -25.0, Mat::Wall);
        let blue = at(-15.0, 0.0, 25.0, Mat::Wall);
        let bank = at(-15.0, -1.0, 3.0, Mat::Wall);
        let shop = at(20.0, 0.0, -10.0, Mat::Wall);
        let wood = at(-24.0, 0.2, 0.0, Mat::Floor);
        let iron = at(21.0, 0.5, 0.0, Mat::Wall);
        let looks = [red, blue, bank, shop, wood, iron];
        for (i, a) in looks.iter().enumerate() {
            for b in &looks[i + 1..] {
                assert!(a.layer != b.layer || a.tint != b.tint, "two areas look alike: {a:?} {b:?}");
            }
        }
        assert!(red.tint[0] > red.tint[2] + 40, "the red base is warm");
        assert!(blue.tint[2] > blue.tint[0] + 25, "the blue base is cool");
        let lum = |s: Surface| s.tint[0] as u32 + s.tint[1] as u32 + s.tint[2] as u32;
        assert!(lum(bank) < lum(at(0.0, 0.0, -12.0, Mat::Wall)), "the banks are darker than the plaza");
        assert!(bank.shine > 0.4, "and wet");
        assert_eq!(wood.layer, env::L_WOOD, "the footbridge is wooden");
        assert_eq!(iron.layer, env::L_IRON, "the covered bridge is iron");
        assert_eq!(callout(map, v3(-28.0, 0.2, 10.0)), "Blue Crane");
        assert_eq!(callout(map, v3(0.0, 0.2, 0.0)), "Stone Bridge");
        assert_eq!(callout(map, v3(0.0, -3.0, 0.0)), "Under the Bridge");
    }
}
