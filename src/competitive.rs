//! The competitive side on screen: ranked 1v1 (the profile, the queue through Steam lobbies by
//! rating and region, settling a match: crate::ranked), rank badges, the killcam after you die,
//! the post-match screen's numbers, spectating (a caster's view and HUD for spectators and for
//! the Court Pits' stands in the hub) and the Replays menu with its viewer (crate::replay).

use super::*;
use crate::ranked::{self, Rank, Rating};
use crate::replay::{self, ViewCam};

/// The ranked queue: how long we've looked, when we ask Steam again, and whether we host a
/// ranked lobby while we wait.
#[derive(Clone, Debug, Default)]
pub struct RankedQueue {
    pub t: f32,
    pub ask: f32,
    pub hosting: bool,
    pub joining: bool,
    /// Lobbies we tried that didn't work out (full, gone).
    pub tried: Vec<u64>,
}

/// A ranked match in play: the opponent, whether it has started (and so stands as a loss until
/// settled), and why it doesn't count, when it doesn't.
#[derive(Clone, Debug, Default)]
pub struct RankedMatch {
    pub opp: Option<usize>,
    pub opp_rating: Option<Rating>,
    pub begun: bool,
    pub settled: bool,
    pub void: Option<&'static str>,
}

/// How a spectator (or someone on the pits' walkway) watches: behind a player, through their
/// eyes, a free camera, or the wide caster's view; and whom.
#[derive(Clone, Copy, Debug)]
pub struct SpecCam {
    pub mode: u8,
    pub who: usize,
    pub pos: V3,
    /// Watching a Court Pit's duel from the stands (the pit).
    pub pit: Option<usize>,
}

impl Default for SpecCam {
    fn default() -> SpecCam {
        SpecCam { mode: 3, who: 0, pos: v3(0.0, 6.0, 0.0), pit: None }
    }
}

pub const SPEC_FOLLOW: u8 = 0;
pub const SPEC_EYES: u8 = 1;
pub const SPEC_FREE: u8 = 2;
pub const SPEC_CASTER: u8 = 3;

/// A small rank badge: a shield in the tier's colour with its mark (a placement shows nothing).
pub fn draw_rank_badge(p: &egui::Painter, c: Pos2, size: f32, rank: u8, alpha: u8) {
    let Some(r) = Rank::from_byte(rank & 0x7f) else { return };
    let [cr, cg, cb] = r.colour();
    let col = Color32::from_rgba_unmultiplied(cr, cg, cb, alpha);
    let dark = Color32::from_rgba_unmultiplied(20, 16, 14, alpha);
    let w = size * 0.5;
    let pts = vec![Pos2::new(c.x - w, c.y - size * 0.55), Pos2::new(c.x + w, c.y - size * 0.55), Pos2::new(c.x + w, c.y + size * 0.05), Pos2::new(c.x, c.y + size * 0.6), Pos2::new(c.x - w, c.y + size * 0.05)];
    p.add(egui::Shape::convex_polygon(pts.clone(), col, Stroke::new((size * 0.08).max(1.0), dark)));
    // the Ledger of Champions wears a crown of light
    if r.tier == ranked::LEDGER {
        p.circle_stroke(c, size * 0.78, Stroke::new(size * 0.06, Color32::from_rgba_unmultiplied(230, 200, 255, alpha / 2)));
    }
    p.text(c + Vec2::new(0.0, -size * 0.04), Align2::CENTER_CENTER, r.mark(), FontId::proportional(size * 0.5), dark);
    // divisions: one, two or three pips under it
    if r.tier < 6 {
        let n = 3 - r.div as i32;
        for k in 0..n {
            let x = c.x + (k as f32 - (n - 1) as f32 * 0.5) * size * 0.22;
            p.circle_filled(Pos2::new(x, c.y + size * 0.82), size * 0.07, col);
        }
    }
}

/// The rank's name for a byte (None: unranked or placing).
pub fn rank_name(b: u8) -> Option<String> {
    Rank::from_byte(b & 0x7f).map(|r| r.name())
}

impl App {
    /// The ranked record for whoever plays on this computer (their Steam account, or "local").
    /// A match left unsettled last time (the game was closed mid-match) counts as a loss now.
    pub(crate) fn load_ranked(&mut self) {
        let account = self.steam.as_ref().map_or(0, |s| s.me);
        let (mut p, note) = ranked::load(account);
        self.ranked_account = account;
        let today = career::today();
        p.age_to(today);
        if let Some(c) = p.settle_abandoned(today) {
            self.ranked_note = Some(format!("Your last ranked match was left before it ended: it counted as a loss ({:+.0}).", c.delta));
        }
        if note.is_some() {
            self.ranked_note = note;
        }
        ranked::save(&p, account);
        self.ranked = p;
    }

    /// What we tell a host of our rank (rank byte, with 0x80 when we came through the ranked queue).
    pub(crate) fn my_rank_tuple(&self, queued: bool) -> (u8, u16, u16) {
        let r = &self.ranked;
        (Rank::to_byte(r.rank()) | if queued { 0x80 } else { 0 }, r.rating.r.round().clamp(0.0, 65000.0) as u16, r.rating.rd.round().clamp(0.0, 65000.0) as u16)
    }

    // ------------------------------------------------------------ the Ranked page

    pub(crate) fn ranked_page(&mut self, ui: &mut egui::Ui) -> (Option<Page>, Option<Box<dyn FnOnce(&mut App)>>) {
        let mut next = None;
        let mut action: Option<Box<dyn FnOnce(&mut App)>> = None;
        ui.heading("Ranked 1v1");
        ui.label(egui::RichText::new("A Duel under fixed rules: first to three rounds, Arena, Close Quarters or the Courtyard, no Ledger gear, and never in a session that used admin commands. Leaving early counts as a loss.").size(13.0).color(Color32::from_gray(195)));
        let p = self.ranked.clone();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 92.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, Color32::from_rgba_unmultiplied(255, 255, 255, 12));
        match p.rank() {
            Some(r) => {
                draw_rank_badge(&painter, Pos2::new(rect.left() + 46.0, rect.center().y - 4.0), 54.0, Rank::to_byte(Some(r)), 255);
                let [cr, cg, cb] = r.colour();
                painter.text(Pos2::new(rect.left() + 92.0, rect.top() + 26.0), Align2::LEFT_CENTER, r.name().to_uppercase(), ui_kit::display(24.0), Color32::from_rgb(cr, cg, cb));
            }
            None => {
                painter.circle_stroke(Pos2::new(rect.left() + 46.0, rect.center().y), 26.0, Stroke::new(2.0, Color32::from_gray(150)));
                painter.text(Pos2::new(rect.left() + 46.0, rect.center().y), Align2::CENTER_CENTER, format!("{}/{}", p.placed(), ranked::PLACEMENTS), FontId::proportional(16.0), Color32::from_gray(220));
                painter.text(Pos2::new(rect.left() + 92.0, rect.top() + 26.0), Align2::LEFT_CENTER, "PLACEMENT", ui_kit::display(24.0), Color32::from_gray(225));
            }
        }
        let losses = p.games.saturating_sub(p.wins);
        let line = if p.rank().is_some() { format!("{} wins  ·  {} losses", p.wins, losses) } else { format!("{} of {} placement matches played: your rank shows after them", p.placed(), ranked::PLACEMENTS) };
        painter.text(Pos2::new(rect.left() + 92.0, rect.top() + 54.0), Align2::LEFT_CENTER, line, FontId::proportional(13.5), Color32::from_gray(210));
        if p.board > 0 && p.board <= 1000 {
            painter.text(Pos2::new(rect.left() + 92.0, rect.top() + 74.0), Align2::LEFT_CENTER, format!("Leaderboard #{}  (the top {} are the Ledger of Champions)", p.board, ranked::CHAMPIONS), FontId::proportional(12.5), Color32::from_gray(180));
        }
        if let Some(n) = &self.ranked_note {
            ui.colored_label(Color32::from_rgb(255, 190, 120), n);
        }
        // recent matches
        if !p.history.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Recent").size(12.5).color(Color32::from_gray(170)));
                for h in p.history.iter().rev().take(10) {
                    let (t, c) = if h.won { ("W", Color32::from_rgb(110, 210, 130)) } else { ("L", Color32::from_rgb(235, 100, 90)) };
                    ui.label(egui::RichText::new(t).strong().color(c)).on_hover_text(format!("{:+.0}{}", h.delta, if h.left { "  (left early)" } else { "" }));
                }
            });
        }
        ui.add_space(4.0);
        let steam = self.steam.is_some();
        match &self.ranked_q {
            Some(q) => {
                let (window, dist) = ranked::search_window(q.t);
                let far = ["nearby", "in your region", "further afield", "worldwide"][dist.min(3) as usize];
                let what = if q.joining { "Found an opponent: joining…".to_string() } else if q.hosting { format!("Waiting for an opponent in your ranked lobby  ·  {:.0}s", q.t) } else { format!("Searching {far}, within {window} rating  ·  {:.0}s", q.t) };
                let pulse = (0.6 + 0.4 * (ui.input(|i| i.time) as f32 * 3.0).sin()).clamp(0.0, 1.0);
                ui.label(egui::RichText::new(what).font(ui_kit::display(18.0)).color(Color32::from_rgba_unmultiplied(255, 170, 90, (150.0 + 105.0 * pulse) as u8)));
                ui.add(egui::ProgressBar::new((q.t / 60.0).min(1.0)).desired_height(6.0));
                if Self::big_button(ui, "Cancel search") {
                    action = Some(Box::new(|app: &mut App| app.cancel_ranked_queue()));
                }
            }
            None => {
                let ok = steam;
                if ui.add_enabled_ui(ok, |ui| Self::big_button(ui, "Find Ranked Match")).inner {
                    UI_SOUND.with(|u| u.set(Some(Sfx::UiConfirm)));
                    action = Some(Box::new(|app: &mut App| app.start_ranked_queue()));
                }
                if !ok {
                    ui.label(egui::RichText::new("Ranked needs Steam running (matches are found through Steam lobbies, by rating and region). Games joined by IP are never ranked.").size(12.5).color(Color32::from_gray(180)));
                }
            }
        }
        if Self::big_button(ui, "Back") {
            next = Some(Page::Main);
        }
        (next, action)
    }

    pub(crate) fn start_ranked_queue(&mut self) {
        self.ranked_q = Some(RankedQueue::default());
        self.ranked_note = None;
        self.ranked_change = None;
    }

    pub(crate) fn cancel_ranked_queue(&mut self) {
        let hosting = self.ranked_q.as_ref().is_some_and(|q| q.hosting);
        self.ranked_q = None;
        if hosting {
            self.leave_to_menu();
            self.screen = Screen::Menu(Page::Ranked);
        }
    }

    /// The queue each frame: ask Steam for ranked lobbies near our rating (wider and further as
    /// time goes on), join the best; with none, host one and keep looking.
    pub(crate) fn ranked_queue_tick(&mut self, dt: f32) {
        if std::env::var_os("DUEL_COMP").is_some() {
            // (a picture of the queue: it just counts)
            if let Some(q) = self.ranked_q.as_mut() {
                q.t += dt;
            }
            return;
        }
        let Some(mut q) = self.ranked_q.take() else { return };
        q.t += dt;
        q.ask -= dt;
        let rating = self.ranked.rating.r;
        let (window, dist) = ranked::search_window(q.t);
        let me = self.steam.as_ref().map_or(0, |s| s.me);
        // hosting and someone came in: the match is on
        if q.hosting {
            if let Screen::Play { game, .. } = &self.screen {
                let n = game.present().len();
                if n >= 2 {
                    self.ranked_match = Some(RankedMatch::default());
                    return;
                }
            } else {
                // our lobby went away
                q.hosting = false;
            }
        }
        if matches!(self.screen, Screen::Lobby { .. }) || q.joining && !matches!(self.screen, Screen::Menu(_)) {
            // (joining: the lobby screen takes it from here)
            self.ranked_q = Some(q);
            return;
        }
        let Some(st) = self.steam.as_mut() else {
            self.ranked_q = None;
            self.ranked_note = Some("Ranked needs Steam running.".into());
            return;
        };
        if q.ask <= 0.0 && !st.refreshing {
            st.refresh_ranked(rating.round() as i32, window, dist);
            q.ask = 4.0;
            self.ranked_q = Some(q);
            return;
        }
        if !st.refreshing && q.ask < 3.5 {
            let rows: Vec<(u64, i32, Option<u32>)> = st.rows.iter().filter(|r| r.ranked && r.players < 2 && r.host != me && !q.tried.contains(&r.id)).map(|r| (r.id, r.rating, r.ping)).collect();
            let ours = st.lobby;
            if let Some(id) = ranked::best_lobby(rating, window, &rows) {
                // (two hosts waiting: the one with the higher lobby id gives way)
                if !q.hosting || ours.is_some_and(|o| id < o) {
                    if q.hosting {
                        self.leave_to_menu();
                    }
                    q.hosting = false;
                    q.joining = true;
                    q.tried.push(id);
                    if let Some(st) = self.steam.as_mut() {
                        st.join_lobby(id);
                    }
                    self.ranked_q = Some(q);
                    return;
                }
            }
            if !q.hosting && q.t > 6.0 && matches!(self.screen, Screen::Menu(_)) {
                q.hosting = true;
                self.ranked_q = Some(q);
                if let Err(e) = self.host_ranked() {
                    self.ranked_q = None;
                    self.ranked_note = Some(e);
                }
                return;
            }
        }
        self.ranked_q = Some(q);
    }

    /// Open a ranked lobby: a public Steam lobby marked ranked with our rating, two players' places,
    /// the fixed rules.
    fn host_ranked(&mut self) -> Result<(), String> {
        let mut n = Net::host(0)?;
        n.cap = 2;
        let me = self.steam.as_ref().map_or(0, |s| s.me);
        let map = ranked::pick_map(me, career::today() as u64);
        if let Some(st) = self.steam.as_mut() {
            n.steam = Some(st.client.clone());
            st.map_name = map.name().to_string();
            st.create_lobby(true);
        }
        let mut g = Game::new(Kind::Host, 0, ranked::RULES.mode, ranked::RULES.target);
        g.set_map(map);
        let name = clean_name(&self.settings.name, 0);
        g.add_player(0, &name, true, None, 0);
        g.say("Ranked: waiting for an opponent (matched by rating and region)".into());
        self.start_game(g, Some(n));
        if let Screen::Play { game, .. } = &mut self.screen {
            game.mt.classic = false;
            game.my_kit = Default::default();
        }
        Ok(())
    }

    /// Each frame of a match: everyone's rank where it's shown, and a ranked match begun, settled
    /// or left.
    pub(crate) fn ranked_frame(&mut self) {
        let queued = self.ranked_q.is_some() || self.ranked_match.is_some();
        let mine = self.my_rank_tuple(queued);
        let gated = self.session_gated;
        let Screen::Play { game, net } = &mut self.screen else { return };
        let me = game.local;
        if !game.watcher && me < game.f.len() {
            game.f[me].rank = mine.0;
            game.f[me].rating = mine.1;
            game.f[me].rating_rd = mine.2;
        }
        if let Some(n) = net.as_mut() {
            n.my_rank = mine;
            if n.is_host {
                for s in 0..game.f.len() {
                    if let Some(r) = n.peer_rank(s) {
                        game.f[s].rank = r.0;
                        game.f[s].rating = r.1;
                        game.f[s].rating_rd = r.2;
                    }
                }
            }
        }
        let Some(mut rm) = self.ranked_match.take() else { return };
        if rm.settled {
            self.ranked_match = Some(rm);
            return;
        }
        let opp = (0..game.f.len()).find(|&s| s != me && game.f[s].present && !game.f[s].spectate);
        if rm.opp.is_none() {
            rm.opp = opp;
        }
        let phase = game.mt.phase;
        if !rm.begun && matches!(phase, Phase::Countdown | Phase::Fight) {
            if let Some(o) = rm.opp.filter(|&o| game.f[o].present) {
                let f = &game.f[o];
                let opp_rating = Rating { r: f.rating as f64, rd: (f.rating_rd as f64).clamp(ranked::MIN_RD, ranked::START_RD), vol: ranked::START_VOL };
                rm.opp_rating = Some(opp_rating);
                rm.begun = true;
                let gear = game.f.iter().any(|f| f.present && f.kit.is_some());
                rm.void = ranked::void_reason(game.mt.mode, game.admin.gated || gated, gear, false);
                if rm.void.is_none() && f.rank & 0x80 == 0 {
                    rm.void = Some("your opponent didn't come through the ranked queue");
                }
                if rm.void.is_none() {
                    self.ranked.begin(opp_rating);
                    ranked::save(&self.ranked, self.ranked_account);
                    if let Some(r) = game.replay_rec.as_mut() {
                        r.ranked = true;
                    }
                    game.say(format!("Ranked match: {} ({})", f.name, rank_name(f.rank).unwrap_or_else(|| "placing".into())));
                } else {
                    game.say(format!("Not ranked: {}", rm.void.unwrap_or("")));
                }
            }
        }
        if rm.begun && rm.void.is_none() {
            let opp_gone = rm.opp.is_some_and(|o| !game.f[o].present);
            let over = phase == Phase::MatchOver;
            if over || opp_gone {
                let won = if opp_gone && !over { true } else { game.local_won() };
                let opp_rating = rm.opp_rating.unwrap_or_default();
                let change = self.ranked.settle(opp_rating, won, opp_gone && !over, career::today());
                ranked::save(&self.ranked, self.ranked_account);
                if let Some(st) = self.steam.as_ref() {
                    st.submit_ranked(self.ranked.rating.r.round() as i32);
                }
                if opp_gone && !over {
                    game.say("Your opponent left: the win is yours.".into());
                }
                self.ranked_change = Some((change, 0.0));
                rm.settled = true;
            }
        }
        self.ranked_match = Some(rm);
    }

    /// Leaving a ranked match before its end counts as a loss.
    pub(crate) fn ranked_leave(&mut self) {
        let Some(rm) = self.ranked_match.take() else { return };
        if rm.begun && !rm.settled && rm.void.is_none() {
            let c = self.ranked.settle(rm.opp_rating.unwrap_or_default(), false, true, career::today());
            ranked::save(&self.ranked, self.ranked_account);
            self.ranked_note = Some(format!("You left a ranked match early: it counts as a loss ({:+.0}).", c.delta));
        }
        self.ranked_q = None;
    }

    /// The post-match screen's ranked part: the rank (animated when it goes up) and the change.
    pub(crate) fn ranked_result_ui(&self, ui: &mut egui::Ui) {
        let Some((c, t)) = self.ranked_change else {
            if let Some(v) = self.ranked_match.as_ref().and_then(|m| m.void) {
                ui.label(egui::RichText::new(format!("Not ranked: {v}")).size(13.0).color(Color32::from_rgb(255, 190, 120)));
            }
            return;
        };
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 84.0), egui::Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 6.0, Color32::from_rgba_unmultiplied(255, 255, 255, 14));
        let up = c.ranked_up();
        // the rank-up: the old badge shrinks away, the new one bursts in with a ring of light
        let k = (t / 1.2).clamp(0.0, 1.0);
        let bc = Pos2::new(rect.left() + 44.0, rect.center().y - 4.0);
        let show_after = !up || k > 0.45;
        let badge = if show_after { c.after } else { c.before };
        let pop = if up && show_after { 1.0 + 0.35 * (1.0 - ((k - 0.45) / 0.55)).max(0.0) } else if up { 1.0 - k * 1.4 } else { 1.0 };
        match badge {
            Some(r) => draw_rank_badge(&p, bc, 50.0 * pop.max(0.1), Rank::to_byte(Some(r)), 255),
            None => {
                p.circle_stroke(bc, 24.0, Stroke::new(2.0, Color32::from_gray(160)));
                p.text(bc, Align2::CENTER_CENTER, format!("{}/{}", c.placement.max(1), ranked::PLACEMENTS), FontId::proportional(15.0), Color32::from_gray(220));
            }
        }
        if up && k > 0.4 && k < 1.0 {
            let rr = 20.0 + 60.0 * (k - 0.4) / 0.6;
            let a = ((1.0 - (k - 0.4) / 0.6) * 220.0) as u8;
            let col = c.after.map_or([255, 220, 150], |r| r.colour());
            p.circle_stroke(bc, rr, Stroke::new(3.0, Color32::from_rgba_unmultiplied(col[0], col[1], col[2], a)));
            for n in 0..10 {
                let ang = n as f32 * 0.628 + k * 2.0;
                p.circle_filled(bc + Vec2::new(ang.cos(), ang.sin()) * rr * 1.1, 2.5, Color32::from_rgba_unmultiplied(255, 240, 200, a));
            }
        }
        let title = if up && show_after {
            match c.after {
                Some(r) if c.before.is_none() => format!("PLACED: {}", r.name().to_uppercase()),
                Some(r) => format!("RANK UP: {}", r.name().to_uppercase()),
                None => String::new(),
            }
        } else if c.placement > 0 && c.after.is_none() {
            format!("PLACEMENT {} / {}", c.placement, ranked::PLACEMENTS)
        } else {
            c.after.map_or("RANKED".into(), |r| r.name().to_uppercase())
        };
        p.text(Pos2::new(rect.left() + 86.0, rect.top() + 24.0), Align2::LEFT_CENTER, title, ui_kit::display(22.0), Color32::from_rgb(255, 225, 170));
        let (txt, col) = if c.won { (format!("Victory  ·  {:+.0} rating", c.delta), Color32::from_rgb(120, 220, 140)) } else { (format!("Defeat  ·  {:+.0} rating{}", c.delta, if c.left { "  (left early)" } else { "" }), Color32::from_rgb(240, 110, 95)) };
        p.text(Pos2::new(rect.left() + 86.0, rect.top() + 52.0), Align2::LEFT_CENTER, txt, FontId::proportional(15.0), col);
        p.text(Pos2::new(rect.left() + 86.0, rect.top() + 71.0), Align2::LEFT_CENTER, format!("Rating {:.0}", c.rating), FontId::proportional(11.5), Color32::from_gray(160));
    }

    // ------------------------------------------------------------ the killcam

    /// The killcam's titles over the replayed moment (the normal HUD stays out of it).
    pub(crate) fn draw_deathcam(&self, p: &egui::Painter, rect: Rect, cam: &Cam, g: &Game, s: f32) {
        let Some(dc) = &g.deathcam else { return };
        let c = rect.center();
        let bar_h = rect.height() * 0.09;
        p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(rect.width(), bar_h)), 0.0, Color32::from_black_alpha(220));
        p.rect_filled(Rect::from_min_size(Pos2::new(rect.left(), rect.bottom() - bar_h), Vec2::new(rect.width(), bar_h)), 0.0, Color32::from_black_alpha(220));
        // a red-tinged edge: this is the past
        for k in 0..6 {
            let a = (40 - k * 6) as u8;
            p.rect_stroke(rect.shrink(k as f32 * 4.0), 0.0, Stroke::new(4.0, Color32::from_rgba_unmultiplied(150, 20, 20, a)), egui::StrokeKind::Inside);
        }
        text_display(p, Pos2::new(rect.left() + 30.0 * s, rect.top() + bar_h * 0.5), Align2::LEFT_CENTER, "KILLCAM", 34.0 * s, Color32::from_rgb(255, 90, 70));
        let killer = g.f.get(dc.killer).map_or("?", |f| f.name.as_str());
        let mut parts: Vec<(String, Color32)> = vec![(format!("{killer}'s view  ·  {}", dc.tool), Color32::from_rgb(240, 230, 220))];
        if dc.head {
            parts.push(("  ".into(), Color32::WHITE));
            parts.push((HS_ICON.into(), HS_GOLD));
            parts.push((" HEADSHOT".into(), HS_GOLD));
        }
        let size = 19.0 * s;
        let font = FontId::proportional(size);
        let pieces = layout_pieces(p, parts, &font, size, |c| c);
        let total: f32 = pieces.iter().map(|pc| pc.width()).sum();
        draw_pieces(p, Pos2::new(c.x - total * 0.5, rect.top() + bar_h * 0.5 - size * 0.6), pieces, size);
        if let Some(r) = rank_name(g.f.get(dc.killer).map_or(0, |f| f.rank)) {
            draw_rank_badge(p, Pos2::new(c.x - total * 0.5 - 18.0 * s, rect.top() + bar_h * 0.5), 18.0 * s, g.f[dc.killer].rank, 255);
            let _ = r;
        }
        // the damage you took, as they saw it land
        for d in &g.dmg_nums {
            if let Some(sp) = cam.project(d.pos) {
                let a = (d.life / 0.9 * 255.0).clamp(0.0, 255.0) as u8;
                let pop = 1.0 + 0.6 * ((d.life - 0.7) / 0.2).clamp(0.0, 1.0);
                let col = if d.head { Color32::from_rgba_unmultiplied(255, 215, 50, a) } else { Color32::from_rgba_unmultiplied(255, 90, 80, a) };
                let size = (20.0 + (d.amount.min(80) as f32 / 80.0) * 14.0) * pop * s;
                text_display(p, sp, Align2::CENTER_CENTER, &format!("{}", if d.amount > 0 { d.amount } else { d.armour }), size, col);
                if d.head {
                    text_shadow(p, sp + Vec2::new(0.0, size * 0.75), Align2::CENTER_CENTER, "HEADSHOT", 12.0 * s, col);
                }
            }
        }
        // the moment of the kill: a stamp
        if dc.t >= DEATHCAM_TIME - 0.15 {
            let k = ((dc.t - (DEATHCAM_TIME - 0.15)) / 0.25).clamp(0.0, 1.0);
            let txt = if dc.head { "HEADSHOT" } else { "ELIMINATED" };
            text_display(p, Pos2::new(c.x, c.y - rect.height() * 0.18), Align2::CENTER_CENTER, txt, (64.0 - 18.0 * k) * s, Color32::from_rgba_unmultiplied(255, if dc.head { 210 } else { 80 }, 60, (255.0 * k) as u8));
        }
        // how far through it, and how to skip
        let bw = rect.width() * 0.4;
        let br = Rect::from_min_size(Pos2::new(c.x - bw * 0.5, rect.bottom() - bar_h * 0.5 - 3.0), Vec2::new(bw, 6.0));
        p.rect_filled(br, 3.0, Color32::from_gray(50));
        p.rect_filled(Rect::from_min_size(br.min, Vec2::new(bw * (dc.t / DEATHCAM_TIME).min(1.0), 6.0)), 3.0, Color32::from_rgb(255, 90, 70));
        text_shadow(p, Pos2::new(rect.right() - 24.0 * s, rect.bottom() - bar_h * 0.5), Align2::RIGHT_CENTER, "SPACE  skip", 14.0 * s, Color32::from_gray(210));
    }

    // ------------------------------------------------------------ spectating

    /// The two to show on the caster's HUD: the duellists, else the two leading.
    pub(crate) fn caster_pair(&self, g: &Game) -> Option<(usize, usize)> {
        if let Some(pit) = self.spec.pit {
            if let Some(d) = g.hub.as_ref().and_then(|h| h.pits[pit.min(1)]) {
                return Some((d.who[0] as usize, d.who[1] as usize));
            }
            return None;
        }
        let [a, b] = g.mt.duel;
        if g.mt.mode.duel_like() && (a as usize) < g.f.len() && (b as usize) < g.f.len() && g.f[a as usize].present && g.f[b as usize].present {
            return Some((a as usize, b as usize));
        }
        let mut ps: Vec<usize> = (0..g.f.len()).filter(|&s| g.f[s].present && !g.f[s].spectate && s != g.local || (g.f[s].present && !g.watcher && s == g.local)).collect();
        ps.sort_by_key(|&s| std::cmp::Reverse(if g.mt.mode.rounds() { g.f[s].wins } else { g.f[s].kills }));
        match ps.as_slice() {
            [a, b, ..] => Some((*a, *b)),
            _ => None,
        }
    }

    /// Watching (a spectator, or from the pits' walkway): the camera.
    pub(crate) fn spectator_camera(&self, g: &Game, rect: Rect) -> Option<Cam> {
        if !g.watcher && self.spec.pit.is_none() {
            return None;
        }
        let sp = self.spec;
        let pair = self.caster_pair(g);
        let who = if g.f.get(sp.who).is_some_and(|f| f.present) { sp.who } else { pair.map_or(0, |p| p.0) };
        let f = &g.f[who.min(g.f.len() - 1)];
        let alpha = (self.acc / TICK).clamp(0.0, 1.0);
        let body = f.prev_pos.lerp(f.pos, alpha);
        Some(match sp.mode {
            SPEC_EYES => Cam::new(body + v3(0.0, 1.62, 0.0), f.yaw, f.pitch, self.settings.fov, rect),
            SPEC_FREE => Cam::new(sp.pos, self.aim_yaw, self.aim_pitch, self.settings.fov, rect),
            SPEC_FOLLOW => {
                let (pivot, dir, md) = g.camera_rig(body, f.yaw, f.pitch * 0.6);
                Cam::new(pivot - dir * md, f.yaw, f.pitch * 0.6, self.settings.fov, rect)
            }
            _ => {
                // the caster's view: high and wide, framing both, easing round them
                let (a, b) = pair.map_or((who, who), |p| p);
                let pa = g.f[a.min(g.f.len() - 1)].pos;
                let pb = g.f[b.min(g.f.len() - 1)].pos;
                let mid = (pa + pb) * 0.5;
                let span = (pa - pb).flat().len().max(4.0);
                let side = (pb - pa).flat();
                let side = if side.len() > 0.1 { side.norm() } else { v3(1.0, 0.0, 0.0) };
                let out = v3(-side.z, 0.0, side.x);
                let sway = (g.time * 0.12).sin() * 0.35;
                let dir_out = (out * sway.cos() + side * sway.sin()).norm();
                let look = mid + v3(0.0, 1.0, 0.0);
                // the first angle round them (from the preferred one outwards) that sees both, at a
                // distance that fits; pulled in short of any wall
                let dist = span * 0.9 + 5.0;
                let rise = 3.5 + span * 0.25;
                let mut pos = look + dir_out * dist + v3(0.0, rise - 1.0, 0.0);
                // (both seen whole: their heads, chests and both sides, and room round the camera)
                let sees = |c: V3| {
                    let across = v3(-(look - c).z, 0.0, (look - c).x).norm() * 0.45;
                    let whole = |p: V3| [v3(0.0, 1.7, 0.0), v3(0.0, 0.7, 0.0), across + v3(0.0, 1.1, 0.0), across * -1.0 + v3(0.0, 1.1, 0.0)].iter().all(|o| g.world.line_clear(c, p + *o));
                    let room = [v3(1.0, 0.0, 0.0), v3(-1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), v3(0.0, 0.0, -1.0)].iter().all(|d| g.world.line_clear(c, c + *d * 1.2));
                    whole(pa) && whole(pb) && room
                };
                'find: for k in 0..16 {
                    let turn = if k % 2 == 0 { k as f32 * 0.2 } else { -(k as f32 + 1.0) * 0.2 };
                    let (sn, cs) = turn.sin_cos();
                    let d = v3(dir_out.x * cs - dir_out.z * sn, 0.0, dir_out.x * sn + dir_out.z * cs);
                    for scale in [1.0f32, 0.7, 0.45] {
                        let mut c = look + d * (dist * scale) + v3(0.0, (rise - 1.0) * scale.max(0.6), 0.0);
                        if let Some((t, _)) = g.world.raycast(look, (c - look).norm(), (c - look).len()) {
                            c = look + (c - look).norm() * (t - 0.5).max(1.5);
                        }
                        if sees(c) {
                            pos = c;
                            break 'find;
                        }
                    }
                }
                let d = (look - pos).norm();
                Cam::new(pos, d.x.atan2(d.z), d.y.clamp(-1.0, 1.0).asin(), 62.0, rect)
            }
        })
    }

    /// Watching: 1 / 2 pick a duellist, Q / E step through everyone, C behind them, V their eyes,
    /// F a free camera (WASD, Space / Ctrl, the mouse), Tab... the caster's wide view with B.
    pub(crate) fn spectator_input(&mut self, ctx: &egui::Context, dt: f32) {
        let Screen::Play { game, .. } = &self.screen else { return };
        if !game.watcher && self.spec.pit.is_none() {
            return;
        }
        let pair = self.caster_pair(game);
        let everyone: Vec<usize> = (0..game.f.len()).filter(|&s| game.f[s].present && !game.f[s].spectate && !(game.watcher && s == game.local)).collect();
        let (k1, k2, q, e, mut c, mut v, mut f, mut b) = ctx.input(|i| (i.key_pressed(Key::Num1), i.key_pressed(Key::Num2), i.key_pressed(Key::Q), i.key_pressed(Key::E), i.key_pressed(Key::C), i.key_pressed(Key::V), i.key_pressed(Key::F), i.key_pressed(Key::B)));
        // a controller (Steam Input's spectating set or the game's own reading): bumpers pick who,
        // Y steps through the cameras
        let pad = self.pad;
        let (q, e) = (q || pad.tapped(pad::btn::LB), e || pad.tapped(pad::btn::RB));
        if pad.tapped(pad::btn::Y) {
            match self.spec.mode {
                SPEC_FOLLOW => v = true,
                SPEC_EYES => b = true,
                SPEC_CASTER => f = true,
                _ => c = true,
            }
        }
        let sp = &mut self.spec;
        if let Some((a, bb)) = pair {
            if k1 {
                sp.who = a;
                sp.mode = sp.mode.min(SPEC_EYES);
            }
            if k2 {
                sp.who = bb;
                sp.mode = sp.mode.min(SPEC_EYES);
            }
        }
        if (q || e) && !everyone.is_empty() {
            let k = everyone.iter().position(|&s| s == sp.who).unwrap_or(0) as i32;
            let n = everyone.len() as i32;
            sp.who = everyone[((k + if e { 1 } else { -1 }).rem_euclid(n)) as usize];
            if sp.mode == SPEC_CASTER || sp.mode == SPEC_FREE {
                sp.mode = SPEC_FOLLOW;
            }
        }
        if c {
            sp.mode = SPEC_FOLLOW;
        }
        if v {
            sp.mode = SPEC_EYES;
        }
        if b {
            sp.mode = SPEC_CASTER;
        }
        if f {
            if sp.mode != SPEC_FREE {
                // the free camera starts where the view is
                if let Some(t) = game.f.get(sp.who) {
                    sp.pos = t.pos + v3(0.0, 3.0, 0.0) - yaw_fwd(t.yaw) * 4.0;
                }
            }
            sp.mode = if sp.mode == SPEC_FREE { SPEC_CASTER } else { SPEC_FREE };
        }
        if sp.mode == SPEC_FREE {
            let (fw, bk, l, r, up, dn) = ctx.input(|i| (i.key_down(Key::W), i.key_down(Key::S), i.key_down(Key::A), i.key_down(Key::D), i.key_down(Key::Space), i.modifiers.ctrl));
            let d = look_dir(self.aim_yaw, self.aim_pitch);
            let rt = yaw_right(self.aim_yaw);
            let mv = d * ((fw as i32 - bk as i32) as f32) + rt * ((r as i32 - l as i32) as f32) + V3::UP * ((up as i32 - dn as i32) as f32);
            self.spec.pos += mv * (10.0 * dt);
        }
    }

    /// The caster's HUD: both sides' names and ranks, health and armour, weapons, ability and guard
    /// bars, the score and the clock; and how to move the camera.
    pub(crate) fn draw_caster(&self, p: &egui::Painter, rect: Rect, g: &Game, s: f32) {
        let Some((a, b)) = self.caster_pair(g) else {
            text_shadow(p, Pos2::new(rect.center().x, rect.top() + 40.0 * s), Align2::CENTER_CENTER, "Watching  ·  waiting for the duel", 20.0 * s, Color32::WHITE);
            return;
        };
        let mid = rect.center().x;
        let top = rect.top() + 18.0 * s;
        let w = (rect.width() * 0.33).min(460.0 * s);
        let h = 86.0 * s;
        let pit = self.spec.pit.and_then(|k| g.hub.as_ref().and_then(|hb| hb.pits[k.min(1)]));
        for (side, slot) in [(0usize, a), (1usize, b)] {
            let f = &g.f[slot.min(g.f.len() - 1)];
            let left = side == 0;
            let x0 = if left { mid - 70.0 * s - w } else { mid + 70.0 * s };
            let r = Rect::from_min_size(Pos2::new(x0, top), Vec2::new(w, h));
            let accent = if left { Color32::from_rgb(235, 120, 60) } else { Color32::from_rgb(80, 160, 235) };
            p.rect_filled(r, 6.0 * s, Color32::from_rgba_unmultiplied(12, 10, 18, 215));
            p.rect_filled(Rect::from_min_size(if left { r.min } else { Pos2::new(r.right() - 5.0 * s, r.top()) }, Vec2::new(5.0 * s, h)), 0.0, accent);
            let align = if left { Align2::LEFT_CENTER } else { Align2::RIGHT_CENTER };
            let tx = if left { r.left() + 14.0 * s } else { r.right() - 14.0 * s };
            // name and rank badge
            let name_x = if f.rank & 0x7f != 0 { tx + if left { 26.0 * s } else { -26.0 * s } } else { tx };
            if f.rank & 0x7f != 0 {
                draw_rank_badge(p, Pos2::new(tx + if left { 9.0 * s } else { -9.0 * s }, top + 17.0 * s), 18.0 * s, f.rank, 255);
            }
            text_display(p, Pos2::new(name_x, top + 17.0 * s), align, &f.name, 21.0 * s, Color32::WHITE);
            let wtxt = if f.alive { f.weapon.name().to_string() } else { "DOWN".into() };
            text_shadow(p, Pos2::new(if left { r.right() - 12.0 * s } else { r.left() + 12.0 * s }, top + 17.0 * s), if left { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER }, &wtxt, 13.5 * s, Color32::from_gray(205));
            // health (and armour in blue)
            let bar = |y: f32, k: f32, col: Color32, hh: f32| {
                let bw = w - 28.0 * s;
                let br = Rect::from_min_size(Pos2::new(r.left() + 14.0 * s, y), Vec2::new(bw, hh));
                p.rect_filled(br, 2.0, Color32::from_gray(40));
                let fw = bw * k.clamp(0.0, 1.0);
                let fr = if left { Rect::from_min_size(br.min, Vec2::new(fw, hh)) } else { Rect::from_min_size(Pos2::new(br.right() - fw, br.top()), Vec2::new(fw, hh)) };
                p.rect_filled(fr, 2.0, col);
            };
            let hp = if f.alive { f.hp / MAX_HP } else { 0.0 };
            let hp_col = if hp > 0.5 { Color32::from_rgb(90, 210, 110) } else if hp > 0.25 { Color32::from_rgb(235, 190, 60) } else { Color32::from_rgb(235, 70, 60) };
            bar(top + 34.0 * s, hp, hp_col, 12.0 * s);
            text_shadow(p, Pos2::new(if left { r.left() + 18.0 * s } else { r.right() - 18.0 * s }, top + 40.0 * s), align, &format!("{:.0}", f.hp.max(0.0)), 11.0 * s, Color32::WHITE);
            bar(top + 49.0 * s, f.ap / MAX_AP, Color32::from_rgb(110, 170, 255), 4.0 * s);
            // the spec bars: ability charge, guard meter (or the Ledger special)
            let spec = f.kit.as_ref().map_or(f.guard_hp / GUARD_MAX, |k| k.spec / 100.0);
            bar(top + 60.0 * s, f.ab_charge, Color32::from_rgb(200, 140, 255), 6.0 * s);
            bar(top + 70.0 * s, spec, Color32::from_rgb(240, 230, 200), 6.0 * s);
            let tags = format!("ABILITY {:.0}%   ·   {} {:.0}%", f.ab_charge * 100.0, if f.kit.is_some() { "SPECIAL" } else { "GUARD" }, spec * 100.0);
            text_shadow(p, Pos2::new(if left { r.left() + 14.0 * s } else { r.right() - 14.0 * s }, top + h + 9.0 * s), align, &tags, 11.0 * s, Color32::from_gray(185));
            let _ = slot;
        }
        // the score and the round in the middle
        let (sa, sb, label) = match pit {
            Some(d) => (d.wins[0] as i32, d.wins[1] as i32, format!("FIRST TO {}", d.first_to)),
            None if g.mt.mode.rounds() => (g.f[a].wins, g.f[b].wins, format!("ROUND {}  ·  FIRST TO {}", g.mt.round.max(1), g.mt.target)),
            None => (g.f[a].kills, g.f[b].kills, format!("FIRST TO {}", g.mt.target)),
        };
        let sr = Rect::from_center_size(Pos2::new(mid, top + 30.0 * s), Vec2::new(124.0 * s, 60.0 * s));
        p.rect_filled(sr, 6.0 * s, Color32::from_rgba_unmultiplied(12, 10, 18, 235));
        text_display(p, Pos2::new(mid, top + 26.0 * s), Align2::CENTER_CENTER, &format!("{sa}  -  {sb}"), 30.0 * s, Color32::WHITE);
        let clock = if g.mt.phase == Phase::Fight && g.mt.timer < 9000.0 { format!("{}:{:02}", (g.mt.timer.max(0.0) as u32) / 60, (g.mt.timer.max(0.0) as u32) % 60) } else { String::new() };
        text_shadow(p, Pos2::new(mid, top + 49.0 * s), Align2::CENTER_CENTER, &clock, 12.0 * s, Color32::from_gray(200));
        text_shadow(p, Pos2::new(mid, top + 70.0 * s), Align2::CENTER_CENTER, &label, 11.0 * s, Color32::from_gray(170));
        // the kill feed, small, on the right
        let mut y = top + h + 30.0 * s;
        for l in g.feed.iter().rev().take(4) {
            let k = l.killer.as_ref().map_or("", |k| k.0.as_str());
            text_shadow(p, Pos2::new(rect.right() - 18.0 * s, y), Align2::RIGHT_CENTER, &format!("{k}  [{}]{}  {}", l.tool, if l.head { " HS" } else { "" }, l.victim.0), 13.0 * s, Color32::from_gray(225));
            y += 20.0 * s;
        }
        // whom the camera is on, and how to move it
        let mode = ["BEHIND", "EYES", "FREE CAMERA", "CASTER VIEW"][self.spec.mode.min(3) as usize];
        let on = g.f.get(self.spec.who).filter(|f| f.present && self.spec.mode < SPEC_FREE).map(|f| format!("  ·  {}", f.name)).unwrap_or_default();
        let watching = if self.spec.pit.is_some() { "WATCHING FROM THE STANDS" } else { "SPECTATING" };
        text_display(p, Pos2::new(rect.left() + 22.0 * s, rect.bottom() - 52.0 * s), Align2::LEFT_CENTER, &format!("{watching}  ·  {mode}{on}"), 17.0 * s, ACCENT);
        let leave = if self.spec.pit.is_some() { "R leave the stands" } else { "Esc menu" };
        text_shadow(p, Pos2::new(rect.left() + 22.0 * s, rect.bottom() - 28.0 * s), Align2::LEFT_CENTER, &format!("1 / 2 a duellist  ·  Q / E everyone  ·  C behind  ·  V eyes  ·  F free camera (WASD, Space, Ctrl)  ·  B caster view  ·  {leave}"), 12.5 * s, Color32::from_gray(200));
    }

    /// The Court Pits' walkway: where a duel can be watched from (the pit under you, if one is on).
    pub(crate) fn stands_pit(g: &Game) -> Option<usize> {
        let f = g.f.get(g.local)?;
        let h = g.hub.as_ref()?;
        if f.pos.y < crate::varrow::PIT_WALL - 1.2 {
            return None;
        }
        (0..2).find(|&k| {
            let pit = crate::varrow::PITS[k];
            h.pits[k].is_some() && f.pos.x > pit.x.0 - 1.0 && f.pos.x < pit.x.1 + 1.0 && f.pos.z > pit.z.0 - 3.0 && f.pos.z < pit.z.1 + 1.0
        })
    }

    /// The hub: R on the pits' walkway watches the duel below with the caster's view; R again (or
    /// the duel ending) leaves it.
    pub(crate) fn stands_frame(&mut self, ctx: &egui::Context) {
        let Screen::Play { game, .. } = &self.screen else {
            self.spec.pit = None;
            return;
        };
        if !game.in_hub() {
            self.spec.pit = None;
            return;
        }
        match self.spec.pit {
            Some(k) => {
                let over = game.hub.as_ref().is_none_or(|h| h.pits[k.min(1)].is_none());
                if over || ctx.input(|i| i.key_pressed(Key::R)) {
                    self.spec.pit = None;
                }
            }
            None => {
                if let Some(k) = Self::stands_pit(game) {
                    if ctx.input(|i| i.key_pressed(Key::R)) && self.hub_ui.panel.is_none() {
                        self.spec = SpecCam { mode: SPEC_CASTER, who: game.hub.as_ref().and_then(|h| h.pits[k]).map_or(0, |d| d.who[0] as usize), pos: game.f[game.local].pos + v3(0.0, 2.0, 0.0), pit: Some(k) };
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------ replays

    /// A finished recording: saved into the Replays folder.
    pub(crate) fn save_finished_replay(&mut self) {
        let Screen::Play { game, .. } = &mut self.screen else { return };
        let Some(bytes) = game.replay_rec.as_mut().and_then(|r| r.finished.take()) else { return };
        game.replay_rec = None;
        if self.screenshot.is_some() && std::env::var_os("DUEL_REPLAY_KEEP").is_none() {
            return; // (pictures and test runs keep nothing)
        }
        let mode = game.mt.mode.name().to_string();
        self.last_replay = replay::save(&bytes, &mode);
    }

    pub(crate) fn open_replay(&mut self, path: &std::path::Path) {
        match std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| replay::decode(&b)) {
            Ok(rep) => {
                self.replay_view = Some(Box::new(replay::Viewer::new(rep, Some(path.to_path_buf()))));
                self.screen = Screen::Menu(Page::Replays);
                self.menu_msg = None;
            }
            Err(e) => self.menu_msg = Some(format!("Couldn't open that replay: {e}")),
        }
    }

    /// The Replays page (the match history), when no replay is open.
    pub(crate) fn replays_page(&mut self, ui: &mut egui::Ui) -> Option<Page> {
        ui.heading("Replays");
        ui.label(egui::RichText::new(format!("Every match you play is kept here (the newest {}): watch it again from anyone's view, slowed down or sped up, and clip its best 30 seconds.", replay::KEEP)).size(13.0).color(Color32::from_gray(195)));
        if self.replay_list.is_none() {
            self.replay_list = Some(replay::list());
        }
        let list = self.replay_list.clone().unwrap_or_default();
        let mut open = None;
        let mut delete = None;
        if list.is_empty() {
            ui.label(egui::RichText::new("No replays yet: play a match and it shows up here.").color(Color32::from_gray(180)));
        }
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            for (k, (path, info)) in list.iter().enumerate() {
                egui::Frame::new().fill(Color32::from_rgba_unmultiplied(255, 255, 255, 10)).corner_radius(6.0).inner_margin(8.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            let tag = if info.ranked { "RANKED  ·  " } else { "" };
                            ui.label(egui::RichText::new(format!("{tag}{}  ·  {}", info.mode().name(), info.map_name())).strong().color(if info.ranked { Color32::from_rgb(255, 205, 120) } else { Color32::from_rgb(245, 235, 225) }));
                            ui.label(egui::RichText::new(info.score_line()).size(13.5).color(if info.won { Color32::from_rgb(130, 220, 150) } else { Color32::from_gray(215) }));
                            ui.label(egui::RichText::new(format!("{}  ·  {}:{:02}  ·  {} fighters", info.date_text(), info.duration as u32 / 60, info.duration as u32 % 60, info.fighters.len())).size(12.0).color(Color32::from_gray(150)));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✕").on_hover_text("Delete this replay").clicked() {
                                delete = Some(k);
                            }
                            if ui.add(egui::Button::new(egui::RichText::new("Watch").strong()).fill(Color32::from_rgb(170, 90, 40))).clicked() {
                                open = Some(path.clone());
                            }
                        });
                    });
                });
            }
        });
        if let Some(k) = delete {
            let _ = std::fs::remove_file(&list[k].0);
            self.replay_list = None;
        }
        if let Some(p) = open {
            self.open_replay(&p);
        }
        if let Some(dir) = replay::folder() {
            if Self::big_button(ui, "Open the replays folder") {
                let _ = crate::platform::open_folder(&dir, None);
            }
        }
        if Self::big_button(ui, "Back") {
            self.replay_list = None;
            return Some(Page::Main);
        }
        None
    }

    /// The viewer each frame: time moves on, the keys, and a clip's pictures.
    pub(crate) fn replay_frame(&mut self, ctx: &egui::Context, dt: f32) {
        let Some(v) = self.replay_view.as_mut() else { return };
        if !matches!(self.screen, Screen::Menu(Page::Replays)) {
            self.replay_view = None;
            return;
        }
        let (space, left, right, slower, faster, q, e, c, vk, f, esc) = ctx.input(|i| {
            (i.key_pressed(Key::Space), i.key_pressed(Key::ArrowLeft), i.key_pressed(Key::ArrowRight), i.key_pressed(Key::OpenBracket), i.key_pressed(Key::CloseBracket), i.key_pressed(Key::Q), i.key_pressed(Key::E), i.key_pressed(Key::C), i.key_pressed(Key::V), i.key_pressed(Key::F), i.key_pressed(Key::Escape))
        });
        let typing = ctx.memory(|m| m.focused().is_some());
        if v.clip.is_none() && !typing {
            if space {
                if v.t >= v.duration() {
                    v.seek(0.0);
                }
                v.playing = !v.playing;
            }
            if left {
                let t = v.t - 5.0;
                v.seek(t);
            }
            if right {
                let t = v.t + 5.0;
                v.seek(t);
            }
            let k = replay::SPEEDS.iter().position(|s| (*s - v.speed).abs() < 1e-3).unwrap_or(2);
            if slower {
                v.speed = replay::SPEEDS[k.saturating_sub(1)];
            }
            if faster {
                v.speed = replay::SPEEDS[(k + 1).min(replay::SPEEDS.len() - 1)];
            }
            if q {
                v.cycle(-1);
            }
            if e {
                v.cycle(1);
            }
            if c {
                v.cam = ViewCam::Follow;
            }
            if vk {
                v.cam = ViewCam::Eyes;
            }
            if f {
                v.cam = ViewCam::Free;
            }
            if esc {
                self.replay_view = None;
                self.replay_list = None;
                self.esc_eaten = true;
                return;
            }
            // the free camera: WASD, Space / Ctrl, and dragging with the right mouse button
            if v.cam == ViewCam::Free {
                let (fw, bk, l, r, up, dn, drag) = ctx.input(|i| (i.key_down(Key::W), i.key_down(Key::S), i.key_down(Key::A), i.key_down(Key::D), i.key_down(Key::Space), i.modifiers.ctrl, if i.pointer.secondary_down() { i.pointer.delta() } else { Vec2::ZERO }));
                let b = |x: bool| x as i32 as f32;
                v.fly(b(fw) - b(bk), b(r) - b(l), b(up) - b(dn), drag.x * 0.004, -drag.y * 0.004, dt);
            }
        }
        // a clip: one picture per frame, saved as it arrives
        let clip_dir = v.clip.as_ref().map(|c| c.dir.clone());
        if let Some(dir) = clip_dir {
            let got = self.shot_slot.lock().ok().and_then(|mut s| s.take());
            let v = self.replay_view.as_mut().unwrap();
            let c = v.clip.as_mut().unwrap();
            if let Some(img) = got {
                let [w, h] = img.size;
                let mut bytes = Vec::with_capacity(w * h * 4);
                for px in &img.pixels {
                    let [r, g, b, _] = px.to_array();
                    bytes.extend_from_slice(&[r, g, b, 255]);
                }
                let _ = image::save_buffer(dir.join(format!("frame_{:05}.png", c.frame)), &bytes, w as u32, h as u32, image::ColorType::Rgba8);
                c.frame += 1;
                c.waiting = false;
            }
            let done = c.from + c.frame as f32 / replay::CLIP_FPS > c.to;
            if done {
                let n = c.frame;
                let text = format!("A clip from Duel Rush: {n} pictures at {fps} frames a second (frame_00000.png on).\nTo make a video with ffmpeg:\n  ffmpeg -framerate {fps} -i frame_%05d.png -c:v libx264 -pix_fmt yuv420p clip.mp4\n", fps = replay::CLIP_FPS as u32);
                let _ = std::fs::write(dir.join("README.txt"), text);
                v.clip = None;
                v.note = Some((format!("Clip saved: {n} pictures in {}", dir.display()), 6.0));
                let _ = crate::platform::open_folder(&dir, None);
            } else if !c.waiting {
                c.waiting = true;
                let slot = self.shot_slot.clone();
                ctx.request_screenshot(move |img| {
                    if let Ok(mut s) = slot.lock() {
                        *s = Some(img);
                    }
                });
            }
        }
        if let Some(v) = self.replay_view.as_mut() {
            v.step(dt);
        }
    }

    /// The viewer's controls: the timeline, play / pause, speed, the camera and whose view, the clip.
    pub(crate) fn replay_hud(&mut self, ctx: &egui::Context, rect: Rect) {
        let Some(v) = self.replay_view.as_mut() else { return };
        let s = (rect.height() / 900.0).clamp(0.75, 1.6);
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("replay-hud")));
        let info = v.rep.info.clone();
        // the title
        p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(rect.width(), 54.0 * s)), 0.0, Color32::from_black_alpha(170));
        text_display(&p, Pos2::new(rect.left() + 20.0 * s, rect.top() + 27.0 * s), Align2::LEFT_CENTER, "REPLAY", 26.0 * s, Color32::from_rgb(255, 150, 70));
        let tag = if info.ranked { "  ·  RANKED" } else { "" };
        text_shadow(&p, Pos2::new(rect.left() + 130.0 * s, rect.top() + 27.0 * s), Align2::LEFT_CENTER, &format!("{}  ·  {}{tag}  ·  {}  ·  {}", info.mode().name(), info.map_name(), info.score_line(), info.date_text()), 15.0 * s, Color32::from_gray(225));
        if v.clip.is_some() {
            let c = v.clip.as_ref().unwrap();
            let k = (c.frame as f32 / ((c.to - c.from) * replay::CLIP_FPS).max(1.0)).min(1.0);
            text_display(&p, Pos2::new(rect.center().x, rect.top() + 80.0 * s), Align2::CENTER_CENTER, &format!("CLIPPING  {:.0}%", k * 100.0), 22.0 * s, Color32::from_rgb(255, 80, 70));
            return; // (the pictures show the match alone)
        }
        // who's in view
        let who = &v.game.f[v.who.min(v.game.f.len() - 1)];
        let cam = match v.cam {
            ViewCam::Follow => format!("Behind {}", who.name),
            ViewCam::Eyes => format!("{}'s eyes", who.name),
            ViewCam::Free => "Free camera".into(),
        };
        text_shadow(&p, Pos2::new(rect.right() - 20.0 * s, rect.top() + 27.0 * s), Align2::RIGHT_CENTER, &cam, 15.0 * s, ACCENT);
        // the scoreboard strip
        let mut y = rect.top() + 70.0 * s;
        for fi in &info.fighters {
            let f = &v.game.f[(fi.slot as usize).min(v.game.f.len() - 1)];
            let score = if info.mode().rounds() { f.wins } else { f.kills };
            let col = if fi.slot as usize == v.who { ACCENT } else { Color32::from_gray(220) };
            draw_rank_badge(&p, Pos2::new(rect.left() + 30.0 * s, y), 15.0 * s, fi.rank, 255);
            text_shadow(&p, Pos2::new(rect.left() + 44.0 * s, y), Align2::LEFT_CENTER, &format!("{}  {}  {}", f.name, score, if f.alive { format!("{:.0} hp", f.hp.max(0.0)) } else { "down".into() }), 14.0 * s, col);
            y += 22.0 * s;
        }
        // the kill feed
        let mut fy = rect.top() + 70.0 * s;
        for l in v.game.feed.iter().take(5) {
            let k = l.killer.as_ref().map_or("", |k| k.0.as_str());
            text_shadow(&p, Pos2::new(rect.right() - 20.0 * s, fy), Align2::RIGHT_CENTER, &format!("{k}  [{}]{}  {}", l.tool, if l.head { " HS" } else { "" }, l.victim.0), 13.0 * s, Color32::from_gray(225));
            fy += 20.0 * s;
        }
        if let Some((n, _)) = &v.note {
            text_display(&p, Pos2::new(rect.center().x, rect.top() + 84.0 * s), Align2::CENTER_CENTER, n, 16.0 * s, Color32::from_rgb(140, 230, 160));
        }
        // the controls panel at the bottom
        let dur = v.duration();
        let mut seek_to = None;
        let mut clip = false;
        let mut close = false;
        let panel_w = rect.width() - 60.0 * s;
        egui::Area::new(egui::Id::new("replay-controls")).fixed_pos(Pos2::new(rect.left() + 30.0 * s, rect.bottom() - 140.0 * s)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::new().fill(Color32::from_rgba_unmultiplied(10, 8, 18, 225)).corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
                ui.set_width(panel_w - 20.0);
                // the timeline: drag to scrub (rounds marked)
                let (tr, resp) = ui.allocate_exact_size(Vec2::new(panel_w - 20.0, 22.0), egui::Sense::click_and_drag());
                let tp = ui.painter_at(tr);
                tp.rect_filled(Rect::from_center_size(tr.center(), Vec2::new(tr.width(), 6.0)), 3.0, Color32::from_gray(60));
                let k = if dur > 0.0 { v.t / dur } else { 0.0 };
                tp.rect_filled(Rect::from_min_size(Pos2::new(tr.left(), tr.center().y - 3.0), Vec2::new(tr.width() * k, 6.0)), 3.0, Color32::from_rgb(255, 140, 60));
                // the kills along it
                let t0 = v.rep.frames.first().map_or(0.0, |f| f.t);
                for fr in &v.rep.frames {
                    if !fr.feed.is_empty() {
                        let x = tr.left() + tr.width() * ((fr.t - t0) / dur.max(0.01));
                        tp.line_segment([Pos2::new(x, tr.top() + 2.0), Pos2::new(x, tr.bottom() - 2.0)], Stroke::new(2.0, Color32::from_rgb(235, 70, 60)));
                    }
                }
                tp.circle_filled(Pos2::new(tr.left() + tr.width() * k, tr.center().y), 8.0, Color32::WHITE);
                if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
                    seek_to = Some(((pos.x - tr.left()) / tr.width()).clamp(0.0, 1.0) * dur);
                }
                ui.horizontal(|ui| {
                    if ui.button(if v.playing { "⏸ Pause" } else { "▶ Play" }).clicked() {
                        if v.t >= dur {
                            v.seek(0.0);
                        }
                        v.playing = !v.playing;
                    }
                    ui.label(egui::RichText::new(format!("{}:{:04.1} / {}:{:02}", v.t as u32 / 60, v.t % 60.0, dur as u32 / 60, dur as u32 % 60)).monospace());
                    ui.separator();
                    ui.label("Speed");
                    for sp in replay::SPEEDS {
                        if ui.selectable_label((v.speed - sp).abs() < 1e-3, format!("{sp}x")).clicked() {
                            v.speed = sp;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    for (cm, name) in [(ViewCam::Follow, "Behind"), (ViewCam::Eyes, "First person"), (ViewCam::Free, "Free camera")] {
                        if ui.selectable_label(v.cam == cm, name).clicked() {
                            v.cam = cm;
                        }
                    }
                    ui.separator();
                    let fighters = v.rep.info.fighters.clone();
                    for fi in &fighters {
                        if ui.selectable_label(v.who == fi.slot as usize, &fi.name).clicked() {
                            v.who = fi.slot as usize;
                            if v.cam == ViewCam::Free {
                                v.cam = ViewCam::Follow;
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("✂ Clip last 30 s").on_hover_text("Saves a picture sequence (30 a second) of the 30 seconds up to here, ready for a video editor").clicked() {
                        clip = true;
                    }
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
                ui.label(egui::RichText::new("Space play / pause  ·  ← → 5 s  ·  [ ] speed  ·  Q / E player  ·  C behind  ·  V eyes  ·  F free (WASD, Space, Ctrl, right-drag to look)  ·  Esc close").size(11.5).color(Color32::from_gray(170)));
            });
        });
        let v = self.replay_view.as_mut().unwrap();
        if let Some(t) = seek_to {
            v.seek(t);
        }
        if clip {
            let base = replay::folder().map(|d| d.join("clips")).unwrap_or_else(std::env::temp_dir);
            let dir = base.join(format!("clip_{}", now_secs()));
            v.start_clip(dir);
        }
        if close {
            self.replay_view = None;
            self.replay_list = None;
        }
    }

    /// Test switch DUEL_COMP=<page>: pictures of the competitive side. ranked_queue: the Ranked page
    /// searching; badges: ranks on everyone (with DUEL_HUB=admin, DUEL_GALLERY=admin:badges);
    /// killcam: a match played until you're killed, the killcam running; caster: two bots duelling
    /// watched as a spectator; postmatch: a duel played to its end, ranked up.
    pub(crate) fn comp_setup(&mut self, page: &str) {
        let staged_profile = || {
            let mut p = ranked::Profile { rating: Rating { r: 1588.0, rd: 72.0, vol: 0.06 }, games: 23, wins: 14, peak: 1611.0, day: career::today(), pending: None, board: 0, history: Vec::new() };
            for k in 0..10 {
                p.history.push(ranked::Played { day: career::today(), won: k % 3 != 1, delta: if k % 3 != 1 { 14.0 } else { -12.0 }, opp: 1570.0, left: false });
            }
            p
        };
        let name = clean_name(&self.settings.name, 0);
        match page {
            "ranked_queue" => {
                self.ranked = staged_profile();
                self.ranked_q = Some(RankedQueue { t: 21.0, ask: 2.0, ..Default::default() });
                self.screen = Screen::Menu(Page::Ranked);
            }
            "ranked" => {
                self.ranked = staged_profile();
                self.screen = Screen::Menu(Page::Ranked);
            }
            "killcam" => {
                let mut g = Game::offline(&name, 1, 1.0, Mode::Ffa, 99);
                g.set_map(MapId::Arena);
                g.f[0].is_bot = true;
                g.bots[0] = Some(bot::Bot::new(0.15, 9));
                g.set_ready(Loadout { melee: Weapon::Katana, gun1: Weapon::Rifle, gun2: Weapon::Revolver, item: 0, body: 0, outfit: 0 });
                g.f[1].rank = Rank::to_byte(Some(Rank { tier: 5, div: 1 }));
                g.f[1].loadout = Loadout { melee: Weapon::Dagger, gun1: Weapon::Rifle, gun2: Weapon::HandCannon, item: 0, body: 2, outfit: 0 };
                // (until a kill on you that the killcam shows as a headshot, or one at all)
                let mut any = None;
                for _ in 0..(600.0 / TICK) as usize {
                    g.tick(&Input::default());
                    if let Some(dc) = g.deathcam.as_ref() {
                        if dc.head {
                            any = None;
                            break;
                        }
                        any.get_or_insert(dc.clone());
                        g.skip_deathcam();
                    }
                }
                if g.deathcam.is_none() {
                    g.deathcam = any;
                }
                if let Some(dc) = g.deathcam.as_mut() {
                    dc.t = std::env::var("DUEL_KILLCAM_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(2.4);
                }
                self.autoplay = true;
                self.start_game(g, None);
            }
            "caster" => {
                let mut g = Game::offline(&name, 2, 0.8, Mode::Duel, 3);
                g.set_map(MapId::Arena);
                g.remove_player(0);
                g.watcher = true;
                for s in 1..3 {
                    g.f[s].ready = true;
                    g.f[s].rank = Rank::to_byte(Some(Rank { tier: 3 + s as u8, div: s as u8 }));
                }
                for _ in 0..(std::env::var("DUEL_CASTER_SECS").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(14.0) / TICK) as usize {
                    g.tick(&Input::default());
                }
                self.autoplay = true;
                self.start_game(g, None);
                if let Screen::Play { game, .. } = &mut self.screen {
                    game.watcher = true;
                }
                self.spec = SpecCam { mode: SPEC_CASTER, who: 1, pos: v3(0.0, 6.0, 0.0), pit: None };
            }
            "postmatch" => {
                let mut g = Game::offline(&name, 1, 0.45, Mode::Duel, 2);
                g.set_map(MapId::Arena);
                g.f[0].is_bot = true;
                g.bots[0] = Some(bot::Bot::new(0.95, 9));
                g.set_ready(Loadout::default());
                for _ in 0..(400.0 / TICK) as usize {
                    g.tick(&Input::default());
                    if g.mt.phase == Phase::MatchOver {
                        break;
                    }
                }
                g.killcam = None;
                g.slowmo = 0.0;
                g.skip_potm();
                if g.my_best.is_none() {
                    g.my_best = Some((100.0, "Triple slash combo".into()));
                }
                let before = Some(Rank { tier: 3, div: 0 });
                let after = Some(Rank { tier: 4, div: 2 });
                self.ranked_change = Some((ranked::Change { before, after, delta: 23.4, rating: 1601.0, won: g.local_won() || true, placement: 0, left: false }, 0.0));
                self.ranked_match = Some(RankedMatch { settled: true, begun: true, ..Default::default() });
                self.autoplay = true;
                self.match_xp = 140;
                self.start_game(g, None);
            }
            _ => {}
        }
    }

    /// Each frame of a picture's scene: what the game would otherwise reset.
    pub(crate) fn comp_frame(&mut self) {
        let Ok(page) = std::env::var("DUEL_COMP") else { return };
        if page == "badges" {
            // (the camera on the three standing in front of you)
            self.aim_yaw = 0.0;
            self.aim_pitch = -0.04;
        }
        let Screen::Play { game, .. } = &mut self.screen else { return };
        match page.as_str() {
            "badges" => {
                // you and three others standing in a row before the camera (DUEL_VIEW 0,1.9,-10,0,-0.04,58)
                let me = game.local;
                let spots = [v3(0.0, 0.0, -6.5), v3(-2.6, 0.0, -2.4), v3(0.0, 0.0, -1.8), v3(2.6, 0.0, -2.4)];
                let mut k = 0;
                let who: Vec<usize> = std::iter::once(me).chain((0..game.f.len()).filter(|&s| s != me && game.f[s].present)).take(4).collect();
                for s in who {
                    let f = &mut game.f[s];
                    f.pos = spots[k];
                    f.prev_pos = spots[k];
                    f.vel = V3::ZERO;
                    f.yaw = if k == 0 { 0.0 } else { std::f32::consts::PI };
                    k += 1;
                }
                let ranks = [(4u8, 1u8), (6, 0), (7, 0), (2, 1), (3, 2), (5, 0), (0, 2), (1, 0)];
                for s in 0..game.f.len() {
                    if game.f[s].present {
                        let (t, d) = ranks[s % ranks.len()];
                        game.f[s].rank = Rank::to_byte(Some(Rank { tier: t, div: d }));
                    }
                }
            }
            "caster" => {
                game.watcher = true;
                for s in 1..3 {
                    game.f[s].rank = Rank::to_byte(Some(Rank { tier: 3 + s as u8, div: s as u8 }));
                }
            }
            "postmatch" => {
                game.killcam = None;
                game.skip_potm();
            }
            _ => {}
        }
    }

    /// Test switch DUEL_REPLAY_DEMO: play a short match against a bot here (in a moment, without
    /// drawing it), record it, and open it in the viewer (for pictures of the viewer).
    pub(crate) fn replay_demo(&mut self) {
        let s = replay::Setup { name: clean_name(&self.settings.name, 0), bots: 1, skill: 0.7, mode: Mode::Duel as u8, target: 3, map: MapId::Arena as u8, seed: 77, loadout: Loadout::default().to_bytes(), classic: false, fin_locks: 0 };
        let mut g = s.game();
        g.replay_rec = Some(Box::new(replay::Recorder::new(Some(s))));
        g.f[0].is_bot = true;
        g.bots[0] = Some(bot::Bot::new(0.75, 5));
        g.set_ready(Loadout::default());
        let secs: f32 = std::env::var("DUEL_REPLAY_DEMO").ok().and_then(|v| v.parse().ok()).filter(|v: &f32| *v > 5.0).unwrap_or(45.0);
        for _ in 0..(secs / TICK) as usize {
            g.tick(&Input::default());
        }
        let mut rec = g.replay_rec.take().unwrap();
        let bytes = rec.finish(&g);
        if let Ok(rep) = replay::decode(&bytes) {
            let mut v = replay::Viewer::new(rep, None);
            let at = std::env::var("DUEL_REPLAY_AT").ok().and_then(|x| x.parse().ok()).unwrap_or(v.duration() * 0.6);
            v.seek(at);
            v.playing = false;
            if std::env::var_os("DUEL_REPLAY_EYES").is_some() {
                v.cam = ViewCam::Eyes;
            }
            self.replay_view = Some(Box::new(v));
            self.screen = Screen::Menu(Page::Replays);
        }
    }
}
