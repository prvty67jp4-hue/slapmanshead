//! The Ashen Vault: the Ashen Court's sealed vault beneath the Undercroft, where Writs are
//! forged. It lies under Varrow Centre (the same world as the hub, 40 m down: nothing above
//! reaches it), five boss arenas in a row joined by short corridors (crate::raid runs it):
//!
//! 1. the Wax Hall (the Warden of Wax): pressure plates in the corners;
//! 2. the Assessors' Court (the Twin Assessors): a dais at each end;
//! 3. the Ledger Stacks (the Chained Ledger): shelves round the walls;
//! 4. the Regent's Roof (the Ashblood Regent): the vault's open rooftop at night under a red
//!    moon, low parapets, four chimney stacks to hide behind;
//! 5. the Magistrate's Pit (the Ashen Magistrate): a whole floor over a fire pit that breaks
//!    into five islands, with walls between them to wall-run along.
//!
//! The Vault Gate (where a party gathers) stands in the hub's south-west corner.

use crate::math::*;
use crate::world::{Block, Mat, Prop, PropKind};

/// The vault's floor height.
pub const VY: f32 = -40.0;
/// Rooms are this tall.
pub const ROOM_H: f32 = 9.0;
/// Anything below this has fallen into the Magistrate's fire.
pub const FIRE_Y: f32 = VY - 2.5;

/// The Vault Gate in the hub: the circle a party stands in (centre, radius).
pub const GATE: (f32, f32, f32) = (-28.0, -29.0, 2.6);
/// The raid's leaderboard on the wall beside it (position, facing).
pub const GATE_BOARD: (f32, f32, f32) = (-35.94, 2.4, -24.6);

#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub name: &'static str,
    pub x: (f32, f32),
    pub z: (f32, f32),
    /// Where the party comes in (position on the floor, facing).
    pub entry: (f32, f32, f32),
    /// The corridor on to the next room (x and z ranges), and the line read in it.
    pub out: Option<((f32, f32), (f32, f32))>,
    pub lore: &'static str,
}

impl Room {
    pub fn contains(&self, p: V3, margin: f32) -> bool {
        p.x >= self.x.0 - margin && p.x <= self.x.1 + margin && p.z >= self.z.0 - margin && p.z <= self.z.1 + margin && p.y < VY + ROOM_H && p.y > VY - 8.0
    }
    pub fn centre(&self) -> V3 {
        v3((self.x.0 + self.x.1) * 0.5, VY, (self.z.0 + self.z.1) * 0.5)
    }
    pub fn entry_pos(&self) -> V3 {
        v3(self.entry.0, VY, self.entry.1)
    }
}

pub const ROOMS: [Room; 5] = [
    Room {
        name: "The Wax Hall",
        x: (-35.0, -15.0),
        z: (-35.0, -15.0),
        entry: (-32.5, -25.0, std::f32::consts::FRAC_PI_2),
        out: Some(((-15.0, -9.0), (-27.0, -23.0))),
        lore: "\"The Court's candles never gutter. The Warden saw to that, and to the thieves.\"",
    },
    Room {
        name: "The Assessors' Court",
        x: (-9.0, 11.0),
        z: (-35.0, -15.0),
        entry: (-7.0, -25.0, std::f32::consts::FRAC_PI_2),
        out: Some(((11.0, 17.0), (-27.0, -23.0))),
        lore: "\"Every debt is weighed twice. Neither Assessor trusts the other's scale.\"",
    },
    Room {
        name: "The Ledger Stacks",
        x: (17.0, 35.0),
        z: (-35.0, -15.0),
        entry: (19.0, -25.0, std::f32::consts::FRAC_PI_2),
        out: Some(((24.0, 28.0), (-15.0, -9.0))),
        lore: "\"Every name the Court ever bound is written here, in ink that remembers.\"",
    },
    Room {
        name: "The Regent's Roof",
        x: (15.0, 35.0),
        z: (-9.0, 11.0),
        entry: (25.0, -7.0, 0.0),
        out: Some(((9.0, 15.0), (-1.0, 3.0))),
        lore: "\"Lord Vesper Kael kept the roof, and the red moon kept him. Neither has slept since.\"",
    },
    Room {
        name: "The Magistrate's Pit",
        x: (-29.0, 8.0),
        z: (-9.0, 27.0),
        entry: (6.0, 1.0, -std::f32::consts::FRAC_PI_2),
        out: None,
        lore: "\"Here the Writs are forged, and here the Ashen Magistrate passes sentence.\"",
    },
];

/// The Wax Hall's pressure plates (centre; radius PLATE_R).
pub const PLATES: [(f32, f32); 4] = [(-31.0, -31.0), (-19.0, -31.0), (-31.0, -19.0), (-19.0, -19.0)];
pub const PLATE_R: f32 = 1.3;
/// The Assessors' daises (centre of each).
pub const DAISES: [(f32, f32); 2] = [(-5.0, -25.0), (7.0, -25.0)];
/// The Regent's Roof: its chimney stacks (centre; CHIMNEY_HALF), cover from his spikes.
pub const CHIMNEYS: [(f32, f32); 4] = [(18.6, -5.4), (31.4, -5.4), (18.6, 7.4), (31.4, 7.4)];
pub const CHIMNEY_HALF: f32 = 0.75;
/// The roof's parapet height.
pub const PARAPET: f32 = 0.9;
/// The roof's middle.
pub const ROOF_CENTRE: (f32, f32) = (25.0, 1.0);

/// The Magistrate's Pit: its five islands (centre; half-size ISLAND_HALF), the walls to run
/// along between them (x0, z0, x1, z1), and the breakable floor over the fire (x0, z0, x1, z1).
pub const PIT_CENTRE: (f32, f32) = (-10.5, 9.0);
pub const ISLAND_HALF: f32 = 3.2;
pub const ISLANDS: [(f32, f32); 5] = [(-10.5, 9.0), (1.5, 9.0), (-22.5, 9.0), (-10.5, 21.0), (-10.5, -3.0)];
pub const RUN_WALLS: [(f32, f32, f32, f32); 4] = [
    (-7.3, 12.9, -1.7, 13.4),
    (-19.3, 4.6, -13.7, 5.1),
    (-14.4, 12.2, -13.9, 17.8),
    (-7.1, 0.2, -6.6, 5.8),
];
pub const FLOOR_TILES: [(f32, f32, f32, f32); 12] = [
    (-29.0, -9.0, 8.0, -6.2),
    (-29.0, -6.2, -13.7, 0.2),
    (-7.3, -6.2, 8.0, 0.2),
    (-29.0, 0.2, 8.0, 5.8),
    (-29.0, 5.8, -25.7, 12.2),
    (-19.3, 5.8, -13.7, 12.2),
    (-7.3, 5.8, -1.7, 12.2),
    (4.7, 5.8, 8.0, 12.2),
    (-29.0, 12.2, 8.0, 17.8),
    (-29.0, 17.8, -13.7, 24.2),
    (-7.3, 17.8, 8.0, 24.2),
    (-29.0, 24.2, 8.0, 27.0),
];

/// Where a broken floor tile's block is kept while the floor is down (the level's block count
/// never changes, so its mesh isn't rebuilt).
pub const TILE_AWAY: f32 = -200.0;

fn bx(x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32, mat: Mat) -> Block {
    Block { b: Aabb::new(v3(x0.min(x1), y0.min(y1), z0.min(z1)), v3(x0.max(x1), y0.max(y1), z0.max(z1))), mat }
}

/// One side of a room's walls, with gaps (along that side) for its doors.
fn wall_side(v: &mut Vec<Block>, fixed: f32, along: (f32, f32), gaps: &[(f32, f32)], x_side: bool, out: f32) {
    let mut a = along.0 - 1.0;
    let mut cuts: Vec<(f32, f32)> = gaps.to_vec();
    cuts.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut push = |s: f32, e: f32| {
        if e - s > 0.01 {
            let (w0, w1) = if out > 0.0 { (fixed, fixed + 1.0) } else { (fixed - 1.0, fixed) };
            if x_side {
                v.push(bx(w0, VY, s, w1, VY + ROOM_H, e, Mat::Wall));
            } else {
                v.push(bx(s, VY, w0, e, VY + ROOM_H, w1, Mat::Wall));
            }
        }
    };
    for (g0, g1) in cuts {
        push(a, g0);
        a = g1;
    }
    push(a, along.1 + 1.0);
}

/// The vault's blocks, its torches and props, added to Varrow Centre's world. The Magistrate's
/// floor tiles are the last FLOOR_TILES.len() blocks pushed (crate::raid moves them).
pub fn build(v: &mut Vec<Block>, deco: &mut Vec<Block>, props: &mut Vec<Prop>, torches: &mut Vec<V3>) {
    // doors (on which side of which room, along which range)
    let doors: [[Vec<(f32, f32)>; 4]; 5] = [
        [vec![], vec![(-27.0, -23.0)], vec![], vec![]],
        [vec![(-27.0, -23.0)], vec![(-27.0, -23.0)], vec![], vec![]],
        [vec![(-27.0, -23.0)], vec![], vec![], vec![(24.0, 28.0)]],
        [vec![(-1.0, 3.0)], vec![], vec![(24.0, 28.0)], vec![]],
        [vec![], vec![(-1.0, 3.0)], vec![], vec![]],
    ];
    for (k, r) in ROOMS.iter().enumerate() {
        let (x0, x1, z0, z1) = (r.x.0, r.x.1, r.z.0, r.z.1);
        if k == 3 {
            // the roof: open to the night, a parapet round it (gaps where the stairs come up and
            // where the way on goes down), a stair-head on the south side
            for (x0_, z0_, x1_, z1_) in [(x0 - 0.5, z0 - 0.5, 23.5, z0), (28.5, z0 - 0.5, x1 + 0.5, z0), (x0 - 0.5, z1, x1 + 0.5, z1 + 0.5), (x1, z0, x1 + 0.5, z1), (x0 - 0.5, z0, x0, -1.0), (x0 - 0.5, 3.0, x0, z1)] {
                v.push(bx(x0_, VY, z0_, x1_, VY + PARAPET, z1_, Mat::Stone));
                // (an invisible rail above it: nobody walks off the roof)
                v.push(bx(x0_, VY + PARAPET, z0_, x1_, VY + 3.0, z1_, Mat::Hidden));
            }
            deco.push(bx(23.4, VY + 3.2, z0 - 1.9, 28.6, VY + 3.6, z0 - 0.3, Mat::Trim));
            v.push(bx(x0, VY - 1.0, z0, x1, VY, z1, Mat::Stone));
            for (x, z) in CHIMNEYS {
                v.push(bx(x - CHIMNEY_HALF, VY, z - CHIMNEY_HALF, x + CHIMNEY_HALF, VY + 2.6, z + CHIMNEY_HALF, Mat::Wall));
                deco.push(bx(x - CHIMNEY_HALF - 0.12, VY + 2.6, z - CHIMNEY_HALF - 0.12, x + CHIMNEY_HALF + 0.12, VY + 2.85, z + CHIMNEY_HALF + 0.12, Mat::Trim));
            }
            // red lanterns on posts at the corners
            for (x, z) in [(x0 + 0.6, z0 + 0.6), (x1 - 0.6, z0 + 0.6), (x0 + 0.6, z1 - 0.6), (x1 - 0.6, z1 - 0.6)] {
                props.push(Prop { kind: PropKind::HubLamp, pos: v3(x, VY, z), yaw: 0.0, scale: 0.7, team: 255 });
            }
            if let Some(((cx0, cx1), (cz0, cz1))) = r.out {
                v.push(bx(cx0, VY - 1.0, cz0, cx1, VY, cz1, Mat::Trim));
                deco.push(bx(cx0, VY + 4.5, cz0, cx1, VY + 5.0, cz1, Mat::Stone));
                v.push(bx(cx0, VY, cz0 - 1.0, cx1, VY + 4.5, cz0, Mat::Wall));
                v.push(bx(cx0, VY, cz1, cx1, VY + 4.5, cz1 + 1.0, Mat::Wall));
            }
            continue;
        }
        wall_side(v, x0, (z0, z1), &doors[k][0], true, -1.0);
        wall_side(v, x1, (z0, z1), &doors[k][1], true, 1.0);
        wall_side(v, z0, (x0, x1), &doors[k][2], false, -1.0);
        wall_side(v, z1, (x0, x1), &doors[k][3], false, 1.0);
        deco.push(bx(x0 - 1.0, VY + ROOM_H, z0 - 1.0, x1 + 1.0, VY + ROOM_H + 0.8, z1 + 1.0, Mat::Stone));
        if k < 4 {
            v.push(bx(x0, VY - 1.0, z0, x1, VY, z1, Mat::Stone));
        }
        // a torch on each wall, halfway along
        for (p, out) in [(v3(x0 + 0.05, VY + 3.2, (z0 + z1) * 0.5 + 3.0), v3(1.0, 0.0, 0.0)), (v3(x1 - 0.05, VY + 3.2, (z0 + z1) * 0.5 - 3.0), v3(-1.0, 0.0, 0.0)), (v3((x0 + x1) * 0.5 - 4.0, VY + 3.2, z0 + 0.05), v3(0.0, 0.0, 1.0)), (v3((x0 + x1) * 0.5 + 4.0, VY + 3.2, z1 - 0.05), v3(0.0, 0.0, -1.0))] {
            props.push(Prop { kind: PropKind::Lantern, pos: p, yaw: out.x.atan2(out.z), scale: 1.0, team: 255 });
            torches.push(p + out * 0.32 - v3(0.0, 0.32, 0.0));
        }
        // the corridor on
        if let Some(((cx0, cx1), (cz0, cz1))) = r.out {
            v.push(bx(cx0, VY - 1.0, cz0, cx1, VY, cz1, Mat::Trim));
            deco.push(bx(cx0, VY + 4.5, cz0, cx1, VY + 5.0, cz1, Mat::Stone));
            if cx1 - cx0 > cz1 - cz0 {
                v.push(bx(cx0, VY, cz0 - 1.0, cx1, VY + 4.5, cz0, Mat::Wall));
                v.push(bx(cx0, VY, cz1, cx1, VY + 4.5, cz1 + 1.0, Mat::Wall));
            } else {
                v.push(bx(cx0 - 1.0, VY, cz0, cx0, VY + 4.5, cz1, Mat::Wall));
                v.push(bx(cx1, VY, cz0, cx1 + 1.0, VY + 4.5, cz1, Mat::Wall));
            }
            let mid = v3((cx0 + cx1) * 0.5, VY + 3.0, (cz0 + cz1) * 0.5);
            torches.push(mid);
        }
    }
    // the Wax Hall: candle columns round the plates
    for (x, z) in PLATES {
        for (dx, dz) in [(-2.4f32, 0.0f32), (2.4, 0.0)] {
            deco.push(bx(x + dx - 0.3, VY, z + dz - 0.3, x + dx + 0.3, VY + 1.4, z + dz + 0.3, Mat::Stone));
        }
    }
    // the Assessors' daises
    for (x, z) in DAISES {
        v.push(bx(x - 2.0, VY, z - 2.0, x + 2.0, VY + 0.3, z + 2.0, Mat::Stone));
    }
    // the Ledger Stacks: shelves along the north and south walls
    for z in [-34.2f32, -16.6] {
        for x in [19.0f32, 25.0, 31.0] {
            v.push(bx(x - 2.2, VY, z - 0.4, x + 2.2, VY + 2.8, z + 0.4, Mat::Trim));
        }
    }
    props.push(Prop { kind: PropKind::Barrel, pos: v3(33.5, VY, -33.5), yaw: 0.4, scale: 1.0, team: 255 });
    props.push(Prop { kind: PropKind::Crate, pos: v3(33.4, VY, -16.8), yaw: 0.2, scale: 1.0, team: 255 });
    // the Magistrate's Pit: the fire pit's floor, the five islands, the walls between them, the
    // judge's bench, and (last) the floor over it all
    let r = ROOMS[4];
    v.push(bx(r.x.0, VY - 5.0, r.z.0, r.x.1, VY - 4.0, r.z.1, Mat::Metal));
    for (x, z) in ISLANDS {
        v.push(bx(x - ISLAND_HALF, VY - 4.0, z - ISLAND_HALF, x + ISLAND_HALF, VY, z + ISLAND_HALF, Mat::Stone));
    }
    for (x0, z0, x1, z1) in RUN_WALLS {
        v.push(bx(x0, VY - 4.0, z0, x1, VY + 4.5, z1, Mat::Wall));
    }
    deco.push(bx(-16.0, VY, 26.2, -5.0, VY + 2.4, 27.0, Mat::Trim));
    for x in [-26.0f32, 5.0] {
        props.push(Prop { kind: PropKind::BannerA, pos: v3(x, VY + 7.0, 26.98), yaw: std::f32::consts::PI, scale: 1.0, team: 255 });
    }
    for (x0, z0, x1, z1) in FLOOR_TILES {
        v.push(bx(x0, VY - 1.0, z0, x1, VY, z1, Mat::Hidden));
    }
}

/// Which room a point is in (None: the corridors, or not in the vault).
pub fn room_at(p: V3) -> Option<usize> {
    ROOMS.iter().position(|r| r.contains(p, 0.0))
}

/// Whether a point is down in the vault at all.
pub fn in_vault(p: V3) -> bool {
    p.y < VY + ROOM_H + 1.0 && p.y > VY - 30.0
}
