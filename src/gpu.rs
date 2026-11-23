//! Hardware 3D renderer. The scene is drawn lit by its map's sky (CC0 HDRI: ambient light and
//! reflections) and sun (with a shadow map), with PBR materials (CC0 textures with normal and
//! roughness maps), flickering brazier and torch lights and height fog, into a high dynamic range
//! picture. Post-processing (ambient occlusion, bloom, light shafts, colour grading, vignette,
//! each switchable in Graphics settings) then makes the picture the UI shows (so the
//! HUD and menus stay on top).

use crate::game::*;
use crate::math::*;
use crate::anim::{Rig, SkinVtx};
use crate::character::{self, CharState};
use crate::env;
use crate::render::{Cam, sword_arm};
use crate::world::{MapId, Mat, Vine, World};
use bytemuck::{Pod, Zeroable};
use eframe::egui;
use eframe::egui_wgpu::RenderState;
use eframe::wgpu;
use std::f32::consts::PI;

const MSAA: u32 = 4;
/// The scene is drawn in high dynamic range; post-processing makes the shown picture.
const COLOR_FMT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const OUT_FMT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const AO_FMT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FMT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// The painted texture layers (banner cloth, fabric) are drawn at this size, then scaled up.
const TEX: u32 = 256;
const LAYERS: u32 = env::LAYERS;
const MAX_JOINTS: usize = 72;
const MAX_INST: usize = 16;
/// Point lights at once (braziers, torches, muzzle flashes).
const MAX_LIGHTS: usize = 12;
/// Per character: two tints, the hit flash (three words of joint bits + strength), the joints.
// tint0, tint1, flash, rim, motion (4 floats each: motion is the velocity and a sway phase),
// then the joints
const INST_FLOATS: usize = 20 + MAX_JOINTS * 16;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vtx {
    pos: [f32; 3],
    nrm: [f32; 3],
    uv: [f32; 2],
    col: [f32; 4],
    // layer (-1 none), material (0 lit, 0.5 fighter texture, 2 unlit, 3 soft particle), ambient
    // occlusion, shininess
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
    /// direction to the sun, its strength
    sun: [f32; 4],
    /// sun colour, shadows on
    sun_col: [f32; 4],
    /// fog colour, density
    fog: [f32; 4],
    /// time, sky turn, sky brightness, sky's last mip level
    params: [f32; 4],
    light_vp: [[f32; 4]; 4],
    /// shadow map texel, soft shadows, lights in use, reflection strength
    shadow: [f32; 4],
    sh: [[f32; 4]; 9],
    lpos: [[f32; 4]; MAX_LIGHTS],
    lcol: [[f32; 4]; MAX_LIGHTS],
}

/// Post-processing settings for one frame.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PostU {
    /// 1/width, 1/height, width, height (of the scene)
    size: [f32; 4],
    /// where the sun is on screen (u, v), how much of it shows
    sun: [f32; 4],
    /// ambient occlusion, bloom, light shafts, sharpen (0/1)
    flags: [f32; 4],
    lift: [f32; 4],
    gamma: [f32; 4],
    gain: [f32; 4],
    /// vignette, bloom strength, light shaft strength, sharpen amount
    misc: [f32; 4],
    sun_col: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    fwd: [f32; 4],
    /// grading on, exposure, ambient occlusion strength, depth-of-field blur (pixels; flags.w is
    /// the focus distance)
    misc2: [f32; 4],
}

/// The graphics effects in use (Graphics settings; each preset starts from its own set).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fx {
    /// Shadow map size (pixels square).
    pub shadow_res: u32,
    pub soft_shadows: bool,
    pub ssao: bool,
    pub bloom: bool,
    pub rays: bool,
    pub grading: bool,
    pub vignette: bool,
}

impl Fx {
    pub const ALL: Fx = Fx { shadow_res: 2048, soft_shadows: true, ssao: true, bloom: true, rays: true, grading: true, vignette: true };
    /// Cheap: hard 4-tap shadows at 1024, no ambient occlusion or light shafts.
    pub const CHEAP: Fx = Fx { shadow_res: 1024, soft_shadows: false, ssao: false, bloom: true, rays: false, grading: true, vignette: true };
}

const U_DECL: &str = r#"
struct U { vp: mat4x4<f32>, cam: vec4<f32>, right: vec4<f32>, up: vec4<f32>, fwd: vec4<f32>, sun: vec4<f32>, sun_col: vec4<f32>, fog: vec4<f32>, params: vec4<f32>, light_vp: mat4x4<f32>, shadow: vec4<f32>, sh: array<vec4<f32>, 9>, lpos: array<vec4<f32>, 12>, lcol: array<vec4<f32>, 12> };
@group(0) @binding(0) var<uniform> u: U;
struct Inst { tint0: vec4<f32>, tint1: vec4<f32>, flash: vec4<f32>, rim: vec4<f32>, motion: vec4<f32>, joints: array<mat4x4<f32>, 72> };
@group(0) @binding(3) var<storage, read> insts: array<Inst>;
struct VIn { @location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) uv: vec2<f32>, @location(3) col: vec4<f32>, @location(4) ex: vec4<f32> };
struct SIn { @location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) j: vec4<u32>, @location(3) w: vec4<f32>, @location(4) mat: u32, @location(5) uv: vec2<f32>, @location(6) on: vec3<f32> };
fn skin_matrix(v: SIn, ii: u32) -> mat4x4<f32> {
  return insts[ii].joints[v.j.x] * v.w.x + insts[ii].joints[v.j.y] * v.w.y + insts[ii].joints[v.j.z] * v.w.z + insts[ii].joints[v.j.w] * v.w.w;
}
// secondary motion: hair, scarf tails, an apron, belt pouches drift a little at rest and trail
// behind the way the fighter moves (bits 4-7 of the material word say how much)
fn sway_offset(v: SIn, ii: u32) -> vec3<f32> {
  let s = f32((v.mat >> 4u) & 15u) / 15.0;
  if (s <= 0.0 || (v.mat >> 8u) >= 255u) { return vec3<f32>(0.0, 0.0, 0.0); }
  let mo = insts[ii].motion;
  let t = u.params.x;
  let ph = mo.w + v.pos.y * 5.0;
  let speed = min(length(mo.xz), 9.0) / 9.0;
  let idle = vec3<f32>(sin(t * 1.7 + ph), 0.0, cos(t * 1.3 + ph * 1.3)) * 0.02;
  let flap = vec3<f32>(sin(t * 9.0 + ph * 2.0), 0.4 * sin(t * 7.0 + ph), cos(t * 8.3 + ph)) * 0.018 * speed;
  var drag = -mo.xyz * vec3<f32>(0.014, 0.006, 0.014);
  let dl = length(drag);
  if (dl > 0.12) { drag = drag * (0.12 / dl); }
  return (idle + flap + drag) * s;
}
"#;

const SHADER: &str = r#"
@group(0) @binding(1) var tex: texture_2d_array<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(4) var ctex: texture_2d_array<f32>;
@group(0) @binding(5) var ntex: texture_2d_array<f32>;
@group(0) @binding(6) var sky: texture_2d<f32>;
@group(0) @binding(7) var skysamp: sampler;
@group(0) @binding(8) var shadow_map: texture_depth_2d;
@group(0) @binding(9) var shadow_samp: sampler_comparison;

const PI: f32 = 3.14159265;

// (bp, bn: a fighter's surface in its own model space, so painted detail sticks to it as it moves)
struct VOut { @builtin(position) clip: vec4<f32>, @location(0) wpos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) uv: vec2<f32>, @location(3) col: vec4<f32>, @location(4) ex: vec4<f32>, @location(5) rim: vec4<f32>, @location(6) bp: vec3<f32>, @location(7) bn: vec3<f32> };

@vertex fn vs(v: VIn) -> VOut {
  var o: VOut;
  o.clip = u.vp * vec4<f32>(v.pos, 1.0);
  o.wpos = v.pos; o.nrm = v.nrm; o.uv = v.uv; o.col = v.col; o.ex = v.ex;
  o.rim = vec4<f32>(0.0, 0.0, 0.0, 1.0);
  o.bp = v.pos; o.bn = v.nrm;
  return o;
}

// where a direction lands in the sky picture (turned so its sun matches the map's sunlight)
fn sky_uv(d: vec3<f32>) -> vec2<f32> {
  let c = cos(u.params.y);
  let s = sin(u.params.y);
  let x = d.x * c - d.z * s;
  let z = d.x * s + d.z * c;
  return vec2<f32>(0.5 + atan2(x, z) / (2.0 * PI), acos(clamp(d.y, -1.0, 1.0)) / PI);
}

// ambient light from the whole sky (spherical harmonics, already divided by pi)
fn sky_light(n: vec3<f32>) -> vec3<f32> {
  var e = u.sh[0].rgb * 0.282095;
  e += u.sh[1].rgb * (0.488603 * n.y) + u.sh[2].rgb * (0.488603 * n.z) + u.sh[3].rgb * (0.488603 * n.x);
  e += u.sh[4].rgb * (1.092548 * n.x * n.y) + u.sh[5].rgb * (1.092548 * n.y * n.z) + u.sh[6].rgb * (0.315392 * (3.0 * n.z * n.z - 1.0));
  e += u.sh[7].rgb * (1.092548 * n.x * n.z) + u.sh[8].rgb * (0.546274 * (n.x * n.x - n.z * n.z));
  return max(e, vec3<f32>(0.0));
}

fn fogit(c: vec3<f32>, wpos: vec3<f32>) -> vec3<f32> {
  let d = length(wpos - u.cam.xyz);
  // thicker low down: depth to the haze without hiding anyone up close
  let low = exp(-max(wpos.y, 0.0) * 0.12);
  let f = 1.0 - exp(-d * u.fog.w * (0.55 + 0.9 * low));
  return mix(c, u.fog.rgb, clamp(f, 0.0, 0.85));
}

// soft sun shadows: a dozen taps over a disc, or four for the cheap setting
fn sun_shadow(wp: vec3<f32>, n: vec3<f32>) -> f32 {
  if (u.sun_col.w < 0.5) { return 1.0; }
  let ndl = dot(n, normalize(u.sun.xyz));
  let p = u.light_vp * vec4<f32>(wp + n * (0.03 + 0.06 * (1.0 - abs(ndl))), 1.0);
  let s = p.xyz / p.w;
  let uv = vec2<f32>(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5);
  if (uv.x <= 0.001 || uv.x >= 0.999 || uv.y <= 0.001 || uv.y >= 0.999 || s.z >= 1.0) { return 1.0; }
  let z = s.z - 0.0008;
  let t = u.shadow.x;
  if (u.shadow.y > 0.5) {
    var taps = array<vec2<f32>, 12>(
      vec2<f32>(-0.326, -0.406), vec2<f32>(-0.840, -0.074), vec2<f32>(-0.696, 0.457), vec2<f32>(-0.203, 0.621),
      vec2<f32>(0.962, -0.195), vec2<f32>(0.473, -0.480), vec2<f32>(0.519, 0.767), vec2<f32>(0.185, -0.893),
      vec2<f32>(0.507, 0.064), vec2<f32>(0.896, 0.412), vec2<f32>(-0.322, -0.933), vec2<f32>(-0.792, -0.598));
    var sum = 0.0;
    for (var k = 0; k < 12; k = k + 1) {
      sum = sum + textureSampleCompareLevel(shadow_map, shadow_samp, uv + taps[k] * t * 2.4, z);
    }
    return sum / 12.0;
  }
  var s4 = textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2<f32>(-0.5, -0.5) * t, z);
  s4 = s4 + textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2<f32>(0.5, -0.5) * t, z);
  s4 = s4 + textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2<f32>(-0.5, 0.5) * t, z);
  s4 = s4 + textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2<f32>(0.5, 0.5) * t, z);
  return s4 * 0.25;
}

// a normal map applied without stored tangents (screen-space derivatives of the surface)
fn perturb(n: vec3<f32>, dp1: vec3<f32>, dp2: vec3<f32>, duv1: vec2<f32>, duv2: vec2<f32>, m: vec3<f32>) -> vec3<f32> {
  let dp2perp = cross(dp2, n);
  let dp1perp = cross(n, dp1);
  let t = dp2perp * duv1.x + dp1perp * duv2.x;
  let b = dp2perp * duv1.y + dp1perp * duv2.y;
  let l = max(dot(t, t), dot(b, b));
  if (l < 1e-14) { return n; }
  let inv = inverseSqrt(l);
  return normalize(t * (m.x * inv) + b * (m.y * inv) + n * m.z);
}

fn hash21(p: vec2<f32>) -> f32 { return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453); }
fn vnoise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let w = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash21(i), hash21(i + vec2<f32>(1.0, 0.0)), w.x), mix(hash21(i + vec2<f32>(0.0, 1.0)), hash21(i + vec2<f32>(1.0, 1.0)), w.x), w.y);
}

fn lum3(c: vec3<f32>) -> f32 { return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722)); }

// The fighters' toon shading: two soft-edged light bands (shade, light, full light), colour-rich
// shade, crisp highlights by material (metal, leather, hair, skin, eyes), a rim of sky light round
// the silhouette, and painted detail on the surface: woven cloth, creased leather, scratched
// metal. The kind of surface comes from the material word (ex.w), or for the outfits from the
// texture's colour: pale grey is metal, warm brown leather, anything else cloth.
fn toon(i: VOut, albedo_in: vec3<f32>, texel: vec3<f32>, n: vec3<f32>, ng: vec3<f32>, v: vec3<f32>, l: vec3<f32>, sh: f32, q: vec2<f32>, fw_weave: f32, fw_q: f32) -> vec3<f32> {
  var kind = u32(i.ex.w + 0.5);
  let mx = max(texel.r, max(texel.g, texel.b));
  let mn = min(texel.r, min(texel.g, texel.b));
  let sat = (mx - mn) / max(mx, 0.001);
  if (kind == 0u) {
    if (sat < 0.22 && mx > 0.42) { kind = 6u; }
    else if (texel.r > texel.g && texel.g > texel.b * 1.25 && texel.g > texel.r * 0.25 && sat > 0.4 && mx < 0.3) { kind = 7u; }
  }
  var alb = albedo_in;
  // painted detail, faded out before it gets finer than a pixel
  let fine = 1.0 - smoothstep(0.9, 2.2, fw_weave);
  let mid = 1.0 - smoothstep(0.12, 0.35, fw_q * 22.0);
  if (kind == 0u) {
    // cloth: a weave up close, soft mottling from afar
    let wq = q * 1050.0;
    let over = step(0.0, sin(wq.x * 0.5) * sin(wq.y * 0.5));
    let thread = mix(abs(sin(wq.y)), abs(sin(wq.x)), over);
    alb = alb * (1.0 - 0.18 * (1.0 - thread) * fine);
    alb = alb * (0.9 + 0.2 * vnoise(q * 9.0));
  } else if (kind == 7u) {
    // leather: creases and grain, a little darker and warmer
    let r = vnoise(q * 40.0 + vec2<f32>(vnoise(q * 8.0) * 2.0, 0.0));
    let crease = smoothstep(0.9, 0.98, 1.0 - abs(r * 2.0 - 1.0)) * mid;
    alb = alb * vec3<f32>(0.95, 0.9, 0.84) * (1.0 - 0.16 * crease) * (0.92 + 0.16 * vnoise(q * 140.0) * fine + 0.08 * (1.0 - fine));
  } else if (kind == 6u) {
    // metal: brighter and cooler, with scratches and worn patches
    alb = mix(alb, vec3<f32>(lum3(alb)), 0.25) * 1.15;
    let scratch = smoothstep(0.93, 1.0, abs(sin(q.x * 260.0 + vnoise(q * 30.0) * 8.0))) * smoothstep(0.45, 0.7, vnoise(q * 12.0)) * mid;
    alb = alb * (1.0 + 0.25 * scratch);
  } else if (kind == 5u) {
    alb = alb * (0.96 + 0.08 * vnoise(q * 30.0));
  } else if (kind == 1u) {
    // eyebrows: darker, defined
    alb = alb * 0.5;
  }
  let sunc = u.sun_col.rgb * u.sun.w;
  let x = min(dot(n, l), mix(-0.3, 1.0, sh));
  let b1 = smoothstep(-0.03, 0.05, x);
  let b2 = smoothstep(0.45, 0.52, x);
  var lit = 0.55 * b1 + 0.45 * b2;
  // eyes and eyebrows stay readable in the shade
  if (kind == 1u || kind == 2u) { lit = 0.5 + 0.5 * lit; }
  // the shade: richer and a little cooler (skin warmer), never black
  let hue = alb / max(max(alb.r, max(alb.g, alb.b)), 0.02);
  var shade_c = alb * mix(vec3<f32>(1.0), hue, 0.3);
  if (kind == 5u) { shade_c = alb * vec3<f32>(1.06, 0.86, 0.82); }
  let amb = sky_light(n) * 0.6 + sky_light(vec3<f32>(0.0, 1.0, 0.0)) * 0.55 + u.fog.rgb * 0.18;
  var c = alb * sunc * lit * 0.85 + mix(shade_c, alb, lit) * amb * 1.32;
  let h = normalize(l + v);
  let ndh = max(dot(n, h), 0.0);
  var sp = 0.0;
  // (metal: only a thin, crisp highlight; no broad sheen that would wash a face of it pale)
  // (as bright as the metal itself: pale buckles glint, dark iron only shows a dull spot)
  if (kind == 6u) { sp = smoothstep(0.978, 0.992, ndh) * clamp(lum3(alb) * 3.0, 0.03, 1.1); }
  else if (kind == 7u) { sp = smoothstep(0.95, 0.975, ndh) * 0.3; }
  else if (kind == 3u) { sp = smoothstep(0.9, 0.95, ndh) * 0.14; }
  else if (kind == 5u) { sp = smoothstep(0.97, 0.99, ndh) * 0.08; }
  else if (kind == 2u) { sp = smoothstep(0.96, 0.985, ndh) * 1.5; }
  c = c + sunc * sp * b1 * mix(vec3<f32>(1.0), hue, select(0.0, 0.5, kind == 6u));
  if (kind == 6u) {
    // metal catches the sky too
    let r = reflect(-v, n);
    // metal catches only a glancing touch of the sky (dark iron stays dark)
    let e = textureSampleLevel(sky, skysamp, sky_uv(r), 3.0).rgb * u.params.z;
    let graze = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 3.0);
    c = c + mix(vec3<f32>(lum3(e)), e, 0.4) * alb * (0.06 + 0.3 * graze);
  }
  // a rim of sky light round the silhouette (brighter facing up)
  let fr = 1.0 - clamp(dot(ng, v), 0.0, 1.0);
  let rim = smoothstep(0.58, 0.8, fr) * (0.45 + 0.55 * clamp(n.y + 0.3, 0.0, 1.0));
  let rim_k = select(0.9, 0.35, kind == 6u);
  c = c + sky_light(normalize(n + vec3<f32>(0.0, 0.8, 0.0))) * rim * rim_k * (0.6 + 0.4 * lit);
  return c;
}

fn shade(i: VOut) -> vec4<f32> {
  let layer = i.ex.x;
  let mat = i.ex.y;
  // every texture is sampled (and the derivatives taken) before any branch
  let li = i32(clamp(layer, 0.0, 17.0) + 0.5);
  let t0 = textureSample(tex, samp, i.uv, li);
  let nt = textureSample(ntex, samp, i.uv, li);
  let t1 = textureSample(ctex, samp, i.uv, i32(max(layer, 0.0) + 0.5));
  let dp1 = dpdx(i.wpos);
  let dp2 = dpdy(i.wpos);
  let duv1 = dpdx(i.uv);
  let duv2 = dpdy(i.uv);
  // two coordinates across a fighter's surface in its own space (for its painted detail), and
  // how fast they change on screen
  let an = abs(i.bn);
  let q = select(select(i.bp.xy, i.bp.xz, an.y >= an.z), i.bp.zy, an.x >= an.y && an.x >= an.z);
  let fw_weave = length(fwidth(q * 1050.0));
  let fw_q = length(fwidth(q));
  let is_char = mat > 0.25 && mat < 0.75;
  // a weapon: its baked, hand-painted texture in the fighters' array
  let is_wpn = mat > 0.75 && mat < 1.25;
  let world_tex = layer >= 0.0 && !is_char && !is_wpn;
  // the level's own materials: the map's palette colour times soft brightness detail
  let is_level = world_tex && (li <= 5 || (li >= 14 && li <= 16));
  // the river (layer -2): rippling, dark, mirroring the sky and the lanterns
  let is_water = layer < -1.5;
  var base = i.col;
  if (layer >= 0.0) { base = base * select(t0, t1, is_char || is_wpn); }
  if (is_level) {
    base = vec4<f32>(i.col.rgb * t0.rgb * 2.0, 1.0);
    // large-scale colour drift and patches of grime and wear, so no tile repeats show
    let wp = vec2<f32>(i.wpos.x + i.wpos.y * 0.71, i.wpos.z - i.wpos.y * 0.53);
    let drift = 0.65 * vnoise(wp * 0.11) + 0.35 * vnoise(wp * 0.37 + vec2<f32>(17.0, 3.0));
    base = vec4<f32>(base.rgb * (0.87 + 0.26 * drift), 1.0);
    let grime = smoothstep(0.58, 0.86, vnoise(wp * 0.65 + vec2<f32>(5.0, 11.0))) * 0.35;
    let grime_col = base.rgb * vec3<f32>(0.8, 0.77, 0.73);
    base = vec4<f32>(mix(base.rgb, grime_col, grime), 1.0);
  }
  if (is_char) {
    // the fighters keep the strongest colours in the scene
    let l = dot(base.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    base = vec4<f32>(max(mix(vec3<f32>(l), base.rgb, 1.15), vec3<f32>(0.0)), base.a);
  }
  if (world_tex && t0.a < 0.5 && mat < 1.5) { discard; }
  if (mat > 2.5) {
    let d = length(i.uv * 2.0 - 1.0);
    let a = clamp(1.0 - d, 0.0, 1.0);
    return vec4<f32>(fogit(base.rgb, i.wpos), base.a * a * a);
  }
  if (mat > 1.5) {
    return vec4<f32>(fogit(base.rgb, i.wpos), base.a);
  }
  let v = normalize(u.cam.xyz - i.wpos);
  var ng = normalize(i.nrm);
  if (dot(ng, v) < 0.0) { ng = -ng; } // two-sided: banners and leaves
  var n = ng;
  var rough = clamp(1.0 - i.ex.w * 0.3, 0.22, 0.9);
  var metal = select(0.0, 0.75, i.ex.w > 1.4);
  if (world_tex) {
    var m = vec3<f32>(nt.r * 2.0 - 1.0, -(nt.g * 2.0 - 1.0), 0.0);
    if (is_level) { m = vec3<f32>(m.xy * 0.4, 0.0); } // soft relief: stylised, not photographic
    m.z = sqrt(max(1.0 - dot(m.xy, m.xy), 0.0));
    n = perturb(ng, dp1, dp2, duv1, duv2, m);
    rough = clamp(nt.b, 0.08, 1.0);
    metal = nt.a;
  }
  if (is_char) { rough = 0.62; metal = 0.0; }
  if (is_water) {
    let t = u.params.x;
    // the river flows east (+x): the ripples are carried along with it
    let p = i.wpos.xz - vec2<f32>(t * 1.3, 0.0);
    let wx = sin(p.x * 1.3 + t * 1.1) * 0.5 + sin((p.x + p.y) * 2.3 - t * 1.7) * 0.3 + sin(p.x * 5.1 - p.y * 3.7 + t * 2.6) * 0.15;
    let wz = sin(p.y * 1.7 - t * 0.9) * 0.5 + sin((p.x - p.y) * 2.9 + t * 1.3) * 0.3 + sin(p.y * 6.3 + p.x * 2.1 - t * 2.2) * 0.15;
    n = normalize(vec3<f32>(wx * 0.09, 1.0, wz * 0.09));
    rough = 0.08;
    metal = 0.0;
    // foam: long streaks drifting with the current, and a white edge where it laps the banks
    let streak = smoothstep(0.72, 0.9, vnoise(vec2<f32>(p.x * 0.22, i.wpos.z * 1.4)) * 0.75 + vnoise(vec2<f32>(p.x * 0.9, i.wpos.z * 3.0)) * 0.25);
    let edge = smoothstep(3.05, 3.95, abs(i.wpos.z)) * (0.6 + 0.4 * sin(i.wpos.x * 3.1 - t * 4.0));
    let foam = clamp(streak * 0.55 + edge, 0.0, 1.0);
    base = vec4<f32>(mix(base.rgb, vec3<f32>(0.78, 0.84, 0.86), foam * 0.75), base.a);
    rough = mix(0.08, 0.6, foam);
  }
  let albedo = base.rgb;
  let l = normalize(u.sun.xyz);
  let sh = sun_shadow(i.wpos, ng);
  let ndl = max(dot(n, l), 0.0);
  let ndv = max(dot(n, v), 0.001);
  let h = normalize(l + v);
  let ndh = max(dot(n, h), 0.0);
  let vdh = max(dot(v, h), 0.0);
  let a = rough * rough;
  let a2 = a * a;
  let dd = ndh * ndh * (a2 - 1.0) + 1.0;
  let dist_term = a2 / (PI * dd * dd);
  let k = (rough + 1.0) * (rough + 1.0) / 8.0;
  let geo = (ndl / (ndl * (1.0 - k) + k)) * (ndv / (ndv * (1.0 - k) + k));
  let f0 = mix(vec3<f32>(0.04), albedo, metal);
  let fres = f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - vdh, 5.0);
  let spec = dist_term * geo * fres / (4.0 * ndl * ndv + 0.0001);
  let kd = (vec3<f32>(1.0) - fres) * (1.0 - metal);
  let sunc = u.sun_col.rgb * u.sun.w;
  var c = (kd * albedo + spec * PI) * sunc * ndl * sh;
  // the sky: soft light from all around, and reflections (blurrier on rough surfaces)
  // baked occlusion, softened on the level (60% weaker)
  let ao = select(i.ex.z, 1.0 - (1.0 - i.ex.z) * 0.4, is_level);
  let fr = f0 + (max(vec3<f32>(1.0 - rough), f0) - f0) * pow(1.0 - ndv, 5.0);
  let r = reflect(-v, n);
  var env = textureSampleLevel(sky, skysamp, sky_uv(r), rough * u.params.w).rgb * u.params.z;
  env = env * mix(0.22, 1.0, smoothstep(-0.25, 0.15, r.y));
  // light bounced up from the sunlit ground (undersides of ledges and balconies aren't black)
  let bounce = u.sun_col.rgb * (u.sun.w * 0.11 * max(-n.y, 0.0) * max(l.y, 0.0));
  // (the sky fill is generous: shaded sides stay mid-tone, never black)
  if (is_char) {
    c = toon(i, albedo, t1.rgb, n, ng, v, l, sh, q, fw_weave, fw_q);
  } else {
    c = c + (sky_light(n) * 1.45 + u.fog.rgb * 0.22 + bounce) * albedo * (1.0 - metal) * ao + env * fr * ao * u.shadow.w * mix(0.55, 1.0, sh);
  }
  // braziers, torches and muzzle flashes
  let nl = i32(u.shadow.z);
  for (var q = 0; q < nl; q = q + 1) {
    let lp = u.lpos[q];
    let ld = lp.xyz - i.wpos;
    let d2 = dot(ld, ld);
    let dist = sqrt(d2);
    if (dist < lp.w) {
      let ln = ld / max(dist, 0.001);
      let x = dist / lp.w;
      let fall = pow(clamp(1.0 - x * x * x * x, 0.0, 1.0), 2.0) / (d2 + 1.0);
      let lh = normalize(ln + v);
      let sp = pow(max(dot(n, lh), 0.0), mix(8.0, 80.0, 1.0 - rough)) * (1.0 - rough) * 0.6;
      c = c + (albedo * (1.0 - metal) * max(dot(n, ln), 0.0) + vec3<f32>(sp)) * u.lcol[q].rgb * fall;
    }
  }
  var outc = fogit(c, i.wpos);
  if (i.rim.x + i.rim.y + i.rim.z > 0.0) {
    // team-coloured rim light: brightest at the silhouette, and the fog only dims it a
    // little, so players stand out against stone and through a couple of metres of haze
    let fr2 = pow(1.0 - clamp(dot(ng, v), 0.0, 1.0), i.rim.w);
    let d = length(i.wpos - u.cam.xyz);
    let ff = clamp(1.0 - exp(-d * u.fog.w * 1.2), 0.0, 1.0);
    outc = outc + i.rim.rgb * (fr2 * (1.0 - ff * 0.35) + 0.012 * (1.0 - ff * 0.5));
  }
  return vec4<f32>(outc, base.a);
}

// solid surfaces: colour, and in alpha how far away they are (for the effects after)
@fragment fn fs(i: VOut) -> @location(0) vec4<f32> {
  let s = shade(i);
  return vec4<f32>(s.rgb, dot(i.wpos - u.cam.xyz, u.fwd.xyz));
}

// see-through things: colour and opacity (the target keeps its distance)
@fragment fn fs_blend(i: VOut) -> @location(0) vec4<f32> {
  return shade(i);
}

// The fighters' outlines: the skinned mesh pushed out along its smoothed normals and drawn
// inside out (front faces culled), so only a rim shows round the silhouette and the creases.
// About two pixels wide up close, thinning to one far away (cam.w is the picture's height).
@vertex fn vs_skin_outline(v: SIn, @builtin(instance_index) ii: u32) -> VOut {
  let m = skin_matrix(v, ii);
  let wp = (m * vec4<f32>(v.pos, 1.0)).xyz + sway_offset(v, ii);
  let n = normalize((m * vec4<f32>(v.on, 0.0)).xyz);
  let low = v.mat & 255u;
  let lay = v.mat >> 8u;
  let kind = (low >> 1u) & 7u;
  let d = length(wp - u.cam.xyz);
  let px = mix(2.0, 1.0, smoothstep(3.0, 30.0, d)) * max(u.cam.w / 1080.0, 0.65);
  let thick = px * 2.0 * u.up.w / max(u.cam.w, 1.0) * d;
  let wp2 = wp + n * thick;
  var o: VOut;
  o.clip = u.vp * vec4<f32>(wp2, 1.0);
  // no line round the eyes, the eyebrows or a glowing trim (or the plain mannequin)
  if (kind == 1u || kind == 2u || kind == 4u || lay >= 255u) { o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0); }
  o.wpos = wp2; o.nrm = n; o.uv = v.uv;
  o.col = insts[ii].tint0;
  o.ex = vec4<f32>(f32(lay), select(0.0, 1.0, (low & 1u) == 1u), 0.0, 0.0);
  o.rim = vec4<f32>(0.0, 0.0, 0.0, 0.0);
  o.bp = v.pos; o.bn = v.on;
  return o;
}

// the line: a dark shade of the surface's own colour (softer than black)
@fragment fn fs_outline(i: VOut) -> @location(0) vec4<f32> {
  var c = textureSample(ctex, samp, i.uv, i32(max(i.ex.x, 0.0) + 0.5)).rgb;
  if (i.ex.y > 0.5) { c = c * mix(vec3<f32>(1.0, 1.0, 1.0), i.col.rgb * 1.35, 0.62); }
  c = max(mix(vec3<f32>(lum3(c)), c, 1.4), vec3<f32>(0.0)) * 0.16 + vec3<f32>(0.004, 0.004, 0.006);
  return vec4<f32>(fogit(c, i.wpos), dot(i.wpos - u.cam.xyz, u.fwd.xyz));
}

@vertex fn vs_skin(v: SIn, @builtin(instance_index) ii: u32) -> VOut {
  let m = skin_matrix(v, ii);
  let wp = (m * vec4<f32>(v.pos, 1.0)).xyz + sway_offset(v, ii);
  var o: VOut;
  o.clip = u.vp * vec4<f32>(wp, 1.0);
  o.wpos = wp;
  o.bp = v.pos;
  o.bn = v.nrm;
  o.nrm = normalize((m * vec4<f32>(v.nrm, 0.0)).xyz);
  let lay = v.mat >> 8u;
  let low = v.mat & 255u;
  o.uv = v.uv;
  var ex = vec4<f32>(-1.0, 0.0, 1.0, 0.35);
  if (lay < 255u) {
    // a textured fighter: its own colours, the team colour only on its accent pieces (hood,
    // shoulder guard or tunic), and a red wash when hurt
    let acc = mix(vec3<f32>(1.0, 1.0, 1.0), insts[ii].tint0.rgb * 1.35, 0.62);
    let base = select(vec3<f32>(1.0, 1.0, 1.0), acc, (low & 1u) == 1u);
    o.col = vec4<f32>(mix(base, vec3<f32>(1.5, 0.35, 0.3), insts[ii].tint1.w), 1.0);
    // (the kind of surface goes to the toon shader in ex.w)
    let kind = (low >> 1u) & 7u;
    ex = vec4<f32>(f32(lay), 0.5, 1.0, f32(kind));
    if (kind == 4u) {
      // a glowing trim: unlit, in the player's colour; a soft accent, bright (and blooming) only
      // while the fighter's ability is on (tint0.w)
      let tc = insts[ii].tint0.rgb;
      o.col = vec4<f32>(tc / max(max(tc.r, max(tc.g, tc.b)), 0.05) * (0.5 + 2.2 * insts[ii].tint0.w), 1.0);
      ex = vec4<f32>(-1.0, 2.0, 1.0, 0.0);
    }
  } else {
    o.col = vec4<f32>(select(insts[ii].tint1, insts[ii].tint0, low == 0u).rgb, 1.0);
  }
  // the body part just hit flashes white: test the joint that moves this vertex most
  var jd = v.j.x; var wd = v.w.x;
  if (v.w.y > wd) { jd = v.j.y; wd = v.w.y; }
  if (v.w.z > wd) { jd = v.j.z; wd = v.w.z; }
  if (v.w.w > wd) { jd = v.j.w; wd = v.w.w; }
  let fl = insts[ii].flash;
  var word = bitcast<u32>(fl.x);
  if (jd >= 64u) { word = bitcast<u32>(fl.z); } else if (jd >= 32u) { word = bitcast<u32>(fl.y); }
  if (fl.w > 0.0 && ((word >> (jd & 31u)) & 1u) == 1u) {
    o.col = vec4<f32>(mix(o.col.rgb, vec3<f32>(2.2, 2.2, 2.2), fl.w), o.col.a);
  }
  o.ex = ex;
  o.rim = insts[ii].rim;
  return o;
}

struct SkyOut { @builtin(position) clip: vec4<f32>, @location(0) ndc: vec2<f32> };
@vertex fn vs_sky(@builtin(vertex_index) vi: u32) -> SkyOut {
  var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
  var o: SkyOut;
  o.clip = vec4<f32>(p[vi], 0.0, 1.0);
  o.ndc = p[vi];
  return o;
}
@fragment fn fs_sky(i: SkyOut) -> @location(0) vec4<f32> {
  let d = normalize(u.fwd.xyz + u.right.xyz * i.ndc.x * u.right.w + u.up.xyz * i.ndc.y * u.up.w);
  var c = textureSampleLevel(sky, skysamp, sky_uv(d), 0.0).rgb * u.params.z;
  // haze at the horizon, matching the fog
  c = mix(c, u.fog.rgb, clamp(1.0 - smoothstep(-0.02, 0.14, d.y), 0.0, 1.0) * 0.4);
  return vec4<f32>(c, 1000.0);
}
"#;

/// Depth from the sun: everything solid casts a shadow (not particles, glows or leaves).
const SHADOW_SHADER: &str = r#"
@vertex fn vs_shadow(v: VIn) -> @builtin(position) vec4<f32> {
  if (v.ex.y > 1.5 || (v.ex.x > 11.5 && v.ex.x < 12.5)) { return vec4<f32>(0.0, 0.0, -2.0, 1.0); }
  return u.light_vp * vec4<f32>(v.pos, 1.0);
}
@vertex fn vs_skin_shadow(v: SIn, @builtin(instance_index) ii: u32) -> @builtin(position) vec4<f32> {
  let wp = (skin_matrix(v, ii) * vec4<f32>(v.pos, 1.0)).xyz + sway_offset(v, ii);
  return u.light_vp * vec4<f32>(wp, 1.0);
}
"#;

/// Effects after the scene: ambient occlusion, bloom, light shafts and the final picture.
const POST_SHADER: &str = r#"
struct P { size: vec4<f32>, sun: vec4<f32>, flags: vec4<f32>, lift: vec4<f32>, gamma: vec4<f32>, gain: vec4<f32>, misc: vec4<f32>, sun_col: vec4<f32>, right: vec4<f32>, up: vec4<f32>, fwd: vec4<f32>, misc2: vec4<f32> };
@group(0) @binding(0) var<uniform> p: P;
@group(0) @binding(1) var ls: sampler;
@group(0) @binding(2) var ta: texture_2d<f32>;
@group(0) @binding(3) var tb: texture_2d<f32>;
@group(0) @binding(4) var tc: texture_2d<f32>;
@group(0) @binding(5) var td: texture_2d<f32>;

struct FOut { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs_full(@builtin(vertex_index) vi: u32) -> FOut {
  var q = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
  var o: FOut;
  o.clip = vec4<f32>(q[vi], 0.0, 1.0);
  o.uv = vec2<f32>(q[vi].x * 0.5 + 0.5, 0.5 - q[vi].y * 0.5);
  return o;
}

fn lum(c: vec3<f32>) -> f32 { return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722)); }

// where a pixel is, relative to the camera, from its stored distance
fn view_pos(uv: vec2<f32>, z: f32) -> vec3<f32> {
  let nx = uv.x * 2.0 - 1.0;
  let ny = 1.0 - uv.y * 2.0;
  return (p.fwd.xyz + p.right.xyz * (nx * p.right.w) + p.up.xyz * (ny * p.up.w)) * z;
}

@fragment fn fs_ao(i: FOut) -> @location(0) vec4<f32> {
  let z = textureSampleLevel(ta, ls, i.uv, 0.0).a;
  if (z > 300.0 || z <= 0.0) { return vec4<f32>(1.0); }
  let px = p.size.xy * 2.0;
  let pc = view_pos(i.uv, z);
  let zx = textureSampleLevel(ta, ls, i.uv + vec2<f32>(px.x, 0.0), 0.0).a;
  let zy = textureSampleLevel(ta, ls, i.uv + vec2<f32>(0.0, px.y), 0.0).a;
  var n = normalize(cross(view_pos(i.uv + vec2<f32>(px.x, 0.0), zx) - pc, view_pos(i.uv + vec2<f32>(0.0, px.y), zy) - pc));
  if (dot(n, pc) > 0.0) { n = -n; }
  let radius = 0.55;
  // the radius on screen shrinks with distance
  let ruv = radius / (z * p.right.w) * 0.5;
  let jitter = fract(sin(dot(floor(i.uv * p.size.zw), vec2<f32>(12.9898, 78.233))) * 43758.5453) * 6.283;
  var occ = 0.0;
  for (var k = 0; k < 12; k = k + 1) {
    let fk = f32(k);
    let ang = fk * 2.39996 + jitter;
    let rr = ruv * (fk + 0.5) / 12.0;
    let suv = i.uv + vec2<f32>(cos(ang), sin(ang) * p.size.z / p.size.w) * rr;
    let zs = textureSampleLevel(ta, ls, suv, 0.0).a;
    let d = view_pos(suv, zs) - pc;
    let dl = length(d);
    if (dl > 0.001) {
      occ = occ + max(dot(n, d / dl) - 0.12, 0.0) * (1.0 - smoothstep(radius * 0.5, radius * 1.6, dl));
    }
  }
  let ao = clamp(1.0 - occ / 12.0 * 2.2, 0.0, 1.0);
  return vec4<f32>(ao, ao, ao, 1.0);
}

// the bright parts, at a quarter size (bloom starts here)
@fragment fn fs_bright(i: FOut) -> @location(0) vec4<f32> {
  let o = p.size.xy;
  var c = textureSampleLevel(ta, ls, i.uv + vec2<f32>(-o.x, -o.y), 0.0).rgb;
  c += textureSampleLevel(ta, ls, i.uv + vec2<f32>(o.x, -o.y), 0.0).rgb;
  c += textureSampleLevel(ta, ls, i.uv + vec2<f32>(-o.x, o.y), 0.0).rgb;
  c += textureSampleLevel(ta, ls, i.uv + vec2<f32>(o.x, o.y), 0.0).rgb;
  c = min(c * 0.25, vec3<f32>(40.0));
  let l = lum(c);
  let knee = clamp(l - 1.8, 0.0, 6.0);
  return vec4<f32>(c * (knee / max(l, 0.0001)), 1.0);
}

fn blur(uv: vec2<f32>, dir: vec2<f32>) -> vec4<f32> {
  var w = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
  var c = textureSampleLevel(ta, ls, uv, 0.0).rgb * w[0];
  for (var k = 1; k < 5; k = k + 1) {
    let o = dir * f32(k) * 1.6;
    c += textureSampleLevel(ta, ls, uv + o, 0.0).rgb * w[k];
    c += textureSampleLevel(ta, ls, uv - o, 0.0).rgb * w[k];
  }
  return vec4<f32>(c, 1.0);
}
@fragment fn fs_blur_h(i: FOut) -> @location(0) vec4<f32> { return blur(i.uv, vec2<f32>(p.size.x * 4.0, 0.0)); }
@fragment fn fs_blur_v(i: FOut) -> @location(0) vec4<f32> { return blur(i.uv, vec2<f32>(0.0, p.size.y * 4.0)); }

// light shafts: open sky near the sun, smeared towards it
@fragment fn fs_rays(i: FOut) -> @location(0) vec4<f32> {
  if (p.sun.z <= 0.0) { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
  let steps = 32;
  let delta = (p.sun.xy - i.uv) / f32(steps) * 0.85;
  var uv = i.uv;
  var decay = 1.0;
  var acc = 0.0;
  for (var k = 0; k < steps; k = k + 1) {
    uv = uv + delta;
    let s = textureSampleLevel(ta, ls, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0);
    let sky = select(0.0, min(lum(s.rgb), 4.0), s.a > 900.0);
    acc = acc + sky * decay;
    decay = decay * 0.955;
  }
  let a = acc / f32(steps) * p.sun.z;
  return vec4<f32>(a, a, a, 1.0);
}

fn aces(x: vec3<f32>) -> vec3<f32> {
  let a = 2.51; let b = 0.03; let c = 2.43; let d = 0.59; let e = 0.14;
  return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// depth of field: how far (pixels) a point at distance z blurs, focused at p.flags.w
fn coc(z: f32) -> f32 {
  let f = p.flags.w;
  return p.misc2.w * smoothstep(0.06, 0.55, abs(z - f) / max(z, 0.1));
}

@fragment fn fs_comp(i: FOut) -> @location(0) vec4<f32> {
  let s = textureSampleLevel(ta, ls, i.uv, 0.0);
  var c = s.rgb;

  if (p.misc2.w > 0.0) {
    // gather: a nearby point adds to this pixel when its own blur reaches this far; things
    // behind this pixel can't blur over it (so the sharp subject keeps clean edges)
    let cc = coc(s.a);
    var acc = c;
    var wsum = 1.0;
    for (var k = 0; k < 32; k = k + 1) {
      let fk = f32(k) + 0.5;
      let r = sqrt(fk / 32.0) * p.misc2.w;
      let a = fk * 2.39996;
      let t = textureSampleLevel(ta, ls, i.uv + vec2<f32>(cos(a), sin(a)) * r * p.size.xy, 0.0);
      let tc = coc(t.a);
      let reach = select(tc, min(tc, cc), t.a > s.a + 0.3);
      let w = clamp(reach - r + 1.0, 0.0, 1.0);
      acc = acc + t.rgb * w;
      wsum = wsum + w;
    }
    c = acc / wsum;
    // and darker the further out of focus, so the subject and the menus stand out
    c = c * (1.0 - p.misc.w * smoothstep(0.0, 1.0, cc / p.misc2.w));
  }

  if (p.flags.x > 0.5 && s.a < 900.0) {
    let ao = textureSampleLevel(tb, ls, i.uv, 0.0).r;
    c = c * mix(1.0, ao, p.misc2.z);
  }
  if (p.flags.y > 0.5) {
    c = c + textureSampleLevel(tc, ls, i.uv, 0.0).rgb * p.misc.y;
  }
  if (p.flags.z > 0.5) {
    c = c + p.sun_col.rgb * textureSampleLevel(td, ls, i.uv, 0.0).r * p.misc.z;
  }
  c = aces(c * p.misc2.y);
  if (p.misc2.x > 0.5) {
    // the map's mood: lift / gamma / gain, saturation and contrast
    c = max(p.gain.rgb * (c + p.lift.rgb * (vec3<f32>(1.0) - c)), vec3<f32>(0.0));
    c = pow(c, vec3<f32>(1.0) / max(p.gamma.rgb, vec3<f32>(0.2)));
    let l = lum(c);
    c = mix(vec3<f32>(l), c, p.lift.w);
    c = clamp((c - 0.5) * p.gamma.w + 0.5, vec3<f32>(0.0), vec3<f32>(1.0));
  }
  if (p.misc.x > 0.0) {
    let d = length((i.uv - 0.5) * vec2<f32>(1.0, p.size.w / p.size.z)) * 1.35;
    c = c * (1.0 - p.misc.x * smoothstep(0.35, 1.05, d));
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

/// A layer the renderer paints itself (banner cloth, fabric): colour and a flat, rough surface.
fn painted_layer(layer: u32) -> (Vec<u8>, Vec<u8>) {
    let mut img = image::RgbaImage::new(TEX, TEX);
    for y in 0..TEX {
        for x in 0..TEX {
            let c = texel(layer, x, y);
            let b = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
            img.put_pixel(x, y, image::Rgba([b(c[0]), b(c[1]), b(c[2]), 255]));
        }
    }
    let big = image::imageops::resize(&img, env::TEX, env::TEX, image::imageops::FilterType::Triangle);
    let n = (env::TEX * env::TEX) as usize;
    let mut surf = vec![0u8; n * 4];
    for k in 0..n {
        surf[k * 4] = 128;
        surf[k * 4 + 1] = 128;
        surf[k * 4 + 2] = 205;
        surf[k * 4 + 3] = 0;
    }
    (big.into_raw(), surf)
}

/// The level's material layers with their mip levels: colour (sRGB, alpha cuts out leaves) and
/// surface (normal x / y, roughness, metalness). The CC0 textures decode in parallel.
fn build_world_layers() -> (Vec<Vec<Vec<u8>>>, Vec<Vec<Vec<u8>>>) {
    let size = env::TEX;
    let done: Vec<(Vec<Vec<u8>>, Vec<Vec<u8>>)> = std::thread::scope(|s| {
        let jobs: Vec<_> = (0..LAYERS)
            .map(|l| {
                s.spawn(move || {
                    let (col, surf) = env::layer(l).unwrap_or_else(|| painted_layer(l));
                    (env::mips(col, size), env::mips(surf, size))
                })
            })
            .collect();
        jobs.into_iter().map(|j| j.join().unwrap_or_else(|_| (env::mips(vec![128; (size * size * 4) as usize], size), env::mips(vec![128; (size * size * 4) as usize], size)))).collect()
    });
    done.into_iter().unzip()
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

/// A prop placed in the level, its triangles moved into place.
fn prop_into(mb: &mut MB, p: &crate::world::Prop, map: MapId) {
    let (s, c) = p.yaw.sin_cos();
    // the model's +x, +y, +z: across, up, out (its front faces yaw_fwd(yaw))
    let ax = v3(c, 0.0, -s);
    let az = v3(s, 0.0, c);
    // the Courtyard's accent is blue: all its banners are blue
    let team = match if map == MapId::Courtyard && p.team < 2 { 1 } else { p.team } {
        0 => Some([0.78f32, 0.1, 0.09, 1.0]),
        1 => Some([0.1, 0.2, 0.85, 1.0]),
        _ => None,
    };
    for v in env::prop_mesh(p.kind) {
        let q = v.pos * p.scale;
        let pos = p.pos + ax * q.x + V3::UP * q.y + az * q.z;
        let n = (ax * v.nrm.x + V3::UP * v.nrm.y + az * v.nrm.z).norm();
        if v.glow {
            // a lantern's glass: unlit and warm (bright enough to bloom)
            mb.vert(pos, n, v.uv, [2.4, 1.35, 0.55, 1.0], [-1.0, 2.0, 1.0, 0.0]);
            continue;
        }
        let col = match (v.cloth, team) {
            (true, Some(t)) => t,
            _ => [v.tint[0], v.tint[1], v.tint[2], 1.0],
        };
        mb.vert(pos, n, v.uv, col, [v.layer as f32, 0.0, 1.0, 0.3]);
    }
}

/// Ivy down a wall: a wandering stem with leaves on both sides (CC0 Quaternius leaf texture).
fn vine_into(mb: &mut MB, v: &Vine) {
    let mut rng = Rng((v.seed as u64).wrapping_mul(2_654_435_761).wrapping_add(97) | 1);
    let along = v3(v.out.z, 0.0, -v.out.x);
    // leaf pictures in the atlas (u0, v0, u1, v1): two heart leaves and a long one
    let rects = [[0.781f32, 0.078, 0.961, 0.268], [0.773, 0.312, 0.965, 0.566], [0.055, 0.049, 0.186, 0.43]];
    let steps = ((v.len / 0.16).ceil() as usize).max(3);
    let phase = rng.range(0.0, 6.28);
    let drift = rng.range(-0.25, 0.25);
    let stem_col = lin([58, 74, 34]);
    let point = |t: f32| -> V3 { v.top + v.out * (0.035 + 0.02 * (t * 9.0 + phase).sin().abs()) - V3::UP * (v.len * t) + along * ((t * 6.5 + phase).sin() * 0.1 + drift * t) };
    let leaf = |mb: &mut MB, rng: &mut Rng, c: V3, size: f32| {
        let r = rects[(rng.f() * 3.0) as usize % 3];
        let ang = rng.range(-0.9, 0.9);
        // the leaf hangs downward, turned a little, tipped out from the wall
        let down = (V3::UP * -ang.cos() + along * ang.sin()).norm();
        let tip = (down + v.out * rng.range(0.15, 0.55)).norm();
        let side = tip.cross(v.out).norm();
        let n = side.cross(tip).norm();
        let w = size * 0.5;
        let h = size;
        let p = [c - side * w, c + side * w, c + side * w + tip * h, c - side * w + tip * h];
        let shade = rng.range(0.78, 1.08);
        mb.quad(p, n, [[r[0], r[1]], [r[2], r[1]], [r[2], r[3]], [r[0], r[3]]], [shade, shade, shade, 1.0], [env::L_LEAVES as f32, 0.0, 1.0, 0.2]);
    };
    for k in 0..steps {
        let t0 = k as f32 / steps as f32;
        let t1 = (k + 1) as f32 / steps as f32;
        mb.tube(point(t0), point(t1), 0.013, 0.011, along, 1.0, stem_col, -1.0, 0.1);
        for _ in 0..2 {
            let c = point(t0 + rng.range(0.0, 1.0) / steps as f32) + along * rng.range(-0.07, 0.07);
            let size = rng.range(0.13, 0.21) * (1.0 - 0.35 * t0);
            leaf(mb, &mut rng, c, size);
        }
    }
    // a thicker clump along the top
    for _ in 0..7 {
        let c = v.top + v.out * 0.05 + along * rng.range(-0.45, 0.45) - V3::UP * rng.range(0.0, 0.25);
        let size = rng.range(0.16, 0.24);
        leaf(mb, &mut rng, c, size);
    }
}

/// A stable random number for a block (where its texture starts, how it's turned).
fn block_hash(b: &crate::math::Aabb) -> u32 {
    let f = |v: f32| (v * 100.0).round() as i32 as u32;
    let mut h = f(b.min.x).wrapping_mul(73_856_093) ^ f(b.min.y).wrapping_mul(19_349_663) ^ f(b.min.z).wrapping_mul(83_492_791) ^ f(b.max.x).wrapping_mul(2_654_435_761) ^ f(b.max.z).wrapping_mul(40_503);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^ (h >> 15)
}

/// How a box is drawn.
struct BoxLook {
    layer: f32,
    tint: [f32; 4],
    tile: f32,
    shine: f32,
    /// Edge bevel (0 for none).
    bevel: f32,
    uv_off: [f32; 2],
    /// Quarter turns of the texture on its top (floors and tops don't all line up).
    turn: u32,
    top_only: bool,
}

impl BoxLook {
    fn new(world: &World, s: env::Surface, b: &crate::math::Aabb, bevel: f32) -> BoxLook {
        let h = block_hash(b);
        let tile = env::layer_tile(s.layer);
        BoxLook {
            layer: s.layer as f32,
            tint: lin(s.tint),
            tile,
            shine: s.shine,
            bevel,
            uv_off: [(h & 1023) as f32 / 1023.0, ((h >> 10) & 1023) as f32 / 1023.0],
            turn: (h >> 20) & 3,
            top_only: false,
        }
        .map_ok(world)
    }
    fn map_ok(self, _world: &World) -> BoxLook {
        self
    }
    fn darker(mut self, k: f32) -> BoxLook {
        for c in 0..3 {
            self.tint[c] *= k;
        }
        self
    }
}

/// Texture coordinates on a face: world-space, the same way up on every side as seen from
/// outside (no mirrored faces), offset per block; tops turned per block.
fn face_uv(n: V3, p: V3, lk: &BoxLook) -> [f32; 2] {
    let (u, v) = if n.y > 0.5 {
        (p.x, p.z)
    } else if n.y < -0.5 {
        (p.x, -p.z)
    } else {
        // the face's right, looking at it from outside
        let right = v3(-n.z, 0.0, n.x);
        (p.dot(right), -p.y)
    };
    let (u, v) = (u / lk.tile, v / lk.tile);
    let (u, v) = if n.y.abs() > 0.5 {
        match lk.turn {
            1 => (-v, u),
            2 => (-u, -v),
            3 => (v, -u),
            _ => (u, v),
        }
    } else {
        (u, v)
    };
    [u + lk.uv_off[0], v + lk.uv_off[1]]
}

/// Ambient occlusion baked per vertex: wall bases and corners a little darker.
fn vertex_ao(world: &World, n: V3, p: V3) -> f32 {
    let mut ao = 1.0f32;
    if n.y.abs() < 0.5 {
        ao = 0.62 + 0.38 * (p.y / 1.8).clamp(0.0, 1.0);
    } else if n.y > 0.5 {
        let q = p + v3(0.0, 0.25, 0.0);
        for d in [v3(1.0, 0.0, 0.0), v3(-1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), v3(0.0, 0.0, -1.0)] {
            if let Some((t, _)) = world.raycast(q, d, 1.2) {
                ao -= 0.15 * (1.0 - t / 1.2);
            }
        }
    }
    ao.max(0.45)
}

/// A box with bevelled edges: each face shrunk by the bevel, a slanted strip along every edge
/// and a small triangle at every corner, so blocks read as cut stone instead of flat boxes.
fn box_into(mb: &mut MB, world: &World, b: &crate::math::Aabb, lk: &BoxLook) {
    let lo = [b.min.x, b.min.y, b.min.z];
    let hi = [b.max.x, b.max.y, b.max.z];
    let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let bev = lk.bevel.min(size[0].min(size[1]).min(size[2]) * 0.3).max(0.0);
    // faces: (axis, sign)
    let faces = [(0usize, 1.0f32), (0, -1.0), (1, 1.0), (1, -1.0), (2, 1.0), (2, -1.0)];
    let drawn = |a: usize, s: f32| -> bool {
        if lk.top_only {
            return a == 1 && s > 0.0;
        }
        !(a == 1 && s < 0.0 && lo[1] <= 0.01)
    };
    let mk = |c: [f32; 3]| v3(c[0], c[1], c[2]);
    let unit = |a: usize, s: f32| {
        let mut c = [0.0f32; 3];
        c[a] = s;
        mk(c)
    };
    let plane = |a: usize, s: f32| if s > 0.0 { hi[a] } else { lo[a] };
    let inset = |a: usize, s: f32| if s > 0.0 { hi[a] - bev } else { lo[a] + bev };
    // the range of axis k left on a face once the bevels on its sides are taken off
    let range = |k: usize| {
        let a0 = if bev > 0.0 && drawn(k, -1.0) { lo[k] + bev } else { lo[k] };
        let a1 = if bev > 0.0 && drawn(k, 1.0) { hi[k] - bev } else { hi[k] };
        (a0, a1)
    };
    let vert = |mb: &mut MB, p: V3, n: V3, uvn: V3| {
        let ao = vertex_ao(world, uvn, p);
        mb.vert(p, n, face_uv(uvn, p, lk), lk.tint, [lk.layer, 0.0, ao, lk.shine]);
    };
    for &(a, s) in faces.iter() {
        if !drawn(a, s) {
            continue;
        }
        let n = unit(a, s);
        let (ku, kv) = ((a + 1) % 3, (a + 2) % 3);
        let (u0, u1) = range(ku);
        let (v0, v1) = range(kv);
        // Where the face is cut into cells. Tops need a grid (their baked shading darkens near
        // walls); a side's shading only changes with height up to 1.8 m and its texture runs
        // straight across, so a side needs just a cut at 1.8 m, which looks the same with far
        // fewer triangles.
        let cuts = |k: usize, c0: f32, c1: f32| -> Vec<f32> {
            if a == 1 {
                let n = (((c1 - c0) / 0.8).ceil() as usize).max(1);
                (0..=n).map(|i| c0 + (c1 - c0) * i as f32 / n as f32).collect()
            } else if k == 1 && c0 < 1.8 - 0.05 && c1 > 1.8 + 0.05 {
                vec![c0, 1.8, c1]
            } else {
                vec![c0, c1]
            }
        };
        let us = cuts(ku, u0, u1);
        let vs = cuts(kv, v0, v1);
        let at = |i: usize, j: usize| {
            let mut c = [0.0f32; 3];
            c[a] = plane(a, s);
            c[ku] = us[i];
            c[kv] = vs[j];
            mk(c)
        };
        // the baked shading at every corner; cells in a row whose corners all share one value
        // are drawn as one long quad (the same picture, far fewer triangles on open floors)
        let ao: Vec<Vec<f32>> = (0..vs.len()).map(|j| (0..us.len()).map(|i| vertex_ao(world, n, at(i, j))).collect()).collect();
        let flat = |i: usize, j: usize, v: f32| (ao[j][i] - v).abs() < 1e-4 && (ao[j][i + 1] - v).abs() < 1e-4 && (ao[j + 1][i] - v).abs() < 1e-4 && (ao[j + 1][i + 1] - v).abs() < 1e-4;
        for j in 0..vs.len() - 1 {
            let mut i = 0;
            while i < us.len() - 1 {
                let v = ao[j][i];
                let mut e = i + 1;
                if flat(i, j, v) {
                    while e < us.len() - 1 && flat(e, j, v) {
                        e += 1;
                    }
                }
                let q = [at(i, j), at(e, j), at(e, j + 1), at(i, j + 1)];
                let qa = [ao[j][i], ao[j][e], ao[j + 1][e], ao[j + 1][i]];
                for k in [0, 1, 2, 0, 2, 3] {
                    mb.vert(q[k], n, face_uv(n, q[k], lk), lk.tint, [lk.layer, 0.0, qa[k], lk.shine]);
                }
                i = e;
            }
        }
    }
    if bev <= 0.0 {
        return;
    }
    // edge strips
    for (i, &(a, sa)) in faces.iter().enumerate() {
        for &(c, sc) in faces.iter().skip(i + 1) {
            if a == c || !drawn(a, sa) || !drawn(c, sc) {
                continue;
            }
            let e = 3 - a - c;
            let (e0, e1) = range(e);
            let pa = |ev: f32| {
                let mut p = [0.0f32; 3];
                p[a] = plane(a, sa);
                p[c] = inset(c, sc);
                p[e] = ev;
                mk(p)
            };
            let pc = |ev: f32| {
                let mut p = [0.0f32; 3];
                p[a] = inset(a, sa);
                p[c] = plane(c, sc);
                p[e] = ev;
                mk(p)
            };
            let na = unit(a, sa);
            let n = (na + unit(c, sc)).norm();
            // the texture as on the side face, so its pattern wraps over the edge
            let uvn = if a == 1 { unit(c, sc) } else { na };
            let q = [pa(e0), pa(e1), pc(e1), pc(e0)];
            for k in [0, 1, 2, 0, 2, 3] {
                vert(mb, q[k], n, uvn);
            }
        }
    }
    // corners
    for sx in [-1.0f32, 1.0] {
        for sy in [-1.0f32, 1.0] {
            for sz in [-1.0f32, 1.0] {
                if !(drawn(0, sx) && drawn(1, sy) && drawn(2, sz)) {
                    continue;
                }
                let p0 = v3(plane(0, sx), inset(1, sy), inset(2, sz));
                let p1 = v3(inset(0, sx), plane(1, sy), inset(2, sz));
                let p2 = v3(inset(0, sx), inset(1, sy), plane(2, sz));
                let n = v3(sx, sy, sz).norm();
                let uvn = unit(0, sx);
                for p in [p0, p1, p2] {
                    vert(mb, p, n, uvn);
                }
            }
        }
    }
}

/// Trim that dresses the blocks up (looks only, nothing collides with it): caps along wall tops,
/// plinths at their feet, bases and capitals on pillars, a band round the top of platforms.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Trim {
    Cap,
    Plinth,
    Base,
    Capital,
    Band,
}

fn trims(world: &World) -> Vec<(crate::math::Aabb, Mat, Trim)> {
    use crate::math::Aabb;
    let mut out = Vec::new();
    let wraps = |b: &Aabb, p: V3| world.blocks.iter().any(|o| !std::ptr::eq(&o.b, b) && o.b.contains(p) && (o.b.max.x - o.b.min.x) > (b.max.x - b.min.x) + 0.1);
    for bl in &world.blocks {
        let b = bl.b;
        if matches!(bl.mat, Mat::Floor | Mat::Crate | Mat::Hidden) {
            continue;
        }
        let (sx, sy, sz) = (b.max.x - b.min.x, b.max.y - b.min.y, b.max.z - b.min.z);
        if sy < 0.7 || (bl.mat == Mat::Metal && sy < 2.0) {
            continue; // steps, slabs, braziers
        }
        let thin = sx.min(sz);
        let long = sx.max(sz);
        let grounded = b.min.y <= 0.05;
        let grow = |o: f32, y0: f32, y1: f32| Aabb::new(v3(b.min.x - o, y0, b.min.z - o), v3(b.max.x + o, y1, b.max.z + o));
        if sy >= 1.8 && thin <= 1.25 && long >= 1.8 {
            // a wall: a cap on its top (a centimetre proud, so the two never flicker), a plinth at its foot
            out.push((grow(0.06, b.max.y - 0.16, b.max.y + 0.012), bl.mat, Trim::Cap));
            if grounded {
                out.push((grow(0.035, b.min.y, b.min.y + 0.28), bl.mat, Trim::Plinth));
            }
        } else if sy >= 3.0 && long <= 2.2 && thin > 0.3 {
            // a pillar or pilaster: base and capital (unless it already stands on / carries one)
            let c = v3((b.min.x + b.max.x) * 0.5, 0.0, (b.min.z + b.max.z) * 0.5);
            if grounded && !wraps(&b, v3(b.max.x + 0.15, b.min.y + 0.1, c.z)) {
                out.push((grow(0.12, b.min.y, b.min.y + 0.38), bl.mat, Trim::Base));
            }
            if !wraps(&b, v3(b.max.x + 0.15, b.max.y - 0.1, c.z)) {
                out.push((grow(0.12, b.max.y - 0.3, b.max.y), bl.mat, Trim::Capital));
                out.push((grow(0.05, b.max.y - 0.44, b.max.y - 0.38), bl.mat, Trim::Capital));
            }
        } else if sy >= 0.9 && thin > 1.25 {
            // a platform or tower: a band round its top edge, a plinth at its foot
            out.push((grow(0.03, b.max.y - 0.16, b.max.y + 0.012), bl.mat, Trim::Band));
            if grounded {
                out.push((grow(0.03, b.min.y, b.min.y + 0.22), bl.mat, Trim::Plinth));
            }
        }
    }
    out
}

/// Which surface a piece of trim uses: Arena's wall caps and bands are dark wood, Close
/// Quarters' are iron, the Courtyard's marble; bases, plinths and capitals are the block's own
/// stone, a shade darker.
fn trim_surface(map: MapId, own: Mat, t: Trim) -> (env::Surface, f32) {
    match t {
        Trim::Cap | Trim::Band => match map {
            MapId::Arena => (env::surface(map, Mat::Trim), 1.0),
            MapId::CloseQuarters => (env::surface(map, Mat::Trim), 1.0),
            MapId::Courtyard => (env::surface(map, Mat::Stone), 1.0),
            MapId::Bridgeworks => (env::surface(map, Mat::Trim), 1.0),
        },
        _ => (env::surface(map, own), 0.84),
    }
}

fn build_world(world: &World) -> Vec<Vtx> {
    let mut mb = MB::default();
    let map = world.map;
    for bl in world.blocks.iter().chain(world.deco.iter()) {
        if bl.mat == Mat::Hidden {
            continue; // (a prop draws it)
        }
        let b = bl.b;
        let sy = b.max.y - b.min.y;
        let bevel = match bl.mat {
            Mat::Floor => 0.0,
            Mat::Trim | Mat::Crate => 0.03,
            _ => 0.05,
        };
        // a block crossing from one area into another is drawn in pieces, each in its own area's
        // look (crate::zones: the bases' brick and slate, the river's wet stone, the bridges...)
        let (mut xs, mut zs) = crate::zones::cuts(map, b.min, b.max);
        let look_at = |x0: f32, x1: f32, z0: f32, z1: f32| {
            let at = v3((x0 + x1) * 0.5, b.max.y.min((b.min.y + b.max.y) * 0.5 + 0.5), (z0 + z1) * 0.5);
            if bl.mat == Mat::Metal && sy < 2.0 { env::IRON } else { crate::zones::surface(map, bl.mat, at) }
        };
        let same = |a: &env::Surface, c: &env::Surface| a.layer == c.layer && a.tint == c.tint && a.shine == c.shine;
        // a cut only stays where the look really changes across it (fewer pieces, fewer triangles)
        let mut i = 1;
        while i + 1 < xs.len() {
            let keep = (0..zs.len() - 1).any(|j| !same(&look_at(xs[i - 1], xs[i], zs[j], zs[j + 1]), &look_at(xs[i], xs[i + 1], zs[j], zs[j + 1])));
            if keep { i += 1 } else { xs.remove(i); }
        }
        let mut j = 1;
        while j + 1 < zs.len() {
            let keep = (0..xs.len() - 1).any(|i| !same(&look_at(xs[i], xs[i + 1], zs[j - 1], zs[j]), &look_at(xs[i], xs[i + 1], zs[j], zs[j + 1])));
            if keep { j += 1 } else { zs.remove(j); }
        }
        for i in 0..xs.len() - 1 {
            for j in 0..zs.len() - 1 {
                let piece = crate::math::Aabb::new(v3(xs[i], b.min.y, zs[j]), v3(xs[i + 1], b.max.y, zs[j + 1]));
                let surf = look_at(xs[i], xs[i + 1], zs[j], zs[j + 1]);
                let bevel: f32 = bevel;
                let mut lk = BoxLook::new(world, surf, &piece, if xs.len() > 2 || zs.len() > 2 { bevel.min(0.02) } else { bevel });
                lk.top_only = bl.mat == Mat::Floor;
                box_into(&mut mb, world, &piece, &lk);
            }
        }
    }
    for (b, own, t) in trims(world) {
        let (surf, shade) = trim_surface(map, own, t);
        let lk = BoxLook::new(world, surf, &b, 0.025).darker(shade);
        box_into(&mut mb, world, &b, &lk);
    }
    // set dressing: banners, barrels, weapon stands, rubble, torches (CC0 Quaternius)
    for p in &world.props {
        prop_into(&mut mb, p, map);
    }
    for v in &world.vines {
        vine_into(&mut mb, v);
    }
    // water: its surface in 4 m tiles (the shader ripples it and mirrors the sky)
    for w in &world.water {
        let y = w.max.y;
        let mut x = w.min.x;
        while x < w.max.x - 1e-3 {
            let x1 = (x + 4.0).min(w.max.x);
            let mut z = w.min.z;
            while z < w.max.z - 1e-3 {
                let z1 = (z + 4.0).min(w.max.z);
                let q = [v3(x, y, z), v3(x, y, z1), v3(x1, y, z1), v3(x1, y, z)];
                mb.quad(q, V3::UP, [[x * 0.25, z * 0.25], [x * 0.25, z1 * 0.25], [x1 * 0.25, z1 * 0.25], [x1 * 0.25, z * 0.25]], lin([34, 68, 74]), [-2.0, 0.0, 1.0, 3.0]);
                z = z1;
            }
            x = x1;
        }
    }
    // glowing landmarks (crate::zones::glows): clock faces with their hands, the forges' mouths
    for (c, n, r, col) in crate::zones::glows(map) {
        let up = V3::UP;
        let side = n.cross(up).norm();
        let seg = 20;
        for k in 0..seg {
            let a0 = k as f32 / seg as f32 * std::f32::consts::TAU;
            let a1 = (k + 1) as f32 / seg as f32 * std::f32::consts::TAU;
            let p = |a: f32| c + side * (a.cos() * r) + up * (a.sin() * r);
            for q in [c, p(a0), p(a1)] {
                mb.vert(q, n, [0.5, 0.5], [col[0], col[1], col[2], 1.0], [-1.0, 2.0, 1.0, 0.0]);
            }
        }
        if r > 1.0 {
            // a clock: two dark hands (ten past ten) and a rim
            let f = c + n * 0.02;
            let dark = [0.03f32, 0.025, 0.02, 1.0];
            for (ang, len, w) in [(1.05f32, 0.55f32, 0.07f32), (2.3, 0.85, 0.05)] {
                let d = side * ang.cos() + up * ang.sin();
                let wv = d.cross(n).norm() * w;
                let q = [f - wv, f + wv, f + wv + d * (r * len), f - wv + d * (r * len)];
                mb.quad(q, n, [[0.5, 0.5]; 4], dark, [-1.0, 2.0, 1.0, 0.0]);
            }
            for k in 0..24 {
                let a0 = k as f32 / 24.0 * std::f32::consts::TAU;
                let a1 = (k + 1) as f32 / 24.0 * std::f32::consts::TAU;
                let ring = |a: f32, rr: f32| f + side * (a.cos() * rr) + up * (a.sin() * rr);
                mb.quad([ring(a0, r), ring(a1, r), ring(a1, r + 0.12), ring(a0, r + 0.12)], n, [[0.5, 0.5]; 4], [0.12, 0.1, 0.08, 1.0], [-1.0, 2.0, 1.0, 0.0]);
            }
        }
    }
    mb.v
}

/// Foam round everything that stands in the water (bridge piers, the bank steps), stirred by the
/// current: rings of white that pulse and drift downstream. Drawn each frame.
fn water_foam(al: &mut MB, world: &World, time: f32) {
    // where the foam goes never changes during a match: the edges are found once per map (the
    // search looks at every block against every other) and only animated each frame
    thread_local! {
        static EDGES: std::cell::RefCell<Option<((u8, usize, usize), Vec<(V3, V3, V3, usize)>)>> = const { std::cell::RefCell::new(None) };
    }
    let key = (world.map as u8, world.blocks.len(), world.water.len());
    EDGES.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.as_ref().map_or(true, |(k, _)| *k != key) {
            *cell = Some((key, foam_edges(world)));
        }
        let Some((_, edges)) = cell.as_ref() else { return };
        for &(a, c, out, k) in edges {
            let len = (c - a).len();
            let n = ((len / 0.6).ceil() as usize).max(1);
            for s in 0..n {
                let t0 = s as f32 / n as f32;
                let t1 = (s + 1) as f32 / n as f32;
                let pulse = 0.55 + 0.45 * (time * 3.0 + (a.x + a.z) * 1.7 + s as f32 * 1.3 + k as f32).sin();
                // the downstream side trails further
                let reach = 0.35 + 0.25 * pulse + if out.x > 0.5 { 0.5 } else { 0.0 };
                let p0 = a.lerp(c, t0);
                let p1 = a.lerp(c, t1);
                let drift = v3((time * 1.3 + s as f32 * 0.37).fract() * 0.2, 0.0, 0.0);
                al.quad([p0, p1, p1 + out * reach + drift, p0 + out * reach + drift], V3::UP, [[0.5, 0.5]; 4], lina([225, 236, 238], 0.55 * pulse), [-1.0, 2.0, 1.0, 0.0]);
            }
        }
    });
}

/// The water-line edges of everything standing in the water: (start, end, outward, side index).
fn foam_edges(world: &World) -> Vec<(V3, V3, V3, usize)> {
    let mut out_edges = Vec::new();
    for w in &world.water {
        let y = w.max.y + 0.015;
        for bl in world.blocks.iter().filter(|b| b.mat != Mat::Hidden && b.b.min.y < y && b.b.max.y > y - 0.05) {
            let b = bl.b;
            if b.max.x < w.min.x || b.min.x > w.max.x || b.max.z < w.min.z || b.min.z > w.max.z {
                continue;
            }
            // only edges that are in the water
            let (x0, x1, z0, z1) = (b.min.x.max(w.min.x), b.max.x.min(w.max.x), b.min.z.max(w.min.z), b.max.z.min(w.max.z));
            for (k, (a, c, out)) in [(v3(x0, y, z0), v3(x1, y, z0), v3(0.0, 0.0, -1.0)), (v3(x1, y, z1), v3(x0, y, z1), v3(0.0, 0.0, 1.0)), (v3(x0, y, z1), v3(x0, y, z0), v3(-1.0, 0.0, 0.0)), (v3(x1, y, z0), v3(x1, y, z1), v3(1.0, 0.0, 0.0))].into_iter().enumerate() {
                let mid = (a + c) * 0.5 + out * 0.3;
                if mid.z <= w.min.z + 0.1 || mid.z >= w.max.z - 0.1 || world.blocks.iter().any(|o| o.mat != Mat::Hidden && o.b.contains(mid + v3(0.0, 0.05, 0.0))) {
                    continue; // against the bank or inside something else: no water there
                }
                out_edges.push((a, c, out, k));
            }
        }
    }
    out_edges
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
            w if w.is_melee() => {
                if f.slash_t > 0.0 {
                    let p = 1.0 - f.slash_t / w.slash_dur();
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
    let wood = lin([70, 46, 30]);
    let gunmetal = lin([60, 62, 70]);
    match f.weapon {
        w if w.is_melee() => {
            // (blade half-length, blade half-width, blade centre, guard width)
            let (len, wid, guard) = match w {
                Weapon::Dagger => (0.2, 0.025, 0.04),
                Weapon::Greatsword => (0.78, 0.06, 0.12),
                _ => (0.56, 0.03, 0.06),
            };
            mb.obox(hand, &wf, v3(0.022, 0.022, 0.13), lin([30, 22, 22]), -1.0, 0.2);
            mb.obox(hand + wf.apply(v3(0.0, 0.0, 0.14)), &wf, v3(guard, 0.02, 0.02), lin([210, 170, 70]), -1.0, 1.0);
            let blade = if w == Weapon::Greatsword { lin([170, 176, 190]) } else { steel };
            mb.obox(hand + wf.apply(v3(0.0, 0.0, 0.16 + len)), &wf, v3(if w == Weapon::Greatsword { 0.015 } else { 0.008 }, wid, len), blade, -1.0, 2.5);
            if f.slash_t > 0.0 && f.alive {
                let p = 1.0 - f.slash_t / w.slash_dur();
                let steps = 8;
                let mut prev: Option<(V3, V3)> = None;
                let shoulder = at(v3(0.23, 1.4, 0.0));
                let tip = 0.16 + len * 2.0 + 0.05;
                for k in 0..=steps {
                    let q = (p - 0.4 * (1.0 - k as f32 / steps as f32)).max(0.0);
                    let ra = sword_arm(&root, f.slash_idx, q);
                    let fa = ra.mul(&M3::rot_x(-bend_r));
                    let hnd = shoulder + ra.apply(v3(0.0, -0.28, 0.0)) + fa.apply(v3(0.0, -0.26, 0.0));
                    let w2 = fa.mul(&M3::rot_x(PI * 0.5));
                    let a0 = hnd + w2.apply(v3(0.0, 0.0, tip * 0.3));
                    let a1 = hnd + w2.apply(v3(0.0, 0.0, tip));
                    if let Some((b0, b1)) = prev {
                        let al = 0.05 + 0.45 * k as f32 / steps as f32;
                        let c = if f.slash_idx >= 2 || w == Weapon::Greatsword { lina([255, 180, 90], al) } else { lina([170, 210, 255], al) };
                        let n = (b1 - b0).cross(a0 - b0).norm();
                        add_mb.quad([b0, b1, a1, a0], n, [[0.5, 0.5]; 4], c, [-1.0, 2.0, 1.0, 0.0]);
                    }
                    prev = Some((a0, a1));
                }
            }
            if f.guard {
                let c = hand + wf.apply(v3(0.0, 0.0, 0.2 + len));
                add_mb.billboard(cam, c, 0.5 + (time * 20.0).sin().abs() * 0.08, lina([90, 150, 255], 0.5));
            }
        }
        Weapon::Shotgun => {
            mb.obox(hand + wf.apply(v3(0.0, 0.06, 0.05)), &wf, v3(0.04, 0.06, 0.25), wood, 4.0, 0.3);
            mb.tube(hand + wf.apply(v3(0.0, 0.11, 0.0)), hand + wf.apply(v3(0.0, 0.11, 0.7)), 0.03, 0.03, wf.x, 1.0, gunmetal, -1.0, 1.5);
            mb.tube(hand + wf.apply(v3(0.0, 0.06, 0.25)), hand + wf.apply(v3(0.0, 0.06, 0.55)), 0.035, 0.035, wf.x, 1.0, lin([80, 56, 36]), 4.0, 0.3);
        }
        Weapon::Revolver => {
            mb.obox(hand + wf.apply(v3(0.0, 0.05, 0.0)), &wf, v3(0.022, 0.06, 0.04), lin([60, 40, 28]), 4.0, 0.3);
            mb.ellipsoid(hand + wf.apply(v3(0.0, 0.1, 0.07)), &wf, v3(0.035, 0.035, 0.05), lin([90, 92, 100]), -1.0, 1.5, 0.0);
            mb.tube(hand + wf.apply(v3(0.0, 0.11, 0.07)), hand + wf.apply(v3(0.0, 0.11, 0.32)), 0.018, 0.018, wf.x, 1.0, lin([120, 122, 132]), -1.0, 1.8);
        }
        Weapon::Smg => {
            mb.obox(hand + wf.apply(v3(0.0, 0.09, 0.12)), &wf, v3(0.035, 0.05, 0.18), lin([40, 42, 48]), -1.0, 1.0);
            mb.obox(hand + wf.apply(v3(0.0, -0.04, 0.12)), &wf, v3(0.02, 0.09, 0.025), lin([30, 30, 34]), -1.0, 0.5);
            mb.tube(hand + wf.apply(v3(0.0, 0.1, 0.28)), hand + wf.apply(v3(0.0, 0.1, 0.4)), 0.015, 0.015, wf.x, 1.0, gunmetal, -1.0, 1.5);
        }
        Weapon::Rifle => {
            mb.obox(hand + wf.apply(v3(0.0, 0.06, 0.0)), &wf, v3(0.035, 0.06, 0.3), wood, 4.0, 0.3);
            mb.tube(hand + wf.apply(v3(0.0, 0.1, 0.2)), hand + wf.apply(v3(0.0, 0.1, 0.95)), 0.018, 0.016, wf.x, 1.0, gunmetal, -1.0, 1.6);
            mb.tube(hand + wf.apply(v3(0.0, 0.17, 0.05)), hand + wf.apply(v3(0.0, 0.17, 0.3)), 0.025, 0.025, wf.x, 1.0, lin([25, 25, 28]), -1.0, 2.0);
        }
        Weapon::HandCannon => {
            mb.obox(hand + wf.apply(v3(0.0, 0.03, 0.0)), &wf, v3(0.024, 0.065, 0.045), lin([40, 40, 44]), -1.0, 0.6);
            mb.obox(hand + wf.apply(v3(0.0, 0.11, 0.12)), &wf, v3(0.03, 0.04, 0.17), lin([150, 152, 160]), -1.0, 2.0);
            mb.obox(hand + wf.apply(v3(0.0, 0.07, 0.2)), &wf, v3(0.026, 0.02, 0.09), lin([70, 72, 80]), -1.0, 1.2);
        }
        Weapon::Sniper => {
            mb.obox(hand + wf.apply(v3(0.0, 0.05, -0.2)), &wf, v3(0.035, 0.07, 0.22), lin([52, 58, 50]), -1.0, 0.6);
            mb.obox(hand + wf.apply(v3(0.0, 0.09, 0.12)), &wf, v3(0.032, 0.045, 0.2), lin([45, 48, 44]), -1.0, 0.8);
            mb.tube(hand + wf.apply(v3(0.0, 0.11, 0.3)), hand + wf.apply(v3(0.0, 0.11, 1.15)), 0.02, 0.016, wf.x, 1.0, gunmetal, -1.0, 1.6);
            mb.tube(hand + wf.apply(v3(0.0, 0.18, 0.0)), hand + wf.apply(v3(0.0, 0.18, 0.32)), 0.032, 0.032, wf.x, 1.0, lin([20, 20, 22]), -1.0, 2.2);
        }
        Weapon::Rocket => {
            mb.tube(hand + wf.apply(v3(0.0, 0.12, -0.35)), hand + wf.apply(v3(0.0, 0.12, 0.55)), 0.075, 0.075, wf.x, 1.0, lin([62, 74, 58]), -1.0, 0.6);
            mb.tube(hand + wf.apply(v3(0.0, 0.12, 0.55)), hand + wf.apply(v3(0.0, 0.12, 0.62)), 0.09, 0.09, wf.x, 1.0, lin([40, 40, 44]), -1.0, 1.0);
            mb.obox(hand + wf.apply(v3(0.0, 0.0, 0.0)), &wf, v3(0.02, 0.06, 0.03), lin([30, 30, 34]), -1.0, 0.5);
        }
        _ => {}
    }
}
/// The long coat as cloth: a grid of points hanging from a ring at the waist (open at the
/// front), moved by gravity and the body's motion (verlet), held at their spacing, and pushed out
/// of the legs and hips so it drapes over them. Team-coloured.
const CLOTH_COLS: usize = 11;
const CLOTH_ROWS: usize = 7;

/// The cloth each fighter wears at the back: (how far round the waist it goes in radians, its
/// length, its colour, its hem colour). The Ranger's long coat is in the team colour; the
/// Huntress wears a shorter cape of her own brown leather with only a team-coloured hem, so the
/// two read apart from behind.
fn coat_style(body: u8, lk: &Look, outfit: u8) -> (f32, f32, [f32; 4], [f32; 4]) {
    let (span, length, base, trim) = match body {
        1 => (3.0, 0.5, lin([112, 84, 56]), lin(lk.coat)),
        _ => (4.4, 0.68, lin(lk.coat_dark), lin(lk.coat)),
    };
    // a mastery outfit colours the coat or cape (the hem keeps the player's colour)
    match crate::career::OUTFITS.get(outfit as usize).and_then(|o| o.2) {
        Some(c) => (span, length, lin(c), trim),
        None => (span, length, base, trim),
    }
}

fn coat_cloth(mb: &mut MB, p: &character::Posed, st: &mut CharState, time: f32, lk: &Look, sc: f32, body: u8, outfit: u8) {
    let (span, length, base, trim) = coat_style(body, lk, outfit);
    let caps = &p.hit.caps;
    let (hips, legs) = (caps[4], [caps[9], caps[10], caps[11], caps[12]]);
    // the belt mostly stays level (it leans a little with the hips, not with the whole spine)
    let up = ((hips.b - hips.a).norm() * 0.35 + v3(0.0, 0.65, 0.0)).norm();
    let across = caps[11].a - caps[9].a; // left thigh -> right thigh
    let mut right = across - up * across.dot(up);
    if right.len() < 0.01 {
        right = v3(1.0, 0.0, 0.0);
    }
    let right = right.norm();
    let fwd = right.cross(up).norm();
    let center = hips.b + up * (0.07 * sc);
    let len = length * sc;
    // the ring sits just outside the hips and thighs, so the body never fights the fabric
    let (rx, rz) = (0.235 * sc, 0.2 * sc);
    // the waist ring, from the right front, round the back, to the left front
    let ring = |c: usize, flare: f32| -> V3 {
        let a = (c as f32 / (CLOTH_COLS - 1) as f32 - 0.5) * span; // radians from straight back
        center + right * (-a.sin() * rx * flare) - fwd * (a.cos() * rz * flare)
    };
    let n = CLOTH_COLS * CLOTH_ROWS;
    let idx = |r: usize, c: usize| r * CLOTH_COLS + c;
    let feet = caps[10].b.y.min(caps[12].b.y) - 0.02;
    let fresh = st.cloth.len() != n || (st.cloth[0] - ring(0, 1.0)).len() > 1.5;
    if fresh {
        st.cloth.clear();
        for r in 0..CLOTH_ROWS {
            for c in 0..CLOTH_COLS {
                let k = r as f32 / (CLOTH_ROWS - 1) as f32;
                st.cloth.push(ring(c, 1.0 + 0.25 * k) - up * (len * k));
            }
        }
        st.cloth_prev = st.cloth.clone();
        st.cloth_time = time;
    }
    let dt = (time - st.cloth_time).clamp(0.0, 0.05);
    st.cloth_time = time;
    if dt > 0.0 {
        let steps = 3;
        let h = dt / steps as f32;
        let lv = len / (CLOTH_ROWS - 1) as f32;
        // gravity, and a light breeze so the cloth is never quite still
        let breeze = v3((time * 0.7).sin() * 0.7 + (time * 1.9).sin() * 0.25, 0.0, (time * 0.45).cos() * 0.5);
        let gravity = (v3(0.0, -9.8, 0.0) + breeze) * (h * h);
        for _ in 0..steps {
            for i in CLOTH_COLS..n {
                let cur = st.cloth[i];
                // a little air drag: heavy fabric, so it mostly hangs and trails a bit
                let mut vel = (cur - st.cloth_prev[i]) * 0.994;
                let vl = vel.len();
                if vl > 0.12 {
                    vel = vel * (0.12 / vl); // never faster than ~30 m/s: no blow-ups
                }
                st.cloth_prev[i] = cur;
                st.cloth[i] = cur + vel + gravity;
            }
            for c in 0..CLOTH_COLS {
                st.cloth[c] = ring(c, 1.0);
                st.cloth_prev[c] = st.cloth[c];
            }
            for _ in 0..4 {
                // hold the spacing: down the coat, and around it (flaring a little towards the hem)
                for r in 1..CLOTH_ROWS {
                    for c in 0..CLOTH_COLS {
                        let (a, b) = (idx(r - 1, c), idx(r, c));
                        let d = st.cloth[b] - st.cloth[a];
                        let l = d.len().max(1e-5);
                        let fix = d * ((l - lv) / l);
                        if r == 1 {
                            st.cloth[b] = st.cloth[b] - fix;
                        } else {
                            st.cloth[a] = st.cloth[a] + fix * 0.5;
                            st.cloth[b] = st.cloth[b] - fix * 0.5;
                        }
                    }
                    let flare = 1.0 + 0.3 * r as f32 / (CLOTH_ROWS - 1) as f32;
                    for c in 0..CLOTH_COLS - 1 {
                        let rest = (ring(c, flare) - ring(c + 1, flare)).len();
                        let (a, b) = (idx(r, c), idx(r, c + 1));
                        let d = st.cloth[b] - st.cloth[a];
                        let l = d.len().max(1e-5);
                        let fix = d * ((l - rest) / l) * 0.5;
                        st.cloth[a] = st.cloth[a] + fix;
                        st.cloth[b] = st.cloth[b] - fix;
                    }
                }
                // drape over the legs and hips, never inside the body or below the feet
                for i in CLOTH_COLS..n {
                    let mut q = st.cloth[i];
                    let before = q;
                    for cap in legs.iter() {
                        // pushed out sideways only (past a limb's ends the cloth would be flung up)
                        let ab = cap.b - cap.a;
                        let t = (q - cap.a).dot(ab) / ab.dot(ab).max(1e-6);
                        if !(0.0..=1.0).contains(&t) {
                            continue;
                        }
                        let near = cap.a + ab * t;
                        let off = q - near;
                        let rr = cap.r + 0.02 * sc;
                        if off.len() < rr {
                            q = near + if off.len() > 1e-4 { off.norm() } else { -fwd } * rr;
                        }
                    }
                    let rel = q - center;
                    let flat = rel - up * rel.dot(up);
                    let (x, z) = (flat.dot(right) / rx, flat.dot(fwd) / rz);
                    let d = (x * x + z * z).sqrt();
                    if d < 0.9 && d > 1e-4 {
                        let s = 0.9 / d;
                        q = center + up * rel.dot(up) + right * (x * s * rx) + fwd * (z * s * rz);
                    }
                    q.y = q.y.max(feet);
                    // heavy fabric never folds up above the row it hangs from (it can stream out
                    // behind, not flip over)
                    let above = st.cloth[i - CLOTH_COLS].y;
                    q.y = q.y.min(above + 0.01);
                    if (q - before).len() > 1e-5 {
                        // touching the body: it slides along instead of bouncing off
                        st.cloth_prev[i] = q - (st.cloth[i] - st.cloth_prev[i]) * 0.3;
                    }
                    st.cloth[i] = q;
                }
            }
        }
    }
    // smooth normals: each point's from the patches around it, so the cloth shades softly
    // instead of in flat facets
    let mut normals = vec![V3::ZERO; n];
    for r in 0..CLOTH_ROWS - 1 {
        for c in 0..CLOTH_COLS - 1 {
            let q = [st.cloth[idx(r, c)], st.cloth[idx(r, c + 1)], st.cloth[idx(r + 1, c + 1)], st.cloth[idx(r + 1, c)]];
            let fn_ = (q[1] - q[0]).cross(q[3] - q[0]) + (q[2] - q[1]).cross(q[0] - q[1]);
            for k in [idx(r, c), idx(r, c + 1), idx(r + 1, c + 1), idx(r + 1, c)] {
                normals[k] = normals[k] + fn_;
            }
        }
    }
    let normal = |k: usize| if normals[k].len() > 1e-8 { normals[k].norm() } else { -fwd };
    // draw it, both sides lit by the same normals (the coat is thin)
    for r in 0..CLOTH_ROWS - 1 {
        for c in 0..CLOTH_COLS - 1 {
            let ids = [idx(r, c), idx(r, c + 1), idx(r + 1, c + 1), idx(r + 1, c)];
            let q = [st.cloth[ids[0]], st.cloth[ids[1]], st.cloth[ids[2]], st.cloth[ids[3]]];
            // the team colour runs along the hem
            let col = if r == CLOTH_ROWS - 2 { trim } else { base };
            // a fine weave (repeated several times across the cloth, never big blocky squares)
            let (u0, u1) = (c as f32 / (CLOTH_COLS - 1) as f32 * 9.0, (c + 1) as f32 / (CLOTH_COLS - 1) as f32 * 9.0);
            let (v0, v1) = (r as f32 / (CLOTH_ROWS - 1) as f32 * 4.5, (r + 1) as f32 / (CLOTH_ROWS - 1) as f32 * 4.5);
            let uv = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
            for k in [0, 1, 2, 0, 2, 3] {
                mb.vert(q[k], normal(ids[k]), uv[k], col, [7.0, 0.0, 1.0, 0.05]);
            }
        }
    }
}

/// Things attached to an animated character's bones: the weapon in the right hand, the sword's
/// swing trail, hair, and the long coat.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
/// How an execution shows fighter `f` right now: (a stand-in to draw instead, e.g. with the gun
/// out for a point-blank shot; whether they're only a blur and not drawn at all). None when
/// they aren't doing one.
fn exec_view(g: &Game, f: &Fighter) -> Option<(Option<Fighter>, bool)> {
    if f.fin == 0 || !f.alive || f.ghost > 0.0 {
        return None;
    }
    let k = f.fin as usize - 1;
    let p = (f.fin_t / crate::exec::execution(f.fin).dur).clamp(0.0, 1.0);
    if crate::exec::hidden(k, p) {
        return Some((None, true));
    }
    if !crate::exec::gun_out(k, p) {
        return Some((None, false));
    }
    let mut v = f.clone();
    let gun = if f.loadout.gun2.is_melee() || f.loadout.gun2 == Weapon::Shuriken { f.loadout.gun1 } else { f.loadout.gun2 };
    v.weapon = gun;
    v.cur = if gun == f.loadout.gun2 { 2 } else { 1 };
    v.since_shot = 0.0;
    v.scope_t = 0.0;
    // aimed at the one being executed (at their chest, kneeling or not)
    let t = g.f.get(f.fin_target).map_or(f.pos + f.fwd(), |t| t.pos);
    let d = t + v3(0.0, 0.9, 0.0) - (f.pos + v3(0.0, 1.5, 0.0));
    v.pitch = d.y.atan2(d.flat().len().max(0.1)).clamp(-1.2, 1.2);
    Some((Some(v), false))
}

/// A stain, crack or scorch on the floor: flat on the ground under it.
fn decal_into(al: &mut MB, ad: &mut MB, d: &crate::game::Decal) {
    let fade = (d.life / d.max_life.max(0.01)).clamp(0.0, 1.0);
    let fade = (fade * 4.0).min(1.0);
    // on whatever face it lies on: the floor, a wall, a ceiling
    let up = if d.normal.len() > 0.5 { d.normal.norm() } else { v3(0.0, 1.0, 0.0) };
    let t1 = if up.y.abs() > 0.9 { v3(1.0, 0.0, 0.0) } else { v3(0.0, 1.0, 0.0).cross(up).norm() };
    let t2 = up.cross(t1).norm();
    let mut rng = Rng((d.seed as u64).wrapping_mul(2_654_435_761) | 1);
    let flat = |c: V3, r: f32, a: f32| -> [V3; 4] {
        let (s, co) = a.sin_cos();
        let x = (t1 * co + t2 * s) * r;
        let z = (t2 * co - t1 * s) * r;
        [c - x - z, c + x - z, c + x + z, c - x + z]
    };
    let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    match d.kind {
        0 => {
            // a stain: a few soft overlapping splashes and drops
            for k in 0..6 {
                let off = (t1 * rng.range(-0.5, 0.5) + t2 * rng.range(-0.5, 0.5)) * d.size * if k == 0 { 0.0 } else { 1.0 };
                let r = d.size * if k == 0 { 0.55 } else { rng.range(0.12, 0.3) };
                al.quad(flat(d.pos + off + up * (0.001 * k as f32), r, rng.range(0.0, 6.28)), up, uv, lina(d.color, 0.85 * fade), [-1.0, 3.0, 1.0, 0.0]);
            }
        }
        1 => {
            // a crack: dark jagged lines out from the blow, and a darker crater
            al.quad(flat(d.pos, d.size * 0.35, 0.0), up, uv, lina(d.color, 0.75 * fade), [-1.0, 3.0, 1.0, 0.0]);
            for _ in 0..9 {
                let mut a = rng.range(0.0, std::f32::consts::TAU);
                let mut p0 = d.pos + up * 0.002;
                let mut w = 0.06;
                for _ in 0..4 {
                    a += rng.range(-0.6, 0.6);
                    let len = d.size * rng.range(0.12, 0.22);
                    let dir = t1 * a.cos() + t2 * a.sin();
                    let side = up.cross(dir).norm();
                    let p1 = p0 + dir * len;
                    al.quad([p0 - side * w, p1 - side * (w * 0.6), p1 + side * (w * 0.6), p0 + side * w], up, [[0.5, 0.5]; 4], lina(d.color, 0.9 * fade), [-1.0, 2.0, 1.0, 0.0]);
                    p0 = p1;
                    w *= 0.6;
                }
            }
        }
        3 => {
            // a bullet hole: a dark pit, a paler ring of chipped surface round it
            al.quad(flat(d.pos, d.size * 1.6, rng.range(0.0, 6.28)), up, uv, lina([150, 142, 130], 0.22 * fade), [-1.0, 3.0, 1.0, 0.0]);
            al.quad(flat(d.pos + up * 0.002, d.size * 0.55, rng.range(0.0, 6.28)), up, uv, lina(d.color, 0.95 * fade), [-1.0, 3.0, 1.0, 0.0]);
        }
        4 => {
            // a ripple spreading on the water
            let k = 1.0 - (d.life / d.max_life.max(0.01)).clamp(0.0, 1.0);
            let r = d.size * (0.3 + 1.2 * k);
            for s in 0..16 {
                let a0 = s as f32 / 16.0 * std::f32::consts::TAU;
                let a1 = (s + 1) as f32 / 16.0 * std::f32::consts::TAU;
                let ring = |a: f32, rr: f32| d.pos + (t1 * a.cos() + t2 * a.sin()) * rr;
                al.quad([ring(a0, r), ring(a1, r), ring(a1, r + 0.06), ring(a0, r + 0.06)], up, [[0.5, 0.5]; 4], lina(d.color, 0.6 * (1.0 - k)), [-1.0, 2.0, 1.0, 0.0]);
            }
        }
        _ => {
            // a scorch from the smith's hot blade: glowing, cooling fast
            let heat = (d.life / d.max_life.max(0.01)).clamp(0.0, 1.0);
            ad.quad(flat(d.pos, d.size * 0.5, 0.0), up, uv, lina(d.color, 0.7 * heat * heat), [-1.0, 3.0, 1.0, 0.0]);
        }
    }
}
fn character_extras(mb: &mut MB, add_mb: &mut MB, cam: &Cam, f: &Fighter, p: &character::Posed, st: &mut CharState, time: f32, lk: &Look, flash: f32, sc: f32) {
    let tint = |c: [u8; 3]| -> [f32; 4] {
        let l = lin(c);
        [l[0] + (1.0 - l[0]) * flash, l[1] + (0.3 - l[1]).max(0.0) * flash, l[2] + (0.3 - l[2]).max(0.0) * flash, 1.0]
    };
    let dressed = crate::chars::shared().is_some();
    if dressed {
        // the fighters have real hair; the long coat is cloth that swings and drapes
        if f.alive || f.death_t < 3.0 {
            coat_cloth(mb, p, st, time, lk, sc, f.loadout.body, f.loadout.outfit);
        }
    } else {
        // hair cap
        let hf = &p.head_frame;
        mb.ellipsoid(p.head + (hf.y * 0.1 - hf.z * 0.015) * sc, hf, v3(0.105, 0.07, 0.115) * sc, tint(lk.hair), -1.0, 0.4, 0.0);
        mb.ellipsoid(p.head + (hf.y * 0.05 - hf.z * 0.06) * sc, hf, v3(0.1, 0.085, 0.07) * sc, tint(lk.hair), -1.0, 0.4, 0.0);
    }
    // long coat from the waist, trailing behind with speed (the plain mannequin's stiff one)
    if f.alive && !dressed {
        let speed = f.vel.flat().len();
        let air = !f.on_ground && f.wall.is_none();
        let flap = 0.15 + (speed / 12.0).min(1.0) * 0.8 + (time * 9.0).sin() * 0.05 * (speed / RUN_SPEED).min(1.0) + if air { 0.3 } else { 0.0 };
        let fwd = f.fwd();
        let side = v3(fwd.z, 0.0, -fwd.x);
        let waist = p.waist - v3(0.0, 0.05 * sc, 0.0);
        let hem = waist + (v3(0.0, -0.58 + flap * 0.15, 0.0) - fwd * (0.06 + flap * 0.3)) * sc;
        mb.tube(waist, hem, 0.2 * sc, 0.31 * sc, side, 0.78, tint(lk.coat_dark), 7.0, 0.05);
    }
    // weapon
    // a blade sits in the fist (its handle through it); guns sit at the wrist
    let anchor = p.grip;
    weapon_mesh(mb, add_mb, cam, f, anchor, &p.weapon, time);
    let w = f.weapon;
    // charging a heavy: the blade glows, then flashes once it's ready
    if w.is_melee() && f.alive && (f.charged || f.charge_t > 0.2) {
        let len = match w {
            Weapon::Dagger => 0.3,
            Weapon::Greatsword => 1.2,
            _ => 0.9,
        } * sc;
        let (col, size) = if f.charged {
            let pulse = 0.75 + 0.25 * (time * 18.0).sin();
            ([255, 236, 190], 0.2 * pulse)
        } else {
            ([255, 170, 80], 0.11)
        };
        for k in 1..=5 {
            let at = anchor + p.weapon.z * (len * k as f32 / 5.0);
            add_mb.billboard(cam, at, size * sc, lina(col, if f.charged { 0.85 } else { 0.5 }));
        }
        if f.charged {
            add_mb.billboard(cam, anchor + p.weapon.z * (len * 0.6), 0.55 * sc * (0.8 + 0.2 * (time * 18.0).sin()), lina([255, 220, 150], 0.35));
        }
    }
    // swing trail: drawn along the blade's arc only while it's really cutting (from the end of
    // the wind-up, brightest at contact, thinning out through the follow-through)
    let blade_len = match w {
        Weapon::Dagger => 0.2,
        Weapon::Greatsword => 0.78,
        _ => 0.56,
    };
    let tip = 0.16 + blade_len * 2.0 + 0.05;
    if w.is_melee() && f.alive {
        // seconds to the game's hit (negative after it)
        let until = if f.fin > 0 {
            let s = crate::exec::execution(f.fin);
            Some((s.hit - f.fin_t / s.dur) * s.dur)
        } else if f.slashing() {
            Some(f.slash_t - w.slash_dur() * w.hit_left())
        } else {
            None
        };
        if let Some(u) = until {
            let k = if f.fin > 0 {
                // an execution: the blade trails through every cut, brightest at each blow
                let e = crate::exec::execution(f.fin);
                e.cues.iter().filter(|c| matches!(c.1, crate::exec::Cue::Whoosh | crate::exec::Cue::Strike | crate::exec::Cue::Lethal)).map(|c| 1.0 - ((f.fin_t - c.0 * e.dur).abs() / 0.2).min(1.0)).fold(0.0f32, f32::max)
            } else if u > 0.0 {
                1.0 - (u / 0.07).min(1.0)
            } else {
                1.0 - (-u / 0.3).min(1.0) * 0.85
            };
            if k > 0.02 {
                st.trail.push_back((anchor + p.weapon.z * (tip * 0.3), anchor + p.weapon.z * tip, time, k));
            }
            // the tell: a glint runs up the blade through a heavy's or a launcher's wind-up
            // (every swing's wind-up shows it, so a reacting player can guard or parry; the big
            // ones brighter)
            let big = f.slash_idx == SLASH_HEAVY || f.slash_idx == 3 || w == Weapon::Greatsword || f.fin > 0;
            if u > 0.0 {
                let g = (u / w.windup().max(0.05)).min(1.0);
                let at = anchor + p.weapon.z * (tip * (0.35 + 0.65 * (1.0 - g)));
                let size = if big { 0.26 } else { 0.18 };
                add_mb.billboard(cam, at, size * sc, lina([255, 245, 215], 0.95 * g.max(0.35).sqrt()));
            }
        }
    }
    // the trail lingers a moment after the swing, then fades
    while st.trail.front().map_or(false, |e| time - e.2 > 0.2 || time < e.2) {
        st.trail.pop_front();
    }
    let n = st.trail.len();
    let heavy = f.slash_idx >= 2 || w == Weapon::Greatsword;
    // an execution's trail is in the fighter's own colour (their team's in team modes)
    let accent = (f.fin > 0).then(|| {
        let c = PALETTE[(f.color_idx as usize) % PALETTE.len()];
        let m = c[0].max(c[1]).max(c[2]).max(1) as f32;
        [(c[0] as f32 / m * 255.0) as u8, (c[1] as f32 / m * 255.0) as u8, (c[2] as f32 / m * 255.0) as u8]
    });
    for k in 1..n {
        let (b0, b1, tb, kb) = st.trail[k - 1];
        let (a0, a1, ta, ka) = st.trail[k];
        let age = |t0: f32| (1.0 - (time - t0) / 0.2).clamp(0.0, 1.0);
        let al = 0.7 * age(ta).min(age(tb)) * (0.3 + 0.7 * k as f32 / n as f32) * ka.min(kb);
        let c = match accent {
            Some(a) => lina(a, al),
            None if heavy => lina([255, 170, 80], al),
            None => lina([150, 200, 255], al),
        };
        let nrm = (b1 - b0).cross(a0 - b0).norm();
        add_mb.quad([b0, b1, a1, a0], nrm, [[0.5, 0.5]; 4], c, [-1.0, 2.0, 1.0, 0.0]);
        // bright core near the blade's edge
        let m0 = b0.lerp(b1, 0.7);
        let m1 = a0.lerp(a1, 0.7);
        add_mb.quad([m0, b1, a1, m1], nrm, [[0.5, 0.5]; 4], lina([255, 255, 255], al * 0.7), [-1.0, 2.0, 1.0, 0.0]);
    }
}

/// How far the revolver's cylinder has turned: a sixth of a turn for each shot fired, and a
/// quick spin while it's reloaded.
fn revolver_turn(f: &Fighter) -> f32 {
    let fired = f.weapon.mag().saturating_sub(f.mag[f.cur.min(2)]) as f32;
    let spin = if f.reload_t > 0.0 { f.reload_t * 24.0 } else { 0.0 };
    fired * PI / 3.0 + spin
}

/// A weapon held at `hand`, pointing along `wf.z`: its model from `assets/weapons` (with the pump
/// racked and the cylinder turned), or simple shapes if the model didn't load.
fn weapon_mesh(mb: &mut MB, add_mb: &mut MB, cam: &Cam, f: &Fighter, hand: V3, wf: &M3, time: f32) {
    // out of sight: nothing to draw (well behind the camera or far off to the side)
    let to = hand - cam.pos;
    let dist = to.len();
    if dist > 2.5 && to.dot(cam.fwd) < dist * 0.15 {
        return;
    }
    let m = if dist > crate::weapons::FAR { crate::weapons::model_far(f.weapon) } else { crate::weapons::model(f.weapon) };
    if let Some(m) = m {
        mb.v.reserve(m.parts.iter().map(|p| p.tris.len()).sum());
        use crate::weapons::PartKind;
        let slide = if f.weapon == Weapon::Shotgun { f.pump_k() * crate::weapons::PUMP_TRAVEL } else { 0.0 };
        let (s, c) = if matches!(f.weapon, Weapon::Revolver | Weapon::HandCannon) { revolver_turn(f).sin_cos() } else { (0.0, 1.0) };
        let turn = |v: V3| v3(v.x * c - v.y * s, v.x * s + v.y * c, v.z);
        for part in &m.parts {
            for v in &part.tris {
                let (p, n) = match part.kind {
                    PartKind::Body => (v.p, v.n),
                    PartKind::Pump => (v.p - v3(0.0, 0.0, slide), v.n),
                    PartKind::Cylinder => (part.pivot + turn(v.p - part.pivot), turn(v.n)),
                };
                mb.vert(hand + wf.apply(p), wf.apply(n), v.uv, v.col, v.ex);
            }
        }
        if f.weapon.is_melee() && f.guard {
            let len = m.max.z * 0.85;
            add_mb.billboard(cam, hand + wf.apply(v3(0.0, 0.0, len)), 0.5 + (time * 20.0).sin().abs() * 0.08, lina([90, 150, 255], 0.5));
        }
        return;
    }
    let steel = lin([200, 206, 218]);
    let wood = lin([70, 46, 30]);
    let gunmetal = lin([60, 62, 70]);
    match f.weapon {
        w if w.is_melee() => {
            let (len, wid, guard) = match w {
                Weapon::Dagger => (0.2, 0.025, 0.04),
                Weapon::Greatsword => (0.78, 0.06, 0.12),
                _ => (0.56, 0.03, 0.06),
            };
            mb.obox(hand, wf, v3(0.022, 0.022, 0.13), lin([30, 22, 22]), -1.0, 0.2);
            mb.obox(hand + wf.apply(v3(0.0, 0.0, 0.14)), wf, v3(guard, 0.02, 0.02), lin([210, 170, 70]), -1.0, 1.0);
            let blade = if w == Weapon::Greatsword { lin([170, 176, 190]) } else { steel };
            mb.obox(hand + wf.apply(v3(0.0, 0.0, 0.16 + len)), wf, v3(if w == Weapon::Greatsword { 0.015 } else { 0.008 }, wid, len), blade, -1.0, 2.5);
            if f.guard {
                let c = hand + wf.apply(v3(0.0, 0.0, 0.2 + len));
                add_mb.billboard(cam, c, 0.5 + (time * 20.0).sin().abs() * 0.08, lina([90, 150, 255], 0.5));
            }
        }
        Weapon::Shotgun => {
            mb.obox(hand + wf.apply(v3(0.0, 0.0, 0.05)), wf, v3(0.04, 0.06, 0.25), wood, 4.0, 0.3);
            mb.tube(hand + wf.apply(v3(0.0, 0.05, 0.0)), hand + wf.apply(v3(0.0, 0.05, 0.7)), 0.03, 0.03, wf.x, 1.0, gunmetal, -1.0, 1.5);
            // the pump slides back and forward after each shot
            let slide = f.pump_k() * 0.12;
            mb.tube(hand + wf.apply(v3(0.0, 0.0, 0.25 - slide)), hand + wf.apply(v3(0.0, 0.0, 0.55 - slide)), 0.038, 0.038, wf.x, 1.0, lin([80, 56, 36]), 4.0, 0.3);
            mb.tube(hand + wf.apply(v3(0.0, 0.0, 0.0)), hand + wf.apply(v3(0.0, 0.0, 0.25 - slide)), 0.02, 0.02, wf.x, 1.0, gunmetal, -1.0, 1.5);
        }
        Weapon::Revolver => {
            mb.obox(hand + wf.apply(v3(0.0, -0.04, 0.0)), wf, v3(0.022, 0.06, 0.04), lin([60, 40, 28]), 4.0, 0.3);
            mb.ellipsoid(hand + wf.apply(v3(0.0, 0.02, 0.07)), wf, v3(0.035, 0.035, 0.05), lin([90, 92, 100]), -1.0, 1.5, 0.0);
            mb.tube(hand + wf.apply(v3(0.0, 0.03, 0.07)), hand + wf.apply(v3(0.0, 0.03, 0.3)), 0.018, 0.018, wf.x, 1.0, lin([120, 122, 132]), -1.0, 1.8);
        }
        Weapon::Smg => {
            mb.obox(hand + wf.apply(v3(0.0, 0.04, 0.1)), wf, v3(0.035, 0.05, 0.18), lin([40, 42, 48]), -1.0, 1.0);
            mb.obox(hand + wf.apply(v3(0.0, -0.06, 0.12)), wf, v3(0.02, 0.09, 0.025), lin([30, 30, 34]), -1.0, 0.5);
            mb.tube(hand + wf.apply(v3(0.0, 0.05, 0.26)), hand + wf.apply(v3(0.0, 0.05, 0.4)), 0.015, 0.015, wf.x, 1.0, gunmetal, -1.0, 1.5);
        }
        Weapon::Rifle => {
            mb.obox(hand + wf.apply(v3(0.0, 0.0, -0.05)), wf, v3(0.035, 0.06, 0.3), wood, 4.0, 0.3);
            mb.tube(hand + wf.apply(v3(0.0, 0.04, 0.15)), hand + wf.apply(v3(0.0, 0.04, 0.95)), 0.018, 0.016, wf.x, 1.0, gunmetal, -1.0, 1.6);
            mb.tube(hand + wf.apply(v3(0.0, 0.11, 0.0)), hand + wf.apply(v3(0.0, 0.11, 0.25)), 0.025, 0.025, wf.x, 1.0, lin([25, 25, 28]), -1.0, 2.0);
        }
        Weapon::HandCannon => {
            // a heavy, square-slide hand cannon
            mb.obox(hand + wf.apply(v3(0.0, -0.04, 0.0)), wf, v3(0.024, 0.065, 0.045), lin([40, 40, 44]), -1.0, 0.6);
            mb.obox(hand + wf.apply(v3(0.0, 0.04, 0.12)), wf, v3(0.03, 0.04, 0.17), lin([150, 152, 160]), -1.0, 2.0);
            mb.obox(hand + wf.apply(v3(0.0, 0.0, 0.2)), wf, v3(0.026, 0.02, 0.09), lin([70, 72, 80]), -1.0, 1.2);
            mb.obox(hand + wf.apply(v3(0.0, 0.085, 0.27)), wf, v3(0.006, 0.008, 0.01), lin([200, 200, 205]), -1.0, 1.5);
        }
        Weapon::Sniper => {
            // a long heavy bolt-action: stock, receiver, long barrel with a muzzle brake, scope, bolt
            mb.obox(hand + wf.apply(v3(0.0, -0.01, -0.22)), wf, v3(0.035, 0.07, 0.22), lin([52, 58, 50]), -1.0, 0.6);
            mb.obox(hand + wf.apply(v3(0.0, 0.03, 0.12)), wf, v3(0.032, 0.045, 0.2), lin([45, 48, 44]), -1.0, 0.8);
            mb.tube(hand + wf.apply(v3(0.0, 0.05, 0.3)), hand + wf.apply(v3(0.0, 0.05, 1.15)), 0.02, 0.016, wf.x, 1.0, gunmetal, -1.0, 1.6);
            mb.obox(hand + wf.apply(v3(0.0, 0.05, 1.17)), wf, v3(0.026, 0.022, 0.04), lin([30, 30, 34]), -1.0, 1.0);
            mb.tube(hand + wf.apply(v3(0.0, 0.12, -0.02)), hand + wf.apply(v3(0.0, 0.12, 0.32)), 0.032, 0.032, wf.x, 1.0, lin([20, 20, 22]), -1.0, 2.2);
            mb.ellipsoid(hand + wf.apply(v3(0.0, 0.12, 0.33)), wf, v3(0.03, 0.03, 0.006), lin([120, 170, 220]), -1.0, 3.0, 0.0);
            mb.tube(hand + wf.apply(v3(0.035, 0.04, 0.02)), hand + wf.apply(v3(0.09, 0.0, 0.02)), 0.008, 0.008, wf.y, 1.0, gunmetal, -1.0, 1.6);
        }
        Weapon::Shuriken => {
            // a star held between the fingers, ready to throw
            let c = hand + wf.apply(v3(0.0, 0.05, 0.08));
            mb.obox(c, wf, v3(0.1, 0.006, 0.024), steel, -1.0, 2.5);
            mb.obox(c, wf, v3(0.024, 0.006, 0.1), steel, -1.0, 2.5);
            mb.ellipsoid(c, wf, v3(0.03, 0.01, 0.03), gunmetal, -1.0, 1.0, 0.0);
        }
        Weapon::Rocket => {
            mb.tube(hand + wf.apply(v3(0.0, 0.1, -0.45)), hand + wf.apply(v3(0.0, 0.1, 0.5)), 0.075, 0.075, wf.x, 1.0, lin([62, 74, 58]), -1.0, 0.6);
            mb.tube(hand + wf.apply(v3(0.0, 0.1, 0.5)), hand + wf.apply(v3(0.0, 0.1, 0.57)), 0.09, 0.09, wf.x, 1.0, lin([40, 40, 44]), -1.0, 1.0);
            mb.obox(hand, wf, v3(0.02, 0.06, 0.03), lin([30, 30, 34]), -1.0, 0.5);
        }
        _ => {}
    }
}

// ------------------------------------------------------------------ GPU state

struct Target {
    size: [u32; 2],
    multisampled: bool,
    /// The scene in high dynamic range (alpha: how far away each pixel is).
    msaa_view: wgpu::TextureView,
    resolve_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    /// Ambient occlusion (half size), bloom (two quarter-size buffers), light shafts (quarter).
    ao_view: wgpu::TextureView,
    bloom_a: wgpu::TextureView,
    bloom_b: wgpu::TextureView,
    rays_view: wgpu::TextureView,
    /// The finished picture the UI shows.
    out_view: wgpu::TextureView,
    bg_ao: wgpu::BindGroup,
    bg_bright: wgpu::BindGroup,
    bg_blur_h: wgpu::BindGroup,
    bg_blur_v: wgpu::BindGroup,
    bg_rays: wgpu::BindGroup,
    bg_comp: wgpu::BindGroup,
}

/// The post-processing passes.
struct Post {
    layout: wgpu::BindGroupLayout,
    ubuf: wgpu::Buffer,
    sampler: wgpu::Sampler,
    ao: wgpu::RenderPipeline,
    bright: wgpu::RenderPipeline,
    blur_h: wgpu::RenderPipeline,
    blur_v: wgpu::RenderPipeline,
    rays: wgpu::RenderPipeline,
    comp: wgpu::RenderPipeline,
}

fn post_pipeline(device: &wgpu::Device, layout: &wgpu::PipelineLayout, module: &wgpu::ShaderModule, entry: &str, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("duel post"),
        layout: Some(layout),
        vertex: wgpu::VertexState { module, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState { module, entry_point: Some(entry), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
        multiview_mask: None,
        cache: None,
    })
}

impl Post {
    fn new(device: &wgpu::Device) -> Post {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("duel post shader"), source: wgpu::ShaderSource::Wgsl(POST_SHADER.into()) });
        let tex_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("duel post bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                tex_entry(2),
                tex_entry(3),
                tex_entry(4),
                tex_entry(5),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("duel post layout"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("duel post uniforms"), size: std::mem::size_of::<PostU>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("duel post sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Post {
            ao: post_pipeline(device, &pl, &module, "fs_ao", AO_FMT),
            bright: post_pipeline(device, &pl, &module, "fs_bright", COLOR_FMT),
            blur_h: post_pipeline(device, &pl, &module, "fs_blur_h", COLOR_FMT),
            blur_v: post_pipeline(device, &pl, &module, "fs_blur_v", COLOR_FMT),
            rays: post_pipeline(device, &pl, &module, "fs_rays", COLOR_FMT),
            comp: post_pipeline(device, &pl, &module, "fs_comp", OUT_FMT),
            layout,
            ubuf,
            sampler,
        }
    }

    fn bind(&self, device: &wgpu::Device, views: [&wgpu::TextureView; 4]) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("duel post bind"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.ubuf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(views[0]) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(views[1]) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(views[2]) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(views[3]) },
            ],
        })
    }
}

/// Draw one full-screen pass into `view`.
fn post_pass(enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, pipe: &wgpu::RenderPipeline, bind: &wgpu::BindGroup) {
    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("duel post pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipe);
    pass.set_bind_group(0, bind, &[]);
    pass.draw(0..3, 0..1);
}

/// The sun's view of the level: an orthographic box over the whole map.
fn light_matrix(world: &World, sun: V3) -> glam::Mat4 {
    let r = world.half * 1.45 + 4.0;
    let (near, far) = (1.0f32, 170.0f32);
    let centre = v3(0.0, 3.0, 0.0);
    let l = sun.norm();
    let eye = centre + l * 80.0;
    let f = -l;
    let up = if l.y.abs() > 0.98 { v3(0.0, 0.0, 1.0) } else { V3::UP };
    let rt = f.cross(up).norm();
    let u2 = rt.cross(f);
    // rows: across, up, depth (0 at `near`, 1 at `far`), w
    let rows = [
        [rt.x / r, rt.y / r, rt.z / r, -rt.dot(eye) / r],
        [u2.x / r, u2.y / r, u2.z / r, -u2.dot(eye) / r],
        [f.x / (far - near), f.y / (far - near), f.z / (far - near), (-f.dot(eye) - near) / (far - near)],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut cols = [[0.0f32; 4]; 4];
    for c in 0..4 {
        for k in 0..4 {
            cols[c][k] = rows[k][c];
        }
    }
    glam::Mat4::from_cols_array_2d(&cols)
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
    /// Which map the level mesh above was built from.
    static_map: MapId,
    dyn_buf: wgpu::Buffer,
    dyn_cap: u64,
    target: Option<Target>,
    pub tex_id: Option<egui::TextureId>,
    pipe_skin: wgpu::RenderPipeline,
    /// The fighters' outlines (an inside-out shell).
    pipe_outline: wgpu::RenderPipeline,
    inst_buf: wgpu::Buffer,
    skin: Option<Skin>,
    chars: Vec<CharState>,
    /// For each body part, which skin joints belong to it (bits, three words).
    part_masks: [[u32; 3]; 7],
    layout: wgpu::PipelineLayout,
    module: wgpu::ShaderModule,
    samples: u32,
    pub quality: Quality,
    /// Dynamic resolution: the render size is multiplied by this (see DynRes).
    pub dyn_scale: f32,
    /// Graphics effects in use (from Graphics settings).
    pub fx: Fx,
    /// Depth of field for the menu: (focus distance in metres, largest blur in pixels at 720p);
    /// a blur of 0 is off.
    pub dof: [f32; 2],
    bgl: wgpu::BindGroupLayout,
    tex_view: wgpu::TextureView,
    ntex_view: wgpu::TextureView,
    ctex_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    sky_view: wgpu::TextureView,
    sky_sampler: wgpu::Sampler,
    /// Which map's sky is loaded.
    sky_map: Option<MapId>,
    sky_lods: f32,
    shadow_view: wgpu::TextureView,
    shadow_size: u32,
    shadow_sampler: wgpu::Sampler,
    shadow_bind: wgpu::BindGroup,
    pipe_shadow: wgpu::RenderPipeline,
    pipe_skin_shadow: wgpu::RenderPipeline,
    post: Post,
}

struct Skin {
    rig: Rig,
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    icount: u32,
    /// Each fighter's mesh (vertices, indices, index count); empty if they didn't load.
    bodies: Vec<(wgpu::Buffer, wgpu::Buffer, u32)>,
}

/// The fighters' textures as layers with their mip levels (RGBA, CHAR_TEX square).
const CHAR_TEX: u32 = 1024;
fn build_char_textures() -> Vec<Vec<Vec<u8>>> {
    use image::imageops::{FilterType, resize};
    crate::chars::TEXTURES
        .iter()
        .map(|(name, bytes)| {
            let img = image::load_from_memory(bytes).map(|i| i.to_rgba8()).unwrap_or_else(|_| image::RgbaImage::from_pixel(CHAR_TEX, CHAR_TEX, image::Rgba([190, 190, 190, 255])));
            let mut img = if img.width() != CHAR_TEX || img.height() != CHAR_TEX { resize(&img, CHAR_TEX, CHAR_TEX, FilterType::Triangle) } else { img };
            // faces that read at a distance
            if *name == "hero_male" || *name == "hero_female" {
                crate::chars::paint_face(&mut img, *name == "hero_female");
            }
            let mut levels = Vec::new();
            loop {
                levels.push(img.as_raw().clone());
                let s = img.width();
                if s <= 1 {
                    break;
                }
                img = resize(&img, s / 2, s / 2, FilterType::Triangle);
            }
            levels
        })
        .collect()
}

/// Which skin joints make up each body part (head, chest, hips, left/right arm, left/right leg).
fn part_masks(rig: &Rig) -> [[u32; 3]; 7] {
    let mut out = [[0u32; 3]; 7];
    let n = |name: &str| rig.node(name);
    let (Some(neck), Some(sp2), Some(pelvis), Some(sp1), Some(ua_l), Some(ua_r), Some(th_l), Some(th_r)) =
        (n("neck_01"), n("spine_02"), n("pelvis"), n("spine_01"), n("upperarm_l"), n("upperarm_r"), n("thigh_l"), n("thigh_r"))
    else {
        return out;
    };
    for (ji, &node) in rig.joints.iter().enumerate().take(96) {
        let part = if rig.is_under(node, neck) {
            0
        } else if rig.is_under(node, ua_l) {
            3
        } else if rig.is_under(node, ua_r) {
            4
        } else if rig.is_under(node, th_l) {
            5
        } else if rig.is_under(node, th_r) {
            6
        } else if rig.is_under(node, sp2) {
            1
        } else if node == pelvis || node == sp1 {
            2
        } else {
            continue;
        };
        out[part][ji / 32] |= 1 << (ji % 32);
    }
    out
}

/// The skinned fighters (`outline`: their inside-out outline shell instead).
fn make_skin_pipeline(device: &wgpu::Device, layout: &wgpu::PipelineLayout, module: &wgpu::ShaderModule, samples: u32, outline: bool) -> wgpu::RenderPipeline {
    let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Uint32x4, 3 => Float32x4, 4 => Uint32, 5 => Float32x2, 6 => Float32x3];
    let vbl = wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<SkinVtx>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(if outline { "duel outline" } else { "duel skinned" }),
        layout: Some(layout),
        vertex: wgpu::VertexState { module, entry_point: Some(if outline { "vs_skin_outline" } else { "vs_skin" }), compilation_options: Default::default(), buffers: &[Some(vbl)] },
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: if outline { Some(wgpu::Face::Front) } else { None }, ..Default::default() },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FMT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState { count: samples, mask: !0, alpha_to_coverage_enabled: false },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(if outline { "fs_outline" } else { "fs" }),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format: COLOR_FMT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// How much the renderer does each frame (from the graphics preset).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Quality {
    /// Edge smoothing samples: 1 (off) or 4.
    pub msaa: u32,
    /// Render size compared with the window (below 1 is blurrier but faster).
    pub scale: f32,
    /// Share of particles drawn (0..1).
    pub fx: f32,
    /// Shell casings and other small extras.
    pub extras: bool,
    /// The effects this preset starts with (each can be switched in Graphics settings).
    pub post: Fx,
}

impl Quality {
    pub const PRESETS: [(&'static str, Quality); 5] = [
        ("Low", Quality { msaa: 1, scale: 0.6, fx: 0.35, extras: false, post: Fx { ssao: false, rays: false, soft_shadows: false, ..Fx::CHEAP } }),
        ("Medium", Quality { msaa: 1, scale: 0.85, fx: 0.7, extras: true, post: Fx { ssao: false, ..Fx::ALL } }),
        ("High", Quality { msaa: 4, scale: 1.0, fx: 1.0, extras: true, post: Fx::ALL }),
        ("Ultra", Quality { msaa: 4, scale: 1.4, fx: 1.0, extras: true, post: Fx { shadow_res: 4096, ..Fx::ALL } }),
        // tuned for the Steam Deck's 1280x800 screen and GPU: no edge smoothing (its pixels are
        // small), a slightly lower resolution, fewer particles and only the cheap effects (hard
        // 1024 shadows, bloom, grading, vignette, sharpen), so 60 fps holds in a full lobby
        ("Steam Deck", Quality { msaa: 1, scale: 0.9, fx: 0.6, extras: false, post: Fx::CHEAP }),
    ];

    /// The preset the Steam Deck starts with.
    pub const DECK: usize = 4;
}

/// Dynamic resolution: when frames come late for a moment, draw the 3D picture a little smaller
/// (the HUD and text stay sharp); when they've been on time for a while, step back up. Each
/// step up that has to be taken back makes the next try wait longer, so it settles instead of
/// bouncing.
#[derive(Clone, Debug)]
pub struct DynRes {
    pub scale: f32,
    late: f32,
    on_time: f32,
    wait_up: f32,
    /// A step up was just taken (if frames come late again soon, it was too much).
    trial: f32,
}

impl Default for DynRes {
    fn default() -> Self {
        DynRes { scale: 1.0, late: 0.0, on_time: 0.0, wait_up: DynRes::WAIT_UP, trial: 0.0 }
    }
}

impl DynRes {
    pub const MIN: f32 = 0.6;
    pub const STEP: f32 = 0.05;
    /// Late frames for this long (seconds) before stepping down.
    pub const LATE_FOR: f32 = 0.25;
    pub const WAIT_UP: f32 = 3.0;

    /// One frame took `frame_dt` seconds; the goal is `target_fps`. Returns the scale to draw at.
    pub fn update(&mut self, frame_dt: f32, target_fps: f32) -> f32 {
        let dt = if frame_dt.is_finite() { frame_dt.clamp(0.0, 0.25) } else { 0.0 };
        let budget = 1.0 / target_fps.max(1.0);
        self.trial = (self.trial - dt).max(0.0);
        if dt > budget * 1.12 {
            self.late += dt;
            self.on_time = 0.0;
            if self.late >= DynRes::LATE_FOR && self.scale > DynRes::MIN {
                self.scale = (self.scale - DynRes::STEP).max(DynRes::MIN);
                self.late = 0.0;
                if self.trial > 0.0 {
                    // the last step up was too much: wait longer before the next try
                    self.wait_up = (self.wait_up * 2.0).min(60.0);
                }
            }
        } else {
            self.late = (self.late - dt * 0.5).max(0.0);
            self.on_time += dt;
            if self.on_time >= self.wait_up && self.scale < 1.0 {
                self.scale = (self.scale + DynRes::STEP).min(1.0);
                self.on_time = 0.0;
                self.trial = 2.0;
            }
        }
        self.scale
    }
}

/// Every pipeline for one sample count: opaque, alpha, additive, sky, skinned, outlines.
fn make_pipelines(device: &wgpu::Device, layout: &wgpu::PipelineLayout, module: &wgpu::ShaderModule, samples: u32) -> [wgpu::RenderPipeline; 6] {
    // see-through things blend their colour but leave the stored distance alone
    let keep = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
    let alpha = wgpu::BlendState { color: wgpu::BlendState::ALPHA_BLENDING.color, alpha: keep };
    let add = wgpu::BlendState {
        color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
        alpha: keep,
    };
    [
        make_pipeline(device, layout, module, None, true, false, samples),
        make_pipeline(device, layout, module, Some(alpha), false, false, samples),
        make_pipeline(device, layout, module, Some(add), false, false, samples),
        make_pipeline(device, layout, module, None, false, true, samples),
        make_skin_pipeline(device, layout, module, samples, false),
        make_skin_pipeline(device, layout, module, samples, true),
    ]
}

fn make_pipeline(device: &wgpu::Device, layout: &wgpu::PipelineLayout, module: &wgpu::ShaderModule, blend: Option<wgpu::BlendState>, depth_write: bool, sky: bool, samples: u32) -> wgpu::RenderPipeline {
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
        multisample: wgpu::MultisampleState { count: samples, mask: !0, alpha_to_coverage_enabled: false },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(if sky { "fs_sky" } else if blend.is_some() { "fs_blend" } else { "fs" }),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format: COLOR_FMT, blend, write_mask: wgpu::ColorWrites::ALL })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// Depth-only pipelines drawing the level and the fighters from the sun.
fn make_shadow_pipelines(device: &wgpu::Device, layout: &wgpu::PipelineLayout, module: &wgpu::ShaderModule) -> (wgpu::RenderPipeline, wgpu::RenderPipeline) {
    let depth = wgpu::DepthStencilState {
        format: DEPTH_FMT,
        depth_write_enabled: Some(true),
        depth_compare: Some(wgpu::CompareFunction::Less),
        stencil: Default::default(),
        bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.5, clamp: 0.0 },
    };
    let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4, 4 => Float32x4];
    let vbl = wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vtx>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs };
    let sattrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Uint32x4, 3 => Float32x4, 4 => Uint32, 5 => Float32x2, 6 => Float32x3];
    let sbl = wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<SkinVtx>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &sattrs };
    let mk = |entry: &str, vb: wgpu::VertexBufferLayout| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("duel shadow"),
            layout: Some(layout),
            vertex: wgpu::VertexState { module, entry_point: Some(entry), compilation_options: Default::default(), buffers: &[Some(vb)] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: Some(depth.clone()),
            multisample: wgpu::MultisampleState::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        })
    };
    (mk("vs_shadow", vbl), mk("vs_skin_shadow", sbl))
}

fn make_shadow_map(device: &wgpu::Device, size: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("duel shadow map"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FMT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// A map's sky picture with its mip levels (half floats).
fn make_sky(rs: &RenderState, map: MapId) -> (wgpu::TextureView, f32) {
    let s = env::sky(map);
    let tex = rs.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("duel sky"),
        size: wgpu::Extent3d { width: s.w, height: s.h, depth_or_array_layers: 1 },
        mip_level_count: s.mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (mip, data) in s.mips.iter().enumerate() {
        let (w, h) = ((s.w >> mip).max(1), (s.h >> mip).max(1));
        rs.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: mip as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            bytemuck::cast_slice(data),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(8 * w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
    }
    (tex.create_view(&Default::default()), (s.mips.len() as f32 - 1.0).max(0.0))
}

/// An RGBA8 texture array with mips, filled with `layers` ([layer][mip] bytes).
fn upload_array(rs: &RenderState, label: &str, size: u32, format: wgpu::TextureFormat, layers: &[Vec<Vec<u8>>]) -> wgpu::TextureView {
    let mips = layers.first().map_or(1, |l| l.len() as u32);
    let tex = rs.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: layers.len().max(1) as u32 },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (layer, levels) in layers.iter().enumerate() {
        for (mip, bytes) in levels.iter().enumerate() {
            let s = (size >> mip).max(1);
            rs.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: mip as u32, origin: wgpu::Origin3d { x: 0, y: 0, z: layer as u32 }, aspect: wgpu::TextureAspect::All },
                bytes,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * s), rows_per_image: Some(s) },
                wgpu::Extent3d { width: s, height: s, depth_or_array_layers: 1 },
            );
        }
    }
    tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() })
}

impl Gpu {
    /// The main bind group: uniforms, materials, fighters' textures, the sky and the shadow map.
    fn make_bind(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("duel bind"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.ubuf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.tex_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: self.inst_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.ctex_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.ntex_view) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&self.sky_view) },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::Sampler(&self.sky_sampler) },
                wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(&self.shadow_view) },
                wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::Sampler(&self.shadow_sampler) },
            ],
        })
    }

    pub fn new(rs: &RenderState, world: &World) -> Gpu {
        let device = &rs.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("duel shader"), source: wgpu::ShaderSource::Wgsl(format!("{U_DECL}{SHADER}").into()) });

        // the level's materials (colour, and normal + roughness + metalness)
        let (cols, surfs) = build_world_layers();
        let tex_view = upload_array(rs, "duel materials", env::TEX, wgpu::TextureFormat::Rgba8UnormSrgb, &cols);
        let ntex_view = upload_array(rs, "duel surfaces", env::TEX, wgpu::TextureFormat::Rgba8Unorm, &surfs);
        // the fighters' textures
        // the fighters' textures, then the weapons' (four to a layer)
        let mut cdata = build_char_textures();
        debug_assert_eq!(cdata.len(), crate::weapons::first_layer());
        if CHAR_TEX == crate::weapons::LAYER {
            cdata.extend(crate::weapons::texture_layers());
        }
        let ctex_view = upload_array(rs, "fighter textures", CHAR_TEX, wgpu::TextureFormat::Rgba8UnormSrgb, &cdata);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("duel sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            // 16x anisotropic filtering: floors stay crisp at grazing angles, no shimmer
            anisotropy_clamp: 16,
            ..Default::default()
        });
        let sky_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("duel sky sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("duel shadow sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let (sky_view, sky_lods) = make_sky(rs, world.map);
        let shadow_size = Fx::ALL.shadow_res;
        let shadow_view = make_shadow_map(device, shadow_size);
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("duel uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let both = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
        let tex_entry = |binding: u32, dim: wgpu::TextureViewDimension| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: dim, multisampled: false },
            count: None,
        };
        let ubuf_entry = wgpu::BindGroupLayoutEntry { binding: 0, visibility: both, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None };
        let inst_entry = wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::VERTEX, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("duel bgl"),
            entries: &[
                ubuf_entry,
                tex_entry(1, wgpu::TextureViewDimension::D2Array),
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                inst_entry,
                tex_entry(4, wgpu::TextureViewDimension::D2Array),
                tex_entry(5, wgpu::TextureViewDimension::D2Array),
                tex_entry(6, wgpu::TextureViewDimension::D2),
                wgpu::BindGroupLayoutEntry { binding: 7, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                wgpu::BindGroupLayoutEntry { binding: 8, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 9, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison), count: None },
            ],
        });
        let inst_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("duel characters"),
            size: (MAX_INST * INST_FLOATS * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("duel layout"), bind_group_layouts: &[Some(&bgl)], immediate_size: 0 });
        let [pipe_opaque, pipe_alpha, pipe_add, pipe_sky, pipe_skin, pipe_outline] = make_pipelines(device, &layout, &module, MSAA);
        // the sun's view: its own small layout (uniforms and the fighters' joints)
        let sbgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("duel shadow bgl"), entries: &[ubuf_entry, inst_entry] });
        let sh_module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("duel shadow shader"), source: wgpu::ShaderSource::Wgsl(format!("{U_DECL}{SHADOW_SHADER}").into()) });
        let sh_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("duel shadow layout"), bind_group_layouts: &[Some(&sbgl)], immediate_size: 0 });
        let (pipe_shadow, pipe_skin_shadow) = make_shadow_pipelines(device, &sh_layout, &sh_module);
        let shadow_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("duel shadow bind"),
            layout: &sbgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() }, wgpu::BindGroupEntry { binding: 3, resource: inst_buf.as_entire_binding() }],
        });
        // the animated character (CC0 Quaternius mannequin)
        let skin = match Rig::load() {
            Ok(rig) => {
                use wgpu::util::DeviceExt;
                let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("character verts"), contents: bytemuck::cast_slice(&rig.verts), usage: wgpu::BufferUsages::VERTEX });
                let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("character indices"), contents: bytemuck::cast_slice(&rig.indices), usage: wgpu::BufferUsages::INDEX });
                let icount = rig.indices.len() as u32;
                // the four fighters (CC0 Quaternius outfits, heads and hair on the same skeleton)
                let bodies = crate::chars::shared()
                    .map(|ms| {
                        ms.iter()
                            .map(|m| {
                                let v = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("fighter verts"), contents: bytemuck::cast_slice(&m.verts), usage: wgpu::BufferUsages::VERTEX });
                                let i = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("fighter indices"), contents: bytemuck::cast_slice(&m.indices), usage: wgpu::BufferUsages::INDEX });
                                (v, i, m.indices.len() as u32)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Some(Skin { rig, vbuf, ibuf, icount, bodies })
            }
            Err(e) => {
                eprintln!("character model not loaded: {e}");
                None
            }
        };
        let part_masks = skin.as_ref().map_or([[0u32; 3]; 7], |s| part_masks(&s.rig));
        let verts = build_world(world);
        let static_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("duel world"),
            size: (verts.len() * std::mem::size_of::<Vtx>()).max(64) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        rs.queue.write_buffer(&static_buf, 0, bytemuck::cast_slice(&verts));
        let dyn_cap = 4 << 20;
        let dyn_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("duel dynamic"), size: dyn_cap, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let post = Post::new(device);
        // a placeholder until the real bind group is made just below
        let placeholder = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("duel placeholder"),
            layout: &sbgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() }, wgpu::BindGroupEntry { binding: 3, resource: inst_buf.as_entire_binding() }],
        });
        let mut g = Gpu {
            pipe_opaque,
            pipe_alpha,
            pipe_add,
            pipe_sky,
            bind: placeholder,
            ubuf,
            static_buf,
            static_count: verts.len() as u32,
            static_map: world.map,
            dyn_buf,
            dyn_cap,
            target: None,
            tex_id: None,
            pipe_skin,
            pipe_outline,
            inst_buf,
            skin,
            chars: (0..MAX_INST).map(|_| CharState::new()).collect(),
            part_masks,
            layout,
            module,
            samples: MSAA,
            quality: Quality::PRESETS[2].1,
            dyn_scale: 1.0,
            fx: Fx::ALL,
            dof: [0.0, 0.0],
            bgl,
            tex_view,
            ntex_view,
            ctex_view,
            sampler,
            sky_view,
            sky_sampler,
            sky_map: Some(world.map),
            sky_lods,
            shadow_view,
            shadow_size,
            shadow_sampler,
            shadow_bind,
            pipe_shadow,
            pipe_skin_shadow,
            post,
        };
        g.bind = g.make_bind(device);
        g
    }
    /// Switch the graphics preset; edge smoothing changes rebuild the pipelines.
    pub fn set_quality(&mut self, rs: &RenderState, q: Quality) {
        self.quality = q;
        let samples = if q.msaa > 1 { MSAA } else { 1 };
        if samples != self.samples {
            let [a, b, c, d, e, o] = make_pipelines(&rs.device, &self.layout, &self.module, samples);
            self.pipe_outline = o;
            self.pipe_opaque = a;
            self.pipe_alpha = b;
            self.pipe_add = c;
            self.pipe_sky = d;
            self.pipe_skin = e;
            self.samples = samples;
            self.target = None; // remade with the new sample count
        }
    }

    fn ensure_target(&mut self, rs: &RenderState, size: [u32; 2]) {
        if self.target.as_ref().map_or(false, |t| t.size == size) {
            return;
        }
        let samples = self.samples;
        let device = &rs.device;
        let mk = |sz: [u32; 2], samples: u32, format: wgpu::TextureFormat, usage: wgpu::TextureUsages| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("duel target"),
                size: wgpu::Extent3d { width: sz[0].max(1), height: sz[1].max(1), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let rt = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let resolve = mk(size, 1, COLOR_FMT, rt);
        // without edge smoothing the scene is drawn straight into the texture the effects read
        let msaa = if samples > 1 { mk(size, samples, COLOR_FMT, wgpu::TextureUsages::RENDER_ATTACHMENT) } else { resolve.clone() };
        let depth = mk(size, samples, DEPTH_FMT, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let half = [(size[0] / 2).max(1), (size[1] / 2).max(1)];
        let quarter = [(size[0] / 4).max(1), (size[1] / 4).max(1)];
        let view = |t: wgpu::Texture| t.create_view(&Default::default());
        let resolve_view = view(resolve);
        let ao_view = view(mk(half, 1, AO_FMT, rt));
        let bloom_a = view(mk(quarter, 1, COLOR_FMT, rt));
        let bloom_b = view(mk(quarter, 1, COLOR_FMT, rt));
        let rays_view = view(mk(quarter, 1, COLOR_FMT, rt));
        let out_view = view(mk(size, 1, OUT_FMT, rt));
        let p = &self.post;
        let t = Target {
            size,
            multisampled: samples > 1,
            msaa_view: view(msaa),
            depth_view: view(depth),
            bg_ao: p.bind(device, [&resolve_view, &resolve_view, &resolve_view, &resolve_view]),
            bg_bright: p.bind(device, [&resolve_view, &resolve_view, &resolve_view, &resolve_view]),
            bg_blur_h: p.bind(device, [&bloom_a, &bloom_a, &bloom_a, &bloom_a]),
            bg_blur_v: p.bind(device, [&bloom_b, &bloom_b, &bloom_b, &bloom_b]),
            bg_rays: p.bind(device, [&resolve_view, &resolve_view, &resolve_view, &resolve_view]),
            bg_comp: p.bind(device, [&resolve_view, &ao_view, &bloom_a, &rays_view]),
            resolve_view,
            ao_view,
            bloom_a,
            bloom_b,
            rays_view,
            out_view,
        };
        let mut r = rs.renderer.write();
        match self.tex_id {
            Some(id) => r.update_egui_texture_from_wgpu_texture(device, &t.out_view, wgpu::FilterMode::Linear, id),
            None => self.tex_id = Some(r.register_native_texture(device, &t.out_view, wgpu::FilterMode::Linear)),
        }
        self.target = Some(t);
    }
    /// Render the scene and return the texture to show.
    pub fn render(&mut self, rs: &RenderState, g: &Game, cam: &Cam, rect: egui::Rect, ppp: f32, alpha: f32, local_override: Option<(usize, V3)>) -> Option<egui::TextureId> {
        if g.world.map != self.static_map {
            // another map: build its level mesh
            let verts = build_world(&g.world);
            self.static_buf = rs.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("duel world"),
                size: (verts.len() * std::mem::size_of::<Vtx>()).max(64) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            rs.queue.write_buffer(&self.static_buf, 0, bytemuck::cast_slice(&verts));
            self.static_count = verts.len() as u32;
            self.static_map = g.world.map;
        }
        // the map's sky (and its light), and the shadow map at the size the settings ask for
        let want_shadow = self.fx.shadow_res.clamp(512, 4096);
        if self.sky_map != Some(g.world.map) || want_shadow != self.shadow_size {
            if self.sky_map != Some(g.world.map) {
                let (v, lods) = make_sky(rs, g.world.map);
                self.sky_view = v;
                self.sky_lods = lods;
                self.sky_map = Some(g.world.map);
            }
            if want_shadow != self.shadow_size {
                self.shadow_view = make_shadow_map(&rs.device, want_shadow);
                self.shadow_size = want_shadow;
            }
            self.bind = self.make_bind(&rs.device);
        }
        let sc = (self.quality.scale * self.dyn_scale.clamp(DynRes::MIN, 1.0)).clamp(0.3, 2.0);
        let w = ((rect.width() * ppp * sc).round() as u32).clamp(16, 8192);
        let h = ((rect.height() * ppp * sc).round() as u32).clamp(16, 8192);
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
        let sky = env::sky(g.world.map);
        let sun = sky.sun_dir;
        let light_vp = light_matrix(&g.world, sun);
        let mut sh = [[0.0f32; 4]; 9];
        for (k, c) in sky.sh.iter().enumerate() {
            sh[k] = [c[0], c[1], c[2], 0.0];
        }
        // point lights: braziers and torches flicker, muzzle flashes pop; the nearest few
        let mut lights: Vec<(V3, f32, [f32; 3])> = Vec::new();
        let flicker = |t: f32, k: usize| 0.8 + 0.12 * (t * 11.0 + k as f32 * 1.7).sin() + 0.08 * (t * 23.3 + k as f32 * 4.1).sin();
        for (k, b) in g.world.braziers.iter().enumerate() {
            let f = flicker(g.time, k) * 7.0;
            lights.push((*b + v3(0.0, 0.35, 0.0), 10.0, [1.0 * f, 0.52 * f, 0.2 * f]));
        }
        for (k, b) in g.world.torches.iter().enumerate() {
            let f = flicker(g.time, k + 20) * 3.6;
            lights.push((*b + v3(0.0, 0.15, 0.0), 7.0, [1.0 * f, 0.5 * f, 0.18 * f]));
        }
        for fl in &g.flashes {
            let k = (fl.life / 0.06).clamp(0.0, 1.0) * 9.0;
            lights.push((fl.pos, 5.0, [1.0 * k, 0.72 * k, 0.4 * k]));
        }
        lights.sort_by(|a, b| (a.0 - cam.pos).len().total_cmp(&(b.0 - cam.pos).len()));
        lights.truncate(MAX_LIGHTS);
        let mut lpos = [[0.0f32; 4]; MAX_LIGHTS];
        let mut lcol = [[0.0f32; 4]; MAX_LIGHTS];
        for (k, (p, r, c)) in lights.iter().enumerate() {
            lpos[k] = [p.x, p.y, p.z, *r];
            lcol[k] = [c[0], c[1], c[2], 0.0];
        }
        let u = Uniforms {
            vp,
            // (w: the picture's height in pixels, for the outlines' width)
            cam: [c.x, c.y, c.z, h as f32],
            right: [cam.right.x, cam.right.y, cam.right.z, 1.0 / sx],
            up: [cam.up.x, cam.up.y, cam.up.z, 1.0 / sy],
            fwd: [cam.fwd.x, cam.fwd.y, cam.fwd.z, 0.0],
            sun: [sun.x, sun.y, sun.z, sky.sun_power],
            sun_col: [sky.sun_col[0], sky.sun_col[1], sky.sun_col[2], 1.0],
            fog: [sky.fog[0], sky.fog[1], sky.fog[2], sky.fog_density],
            params: [g.time, sky.yaw, 1.0, self.sky_lods.min(7.0)],
            light_vp: light_vp.to_cols_array_2d(),
            shadow: [1.0 / self.shadow_size as f32, if self.fx.soft_shadows { 1.0 } else { 0.0 }, lights.len() as f32, 1.0],
            sh,
            lpos,
            lcol,
        };
        // the effects after the scene
        let sun_clip = {
            let q = cam.pos + sun * 400.0;
            let d = q - cam.pos;
            let z = d.dot(cam.fwd);
            if z > 0.0 { Some((0.5 + 0.5 * sx * d.dot(cam.right) / z, 0.5 - 0.5 * sy * d.dot(cam.up) / z)) } else { None }
        };
        let (sun_u, sun_v, sun_vis) = match sun_clip {
            Some((x, y)) => {
                let off = ((x - 0.5).abs().max((y - 0.5).abs()) - 0.5).max(0.0);
                (x, y, (1.0 - off * 2.0).clamp(0.0, 1.0) * sun.dot(cam.fwd).clamp(0.0, 1.0))
            }
            None => (0.5, 0.5, 0.0),
        };
        let gr = sky.grade;
        let fx = self.fx;
        let pu = PostU {
            size: [1.0 / w as f32, 1.0 / h as f32, w as f32, h as f32],
            sun: [sun_u, sun_v, sun_vis, 0.0],
            flags: [fx.ssao as u8 as f32, fx.bloom as u8 as f32, fx.rays as u8 as f32, self.dof[0]],
            lift: [gr.lift[0], gr.lift[1], gr.lift[2], gr.saturation],
            gamma: [gr.gamma[0], gr.gamma[1], gr.gamma[2], gr.contrast],
            gain: [gr.gain[0], gr.gain[1], gr.gain[2], 0.0],
            misc: [if fx.vignette { 0.32 } else { 0.0 }, 0.14, 0.35, if self.dof[1] > 0.0 { 0.3 } else { 0.0 }],
            sun_col: [sky.sun_col[0], sky.sun_col[1], sky.sun_col[2], 0.0],
            right: [cam.right.x, cam.right.y, cam.right.z, 1.0 / sx],
            up: [cam.up.x, cam.up.y, cam.up.z, 1.0 / sy],
            fwd: [cam.fwd.x, cam.fwd.y, cam.fwd.z, 0.0],
            misc2: [fx.grading as u8 as f32, gr.exposure, 0.35, self.dof[1] * h as f32 / 720.0],
        };
        rs.queue.write_buffer(&self.post.ubuf, 0, bytemuck::bytes_of(&pu));        rs.queue.write_buffer(&self.ubuf, 0, bytemuck::bytes_of(&u));

        // ---- dynamic geometry
        let mut op = MB::default();
        let mut al = MB::default();
        let mut ad = MB::default();
        let mut insts: Vec<Vec<f32>> = Vec::new();
        let mut inst_body: Vec<usize> = Vec::new();
        // Afterimage decoys are drawn as their owners (after the real fighters)
        // (and an execution's afterimages, after them)
        let decoys: Vec<Fighter> = g.decoys.iter().filter_map(|d| g.decoy_fighter(d)).chain(g.exec_ghosts()).take(MAX_INST - MAX_PLAYERS).collect();
        for (i, f) in g.f.iter().enumerate().chain(decoys.iter().enumerate().map(|(k, f)| (MAX_PLAYERS + k, f))) {
            if !f.present || f.spectate {
                continue;
            }
            // vanished (Afterimage): nobody else sees them, except as a faint shimmer up close
            if f.invis_t > 0.0 && i != g.local {
                let p = f.prev_pos.lerp(f.pos, alpha.clamp(0.0, 1.0));
                let me = g.f.get(g.local).map_or(cam.pos, |m| m.pos);
                let d = (p - me).len();
                if f.alive && d < crate::game::SHIMMER_RANGE {
                    let k = (1.0 - d / crate::game::SHIMMER_RANGE).clamp(0.0, 1.0);
                    for (n, h) in [0.35f32, 0.85, 1.3, 1.7].iter().enumerate() {
                        let wob = (g.time * 9.0 + n as f32 * 1.7).sin();
                        let at = p + v3(wob * 0.08, *h, (g.time * 7.0 + n as f32).cos() * 0.08);
                        ad.billboard(cam, at, 0.38 + 0.06 * wob, lina([200, 215, 255], (0.05 + 0.05 * wob.abs()) * (0.4 + 0.6 * k)));
                    }
                }
                continue;
            }
            // an execution: only a blur for a moment (its afterimages strike), and the gun drawn for
            // a point-blank shot
            let exec_view = exec_view(g, f);
            if exec_view.as_ref().is_some_and(|v| v.1) {
                continue;
            }
            let f: &Fighter = match &exec_view {
                Some((Some(v), _)) => v,
                _ => f,
            };
            let mut pos = f.prev_pos.lerp(f.pos, alpha.clamp(0.0, 1.0));
            if let Some((li, p)) = local_override {
                if li == i {
                    pos = p;
                }
            }
            // (the camera inside your own head hides you; not in an execution's cinematic shots)
            if i == g.local && f.fin == 0 && (cam.pos - (pos + v3(0.0, 1.1, 0.0))).len() < 0.8 {
                continue;
            }
            let posed = match (&self.skin, insts.len() < MAX_INST) {
                (Some(sk), true) => {
                    // the menu's fighter, and yours on the loadout screen, strike their hero pose
                    let hero = i == g.local && (g.showcase == Some(i) || (g.mt.phase == crate::game::Phase::Loadout && !f.ready && !f.is_bot));
                    character::animate_show_as(&sk.rig, &mut self.chars[i], f, pos, g.time, &g.tune, hero)
                }
                _ => None,
            };
            match posed {
                Some(p) => {
                    let lk = look(f.color_idx);
                    let flash = if f.hurt_t > 0.0 { (f.hurt_t * 1.4).min(0.35) } else { 0.0 };
                    let tint = |c: [u8; 3]| -> [f32; 4] {
                        let l = lin(c);
                        [l[0] + (1.0 - l[0]) * flash, l[1] + (0.3 - l[1]).max(0.0) * flash, l[2] + (0.3 - l[2]).max(0.0) * flash, 1.0]
                    };
                    let mut data = Vec::with_capacity(INST_FLOATS);
                    let mut t0 = tint(lk.coat);
                    // (w: a glowing trim brightens while the ability is on: Afterimage's vanish)
                    t0[3] = if f.invis_t > 0.0 || f.ghost > 0.0 { 1.0 } else { 0.0 };
                    data.extend_from_slice(&t0);
                    let pants = tint(lk.pants);
                    // (a textured fighter reads the hurt flash from here)
                    data.extend_from_slice(&[pants[0], pants[1], pants[2], flash * 1.6]);
                    // (an execution's afterimage: the whole body white-hot, as energy)
                    let m = if f.ghost > 0.0 { [u32::MAX; 3] } else { self.part_masks[(f.hit_part as usize).min(6)] };
                    let strength = if f.ghost > 0.0 { 0.45 + 0.35 * f.ghost } else { (f.flash_t / 0.12).clamp(0.0, 1.0) * 0.85 };
                    data.extend_from_slice(&[f32::from_bits(m[0]), f32::from_bits(m[1]), f32::from_bits(m[2]), strength]);
                    // rim light in the player's colour, brightened to full strength (dimmer on
                    // yourself, so it doesn't distract)
                    let rc = lin(lk.coat);
                    let top = rc[0].max(rc[1]).max(rc[2]).max(0.05);
                    let own = if i == g.local { 0.35 } else { 1.0 };
                    // (an afterimage glows all over in its colour)
                    let rk = g.tune.rim * own * if f.ghost > 0.0 { 2.5 + 3.0 * f.ghost } else if f.alive { 1.0 } else { 0.3 };
                    data.extend_from_slice(&[rc[0] / top * rk, rc[1] / top * rk, rc[2] / top * rk, g.tune.rim_width.max(0.5)]);
                    // how it moves (for the sway of hair, scarves and aprons), and its own phase
                    data.extend_from_slice(&[f.vel.x, f.vel.y, f.vel.z, i as f32 * 1.7]);
                    for j in 0..MAX_JOINTS {
                        let m = p.palette.get(j).copied().unwrap_or(glam::Mat4::IDENTITY);
                        data.extend_from_slice(&m.to_cols_array());
                    }
                    insts.push(data);
                    inst_body.push(f.loadout.body as usize);
                    character_extras(&mut op, &mut ad, cam, f, &p, &mut self.chars[i], g.time, &lk, flash, g.tune.char_scale);
                }
                None => fighter_mesh(&mut op, &mut ad, cam, f, pos, g.time),
            }
            if f.spawn_prot > 0.0 && f.alive {
                // spawn protection shimmer
                let a = 0.18 + 0.08 * (g.time * 12.0).sin();
                ad.billboard(cam, pos + v3(0.0, 1.0, 0.0), 1.15, lina([140, 200, 255], a));
            }
            // abilities: the grapple line, the Iron Wall's glowing panel
            if let Some(a) = f.grapple.filter(|_| f.alive) {
                let p0 = pos + v3(0.0, 1.35, 0.0);
                let side = (a - p0).cross(v3(0.0, 1.0, 0.0));
                let side = if side.len() > 1e-3 { side.norm() } else { v3(1.0, 0.0, 0.0) };
                op.tube(p0, a, 0.02, 0.02, side, 1.0, lin([70, 60, 48]), -1.0, 0.6);
                ad.billboard(cam, a, 0.4, lina([255, 230, 190], 0.6));
            }
            if f.shield_t > 0.0 && f.alive {
                let fw = yaw_fwd(f.yaw);
                let rt = yaw_right(f.yaw);
                let up = v3(0.0, 1.0, 0.0);
                let c = pos + fw * 0.75 + v3(0.0, 1.05, 0.0);
                let flick = 0.8 + 0.2 * (g.time * 30.0).sin().abs();
                ad.obox(c, &M3 { x: rt, y: up, z: fw }, v3(0.62, 0.95, 0.02), lina([50, 110, 255], 0.35 * flick), -1.0, 1.0);
                for k in 0..10 {
                    let u = k as f32 / 9.0 * 2.0 - 1.0;
                    ad.billboard(cam, c + rt * (u * 0.62) + up * 0.95, 0.16, lina([160, 210, 255], 0.5));
                    ad.billboard(cam, c + rt * (u * 0.62) - up * 0.95, 0.16, lina([160, 210, 255], 0.5));
                }
            }
            // a scoped sniper's lens glints at whoever it's pointed at (Sable Rook's stays hidden
            // for the first moment scoped)
            let glint_hidden = g.abilities_on() && Ability::of(f.loadout.body) == Ability::Grapple && g.scoped_since.get(i).is_some_and(|&s| s >= 0.0 && g.time - s < g.tune.glint_hide);
            if i != g.local && f.alive && f.weapon == Weapon::Sniper && f.scope_t > 0.0 && !glint_hidden {
                let eye = pos + v3(0.0, 1.62, 0.0);
                let aim = look_dir(f.yaw, f.pitch);
                let to_cam = (cam.pos - eye).norm();
                let facing = aim.dot(to_cam);
                if facing > 0.93 {
                    let k = ((facing - 0.93) / 0.07).clamp(0.0, 1.0);
                    let pulse = 0.8 + 0.2 * (g.time * 9.0 + i as f32).sin();
                    let gp = eye + aim * 0.35 + yaw_right(f.yaw) * 0.05;
                    let dist = (cam.pos - gp).len();
                    let size = (0.25 + dist * 0.02) * (0.6 + 0.8 * k) * pulse;
                    ad.billboard(cam, gp, size, lina([255, 250, 220], 0.9 * k));
                    ad.billboard(cam, gp, size * 2.2, lina([180, 210, 255], 0.35 * k));
                }
            }
        }
        // Capture the Flag: base rings and the flags
        if g.mt.mode == Mode::Ctf {
            for t in 0..2usize {
                let col = if t == 0 { [220, 50, 50] } else { [60, 110, 230] };
                let base = g.world.flag_base(t);
                let seg = 24;
                for k in 0..seg {
                    let a0 = k as f32 / seg as f32 * 2.0 * PI;
                    let a1 = (k + 1) as f32 / seg as f32 * 2.0 * PI;
                    let (r0, r1) = (1.35, 1.7);
                    let pt = |a: f32, r: f32| base + v3(a.cos() * r, 0.03, a.sin() * r);
                    al.quad([pt(a0, r0), pt(a1, r0), pt(a1, r1), pt(a0, r1)], V3::UP, [[0.5, 0.5]; 4], lina(col, 0.55), [-1.0, 2.0, 1.0, 0.0]);
                }
                let fl = g.mt.flags[t];
                let (foot, h) = match fl.state {
                    1 if (fl.carrier as usize) < g.f.len() => {
                        let c = &g.f[fl.carrier as usize];
                        (c.prev_pos.lerp(c.pos, alpha) + v3(0.0, 1.1, 0.0) - c.fwd() * 0.3, 1.3)
                    }
                    _ => (fl.pos, 2.3),
                };
                let top = foot + v3(0.0, h, 0.0);
                op.tube(foot, top, 0.025, 0.025, v3(1.0, 0.0, 0.0), 1.0, lin([200, 200, 205]), -1.0, 1.5);
                let wave = (g.time * 6.0 + t as f32).sin() * 0.12;
                let side = v3(0.8, 0.0, wave).norm() * 0.75;
                let c = lin([col[0], col[1], col[2]]);
                let q = [top, top + side, top + side - v3(0.0, 0.5, 0.0) + v3(0.0, wave * 0.5, 0.0), top - v3(0.0, 0.5, 0.0)];
                op.quad(q, side.cross(V3::UP).norm(), [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], c, [6.0, 0.0, 1.0, 0.1]);
                ad.billboard(cam, top - v3(0.0, 0.25, 0.0), 0.7, lina(col, 0.25));
            }
        }
        for gr in &g.grenades {
            if gr.kind == 2 {
                // a throwing star, spinning flat as it flies, with a faint streak behind. Fresh
                // from the hand it's drawn where the hand let go, easing onto its true path.
                let age = STAR_LIFE - gr.fuse;
                let k = (1.0 - age / STAR_VIS_TIME).clamp(0.0, 1.0);
                let pos = gr.pos + gr.vis * (k * k);
                let spin = M3::rot_y(gr.fuse * 45.0);
                let steel = lin([190, 196, 210]);
                op.obox(pos, &spin, v3(0.12, 0.008, 0.028), steel, -1.0, 2.5);
                op.obox(pos, &spin, v3(0.028, 0.008, 0.12), steel, -1.0, 2.5);
                op.ellipsoid(pos, &spin, v3(0.035, 0.012, 0.035), lin([60, 62, 70]), -1.0, 1.0, 0.0);
                let d = gr.vel.norm();
                let tail = (age * STAR_SPEED).min(0.9);
                ad.beam(cam, pos - d * tail, pos, 0.03, lina([210, 230, 255], 0.35));
                continue;
            }
            if gr.kind == 1 {
                let d = gr.vel.norm();
                op.tube(gr.pos - d * 0.35, gr.pos + d * 0.2, 0.06, 0.04, V3::UP.cross(d), 1.0, lin([70, 80, 64]), -1.0, 0.8);
                ad.billboard(cam, gr.pos - d * 0.45, 0.35, lina([255, 180, 80], 0.9));
                al.billboard(cam, gr.pos - d * 0.9, 0.3, lina([160, 160, 160], 0.4));
                continue;
            }
            op.ellipsoid(gr.pos, &M3::rot_y(gr.fuse * 9.0), v3(0.1, 0.11, 0.1), lin([45, 52, 45]), -1.0, 1.0, 0.0);
            // the fuse light: blinks faster and brighter as it runs out
            let urgency = (1.0 - gr.fuse / 1.7).clamp(0.0, 1.0);
            if ((1.7 - gr.fuse) * (3.0 + 10.0 * urgency)).fract() < 0.45 {
                ad.billboard(cam, gr.pos + v3(0.0, 0.13, 0.0), 0.16 + 0.12 * urgency, lina([255, 40, 30], 1.0));
                ad.billboard(cam, gr.pos, 0.5 + 0.3 * urgency, lina([255, 60, 40], 0.25));
            }
        }
        let fx_every = (1.0 / self.quality.fx.clamp(0.1, 1.0)).round().max(1.0) as usize;
        for (pi, p) in g.particles.iter().enumerate() {
            if pi % fx_every != 0 {
                continue;
            }
            let t = (p.life / p.max_life).clamp(0.0, 1.0);
            let size = if p.size > 0.5 { p.size * (1.3 - t) * 2.0 } else { p.size * 2.2 };
            let glow = p.color[0] > 240 && p.color[1] > 150;
            if glow && p.size < 0.13 && p.vel.len() > 2.0 {
                // fast sparks read as streaks
                ad.beam(cam, p.pos - p.vel * 0.035, p.pos, p.size * 0.35, lina(p.color, t));
            } else if glow {
                ad.billboard(cam, p.pos, size, lina(p.color, t));
            } else {
                al.billboard(cam, p.pos, size, lina(p.color, t * 0.9));
            }
        }
        // Warden's Marks: a red spark in flight; landed, a pulsing beacon with a ring showing
        // its reach, so everyone sees where it is
        // stains and cracks on the floor (executions)
        for d in &g.decals {
            decal_into(&mut al, &mut ad, d);
        }
        water_foam(&mut al, &g.world, g.time);
        // an execution's afterimages shimmer in their colour
        for f in decoys.iter().filter(|f| f.ghost > 0.0) {
            let c = PALETTE[(f.color_idx as usize) % PALETTE.len()];
            for (n, h) in [0.4f32, 0.95, 1.5].iter().enumerate() {
                let wob = (g.time * 13.0 + n as f32 * 2.1).sin();
                ad.billboard(cam, f.pos + v3(wob * 0.1, *h, 0.0), 0.5, lina(c, 0.12 * f.ghost));
            }
        }
        for m in &g.marks {
            if !m.landed {
                ad.billboard(cam, m.pos, 0.3, lina([255, 80, 60], 0.9));
                continue;
            }
            let pulse = 0.5 + 0.5 * (g.time * 6.0).sin();
            ad.billboard(cam, m.pos, 0.5 + 0.2 * pulse, lina([255, 70, 50], 0.9));
            for k in 0..6 {
                ad.billboard(cam, m.pos + v3(0.0, 0.4 + k as f32 * 0.5, 0.0), 0.35 - k as f32 * 0.04, lina([255, 90, 60], 0.35 - k as f32 * 0.05));
            }
            let r = g.tune.mark_radius;
            let n = 40;
            for k in 0..n {
                let a = k as f32 / n as f32 * std::f32::consts::TAU + g.time * 0.3;
                ad.billboard(cam, m.pos + v3(a.sin() * r, 0.15, a.cos() * r), 0.3, lina([255, 80, 60], 0.25 + 0.15 * pulse));
            }
        }
        for fl in &g.flashes {
            let k = (fl.life / 0.06).clamp(0.0, 1.0);
            ad.billboard(cam, fl.pos, fl.size * (0.6 + 0.4 * k), lina([255, 205, 120], k));
            ad.billboard(cam, fl.pos, fl.size * 0.35, lina([255, 255, 230], k));
            ad.beam(cam, fl.pos, fl.pos + fl.dir * fl.size * 1.8, fl.size * 0.3, lina([255, 185, 90], k * 0.9));
        }
        // spent shells and dropped magazines (cheap: drawn on every preset)
        for c in g.casings.iter() {
            let r = M3::rot_y(c.spin).mul(&M3::rot_x(c.spin * 1.3));
            if c.kind == 1 {
                op.obox(c.pos, &r, v3(0.022, 0.07, 0.035), lin(c.color), -1.0, 1.2);
            } else {
                op.obox(c.pos, &r, v3(0.012, 0.012, 0.03), lin(c.color), -1.0, 1.8);
            }
        }
        // smoke clouds: dense, soft puffs (they hide what's behind them)
        for (si, s) in g.smokes.iter().enumerate() {
            let k = s.density();
            if k <= 0.01 {
                continue;
            }
            let col = if s.body == 2 { [132, 120, 108] } else { [150, 154, 162] };
            let mut rng = Rng(0x9E37_79B9 ^ (si as u64 + 1).wrapping_mul(2_654_435_761));
            let r = crate::game::SMOKE_R * (0.6 + 0.4 * k);
            for n in 0..34 {
                let a = rng.range(0.0, std::f32::consts::TAU);
                let rr = rng.range(0.0, 1.0f32).sqrt() * r * 0.8;
                let y = rng.range(0.2, 2.6);
                let drift = v3((g.time * 0.21 + n as f32).sin() * 0.25, (g.time * 0.13 + n as f32 * 0.7).sin() * 0.1, (g.time * 0.17 + n as f32 * 1.3).cos() * 0.25);
                let p = s.pos + v3(a.cos() * rr, y, a.sin() * rr) + drift;
                let size = rng.range(1.3, 2.1) * (0.7 + 0.3 * k);
                let shade = rng.range(0.85, 1.05);
                let c = [(col[0] as f32 * shade) as u8, (col[1] as f32 * shade) as u8, (col[2] as f32 * shade) as u8];
                al.billboard(cam, p, size, lina(c, 0.62 * k));
            }
            if s.body == 2 && k > 0.3 {
                // the forge smoke carries a few embers
                for n in 0..6 {
                    let ph = (g.time * 0.4 + n as f32 * 0.17).fract();
                    let a = n as f32 * 1.1 + g.time * 0.3;
                    ad.billboard(cam, s.pos + v3(a.cos() * 1.5, 0.3 + ph * 2.6, a.sin() * 1.5), 0.08, lina([255, 150, 60], 0.8 * (1.0 - ph) * k));
                }
            }
        }
        for tr in &g.tracers {
            let t = (tr.life / tr.max_life.max(0.01)).clamp(0.0, 1.0);
            ad.beam(cam, tr.a, tr.b, tr.width * (0.6 + 0.4 * t), lina(tr.color, t));
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
        // torch flames: the same, smaller
        for (ti, b) in g.world.torches.iter().enumerate() {
            for k in 0..5 {
                let ph = (g.time * 2.1 + k as f32 / 5.0 + ti as f32 * 0.41).fract();
                let jx = (k as f32 * 12.9898 + ti as f32).sin() * 0.05;
                let jz = (k as f32 * 78.233 + ti as f32).cos() * 0.05;
                let p = *b + v3(jx * (1.0 - ph), ph * 0.38, jz * (1.0 - ph));
                let col = if ph < 0.4 { [255, 205, 100] } else { [255, 125, 45] };
                ad.billboard(cam, p, 0.13 * (1.0 - ph) + 0.03, lina(col, 0.9 * (1.0 - ph)));
            }
            ad.billboard(cam, *b + v3(0.0, 0.12, 0.0), 0.5, lina([255, 150, 60], 0.16));
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
        let n_inst = insts.len() as u32;
        if n_inst > 0 {
            let flat: Vec<f32> = insts.concat();
            rs.queue.write_buffer(&self.inst_buf, 0, bytemuck::cast_slice(&flat));
        }

        // ---- draw
        let t = self.target.as_ref()?;
        let mut enc = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("duel frame") });
        let a0 = n_op as u32;
        let a1 = (n_op + n_al) as u32;
        let a2 = (n_op + n_al + n_ad) as u32;
        // 1. the sun's view: depth of everything solid, for the shadows
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("duel shadow pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.shadow_bind, &[]);
            pass.set_pipeline(&self.pipe_shadow);
            pass.set_vertex_buffer(0, self.static_buf.slice(..));
            pass.draw(0..self.static_count, 0..1);
            if a0 > 0 {
                pass.set_vertex_buffer(0, self.dyn_buf.slice(..));
                pass.draw(0..a0, 0..1);
            }
            if let (Some(sk), true) = (&self.skin, n_inst > 0) {
                pass.set_pipeline(&self.pipe_skin_shadow);
                for k in 0..n_inst {
                    let (vb, ib, n) = match inst_body.get(k as usize).and_then(|&b| sk.bodies.get(b)) {
                        Some((v, i, n)) => (v, i, *n),
                        None => (&sk.vbuf, &sk.ibuf, sk.icount),
                    };
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..n, 0, k..k + 1);
                }
            }
        }
        // 2. the scene, in high dynamic range
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("duel pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.msaa_view,
                    depth_slice: None,
                    resolve_target: if t.multisampled { Some(&t.resolve_view) } else { None },
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1000.0 }), store: if t.multisampled { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store } },
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
            if a0 > 0 {
                pass.set_vertex_buffer(0, self.dyn_buf.slice(..));
                pass.draw(0..a0, 0..1);
            }
            if let (Some(sk), true) = (&self.skin, n_inst > 0) {
                pass.set_pipeline(&self.pipe_skin);
                for k in 0..n_inst {
                    // each fighter's own mesh (the plain mannequin if they didn't load)
                    let (vb, ib, n) = match inst_body.get(k as usize).and_then(|&b| sk.bodies.get(b)) {
                        Some((v, i, n)) => (v, i, *n),
                        None => (&sk.vbuf, &sk.ibuf, sk.icount),
                    };
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..n, 0, k..k + 1);
                }
                // their outlines
                pass.set_pipeline(&self.pipe_outline);
                for k in 0..n_inst {
                    let (vb, ib, n) = match inst_body.get(k as usize).and_then(|&b| sk.bodies.get(b)) {
                        Some((v, i, n)) => (v, i, *n),
                        None => (&sk.vbuf, &sk.ibuf, sk.icount),
                    };
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..n, 0, k..k + 1);
                }
            }
            if !all.is_empty() {
                pass.set_vertex_buffer(0, self.dyn_buf.slice(..));
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
        // 3. the effects, then the finished picture
        let p = &self.post;
        if fx.ssao {
            post_pass(&mut enc, &t.ao_view, &p.ao, &t.bg_ao);
        }
        if fx.bloom {
            post_pass(&mut enc, &t.bloom_a, &p.bright, &t.bg_bright);
            post_pass(&mut enc, &t.bloom_b, &p.blur_h, &t.bg_blur_h);
            post_pass(&mut enc, &t.bloom_a, &p.blur_v, &t.bg_blur_v);
        }
        if fx.rays {
            // (with the sun off screen the pass just clears them)
            post_pass(&mut enc, &t.rays_view, &p.rays, &t.bg_rays);
        }
        post_pass(&mut enc, &t.out_view, &p.comp, &t.bg_comp);        rs.queue.submit(Some(enc.finish()));
        self.tex_id
    }
}

#[cfg(test)]
mod cloth_tests {
    use super::*;

    #[test]
    fn the_coat_hangs_and_swings_without_going_wild() {
        let rig = crate::anim::shared().expect("rig");
        let mut f = Fighter::new("t", 0, true);
        f.present = true;
        f.alive = true;
        f.on_ground = true;
        let mut st = CharState::new();
        let lk = look(0);
        let mut mb = MB { v: Vec::new() };
        let mut pos = V3::ZERO;
        for k in 0..240 {
            let t = k as f32 / 120.0;
            // stand a second, then run
            if k > 120 {
                f.vel = v3(0.0, 0.0, 7.2);
                pos = pos + f.vel * (1.0 / 120.0);
            }
            let p = character::animate(rig, &mut st, &f, pos, t, &Tuning::default()).expect("posed");
            mb.v.clear();
            coat_cloth(&mut mb, &p, &mut st, t, &lk, 1.0, 0, 0);
            let waist = p.hit.caps[4].b.y + 0.1;
            let _ = waist;
            let belt = st.cloth[..CLOTH_COLS].iter().fold(f32::MIN, |m, q| m.max(q.y));
            for q in &st.cloth {
                assert!(q.x.is_finite() && q.y.is_finite() && q.z.is_finite());
                // (a knee coming up can lift the open front edge a little above the belt)
                assert!(q.y <= belt + 0.15 && q.y >= -0.05, "coat between the belt and the floor (k {k}): {q:?}, belt {belt}");
                assert!((*q - pos).flat().len() < 1.2, "stays with the body: {q:?}");
            }
            assert!(!mb.v.is_empty());
            if k == 120 {
                // standing still it hangs: the hem is well below the waist
                let hem = st.cloth[st.cloth.len() - 1];
                assert!(hem.y < waist - 0.45, "hangs down: {hem:?}");
            }
        }
        // running, it trails behind (towards -z)
        let hem_mid = st.cloth[(CLOTH_ROWS - 1) * CLOTH_COLS + CLOTH_COLS / 2];
        assert!(hem_mid.z < pos.z - 0.12, "trails behind when running: {hem_mid:?} vs {pos:?}");
    }
}

#[cfg(test)]
mod dynres_tests {
    use super::*;

    #[test]
    fn late_frames_lower_the_resolution_and_on_time_ones_bring_it_back_without_bouncing() {
        let mut d = DynRes::default();
        // a heavy moment: 45 fps for two seconds
        for _ in 0..90 {
            d.update(1.0 / 45.0, 60.0);
        }
        assert!(d.scale < 0.9 && d.scale >= DynRes::MIN, "stepped down: {}", d.scale);
        let low = d.scale;
        // a single hitch doesn't move it
        let mut e = DynRes::default();
        e.update(0.1, 60.0);
        for _ in 0..30 {
            e.update(1.0 / 60.0, 60.0);
        }
        assert_eq!(e.scale, 1.0);
        // steady 60 again: it climbs back over time
        for _ in 0..(60 * 30) {
            d.update(1.0 / 60.0, 60.0);
        }
        assert!(d.scale > low, "came back up: {}", d.scale);
        assert!(d.update(f32::NAN, 60.0).is_finite());
    }
}

/// The reported freeze: a bot's gunshot kills you as the match-winning kill. Every frame here is
/// run the way the game runs one (the kill cam's clock, slow motion, the ticks, every fighter
/// posed with its coat, the cameras, the career count) and none may take a second.
#[cfg(test)]
mod final_kill_tests {
    use super::*;
    use crate::career::Career;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[derive(Clone, Copy, Debug)]
    enum Doing {
        Standing,
        InAir,
        Dashing,
        Sliding,
        Scoped,
        Reloading,
    }

    const FRAME: f32 = 1.0 / 60.0;

    struct Run {
        g: Game,
        acc: f32,
        chars: Vec<CharState>,
        career: Career,
        label: String,
    }

    impl Run {
        /// One frame as the game draws it. Checks that nothing became a broken number.
        fn frame(&mut self, inp: &mut Input) {
            let g = &mut self.g;
            g.view_alpha = (self.acc / TICK).clamp(0.0, 1.0);
            g.advance(&mut self.acc, FRAME, inp);
            // the career count (not saved: this is a test)
            for ev in std::mem::take(&mut g.career) {
                self.career.apply(ev);
            }
            // sounds go to the speakers with these values
            for (_, at, pitch) in g.sounds.drain(..) {
                assert!(pitch.is_finite() && pitch > 0.0, "{}: sound pitch {pitch}", self.label);
                if let Some(p) = at {
                    assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite(), "{}: sound at {p:?}", self.label);
                }
            }
            let alpha = (self.acc / TICK).clamp(0.0, 1.0);
            // the cameras: kill cam, or following your body
            if let Some((pos, at)) = g.killcam_view(alpha) {
                assert!((at - pos).len() > 0.5, "{}: kill cam too close", self.label);
            }
            let me = &g.f[g.local];
            let (pivot, dir, d) = g.camera_rig(me.pos, me.yaw, me.pitch);
            let _ = g.world.raycast(pivot - dir * d, dir, 300.0);
            // everyone posed and dressed, as the renderer does
            let rig = crate::anim::shared().expect("rig");
            let lk = look(0);
            let mut mb = MB { v: Vec::new() };
            for (i, f) in g.f.iter().enumerate() {
                let ok = |v: V3| v.x.is_finite() && v.y.is_finite() && v.z.is_finite();
                assert!(ok(f.pos) && ok(f.vel) && f.yaw.is_finite() && f.pitch.is_finite(), "{}: fighter {i} broken: {:?} {:?} {} {}", self.label, f.pos, f.vel, f.yaw, f.pitch);
                if !f.present || f.spectate {
                    continue;
                }
                let pos = f.prev_pos.lerp(f.pos, alpha);
                if let Some(p) = character::animate_show(rig, &mut self.chars[i], f, pos, g.time, &g.tune) {
                    mb.v.clear();
                    coat_cloth(&mut mb, &p, &mut self.chars[i], g.time, &lk, g.tune.char_scale, f.loadout.body, f.loadout.outfit);
                }
            }
        }
    }

    /// Keep the scene set up for the final shot: you on 1 HP doing `doing`, the shooter in
    /// front of you with `gun`, nobody else around.
    fn stage(g: &mut Game, shooter: usize, gun: Weapon, doing: Doing, inp: &mut Input, k: usize) {
        let me = g.local;
        // somewhere 9 m away with a clear view of you (too far for blades)
        let here = g.f[me].pos;
        let there = g.f[shooter].pos;
        let d = (there - here).flat().len();
        let clear = |p: V3| g.world.line_clear(here + v3(0.0, 1.2, 0.0), p + v3(0.0, 1.5, 0.0));
        let mut spot = None;
        if d < 6.0 || d > 12.0 || !clear(there) {
            let ground = if g.f[me].on_ground { here.y } else { there.y.min(here.y) };
            for j in 0..16 {
                let a = j as f32 * PI / 8.0;
                let p = v3(here.x + a.sin() * 9.0, ground, here.z + a.cos() * 9.0);
                if p.x.abs() < g.world.half - 1.0 && p.z.abs() < g.world.half - 1.0 && clear(p) && g.world.line_clear(p + v3(0.0, 0.3, 0.0), p + v3(0.0, 1.9, 0.0)) {
                    spot = Some(p);
                    break;
                }
            }
        }
        for s in 0..g.f.len() {
            if s != me && s != shooter && g.f[s].present && g.f[s].alive {
                // the others stand far away, and can't fall (only the shot at you may end it)
                g.f[s].pos = v3(-14.0 + s as f32 * 2.0, g.f[s].pos.y, -14.0);
                g.f[s].vel = V3::ZERO;
                g.f[s].hp = g.f[s].hp.max(500.0);
            }
        }
        let (f, b) = if me < shooter {
            let (a, b) = g.f.split_at_mut(shooter);
            (&mut a[me], &mut b[0])
        } else {
            let (a, b) = g.f.split_at_mut(me);
            (&mut b[0], &mut a[shooter])
        };
        f.hp = 1.0;
        f.spawn_prot = 0.0;
        f.regen_left = 0.0;
        b.spawn_prot = 0.0;
        b.loadout.gun1 = gun;
        b.loadout.gun2 = gun;
        if b.cur == 0 {
            b.cur = 1;
        }
        b.weapon = gun;
        b.mag = [b.mag[0], 30, 30];
        b.items = 0; // a gunshot, not a grenade
        b.reload_t = 0.0;
        if let Some(p) = spot {
            b.pos = p;
            b.prev_pos = p;
            b.vel = V3::ZERO;
        }
        *inp = Input { yaw: f.yaw, pitch: f.pitch, ..Default::default() };
        match doing {
            Doing::Standing => {}
            Doing::InAir => {
                if f.on_ground {
                    f.vel.y = 7.0;
                    f.on_ground = false;
                }
            }
            Doing::Dashing => {
                inp.fwd = 1.0;
                inp.dash = k % 40 == 0;
            }
            Doing::Sliding => {
                inp.fwd = 1.0;
                inp.crouch = true;
            }
            Doing::Scoped => {
                f.loadout.gun2 = Weapon::Sniper;
                f.cur = 2;
                f.weapon = Weapon::Sniper;
                f.scope_t = 1.0;
                inp.guard = true;
            }
            Doing::Reloading => {
                f.reload_t = 1.0;
                inp.reload = true;
            }
        }
    }

    /// Play one final kill and what follows. Sends a beat after every frame.
    fn play(mode: Mode, gun: Weapon, doing: Doing, beat: &mpsc::Sender<String>) {
        let label = format!("{mode:?} / {gun:?} / {doing:?}");
        let mut g = if mode == Mode::Gauntlet {
            Game::gauntlet("P", 0)
        } else {
            let bots = if matches!(mode, Mode::Duel | Mode::Lms) { 1 } else { 3 };
            Game::offline("P", bots, 1.0, mode, 3)
        };
        g.f[0].ready = true;
        let n = g.f.len();
        let mut run = Run { g, acc: 0.0, chars: (0..n).map(|_| CharState::new()).collect(), career: Career::default(), label: label.clone() };
        let mut inp = Input::default();
        let mut frames = 0usize;
        let mut step = |run: &mut Run, inp: &mut Input| {
            let t0 = Instant::now();
            run.frame(inp);
            let took = t0.elapsed();
            assert!(took < Duration::from_secs(1), "{}: a frame took {took:?}", run.label);
            frames += 1;
            let _ = beat.send(format!("{} frame {frames} phase {:?} killcam {:?} slowmo {:.2}", run.label, run.g.mt.phase, run.g.killcam.as_ref().map(|k| k.t), run.g.slowmo));
        };
        // into the fight (the gauntlet's first wave comes after a short wait)
        for _ in 0..(8.0 / FRAME) as usize {
            step(&mut run, &mut inp);
            let wave_in = mode != Mode::Gauntlet || (1..n).any(|s| run.g.f[s].present && run.g.f[s].alive);
            if run.g.mt.phase == Phase::Fight && wave_in {
                break;
            }
        }
        assert_eq!(run.g.mt.phase, Phase::Fight, "{label}: never got into the fight");
        // the shooter is one kill (or round) from winning
        let shooter = (1..n).find(|&s| run.g.f[s].present && run.g.f[s].alive && !run.g.f[s].spectate).expect("a shooter");
        let t = run.g.mt.target as i32;
        match mode {
            Mode::Ffa => run.g.f[shooter].kills = t - 1,
            Mode::Tdm => {
                let team = run.g.f[shooter].team as usize;
                run.g.mt.team_score[team.min(1)] = (t - 1).max(0) as _;
            }
            Mode::Duel | Mode::Lms => run.g.f[shooter].wins = t - 1,
            _ => {}
        }
        assert!(run.g.is_enemy(shooter, 0), "{label}: the shooter is on your side");
        // the bot takes its shot (by itself: its own aim and trigger)
        let mut ended = false;
        for k in 0..(12.0 / FRAME) as usize {
            stage(&mut run.g, shooter, gun, doing, &mut inp, k);
            step(&mut run, &mut inp);
            if run.g.mt.phase == Phase::MatchOver {
                ended = true;
                break;
            }
        }
        assert!(ended, "{label}: the bot never landed the final shot");
        assert!(!run.g.f[0].alive, "{label}: you were the one who fell");
        let (victim, killer, w, _) = run.g.last_kill.expect("a last kill");
        assert_eq!((victim, killer), (0, Some(shooter)), "{label}: the bot killed you");
        assert!(w == gun as u8 || w == Weapon::Rocket as u8, "{label}: killed by a gunshot, got tool {w}");
        assert!(run.g.killcam.is_some() && run.g.slowmo > 0.0, "{label}: the kill cam starts in slow motion");
        assert_eq!(run.career.deaths, 1, "{label}: your death counted");
        // the kill cam, the slow motion and the end screen, with you still holding keys
        let kills_then: Vec<i32> = run.g.f.iter().map(|f| f.kills).collect();
        for k in 0..(8.0 / FRAME) as usize {
            inp = Input { yaw: run.g.f[0].yaw + 0.01 * k as f32, fwd: 1.0, fire: true, guard: k % 2 == 0, dash: k % 30 == 0, jump: k % 40 == 0, ..Default::default() };
            step(&mut run, &mut inp);
        }
        assert_eq!(run.g.mt.phase, Phase::MatchOver, "{label}: the end screen stays up");
        assert!(run.g.slowmo <= 0.0 && run.g.killcam_view(0.5).is_none(), "{label}: the kill cam finished");
        let kills_now: Vec<i32> = run.g.f.iter().map(|f| f.kills).collect();
        assert_eq!(kills_then, kills_now, "{label}: nobody scores after the match");
        if mode != Mode::Gauntlet {
            assert!(run.career.matches == 1, "{label}: the match counted");
        }
        // rematch: everyone back in
        run.g.vote_rematch();
        for _ in 0..(6.0 / FRAME) as usize {
            inp = Input { yaw: run.g.f[0].yaw, ..Default::default() };
            step(&mut run, &mut inp);
        }
        assert_ne!(run.g.mt.phase, Phase::MatchOver, "{label}: the rematch started");
        assert!(run.g.killcam.is_none() && run.g.slowmo <= 0.0, "{label}: no kill cam left over");
    }

    /// 60 fps on a Steam Deck leaves 16.7 ms a frame for everything. The game's own work for a
    /// frame (the ticks, eight fighters posed with their coats, the cameras) must stay a small
    /// part of that, since the Deck's processor is slower than a desktop's.
    #[test]
    fn a_full_lobby_frame_fits_a_steam_deck_budget() {
        let mut g = Game::offline("P", 7, 0.8, Mode::Ffa, 200);
        g.f[0].ready = true;
        let n = g.f.len();
        let mut run = Run { g, acc: 0.0, chars: (0..n).map(|_| CharState::new()).collect(), career: Career::default(), label: "full lobby".into() };
        let mut inp = Input::default();
        for _ in 0..(5.0 / FRAME) as usize {
            run.frame(&mut inp);
        }
        assert_eq!(run.g.mt.phase, Phase::Fight);
        let mut times = Vec::new();
        for _ in 0..(10.0 / FRAME) as usize {
            inp = Input { yaw: run.g.f[0].yaw, ..Default::default() };
            let t0 = Instant::now();
            run.frame(&mut inp);
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(|a, b| a.total_cmp(b));
        let avg = times.iter().sum::<f64>() / times.len() as f64;
        let p99 = times[times.len() * 99 / 100];
        eprintln!("full lobby frame: average {avg:.2} ms, 99th percentile {p99:.2} ms");
        assert!(avg < 3.0, "average {avg:.2} ms (budget 3 ms here, about 6 on a Deck)");
        assert!(p99 < 6.0, "99th percentile {p99:.2} ms");
    }

    #[test]
    fn a_bot_shooting_you_for_the_win_never_freezes_a_frame() {
        let mut cases = Vec::new();
        for mode in [Mode::Ffa, Mode::Tdm, Mode::Duel, Mode::Lms, Mode::Gauntlet] {
            for gun in [Weapon::Revolver, Weapon::Rifle, Weapon::Shotgun, Weapon::Smg, Weapon::HandCannon] {
                cases.push((mode, gun, Doing::Standing));
            }
            for doing in [Doing::InAir, Doing::Dashing, Doing::Sliding, Doing::Scoped, Doing::Reloading] {
                cases.push((mode, Weapon::Rifle, doing));
            }
        }
        cases.push((Mode::Ffa, Weapon::Sniper, Doing::Standing));
        let (tx, rx) = mpsc::channel::<String>();
        let worker = std::thread::spawn(move || {
            for (mode, gun, doing) in cases {
                play(mode, gun, doing, &tx);
            }
        });
        // a frame that never ends is a freeze: wait at most a second for each one
        let mut last = String::from("start");
        loop {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(m) => last = m,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if worker.is_finished() {
                        break;
                    }
                    panic!("froze: no frame finished within a second after {last}");
                }
            }
        }
        if let Err(e) = worker.join() {
            std::panic::resume_unwind(e);
        }
    }
}

#[cfg(test)]
mod map_cost_tests {
    use super::*;

    /// How much static geometry each map makes (printed; the team map is checked against the
    /// biggest duel map).
    #[test]
    fn static_geometry_sizes() {
        let mut sizes = Vec::new();
        for map in crate::world::MapId::ALL {
            let w = World::build(map);
            let v = build_world(&w);
            let mut mb = MB::default();
            for p in &w.props {
                prop_into(&mut mb, p, map);
            }
            let props = mb.v.len() / 3;
            let mut kinds: Vec<(String, usize, usize)> = Vec::new();
            for p in &w.props {
                let name = format!("{:?}", p.kind);
                let tris = env::prop_mesh(p.kind).len() / 3;
                match kinds.iter_mut().find(|k| k.0 == name) {
                    Some(k) => k.1 += 1,
                    None => kinds.push((name, 1, tris)),
                }
            }
            kinds.sort_by_key(|k| std::cmp::Reverse(k.1 * k.2));
            let top: Vec<String> = kinds.iter().take(6).map(|k| format!("{} {}x{}", k.0, k.1, k.2)).collect();
            println!("  {map:?} props by cost: {}", top.join(", "));
            let mut mb = MB::default();
            for vn in &w.vines {
                vine_into(&mut mb, vn);
            }
            let vines = mb.v.len() / 3;
            println!(
                "{map:?}: {} blocks, {} triangles ({props} in props, {vines} in vines), {} props, {} lights",
                w.blocks.len(),
                v.len() / 3,
                w.props.len(),
                w.torches.len() + w.braziers.len()
            );
            sizes.push((map, v.len() / 3));
        }
    }
}
