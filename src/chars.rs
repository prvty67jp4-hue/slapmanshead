//! The four fighters you can play as, built from CC0 parts by Quaternius ("Modular Character
//! Outfits - Fantasy" and "Universal Base Characters", both rigged to the same skeleton as the
//! animation library). Each fighter is an outfit (body, arms, legs, boots, hood...), a head with
//! face, eyes and eyebrows taken from a base character, and hair. Every part is moved onto the
//! outfit's skeleton when loaded, so one set of bone matrices drives the whole fighter; the
//! animations (made for the library's mannequin) are carried over bone by bone.
//!
//! Each fighter also wears a signature accessory built in Blender (tools/blender/build_accessories.py,
//! `assets/characters/acc_<name>.glb`): its pieces are skinned onto the skeleton here, piece by
//! piece (ACCESSORIES says which bones each follows and what sways).
//!
//! Every vertex's material word says how the shader treats it: bits 8+ the texture layer, bit 0
//! the team colour, bits 1-3 the kind of surface (KIND_*: the toon shader picks its highlights,
//! detail and outline from it) and bits 4-7 how much it sways (hair, scarf tails, an apron).

use crate::anim::{Rig, SkinVtx, Trs};
use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;

/// One glTF file with its binary buffer.
struct File {
    json: &'static str,
    bin: &'static [u8],
}

macro_rules! file {
    ($n:literal) => {
        File { json: include_str!(concat!("../assets/characters/", $n, ".gltf")), bin: include_bytes!(concat!("../assets/characters/", $n, ".bin")) }
    };
}

/// The fighters' texture set, one layer each (image bytes, all PNG).
pub const TEXTURES: [(&str, &[u8]); 17] = [
    ("ranger", include_bytes!("../assets/characters/ranger.png")),
    ("ranger_alt", include_bytes!("../assets/characters/ranger_alt.png")),
    ("peasant", include_bytes!("../assets/characters/peasant.png")),
    ("peasant_alt", include_bytes!("../assets/characters/peasant_alt.png")),
    ("skin_male", include_bytes!("../assets/characters/skin_male.png")),
    ("skin_female", include_bytes!("../assets/characters/skin_female.png")),
    ("hero_male", include_bytes!("../assets/characters/hero_male.png")),
    ("hero_female", include_bytes!("../assets/characters/hero_female.png")),
    ("hair1", include_bytes!("../assets/characters/hair1.png")),
    ("hair2", include_bytes!("../assets/characters/hair2.png")),
    ("eye", include_bytes!("../assets/characters/eye.png")),
    // the four accessories' baked textures, a quarter each
    ("accessories", include_bytes!("../assets/characters/accessories.png")),
    // Ledger Duels gear (tools/blender/build_gear.py): the four fighters' bakes, a quarter each
    ("gear", include_bytes!("../assets/characters/gear.png")),
    // the wardrobe's outfits (tools/blender/build_outfits.py), a quarter per fighter each
    ("look_herald", include_bytes!("../assets/characters/look_herald.png")),
    ("look_smuggler", include_bytes!("../assets/characters/look_smuggler.png")),
    ("look_pilgrim", include_bytes!("../assets/characters/look_pilgrim.png")),
    // the Ashen Vault's drops (tools/blender/build_vault_outfits.py): Warden, Magistrate, Regent
    ("look_vault", include_bytes!("../assets/characters/look_vault.png")),
];

/// Ledger Duels gear with a model, fitted to each fighter (tools/blender/build_gear.py), in the
/// fighters' order; and which item each piece (by mesh name) is.
macro_rules! looks {
    ($n:literal) => {
        [
            (include_bytes!(concat!("../assets/characters/gear_bridgewright_", $n, ".glb")) as &[u8], "gear"),
            (include_bytes!(concat!("../assets/characters/look_herald_", $n, ".glb")) as &[u8], "look_herald"),
            (include_bytes!(concat!("../assets/characters/look_smuggler_", $n, ".glb")) as &[u8], "look_smuggler"),
            (include_bytes!(concat!("../assets/characters/look_pilgrim_", $n, ".glb")) as &[u8], "look_pilgrim"),
            (include_bytes!(concat!("../assets/characters/look_vault_", $n, ".glb")) as &[u8], "look_vault"),
        ]
    };
}
/// Each fighter's fitted gear and outfits: (file, its texture layer).
const GEAR_FILES: [[(&[u8], &str); 5]; 4] = [looks!("ranger"), looks!("huntress"), looks!("brawler"), looks!("striker")];
/// Every gear piece with a model (by mesh name) and its id (crate::hub::LOOKS says its slot).
pub const GEAR_PIECES: [(&str, u8); 39] = [
    ("bw_head", 51),
    ("bw_body", 52),
    ("bw_legs", 53),
    ("bw_hands", 54),
    ("bw_feet", 55),
    ("bw_charm", 56),
    ("hr_head", 100),
    ("hr_cape", 101),
    ("hr_neck", 102),
    ("hr_body", 103),
    ("hr_legs", 104),
    ("hr_hands", 105),
    ("hr_feet", 106),
    ("sm_head", 110),
    ("sm_cape", 111),
    ("sm_neck", 112),
    ("sm_body", 113),
    ("sm_legs", 114),
    ("sm_hands", 115),
    ("sm_feet", 116),
    ("ap_head", 120),
    ("ap_cape", 121),
    ("ap_neck", 122),
    ("ap_body", 123),
    ("ap_legs", 124),
    ("ap_hands", 125),
    ("ap_feet", 126),
    ("wv_head", 130),
    ("wv_cape", 131),
    ("wv_neck", 132),
    ("wv_body", 133),
    ("wv_legs", 134),
    ("wv_hands", 135),
    ("wv_feet", 136),
    ("mg_head", 140),
    ("mg_cape", 141),
    ("mg_neck", 142),
    ("mg_body", 143),
    ("rg_body", 150),
];

/// The wardrobe slot a gear piece is worn in, plus one (0: none), for its dyes.
pub fn piece_slot(id: u8) -> u32 {
    crate::hub::LOOKS.iter().find(|l| l.piece == id && id != 0).map_or(0, |l| l.slot as u32 + 1)
}

/// The part of the fighter a worn piece hides outright (whatever the outfit under it, so a long
/// coat's ragged hem or a shirt never shows through): every vertex of theirs that mostly moves
/// with those bones, within those heights. The piece must cover all of it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Covers {
    /// Nothing outright (a helm, a hat): only what lies just under its surface.
    Surface,
    /// Nothing at all (a cape, a chain, a pendant: they swing clear of the body).
    Nothing,
    /// The palms and the forearms this far back from the wrist, not the fingers (fingerless
    /// gloves).
    Palms(f32),
    /// Hips to ankles: (top, bottom) heights.
    Legs(f32, f32),
    /// The feet up to this height.
    Feet(f32),
    /// The torso between these heights and the upper arms this far from the shoulder joint.
    Torso(f32, f32, f32),
    /// The hands and the forearms this far back from the wrist.
    Hands(f32),
}

/// What each gear piece (by mesh name) hides of the fighter.
pub fn piece_covers(name: &str) -> Covers {
    match name {
        // (the smuggler's gloves leave the fingers out; his boots come up to the knee)
        "sm_hands" => Covers::Palms(0.14),
        "sm_feet" => Covers::Feet(0.5),
        _ if name.ends_with("_legs") => Covers::Legs(0.92, 0.24),
        _ if name.ends_with("_feet") => Covers::Feet(0.26),
        _ if name.ends_with("_body") => Covers::Torso(1.46, 0.97, 0.17),
        _ if name.ends_with("_hands") => Covers::Hands(0.14),
        // capes, chains and pendants hang clear: they hide nothing
        _ if name.ends_with("_cape") || name.ends_with("_neck") || name.ends_with("_charm") => Covers::Nothing,
        _ => Covers::Surface,
    }
}

/// Does a region hide a vertex at `p` that mostly moves with joint `jn` (`head_of` gives a
/// joint's rest position)?
fn region_hides(c: Covers, jn: &str, p: Vec3, head_of: &dyn Fn(&str) -> Vec3) -> bool {
    let is = |pre: &[&str]| pre.iter().any(|x| jn.starts_with(x));
    let side = if jn.ends_with("_l") { "l" } else { "r" };
    match c {
        Covers::Surface | Covers::Nothing => false,
        Covers::Palms(cuff) => {
            is(&["hand"])
                || (is(&["lowerarm"]) && {
                    let (a, b) = (head_of(&format!("hand_{side}")), head_of(&format!("lowerarm_{side}")));
                    (p - a).dot((b - a).normalize_or_zero()) < cuff
                })
        }
        // (anything hanging from the hips or thighs, down to the ground: a long hem)
        // (and whatever hangs from the waist or back below it: a coat's tails never show under
        // breeches and a doublet)
        Covers::Legs(top, bot) => p.y < top && (is(&["thigh", "pelvis", "spine", "root"]) || (is(&["calf"]) && p.y > bot)),
        Covers::Feet(top) => is(&["foot", "ball", "calf"]) && p.y < top,
        Covers::Torso(top, bot, sleeve) => {
            (is(&["spine", "clavicle", "pelvis"]) && p.y < top && p.y > bot)
                || (is(&["upperarm"]) && {
                    let (a, b) = (head_of(&format!("upperarm_{side}")), head_of(&format!("lowerarm_{side}")));
                    (p - a).dot((b - a).normalize_or_zero()) < sleeve
                })
        }
        Covers::Hands(cuff) => {
            is(&["hand", "index", "middle", "ring", "pinky", "thumb"])
                || (is(&["lowerarm"]) && {
                    let (a, b) = (head_of(&format!("hand_{side}")), head_of(&format!("lowerarm_{side}")));
                    (p - a).dot((b - a).normalize_or_zero()) < cuff
                })
        }
    }
}

/// A gear material's dye zone (bits 16-17 of the material word): 1 the coat, 2 its trim, 3 the
/// accent; 0 keeps its own colour.
pub fn dye_zone(mat: &str) -> u32 {
    // (`dye_coat`, or with its surface: `dye_coat_leather`, `dye_trim_metal`, `dye_accent_fur`...)
    let m = mat.split("__").next().unwrap_or(mat);
    if m == "dye_coat" || m.starts_with("dye_coat_") {
        1
    } else if m == "dye_trim" || m.starts_with("dye_trim_") {
        2
    } else if m == "dye_accent" || m.starts_with("dye_accent_") || m == "glow_accent" {
        3
    } else {
        0
    }
}

/// Bit 22 of a gear vertex's material word: plain cloth (the shader never takes a pale dyed
/// cloth for metal by its colour).
pub const CLOTH_BIT: u32 = 1 << 22;

/// Kinds of surface (bits 1-3 of the material word): picked from the material, or (AUTO) from
/// the texture's colour in the shader (metal, leather or cloth).
pub const KIND_AUTO: u32 = 0;
pub const KIND_BROWS: u32 = 1;
pub const KIND_EYES: u32 = 2;
pub const KIND_HAIR: u32 = 3;
/// Drawn unlit in the player's colour (the hood's trim).
pub const KIND_GLOW: u32 = 4;
pub const KIND_SKIN: u32 = 5;
pub const KIND_METAL: u32 = 6;
pub const KIND_LEATHER: u32 = 7;

/// A vertex's material word.
pub fn mat_word(layer: u32, accent: bool, kind: u32, sway: f32) -> u32 {
    let s = (sway.clamp(0.0, 1.0) * 15.0).round() as u32;
    (layer << 8) | accent as u32 | (kind & 7) << 1 | s << 4
}

/// How an accessory piece follows the skeleton.
#[derive(Clone, Copy)]
enum Follow {
    One(&'static str),
    /// From bone a (at height ya and above) to bone b (at yb and below), blended in between.
    Blend(&'static str, &'static str, f32, f32),
    /// Snug on the body: each vertex follows the bones of the nearest vertex of the fighter's
    /// own skin (the pauldron bends with the shoulder under it).
    Skin,
    /// The apron: the hips at the top, the thighs (each its own side) towards the hem.
    Skirt,
}

struct Piece {
    /// Mesh name (a prefix is enough).
    name: &'static str,
    follow: Follow,
    accent: bool,
    /// Sways below this height (y), fully at this much lower: (top, length).
    sway: Option<(f32, f32)>,
}

struct Accessory {
    glb: &'static [u8],
    pieces: &'static [Piece],
}

const ACCESSORIES: [Accessory; 4] = [
    // Corvin Vale: a brass warden badge on the coat
    Accessory { glb: include_bytes!("../assets/characters/acc_ranger.glb"), pieces: &[Piece { name: "badge", follow: Follow::One("spine_03"), accent: false, sway: None }] },
    // Sable Rook: a long scarf in her colour, its tails swaying down the chest
    Accessory {
        glb: include_bytes!("../assets/characters/acc_huntress.glb"),
        pieces: &[
            Piece { name: "scarf_tail", follow: Follow::One("spine_03"), accent: true, sway: Some((1.41, 0.42)) },
            Piece { name: "scarf", follow: Follow::Blend("neck_01", "spine_03", 1.46, 1.36), accent: true, sway: None },
        ],
    },
    // Bram Kask: a heavy pauldron and strap, a smith's apron on a belt
    Accessory {
        glb: include_bytes!("../assets/characters/acc_brawler.glb"),
        pieces: &[
            Piece { name: "pauldron", follow: Follow::Skin, accent: false, sway: None },
            Piece { name: "strap", follow: Follow::Blend("spine_03", "spine_01", 1.38, 1.08), accent: false, sway: None },
            Piece { name: "apron", follow: Follow::Skirt, accent: false, sway: Some((1.0, 0.45)) },
            Piece { name: "belt", follow: Follow::One("pelvis"), accent: false, sway: None },
        ],
    },
    // Ilya Fen: a hood, its trim glowing in her colour
    Accessory {
        glb: include_bytes!("../assets/characters/acc_striker.glb"),
        pieces: &[
            Piece { name: "hood_trim", follow: Follow::Blend("Head", "spine_03", 1.5, 1.4), accent: true, sway: None },
            Piece { name: "hood", follow: Follow::Blend("Head", "spine_03", 1.5, 1.4), accent: false, sway: None },
        ],
    },
];

fn layer(name: &str) -> u32 {
    TEXTURES.iter().position(|t| t.0 == name).unwrap_or(0) as u32
}

/// Which texture a material uses (a fighter can swap one for a recoloured version).
fn material_layer(mat: &str, swaps: &[(&str, &str)]) -> u32 {
    if let Some((_, l)) = swaps.iter().find(|(m, _)| *m == mat) {
        return layer(l);
    }
    layer(match mat {
        "MI_Ranger" => "ranger",
        "MI_Peasant" => "peasant",
        "MI_Regular_Male" => "skin_male",
        "MI_Regular_Female" => "skin_female",
        "MI_Superhero_Male" => "hero_male",
        "MI_Superhero_Female" => "hero_female",
        "MI_Hair_1" => "hair1",
        "MI_Hair_2" => "hair2",
        "MI_Eyes" => "eye",
        _ => "skin_male",
    })
}

/// The kind of surface a part's material is (its mesh name decides the eyebrows).
fn kind_of(mesh: &str, mat: &str) -> u32 {
    if mat == "MI_Eyes" {
        KIND_EYES
    } else if mesh == "Eyebrows" || (mesh == "Face" && mat == "MI_Hair_1") {
        KIND_BROWS
    } else if mat.starts_with("MI_Hair") {
        KIND_HAIR
    } else if mat.starts_with("MI_Regular") || mat.starts_with("MI_Superhero") {
        KIND_SKIN
    } else {
        KIND_AUTO
    }
}

/// An accessory material's kind (the palette names from tools/blender).
fn acc_kind(mat: &str) -> u32 {
    let m = mat.split("__").next().unwrap_or(mat);
    // a dye zone with its surface named: `dye_<zone>_<cloth|leather|metal|fur>`
    if m.starts_with("dye_") && m.matches('_').count() >= 2 {
        return match m.rsplit('_').next().unwrap_or("") {
            "metal" => KIND_METAL,
            "leather" => KIND_LEATHER,
            "fur" => KIND_HAIR,
            _ => KIND_AUTO,
        };
    }
    if m.starts_with("leather") {
        return KIND_LEATHER;
    }
    if m.starts_with("fur") {
        return KIND_HAIR;
    }
    match m {
        "glow" | "glow_accent" => KIND_GLOW,
        "dye_trim" => KIND_METAL,
        "dye_coat" | "rope" => KIND_LEATHER,
        "brass" | "iron" | "metal_dark" | "metal_mid" | "steel" | "copper" => KIND_METAL,
        "leather" | "apron" | "tan" => KIND_LEATHER,
        _ => KIND_AUTO,
    }
}

/// How much a vertex of a fighter's own mesh sways: long hair from the shoulders down, the
/// pouches hanging off a belt, the skirt of a coat or tunic below the waist.
fn part_sway(mesh: &str, y: f32) -> f32 {
    if mesh.starts_with("Hair_Long") {
        ((1.6 - y) / 0.45).clamp(0.0, 1.0)
    } else if mesh.contains("Belt") {
        ((1.08 - y) / 0.2).clamp(0.0, 0.5)
    } else if mesh.ends_with("_Body") {
        ((1.02 - y) / 0.15).clamp(0.0, 0.45)
    } else {
        0.0
    }
}

/// Faces that read at a distance: painted over the base characters' face textures when they
/// load (positions in the 1024 x 1024 texture): a dark lash line over each eye and a fainter one
/// under it, shaded eye sockets, the sides of the nose and the cheekbones shaded, a little colour
/// on the cheeks and a defined lip line.
pub fn paint_face(img: &mut image::RgbaImage, female: bool) {
    let s = img.width() as f32 / 1024.0;
    let (eyes, nose, mouth) = if female { ([133.0f32, 240.0], (188.0f32, 230.0f32), (188.0f32, 268.0f32)) } else { ([140.0, 240.0], (190.0, 236.0), (190.0, 270.0)) };
    let ey = 183.0;
    // multiply (towards a colour) inside a soft ellipse
    let mut dab = |cx: f32, cy: f32, rx: f32, ry: f32, col: [f32; 3], k: f32| {
        let (cx, cy, rx, ry) = (cx * s, cy * s, rx * s, ry * s);
        let x0 = (cx - rx).floor().max(0.0) as u32;
        let x1 = ((cx + rx).ceil() as u32).min(img.width() - 1);
        let y0 = (cy - ry).floor().max(0.0) as u32;
        let y1 = ((cy + ry).ceil() as u32).min(img.height() - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = (x as f32 + 0.5 - cx) / rx;
                let dy = (y as f32 + 0.5 - cy) / ry;
                let d = dx * dx + dy * dy;
                if d >= 1.0 {
                    continue;
                }
                let a = (1.0 - d) * (1.0 - d) * k;
                let p = img.get_pixel_mut(x, y);
                for c in 0..3 {
                    let v = p[c] as f32;
                    p[c] = (v + (v * col[c] - v) * a).clamp(0.0, 255.0) as u8;
                }
            }
        }
    };
    let lash = [0.32, 0.22, 0.2];
    for (side, &ex) in eyes.iter().enumerate() {
        let out = if side == 0 { -1.0 } else { 1.0 };
        // the socket, a touch darker
        dab(ex, ey - 12.0, 30.0, 16.0, [0.82, 0.74, 0.72], 0.8);
        // the upper lash line: an arc of dabs, heavier towards the outer corner
        for k in 0..=16 {
            let a = std::f32::consts::PI * (0.08 + 0.84 * k as f32 / 16.0);
            let x = ex + 25.0 * a.cos() * -out;
            let y = ey - 13.0 * a.sin() + 1.0;
            let outer = ((x - ex) * out / 25.0).clamp(0.0, 1.0);
            dab(x, y, 4.5 + 2.0 * outer, 3.5 + 1.2 * outer, lash, 0.95);
        }
        // a wing at the outer corner, and the fainter lower lash line
        dab(ex + 27.0 * out, ey - 2.0, 7.0, 3.0, lash, 0.8);
        for k in 0..=10 {
            let a = std::f32::consts::PI * (0.15 + 0.7 * k as f32 / 10.0);
            dab(ex + 22.0 * a.cos(), ey + 10.0 * a.sin() + 1.0, 3.0, 2.2, [0.55, 0.42, 0.4], 0.55);
        }
        // the cheekbone's shadow below and outside, and a little colour on the cheek
        dab(ex + 22.0 * out, ey + 66.0, 22.0, 34.0, [0.86, 0.8, 0.8], 0.7);
        dab(ex + 4.0 * out, ey + 45.0, 28.0, 18.0, [1.0, 0.84, 0.8], 0.35);
    }
    // the sides of the nose and the nostrils
    for out in [-1.0f32, 1.0] {
        dab(nose.0 + 13.0 * out, nose.1 - 24.0, 7.0, 30.0, [0.8, 0.72, 0.7], 0.75);
        dab(nose.0 + 9.0 * out, nose.1 + 2.0, 5.0, 3.5, [0.55, 0.4, 0.38], 0.7);
    }
    // the lip line, and a soft shadow under the lower lip
    dab(mouth.0, mouth.1, 26.0, 3.5, [0.6, 0.42, 0.4], 0.8);
    dab(mouth.0, mouth.1 + 16.0, 18.0, 6.0, [0.86, 0.78, 0.76], 0.6);
}

/// A piece of a fighter.
struct Part {
    file: fn() -> File,
    /// Only the head and neck (taken from a full-body base character).
    head_only: bool,
}

struct Spec {
    name: &'static str,
    blurb: &'static str,
    /// The outfit whose skeleton the fighter uses.
    skeleton: fn() -> File,
    parts: &'static [Part],
    /// Meshes (by name, a prefix is enough) shown in the team colour.
    accent: &'static [&'static str],
    swaps: &'static [(&'static str, &'static str)],
}

fn male_ranger() -> File {
    file!("Male_Ranger")
}
fn female_ranger() -> File {
    file!("Female_Ranger")
}
fn male_peasant() -> File {
    file!("Male_Peasant")
}
fn female_peasant() -> File {
    file!("Female_Peasant")
}
fn hero_male() -> File {
    file!("Superhero_Male_FullBody")
}
fn hero_female() -> File {
    file!("Superhero_Female_FullBody")
}
fn hair_beard() -> File {
    file!("Hair_Beard")
}
fn hair_buzzed() -> File {
    file!("Hair_Buzzed")
}

const SPECS: [Spec; 4] = [
    Spec {
        name: "Corvin Vale",
        blurb: "Ranger: hooded scout in leather and bracers, a warden's badge on his coat",
        skeleton: male_ranger,
        parts: &[Part { file: male_ranger, head_only: false }, Part { file: hero_male, head_only: true }, Part { file: hair_beard, head_only: false }],
        accent: &["Male_Ranger_Head_Hood", "Male_Ranger_Acc_Pauldron"],
        swaps: &[],
    },
    Spec {
        name: "Sable Rook",
        blurb: "Huntress: light hooded hunter with pauldrons and a long scarf",
        skeleton: female_ranger,
        parts: &[Part { file: female_ranger, head_only: false }, Part { file: hero_female, head_only: true }],
        // only the pauldrons take the team colour: her hood keeps its own brown, so from behind she
        // doesn't look like the Ranger
        accent: &["Female_Ranger_Acc_Pauldrons"],
        swaps: &[("MI_Ranger", "ranger_alt")],
    },
    Spec {
        name: "Bram \"Anvil\" Kask",
        blurb: "Brawler: bearded smith in a leather apron, an iron pauldron on his shoulder",
        skeleton: male_peasant,
        parts: &[Part { file: male_peasant, head_only: false }, Part { file: hero_male, head_only: true }, Part { file: hair_buzzed, head_only: false }, Part { file: hair_beard, head_only: false }],
        accent: &["Male_Peasant_Body"],
        swaps: &[],
    },
    Spec {
        name: "Ilya Fen",
        blurb: "Striker: quick duelist in a fitted tunic and a hood with a glowing trim",
        skeleton: female_peasant,
        // (her hair is under the hood)
        parts: &[Part { file: female_peasant, head_only: false }, Part { file: hero_female, head_only: true }],
        accent: &["Female_Peasant_Body"],
        swaps: &[("MI_Peasant", "peasant_alt")],
    },
];

pub const COUNT: usize = SPECS.len();

/// The fighter's name, as assets/lore.txt gives it (the built-in one if the file doesn't).
pub fn name(i: u8) -> &'static str {
    crate::lore::fighter_name(i, SPECS[(i as usize).min(COUNT - 1)].name)
}
pub fn blurb(i: u8) -> &'static str {
    SPECS[(i as usize).min(COUNT - 1)].blurb
}

/// A fighter ready to draw and animate.
pub struct Model {
    pub verts: Vec<SkinVtx>,
    pub indices: Vec<u32>,
    /// Its skeleton's inverse bind matrices, in the rig's joint order.
    pub inv_bind: Vec<Mat4>,
    /// Its skeleton's rest transforms, per rig node (None where it has no such bone).
    rest: Vec<Option<Trs>>,
    /// Scale that brings it to the rig's height (everyone is the same size: fair hit shapes).
    pub norm: f32,
    /// Indices of the fighter alone (gear pieces follow them).
    pub base_count: u32,
    /// Ledger Duels gear pieces fitted to this fighter: item id, first index, index count (each
    /// drawn only while worn).
    pub gear: Vec<(u8, u32, u32)>,
    /// Where each vertex goes while a gear piece covers it: a point on its bone (inside the
    /// body), in its rest space. Gear vertices keep their own place.
    pub cores: Vec<[f32; 3]>,
    /// Per gear piece (by its place in GEAR_PIECES): which vertices of the fighter it covers, a
    /// bit each (`cover_words` words per piece; empty if this fighter has none of that piece).
    pub cover: Vec<Vec<u32>>,
    pub cover_words: usize,
}

/// All four fighters, loaded once (None if the files couldn't be read).
pub fn shared() -> Option<&'static Vec<Model>> {
    static MODELS: std::sync::OnceLock<Option<Vec<Model>>> = std::sync::OnceLock::new();
    MODELS
        .get_or_init(|| {
            let rig = crate::anim::shared()?;
            let mut out = Vec::new();
            for (k, s) in SPECS.iter().enumerate() {
                match build(rig, k, true) {
                    Ok(m) => out.push(m),
                    Err(e) => {
                        eprintln!("fighter {} not loaded: {e}", s.name);
                        return None;
                    }
                }
            }
            Some(out)
        })
        .as_ref()
}

/// One fighter's model, if they're loaded.
pub fn model(i: u8) -> Option<&'static Model> {
    shared().and_then(|m| m.get((i as usize).min(COUNT - 1)))
}

struct Skeleton {
    /// Inverse bind matrix of each bone, by name.
    inv_bind: HashMap<String, Mat4>,
    /// Rest transform of each node, by name.
    rest: HashMap<String, Trs>,
}

fn parse(f: &File) -> Result<gltf::Document, String> {
    let g = gltf::Gltf::from_slice(f.json.as_bytes()).map_err(|e| e.to_string())?;
    Ok(g.document)
}

fn skeleton(f: &File) -> Result<Skeleton, String> {
    let doc = parse(f)?;
    let get = |_b: gltf::Buffer| Some(f.bin);
    let skin = doc.skins().next().ok_or("no skin")?;
    let names: Vec<String> = skin.joints().map(|j| j.name().unwrap_or("").to_string()).collect();
    let ib: Vec<Mat4> = skin.reader(get).read_inverse_bind_matrices().map(|it| it.map(|m| Mat4::from_cols_array_2d(&m)).collect()).unwrap_or_default();
    let mut inv_bind = HashMap::new();
    for (k, n) in names.iter().enumerate() {
        inv_bind.insert(n.clone(), ib.get(k).copied().unwrap_or(Mat4::IDENTITY));
    }
    let mut rest = HashMap::new();
    for node in doc.nodes() {
        if let Some(n) = node.name() {
            let (t, r, s) = node.transform().decomposed();
            rest.insert(n.to_string(), Trs { t: Vec3::from(t), r: Quat::from_array(r).normalize(), s: Vec3::from(s) });
        }
    }
    Ok(Skeleton { inv_bind, rest })
}

fn build(rig: &Rig, which: usize, accessory: bool) -> Result<Model, String> {
    let spec = &SPECS[which];
    let sk = skeleton(&(spec.skeleton)())?;
    let rig_joints = rig.joint_names();
    let joint_index: HashMap<&str, usize> = rig_joints.iter().enumerate().map(|(k, n)| (n.as_str(), k)).collect();
    let mut verts = Vec::new();
    let mut indices = Vec::new();
    for part in spec.parts {
        let f = (part.file)();
        let doc = parse(&f)?;
        let get = |_b: gltf::Buffer| Some(f.bin);
        let Some(skin) = doc.skins().next() else { continue };
        let pnames: Vec<String> = skin.joints().map(|j| j.name().unwrap_or("").to_string()).collect();
        let pib: Vec<Mat4> = skin.reader(get).read_inverse_bind_matrices().map(|it| it.map(|m| Mat4::from_cols_array_2d(&m)).collect()).unwrap_or_default();
        // per joint of this part: its bind space -> the fighter's bind space, and the rig index
        let conv: Vec<Mat4> = pnames
            .iter()
            .enumerate()
            .map(|(j, n)| match sk.inv_bind.get(n) {
                Some(ibc) => ibc.inverse() * pib.get(j).copied().unwrap_or(Mat4::IDENTITY),
                None => Mat4::IDENTITY,
            })
            .collect();
        let remap: Vec<u32> = pnames.iter().map(|n| joint_index.get(n.as_str()).copied().unwrap_or(0) as u32).collect();
        let head_bones: Vec<bool> = pnames.iter().map(|n| matches!(n.as_str(), "Head" | "neck_01")).collect();
        for mesh in doc.meshes() {
            let mname = mesh.name().unwrap_or("");
            let accent = spec.accent.iter().any(|a| mname.starts_with(a));
            for prim in mesh.primitives() {
                let r = prim.reader(get);
                let Some(pos) = r.read_positions() else { continue };
                let pos: Vec<[f32; 3]> = pos.collect();
                let nrm: Vec<[f32; 3]> = r.read_normals().map(|i| i.collect()).unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
                let uv: Vec<[f32; 2]> = r.read_tex_coords(0).map(|t| t.into_f32().collect()).unwrap_or_else(|| vec![[0.0, 0.0]; pos.len()]);
                let jnt: Vec<[u16; 4]> = r.read_joints(0).map(|j| j.into_u16().collect()).unwrap_or_else(|| vec![[0; 4]; pos.len()]);
                let wts: Vec<[f32; 4]> = r.read_weights(0).map(|w| w.into_f32().collect()).unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 0.0]; pos.len()]);
                let mat_name = prim.material().name().unwrap_or("");
                let lay = material_layer(mat_name, spec.swaps);
                let tri: Vec<u32> = match r.read_indices() {
                    Some(ix) => ix.into_u32().collect(),
                    None => (0..pos.len() as u32).collect(),
                };
                // a head taken off a full body: keep the triangles that move with the head or neck
                let keep_vert: Vec<bool> = (0..pos.len())
                    .map(|k| {
                        if !part.head_only || mname != hero_body_mesh(&doc) {
                            return true;
                        }
                        let (j, w) = (jnt[k], wts[k]);
                        let mut best = 0;
                        for q in 1..4 {
                            if w[q] > w[best] {
                                best = q;
                            }
                        }
                        head_bones.get(j[best] as usize).copied().unwrap_or(false)
                    })
                    .collect();
                let base = verts.len() as u32;
                let mut used = vec![u32::MAX; pos.len()];
                let mut next = base;
                for t in tri.chunks(3) {
                    if t.len() < 3 || !t.iter().all(|&k| keep_vert.get(k as usize).copied().unwrap_or(false)) {
                        continue;
                    }
                    for &k in t {
                        let k = k as usize;
                        if used[k] == u32::MAX {
                            used[k] = next;
                            next += 1;
                            // move into the fighter's bind space, blending each bone's offset
                            let (j, w) = (jnt[k], wts[k]);
                            let p = Vec3::from(pos[k]);
                            let n = Vec3::from(nrm[k]);
                            let mut pp = Vec3::ZERO;
                            let mut nn = Vec3::ZERO;
                            let mut ws = 0.0;
                            for q in 0..4 {
                                if w[q] <= 0.0 {
                                    continue;
                                }
                                let m = conv.get(j[q] as usize).copied().unwrap_or(Mat4::IDENTITY);
                                pp += m.transform_point3(p) * w[q];
                                nn += m.transform_vector3(n) * w[q];
                                ws += w[q];
                            }
                            let (pp, nn) = if ws > 0.0 { (pp / ws, nn.normalize_or(n)) } else { (p, n) };
                            let jr = [0, 1, 2, 3].map(|q| remap.get(j[q] as usize).copied().unwrap_or(0));
                            let mat = mat_word(lay, accent, kind_of(mname, mat_name), part_sway(mname, pp.y));
                            verts.push(SkinVtx { pos: pp.to_array(), nrm: nn.to_array(), joints: jr, weights: w, mat, uv: uv[k], onrm: nn.to_array() });
                        }
                        indices.push(used[k]);
                    }
                }
                let _ = base;
            }
        }
    }
    if accessory {
        add_accessory(&ACCESSORIES[which], &sk, &joint_index, &mut verts, &mut indices)?;
    }
    if indices.is_empty() {
        return Err("no triangles".into());
    }
    let base_count = indices.len() as u32;
    let body_n = verts.len();
    let mut gear = Vec::new();
    if accessory {
        for (file, tex) in GEAR_FILES[which] {
            match add_gear(file, tex, &sk, &joint_index, body_n, &mut verts, &mut indices) {
                Ok(g) => gear.extend(g),
                Err(e) => eprintln!("{tex} for {} not loaded: {e}", spec.name),
            }
        }
    }
    outline_normals(&mut verts);
    let (cores, cover, cover_words) = cover_maps(rig, &sk, &verts, &indices, body_n, &gear);
    // its skeleton in the rig's terms
    let inv_bind: Vec<Mat4> = rig_joints.iter().enumerate().map(|(k, n)| sk.inv_bind.get(n).copied().unwrap_or_else(|| rig.palette_inv_bind(k))).collect();
    let n_nodes = rig.node_names.len();
    let rest: Vec<Option<Trs>> = (0..n_nodes).map(|i| sk.rest.get(&rig.node_names[i]).copied().filter(|_| !rig.node_names[i].is_empty())).collect();
    let mut m = Model { verts, indices, inv_bind, rest, norm: 1.0, base_count, gear, cores, cover, cover_words };
    // same height as the rig: compare where the head bone sits at rest
    if let Some(head) = rig.node("Head") {
        let rig_g = rig.globals(&rig.rest_pose());
        let mine = m.retarget(rig, &rig.rest_pose());
        let my_g = rig.globals(&mine);
        let (a, b) = (rig_g[head].w_axis.truncate().length(), my_g[head].w_axis.truncate().length());
        if b > 0.1 {
            m.norm = (a / b).clamp(0.8, 1.25);
        }
    }
    Ok(m)
}

/// What each worn gear piece hides of the fighter under it (so a shirt or a ragged hem never
/// pokes through a coat or trousers): a vertex of the fighter is covered when the piece's
/// surface lies just outside it, running the same way as the skin (looking out along its normal
/// within 13 cm, or in within 4 cm for loose cloth already outside), and while covered it is
/// drawn folded onto its bone, inside the body. Also each vertex's point on its bone.
fn cover_maps(rig: &Rig, sk: &Skeleton, verts: &[SkinVtx], indices: &[u32], body_n: usize, gear: &[(u8, u32, u32)]) -> (Vec<[f32; 3]>, Vec<Vec<u32>>, usize) {
    // the bones in rest space: each joint's head, and the segments to its children and parent
    let names = rig.joint_names();
    let head: Vec<Vec3> = (0..names.len())
        .map(|k| sk.inv_bind.get(&names[k]).copied().unwrap_or_else(|| rig.palette_inv_bind(k)).inverse().w_axis.truncate())
        .collect();
    let parent: Vec<Option<usize>> = (0..names.len()).map(|k| rig.joint_parent(k)).collect();
    let mut segs: Vec<Vec<(Vec3, Vec3)>> = vec![Vec::new(); names.len()];
    for k in 0..names.len() {
        if let Some(p) = parent[k] {
            let s = (head[p], head[k]);
            if (s.1 - s.0).length() > 0.01 {
                segs[p].push(s);
                segs[k].push(s);
            }
        }
    }
    let on_seg = |p: Vec3, a: Vec3, b: Vec3| {
        let ab = b - a;
        a + ab * ((p - a).dot(ab) / ab.length_squared().max(1e-8)).clamp(0.0, 1.0)
    };
    let mut cores: Vec<[f32; 3]> = verts.iter().map(|v| v.pos).collect();
    for (k, v) in verts[..body_n].iter().enumerate() {
        let mut best = 0;
        for q in 1..4 {
            if v.weights[q] > v.weights[best] {
                best = q;
            }
        }
        let j = (v.joints[best] as usize).min(names.len().saturating_sub(1));
        let p = Vec3::from(v.pos);
        let mut c = head.get(j).copied().unwrap_or(p);
        let mut d = (c - p).length_squared();
        for &(a, b) in &segs[j] {
            let q = on_seg(p, a, b);
            if (q - p).length_squared() < d {
                d = (q - p).length_squared();
                c = q;
            }
        }
        cores[k] = c.to_array();
    }
    let words = verts.len().div_ceil(32);
    let mut cover = vec![Vec::new(); GEAR_PIECES.len()];
    let cell = 0.05f32;
    let key = |p: Vec3| ((p.x / cell).floor() as i32, (p.y / cell).floor() as i32, (p.z / cell).floor() as i32);
    for &(id, s, c) in gear {
        let Some(pi) = GEAR_PIECES.iter().position(|g| g.1 == id) else { continue };
        // (capes, chains and pendants hang clear of the body: nothing to hide)
        if piece_covers(GEAR_PIECES[pi].0) == Covers::Nothing {
            continue;
        }
        let tris: Vec<[Vec3; 3]> = indices[s as usize..(s + c) as usize].chunks(3).map(|t| [0, 1, 2].map(|q| Vec3::from(verts[t[q] as usize].pos))).collect();
        let mut grid: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::new();
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for (ti, t) in tris.iter().enumerate() {
            let a = t[0].min(t[1]).min(t[2]);
            let b = t[0].max(t[1]).max(t[2]);
            lo = lo.min(a);
            hi = hi.max(b);
            let (ka, kb) = (key(a), key(b));
            for x in ka.0..=kb.0 {
                for y in ka.1..=kb.1 {
                    for z in ka.2..=kb.2 {
                        grid.entry((x, y, z)).or_default().push(ti as u32);
                    }
                }
            }
        }
        let (lo, hi) = (lo - Vec3::splat(0.15), hi + Vec3::splat(0.15));
        // a ray from p along d hits the piece within `reach`, on a surface running like the skin
        let hits = |p: Vec3, d: Vec3, reach: f32| -> bool {
            let mut seen: Vec<(i32, i32, i32)> = Vec::new();
            let steps = (reach / (cell * 0.5)).ceil() as i32;
            for st in 0..=steps {
                let k = key(p + d * (st as f32 * cell * 0.5).min(reach));
                if seen.contains(&k) {
                    continue;
                }
                seen.push(k);
                for &ti in grid.get(&k).map(|v| v.as_slice()).unwrap_or(&[]) {
                    let [a, b, c] = tris[ti as usize];
                    let (e1, e2) = (b - a, c - a);
                    let n = e1.cross(e2);
                    let nl = n.length();
                    if nl < 1e-9 || (n.dot(d) / nl).abs() < 0.35 {
                        continue;
                    }
                    let h = d.cross(e2);
                    let det = e1.dot(h);
                    if det.abs() < 1e-9 {
                        continue;
                    }
                    let inv = 1.0 / det;
                    let sv = p - a;
                    let u = sv.dot(h) * inv;
                    if !(0.0..=1.0).contains(&u) {
                        continue;
                    }
                    let q = sv.cross(e1);
                    let v = d.dot(q) * inv;
                    if v < 0.0 || u + v > 1.0 {
                        continue;
                    }
                    let t = e2.dot(q) * inv;
                    if t > 0.0 && t <= reach {
                        return true;
                    }
                }
            }
            false
        };
        let mut bits = vec![0u32; words];
        let region = piece_covers(GEAR_PIECES[pi].0);
        let head_of = |n: &str| names.iter().position(|x| x == n).map_or(Vec3::ZERO, |k| head[k]);
        for (k, v) in verts[..body_n].iter().enumerate() {
            let p = Vec3::from(v.pos);
            let mut best = 0;
            for q in 1..4 {
                if v.weights[q] > v.weights[best] {
                    best = q;
                }
            }
            let jn = names.get(v.joints[best] as usize).map_or("", |s| s.as_str());
            if region_hides(region, jn, p, &head_of) {
                bits[k / 32] |= 1 << (k % 32);
                continue;
            }
            if p.cmplt(lo).any() || p.cmpgt(hi).any() {
                continue;
            }
            let n = Vec3::from(v.nrm).normalize_or_zero();
            if n == Vec3::ZERO {
                continue;
            }
            if hits(p, n, 0.13) || hits(p, -n, 0.04) {
                bits[k / 32] |= 1 << (k % 32);
            }
        }
        cover[pi] = bits;
    }
    (cores, cover, words)
}

/// Smooth normals for the outline shell: every vertex at the same spot (the two sides of a
/// hard edge, where parts meet) gets the average, so the pushed-out shell has no cracks.
fn outline_normals(verts: &mut [SkinVtx]) {
    let key = |p: [f32; 3]| [(p[0] * 2000.0).round() as i32, (p[1] * 2000.0).round() as i32, (p[2] * 2000.0).round() as i32];
    let mut sum: HashMap<[i32; 3], Vec3> = HashMap::new();
    for v in verts.iter() {
        *sum.entry(key(v.pos)).or_insert(Vec3::ZERO) += Vec3::from(v.nrm);
    }
    for v in verts.iter_mut() {
        let n = sum.get(&key(v.pos)).copied().unwrap_or(Vec3::from(v.nrm));
        v.onrm = n.normalize_or(Vec3::from(v.nrm)).to_array();
    }
}

/// An accessory's pieces onto the fighter: read from its glTF (already in the fighter's model
/// space), each vertex given bones and weights by its piece's rule.
fn add_accessory(acc: &Accessory, sk: &Skeleton, joints: &HashMap<&str, usize>, verts: &mut Vec<SkinVtx>, indices: &mut Vec<u32>) -> Result<(), String> {
    let g = gltf::Gltf::from_slice(acc.glb).map_err(|e| format!("accessory: {e}"))?;
    let blob = g.blob.clone().ok_or("accessory: no binary chunk")?;
    let get = |_b: gltf::Buffer| Some(blob.as_slice());
    let lay = layer("accessories");
    // the fighter's own skin (everything before the accessory)
    let body_n = verts.len();
    let j = |n: &str| joints.get(n).copied().unwrap_or(0) as u32;
    // the bones' heights, for the blends
    let at = |n: &str| sk.inv_bind.get(n).map(|m| m.inverse().w_axis.truncate()).unwrap_or(Vec3::ZERO);
    for mesh in g.document.meshes() {
        let mname = mesh.name().unwrap_or("");
        let Some(piece) = acc.pieces.iter().find(|p| mname.starts_with(p.name)) else { continue };
        for prim in mesh.primitives() {
            let r = prim.reader(get);
            let Some(pos) = r.read_positions() else { continue };
            let pos: Vec<[f32; 3]> = pos.collect();
            let nrm: Vec<[f32; 3]> = r.read_normals().map(|i| i.collect()).unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
            let uv: Vec<[f32; 2]> = r.read_tex_coords(0).map(|t| t.into_f32().collect()).unwrap_or_else(|| vec![[0.0, 0.0]; pos.len()]);
            let kind = acc_kind(prim.material().name().unwrap_or(""));
            let base = verts.len() as u32;
            for k in 0..pos.len() {
                let p = Vec3::from(pos[k]);
                let (jn, w): ([u32; 4], [f32; 4]) = match piece.follow {
                    Follow::One(b) => ([j(b), 0, 0, 0], [1.0, 0.0, 0.0, 0.0]),
                    Follow::Blend(a, b, ya, yb) => {
                        let t = ((p.y - yb) / (ya - yb)).clamp(0.0, 1.0);
                        ([j(a), j(b), 0, 0], [t, 1.0 - t, 0.0, 0.0])
                    }
                    Follow::Skin => {
                        let near = (0..body_n).min_by(|&a, &b| (Vec3::from(verts[a].pos) - p).length_squared().total_cmp(&(Vec3::from(verts[b].pos) - p).length_squared()));
                        match near {
                            Some(n) => (verts[n].joints, verts[n].weights),
                            None => ([0, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]),
                        }
                    }
                    Follow::Skirt => {
                        let hip = at("pelvis").y.max(0.5);
                        let t = ((hip + 0.05 - p.y) / 0.45).clamp(0.0, 1.0);
                        let left = (0.5 + p.x / 0.3).clamp(0.0, 1.0);
                        let (tl, tr) = (t * 0.45 * left, t * 0.45 * (1.0 - left));
                        ([j("pelvis"), j("thigh_l"), j("thigh_r"), 0], [1.0 - tl - tr, tl, tr, 0.0])
                    }
                };
                let sway = piece.sway.map_or(0.0, |(top, len)| ((top - p.y) / len).clamp(0.0, 1.0));
                verts.push(SkinVtx { pos: pos[k], nrm: nrm[k], joints: jn, weights: w, mat: mat_word(lay, piece.accent, kind, sway), uv: uv[k], onrm: nrm[k] });
            }
            match r.read_indices() {
                Some(ix) => indices.extend(ix.into_u32().map(|i| base + i)),
                None => indices.extend(base..verts.len() as u32),
            }
        }
    }
    Ok(())
}

/// Ledger Duels gear pieces onto the fighter: each vertex follows the bones of the nearest
/// vertex of the fighter's own skin (the first `body_n`), found through a grid; each piece's
/// triangles are one range of the index buffer (drawn only while it's worn). Dye zones go in
/// bits 16-17 of the material word.
fn add_gear(glb: &[u8], tex: &str, sk: &Skeleton, joints: &HashMap<&str, usize>, body_n: usize, verts: &mut Vec<SkinVtx>, indices: &mut Vec<u32>) -> Result<Vec<(u8, u32, u32)>, String> {
    let g = gltf::Gltf::from_slice(glb).map_err(|e| format!("gear: {e}"))?;
    let j = |n: &str| joints.get(n).copied().unwrap_or(0) as u32;
    let at = |n: &str| sk.inv_bind.get(n).map(|m| m.inverse().w_axis.truncate()).unwrap_or(Vec3::ZERO);
    // Trousers, boots and the coat's skirt follow the leg bones themselves (the nearest skin
    // vertex can belong to the other leg or the wrong joint, which stretched them in a stance):
    // each vertex goes to the nearest bone of its own leg's chain, blended across each joint.
    let left_x = at("thigh_l").x;
    // `cx`: the middle of the boot or trouser leg the vertex belongs to (its side)
    let leg = |p: Vec3, cx: f32| -> ([u32; 4], [f32; 4]) {
        let s = if (cx >= 0.0) == (left_x >= 0.0) { "l" } else { "r" };
        let chain = ["pelvis".to_string(), format!("thigh_{s}"), format!("calf_{s}"), format!("foot_{s}"), format!("ball_{s}")];
        let pts: Vec<Vec3> = chain.iter().map(|n| at(n)).collect();
        let mut best = (f32::MAX, 0usize, 0.0f32);
        for k in 0..pts.len() - 1 {
            let (a, b) = (pts[k], pts[k + 1]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let d = (a + ab * t - p).length_squared();
            if d < best.0 {
                best = (d, k, t);
            }
        }
        let (_, k, t) = best;
        let mut w = [(j(&chain[k]), 1.0f32), (0, 0.0), (0, 0.0)];
        if t < 0.25 && k > 0 {
            w[1] = (j(&chain[k - 1]), 0.5 * (1.0 - t / 0.25));
        }
        if t > 0.75 {
            w[2] = (j(&chain[k + 1]), 0.5 * ((t - 0.75) / 0.25));
        }
        let sum: f32 = w.iter().map(|x| x.1).sum();
        ([w[0].0, w[1].0, w[2].0, 0], [w[0].1 / sum, w[1].1 / sum, w[2].1 / sum, 0.0])
    };
    let hip = at("pelvis").y.max(0.5);
    let skirt = |p: Vec3| -> ([u32; 4], [f32; 4]) {
        let t = ((hip + 0.02 - p.y) / 0.4).clamp(0.0, 1.0);
        let left = (0.5 + p.x.signum() * left_x.signum() * p.x.abs() / 0.3).clamp(0.0, 1.0);
        let (tl, tr) = (t * 0.5 * left, t * 0.5 * (1.0 - left));
        ([j("pelvis"), j("thigh_l"), j("thigh_r"), 0], [1.0 - tl - tr, tl, tr, 0.0])
    };
    let blob = g.blob.clone().ok_or("gear: no binary chunk")?;
    let get = |_b: gltf::Buffer| Some(blob.as_slice());
    let lay = layer(tex);
    // the fighter's own vertices in 4 cm cells
    let cell = 0.04f32;
    let key = |p: Vec3| ((p.x / cell).floor() as i32, (p.y / cell).floor() as i32, (p.z / cell).floor() as i32);
    let mut grid: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
    for (k, v) in verts[..body_n].iter().enumerate() {
        grid.entry(key(Vec3::from(v.pos))).or_default().push(k);
    }
    let nearest = |p: Vec3| -> Option<usize> {
        let (cx, cy, cz) = key(p);
        for r in 1..12 {
            let mut best: Option<(f32, usize)> = None;
            for x in cx - r..=cx + r {
                for y in cy - r..=cy + r {
                    for z in cz - r..=cz + r {
                        if let Some(list) = grid.get(&(x, y, z)) {
                            for &k in list {
                                let q = Vec3::from(verts[k].pos);
                                // off the middle: only the same side of the body (a trouser leg
                                // must never follow the other leg)
                                if p.x.abs() > 0.03 && q.x * p.x < 0.0 {
                                    continue;
                                }
                                let d = (q - p).length_squared();
                                if best.map_or(true, |b| d < b.0) {
                                    best = Some((d, k));
                                }
                            }
                        }
                    }
                }
            }
            // found within this ring: nothing outside it can be nearer than the ring's inner edge
            if let Some((d, k)) = best {
                if d.sqrt() <= (r as f32 - 0.5) * cell || r >= 11 {
                    return Some(k);
                }
            }
        }
        None
    };
    let mut out = Vec::new();
    let mut new_verts: Vec<SkinVtx> = Vec::new();
    let mut new_idx: Vec<u32> = Vec::new();
    for (prefix, id) in GEAR_PIECES {
        let start = (indices.len() + new_idx.len()) as u32;
        for mesh in g.document.meshes().filter(|m| m.name().is_some_and(|n| n.starts_with(prefix))) {
            for prim in mesh.primitives() {
                let r = prim.reader(get);
                let Some(pos) = r.read_positions() else { continue };
                let pos: Vec<[f32; 3]> = pos.collect();
                let nrm: Vec<[f32; 3]> = r.read_normals().map(|i| i.collect()).unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos.len()]);
                let uv: Vec<[f32; 2]> = r.read_tex_coords(0).map(|t| t.into_f32().collect()).unwrap_or_else(|| vec![[0.0, 0.0]; pos.len()]);
                let mat = prim.material().name().unwrap_or("").to_string();
                // (bits 18-21: the wardrobe slot it's worn in, for its dyes)
                let slot = piece_slot(id);
                let cloth = if acc_kind(&mat) == KIND_AUTO { CLOTH_BIT } else { 0 };
                let word = mat_word(lay, false, acc_kind(&mat), 0.0) | dye_zone(&mat) << 16 | slot << 18 | cloth;
                let (legs, feet, torso, cape) = (crate::hub::Slot::Legs as u32 + 1, crate::hub::Slot::Feet as u32 + 1, crate::hub::Slot::Body as u32 + 1, crate::hub::Slot::Cape as u32 + 1);
                let head = crate::hub::Slot::Head as u32 + 1;
                // (the top of the neck: a head piece rides the head above it)
                let neck_y = at("neck_01").y + 0.05;
                let base = (verts.len() + new_verts.len()) as u32;
                let ix: Vec<u32> = match r.read_indices() {
                    Some(ix) => ix.into_u32().collect(),
                    None => (0..pos.len() as u32).collect(),
                };
                // each separate part (a boot, a trouser leg, a band) and the middle of it
                let mut part: Vec<usize> = (0..pos.len()).collect();
                fn root(part: &mut [usize], mut a: usize) -> usize {
                    while part[a] != a {
                        part[a] = part[part[a]];
                        a = part[a];
                    }
                    a
                }
                for t in ix.chunks(3) {
                    for (x, y) in [(t[0], t[1]), (t[1], t[2])] {
                        let (a, b) = (root(&mut part, x as usize), root(&mut part, y as usize));
                        part[a] = b;
                    }
                }
                // (and across its seams: the export splits a tube into strips where its normals
                // or texture break; the two legs are kept apart, so welding never joins them)
                // (never across the middle: a point a hair either side of it belongs to a different leg)
                let weld = |p: [f32; 3]| [(p[0] * 2000.0).round() as i32 * 2 + (p[0] >= 0.0) as i32, (p[1] * 2000.0).round() as i32, (p[2] * 2000.0).round() as i32];
                let mut same: HashMap<[i32; 3], usize> = HashMap::new();
                for k in 0..pos.len() {
                    // (points on the middle line itself are left alone: which leg they are is a
                    // rounding error)
                    if pos[k][0].abs() < 0.003 {
                        continue;
                    }
                    let o = *same.entry(weld(pos[k])).or_insert(k);
                    let (a, b) = (root(&mut part, o), root(&mut part, k));
                    part[a] = b;
                }
                // (x sum, count, lowest x, highest x, y sum)
                let mut mid: HashMap<usize, (f32, f32, f32, f32, f32)> = HashMap::new();
                for k in 0..pos.len() {
                    let e = mid.entry(root(&mut part, k)).or_insert((0.0, 0.0, f32::MAX, f32::MIN, 0.0));
                    e.0 += pos[k][0];
                    e.1 += 1.0;
                    e.2 = e.2.min(pos[k][0]);
                    e.3 = e.3.max(pos[k][0]);
                    e.4 += pos[k][1];
                }
                for k in 0..pos.len() {
                    let p = Vec3::from(pos[k]);
                    let mut word = word;
                    let (jn, w) = if slot == cape {
                        // a cape hangs from the shoulders down the back (and swings, lower down)
                        word = mat_word(lay, false, acc_kind(&mat), ((1.3 - p.y) / 0.6).clamp(0.0, 1.0)) | dye_zone(&mat) << 16 | slot << 18 | cloth;
                        if p.y >= 1.3 {
                            ([j("spine_03"), 0, 0, 0], [1.0, 0.0, 0.0, 0.0])
                        } else if p.y >= 1.0 {
                            let t = (1.3 - p.y) / 0.3;
                            ([j("spine_03"), j("spine_01"), 0, 0], [1.0 - t, t, 0.0, 0.0])
                        } else {
                            let t = ((1.0 - p.y) / 0.3).min(1.0);
                            ([j("spine_01"), j("pelvis"), 0, 0], [1.0 - t, t, 0.0, 0.0])
                        }
                    } else if slot == legs || slot == feet {
                        let m = mid[&root(&mut part, k)];
                        let both_sides = m.2 < -0.03 && m.3 > 0.03;
                        if both_sides && slot == legs && m.4 / m.1 > 0.9 {
                            // round the middle at the waist (the waistband): the hips
                            ([j("pelvis"), 0, 0, 0], [1.0, 0.0, 0.0, 0.0])
                        } else if both_sides {
                            // (something joined across both legs: each vertex goes with its own side)
                            leg(p, p.x)
                        } else {
                            leg(p, m.0 / m.1)
                        }
                    } else if slot == torso && p.y < hip + 0.02 {
                        skirt(p)
                    } else if slot == head && p.y > neck_y {
                        // a hat, a hood and its plumes ride the head (blended into the neck just
                        // above it): nothing on it stretches, however far a plume sticks out
                        let t = ((p.y - neck_y) / 0.06).clamp(0.0, 1.0);
                        ([j("Head"), j("neck_01"), 0, 0], [t, 1.0 - t, 0.0, 0.0])
                    } else {
                        match nearest(p).or_else(|| (0..body_n).min_by(|&a, &b| (Vec3::from(verts[a].pos) - p).length_squared().total_cmp(&(Vec3::from(verts[b].pos) - p).length_squared()))) {
                            Some(n) => (verts[n].joints, verts[n].weights),
                            None => ([j("spine_03"), 0, 0, 0], [1.0, 0.0, 0.0, 0.0]),
                        }
                    };
                    new_verts.push(SkinVtx { pos: pos[k], nrm: nrm[k], joints: jn, weights: w, mat: word, uv: uv[k], onrm: nrm[k] });
                }
                new_idx.extend(ix.iter().map(|i| base + i));
            }
        }
        let end = (indices.len() + new_idx.len()) as u32;
        if end > start {
            out.push((id, start, end - start));
        }
    }
    verts.extend(new_verts);
    indices.extend(new_idx);
    Ok(out)
}

/// The base character's body mesh (the one we cut the head from).
fn hero_body_mesh(doc: &gltf::Document) -> &'static str {
    for m in doc.meshes() {
        if let Some(n) = m.name() {
            if n.starts_with("Superhero_Female") {
                return "Superhero_Female";
            }
            if n.starts_with("Sphere.005_Retopology") {
                return "Sphere.005_Retopology.004";
            }
        }
    }
    ""
}

impl Model {
    /// Carry a pose of the rig over to this fighter's skeleton: each bone turns by as much as
    /// the rig's bone turned from its rest; bones keep their own lengths, and the hips move by
    /// as much (scaled to the leg length).
    pub fn retarget(&self, rig: &Rig, pose: &[Trs]) -> Vec<Trs> {
        pose.iter()
            .enumerate()
            .map(|(n, p)| match self.rest.get(n).copied().flatten() {
                Some(cr) => {
                    let rr = rig.rest_local(n);
                    let r = (cr.r * (rr.r.inverse() * p.r)).normalize();
                    let k = if rr.t.length() > 1e-4 { cr.t.length() / rr.t.length() } else { 1.0 };
                    let t = cr.t + (p.t - rr.t) * k;
                    Trs { t, r, s: p.s }
                }
                None => *p,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Fighter, Tuning};
    use crate::math::v3;

    #[test]
    fn four_fighters_load_and_sit_on_the_skeleton() {
        let rig = crate::anim::shared().expect("rig");
        let models = shared().expect("fighters load");
        assert_eq!(models.len(), 4);
        let nj = rig.joint_names().len() as u32;
        for (k, m) in models.iter().enumerate() {
            assert!(m.verts.len() > 3000 && m.indices.len() % 3 == 0, "{}: a real mesh", name(k as u8));
            assert!(m.verts.iter().all(|v| v.joints.iter().all(|&j| j < nj)), "joints in range");
            assert!(m.verts.iter().all(|v| (v.mat >> 8) & 255 < TEXTURES.len() as u32), "every part textured");
            // the Bridgewright's Rig is fitted to every fighter: six pieces, each a range of its own
            assert_eq!(m.gear.len(), GEAR_PIECES.len(), "{}: the Rig's pieces", name(k as u8));
            assert!(m.gear.iter().all(|&(_, s, c)| s >= m.base_count && c > 30 && (s + c) as usize <= m.indices.len()));
            assert!(m.verts.iter().any(|v| (v.mat >> 16) & 3 == 1) && m.verts.iter().any(|v| (v.mat >> 16) & 3 == 2) && m.verts.iter().any(|v| (v.mat >> 16) & 3 == 3), "its three dye zones");
            assert!(m.verts.iter().any(|v| v.mat & 1 == 1), "{}: has team-colour pieces", name(k as u8));
            // at rest the skinned mesh lands exactly where it was modelled (the parts were moved
            // onto this skeleton correctly, and the pose carries over without twisting)
            let pose = m.retarget(rig, &rig.rest_pose());
            let pal = rig.palette_with(&pose, Mat4::IDENTITY, &m.inv_bind);
            let mut worst: f32 = 0.0;
            for v in m.verts.iter().step_by(37) {
                let p = Vec3::from(v.pos);
                let mut s = Vec3::ZERO;
                for q in 0..4 {
                    s += pal[v.joints[q] as usize].transform_point3(p) * v.weights[q];
                }
                worst = worst.max((s - rig.fix.transform_point3(p)).length());
            }
            assert!(worst < 0.01, "{}: rest pose off by {worst} m", name(k as u8));
        }
    }

    /// Each fighter wears their accessory: skinned onto real bones with whole weights, textured
    /// from the accessories layer; and the pieces that should sway do (and nothing on the head).
    #[test]
    fn accessories_are_worn_and_sway_where_they_should() {
        let rig = crate::anim::shared().expect("rig");
        let models = shared().expect("fighters load");
        let nj = rig.joint_names().len() as u32;
        let acc = layer("accessories");
        for (k, m) in models.iter().enumerate() {
            let worn: Vec<&SkinVtx> = m.verts.iter().filter(|v| v.mat >> 8 == acc).collect();
            assert!(worn.len() > 200, "{}: wears its accessory ({} vertices)", name(k as u8), worn.len());
            for v in &worn {
                let s: f32 = v.weights.iter().sum();
                assert!((s - 1.0).abs() < 1e-3 && v.joints.iter().all(|&j| j < nj), "{}: skinned", name(k as u8));
            }
            let sway = |v: &SkinVtx| (v.mat >> 4) & 15;
            // nothing above the neck sways (the head stays crisp)
            assert!(m.verts.iter().filter(|v| v.pos[1] > 1.5).all(|v| sway(v) == 0), "{}: no sway on the head", name(k as u8));
            let swaying = m.verts.iter().filter(|v| sway(v) > 0).count();
            match k {
                // scarf tails and the apron sway, their tips most
                1 | 2 => assert!(worn.iter().filter(|v| sway(v) >= 10).count() > 20, "{}: the tails / apron hem sway", name(k as u8)),
                _ => {}
            }
            assert!(swaying > 0, "{}: something sways (belt pouches, tails, an apron)", name(k as u8));
            // the faces: eyes and eyebrows marked for the shader
            let kind = |v: &&SkinVtx| (v.mat >> 1) & 7;
            assert!(m.verts.iter().filter(|v| kind(v) == KIND_EYES).count() > 20, "{}: eyes", name(k as u8));
            assert!(m.verts.iter().filter(|v| kind(v) == KIND_BROWS).count() > 10, "{}: eyebrows", name(k as u8));
            // outline normals are unit length
            assert!(m.verts.iter().all(|v| (Vec3::from(v.onrm).length() - 1.0).abs() < 1e-3));
        }
        // the striker's hood trim glows in her colour, the huntress's scarf is in hers
        assert!(models[3].verts.iter().any(|v| (v.mat >> 1) & 7 == KIND_GLOW && v.mat & 1 == 1));
        assert!(models[1].verts.iter().any(|v| v.mat >> 8 == acc && v.mat & 1 == 1));
    }

    /// The face paint darkens the lash lines and leaves the rest of the texture alone.
    #[test]
    fn faces_are_painted_where_the_eyes_are() {
        for (female, eye) in [(false, (140u32, 183u32)), (true, (133, 183))] {
            let mut img = image::RgbaImage::from_pixel(1024, 1024, image::Rgba([200, 160, 140, 255]));
            paint_face(&mut img, female);
            let lash = img.get_pixel(eye.0, eye.1 - 12);
            assert!(lash[0] < 150, "a dark line over the eye: {lash:?}");
            assert_eq!(img.get_pixel(700, 700)[0], 200, "the body untouched");
            assert_eq!(img.get_pixel(eye.0, eye.1 - 70)[0], 200, "the forehead untouched");
        }
    }

    /// Writes each fighter's rest-pose mesh (OBJ, in the game's model space: metres, y up) and
    /// where its bones sit, for tools/blender/build_accessories.py to fit the accessories on:
    /// `cargo test --release dump_fighter_refs -- --ignored`
    #[test]
    #[ignore]
    fn dump_fighter_refs() {
        let rig = crate::anim::shared().expect("rig");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/blender/ref");
        std::fs::create_dir_all(&dir).unwrap();
        let bones = ["Head", "neck_01", "spine_01", "spine_02", "spine_03", "pelvis", "clavicle_l", "clavicle_r", "upperarm_l", "upperarm_r", "lowerarm_l", "lowerarm_r", "hand_l", "hand_r", "thigh_l", "thigh_r", "calf_l", "calf_r", "foot_l", "foot_r", "ball_l", "ball_r"];
        let mut json = String::from("{\n");
        for b in 0..COUNT {
            let s = &SPECS[b];
            // the fighter without its accessory: the shape the accessory is fitted to
            let m = build(rig, b, false).unwrap();
            let mut obj = String::new();
            for v in &m.verts {
                obj.push_str(&format!("v {:.5} {:.5} {:.5}\n", v.pos[0], v.pos[1], v.pos[2]));
            }
            for t in m.indices.chunks(3) {
                obj.push_str(&format!("f {} {} {}\n", t[0] + 1, t[1] + 1, t[2] + 1));
            }
            let key = ["ranger", "huntress", "brawler", "striker"][b];
            std::fs::write(dir.join(format!("{key}.obj")), obj).unwrap();
            let sk = skeleton(&(s.skeleton)()).unwrap();
            json.push_str(&format!("  \"{key}\": {{\n"));
            let rows: Vec<String> = bones
                .iter()
                .filter_map(|n| sk.inv_bind.get(*n).map(|ib| (n, ib.inverse())))
                .map(|(n, g)| {
                    let p = g.w_axis.truncate();
                    format!("    \"{n}\": [{:.5}, {:.5}, {:.5}]", p.x, p.y, p.z)
                })
                .collect();
            json.push_str(&rows.join(",\n"));
            json.push_str(if b + 1 < COUNT { "\n  },\n" } else { "\n  }\n" });
        }
        json.push_str("}\n");
        std::fs::write(dir.join("anchors.json"), json).unwrap();
    }

    #[test]
    fn every_fighter_is_the_same_target() {
        let rig = crate::anim::shared().expect("rig");
        let mut tops = Vec::new();
        for b in 0..COUNT as u8 {
            let mut f = Fighter::new("t", 0, true);
            f.present = true;
            f.alive = true;
            f.loadout.body = b;
            let mut st = crate::character::CharState::new();
            let mut p = None;
            for k in 0..20 {
                p = crate::character::animate(rig, &mut st, &f, v3(0.0, 0.0, 0.0), k as f32 * 0.016, &Tuning::default());
            }
            let p = p.expect("posed");
            let head = p.hit.caps[0];
            tops.push(head.a.y.max(head.b.y) + head.r);
            // the hand holds the weapon where the model's hand is
            assert!((p.hit.caps[8].b - p.hand).len() < 0.02);
        }
        let (lo, hi) = tops.iter().fold((f32::MAX, f32::MIN), |(a, b), &t| (a.min(t), b.max(t)));
        assert!(hi - lo < 0.06, "head heights {tops:?}");
        assert!(lo > 1.6 && hi < 2.05);
    }

    /// Worn gear hides some of each fighter under it, and not all of it.
    #[test]
    fn gear_hides_what_it_covers() {
        let t = std::time::Instant::now();
        let models = shared().expect("fighters load");
        eprintln!("fighters built in {:?}", t.elapsed());
        let mut clips: Vec<_> = crate::anim::shared().unwrap().clips.keys().cloned().collect();
        clips.sort();
        eprintln!("clips: {clips:?}");
        for (b, m) in models.iter().enumerate() {
            let body_n = m.verts.len();
            assert_eq!(m.cores.len(), m.verts.len());
            let mut line = format!("body {b}:");
            for (p, (name, _)) in GEAR_PIECES.iter().enumerate() {
                let n: u32 = m.cover[p].iter().map(|w| w.count_ones()).sum();
                line += &format!(" {name} {n}");
                match piece_covers(name) {
                    Covers::Nothing => {
                        assert_eq!(n, 0, "{name}: capes, chains and pendants hide nothing");
                        continue;
                    }
                    Covers::Surface => {}
                    _ => assert!(n > 0, "{name} hides nothing on body {b}"),
                }
                assert!((n as usize) < body_n / 2, "{name} hides most of body {b}");
            }
            eprintln!("{line} (of {})", m.verts.len());
        }
    }

    /// (a probe) The fighters' own team-coloured pieces below the waist: which bone moves each,
    /// and how many the Herald's legs hide.
    #[test]
    #[ignore]
    fn probe_coat_tails() {
        let rig = crate::anim::shared().expect("rig");
        let names = rig.joint_names();
        let models = shared().expect("fighters load");
        let legs = GEAR_PIECES.iter().position(|g| g.0 == "hr_legs").unwrap();
        for (b, m) in models.iter().enumerate() {
            let mut by: std::collections::BTreeMap<String, (u32, u32)> = Default::default();
            for (k, v) in m.verts.iter().enumerate() {
                if v.mat & 1 == 0 || v.pos[1] > 0.9 || (v.mat >> 8) & 255 == 255 {
                    continue;
                }
                let mut best = 0;
                for q in 1..4 {
                    if v.weights[q] > v.weights[best] {
                        best = q;
                    }
                }
                let jn = names.get(v.joints[best] as usize).cloned().unwrap_or_default();
                let hidden = m.cover[legs].get(k / 32).is_some_and(|w| (w >> (k % 32)) & 1 == 1);
                let e = by.entry(jn).or_default();
                e.0 += 1;
                e.1 += hidden as u32;
            }
            eprintln!("body {b}: {by:?}");
        }
    }

    /// No legs or feet piece has a triangle whose corners follow different legs (it would stretch
    /// from one foot to the other in a stride).
    #[test]
    fn no_gear_triangle_spans_both_legs() {
        let rig = crate::anim::shared().expect("rig");
        let names = rig.joint_names();
        let models = shared().expect("fighters load");
        for m in models.iter() {
        let side = |v: &SkinVtx| -> char {
            let mut best = 0;
            for q in 1..4 {
                if v.weights[q] > v.weights[best] {
                    best = q;
                }
            }
            let n = names.get(v.joints[best] as usize).map_or("", |s| s.as_str());
            if n.ends_with("_l") { 'l' } else if n.ends_with("_r") { 'r' } else { '-' }
        };
        for &(id, s, c) in &m.gear {
            let name = GEAR_PIECES.iter().find(|g| g.1 == id).map_or("?", |g| g.0);
            let mut bad = 0;
            let mut first = None;
            for t in m.indices[s as usize..(s + c) as usize].chunks(3) {
                let sides: Vec<char> = t.iter().map(|&i| side(&m.verts[i as usize])).collect();
                if sides.contains(&'l') && sides.contains(&'r') {
                    bad += 1;
                    if first.is_none() {
                        first = Some(t.iter().map(|&i| m.verts[i as usize].pos).collect::<Vec<_>>());
                    }
                }
            }
            let slot = piece_slot(id);
            if slot == crate::hub::Slot::Legs as u32 + 1 || slot == crate::hub::Slot::Feet as u32 + 1 {
                assert_eq!(bad, 0, "{name}: {bad} triangles across both legs, e.g. {first:?}");
            }
        }
        }
    }

    /// (a probe) What a feet piece hides: by bone, with the height and side ranges.
    #[test]
    #[ignore]
    fn probe_feet_hide() {
        let rig = crate::anim::shared().expect("rig");
        let names = rig.joint_names();
        let models = shared().expect("fighters load");
        for piece in ["ap_feet", "hr_feet"] {
            let pi = GEAR_PIECES.iter().position(|g| g.0 == piece).unwrap();
            for (b, m) in models.iter().enumerate().take(2) {
                let mut by: std::collections::BTreeMap<String, (u32, f32, f32, f32, f32)> = Default::default();
                for (k, v) in m.verts.iter().enumerate() {
                    if !m.cover[pi].get(k / 32).is_some_and(|w| (w >> (k % 32)) & 1 == 1) {
                        continue;
                    }
                    let mut best = 0;
                    for q in 1..4 {
                        if v.weights[q] > v.weights[best] {
                            best = q;
                        }
                    }
                    let jn = names.get(v.joints[best] as usize).cloned().unwrap_or_default();
                    let e = by.entry(jn).or_insert((0, f32::MAX, f32::MIN, f32::MAX, f32::MIN));
                    e.0 += 1;
                    e.1 = e.1.min(v.pos[1]);
                    e.2 = e.2.max(v.pos[1]);
                    e.3 = e.3.min(v.pos[0]);
                    e.4 = e.4.max(v.pos[0]);
                }
                eprintln!("{piece} body {b}: {by:?}");
            }
        }
    }

    /// (a probe) Each Ledger gear piece skinned in a standing pose: its longest triangle edge.
    #[test]
    #[ignore]
    fn probe_gear_stretch() {
        let rig = crate::anim::shared().expect("rig");
        let models = shared().expect("fighters load");
        for (b, m) in models.iter().enumerate() {
            let mut f = Fighter::new("t", 0, true);
            f.present = true;
            f.alive = true;
            f.loadout.body = b as u8;
            let mut st = crate::character::CharState::new();
            let mut p = None;
            for k in 0..30 {
                p = crate::character::animate(rig, &mut st, &f, v3(0.0, 0.0, 0.0), k as f32 * 0.016, &Tuning::default());
            }
            let p = p.unwrap();
            let skin = |i: u32| {
                let v = &m.verts[i as usize];
                let mut s = Vec3::ZERO;
                for q in 0..4 {
                    s += p.palette[v.joints[q] as usize].transform_point3(Vec3::from(v.pos)) * v.weights[q];
                }
                s
            };
            for &(id, s, c) in &m.gear {
                let mut worst = (0.0f32, 0u32);
                let mut rest = 0.0f32;
                let mut pair = (0u32, 0u32);
                for t in m.indices[s as usize..(s + c) as usize].chunks(3) {
                    for (x, y) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                        let e = (skin(x) - skin(y)).length();
                        if e > worst.0 {
                            worst = (e, x);
                            pair = (x, y);
                            rest = (Vec3::from(m.verts[x as usize].pos) - Vec3::from(m.verts[y as usize].pos)).length();
                        }
                    }
                }
                let (va, vb) = (&m.verts[pair.0 as usize], &m.verts[pair.1 as usize]);
                println!("   rest {rest:.2}: {:?} {:?} -> {:?} {:?}", va.pos, va.joints, vb.pos, vb.joints);
                let mut by_joint: std::collections::BTreeMap<u32, (u32, f32, f32)> = Default::default();
                let mut seen = std::collections::HashSet::new();
                for &i in &m.indices[s as usize..(s + c) as usize] {
                    if seen.insert(i) {
                        let v = &m.verts[i as usize];
                        let e = by_joint.entry(v.joints[0]).or_insert((0, f32::MAX, f32::MIN));
                        e.0 += 1;
                        e.1 = e.1.min(v.pos[0]);
                        e.2 = e.2.max(v.pos[0]);
                    }
                }
                println!("   item {id} first joints: {by_joint:?}");
                if id == 53 && b == 0 {
                    let ix = &m.indices[s as usize..(s + c) as usize];
                    let mut part: HashMap<u32, u32> = HashMap::new();
                    fn root(p: &mut HashMap<u32, u32>, mut a: u32) -> u32 {
                        while let Some(&n) = p.get(&a) {
                            if n == a {
                                break;
                            }
                            a = n;
                        }
                        a
                    }
                    for &i in ix {
                        part.entry(i).or_insert(i);
                    }
                    for t in ix.chunks(3) {
                        let (a, bb) = (root(&mut part, t[0]), root(&mut part, t[1]));
                        part.insert(a, bb);
                        let (a, bb) = (root(&mut part, t[1]), root(&mut part, t[2]));
                        part.insert(a, bb);
                    }
                    let mut comps: HashMap<u32, (u32, Vec3, Vec3, Vec3, u32)> = HashMap::new();
                    let keys: Vec<u32> = part.keys().copied().collect();
                    for i in keys {
                        let r = root(&mut part, i);
                        let p = Vec3::from(m.verts[i as usize].pos);
                        let e = comps.entry(r).or_insert((0, Vec3::ZERO, Vec3::splat(9.0), Vec3::splat(-9.0), m.verts[i as usize].mat));
                        e.0 += 1;
                        e.1 += p;
                        e.2 = e.2.min(p);
                        e.3 = e.3.max(p);
                    }
                    for (_, (n, sum, lo, hi, mat)) in comps {
                        println!("      part: {n} verts, centre {:.3?}, from {:.2?} to {:.2?}, zone {}", sum / n as f32, lo, hi, (mat >> 16) & 3);
                    }
                }
                let v = &m.verts[worst.1 as usize];
                println!("body {b} item {id}: longest edge {:.2} m at rest pos {:?} joints {:?} w {:?}", worst.0, v.pos, v.joints, v.weights);
            }
        }
    }
}