//! Controller glyphs and Steam Input.
//!
//! - Which controller is in hand ([`PadKind`]): from Steam Input's own report when it's reading
//!   the pad, else from the pad's name, unless Settings picks a family.
//! - Button names and drawn glyphs for each family: Xbox (coloured A/B/X/Y), PlayStation (the
//!   four shapes), Switch (its swapped letters: B is the bottom button) and the Steam Deck (its
//!   own dark buttons, L1/R1, the back grips).
//! - Steam Input: the action names in each action set (menu, combat, hub, spectate), turning
//!   the actions Steam reports back into the game's buttons ([`pad_from_actions`]), and the
//!   files for the Steamworks partner site: the action manifest and a default layout per
//!   controller, the Steam Deck's included (`steam/`).

use crate::pad::btn;
use crate::platform::steam_api::{ActionSet, PadKind, SteamPad};
use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};
use std::sync::atomic::{AtomicU8, Ordering};

impl PadKind {
    pub const ALL: [PadKind; 4] = [PadKind::Xbox, PadKind::PlayStation, PadKind::Switch, PadKind::Deck];
    pub fn title(self) -> &'static str {
        match self {
            PadKind::Xbox => "Xbox",
            PadKind::PlayStation => "PlayStation",
            PadKind::Switch => "Switch Pro",
            PadKind::Deck => "Steam Deck",
        }
    }
    /// From the name the system gives the pad.
    pub fn from_name(name: Option<&str>) -> PadKind {
        let n = name.unwrap_or("").to_ascii_lowercase();
        if n.contains("steam deck") || n.contains("neptune") || n.contains("valve") && n.contains("deck") {
            PadKind::Deck
        } else if n.contains("playstation") || n.contains("dualshock") || n.contains("dualsense") || n.contains("ps4") || n.contains("ps5") || n.contains("sony") || n.contains("wireless controller") && !n.contains("xbox") {
            PadKind::PlayStation
        } else if n.contains("nintendo") || n.contains("switch") || n.contains("pro controller") || n.contains("joy-con") {
            PadKind::Switch
        } else {
            PadKind::Xbox
        }
    }
    fn to_u8(self) -> u8 {
        match self {
            PadKind::Xbox => 1,
            PadKind::PlayStation => 2,
            PadKind::Switch => 3,
            PadKind::Deck => 4,
        }
    }
    pub fn from_u8(v: u8) -> Option<PadKind> {
        match v {
            1 => Some(PadKind::Xbox),
            2 => Some(PadKind::PlayStation),
            3 => Some(PadKind::Switch),
            4 => Some(PadKind::Deck),
            _ => None,
        }
    }
}

/// The family Steam Input reports.
pub fn kind_of_input(t: steamworks::InputType) -> PadKind {
    use steamworks::InputType as T;
    match t {
        T::PS3Controller | T::PS4Controller | T::PS5Controller => PadKind::PlayStation,
        T::SwitchProController | T::SwitchJoyConPair | T::SwitchJoyConSingle => PadKind::Switch,
        T::SteamDeckController | T::SteamController => PadKind::Deck,
        _ => PadKind::Xbox,
    }
}

/// The family prompts use this frame (0: worked out from the pad's name), set by the game each
/// frame from Settings and Steam Input.
static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_current(k: Option<PadKind>) {
    CURRENT.store(k.map_or(0, PadKind::to_u8), Ordering::Relaxed);
}

/// The family for prompts: the one set this frame, else from the pad's name.
pub fn current(pad_name: Option<&str>) -> PadKind {
    PadKind::from_u8(CURRENT.load(Ordering::Relaxed)).unwrap_or_else(|| PadKind::from_name(pad_name))
}

/// Which family to show: Settings' choice (0 automatic), else what Steam Input reports, else the
/// pad's name.
pub fn pick(setting: u8, steam: Option<PadKind>, pad_name: Option<&str>) -> PadKind {
    PadKind::from_u8(setting).or(steam).unwrap_or_else(|| PadKind::from_name(pad_name))
}

/// What a button is called on this family (for prompts in text).
pub fn name(bit: u32, k: PadKind) -> &'static str {
    use PadKind::*;
    match (bit, k) {
        (btn::A, PlayStation) => "Cross",
        (btn::B, PlayStation) => "Circle",
        (btn::X, PlayStation) => "Square",
        (btn::Y, PlayStation) => "Triangle",
        // the Switch's letters sit the other way round: B is the bottom button
        (btn::A, Switch) => "B",
        (btn::B, Switch) => "A",
        (btn::X, Switch) => "Y",
        (btn::Y, Switch) => "X",
        (btn::A, _) => "A",
        (btn::B, _) => "B",
        (btn::X, _) => "X",
        (btn::Y, _) => "Y",
        (btn::LB, Xbox) => "LB",
        (btn::RB, Xbox) => "RB",
        (btn::LT, Xbox) => "LT",
        (btn::RT, Xbox) => "RT",
        (btn::LB, Switch) => "L",
        (btn::RB, Switch) => "R",
        (btn::LT, Switch) => "ZL",
        (btn::RT, Switch) => "ZR",
        (btn::LB, _) => "L1",
        (btn::RB, _) => "R1",
        (btn::LT, _) => "L2",
        (btn::RT, _) => "R2",
        (btn::VIEW, Xbox) => "View",
        (btn::VIEW, PlayStation) => "Create",
        (btn::VIEW, Switch) => "Minus",
        (btn::VIEW, Deck) => "View",
        (btn::MENU, PlayStation) => "Options",
        (btn::MENU, Switch) => "Plus",
        (btn::MENU, _) => "Menu",
        (btn::L3, Xbox) | (btn::L3, Switch) => "LS",
        (btn::R3, Xbox) | (btn::R3, Switch) => "RS",
        (btn::L3, _) => "L3",
        (btn::R3, _) => "R3",
        (btn::UP, _) => "D-pad up",
        (btn::DOWN, _) => "D-pad down",
        (btn::LEFT, _) => "D-pad left",
        (btn::RIGHT, _) => "D-pad right",
        _ => "?",
    }
}

/// Every gameplay action and its button (the glyph layout in Settings and How to Play). None:
/// a stick (named by the text).
pub const LAYOUT: [(&str, Option<u32>, &str); 18] = [
    ("Move", None, "Left stick"),
    ("Look", None, "Right stick"),
    ("Shoot / slash", Some(btn::RT), ""),
    ("Guard / aim down sights", Some(btn::LT), ""),
    ("Jump / wall-run / wall-jump", Some(btn::A), ""),
    ("Slide (hold, while running)", Some(btn::B), ""),
    ("Reload", Some(btn::X), ""),
    ("Next weapon", Some(btn::Y), ""),
    ("Dash", Some(btn::L3), ""),
    ("Ability", Some(btn::LB), ""),
    ("Kick", Some(btn::RB), ""),
    ("Finisher", Some(btn::R3), ""),
    ("Use item", Some(btn::UP), ""),
    ("Blade", Some(btn::LEFT), ""),
    ("Swap guns", Some(btn::RIGHT), ""),
    ("Quick chat", Some(btn::DOWN), ""),
    ("Player list (hold)", Some(btn::VIEW), ""),
    ("Pause", Some(btn::MENU), ""),
];

/// The Steam Deck's extra controls in its default layout (back grips and trackpads).
pub const DECK_EXTRAS: [(&str, &str); 6] = [
    ("L4 (back grip)", "Dash"),
    ("R4 (back grip)", "Kick"),
    ("L5 (back grip)", "Quick chat"),
    ("R5 (back grip)", "Finisher"),
    ("Right trackpad", "Look, like a mouse (fine aim)"),
    ("Gyro", "Look while guarding / aiming down sights"),
];

fn face_colour(bit: u32, k: PadKind) -> Color32 {
    match k {
        PadKind::Xbox => match bit {
            btn::A => Color32::from_rgb(96, 196, 90),
            btn::B => Color32::from_rgb(232, 72, 64),
            btn::X => Color32::from_rgb(70, 140, 240),
            _ => Color32::from_rgb(250, 196, 50),
        },
        PadKind::PlayStation => match bit {
            btn::A => Color32::from_rgb(130, 170, 255),
            btn::B => Color32::from_rgb(255, 104, 110),
            btn::X => Color32::from_rgb(232, 140, 216),
            _ => Color32::from_rgb(70, 216, 186),
        },
        _ => Color32::from_rgb(236, 236, 240),
    }
}

/// Draw one button's glyph centred on `c`, about `size` tall.
pub fn paint(p: &Painter, c: Pos2, size: f32, bit: u32, k: PadKind) {
    let r = size * 0.5;
    let dark = match k {
        PadKind::Deck => Color32::from_rgb(34, 37, 44),
        PadKind::Switch => Color32::from_rgb(40, 40, 46),
        _ => Color32::from_rgb(24, 24, 30),
    };
    let rim = Stroke::new((size * 0.06).max(1.0), Color32::from_rgba_unmultiplied(255, 255, 255, 120));
    let font = FontId::proportional(size * 0.55);
    match bit {
        btn::A | btn::B | btn::X | btn::Y => {
            p.circle_filled(c, r, dark);
            p.circle_stroke(c, r, rim);
            let col = face_colour(bit, k);
            if k == PadKind::PlayStation {
                let s = r * 0.48;
                let w = Stroke::new((size * 0.09).max(1.2), col);
                match bit {
                    btn::A => {
                        p.line_segment([c + Vec2::new(-s, -s), c + Vec2::new(s, s)], w);
                        p.line_segment([c + Vec2::new(-s, s), c + Vec2::new(s, -s)], w);
                    }
                    btn::B => {
                        p.circle_stroke(c, s * 1.05, w);
                    }
                    btn::X => {
                        p.rect_stroke(Rect::from_center_size(c, Vec2::splat(s * 1.8)), 0.0, w, StrokeKind::Middle);
                    }
                    _ => {
                        let pts = [c + Vec2::new(0.0, -s * 1.1), c + Vec2::new(s * 1.05, s * 0.75), c + Vec2::new(-s * 1.05, s * 0.75)];
                        p.add(Shape::closed_line(pts.to_vec(), w));
                    }
                }
            } else {
                p.text(c + Vec2::new(0.0, size * 0.02), Align2::CENTER_CENTER, name(bit, k), font, col);
            }
        }
        btn::LB | btn::RB | btn::LT | btn::RT => {
            let trig = bit == btn::LT || bit == btn::RT;
            let w = size * 1.45;
            let rr = Rect::from_center_size(c, Vec2::new(w, size * 0.82));
            let round = if trig { size * 0.36 } else { size * 0.18 };
            p.rect_filled(rr, round, dark);
            p.rect_stroke(rr, round, rim, StrokeKind::Inside);
            p.text(rr.center(), Align2::CENTER_CENTER, name(bit, k), FontId::proportional(size * 0.44), Color32::WHITE);
        }
        btn::L3 | btn::R3 => {
            p.circle_filled(c, r, dark);
            p.circle_stroke(c, r * 0.68, Stroke::new(rim.width, Color32::from_gray(150)));
            p.circle_stroke(c, r, rim);
            let t = if bit == btn::L3 { "L" } else { "R" };
            p.text(c, Align2::CENTER_CENTER, t, FontId::proportional(size * 0.42), Color32::WHITE);
            // pressed in: a small arrow under it
            let y = c.y + r + size * 0.02;
            p.add(Shape::convex_polygon(vec![Pos2::new(c.x - size * 0.12, y), Pos2::new(c.x + size * 0.12, y), Pos2::new(c.x, y + size * 0.14)], Color32::WHITE, Stroke::NONE));
        }
        btn::UP | btn::DOWN | btn::LEFT | btn::RIGHT => {
            let a = size * 0.3;
            let l = size * 0.5;
            let arm = |dx: f32, dy: f32| Rect::from_center_size(c + Vec2::new(dx, dy) * (l - a * 0.5), Vec2::new(if dx != 0.0 { l } else { a }, if dy != 0.0 { l } else { a }));
            for (dx, dy, b) in [(0.0, -1.0, btn::UP), (0.0, 1.0, btn::DOWN), (-1.0, 0.0, btn::LEFT), (1.0, 0.0, btn::RIGHT)] {
                p.rect_filled(arm(dx, dy), size * 0.05, if b == bit { Color32::from_rgb(255, 196, 120) } else { dark });
            }
            p.rect_filled(Rect::from_center_size(c, Vec2::splat(a)), 0.0, dark);
            p.rect_stroke(Rect::from_center_size(c, Vec2::new(l * 2.0, a)), size * 0.05, Stroke::new(rim.width * 0.7, rim.color), StrokeKind::Outside);
            p.rect_stroke(Rect::from_center_size(c, Vec2::new(a, l * 2.0)), size * 0.05, Stroke::new(rim.width * 0.7, rim.color), StrokeKind::Outside);
        }
        btn::VIEW | btn::MENU => {
            let rr = Rect::from_center_size(c, Vec2::new(size * 0.95, size * 0.7));
            p.rect_filled(rr, size * 0.3, dark);
            p.rect_stroke(rr, size * 0.3, rim, StrokeKind::Inside);
            let w = Stroke::new((size * 0.07).max(1.0), Color32::WHITE);
            match (bit, k) {
                (btn::VIEW, PadKind::Switch) => {
                    p.line_segment([c + Vec2::new(-size * 0.17, 0.0), c + Vec2::new(size * 0.17, 0.0)], w);
                }
                (btn::MENU, PadKind::Switch) => {
                    p.line_segment([c + Vec2::new(-size * 0.17, 0.0), c + Vec2::new(size * 0.17, 0.0)], w);
                    p.line_segment([c + Vec2::new(0.0, -size * 0.17), c + Vec2::new(0.0, size * 0.17)], w);
                }
                (btn::VIEW, _) => {
                    let s = size * 0.13;
                    p.rect_stroke(Rect::from_center_size(c + Vec2::new(-s * 0.45, -s * 0.35), Vec2::splat(s * 1.5)), 1.0, w, StrokeKind::Middle);
                    p.rect_filled(Rect::from_center_size(c + Vec2::new(s * 0.45, s * 0.35), Vec2::splat(s * 1.5)), 1.0, Color32::WHITE);
                }
                _ => {
                    for k in -1..=1 {
                        let y = c.y + k as f32 * size * 0.11;
                        p.line_segment([Pos2::new(c.x - size * 0.17, y), Pos2::new(c.x + size * 0.17, y)], w);
                    }
                }
            }
        }
        _ => {}
    }
}

/// A stick glyph ("L" or "R" with arrows round it: move or look).
pub fn paint_stick(p: &Painter, c: Pos2, size: f32, left: bool, k: PadKind) {
    let r = size * 0.5;
    let dark = if k == PadKind::Deck { Color32::from_rgb(34, 37, 44) } else { Color32::from_rgb(24, 24, 30) };
    p.circle_filled(c, r, dark);
    p.circle_stroke(c, r * 0.68, Stroke::new(1.0, Color32::from_gray(150)));
    p.circle_stroke(c, r, Stroke::new((size * 0.06).max(1.0), Color32::from_rgba_unmultiplied(255, 255, 255, 120)));
    p.text(c, Align2::CENTER_CENTER, if left { "L" } else { "R" }, FontId::proportional(size * 0.42), Color32::WHITE);
    for (dx, dy) in [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
        let d = Vec2::new(dx, dy);
        let tip = c + d * (r + size * 0.2);
        let side = Vec2::new(-dy, dx) * size * 0.09;
        let base = c + d * (r + size * 0.07);
        p.add(Shape::convex_polygon(vec![tip, base + side, base - side], Color32::from_gray(220), Stroke::NONE));
    }
}

// ================================================================================ Steam Input

/// Digital actions in the action manifest and the game button each one presses. The hub and
/// spectating sets press the same buttons as the match does for the same thumb.
pub const DIGITAL_ACTIONS: &[(&str, u32)] = &[
    // combat
    ("jump", btn::A),
    ("slide", btn::B),
    ("reload", btn::X),
    ("next_weapon", btn::Y),
    ("ability", btn::LB),
    ("kick", btn::RB),
    ("dash", btn::L3),
    ("finisher", btn::R3),
    ("use_item", btn::UP),
    ("blade", btn::LEFT),
    ("swap_guns", btn::RIGHT),
    ("quick_chat", btn::DOWN),
    ("scoreboard", btn::VIEW),
    ("pause", btn::MENU),
    // menus
    ("menu_select", btn::A),
    ("menu_back", btn::B),
    ("menu_up", btn::UP),
    ("menu_down", btn::DOWN),
    ("menu_left", btn::LEFT),
    ("menu_right", btn::RIGHT),
    ("menu_prev_tab", btn::LB),
    ("menu_next_tab", btn::RB),
    ("menu_close", btn::MENU),
    // Varrow Centre
    ("hub_jump", btn::A),
    ("hub_crouch", btn::B),
    ("hub_interact", btn::X),
    ("hub_wheel", btn::Y),
    ("hub_dash", btn::L3),
    ("hub_chat", btn::DOWN),
    ("hub_players", btn::VIEW),
    ("hub_menu", btn::MENU),
    // spectating
    ("spec_next", btn::RB),
    ("spec_prev", btn::LB),
    ("spec_camera", btn::Y),
    ("spec_leave", btn::B),
    ("spec_scoreboard", btn::VIEW),
    ("spec_menu", btn::MENU),
];

/// Analog actions: sticks (move / look) and triggers.
pub const ANALOG_ACTIONS: &[&str] = &["move", "look", "fire", "guard", "menu_move", "menu_scroll", "hub_move", "hub_look", "spec_move", "spec_look"];

/// Which actions each set holds (digital, then analog) for the manifest.
pub fn set_actions(s: ActionSet) -> (Vec<&'static str>, Vec<(&'static str, &'static str)>) {
    let pre = match s {
        ActionSet::Menu => "menu_",
        ActionSet::Hub => "hub_",
        ActionSet::Spectate => "spec_",
        ActionSet::Combat => "",
    };
    let digital: Vec<&str> = DIGITAL_ACTIONS
        .iter()
        .map(|a| a.0)
        .filter(|n| if pre.is_empty() { !n.starts_with("menu_") && !n.starts_with("hub_") && !n.starts_with("spec_") } else { n.starts_with(pre) })
        .collect();
    let analog: Vec<(&str, &str)> = match s {
        ActionSet::Combat => vec![("move", "joystick_move"), ("look", "joystick_move"), ("fire", "analog_trigger"), ("guard", "analog_trigger")],
        ActionSet::Menu => vec![("menu_move", "joystick_move"), ("menu_scroll", "joystick_move")],
        ActionSet::Hub => vec![("hub_move", "joystick_move"), ("hub_look", "joystick_move"), ("fire", "analog_trigger"), ("guard", "analog_trigger")],
        ActionSet::Spectate => vec![("spec_move", "joystick_move"), ("spec_look", "joystick_move")],
    };
    (digital, analog)
}

/// The actions Steam reported, as the game's buttons, sticks and triggers.
pub fn pad_from_actions(digital: &[(&str, bool, u32)], analog: &[(&str, f32, f32)]) -> SteamPad {
    let mut out = SteamPad::default();
    for &(_, on, bit) in digital {
        if on {
            out.held |= bit;
        }
    }
    for &(n, x, y) in analog {
        let (x, y) = (if x.is_finite() { x.clamp(-1.0, 1.0) } else { 0.0 }, if y.is_finite() { y.clamp(-1.0, 1.0) } else { 0.0 });
        match n {
            "move" | "menu_move" | "hub_move" | "spec_move" if x != 0.0 || y != 0.0 => (out.lx, out.ly) = crate::pad::stick(x, y),
            "look" | "menu_scroll" | "hub_look" | "spec_look" if x != 0.0 || y != 0.0 => (out.rx, out.ry) = crate::pad::stick(x, y),
            "fire" => out.rt = out.rt.max(x.max(0.0)),
            "guard" => out.lt = out.lt.max(x.max(0.0)),
            _ => {}
        }
    }
    if out.lt > crate::pad::TRIGGER {
        out.held |= btn::LT;
    }
    if out.rt > crate::pad::TRIGGER {
        out.held |= btn::RT;
    }
    out
}

/// The action set for what's on screen.
pub fn set_for(in_match: bool, hub: bool, watching: bool, captured: bool) -> ActionSet {
    if !captured {
        ActionSet::Menu
    } else if watching {
        ActionSet::Spectate
    } else if hub {
        ActionSet::Hub
    } else if in_match {
        ActionSet::Combat
    } else {
        ActionSet::Menu
    }
}

fn title_of(action: &str) -> String {
    let base = action.trim_start_matches("menu_").trim_start_matches("hub_").trim_start_matches("spec_");
    let mut s: String = base.replace('_', " ");
    if let Some(f) = s.get_mut(0..1) {
        f.make_ascii_uppercase();
    }
    s
}

/// steam/steam_input_manifest.vdf: the action sets, their actions and the default layout for
/// each controller (In-Game Actions format).
pub fn manifest(app: u32) -> String {
    let mut s = String::from("\"Action Manifest\"\n{\n\t\"configurations\"\n\t{\n");
    s += &format!("\t\t\"{app}\"\n\t\t{{\n");
    for (ty, file) in CONFIGS {
        s += &format!("\t\t\t\"{ty}\"\n\t\t\t{{\n\t\t\t\t\"0\"\n\t\t\t\t{{\n\t\t\t\t\t\"path\"\t\t\"{file}\"\n\t\t\t\t}}\n\t\t\t}}\n");
    }
    s += "\t\t}\n\t}\n\t\"actions\"\n\t{\n";
    let mut loc: Vec<(String, String)> = Vec::new();
    for set in ActionSet::ALL {
        let (digital, analog) = set_actions(set);
        let st = format!("Set_{}", set.key());
        loc.push((st.clone(), set.title().to_string()));
        s += &format!("\t\t\"{}\"\n\t\t{{\n\t\t\t\"title\"\t\t\"#{st}\"\n\t\t\t\"legacy_set\"\t\t\"0\"\n", set.key());
        s += "\t\t\t\"StickPadGyro\"\n\t\t\t{\n";
        for (a, mode) in analog.iter().filter(|a| a.1 == "joystick_move") {
            let t = format!("Action_{a}");
            loc.push((t.clone(), title_of(a)));
            s += &format!("\t\t\t\t\"{a}\"\n\t\t\t\t{{\n\t\t\t\t\t\"title\"\t\t\"#{t}\"\n\t\t\t\t\t\"input_mode\"\t\t\"{mode}\"\n\t\t\t\t}}\n");
        }
        s += "\t\t\t}\n\t\t\t\"AnalogTrigger\"\n\t\t\t{\n";
        for (a, _) in analog.iter().filter(|a| a.1 == "analog_trigger") {
            let t = format!("Action_{a}");
            if !loc.iter().any(|l| l.0 == t) {
                loc.push((t.clone(), title_of(a)));
            }
            s += &format!("\t\t\t\t\"{a}\"\t\t\"#{t}\"\n");
        }
        s += "\t\t\t}\n\t\t\t\"Button\"\n\t\t\t{\n";
        for a in digital {
            let t = format!("Action_{a}");
            loc.push((t.clone(), title_of(a)));
            s += &format!("\t\t\t\t\"{a}\"\t\t\"#{t}\"\n");
        }
        s += "\t\t\t}\n\t\t}\n";
    }
    s += "\t}\n\t\"localization\"\n\t{\n\t\t\"english\"\n\t\t{\n";
    for (k, v) in loc {
        s += &format!("\t\t\t\"{k}\"\t\t\"{v}\"\n");
    }
    s += "\t\t}\n\t}\n}\n";
    s
}

/// Each controller type's default layout file.
pub const CONFIGS: [(&str, &str); 5] = [
    ("controller_xboxone", "controller_xbox.vdf"),
    ("controller_xbox360", "controller_xbox.vdf"),
    ("controller_ps5", "controller_ps.vdf"),
    ("controller_switch_pro", "controller_switch.vdf"),
    ("controller_neptune", "controller_steamdeck.vdf"),
];

/// Physical inputs (Steam's names) for a game button, per action set.
fn source_of(bit: u32) -> (&'static str, &'static str) {
    match bit {
        btn::A => ("button_diamond", "button_a"),
        btn::B => ("button_diamond", "button_b"),
        btn::X => ("button_diamond", "button_x"),
        btn::Y => ("button_diamond", "button_y"),
        btn::LB => ("switch", "left_bumper"),
        btn::RB => ("switch", "right_bumper"),
        btn::VIEW => ("switch", "button_escape"),
        btn::MENU => ("switch", "button_menu"),
        btn::L3 => ("left_joystick", "click"),
        btn::R3 => ("right_joystick", "click"),
        btn::UP => ("dpad", "dpad_north"),
        btn::DOWN => ("dpad", "dpad_south"),
        btn::LEFT => ("dpad", "dpad_west"),
        btn::RIGHT => ("dpad", "dpad_east"),
        _ => ("switch", "none"),
    }
}

/// A default controller layout (Steam's controller configuration format). The Deck's adds the
/// back grips, the right trackpad as a mouse and gyro aiming while guarding.
pub fn controller_config(ty: &str, title: &str) -> String {
    let deck = ty == "controller_neptune";
    let mut s = format!(
        "\"controller_mappings\"\n{{\n\t\"version\"\t\t\"3\"\n\t\"revision\"\t\t\"1\"\n\t\"title\"\t\t\"Duel Rush - {title}\"\n\t\"description\"\t\t\"The default layout: move on the left stick, look on the right, shoot on the right trigger, guard on the left.\"\n\t\"creator\"\t\t\"0\"\n\t\"controller_type\"\t\t\"{ty}\"\n\t\"Timestamp\"\t\t\"0\"\n"
    );
    let mut gid = 0;
    let mut presets = String::new();
    for (pid, set) in ActionSet::ALL.iter().enumerate() {
        let (digital, analog) = set_actions(*set);
        let mut bindings: Vec<(String, Vec<(String, String)>)> = Vec::new();
        let mut add = |group: &str, input: &str, action: String| {
            match bindings.iter_mut().find(|b| b.0 == group) {
                Some(b) => b.1.push((input.to_string(), action)),
                None => bindings.push((group.to_string(), vec![(input.to_string(), action)])),
            }
        };
        for a in &digital {
            let bit = DIGITAL_ACTIONS.iter().find(|d| d.0 == *a).map_or(0, |d| d.1);
            let (g, i) = source_of(bit);
            add(g, i, format!("game_action {} {a}", set.key()));
            if deck {
                // the back grips double the most-used thumb buttons
                let grip = match *a {
                    "dash" | "hub_dash" => Some("button_back_left"),
                    "kick" => Some("button_back_right"),
                    "quick_chat" | "hub_chat" => Some("button_back_left_upper"),
                    "finisher" => Some("button_back_right_upper"),
                    _ => None,
                };
                if let Some(gi) = grip {
                    add("switch", gi, format!("game_action {} {a}", set.key()));
                }
            }
        }
        let mut groups = String::new();
        let mut sources = String::new();
        for (g, inputs) in &bindings {
            let mode = match g.as_str() {
                "button_diamond" => "four_buttons",
                "dpad" => "dpad",
                "left_joystick" | "right_joystick" => "joystick_move",
                _ => "switches",
            };
            groups += &format!("\t\"group\"\n\t{{\n\t\t\"id\"\t\t\"{gid}\"\n\t\t\"mode\"\t\t\"{mode}\"\n\t\t\"inputs\"\n\t\t{{\n");
            for (input, action) in inputs {
                groups += &format!("\t\t\t\"{input}\"\n\t\t\t{{\n\t\t\t\t\"activators\"\n\t\t\t\t{{\n\t\t\t\t\t\"Full_Press\"\n\t\t\t\t\t{{\n\t\t\t\t\t\t\"bindings\"\n\t\t\t\t\t\t{{\n\t\t\t\t\t\t\t\"binding\"\t\t\"{action}\"\n\t\t\t\t\t\t}}\n\t\t\t\t\t}}\n\t\t\t\t}}\n\t\t\t}}\n");
            }
            groups += "\t\t}\n\t}\n";
            sources += &format!("\t\t\t\"{gid}\"\t\t\"{g} active\"\n");
            gid += 1;
        }
        // sticks, triggers, the trackpad and gyro
        for (a, mode) in &analog {
            let (src, gm) = match (*a, *mode) {
                ("move" | "menu_move" | "hub_move" | "spec_move", _) => ("left_joystick", "joystick_move"),
                ("look" | "menu_scroll" | "hub_look" | "spec_look", _) => ("right_joystick", "joystick_move"),
                ("fire", _) => ("right_trigger", "trigger"),
                _ => ("left_trigger", "trigger"),
            };
            groups += &format!("\t\"group\"\n\t{{\n\t\t\"id\"\t\t\"{gid}\"\n\t\t\"mode\"\t\t\"{gm}\"\n\t\t\"gameactions\"\n\t\t{{\n\t\t\t\"{}\"\t\t\"{a}\"\n\t\t}}\n\t}}\n", set.key());
            sources += &format!("\t\t\t\"{gid}\"\t\t\"{src} active\"\n");
            gid += 1;
            if deck && matches!(*a, "look" | "hub_look" | "spec_look") {
                groups += &format!("\t\"group\"\n\t{{\n\t\t\"id\"\t\t\"{gid}\"\n\t\t\"mode\"\t\t\"absolute_mouse\"\n\t\t\"gameactions\"\n\t\t{{\n\t\t\t\"{}\"\t\t\"{a}\"\n\t\t}}\n\t}}\n", set.key());
                sources += &format!("\t\t\t\"{gid}\"\t\t\"right_trackpad active\"\n");
                gid += 1;
                if *a == "look" {
                    groups += &format!("\t\"group\"\n\t{{\n\t\t\"id\"\t\t\"{gid}\"\n\t\t\"mode\"\t\t\"gyro_to_mouse\"\n\t\t\"settings\"\n\t\t{{\n\t\t\t\"gyro_button\"\t\t\"left_trigger_threshold\"\n\t\t}}\n\t\t\"gameactions\"\n\t\t{{\n\t\t\t\"{}\"\t\t\"{a}\"\n\t\t}}\n\t}}\n", set.key());
                    sources += &format!("\t\t\t\"{gid}\"\t\t\"gyro active\"\n");
                    gid += 1;
                }
            }
        }
        s += &groups;
        presets += &format!("\t\"preset\"\n\t{{\n\t\t\"id\"\t\t\"{pid}\"\n\t\t\"name\"\t\t\"{}\"\n\t\t\"group_source_bindings\"\n\t\t{{\n{sources}\t\t}}\n\t}}\n", set.key());
    }
    s += &presets;
    s += "\t\"settings\"\n\t{\n\t\t\"action_set_trigger_cursor_show\"\t\t\"0\"\n\t}\n}\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::steam_api::{FakeSteam, SteamApi};

    #[test]
    fn the_glyphs_match_the_pad_in_hand() {
        assert_eq!(PadKind::from_name(Some("DualSense Wireless Controller")), PadKind::PlayStation);
        assert_eq!(PadKind::from_name(Some("Xbox Wireless Controller")), PadKind::Xbox);
        assert_eq!(PadKind::from_name(Some("Nintendo Switch Pro Controller")), PadKind::Switch);
        assert_eq!(PadKind::from_name(Some("Steam Deck")), PadKind::Deck);
        assert_eq!(PadKind::from_name(None), PadKind::Xbox);
        // the Switch's bottom button is B, PlayStation's is Cross
        assert_eq!(name(btn::A, PadKind::Switch), "B");
        assert_eq!(name(btn::A, PadKind::PlayStation), "Cross");
        assert_eq!(name(btn::RT, PadKind::Switch), "ZR");
        assert_eq!(name(btn::LB, PadKind::Deck), "L1");
        assert_eq!(name(btn::MENU, PadKind::PlayStation), "Options");
        // Settings beats Steam Input, which beats the pad's name
        assert_eq!(pick(0, Some(PadKind::Deck), Some("Xbox Controller")), PadKind::Deck);
        assert_eq!(pick(2, Some(PadKind::Deck), None), PadKind::PlayStation);
        assert_eq!(pick(0, None, Some("PS5 Controller")), PadKind::PlayStation);
        assert_eq!(kind_of_input(steamworks::InputType::SteamDeckController), PadKind::Deck);
        assert_eq!(kind_of_input(steamworks::InputType::PS5Controller), PadKind::PlayStation);
        // the text prompts follow the pad (a family set for the frame wins: the game sets it from
        // Settings and Steam Input; not set in tests, which run side by side)
        assert_eq!(crate::pad::button_name(btn::A, Some("DualSense")), "Cross");
        assert_eq!(crate::pad::button_name(btn::Y, Some("Pro Controller")), "X");
    }

    #[test]
    fn steam_input_actions_press_the_game_buttons() {
        // every action named in a set exists, and every action belongs to a set
        let mut all: Vec<&str> = Vec::new();
        for s in ActionSet::ALL {
            let (d, a) = set_actions(s);
            assert!(!d.is_empty() && !a.is_empty(), "{s:?}");
            all.extend(d);
            all.extend(a.iter().map(|x| x.0));
        }
        for (n, _) in DIGITAL_ACTIONS {
            assert!(all.contains(n), "{n} in a set");
        }
        for n in ANALOG_ACTIONS {
            assert!(all.contains(n), "{n} in a set");
        }
        let p = pad_from_actions(&[("jump", true, btn::A), ("kick", false, btn::RB), ("menu_select", true, btn::A)], &[("move", 0.0, 1.0), ("look", 0.5, 0.0), ("fire", 0.9, 0.0), ("guard", 0.1, 0.0)]);
        assert_eq!(p.held & btn::A, btn::A);
        assert_eq!(p.held & btn::RB, 0);
        assert!(p.ly > 0.99 && p.rx > 0.3 && p.held & btn::RT != 0 && p.held & btn::LT == 0);
        // the right set for the screen
        assert_eq!(set_for(true, false, false, true), ActionSet::Combat);
        assert_eq!(set_for(true, true, false, true), ActionSet::Hub);
        assert_eq!(set_for(true, false, true, true), ActionSet::Spectate);
        assert_eq!(set_for(true, false, false, false), ActionSet::Menu);
        // a fake Steam Input pad: read in the set asked for
        let mut f = FakeSteam::new(1);
        assert!(f.pad(ActionSet::Menu).is_none(), "no Steam Input: the game reads the pad itself");
        f.pad_state = Some((PadKind::Deck, p));
        assert_eq!(f.pad(ActionSet::Combat).map(|x| x.0), Some(PadKind::Deck));
        f.pad(ActionSet::Spectate);
        assert_eq!(f.sets_seen, vec![ActionSet::Combat, ActionSet::Spectate]);
    }

    #[test]
    fn the_manifest_and_deck_layout_cover_every_action() {
        let m = manifest(480);
        for s in ActionSet::ALL {
            assert!(m.contains(&format!("\"{}\"", s.key())));
        }
        for (n, _) in DIGITAL_ACTIONS {
            assert!(m.contains(&format!("\"{n}\"")), "{n}");
        }
        assert_eq!(m.matches('{').count(), m.matches('}').count(), "balanced");
        let deck = controller_config("controller_neptune", "Steam Deck");
        assert!(deck.contains("button_back_left") && deck.contains("right_trackpad") && deck.contains("gyro"));
        assert_eq!(deck.matches('{').count(), deck.matches('}').count());
        let xb = controller_config("controller_xboxone", "Xbox");
        assert!(!xb.contains("button_back_left") && xb.contains("game_action combat_controls jump"));
    }
}
