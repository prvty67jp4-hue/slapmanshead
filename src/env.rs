//! How the maps look: CC0 PBR materials and HDRI skies from Poly Haven, and CC0 props from
//! Quaternius (sources in assets/CREDITS.txt). Everything here is plain data the renderer uploads:
//! material layers (colour, and normal + roughness + metalness), one sky per map with the light it
//! gives (sun direction and colour, ambient light as spherical harmonics, fog colour) and its mood
//! (colour grading), and the prop meshes.

use crate::math::*;
use crate::world::{Mat, MapId, PropKind};
use std::sync::OnceLock;

/// Every material layer is this many pixels square.
pub const TEX: u32 = 1024;

/// Layers of the level's material arrays. The level's own materials (0-5 and 14-16) hold
/// softened brightness detail only; each map's palette gives them their colour (see `surface`).
pub const L_FLOOR_SLABS: u32 = 0;
pub const L_SANDSTONE: u32 = 1;
pub const L_RUBBLE_STONE: u32 = 2;
pub const L_IRON: u32 = 3;
pub const L_WOOD: u32 = 4;
pub const L_MARBLE: u32 = 5;
pub const L_TRIM_CLOTH: u32 = 8;
pub const L_TRIM_FURNITURE: u32 = 9;
pub const L_TRIM_METAL: u32 = 10;
pub const L_TRIM_PROPS: u32 = 11;
pub const L_LEAVES: u32 = 12;
pub const L_ROCKS: u32 = 13;
pub const L_BRICK: u32 = 14;
pub const L_SLATE: u32 = 15;
pub const L_BLOCKS: u32 = 16;
pub const LAYERS: u32 = 17;

/// Where a texture comes from.
enum Src {
    /// Poly Haven set: colour, OpenGL normal map, roughness; a fixed metalness. Turned into soft
    /// brightness detail for the palette to colour.
    Haven { diff: &'static [u8], nor: &'static [u8], rough: &'static [u8], metal: f32 },
    /// Quaternius trim sheet: colour, normal, occlusion / roughness / metalness.
    Trim { diff: &'static [u8], nor: &'static [u8], orm: &'static [u8] },
    /// Colour only (with alpha for cut-out leaves), flat and rough.
    Plain { diff: &'static [u8], rough: f32 },
    /// Painted by the renderer (banner cloth, fabric).
    Procedural,
}

macro_rules! prop_tex {
    ($n:literal) => {
        include_bytes!(concat!("../assets/env/props/", $n))
    };
}
macro_rules! haven {
    ($n:literal, $metal:expr) => {
        Src::Haven { diff: include_bytes!(concat!("../assets/env/tex/", $n, "_diff.jpg")), nor: include_bytes!(concat!("../assets/env/tex/", $n, "_nor.jpg")), rough: include_bytes!(concat!("../assets/env/tex/", $n, "_rough.jpg")), metal: $metal }
    };
}

fn source(layer: u32) -> Src {
    match layer {
        L_FLOOR_SLABS => haven!("large_floor_tiles_02", 0.0),
        L_SANDSTONE => haven!("sandstone_blocks_08", 0.0),
        L_RUBBLE_STONE => haven!("medieval_blocks_05", 0.0),
        L_IRON => haven!("metal_plate_02", 0.55),
        L_WOOD => haven!("dark_planks", 0.0),
        L_MARBLE => haven!("marble_01", 0.0),
        L_BRICK => haven!("white_sandstone_blocks_02", 0.0),
        L_SLATE => haven!("patterned_slate_tiles", 0.0),
        L_BLOCKS => haven!("stone_pavers", 0.0),
        L_TRIM_CLOTH => Src::Trim { diff: prop_tex!("trim_cloth_diff.jpg"), nor: prop_tex!("trim_cloth_nor.jpg"), orm: prop_tex!("trim_cloth_orm.jpg") },
        L_TRIM_FURNITURE => Src::Trim { diff: prop_tex!("trim_furniture_diff.jpg"), nor: prop_tex!("trim_furniture_nor.jpg"), orm: prop_tex!("trim_furniture_orm.jpg") },
        L_TRIM_METAL => Src::Trim { diff: prop_tex!("trim_metal_diff.jpg"), nor: prop_tex!("trim_metal_nor.jpg"), orm: prop_tex!("trim_metal_orm.jpg") },
        L_TRIM_PROPS => Src::Trim { diff: prop_tex!("trim_props_diff.jpg"), nor: prop_tex!("trim_props_nor.jpg"), orm: prop_tex!("trim_props_orm.jpg") },
        L_LEAVES => Src::Plain { diff: prop_tex!("leaves.png"), rough: 0.7 },
        L_ROCKS => Src::Plain { diff: prop_tex!("rocks_diff.jpg"), rough: 0.85 },
        _ => Src::Procedural,
    }
}

/// Metres one repeat of a layer covers, set so a stone is about 40-60 cm everywhere (and floor
/// slabs about a metre, so floors don't turn into a busy grid in the distance).
pub fn layer_tile(l: u32) -> f32 {
    match l {
        L_FLOOR_SLABS => 3.0,  // 3 x 3 slabs: 1 m each
        L_SANDSTONE => 2.0,    // about 4 blocks across: 50 cm
        L_RUBBLE_STONE => 3.4, // about 7 stones across: 50 cm
        L_IRON => 2.0,         // plates about 50 cm
        L_WOOD => 2.4,         // planks about 30 cm high
        L_MARBLE => 3.0,       // 3 slabs across: 1 m by 60 cm
        L_BRICK => 2.0,        // about 4 blocks across: 50 cm by 20 cm
        L_SLATE => 3.2,        // about 8 setts across: 40 cm
        L_BLOCKS => 1.8,       // 3 blocks across: 60 cm
        _ => 2.0,
    }
}

/// A map's look for one kind of surface: which layer, its colour (sRGB) and how shiny.
#[derive(Clone, Copy, Debug)]
pub struct Surface {
    pub layer: u32,
    pub tint: [u8; 3],
    pub shine: f32,
}

/// Each map keeps to three main materials and one accent, all mid-tone and muted so the
/// fighters (who keep the strongest colours) stand out.
/// - Arena: warm sandstone walls, a pale stone floor, dark wood trim (iron braziers).
/// - Close Quarters: cool grey stone, a slate floor, iron as the accent.
/// - Open Courtyard: white marble, light sandstone, blue banners as the accent.
pub fn surface(map: MapId, m: Mat) -> Surface {
    let s = |layer, tint, shine| Surface { layer, tint, shine };
    match map {
        MapId::Arena => match m {
            Mat::Floor => s(L_FLOOR_SLABS, [176, 166, 146], 0.3),
            Mat::Wall => s(L_SANDSTONE, [176, 149, 115], 0.05),
            Mat::Stone => s(L_SANDSTONE, [166, 140, 107], 0.08),
            Mat::Metal => s(L_SANDSTONE, [160, 135, 103], 0.08),
            Mat::Trim | Mat::Crate => s(L_WOOD, [92, 71, 55], 0.1),
        },
        MapId::CloseQuarters => match m {
            Mat::Floor => s(L_SLATE, [104, 108, 116], 0.2),
            Mat::Wall => s(L_BLOCKS, [136, 141, 150], 0.05),
            Mat::Stone => s(L_BLOCKS, [122, 127, 136], 0.08),
            Mat::Metal => s(L_BLOCKS, [116, 121, 130], 0.08),
            Mat::Trim | Mat::Crate => s(L_IRON, [112, 115, 122], 0.4),
        },
        MapId::Courtyard => match m {
            Mat::Floor => s(L_MARBLE, [142, 141, 136], 0.35),
            Mat::Wall => s(L_BRICK, [168, 153, 126], 0.05),
            Mat::Stone | Mat::Metal => s(L_MARBLE, [172, 170, 162], 0.3),
            Mat::Trim | Mat::Crate => s(L_SANDSTONE, [160, 146, 120], 0.08),
        },
    }
}

/// Braziers and other small iron fittings, on every map.
pub const IRON: Surface = Surface { layer: L_IRON, tint: [74, 73, 72], shine: 0.4 };

fn decode(bytes: &[u8]) -> image::RgbaImage {
    let img = image::load_from_memory(bytes).map(|i| i.to_rgba8()).unwrap_or_else(|_| image::RgbaImage::from_pixel(TEX, TEX, image::Rgba([128, 128, 128, 255])));
    if img.width() != TEX || img.height() != TEX { image::imageops::resize(&img, TEX, TEX, image::imageops::FilterType::Triangle) } else { img }
}

/// Softened: the fine photographic detail smoothed away, big shapes kept (a stylised look).
fn soften(img: &image::RgbaImage, to: u32) -> image::RgbaImage {
    let small = image::imageops::resize(img, to, to, image::imageops::FilterType::Triangle);
    image::imageops::resize(&small, TEX, TEX, image::imageops::FilterType::CatmullRom)
}

/// One material layer at full size: colour (sRGB, alpha = cut-out) and surface (normal x, normal
/// y, roughness, metalness). None for layers the renderer paints itself.
pub fn layer(layer: u32) -> Option<(Vec<u8>, Vec<u8>)> {
    let n = (TEX * TEX) as usize;
    match source(layer) {
        Src::Procedural => None,
        Src::Haven { diff, nor, rough, metal } => {
            let (d, nm, r) = (soften(&decode(diff), 512), soften(&decode(nor), 512), decode(rough));
            let mut surf = vec![0u8; n * 4];
            for k in 0..n {
                surf[k * 4] = nm.as_raw()[k * 4];
                surf[k * 4 + 1] = nm.as_raw()[k * 4 + 1];
                // a little rougher and more even: stylised surfaces, not wet photos
                surf[k * 4 + 2] = (150.0 + r.as_raw()[k * 4] as f32 * 0.4) as u8;
                surf[k * 4 + 3] = (metal * 255.0) as u8;
            }
            // brightness only (the map's palette colours it), squeezed: the dark mortar and grout
            // lifted most, so the lines between stones read as soft and subtle
            let lin = |v: u8| (v as f32 / 255.0).powf(2.2);
            let mut lum = vec![0.0f32; n];
            let mut mean = 0.0f64;
            for k in 0..n {
                let px = &d.as_raw()[k * 4..k * 4 + 3];
                let l = 0.2126 * lin(px[0]) + 0.7152 * lin(px[1]) + 0.0722 * lin(px[2]);
                lum[k] = l;
                mean += l as f64;
            }
            let mean = ((mean / n as f64) as f32).max(1e-4);
            let mut col = vec![0u8; n * 4];
            for k in 0..n {
                let l = lum[k];
                let squeezed = if l < mean { mean + (l - mean) * 0.42 } else { mean + (l - mean) * 0.62 };
                // detail around 1 (stored at half brightness: the shader doubles it)
                let detail = (squeezed / mean).clamp(0.55, 1.5) * 0.5;
                let s = (detail.powf(1.0 / 2.2) * 255.0).round() as u8;
                col[k * 4] = s;
                col[k * 4 + 1] = s;
                col[k * 4 + 2] = s;
                col[k * 4 + 3] = 255;
            }
            Some((col, surf))
        }        Src::Trim { diff, nor, orm } => {
            let (d, nm, o) = (decode(diff), decode(nor), decode(orm));
            let mut col = d.into_raw();
            let mut surf = vec![0u8; n * 4];
            for k in 0..n {
                // the occlusion is baked into the colour
                let ao = o.as_raw()[k * 4] as f32 / 255.0;
                let ao = 0.35 + 0.65 * ao;
                for c in 0..3 {
                    col[k * 4 + c] = (col[k * 4 + c] as f32 * ao) as u8;
                }
                col[k * 4 + 3] = 255;
                surf[k * 4] = nm.as_raw()[k * 4];
                surf[k * 4 + 1] = nm.as_raw()[k * 4 + 1];
                surf[k * 4 + 2] = o.as_raw()[k * 4 + 1];
                surf[k * 4 + 3] = o.as_raw()[k * 4 + 2];
            }
            Some((col, surf))
        }
        Src::Plain { diff, rough } => {
            let col = decode(diff).into_raw();
            let mut surf = vec![0u8; n * 4];
            for k in 0..n {
                surf[k * 4] = 128;
                surf[k * 4 + 1] = 128;
                surf[k * 4 + 2] = (rough * 255.0) as u8;
                surf[k * 4 + 3] = 0;
            }
            Some((col, surf))
        }
    }
}

/// Mip levels of a square RGBA8 image (box filter), the full size first.
pub fn mips(top: Vec<u8>, size: u32) -> Vec<Vec<u8>> {
    let mut out = vec![top];
    let mut s = size;
    while s > 1 {
        let prev = out.last().unwrap();
        let ns = s / 2;
        let mut m = vec![0u8; (ns * ns * 4) as usize];
        for y in 0..ns {
            for x in 0..ns {
                for k in 0..4 {
                    let at = |xx: u32, yy: u32| prev[((yy * s + xx) * 4 + k) as usize] as u32;
                    let v = at(2 * x, 2 * y) + at(2 * x + 1, 2 * y) + at(2 * x, 2 * y + 1) + at(2 * x + 1, 2 * y + 1);
                    m[((y * ns + x) * 4 + k) as usize] = ((v + 2) / 4) as u8;
                }
            }
        }
        out.push(m);
        s = ns;
    }
    out
}

// ---------------------------------------------------------------------------------- skies

/// One map's sky and mood.
pub struct Sky {
    pub w: u32,
    pub h: u32,
    /// RGBA half floats, full size first.
    pub mips: Vec<Vec<u16>>,
    /// Turn applied to the picture so its sun sits where the map's sunlight comes from.
    pub yaw: f32,
    /// Picture brightness (scaled so the sky averages about 1; checked by the tests, the
    /// renderer's grading sets its own).
    #[cfg_attr(not(test), allow(dead_code))]
    pub exposure: f32,
    /// Sunlight: direction towards the sun, colour, strength.
    pub sun_dir: V3,
    pub sun_col: [f32; 3],
    pub sun_power: f32,
    /// Ambient light from the whole sky (third order spherical harmonics, irradiance / pi).
    pub sh: [[f32; 3]; 9],
    pub fog: [f32; 3],
    pub fog_density: f32,
    pub grade: Grade,
}

/// Colour grading: one strong mood per map.
#[derive(Clone, Copy, Debug)]
pub struct Grade {
    pub lift: [f32; 3],
    pub gamma: [f32; 3],
    pub gain: [f32; 3],
    pub saturation: f32,
    pub contrast: f32,
    pub exposure: f32,
}

struct SkySrc {
    hdr: &'static [u8],
    /// Which way the sunlight should come from (azimuth, radians, as yaw_fwd) and how high
    /// (elevation clamped to this range, so shadows stay readable).
    sun_azimuth: f32,
    elevation: (f32, f32),
    sun_power: f32,
    fog_density: f32,
    grade: Grade,
}

fn sky_src(map: MapId) -> SkySrc {
    match map {
        // golden late afternoon: warm, rich, long shadows
        MapId::Arena => SkySrc {
            hdr: include_bytes!("../assets/env/sky/qwantani_late_afternoon_puresky.hdr"),
            sun_azimuth: 0.69,
            elevation: (0.5, 0.75),
            sun_power: 2.3,
            fog_density: 0.0032,
            grade: Grade { lift: [0.008, 0.004, -0.004], gamma: [1.0, 0.99, 0.97], gain: [1.04, 1.0, 0.95], saturation: 1.0, contrast: 1.03, exposure: 0.8 },
        },
        // bright partly cloudy midday: crisp and cool
        MapId::Courtyard => SkySrc {
            hdr: include_bytes!("../assets/env/sky/kloofendal_48d_partly_cloudy_puresky.hdr"),
            sun_azimuth: -0.9,
            elevation: (0.75, 0.95),
            sun_power: 2.25,
            fog_density: 0.0022,
            grade: Grade { lift: [-0.004, 0.0, 0.006], gamma: [1.0, 1.0, 1.01], gain: [0.99, 1.0, 1.03], saturation: 1.02, contrast: 1.02, exposure: 0.78 },
        },
        // stormy sunset: teal shadows, orange light, heavy and moody
        MapId::CloseQuarters => SkySrc {
            hdr: include_bytes!("../assets/env/sky/wasteland_clouds_puresky.hdr"),
            sun_azimuth: 2.3,
            elevation: (0.42, 0.6),
            sun_power: 1.9,
            fog_density: 0.0055,
            grade: Grade { lift: [0.0, 0.008, 0.016], gamma: [0.99, 1.0, 1.02], gain: [1.05, 0.99, 0.93], saturation: 0.95, contrast: 1.06, exposure: 0.85 },
        },
    }
}

/// Direction of an equirect pixel centre (v down from the top).
pub fn equirect_dir(u: f32, v: f32) -> V3 {
    let th = v * std::f32::consts::PI;
    let ph = (u - 0.5) * 2.0 * std::f32::consts::PI;
    v3(th.sin() * ph.sin(), th.cos(), th.sin() * ph.cos())
}

pub fn f16(x: f32) -> u16 {
    let x = if x.is_finite() { x.clamp(-65000.0, 65000.0) } else { 0.0 };
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let e = ((b >> 23) & 0xff) as i32 - 127 + 15;
    let m = b & 0x7f_ffff;
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let m = (m | 0x80_0000) >> (1 - e);
        return sign | ((m + 0x1000) >> 13) as u16;
    }
    if e >= 31 {
        return sign | 0x7bff;
    }
    let v = ((e as u32) << 10) | ((m + 0x1000) >> 13);
    sign | (v.min(0x7bff) as u16)
}

fn build_sky(map: MapId) -> Sky {
    let src = sky_src(map);
    let img = image::load_from_memory_with_format(src.hdr, image::ImageFormat::Hdr).map(|i| i.to_rgb32f()).ok();
    let (w, h, px) = match img {
        Some(i) => (i.width(), i.height(), i.into_raw()),
        None => (2, 1, vec![0.6, 0.65, 0.8, 0.6, 0.65, 0.8]),
    };
    let lum = |r: f32, g: f32, b: f32| 0.2126 * r + 0.7152 * g + 0.0722 * b;
    // the sun: the brightest spot
    let mut best = (0usize, 0.0f32);
    for k in 0..(w * h) as usize {
        let l = lum(px[k * 3], px[k * 3 + 1], px[k * 3 + 2]);
        if l > best.1 {
            best = (k, l);
        }
    }
    let (bx, by) = ((best.0 as u32 % w) as f32 + 0.5, (best.0 as u32 / w) as f32 + 0.5);
    let sun_hdr = equirect_dir(bx / w as f32, by / h as f32);
    let hdr_az = sun_hdr.x.atan2(sun_hdr.z);
    let yaw = src.sun_azimuth - hdr_az;
    let elev = sun_hdr.y.clamp(-1.0, 1.0).asin().clamp(src.elevation.0, src.elevation.1);
    let sun_dir = v3(src.sun_azimuth.sin() * elev.cos(), elev.sin(), src.sun_azimuth.cos() * elev.cos()).norm();
    // the sun's colour from the pixels around it
    let mut sc = [0.0f32; 3];
    let r = 6i32;
    for dy in -r..=r {
        for dx in -r..=r {
            let x = ((bx as i32 + dx).rem_euclid(w as i32)) as usize;
            let y = (by as i32 + dy).clamp(0, h as i32 - 1) as usize;
            let k = y * w as usize + x;
            for c in 0..3 {
                sc[c] += px[k * 3 + c];
            }
        }
    }
    let top = sc[0].max(sc[1]).max(sc[2]).max(1e-6);
    let sun_col = [sc[0] / top, sc[1] / top, sc[2] / top];
    // brightness: the sky (sun spot left out) averages about 1
    let mut sum = 0.0f64;
    let mut wsum = 0.0f64;
    for y in 0..h {
        let th = (y as f32 + 0.5) / h as f32 * std::f32::consts::PI;
        if th > std::f32::consts::FRAC_PI_2 {
            break;
        }
        let wt = th.sin() as f64;
        for x in 0..w {
            let k = (y * w + x) as usize;
            sum += lum(px[k * 3], px[k * 3 + 1], px[k * 3 + 2]).min(20.0) as f64 * wt;
            wsum += wt;
        }
    }
    let avg = (sum / wsum.max(1e-9)) as f32;
    let exposure = 1.0 / avg.max(1e-4);
    // ambient light: spherical harmonics of the sky (sun clipped, it lights directly)
    let mut sh = [[0.0f32; 3]; 9];
    let mut fog = [0.0f32; 3];
    let mut fog_n = 0.0f32;
    let step = (w / 128).max(1);
    let mut total_w = 0.0f32;
    for y in (0..h).step_by(step as usize) {
        for x in (0..w).step_by(step as usize) {
            let (u, v) = ((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
            // turned like the picture is drawn (so its bright side lights the sunny side)
            let dp = equirect_dir(u, v);
            let (ys, yc) = yaw.sin_cos();
            let d = v3(dp.x * yc + dp.z * ys, dp.y, -dp.x * ys + dp.z * yc);
            let k = (y * w + x) as usize;
            let mut c = [px[k * 3] * exposure, px[k * 3 + 1] * exposure, px[k * 3 + 2] * exposure];
            let l = lum(c[0], c[1], c[2]);
            if l > 6.0 {
                for ch in c.iter_mut() {
                    *ch *= 6.0 / l;
                }
            }
            if d.y < 0.0 {
                // below the horizon is the arena floor and walls, not the picture's ground
                let g = 0.35;
                c = [c[0] * g, c[1] * g, c[2] * g];
            }
            let wt = (v * std::f32::consts::PI).sin();
            let basis = [
                0.282095,
                0.488603 * d.y,
                0.488603 * d.z,
                0.488603 * d.x,
                1.092548 * d.x * d.y,
                1.092548 * d.y * d.z,
                0.315392 * (3.0 * d.z * d.z - 1.0),
                1.092548 * d.x * d.z,
                0.546274 * (d.x * d.x - d.z * d.z),
            ];
            for (i, b) in basis.iter().enumerate() {
                for ch in 0..3 {
                    sh[i][ch] += c[ch] * b * wt;
                }
            }
            total_w += wt;
            if d.y.abs() < 0.12 {
                for ch in 0..3 {
                    fog[ch] += c[ch];
                }
                fog_n += 1.0;
            }
        }
    }
    // normalise to the sphere, and fold in the cosine lobe (irradiance / pi)
    let norm = 4.0 * std::f32::consts::PI / total_w.max(1e-6);
    let band = [1.0, 2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 0.25, 0.25, 0.25, 0.25, 0.25];
    for i in 0..9 {
        for ch in 0..3 {
            sh[i][ch] *= norm * band[i];
        }
    }
    // the haze takes the horizon's colour, a little darker than the bright sky itself
    let fog = if fog_n > 0.0 { [fog[0] / fog_n * 0.75, fog[1] / fog_n * 0.75, fog[2] / fog_n * 0.75] } else { [0.55, 0.55, 0.6] };
    // the picture with its mips, as half floats (bright spots kept, but within reason)
    let mut lvl: Vec<f32> = Vec::with_capacity((w * h * 4) as usize);
    for k in 0..(w * h) as usize {
        for c in 0..3 {
            lvl.push((px[k * 3 + c] * exposure).min(64.0));
        }
        lvl.push(1.0);
    }
    let mut mips = Vec::new();
    let (mut cw, mut ch) = (w, h);
    loop {
        mips.push(lvl.iter().map(|&v| f16(v)).collect::<Vec<u16>>());
        if cw <= 1 && ch <= 1 {
            break;
        }
        let (nw, nh) = ((cw / 2).max(1), (ch / 2).max(1));
        let mut next = vec![0.0f32; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let mut s = 0.0;
                    let mut n = 0.0;
                    for (ox, oy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let (sx, sy) = ((x * 2 + ox).min(cw - 1), (y * 2 + oy).min(ch - 1));
                        s += lvl[((sy * cw + sx) * 4 + c) as usize];
                        n += 1.0;
                    }
                    next[((y * nw + x) * 4 + c) as usize] = s / n;
                }
            }
        }
        lvl = next;
        cw = nw;
        ch = nh;
    }
    Sky { w, h, mips, yaw, exposure, sun_dir, sun_col, sun_power: src.sun_power, sh, fog, fog_density: src.fog_density, grade: src.grade }
}

/// A map's sky (worked out once, the first time it's needed).
pub fn sky(map: MapId) -> &'static Sky {
    static SKIES: [OnceLock<Sky>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    SKIES[map as usize].get_or_init(|| build_sky(map))
}

// ---------------------------------------------------------------------------------- props

/// A prop's triangles in its own space: position, normal, texture coordinate, material layer,
/// and whether it's the banner cloth (which takes the team colour).
#[derive(Clone, Copy, Debug)]
pub struct PropVert {
    pub pos: V3,
    pub nrm: V3,
    pub uv: [f32; 2],
    pub layer: u32,
    pub cloth: bool,
    /// The model's own vertex colour (Quaternius uses it to tint some pieces).
    pub tint: [f32; 3],
}

struct ModelSrc {
    json: &'static str,
    bin: &'static [u8],
    /// Moved by this so it sits where the world wants it (banners: the bracket's middle).
    offset: V3,
}

macro_rules! model {
    ($n:literal, $off:expr) => {
        ModelSrc { json: include_str!(concat!("../assets/env/props/", $n, ".gltf")), bin: include_bytes!(concat!("../assets/env/props/", $n, ".bin")), offset: $off }
    };
}

fn model_src(kind: PropKind) -> ModelSrc {
    match kind {
        PropKind::BannerA => model!("Banner_1", v3(-0.8, 0.0, 0.0)),
        PropKind::BannerB => model!("Banner_2", v3(-0.8, 0.0, 0.0)),
        PropKind::Barrel => model!("Barrel", V3::ZERO),
        PropKind::WeaponStand => model!("WeaponStand", V3::ZERO),
        PropKind::Crate => model!("Crate_Wooden", V3::ZERO),
        PropKind::Rubble => model!("Vase_Rubble_Medium", V3::ZERO),
        PropKind::Shield => model!("Shield_Wooden", V3::ZERO),
        PropKind::Torch => model!("Torch_Metal", V3::ZERO),
        PropKind::RockA => model!("Rock_Medium_1", v3(0.0, 0.27, 0.0)),
        PropKind::RockB => model!("Rock_Medium_2", v3(0.0, 0.27, 0.0)),
        PropKind::Plant => model!("Plant_7", V3::ZERO),
    }
}

fn layer_for(material: &str) -> u32 {
    let m = material.to_ascii_lowercase();
    if m.contains("banner") || m.contains("cloth") {
        L_TRIM_CLOTH
    } else if m.contains("furniture") {
        L_TRIM_FURNITURE
    } else if m.contains("metal") {
        L_TRIM_METAL
    } else if m.contains("leaves") || m.contains("grass") {
        L_LEAVES
    } else if m.contains("rock") {
        L_ROCKS
    } else {
        L_TRIM_PROPS
    }
}

fn load_model(kind: PropKind) -> Vec<PropVert> {
    let src = model_src(kind);
    let Ok(g) = gltf::Gltf::from_slice(src.json.as_bytes()) else { return Vec::new() };
    let doc = g.document;
    let get = |_b: gltf::Buffer| Some(src.bin);
    let mut out = Vec::new();
    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            let r = prim.reader(get);
            let Some(pos) = r.read_positions() else { continue };
            let pos: Vec<[f32; 3]> = pos.collect();
            let nrm: Vec<[f32; 3]> = r.read_normals().map(|i| i.collect()).unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
            let uv: Vec<[f32; 2]> = r.read_tex_coords(0).map(|t| t.into_f32().collect()).unwrap_or_else(|| vec![[0.0, 0.0]; pos.len()]);
            let idx: Vec<u32> = r.read_indices().map(|i| i.into_u32().collect()).unwrap_or_else(|| (0..pos.len() as u32).collect());
            let mat = prim.material().name().unwrap_or("").to_string();
            let layer = layer_for(&mat);
            let cloth = mat.to_ascii_lowercase().contains("banner");
            // pieces whose material says so are tinted by their vertex colours
            let tinted = mat.to_ascii_lowercase().contains("vertex");
            let cols: Vec<[f32; 4]> = if tinted { r.read_colors(0).map(|c| c.into_rgba_f32().collect()).unwrap_or_default() } else { Vec::new() };
            for &i in &idx {
                let i = i as usize;
                if i >= pos.len() {
                    continue;
                }
                let p = pos[i];
                let n = nrm.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
                let c = cols.get(i).copied().unwrap_or([1.0, 1.0, 1.0, 1.0]);
                out.push(PropVert { pos: v3(p[0], p[1], p[2]) + src.offset, nrm: v3(n[0], n[1], n[2]), uv: uv.get(i).copied().unwrap_or([0.0, 0.0]), layer, cloth, tint: [c[0], c[1], c[2]] });
            }
        }
    }
    out
}

/// A prop's mesh (loaded once).
pub fn prop_mesh(kind: PropKind) -> &'static [PropVert] {
    const N: usize = 11;
    static MESHES: [OnceLock<Vec<PropVert>>; N] = [const { OnceLock::new() }; N];
    MESHES[kind as usize].get_or_init(|| load_model(kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_prop_loads_and_every_sky_lights_from_above() {
        for k in [PropKind::BannerA, PropKind::BannerB, PropKind::Barrel, PropKind::WeaponStand, PropKind::Crate, PropKind::Rubble, PropKind::Shield, PropKind::Torch, PropKind::RockA, PropKind::RockB, PropKind::Plant] {
            let m = prop_mesh(k);
            assert!(m.len() >= 3 && m.len() % 3 == 0, "{k:?} has triangles");
            assert!(m.iter().all(|v| v.pos.x.is_finite() && v.uv[0].is_finite()));
        }
        assert!(prop_mesh(PropKind::BannerA).iter().any(|v| v.cloth), "a banner has cloth to colour");
        for map in MapId::ALL {
            let s = sky(map);
            assert!(s.sun_dir.y > 0.35, "{map:?}: the sun is up");
            assert!(s.exposure.is_finite() && s.exposure > 0.0);
            assert!(s.sh[0].iter().all(|c| *c > 0.05 && c.is_finite()), "{map:?}: the sky lights the arena: {:?}", s.sh[0]);
            assert_eq!(s.mips[0].len(), (s.w * s.h * 4) as usize);
        }
        for l in 0..LAYERS {
            if let Some((c, s)) = layer(l) {
                assert_eq!(c.len(), (TEX * TEX * 4) as usize);
                assert_eq!(s.len(), c.len());
            }
        }
        assert_eq!(f16(1.0), 0x3c00);
        assert_eq!(f16(0.5), 0x3800);
        assert_eq!(f16(-2.0), 0xc000);
    }
}
