use crate::math::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mat {
    Floor,
    Wall,
    Stone,
    Metal,
    Crate,
    Trim,
}

#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub b: Aabb,
    pub mat: Mat,
}

/// The maps a match can be played on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapId {
    /// 30 x 30 m, tight and vertical: wall-run corridors, three levels, dash gaps.
    Arena = 0,
    /// The original wide 48 x 48 m courtyard.
    Courtyard = 1,
}

impl MapId {
    pub const ALL: [MapId; 2] = [MapId::Arena, MapId::Courtyard];
    pub fn from_u8(v: u8) -> MapId {
        if v == 1 { MapId::Courtyard } else { MapId::Arena }
    }
    pub fn name(self) -> &'static str {
        match self {
            MapId::Arena => "Arena",
            MapId::Courtyard => "Open Courtyard",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            MapId::Arena => "small and vertical: wall-run corridors, three levels, gaps to dash across",
            MapId::Courtyard => "big and open: long sight lines around a central dais",
        }
    }
}

pub struct World {
    pub blocks: Vec<Block>,
    /// Spawn points (position, facing). Red team uses z < 0, blue team z > 0.
    pub spawn_points: Vec<(V3, f32)>,
    pub half: f32,
    pub deco: Vec<Block>,
    pub braziers: Vec<V3>,
    pub banners: Vec<(V3, V3, u8)>,
    pub map: MapId,
    /// Capture-the-flag bases (red, blue).
    pub flags: [V3; 2],
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

/// Outer walls with battlements on top, pilasters along them and banners.
fn enclosure(v: &mut Vec<Block>, h: f32, wall_h: f32, banner_y: f32, banner_gap: f32) -> (Vec<Block>, Vec<(V3, V3, u8)>) {
    v.push(bx(-h, -1.0, -h, h, 0.0, h, Mat::Floor));
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
        }
    }

    /// The main arena: 30 x 30 m, built for wall-running and fighting up and down. Three levels
    /// (ground, 3 m balconies and platforms, a 5 m tower and wall tops), corridors with walls on
    /// both sides, a gap only a dash or wall-jump crosses, and nowhere open more than ~8 m across.
    pub fn arena() -> World {
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

        World {
            blocks: v,
            spawn_points: mirrored_spawns(&[(-7.5, 0.0, -14.2), (0.0, 0.0, -14.0), (7.5, 0.0, -14.2), (0.0, lvl2, -13.6), (-13.0, 0.0, -8.0), (13.0, 0.0, -10.0)]),
            half: h,
            deco,
            braziers,
            banners,
            map: MapId::Arena,
            flags: [v3(0.0, 0.0, -13.8), v3(0.0, 0.0, 13.8)],
        }
    }

    /// The open courtyard: 48 x 48 m around a raised dais, tall pillars and corner towers.
    pub fn courtyard() -> World {
        let h = 24.0; // half size
        let wall_h = 9.0;
        let mut v = Vec::new();
        let (deco, banners) = enclosure(&mut v, h, wall_h, 5.2, 12.0);
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

        World {
            blocks: v,
            spawn_points: mirrored_spawns(&[(0.0, 0.0, -19.0), (-11.0, 0.0, -19.0), (11.0, 0.0, -19.0), (12.0, 0.0, -8.0), (-20.0, 0.0, -8.0), (4.5, 0.0, -10.0)]),
            half: h,
            deco,
            braziers,
            banners,
            map: MapId::Courtyard,
            flags: [v3(0.0, 0.0, -21.5), v3(0.0, 0.0, 21.5)],
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
        World { blocks: self.blocks.clone(), spawn_points: self.spawn_points.clone(), half: self.half, deco: self.deco.clone(), braziers: self.braziers.clone(), banners: self.banners.clone(), map: self.map, flags: self.flags }
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
