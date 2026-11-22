use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

impl V3 {
    pub const ZERO: V3 = v3(0.0, 0.0, 0.0);
    pub const UP: V3 = v3(0.0, 1.0, 0.0);
    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn norm(self) -> V3 {
        let l = self.len();
        if l > 1e-6 { self * (1.0 / l) } else { V3::ZERO }
    }
    pub fn flat(self) -> V3 {
        v3(self.x, 0.0, self.z)
    }
    pub fn lerp(self, o: V3, t: f32) -> V3 {
        self + (o - self) * t
    }
    pub fn mulv(self, o: V3) -> V3 {
        v3(self.x * o.x, self.y * o.y, self.z * o.z)
    }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for V3 {
    type Output = V3;
    fn mul(self, s: f32) -> V3 {
        v3(self.x * s, self.y * s, self.z * s)
    }
}
impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        v3(-self.x, -self.y, -self.z)
    }
}
impl AddAssign for V3 {
    fn add_assign(&mut self, o: V3) {
        *self = *self + o;
    }
}
impl SubAssign for V3 {
    fn sub_assign(&mut self, o: V3) {
        *self = *self - o;
    }
}

/// Forward direction for a yaw (radians). yaw 0 looks along +z.
pub fn yaw_fwd(yaw: f32) -> V3 {
    v3(yaw.sin(), 0.0, yaw.cos())
}
pub fn yaw_right(yaw: f32) -> V3 {
    v3(yaw.cos(), 0.0, -yaw.sin())
}
pub fn look_dir(yaw: f32, pitch: f32) -> V3 {
    v3(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos())
}

/// 3x3 rotation stored as three axis columns.
#[derive(Clone, Copy, Debug)]
pub struct M3 {
    pub x: V3,
    pub y: V3,
    pub z: V3,
}

impl M3 {
    pub const I: M3 = M3 { x: v3(1.0, 0.0, 0.0), y: v3(0.0, 1.0, 0.0), z: v3(0.0, 0.0, 1.0) };
    pub fn rot_y(a: f32) -> M3 {
        let (s, c) = a.sin_cos();
        M3 { x: v3(c, 0.0, -s), y: v3(0.0, 1.0, 0.0), z: v3(s, 0.0, c) }
    }
    /// Rotation about x: positive tilts +z towards -y (pitch forward/down).
    pub fn rot_x(a: f32) -> M3 {
        let (s, c) = a.sin_cos();
        M3 { x: v3(1.0, 0.0, 0.0), y: v3(0.0, c, s), z: v3(0.0, -s, c) }
    }
    pub fn rot_z(a: f32) -> M3 {
        let (s, c) = a.sin_cos();
        M3 { x: v3(c, s, 0.0), y: v3(-s, c, 0.0), z: v3(0.0, 0.0, 1.0) }
    }
    pub fn apply(&self, v: V3) -> V3 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }
    pub fn mul(&self, o: &M3) -> M3 {
        M3 { x: self.apply(o.x), y: self.apply(o.y), z: self.apply(o.z) }
    }
}

/// Axis aligned box.
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: V3,
    pub max: V3,
}

impl Aabb {
    pub fn new(min: V3, max: V3) -> Self {
        Aabb { min, max }
    }
    pub fn overlaps(&self, o: &Aabb) -> bool {
        self.min.x < o.max.x
            && self.max.x > o.min.x
            && self.min.y < o.max.y
            && self.max.y > o.min.y
            && self.min.z < o.max.z
            && self.max.z > o.min.z
    }
    pub fn contains(&self, p: V3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y && p.z >= self.min.z && p.z <= self.max.z
    }
    /// Ray-box slab test. Returns entry distance and normal.
    pub fn ray(&self, o: V3, d: V3, max_t: f32) -> Option<(f32, V3)> {
        let mut tmin = 0.0f32;
        let mut tmax = max_t;
        let mut n = V3::ZERO;
        let oa = [o.x, o.y, o.z];
        let da = [d.x, d.y, d.z];
        let mn = [self.min.x, self.min.y, self.min.z];
        let mx = [self.max.x, self.max.y, self.max.z];
        for i in 0..3 {
            if da[i].abs() < 1e-8 {
                if oa[i] < mn[i] || oa[i] > mx[i] {
                    return None;
                }
            } else {
                let inv = 1.0 / da[i];
                let mut t1 = (mn[i] - oa[i]) * inv;
                let mut t2 = (mx[i] - oa[i]) * inv;
                let mut sign = -1.0;
                if t1 > t2 {
                    std::mem::swap(&mut t1, &mut t2);
                    sign = 1.0;
                }
                if t1 > tmin {
                    tmin = t1;
                    n = V3::ZERO;
                    match i {
                        0 => n.x = sign,
                        1 => n.y = sign,
                        _ => n.z = sign,
                    }
                }
                tmax = tmax.min(t2);
                if tmin > tmax {
                    return None;
                }
            }
        }
        Some((tmin, n))
    }
}

/// Small deterministic RNG (xorshift).
#[derive(Clone, Debug)]
pub struct Rng(pub u64);
impl Rng {
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 16) as u32
    }
    pub fn f(&mut self) -> f32 {
        (self.next_u32() as f32) / (u32::MAX as f32)
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
}

pub fn wrap_angle(mut a: f32) -> f32 {
    use std::f32::consts::PI;
    while a > PI {
        a -= 2.0 * PI;
    }
    while a < -PI {
        a += 2.0 * PI;
    }
    a
}

pub fn approach(cur: f32, target: f32, step: f32) -> f32 {
    if cur < target { (cur + step).min(target) } else { (cur - step).max(target) }
}
