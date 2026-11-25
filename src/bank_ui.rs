//! The bank, gifts and the admin side of the menu.
//!
//! - The Bank: Banker Odo's booth in Varrow Centre (crate::varrow::BANK). Everything you own is
//!   shown there: cosmetics, weapon finishes, dyes, titles and the pet. You can file things
//!   under tabs you name, search and sort, and drag between the bank, the tabs and a 28-slot
//!   carried inventory. You can put something on (the wardrobe opens) or gift it.
//! - Gifts (crate::gifts): right-click a player, then Gift, pick an item, check what moves, send.
//!   The receiver gets the same check before saying yes.
//! - The admin side (crate::roles, crate::commands): Steam identities each frame, messages
//!   straight to players, the command log, ::max and ::reset on this computer, being kicked,
//!   the ::help list, the announcement banner, and what Tab completes in the chat box.

use super::*;
use crate::hub_ui::{flat_dist, paint_look, tile, title, HubPanel, LOOK_ICONS};
use crate::items::{self, Category, ItemKey, ItemStore, CARRY_SLOTS};
use crate::roles::Role;

/// Something being dragged in the bank.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drag {
    Bank(ItemKey),
    Carried(usize),
}

#[derive(Default)]
pub struct BankUi {
    /// The tab shown (None: everything).
    pub tab: Option<usize>,
    pub search: String,
    /// 0 by kind, 1 by name, 2 by count.
    pub sort: u8,
    pub filter: Option<Category>,
    pub selected: Option<ItemKey>,
    pub drag: Option<Drag>,
    pub new_tab: String,
    pub renaming: Option<(usize, String)>,
    /// The gift being put together: to whom, what, how many, made new (admins), being reviewed.
    pub gift_to: Option<usize>,
    pub gift_item: Option<ItemKey>,
    pub gift_n: u32,
    pub gift_create: bool,
    pub gift_review: bool,
    pub gift_search: String,
    /// The bank on disk: the change last saved, whose it is, when it's next saved.
    pub saved_version: u32,
    pub account: u64,
    pub save_t: f32,
    pub lobby_t: f32,
    pub refresh_t: f32,
}

/// Whether you stand at the bank's counter.
pub fn at_bank(g: &Game) -> bool {
    let (kx, _, kz) = varrow::BANK;
    let p = g.f[g.local].pos;
    g.hub.is_some() && !g.in_raid(g.local) && flat_dist(p, v3(kx - 1.0, 0.0, kz)) < 1.9
}

fn swatch(p: &egui::Painter, r: Rect, key: ItemKey) {
    match key {
        ItemKey::Dye(d) => {
            let c = crate::gear::DYES.get(d as usize).map_or([0.5; 3], |d| d.1);
            let to8 = |v: f32| (v.max(0.0).powf(1.0 / 2.2) * 255.0).min(255.0) as u8;
            p.circle_filled(r.center(), r.width() * 0.32, Color32::from_rgb(to8(c[0]), to8(c[1]), to8(c[2])));
            p.circle_stroke(r.center(), r.width() * 0.32, Stroke::new(2.0, Color32::from_gray(40)));
        }
        ItemKey::Title(_) => {
            let rr = r.shrink(r.width() * 0.2);
            p.rect_filled(rr, 4.0, Color32::from_rgb(222, 200, 150));
            p.rect_stroke(rr, 4.0, Stroke::new(2.0, Color32::from_rgb(120, 80, 30)), egui::StrokeKind::Inside);
            p.text(rr.center(), Align2::CENTER_CENTER, "T", FontId::proportional(r.height() * 0.35), Color32::from_rgb(90, 50, 20));
        }
        ItemKey::Pet(_) => {
            p.circle_filled(r.center(), r.width() * 0.3, Color32::from_rgb(255, 140, 40));
            p.circle_filled(r.center() + Vec2::new(0.0, -r.width() * 0.05), r.width() * 0.16, Color32::from_rgb(255, 230, 150));
            for s in [-1.0f32, 1.0] {
                p.circle_filled(r.center() + Vec2::new(s * r.width() * 0.08, -r.width() * 0.04), r.width() * 0.03, Color32::from_rgb(50, 20, 10));
            }
        }
        ItemKey::Look(_) => {}
    }
}

/// Draw an item in a square.
fn paint_item(p: &egui::Painter, r: Rect, icons: Option<&egui::TextureHandle>, key: ItemKey, n: u32) {
    match key {
        ItemKey::Look(id) => paint_look(p, r, icons, id, false),
        _ => swatch(p, r, key),
    }
    if n > 1 {
        let at = r.right_bottom() - Vec2::new(4.0, 4.0);
        p.text(at + Vec2::new(1.0, 1.0), Align2::RIGHT_BOTTOM, format!("{n}"), FontId::proportional(13.0), Color32::BLACK);
        p.text(at, Align2::RIGHT_BOTTOM, format!("{n}"), FontId::proportional(13.0), Color32::from_rgb(255, 230, 140));
    }
}

impl App {
    fn ensure_icons(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if self.hub_ui.icons.is_none() {
            if let Ok(img) = image::load_from_memory(LOOK_ICONS) {
                let img = img.to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                self.hub_ui.icons = Some(ctx.load_texture("look-icons", egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), egui::TextureOptions::LINEAR));
            }
        }
        self.hub_ui.icons.clone()
    }

    // ================================================================ every frame: the admin side

    pub fn admin_frame(&mut self, dt: f32) {
        let my_id = self.steam.as_ref().map_or(0, |s| s.me);
        let lobby: Option<Vec<u64>> = {
            self.bank_ui.lobby_t -= dt;
            if self.bank_ui.lobby_t <= 0.0 {
                self.bank_ui.lobby_t = 1.0;
                Some(self.steam.as_ref().map(|s| s.lobby_members()).unwrap_or_default())
            } else {
                None
            }
        };
        let Screen::Play { game, net } = &mut self.screen else { return };
        if !game.admin.started {
            game.admin = crate::roles::Session::start(my_id);
            let me = game.local;
            game.set_verified_id(me, my_id);
        }
        // who everyone is: what our own connections prove
        if let Some(n) = net.as_ref() {
            if n.is_host {
                for s in 0..game.f.len().min(MAX_PLAYERS) {
                    if s != game.local {
                        if let Some(id) = n.peer_steam(s) {
                            game.set_verified_id(s, id);
                        }
                    }
                }
            } else if let Some(id) = n.host_steam() {
                let h = game.host_slot;
                game.set_verified_id(h, id);
            }
        }
        if let Some(l) = lobby {
            game.admin.lobby = l;
        }
        // messages straight to players (they need Steam)
        let out = std::mem::take(&mut game.admin.direct_out);
        for (to, bytes) in out {
            match self.steam.as_ref() {
                Some(st) => st.send_direct(to, &bytes),
                None => game.say("That needs Steam: nothing was sent.".into()),
            }
        }
        for r in std::mem::take(&mut game.admin.replies) {
            game.admin_line(r);
        }
        // the command log
        let lines = std::mem::take(&mut game.admin.log);
        if !lines.is_empty() && !(std::env::var_os("DUEL_DATA_DIR").is_some() && self.screenshot.is_some()) {
            if let Some(p) = crate::platform::data_file("admin_log.txt") {
                use std::io::Write;
                if let Some(d) = p.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
                    for l in lines {
                        let _ = writeln!(f, "{l}");
                    }
                }
            }
        }
        // (host) players kicked: off the connection, out of the game
        let drop = std::mem::take(&mut game.admin.drop_slots);
        for s in drop {
            if let Some(n) = net.as_mut() {
                n.drop_slot(s);
            }
            if s < game.f.len() && game.f[s].present {
                game.remove_player(s);
            }
        }
        if let Some(reason) = game.admin.kicked.take() {
            self.disconnected = Some(format!("You were removed from the session: {reason}"));
        }
        // ::max and ::reset on this computer
        if std::mem::take(&mut game.admin.max_req) {
            self.career.max_mastery();
            if !(std::env::var_os("DUEL_DATA_DIR").is_some() && self.screenshot.is_some()) {
                self.career.save();
            }
        }
        if std::mem::take(&mut game.admin.reset_req) {
            let me = game.local;
            self.hub_ui.save = hub::HubSave::default();
            game.f[me].hub.look = hub::Outfit::default();
            if let Some(h) = game.hub.as_mut() {
                let mastery = h.earned.mastery;
                h.bank = items::LocalBank::default();
                h.earned = hub::Earned { mastery, ..Default::default() };
                h.run_best = 0;
                h.aim_best = [0; crate::hub_aim::DRILLS];
                h.ghost.clear();
                h.refresh_bank();
            }
            self.hub_ui.dirty = true;
            self.hub_ui.save_t = 0.0;
        }
        // a gated session: the career and leaderboards (crate::roles)
        let gated = game.admin.gated;
        self.session_gated |= gated;
        // the bank: newly earned things in, saved when it changes
        if let Some(h) = game.hub.as_mut() {
            self.bank_ui.refresh_t -= dt;
            if self.bank_ui.refresh_t <= 0.0 {
                self.bank_ui.refresh_t = 0.5;
                h.refresh_bank();
            }
            self.bank_ui.save_t -= dt;
            if h.bank.version != self.bank_ui.saved_version && self.bank_ui.save_t <= 0.0 {
                self.bank_ui.save_t = 2.0;
                self.bank_ui.saved_version = h.bank.version;
                if !(std::env::var_os("DUEL_DATA_DIR").is_some() && self.screenshot.is_some()) && !self.hub_ui.unlock_all {
                    items::save(&h.bank, self.bank_ui.account);
                }
            }
        }
    }

    /// The bank for this account onto the hub (and what the save said, the first time).
    pub(crate) fn load_bank(&mut self, earned: &mut hub::Earned) -> items::LocalBank {
        let account = self.steam.as_ref().map_or(0, |s| s.me);
        self.bank_ui.account = account;
        let mut bank;
        let mut note = None;
        if self.hub_ui.unlock_all || std::env::var_os("DUEL_UNLOCK_ALL").is_some() {
            bank = items::LocalBank::default();
            bank.sync(earned);
            for k in items::catalogue() {
                if !k.starter() && bank.count(k) == 0 {
                    bank.grant(k, 1, "everything (pictures)");
                }
            }
        } else {
            let existed = items::bank_path(account).is_some_and(|p| p.exists() || p.with_extension("bak").exists());
            (bank, note) = items::load(account);
            if !existed {
                // the first time: what the old save had earned goes in
                bank.sync(earned);
            }
            // from now on, progress is the bank's own copy (the save's can be edited)
            let e = bank.earned;
            earned.rooftop = e.rooftop;
            earned.table_hard = e.table_hard;
            earned.pit_wins = e.pit_wins;
            earned.raid = e.raid;
            bank.sync(earned);
        }
        bank.apply_to(earned);
        self.bank_ui.saved_version = bank.version;
        if let Some(n) = note {
            self.menu_msg = Some(n.clone());
            self.pending_toast = Some(n);
        }
        bank
    }

    // ===================================================================== the bank's screen

    /// The bank, full screen. Returns true to close.
    pub fn bank_screen(&mut self, ctx: &egui::Context, rect: Rect) -> bool {
        let icons = self.ensure_icons(ctx);
        let s = (rect.height() / 900.0).clamp(0.75, 1.6);
        let Screen::Play { game, .. } = &mut self.screen else { return true };
        let me = game.local;
        let role = game.role_of(me);
        let Some(h) = game.hub.as_mut() else { return true };
        let ui_st = &mut self.bank_ui;
        let mut close = false;
        let mut wear: Option<ItemKey> = None;
        let mut gift: Option<ItemKey> = None;
        let pointer_released = ctx.input(|i| i.pointer.any_released());
        let mut dropped_on: Option<Drag> = None; // where a drag ended: a bank cell (Bank) or a slot
        let mut dropped_tab: Option<Option<usize>> = None;
        // the dim backdrop
        ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("bank-dim"))).rect_filled(rect, 0.0, Color32::from_black_alpha(140));
        let w = (rect.width() - 48.0 * s).min(1240.0 * s);
        let pos = Pos2::new(rect.center().x - w / 2.0, rect.top() + 30.0 * s);
        egui::Area::new(egui::Id::new("bank")).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
            ui_kit::panel_frame(ui, |ui| {
                ui.set_width(w);
                ui.horizontal(|ui| {
                    title(ui, "THE BANK OF VARROW", s);
                    ui.label(egui::RichText::new("·  Banker Odo keeps it all").size(14.0 * s).color(Color32::from_gray(190)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("Close (Esc)").size(14.0 * s)).clicked() {
                            close = true;
                        }
                        let acct = if ui_st.account == 0 { "this PC".to_string() } else { format!("Steam {}", ui_st.account) };
                        ui.label(egui::RichText::new(format!("Account: {acct}")).size(12.0 * s).color(Color32::from_gray(160)));
                    });
                });
                ui.add_space(4.0 * s);
                // ---- the tabs
                ui.horizontal_wrapped(|ui| {
                    let dragging = ui_st.drag.is_some();
                    let tab_button = |ui: &mut egui::Ui, cur: &mut Option<usize>, dropped: &mut Option<Option<usize>>, label: String, which: Option<usize>| {
                        let on = *cur == which;
                        let r = ui.add(egui::Button::new(egui::RichText::new(label).size(14.0 * s).color(if on { Color32::BLACK } else { Color32::from_gray(230) })).fill(if on { ACCENT } else { Color32::from_rgb(40, 34, 30) }).min_size(Vec2::new(70.0 * s, 28.0 * s)));
                        if r.clicked() {
                            *cur = which;
                        }
                        if dragging && r.contains_pointer() && pointer_released {
                            *dropped = Some(which);
                        }
                        r
                    };
                    tab_button(ui, &mut ui_st.tab, &mut dropped_tab, "All".into(), None);
                    for (k, t) in h.bank.tabs.clone().iter().enumerate() {
                        let n = t.items.len();
                        let r = tab_button(ui, &mut ui_st.tab, &mut dropped_tab, format!("{} ({n})", t.name), Some(k));
                        r.context_menu(|ui| {
                            if ui.button("Rename").clicked() {
                                ui_st.renaming = Some((k, t.name.clone()));
                                ui.close();
                            }
                            if ui.button("Delete tab (its items stay in the bank)").clicked() {
                                h.bank.remove_tab(k);
                                ui_st.tab = None;
                                ui.close();
                            }
                        });
                    }
                    if h.bank.tabs.len() < items::MAX_TABS {
                        ui.add(egui::TextEdit::singleline(&mut ui_st.new_tab).desired_width(110.0 * s).hint_text("new tab name"));
                        if ui.button(egui::RichText::new("+ Tab").size(14.0 * s)).clicked() {
                            let name = if ui_st.new_tab.trim().is_empty() { format!("Tab {}", h.bank.tabs.len() + 1) } else { ui_st.new_tab.clone() };
                            ui_st.tab = h.bank.add_tab(&name);
                            ui_st.new_tab.clear();
                        }
                    }
                });
                if let Some((k, mut name)) = ui_st.renaming.take() {
                    ui.horizontal(|ui| {
                        ui.label("Rename tab:");
                        let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(140.0 * s));
                        r.request_focus();
                        if ui.button("OK").clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                            h.bank.rename_tab(k, &name);
                        } else {
                            ui_st.renaming = Some((k, name));
                        }
                    });
                }
                // ---- search, sort, filter
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Search").size(13.0 * s));
                    ui.add(egui::TextEdit::singleline(&mut ui_st.search).desired_width(180.0 * s).hint_text("a name..."));
                    ui.add_space(10.0 * s);
                    ui.label(egui::RichText::new("Sort").size(13.0 * s));
                    for (k, l) in ["Kind", "Name", "Count"].iter().enumerate() {
                        if ui.selectable_label(ui_st.sort == k as u8, *l).clicked() {
                            ui_st.sort = k as u8;
                        }
                    }
                    ui.add_space(10.0 * s);
                    if ui.selectable_label(ui_st.filter.is_none(), "Everything").clicked() {
                        ui_st.filter = None;
                    }
                    for c in [Category::Cosmetic, Category::Weapon, Category::Dye, Category::Title, Category::Pet] {
                        if ui.selectable_label(ui_st.filter == Some(c), c.name()).clicked() {
                            ui_st.filter = Some(c);
                        }
                    }
                });
                ui.separator();
                // ---- the items
                let mut list = h.bank.everything();
                // (in the bank: what's carried is shown below, not here)
                list.retain(|(k, _)| h.bank.items.get(k).copied().unwrap_or(0) > 0 || k.starter());
                for (k, n) in list.iter_mut() {
                    if !k.starter() {
                        *n = h.bank.items.get(k).copied().unwrap_or(0);
                    }
                }
                if let Some(t) = ui_st.tab.and_then(|t| h.bank.tabs.get(t)) {
                    let order = t.items.clone();
                    list.retain(|(k, _)| order.contains(k));
                    list.sort_by_key(|(k, _)| order.iter().position(|x| x == k).unwrap_or(0));
                } else {
                    match ui_st.sort {
                        1 => list.sort_by_key(|(k, _)| k.name().to_lowercase()),
                        2 => list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0))),
                        _ => list.sort_by_key(|(k, _)| (k.category(), k.starter(), *k)),
                    }
                }
                if let Some(c) = ui_st.filter {
                    list.retain(|(k, _)| k.category() == c);
                }
                let q = ui_st.search.trim().to_lowercase();
                if !q.is_empty() {
                    list.retain(|(k, _)| k.name().to_lowercase().contains(&q));
                }
                let side_w = 300.0 * s;
                ui.horizontal_top(|ui| {
                    let grid_w = w - side_w - 20.0 * s;
                    let cell = 66.0 * s;
                    let per_row = ((grid_w / (cell + 6.0)).floor() as usize).max(4);
                    ui.vertical(|ui| {
                        ui.set_width(grid_w);
                        ui.label(egui::RichText::new(format!("{} kinds of thing  ·  drag onto a tab to file it, onto a slot below to carry it", list.len())).size(12.0 * s).color(Color32::from_gray(170)));
                        egui::ScrollArea::vertical().id_salt("bank-grid").max_height(rect.height() * 0.42).show(ui, |ui| {
                            for row in list.chunks(per_row) {
                                ui.horizontal(|ui| {
                                    for (k, n) in row {
                                        let resp = tile(ui, Vec2::splat(cell), ui_st.selected == Some(*k), true, |p, r| paint_item(p, r, icons.as_ref(), *k, *n));
                                        let resp = resp.interact(egui::Sense::click_and_drag());
                                        if resp.clicked() {
                                            ui_st.selected = Some(*k);
                                        }
                                        if resp.drag_started() && !k.starter() {
                                            ui_st.drag = Some(Drag::Bank(*k));
                                        }
                                        if ui_st.drag.is_some() && resp.contains_pointer() && pointer_released {
                                            dropped_on = Some(Drag::Bank(*k));
                                        }
                                        resp.on_hover_text(format!("{}\n{}{}", k.name(), k.source(), if k.starter() { "\n(everyone has it: can't be gifted)" } else { "" }));
                                    }
                                });
                            }
                        });
                    });
                    // ---- the selected item
                    ui.vertical(|ui| {
                        ui.set_width(side_w);
                        match ui_st.selected {
                            Some(k) => {
                                let r = ui.allocate_exact_size(Vec2::splat(110.0 * s), egui::Sense::hover()).0;
                                ui.painter().rect_filled(r, 8.0, Color32::from_rgb(30, 26, 24));
                                paint_item(ui.painter(), r, icons.as_ref(), k, 0);
                                ui.label(egui::RichText::new(k.name()).size(17.0 * s).strong().color(Color32::WHITE));
                                ui.label(egui::RichText::new(format!("{}  ·  {}", k.category().name(), k.source())).size(12.5 * s).color(Color32::from_gray(190)));
                                let owned = if k.starter() { "everyone has it".to_string() } else { format!("in the bank: {}   carried: {}", h.bank.items.get(&k).copied().unwrap_or(0), h.bank.carried.iter().filter(|c| **c == Some(k)).count()) };
                                ui.label(egui::RichText::new(owned).size(12.5 * s));
                                ui.add_space(6.0 * s);
                                if !k.starter() && ui.button("Take into the inventory").clicked() && !h.bank.carry(k) {
                                    h.toast("Your inventory is full (28 slots)");
                                }
                                if matches!(k, ItemKey::Look(_) | ItemKey::Dye(_) | ItemKey::Title(_) | ItemKey::Pet(_)) && ui.button("Put it on (the wardrobe)").clicked() {
                                    wear = Some(k);
                                }
                                if !k.starter() && ui.button("Gift it...").clicked() {
                                    gift = Some(k);
                                }
                                ui.horizontal(|ui| {
                                    ui.label("File under:");
                                    let cur = h.bank.tab_of(k);
                                    egui::ComboBox::from_id_salt("bank-file").selected_text(cur.and_then(|t| h.bank.tabs.get(t)).map_or("no tab".into(), |t| t.name.clone())).show_ui(ui, |ui| {
                                        if ui.selectable_label(cur.is_none(), "no tab").clicked() {
                                            h.bank.file(k, None, None);
                                        }
                                        for (t, tab) in h.bank.tabs.clone().iter().enumerate() {
                                            if ui.selectable_label(cur == Some(t), &tab.name).clicked() {
                                                h.bank.file(k, Some(t), None);
                                            }
                                        }
                                    });
                                });
                            }
                            None => {
                                ui.label(egui::RichText::new("Pick something to see it here.").size(13.0 * s).color(Color32::from_gray(170)));
                            }
                        }
                        ui.add_space(8.0 * s);
                        ui.label(egui::RichText::new("Lately").strong().size(13.0 * s));
                        for l in h.bank.history.iter().rev().take(6) {
                            ui.label(egui::RichText::new(l).size(11.5 * s).color(Color32::from_gray(185)));
                        }
                        if role >= Role::Admin {
                            ui.add_space(6.0 * s);
                            ui.label(egui::RichText::new("Admin: ::give and ::item search work from the chat").size(11.5 * s).color(Color32::from_rgb(230, 200, 120)));
                        }
                    });
                });
                ui.separator();
                // ---- the carried inventory: 28 slots, 14 a row
                ui.label(egui::RichText::new(format!("CARRIED  ·  {}/{CARRY_SLOTS}  ·  click to put back, drag to rearrange", h.bank.carried.iter().filter(|c| c.is_some()).count())).size(13.0 * s).color(ACCENT));
                let slot_s = ((w - 30.0 * s) / 14.0 - 6.0).min(58.0 * s);
                for row in 0..2 {
                    ui.horizontal(|ui| {
                        for col in 0..14 {
                            let i = row * 14 + col;
                            let it = h.bank.carried[i];
                            let resp = tile(ui, Vec2::splat(slot_s), false, true, |p, r| {
                                if let Some(k) = it {
                                    paint_item(p, r, icons.as_ref(), k, 0);
                                }
                            });
                            let resp = resp.interact(egui::Sense::click_and_drag());
                            if resp.clicked() && it.is_some() {
                                h.bank.deposit(i);
                            }
                            if resp.drag_started() && it.is_some() {
                                ui_st.drag = Some(Drag::Carried(i));
                            }
                            if ui_st.drag.is_some() && resp.contains_pointer() && pointer_released {
                                dropped_on = Some(Drag::Carried(i));
                            }
                            if let Some(k) = it {
                                resp.on_hover_text(k.name());
                            }
                        }
                    });
                }
            });
        });
        // ---- a drag ends
        if let Some(d) = ui_st.drag {
            if let Some(pos) = ctx.pointer_latest_pos() {
                let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("bank-drag")));
                let key = match d {
                    Drag::Bank(k) => Some(k),
                    Drag::Carried(i) => h.bank.carried[i],
                };
                if let Some(k) = key {
                    let r = Rect::from_center_size(pos, Vec2::splat(52.0 * s));
                    p.rect_filled(r, 6.0, Color32::from_black_alpha(150));
                    paint_item(&p, r, icons.as_ref(), k, 0);
                }
            }
            if pointer_released {
                match (d, dropped_on, dropped_tab) {
                    (Drag::Bank(k), _, Some(t)) => h.bank.file(k, t, None),
                    (Drag::Bank(k), Some(Drag::Carried(i)), _) => {
                        if h.bank.carried[i].is_none() {
                            if h.bank.carry(k) {
                                // (carry() takes the first free slot: move it to the one dropped on)
                                if let Some(j) = h.bank.carried.iter().position(|c| *c == Some(k)) {
                                    h.bank.swap_carried(i, j);
                                }
                            }
                        }
                    }
                    (Drag::Bank(k), Some(Drag::Bank(on)), _) => {
                        // in a tab: drop before another item to reorder
                        if let Some(t) = ui_st.tab {
                            let at = h.bank.tabs.get(t).and_then(|tab| tab.items.iter().position(|x| *x == on));
                            h.bank.file(k, Some(t), at);
                        }
                    }
                    (Drag::Carried(i), Some(Drag::Carried(j)), _) => h.bank.swap_carried(i, j),
                    (Drag::Carried(i), Some(Drag::Bank(_)), _) => {
                        h.bank.deposit(i);
                    }
                    _ => {}
                }
                ui_st.drag = None;
            }
        }
        h.bank.apply_to(&mut h.earned);
        if let Some(k) = gift {
            ui_st.gift_item = Some(k);
            ui_st.gift_n = 1;
            ui_st.gift_review = false;
            ui_st.gift_to = None;
            self.hub_ui.panel = Some(HubPanel::Gift(usize::MAX));
            return false;
        }
        if let Some(k) = wear {
            let o = &mut game.f[me].hub.look;
            match k {
                ItemKey::Look(id) => {
                    if let Some(l) = hub::look(id) {
                        o.items[l.slot as usize] = id;
                        o.dyes[l.slot as usize] = hub::set_dyes(l.set);
                    }
                }
                ItemKey::Pet(_) => o.pet = 1,
                ItemKey::Title(t) => o.title = t,
                ItemKey::Dye(_) => {}
            }
            let look = *o;
            game.hub_wear(look);
            self.hub_ui.panel = Some(HubPanel::Wardrobe);
            self.hub_ui.mirror = true;
            return false;
        }
        close
    }

    // ===================================================================== gifts

    /// The gift panel's contents (inside the hub's side panel). `to`: who (usize::MAX: pick).
    pub fn gift_panel(&mut self, ui: &mut egui::Ui, to: usize, close: &mut bool) {
        let Screen::Play { game, .. } = &mut self.screen else { return };
        let me = game.local;
        let role = game.role_of(me);
        let st = &mut self.bank_ui;
        if to != usize::MAX {
            st.gift_to = Some(to);
        }
        title(ui, "A GIFT", 1.0);
        let waiting = game.hub.as_ref().and_then(|h| h.gifts.outgoing);
        if let Some(o) = waiting {
            let who = game.f.get(o.to as usize).map_or("them".into(), |f| f.name.clone());
            ui.label(format!("Waiting for {who} to answer: {} x {}", o.n, o.item.name()));
            ui.label(egui::RichText::new("It stays set aside until they say yes or no (a minute at most).").small().color(Color32::from_gray(170)));
            return;
        }
        // ---- who
        let players: Vec<usize> = (0..game.f.len()).filter(|&s| s != me && game.f[s].present && !game.f[s].is_bot && !game.f[s].hub.townsfolk).collect();
        ui.horizontal_wrapped(|ui| {
            ui.label("To:");
            if players.is_empty() {
                ui.label(egui::RichText::new("nobody else is here").color(Color32::from_gray(160)));
            }
            for &s in &players {
                if ui.selectable_label(st.gift_to == Some(s), &game.f[s].name).clicked() {
                    st.gift_to = Some(s);
                    st.gift_review = false;
                }
            }
        });
        // ---- what
        let bank = game.hub.as_ref().map(|h| h.bank.clone()).unwrap_or_default();
        if role >= Role::Admin {
            ui.checkbox(&mut st.gift_create, "Make it new (admins: nothing leaves your bank)");
        } else {
            st.gift_create = false;
        }
        ui.horizontal(|ui| {
            ui.label("What:");
            ui.add(egui::TextEdit::singleline(&mut st.gift_search).desired_width(160.0).hint_text("search"));
        });
        let q = st.gift_search.trim().to_lowercase();
        let pool: Vec<(ItemKey, u32)> = if st.gift_create { items::catalogue().into_iter().filter(|k| !k.starter()).map(|k| (k, 99)).collect() } else { bank.list() };
        egui::ScrollArea::vertical().id_salt("gift-pick").max_height(170.0).show(ui, |ui| {
            for (k, n) in pool.iter().filter(|(k, _)| q.is_empty() || k.name().to_lowercase().contains(&q)) {
                let label = if st.gift_create { k.name() } else { format!("{}  (you have {n})", k.name()) };
                if ui.selectable_label(st.gift_item == Some(*k), label).clicked() {
                    st.gift_item = Some(*k);
                    st.gift_n = 1;
                    st.gift_review = false;
                }
            }
            if pool.is_empty() {
                ui.label(egui::RichText::new("Your bank has nothing that can be gifted yet (starter items can't be).").color(Color32::from_gray(170)));
            }
        });
        let have = st.gift_item.map_or(0, |k| if st.gift_create { 99 } else { bank.count(k) });
        if have > 1 {
            ui.add(egui::Slider::new(&mut st.gift_n, 1..=have.min(99)).text("how many"));
        }
        st.gift_n = st.gift_n.clamp(1, have.max(1));
        let (Some(to), Some(item)) = (st.gift_to, st.gift_item) else {
            ui.label(egui::RichText::new("Pick who and what.").color(Color32::from_gray(170)));
            return;
        };
        let to_name = game.f.get(to).map_or("?".into(), |f| f.name.clone());
        if !st.gift_review {
            if ui.button(egui::RichText::new("Review the gift").strong()).clicked() {
                st.gift_review = true;
            }
            return;
        }
        // ---- the confirm screen: exactly what moves
        ui.add_space(6.0);
        ui_kit::panel_frame(ui, |ui| {
            ui.label(egui::RichText::new("CHECK BEFORE SENDING").strong().color(ACCENT));
            ui.label(egui::RichText::new(format!("{} x {}", st.gift_n, item.name())).size(18.0).strong().color(Color32::WHITE));
            ui.label(format!("{}  ·  {}", item.category().name(), item.source()));
            ui.label(format!("From: you    To: {to_name}"));
            if st.gift_create {
                ui.label(egui::RichText::new("Made new by you as an admin: nothing leaves your bank.").color(Color32::from_rgb(240, 210, 130)));
            } else {
                let left = bank.count(item).saturating_sub(st.gift_n);
                ui.label(egui::RichText::new(format!("It leaves your bank now (you'll have {left} left), and is theirs once they accept. If they say no, it comes back.")).color(Color32::from_gray(210)));
            }
        });
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new("Send the gift").strong().color(ACCENT)).clicked() {
                match crate::gifts::offer(game, to, item, st.gift_n, st.gift_create) {
                    Ok(_) => game.say(format!("Gift sent to {to_name}: waiting for their answer")),
                    Err(e) => game.say(e),
                }
                st.gift_review = false;
            }
            if ui.button("Back").clicked() {
                st.gift_review = false;
            }
        });
        let _ = close;
    }

    /// A gift offered to us: the same check, then yes or no.
    pub fn gift_incoming(&mut self, ctx: &egui::Context, rect: Rect) {
        let Screen::Play { game, .. } = &mut self.screen else { return };
        let Some(o) = game.hub.as_ref().and_then(|h| h.gifts.incoming) else { return };
        let s = (rect.height() / 900.0).clamp(0.75, 1.6);
        let from = game.f.get(o.from as usize).map_or("Someone".into(), |f| f.name.clone());
        let role = game.role_of(o.from as usize);
        let mut answer = None;
        let w = 460.0 * s;
        egui::Area::new(egui::Id::new("gift-in")).fixed_pos(Pos2::new(rect.center().x - w / 2.0, rect.center().y - 160.0 * s)).order(egui::Order::Foreground).show(ctx, |ui| {
            ui_kit::panel_frame(ui, |ui| {
                ui.set_width(w);
                ui.horizontal(|ui| {
                    if role > Role::Player {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0 * s), egui::Sense::hover());
                        draw_crown(ui.painter(), r.center(), 8.0 * s, role as u8, 255);
                    }
                    title(ui, &format!("A GIFT FROM {}", from.to_uppercase()), s);
                });
                ui.label(egui::RichText::new(format!("{} x {}", o.n, o.item.name())).size(20.0 * s).strong().color(Color32::WHITE));
                ui.label(egui::RichText::new(format!("{}  ·  {}", o.item.category().name(), o.item.source())).size(13.0 * s).color(Color32::from_gray(200)));
                ui.label(egui::RichText::new(if o.create { format!("Made new by {from} (an admin). It goes into your bank.") } else { format!("It moves from {from}'s bank into yours.") }).size(13.0 * s));
                ui.add_space(6.0 * s);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("Accept").strong().size(16.0 * s)).fill(Color32::from_rgb(60, 110, 50))).clicked() {
                        answer = Some(true);
                    }
                    if ui.add(egui::Button::new(egui::RichText::new("No thanks").size(16.0 * s))).clicked() {
                        answer = Some(false);
                    }
                });
            });
        });
        if let Some(a) = answer {
            crate::gifts::answer(game, a);
            if let Some(h) = game.hub.as_mut() {
                h.gifts.incoming = None;
            }
        }
    }

    /// Right-clicking a player: what you can do with them.
    pub fn player_menu(&mut self, ui: &mut egui::Ui, who: usize, close: &mut bool) {
        let Screen::Play { game, .. } = &mut self.screen else { return };
        let me = game.local;
        let Some(f) = game.f.get(who).filter(|f| f.present) else {
            *close = true;
            return;
        };
        let name = f.name.clone();
        let role = game.role_of(who);
        ui.horizontal(|ui| {
            if role > Role::Player {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::hover());
                draw_crown(ui.painter(), r.center(), 8.0, role as u8, 255);
            }
            title(ui, &name.to_uppercase(), 1.0);
        });
        ui.label(egui::RichText::new(role.name()).color(Color32::from_gray(180)));
        let mut next = None;
        if ui.button("Challenge to a duel").clicked() {
            next = Some(HubPanel::Challenge(who));
        }
        if !f.is_bot && ui.button("Gift...").clicked() {
            self.bank_ui.gift_to = Some(who);
            self.bank_ui.gift_item = None;
            self.bank_ui.gift_review = false;
            next = Some(HubPanel::Gift(who));
        }
        if game.role_of(me) >= Role::Admin {
            ui.separator();
            ui.label(egui::RichText::new("Admin").color(Color32::from_rgb(230, 200, 120)));
            ui.horizontal_wrapped(|ui| {
                for (label, cmd) in [("Bring here", "bring"), ("Go to", "tp"), ("Mute 10 min", "mute"), ("Kick", "kick")] {
                    if ui.button(label).clicked() {
                        game.say_chat(&format!("::{cmd} {name}"));
                    }
                }
            });
        }
        if let Some(p) = next {
            self.hub_ui.panel = Some(p);
        }
    }

    // ================================================================ the ::help list, banners

    pub fn help_panel(&mut self, ctx: &egui::Context, rect: Rect) {
        let Screen::Play { game, .. } = &mut self.screen else { return };
        if !game.admin.show_help {
            return;
        }
        let s = (rect.height() / 900.0).clamp(0.75, 1.6);
        let role = game.role_of(game.local);
        let w = 640.0 * s;
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
        egui::Area::new(egui::Id::new("admin-help")).fixed_pos(Pos2::new(rect.center().x - w / 2.0, rect.top() + 70.0 * s)).order(egui::Order::Foreground).show(ctx, |ui| {
            ui_kit::panel_frame(ui, |ui| {
                ui.set_width(w);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(26.0 * s), egui::Sense::hover());
                    draw_crown(ui.painter(), r.center(), 10.0 * s, role as u8, 255);
                    title(ui, &format!("ADMIN COMMANDS  ·  you are {}", role.name()), s);
                });
                ui.label(egui::RichText::new("Type them in chat (Enter). Tab completes a command, a player or an item. Every command is logged.").size(12.5 * s).color(Color32::from_gray(190)));
                ui.add_space(4.0 * s);
                egui::Grid::new("help-grid").num_columns(2).spacing(Vec2::new(14.0 * s, 5.0 * s)).show(ui, |ui| {
                    for c in crate::commands::COMMANDS.iter().filter(|c| c.role <= role) {
                        ui.label(egui::RichText::new(format!("::{} {}", c.name, c.args)).monospace().size(13.5 * s).color(ACCENT));
                        ui.label(egui::RichText::new(c.help).size(13.0 * s));
                        ui.end_row();
                    }
                });
                ui.add_space(6.0 * s);
                ui.label(egui::RichText::new("::max, ::god, ::speed and ::give to yourself mark this session: it counts for no leaderboard, record, career or achievement.").size(12.0 * s).color(Color32::from_rgb(240, 200, 120)));
                if ui.button("Close (Esc)").clicked() {
                    close = true;
                }
            });
        });
        if close {
            game.admin.show_help = false;
        }
    }
}

/// The announcement banner, and a line while the session doesn't count for records.
pub fn admin_hud(p: &egui::Painter, rect: Rect, g: &Game) {
    let s = (rect.height() / 900.0).clamp(0.75, 1.6);
    if let Some((t, life)) = &g.admin.announce {
        let a = (life / 0.6).clamp(0.0, 1.0);
        let r = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 150.0 * s), Vec2::new((rect.width() * 0.7).min(980.0 * s), 86.0 * s));
        p.rect_filled(r, 10.0, Color32::from_rgba_unmultiplied(28, 16, 8, (a * 225.0) as u8));
        p.rect_stroke(r, 10.0, Stroke::new(3.0, Color32::from_rgba_unmultiplied(255, 190, 70, (a * 255.0) as u8)), egui::StrokeKind::Inside);
        p.text(r.center_top() + Vec2::new(0.0, 18.0 * s), Align2::CENTER_CENTER, "ANNOUNCEMENT", FontId::proportional(14.0 * s), Color32::from_rgba_unmultiplied(255, 200, 110, (a * 255.0) as u8));
        p.text(r.center() + Vec2::new(0.0, 10.0 * s), Align2::CENTER_CENTER, t, FontId::proportional(28.0 * s), Color32::from_rgba_unmultiplied(255, 250, 235, (a * 255.0) as u8));
    }
    if g.admin.gated {
        p.text(Pos2::new(rect.right() - 16.0 * s, rect.bottom() - 16.0 * s), Align2::RIGHT_BOTTOM, "admin session: no records", FontId::proportional(12.0 * s), Color32::from_rgba_unmultiplied(240, 200, 120, 170));
    }
}

/// Test switch DUEL_GALLERY=admin:<bank|help|gift|badges> (with DUEL_HUB=admin): who's who for
/// the pictures, held each frame. Slot 1 is the game's owner, slot 2 an admin, the rest players;
/// their lines are in the chat.
pub fn stage(g: &mut Game, page: &str) {
    let owner = crate::roles::Roster::parse(crate::roles::DEFAULT_OWNERS).owners.first().copied().unwrap_or(1);
    let admin_id = 76561198000000042;
    if !g.admin.roster.admins.contains(&admin_id) {
        g.admin.roster.admins.push(admin_id);
        g.admin.roster.owners = vec![owner];
    }
    for (s, id, name) in [(1usize, owner, "weskk"), (2, admin_id, "Mira"), (3, 76561198000000077, "Oskar")] {
        if s < g.f.len() && g.f[s].present {
            g.f[s].is_bot = false;
            g.f[s].name = name.into();
            g.set_verified_id(s, id);
        }
    }
    if g.chat.is_empty() {
        g.add_chat(1, "Welcome to the new Bank of Varrow!");
        g.add_chat(2, "Raid at eight, bring your best gear");
        g.add_chat(3, "can someone gift me the red moon dye? :)");
    }
    for c in g.chat.iter_mut() {
        c.life = 30.0;
    }
    if page == "help" {
        g.admin.show_help = true;
    }
    if page == "badges" {
        let me = g.local;
        for (k, s) in [1usize, 2, 3].into_iter().enumerate() {
            if s < g.f.len() {
                let p = g.f[me].pos + v3(-2.2 + k as f32 * 2.2, 0.0, 4.2 + (k % 2) as f32 * 0.6);
                let f = &mut g.f[s];
                f.pos = p;
                f.prev_pos = p;
                f.vel = V3::ZERO;
                f.yaw = std::f32::consts::PI;
                f.hub.look.pet = if s == 1 { 1 } else { 0 };
            }
        }
    }
}
