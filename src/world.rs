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

pub struct World {
    pub blocks: Vec<Block>,
    /// Spawn points (position, facing). Red team uses z < 0, blue team z > 0.
    pub spawn_points: Vec<(V3, f32)>,
    pub half: f32,
    pub deco: Vec<Block>,
    pub braziers: Vec<V3>,
    pub banners: Vec<(V3, V3, u8)>,
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

impl World {
    pub fn arena() -> World {
        let h = 24.0; // half size
        let wall_h = 9.0;
        let mut v = Vec::new();
        // floor slab
        v.push(bx(-h, -1.0, -h, h, 0.0, h, Mat::Floor));
        // outer walls
        v.push(bx(-h - 1.0, 0.0, -h - 1.0, h + 1.0, wall_h, -h, Mat::Wall));
        v.push(bx(-h - 1.0, 0.0, h, h + 1.0, wall_h, h + 1.0, Mat::Wall));
        v.push(bx(-h - 1.0, 0.0, -h, -h, wall_h, h, Mat::Wall));
        v.push(bx(h, 0.0, -h, h + 1.0, wall_h, h, Mat::Wall));
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
        // pilasters along the outer walls
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
        // braziers on the dais corners
        let mut braziers = Vec::new();
        for (x, z) in [(-3.4f32, -3.4f32), (3.4, -3.4), (-3.4, 3.4), (3.4, 3.4)] {
            v.push(bx(x - 0.3, 1.0, z - 0.3, x + 0.3, 1.9, z + 0.3, Mat::Metal));
            braziers.push(v3(x, 1.95, z));
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
        let mut b = -h + 7.0;
        while b < h - 4.0 {
            banners.push((v3(b, 5.2, -h + 0.37), v3(0.0, 0.0, 1.0), 0u8));
            banners.push((v3(b, 5.2, h - 0.37), v3(0.0, 0.0, -1.0), 1u8));
            b += 12.0;
        }

        World {
            blocks: v,
            spawn_points: {
                let half = [(0.0f32, -19.0f32), (-11.0, -19.0), (11.0, -19.0), (12.0, -8.0), (-20.0, -8.0), (4.5, -10.0)];
                let mut s = Vec::new();
                for (x, z) in half.iter().copied().chain(half.iter().map(|&(x, z)| (-x, -z))) {
                    s.push((v3(x, 0.0, z), (-x).atan2(-z)));
                }
                s
            },
            half: h,
            deco,
            braziers,
            banners,
        }
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

    pub fn line_clear(&self, a: V3, b: V3) -> bool {
        let d = b - a;
        let l = d.len();
        if l < 1e-4 {
            return true;
        }
        self.raycast(a, d * (1.0 / l), l).is_none()
    }
}
