//! The game's look for menus and the HUD: the two bundled fonts (Rajdhani for titles, buttons
//! and numbers; Inter for reading), the shared panel, button, tab and header style, weapon and
//! item icons drawn as silhouettes, and the full-screen loadout screen.

use crate::game::{Loadout, Sfx, Weapon, ITEM_BLURBS, ITEM_NAMES};
use crate::{chars, ACCENT, UI_SOUND};
use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2};
use std::sync::Arc;

/// The display face (Rajdhani Bold): titles, buttons and every big number.
pub fn display(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("display".into()))
}

/// The reading face (Inter).
pub fn body(size: f32) -> FontId {
    FontId::proportional(size)
}

/// Register both fonts. Anything they don't have (symbols, other scripts) falls back to egui's
/// own fonts.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("display".into(), Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/Rajdhani-Bold.ttf"))));
    fonts.font_data.insert("body".into(), Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/Inter.ttf"))));
    let defaults = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "body".into());
    let mut d = vec!["display".to_string(), "body".to_string()];
    d.extend(defaults);
    fonts.families.insert(FontFamily::Name("display".into()), d);
    ctx.set_fonts(fonts);
}

/// Text sizes, spacing and widget colours shared by every menu: roomy controls, the accent
/// colour on hover and selection, buttons and headings in the display face.
pub fn apply_style(style: &mut egui::Style) {
    use egui::TextStyle;
    style.text_styles.insert(TextStyle::Button, display(21.0));
    style.text_styles.insert(TextStyle::Body, body(15.5));
    style.text_styles.insert(TextStyle::Heading, display(34.0));
    style.text_styles.insert(TextStyle::Small, body(12.5));
    let sp = &mut style.spacing;
    sp.button_padding = Vec2::new(16.0, 7.0);
    sp.item_spacing = Vec2::new(10.0, 9.0);
    sp.interact_size = Vec2::new(44.0, 32.0);
    sp.slider_width = 230.0;
    sp.icon_width = 22.0;
    sp.icon_width_inner = 12.0;
    sp.icon_spacing = 8.0;
    let v = &mut style.visuals;
    v.selection.bg_fill = Color32::from_rgb(176, 88, 36);
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.slider_trailing_fill = true;
    v.extreme_bg_color = Color32::from_rgba_unmultiplied(8, 7, 14, 235);
    let r = egui::CornerRadius::same(5);
    let w = &mut v.widgets;
    w.noninteractive.corner_radius = r;
    w.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_gray(215));
    w.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_white_alpha(18));
    w.inactive.corner_radius = r;
    w.inactive.weak_bg_fill = Color32::from_rgba_unmultiplied(34, 31, 50, 240);
    w.inactive.bg_fill = Color32::from_rgba_unmultiplied(44, 40, 62, 240);
    w.inactive.bg_stroke = Stroke::new(1.0, Color32::from_white_alpha(22));
    w.inactive.fg_stroke = Stroke::new(1.0, Color32::from_gray(228));
    w.hovered.corner_radius = r;
    w.hovered.weak_bg_fill = Color32::from_rgb(78, 48, 34);
    w.hovered.bg_fill = Color32::from_rgb(90, 56, 38);
    w.hovered.bg_stroke = Stroke::new(1.5, ACCENT);
    w.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    w.hovered.expansion = 1.5;
    w.active.corner_radius = r;
    w.active.weak_bg_fill = Color32::from_rgb(150, 76, 34);
    w.active.bg_fill = Color32::from_rgb(170, 86, 38);
    w.active.bg_stroke = Stroke::new(2.0, Color32::from_rgb(255, 190, 130));
    w.active.fg_stroke = Stroke::new(2.0, Color32::WHITE);
    w.active.expansion = 0.5;
    w.open.corner_radius = r;
    w.open.weak_bg_fill = Color32::from_rgb(70, 44, 32);
}

/// Mix two colours.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()), f(a.a(), b.a()))
}

/// Text with a soft drop shadow.
pub fn label(p: &Painter, pos: Pos2, align: Align2, text: &str, font: FontId, col: Color32) {
    let off = (font.size * 0.07).clamp(1.0, 3.0);
    p.text(pos + Vec2::splat(off), align, text, font.clone(), Color32::from_black_alpha((170.0 * col.a() as f32 / 255.0) as u8));
    p.text(pos, align, text, font, col);
}

/// The font shrunk (down to 60%) until `text` fits `max_w`.
pub fn fitted(p: &Painter, text: &str, font: FontId, max_w: f32) -> FontId {
    let mut f = font.clone();
    for _ in 0..8 {
        if p.layout_no_wrap(text.to_string(), f.clone(), Color32::WHITE).size().x <= max_w || f.size < font.size * 0.6 {
            break;
        }
        f.size *= 0.92;
    }
    f
}

pub fn text_width(p: &Painter, text: &str, font: FontId) -> f32 {
    p.layout_no_wrap(text.to_string(), font, Color32::WHITE).size().x
}

fn ui_sound(s: Sfx) {
    UI_SOUND.with(|u| u.set(Some(s)));
}

/// A left-to-right colour fade over `r` (stops are x fractions and colours).
pub fn hgrad(p: &Painter, r: Rect, stops: &[(f32, Color32)]) {
    let mut mesh = egui::epaint::Mesh::default();
    for (k, (t, c)) in stops.iter().enumerate() {
        let x = r.left() + r.width() * t;
        mesh.colored_vertex(Pos2::new(x, r.top()), *c);
        mesh.colored_vertex(Pos2::new(x, r.bottom()), *c);
        if k > 0 {
            let i = k as u32 * 2;
            mesh.add_triangle(i - 2, i - 1, i + 1);
            mesh.add_triangle(i - 2, i + 1, i);
        }
    }
    p.add(Shape::mesh(mesh));
}

/// A top-to-bottom colour fade over `r`.
pub fn vgrad(p: &Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut m = egui::epaint::Mesh::default();
    m.colored_vertex(r.left_top(), top);
    m.colored_vertex(r.right_top(), top);
    m.colored_vertex(r.left_bottom(), bottom);
    m.colored_vertex(r.right_bottom(), bottom);
    m.add_triangle(0, 1, 3);
    m.add_triangle(0, 3, 2);
    p.add(Shape::mesh(m));
}

/// The menu panel look: dark and see-through, an accent edge down the left and a glow along
/// the top.
pub fn panel_frame<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let res = egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(12, 11, 21, 232))
        .corner_radius(6.0)
        .inner_margin(egui::Margin { left: 28, right: 22, top: 20, bottom: 20 })
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(16)))
        .shadow(egui::Shadow { offset: [0, 8], blur: 26, spread: 0, color: Color32::from_black_alpha(110) })
        .show(ui, add);
    edge_marks(ui.painter(), res.response.rect, 1.0, false);
    res.inner
}

/// The accent edge and top glow of a panel (on the right side when `right`).
pub fn edge_marks(p: &Painter, r: Rect, s: f32, right: bool) {
    let w = 4.0 * s;
    let bar = if right { Rect::from_min_size(Pos2::new(r.right() - w, r.top()), Vec2::new(w, r.height())) } else { Rect::from_min_size(r.min, Vec2::new(w, r.height())) };
    p.rect_filled(bar, 2.0 * s, ACCENT);
    let glow = Rect::from_min_size(r.min, Vec2::new(r.width(), 2.0 * s));
    let (a, b) = (ACCENT.gamma_multiply(0.85), ACCENT.gamma_multiply(0.0));
    if right {
        hgrad(p, glow, &[(0.0, b), (0.35, b), (1.0, a)]);
    } else {
        hgrad(p, glow, &[(0.0, a), (0.65, b), (1.0, b)]);
    }
}

/// A large menu button: slides and lights up on hover or when selected with a controller,
/// sinks when pressed.
pub fn menu_button(ui: &mut egui::Ui, text: &str, h: f32) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let ctx = ui.ctx().clone();
    let hot = ctx.animate_bool_with_time(resp.id.with("hot"), resp.hovered() || resp.has_focus(), 0.12);
    let press = ctx.animate_bool_with_time(resp.id.with("press"), resp.is_pointer_button_down_on(), 0.05);
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let r = rect.shrink(2.0 * press);
        let fill = mix(Color32::from_rgba_unmultiplied(30, 27, 44, 235), Color32::from_rgba_unmultiplied(84, 48, 32, 245), hot);
        p.rect_filled(r, 5.0, fill);
        if hot > 0.01 {
            // light sweeping in from the left
            hgrad(p, r.shrink(1.0), &[(0.0, ACCENT.gamma_multiply(0.28 * hot)), (0.6, ACCENT.gamma_multiply(0.0)), (1.0, ACCENT.gamma_multiply(0.0))]);
        }
        p.rect_stroke(r, 5.0, Stroke::new(1.0 + hot, mix(Color32::from_white_alpha(24), ACCENT, hot)), StrokeKind::Inside);
        p.rect_filled(Rect::from_min_size(r.min, Vec2::new(3.0 + 3.0 * hot, r.height())), 2.0, ACCENT.gamma_multiply(0.5 + 0.5 * hot));
        let font = fitted(p, &text.to_uppercase(), display(h * 0.5), r.width() - 60.0);
        label(p, Pos2::new(r.left() + 20.0 + 6.0 * hot, r.center().y + 1.0), Align2::LEFT_CENTER, &text.to_uppercase(), font, mix(Color32::from_gray(222), Color32::WHITE, hot));
        if hot > 0.01 {
            let c = Pos2::new(r.right() - 20.0 + 4.0 * hot, r.center().y);
            let k = 6.0;
            let col = ACCENT.gamma_multiply(hot);
            p.line_segment([c + Vec2::new(-k * 0.6, -k), c], Stroke::new(2.5, col));
            p.line_segment([c + Vec2::new(-k * 0.6, k), c], Stroke::new(2.5, col));
        }
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, text));
    resp
}

/// The main menu's big PLAY button: accent-filled, with a line under the word.
pub fn play_button(ui: &mut egui::Ui, title: &str, sub: &str, h: f32) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let ctx = ui.ctx().clone();
    let hot = ctx.animate_bool_with_time(resp.id.with("hot"), resp.hovered() || resp.has_focus(), 0.12);
    let press = ctx.animate_bool_with_time(resp.id.with("press"), resp.is_pointer_button_down_on(), 0.05);
    let breathe = 0.5 + 0.5 * (ctx.input(|i| i.time) as f32 * 2.2).sin();
    let p = ui.painter();
    let r = rect.shrink(3.0 * press);
    for (e, a) in [(12.0, 0.08), (6.0, 0.16)] {
        p.rect_stroke(r.expand(e), 8.0 + e, Stroke::new(3.0, ACCENT.gamma_multiply(a * (0.5 + 0.3 * breathe + 0.6 * hot))), StrokeKind::Outside);
    }
    p.rect_filled(r, 7.0, mix(Color32::from_rgb(214, 108, 42), Color32::from_rgb(255, 156, 76), hot));
    vgrad(p, r.shrink(1.0), Color32::from_white_alpha(46), Color32::from_black_alpha(36));
    p.rect_stroke(r, 7.0, Stroke::new(1.5 + hot, Color32::from_rgba_unmultiplied(255, 222, 186, 210)), StrokeKind::Inside);
    let dark = Color32::from_rgb(28, 14, 6);
    let x = r.left() + 24.0 + 6.0 * hot;
    p.text(Pos2::new(x, r.center().y - h * 0.08), Align2::LEFT_CENTER, title, display(h * 0.62), dark);
    p.text(Pos2::new(x + 2.0, r.bottom() - h * 0.17), Align2::LEFT_CENTER, sub, fitted(p, sub, body(13.5), r.width() - 60.0), Color32::from_rgba_unmultiplied(40, 20, 8, 220));
    // an arrow on the right
    let c = Pos2::new(r.right() - 30.0 + 6.0 * hot, r.center().y);
    p.add(Shape::convex_polygon(vec![c + Vec2::new(-9.0, -14.0), c + Vec2::new(9.0, 0.0), c + Vec2::new(-9.0, 14.0)], dark, Stroke::NONE));
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, title));
    resp
}

/// A mode in the PLAY picker: a big card with its name and one line about it.
pub fn mode_card(ui: &mut egui::Ui, w: f32, h: f32, title: &str, desc: &str, enabled: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let resp = if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp };
    let ctx = ui.ctx().clone();
    let hot = if enabled { ctx.animate_bool_with_time(resp.id.with("hot"), resp.hovered() || resp.has_focus(), 0.1) } else { 0.0 };
    let press = ctx.animate_bool_with_time(resp.id.with("press"), resp.is_pointer_button_down_on() && enabled, 0.05);
    let p = ui.painter();
    let r = rect.shrink(2.0 * press);
    if hot > 0.01 {
        for (e, a) in [(6.0, 0.12), (3.0, 0.25)] {
            p.rect_stroke(r.expand(e * hot), 6.0 + e, Stroke::new(2.5, ACCENT.gamma_multiply(a * hot)), StrokeKind::Outside);
        }
    }
    p.rect_filled(r, 6.0, mix(Color32::from_rgba_unmultiplied(28, 25, 40, 240), Color32::from_rgba_unmultiplied(78, 46, 32, 246), hot));
    if hot > 0.01 {
        hgrad(p, r.shrink(1.0), &[(0.0, ACCENT.gamma_multiply(0.25 * hot)), (0.7, ACCENT.gamma_multiply(0.0)), (1.0, ACCENT.gamma_multiply(0.0))]);
    }
    p.rect_filled(Rect::from_min_size(r.min, Vec2::new(4.0 + 2.0 * hot, r.height())), 2.0, if enabled { ACCENT } else { Color32::from_gray(90) });
    p.rect_stroke(r, 6.0, Stroke::new(1.0 + hot, mix(Color32::from_white_alpha(24), ACCENT, hot)), StrokeKind::Inside);
    let ink = if enabled { mix(Color32::from_gray(230), Color32::WHITE, hot) } else { Color32::from_gray(120) };
    let x = r.left() + 18.0 + 4.0 * hot;
    let tt = title.to_uppercase();
    label(p, Pos2::new(x, r.center().y + 2.0), Align2::LEFT_BOTTOM, &tt, fitted(p, &tt, display(25.0), r.width() - 30.0), ink);
    p.text(Pos2::new(x, r.center().y + 5.0), Align2::LEFT_TOP, desc, fitted(p, desc, body(12.5), r.width() - 30.0), if enabled { Color32::from_gray(190) } else { Color32::from_gray(115) });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, title));
    resp
}
/// A row of big tabs. Returns true when one was picked.
pub fn tabs(ui: &mut egui::Ui, cur: &mut u8, names: &[&str]) -> bool {
    let n = names.len().max(1) as f32;
    let gap = 6.0;
    let w = ((ui.available_width() - gap * (n - 1.0)) / n).max(60.0);
    let mut picked = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (k, name) in names.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 40.0), Sense::click());
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() && *cur != k as u8 {
                *cur = k as u8;
                picked = true;
                ui_sound(Sfx::UiClick);
            }
            let sel = *cur == k as u8;
            let hot = ui.ctx().animate_bool_with_time(resp.id.with("hot"), resp.hovered() || resp.has_focus(), 0.1);
            let p = ui.painter();
            let fill = if sel { Color32::from_rgba_unmultiplied(96, 54, 32, 240) } else { mix(Color32::from_rgba_unmultiplied(28, 25, 40, 230), Color32::from_rgba_unmultiplied(52, 40, 44, 240), hot) };
            p.rect_filled(rect, 4.0, fill);
            let bar_h = if sel { 3.0 } else { 2.0 * hot };
            if bar_h > 0.1 {
                p.rect_filled(Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - bar_h), rect.max), 1.0, if sel { ACCENT } else { ACCENT.gamma_multiply(0.6) });
            }
            let font = fitted(p, &name.to_uppercase(), display(20.0), w - 12.0);
            label(p, rect.center() + Vec2::new(0.0, 1.0), Align2::CENTER_CENTER, &name.to_uppercase(), font, if sel { Color32::WHITE } else { mix(Color32::from_gray(185), Color32::WHITE, hot) });
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, sel, *name));
        }
    });
    picked
}

/// A section header in the display face, with a rule out to the right.
pub fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    let r = ui.label(egui::RichText::new(text.to_uppercase()).font(display(21.0)).color(ACCENT));
    let y = r.rect.center().y;
    let (x0, x1) = (r.rect.right() + 10.0, ui.max_rect().right());
    if x1 > x0 + 8.0 {
        ui.painter().line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], Stroke::new(1.0, ACCENT.gamma_multiply(0.3)));
    }
}

/// A keycap with a key or button name in it, centred at `c`.
pub fn keycap(p: &Painter, c: Pos2, key: &str, s: f32) -> Rect {
    let font = display(13.0 * s);
    let w = (text_width(p, key, font.clone()) + 10.0 * s).max(18.0 * s);
    let r = Rect::from_center_size(c, Vec2::new(w, 18.0 * s));
    p.rect_filled(r, 3.0 * s, Color32::from_rgba_unmultiplied(235, 228, 220, 230));
    p.rect_filled(Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 2.0 * s), r.max), 2.0 * s, Color32::from_rgba_unmultiplied(120, 110, 100, 230));
    p.text(c + Vec2::new(0.0, -0.5 * s), Align2::CENTER_CENTER, key, font, Color32::from_rgb(24, 18, 14));
    r
}

// ------------------------------------------------------------------ icons

/// A weapon's silhouette, pointing right, centred in `r`.
pub fn weapon_icon(p: &Painter, r: Rect, w: Weapon, col: Color32) {
    let aspect = 2.6;
    let bw = r.width().min(r.height() * aspect);
    let b = Rect::from_center_size(r.center(), Vec2::new(bw, bw / aspect));
    let pt = |x: f32, y: f32| Pos2::new(b.left() + x * b.width(), b.top() + y * b.height());
    let poly = |pts: &[(f32, f32)]| {
        p.add(Shape::convex_polygon(pts.iter().map(|&(x, y)| pt(x, y)).collect(), col, Stroke::NONE));
    };
    let bx = |x0: f32, y0: f32, x1: f32, y1: f32| poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)]);
    let dot = |x: f32, y: f32, rr: f32| p.circle_filled(pt(x, y), rr * b.height(), col);
    let dark = Color32::from_black_alpha(150);
    let line = |a: (f32, f32), c: (f32, f32), th: f32| {
        p.line_segment([pt(a.0, a.1), pt(c.0, c.1)], Stroke::new(th * b.height(), dark));
    };
    match w {
        Weapon::Dagger => {
            dot(0.18, 0.5, 0.07);
            bx(0.18, 0.44, 0.38, 0.56);
            bx(0.38, 0.30, 0.42, 0.70);
            poly(&[(0.42, 0.40), (0.74, 0.45), (0.84, 0.5), (0.74, 0.55), (0.42, 0.60)]);
            line((0.44, 0.5), (0.72, 0.5), 0.025);
        }
        Weapon::Katana => {
            bx(0.02, 0.45, 0.27, 0.55);
            for k in 0..5 {
                let x = 0.05 + k as f32 * 0.045;
                line((x, 0.45), (x + 0.02, 0.55), 0.02);
            }
            p.add(Shape::ellipse_filled(pt(0.285, 0.5), Vec2::new(0.018 * b.width(), 0.17 * b.height()), col));
            poly(&[(0.30, 0.45), (0.64, 0.42), (0.64, 0.53), (0.30, 0.55)]);
            poly(&[(0.64, 0.42), (0.95, 0.34), (1.0, 0.36), (0.64, 0.53)]);
        }
        Weapon::Greatsword => {
            dot(0.03, 0.5, 0.09);
            bx(0.04, 0.44, 0.24, 0.56);
            bx(0.24, 0.18, 0.29, 0.82);
            poly(&[(0.29, 0.36), (0.86, 0.36), (0.99, 0.5), (0.86, 0.64), (0.29, 0.64)]);
            line((0.31, 0.5), (0.84, 0.5), 0.04);
        }
        Weapon::Shotgun => {
            poly(&[(0.0, 0.42), (0.22, 0.36), (0.22, 0.58), (0.03, 0.72), (0.0, 0.70)]);
            bx(0.22, 0.34, 0.44, 0.58);
            bx(0.44, 0.36, 0.98, 0.46);
            bx(0.50, 0.47, 0.76, 0.58);
            poly(&[(0.25, 0.56), (0.34, 0.56), (0.30, 0.76), (0.21, 0.76)]);
            line((0.53, 0.52), (0.73, 0.52), 0.03);
        }
        Weapon::Revolver => {
            bx(0.46, 0.30, 0.90, 0.40);
            bx(0.86, 0.24, 0.89, 0.31);
            bx(0.30, 0.26, 0.48, 0.48);
            p.rect_filled(Rect::from_min_max(pt(0.33, 0.30), pt(0.50, 0.54)), 0.06 * b.height(), col);
            poly(&[(0.27, 0.26), (0.31, 0.18), (0.34, 0.26)]);
            poly(&[(0.28, 0.44), (0.40, 0.46), (0.34, 0.84), (0.20, 0.82)]);
            line((0.35, 0.42), (0.48, 0.42), 0.03);
        }
        Weapon::HandCannon => {
            bx(0.46, 0.24, 0.96, 0.44);
            bx(0.90, 0.16, 0.94, 0.25);
            bx(0.28, 0.22, 0.48, 0.50);
            p.rect_filled(Rect::from_min_max(pt(0.31, 0.26), pt(0.52, 0.60)), 0.07 * b.height(), col);
            poly(&[(0.24, 0.24), (0.29, 0.12), (0.33, 0.24)]);
            poly(&[(0.25, 0.46), (0.40, 0.50), (0.33, 0.92), (0.14, 0.90)]);
            line((0.50, 0.34), (0.94, 0.34), 0.03);
        }
        Weapon::Smg => {
            bx(0.05, 0.36, 0.25, 0.41);
            bx(0.05, 0.36, 0.09, 0.58);
            bx(0.25, 0.30, 0.70, 0.52);
            bx(0.70, 0.37, 0.88, 0.45);
            poly(&[(0.43, 0.52), (0.52, 0.52), (0.55, 0.90), (0.46, 0.90)]);
            poly(&[(0.28, 0.52), (0.37, 0.52), (0.34, 0.76), (0.25, 0.76)]);
            bx(0.33, 0.24, 0.42, 0.30);
        }
        Weapon::Rifle => {
            poly(&[(0.0, 0.36), (0.25, 0.36), (0.25, 0.54), (0.03, 0.64), (0.0, 0.62)]);
            bx(0.25, 0.34, 0.62, 0.54);
            bx(0.55, 0.36, 0.82, 0.50);
            bx(0.82, 0.40, 0.99, 0.46);
            bx(0.38, 0.24, 0.50, 0.34);
            poly(&[(0.44, 0.54), (0.53, 0.54), (0.58, 0.82), (0.49, 0.84)]);
            poly(&[(0.28, 0.54), (0.36, 0.54), (0.33, 0.76), (0.25, 0.76)]);
            line((0.58, 0.43), (0.80, 0.43), 0.03);
        }
        Weapon::Sniper => {
            poly(&[(0.0, 0.40), (0.22, 0.40), (0.22, 0.56), (0.04, 0.66), (0.0, 0.64)]);
            bx(0.22, 0.40, 0.56, 0.56);
            bx(0.56, 0.44, 1.0, 0.50);
            bx(0.31, 0.26, 0.54, 0.35);
            bx(0.28, 0.22, 0.33, 0.38);
            bx(0.52, 0.22, 0.58, 0.38);
            bx(0.38, 0.35, 0.42, 0.40);
            poly(&[(0.25, 0.56), (0.33, 0.56), (0.30, 0.78), (0.22, 0.78)]);
            line((0.70, 0.50), (0.62, 0.80), 0.03);
            line((0.70, 0.50), (0.78, 0.80), 0.03);
        }
        Weapon::Rocket => {
            bx(0.06, 0.32, 0.88, 0.56);
            poly(&[(0.0, 0.36), (0.06, 0.32), (0.06, 0.56), (0.0, 0.52)]);
            poly(&[(0.86, 0.28), (0.96, 0.24), (0.96, 0.64), (0.86, 0.60)]);
            bx(0.36, 0.20, 0.50, 0.32);
            poly(&[(0.30, 0.56), (0.39, 0.56), (0.36, 0.80), (0.27, 0.80)]);
            poly(&[(0.52, 0.56), (0.60, 0.56), (0.58, 0.74), (0.50, 0.74)]);
            line((0.12, 0.44), (0.84, 0.44), 0.03);
        }
        Weapon::Shuriken => {
            let c = b.center();
            let h = b.height() * 0.95;
            for k in 0..4 {
                let a = k as f32 * std::f32::consts::FRAC_PI_2 + 0.35;
                let d = |t: f32, len: f32| c + Vec2::new((a + t).cos(), (a + t).sin()) * len * h;
                p.add(Shape::convex_polygon(vec![d(0.0, 0.5), d(1.15, 0.16), d(-0.5, 0.12)], col, Stroke::NONE));
            }
            p.circle_filled(c, 0.17 * h, col);
            p.circle_filled(c, 0.07 * h, dark);
        }
    }
}

/// An item's icon (0 medkit, 1 armour kit, 2 grenades), centred in `r`.
pub fn item_icon(p: &Painter, r: Rect, item: u8, col: Color32) {
    let s = r.width().min(r.height());
    let b = Rect::from_center_size(r.center(), Vec2::splat(s));
    let pt = |x: f32, y: f32| Pos2::new(b.left() + x * s, b.top() + y * s);
    let a = col.a() as f32 / 255.0;
    match item {
        0 => {
            p.rect_stroke(Rect::from_min_max(pt(0.38, 0.12), pt(0.62, 0.28)), 0.04 * s, Stroke::new(0.06 * s, col), StrokeKind::Inside);
            p.rect_filled(Rect::from_min_max(pt(0.14, 0.24), pt(0.86, 0.86)), 0.1 * s, col);
            let red = Color32::from_rgb(214, 52, 52).gamma_multiply(a);
            p.rect_filled(Rect::from_min_max(pt(0.43, 0.36), pt(0.57, 0.76)), 0.02 * s, red);
            p.rect_filled(Rect::from_min_max(pt(0.30, 0.49), pt(0.70, 0.63)), 0.02 * s, red);
        }
        1 => {
            let shield = |k: f32| vec![pt(0.18 + k, 0.12 + k), pt(0.82 - k, 0.12 + k), pt(0.82 - k, 0.50), pt(0.5, 0.92 - k * 1.4), pt(0.18 + k, 0.50)];
            p.add(Shape::convex_polygon(shield(0.0), col, Stroke::NONE));
            p.add(Shape::convex_polygon(shield(0.1), Color32::from_rgb(70, 130, 220).gamma_multiply(a), Stroke::NONE));
            p.add(Shape::convex_polygon(vec![pt(0.5, 0.24), pt(0.62, 0.24), pt(0.62, 0.50), pt(0.5, 0.74)], col.gamma_multiply(0.7), Stroke::NONE));
        }
        _ => {
            p.circle_filled(pt(0.48, 0.60), 0.29 * s, col);
            p.rect_filled(Rect::from_min_max(pt(0.38, 0.20), pt(0.58, 0.36)), 0.03 * s, col);
            p.add(Shape::convex_polygon(vec![pt(0.56, 0.22), pt(0.82, 0.30), pt(0.80, 0.38), pt(0.56, 0.32)], col, Stroke::NONE));
            p.circle_stroke(pt(0.30, 0.18), 0.09 * s, Stroke::new(0.05 * s, col));
            let dark = Color32::from_black_alpha(110);
            p.line_segment([pt(0.24, 0.60), pt(0.72, 0.60)], Stroke::new(0.04 * s, dark));
            p.line_segment([pt(0.48, 0.36), pt(0.48, 0.86)], Stroke::new(0.04 * s, dark));
        }
    }
}

/// A health cross for the HUD.
pub fn cross_icon(p: &Painter, c: Pos2, r: f32, col: Color32) {
    p.rect_filled(Rect::from_center_size(c, Vec2::new(r * 2.0, r * 0.75)), r * 0.15, col);
    p.rect_filled(Rect::from_center_size(c, Vec2::new(r * 0.75, r * 2.0)), r * 0.15, col);
}

/// An armour shield for the HUD.
pub fn shield_icon(p: &Painter, c: Pos2, r: f32, col: Color32) {
    let pts = vec![c + Vec2::new(-r * 0.85, -r), c + Vec2::new(r * 0.85, -r), c + Vec2::new(r * 0.85, r * 0.1), c + Vec2::new(0.0, r), c + Vec2::new(-r * 0.85, r * 0.1)];
    p.add(Shape::convex_polygon(pts, col, Stroke::NONE));
}

/// A bar in segments (`segs` of them), for health, armour and weapon stats.
pub fn seg_bar(p: &Painter, r: Rect, frac: f32, segs: usize, on: Color32, off: Color32) {
    let segs = segs.max(1);
    let gap = (r.height() * 0.22).clamp(1.0, 3.0);
    let w = (r.width() - gap * (segs as f32 - 1.0)) / segs as f32;
    let fill = frac.clamp(0.0, 1.0) * segs as f32;
    for k in 0..segs {
        let x = r.left() + k as f32 * (w + gap);
        let cell = Rect::from_min_size(Pos2::new(x, r.top()), Vec2::new(w, r.height()));
        p.rect_filled(cell, 1.0, off);
        let part = (fill - k as f32).clamp(0.0, 1.0);
        if part > 0.0 {
            p.rect_filled(Rect::from_min_size(cell.min, Vec2::new(w * part, r.height())), 1.0, on);
        }
    }
}

/// How each weapon compares, 0..1: damage, reach or range, and speed or fire rate.
pub fn weapon_stats(w: Weapon) -> [(&'static str, f32); 3] {
    let (a, b, c) = match w {
        Weapon::Dagger => (0.35, 0.3, 1.0),
        Weapon::Katana => (0.6, 0.6, 0.7),
        Weapon::Greatsword => (1.0, 0.95, 0.35),
        Weapon::Shotgun => (0.95, 0.25, 0.25),
        Weapon::Revolver => (0.7, 0.75, 0.55),
        Weapon::HandCannon => (0.85, 0.7, 0.45),
        Weapon::Smg => (0.3, 0.45, 1.0),
        Weapon::Rifle => (0.6, 0.85, 0.5),
        Weapon::Sniper => (1.0, 1.0, 0.15),
        Weapon::Rocket => (0.9, 0.5, 0.2),
        Weapon::Shuriken => (0.6, 0.55, 0.6),
    };
    if w.is_melee() {
        [("DMG", a), ("REACH", b), ("SPEED", c)]
    } else {
        [("DMG", a), ("RANGE", b), ("RATE", c)]
    }
}

// ------------------------------------------------------------------ the loadout screen

/// What the loadout screen remembers between frames.
#[derive(Default)]
pub struct LoadoutUi {
    /// Which slot the preview fighter holds (0 blade, 1 gun 1, 2 gun 2).
    pub slot: u8,
    /// The last thing picked (name and description), shown when nothing is pointed at.
    pub last: Option<(String, String)>,
    /// Which gun slot the gun grid fills (0 gun 1, 1 gun 2).
    pub gun_slot: u8,
}

/// How the loadout screen is set up this time.
pub struct LoadoutView<'a> {
    /// Seconds left before the match starts (none mid-match).
    pub timer: Option<f32>,
    /// Gladiator: guns can't be used.
    pub melee_only: bool,
    /// A controller is in use (it gets a selected card at once).
    pub pad: bool,
    /// LB / RB this frame: step through the fighters.
    pub tab_step: i32,
    /// Key or button hints for the blade, gun 1 and gun 2.
    pub keys: [String; 3],
    /// The controls line, top right.
    pub hint: String,
    pub subtitle: &'a str,
    /// The big button's words.
    pub action: &'a str,
    /// Also offer a Back button.
    pub back: bool,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum LoadoutAct {
    None,
    Ready,
    Back,
}

struct CardLook {
    selected: bool,
    dim: bool,
    tag: Option<&'static str>,
    /// A small label in the top-right corner instead of the check mark (lit: accent-filled).
    badge: Option<(&'static str, bool)>,
}

/// One clickable, selectable card. `content` draws inside it with the ink colour.
fn card(ui: &mut egui::Ui, p: &Painter, r: Rect, id: egui::Id, look: &CardLook, k: f32, content: impl FnOnce(&Painter, Rect, Color32, bool)) -> egui::Response {
    let resp = ui.interact(r, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let ctx = ui.ctx().clone();
    let hot = ctx.animate_bool_with_time(id.with("hot"), resp.hovered() || resp.has_focus(), 0.1);
    let press = ctx.animate_bool_with_time(id.with("press"), resp.is_pointer_button_down_on(), 0.05);
    let r = r.shrink(2.5 * k * press);
    let rad = 6.0 * k;
    if hot > 0.01 {
        for (e, a) in [(7.0, 0.10), (4.0, 0.18), (1.5, 0.35)] {
            p.rect_stroke(r.expand(e * k * hot), rad + e * k, Stroke::new(2.5 * k, ACCENT.gamma_multiply(a * hot)), StrokeKind::Outside);
        }
    }
    let base = Color32::from_rgba_unmultiplied(20, 18, 31, 238);
    let fill = if look.selected { Color32::from_rgba_unmultiplied(66, 40, 28, 246) } else { mix(base, Color32::from_rgba_unmultiplied(44, 35, 46, 245), hot) };
    p.rect_filled(r, rad, fill);
    if look.selected {
        vgrad(p, r.shrink(1.0), ACCENT.gamma_multiply(0.0), ACCENT.gamma_multiply(0.16));
    }
    let ink = if look.selected { Color32::WHITE } else { mix(Color32::from_gray(196), Color32::WHITE, hot) };
    content(p, r, ink, look.selected || hot > 0.5);
    if look.dim {
        p.rect_filled(r, rad, Color32::from_black_alpha(150));
        if let Some(t) = look.tag {
            let font = display(13.0 * k);
            let w = text_width(p, t, font.clone()) + 12.0 * k;
            let tr = Rect::from_center_size(r.center(), Vec2::new(w, 18.0 * k));
            p.rect_filled(tr, 3.0 * k, Color32::from_rgba_unmultiplied(30, 26, 36, 235));
            p.rect_stroke(tr, 3.0 * k, Stroke::new(1.0, Color32::from_white_alpha(60)), StrokeKind::Inside);
            p.text(tr.center(), Align2::CENTER_CENTER, t, font, Color32::from_gray(215));
        }
    }
    let edge = if look.selected { Stroke::new(2.5 * k, ACCENT) } else { Stroke::new(1.0 + hot, mix(Color32::from_white_alpha(26), ACCENT, hot * 0.85)) };
    p.rect_stroke(r, rad, edge, StrokeKind::Inside);
    if let Some((b, lit)) = look.badge {
        let font = display(12.5 * k);
        let w = text_width(p, b, font.clone()) + 10.0 * k;
        let br = Rect::from_min_size(Pos2::new(r.right() - 5.0 * k - w, r.top() + 5.0 * k), Vec2::new(w, 16.0 * k));
        p.rect_filled(br, 3.0 * k, if lit { ACCENT } else { Color32::from_rgba_unmultiplied(60, 52, 66, 240) });
        p.text(br.center() + Vec2::new(0.0, 0.5 * k), Align2::CENTER_CENTER, b, font, if lit { Color32::from_rgb(34, 18, 8) } else { Color32::from_gray(225) });
    } else if look.selected {
        let c = Pos2::new(r.right() - 11.0 * k, r.top() + 11.0 * k);
        p.circle_filled(c, 8.0 * k, ACCENT);
        p.add(Shape::line(vec![c + Vec2::new(-4.0, 0.2) * k, c + Vec2::new(-1.2, 3.0) * k, c + Vec2::new(4.0, -3.2) * k], Stroke::new(2.0 * k, Color32::from_rgb(34, 18, 8))));
    }
    resp
}

fn weapon_card(p: &Painter, r: Rect, w: Weapon, ink: Color32, lit: bool, k: f32) {
    let inner = r.shrink(8.0 * k);
    let name = w.name().to_uppercase();
    let font = fitted(p, &name, display(19.0 * k), inner.width() - 18.0 * k);
    label(p, inner.left_top() + Vec2::new(0.0, -2.0 * k), Align2::LEFT_TOP, &name, font, ink);
    let row_h = (12.0 * k).min(inner.height() * 0.15);
    let stats_h = row_h * 3.0;
    let icon = Rect::from_min_max(Pos2::new(inner.left(), inner.top() + 19.0 * k), Pos2::new(inner.right(), inner.bottom() - stats_h - 3.0 * k));
    if icon.height() > 6.0 {
        weapon_icon(p, icon.shrink(2.0 * k), w, ink.gamma_multiply(0.92));
    }
    let on = if lit { ACCENT } else { Color32::from_gray(185) };
    for (i, (lab, v)) in weapon_stats(w).iter().enumerate() {
        let y = inner.bottom() - stats_h + i as f32 * row_h + row_h * 0.5;
        p.text(Pos2::new(inner.left(), y), Align2::LEFT_CENTER, *lab, display(11.5 * k), Color32::from_gray(160));
        let bar = Rect::from_min_max(Pos2::new(inner.left() + 38.0 * k, y - row_h * 0.22), Pos2::new(inner.right(), y + row_h * 0.22));
        seg_bar(p, bar, *v, 10, on, Color32::from_white_alpha(22));
    }
}

fn item_card(p: &Painter, r: Rect, item: u8, ink: Color32, k: f32) {
    let inner = r.shrink2(Vec2::new(10.0 * k, 6.0 * k));
    let ic = Rect::from_min_size(inner.min, Vec2::splat(inner.height()));
    item_icon(p, ic.shrink(2.0 * k), item, ink);
    let x = ic.right() + 10.0 * k;
    let room = inner.right() - x - 16.0 * k;
    let name = ITEM_NAMES[item as usize].to_uppercase();
    label(p, Pos2::new(x, inner.center().y + 1.0 * k), Align2::LEFT_BOTTOM, &name, fitted(p, &name, display(20.0 * k), room), ink);
    let blurb = ITEM_BLURBS[item as usize];
    p.text(Pos2::new(x, inner.center().y + 2.0 * k), Align2::LEFT_TOP, blurb, fitted(p, blurb, body(11.5 * k), room), Color32::from_gray(175));
}

/// The loadout screen: three columns of cards (Blade | Gun 1 | Gun 2) and an item row on the
/// left, nothing that scrolls; the fighter (shown in 3D by the game behind it) and the big
/// button on the right. Every card can be reached with the arrow keys or a controller.
pub fn loadout_screen(ctx: &egui::Context, rect: Rect, lo: &mut Loadout, st: &mut LoadoutUi, v: &LoadoutView) -> LoadoutAct {
    let k = (rect.height() / 720.0).min(rect.width() / 1280.0 * 1.15).clamp(0.7, 1.6);
    let mut act = LoadoutAct::None;
    if v.tab_step != 0 {
        lo.body = (lo.body as i32 + v.tab_step).rem_euclid(chars::COUNT as i32) as u8;
        ui_sound(Sfx::UiClick);
    }
    let click = |st: &mut LoadoutUi, slot: Option<u8>, name: &str, blurb: &str| {
        if let Some(s) = slot {
            st.slot = s;
        }
        st.last = Some((name.to_string(), blurb.to_string()));
        ui_sound(Sfx::UiClick);
    };
    egui::Area::new(egui::Id::unique("loadout-screen")).fixed_pos(rect.min).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.set_min_size(rect.size());
        let p = ui.painter().clone();
        let split = rect.left() + rect.width() * 0.64;
        let m = 20.0 * k;
        // dark behind the cards, fading out into the 3D fighter on the right
        let fade_end = ((split + 150.0 * k - rect.left()) / rect.width()).min(1.0);
        let mid = ((split - 40.0 * k - rect.left()) / rect.width()).min(fade_end);
        hgrad(&p, rect, &[(0.0, Color32::from_rgba_unmultiplied(8, 7, 14, 236)), (mid, Color32::from_rgba_unmultiplied(8, 7, 14, 214)), (fade_end, Color32::from_rgba_unmultiplied(8, 7, 14, 0)), (1.0, Color32::from_rgba_unmultiplied(8, 7, 14, 0))]);
        vgrad(&p, Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 170.0 * k), rect.max), Color32::from_black_alpha(0), Color32::from_black_alpha(170));
        let l = Rect::from_min_max(rect.min + Vec2::splat(m), Pos2::new(split - m * 0.5, rect.bottom() - m));
        let mut y = l.top();
        // title
        label(&p, Pos2::new(l.left(), y - 6.0 * k), Align2::LEFT_TOP, "LOADOUT", display(46.0 * k), Color32::WHITE);
        let tw = text_width(&p, "LOADOUT", display(46.0 * k));
        p.rect_filled(Rect::from_min_size(Pos2::new(l.left(), y + 40.0 * k), Vec2::new(tw * 0.5, 3.0 * k)), 1.0, ACCENT);
        p.text(Pos2::new(l.left() + tw + 16.0 * k, y + 30.0 * k), Align2::LEFT_BOTTOM, v.subtitle, fitted(&p, v.subtitle, body(14.0 * k), l.right() - l.left() - tw - 16.0 * k), Color32::from_gray(190));
        y += 52.0 * k;
        let fighter_y = y;
        y += 40.0 * k;
        // headers: the blade column, and the guns (two slots over one grid)
        let item_h = 60.0 * k;
        let det_h = 26.0 * k;
        let g = 8.0 * k;
        let cg = 14.0 * k;
        let u = (l.width() - 3.0 * g - cg) / 5.0;
        let bx = l.left();
        let gx = l.left() + u + cg;
        let gw = 4.0 * u + 3.0 * g;
        for (x, w, name) in [(bx, u, "BLADE"), (gx, gw, "GUNS")] {
            p.rect_filled(Rect::from_min_size(Pos2::new(x, y + 3.0 * k), Vec2::new(3.0 * k, 16.0 * k)), 1.0, ACCENT);
            label(&p, Pos2::new(x + 9.0 * k, y + 11.0 * k), Align2::LEFT_CENTER, name, display(21.0 * k), Color32::WHITE);
            if name == "BLADE" {
                let kw = text_width(&p, &v.keys[0], display(13.0 * k)) + 10.0 * k;
                keycap(&p, Pos2::new(x + w - kw.max(18.0 * k) * 0.5, y + 11.0 * k), &v.keys[0], k);
            } else {
                let hw = text_width(&p, "GUNS", display(21.0 * k));
                p.text(Pos2::new(x + 18.0 * k + hw, y + 12.0 * k), Align2::LEFT_CENTER, "pick a slot, then a gun to fill it", body(12.5 * k), Color32::from_gray(165));
            }
        }
        y += 28.0 * k;
        let grid_bottom = l.bottom() - item_h - det_h - g * 2.0;
        let gh = (grid_bottom - y).max(120.0 * k);
        let mut hover: Option<(String, String)> = None;
        let mut first_id = None;
        // blades
        let mh = (gh - 2.0 * g) / 3.0;
        for (i, w) in Weapon::MELEE.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(bx, y + i as f32 * (mh + g)), Vec2::new(u, mh));
            let id = ui.scope_id().with(("lo-melee", i));
            let look = CardLook { selected: lo.melee == *w, dim: false, tag: None, badge: None };
            if look.selected {
                first_id = Some(id);
            }
            let resp = card(ui, &p, r, id, &look, k, |p, r, ink, lit| weapon_card(p, r, *w, ink, lit, k));
            if resp.hovered() || resp.has_focus() {
                hover = Some((w.name().to_string(), w.blurb().to_string()));
            }
            if resp.clicked() {
                lo.melee = *w;
                click(st, Some(0), w.name(), w.blurb());
            }
        }
        // the two gun slots: pick one, then a gun below fills it
        let active = st.gun_slot.min(1) as usize;
        let sh = (gh * 0.26).clamp(64.0 * k, 96.0 * k);
        let sw = (gw - g) / 2.0;
        for s in 0..2usize {
            let r = Rect::from_min_size(Pos2::new(gx + s as f32 * (sw + g), y), Vec2::new(sw, sh));
            let gun = if s == 0 { lo.gun1 } else { lo.gun2 };
            let id = ui.scope_id().with(("lo-slot", s));
            let resp = ui.interact(r, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let hot = ctx.animate_bool_with_time(id.with("hot"), resp.hovered() || resp.has_focus(), 0.1);
            let on = s == active && !v.melee_only;
            if on || hot > 0.01 {
                let a = if on { 1.0 } else { hot };
                for (e, al) in [(7.0, 0.10), (3.5, 0.22)] {
                    p.rect_stroke(r.expand(e * k), 6.0 * k + e * k, Stroke::new(2.5 * k, ACCENT.gamma_multiply(al * a)), StrokeKind::Outside);
                }
            }
            p.rect_filled(r, 6.0 * k, if on { Color32::from_rgba_unmultiplied(66, 40, 28, 246) } else { mix(Color32::from_rgba_unmultiplied(20, 18, 31, 238), Color32::from_rgba_unmultiplied(44, 35, 46, 245), hot) });
            let inner = r.shrink2(Vec2::new(12.0 * k, 8.0 * k));
            let slot_name = if s == 0 { "GUN 1" } else { "GUN 2" };
            p.text(inner.left_top(), Align2::LEFT_TOP, slot_name, display(15.0 * k), if on { ACCENT } else { Color32::from_gray(165) });
            let sw0 = text_width(&p, slot_name, display(15.0 * k));
            let kw = (text_width(&p, &v.keys[1 + s], display(13.0 * k)) + 10.0 * k).max(18.0 * k);
            keycap(&p, Pos2::new(inner.left() + sw0 + 8.0 * k + kw * 0.5, inner.top() + 8.0 * k), &v.keys[1 + s], k);
            if on {
                let t = "PICKING";
                let tw = text_width(&p, t, display(12.5 * k)) + 10.0 * k;
                let tr = Rect::from_min_size(Pos2::new(inner.right() - tw, inner.top()), Vec2::new(tw, 16.0 * k));
                p.rect_filled(tr, 3.0 * k, ACCENT);
                p.text(tr.center() + Vec2::new(0.0, 0.5 * k), Align2::CENTER_CENTER, t, display(12.5 * k), Color32::from_rgb(34, 18, 8));
            }
            let name = gun.name().to_uppercase();
            let icon_w = (inner.width() * 0.45).min(inner.height() * 2.2);
            label(&p, Pos2::new(inner.left(), inner.bottom() + 2.0 * k), Align2::LEFT_BOTTOM, &name, fitted(&p, &name, display(26.0 * k), inner.width() - icon_w - 6.0 * k), Color32::WHITE);
            let ir = Rect::from_min_max(Pos2::new(inner.right() - icon_w, inner.top() + 18.0 * k), inner.max);
            weapon_icon(&p, ir, gun, if on { Color32::WHITE } else { Color32::from_gray(200) });
            if v.melee_only {
                p.rect_filled(r, 6.0 * k, Color32::from_black_alpha(150));
            }
            p.rect_stroke(r, 6.0 * k, if on { Stroke::new(2.5 * k, ACCENT) } else { Stroke::new(1.0 + hot, mix(Color32::from_white_alpha(26), ACCENT, hot * 0.85)) }, StrokeKind::Inside);
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, slot_name));
            if resp.hovered() || resp.has_focus() {
                hover = Some((format!("{slot_name}: {}", gun.name()), format!("{}  (pick a gun below to put it here)", gun.blurb())));
            }
            if resp.clicked() {
                st.gun_slot = s as u8;
                st.slot = 1 + s as u8;
                ui_sound(Sfx::UiClick);
            }
        }
        // one grid of every gun, four across
        let gy = y + sh + g * 1.5;
        let gch = (gh - sh - g * 2.5) / 2.0;
        for (i, w) in Weapon::GUNS.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(gx + (i % 4) as f32 * (u + g), gy + (i / 4) as f32 * (gch + g)), Vec2::new(u, gch));
            let in_slot = if lo.gun1 == *w { Some(0usize) } else if lo.gun2 == *w { Some(1) } else { None };
            let badge = in_slot.map(|s| (if s == 0 { "GUN 1" } else { "GUN 2" }, s == active));
            let look = CardLook { selected: in_slot == Some(active) && !v.melee_only, dim: v.melee_only, tag: if v.melee_only { Some("BLADES ONLY") } else { None }, badge: if v.melee_only { None } else { badge } };
            let id = ui.scope_id().with(("lo-gun", i));
            let resp = card(ui, &p, r, id, &look, k, |p, r, ink, lit| weapon_card(p, r, *w, ink, lit, k));
            if resp.hovered() || resp.has_focus() {
                let extra = if v.melee_only {
                    "  (guns are off in Gladiator)".to_string()
                } else if in_slot.is_some_and(|s| s != active) {
                    format!("  (in Gun {}: picking it swaps your two guns)", 2 - active)
                } else {
                    String::new()
                };
                hover = Some((w.name().to_string(), format!("{}{}", w.blurb(), extra)));
            }
            if resp.clicked() && !v.melee_only {
                match in_slot {
                    Some(s) if s != active => std::mem::swap(&mut lo.gun1, &mut lo.gun2),
                    Some(_) => {}
                    None if active == 0 => lo.gun1 = *w,
                    None => lo.gun2 = *w,
                }
                click(st, Some(1 + active as u8), w.name(), w.blurb());
            }
        }        // what's pointed at (or last picked)
        let dy = grid_bottom + g;
        // items
        let iy = dy + det_h + g;
        p.rect_filled(Rect::from_min_size(Pos2::new(l.left(), iy + item_h * 0.5 - 8.0 * k), Vec2::new(3.0 * k, 16.0 * k)), 1.0, ACCENT);
        label(&p, Pos2::new(l.left() + 9.0 * k, iy + item_h * 0.5), Align2::LEFT_CENTER, "ITEM", display(21.0 * k), Color32::WHITE);
        let ix0 = l.left() + u * 0.5 + 24.0 * k;
        let iw = (l.right() - ix0 - 2.0 * g) / 3.0;
        for i in 0..3u8 {
            let r = Rect::from_min_size(Pos2::new(ix0 + i as f32 * (iw + g), iy), Vec2::new(iw, item_h));
            let look = CardLook { selected: lo.item == i, dim: false, tag: None, badge: None };
            let id = ui.scope_id().with(("lo-item", i));
            let resp = card(ui, &p, r, id, &look, k, |p, r, ink, _| item_card(p, r, i, ink, k));
            if resp.hovered() || resp.has_focus() {
                hover = Some((ITEM_NAMES[i as usize].to_string(), ITEM_BLURBS[i as usize].to_string()));
            }
            if resp.clicked() {
                lo.item = i;
                click(st, None, ITEM_NAMES[i as usize], ITEM_BLURBS[i as usize]);
            }
        }
        // fighters
        p.text(Pos2::new(l.left(), fighter_y + 15.0 * k), Align2::LEFT_CENTER, "FIGHTER", display(17.0 * k), Color32::from_gray(170));
        let fx0 = l.left() + 78.0 * k;
        let n = chars::COUNT as f32;
        let cw = ((l.right() - fx0 - g * (n - 1.0)) / n).min(160.0 * k);
        for b in 0..chars::COUNT as u8 {
            let r = Rect::from_min_size(Pos2::new(fx0 + b as f32 * (cw + g), fighter_y), Vec2::new(cw, 30.0 * k));
            let id = ui.scope_id().with(("lo-body", b));
            let resp = ui.interact(r, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let hot = ctx.animate_bool_with_time(id.with("hot"), resp.hovered() || resp.has_focus(), 0.1);
            let sel = lo.body == b;
            let fill = if sel { Color32::from_rgba_unmultiplied(96, 54, 32, 245) } else { mix(Color32::from_rgba_unmultiplied(24, 22, 36, 235), Color32::from_rgba_unmultiplied(50, 40, 46, 240), hot) };
            if hot > 0.01 {
                p.rect_stroke(r.expand(3.0 * k * hot), 7.0 * k, Stroke::new(2.0 * k, ACCENT.gamma_multiply(0.3 * hot)), StrokeKind::Outside);
            }
            p.rect_filled(r, 4.0 * k, fill);
            p.rect_stroke(r, 4.0 * k, if sel { Stroke::new(2.0 * k, ACCENT) } else { Stroke::new(1.0, mix(Color32::from_white_alpha(24), ACCENT, hot)) }, StrokeKind::Inside);
            let name = chars::name(b).to_uppercase();
            p.text(r.center() + Vec2::new(0.0, 1.0 * k), Align2::CENTER_CENTER, &name, fitted(&p, &name, display(18.0 * k), cw - 10.0 * k), if sel || hot > 0.5 { Color32::WHITE } else { Color32::from_gray(190) });
            if resp.hovered() || resp.has_focus() {
                hover = Some((chars::name(b).to_string(), chars::blurb(b).to_string()));
            }
            if resp.clicked() {
                lo.body = b;
                click(st, None, chars::name(b), chars::blurb(b));
            }
        }
        // what's pointed at (or last picked)
        let (dn, db) = hover.or_else(|| st.last.clone()).unwrap_or_else(|| (lo.melee.name().to_string(), lo.melee.blurb().to_string()));
        let dn = dn.to_uppercase();
        let nf = display(19.0 * k);
        let nw = text_width(&p, &dn, nf.clone());
        label(&p, Pos2::new(l.left(), dy + det_h * 0.5), Align2::LEFT_CENTER, &dn, nf, ACCENT);
        let bx = l.left() + nw + 12.0 * k;
        p.text(Pos2::new(bx, dy + det_h * 0.5 + 1.0 * k), Align2::LEFT_CENTER, &db, fitted(&p, &db, body(14.0 * k), l.right() - bx), Color32::from_gray(215));
        // right side: who you are, and the big button

        let right_w = rect.right() - split;
        let bh = 68.0 * k;
        let bw = (300.0 * k).min(right_w - m);
        let br = Rect::from_min_size(Pos2::new(rect.right() - m - bw, rect.bottom() - m - bh), Vec2::new(bw, bh));
        let fname = chars::name(lo.body).to_uppercase();
        label(&p, Pos2::new(br.right(), br.top() - 34.0 * k), Align2::RIGHT_BOTTOM, &fname, display(40.0 * k), Color32::WHITE);
        let bl = chars::blurb(lo.body);
        p.text(Pos2::new(br.right(), br.top() - 30.0 * k), Align2::RIGHT_TOP, bl, fitted(&p, bl, body(13.5 * k), right_w - m * 1.5), Color32::from_gray(215));
        let held = match st.slot {
            1 => lo.gun1,
            2 => lo.gun2,
            _ => lo.melee,
        };
        let holding = format!("HOLDING  {}", held.name().to_uppercase());
        let hf = display(17.0 * k);
        let hw = text_width(&p, &holding, hf.clone());
        let hb = Rect::from_min_max(Pos2::new(br.right() - hw - 14.0 * k, br.top() - 104.0 * k), Pos2::new(br.right() + 6.0 * k, br.top() - 80.0 * k));
        p.rect_filled(hb, 4.0 * k, Color32::from_rgba_unmultiplied(10, 9, 18, 215));
        p.rect_filled(Rect::from_min_size(hb.min, Vec2::new(3.0 * k, hb.height())), 1.0, ACCENT);
        p.text(Pos2::new(br.right() - 2.0 * k, hb.center().y + 1.0 * k), Align2::RIGHT_CENTER, &holding, hf, Color32::from_rgb(255, 200, 150));
        let id = ui.scope_id().with("lo-ready");
        let resp = ui.interact(br, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let hot = ctx.animate_bool_with_time(id.with("hot"), resp.hovered() || resp.has_focus(), 0.1);
        let press = ctx.animate_bool_with_time(id.with("press"), resp.is_pointer_button_down_on(), 0.05);
        let urgent = v.timer.is_some_and(|t| t < 5.5);
        let pulse = if urgent { 0.5 + 0.5 * (ctx.input(|i| i.time) as f32 * 8.0).sin() } else { 0.0 };
        let r = br.shrink(3.0 * k * press);
        for (e, a) in [(10.0, 0.10), (5.0, 0.2)] {
            p.rect_stroke(r.expand(e * k), 8.0 * k + e * k, Stroke::new(3.0 * k, ACCENT.gamma_multiply(a * (0.6 + 0.4 * hot + 0.5 * pulse))), StrokeKind::Outside);
        }
        p.rect_filled(r, 6.0 * k, mix(Color32::from_rgb(214, 108, 42), Color32::from_rgb(255, 156, 76), hot.max(pulse * 0.6)));
        vgrad(&p, r.shrink(1.0), Color32::from_white_alpha(40), Color32::from_black_alpha(30));
        p.rect_stroke(r, 6.0 * k, Stroke::new(1.5 * k + hot, Color32::from_rgba_unmultiplied(255, 220, 180, 200)), StrokeKind::Inside);
        let dark = Color32::from_rgb(28, 14, 6);
        let mut text_room = r.width() - 40.0 * k;
        if let Some(t) = v.timer {
            let box_r = Rect::from_min_size(Pos2::new(r.right() - r.height() + 8.0 * k, r.top() + 8.0 * k), Vec2::splat(r.height() - 16.0 * k));
            p.rect_filled(box_r, 4.0 * k, Color32::from_rgba_unmultiplied(28, 14, 6, 200));
            p.text(box_r.center() + Vec2::new(0.0, 1.0 * k), Align2::CENTER_CENTER, format!("{}", t.max(0.0).ceil() as i32), display(32.0 * k), if urgent { Color32::from_rgb(255, 210, 160) } else { Color32::WHITE });
            text_room -= box_r.width() + 8.0 * k;
        }
        let action = v.action.to_uppercase();
        p.text(Pos2::new(r.left() + 22.0 * k + 4.0 * hot, r.center().y + 2.0 * k), Align2::LEFT_CENTER, &action, fitted(&p, &action, display(36.0 * k), text_room), dark);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, v.action));
        if resp.clicked() {
            act = LoadoutAct::Ready;
            ui_sound(Sfx::UiConfirm);
        }
        if v.back {
            let bb = Rect::from_min_size(Pos2::new(br.left() - 12.0 * k - 130.0 * k, br.top() + 10.0 * k), Vec2::new(130.0 * k, bh - 20.0 * k));
            let id = ui.scope_id().with("lo-back");
            let resp = ui.interact(bb, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let hot = ctx.animate_bool_with_time(id.with("hot"), resp.hovered() || resp.has_focus(), 0.1);
            p.rect_filled(bb, 5.0 * k, mix(Color32::from_rgba_unmultiplied(30, 27, 44, 235), Color32::from_rgba_unmultiplied(84, 48, 32, 245), hot));
            p.rect_stroke(bb, 5.0 * k, Stroke::new(1.0 + hot, mix(Color32::from_white_alpha(30), ACCENT, hot)), StrokeKind::Inside);
            p.text(bb.center() + Vec2::new(0.0, 1.0 * k), Align2::CENTER_CENTER, "BACK", display(24.0 * k), Color32::WHITE);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Back"));
            if resp.clicked() {
                act = LoadoutAct::Back;
                ui_sound(Sfx::UiBack);
            }
        }
        // controls, top right
        p.text(Pos2::new(rect.right() - m, rect.top() + m), Align2::RIGHT_TOP, &v.hint, fitted(&p, &v.hint, body(13.0 * k), right_w + 40.0 * k), Color32::from_rgba_unmultiplied(230, 220, 210, 200));
        // a controller starts on the chosen blade
        if v.pad && ctx.memory(|mm| mm.focused().is_none()) {
            if let Some(id) = first_id {
                ctx.memory_mut(|mm| mm.request_focus(id));
            }
        }
    });
    act
}


