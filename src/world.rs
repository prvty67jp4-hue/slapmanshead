use crate::math::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mat {
    Floor,
    Wall,
    Stone,
    Metal,
    Crate,
    Trim,
    /// Collides, but isn't drawn: a prop shows it (a bridge railing, an archway's pillars, a
    /// crane's mast).
    Hidden,
}

#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub b: Aabb,
    pub mat: Mat,
}

/// The maps a match can be played on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapId {
    /// 38 x 38 m: a clear centre, three lanes from each spawn, wall-run corridors, three levels.
    Arena = 0,
    /// The original wide 48 x 48 m courtyard.
    Courtyard = 1,
    /// 30 x 30 m, tight and vertical: narrow corridors, lots of cover, dash gaps.
    CloseQuarters = 2,
    /// 60 x 60 m, 4v4 team map at dusk: two mirrored bases across a river, three lanes (a long
    /// riverside quay, a stone bridge and plaza, covered workshops), three height levels.
    Bridgeworks = 3,
}

impl MapId {
    pub const ALL: [MapId; 4] = [MapId::Arena, MapId::Courtyard, MapId::CloseQuarters, MapId::Bridgeworks];
    pub fn from_u8(v: u8) -> MapId {
        match v {
            1 => MapId::Courtyard,
            2 => MapId::CloseQuarters,
            3 => MapId::Bridgeworks,
            _ => MapId::Arena,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            MapId::Arena => "Arena",
            MapId::Courtyard => "Open Courtyard",
            MapId::CloseQuarters => "Close Quarters",
            MapId::Bridgeworks => "Varrow Bridgeworks",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            MapId::Arena => "a clear centre with a tower, three lanes from each spawn, wall-run corridors and balconies",
            MapId::Courtyard => "big and open: long sight lines around a central dais",
            MapId::CloseQuarters => "small and vertical: tight wall-run corridors, three levels, gaps to dash across",
            MapId::Bridgeworks => "4v4 at dusk: two bases across a river, a sniper quay, a stone bridge plaza and covered workshops",
        }
    }
    /// The team map is for team modes only; the duel maps suit every mode.
    pub fn fits(self, teams: bool) -> bool {
        teams || self != MapId::Bridgeworks
    }
    /// What a mode starts on: the team map for team modes, the Arena otherwise.
    pub fn default_for(teams: bool) -> MapId {
        if teams { MapId::Bridgeworks } else { MapId::Arena }
    }
    /// The maps a mode can be played on.
    pub fn for_mode(teams: bool) -> Vec<MapId> {
        MapId::ALL.iter().copied().filter(|m| m.fits(teams)).collect()
    }
}

/// Set dressing: CC0 Quaternius models (see assets/CREDITS.txt). Props are looks only (nothing
/// collides with them) and sit where nobody runs: tucked into corners, against walls, or high on
/// them (the placement test checks this).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    BannerA,
    BannerB,
    Barrel,
    WeaponStand,
    Crate,
    Rubble,
    Shield,
    Torch,
    RockA,
    RockB,
    Plant,
    // Varrow Bridgeworks' own pieces (made by tools/blender/build_props.py)
    /// A bridge railing, 3 m long (stands on a hidden railing block).
    Railing,
    /// A stone archway 3.6 m wide (its pillars are hidden blocks).
    Archway,
    /// A dock crane: mast, jib, pulley and hook (on a stone plinth; its mast is a hidden block).
    Crane,
    /// Three strapped cargo crates.
    CrateStack,
    /// A lantern on a wall bracket (it lights the wall: see World::torches).
    Lantern,
}

impl PropKind {
    /// Pieces that are part of the structure (railings, archways, a crane): they stand on or
    /// round the blocks that carry them, out in the open.
    #[cfg(test)]
    pub fn structural(self) -> bool {
        matches!(self, PropKind::Railing | PropKind::Archway | PropKind::Crane)
    }
    /// Rough footprint radius on the ground at scale 1 (0 for things hung on walls).
    #[cfg(test)]
    pub fn radius(self) -> f32 {
        match self {
            PropKind::Barrel => 0.36,
            PropKind::WeaponStand => 0.5, // 1.4 m wide but 1 m deep: it stands with its back to a wall
            PropKind::Crate => 0.58,
            PropKind::Rubble => 0.42,
            PropKind::RockA | PropKind::RockB => 1.8,
            PropKind::Plant => 0.5,
            PropKind::CrateStack => 0.9,
            PropKind::Railing | PropKind::Archway | PropKind::Crane => 0.5,
            PropKind::BannerA | PropKind::BannerB | PropKind::Shield | PropKind::Torch | PropKind::Lantern => 0.0,
        }
    }
    #[cfg(test)]
    pub fn on_wall(self) -> bool {
        self.radius() == 0.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Prop {
    pub kind: PropKind,
    /// Where its origin goes (on the floor for floor props, on the wall face for wall props).
    pub pos: V3,
    /// Turn about the vertical: the model's front (+z) faces yaw_fwd(yaw).
    pub yaw: f32,
    pub scale: f32,
    /// Banner colour: 0 red, 1 blue, 255 its own.
    pub team: u8,
}

/// Ivy hanging down a wall from `top` (on the wall face), `out` the wall's facing, `len` metres.
#[derive(Clone, Copy, Debug)]
pub struct Vine {
    pub top: V3,
    pub out: V3,
    pub len: f32,
    pub seed: u32,
}

pub struct World {
    pub blocks: Vec<Block>,
    /// Spawn points (position, facing). Red team uses z < 0, blue team z > 0.
    pub spawn_points: Vec<(V3, f32)>,
    pub half: f32,
    pub deco: Vec<Block>,
    pub braziers: Vec<V3>,
    pub props: Vec<Prop>,
    pub vines: Vec<Vine>,
    /// Wall torches: where their flames burn (they light the walls around them).
    pub torches: Vec<V3>,
    pub map: MapId,
    /// Capture-the-flag bases (red, blue).
    pub flags: [V3; 2],
    /// Water surfaces (flat boxes: the top is the surface): drawn only, nothing collides.
    pub water: Vec<Aabb>,
}

fn bx(x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32, mat: Mat) -> Block {
    Block { b: Aabb::new(v3(x0.min(x1), y0.min(y1), z0.min(z1)), v3(x0.max(x1), y0.max(y1), z0.max(z1))), mat }
}

/// Add a box and its mirror through the arena centre (point symmetric, so both spawns are fair).
fn sym(v: &mut Vec<Block>, b: Block) {
    v.push(b);
    let m = Block {
        b: Aabb::new(v3(-b.b.max.x, b.b.min.y, -b.b.max.z), v3(-b.b.min.x, b.b.max.y, -b.b.min.z)),
        mat: b.mat,
    };
    v.push(m);
}

/// Spawn points from one team's half, mirrored for the other, each facing the centre.
fn mirrored_spawns(half: &[(f32, f32, f32)]) -> Vec<(V3, f32)> {
    let mut s = Vec::new();
    for (x, y, z) in half.iter().copied().chain(half.iter().map(|&(x, y, z)| (-x, y, -z))) {
        s.push((v3(x, y, z), (-x).atan2(-z)));
    }
    s
}

/// Outer walls with battlements on top, pilasters along them and banners (and the floor).
fn enclosure(v: &mut Vec<Block>, h: f32, wall_h: f32, banner_y: f32, banner_gap: f32) -> (Vec<Block>, Vec<(V3, V3, u8)>) {
    v.push(bx(-h, -1.0, -h, h, 0.0, h, Mat::Floor));
    walls(v, h, wall_h, banner_y, banner_gap)
}

/// The outer walls only (a map that lays its own floor).
fn walls(v: &mut Vec<Block>, h: f32, wall_h: f32, banner_y: f32, banner_gap: f32) -> (Vec<Block>, Vec<(V3, V3, u8)>) {
    v.push(bx(-h - 1.0, 0.0, -h - 1.0, h + 1.0, wall_h, -h, Mat::Wall));
    v.push(bx(-h - 1.0, 0.0, h, h + 1.0, wall_h, h + 1.0, Mat::Wall));
    v.push(bx(-h - 1.0, 0.0, -h, -h, wall_h, h, Mat::Wall));
    v.push(bx(h, 0.0, -h, h + 1.0, wall_h, h, Mat::Wall));
    let mut k = -h + 4.0;
    while k < h - 3.0 {
        if k.abs() > 2.5 {
            v.push(bx(k - 0.5, 0.0, -h, k + 0.5, wall_h, -h + 0.35, Mat::Stone));
            v.push(bx(k - 0.5, 0.0, h - 0.35, k + 0.5, wall_h, h, Mat::Stone));
            v.push(bx(-h, 0.0, k - 0.5, -h + 0.35, wall_h, k + 0.5, Mat::Stone));
            v.push(bx(h - 0.35, 0.0, k - 0.5, h, wall_h, k + 0.5, Mat::Stone));
        }
        k += 6.0;
    }
    // battlements on top of the outer walls (looks only)
    let mut deco = Vec::new();
    let mut m = -h - 1.0;
    while m < h + 1.0 {
        deco.push(bx(m, wall_h, -h - 1.0, m + 1.0, wall_h + 1.0, -h - 0.2, Mat::Wall));
        deco.push(bx(m, wall_h, h + 0.2, m + 1.0, wall_h + 1.0, h + 1.0, Mat::Wall));
        deco.push(bx(-h - 1.0, wall_h, m, -h - 0.2, wall_h + 1.0, m + 1.0, Mat::Wall));
        deco.push(bx(h + 0.2, wall_h, m, h + 1.0, wall_h + 1.0, m + 1.0, Mat::Wall));
        m += 2.0;
    }
    // banners hanging on the walls: (centre, facing normal, team)
    let mut banners = Vec::new();
    let mut b = -h + banner_gap * 0.6;
    while b < h - 4.0 {
        banners.push((v3(b, banner_y, -h + 0.37), v3(0.0, 0.0, 1.0), 0u8));
        banners.push((v3(b, banner_y, h - 0.37), v3(0.0, 0.0, -1.0), 1u8));
        b += banner_gap;
    }
    (deco, banners)
}

/// A prop and its mirror through the centre (the maps are point symmetric).
fn sym_prop(v: &mut Vec<Prop>, kind: PropKind, x: f32, y: f32, z: f32, yaw: f32, scale: f32, team: u8) {
    v.push(Prop { kind, pos: v3(x, y, z), yaw, scale, team });
    let other = if team < 2 { 1 - team } else { team };
    v.push(Prop { kind, pos: v3(-x, y, -z), yaw: yaw + std::f32::consts::PI, scale, team: other });
}

/// A vine and its mirror.
fn sym_vine(v: &mut Vec<Vine>, top: V3, out: V3, len: f32, seed: u32) {
    v.push(Vine { top, out, len, seed });
    v.push(Vine { top: v3(-top.x, top.y, -top.z), out: v3(-out.x, out.y, -out.z), len, seed: seed.wrapping_mul(7919) + 1 });
}

/// The banners along the walls become Quaternius banners (red on one side, blue on the other).
fn banner_props(banners: &[(V3, V3, u8)], wall_inset: f32) -> Vec<Prop> {
    banners
        .iter()
        .enumerate()
        .map(|(k, (c, n, team))| Prop {
            kind: if k % 4 < 2 { PropKind::BannerA } else { PropKind::BannerB },
            // on the wall face, the bar about 0.7 m over the old banner's middle
            pos: *c - *n * wall_inset + v3(0.0, 0.7, 0.0),
            yaw: n.x.atan2(n.z),
            scale: 1.0,
            team: *team,
        })
        .collect()
}

/// A tidy corner: two barrels, a crate, a little rubble and a plant, tucked against the two walls
/// that meet at (cx, cz) (sx, sz: which way is into the map from it).
fn corner_props(v: &mut Vec<Prop>, cx: f32, cz: f32, sx: f32, sz: f32) {
    let at = |dx: f32, dz: f32| (cx + sx * dx, cz + sz * dz);
    let (x, z) = at(0.62, 0.62);
    sym_prop(v, PropKind::Crate, x, 0.0, z, 0.15, 1.0, 255);
    let (x, z) = at(1.65, 0.42);
    sym_prop(v, PropKind::Barrel, x, 0.0, z, 0.0, 1.0, 255);
    let (x, z) = at(0.42, 1.65);
    sym_prop(v, PropKind::Barrel, x, 0.0, z, 1.1, 1.0, 255);
    let (x, z) = at(1.3, 1.3);
    sym_prop(v, PropKind::Rubble, x, 0.0, z, 0.7, 1.0, 255);
    let (x, z) = at(2.4, 0.5);
    sym_prop(v, PropKind::Plant, x, 0.0, z, 0.3, 1.0, 255);
}

/// Steps (0.5 m each, climbed just by walking) from the ground up to `top`, `n` of them, each
/// `depth` deep, running from `edge` (where the top step meets a platform) in direction `dir`
/// (+1/-1) along z, across x0..x1.
fn stairs_z(v: &mut Vec<Block>, x0: f32, x1: f32, edge: f32, dir: f32, n: usize, depth: f32, mat: Mat) {
    for i in 1..=n {
        let near = edge + dir * (n - i) as f32 * depth;
        let far = edge + dir * (n - i + 1) as f32 * depth;
        sym(v, bx(x0, 0.0, near, x1, 0.5 * i as f32, far, mat));
    }
}

impl World {
    pub fn build(map: MapId) -> World {
        match map {
            MapId::Arena => World::arena(),
            MapId::Courtyard => World::courtyard(),
            MapId::CloseQuarters => World::close_quarters(),
            MapId::Bridgeworks => World::bridgeworks(),
        }
    }

    /// The main arena: 38 x 38 m. A clear 14 m centre holding only a tower and the pillar pair
    /// beside it; from each spawn three lanes (left, middle, right) lead there between two big
    /// platforms; 5 m wall-run corridors down both sides; balconies over the spawns; the
    /// platforms sit 6.5 m apart (a dash or a wall-jump crosses). Point symmetric.
    pub fn arena() -> World {
        let h = 19.0; // half size
        let wall_h = 9.0;
        let lvl2 = 3.0;
        let mut v = Vec::new();
        let (deco, banners) = enclosure(&mut v, h, wall_h, 7.6, 12.0);

        // top level: high ledges along the back walls
        v.push(bx(-h, 5.0, -h, h, 5.5, -h + 1.2, Mat::Trim));
        v.push(bx(-h, 5.0, h - 1.2, h, 5.5, h, Mat::Trim));

        // ---- balconies (level 2) over the spawns, on a few big supports
        sym(&mut v, bx(-13.0, 2.6, -h, 13.0, lvl2, -15.5, Mat::Stone));
        sym(&mut v, bx(-5.3, 0.0, -16.1, -3.7, 2.6, -15.5, Mat::Stone));
        sym(&mut v, bx(3.7, 0.0, -16.1, 5.3, 2.6, -15.5, Mat::Stone));
        // stairs up from the left lane, alongside the corridor wall
        stairs_z(&mut v, -13.0, -11.4, -15.5, 1.0, 5, 0.6, Mat::Stone);

        // ---- side corridors: 5 m between the outer wall and a tall inner wall, an 8 m doorway
        // into the middle
        sym(&mut v, bx(-13.6, 0.0, -15.5, -13.0, 5.0, -4.0, Mat::Wall));
        sym(&mut v, bx(-13.6, 0.0, 4.0, -13.0, 5.0, 15.5, Mat::Wall));

        // ---- two big platforms (level 2) split each half into three lanes; 6.5 m apart
        sym(&mut v, bx(-8.0, 0.0, -13.0, -3.25, lvl2, -7.5, Mat::Stone));
        sym(&mut v, bx(3.25, 0.0, -13.0, 8.0, lvl2, -7.5, Mat::Stone));
        // a bridge from the balcony out to the left platform (walk under it)
        sym(&mut v, bx(-6.3, 2.6, -15.5, -5.2, lvl2, -13.0, Mat::Trim));
        // steps up onto the right platform from the centre side
        stairs_z(&mut v, 6.5, 8.0, -7.5, 1.0, 5, 0.5, Mat::Stone);

        // ---- the centre: a 5 m tower with low ledges front and back, and a tall pillar on each
        // side 2.5 m from it (wall-jump between pillar and tower)
        v.push(bx(-1.5, 0.0, -1.5, 1.5, 5.0, 1.5, Mat::Stone));
        sym(&mut v, bx(-1.5, 0.0, -2.3, 1.5, 2.5, -1.5, Mat::Trim));
        sym(&mut v, bx(4.0, 0.0, -0.5, 5.0, 7.0, 0.5, Mat::Metal));

        // braziers on the tower and the balcony back corners
        let mut braziers = Vec::new();
        for (x, y, z) in [(-1.1f32, 5.0f32, -1.1f32), (1.1, 5.0, 1.1), (-12.6, lvl2, -18.4), (12.6, lvl2, -18.4), (-12.6, lvl2, 18.4), (12.6, lvl2, 18.4)] {
            v.push(bx(x - 0.25, y, z - 0.25, x + 0.25, y + 0.7, z + 0.25, Mat::Metal));
            braziers.push(v3(x, y + 0.75, z));
        }

        // set dressing
        use PropKind::*;
        let mut props = banner_props(&banners, 0.36);
        corner_props(&mut props, -h, -h, 1.0, 1.0);
        corner_props(&mut props, h, -h, -1.0, 1.0);
        // a weapon stand with two shields over it, under each balcony
        sym_prop(&mut props, WeaponStand, -6.5, 0.0, -h + 0.52, 0.0, 1.0, 255);
        sym_prop(&mut props, Shield, -7.2, 1.85, -h, 0.0, 1.0, 255);
        sym_prop(&mut props, Shield, -5.8, 1.85, -h, 0.0, 1.0, 255);
        // rubble at the tower's foot
        sym_prop(&mut props, Rubble, 2.05, 0.0, -2.1, 2.2, 0.9, 255);
        // torches high on the corridor walls
        let mut torches = Vec::new();
        for z in [-8.0f32, 8.0] {
            sym_prop(&mut props, Torch, -h, 4.2, z, std::f32::consts::FRAC_PI_2, 1.0, 255);
            torches.push(v3(-h + 0.3, 4.62, z));
            torches.push(v3(h - 0.3, 4.62, -z));
        }
        // ivy down the walls, the balcony fronts and the high ledges
        let mut vines = Vec::new();
        for (k, (x, len)) in [(-16.5f32, 3.6f32), (-6.0, 4.2), (5.5, 3.0), (16.2, 2.6)].iter().enumerate() {
            sym_vine(&mut vines, v3(*x, wall_h, -h), v3(0.0, 0.0, 1.0), *len, k as u32 + 1);
        }
        for (k, (z, len)) in [(-12.0f32, 4.0f32), (2.0, 3.2), (12.0, 3.0)].iter().enumerate() {
            sym_vine(&mut vines, v3(-h, wall_h, *z), v3(1.0, 0.0, 0.0), *len, k as u32 + 11);
        }
        for (k, x) in [-10.0f32, 1.0, 10.5].iter().enumerate() {
            sym_vine(&mut vines, v3(*x, lvl2, -15.5), v3(0.0, 0.0, 1.0), 0.7, k as u32 + 21);
        }
        for (k, x) in [-2.0f32, 8.0].iter().enumerate() {
            sym_vine(&mut vines, v3(*x, 5.5, -h + 1.2), v3(0.0, 0.0, 1.0), 1.5, k as u32 + 31);
        }

        World {
            blocks: v,
            spawn_points: mirrored_spawns(&[(-9.5, 0.0, -17.5), (0.0, 0.0, -17.5), (9.5, 0.0, -17.5), (0.0, lvl2, -17.4), (-16.5, 0.0, -11.0), (16.5, 0.0, -8.0)]),
            half: h,
            deco,
            braziers,
            props,
            vines,
            torches,
            map: MapId::Arena,
            flags: [v3(0.0, 0.0, -17.2), v3(0.0, 0.0, 17.2)],
            water: Vec::new(),
        }
    }

    /// Close Quarters: 30 x 30 m, built for wall-running and fighting up and down. Three levels
    /// (ground, 3 m balconies and platforms, a 5 m tower and wall tops), corridors with walls on
    /// both sides, a gap only a dash or wall-jump crosses, and nowhere open more than ~8 m across.
    pub fn close_quarters() -> World {
        let h = 15.0; // half size
        let wall_h = 9.0;
        let lvl2 = 3.0;
        let mut v = Vec::new();
        let (deco, banners) = enclosure(&mut v, h, wall_h, 7.6, 10.0);

        // top level: high ledges along the back walls (wall-climb up from the balconies)
        v.push(bx(-h, 5.0, -h, h, 5.5, -h + 1.2, Mat::Trim));
        v.push(bx(-h, 5.0, h - 1.2, h, 5.5, h, Mat::Trim));

        // ---- back balconies (level 2) over the spawn alcoves
        sym(&mut v, bx(-10.4, 2.6, -h, 10.4, lvl2, -12.4, Mat::Stone));
        // its supports: the two in the middle are wide enough to run up onto the balcony
        sym(&mut v, bx(-3.2, 0.0, -12.9, -1.8, 2.6, -12.4, Mat::Stone));
        sym(&mut v, bx(1.8, 0.0, -12.9, 3.2, 2.6, -12.4, Mat::Stone));
        sym(&mut v, bx(-7.9, 0.0, -12.9, -7.1, 2.6, -12.4, Mat::Stone));
        sym(&mut v, bx(7.1, 0.0, -12.9, 7.9, 2.6, -12.4, Mat::Stone));
        // alcove walls under the balcony
        sym(&mut v, bx(-5.2, 0.0, -h, -4.8, 2.6, -13.2, Mat::Wall));
        sym(&mut v, bx(4.8, 0.0, -h, 5.2, 2.6, -13.2, Mat::Wall));
        // stairs up to the balcony, alongside the corridor wall
        stairs_z(&mut v, -10.4, -8.8, -12.4, 1.0, 5, 0.6, Mat::Stone);

        // ---- side corridors: 4 m between the outer wall and a tall inner wall, for chained
        // wall-runs; a doorway in the middle and open ends into the alcoves
        sym(&mut v, bx(-11.0, 0.0, -12.4, -10.4, 5.0, -3.0, Mat::Wall));
        sym(&mut v, bx(-11.0, 0.0, 3.0, -10.4, 5.0, 12.4, Mat::Wall));
        // half cover in each corridor (a low crate with one stacked on it)
        sym(&mut v, bx(-h, 0.0, -0.7, -13.4, 1.3, 0.7, Mat::Crate));
        sym(&mut v, bx(-h, 1.3, -0.6, -14.2, 2.5, 0.6, Mat::Crate));

        // ---- middle: platforms at level 2 (solid, so their sides are walls to run and climb)
        // P1 and P2 sit 6.5 m apart: a jump won't make it, a dash or a wall-jump will
        sym(&mut v, bx(-8.8, 0.0, -8.0, -4.5, lvl2, -5.5, Mat::Stone));
        sym(&mut v, bx(2.0, 0.0, -8.0, 6.0, lvl2, -5.5, Mat::Stone));
        // a narrow ledge from the balcony out to P1
        sym(&mut v, bx(-6.6, 0.0, -12.4, -5.6, lvl2, -8.0, Mat::Trim));
        // steps up onto P2 from the centre side
        stairs_z(&mut v, 3.0, 4.4, -5.5, 1.0, 5, 0.5, Mat::Stone);

        // ---- the centre tower (level 3, 5 m): run up its walls; a 2.5 m ledge on two sides
        v.push(bx(-1.5, 0.0, -1.5, 1.5, 5.0, 1.5, Mat::Stone));
        sym(&mut v, bx(-2.3, 0.0, -1.5, -1.5, 2.5, 1.5, Mat::Trim));

        // ---- tall pillar pairs, 2 m apart: wall-jump between them
        sym(&mut v, bx(-6.3, 0.0, -2.0, -5.3, 7.0, -1.0, Mat::Metal));
        sym(&mut v, bx(-6.3, 0.0, 1.0, -5.3, 7.0, 2.0, Mat::Metal));

        // ---- a lane between a wall and the corridor wall (2.2 m: wall-run from side to side)
        sym(&mut v, bx(7.6, 0.0, -11.0, 8.2, 4.0, -2.5, Mat::Wall));

        // ---- low cover
        sym(&mut v, bx(-2.0, 0.0, -10.6, -0.4, 1.2, -10.0, Mat::Crate));
        sym(&mut v, bx(3.4, 0.0, -1.2, 4.4, 1.0, -0.2, Mat::Crate));
        sym(&mut v, bx(-3.8, 0.0, -4.4, -2.8, 1.0, -3.4, Mat::Crate));

        // braziers on the tower and the balcony ends
        let mut braziers = Vec::new();
        for (x, y, z) in [(-1.1f32, 5.0f32, -1.1f32), (1.1, 5.0, 1.1), (-9.9, lvl2, -14.5), (9.9, lvl2, -14.5), (-9.9, lvl2, 14.5), (9.9, lvl2, 14.5)] {
            v.push(bx(x - 0.25, y, z - 0.25, x + 0.25, y + 0.7, z + 0.25, Mat::Metal));
            braziers.push(v3(x, y + 0.75, z));
        }

        // set dressing
        use PropKind::*;
        let mut props = banner_props(&banners, 0.36);
        corner_props(&mut props, -h, -h, 1.0, 1.0);
        corner_props(&mut props, h, -h, -1.0, 1.0);
        sym_prop(&mut props, Rubble, -2.75, 0.0, 1.9, 0.4, 0.9, 255);
        let mut torches = Vec::new();
        for z in [-3.6f32, 9.8] {
            sym_prop(&mut props, Torch, -h, 4.2, z, std::f32::consts::FRAC_PI_2, 1.0, 255);
            torches.push(v3(-h + 0.3, 4.62, z));
            torches.push(v3(h - 0.3, 4.62, -z));
        }
        let mut vines = Vec::new();
        for (k, (x, len)) in [(-12.5f32, 3.8f32), (-4.0, 3.2), (6.0, 4.4)].iter().enumerate() {
            sym_vine(&mut vines, v3(*x, wall_h, -h), v3(0.0, 0.0, 1.0), *len, k as u32 + 41);
        }
        for (k, (z, len)) in [(-8.0f32, 3.4f32), (3.0, 4.0)].iter().enumerate() {
            sym_vine(&mut vines, v3(-h, wall_h, *z), v3(1.0, 0.0, 0.0), *len, k as u32 + 51);
        }
        for (k, x) in [-0.5f32, 5.5].iter().enumerate() {
            sym_vine(&mut vines, v3(*x, lvl2, -12.4), v3(0.0, 0.0, 1.0), 0.7, k as u32 + 61);
        }

        World {
            blocks: v,
            spawn_points: mirrored_spawns(&[(-7.5, 0.0, -14.2), (0.0, 0.0, -14.0), (7.5, 0.0, -14.2), (0.0, lvl2, -13.6), (-13.0, 0.0, -8.0), (13.0, 0.0, -10.0)]),
            half: h,
            deco,
            braziers,
            props,
            vines,
            torches,
            map: MapId::CloseQuarters,
            flags: [v3(0.0, 0.0, -13.8), v3(0.0, 0.0, 13.8)],
            water: Vec::new(),
        }
    }

    /// The open courtyard: 48 x 48 m around a raised dais, tall pillars and corner towers.
    pub fn courtyard() -> World {
        let h = 24.0; // half size
        let wall_h = 9.0;
        let mut v = Vec::new();
        // banners hang below the high ledge (it sits at 5.5 m)
        let (deco, banners) = enclosure(&mut v, h, wall_h, 4.5, 12.0);
        // wall trim (a ledge you can stand on high up)
        v.push(bx(-h, 5.5, -h, h, 6.0, -h + 1.2, Mat::Trim));
        v.push(bx(-h, 5.5, h - 1.2, h, 6.0, h, Mat::Trim));

        // centre raised dais with steps
        v.push(bx(-4.0, 0.0, -4.0, 4.0, 1.0, 4.0, Mat::Stone));
        v.push(bx(-2.5, 1.0, -2.5, 2.5, 1.5, 2.5, Mat::Trim));
        sym(&mut v, bx(-1.5, 0.0, 4.0, 1.5, 0.5, 5.0, Mat::Stone));
        sym(&mut v, bx(4.0, 0.0, -1.5, 5.0, 0.5, 1.5, Mat::Stone));

        // four tall pillars around the dais (wall-run!)
        sym(&mut v, bx(-9.0, 0.0, -9.0, -7.0, 8.0, -7.0, Mat::Metal));
        sym(&mut v, bx(7.0, 0.0, -9.0, 9.0, 8.0, -7.0, Mat::Metal));

        // long cover walls near each spawn
        sym(&mut v, bx(-6.0, 0.0, -15.0, 6.0, 3.2, -14.2, Mat::Wall));
        // side walls good for wall running
        sym(&mut v, bx(-17.0, 0.0, -6.0, -16.2, 5.0, 6.0, Mat::Wall));

        // corner towers with bridge ledges
        sym(&mut v, bx(-20.0, 0.0, -20.0, -15.0, 3.0, -15.0, Mat::Stone));
        sym(&mut v, bx(15.0, 0.0, -20.0, 20.0, 3.0, -15.0, Mat::Stone));
        sym(&mut v, bx(-20.0, 0.0, -15.0, -18.5, 1.5, -12.5, Mat::Stone));
        sym(&mut v, bx(15.0, 0.0, -15.0, 16.5, 1.5, -12.5, Mat::Stone));

        // crates scattered
        sym(&mut v, bx(-12.0, 0.0, -4.0, -10.5, 1.5, -2.5, Mat::Crate));
        sym(&mut v, bx(-11.8, 1.5, -3.8, -10.7, 2.6, -2.7, Mat::Crate));
        sym(&mut v, bx(10.0, 0.0, -12.0, 11.5, 1.5, -10.5, Mat::Crate));
        sym(&mut v, bx(-3.0, 0.0, -20.5, -1.5, 1.5, -19.0, Mat::Crate));
        sym(&mut v, bx(12.0, 0.0, 3.0, 13.2, 1.2, 4.2, Mat::Crate));

        // column bases and capitals
        for (cx, cz) in [(-8.0f32, -8.0f32), (8.0, -8.0), (-8.0, 8.0), (8.0, 8.0)] {
            v.push(bx(cx - 1.3, 0.0, cz - 1.3, cx + 1.3, 0.6, cz + 1.3, Mat::Stone));
            v.push(bx(cx - 1.3, 7.6, cz - 1.3, cx + 1.3, 8.4, cz + 1.3, Mat::Trim));
        }
        // braziers on the dais corners
        let mut braziers = Vec::new();
        for (x, z) in [(-3.4f32, -3.4f32), (3.4, -3.4), (-3.4, 3.4), (3.4, 3.4)] {
            v.push(bx(x - 0.3, 1.0, z - 0.3, x + 0.3, 1.9, z + 0.3, Mat::Metal));
            braziers.push(v3(x, 1.95, z));
        }

        // set dressing
        use PropKind::*;
        let mut props = banner_props(&banners, 0.36);
        corner_props(&mut props, -h, -h, 1.0, 1.0);
        corner_props(&mut props, h, -h, -1.0, 1.0);
        sym_prop(&mut props, WeaponStand, -5.5, 0.0, -h + 0.52, 0.0, 1.0, 255);
        sym_prop(&mut props, Shield, -6.2, 1.85, -h, 0.0, 1.0, 255);
        sym_prop(&mut props, Shield, -4.8, 1.85, -h, 0.0, 1.0, 255);
        // small rocks and rubble at the column bases, on the side away from the dais
        sym_prop(&mut props, RockA, -9.72, 0.0, -9.72, 0.6, 0.22, 255);
        sym_prop(&mut props, RockB, 9.72, 0.0, -9.72, 2.1, 0.22, 255);
        sym_prop(&mut props, Rubble, -4.5, 0.0, 4.9, 1.0, 0.9, 255);
        let mut torches = Vec::new();
        for z in [-13.0f32, 1.0] {
            sym_prop(&mut props, Torch, -h, 4.2, z, std::f32::consts::FRAC_PI_2, 1.0, 255);
            torches.push(v3(-h + 0.3, 4.62, z));
            torches.push(v3(h - 0.3, 4.62, -z));
        }
        let mut vines = Vec::new();
        for (k, (x, len)) in [(-11.5f32, 3.6f32), (1.0, 2.8), (13.0, 4.2)].iter().enumerate() {
            sym_vine(&mut vines, v3(*x, wall_h, -h), v3(0.0, 0.0, 1.0), *len, k as u32 + 71);
        }
        for (k, (z, len)) in [(-17.0f32, 3.8f32), (-5.0, 3.0), (10.0, 4.0)].iter().enumerate() {
            sym_vine(&mut vines, v3(-h, wall_h, *z), v3(1.0, 0.0, 0.0), *len, k as u32 + 81);
        }
        for (k, x) in [-3.5f32, 9.0].iter().enumerate() {
            sym_vine(&mut vines, v3(*x, 6.0, -h + 1.2), v3(0.0, 0.0, 1.0), 1.4, k as u32 + 91);
        }

        World {
            blocks: v,
            spawn_points: mirrored_spawns(&[(0.0, 0.0, -19.0), (-11.0, 0.0, -19.0), (11.0, 0.0, -19.0), (12.0, 0.0, -8.0), (-20.0, 0.0, -8.0), (4.5, 0.0, -10.0)]),
            half: h,
            deco,
            braziers,
            props,
            vines,
            torches,
            map: MapId::Courtyard,
            flags: [v3(0.0, 0.0, -21.5), v3(0.0, 0.0, 21.5)],
            water: Vec::new(),
        }
    }

    /// Varrow Bridgeworks: a 60 x 60 m team map at dusk, mirrored across the river that runs
    /// east-west through the middle (red's base in the south, z < 0; blue's in the north).
    ///
    /// - Each base: a roofed barracks to spawn in (two exits: a screened door to the yard and a
    ///   door into the flag room), a flag room with its doors off the main line, ramparts along
    ///   the front wall, and three gates (west, middle, east) onto the three lanes.
    /// - West lane, the riverside quay: long and open (the sniper lane), across a wooden
    ///   footbridge; a crane, a lock hut and crates for cover; a warehouse with a balcony.
    /// - Middle: a wide stone bridge between two plazas, a monument in its middle breaking the
    ///   line between the gates, low walls and crates (mid-range fights), a tower and a platform.
    /// - East lane: covered workshops (corridors, a hall, a wall-run passage) and a covered bridge
    ///   whose roof is broken in the middle: a 7 m gap only a dash crosses.
    /// - Levels: the ground, 3.5 m (ramparts, balconies, the workshop roofs, the platform) and
    ///   7 m (the warehouse roofs and the towers); and the riverbed below, wading deep, with steps
    ///   out on each lane.
    pub fn bridgeworks() -> World {
        let h = 30.0;
        let wall_h = 10.0;
        let l2 = 3.5;
        let l3 = 7.0;
        let mut v = Vec::new();
        let (deco, banners) = walls(&mut v, h, wall_h, 7.6, 12.0);
        // mirrored across the river (z -> -z): blue's half written, red's made from it
        let zm = |v: &mut Vec<Block>, b: Block| {
            v.push(b);
            v.push(Block { b: Aabb::new(v3(b.b.min.x, b.b.min.y, -b.b.max.z), v3(b.b.max.x, b.b.max.y, -b.b.min.z)), mat: b.mat });
        };

        // ---- the ground and the river: quays at 0, a stone bank wall, the bed 3 m down
        zm(&mut v, bx(-h, -1.0, 4.6, h, 0.0, h, Mat::Floor));
        zm(&mut v, bx(-h, -3.0, 4.0, h, 0.0, 4.6, Mat::Stone));
        v.push(bx(-h, -4.0, -4.0, h, -3.0, 4.0, Mat::Floor));
        // steps up out of the river on each lane (against the bank, 0.5 m each)
        for (x0, dir) in [(-20.5f32, 1.0f32), (10.5, 1.0), (26.0, 1.0)] {
            for k in 1..=6 {
                let a = x0 + dir * 0.5 * (k - 1) as f32;
                zm(&mut v, bx(a, -3.0, 2.6, a + dir * 0.5, -3.0 + 0.5 * k as f32, 4.0, Mat::Stone));
            }
        }

        // ---- the three bridges
        // west: a wooden footbridge with railings (the sniper lane crosses here)
        v.push(bx(-26.0, -0.45, -4.0, -22.0, 0.0, 4.0, Mat::Trim));
        v.push(bx(-26.0, 0.0, -4.0, -25.8, 1.0, 4.0, Mat::Hidden));
        v.push(bx(-22.2, 0.0, -4.0, -22.0, 1.0, 4.0, Mat::Hidden));
        // middle: the stone bridge, its railings, the monument at its middle
        v.push(bx(-8.0, -0.8, -4.0, 8.0, 0.0, 4.0, Mat::Stone));
        v.push(bx(-8.0, 0.0, -4.0, -7.8, 1.0, 4.0, Mat::Hidden));
        v.push(bx(7.8, 0.0, -4.0, 8.0, 1.0, 4.0, Mat::Hidden));
        v.push(bx(-1.5, 0.0, -1.5, 1.5, 2.6, 1.5, Mat::Stone));
        // east: the covered bridge, walls on both sides, its roof broken over the middle
        v.push(bx(19.0, -0.45, -4.0, 24.0, 0.0, 4.0, Mat::Trim));
        v.push(bx(18.5, 0.0, -5.0, 19.0, l2, 5.0, Mat::Wall));
        v.push(bx(24.0, 0.0, -5.0, 24.5, l2, 5.0, Mat::Wall));
        zm(&mut v, bx(18.5, l2, 3.5, 24.5, l2 + 0.4, 5.0, Mat::Trim));
        // a crate halfway across (cover in the corridor)
        v.push(bx(19.4, 0.0, -0.6, 20.6, 1.2, 0.6, Mat::Crate));

        // ---- west lane: the riverside quay
        // the warehouse between the quay and the plaza (roof: level 3)
        zm(&mut v, bx(-17.0, 0.0, 6.0, -12.0, l3, 13.0, Mat::Wall));
        // its balcony over the quay (level 2), on two posts, stairs up from the base end
        zm(&mut v, bx(-19.5, l2 - 0.3, 6.0, -17.0, l2, 13.0, Mat::Stone));
        zm(&mut v, bx(-19.5, 0.0, 6.0, -19.0, l2 - 0.3, 6.5, Mat::Stone));
        zm(&mut v, bx(-19.5, 0.0, 9.3, -19.0, l2 - 0.3, 9.8, Mat::Stone));
        for k in 1..=7 {
            let z0 = 13.0 + 0.5 * (7 - k) as f32;
            zm(&mut v, bx(-19.5, 0.0, z0, -17.5, 0.5 * k as f32, z0 + 0.5, Mat::Stone));
        }
        // stone steps from the balcony up to the warehouse roof (level 3); a crate on the
        // balcony's far end for cover
        for k in 1..=7 {
            let z1 = 13.0 - 0.5 * k as f32;
            zm(&mut v, bx(-19.5, l2, z1 - 0.5, -17.0, l2 + 0.5 * k as f32, z1, Mat::Stone));
        }
        zm(&mut v, bx(-19.3, l2, 6.4, -18.1, l2 + 1.2, 7.6, Mat::Crate));
        // the crane on its plinth (the mast is a hidden block: the prop draws it)
        zm(&mut v, bx(-29.6, 0.0, 9.0, -27.6, 1.0, 11.0, Mat::Stone));
        zm(&mut v, bx(-28.85, 1.0, 9.75, -28.35, 7.2, 10.25, Mat::Hidden));
        // the lock-keeper's hut and crates (cover along the quay)
        zm(&mut v, bx(-29.5, 0.0, 5.2, -27.0, 2.6, 7.4, Mat::Stone));
        zm(&mut v, bx(-24.6, 0.0, 11.6, -23.4, 1.2, 12.8, Mat::Crate));
        zm(&mut v, bx(-21.2, 0.0, 14.6, -20.0, 1.2, 15.8, Mat::Crate));

        // ---- middle: the plazas
        // the west tower at the bridge (level 3: reached from the warehouse roof)
        zm(&mut v, bx(-11.0, 0.0, 4.6, -8.5, l3, 7.6, Mat::Stone));
        // the east platform (level 2) with steps up from the plaza; the workshop roof is a step on
        zm(&mut v, bx(9.0, 0.0, 4.6, 12.6, l2, 8.0, Mat::Stone));
        for k in 1..=7 {
            let a = 5.5 + 0.5 * (k - 1) as f32;
            zm(&mut v, bx(a, 0.0, 6.0, a + 0.5, 0.5 * k as f32, 7.4, Mat::Stone));
        }
        // mid-range cover: low walls, crates, a pair of archways on the way to each gate
        zm(&mut v, bx(-6.0, 0.0, 9.0, -2.5, 1.1, 9.6, Mat::Stone));
        zm(&mut v, bx(2.5, 0.0, 11.6, 6.0, 1.1, 12.2, Mat::Stone));
        zm(&mut v, bx(-10.6, 0.0, 12.0, -9.4, 1.2, 13.2, Mat::Crate));
        zm(&mut v, bx(8.4, 0.0, 13.4, 9.6, 1.2, 14.6, Mat::Crate));
        zm(&mut v, bx(-3.3, 0.0, 14.2, -2.7, 4.2, 14.8, Mat::Hidden));
        zm(&mut v, bx(2.7, 0.0, 14.2, 3.3, 4.2, 14.8, Mat::Hidden));

        // ---- east lane: the workshops (walls 3.5 m, a walkable roof over them)
        // west wall onto the plaza, a door in it
        zm(&mut v, bx(13.0, 0.0, 5.0, 13.6, l2, 9.5, Mat::Wall));
        zm(&mut v, bx(13.0, 0.0, 12.0, 13.6, l2, 16.0, Mat::Wall));
        // the river side, open where the covered bridge comes in
        zm(&mut v, bx(13.6, 0.0, 5.0, 18.5, l2, 5.6, Mat::Wall));
        zm(&mut v, bx(24.5, 0.0, 5.0, h, l2, 5.6, Mat::Wall));
        // the corridor on from the bridge, into the hall
        zm(&mut v, bx(18.5, 0.0, 5.6, 19.0, l2, 9.0, Mat::Wall));
        zm(&mut v, bx(24.0, 0.0, 5.6, 24.5, l2, 9.0, Mat::Wall));
        // a narrow passage along the outer wall (wall-runs), open into the hall
        zm(&mut v, bx(27.0, 0.0, 5.6, 27.5, l2, 13.0, Mat::Wall));
        // the hall's back wall, its door lined up with the base's east gate
        zm(&mut v, bx(13.6, 0.0, 15.6, 24.0, l2, 16.0, Mat::Wall));
        zm(&mut v, bx(28.0, 0.0, 15.6, h, l2, 16.0, Mat::Wall));
        // the roof, with an opening over the stairs up to it
        zm(&mut v, bx(13.0, l2, 5.0, h, l2 + 0.4, 13.6, Mat::Trim));
        zm(&mut v, bx(18.0, l2, 13.6, h, l2 + 0.4, 16.0, Mat::Trim));
        for k in 1..=7 {
            let a = 14.0 + 0.5 * (k - 1) as f32;
            zm(&mut v, bx(a, 0.0, 13.8, a + 0.5, 0.5 * k as f32, 15.6, Mat::Stone));
        }
        // the furnace (big cover in the hall) and crates
        zm(&mut v, bx(20.0, 0.0, 11.0, 22.5, 1.6, 13.0, Mat::Metal));
        zm(&mut v, bx(15.0, 0.0, 9.6, 16.2, 1.2, 10.8, Mat::Crate));
        zm(&mut v, bx(25.0, 0.0, 10.2, 26.2, 1.2, 11.4, Mat::Crate));

        // ---- the bases
        // the front wall with three gates; its top is the rampart (level 2)
        for (x0, x1) in [(-h, -25.0f32), (-21.0, -2.0), (2.0, 24.0), (28.0, h)] {
            zm(&mut v, bx(x0, 0.0, 17.0, x1, l2, 19.0, Mat::Wall));
        }
        // steps up to the rampart, both sides of the middle gate
        for (x0, dir) in [(-8.5f32, -1.0f32), (8.5, 1.0)] {
            for k in 1..=7 {
                let a = x0 + dir * 0.5 * (k - 1) as f32;
                zm(&mut v, bx(a, 0.0, 19.0, a + dir * 0.5, 0.5 * k as f32, 20.5, Mat::Stone));
            }
        }
        // the barracks (spawn): a front wall with a door, a screen inside it, a door into the
        // flag room; roofed (level 2 above)
        zm(&mut v, bx(-h, 0.0, 22.0, -20.0, l2, 22.5, Mat::Wall));
        zm(&mut v, bx(-17.0, 0.0, 22.0, -5.0, l2, 22.5, Mat::Wall));
        zm(&mut v, bx(-22.5, 0.0, 23.8, -13.5, 2.8, 24.3, Mat::Wall));
        // and one inside the door from the flag room (walk round its north end)
        zm(&mut v, bx(-8.0, 0.0, 24.8, -7.5, 2.8, h, Mat::Wall));
        zm(&mut v, bx(-5.5, 0.0, 22.5, -5.0, l2, 26.0, Mat::Wall));
        zm(&mut v, bx(-5.5, 0.0, 28.0, -5.0, l2, h, Mat::Wall));
        // the flag room: its door to the yard off to one side, a side door to the east yard
        zm(&mut v, bx(-5.0, 0.0, 22.0, 2.0, l2, 22.5, Mat::Wall));
        zm(&mut v, bx(4.5, 0.0, 22.0, 5.5, l2, 22.5, Mat::Wall));
        zm(&mut v, bx(5.0, 0.0, 22.5, 5.5, l2, 26.0, Mat::Wall));
        zm(&mut v, bx(5.0, 0.0, 28.0, 5.5, l2, h, Mat::Wall));
        zm(&mut v, bx(-h, l2, 22.0, 5.5, l2 + 0.4, h, Mat::Trim));
        // the east yard: an armoury bench and crates
        zm(&mut v, bx(12.0, 0.0, 26.0, 16.0, 1.0, 27.2, Mat::Trim));
        zm(&mut v, bx(20.0, 0.0, 23.5, 21.2, 1.2, 24.7, Mat::Crate));

        // ---- braziers: the monument, the tower tops, the east platform
        let mut braziers = Vec::new();
        let fire = |v: &mut Vec<Block>, b: &mut Vec<V3>, x: f32, y: f32, z: f32| {
            v.push(bx(x - 0.25, y, z - 0.25, x + 0.25, y + 0.7, z + 0.25, Mat::Metal));
            b.push(v3(x, y + 0.75, z));
        };
        fire(&mut v, &mut braziers, 0.0, 2.6, 0.0);
        for s in [1.0f32, -1.0] {
            fire(&mut v, &mut braziers, -9.75, l3, 6.1 * s);
            fire(&mut v, &mut braziers, 12.0, l2, 7.4 * s);
        }

        // ---- set dressing
        use PropKind::*;
        let mut props: Vec<Prop> = banner_props(&banners, 0.36);
        // a prop on blue's side and its mirror on red's (banners swap colour)
        let pm = |p: &mut Vec<Prop>, kind: PropKind, x: f32, y: f32, z: f32, yaw: f32, scale: f32, team: u8| {
            p.push(Prop { kind, pos: v3(x, y, z), yaw, scale, team });
            let other = if team < 2 { 1 - team } else { team };
            p.push(Prop { kind, pos: v3(x, y, -z), yaw: std::f32::consts::PI - yaw, scale, team: other });
        };
        let pi = std::f32::consts::PI;
        let half_pi = std::f32::consts::FRAC_PI_2;
        // railings along the bridges (blue's half of each, mirrored)
        for (x, yaw) in [(-25.9f32, half_pi), (-22.1, -half_pi), (-7.9, half_pi), (7.9, -half_pi)] {
            for z in [0.75f32, 2.75] {
                pm(&mut props, Railing, x, 0.0, z, yaw, 1.0, 255);
            }
        }
        // the archways on the way to the middle gates
        pm(&mut props, Archway, 0.0, 0.0, 14.5, 0.0, 1.0, 255);
        // the crane over the river
        pm(&mut props, Crane, -28.6, 1.0, 10.0, pi, 1.0, 255);
        // banners on the front walls, facing out (team colours)
        for x in [-15.0f32, -7.0, 7.0, 15.0] {
            pm(&mut props, if x < 0.0 { BannerA } else { BannerB }, x, 3.2, 16.99, pi, 1.0, 1);
        }
        // cargo, barrels and rubble along the walls
        pm(&mut props, CrateStack, -14.5, 0.0, 5.1, pi, 1.0, 255);
        pm(&mut props, CrateStack, 29.0, 0.0, 21.0, -half_pi, 1.0, 255);
        pm(&mut props, Barrel, -12.5, 0.0, 15.6, 0.3, 1.0, 255);
        pm(&mut props, Barrel, -13.3, 0.0, 15.7, 1.4, 1.0, 255);
        pm(&mut props, Crate, 14.3, 0.0, 6.3, 0.2, 1.0, 255);
        pm(&mut props, Barrel, 29.2, 0.0, 14.9, 0.0, 1.0, 255);
        pm(&mut props, Rubble, -16.5, 0.0, 16.4, 0.8, 0.9, 255);
        pm(&mut props, WeaponStand, 13.0, 0.0, 29.48, pi, 1.0, 255);
        pm(&mut props, Shield, 12.3, 1.85, 30.0, pi, 1.0, 255);
        pm(&mut props, Shield, 13.7, 1.85, 30.0, pi, 1.0, 255);
        pm(&mut props, Plant, -12.7, 0.0, 22.95, 0.0, 1.0, 255);
        // lanterns: warm light on the walls (each also a light: torches)
        let mut torches = Vec::new();
        let lantern = |p: &mut Vec<Prop>, t: &mut Vec<V3>, x: f32, y: f32, z: f32, out: V3| {
            let yaw = out.x.atan2(out.z);
            for s in [1.0f32, -1.0] {
                let o = v3(out.x, 0.0, out.z * s);
                p.push(Prop { kind: Lantern, pos: v3(x, y, z * s), yaw: o.x.atan2(o.z), scale: 1.0, team: 255 });
                t.push(v3(x, y, z * s) + o * 0.32 - v3(0.0, 0.32, 0.0));
            }
            let _ = yaw;
        };
        let (px, nx, pz, nz) = (v3(1.0, 0.0, 0.0), v3(-1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), v3(0.0, 0.0, -1.0));
        lantern(&mut props, &mut torches, -11.0, 3.1, 17.0, nz);
        lantern(&mut props, &mut torches, 11.0, 3.1, 17.0, nz);
        lantern(&mut props, &mut torches, -26.5, 3.1, 17.0, nz);
        lantern(&mut props, &mut torches, -12.0, 4.2, 9.5, px);
        lantern(&mut props, &mut torches, -17.0, 5.3, 11.0, nx);
        lantern(&mut props, &mut torches, -8.5, 4.6, 6.1, px);
        lantern(&mut props, &mut torches, 13.6, 3.0, 14.0, px);
        lantern(&mut props, &mut torches, 19.0, 3.0, 7.3, px);
        lantern(&mut props, &mut torches, 21.0, 3.0, 15.6, nz);
        lantern(&mut props, &mut torches, -25.0, 3.0, 22.5, pz);
        lantern(&mut props, &mut torches, 0.0, 3.0, 30.0, nz);
        lantern(&mut props, &mut torches, 24.0, 3.0, 30.0, nz);
        // ivy down the outer walls and the warehouses
        let mut vines = Vec::new();
        for (k, (x, len)) in [(-22.0f32, 3.6f32), (-3.0, 4.0), (16.0, 3.0)].iter().enumerate() {
            vines.push(Vine { top: v3(*x, wall_h, h), out: nz, len: *len, seed: k as u32 + 101 });
            vines.push(Vine { top: v3(*x, wall_h, -h), out: pz, len: *len, seed: k as u32 + 111 });
        }
        for (k, z) in [8.0f32, 11.5].iter().enumerate() {
            vines.push(Vine { top: v3(-12.0, l3, *z), out: px, len: 2.6, seed: k as u32 + 121 });
            vines.push(Vine { top: v3(-12.0, l3, -*z), out: px, len: 2.6, seed: k as u32 + 131 });
        }

        // spawns: in the back of the barracks, away from its doors
        let blue = [(-28.0f32, 25.0f32), (-28.0, 28.5), (-25.0, 29.0), (-22.5, 28.5), (-12.5, 29.0), (-8.5, 29.0)];
        let mut spawn_points = Vec::new();
        for &(x, z) in &blue {
            spawn_points.push((v3(x, 0.0, -z), 0.0));
        }
        for &(x, z) in &blue {
            spawn_points.push((v3(x, 0.0, z), pi));
        }
        World {
            blocks: v,
            spawn_points,
            half: h,
            deco,
            braziers,
            props,
            vines,
            torches,
            map: MapId::Bridgeworks,
            flags: [v3(0.0, 0.0, -27.0), v3(0.0, 0.0, 27.0)],
            water: vec![Aabb::new(v3(-h, -3.0, -4.0), v3(h, -2.62, 4.0))],
        }
    }

    /// A team's flag base (capture the flag).
    pub fn flag_base(&self, team: usize) -> V3 {
        self.flags[team.min(1)]
    }

    /// Nearest hit of a ray against level geometry.
    pub fn raycast(&self, o: V3, d: V3, max_t: f32) -> Option<(f32, V3)> {
        let mut best: Option<(f32, V3)> = None;
        for bl in &self.blocks {
            if let Some((t, n)) = bl.b.ray(o, d, max_t) {
                if t > 0.0 && best.map_or(true, |(bt, _)| t < bt) {
                    best = Some((t, n));
                }
            }
        }
        best
    }

    pub fn ground_below(&self, p: V3) -> f32 {
        let mut g = -1.0f32;
        for bl in &self.blocks {
            let b = &bl.b;
            if p.x >= b.min.x && p.x <= b.max.x && p.z >= b.min.z && p.z <= b.max.z && b.max.y <= p.y + 0.05 {
                g = g.max(b.max.y);
            }
        }
        g
    }

    /// A copy of this world (tests).
    #[cfg(test)]
    pub fn clone_world(&self) -> World {
        World { blocks: self.blocks.clone(), spawn_points: self.spawn_points.clone(), half: self.half, deco: self.deco.clone(), braziers: self.braziers.clone(), props: self.props.clone(), vines: self.vines.clone(), torches: self.torches.clone(), map: self.map, flags: self.flags, water: self.water.clone() }
    }

    /// What's under someone's feet: the material of the highest surface just below `p`.
    pub fn ground_mat(&self, p: V3) -> Mat {
        let mut best = (-1.0f32, Mat::Floor);
        for bl in &self.blocks {
            let b = &bl.b;
            if p.x >= b.min.x && p.x <= b.max.x && p.z >= b.min.z && p.z <= b.max.z && b.max.y <= p.y + 0.05 && b.max.y > best.0 {
                best = (b.max.y, bl.mat);
            }
        }
        best.1
    }

    pub fn line_clear(&self, a: V3, b: V3) -> bool {
        let d = b - a;
        let l = d.len();
        if l < 1e-4 {
            return true;
        }
        self.raycast(a, d * (1.0 / l), l).is_none()
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;

    fn inside_block(w: &World, p: V3) -> bool {
        w.blocks.iter().any(|bl| bl.b.contains(p))
    }

    /// Distance on the ground from (x, z) to a block's footprint.
    fn flat_dist(b: &Aabb, x: f32, z: f32) -> f32 {
        let dx = (b.min.x - x).max(x - b.max.x).max(0.0);
        let dz = (b.min.z - z).max(z - b.max.z).max(0.0);
        (dx * dx + dz * dz).sqrt()
    }

    #[test]
    fn props_and_vines_stay_out_of_the_way() {
        for map in MapId::ALL {
            let w = World::build(map);
            assert!(!w.props.is_empty() && !w.vines.is_empty() && !w.torches.is_empty(), "{map:?} is dressed");
            let floor: Vec<&Prop> = w.props.iter().filter(|p| !p.kind.on_wall() && !p.kind.structural()).collect();
            for p in &w.props {
                let tag = format!("{map:?} {:?} at {:?}", p.kind, p.pos);
                if p.kind.structural() {
                    // carried by its blocks: a railing on its hidden railing block, an archway on
                    // its two hidden pillars, a crane on its plinth with its hidden mast
                    let hidden_at = |q: V3| w.blocks.iter().any(|bl| bl.mat == Mat::Hidden && bl.b.contains(q));
                    let side = yaw_right(p.yaw);
                    match p.kind {
                        PropKind::Railing => assert!(hidden_at(p.pos + v3(0.0, 0.5, 0.0)), "{tag}: on its railing block"),
                        PropKind::Archway => {
                            assert!(hidden_at(p.pos + side * 3.0 + v3(0.0, 1.0, 0.0)) && hidden_at(p.pos - side * 3.0 + v3(0.0, 1.0, 0.0)), "{tag}: its pillars")
                        }
                        _ => {
                            assert!(hidden_at(p.pos + v3(0.0, 2.0, 0.0)), "{tag}: its mast");
                            assert!((w.ground_below(p.pos + v3(0.0, 0.05, 0.0)) - p.pos.y).abs() < 0.02, "{tag}: on its plinth");
                        }
                    }
                    continue;
                }
                if p.kind.on_wall() {
                    // hung on a wall face, above heads
                    let back = p.pos - yaw_fwd(p.yaw) * 0.06;
                    assert!(inside_block(&w, back), "{tag}: a wall right behind it");
                    let min_y = if p.kind == PropKind::Shield { 1.5 } else { 3.0 };
                    assert!(p.pos.y >= min_y, "{tag}: high enough not to brush anyone");
                    continue;
                }
                if p.kind.structural() {
                    // part of the structure: on the blocks that carry it (a hidden block for its
                    // body, or the floor / plinth under it), never over the edge of the map
                    let g = w.ground_below(p.pos + v3(0.0, 0.05, 0.0));
                    assert!((g - p.pos.y).abs() < 0.02, "{tag}: stands on something ({g})");
                    assert!(p.pos.x.abs() < w.half && p.pos.z.abs() < w.half, "{tag}: inside the map");
                    continue;
                }
                let r = p.kind.radius() * p.scale;
                // standing on something
                let g = w.ground_below(p.pos + v3(0.0, 0.05, 0.0));
                assert!((g - p.pos.y).abs() < 0.02, "{tag}: on the ground ({g})");
                // nothing solid inside its footprint (it may touch a wall)
                for bl in &w.blocks {
                    let b = &bl.b;
                    if b.max.y <= p.pos.y + 0.05 || b.min.y > p.pos.y + 1.5 {
                        continue;
                    }
                    assert!(flat_dist(b, p.pos.x, p.pos.z) > r * 0.85 - 0.03, "{tag}: overlaps a block {:?}", b);
                }
                // tucked against a wall (never out in a lane)
                let near = w.blocks.iter().filter(|bl| bl.b.max.y > p.pos.y + 0.6 && bl.b.min.y <= p.pos.y + 0.05).map(|bl| flat_dist(&bl.b, p.pos.x, p.pos.z) - r).fold(f32::MAX, f32::min);
                assert!(near < 1.4, "{tag}: next to a wall ({near:.2} m)");
                // away from where people appear and from the flags
                for (s, _) in &w.spawn_points {
                    assert!((*s - p.pos).flat().len() > 2.0 + r * 0.5, "{tag}: clear of the spawn at {s:?}");
                }
                for f in &w.flags {
                    assert!((*f - p.pos).flat().len() > 2.5, "{tag}: clear of the flag base");
                }
                for q in &floor {
                    if std::ptr::eq(*q, p) {
                        continue;
                    }
                    let d = (q.pos - p.pos).flat().len();
                    assert!(d > (r + q.kind.radius() * q.scale) * 0.7, "{tag}: not inside another prop");
                }
            }
            for v in &w.vines {
                assert!(inside_block(&w, v.top - v.out * 0.06 - v3(0.0, 0.05, 0.0)), "{map:?} vine at {:?} hangs on a wall", v.top);
                assert!(v.top.y - v.len >= 2.2, "{map:?} vine at {:?} ends above heads", v.top);
            }
            for t in &w.torches {
                assert!(t.y >= 2.6, "torch and lantern flames burn above heads");
            }
        }
    }
}