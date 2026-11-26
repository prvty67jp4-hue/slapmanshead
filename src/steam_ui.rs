//! Steam on screen: the Achievements page and its unlock toast, the friends panel (invite, join,
//! the party), rich presence and Steam Cloud each frame, Steam Input's controller and the glyph
//! layout in Settings. The rules live in crate::achievements, crate::cloud, crate::party,
//! crate::presence and crate::glyphs; Steam itself behind crate::platform::steam_api.

use super::*;
use crate::achievements::{self as ach, Record};
use crate::party::PartyAct;
use crate::platform::steam_api::{FriendRow, PadKind};
use crate::presence::{self, Where};

/// The screen side's state.
#[derive(Default)]
pub struct SteamUi {
    pub friends_open: bool,
    pub friends: Vec<FriendRow>,
    pub friends_t: f32,
    pub presence_t: f32,
    pub check_t: f32,
    pub save_t: f32,
    pub stats_t: f32,
    pub look_t: f32,
    /// The Achievements page's tab: 0 everything, then each section.
    pub tab: u8,
    /// Each achievement's icon (earned, locked), made on first use.
    pub icons: Option<Vec<(egui::TextureHandle, egui::TextureHandle)>>,
    /// The vault as last seen: (room, that room's boss beaten, wipes).
    pub raid_seen: Option<(usize, bool, u32)>,
    /// Steam Input's controller family, and its buttons held last frame.
    pub pad_kind: Option<PadKind>,
    pub pad_held: u32,
    /// A line from the party or the friends panel (seconds left).
    pub note: Option<(String, f32)>,
    /// A party invite accepted before Steam was ready.
    pub pending_party: Option<u64>,
    /// (picture scenes) Settings opens at the controller layout.
    pub show_glyphs: bool,
}

impl App {
    /// Start the Steam layer once Steam is (or isn't) up: the cloud first (a newer save from
    /// another computer replaces this one before it's read), then the achievements.
    pub(crate) fn steam_layer_start(&mut self) {
        self.api = crate::platform::steam_api::open(self.steam.as_ref().map(|s| &s.client));
        let account = self.steam.as_ref().map_or(0, |s| s.me);
        let down = self.cloud.start(self.api.as_mut(), account);
        if down.iter().any(|n| n == "settings.txt") {
            self.settings = Settings::load();
        }
        if down.iter().any(|n| n == "career.txt") {
            self.career = career::Career::load();
        }
        self.ach = Record::load(&self.career);
        if down.iter().any(|n| n == "achievements.txt") {
            self.ach = Record::load(&self.career);
        }
        let (from, to) = self.ach.reconcile(self.api.as_mut());
        if from + to > 0 {
            self.ach.save();
        }
        // started from a friend's invite or "Join game" while the game wasn't running
        match steam::connect_from_args(&std::env::args().collect::<Vec<_>>()) {
            Some(steam::Connect::Party(p)) => self.sui.pending_party = Some(p),
            Some(steam::Connect::Watch(_)) => self.watch_next = true,
            _ => {}
        }
    }

    /// A session that used admin commands: nothing unlocks, nothing counts.
    pub(crate) fn ach_gated(&self) -> bool {
        self.session_gated || matches!(&self.screen, Screen::Play { game, .. } if game.admin.gated)
    }

    /// A line for the player: in a match it's said there, in the menus it shows under the
    /// friends button.
    pub(crate) fn steam_note(&mut self, s: String) {
        if let Screen::Play { game, .. } = &mut self.screen {
            game.say(s.clone());
        }
        self.sui.note = Some((s, 6.0));
    }

    /// Every frame: achievements, presence, the party, the cloud, the friends list.
    pub(crate) fn steam_layer_frame(&mut self, dt: f32) {
        let gated = self.ach_gated();
        // ---- what Steam reported about the party
        if let Some(p) = self.sui.pending_party.take() {
            self.party.join(self.api.as_mut(), p);
        }
        for ev in self.api.poll() {
            if let Some(act) = self.party.on_event(self.api.as_mut(), &ev) {
                self.party_act(act);
            }
        }
        // ---- what the game shows that counts: the vault, the secrets
        if !gated {
            self.watch_for_achievements();
        }
        // ---- cosmetics and the ranked record, now and then
        self.sui.look_t -= dt;
        if self.sui.look_t <= 0.0 {
            self.sui.look_t = 2.0;
            let earned = match &self.screen {
                Screen::Play { game, .. } if game.hub.is_some() => game.hub.as_ref().map(|h| h.earned),
                _ if self.hub_ui.loaded => Some(self.hub_ui.save.earned),
                _ => None,
            };
            if let Some(e) = earned {
                self.ach.cosmetics_seen(ach::cosmetics_owned(&e), gated);
            }
            if !gated {
                let r = &self.ranked;
                self.ach.ranked_seen(r.games, r.rank().map(|k| k.tier), r.board);
            }
        }
        // ---- unlock, save, send the stats
        self.sui.check_t -= dt;
        if self.sui.check_t <= 0.0 {
            self.sui.check_t = 0.5;
            let new = self.ach.check(self.api.as_mut(), gated);
            if !new.is_empty() {
                UI_SOUND.with(|u| u.set(Some(Sfx::UiConfirm)));
            }
        }
        self.sui.save_t -= dt;
        if self.ach.dirty && self.sui.save_t <= 0.0 && self.screenshot.is_none() {
            self.sui.save_t = 3.0;
            self.ach.save();
        }
        self.sui.stats_t -= dt;
        if self.ach.stats_dirty && self.sui.stats_t <= 0.0 {
            self.sui.stats_t = 60.0;
            self.ach.push_stats(self.api.as_mut());
        }
        self.ach.toast_tick(dt);
        if let Some(n) = self.sui.note.as_mut() {
            n.1 -= dt;
            if n.1 <= 0.0 {
                self.sui.note = None;
            }
        }
        // ---- rich presence, about once a second
        self.sui.presence_t -= dt;
        if self.sui.presence_t <= 0.0 {
            self.sui.presence_t = 1.0;
            if self.api.features().steam {
                let p = self.presence_now();
                presence::send(self.api.as_mut(), &p);
            }
            // the party: the leader says where it is going, the rest follow
            let lobby = self.steam.as_ref().and_then(|s| s.lobby).unwrap_or(0);
            let in_match = matches!(self.screen, Screen::Play { net: Some(_), .. } | Screen::Lobby { .. });
            let ranked = self.ranked_q.is_some() || self.ranked_match.is_some();
            self.party.leader_at(self.api.as_mut(), if in_match { lobby } else { 0 }, ranked);
            if let Some(act) = self.party.follow(self.api.as_ref(), lobby) {
                self.party_act(act);
            }
        }
        // ---- the cloud: send saves that changed
        let account = self.steam.as_ref().map_or(0, |s| s.me);
        self.cloud.upkeep(self.api.as_mut(), account, dt, false);
        // ---- the friends list while the panel is open
        if self.sui.friends_open {
            self.sui.friends_t -= dt;
            if self.sui.friends_t <= 0.0 {
                self.sui.friends_t = 3.0;
                self.sui.friends = self.api.friends();
                self.party.refresh(self.api.as_ref());
            }
        }
    }

    fn party_act(&mut self, act: PartyAct) {
        match act {
            PartyAct::Note(s) => self.steam_note(s),
            PartyAct::Join { lobby, watch } => {
                self.watch_next = watch;
                self.pending_lobby = Some(lobby);
                self.steam_note(if watch { "Following your party leader: their ranked 1v1 has one place, so you'll watch it.".into() } else { "Following your party leader into their game...".into() });
            }
        }
    }

    /// The vault's bosses and clears, and the hidden ones (the mirror, a photo, the stands).
    fn watch_for_achievements(&mut self) {
        if self.hub_ui.mirror {
            self.ach.secret(ach::SECRET_MIRROR);
        }
        if self.hub_ui.photo_req {
            self.ach.secret(ach::SECRET_PHOTO);
        }
        let Screen::Play { game, .. } = &self.screen else {
            self.sui.raid_seen = None;
            return;
        };
        if self.spec.pit.is_some() || game.watcher {
            self.ach.secret(ach::SECRET_STANDS);
        }
        let me = game.local;
        let Some(r) = game.raid.as_ref().filter(|_| game.in_raid(me)) else {
            self.sui.raid_seen = None;
            return;
        };
        use crate::raid::Stage;
        let beaten = matches!(r.stage, Stage::Cleared | Stage::Done);
        let now = (r.room, beaten, r.wipes);
        match self.sui.raid_seen {
            // (first sight of a raid already under way: nothing is counted for what came before)
            None => {}
            Some((room, was_beaten, wipes)) => {
                if beaten && (room != r.room || !was_beaten) {
                    self.ach.boss_down(r.room as u8);
                    if r.stage == Stage::Done {
                        let ms = if r.result_ms > 0 { r.result_ms } else { (r.total_time * 1000.0) as u32 };
                        self.ach.vault_cleared(ms, r.hard);
                    }
                }
                if r.wipes > wipes {
                    self.ach.wipe();
                }
            }
        }
        self.sui.raid_seen = Some(now);
    }

    /// Where we are, for friends (and whether they can join).
    pub(crate) fn presence_now(&self) -> presence::Presence {
        let lobby = self.steam.as_ref().and_then(|s| s.lobby);
        let ranked = self.ranked_q.is_some() || self.ranked_match.is_some();
        let party = self.party.active().then(|| (self.party.id, self.party.members.len()));
        let (w, join) = match &self.screen {
            Screen::Menu(Page::Replays) if self.replay_view.is_some() => (Where::Replays, None),
            Screen::Menu(_) => (if self.hub_keep.is_some() { Where::Hub } else { Where::Menu }, None),
            Screen::Lobby { .. } => (Where::Menu, lobby.map(|l| (l, ranked))),
            Screen::Play { game, .. } => {
                let map = lore::map_name(game.world.map).to_string();
                let me = game.local;
                let join = lobby.map(|l| (l, ranked));
                if game.watcher {
                    (Where::Watching { map }, join.map(|j| (j.0, true)))
                } else if let Some(r) = game.raid.as_ref().filter(|_| game.in_raid(me)) {
                    (Where::Raid { room: r.room, rooms: crate::vault::ROOMS.len(), hard: r.hard }, None)
                } else if game.hub.is_some() {
                    (Where::Hub, join)
                } else if let Some(gt) = game.gauntlet.as_ref() {
                    (Where::Gauntlet { wave: gt.wave.max(1) }, None)
                } else if game.train.is_some() {
                    (Where::Training, None)
                } else {
                    let score_of = |s: usize| if game.mt.mode.rounds() { game.f[s].wins } else { game.f[s].kills };
                    let opp = (0..game.f.len()).find(|&s| s != me && game.f[s].present && !game.f[s].spectate);
                    let score = if game.mt.mode.teams() {
                        let t = (game.f[me].team as usize).min(1);
                        Some((game.mt.team_score[t] as u32, game.mt.team_score[1 - t] as u32))
                    } else if game.mt.mode.duel_like() || ranked {
                        opp.map(|o| (score_of(me).max(0) as u32, score_of(o).max(0) as u32))
                    } else {
                        None
                    };
                    if ranked {
                        let rank = self.ranked.rank().map_or("Placement".to_string(), |r| ranked::TIERS[r.tier.min(7) as usize].0.to_string());
                        let (a, b) = score.unwrap_or((0, 0));
                        (Where::Ranked { rank, me: a, them: b, map }, join)
                    } else {
                        (Where::Match { mode: game.mt.mode.name().to_string(), map, score }, join)
                    }
                }
            }
        };
        presence::describe(&w, join, party)
    }

    /// Steam Input's controller, when it's reading one: the action set for the screen, its state as
    /// the game's buttons.
    pub(crate) fn steam_pad_poll(&mut self) -> Option<pad::PadState> {
        let (in_match, hub, watching) = match &self.screen {
            Screen::Play { game, .. } => (true, game.hub.is_some(), game.watcher || self.spec.pit.is_some()),
            _ => (false, false, false),
        };
        let set = crate::glyphs::set_for(in_match, hub, watching, self.captured);
        let Some((kind, sp)) = self.api.pad(set) else {
            self.sui.pad_kind = None;
            return None;
        };
        self.sui.pad_kind = Some(kind);
        let pressed = sp.held & !self.sui.pad_held;
        self.sui.pad_held = sp.held;
        self.pads.name = Some(format!("{} (Steam Input)", kind.title()));
        Some(pad::PadState { connected: true, lx: sp.lx, ly: sp.ly, rx: sp.rx, ry: sp.ry, lt: sp.lt, rt: sp.rt, held: sp.held, pressed })
    }

    /// The family prompts and glyphs use this frame.
    pub(crate) fn glyph_kind(&self) -> PadKind {
        crate::glyphs::pick(self.settings.glyphs, self.sui.pad_kind, self.pads.name.as_deref())
    }

    fn ach_icons(&mut self, ctx: &egui::Context) {
        if self.sui.icons.is_some() {
            return;
        }
        let tex = |name: String, img: image::RgbaImage| {
            let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
            ctx.load_texture(name, ci, egui::TextureOptions::LINEAR)
        };
        self.sui.icons = Some(ach::ALL.iter().map(|d| (tex(format!("ach-{}", d.id), ach::icon(d, 96, false)), tex(format!("ach-{}-l", d.id), ach::icon(d, 96, true)))).collect());
    }

    /// Settings > Steam's lines: what's live through Steam and what's kept here.
    fn steam_status_line(&self) -> (String, Color32) {
        let f = self.api.features();
        if !f.steam {
            ("Steam isn't running: achievements and stats are kept on this computer and shown the same way.".into(), Color32::from_gray(175))
        } else if f.achievements {
            (format!("Achievements and stats sync with Steam (App ID {}).", self.api.app_id()), Color32::from_rgb(140, 220, 160))
        } else {
            ("Kept on this computer for now: Steam achievements switch on with the game's own App ID (steam/app_id.txt); everything earned here goes to Steam then.".into(), Color32::from_rgb(235, 200, 150))
        }
    }

    /// The Achievements page.
    pub(crate) fn achievements_page(&mut self, ui: &mut egui::Ui) -> Option<Page> {
        let mut next = None;
        self.ach_icons(ui.ctx());
        let total = ach::ALL.len();
        let got = ach::ALL.iter().filter(|d| self.ach.is_unlocked(d.id)).count();
        ui.heading(format!("Achievements  ·  {got} / {total}"));
        ui.add(egui::ProgressBar::new(got as f32 / total as f32).desired_height(8.0));
        let (line, col) = self.steam_status_line();
        ui.label(egui::RichText::new(line).size(12.5).color(col));
        if self.ach_gated() {
            ui.label(egui::RichText::new("This session used admin commands: nothing unlocks until the game is restarted.").size(12.5).color(Color32::from_rgb(255, 150, 120)));
        }
        // the sections
        ui.horizontal_wrapped(|ui| {
            let tabs = std::iter::once("Everything").chain(ach::Group::ALL.iter().map(|g| g.title()));
            for (k, t) in tabs.enumerate() {
                if ui.selectable_label(self.sui.tab == k as u8, t).clicked() {
                    self.sui.tab = k as u8;
                }
            }
        });
        if self.pad_tab != 0 {
            self.sui.tab = (self.sui.tab as i32 + self.pad_tab).rem_euclid(ach::Group::ALL.len() as i32 + 1) as u8;
            self.pad_tab = 0;
        }
        let icons = self.sui.icons.clone().unwrap_or_default();
        let tab = self.sui.tab;
        let h = (ui.ctx().content_rect().height() - 420.0).max(180.0);
        egui::ScrollArea::vertical().max_height(h).show(ui, |ui| {
            for g in ach::Group::ALL {
                if tab != 0 && ach::Group::ALL[tab as usize - 1] != g {
                    continue;
                }
                ui_kit::section(ui, g.title());
                for (k, d) in ach::ALL.iter().enumerate().filter(|(_, d)| d.group == g) {
                    let done = self.ach.unlocked.get(d.id).copied();
                    let (cur, goal) = self.ach.progress(d.goal);
                    let secret = d.hidden && done.is_none();
                    egui::Frame::new().fill(Color32::from_rgba_unmultiplied(255, 255, 255, if done.is_some() { 14 } else { 6 })).corner_radius(5.0).inner_margin(6.0).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            if let Some((on, off)) = icons.get(k) {
                                let t = if done.is_some() { on } else { off };
                                ui.add(egui::Image::new((t.id(), Vec2::splat(54.0))));
                            }
                            ui.vertical(|ui| {
                                let name = if secret { "Hidden achievement" } else { d.name };
                                let nc = if done.is_some() { Color32::from_rgb(255, 214, 150) } else { Color32::from_gray(205) };
                                ui.label(egui::RichText::new(name).font(ui_kit::display(17.0)).color(nc));
                                let desc = if secret { "Keep exploring Varrow: this one shows itself once it's earned." } else { d.desc };
                                ui.label(egui::RichText::new(desc).size(12.5).color(Color32::from_gray(185)));
                                match done {
                                    Some(t) => {
                                        ui.label(egui::RichText::new(format!("Earned {}", ago(t))).size(11.5).color(Color32::from_rgb(140, 220, 160)));
                                    }
                                    None if goal > 1 && !secret => {
                                        ui.add(egui::ProgressBar::new(cur as f32 / goal as f32).desired_width(ui.available_width().min(320.0)).desired_height(10.0).text(egui::RichText::new(format!("{cur} / {goal}")).size(10.5)));
                                    }
                                    None => {}
                                }
                            });
                        });
                    });
                }
            }
        });
        if Self::big_button(ui, "Back") {
            next = Some(Page::Main);
        }
        next
    }

    /// The unlock toast: slides in at the top, holds, slides away.
    pub(crate) fn draw_ach_toast(&mut self, ctx: &egui::Context, rect: Rect) {
        let Some(&(k, t)) = self.ach.toasts.first() else { return };
        self.ach_icons(ctx);
        let d = &ach::ALL[k];
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("ach-toast")));
        let ease = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
        let show = ease(t / 0.35) * ease((ach::TOAST_TIME - t) / 0.35);
        let w = 440.0;
        let hgt = 86.0;
        let top = rect.top() + 18.0 - (1.0 - show) * (hgt + 30.0);
        let r = Rect::from_min_size(Pos2::new(rect.center().x - w * 0.5, top), Vec2::new(w, hgt));
        let a = (show * 255.0) as u8;
        p.rect_filled(r.translate(Vec2::new(0.0, 4.0)), 10.0, Color32::from_black_alpha((show * 120.0) as u8));
        p.rect_filled(r, 10.0, Color32::from_rgba_unmultiplied(16, 13, 22, (show * 238.0) as u8));
        let c = d.group.colour();
        let accent = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], a);
        p.rect_stroke(r, 10.0, Stroke::new(1.5, accent), egui::StrokeKind::Inside);
        // a shine sweeping across once it lands
        let sweep = ((t - 0.35) / 0.9).clamp(0.0, 1.0);
        if sweep > 0.0 && sweep < 1.0 {
            let x = r.left() + r.width() * sweep;
            p.rect_filled(Rect::from_min_max(Pos2::new((x - 30.0).max(r.left()), r.top() + 2.0), Pos2::new((x + 30.0).min(r.right()), r.bottom() - 2.0)), 6.0, Color32::from_rgba_unmultiplied(255, 230, 190, 18));
        }
        if let Some((on, _)) = self.sui.icons.as_ref().and_then(|v| v.get(k)) {
            let ir = Rect::from_center_size(Pos2::new(r.left() + 46.0, r.center().y), Vec2::splat(66.0));
            p.image(on.id(), ir, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::from_white_alpha(a));
        }
        let x = r.left() + 92.0;
        p.text(Pos2::new(x, r.top() + 14.0), Align2::LEFT_TOP, "ACHIEVEMENT UNLOCKED", ui_kit::display(13.0), accent);
        let nf = ui_kit::fitted(&p, d.name, ui_kit::display(24.0), r.right() - x - 14.0);
        p.text(Pos2::new(x, r.top() + 31.0), Align2::LEFT_TOP, d.name, nf, Color32::from_white_alpha(a));
        let desc = if d.desc.len() > 64 { format!("{}...", &d.desc[..d.desc[..61].rfind(' ').unwrap_or(61)]) } else { d.desc.to_string() };
        p.text(Pos2::new(x, r.top() + 60.0), Align2::LEFT_TOP, desc, FontId::proportional(12.5), Color32::from_rgba_unmultiplied(210, 205, 200, a));
    }

    /// The friends button (top right of the menus and Varrow Centre) and its panel.
    pub(crate) fn friends_ui(&mut self, ctx: &egui::Context, rect: Rect) {
        if ctx.input(|i| i.key_pressed(Key::F6)) {
            self.sui.friends_open = !self.sui.friends_open;
            self.sui.friends_t = 0.0;
        }
        let online = self.sui.friends.iter().filter(|f| f.online).count();
        let label = if self.party.active() { format!("Friends  ·  party {}/{}", self.party.members.len(), crate::party::MAX) } else if online > 0 { format!("Friends  ·  {online} online") } else { "Friends".to_string() };
        egui::Area::new(egui::Id::new("friends-button")).fixed_pos(Pos2::new(rect.right() - 212.0, rect.top() + 14.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            let b = egui::Button::new(egui::RichText::new(label).size(14.0)).min_size(Vec2::new(198.0, 30.0));
            if ui.add(b).on_hover_text("Invite Steam friends, join their games, party up (F6)").clicked() {
                self.sui.friends_open = !self.sui.friends_open;
                self.sui.friends_t = 0.0;
            }
            if let Some((n, _)) = &self.sui.note {
                if !self.sui.friends_open {
                    ui.label(egui::RichText::new(n.as_str()).size(12.0).color(Color32::from_rgb(255, 210, 160)));
                }
            }
        });
        if !self.sui.friends_open {
            return;
        }
        let mut act: Option<Box<dyn FnOnce(&mut App)>> = None;
        let feats = self.api.features();
        let me = self.api.me();
        let lobby = self.steam.as_ref().and_then(|s| s.lobby);
        let in_game_lobby = lobby.is_some() && matches!(self.screen, Screen::Play { net: Some(_), .. } | Screen::Lobby { .. });
        let ranked = self.ranked_q.is_some() || self.ranked_match.is_some();
        let width = 330.0;
        egui::Area::new(egui::Id::new("friends-panel")).fixed_pos(Pos2::new(rect.right() - width - 14.0, rect.top() + 52.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::new().fill(Color32::from_rgba_unmultiplied(12, 10, 18, 236)).corner_radius(8.0).inner_margin(12.0).stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 190, 130, 70))).show(ui, |ui| {
                ui.set_width(width - 24.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("FRIENDS").font(ui_kit::display(20.0)).color(ACCENT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Close").clicked() {
                            self.sui.friends_open = false;
                        }
                    });
                });
                if !feats.steam {
                    ui.label(egui::RichText::new("Steam isn't running: friends, invites and parties need it. Join by IP still works from Play.").size(12.5).color(Color32::from_gray(185)));
                    return;
                }
                if let Some((n, _)) = &self.sui.note {
                    ui.label(egui::RichText::new(n.as_str()).size(12.0).color(Color32::from_rgb(255, 210, 160)));
                }
                // ---- the party
                ui_kit::section(ui, "Party");
                if self.party.active() {
                    for (id, name) in self.party.members.clone() {
                        let lead = id == self.party.leader;
                        ui.label(egui::RichText::new(format!("{}{}{}", if lead { "Leader: " } else { "" }, name, if id == me { "  (you)" } else { "" })).size(13.5).color(if lead { Color32::from_rgb(255, 214, 140) } else { Color32::from_gray(220) }));
                    }
                    let tip = if self.party.leading(me) {
                        if ranked { "You lead: a ranked 1v1 has one place, so your party watches it." } else { "You lead: queue or host and your party follows you in." }
                    } else {
                        "You follow the leader into their matches."
                    };
                    ui.label(egui::RichText::new(tip).size(12.0).color(Color32::from_gray(175)));
                    if ui.small_button("Leave party").clicked() {
                        act = Some(Box::new(|app: &mut App| {
                            app.party.leave(app.api.as_mut());
                            app.steam_note("You left the party.".into());
                        }));
                    }
                } else {
                    ui.label(egui::RichText::new("Invite a friend to make a party (up to four): its leader queues and everyone follows.").size(12.0).color(Color32::from_gray(175)));
                }
                // ---- the list
                ui_kit::section(ui, if in_game_lobby { "Invite to this game" } else { "Steam friends" });
                let rows = self.sui.friends.clone();
                if rows.is_empty() {
                    ui.label(egui::RichText::new("No Steam friends to show yet.").size(12.5).color(Color32::from_gray(170)));
                }
                egui::ScrollArea::vertical().max_height((rect.height() - 330.0).max(160.0)).show(ui, |ui| {
                    for f in rows {
                        ui.horizontal(|ui| {
                            let dot = if f.in_game { Color32::from_rgb(140, 220, 110) } else if f.online { Color32::from_rgb(110, 170, 240) } else { Color32::from_gray(90) };
                            let (r, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
                            ui.painter().circle_filled(r.center(), 4.5, dot);
                            ui.vertical(|ui| {
                                ui.set_width(width - 150.0);
                                ui.label(egui::RichText::new(&f.name).size(14.0).color(if f.online { Color32::WHITE } else { Color32::from_gray(140) }));
                                ui.label(egui::RichText::new(&f.status).size(11.5).color(if f.in_game { Color32::from_rgb(170, 225, 150) } else { Color32::from_gray(150) }));
                            });
                            if let Some(c) = f.connect.clone() {
                                let watch = c.contains("+watch");
                                if ui.small_button(if watch { "Watch" } else { "Join" }).on_hover_text("Join their game through Steam").clicked() {
                                    act = Some(Box::new(move |app: &mut App| {
                                        if let Some(conn) = steam::parse_connect(&c) {
                                            match conn {
                                                steam::Connect::Lobby(l) => app.pending_lobby = Some(l),
                                                steam::Connect::Watch(l) => {
                                                    app.watch_next = true;
                                                    app.pending_lobby = Some(l);
                                                }
                                                steam::Connect::Party(p) => app.party.join(app.api.as_mut(), p),
                                            }
                                        }
                                    }));
                                }
                            }
                            if f.online && ui.small_button("Invite").on_hover_text(if in_game_lobby { "Invite them into this game" } else { "Invite them to your party" }).clicked() {
                                let (id, name) = (f.id, f.name.clone());
                                act = Some(Box::new(move |app: &mut App| app.invite_friend(id, &name)));
                            }
                        });
                    }
                });
                ui.add_space(4.0);
                if ui.button("Invite with Steam...").on_hover_text("Steam's own invite window").clicked() {
                    act = Some(Box::new(|app: &mut App| {
                        let conn = match app.steam.as_ref().and_then(|s| s.lobby) {
                            Some(l) => crate::party::lobby_connect(l, app.ranked_q.is_some() || app.ranked_match.is_some()),
                            None if app.party.active() => crate::party::party_connect(app.party.id),
                            None => {
                                app.steam_note("Host a game or invite someone to a party first.".into());
                                return;
                            }
                        };
                        app.api.invite_overlay(&conn);
                    }));
                }
            });
        });
        if let Some(a) = act {
            a(self);
        }
    }

    /// Invite a friend: into the game we're in, or into the party.
    pub(crate) fn invite_friend(&mut self, id: u64, name: &str) {
        let lobby = self.steam.as_ref().and_then(|s| s.lobby);
        let in_game = matches!(self.screen, Screen::Play { net: Some(_), .. } | Screen::Lobby { .. });
        match lobby {
            Some(l) if in_game => {
                let ranked = self.ranked_q.is_some() || self.ranked_match.is_some();
                self.api.invite(id, &crate::party::lobby_connect(l, ranked));
                self.steam_note(format!("Invited {name}{}.", if ranked { " to watch your ranked match" } else { " to this game" }));
            }
            _ => {
                let act = self.party.invite(self.api.as_mut(), id);
                self.party_act(act);
            }
        }
    }

    /// Settings > Controls > Controller: the family picker, the layout drawn with that
    /// family's glyphs, the Deck's extras, and Steam Input.
    pub(crate) fn controller_glyphs_ui(&mut self, ui: &mut egui::Ui) {
        // (the controller picture scene opens the page here)
        if self.sui.show_glyphs {
            ui.scroll_to_cursor(Some(egui::Align::TOP));
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Button icons");
            for (k, t) in ["Match my controller", "Xbox", "PlayStation", "Switch Pro", "Steam Deck"].iter().enumerate() {
                if ui.selectable_label(self.settings.glyphs == k as u8, *t).clicked() {
                    self.settings.glyphs = k as u8;
                }
            }
        });
        let kind = self.glyph_kind();
        let f = self.api.features();
        let input_line = if f.input {
            format!("Steam Input is reading the controller ({}): menus, combat, Varrow Centre and spectating each have their own action set.", self.sui.pad_kind.map_or("waiting for a controller", |k| k.title()))
        } else if f.steam {
            "Steam Input: the game reads the controller itself (Steam's action sets switch on with the game's own App ID and its controller layouts).".to_string()
        } else {
            "The game reads the controller itself (Steam isn't running).".to_string()
        };
        ui.label(egui::RichText::new(input_line).size(12.5).color(Color32::from_gray(180)));
        if f.input && ui.button("Steam controller layout...").clicked() {
            self.api.binding_panel();
        }
        ui.label(egui::RichText::new(format!("{} layout", kind.title())).font(ui_kit::display(17.0)).color(ACCENT));
        let row_h = 30.0;
        for (what, bit, stick) in crate::glyphs::LAYOUT {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::new(64.0, row_h), egui::Sense::hover());
                let c = Pos2::new(r.left() + 30.0, r.center().y);
                match bit {
                    Some(b) => crate::glyphs::paint(ui.painter(), c, 24.0, b, kind),
                    None => crate::glyphs::paint_stick(ui.painter(), c, 20.0, stick.starts_with("Left"), kind),
                }
                let name = match bit {
                    Some(b) => crate::glyphs::name(b, kind).to_string(),
                    None => stick.to_string(),
                };
                ui.label(egui::RichText::new(format!("{what}")).size(14.0));
                ui.label(egui::RichText::new(name).size(12.0).color(Color32::from_gray(160)));
            });
        }
        if kind == PadKind::Deck {
            ui.label(egui::RichText::new("Steam Deck extras").font(ui_kit::display(15.0)).color(ACCENT));
            for (input, what) in crate::glyphs::DECK_EXTRAS {
                ui.label(egui::RichText::new(format!("{input}: {what}")).size(13.0).color(Color32::from_gray(210)));
            }
        }
        // every family side by side
        ui.label(egui::RichText::new("Face buttons on each controller").size(12.5).color(Color32::from_gray(170)));
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(520.0), 92.0), egui::Sense::hover());
        let cw = r.width() / 4.0;
        for (k, fam) in PadKind::ALL.iter().enumerate() {
            let c = Pos2::new(r.left() + cw * (k as f32 + 0.5), r.top() + 38.0);
            let s = 20.0;
            for (b, d) in [(pad::btn::Y, Vec2::new(0.0, -s)), (pad::btn::X, Vec2::new(-s, 0.0)), (pad::btn::B, Vec2::new(s, 0.0)), (pad::btn::A, Vec2::new(0.0, s))] {
                crate::glyphs::paint(ui.painter(), c + d, 18.0, b, *fam);
            }
            ui.painter().text(Pos2::new(c.x, r.bottom() - 8.0), Align2::CENTER_BOTTOM, fam.title(), FontId::proportional(12.0), if *fam == kind { ACCENT } else { Color32::from_gray(170) });
        }
    }

    /// The Stats page's lifetime numbers (the ones sent to Steam).
    pub(crate) fn lifetime_stats_ui(&self, ui: &mut egui::Ui) {
        ui_kit::section(ui, "Lifetime");
        let v = self.ach.stat_values();
        egui::Grid::new("lifetime").num_columns(2).spacing([18.0, 3.0]).show(ui, |ui| {
            for ((_, title), (name, n)) in ach::STATS.iter().zip(v.iter()) {
                ui.label(*title);
                let s = if *name == "STAT_ROOFTOP_MS" { if *n > 0 { format!("{:.2} s", *n as f32 / 1000.0) } else { "-".into() } } else { n.to_string() };
                ui.label(egui::RichText::new(s).strong());
                ui.end_row();
            }
        });
        let (line, col) = self.steam_status_line();
        ui.label(egui::RichText::new(line).size(12.0).color(col));
        if !self.cloud.report.is_empty() {
            for l in &self.cloud.report {
                ui.label(egui::RichText::new(format!("Steam Cloud: {l}")).size(12.0).color(Color32::from_gray(175)));
            }
        }
    }

    /// Picture scenes (DUEL_STEAM_SCENE=achievements | toast | friends | glyphs:<0-4>): a record
    /// part-way through, a fresh unlock, the friends panel, the controller layout.
    pub(crate) fn steam_scene(&mut self, scene: &str) {
        let day = career::today();
        let mut r = Record::default();
        for _ in 0..37 {
            r.feed(CareerEv::MatchWon, Some(1), day);
        }
        for k in 0..812u32 {
            r.feed(CareerEv::Kill { head: k % 4 == 0, finisher: if k % 40 == 0 { (k / 40 % 16) as u8 + 1 } else { 0 } }, Some((k % 4) as u8), day);
        }
        for _ in 0..4 {
            r.feed(CareerEv::Kill { head: true, finisher: 0 }, Some(1), day);
        }
        for _ in 0..64 {
            r.feed(CareerEv::Parry, None, day);
        }
        r.feed(CareerEv::Chain(5), None, day);
        r.feed(CareerEv::Rooftop { ms: 52_300 }, None, day);
        r.boss_down(0);
        r.boss_down(1);
        r.ranked_seen(5, Some(3), 0);
        r.cosmetics_seen(23, false);
        r.secret(ach::SECRET_MIRROR);
        let mut quiet = crate::platform::steam_api::FakeSteam::new(1);
        r.check(&mut quiet, false);
        // spread the unlock dates out
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        for (k, v) in r.unlocked.values_mut().enumerate() {
            *v = now - (k as u64 * 97_000) % (40 * 86_400);
        }
        r.toasts.clear();
        r.dirty = false;
        self.ach = r;
        match scene {
            "achievements" => self.screen = Screen::Menu(Page::Achievements),
            "toast" => {
                if let Some(k) = ach::ALL.iter().position(|d| d.id == "boss_regent") {
                    self.ach.unlocked.remove("boss_regent");
                    self.ach.life.bosses |= 1 << 3;
                    self.ach.unlocked.insert("boss_regent".into(), now);
                    self.ach.toasts.push((k, 1.6));
                }
            }
            "friends" => {
                self.sui.friends_open = true;
                self.sui.friends = crate::platform::steam_api::FakeSteam::sample_friends();
                self.party.id = 9001;
                self.party.leader = self.api.me();
                self.party.members = vec![(self.api.me(), clean_name(&self.settings.name, 0)), (12, "Odo Brandt".into())];
                self.sui.friends_t = 1e9;
            }
            s if s.starts_with("glyphs") => {
                self.settings.glyphs = s.trim_start_matches("glyphs").trim_start_matches(':').parse::<u8>().unwrap_or(4).min(4);
                self.settings_tab = 1;
                self.screen = Screen::Menu(Page::Settings);
                self.pad_active = true;
                self.sui.show_glyphs = true;
            }
            _ => {}
        }
    }
}

/// "today", "3 days ago"... for an unlock time.
fn ago(t: u64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let days = now.saturating_sub(t) / 86_400;
    match days {
        0 => "today".into(),
        1 => "yesterday".into(),
        n if n < 60 => format!("{n} days ago"),
        n => format!("{} months ago", n / 30),
    }
}

/// Write every file the Steamworks partner site takes into dir (steam/): the achievements and
/// stats with their icons, the rich presence lines, the Steam Input manifest and each controller's
/// default layout (the Steam Deck's included). Returns how many files.
pub fn write_steam_files(dir: &std::path::Path, app: u32) -> std::io::Result<usize> {
    let mut n = ach::export(dir, app)?;
    std::fs::write(dir.join("rich_presence.vdf"), presence::vdf())?;
    std::fs::write(dir.join("steam_input_manifest.vdf"), crate::glyphs::manifest(app))?;
    n += 2;
    for (ty, file) in crate::glyphs::CONFIGS {
        let title = match ty {
            "controller_neptune" => "Steam Deck",
            "controller_ps5" => "PlayStation",
            "controller_switch_pro" => "Switch Pro",
            _ => "Xbox",
        };
        std::fs::write(dir.join(file), crate::glyphs::controller_config(ty, title))?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_steam_folder_is_current() {
        // (after a change: DUEL_WRITE_STEAM=steam duel_arena.exe, or the ignored test below)
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("steam");
        let read = |f: &str| std::fs::read_to_string(dir.join(f)).unwrap_or_default().replace("\r\n", "\n");
        assert_eq!(read("rich_presence.vdf"), presence::vdf(), "rich_presence.vdf");
        assert_eq!(read("steam_input_manifest.vdf"), crate::glyphs::manifest(480), "steam_input_manifest.vdf");
        assert_eq!(read("controller_steamdeck.vdf"), crate::glyphs::controller_config("controller_neptune", "Steam Deck"));
        for d in ach::ALL.iter() {
            assert!(dir.join("icons").join(format!("{}.png", d.id)).is_file() && dir.join("icons").join(format!("{}_locked.png", d.id)).is_file(), "{} icons", d.id);
        }
    }

    #[test]
    #[ignore]
    fn write_steam_folder() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("steam");
        assert!(write_steam_files(&dir, 480).unwrap() > 100);
    }

    #[test]
    fn the_party_and_presence_reach_steam_from_the_app_side() {
        let p = presence::describe(&Where::Hub, Some((5, false)), Some((9, 2)));
        assert_eq!(p.connect.as_deref(), Some("+connect_lobby 5"));
        assert_eq!(ago(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()), "today");
        let mut f = crate::platform::steam_api::FakeSteam::new(3);
        let mut party = crate::party::Party::default();
        let act = party.invite(&mut f, 4);
        assert!(matches!(act, PartyAct::Note(_)));
    }
}
