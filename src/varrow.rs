//! Varrow Centre: the town square you load into (the hub, crate::hub). 72 x 72 m inside the old
//! town walls, at the same dusk as the Bridgeworks across the river:
//! - the plaza: a fountain in the middle, benches round it, lamp posts;
//! - north: the two Court Pits (walled duel rings, a spectators' walkway along their tops and a
//!   terrace between them), with the Challenge Boards on their front walls;
//! - east: the Gallery, two 60 m shooting lanes side by side (the aim rooms);
//! - west: the tables pavilion (chess, checkers and Connect 4 under a timber roof);
//! - south: the tailor's (the wardrobe and the mirror) and the catwalk with its podium;
//! - the Rooftop Run: crates, the pavilion roof, a tower, a dash gap, a beam and the walkway.
//!
//! Every place the hub uses (seats, tables, the pits' spawns, the lanes, the course's rings) is a
//! constant here, so the map and the hub agree.

use crate::math::*;
use crate::world::{Block, Mat, MapId, Prop, PropKind, Vine, World};

pub const HALF: f32 = 36.0;
const WALL_H: f32 = 10.0;
/// The pits' walls and the walkway on top of them.
pub const PIT_WALL: f32 = 4.5;

fn bx(x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32, mat: Mat) -> Block {
    Block { b: Aabb::new(v3(x0.min(x1), y0.min(y1), z0.min(z1)), v3(x0.max(x1), y0.max(y1), z0.max(z1))), mat }
}

/// Where you arrive (and come back to after a duel): the south side of the plaza, facing the
/// fountain. (position, facing)
pub const ARRIVALS: [(f32, f32, f32); 8] = [(-4.0, -12.0, 0.0), (4.0, -12.0, 0.0), (0.0, -13.5, 0.0), (-8.0, -10.0, 0.6), (8.0, -10.0, -0.6), (-2.0, -15.0, 0.0), (2.0, -15.0, 0.0), (0.0, -10.5, 0.0)];

/// A Court Pit: its floor (x, z ranges) and the two duellists' spawns (position, facing).
#[derive(Clone, Copy, Debug)]
pub struct Pit {
    pub x: (f32, f32),
    pub z: (f32, f32),
    pub spawns: [(V3, f32); 2],
}

pub const PITS: [Pit; 2] = [
    Pit { x: (-22.0, -6.0), z: (22.6, 35.6), spawns: [(V3 { x: -14.0, y: 0.0, z: 24.5 }, 0.0), (V3 { x: -14.0, y: 0.0, z: 33.5 }, std::f32::consts::PI)] },
    Pit { x: (6.0, 22.0), z: (22.6, 35.6), spawns: [(V3 { x: 14.0, y: 0.0, z: 24.5 }, 0.0), (V3 { x: 14.0, y: 0.0, z: 33.5 }, std::f32::consts::PI)] },
];

/// The Challenge Boards (on the pits' front walls, facing the plaza): where they hang.
pub const BOARDS: [(f32, f32, f32); 2] = [(-11.0, 2.2, 20.96), (11.0, 2.2, 20.96)];
/// The Gallery's scoreboard and the Rooftop Run's (on walls, facing out).
pub const GALLERY_BOARD: (f32, f32, f32) = (23.94, 2.4, -24.0);
pub const RUN_BOARD: (f32, f32, f32) = (-20.96, 2.4, -16.0);

/// The Gallery: two lanes running north from the firing line.
#[derive(Clone, Copy, Debug)]
pub struct Lane {
    /// The lane's sides (x) and where you stand to shoot (z, at the firing line).
    pub x: (f32, f32),
    pub stand: V3,
}

pub const FIRING_Z: f32 = -30.0;
pub const LANES: [Lane; 2] = [Lane { x: (24.6, 29.4), stand: V3 { x: 27.0, y: 0.0, z: -30.0 } }, Lane { x: (30.6, 35.4), stand: V3 { x: 33.0, y: 0.0, z: -30.0 } }];
/// The lectern you walk up to to start a room (one per lane).
pub const LECTERNS: [(f32, f32, f32); 2] = [(25.4, 0.0, -32.6), (31.4, 0.0, -32.6)];

/// The tables (centre of the table top) and the games they hold, under the pavilion.
pub const TABLES: [(f32, f32, crate::boards::GameKind); 4] = [
    (-29.0, -6.0, crate::boards::GameKind::Chess),
    (-24.5, -6.0, crate::boards::GameKind::Checkers),
    (-29.0, 3.0, crate::boards::GameKind::Connect4),
    (-24.5, 3.0, crate::boards::GameKind::Chess),
];
pub const TABLE_TOP: f32 = 0.95;
/// How far each chair is from its table's middle (along x: seat 0 on the west side).
pub const CHAIR_OFF: f32 = 1.05;
/// The pavilion's deck height (the tables stand on it).
pub const DECK: f32 = 0.2;

/// The tailor's: the mirror (on the back wall, facing north) and the wardrobe chests.
pub const MIRROR: (f32, f32, f32) = (-8.0, 1.6, -35.55);
/// The Bank's booth: the middle of its counter (players stand on its west side, x - 1.1; the
/// Banker behind it, x + 1.3, facing west).
pub const BANK: (f32, f32, f32) = (22.0, 0.0, -12.5);
pub const WARDROBES: [(f32, f32, f32); 2] = [(-12.6, 0.0, -33.0), (-3.4, 0.0, -33.0)];
/// The catwalk (x, z ranges, its top) and the podium at its end.
pub const CATWALK: ((f32, f32), (f32, f32), f32) = ((4.5, 7.5), (-30.0, -18.0), 0.6);
pub const PODIUM: ((f32, f32), (f32, f32), f32) = ((3.0, 9.0), (-35.4, -30.0), 1.1);

/// Benches and ledges you can sit on: (seat position, facing). Benches round the fountain, the
/// fountain's rim, the pits' walkway, the catwalk's edge.
pub fn seats() -> Vec<(V3, f32)> {
    let mut s = Vec::new();
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::TAU / 8.0 + std::f32::consts::FRAC_PI_8;
        let p = v3(a.sin() * 8.2, 0.45, a.cos() * 8.2);
        // facing the fountain
        let face = (-p.x).atan2(-p.z);
        let side = v3(face.cos(), 0.0, -face.sin()) * 0.45;
        s.push((p + side, face));
        s.push((p - side, face));
    }
    // the fountain's rim, facing out
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::TAU / 8.0;
        let p = v3(a.sin() * 4.25, 0.6, a.cos() * 4.25);
        s.push((p, a));
    }
    // the walkway's edge over each pit (facing in, legs over the drop)
    for x in [-18.0f32, -14.0, -10.0, 10.0, 14.0, 18.0] {
        s.push((v3(x, PIT_WALL, 22.35), 0.0));
    }
    s
}

/// The Rooftop Run's rings, in order: start, five on the way, the finish. (centre)
pub const RUN_RINGS: [(f32, f32, f32); 7] = [
    (-17.0, 1.2, -19.0),
    (-19.6, 4.4, -14.2),
    (-27.0, 5.6, -2.0),
    (-28.0, 6.2, 12.0),
    (-28.8, 6.2, 22.2),
    (-14.0, 5.7, 21.8),
    (0.0, 1.4, 9.5),
];
pub const RING_RADIUS: f32 = 1.7;

/// The course's blocks (also used to check it can be run).
fn rooftop_run(v: &mut Vec<Block>) {
    // three crates, each a jump higher: 1.2, 2.2, 3.2
    v.push(bx(-20.2, 0.0, -18.0, -18.8, 1.2, -16.6, Mat::Crate));
    v.push(bx(-20.2, 0.0, -16.6, -18.8, 2.2, -15.2, Mat::Crate));
    v.push(bx(-20.2, 0.0, -15.2, -18.8, 3.2, -13.8, Mat::Crate));
    // a plank from the last crate onto the pavilion roof (4.2 high, a step up from 3.2 with a
    // short hop)
    v.push(bx(-21.0, 3.2, -14.4, -18.8, 3.7, -13.0, Mat::Trim));
    // the tower past the pavilion's north end (a 2.5 m jump from the roof)
    v.push(bx(-30.0, 0.0, 10.5, -26.0, 5.0, 13.5, Mat::Stone));
    // the dash gap: 6.5 m on to the next platform, a wall alongside to run on instead
    v.push(bx(-31.6, 0.0, 13.5, -31.0, 8.0, 20.0, Mat::Wall));
    v.push(bx(-31.0, 0.0, 20.0, -27.0, 5.0, 26.0, Mat::Stone));
    // the beam over to the pits' walkway
    v.push(bx(-27.0, 4.1, 21.0, -22.6, 4.5, 22.4, Mat::Trim));
    v.push(bx(-27.0, 0.0, 21.6, -26.6, 4.1, 22.2, Mat::Hidden));
}

pub fn world() -> World {
    let h = HALF;
    let mut v = Vec::new();
    let mut deco = Vec::new();
    // ---- the ground and the town walls (house fronts, with a cornice and chimneys on top)
    v.push(bx(-h, -1.0, -h, h, 0.0, h, Mat::Floor));
    v.push(bx(-h - 1.0, 0.0, -h - 1.0, h + 1.0, WALL_H, -h, Mat::Wall));
    v.push(bx(-h - 1.0, 0.0, h, h + 1.0, WALL_H, h + 1.0, Mat::Wall));
    v.push(bx(-h - 1.0, 0.0, -h, -h, WALL_H, h, Mat::Wall));
    v.push(bx(h, 0.0, -h, h + 1.0, WALL_H, h, Mat::Wall));
    for k in 0..12 {
        let a = -h + 6.0 * k as f32 + 3.0;
        for (x0, z0, x1, z1) in [(a - 0.4, -h - 0.6, a + 0.4, -h + 0.2), (a - 0.4, h - 0.2, a + 0.4, h + 0.6), (-h - 0.6, a - 0.4, -h + 0.2, a + 0.4), (h - 0.2, a - 0.4, h + 0.6, a + 0.4)] {
            deco.push(bx(x0, WALL_H, z0, x1, WALL_H + 1.6, z1, Mat::Stone));
        }
    }
    for (x0, z0, x1, z1) in [(-h - 1.2, -h - 1.2, h + 1.2, -h + 0.1), (-h - 1.2, h - 0.1, h + 1.2, h + 1.2), (-h - 1.2, -h, -h + 0.1, h), (h - 0.1, -h, h + 1.2, h)] {
        deco.push(bx(x0, WALL_H - 0.3, z0, x1, WALL_H, z1, Mat::Trim));
    }
    // timber string courses on the house fronts
    for y in [3.4f32, 6.8] {
        for (x0, z0, x1, z1) in [(-h, -h, h, -h + 0.12), (-h, h - 0.12, h, h), (-h, -h, -h + 0.12, h), (h - 0.12, -h, h, h)] {
            deco.push(bx(x0, y, z0, x1, y + 0.25, z1, Mat::Trim));
        }
    }

    // ---- the fountain: a square basin, its pedestal (the statue is a prop)
    for (x0, z0, x1, z1) in [(-4.4, -4.4, 4.4, -3.8), (-4.4, 3.8, 4.4, 4.4), (-4.4, -3.8, -3.8, 3.8), (3.8, -3.8, 4.4, 3.8)] {
        v.push(bx(x0, 0.0, z0, x1, 0.6, z1, Mat::Stone));
    }
    v.push(bx(-3.8, 0.0, -3.8, 3.8, 0.12, 3.8, Mat::Stone));
    v.push(bx(-0.9, 0.0, -0.9, 0.9, 2.4, 0.9, Mat::Stone));
    // benches round it (low: you walk round them; the props draw them)
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::TAU / 8.0 + std::f32::consts::FRAC_PI_8;
        let (c, s) = (a.sin() * 8.6, a.cos() * 8.6);
        v.push(bx(c - 0.55, 0.0, s - 0.55, c + 0.55, 0.42, s + 0.55, Mat::Hidden));
    }

    // ---- the Court Pits: walls round each ring, a walkway on their front walls, the terrace
    // between them with steps up from the plaza
    for p in PITS {
        v.push(bx(p.x.0 - 0.6, 0.0, 21.0, p.x.1 + 0.6, PIT_WALL, 22.6, Mat::Wall));
        v.push(bx(p.x.0 - 0.6, 0.0, 22.6, p.x.0, PIT_WALL, h, Mat::Wall));
        v.push(bx(p.x.1, 0.0, 22.6, p.x.1 + 0.6, PIT_WALL, h, Mat::Wall));
        // the railing along the walkway's inner edge (the prop draws it)
        v.push(bx(p.x.0, PIT_WALL, 22.45, p.x.1, PIT_WALL + 1.0, 22.6, Mat::Hidden));
        // sand underfoot, a low post in each corner
        v.push(bx(p.x.0, 0.0, p.z.0, p.x.1, 0.05, p.z.1, Mat::Floor));
        for (x, z) in [(p.x.0 + 0.6, p.z.0 + 0.6), (p.x.1 - 0.6, p.z.0 + 0.6), (p.x.0 + 0.6, p.z.1 - 0.6), (p.x.1 - 0.6, p.z.1 - 0.6)] {
            v.push(bx(x - 0.3, 0.0, z - 0.3, x + 0.3, 1.1, z + 0.3, Mat::Stone));
        }
        // cover in the middle of each ring: two low walls
        let cx = (p.x.0 + p.x.1) * 0.5;
        v.push(bx(cx - 3.5, 0.0, 28.6, cx - 1.5, 1.2, 29.4, Mat::Stone));
        v.push(bx(cx + 1.5, 0.0, 28.6, cx + 3.5, 1.2, 29.4, Mat::Stone));
    }
    v.push(bx(-5.4, 0.0, 21.0, 5.4, PIT_WALL, h, Mat::Stone));
    for k in 1..=9 {
        let z0 = 21.0 - (10 - k) as f32;
        v.push(bx(-3.0, 0.0, z0, 3.0, 0.5 * k as f32, z0 + 1.0, Mat::Stone));
    }
    // the terrace's front edge rails, either side of the steps
    v.push(bx(-5.4, PIT_WALL, 21.0, -3.0, PIT_WALL + 1.0, 21.15, Mat::Hidden));
    v.push(bx(3.0, PIT_WALL, 21.0, 5.4, PIT_WALL + 1.0, 21.15, Mat::Hidden));

    // ---- the Gallery: its wall onto the plaza (a door by the firing line), the lanes' divider,
    // a roof over the firing line, target frames at the far end
    v.push(bx(24.0, 0.0, -h, 24.6, 6.0, -33.6, Mat::Wall));
    v.push(bx(24.0, 0.0, -28.4, 24.6, 6.0, h, Mat::Wall));
    v.push(bx(29.4, 0.0, -32.0, 30.6, 6.0, h, Mat::Wall));
    v.push(bx(24.0, 6.0, -h, h, 6.4, -28.0, Mat::Trim));
    v.push(bx(24.6, 0.0, 34.4, h, 3.6, h, Mat::Stone));
    // the lecterns
    for (x, _, z) in LECTERNS {
        v.push(bx(x - 0.35, 0.0, z - 0.3, x + 0.35, 1.1, z + 0.3, Mat::Hidden));
    }

    // ---- the tables pavilion: a timber deck, posts, a roof (part of the Rooftop Run)
    v.push(bx(-33.0, 0.0, -12.0, -21.0, DECK, 8.0, Mat::Trim));
    for (x, z) in [(-33.0f32, -12.0f32), (-21.4, -12.0), (-33.0, 7.6), (-21.4, 7.6), (-33.0, -2.2), (-21.4, -2.2)] {
        v.push(bx(x, DECK, z, x + 0.4, 4.0, z + 0.4, Mat::Trim));
    }
    v.push(bx(-33.4, 4.0, -12.4, -20.6, 4.2, 8.4, Mat::Trim));
    for (x, z, _) in TABLES {
        v.push(bx(x - 0.5, DECK, z - 0.5, x + 0.5, TABLE_TOP, z + 0.5, Mat::Hidden));
    }

    // ---- the tailor's (an open shop front in the south wall) and the catwalk
    v.push(bx(-14.0, 0.0, -h, -13.4, 4.0, -30.0, Mat::Wall));
    v.push(bx(-2.6, 0.0, -h, -2.0, 4.0, -30.0, Mat::Wall));
    v.push(bx(-14.0, 4.0, -h, -2.0, 4.4, -29.6, Mat::Trim));
    for (x, _, z) in WARDROBES {
        v.push(bx(x - 0.55, 0.0, z - 0.45, x + 0.55, 2.2, z + 0.45, Mat::Hidden));
    }
    let ((cx0, cx1), (cz0, cz1), ch) = CATWALK;
    v.push(bx(cx0, 0.0, cz0, cx1, ch, cz1, Mat::Trim));
    let ((px0, px1), (pz0, pz1), ph) = PODIUM;
    v.push(bx(px0, 0.0, pz0, px1, ph, pz1, Mat::Stone));
    // steps up onto the catwalk's north end
    v.push(bx(cx0, 0.0, cz1, cx1, 0.3, cz1 + 0.6, Mat::Trim));

    // ---- the Rooftop Run
    rooftop_run(&mut v);

    // ---- light: braziers by the pits' gates, lanterns on the walls
    let mut braziers = Vec::new();
    for x in [-4.2f32, 4.2] {
        v.push(bx(x - 0.25, PIT_WALL, 21.3, x + 0.25, PIT_WALL + 0.7, 21.8, Mat::Metal));
        braziers.push(v3(x, PIT_WALL + 0.75, 21.55));
    }
    let mut props = Vec::new();
    let mut torches = Vec::new();
    let mut lantern = |x: f32, y: f32, z: f32, out: V3| {
        props.push(Prop { kind: PropKind::Lantern, pos: v3(x, y, z), yaw: out.x.atan2(out.z), scale: 1.0, team: 255 });
        torches.push(v3(x, y, z) + out * 0.32 - v3(0.0, 0.32, 0.0));
    };
    let (px, nx, pz, nz) = (v3(1.0, 0.0, 0.0), v3(-1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), v3(0.0, 0.0, -1.0));
    lantern(-20.0, 3.2, 20.98, nz);
    lantern(20.0, 3.2, 20.98, nz);
    lantern(23.98, 3.0, -10.0, nx);
    lantern(23.98, 3.0, 10.0, nx);
    lantern(-h, 3.2, -24.0, px);
    lantern(-h, 3.2, 20.0, px);
    lantern(-10.0, 3.2, -h, pz);
    lantern(14.0, 3.2, -h, pz);
    lantern(20.0, 3.2, -h, pz);
    lantern(-20.0, 3.2, h, nz);
    lantern(0.0, 6.6, h, nz);
    lantern(h, 3.2, -h + 4.0, nx);
    use PropKind::*;
    let pi = std::f32::consts::PI;
    let hp = std::f32::consts::FRAC_PI_2;
    let mut put = |kind: PropKind, x: f32, y: f32, z: f32, yaw: f32| props.push(Prop { kind, pos: v3(x, y, z), yaw, scale: 1.0, team: 255 });
    // the fountain's statue and the benches
    put(HubFountain, 0.0, 0.0, 0.0, 0.0);
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::TAU / 8.0 + std::f32::consts::FRAC_PI_8;
        put(HubBench, a.sin() * 8.6, 0.0, a.cos() * 8.6, a + pi);
    }
    // lamp posts round the plaza
    for (x, z) in [(-12.0f32, -12.0f32), (12.0, -12.0), (-12.0, 12.0), (12.0, 12.0), (-18.0, 0.0), (18.0, 0.0)] {
        put(HubLamp, x, 0.0, z, 0.0);
    }
    // the Challenge Boards, the scoreboards
    for (x, y, z) in BOARDS {
        put(HubBoard, x, y, z, pi);
    }
    put(HubBoard, GALLERY_BOARD.0, GALLERY_BOARD.1, GALLERY_BOARD.2, -hp);
    put(HubBoard, RUN_BOARD.0, RUN_BOARD.1, RUN_BOARD.2, hp);
    // railings along the walkway and the terrace's edge
    for p in PITS {
        let mut x = p.x.0 + 1.5;
        while x < p.x.1 - 1.0 {
            put(Railing, x, PIT_WALL, 22.52, 0.0);
            x += 3.0;
        }
    }
    // the pits' gates (closed: duellists are brought in) under the walkway
    for p in PITS {
        put(Archway, (p.x.0 + p.x.1) * 0.5, 0.0, 20.9, 0.0);
    }
    // the Gallery: lecterns, target frames down each lane
    for (x, _, z) in LECTERNS {
        put(HubLectern, x, 0.0, z, 0.0);
    }
    for l in LANES {
        let cx = (l.x.0 + l.x.1) * 0.5;
        for z in [-14.0f32, 4.0, 22.0] {
            put(HubTargetFrame, cx, 0.0, z, pi);
        }
    }
    // the tables and their chairs
    for (x, z, _) in TABLES {
        put(HubTable, x, DECK, z, 0.0);
        put(HubChair, x - CHAIR_OFF, DECK, z, hp);
        put(HubChair, x + CHAIR_OFF, DECK, z, -hp);
    }
    // the tailor's: the mirror, the wardrobes, a dummy in the window
    put(HubMirror, MIRROR.0, 0.0, -h + 0.05, 0.0);
    for (k, (x, y, z)) in WARDROBES.iter().enumerate() {
        put(HubWardrobe, *x, *y, *z, if k == 0 { hp } else { -hp });
    }
    // spotlights on the podium
    put(HubLamp, PODIUM.0 .0 - 0.6, 0.0, -31.0, 0.0);
    put(HubLamp, PODIUM.0 .1 + 0.6, 0.0, -31.0, 0.0);
    // crates, barrels and plants along the walls
    put(CrateStack, 22.5, 0.0, -34.6, 0.0);
    put(Barrel, -34.8, 0.0, -30.0, 0.0);
    put(Barrel, -34.6, 0.0, -29.2, 1.1);
    put(Plant, -15.5, 0.0, -34.8, 0.0);
    put(Plant, -0.5, 0.0, -34.8, 0.0);
    put(Plant, -5.6, 0.0, 20.6, 0.0);
    put(Plant, 5.6, 0.0, 20.6, 0.0);
    put(Barrel, 23.2, 0.0, 20.0, 0.4);
    put(Crate, -34.4, 0.0, 33.6, 0.3);
    // banners on the town walls (Varrow's own colours)
    for x in [-24.0f32, -8.0, 8.0, 24.0] {
        put(BannerA, x, 6.4, h - 0.02, pi);
    }
    for z in [-20.0f32, 0.0, 20.0] {
        put(BannerB, -h + 0.02, 6.4, z, hp);
    }
    market(&mut v, &mut props, &mut torches);
    // the Vault Gate (an arch over the circle a raid party gathers in) and the raid's board
    let (gx, gz, _) = crate::vault::GATE;
    props.push(Prop { kind: PropKind::Archway, pos: v3(gx, 0.0, gz - 3.4), yaw: 0.0, scale: 1.0, team: 255 });
    v.push(bx(gx - 2.25, 0.0, gz - 3.8, gx - 1.55, 4.2, gz - 3.0, Mat::Hidden));
    v.push(bx(gx + 1.55, 0.0, gz - 3.8, gx + 2.25, 4.2, gz - 3.0, Mat::Hidden));
    let (bx_, by_, bz_) = crate::vault::GATE_BOARD;
    props.push(Prop { kind: PropKind::HubBoard, pos: v3(bx_, by_, bz_), yaw: std::f32::consts::FRAC_PI_2, scale: 1.0, team: 255 });
    torches.push(v3(gx, 1.0, gz));
    // the Bank: a booth against the east pits' wall, a counter with the Banker behind it
    let (kx, _, kz) = BANK;
    v.push(bx(kx - 0.3, 0.0, kz - 1.7, kx + 0.3, 1.05, kz + 1.7, Mat::Crate));
    v.push(bx(kx + 2.2, 0.0, kz - 2.0, kx + 2.55, 2.9, kz + 2.0, Mat::Trim));
    v.push(bx(kx - 0.35, 0.0, kz - 2.05, kx + 2.55, 2.9, kz - 1.85, Mat::Trim));
    v.push(bx(kx - 0.35, 0.0, kz + 1.85, kx + 2.55, 2.9, kz + 2.05, Mat::Trim));
    deco.push(bx(kx - 0.4, 1.05, kz - 1.75, kx + 0.4, 1.12, kz + 1.75, Mat::Trim));
    for z in [kz - 1.95, kz + 1.95] {
        deco.push(bx(kx - 0.45, 0.0, z - 0.12, kx - 0.25, 3.2, z + 0.12, Mat::Trim));
    }
    deco.push(bx(kx - 0.75, 3.2, kz - 2.25, kx + 2.7, 3.42, kz + 2.25, Mat::Stone));
    deco.push(bx(kx + 1.9, 0.0, kz - 1.7, kx + 2.2, 2.2, kz + 1.7, Mat::Crate));
    for z in [kz - 1.6, kz + 1.6] {
        props.push(Prop { kind: PropKind::Lantern, pos: v3(kx - 0.6, 2.5, z), yaw: -std::f32::consts::FRAC_PI_2, scale: 0.9, team: 255 });
        torches.push(v3(kx - 0.9, 2.2, z));
    }
    // the vault itself, 40 m down (its floor tiles are the last blocks: see crate::vault)
    crate::vault::build(&mut v, &mut deco, &mut props, &mut torches);
    // ivy
    let mut vines = Vec::new();
    for (k, x) in [-30.0f32, -18.0, 2.0, 16.0].iter().enumerate() {
        vines.push(Vine { top: v3(*x, WALL_H, -h), out: pz, len: 3.2 + k as f32 * 0.3, seed: 300 + k as u32 });
    }
    for (k, z) in [-26.0f32, 28.0].iter().enumerate() {
        vines.push(Vine { top: v3(-h, WALL_H, *z), out: px, len: 3.0, seed: 310 + k as u32 });
    }
    let spawn_points = ARRIVALS.iter().map(|&(x, z, yaw)| (v3(x, 0.0, z), yaw)).collect();
    World {
        blocks: v,
        spawn_points,
        half: h,
        deco,
        braziers,
        props,
        vines,
        torches,
        map: MapId::Varrow,
        flags: [v3(0.0, 0.0, -20.0), v3(0.0, 0.0, 20.0)],
        water: vec![Aabb::new(v3(-3.8, 0.0, -3.8), v3(3.8, 0.4, 3.8))],
    }
}

/// The market stalls: (kind, centre, the way its front faces). Off the regulars' paths and the
/// Rooftop Run.
pub const STALLS: [(PropKind, f32, f32, f32, f32); 4] = [
    (PropKind::HubStallProduce, -12.0, 16.6, 0.0, -1.0),
    (PropKind::HubStallCloth, 12.0, 16.6, 0.0, -1.0),
    (PropKind::HubStallPottery, 20.8, 6.5, -1.0, 0.0),
    (PropKind::HubStallProduce, 20.8, -6.5, -1.0, 0.0),
];
/// Trees in their stone rings (centre).
pub const TREES: [(f32, f32); 4] = [(-16.0, 9.0), (16.0, 10.5), (-7.0, -21.0), (14.0, -21.5)];

/// Varrow's market and greenery: the stalls (each solid, lit by its own lantern), trees and
/// planters, crates and barrels round the stalls, more banners, and warm light from the plaza's
/// lamp posts.
fn market(v: &mut Vec<Block>, props: &mut Vec<Prop>, torches: &mut Vec<V3>) {
    let mut put = |props: &mut Vec<Prop>, kind: PropKind, x: f32, y: f32, z: f32, yaw: f32| props.push(Prop { kind, pos: v3(x, y, z), yaw, scale: 1.0, team: 255 });
    for (kind, x, z, fx, fz) in STALLS {
        let yaw = fx.atan2(fz);
        put(props, kind, x, 0.0, z, yaw);
        // solid where it stands (3 m along its front, 2 m deep), to the awning's height
        let (hw, hd) = if fx.abs() > 0.5 { (1.0, 1.5) } else { (1.5, 1.0) };
        v.push(bx(x - hw, 0.0, z - hd, x + hw, 2.3, z + hd, Mat::Hidden));
        // its lantern (under the awning's front corner) lights the counter
        let (lx, lz) = (1.2f32, 0.95f32);
        let (c, s) = (yaw.cos(), yaw.sin());
        torches.push(v3(x + lx * c + lz * s, 1.55, z - lx * s + lz * c));
    }
    for (x, z) in TREES {
        put(props, PropKind::HubTree, x, 0.0, z, (x * 0.37 + z).sin() * 3.0);
        // the ring is a low step, the trunk solid
        v.push(bx(x - 1.15, 0.0, z - 1.15, x + 1.15, 0.42, z + 1.15, Mat::Hidden));
        v.push(bx(x - 0.22, 0.42, z - 0.22, x + 0.22, 3.0, z + 0.22, Mat::Hidden));
    }
    // planters under the pits' walls and along the Gallery's
    let hp = std::f32::consts::FRAC_PI_2;
    for (x, z, yaw) in [(-16.5f32, 20.3f32, 0.0f32), (-8.0, 20.3, 0.0), (8.0, 20.3, 0.0), (16.5, 20.3, 0.0), (23.4, 1.5, hp), (23.4, 14.0, hp)] {
        put(props, PropKind::HubPlanter, x, 0.0, z, yaw);
        let (hw, hd) = if yaw != 0.0 { (0.36, 0.95) } else { (0.95, 0.36) };
        v.push(bx(x - hw, 0.0, z - hd, x + hw, 0.55, z + hd, Mat::Hidden));
    }
    // crates and barrels round the stalls
    for (kind, x, z, yaw) in [
        (PropKind::Crate, -14.3, 17.9, 0.3),
        (PropKind::Barrel, -9.6, 17.8, 0.0),
        (PropKind::Barrel, -9.3, 16.9, 1.2),
        (PropKind::CrateStack, 14.6, 18.0, 0.2),
        (PropKind::Barrel, 9.5, 17.6, 0.6),
        (PropKind::Crate, 22.5, 9.0, 1.4),
        (PropKind::Barrel, 22.6, 4.0, 0.0),
        (PropKind::Crate, 22.6, -9.1, 0.2),
        (PropKind::Barrel, 22.8, -3.9, 0.9),
    ] {
        put(props, kind, x, 0.0, z, yaw);
        let r = if kind == PropKind::CrateStack { 0.85 } else if kind == PropKind::Crate { 0.5 } else { 0.35 };
        v.push(bx(x - r, 0.0, z - r, x + r, if kind == PropKind::CrateStack { 1.6 } else { 0.9 }, z + r, Mat::Hidden));
    }
    // banners on the Gallery's wall and under the pits' walkway
    for z in [-18.0f32, -2.0, 12.0] {
        put(props, PropKind::BannerB, 23.98, 5.4, z, -hp);
    }
    for x in [-19.5f32, -14.0, 14.0, 19.5] {
        put(props, PropKind::BannerA, x, 4.3, 20.98, std::f32::consts::PI);
    }
    // the plaza's lamp posts glow warm (their lanterns light the paving round them)
    for (x, z) in [(-12.0f32, -12.0f32), (12.0, -12.0), (-12.0, 12.0), (12.0, 12.0), (-18.0, 0.0), (18.0, 0.0)] {
        torches.push(v3(x, 3.1, z));
    }
}

/// Bunting: strings of pennants across the square, (from, to) at their fixing points. They sag
/// in the middle (the renderer hangs them, crate::gpu).
pub const BUNTING: [(V3, V3); 8] = [
    (V3 { x: -12.0, y: 3.45, z: -12.0 }, V3 { x: 12.0, y: 3.45, z: -12.0 }),
    (V3 { x: -12.0, y: 3.45, z: 12.0 }, V3 { x: 12.0, y: 3.45, z: 12.0 }),
    (V3 { x: -12.0, y: 3.45, z: -12.0 }, V3 { x: -12.0, y: 3.45, z: 12.0 }),
    (V3 { x: 12.0, y: 3.45, z: -12.0 }, V3 { x: 12.0, y: 3.45, z: 12.0 }),
    (V3 { x: 12.0, y: 3.45, z: -12.0 }, V3 { x: 24.0, y: 5.6, z: -14.0 }),
    (V3 { x: -12.0, y: 3.45, z: -12.0 }, V3 { x: -14.0, y: 4.2, z: -29.7 }),
    (V3 { x: -12.0, y: 3.45, z: 12.0 }, V3 { x: -14.0, y: 4.4, z: 20.9 }),
    (V3 { x: 12.0, y: 3.45, z: 12.0 }, V3 { x: 14.0, y: 4.4, z: 20.9 }),
];

/// Where a point is in Varrow (the minimap's callouts).
pub fn place(p: V3) -> &'static str {
    let in_pit = PITS.iter().position(|q| p.x >= q.x.0 && p.x <= q.x.1 && p.z >= q.z.0 && p.z <= q.z.1);
    match in_pit {
        Some(0) => "West Court Pit",
        Some(_) => "East Court Pit",
        None if p.x >= 24.0 => "The Gallery",
        None if p.x <= -20.5 && p.z > -12.5 && p.z < 8.5 => "Tables Pavilion",
        None if p.z <= -28.0 && p.x < -1.5 && p.x > -14.5 => "The Tailor's",
        None if p.z <= -17.5 && p.x > 2.5 && p.x < 9.5 => "The Catwalk",
        None if p.z >= 14.0 && p.y > 3.0 => "The Terrace",
        None if p.z >= 14.0 => "Challenge Boards",
        None if p.x.abs() < 5.0 && p.z.abs() < 5.0 => "The Fountain",
        _ => "Varrow Centre",
    }
}
