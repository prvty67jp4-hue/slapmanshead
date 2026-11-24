//! The Varrow Hub on screen (crate::hub has the rules): what the keys do, the panels (the
//! wardrobe with its mirror, the emote wheel, photo mode, the game tables, the Gallery's
//! lecterns, the Challenge Boards and scoreboards, challenges asked of you), the hub's part of
//! the HUD (the aim room, the Rooftop Run, a pit duel), its cameras, the hub save, and the
//! scenes the picture-taking test switch (DUEL_HUB) sets up.
//!
//! Keys in the hub: E use what's in front of you (sit or stand, the wardrobe, the mirror, a
//! lectern, a board), F challenge the player you're looking at, G the emote wheel (1-8 pick),
//! P photo mode (WASD and the mouse fly, Space/Ctrl up and down, H hides the HUD, 1-8 pose,
//! Enter takes the photo), Y / N answer a challenge, Esc closes a panel, then opens the menu.

use super::*;
use crate::boards::{self, AnyMove, Board, CMove, GameKind, Level, Outcome};
use crate::hub::{self, DuelKind, Emote, HubSave, LookItem, Outfit, Slot, TableAct, LOOKS, SEAT_NONE, TABLE_SEATS};
use crate::hub_aim::Drill;
use crate::varrow;

/// What's open in the hub.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HubPanel {
    Wardrobe,
    Lectern(usize),
    /// A Challenge Board (0 west, 1 east), the Gallery's scoreboard (2), the Run's (3).
    Board(usize),
    /// Challenging this player.
    Challenge(usize),
}

#[derive(Clone, Copy, Debug)]
pub struct Photo {
    pub pos: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub hud: bool,
    /// A white flash after a photo.
    pub flash: f32,
}

#[derive(Default)]
pub struct HubUi {
    pub panel: Option<HubPanel>,
    /// The mirror: the camera faces you.
    pub mirror: bool,
    pub wheel: bool,
    pub photo: Option<Photo>,
    /// The table you've selected a square on, and the square.
    pub sel: Option<(usize, u8)>,
    pub save: HubSave,
    pub loaded: bool,
    pub outfit_name: String,
    /// Seconds to the next autosave, and whether there's something to save.
    pub save_t: f32,
    pub dirty: bool,
    /// The hub kept while the menu is open (on this computer only).
    pub drill: usize,
    pub aim_tab: usize,
    /// A photo being written (the screenshot request is out).
    pub photo_req: bool,
    pub photo_pending: bool,
    pub scene_done: bool,
    /// (picture scenes) everything unlocked, and how many regulars.
    pub unlock_all: bool,
    pub bots: Option<usize>,
    /// The wardrobe: the slot picked on the left, how far the preview has turned (radians), when
    /// it was last dragged (game time; it turns by itself after a while), the icons.
    pub ward_slot: usize,
    pub spin: f32,
    pub spun_at: f64,
    pub icons: Option<egui::TextureHandle>,
    /// Saved outfits' thumbnails (by the outfit's key), the ones that have none on disk, a
    /// thumbnail being taken (its key) and the frame it waits for.
    pub thumbs: std::collections::HashMap<u64, egui::TextureHandle>,
    pub no_thumb: std::collections::HashSet<u64>,
    pub thumb_req: Option<u64>,
    pub thumb_pending: Option<u64>,
    pub thumb_slot: std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<egui::ColorImage>>>>,
    /// Where the preview is on screen (the thumbnail is cut from there).
    pub preview: Option<Rect>,
    /// Turn the view to this heading (the wardrobe turns you to face the room).
    pub turn_to: Option<f32>,
}

/// A saved outfit's key (its name and what it is), for its thumbnail file.
pub fn outfit_key(name: &str, o: &Outfit) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut h);
    o.to_text().hash(&mut h);
    h.finish()
}

fn thumb_path(key: u64) -> Option<PathBuf> {
    hub_save_path().and_then(|p| p.parent().map(|d| d.join("outfits").join(format!("{key:016x}.png"))))
}

/// The wardrobe's icons (tools/blender/build_look_icons.py): one per look, in hub::LOOKS order,
/// 8 a row.
const LOOK_ICONS: &[u8] = include_bytes!("../assets/ui/look_icons.png");
const ICON_COLS: usize = 8;

fn icon_uv(id: u8) -> Option<Rect> {
    let k = LOOKS.iter().position(|l| l.id == id)?;
    let rows = LOOKS.len().div_ceil(ICON_COLS) as f32;
    let (c, r) = ((k % ICON_COLS) as f32, (k / ICON_COLS) as f32);
    Some(Rect::from_min_max(Pos2::new(c / ICON_COLS as f32, r / rows), Pos2::new((c + 1.0) / ICON_COLS as f32, (r + 1.0) / rows)))
}

impl HubUi {
    /// A panel (or a wheel) wants the mouse: the view stops following it.
    pub fn blocks(&self) -> bool {
        self.panel.is_some() || self.wheel
    }
}

pub fn hub_save_path() -> Option<PathBuf> {
    settings_path().and_then(|p| p.parent().map(|d| d.join("hub.txt")))
}

fn load_save() -> HubSave {
    hub_save_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| HubSave::from_text(&t)).unwrap_or_default()
}

/// Where a point is relative to the local fighter (distance on the ground).
fn flat_dist(a: V3, b: V3) -> f32 {
    (a - b).flat().len()
}

impl App {
    /// Load into Varrow Centre: on this computer with its regulars, or as a host others join.
    pub fn start_hub(&mut self, host: bool) {
        if !self.hub_ui.loaded {
            self.hub_ui.save = load_save();
            self.hub_ui.loaded = true;
        }
        let name = clean_name(&self.settings.name, 0);
        let regulars: usize = self.hub_ui.bots.unwrap_or(9);
        let mut net = None;
        let kind = if host {
            let via_steam = self.settings.use_steam && self.steam.is_some();
            match Net::host(net::DEFAULT_PORT).or_else(|_| Net::host(0)) {
                Ok(mut n) => {
                    n.cap = hub::HUB_PLAYERS;
                    if via_steam {
                        if let Some(st) = self.steam.as_mut() {
                            n.steam = Some(st.client.clone());
                            st.map_name = "Varrow Centre".into();
                            st.create_lobby(self.settings.steam_public);
                        }
                    }
                    net = Some(n);
                    Kind::Host
                }
                Err(e) => {
                    self.menu_msg = Some(e);
                    Kind::Offline
                }
            }
        } else {
            Kind::Offline
        };
        let mut g = Game::hub(&name, if host { regulars.min(6) } else { regulars }, kind);
        if let Some(n) = net.as_mut() {
            n.soft = (1..=regulars.min(6)).collect();
            g.say(format!("Varrow Centre is open to friends: {}", n.address()));
        }
        self.apply_hub_save(&mut g);
        self.start_game(g, net);
        if let Screen::Play { game, .. } = &mut self.screen {
            let me = game.local;
            game.apply_look_weapon(me);
        }
    }

    /// The save on your fighter: your look (only what's earned), the run's best and ghost,
    /// the aim bests and what's been earned.
    fn apply_hub_save(&mut self, g: &mut Game) {
        let mut earned = self.hub_ui.save.earned;
        earned.mastery = (0..chars::COUNT as u8).map(|b| self.career.mastery_level(b)).max().unwrap_or(1);
        if self.hub_ui.unlock_all || std::env::var_os("DUEL_UNLOCK_ALL").is_some() {
            earned = hub::Earned { rooftop: true, table_hard: true, pit_wins: 99, mastery: 99 };
        }
        let me = g.local;
        g.f[me].hub.look = self.hub_ui.save.look.checked(&earned);
        if let Some(h) = g.hub.as_mut() {
            h.earned = earned;
            h.ghost = self.hub_ui.save.ghost.clone();
            h.run_best = self.hub_ui.save.run_best;
            h.aim_best = self.hub_ui.save.aim_best;
        }
    }

    /// Keep the save up to date with the hub (every few seconds, and when leaving).
    pub fn hub_frame(&mut self, dt: f32) {
        let Screen::Play { game, .. } = &self.screen else { return };
        let Some(h) = game.hub.as_ref() else { return };
        let me = game.local;
        let s = &mut self.hub_ui.save;
        let mut changed = false;
        if s.look != game.f[me].hub.look {
            s.look = game.f[me].hub.look;
            changed = true;
        }
        let e = hub::Earned { mastery: 0, ..h.earned };
        let old = hub::Earned { mastery: 0, ..s.earned };
        if e.rooftop && !old.rooftop || e.table_hard && !old.table_hard || e.pit_wins > old.pit_wins {
            s.earned = hub::Earned { rooftop: e.rooftop || old.rooftop, table_hard: e.table_hard || old.table_hard, pit_wins: e.pit_wins.max(old.pit_wins), mastery: 0 };
            changed = true;
        }
        if h.run_best != 0 && (s.run_best == 0 || h.run_best < s.run_best) {
            s.run_best = h.run_best;
            s.ghost = h.ghost.clone();
            changed = true;
        }
        for k in 0..crate::hub_aim::DRILLS {
            if h.aim_best[k] > s.aim_best[k] {
                s.aim_best[k] = h.aim_best[k];
                changed = true;
            }
        }
        self.hub_ui.dirty |= changed;
        self.hub_ui.save_t -= dt;
        if self.hub_ui.dirty && self.hub_ui.save_t <= 0.0 {
            self.write_hub_save();
        }
        if let Some(p) = self.hub_ui.photo.as_mut() {
            p.flash = (p.flash - dt * 3.0).max(0.0);
        }
    }

    pub fn write_hub_save(&mut self) {
        self.hub_ui.save_t = 10.0;
        self.hub_ui.dirty = false;
        if std::env::var_os("DUEL_DATA_DIR").is_some() && self.screenshot.is_some() {
            return; // (a picture-taking run never writes)
        }
        if let Some(p) = hub_save_path() {
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(p, self.hub_ui.save.to_text());
        }
    }

    /// The hub's keys (before the usual ones). Returns true when it used Esc.
    /// Returns 0 when it didn't use Esc, 1 when it did, 2 for "open the menu".
    pub fn hub_keys(&mut self, ctx: &egui::Context) -> u8 {
        let Screen::Play { game, net } = &mut self.screen else { return 0 };
        if !game.in_hub() || self.chat_open {
            return 0;
        }
        let me = game.local;
        let online = net.is_some();
        let (esc, e, f, g_key, p_key, y, n, h_key, enter) = ctx.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::E), i.key_pressed(Key::F), i.key_pressed(Key::G), i.key_pressed(Key::P), i.key_pressed(Key::Y), i.key_pressed(Key::N), i.key_pressed(Key::H), i.key_pressed(Key::Enter)));
        let nums: Vec<usize> = ctx.input(|i| [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8].iter().enumerate().filter(|(_, k)| i.key_pressed(**k)).map(|(k, _)| k).collect());
        let ui = &mut self.hub_ui;
        // a challenge asked of you
        if let Some(c) = game.hub.as_ref().and_then(|h| h.asked) {
            if y {
                game.hub_answer(c.id, true);
            } else if n {
                game.hub_answer(c.id, false);
            }
        }
        if let Some((from, drill)) = game.hub.as_ref().and_then(|h| h.duo_ask) {
            if y {
                game.aim_duo_yes(from, drill);
            } else if n {
                if let Some(h) = game.hub.as_mut() {
                    h.duo_ask = None;
                }
            }
        }
        // photo mode
        if let Some(ph) = ui.photo.as_mut() {
            if esc || p_key {
                ui.photo = None;
                return 1;
            }
            if h_key {
                ph.hud = !ph.hud;
            }
            if enter {
                ui.photo_req = true;
            }
            if let Some(&k) = nums.first() {
                game.hub_emote(Emote::WHEEL[k]);
            }
            return esc as u8;
        }
        if p_key && ui.panel.is_none() {
            let (pos, _, _) = game.camera_rig(game.f[me].pos, self.aim_yaw, self.aim_pitch);
            ui.photo = Some(Photo { pos: pos - look_dir(self.aim_yaw, 0.0) * 2.4 + v3(0.0, 0.3, 0.0), yaw: self.aim_yaw, pitch: self.aim_pitch.min(0.0) - 0.05, hud: false, flash: 0.0 });
            ui.wheel = false;
            return 0;
        }
        // the emote wheel
        if g_key {
            ui.wheel = !ui.wheel;
        }
        if ui.wheel {
            if let Some(&k) = nums.first() {
                game.hub_emote(Emote::WHEEL[k]);
                ui.wheel = false;
            }
        }
        if esc {
            if ui.wheel {
                ui.wheel = false;
                return 1;
            }
            if ui.panel.is_some() || ui.mirror {
                ui.panel = None;
                ui.mirror = false;
                return 1;
            }
            if game.hub.as_ref().is_some_and(|h| h.aim.is_some()) {
                game.stop_aim();
                return 1;
            }
            // the menu is one key away (online, the pause menu: the others carry on)
            return if online { 0 } else { 2 };
        }
        if e && ui.panel.is_none() {
            let pos = game.f[me].pos;
            if game.f[me].hub.seat != SEAT_NONE {
                game.hub_sit_toggle();
            } else if flat_dist(pos, v3(varrow::MIRROR.0, 0.0, varrow::MIRROR.2)) < 3.2 || varrow::WARDROBES.iter().any(|w| flat_dist(pos, v3(w.0, 0.0, w.2)) < 2.0) {
                ui.panel = Some(HubPanel::Wardrobe);
                ui.mirror = true;
                // turn round to face the room: the preview camera stands in front of you
                let mirror = v3(varrow::MIRROR.0, 0.0, varrow::MIRROR.2);
                let away = (pos - mirror).flat();
                let away = if away.len() > 0.1 { away.norm() } else { v3(0.0, 0.0, 1.0) };
                ui.turn_to = Some(away.x.atan2(away.z));
                ui.spin = 0.4;
                ui.spun_at = game.time as f64 + 2.0;
            } else if let Some(k) = varrow::LECTERNS.iter().position(|l| flat_dist(pos, v3(l.0, 0.0, l.2)) < 1.8) {
                ui.panel = Some(HubPanel::Lectern(k));
            } else if let Some(k) = [varrow::BOARDS[0], varrow::BOARDS[1], varrow::GALLERY_BOARD, varrow::RUN_BOARD].iter().position(|b| flat_dist(pos, v3(b.0, 0.0, b.2)) < 3.0) {
                ui.panel = Some(HubPanel::Board(k));
            } else if !game.hub_sit_toggle() {
                // nothing to sit on: on the podium, strike a pose
                let ((x0, x1), (z0, z1), _) = varrow::PODIUM;
                if pos.x > x0 && pos.x < x1 && pos.z > z0 && pos.z < z1 {
                    let k = (game.time * 10.0) as usize % 4;
                    game.hub_emote([Emote::Bow, Emote::Salute, Emote::Cheer, Emote::Wave][k]);
                }
            }
        }
        if f && ui.panel.is_none() {
            if let Some(t) = looked_at(game, me, 4.5) {
                ui.panel = Some(HubPanel::Challenge(t));
            }
        }
        0
    }

    /// The hub's panels (and the emote wheel). Called while the hub is on screen.
    pub fn hub_panels(&mut self, ctx: &egui::Context, rect: Rect) {
        let Screen::Play { game, .. } = &mut self.screen else { return };
        if !game.in_hub() {
            return;
        }
        let ui_state = &mut self.hub_ui;
        if ui_state.photo.is_some() {
            return;
        }
        let me = game.local;
        let s = (rect.height() / 900.0).clamp(0.75, 1.6);
        // ---- the emote wheel
        if ui_state.wheel {
            let c = rect.center();
            egui::Area::new(egui::Id::unique("hub-wheel")).fixed_pos(c - Vec2::new(170.0, 170.0) * s).order(egui::Order::Foreground).show(ctx, |ui| {
                ui.set_width(340.0 * s);
                ui.set_height(340.0 * s);
                let r = 120.0 * s;
                for (k, e) in Emote::WHEEL.iter().enumerate() {
                    let a = k as f32 / 8.0 * std::f32::consts::TAU;
                    let p = c + Vec2::new(a.sin() * r, -a.cos() * r);
                    let b = Rect::from_center_size(p, Vec2::new(104.0, 40.0) * s);
                    let resp = ui.put(b, egui::Button::new(egui::RichText::new(format!("{}  {}", k + 1, e.name())).size(16.0 * s)));
                    if resp.clicked() {
                        game.hub_emote(*e);
                        ui_state.wheel = false;
                    }
                }
                ui.painter().text(c, Align2::CENTER_CENTER, "EMOTES", FontId::proportional(18.0 * s), ACCENT);
            });
        }
        if matches!(ui_state.panel, Some(HubPanel::Wardrobe)) {
            if wardrobe_screen(ctx, rect, s, game, ui_state) {
                ui_state.panel = None;
                ui_state.mirror = false;
            }
            return;
        }
        let Some(panel) = ui_state.panel else {
            // seated at a table: its board
            let seat = game.f[me].hub.seat;
            if seat != SEAT_NONE && seat >= TABLE_SEATS {
                let t = ((seat - TABLE_SEATS) / 2) as usize;
                table_panel(ctx, rect, s, game, t, &mut ui_state.sel, true);
            } else if let Some(t) = varrow::TABLES.iter().position(|tb| flat_dist(game.f[me].pos, v3(tb.0, 0.0, tb.1)) < 2.6 && (game.f[me].pos.y - varrow::DECK).abs() < 1.0) {
                // standing by one: watch it
                table_panel(ctx, rect, s, game, t, &mut ui_state.sel, false);
            }
            return;
        };
        let pos = Pos2::new(rect.right() - 470.0 * s, rect.top() + 90.0 * s);
        let mut close = false;
        egui::Area::new(egui::Id::unique("hub-panel")).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
            ui_kit::panel_frame(ui, |ui| {
                ui.set_width(400.0 * s);
                egui::ScrollArea::vertical().max_height(rect.height() - 200.0 * s).show(ui, |ui| match panel {
                    HubPanel::Wardrobe => {}
                    HubPanel::Lectern(k) => lectern(ui, game, ui_state, k, &mut close),
                    HubPanel::Board(k) => board(ui, game, k, &mut close),
                    HubPanel::Challenge(t) => challenge(ui, game, t, &mut close),
                });
                ui.add_space(8.0);
                if ui.button("Close (Esc)").clicked() {
                    close = true;
                }
            });
        });
        if close {
            ui_state.panel = None;
            ui_state.mirror = false;
        }
    }

    /// The hub's part of the HUD.
    pub fn hub_hud(&self, p: &egui::Painter, rect: Rect, cam: &Cam, g: &Game) {
        let Some(h) = g.hub.as_ref() else { return };
        let s = (rect.height() / 900.0).clamp(0.75, 1.6);
        let me = g.local;
        let f = &g.f[me];
        let mid = rect.center().x;
        // names over heads: nearest first, smaller and fainter further off, and stacked upwards
        // when they would overlap in a crowd (a name with no room left is left out); an emote's
        // name shows under it only for its first moments
        for (sp, size, a, name, emote) in Self::name_tags(g, cam, s) {
            let col = Color32::from_rgba_unmultiplied(235, 230, 220, (a * 235.0) as u8);
            text_shadow(p, sp, Align2::CENTER_BOTTOM, &name, size, col);
            if let Some((e, ea)) = emote {
                text_shadow(p, sp + Vec2::new(0.0, size * 0.95), Align2::CENTER_BOTTOM, e, size * 0.8, Color32::from_rgba_unmultiplied(255, 200, 140, (a * ea * 210.0) as u8));
            }
        }
        // what E (or F) does here
        let prompt = hub_prompt(g, me, &self.hub_ui);
        if let Some(t) = prompt {
            text_shadow(p, Pos2::new(mid, rect.bottom() - 170.0 * s), Align2::CENTER_CENTER, &t, 18.0 * s, Color32::from_rgb(240, 235, 225));
        }
        // a challenge asked of you
        if let Some(c) = h.asked {
            let from = g.f.get(c.from as usize).map_or("Someone", |f| f.name.as_str());
            let r = Rect::from_center_size(Pos2::new(mid, rect.top() + 150.0 * s), Vec2::new(560.0, 74.0) * s);
            p.rect_filled(r, 8.0, Color32::from_rgba_unmultiplied(20, 14, 10, 225));
            p.rect_stroke(r, 8.0, Stroke::new(2.0, ACCENT), egui::StrokeKind::Inside);
            p.text(r.center() - Vec2::new(0.0, 12.0 * s), Align2::CENTER_CENTER, format!("{from} challenges you to a {}", c.kind.name()), FontId::proportional(20.0 * s), Color32::WHITE);
            p.text(r.center() + Vec2::new(0.0, 16.0 * s), Align2::CENTER_CENTER, format!("Y accept  ·  N decline  ·  {:.0}s", c.t.max(0.0)), FontId::proportional(15.0 * s), ACCENT);
        }
        if let Some((from, drill)) = h.duo_ask {
            let who = g.f.get(from as usize).map_or("Someone", |f| f.name.as_str());
            text_shadow(p, Pos2::new(mid, rect.top() + 230.0 * s), Align2::CENTER_CENTER, &format!("{who} wants to race {} side by side  ·  Y yes  ·  N no", Drill::from_u8(drill).name()), 17.0 * s, ACCENT);
        }
        // a pit duel: the score, the bell
        if let Some(pi) = h.pit_of(me) {
            if let Some(d) = h.pits[pi] {
                let side = d.side(me).unwrap_or(0);
                let them = g.f.get(d.who[1 - side] as usize).map_or("?", |f| f.name.as_str());
                let top = format!("{}  ·  you {} - {} {them}  ·  first to {}", d.kind.name(), d.wins[side], d.wins[1 - side], d.first_to);
                text_shadow(p, Pos2::new(mid, rect.top() + 70.0 * s), Align2::CENTER_CENTER, &top, 22.0 * s, Color32::WHITE);
                let big = match d.phase {
                    hub::PitPhase::Intro | hub::PitPhase::Break if d.t > 0.0 => Some(format!("{}", d.t.ceil() as i32)),
                    hub::PitPhase::Over => Some(if d.winner() == Some(side) { "YOU WIN".into() } else { "BEATEN".into() }),
                    _ => None,
                };
                if let Some(b) = big {
                    text_shadow(p, Pos2::new(mid, rect.center().y - 80.0 * s), Align2::CENTER_CENTER, &b, 64.0 * s, ACCENT);
                }
            }
        } else {
            // duels on: a line at the top
            for (k, d) in h.pits.iter().enumerate() {
                if let Some(d) = d {
                    let n = |w: u8| g.f.get(w as usize).map_or("?", |f| f.name.as_str());
                    let t = format!("{}: {} {} - {} {}", if k == 0 { "West Pit" } else { "East Pit" }, n(d.who[0]), d.wins[0], d.wins[1], n(d.who[1]));
                    text_shadow(p, Pos2::new(rect.right() - 20.0 * s, rect.top() + (70.0 + 22.0 * k as f32) * s), Align2::RIGHT_CENTER, &t, 15.0 * s, Color32::from_rgb(230, 220, 200));
                }
            }
        }
        // the aim room
        if let Some(a) = h.aim.as_ref() {
            let r = Rect::from_min_size(Pos2::new(mid - 230.0 * s, rect.top() + 24.0 * s), Vec2::new(460.0, 86.0) * s);
            p.rect_filled(r, 8.0, Color32::from_rgba_unmultiplied(10, 10, 20, 200));
            let title = format!("{}{}", a.drill.name(), if a.duo.is_some() { "  ·  side by side" } else { "" });
            p.text(r.left_top() + Vec2::new(14.0, 10.0) * s, Align2::LEFT_TOP, title, FontId::proportional(18.0 * s), ACCENT);
            let line = if a.t < 0.0 {
                format!("starting in {}", (-a.t).ceil() as i32)
            } else if a.done {
                format!("done: {} points", a.score())
            } else {
                match a.drill {
                    Drill::Tracking => format!("on target {:.1} s  ·  {:.0} s left", a.on_target, (crate::hub_aim::TRACK_TIME - a.t).max(0.0)),
                    Drill::Parry => format!("{} points  ·  blow {} of {}", a.points as u32, a.swings.iter().filter(|w| w.judged).count() + 1, a.swings.len()),
                    _ => format!("{} points  ·  target {}  ·  {:.0}% accuracy", a.points as u32, a.spawned, a.accuracy() * 100.0),
                }
            };
            p.text(r.left_top() + Vec2::new(14.0, 38.0) * s, Align2::LEFT_TOP, line, FontId::proportional(16.0 * s), Color32::WHITE);
            p.text(r.left_top() + Vec2::new(14.0, 60.0) * s, Align2::LEFT_TOP, format!("reaction {} ms  ·  {}", a.reaction_ms(), a.drill.blurb()), FontId::proportional(13.0 * s), Color32::from_gray(190));
            if let Some((c, _)) = &a.call {
                text_shadow(p, Pos2::new(mid, rect.center().y + 70.0 * s), Align2::CENTER_CENTER, c, 26.0 * s, Color32::from_rgb(255, 230, 160));
            }
            if a.drill == Drill::Parry && a.live() {
                let (_, k) = a.post();
                if k > 0.0 && k < 1.0 {
                    let w = 160.0 * s;
                    let br = Rect::from_center_size(Pos2::new(mid, rect.center().y + 110.0 * s), Vec2::new(w, 10.0 * s));
                    p.rect_filled(br, 4.0, Color32::from_black_alpha(160));
                    p.rect_filled(Rect::from_min_size(br.min, Vec2::new(w * k, 10.0 * s)), 4.0, Color32::from_rgb(255, 170, 60));
                }
            }
        }
        if let Some((t, _)) = &h.aim_result {
            text_shadow(p, Pos2::new(mid, rect.top() + 130.0 * s), Align2::CENTER_CENTER, t, 17.0 * s, Color32::from_rgb(255, 230, 170));
        }
        // the Rooftop Run
        if h.run.running {
            let best = if h.run_best > 0 { format!("  ·  best {:.2}", h.run_best as f32 / 1000.0) } else { String::new() };
            let t = format!("ROOFTOP RUN  {:.2} s  ·  ring {}/{}{}", h.run.t, h.run.next.saturating_sub(1), varrow::RUN_RINGS.len() - 2, best);
            text_shadow(p, Pos2::new(mid, rect.top() + 60.0 * s), Align2::CENTER_CENTER, &t, 22.0 * s, Color32::from_rgb(150, 230, 255));
        } else if let Some((t, best)) = h.run.last {
            text_shadow(p, Pos2::new(mid, rect.top() + 60.0 * s), Align2::CENTER_CENTER, &format!("Rooftop Run: {t:.2} s{}", if best { "  ·  YOUR BEST" } else { "" }), 22.0 * s, Color32::from_rgb(150, 230, 255));
        }
        // toasts
        for (k, (t, life)) in h.toasts.iter().rev().enumerate() {
            let a = (life / 0.5).clamp(0.0, 1.0);
            text_shadow(p, Pos2::new(rect.left() + 24.0 * s, rect.top() + rect.height() * 0.36 + k as f32 * 24.0 * s), Align2::LEFT_CENTER, t, 15.0 * s, Color32::from_rgba_unmultiplied(240, 235, 220, (a * 255.0) as u8));
        }
        // entering or leaving a pit: a fade
        if let Some((t, text)) = &h.fade {
            let a = (t / 0.6).clamp(0.0, 1.0);
            p.rect_filled(rect, 0.0, Color32::from_black_alpha((a * 235.0) as u8));
            p.text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(30.0 * s), Color32::from_rgba_unmultiplied(255, 220, 170, (a * 255.0) as u8));
        }
        let _ = f;
    }

    /// The hub's cameras: photo mode's free camera, the mirror, the view over your table.
    pub fn hub_camera(&self, game: &Game, rect: Rect, body: V3) -> Option<Cam> {
        if !game.in_hub() {
            return None;
        }
        let me = game.local;
        if let Some(ph) = self.hub_ui.photo {
            return Some(Cam::new(ph.pos, ph.yaw, ph.pitch, self.settings.fov.min(80.0), rect));
        }
        if self.hub_ui.mirror {
            // the wardrobe's preview: in front of you (turned as far as it's been dragged), the
            // whole fighter framed in the free space between the panels
            let yaw = game.f[me].yaw + self.hub_ui.spin;
            let fwd = look_dir(yaw, 0.0);
            // (nearer if something stands between: never inside a wall)
            let mut dist = 3.6;
            while dist > 1.6 && !game.world.line_clear(body + v3(0.0, 1.1, 0.0), body + fwd * (dist + 0.3) + v3(0.0, 1.2, 0.0)) {
                dist -= 0.2;
            }
            let pos0 = body + fwd * dist + v3(0.0, 1.2, 0.0);
            let d = (body + v3(0.0, 0.78, 0.0) - pos0).norm();
            let right = v3(d.z, 0.0, -d.x).norm();
            // (the field of view is across the picture)
            let fov: f32 = 62.0;
            let shift = match self.hub_ui.preview {
                Some(pr) => {
                    let dx = (pr.center().x - rect.center().x) / rect.width().max(1.0);
                    let w = 2.0 * dist * (fov.to_radians() * 0.5).tan();
                    -dx * w
                }
                None => 0.0,
            };
            let pos = pos0 + right * shift;
            return Some(Cam::new(pos, d.x.atan2(d.z), d.y.asin(), fov, rect));
        }
        let seat = game.f[me].hub.seat;
        if seat != SEAT_NONE && seat >= TABLE_SEATS {
            let t = ((seat - TABLE_SEATS) / 2) as usize;
            let (x, z, _) = varrow::TABLES[t];
            let side = if (seat - TABLE_SEATS) % 2 == 0 { -1.0 } else { 1.0 };
            let pos = v3(x + side * 0.95, varrow::TABLE_TOP + 0.85, z);
            let d = (v3(x, varrow::TABLE_TOP, z) - pos).norm();
            return Some(Cam::new(pos, d.x.atan2(d.z), d.y.asin(), 55.0, rect));
        }
        None
    }

    /// A saved outfit's picture: the next frame is captured, the wardrobe's preview cut out of
    /// it, shrunk and kept (in memory, and as a file beside the hub save).
    pub fn take_outfit_thumb(&mut self, ctx: &egui::Context) {
        let st = &mut self.hub_ui;
        if let Some(key) = st.thumb_req.take() {
            if st.thumb_pending.is_none() && self.screenshot.is_none() {
                let slot = st.thumb_slot.clone();
                ctx.request_screenshot(move |img| {
                    if let Ok(mut s) = slot.lock() {
                        *s = Some(img);
                    }
                });
                st.thumb_pending = Some(key);
            }
        }
        let Some(key) = st.thumb_pending else { return };
        let got = st.thumb_slot.lock().ok().and_then(|mut s| s.take());
        let Some(img) = got else { return };
        st.thumb_pending = None;
        let Some(pr) = st.preview else { return };
        let ppp = ctx.pixels_per_point();
        let [w, h] = img.size;
        // a 4:5 picture round the middle of the preview
        let ph = (pr.height() * ppp).min(h as f32);
        let pw = (ph * 0.8).min(w as f32);
        let x0 = ((pr.center().x * ppp - pw / 2.0).max(0.0) as usize).min(w.saturating_sub(1));
        let y0 = ((pr.top() * ppp).max(0.0) as usize).min(h.saturating_sub(1));
        let (cw, chh) = ((pw as usize).min(w - x0), (ph as usize).min(h - y0));
        if cw < 8 || chh < 8 {
            return;
        }
        let mut crop = image::RgbaImage::new(cw as u32, chh as u32);
        for y in 0..chh {
            for x in 0..cw {
                let c = img.pixels[(y0 + y) * w + x0 + x].to_array();
                crop.put_pixel(x as u32, y as u32, image::Rgba([c[0], c[1], c[2], 255]));
            }
        }
        let small = image::imageops::resize(&crop, 160, 200, image::imageops::FilterType::Triangle);
        if let Some(path) = thumb_path(key) {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = small.save(&path);
        }
        st.no_thumb.remove(&key);
        st.thumbs.insert(key, ctx.load_texture(format!("outfit-{key:x}"), egui::ColorImage::from_rgba_unmultiplied([160, 200], small.as_raw()), egui::TextureOptions::LINEAR));
    }

    /// Photo mode: fly the camera (WASD, the mouse, Space and Ctrl).
    pub fn photo_fly(&mut self, ctx: &egui::Context, dt: f32) {
        let Some(ph) = self.hub_ui.photo.as_mut() else { return };
        let sens = self.settings.sens * 0.0024;
        let (w, a, s, d, up, down, md) = ctx.input(|i| {
            let mut m = Vec2::ZERO;
            for e in &i.events {
                if let egui::Event::MouseMoved(v) = e {
                    m += *v;
                }
            }
            (i.key_down(Key::W), i.key_down(Key::A), i.key_down(Key::S), i.key_down(Key::D), i.key_down(Key::Space), i.modifiers.ctrl, m)
        });
        ph.yaw = wrap_angle(ph.yaw + md.x * sens);
        ph.pitch = (ph.pitch - md.y * sens).clamp(-1.5, 1.5);
        let fwd = look_dir(ph.yaw, ph.pitch);
        let right = v3(fwd.z, 0.0, -fwd.x).norm();
        let mut v = V3::ZERO;
        if w {
            v = v + fwd;
        }
        if s {
            v = v - fwd;
        }
        if d {
            v = v + right;
        }
        if a {
            v = v - right;
        }
        if up {
            v = v + V3::UP;
        }
        if down {
            v = v - V3::UP;
        }
        ph.pos = ph.pos + v * (4.0 * dt);
        let h = varrow::HALF - 0.5;
        ph.pos = v3(ph.pos.x.clamp(-h, h), ph.pos.y.clamp(0.3, 14.0), ph.pos.z.clamp(-h, h));
    }
}

/// The player in front of you (looked at, within reach).
fn looked_at(g: &Game, me: usize, reach: f32) -> Option<usize> {
    let f = &g.f[me];
    let eye = f.eye();
    let dir = look_dir(f.yaw, f.pitch);
    let mut best: Option<(f32, usize)> = None;
    for (k, o) in g.f.iter().enumerate() {
        if k == me || !o.present || !o.alive {
            continue;
        }
        let to = o.chest() - eye;
        let d = to.len();
        if d > reach || to.norm().dot(dir) < 0.9 {
            continue;
        }
        if best.map_or(true, |b| d < b.0) {
            best = Some((d, k));
        }
    }
    best.map(|b| b.1)
}

/// What the keys would do right here.
fn hub_prompt(g: &Game, me: usize, ui: &HubUi) -> Option<String> {
    if ui.panel.is_some() {
        return None;
    }
    let f = &g.f[me];
    let pos = f.pos;
    if f.hub.pit != 255 {
        return None;
    }
    if f.hub.seat != SEAT_NONE {
        return Some("E stand up  ·  G emotes".into());
    }
    if g.hub.as_ref().is_some_and(|h| h.aim.is_some()) {
        return Some("Esc leave the aim room".into());
    }
    if flat_dist(pos, v3(varrow::MIRROR.0, 0.0, varrow::MIRROR.2)) < 3.2 || varrow::WARDROBES.iter().any(|w| flat_dist(pos, v3(w.0, 0.0, w.2)) < 2.0) {
        return Some("E the wardrobe and the mirror".into());
    }
    if varrow::LECTERNS.iter().any(|l| flat_dist(pos, v3(l.0, 0.0, l.2)) < 1.8) {
        return Some("E pick an aim room".into());
    }
    if let Some(k) = [varrow::BOARDS[0], varrow::BOARDS[1], varrow::GALLERY_BOARD, varrow::RUN_BOARD].iter().position(|b| flat_dist(pos, v3(b.0, 0.0, b.2)) < 3.0) {
        return Some(if k < 2 { "E read the Challenge Board".into() } else { "E read the scoreboard".into() });
    }
    if let Some(t) = looked_at(g, me, 4.5) {
        return Some(format!("F challenge {}  ·  G emotes  ·  P photo mode", g.f[t].name));
    }
    if hub::free_seat_near(g, pos, 1.6, me).is_some() {
        return Some("E sit".into());
    }
    let ((x0, x1), (z0, z1), _) = varrow::PODIUM;
    if pos.x > x0 && pos.x < x1 && pos.z > z0 && pos.z < z1 {
        return Some("On the podium: E strike a pose  ·  P photo mode".into());
    }
    None
}

// ================================================================================== panels

fn title(ui: &mut egui::Ui, t: &str, s: f32) {
    ui.label(egui::RichText::new(t).size(22.0 * s).color(ACCENT).strong());
}

fn dye_combo(ui: &mut egui::Ui, id: String, v: &mut u8) -> bool {
    let before = *v;
    egui::ComboBox::from_id_salt(id).width(92.0).selected_text(gear::DYES[(*v as usize).min(gear::DYES.len() - 1)].0).show_ui(ui, |ui| {
        for (k, (n, c)) in gear::DYES.iter().enumerate() {
            let col = Color32::from_rgb((c[0].powf(1.0 / 2.2) * 255.0) as u8, (c[1].powf(1.0 / 2.2) * 255.0) as u8, (c[2].powf(1.0 / 2.2) * 255.0) as u8);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 2.0, col);
                ui.selectable_value(v, k as u8, *n);
            });
        }
    });
    *v != before
}

/// A tile you can click: a frame (lit when picked or hovered) round whatever `draw` paints.
fn tile(ui: &mut egui::Ui, size: Vec2, picked: bool, enabled: bool, draw: impl FnOnce(&egui::Painter, Rect)) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(size, if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let p = ui.painter_at(r.expand(2.0));
    let hot = resp.hovered() && enabled;
    let fill = if picked { Color32::from_rgba_unmultiplied(255, 150, 70, 70) } else if hot { Color32::from_rgba_unmultiplied(255, 255, 255, 26) } else { Color32::from_rgba_unmultiplied(10, 9, 18, 170) };
    p.rect_filled(r, 6.0, fill);
    p.rect_stroke(r, 6.0, Stroke::new(if picked { 2.0 } else { 1.0 }, if picked { ACCENT } else { Color32::from_gray(if hot { 170 } else { 80 }) }), egui::StrokeKind::Inside);
    draw(&p, r.shrink(4.0));
    resp
}

/// A look's icon (or, with no model, its name) into `r`; greyed when locked.
fn paint_look(p: &egui::Painter, r: Rect, icons: Option<&egui::TextureHandle>, id: u8, locked: bool) {
    match (icons, icon_uv(id)) {
        (Some(t), Some(uv)) => {
            let tint = if locked { Color32::from_rgba_unmultiplied(90, 90, 90, 150) } else { Color32::WHITE };
            p.image(t.id(), r, uv, tint);
        }
        _ => {
            let name = hub::look(id).map_or("nothing", |l| l.name);
            p.text(r.center(), Align2::CENTER_CENTER, name, FontId::proportional(11.0), Color32::from_gray(if locked { 110 } else { 210 }));
        }
    }
    if locked {
        // a padlock
        let c = r.right_top() + Vec2::new(-11.0, 12.0);
        p.circle_stroke(c - Vec2::new(0.0, 4.0), 4.0, Stroke::new(2.0, Color32::from_gray(220)));
        p.rect_filled(Rect::from_center_size(c + Vec2::new(0.0, 2.0), Vec2::new(12.0, 9.0)), 2.0, Color32::from_gray(220));
    }
}

fn dye_color(k: u8) -> Color32 {
    // (the dyes are linear: shown in sRGB, as the game draws them)
    let c = crate::gear::dye(k);
    let to = |v: f32| (v.max(0.0).powf(1.0 / 2.2) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(to(c[0]), to(c[1]), to(c[2]))
}

/// Fashionscape, the wardrobe: the nine slots down the left (each showing what's in it), your
/// fighter large in the middle turning slowly (drag to turn it), the chosen slot's looks as a
/// grid of icons on the right (locked ones greyed, with how to earn them), its three dye zones
/// as swatches, randomise, copy what someone near you wears, whole sets, and along the bottom
/// the ten saved outfits, each with its picture. Returns true to close.
fn wardrobe_screen(ctx: &egui::Context, rect: Rect, s: f32, g: &mut Game, st: &mut HubUi) -> bool {
    let me = g.local;
    let earned = g.hub.as_ref().map(|h| h.earned).unwrap_or_default();
    let mut o = g.f[me].hub.look;
    let mut close = false;
    if st.icons.is_none() {
        if let Ok(img) = image::load_from_memory(LOOK_ICONS) {
            let img = img.to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            st.icons = Some(ctx.load_texture("look-icons", egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), egui::TextureOptions::LINEAR));
        }
    }
    let icons = st.icons.clone();
    let slot = Slot::ALL[st.ward_slot.min(hub::SLOT_COUNT - 1)];
    // ---- the slots, down the left
    let tile_s = (58.0 * s).min(rect.height() / 11.5);
    egui::Area::new(egui::Id::new("ward-slots")).fixed_pos(Pos2::new(rect.left() + 16.0 * s, rect.top() + 60.0 * s)).order(egui::Order::Foreground).show(ctx, |ui| {
        ui_kit::panel_frame(ui, |ui| {
        ui.set_width(tile_s + 140.0 * s);
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 5.0);
        for (k, sl) in Slot::ALL.iter().enumerate() {
            ui.horizontal(|ui| {
                let id = o.items[k];
                let resp = tile(ui, Vec2::splat(tile_s), k == st.ward_slot, true, |p, r| {
                    if id != 0 {
                        paint_look(p, r, icons.as_ref(), id, false);
                    }
                });
                if resp.clicked() {
                    st.ward_slot = k;
                }
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(sl.name()).strong().size(13.0 * s).color(if k == st.ward_slot { ACCENT } else { Color32::from_gray(220) }));
                    ui.label(egui::RichText::new(hub::look(id).map_or("empty", |l| l.name)).size(11.5 * s).color(Color32::from_gray(215)));
                });
            });
        }
        });
    });
    // ---- the chosen slot: its looks, its dyes, and the buttons, on the right
    let panel_w = 400.0 * s;
    let right = Pos2::new(rect.right() - panel_w - 24.0 * s, rect.top() + 60.0 * s);
    let strip_h = 150.0 * s;
    egui::Area::new(egui::Id::new("ward-panel")).fixed_pos(right).order(egui::Order::Foreground).show(ctx, |ui| {
        ui_kit::panel_frame(ui, |ui| {
            ui.set_width(panel_w);
            egui::ScrollArea::vertical().max_height(rect.height() - strip_h - 110.0 * s).show(ui, |ui| {
                title(ui, &format!("WARDROBE  ·  {}", slot.name().to_uppercase()), s);
                let per_row = 5usize;
                let cell = (panel_w - 24.0) / per_row as f32 - 6.0;
                let mut choices: Vec<Option<&LookItem>> = vec![None];
                choices.extend(LOOKS.iter().filter(|l| l.slot == slot).map(Some));
                for row in choices.chunks(per_row) {
                    ui.horizontal(|ui| {
                        for l in row {
                            let id = l.map_or(0, |l| l.id);
                            let ok = l.is_none_or(|l| earned.has(l.unlock));
                            let resp = tile(ui, Vec2::new(cell, cell), o.items[slot as usize] == id, ok, |p, r| {
                                if id == 0 {
                                    p.text(r.center(), Align2::CENTER_CENTER, "none", FontId::proportional(13.0 * s), Color32::from_gray(170));
                                } else {
                                    paint_look(p, r, icons.as_ref(), id, !ok);
                                }
                            });
                            let tip = match l {
                                Some(l) if ok => format!("{}\n{}", l.name, l.set),
                                Some(l) => format!("{}\n{}\nLocked: {}", l.name, l.set, l.unlock.text()),
                                None => "Nothing in this slot".to_string(),
                            };
                            let resp = resp.on_hover_text(tip);
                            if resp.clicked() && ok {
                                o.items[slot as usize] = id;
                                if let Some(l) = l {
                                    // a look from a set comes in that set's colours
                                    o.dyes[slot as usize] = hub::set_dyes(l.set);
                                }
                            }
                        }
                    });
                }
                // what the hovered or chosen look is, and how to earn the locked ones
                let locked: Vec<&LookItem> = LOOKS.iter().filter(|l| l.slot == slot && !earned.has(l.unlock)).collect();
                if let Some(l) = hub::look(o.items[slot as usize]) {
                    ui.label(egui::RichText::new(format!("{}  ·  {}", l.name, l.set)).strong().color(Color32::WHITE));
                }
                for l in locked.iter().take(3) {
                    ui.label(egui::RichText::new(format!("Locked: {} - {}", l.name, l.unlock.text())).size(11.5 * s).color(Color32::from_gray(150)));
                }
                // the dye zones as swatches
                let dyed = hub::look(o.items[slot as usize]).is_some_and(|l| l.piece != 0);
                if dyed {
                    ui.add_space(6.0);
                    for (z, zone) in crate::gear::DYE_ZONES.iter().enumerate() {
                        let cur = o.dyes[slot as usize][z];
                        ui.label(egui::RichText::new(format!("{zone}: {}", crate::gear::DYES[cur as usize % crate::gear::DYES.len()].0)).size(12.5 * s).color(Color32::from_gray(210)));
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(3.0, 3.0);
                            for (k, (name, _)) in crate::gear::DYES.iter().enumerate() {
                                let k = k as u8;
                                let col = dye_color(k);
                                let resp = tile(ui, Vec2::splat(11.5 * s), cur == k, true, |p, r| {
                                    p.rect_filled(r.expand(2.0), 3.0, col);
                                });
                                if resp.on_hover_text(*name).clicked() {
                                    o.dyes[slot as usize][z] = k;
                                }
                            }
                        });
                    }
                }
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Randomise").on_hover_text("A random look in every slot from what you've earned, in random dyes").clicked() {
                        o = randomised(g.time, &earned);
                    }
                    if ui.button("Take everything off").clicked() {
                        o = Outfit::default();
                    }
                });
                // copy what someone near you wears (what you've earned of it)
                let near: Vec<(usize, String)> = g.f.iter().enumerate().filter(|(k, f)| *k != me && f.present && (f.pos - g.f[me].pos).flat().len() < 25.0).map(|(k, f)| (k, f.name.clone())).collect();
                ui.horizontal(|ui| {
                    ui.label("Copy from");
                    egui::ComboBox::from_id_salt("ward-copy").width(170.0 * s).selected_text("a player near you").show_ui(ui, |ui| {
                        if near.is_empty() {
                            ui.label("nobody near");
                        }
                        for (k, n) in &near {
                            if ui.selectable_label(false, n).clicked() {
                                o = g.f[*k].hub.look.checked(&earned);
                            }
                        }
                    });
                });
                ui.label(egui::RichText::new("Whole sets").strong());
                ui.horizontal_wrapped(|ui| {
                    for set in hub::sets() {
                        let ok = LOOKS.iter().filter(|l| l.set == set).all(|l| earned.has(l.unlock));
                        let resp = ui.add_enabled(ok, egui::Button::new(set));
                        if resp.clicked() {
                            o = o.with_set(set);
                        }
                        if !ok {
                            if let Some(l) = LOOKS.iter().find(|l| l.set == set) {
                                resp.on_disabled_hover_text(format!("To unlock: {}", l.unlock.text()));
                            }
                        }
                    }
                });
                ui.add_space(6.0);
                if ui.button("Close (Esc)").clicked() {
                    close = true;
                }
            });
        });
    });
    // ---- the saved outfits along the bottom, each with its picture
    let n_slots = hub::OUTFIT_SAVES;
    let cw = ((rect.width() - 48.0 * s) / n_slots as f32 - 8.0).min(96.0 * s);
    let ch = cw * 1.25;
    let strip_x = rect.center().x - (cw + 8.0) * n_slots as f32 / 2.0;
    let mut wear = None;
    let mut save_at = None;
    let mut delete = None;
    egui::Area::new(egui::Id::new("ward-outfits")).fixed_pos(Pos2::new(strip_x, rect.bottom() - ch - 46.0 * s)).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);
            for k in 0..n_slots {
                ui.vertical(|ui| {
                    let saved = st.save.outfits.get(k).cloned();
                    let key = saved.as_ref().map(|(n, fit)| outfit_key(n, fit));
                    if let Some(key) = key {
                        if !st.thumbs.contains_key(&key) && !st.no_thumb.contains(&key) {
                            match thumb_path(key).and_then(|p| std::fs::read(p).ok()).and_then(|b| image::load_from_memory(&b).ok()) {
                                Some(img) => {
                                    let img = img.to_rgba8();
                                    let size = [img.width() as usize, img.height() as usize];
                                    st.thumbs.insert(key, ctx.load_texture(format!("outfit-{key:x}"), egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), egui::TextureOptions::LINEAR));
                                }
                                None => {
                                    st.no_thumb.insert(key);
                                }
                            }
                        }
                    }
                    let thumb = key.and_then(|k| st.thumbs.get(&k).cloned());
                    let resp = tile(ui, Vec2::new(cw, ch), false, true, |p, r| match (&saved, &thumb) {
                        (Some(_), Some(t)) => {
                            p.image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                        }
                        (Some((_, fit)), None) => {
                            // no picture yet: its body look's icon
                            let id = fit.items[Slot::Body as usize];
                            if id != 0 {
                                paint_look(p, r, icons.as_ref(), id, false);
                            }
                        }
                        _ => {
                            p.text(r.center(), Align2::CENTER_CENTER, "+\nsave", FontId::proportional(14.0 * s), Color32::from_gray(150));
                        }
                    });
                    let resp = match &saved {
                        Some((n, _)) => resp.on_hover_text(format!("{n}\nClick to wear it. Right-click to save over it.")),
                        None => resp.on_hover_text("Save what you're wearing here"),
                    };
                    if resp.clicked() {
                        match &saved {
                            Some((_, fit)) => wear = Some(fit.checked(&earned)),
                            None => save_at = Some(k),
                        }
                    }
                    if resp.secondary_clicked() {
                        save_at = Some(k);
                    }
                    let label = saved.as_ref().map_or(format!("Outfit {}", k + 1), |(n, _)| n.clone());
                    ui.horizontal(|ui| {
                        ui.set_width(cw);
                        ui.label(egui::RichText::new(label).size(11.0 * s).color(Color32::from_gray(200)));
                        if saved.is_some() && ui.small_button("x").on_hover_text("Delete").clicked() {
                            delete = Some(k);
                        }
                    });
                });
            }
        });
    });
    if let Some(w) = wear {
        o = w;
    }
    if let Some(k) = save_at {
        let fit = o.checked(&earned);
        let name = if k < st.save.outfits.len() { st.save.outfits[k].0.clone() } else { format!("Outfit {}", st.save.outfits.len() + 1) };
        if k < st.save.outfits.len() {
            st.save.outfits[k].1 = fit;
        } else if st.save.outfits.len() < hub::OUTFIT_SAVES {
            st.save.outfits.push((name.clone(), fit));
        }
        // its picture: the preview as it is now, taken from the next frame
        st.thumb_req = Some(outfit_key(&name, &fit));
        st.dirty = true;
        st.save_t = 0.0;
    }
    if let Some(k) = delete {
        if k < st.save.outfits.len() {
            st.save.outfits.remove(k);
            st.dirty = true;
        }
    }
    // ---- the preview: drag anywhere free to turn your fighter; it turns by itself otherwise
    let left_edge = rect.left() + 16.0 * s + tile_s + 170.0 * s;
    let preview = Rect::from_min_max(Pos2::new(left_edge, rect.top() + 40.0 * s), Pos2::new(right.x - 10.0, rect.bottom() - ch - 60.0 * s));
    st.preview = Some(preview);
    let (drag, at) = ctx.input(|i| (if i.pointer.primary_down() { i.pointer.delta().x } else { 0.0 }, i.pointer.interact_pos()));
    if drag != 0.0 && at.is_some_and(|a| preview.contains(a)) {
        st.spin += drag * 0.012;
        st.spun_at = g.time as f64;
    } else if g.time as f64 - st.spun_at > 3.0 {
        st.spin += 0.35 * ctx.input(|i| i.stable_dt).min(0.1);
    }
    st.spin = wrap_angle(st.spin);
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("ward-hint")));
    p.text(Pos2::new(preview.center().x, preview.bottom() - 6.0), Align2::CENTER_BOTTOM, "drag to turn", FontId::proportional(13.0 * s), Color32::from_gray(200));
    let o = o.checked(&earned);
    if o != g.f[me].hub.look {
        g.hub_wear(o);
    }
    st.mirror = true;
    close
}

/// A random outfit from what's been earned: a look in each slot (now and then none in the
/// cape, neck and off hand), each in random dyes.
fn randomised(seed: f32, earned: &hub::Earned) -> Outfit {
    let mut x = (seed.to_bits() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut next = |n: usize| -> usize {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x % n.max(1) as u64) as usize
    };
    let mut o = Outfit::default();
    for slot in Slot::ALL {
        let k = slot as usize;
        let mut pool: Vec<u8> = LOOKS.iter().filter(|l| l.slot == slot && earned.has(l.unlock)).map(|l| l.id).collect();
        if matches!(slot, Slot::Cape | Slot::Neck | Slot::Off) {
            pool.push(0);
        }
        if !pool.is_empty() {
            o.items[k] = pool[next(pool.len())];
        }
        for z in 0..3 {
            o.dyes[k][z] = next(crate::gear::DYES.len()) as u8;
        }
    }
    o.checked(earned)
}

fn lectern(ui: &mut egui::Ui, g: &mut Game, st: &mut HubUi, lane: usize, close: &mut bool) {
    title(ui, "THE GALLERY", 1.0);
    ui.label("Pick a drill. Solo, or side by side with whoever stands at the other lectern.");
    let best = g.hub.as_ref().map(|h| h.aim_best).unwrap_or_default();
    for d in Drill::ALL {
        ui.radio_value(&mut st.drill, d as usize, format!("{}  ·  your best {}", d.name(), best[d as usize]));
        ui.label(egui::RichText::new(d.blurb()).small().color(Color32::from_gray(180)));
    }
    let drill = Drill::ALL[st.drill.min(Drill::ALL.len() - 1)];
    let me = g.local;
    // who's at the other lectern
    let other = varrow::LECTERNS[1 - lane.min(1)];
    let partner = (0..g.f.len()).find(|&k| k != me && g.f[k].present && !g.f[k].is_bot && flat_dist(g.f[k].pos, v3(other.0, 0.0, other.2)) < 2.0);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button(egui::RichText::new("Start solo").strong()).clicked() {
            let seed = (g.time * 997.0) as u32;
            g.start_aim(drill, lane, seed, None);
            *close = true;
        }
        match partner {
            Some(p) => {
                if ui.button(format!("Race {} side by side", g.f[p].name)).clicked() {
                    g.aim_duo(p, drill);
                    *close = true;
                }
            }
            None => {
                ui.add_enabled(false, egui::Button::new("Side by side: someone at the other lectern"));
            }
        }
    });
    ui.add_space(6.0);
    let rows = g.hub.as_ref().and_then(|h| h.boards.get(drill as usize).cloned()).unwrap_or_default();
    ui.label(egui::RichText::new(format!("{}: best in Varrow today", drill.name())).strong());
    for (k, (n, v)) in rows.iter().take(5).enumerate() {
        ui.label(format!("{}. {n}  {v}", k + 1));
    }
}

fn board(ui: &mut egui::Ui, g: &mut Game, k: usize, close: &mut bool) {
    let me = g.local;
    let Some(h) = g.hub.as_ref() else { return };
    match k {
        0 | 1 => {
            title(ui, "THE CHALLENGE BOARD", 1.0);
            ui.label("Post an open challenge anyone can take, or take one up. Walk up to someone and press F to challenge them yourself.");
            let open: Vec<hub::Challenge> = h.challenges.iter().copied().filter(|c| c.to == 255).collect();
            let mut answer = None;
            for c in &open {
                let from = g.f.get(c.from as usize).map_or("?", |f| f.name.as_str());
                ui.horizontal(|ui| {
                    ui.label(format!("{from}: an open {}  ({:.0}s)", c.kind.name(), c.t));
                    if c.from as usize != me && ui.button("Take it").clicked() {
                        answer = Some((c.id, true));
                    }
                    if c.from as usize == me && ui.button("Withdraw").clicked() {
                        answer = Some((c.id, false));
                    }
                });
            }
            if open.is_empty() {
                ui.label(egui::RichText::new("No open challenges right now.").color(Color32::from_gray(170)));
            }
            let mut post = None;
            ui.horizontal(|ui| {
                if ui.button("Post an open 1v1 Duel").clicked() {
                    post = Some(DuelKind::Duel);
                }
                if ui.button("Post an open Ledger Duel").clicked() {
                    post = Some(DuelKind::Ledger);
                }
            });
            ui.add_space(6.0);
            ui.label(egui::RichText::new("In the pits").strong());
            for (p, d) in h.pits.iter().enumerate() {
                let line = match d {
                    Some(d) => format!("{}: {} {} - {} {} ({})", if p == 0 { "West Pit" } else { "East Pit" }, g.f[d.who[0] as usize].name, d.wins[0], d.wins[1], g.f[d.who[1] as usize].name, d.kind.name()),
                    None => format!("{}: free", if p == 0 { "West Pit" } else { "East Pit" }),
                };
                ui.label(line);
            }
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Lately").strong());
            for r in h.results.iter().take(8) {
                ui.label(egui::RichText::new(r).small());
            }
            if let Some((id, yes)) = answer {
                g.hub_answer(id, yes);
                *close = true;
            }
            if let Some(kind) = post {
                g.hub_challenge(255, kind);
                *close = true;
            }
        }
        2 => {
            title(ui, "THE GALLERY'S SCORES", 1.0);
            for d in Drill::ALL {
                ui.label(egui::RichText::new(d.name()).strong());
                let rows = &h.boards[d as usize];
                if rows.is_empty() {
                    ui.label(egui::RichText::new("no scores yet").small().color(Color32::from_gray(160)));
                }
                for (k, (n, v)) in rows.iter().take(5).enumerate() {
                    ui.label(format!("{}. {n}  {v}", k + 1));
                }
                ui.label(egui::RichText::new(format!("your best: {}", h.aim_best[d as usize])).small().color(ACCENT));
            }
        }
        _ => {
            title(ui, "THE ROOFTOP RUN", 1.0);
            ui.label("Start at the ring by the crates, up over the pavilion roof, the tower, the gap (dash it, or run the wall), the beam, the pits' walkway, and down to the finish by the terrace steps. Your best run's ghost races you.");
            let rows = &h.boards[hub::BOARD_RUN as usize];
            for (k, (n, v)) in rows.iter().take(8).enumerate() {
                ui.label(format!("{}. {n}  {:.2} s", k + 1, *v as f32 / 1000.0));
            }
            if h.run_best > 0 {
                ui.label(egui::RichText::new(format!("your best: {:.2} s", h.run_best as f32 / 1000.0)).color(ACCENT));
            }
        }
    }
}

fn challenge(ui: &mut egui::Ui, g: &mut Game, t: usize, close: &mut bool) {
    let name = g.f.get(t).map_or("?".to_string(), |f| f.name.clone());
    title(ui, &format!("CHALLENGE {}", name.to_uppercase()), 1.0);
    ui.label("If they say yes, you're both taken to a free Court Pit; first to two rounds (a Ledger Duel: your gear and the Court's rules). Then back here.");
    ui.horizontal(|ui| {
        if ui.button(egui::RichText::new("1v1 Duel").strong()).clicked() {
            g.hub_challenge(t as u8, DuelKind::Duel);
            *close = true;
        }
        if ui.button(egui::RichText::new("Ledger Duel").strong()).clicked() {
            g.hub_challenge(t as u8, DuelKind::Ledger);
            *close = true;
        }
    });
}

/// A table's board: yours to play when seated (click a piece, then where it goes), or to watch.
fn table_panel(ctx: &egui::Context, rect: Rect, s: f32, g: &mut Game, t: usize, sel: &mut Option<(usize, u8)>, seated: bool) {
    let me = g.local;
    let Some(h) = g.hub.as_ref() else { return };
    let tb = &h.tables[t];
    let kind = tb.kind();
    let my_side = tb.side_of(me);
    let names = |k: usize| match tb.seats[k] {
        hub::Sitter::Empty => "(empty)".to_string(),
        hub::Sitter::Player(p) => g.f.get(p as usize).map_or("?".into(), |f| f.name.clone()),
        hub::Sitter::Bot(l) => format!("Table bot ({})", l.name()),
    };
    let sides = kind.sides();
    let head = format!("{}: {} ({}) vs {} ({})", kind.name(), names(0), sides[0], names(1), sides[1]);
    let status = match tb.outcome {
        Outcome::Playing if tb.playing() => format!("{} to move{}", sides[tb.board.turn() as usize], if my_side == Some(tb.board.turn() as usize) { "  ·  your move" } else { "" }),
        Outcome::Playing => "waiting for a second player (or a table bot)".into(),
        o => o.text(sides),
    };
    let board = tb.board.clone();
    let last = tb.last.clone();
    let can_move = seated && tb.playing() && my_side == Some(tb.board.turn() as usize);
    let other_free = my_side.is_some_and(|m| matches!(tb.seats[1 - m], hub::Sitter::Empty | hub::Sitter::Bot(_)));
    let over = tb.outcome != Outcome::Playing;
    let size = if seated { 46.0 } else { 26.0 } * s;
    let mut act: Option<TableAct> = None;
    let pos = if seated { Pos2::new(rect.right() - size * 8.0 - 70.0 * s, rect.top() + 80.0 * s) } else { Pos2::new(rect.right() - size * 8.0 - 40.0 * s, rect.top() + 140.0 * s) };
    egui::Area::new(egui::Id::unique(["table-0","table-1","table-2","table-3"][t.min(3)])).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
        ui_kit::panel_frame(ui, |ui| {
            ui.label(egui::RichText::new(&head).strong().size(15.0 * s));
            ui.label(egui::RichText::new(&status).color(ACCENT));
            if !last.is_empty() {
                ui.label(egui::RichText::new(format!("last move: {last}")).small());
            }
            let (cols, rows) = if kind == GameKind::Connect4 { (boards::C4_COLS, boards::C4_ROWS) } else { (8, 8) };
            let (r, resp) = ui.allocate_exact_size(Vec2::new(size * cols as f32, size * rows as f32), egui::Sense::click());
            let p = ui.painter();
            // seen from your side (black at the bottom when you play black)
            let flip = my_side == Some(1) && kind != GameKind::Connect4;
            let cell_rect = |c: usize, row: usize| {
                let (c, row) = if flip { (cols - 1 - c, rows - 1 - row) } else { (c, row) };
                Rect::from_min_size(r.min + Vec2::new(c as f32 * size, (rows - 1 - row) as f32 * size), Vec2::splat(size))
            };
            let legal_from: Vec<(u8, u8)> = match &board {
                Board::Chess(c) if can_move => c.legal().iter().map(|m| (m.from, m.to)).collect(),
                Board::Checkers(c) if can_move => c.legal().iter().map(|m| (m.0[0], *m.0.last().unwrap_or(&m.0[0]))).collect(),
                _ => Vec::new(),
            };
            match &board {
                Board::Chess(_) | Board::Checkers(_) => {
                    for sq in 0..64u8 {
                        let cr = cell_rect((sq % 8) as usize, (sq / 8) as usize);
                        let dark = boards::dark(sq);
                        let mut col = if dark { Color32::from_rgb(110, 78, 54) } else { Color32::from_rgb(222, 196, 150) };
                        if *sel == Some((t, sq)) {
                            col = Color32::from_rgb(214, 160, 60);
                        } else if sel.is_some_and(|(tt, f)| tt == t && legal_from.iter().any(|m| m.0 == f && m.1 == sq)) {
                            col = Color32::from_rgb(140, 170, 90);
                        }
                        p.rect_filled(cr, 0.0, col);
                        let (glyph, fg) = match &board {
                            Board::Chess(c) => {
                                let v = c.sq[sq as usize];
                                let g = ["", "♟", "♞", "♝", "♜", "♛", "♚"][v.unsigned_abs().min(6) as usize];
                                (g, if v > 0 { Color32::WHITE } else { Color32::from_rgb(20, 18, 20) })
                            }
                            Board::Checkers(c) => {
                                let v = c.sq[sq as usize];
                                (if v.abs() == 2 { "◉" } else if v != 0 { "●" } else { "" }, if v > 0 { Color32::from_rgb(200, 40, 34) } else { Color32::from_rgb(20, 18, 20) })
                            }
                            _ => ("", Color32::WHITE),
                        };
                        if !glyph.is_empty() {
                            p.text(cr.center() + Vec2::new(1.0, 1.0), Align2::CENTER_CENTER, glyph, FontId::proportional(size * 0.78), Color32::from_black_alpha(120));
                            p.text(cr.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(size * 0.78), fg);
                        }
                    }
                }
                Board::Connect4(c) => {
                    p.rect_filled(r, 6.0, Color32::from_rgb(40, 72, 150));
                    for col in 0..cols {
                        for row in 0..rows {
                            let cr = cell_rect(col, row);
                            let fill = match c.cells[col][row] {
                                1 => Color32::from_rgb(210, 40, 34),
                                2 => Color32::from_rgb(240, 200, 40),
                                _ => Color32::from_rgb(16, 20, 36),
                            };
                            p.circle_filled(cr.center(), size * 0.4, fill);
                        }
                    }
                }
            }
            // a click: a piece of yours, then where it goes (a checkers jump picks its whole run)
            if resp.clicked() && can_move {
                if let Some(at) = resp.interact_pointer_pos() {
                    let v = at - r.min;
                    let mut c = (v.x / size) as usize;
                    let mut row = rows - 1 - (v.y / size).clamp(0.0, rows as f32 - 1.0) as usize;
                    if flip {
                        c = cols - 1 - c;
                        row = rows - 1 - row;
                    }
                    match &board {
                        Board::Connect4(_) => act = Some(TableAct::Move(AnyMove::Drop(c.min(cols - 1) as u8))),
                        Board::Chess(b) => {
                            let sq = (row * 8 + c.min(7)) as u8;
                            match *sel {
                                Some((tt, from)) if tt == t && legal_from.iter().any(|m| m.0 == from && m.1 == sq) => {
                                    act = Some(TableAct::Move(AnyMove::Chess(CMove { from, to: sq, promo: 0 })));
                                    *sel = None;
                                }
                                _ => {
                                    let mine = b.sq[sq as usize] != 0 && (b.sq[sq as usize] > 0) == (b.turn == 0);
                                    *sel = mine.then_some((t, sq));
                                }
                            }
                        }
                        Board::Checkers(b) => {
                            let sq = (row * 8 + c.min(7)) as u8;
                            match *sel {
                                Some((tt, from)) if tt == t => {
                                    if let Some(m) = b.legal().into_iter().find(|m| m.0[0] == from && *m.0.last().unwrap_or(&from) == sq) {
                                        act = Some(TableAct::Move(AnyMove::Checkers(m)));
                                    }
                                    *sel = None;
                                }
                                _ => {
                                    let v = b.sq[sq as usize];
                                    let mine = v != 0 && (v < 0) == (b.turn == 0);
                                    *sel = mine.then_some((t, sq));
                                }
                            }
                        }
                    }
                }
            }
            if seated {
                ui.horizontal(|ui| {
                    if other_free {
                        for l in Level::ALL {
                            if ui.small_button(format!("Bot: {}", l.name())).clicked() {
                                act = Some(TableAct::Bot(l));
                            }
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if over && ui.small_button("New game").clicked() {
                        act = Some(TableAct::Reset);
                    }
                    if !over && tb_playing(&board) && ui.small_button("Resign").clicked() {
                        act = Some(TableAct::Resign);
                    }
                    if ui.small_button("Stand up (E)").clicked() {
                        act = Some(TableAct::Stand);
                    }
                });
                if kind == GameKind::Chess {
                    ui.label(egui::RichText::new("A pawn reaching the far end becomes a queen.").small().color(Color32::from_gray(170)));
                }
            } else {
                ui.label(egui::RichText::new("Watching. Sit in a free chair (E) to play.").small().color(Color32::from_gray(180)));
            }
        });
    });
    if let Some(a) = act {
        if a == TableAct::Stand {
            g.hub_sit_toggle();
        } else {
            g.hub_table(t, a);
        }
    }
}

fn tb_playing(b: &Board) -> bool {
    b.outcome() == Outcome::Playing
}

// ====================================================================== the picture scenes

impl App {
    /// Test switch DUEL_HUB=<scene>: load into the hub and stage it for a picture:
    /// overview, wardrobe, photo, aim, run, chess (DUEL_SHOT_FRAME takes it).
    pub fn hub_scene(&mut self) {
        let Ok(scene) = std::env::var("DUEL_HUB") else { return };
        self.hub_ui.unlock_all = true;
        // (a picture: it runs on while the window isn't focused)
        self.autoplay = true;
        self.hub_ui.bots = Some(if scene == "overview" || scene == "chess" { 15 } else { 9 });
        self.start_hub(false);
        let Screen::Play { game, .. } = &mut self.screen else { return };
        let me = game.local;
        let mut fit = Outfit::default().with_set(hub::HERALD);
        fit.items[Slot::Main as usize] = 61;
        game.hub_wear(fit);
        match scene.as_str() {
            "wardrobe" => {
                game.f[me].respawn_at(v3(varrow::MIRROR.0, 0.0, varrow::MIRROR.2 + 2.6), 0.0);
                self.aim_yaw = 0.0;
                self.hub_ui.panel = Some(HubPanel::Wardrobe);
                self.hub_ui.mirror = true;
                // a few outfits saved already, the coat's slot picked
                self.hub_ui.save.outfits = vec![
                    ("Court day".into(), Outfit::default().with_set(hub::HERALD)),
                    ("Night river".into(), Outfit::default().with_set(hub::SMUGGLER)),
                    ("Ash road".into(), Outfit::default().with_set(hub::PILGRIM)),
                    ("Rigger".into(), Outfit::default().with_set(hub::RIG)),
                ];
                self.hub_ui.ward_slot = Slot::Body as usize;
                self.hub_ui.spin = 0.5;
                self.hub_ui.spun_at = 1e9;
            }
            "photo" => {
                let ((x0, x1), (z0, z1), top) = varrow::PODIUM;
                let at = v3((x0 + x1) * 0.5, top, (z0 + z1) * 0.5);
                game.f[me].respawn_at(at, 0.0);
                self.aim_yaw = 0.0;
                let mut fit = Outfit::default().with_set(hub::SMUGGLER);
                fit.items[Slot::Cape as usize] = 21;
                fit.dyes[Slot::Cape as usize] = [12, 1, 7];
                game.hub_wear(fit);
                game.hub_emote(Emote::Cheer);
                self.hub_ui.drill = 98;
                // in front of the fighter (as they face), a little to the side
                let fwd = look_dir(0.0, 0.0);
                let cam = at + fwd * 2.7 + v3(fwd.z, 0.0, -fwd.x) * 0.9 + v3(0.0, 1.45, 0.0);
                let d = (at + v3(0.0, 1.25, 0.0) - cam).norm();
                self.hub_ui.photo = Some(Photo { pos: cam, yaw: d.x.atan2(d.z), pitch: d.y.asin(), hud: false, flash: 0.0 });
            }
            "aim" => {
                game.start_aim(Drill::Flicks, 0, 1234, None);
                self.aim_yaw = 0.0;
            }
            "chess" => {
                // two regulars at the first chess table, others watching round it
                let (tx, tz, _) = varrow::TABLES[0];
                for (side, s) in [(0u16, 1usize), (1, 2)] {
                    let seat = TABLE_SEATS + side;
                    if let Some((p, yaw)) = hub::seat_pose(seat) {
                        let fwd = v3(yaw.sin(), 0.0, yaw.cos());
                        game.f[s].respawn_at(v3(p.x, varrow::DECK, p.z) + fwd * 0.1, yaw);
                        game.hub_remote(s, hub::HubEv::Sit { who: s as u8, seat });
                    }
                    if let Some(r) = game.hub.as_mut().and_then(|h| h.regulars.get_mut(s)) {
                        r.plan = crate::hub_bots::Plan::Playing(0, 9999.0);
                    }
                }
                // an opening played already
                if let Some(h) = game.hub.as_mut() {
                    let t = &mut h.tables[0];
                    if let Board::Chess(c) = &mut t.board {
                        for (f, to) in [(12u8, 28u8), (52, 36), (6, 21), (57, 42), (5, 26), (61, 34)] {
                            c.play(CMove { from: f, to, promo: 0 });
                        }
                    }
                    t.last = "Bf1-c4".into();
                }
                for (k, s) in [3usize, 4, 5, 6].iter().enumerate() {
                    let a = 0.6 + k as f32 * 0.55;
                    let p = v3(tx + a.cos() * 1.6 * if k % 2 == 0 { 1.0 } else { -1.0 }, varrow::DECK, tz + 1.3 + a.sin() * 0.3);
                    let face = (v3(tx, 0.0, tz) - p).flat();
                    game.f[*s].respawn_at(p, face.x.atan2(face.z));
                    if let Some(r) = game.hub.as_mut().and_then(|h| h.regulars.get_mut(*s)) {
                        r.plan = crate::hub_bots::Plan::Watching(v3(tx, varrow::TABLE_TOP, tz), 9999.0);
                    }
                }
                // you: off to one side, the table and its watchers in view
                let at = v3(tx - 1.5, varrow::DECK, tz - 2.6);
                let face = (v3(tx + 0.3, 0.0, tz) - at).flat();
                game.f[me].respawn_at(at, face.x.atan2(face.z));
                self.aim_yaw = face.x.atan2(face.z);
            }
            "run" => {
                // a best run first (the ghost), then you on the course a little behind it
                let route = crate::hub_bots::course();
                let mut sim = Game::hub("Runner", 0, Kind::Offline);
                sim.f[0].respawn_at(v3(-15.5, 0.0, -20.5), -0.6);
                let mut st = crate::hub_bots::Runner::default();
                for _ in 0..(80.0 / TICK) as usize {
                    let inp = crate::hub_bots::run_input(&sim, 0, &route, &mut st);
                    sim.tick(&inp);
                    if sim.hub.as_ref().is_some_and(|h| h.run.last.is_some()) {
                        break;
                    }
                }
                if let (Some(src), Some(h)) = (sim.hub.as_ref(), game.hub.as_mut()) {
                    // (the scripted runner is the ghost to the frame: the ghost goes a second ahead)
                    h.ghost = src.ghost.iter().map(|g| hub::GhostFrame { t: g.t - 0.9, ..*g }).collect();
                    h.run_best = src.run_best;
                }
                game.f[me].respawn_at(v3(-15.5, 0.0, -20.5), -0.6);
                self.hub_ui.drill = 99;
            }
            _ => {
                // the overview: everyone about the square
                self.aim_yaw = 0.0;
            }
        }
    }

    /// Where the hub's name tags go: (bottom-centre on screen, text size, opacity, name, the emote's
/// name and its own opacity while it's fresh). Nearest first; each is moved up past any tag
/// already placed that it would overlap (at most three steps, else it's left out), fading and
/// shrinking with distance (gone past 18 m), and never seen through a wall.
pub fn name_tags(g: &Game, cam: &Cam, s: f32) -> Vec<(Pos2, f32, f32, String, Option<(&'static str, f32)>)> {
    let me = g.local;
    let mut cands: Vec<(f32, Pos2, usize)> = Vec::new();
    for (k, o) in g.f.iter().enumerate() {
        if k == me || !o.present {
            continue;
        }
        let head = o.pos + v3(0.0, 2.15, 0.0);
        let d = (head - cam.pos).len();
        if d > 18.0 || !g.world.line_clear(cam.pos, head) {
            continue;
        }
        if let Some(sp) = cam.project(head) {
            cands.push((d, sp, k));
        }
    }
    cands.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut placed: Vec<Rect> = Vec::new();
    let mut out = Vec::new();
    for (d, sp, k) in cands {
        let o = &g.f[k];
        let size = (15.0 * s * (1.15 - d / 30.0)).clamp(10.0 * s, 16.0 * s);
        let a = ((18.0 - d) / 7.0).clamp(0.0, 1.0) * (0.55 + 0.45 * (1.0 - d / 18.0).clamp(0.0, 1.0));
        let w = o.name.chars().count() as f32 * size * 0.55 + 8.0;
        let h = size * 1.15;
        let age = g.time - o.hub.emote_t;
        let emote = (o.hub.emote != 0 && age < 2.5).then(|| (Emote::from_u8(o.hub.emote).name(), (1.0 - (age - 1.8) / 0.7).clamp(0.0, 1.0)));
        let mut at = sp;
        let mut ok = false;
        for _ in 0..4 {
            let below = if emote.is_some() { h * 0.8 } else { 0.0 };
            let r = Rect::from_min_max(Pos2::new(at.x - w / 2.0, at.y - h), Pos2::new(at.x + w / 2.0, at.y + below));
            if let Some(hit) = placed.iter().find(|q| q.intersects(r)) {
                at.y = hit.top() - 2.0 - below;
                continue;
            }
            placed.push(r);
            ok = true;
            break;
        }
        if ok {
            out.push((at, size, a, o.name.clone(), emote));
        }
    }
    out
}

/// Test switch DUEL_GALLERY=fit:<herald|smuggler|pilgrim|rig>:<idle|back|run|wall|emote|sit>
/// (with DUEL_HUB=fit): the four fighters, one of each size, in that set and its own colours,
/// side by side on the plaza and held in that pose (you stand out of sight), to check the
/// outfits bend with the body without clipping. Run after each game step, before drawing.
pub fn stage_fit(g: &mut Game, page: &str) {
    let mut it = page.split(':');
    let set = match it.next().unwrap_or("herald") {
        "smuggler" => hub::SMUGGLER,
        "pilgrim" => hub::PILGRIM,
        "rig" => hub::RIG,
        _ => hub::HERALD,
    };
    let pose = it.next().unwrap_or("idle");
    // (then any slots to leave empty, by number: to find which piece something belongs to)
    let bare: Vec<usize> = it.filter_map(|x| x.parse().ok()).collect();
    let t = g.time;
    let me = g.local;
    g.f[me].pos = v3(0.0, 0.0, -40.0);
    for i in 1..=4usize {
        if i >= g.f.len() || !g.f[i].present {
            continue;
        }
        let f = &mut g.f[i];
        let p = v3(-2.85 + (i - 1) as f32 * 1.9, 0.0, -14.0);
        let turn = match pose {
            "back" => std::f32::consts::PI,
            "run" | "wall" => if i % 2 == 0 { 1.1 } else { -1.1 },
            _ => if i % 2 == 0 { 0.35 } else { -0.35 },
        };
        f.pos = p;
        f.prev_pos = p;
        f.net_pos = p;
        f.yaw = turn;
        f.pitch = 0.0;
        f.alive = true;
        f.loadout.body = (i - 1) as u8;
        f.hub.look = Outfit::default().with_set(set);
        for &k in &bare {
            if k < hub::SLOT_COUNT {
                f.hub.look.items[k] = 0;
            }
        }
        f.hub.seat = SEAT_NONE;
        f.hub.emote = 0;
        f.vel = V3::ZERO;
        f.on_ground = true;
        f.wall = None;
        let fwd = f.fwd();
        let right = v3(fwd.z, 0.0, -fwd.x);
        match pose {
            "run" => f.vel = fwd * RUN_SPEED,
            "wall" => {
                f.on_ground = false;
                f.pos.y = 1.2;
                f.wall = Some(WallRun { normal: right, dir: fwd, t: 0.3, vertical: false, speed: 10.0 });
                f.vel = fwd * 10.0;
            }
            "emote" => {
                f.hub.emote = [Emote::Dance, Emote::Cheer, Emote::Bow, Emote::Wave][(i - 1) % 4] as u8;
                f.hub.emote_t = t - 0.8;
            }
            "sit" => {
                f.hub.emote = Emote::Sit as u8;
                f.hub.emote_t = t - 1.0;
            }
            _ => {}
        }
    }
    for k in 5..g.f.len() {
        g.f[k].pos = v3(0.0, 0.0, -45.0 - k as f32);
    }
}

/// The run scene: you run the course yourself (a scripted runner) behind your ghost.
    pub fn hub_scene_input(&mut self, inp: &mut Input) {
        // the photo scene: the camera stays in front of the fighter, as they face
        if self.hub_ui.drill == 98 {
            if let (Screen::Play { game, .. }, Some(ph)) = (&self.screen, self.hub_ui.photo.as_mut()) {
                let f = &game.f[game.local];
                let fwd = look_dir(f.yaw, 0.0);
                let cam = f.pos + fwd * 2.5 + v3(fwd.z, 0.0, -fwd.x) * 1.3 + v3(0.0, 1.25, 0.0);
                let d = (f.pos + v3(0.0, 1.1, 0.0) - cam).norm();
                ph.pos = cam;
                ph.yaw = d.x.atan2(d.z);
                ph.pitch = d.y.asin();
            }
            return;
        }
        if self.hub_ui.drill != 99 {
            return;
        }
        if let Screen::Play { game, .. } = &self.screen {
            let route = crate::hub_bots::course();
            let mut st = self.runner_state.take().unwrap_or_default();
            *inp = crate::hub_bots::run_input(game, game.local, &route, &mut st);
            self.aim_yaw = inp.yaw;
            self.runner_state = Some(st);
        }
    }
}
