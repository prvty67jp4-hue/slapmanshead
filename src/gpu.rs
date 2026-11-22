//! Hardware 3D renderer: textured, lit, depth-tested, multisampled. It draws into its own
//! texture, which the UI then shows (so the HUD and menus stay on top).

use crate::game::*;
use crate::math::*;
use crate::render::{Cam, sword_arm};
use crate::world::{Mat, World};
use bytemuck::{Pod, Zeroable};
use eframe::egui;
use eframe::egui_wgpu::RenderState;
use eframe::wgpu;
use std::f32::consts::PI;

const MSAA: u32 = 4;
const COLOR_FMT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FMT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const TEX: u32 = 256;
const LAYERS: u32 = 8;
const SUN: V3 = v3(0.5, 0.62, 0.6);

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vtx {
    pos: [f32; 3],
    nrm: [f32; 3],
    uv: [f32; 2],
    col: [f32; 4],
    // layer (-1 none), material (0 lit, 2 unlit, 3 soft particle), sun visibility, shininess
    ex: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    vp: [[f32; 4]; 4],
    cam: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    fwd: [f32; 4],
    sun: [f32; 4],
    fog: [f32; 4],
    params: [f32; 4],
}

const SHADER: &str = r#"
struct U { vp: mat4x4<f32>, cam: vec4<f32>, right: vec4<f32>, up: vec4<f32>, fwd: vec4<f32>, sun: vec4<f32>, fog: vec4<f32>, params: vec4<f32> };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var tex: texture_2d_array<f32>;
@group(0) @binding(2) var samp: sampler;

struct VIn { @location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) uv: vec2<f32>, @location(3) col: vec4<f32>, @location(4) ex: vec4<f32> };
struct VOut { @builtin(position) clip: vec4<f32>, @location(0) wpos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) uv: vec2<f32>, @location(3) col: vec4<f32>, @location(4) ex: vec4<f32> };

@vertex fn vs(v: VIn) -> VOut {
  var o: VOut;
  o.clip = u.vp * vec4<f32>(v.pos, 1.0);
  o.wpos = v.pos; o.nrm = v.nrm; o.uv = v.uv; o.col = v.col; o.ex = v.ex;
  return o;
}

fn fogit(c: vec3<f32>, wpos: vec3<f32>) -> vec3<f32> {
  let d = length(wpos - u.cam.xyz);
  let f = clamp((d - u.params.y) / (u.params.z - u.params.y), 0.0, 1.0) * u.fog.w;
  return mix(c, u.fog.rgb, f);
}

@fragment fn fs(i: VOut) -> @location(0) vec4<f32> {
  let layer = i.ex.x;
  let mat = i.ex.y;
  let t = textureSample(tex, samp, i.uv, i32(max(layer, 0.0) + 0.5));
  var base = i.col;
  if (layer >= 0.0) { base = base * t; }
  if (mat > 2.5) {
    let d = length(i.uv * 2.0 - 1.0);
    let a = clamp(1.0 - d, 0.0, 1.0);
    return vec4<f32>(fogit(base.rgb, i.wpos), base.a * a * a);
  }
  var c = base.rgb;
  if (mat < 1.5) {
    let n = normalize(i.nrm);
    let l = normalize(u.sun.xyz);
    let ndl = max(dot(n, l), 0.0);
    let sunvis = i.ex.z;
    let hemi = mix(vec3<f32>(0.30, 0.25, 0.24), vec3<f32>(0.48, 0.52, 0.66), n.y * 0.5 + 0.5);
    let sunc = vec3<f32>(1.35, 1.12, 0.86);
    let v = normalize(u.cam.xyz - i.wpos);
    let h = normalize(l + v);
    let spec = pow(max(dot(n, h), 0.0), 40.0) * i.ex.w * sunvis * ndl;
    c = base.rgb * (hemi + sunc * ndl * sunvis) + sunc * spec;
  }
  return vec4<f32>(fogit(c, i.wpos), base.a);
}

struct SkyOut { @builtin(position) clip: vec4<f32>, @location(0) ndc: vec2<f32> };
@vertex fn vs_sky(@builtin(vertex_index) vi: u32) -> SkyOut {
  var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
  var o: SkyOut;
  o.clip = vec4<f32>(p[vi], 0.0, 1.0);
  o.ndc = p[vi];
  return o;
}
fn hash(p: vec2<f32>) -> f32 { return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453); }
fn noise(p: vec2<f32>) -> f32 {
  let i = floor(p); let f = fract(p);
  let w = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), w.x), mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), w.x), w.y);
}
fn fbm(p: vec2<f32>) -> f32 {
  var s = 0.0; var a = 0.5; var q = p;
  for (var k = 0; k < 5; k = k + 1) { s = s + a * noise(q); q = q * 2.03; a = a * 0.5; }
  return s;
}
@fragment fn fs_sky(i: SkyOut) -> @location(0) vec4<f32> {
  let d = normalize(u.fwd.xyz + u.right.xyz * i.ndc.x * u.right.w + u.up.xyz * i.ndc.y * u.up.w);
  let y = d.y;
  let zen = vec3<f32>(0.10, 0.16, 0.38);
  let mid = vec3<f32>(0.42, 0.40, 0.60);
  let hor = vec3<f32>(0.98, 0.62, 0.38);
  var c = mix(hor, mid, smoothstep(0.0, 0.25, y));
  c = mix(c, zen, smoothstep(0.25, 0.9, y));
  if (y < 0.0) { c = mix(hor, u.fog.rgb, smoothstep(0.0, -0.2, y)); }
  let l = normalize(u.sun.xyz);
  let sd = max(dot(d, l), 0.0);
  c = c + vec3<f32>(1.0, 0.7, 0.4) * pow(sd, 8.0) * 0.45 + vec3<f32>(1.0, 0.9, 0.7) * pow(sd, 300.0) * 3.0;
  if (y > 0.0) {
    let uv = d.xz / (y + 0.12) * 1.6 + vec2<f32>(u.params.x * 0.01, 0.0);
    let cl = smoothstep(0.5, 0.85, fbm(uv));
    let ccol = mix(vec3<f32>(0.95, 0.70, 0.62), vec3<f32>(1.0, 0.88, 0.75), pow(sd, 4.0));
    c = mix(c, ccol, cl * smoothstep(0.0, 0.15, y) * 0.8);
  }
  return vec4<f32>(c, 1.0);
}
"#;

// ------------------------------------------------------------------ colours

fn lin(c: [u8; 3]) -> [f32; 4] {
    let f = |v: u8| (v as f32 / 255.0).powf(2.2);
    [f(c[0]), f(c[1]), f(c[2]), 1.0]
}
fn lina(c: [u8; 3], a: f32) -> [f32; 4] {
    let mut l = lin(c);
    l[3] = a;
    l
}

// ------------------------------------------------------------------ procedural textures

fn h2(x: i32, y: i32, s: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6b343) ^ (y as u32).wrapping_mul(0xd8163841) ^ s.wrapping_mul(0xcb1ab31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^= h >> 15;
    (h & 0xffffff) as f32 / 16777215.0
}

/// Tileable value noise with `period` cells across.
fn vnoise(x: f32, y: f32, period: i32, s: u32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let fx = x - xi as f32;
    let fy = y - yi as f32;
    let w = |t: f32| t * t * (3.0 - 2.0 * t);
    let g = |a: i32, b: i32| h2(a.rem_euclid(period), b.rem_euclid(period), s);
    let a = g(xi, yi) + (g(xi + 1, yi) - g(xi, yi)) * w(fx);
    let b = g(xi, yi + 1) + (g(xi + 1, yi + 1) - g(xi, yi + 1)) * w(fx);
    a + (b - a) * w(fy)
}

/// u, v in 0..1 over the texture.
fn fbm(u: f32, v: f32, base: i32, s: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut p = base;
    for o in 0..5 {
        sum += amp * vnoise(u * p as f32, v * p as f32, p, s + o);
        amp *= 0.5;
        p *= 2;
    }
    sum
}

fn texel(layer: u32, x: u32, y: u32) -> [f32; 3] {
    let n = TEX as f32;
    let u = x as f32 / n;
    let v = y as f32 / n;
    let grain = fbm(u, v, 16, layer * 31 + 7);
    match layer {
        0 => {
            // marble floor tiles, 2x2 per texture
            let tx = x / 128;
            let ty = y / 128;
            let lx = x % 128;
            let ly = y % 128;
            let dark = (tx + ty) % 2 == 1;
            let warp = fbm(u, v, 4, 3);
            let vein = (((u + v) * 9.0 + warp * 7.0) * PI).sin().abs();
            let vein = 1.0 - (vein / 0.06).min(1.0);
            let mut c = if dark { [0.50, 0.24, 0.18] } else { [0.88, 0.83, 0.74] };
            let k = 0.88 + grain * 0.24 - vein * 0.22;
            c = [c[0] * k, c[1] * k, c[2] * k];
            let edge = lx.min(ly).min(127 - lx).min(127 - ly);
            if edge < 2 {
                c = [0.22, 0.2, 0.18];
            } else if edge < 5 {
                let b = if lx < 5 || ly < 5 { 1.12 } else { 0.85 };
                c = [c[0] * b, c[1] * b, c[2] * b];
            }
            c
        }
        1 => {
            // stone bricks: 8 rows of 4
            let row = y / 32;
            let off = if row % 2 == 0 { 0 } else { 32 };
            let bxi = ((x + off) / 64) % 4;
            let lx = (x + off) % 64;
            let ly = y % 32;
            let id = h2(bxi as i32, row as i32, 9);
            let base = [0.55 + id * 0.12, 0.47 + id * 0.1, 0.40 + id * 0.08];
            let k = 0.8 + grain * 0.4;
            let mut c = [base[0] * k, base[1] * k, base[2] * k];
            let edge = lx.min(63 - lx).min(ly).min(31 - ly);
            if edge < 2 {
                c = [0.30, 0.27, 0.24];
            } else if edge < 4 {
                c = [c[0] * 0.82, c[1] * 0.82, c[2] * 0.82];
            }
            c
        }
        2 => {
            // flagstones (cells)
            let cell = 64.0;
            let px = x as f32 / cell;
            let py = y as f32 / cell;
            let (mut d1, mut d2, mut id) = (9.0f32, 9.0f32, 0.0f32);
            for oy in -1..=1 {
                for ox in -1..=1 {
                    let cx = px.floor() as i32 + ox;
                    let cy = py.floor() as i32 + oy;
                    let jx = h2(cx.rem_euclid(4), cy.rem_euclid(4), 21) * 0.7 + 0.15;
                    let jy = h2(cx.rem_euclid(4), cy.rem_euclid(4), 22) * 0.7 + 0.15;
                    let dx = cx as f32 + jx - px;
                    let dy = cy as f32 + jy - py;
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < d1 {
                        d2 = d1;
                        d1 = d;
                        id = h2(cx.rem_euclid(4), cy.rem_euclid(4), 23);
                    } else if d < d2 {
                        d2 = d;
                    }
                }
            }
            let k = 0.75 + grain * 0.4;
            let mut c = [(0.52 + id * 0.1) * k, (0.48 + id * 0.08) * k, (0.44 + id * 0.06) * k];
            let e = d2 - d1;
            if e < 0.05 {
                c = [0.2, 0.18, 0.17];
            } else if e < 0.1 {
                c = [c[0] * 0.8, c[1] * 0.8, c[2] * 0.8];
            }
            c
        }
        3 => {
            // fluted limestone
            let flute = 0.78 + 0.22 * (u * 2.0 * PI * 8.0).cos();
            let k = flute * (0.85 + grain * 0.3);
            [0.82 * k, 0.76 * k, 0.64 * k]
        }
        4 => {
            // wooden planks
            let plank = y / 64;
            let ly = y % 64;
            let id = h2(plank as i32, 0, 41);
            let g = ((v * 40.0 + fbm(u, v, 4, 42) * 6.0 + id * 3.0) * PI).sin() * 0.5 + 0.5;
            let k = 0.7 + g * 0.2 + grain * 0.2;
            let mut c = [0.50 * k, 0.33 * k, 0.18 * k];
            if ly < 3 {
                c = [0.12, 0.08, 0.05];
            }
            let nx = (x % 128) as i32 - 10;
            let ny = ly as i32 - 32;
            if nx * nx + ny * ny < 9 {
                c = [0.25, 0.25, 0.27];
            }
            c
        }
        5 => {
            // carved sandstone trim
            let band = (y % 64) as i32;
            let groove = if band < 3 || (band > 30 && band < 33) { 0.7 } else { 1.0 };
            let ly = (band - 16).abs() as f32 / 16.0;
            let lx = ((x % 32) as f32 - 16.0).abs() / 16.0;
            let diamond = if band > 3 && band < 30 && lx + ly < 0.55 && lx + ly > 0.4 { 0.82 } else { 1.0 };
            let k = (0.82 + grain * 0.3) * groove * diamond;
            [0.80 * k, 0.68 * k, 0.48 * k]
        }
        6 => {
            // banner cloth with a border and crest (tinted by vertex colour)
            let weave = 0.9 + 0.1 * ((x % 4) as f32 / 3.0) * ((y % 4) as f32 / 3.0);
            let bx = u.min(1.0 - u);
            let k = weave * (0.9 + grain * 0.15);
            let mut c = [k, k, k];
            if bx < 0.08 || v < 0.05 {
                c = [1.0, 0.85, 0.4];
            }
            let dx = u - 0.5;
            let dy = v - 0.42;
            let r = (dx * dx + dy * dy * 0.6).sqrt();
            if (r > 0.17 && r < 0.21) || (dx.abs() < 0.025 && dy.abs() < 0.15) || (dy.abs() < 0.025 && dx.abs() < 0.12) {
                c = [1.0, 0.85, 0.4];
            }
            if v > 0.9 {
                let tooth = ((u * 10.0).fract() - 0.5).abs() * 2.0;
                if (v - 0.9) * 10.0 > tooth {
                    c = [c[0] * 0.5, c[1] * 0.5, c[2] * 0.5];
                }
            }
            c
        }
        _ => {
            // fabric for clothes
            let weave = 0.92 + 0.08 * (((x / 2) + (y / 2)) % 2) as f32;
            let k = weave * (0.88 + grain * 0.2);
            [k, k, k]
        }
    }
}

fn build_textures() -> Vec<Vec<Vec<u8>>> {
    // [layer][mip] -> rgba8 (sRGB bytes)
    let mut out = Vec::new();
    for layer in 0..LAYERS {
        let mut lvl0 = vec![0u8; (TEX * TEX * 4) as usize];
        for y in 0..TEX {
            for x in 0..TEX {
                let c = texel(layer, x, y);
                let i = ((y * TEX + x) * 4) as usize;
                for k in 0..3 {
                    lvl0[i + k] = (c[k].clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
                }
                lvl0[i + 3] = 255;
            }
        }
        let mut mips = vec![lvl0];
        let mut size = TEX;
        while size > 1 {
            let prev = mips.last().unwrap();
            let ns = size / 2;
            let mut m = vec![0u8; (ns * ns * 4) as usize];
            for y in 0..ns {
                for x in 0..ns {
                    for k in 0..4 {
                        let s = |xx: u32, yy: u32| prev[((yy * size + xx) * 4 + k) as usize] as u32;
                        let v = s(2 * x, 2 * y) + s(2 * x + 1, 2 * y) + s(2 * x, 2 * y + 1) + s(2 * x + 1, 2 * y + 1);
                        m[((y * ns + x) * 4 + k) as usize] = (v / 4) as u8;
                    }
                }
            }
            mips.push(m);
            size = ns;
        }
        out.push(mips);
    }
    out
}

// ------------------------------------------------------------------ mesh building

#[derive(Default)]
pub struct MB {
    pub v: Vec<Vtx>,
}

impl MB {
    fn vert(&mut self, p: V3, n: V3, uv: [f32; 2], col: [f32; 4], ex: [f32; 4]) {
        self.v.push(Vtx { pos: [p.x, p.y, p.z], nrm: [n.x, n.y, n.z], uv, col, ex });
    }
    fn tri(&mut self, a: Vtx, b: Vtx, c: Vtx) {
        self.v.push(a);
        self.v.push(b);
        self.v.push(c);
    }
    /// Quad with a flat normal (p0..p3 in order around).
    fn quad(&mut self, p: [V3; 4], n: V3, uv: [[f32; 2]; 4], col: [f32; 4], ex: [f32; 4]) {
        for i in [0, 1, 2, 0, 2, 3] {
            self.vert(p[i], n, uv[i], col, ex);
        }
    }
    /// Oriented box.
    fn obox(&mut self, c: V3, r: &M3, h: V3, col: [f32; 4], layer: f32, shine: f32) {
        let ax = [r.x, r.y, r.z];
        let he = [h.x, h.y, h.z];
        for a in 0..3 {
            for s in [-1.0f32, 1.0] {
                let n = ax[a] * s;
                let u = ax[(a + 1) % 3] * he[(a + 1) % 3];
                let v = ax[(a + 2) % 3] * he[(a + 2) % 3];
                let fc = c + n * he[a];
                let su = he[(a + 1) % 3] * 2.0;
                let sv = he[(a + 2) % 3] * 2.0;
                self.quad(
                    [fc - u - v, fc + u - v, fc + u + v, fc - u + v],
                    n,
                    [[0.0, 0.0], [su, 0.0], [su, sv], [0.0, sv]],
                    col,
                    [layer, 0.0, 1.0, shine],
                );
            }
        }
    }
    /// Ellipsoid with radii along the rotation's axes.
    fn ellipsoid(&mut self, c: V3, r: &M3, rad: V3, col: [f32; 4], layer: f32, shine: f32, mat: f32) {
        let (seg, rings) = (12, 8);
        let pt = |i: usize, j: usize| -> (V3, V3) {
            let th = i as f32 / seg as f32 * 2.0 * PI;
            let ph = j as f32 / rings as f32 * PI;
            let unit = v3(ph.sin() * th.cos(), ph.cos(), ph.sin() * th.sin());
            let p = c + r.x * (unit.x * rad.x) + r.y * (unit.y * rad.y) + r.z * (unit.z * rad.z);
            let n = (r.x * (unit.x / rad.x) + r.y * (unit.y / rad.y) + r.z * (unit.z / rad.z)).norm();
            (p, n)
        };
        let ex = [layer, mat, 1.0, shine];
        for j in 0..rings {
            for i in 0..seg {
                let (a, na) = pt(i, j);
                let (b, nb) = pt(i + 1, j);
                let (cc, nc) = pt(i + 1, j + 1);
                let (d, nd) = pt(i, j + 1);
                let uv = |i: usize, j: usize| [i as f32 / seg as f32, j as f32 / rings as f32];
                let mk = |p: V3, n: V3, t: [f32; 2]| Vtx { pos: [p.x, p.y, p.z], nrm: [n.x, n.y, n.z], uv: t, col, ex };
                self.tri(mk(a, na, uv(i, j)), mk(b, nb, uv(i + 1, j)), mk(cc, nc, uv(i + 1, j + 1)));
                self.tri(mk(a, na, uv(i, j)), mk(cc, nc, uv(i + 1, j + 1)), mk(d, nd, uv(i, j + 1)));
            }
        }
    }
    /// Tapered tube from p0 to p1; `side` gives the cross-section's first axis; `flat` squashes the other.
    fn tube(&mut self, p0: V3, p1: V3, r0: f32, r1: f32, side: V3, flat: f32, col: [f32; 4], layer: f32, shine: f32) {
        let axis = p1 - p0;
        let len = axis.len().max(1e-4);
        let d = axis * (1.0 / len);
        let mut a = (side - d * side.dot(d)).norm();
        if a.len() < 0.5 {
            a = if d.y.abs() < 0.9 { d.cross(V3::UP).norm() } else { d.cross(v3(1.0, 0.0, 0.0)).norm() };
        }
        let b = d.cross(a).norm();
        let seg = 10;
        let ex = [layer, 0.0, 1.0, shine];
        for i in 0..seg {
            let t0 = i as f32 / seg as f32 * 2.0 * PI;
            let t1 = (i + 1) as f32 / seg as f32 * 2.0 * PI;
            let ring = |t: f32, r: f32| a * (t.cos() * r) + b * (t.sin() * r * flat);
            let nr = |t: f32| (a * t.cos() + b * (t.sin() / flat.max(0.1))).norm();
            let q = [p0 + ring(t0, r0), p0 + ring(t1, r0), p1 + ring(t1, r1), p1 + ring(t0, r1)];
            let n = [nr(t0), nr(t1), nr(t1), nr(t0)];
            let uv = [[t0 / PI, 0.0], [t1 / PI, 0.0], [t1 / PI, len * 2.0], [t0 / PI, len * 2.0]];
            let mk = |k: usize| Vtx { pos: [q[k].x, q[k].y, q[k].z], nrm: [n[k].x, n[k].y, n[k].z], uv: uv[k], col, ex };
            self.tri(mk(0), mk(1), mk(2));
            self.tri(mk(0), mk(2), mk(3));
        }
    }
    fn billboard(&mut self, cam: &Cam, p: V3, size: f32, col: [f32; 4]) {
        let r = cam.right * size;
        let u = cam.up * size;
        let n = -cam.fwd;
        self.quad([p - r - u, p + r - u, p + r + u, p - r + u], n, [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]], col, [-1.0, 3.0, 1.0, 0.0]);
    }
    fn beam(&mut self, cam: &Cam, a: V3, b: V3, w: f32, col: [f32; 4]) {
        let d = b - a;
        let side = d.cross(a - cam.pos).norm() * w;
        let n = -cam.fwd;
        self.quad([a - side, b - side, b + side, a + side], n, [[0.5, 0.0], [0.5, 0.0], [0.5, 1.0], [0.5, 1.0]], col, [-1.0, 3.0, 1.0, 0.0]);
    }
}

fn mat_params(m: Mat) -> (f32, f32, f32) {
    // layer, metres per texture repeat, shininess
    match m {
        Mat::Floor => (0.0, 4.0, 0.5),
        Mat::Wall => (1.0, 3.0, 0.05),
        Mat::Stone => (2.0, 3.0, 0.1),
        Mat::Metal => (3.0, 2.2, 0.15),
        Mat::Crate => (4.0, 1.5, 0.1),
        Mat::Trim => (5.0, 2.0, 0.25),
    }
}

fn build_world(world: &World) -> Vec<Vtx> {
    let mut mb = MB::default();
    let sun = SUN.norm();
    let shadow = |p: V3| -> f32 {
        if world.raycast(p, sun, 90.0).is_some() { 0.0 } else { 1.0 }
    };
    let all: Vec<_> = world.blocks.iter().chain(world.deco.iter()).collect();
    for bl in all {
        let b = bl.b;
        let (layer, tile, shine) = mat_params(bl.mat);
        let sx = b.max.x - b.min.x;
        let sy = b.max.y - b.min.y;
        let sz = b.max.z - b.min.z;
        let defs: [(V3, V3, V3, V3); 6] = [
            (v3(0.0, 1.0, 0.0), v3(b.min.x, b.max.y, b.min.z), v3(sx, 0.0, 0.0), v3(0.0, 0.0, sz)),
            (v3(0.0, -1.0, 0.0), v3(b.min.x, b.min.y, b.min.z), v3(0.0, 0.0, sz), v3(sx, 0.0, 0.0)),
            (v3(1.0, 0.0, 0.0), v3(b.max.x, b.min.y, b.min.z), v3(0.0, sy, 0.0), v3(0.0, 0.0, sz)),
            (v3(-1.0, 0.0, 0.0), v3(b.min.x, b.min.y, b.min.z), v3(0.0, 0.0, sz), v3(0.0, sy, 0.0)),
            (v3(0.0, 0.0, 1.0), v3(b.min.x, b.min.y, b.max.z), v3(sx, 0.0, 0.0), v3(0.0, sy, 0.0)),
            (v3(0.0, 0.0, -1.0), v3(b.min.x, b.min.y, b.min.z), v3(0.0, sy, 0.0), v3(sx, 0.0, 0.0)),
        ];
        for (n, o, e1, e2) in defs.iter() {
            if n.y < -0.5 && b.min.y <= 0.01 {
                continue;
            }
            if bl.mat == Mat::Floor && n.y < 0.5 {
                continue;
            }
            let cell = if bl.mat == Mat::Floor { 0.75 } else { 0.8 };
            let nu = ((e1.len() / cell).ceil() as usize).max(1);
            let nv = ((e2.len() / cell).ceil() as usize).max(1);
            // vertex grid with baked light
            let mut grid = vec![(V3::ZERO, 0.0f32, 1.0f32); (nu + 1) * (nv + 1)];
            for j in 0..=nv {
                for i in 0..=nu {
                    let p = *o + *e1 * (i as f32 / nu as f32) + *e2 * (j as f32 / nv as f32);
                    let sv = if n.dot(sun) > 0.0 { shadow(p + *n * 0.04) } else { 0.0 };
                    let mut ao = 1.0f32;
                    if n.y.abs() < 0.5 {
                        ao = 0.55 + 0.45 * (p.y / 2.2).clamp(0.0, 1.0);
                    } else if n.y > 0.5 {
                        let q = p + v3(0.0, 0.25, 0.0);
                        for d in [v3(1.0, 0.0, 0.0), v3(-1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), v3(0.0, 0.0, -1.0)] {
                            if let Some((t, _)) = world.raycast(q, d, 1.2) {
                                ao -= 0.16 * (1.0 - t / 1.2);
                            }
                        }
                    }
                    grid[j * (nu + 1) + i] = (p, sv, ao.max(0.4));
                }
            }
            let uvf = |p: V3| -> [f32; 2] {
                if n.y.abs() > 0.5 {
                    [p.x / tile, p.z / tile]
                } else if n.x.abs() > 0.5 {
                    [p.z / tile, -p.y / tile]
                } else {
                    [p.x / tile, -p.y / tile]
                }
            };
            let mut tint = [1.0f32, 1.0, 1.0];
            if bl.mat == Mat::Metal && sy < 2.0 {
                tint = [0.35, 0.33, 0.32]; // braziers: dark iron
            }
            for j in 0..nv {
                for i in 0..nu {
                    let idx = [j * (nu + 1) + i, j * (nu + 1) + i + 1, (j + 1) * (nu + 1) + i + 1, (j + 1) * (nu + 1) + i];
                    for k in [0, 1, 2, 0, 2, 3] {
                        let (p, sv, ao) = grid[idx[k]];
                        let col = [tint[0] * ao, tint[1] * ao, tint[2] * ao, 1.0];
                        mb.vert(p, *n, uvf(p), col, [layer, 0.0, sv, shine]);
                    }
                }
            }
        }
    }
    // banners
    for (c, n, team) in &world.banners {
        let right = v3(n.z, 0.0, -n.x);
        let w = 0.75;
        let hgt = 1.7;
        let col = if *team == 0 { lin([150, 34, 38]) } else { lin([46, 78, 160]) };
        let p = *c + *n * 0.03;
        mb.quad(
            [p - right * w + v3(0.0, hgt, 0.0), p + right * w + v3(0.0, hgt, 0.0), p + right * w - v3(0.0, hgt, 0.0), p - right * w - v3(0.0, hgt, 0.0)],
            *n,
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            col,
            [6.0, 2.0, 1.0, 0.0],
        );
        let rod = M3 { x: right, y: V3::UP, z: *n };
        mb.obox(p + v3(0.0, hgt + 0.05, 0.05), &rod, v3(w + 0.15, 0.05, 0.05), lin([220, 220, 220]), 5.0, 0.8);
    }
    // distant mountains
    let mut rng = Rng(424242);
    let count = 90;
    let mut hs: Vec<f32> = (0..count).map(|_| rng.range(8.0, 30.0)).collect();
    for _ in 0..2 {
        let c = hs.clone();
        for k in 0..count {
            hs[k] = (c[(k + count - 1) % count] + c[k] * 2.0 + c[(k + 1) % count]) * 0.25;
        }
    }
    for k in 0..count {
        let a0 = k as f32 / count as f32 * PI * 2.0;
        let a1 = (k + 1) as f32 / count as f32 * PI * 2.0;
        let am = (a0 + a1) * 0.5;
        let r = 210.0;
        let h = hs[k] + rng.range(0.0, 6.0);
        let col = lin([96, 84, 118]);
        let ex = [-1.0, 2.0, 1.0, 0.0];
        mb.vert(v3(a0.sin() * r, -2.0, a0.cos() * r), V3::UP, [0.0, 0.0], col, ex);
        mb.vert(v3(am.sin() * r, h, am.cos() * r), V3::UP, [0.0, 0.0], col, ex);
        mb.vert(v3(a1.sin() * r, -2.0, a1.cos() * r), V3::UP, [0.0, 0.0], col, ex);
    }
    mb.v
}

// ------------------------------------------------------------------ characters

struct Look {
    coat: [u8; 3],
    coat_dark: [u8; 3],
    pants: [u8; 3],
    hair: [u8; 3],
    shirt: [u8; 3],
}

/// Coat colours: 0 red and 1 blue double as the team colours.
pub const PALETTE: [[u8; 3]; 8] = [[150, 22, 30], [28, 52, 120], [30, 110, 60], [100, 40, 130], [190, 100, 20], [20, 120, 130], [40, 40, 44], [200, 196, 186]];

fn look(idx: u8) -> Look {
    let coat = PALETTE[(idx as usize) % PALETTE.len()];
    let dark = [(coat[0] as f32 * 0.62) as u8, (coat[1] as f32 * 0.62) as u8, (coat[2] as f32 * 0.62) as u8];
    let hairs = [[28, 20, 18], [215, 210, 200], [90, 50, 25], [20, 20, 24], [200, 160, 80], [60, 30, 20], [180, 40, 40], [30, 30, 34]];
    let shirt = if idx % 2 == 0 { [210, 205, 195] } else { [40, 40, 46] };
    Look { coat, coat_dark: dark, pants: [36, 34, 42], hair: hairs[(idx as usize) % hairs.len()], shirt }
}

fn fighter_mesh(mb: &mut MB, add_mb: &mut MB, cam: &Cam, f: &Fighter, pos: V3, time: f32) {
    let lk = look(f.color_idx);
    let flash = if f.hurt_t > 0.0 { (f.hurt_t * 3.0).min(0.8) } else { 0.0 };
    let tint = |c: [u8; 3]| -> [f32; 4] {
        let l = lin(c);
        [l[0] + (1.0 - l[0]) * flash, l[1] + (0.3 - l[1]).max(0.0) * flash, l[2] + (0.3 - l[2]).max(0.0) * flash, 1.0]
    };
    let skin = tint([236, 196, 168]);
    let cloth = 7.0;

    let mut root = M3::rot_y(f.yaw);
    let mut rp = pos;
    let speed = f.vel.flat().len();
    if !f.alive {
        let a = (f.death_t * 3.5).min(1.0) * (PI * 0.5);
        root = root.mul(&M3::rot_x(-a));
        rp.y += 0.15 * (a / (PI * 0.5));
    } else if let Some(w) = f.wall {
        if w.vertical {
            root = root.mul(&M3::rot_x(-0.25));
        } else {
            let side = yaw_right(f.yaw).dot(w.normal);
            root = root.mul(&M3::rot_z(-side * 0.45));
        }
    } else if f.dash_t > 0.0 {
        root = root.mul(&M3::rot_x(0.4));
    } else if f.on_ground && speed > 1.0 {
        root = root.mul(&M3::rot_x(0.1 * (speed / RUN_SPEED).min(1.0)));
    }
    if f.stun_t > 0.0 && f.alive {
        root = root.mul(&M3::rot_x(-0.3));
    }
    let at = |p: V3| rp + root.apply(p);
    let side = root.x;

    // legs
    let moving = if f.on_ground { (speed / RUN_SPEED).min(1.0) } else { 0.0 };
    let ph = f.walk_phase * 2.2;
    let air = !f.on_ground && f.alive && f.wall.is_none();
    let (sl, sr) = if f.wall.is_some() {
        (ph.sin() * 0.9, (ph + PI).sin() * 0.9)
    } else if air {
        (-0.7, 0.3)
    } else {
        (ph.sin() * 0.8 * moving, (ph + PI).sin() * 0.8 * moving)
    };
    for (sx, sw) in [(-0.11f32, sl), (0.11, sr)] {
        let thigh = root.mul(&M3::rot_x(sw));
        let hip = at(v3(sx, 0.93, 0.0));
        let knee = hip + thigh.apply(v3(0.0, -0.44, 0.0));
        let kb = (-sw).max(0.0) * 1.1 + if air { 0.9 } else { 0.08 } + moving * 0.25;
        let shin = root.mul(&M3::rot_x(sw + kb));
        let ankle = knee + shin.apply(v3(0.0, -0.43, 0.0));
        mb.tube(hip, knee, 0.085, 0.068, side, 1.0, tint(lk.pants), cloth, 0.05);
        mb.ellipsoid(knee, &shin, v3(0.068, 0.068, 0.068), tint(lk.pants), cloth, 0.05, 0.0);
        mb.tube(knee, ankle, 0.066, 0.055, side, 1.0, tint(lk.pants), cloth, 0.05);
        let boot_r = root.mul(&M3::rot_x(sw * 0.3));
        mb.ellipsoid(ankle + boot_r.apply(v3(0.0, -0.02, 0.06)), &boot_r, v3(0.07, 0.06, 0.13), tint([26, 22, 22]), -1.0, 0.5, 0.0);
    }
    // long coat skirt, trailing behind with speed
    let flap = 0.15 + (speed / 12.0).min(1.0) * 0.8 + (time * 9.0).sin() * 0.05 * moving + if air { 0.3 } else { 0.0 };
    let waist = at(v3(0.0, 1.0, 0.0));
    let hem = at(v3(0.0, 0.42 + flap * 0.15, -0.08 - flap * 0.3));
    mb.tube(waist, hem, 0.19, 0.3, side, 0.78, tint(lk.coat_dark), cloth, 0.05);
    // torso
    let chest = at(v3(0.0, 1.43, 0.0));
    mb.tube(waist, chest, 0.17, 0.215, side, 0.68, tint(lk.coat), cloth, 0.1);
    mb.ellipsoid(at(v3(0.0, 1.3, 0.1)), &root, v3(0.08, 0.15, 0.05), tint(lk.shirt), cloth, 0.05, 0.0);
    mb.tube(at(v3(0.0, 0.97, 0.0)), at(v3(0.0, 1.03, 0.0)), 0.18, 0.18, side, 0.72, tint([40, 30, 24]), -1.0, 0.4);
    mb.ellipsoid(at(v3(0.0, 1.02, 0.13)), &root, v3(0.04, 0.03, 0.02), tint([220, 180, 80]), -1.0, 1.0, 0.0);
    // collar
    mb.tube(at(v3(0.0, 1.42, 0.0)), at(v3(0.0, 1.52, -0.01)), 0.13, 0.11, side, 0.85, tint(lk.coat_dark), cloth, 0.1);
    // head
    let hr = root.mul(&M3::rot_x(-f.pitch * 0.45));
    let neck = at(v3(0.0, 1.5, 0.0));
    mb.tube(neck, neck + hr.apply(v3(0.0, 0.08, 0.0)), 0.05, 0.05, side, 1.0, skin, -1.0, 0.1);
    let head = neck + hr.apply(v3(0.0, 0.17, 0.015));
    mb.ellipsoid(head, &hr, v3(0.105, 0.125, 0.115), skin, -1.0, 0.15, 0.0);
    mb.ellipsoid(neck + hr.apply(v3(0.0, 0.215, -0.02)), &hr, v3(0.118, 0.1, 0.125), tint(lk.hair), -1.0, 0.4, 0.0);
    mb.ellipsoid(neck + hr.apply(v3(0.0, 0.15, -0.06)), &hr, v3(0.11, 0.1, 0.08), tint(lk.hair), -1.0, 0.4, 0.0);
    for ex in [-0.04f32, 0.04] {
        mb.ellipsoid(neck + hr.apply(v3(ex, 0.18, 0.107)), &hr, v3(0.016, 0.012, 0.008), lin([20, 20, 26]), -1.0, 0.0, 2.0);
    }

    // arms
    let recoil = f.recoil * 0.35;
    let moving_sway = ph.sin() * 0.3 * moving;
    let (r_arm, l_arm, bend_r, bend_l): (M3, M3, f32, f32) = if !f.alive {
        (root.mul(&M3::rot_z(0.3)), root.mul(&M3::rot_z(-0.3)), 0.2, 0.2)
    } else {
        match f.weapon {
            Weapon::Sword => {
                if f.slash_t > 0.0 {
                    let p = 1.0 - f.slash_t / SLASH_DUR;
                    (sword_arm(&root, f.slash_idx, p), root.mul(&M3::rot_x(-0.4)).mul(&M3::rot_z(-0.25)), 0.15, 0.6)
                } else if f.guard {
                    (root.mul(&M3::rot_y(-0.7)).mul(&M3::rot_x(-PI * 0.5 + 0.2)), root.mul(&M3::rot_y(0.5)).mul(&M3::rot_x(-PI * 0.5 + 0.3)), 0.3, 0.9)
                } else {
                    (root.mul(&M3::rot_x(-0.45 + moving_sway)), root.mul(&M3::rot_x(-moving_sway * 1.2)).mul(&M3::rot_z(-0.12)), 0.5, 0.3)
                }
            }
            _ => {
                let a = -PI * 0.5 - f.pitch * 0.9 - recoil;
                let reload = if f.reload_t > 0.0 { 0.7 } else { 0.0 };
                (
                    root.mul(&M3::rot_y(-0.08)).mul(&M3::rot_x(a + reload)),
                    root.mul(&M3::rot_y(0.45)).mul(&M3::rot_x(a + 0.15 + reload * 0.5)),
                    0.12,
                    0.5,
                )
            }
        }
    };
    let mut hand_r = V3::ZERO;
    let mut fore_r = r_arm;
    for (sx, arm, bend, right) in [(0.23f32, r_arm, bend_r, true), (-0.23, l_arm, bend_l, false)] {
        let sh = at(v3(sx, 1.4, 0.0));
        let elbow = sh + arm.apply(v3(0.0, -0.28, 0.0));
        let fore = arm.mul(&M3::rot_x(-bend));
        let hand = elbow + fore.apply(v3(0.0, -0.26, 0.0));
        mb.ellipsoid(sh, &root, v3(0.09, 0.08, 0.09), tint(lk.coat), cloth, 0.1, 0.0);
        mb.tube(sh, elbow, 0.07, 0.06, side, 1.0, tint(lk.coat), cloth, 0.1);
        mb.ellipsoid(elbow, &arm, v3(0.06, 0.06, 0.06), tint(lk.coat), cloth, 0.1, 0.0);
        mb.tube(elbow, hand, 0.062, 0.05, side, 1.0, tint(lk.coat), cloth, 0.1);
        mb.ellipsoid(hand, &fore, v3(0.045, 0.055, 0.04), skin, -1.0, 0.1, 0.0);
        if right {
            hand_r = hand;
            fore_r = fore;
        }
    }

    // weapon in the right hand
    let wf = fore_r.mul(&M3::rot_x(PI * 0.5));
    let hand = hand_r;
    let steel = lin([200, 206, 218]);
    match f.weapon {
        Weapon::Sword => {
            mb.obox(hand, &wf, v3(0.022, 0.022, 0.13), lin([30, 22, 22]), -1.0, 0.2);
            mb.obox(hand + wf.apply(v3(0.0, 0.0, 0.14)), &wf, v3(0.06, 0.02, 0.02), lin([210, 170, 70]), -1.0, 1.0);
            mb.obox(hand + wf.apply(v3(0.0, 0.0, 0.72)), &wf, v3(0.008, 0.03, 0.56), steel, -1.0, 2.5);
            if f.slash_t > 0.0 && f.alive {
                let p = 1.0 - f.slash_t / SLASH_DUR;
                let steps = 8;
                let mut prev: Option<(V3, V3)> = None;
                let shoulder = at(v3(0.23, 1.4, 0.0));
                for k in 0..=steps {
                    let q = (p - 0.4 * (1.0 - k as f32 / steps as f32)).max(0.0);
                    let ra = sword_arm(&root, f.slash_idx, q);
                    let fa = ra.mul(&M3::rot_x(-bend_r));
                    let hnd = shoulder + ra.apply(v3(0.0, -0.28, 0.0)) + fa.apply(v3(0.0, -0.26, 0.0));
                    let w2 = fa.mul(&M3::rot_x(PI * 0.5));
                    let a0 = hnd + w2.apply(v3(0.0, 0.0, 0.35));
                    let a1 = hnd + w2.apply(v3(0.0, 0.0, 1.3));
                    if let Some((b0, b1)) = prev {
                        let al = 0.05 + 0.45 * k as f32 / steps as f32;
                        let c = if f.slash_idx >= 2 { lina([255, 180, 90], al) } else { lina([170, 210, 255], al) };
                        let n = (b1 - b0).cross(a0 - b0).norm();
                        add_mb.quad([b0, b1, a1, a0], n, [[0.5, 0.5]; 4], c, [-1.0, 2.0, 1.0, 0.0]);
                    }
                    prev = Some((a0, a1));
                }
            }
            if f.guard {
                let c = hand + wf.apply(v3(0.0, 0.0, 0.6));
                add_mb.billboard(cam, c, 0.5 + (time * 20.0).sin().abs() * 0.08, lina([90, 150, 255], 0.5));
            }
        }
        Weapon::Shotgun => {
            mb.obox(hand + wf.apply(v3(0.0, 0.06, 0.05)), &wf, v3(0.04, 0.06, 0.25), lin([70, 46, 30]), 4.0, 0.3);
            mb.tube(hand + wf.apply(v3(0.0, 0.11, 0.0)), hand + wf.apply(v3(0.0, 0.11, 0.7)), 0.03, 0.03, wf.x, 1.0, lin([60, 62, 70]), -1.0, 1.5);
            mb.tube(hand + wf.apply(v3(0.0, 0.06, 0.25)), hand + wf.apply(v3(0.0, 0.06, 0.55)), 0.035, 0.035, wf.x, 1.0, lin([80, 56, 36]), 4.0, 0.3);
        }
        Weapon::Revolver => {
            mb.obox(hand + wf.apply(v3(0.0, 0.05, 0.0)), &wf, v3(0.022, 0.06, 0.04), lin([60, 40, 28]), 4.0, 0.3);
            mb.ellipsoid(hand + wf.apply(v3(0.0, 0.1, 0.07)), &wf, v3(0.035, 0.035, 0.05), lin([90, 92, 100]), -1.0, 1.5, 0.0);
            mb.tube(hand + wf.apply(v3(0.0, 0.11, 0.07)), hand + wf.apply(v3(0.0, 0.11, 0.32)), 0.018, 0.018, wf.x, 1.0, lin([120, 122, 132]), -1.0, 1.8);
        }
    }
}

fn shadow_disc(mb: &mut MB, world: &World, pos: V3) {
    let gy = world.ground_below(pos + v3(0.0, 0.1, 0.0));
    let hgt = (pos.y - gy).max(0.0);
    let rad = (0.6 - hgt * 0.05).max(0.25);
    let a = (0.55 - hgt * 0.06).clamp(0.12, 0.55);
    let c = v3(pos.x, gy + 0.025, pos.z);
    let seg = 16;
    let ex = [-1.0, 2.0, 1.0, 0.0];
    for k in 0..seg {
        let a0 = k as f32 / seg as f32 * 2.0 * PI;
        let a1 = (k + 1) as f32 / seg as f32 * 2.0 * PI;
        let p0 = c + v3(a0.cos() * rad, 0.0, a0.sin() * rad);
        let p1 = c + v3(a1.cos() * rad, 0.0, a1.sin() * rad);
        mb.vert(c, V3::UP, [0.5, 0.5], [0.0, 0.0, 0.0, a], ex);
        mb.vert(p0, V3::UP, [0.5, 0.5], [0.0, 0.0, 0.0, 0.0], ex);
        mb.vert(p1, V3::UP, [0.5, 0.5], [0.0, 0.0, 0.0, 0.0], ex);
    }
}

// ------------------------------------------------------------------ GPU state

struct Target {
    size: [u32; 2],
    msaa_view: wgpu::TextureView,
    resolve_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
}

pub struct Gpu {
    pipe_opaque: wgpu::RenderPipeline,
    pipe_alpha: wgpu::RenderPipeline,
    pipe_add: wgpu::RenderPipeline,
    pipe_sky: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    ubuf: wgpu::Buffer,
    static_buf: wgpu::Buffer,
    static_count: u32,
    dyn_buf: wgpu::Buffer,
    dyn_cap: u64,
    target: Option<Target>,
    pub tex_id: Option<egui::TextureId>,
}

fn make_pipeline(device: &wgpu::Device, layout: &wgpu::PipelineLayout, module: &wgpu::ShaderModule, blend: Option<wgpu::BlendState>, depth_write: bool, sky: bool) -> wgpu::RenderPipeline {
    let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4, 4 => Float32x4];
    let vbl = wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vtx>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs };
    let buffers: Vec<Option<wgpu::VertexBufferLayout>> = if sky { vec![] } else { vec![Some(vbl)] };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("duel pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some(if sky { "vs_sky" } else { "vs" }),
            compilation_options: Default::default(),
            buffers: &buffers,
        },
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FMT,
            depth_write_enabled: Some(depth_write),
            depth_compare: Some(if sky { wgpu::CompareFunction::Always } else { wgpu::CompareFunction::Less }),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState { count: MSAA, mask: !0, alpha_to_coverage_enabled: false },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(if sky { "fs_sky" } else { "fs" }),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format: COLOR_FMT, blend, write_mask: wgpu::ColorWrites::ALL })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

impl Gpu {
    pub fn new(rs: &RenderState, world: &World) -> Gpu {
        let device = &rs.device;
        let queue = &rs.queue;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("duel shader"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });

        // textures
        let mips = (TEX as f32).log2() as u32 + 1;
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("duel textures"),
            size: wgpu::Extent3d { width: TEX, height: TEX, depth_or_array_layers: LAYERS },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let data = build_textures();
        for (layer, levels) in data.iter().enumerate() {
            for (mip, bytes) in levels.iter().enumerate() {
                let s = (TEX >> mip).max(1);
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: mip as u32, origin: wgpu::Origin3d { x: 0, y: 0, z: layer as u32 }, aspect: wgpu::TextureAspect::All },
                    bytes,
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * s), rows_per_image: Some(s) },
                    wgpu::Extent3d { width: s, height: s, depth_or_array_layers: 1 },
                );
            }
        }
        let tex_view = tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("duel sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("duel uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("duel bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("duel bind"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&tex_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("duel layout"), bind_group_layouts: &[Some(&bgl)], immediate_size: 0 });
        let alpha = wgpu::BlendState::ALPHA_BLENDING;
        let add = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
        };
        let pipe_opaque = make_pipeline(device, &layout, &module, None, true, false);
        let pipe_alpha = make_pipeline(device, &layout, &module, Some(alpha), false, false);
        let pipe_add = make_pipeline(device, &layout, &module, Some(add), false, false);
        let pipe_sky = make_pipeline(device, &layout, &module, None, false, true);

        let verts = build_world(world);
        let static_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("duel world"),
            size: (verts.len() * std::mem::size_of::<Vtx>()).max(64) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&static_buf, 0, bytemuck::cast_slice(&verts));
        let dyn_cap = 4 << 20;
        let dyn_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("duel dynamic"), size: dyn_cap, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        Gpu {
            pipe_opaque,
            pipe_alpha,
            pipe_add,
            pipe_sky,
            bind,
            ubuf,
            static_buf,
            static_count: verts.len() as u32,
            dyn_buf,
            dyn_cap,
            target: None,
            tex_id: None,
        }
    }

    fn ensure_target(&mut self, rs: &RenderState, size: [u32; 2]) {
        if self.target.as_ref().map_or(false, |t| t.size == size) {
            return;
        }
        let device = &rs.device;
        let mk = |samples: u32, format: wgpu::TextureFormat, usage: wgpu::TextureUsages| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("duel target"),
                size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let msaa = mk(MSAA, COLOR_FMT, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let resolve = mk(1, COLOR_FMT, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
        let depth = mk(MSAA, DEPTH_FMT, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let t = Target {
            size,
            msaa_view: msaa.create_view(&Default::default()),
            resolve_view: resolve.create_view(&Default::default()),
            depth_view: depth.create_view(&Default::default()),
        };
        let mut r = rs.renderer.write();
        match self.tex_id {
            Some(id) => r.update_egui_texture_from_wgpu_texture(device, &t.resolve_view, wgpu::FilterMode::Linear, id),
            None => self.tex_id = Some(r.register_native_texture(device, &t.resolve_view, wgpu::FilterMode::Linear)),
        }
        self.target = Some(t);
    }

    /// Render the scene and return the texture to show.
    pub fn render(&mut self, rs: &RenderState, g: &Game, cam: &Cam, rect: egui::Rect, ppp: f32, alpha: f32, local_override: Option<(usize, V3)>) -> Option<egui::TextureId> {
        let w = ((rect.width() * ppp).round() as u32).clamp(16, 8192);
        let h = ((rect.height() * ppp).round() as u32).clamp(16, 8192);
        self.ensure_target(rs, [w, h]);

        // ---- uniforms
        let sx = cam.focal / (rect.width() * 0.5);
        let sy = cam.focal / (rect.height() * 0.5);
        let (near, far) = (0.05f32, 500.0f32);
        let a = far / (far - near);
        let b = -far * near / (far - near);
        let c = cam.pos;
        let rows = [
            [sx * cam.right.x, sx * cam.right.y, sx * cam.right.z, -sx * cam.right.dot(c)],
            [sy * cam.up.x, sy * cam.up.y, sy * cam.up.z, -sy * cam.up.dot(c)],
            [a * cam.fwd.x, a * cam.fwd.y, a * cam.fwd.z, -a * cam.fwd.dot(c) + b],
            [cam.fwd.x, cam.fwd.y, cam.fwd.z, -cam.fwd.dot(c)],
        ];
        let mut vp = [[0.0f32; 4]; 4];
        for col in 0..4 {
            for row in 0..4 {
                vp[col][row] = rows[row][col];
            }
        }
        let sun = SUN.norm();
        let fog = lin([214, 160, 140]);
        let u = Uniforms {
            vp,
            cam: [c.x, c.y, c.z, 1.0],
            right: [cam.right.x, cam.right.y, cam.right.z, 1.0 / sx],
            up: [cam.up.x, cam.up.y, cam.up.z, 1.0 / sy],
            fwd: [cam.fwd.x, cam.fwd.y, cam.fwd.z, 0.0],
            sun: [sun.x, sun.y, sun.z, 0.0],
            fog: [fog[0], fog[1], fog[2], 0.8],
            params: [g.time, 35.0, 260.0, 0.0],
        };
        rs.queue.write_buffer(&self.ubuf, 0, bytemuck::bytes_of(&u));

        // ---- dynamic geometry
        let mut op = MB::default();
        let mut al = MB::default();
        let mut ad = MB::default();
        for (i, f) in g.f.iter().enumerate() {
            if !f.present {
                continue;
            }
            let mut pos = f.prev_pos.lerp(f.pos, alpha.clamp(0.0, 1.0));
            if let Some((li, p)) = local_override {
                if li == i {
                    pos = p;
                }
            }
            if i == g.local && (cam.pos - (pos + v3(0.0, 1.1, 0.0))).len() < 0.8 {
                continue;
            }
            fighter_mesh(&mut op, &mut ad, cam, f, pos, g.time);
            if f.spawn_prot > 0.0 && f.alive {
                // spawn protection shimmer
                let a = 0.18 + 0.08 * (g.time * 12.0).sin();
                ad.billboard(cam, pos + v3(0.0, 1.0, 0.0), 1.15, lina([140, 200, 255], a));
            }
            shadow_disc(&mut al, &g.world, pos);
        }
        for gr in &g.grenades {
            op.ellipsoid(gr.pos, &M3::rot_y(gr.fuse * 9.0), v3(0.1, 0.11, 0.1), lin([45, 52, 45]), -1.0, 1.0, 0.0);
            if (gr.fuse * 10.0).sin() > 0.0 {
                ad.billboard(cam, gr.pos + v3(0.0, 0.13, 0.0), 0.15, lina([255, 40, 30], 1.0));
            }
        }
        for p in &g.particles {
            let t = (p.life / p.max_life).clamp(0.0, 1.0);
            let size = if p.size > 0.5 { p.size * (1.3 - t) * 2.0 } else { p.size * 2.2 };
            let glow = p.color[0] > 240 && p.color[1] > 150;
            if glow {
                ad.billboard(cam, p.pos, size, lina(p.color, t));
            } else {
                al.billboard(cam, p.pos, size, lina(p.color, t * 0.9));
            }
        }
        for tr in &g.tracers {
            let t = (tr.life / 0.09).clamp(0.0, 1.0);
            ad.beam(cam, tr.a, tr.b, 0.03, lina(tr.color, t));
        }
        // brazier flames (animated without state)
        for (bi, b) in g.world.braziers.iter().enumerate() {
            for k in 0..7 {
                let ph = (g.time * 1.6 + k as f32 / 7.0 + bi as f32 * 0.37).fract();
                let jx = (k as f32 * 12.9898 + bi as f32).sin() * 0.12;
                let jz = (k as f32 * 78.233 + bi as f32).cos() * 0.12;
                let p = *b + v3(jx * (1.0 - ph), ph * 0.9, jz * (1.0 - ph));
                let sz = 0.32 * (1.0 - ph) + 0.05;
                let col = if ph < 0.4 { [255, 200, 90] } else { [255, 120, 40] };
                ad.billboard(cam, p, sz, lina(col, 0.9 * (1.0 - ph)));
            }
            ad.billboard(cam, *b + v3(0.0, 0.3, 0.0), 1.1, lina([255, 150, 60], 0.18));
        }

        let n_op = op.v.len();
        let n_al = al.v.len();
        let n_ad = ad.v.len();
        let mut all = op.v;
        all.extend_from_slice(&al.v);
        all.extend_from_slice(&ad.v);
        let bytes = (all.len() * std::mem::size_of::<Vtx>()) as u64;
        if bytes > self.dyn_cap {
            self.dyn_cap = bytes.next_power_of_two();
            self.dyn_buf = rs.device.create_buffer(&wgpu::BufferDescriptor { label: Some("duel dynamic"), size: self.dyn_cap, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        }
        if !all.is_empty() {
            rs.queue.write_buffer(&self.dyn_buf, 0, bytemuck::cast_slice(&all));
        }

        // ---- draw
        let t = self.target.as_ref()?;
        let mut enc = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("duel frame") });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("duel pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.msaa_view,
                    depth_slice: None,
                    resolve_target: Some(&t.resolve_view),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_pipeline(&self.pipe_sky);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&self.pipe_opaque);
            pass.set_vertex_buffer(0, self.static_buf.slice(..));
            pass.draw(0..self.static_count, 0..1);
            if !all.is_empty() {
                pass.set_vertex_buffer(0, self.dyn_buf.slice(..));
                let a0 = n_op as u32;
                let a1 = (n_op + n_al) as u32;
                let a2 = (n_op + n_al + n_ad) as u32;
                if a0 > 0 {
                    pass.draw(0..a0, 0..1);
                }
                if a1 > a0 {
                    pass.set_pipeline(&self.pipe_alpha);
                    pass.draw(a0..a1, 0..1);
                }
                if a2 > a1 {
                    pass.set_pipeline(&self.pipe_add);
                    pass.draw(a1..a2, 0..1);
                }
            }
        }
        rs.queue.submit(Some(enc.finish()));
        self.tex_id
    }
}
