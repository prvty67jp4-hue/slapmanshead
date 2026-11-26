//! Seasons on screen: the season page (the track you scroll, each reward previewed on your
//! fighter, the countdown; the challenges), the profile page (your card, what's on it, the match
//! history), a player's card in Varrow Centre, the season start screen and the patch notes. The
//! rules are in crate::season.

use super::*;
use crate::season::{self, Card, Item, Pattern, Reward, Season};

#[derive(Default)]
pub struct SeasonUi {
    /// The season page's tab (0 the track, 1 challenges), the profile page's (0 card, 1 history).
    pub tab: u8,
    pub profile_tab: u8,
    /// The reward picked on the track (a Registry::items index).
    pub sel: Option<usize>,
    /// The season start screen is up; a player's card is up (their slot).
    pub start_open: bool,
    pub view: Option<usize>,
    /// Frames the card has been up (the click that opened it doesn't close it).
    pub view_frames: u32,
    /// The menu's fighter is showing a reward.
    pub preview_on: bool,
    pub save_t: f32,
    pub card_t: f32,
    pub roll_t: f32,
    pub patch: Option<String>,
    /// Scroll the track to the current tier once.
    pub scroll_once: bool,
}

/// The patch notes (patch_notes.txt in the save folder or beside the game; the built-in copy).
pub fn load_patch_notes() -> String {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(p) = platform::data_file("patch_notes.txt") {
        paths.push(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            paths.push(d.join("patch_notes.txt"));
            paths.push(d.join("../../patch_notes.txt"));
        }
    }
    for p in paths {
        if let Ok(t) = std::fs::read_to_string(&p) {
            if !t.trim().is_empty() {
                return t;
            }
        }
    }
    include_str!("../patch_notes.txt").to_string()
}

fn c32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
fn c32a(c: [u8; 3], a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], a)
}

/// A reward's icon, drawn from what it is.
pub fn paint_reward(p: &egui::Painter, r: Rect, it: &Item, owned: bool) {
    let c = r.center();
    let s = r.width().min(r.height());
    let col = c32(it.reward.colour());
    let dim = |c: Color32| if owned { c } else { ui_kit::mix(c, Color32::from_gray(60), 0.6) };
    match &it.reward {
        Reward::Outfit { coat, hem } => {
            // a long coat: shoulders, a body flaring to the hem
            let w = s * 0.32;
            let pts = vec![c + Vec2::new(-w * 0.55, -s * 0.34), c + Vec2::new(w * 0.55, -s * 0.34), c + Vec2::new(w, s * 0.3), c + Vec2::new(-w, s * 0.3)];
            p.add(egui::Shape::convex_polygon(pts, dim(c32(*coat)), Stroke::new(1.0, Color32::from_black_alpha(120))));
            p.rect_filled(Rect::from_min_max(c + Vec2::new(-w, s * 0.24), c + Vec2::new(w, s * 0.34)), 2.0, dim(c32(*hem)));
            p.line_segment([c + Vec2::new(0.0, -s * 0.34), c + Vec2::new(0.0, s * 0.3)], Stroke::new(1.5, Color32::from_black_alpha(90)));
        }
        Reward::Dye(_) => {
            // a dye drop
            p.circle_filled(c + Vec2::new(0.0, s * 0.08), s * 0.22, dim(col));
            p.add(egui::Shape::convex_polygon(vec![c + Vec2::new(0.0, -s * 0.32), c + Vec2::new(s * 0.19, -s * 0.02), c + Vec2::new(-s * 0.19, -s * 0.02)], dim(col), Stroke::NONE));
            p.circle_filled(c + Vec2::new(-s * 0.07, s * 0.02), s * 0.05, Color32::from_white_alpha(120));
        }
        Reward::Finish { shine, .. } => {
            // a blade in the finish, with its shine
            let a = c + Vec2::new(-s * 0.3, s * 0.3);
            let b = c + Vec2::new(s * 0.3, -s * 0.3);
            p.line_segment([a, b], Stroke::new(s * 0.1, dim(col)));
            p.line_segment([a + Vec2::new(s * 0.03, -s * 0.06), b - Vec2::new(s * 0.06, -s * 0.03)], Stroke::new(s * 0.025, Color32::from_white_alpha((80.0 + 150.0 * shine) as u8)));
            p.line_segment([a + Vec2::new(-s * 0.1, -s * 0.1), a + Vec2::new(s * 0.1, s * 0.1)], Stroke::new(s * 0.06, dim(Color32::from_rgb(200, 160, 70))));
        }
        Reward::Emote(_) => {
            // a figure with an arm raised
            let st = Stroke::new(s * 0.06, dim(Color32::from_rgb(255, 210, 160)));
            p.circle_filled(c + Vec2::new(0.0, -s * 0.22), s * 0.09, st.color);
            p.line_segment([c + Vec2::new(0.0, -s * 0.12), c + Vec2::new(0.0, s * 0.12)], st);
            p.line_segment([c + Vec2::new(0.0, -s * 0.06), c + Vec2::new(s * 0.2, -s * 0.24)], st);
            p.line_segment([c + Vec2::new(0.0, -s * 0.06), c + Vec2::new(-s * 0.18, s * 0.04)], st);
            p.line_segment([c + Vec2::new(0.0, s * 0.12), c + Vec2::new(-s * 0.12, s * 0.32)], st);
            p.line_segment([c + Vec2::new(0.0, s * 0.12), c + Vec2::new(s * 0.12, s * 0.32)], st);
        }
        Reward::Title => {
            // a scroll with lines of writing
            let rr = Rect::from_center_size(c, Vec2::new(s * 0.62, s * 0.44));
            p.rect_filled(rr, s * 0.04, dim(Color32::from_rgb(232, 214, 176)));
            for k in 0..3 {
                let y = rr.top() + rr.height() * (0.3 + k as f32 * 0.2);
                p.line_segment([Pos2::new(rr.left() + s * 0.08, y), Pos2::new(rr.right() - s * 0.08 - k as f32 * s * 0.06, y)], Stroke::new(s * 0.03, Color32::from_rgb(90, 40, 30)));
            }
            p.circle_filled(Pos2::new(rr.right() - s * 0.06, rr.bottom()), s * 0.07, dim(Color32::from_rgb(180, 30, 36)));
        }
        Reward::Banner { top, bottom, pattern } => {
            let rr = Rect::from_center_size(c, Vec2::new(s * 0.5, s * 0.68));
            ui_kit::vgrad(p, rr, dim(c32(*top)), dim(c32(*bottom)));
            banner_pattern(p, rr, *pattern, owned);
            p.add(egui::Shape::convex_polygon(vec![Pos2::new(rr.left(), rr.bottom()), Pos2::new(rr.center().x, rr.bottom() - s * 0.1), Pos2::new(rr.right(), rr.bottom()), Pos2::new(rr.right(), rr.bottom() + s * 0.06), Pos2::new(rr.left(), rr.bottom() + s * 0.06)], Color32::from_black_alpha(0), Stroke::NONE));
        }
        Reward::Nameplate { fill, text } => {
            let rr = Rect::from_center_size(c, Vec2::new(s * 0.78, s * 0.3));
            p.rect_filled(rr, s * 0.08, dim(c32(*fill)));
            p.rect_stroke(rr, s * 0.08, Stroke::new(1.0, c32a(*text, 140)), egui::StrokeKind::Inside);
            p.text(rr.center(), Align2::CENTER_CENTER, "NAME", ui_kit::display(s * 0.18), dim(c32(*text)));
        }
        Reward::Pet { col } => {
            p.circle_filled(c, s * 0.3, c32a(*col, if owned { 60 } else { 25 }));
            p.circle_filled(c, s * 0.17, dim(c32(*col)));
            for x in [-1.0f32, 1.0] {
                p.circle_filled(c + Vec2::new(s * 0.06 * x, -s * 0.03), s * 0.03, Color32::from_rgb(40, 10, 10));
            }
        }
    }
}

fn banner_pattern(p: &egui::Painter, r: Rect, pat: Pattern, bright: bool) {
    let a = if bright { 70 } else { 30 };
    match pat {
        Pattern::Plain => {}
        Pattern::Moon => {
            let c = Pos2::new(r.right() - r.width() * 0.28, r.top() + r.height() * 0.32);
            let rad = r.width().min(r.height()) * 0.2;
            p.circle_filled(c, rad * 1.8, Color32::from_rgba_unmultiplied(255, 120, 110, a / 2));
            p.circle_filled(c, rad, Color32::from_rgba_unmultiplied(255, 140, 120, a * 2));
        }
        Pattern::Stripes => {
            let n = 6;
            for k in 0..n {
                let x = r.left() + r.width() * (k as f32 + 0.5) / n as f32;
                p.line_segment([Pos2::new(x, r.top()), Pos2::new(x - r.height() * 0.3, r.bottom())], Stroke::new(r.width() / (n as f32 * 3.0), Color32::from_white_alpha(a / 2)));
            }
        }
        Pattern::Chevron => {
            for k in 0..3 {
                let y = r.top() + r.height() * (0.25 + k as f32 * 0.22);
                let w = Stroke::new(r.height() * 0.05, Color32::from_white_alpha(a / 2 + 10));
                p.line_segment([Pos2::new(r.left(), y), Pos2::new(r.center().x, y + r.height() * 0.12)], w);
                p.line_segment([Pos2::new(r.center().x, y + r.height() * 0.12), Pos2::new(r.right(), y)], w);
            }
        }
    }
}

/// A profile card. `border`: the prestige colour.
pub fn paint_card(p: &egui::Painter, r: Rect, c: &Card, border: Option<[u8; 3]>) {
    let s = r.width() / 520.0;
    let reg = season::reg();
    p.rect_filled(r.translate(Vec2::new(0.0, 6.0 * s)), 14.0 * s, Color32::from_black_alpha(120));
    p.rect_filled(r, 14.0 * s, Color32::from_rgb(18, 15, 24));
    // the banner across the top
    let band = Rect::from_min_max(r.min, Pos2::new(r.right(), r.top() + r.height() * 0.44));
    let (top, bottom, pat) = match reg.items.get(c.banner as usize).map(|i| &i.reward) {
        Some(Reward::Banner { top, bottom, pattern }) => (c32(*top), c32(*bottom), *pattern),
        _ => (Color32::from_rgb(60, 44, 40), Color32::from_rgb(28, 22, 26), Pattern::Plain),
    };
    ui_kit::vgrad(p, band.shrink(2.0 * s), top, bottom);
    banner_pattern(p, band.shrink(2.0 * s), pat, true);
    // season and tier
    let season_name = reg.season(c.season as u32).map_or(String::new(), |s| s.name.to_uppercase());
    let tier = if c.prestige > 0 { format!("TIER 50  ·  PRESTIGE {}", c.prestige) } else { format!("TIER {}", c.tier) };
    p.text(band.min + Vec2::new(16.0, 12.0) * s, Align2::LEFT_TOP, format!("SEASON {}  ·  {season_name}", c.season), ui_kit::display(14.0 * s), Color32::from_rgba_unmultiplied(255, 235, 220, 220));
    p.text(band.min + Vec2::new(16.0, 30.0) * s, Align2::LEFT_TOP, tier, ui_kit::display(13.0 * s), Color32::from_rgba_unmultiplied(255, 205, 170, 200));
    // the nameplate and the name
    let plate = Rect::from_min_size(Pos2::new(r.left() + 16.0 * s, band.bottom() - 30.0 * s), Vec2::new(r.width() - 32.0 * s, 52.0 * s));
    let (fill, text) = match reg.items.get(c.nameplate as usize).map(|i| &i.reward) {
        Some(Reward::Nameplate { fill, text }) => (c32a(*fill, 240), c32(*text)),
        _ => (Color32::from_rgba_unmultiplied(12, 10, 18, 220), Color32::WHITE),
    };
    p.rect_filled(plate, 8.0 * s, fill);
    p.rect_stroke(plate, 8.0 * s, Stroke::new(1.0, Color32::from_white_alpha(40)), egui::StrokeKind::Inside);
    let name = if c.name.is_empty() { "PLAYER".to_string() } else { c.name.to_uppercase() };
    let nx = plate.left() + 60.0 * s;
    let nf = ui_kit::fitted(p, &name, ui_kit::display(30.0 * s), plate.width() - 80.0 * s);
    p.text(Pos2::new(nx, plate.center().y - 2.0 * s), Align2::LEFT_CENTER, &name, nf, text);
    if c.rank & 0x7f != 0 {
        crate::competitive::draw_rank_badge(p, Pos2::new(plate.left() + 30.0 * s, plate.center().y), 34.0 * s, c.rank, 255);
    } else {
        p.circle_stroke(Pos2::new(plate.left() + 30.0 * s, plate.center().y), 14.0 * s, Stroke::new(1.5, Color32::from_white_alpha(60)));
    }
    // the title, the rank and the favourite fighter
    let mut y = plate.bottom() + 10.0 * s;
    if let Some(t) = c.title_text() {
        p.text(Pos2::new(nx, y), Align2::LEFT_TOP, t, FontId::proportional(15.0 * s), Color32::from_rgb(255, 205, 170));
    }
    y += 22.0 * s;
    let rank = crate::ranked::Rank::from_byte(c.rank).map_or("Unranked".to_string(), |k| k.name());
    p.text(Pos2::new(r.left() + 20.0 * s, y), Align2::LEFT_TOP, format!("{rank}  ·  favourite: {}", chars::name(c.fav)), FontId::proportional(13.0 * s), Color32::from_gray(190));
    y += 24.0 * s;
    // top stats
    let cw = (r.width() * 0.58) / 3.0;
    for (k, (label, v)) in season::CARD_STATS.iter().zip(c.stats.iter()).enumerate() {
        let x = r.left() + 20.0 * s + cw * (k % 3) as f32;
        let yy = y + (k / 3) as f32 * 44.0 * s;
        p.text(Pos2::new(x, yy), Align2::LEFT_TOP, v.to_string(), ui_kit::display(22.0 * s), Color32::WHITE);
        p.text(Pos2::new(x, yy + 24.0 * s), Align2::LEFT_TOP, label.to_uppercase(), FontId::proportional(10.5 * s), Color32::from_gray(150));
    }
    // the showcase
    let sx = r.left() + r.width() * 0.64;
    let box_w = (r.right() - 16.0 * s - sx) / 3.0;
    for (k, &idx) in c.showcase.iter().enumerate() {
        let b = Rect::from_min_size(Pos2::new(sx + box_w * k as f32 + 3.0 * s, y - 4.0 * s), Vec2::new(box_w - 6.0 * s, 86.0 * s));
        p.rect_filled(b, 6.0 * s, Color32::from_rgba_unmultiplied(255, 255, 255, 10));
        match reg.items.get(idx as usize) {
            Some(it) => {
                paint_reward(p, Rect::from_center_size(b.center() - Vec2::new(0.0, 8.0 * s), Vec2::splat(56.0 * s)), it, true);
                let f = ui_kit::fitted(p, &it.name, FontId::proportional(10.0 * s), b.width() - 4.0);
                p.text(Pos2::new(b.center().x, b.bottom() - 4.0 * s), Align2::CENTER_BOTTOM, &it.name, f, Color32::from_gray(200));
            }
            None => {
                p.text(b.center(), Align2::CENTER_CENTER, "—", FontId::proportional(18.0 * s), Color32::from_gray(80));
            }
        }
    }
    // the frame: the prestige colour, else a quiet one
    let (w, col) = match border {
        Some(b) => (4.0 * s, c32(b)),
        None => (1.5 * s, Color32::from_white_alpha(50)),
    };
    p.rect_stroke(r, 14.0 * s, Stroke::new(w, col), egui::StrokeKind::Inside);
    if border.is_some() {
        p.rect_stroke(r.shrink(6.0 * s), 10.0 * s, Stroke::new(1.0, col.gamma_multiply(0.5)), egui::StrokeKind::Inside);
    }
}

impl App {
    /// The season record at start-up: today's challenges, a new season settled (its ranked soft
    /// reset), and the start screen once per season.
    pub(crate) fn season_start(&mut self) {
        self.season = season::load();
        self.season_roll();
        let test = self.season_test_run();
        let reg = season::reg();
        if let Some(s) = reg.current(season::day_now()).filter(|s| s.active(season::day_now())) {
            if self.season.seen != s.id && !test {
                self.season_ui.start_open = true;
            }
        }
    }

    /// A picture or test run: no start screen, no ranked reset, nothing saved.
    fn season_test_run(&self) -> bool {
        self.screenshot.is_some() || self.autoplay || std::env::var_os("DUEL_AUTOPLAY").is_some() || std::env::var_os("DUEL_BENCH").is_some()
    }

    /// New day, week or season.
    fn season_roll(&mut self) {
        let reg = season::reg();
        let day = season::day_now();
        if let Some(ro) = self.season.roll(reg, day) {
            if !ro.rank_rewards.is_empty() {
                self.steam_note(format!("Season-end rewards for your peak rank: {}", ro.rank_rewards.join(", ")));
            }
        }
        // the ranked soft reset, once per season (not for test runs)
        if let Some(s) = reg.current(day).filter(|s| s.active(day)) {
            if self.season.ranked_reset != s.id && !self.season_test_run() {
                if self.ranked.games > 0 {
                    season::soft_reset(&mut self.ranked.rating);
                    ranked::save(&self.ranked, self.ranked_account);
                    self.ranked_note = Some(format!("Season {} has begun: your ranked rating moved halfway back to the middle and will settle quickly.", s.id));
                }
                self.season.ranked_reset = s.id;
                self.season.dirty = true;
            }
        }
        self.season.publish(reg);
    }

    /// The season that's on (or the last / next one between seasons).
    pub(crate) fn season_now(&self) -> Option<&'static Season> {
        season::reg().current(season::day_now())
    }

    /// Every frame: roll over at midnight UTC, the peak rank, what's worn, the card in the hub, notes.
    pub(crate) fn season_frame(&mut self, dt: f32) {
        self.season_ui.roll_t -= dt;
        if self.season_ui.roll_t <= 0.0 {
            self.season_ui.roll_t = 10.0;
            self.season_roll();
            let gated = self.ach_gated();
            self.season.rank_seen(self.ranked.rank().map(|k| k.tier), gated);
        }
        // what the season gave: said in a match, under the friends button in the menus
        for n in std::mem::take(&mut self.season.notes) {
            self.steam_note(n);
            self.season.publish(season::reg());
        }
        // the season pet on your shoulder in Varrow Centre (or off again)
        let pet = self.season.equip.pet.as_deref().and_then(|k| season::reg().item(k)).and_then(|(i, _)| season::pet_value(i));
        if let Screen::Play { game, .. } = &mut self.screen {
            if game.in_hub() {
                let me = game.local;
                let cur = game.f[me].hub.look.pet;
                let want = match pet {
                    Some(v) => v,
                    None if cur >= 2 => 0,
                    None => cur,
                };
                if want != cur {
                    game.f[me].hub.look.pet = want;
                    let look = game.f[me].hub.look;
                    game.hub_send(crate::hub::HubEv::Look { who: me as u8, look });
                }
            }
        }
        // our card round the hub now and then (and to anyone who joins)
        self.season_ui.card_t -= dt;
        if self.season_ui.card_t <= 0.0 {
            self.season_ui.card_t = 8.0;
            let card = self.my_card().to_bytes();
            if let Screen::Play { game, .. } = &mut self.screen {
                if game.in_hub() {
                    let me = game.local as u8;
                    game.hub_send(crate::hub::HubEv::Card { who: me, card });
                }
            }
        }
        self.season_ui.save_t -= dt;
        if self.season.dirty && self.season_ui.save_t <= 0.0 && self.screenshot.is_none() {
            self.season_ui.save_t = 3.0;
            season::save(&mut self.season);
        }
    }

    /// Wear what's equipped from the season locker (called with the mastery outfit each frame).
    pub(crate) fn season_wear(&mut self) {
        let reg = season::reg();
        if let Some(n) = self.season.equip.outfit.as_deref().filter(|k| self.season.owned.contains(*k)).and_then(|k| reg.item(k)).and_then(|(i, _)| season::outfit_number(i)) {
            self.settings.loadout.outfit = n;
        }
        self.settings.loadout.finish = self.season.equip.finish.as_deref().filter(|k| self.season.owned.contains(*k)).and_then(|k| reg.item(k)).and_then(|(i, _)| season::finish_number(i)).unwrap_or(0);
    }

    /// This player's card.
    pub(crate) fn my_card(&self) -> Card {
        let reg = season::reg();
        let idx = |k: &Option<String>| k.as_deref().filter(|k| self.season.owned.contains(*k)).and_then(|k| reg.item(k)).map_or(season::NONE, |(i, _)| i as u16);
        let e = &self.season.equip;
        let (tier, prestige, sid) = match self.season_now() {
            Some(s) if self.season.season == s.id => {
                let t = self.season.tier(s);
                (t.0 as u8, t.3.min(255) as u8, s.id as u8)
            }
            Some(s) => (0, 0, s.id as u8),
            None => (0, 0, 0),
        };
        let fav = (0..career::FIGHTERS).max_by_key(|&b| self.career.mastery[b]).unwrap_or(0) as u8;
        let v = self.ach.stat_values();
        let st = |name: &str| v.iter().find(|x| x.0 == name).map_or(0, |x| x.1.max(0) as u32);
        Card {
            name: clean_name(&self.settings.name, 0),
            banner: idx(&e.banner),
            title: idx(&e.title),
            nameplate: idx(&e.nameplate),
            showcase: [idx(&e.showcase[0]), idx(&e.showcase[1]), idx(&e.showcase[2])],
            season: sid,
            tier,
            prestige,
            rank: ranked::Rank::to_byte(self.ranked.rank()),
            fav,
            stats: [st("STAT_WINS"), st("STAT_KILLS"), st("STAT_HEADSHOTS"), st("STAT_PARRIES"), st("STAT_EXECUTIONS"), st("STAT_RAID_CLEARS")],
        }
    }

    fn my_border(&self) -> Option<[u8; 3]> {
        self.season_now().filter(|s| s.id == self.season.season).and_then(|s| self.season.border(s))
    }

    /// The Season page.
    pub(crate) fn season_page(&mut self, ui: &mut egui::Ui) -> Option<Page> {
        let mut next = None;
        let reg = season::reg();
        let day = season::day_now();
        let Some(s) = self.season_now() else {
            ui.heading("Seasons");
            ui.label("No season files were found.");
            if Self::big_button(ui, "Back") {
                next = Some(Page::Main);
            }
            return next;
        };
        let on = s.active(day);
        let now = season::secs_now();
        // (held to the panel's width, so the fighter beside it shows each reward)
        let full = ui.available_width().min(560.0);
        ui.set_max_width(full);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("SEASON {}", s.id)).font(ui_kit::display(16.0)).color(c32(s.colour)));
            ui.label(egui::RichText::new(&s.name).font(ui_kit::display(26.0)).color(Color32::WHITE));
        });
        let when = if on {
            format!("Ends in {}  ·  {} to {} (UTC)", season::span((s.end() as u64 * 86_400).saturating_sub(now)), season::date_of(s.start), season::date_of(s.end()))
        } else if day < s.start {
            format!("Starts in {}", season::span((s.start as u64 * 86_400).saturating_sub(now)))
        } else {
            "This season has ended: the next one begins soon.".into()
        };
        ui.label(egui::RichText::new(when).size(13.0).color(Color32::from_rgb(255, 205, 170)));
        let (tier, into, need, prestige) = if self.season.season == s.id { self.season.tier(s) } else { (0, 0, s.tier_xp, 0) };
        let head = if prestige > 0 { format!("Tier 50  ·  prestige {prestige}  ·  {into} / {need} XP to the next border") } else { format!("Tier {tier} / {}  ·  {into} / {need} XP to tier {}", season::TIERS, tier + 1) };
        ui.add(egui::ProgressBar::new(into as f32 / need.max(1) as f32).desired_height(10.0).text(egui::RichText::new(head).size(12.0)));
        if ui_kit::tabs(ui, &mut self.season_ui.tab, &["The track", "Challenges"]) {
            self.season_ui.sel = None;
        }
        ui.horizontal(|ui| {
            if ui.small_button("Patch notes").clicked() {
                next = Some(Page::PatchNotes);
            }
            if ui.small_button("About this season").clicked() {
                self.season_ui.start_open = true;
            }
        });
        if self.season_ui.tab == 1 {
            self.challenges_ui(ui, s, now);
        } else {
            self.track_ui(ui, s, tier);
        }
        for pb in reg.problems.iter().take(3) {
            ui.label(egui::RichText::new(format!("Season file: {pb}")).size(11.0).color(Color32::from_rgb(255, 150, 120)));
        }
        if Self::big_button(ui, "Back") {
            next = Some(Page::Main);
        }
        next
    }

    fn track_ui(&mut self, ui: &mut egui::Ui, s: &'static Season, tier: u32) {
        let reg = season::reg();
        let tw = 96.0;
        let th = 128.0;
        let scroll_to = std::mem::take(&mut self.season_ui.scroll_once);
        // (held to the panel's width: the track scrolls inside it)
        let w = ui.available_width().min(560.0);
        egui::ScrollArea::horizontal().id_salt("season-track").max_width(w).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (k, slot) in s.tiers.iter().enumerate() {
                    let t = k as u32 + 1;
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(tw, th), egui::Sense::click());
                    let p = ui.painter();
                    let owned_tier = t <= tier;
                    let cur = t == tier + 1;
                    let item = slot.and_then(|i| reg.items.get(i).map(|it| (i, it)));
                    let sel = item.is_some_and(|(i, _)| self.season_ui.sel == Some(i));
                    let bg = if owned_tier { Color32::from_rgba_unmultiplied(110, 40, 40, 150) } else { Color32::from_rgba_unmultiplied(255, 255, 255, 12) };
                    p.rect_filled(r, 8.0, bg);
                    if cur {
                        p.rect_stroke(r, 8.0, Stroke::new(2.0, c32(s.colour)), egui::StrokeKind::Inside);
                    }
                    if sel || resp.hovered() {
                        p.rect_stroke(r.shrink(2.0), 7.0, Stroke::new(2.0, if sel { ACCENT } else { Color32::from_white_alpha(80) }), egui::StrokeKind::Inside);
                    }
                    p.text(r.min + Vec2::new(8.0, 6.0), Align2::LEFT_TOP, t.to_string(), ui_kit::display(16.0), if owned_tier { Color32::from_rgb(255, 214, 170) } else { Color32::from_gray(150) });
                    if let Some((i, it)) = item {
                        let have = self.season.owned.contains(&it.key);
                        paint_reward(p, Rect::from_center_size(r.center() - Vec2::new(0.0, 6.0), Vec2::splat(54.0)), it, have);
                        let f = ui_kit::fitted(p, &it.name, FontId::proportional(10.5), tw - 8.0);
                        p.text(Pos2::new(r.center().x, r.bottom() - 18.0), Align2::CENTER_CENTER, &it.name, f, Color32::from_gray(if have { 225 } else { 150 }));
                        p.text(Pos2::new(r.center().x, r.bottom() - 6.0), Align2::CENTER_CENTER, it.reward.kind(), FontId::proportional(9.5), Color32::from_gray(130));
                        if self.season.fresh.contains(&it.key) {
                            p.text(Pos2::new(r.right() - 6.0, r.top() + 6.0), Align2::RIGHT_TOP, "NEW", ui_kit::display(11.0), ACCENT);
                        }
                        if resp.clicked() {
                            self.season_ui.sel = Some(i);
                            self.season.fresh.remove(&it.key);
                        }
                    }
                    if scroll_to && cur {
                        resp.scroll_to_me(Some(egui::Align::Center));
                    }
                }
            });
        });
        // the picked reward
        ui.add_space(4.0);
        let Some(i) = self.season_ui.sel.or_else(|| s.tiers.get(tier.min(season::TIERS - 1) as usize).copied().flatten()) else { return };
        let Some(it) = reg.items.get(i) else { return };
        let have = self.season.owned.contains(&it.key);
        let at = s.tiers.iter().position(|t| *t == Some(i)).map(|k| format!("tier {}", k + 1)).or_else(|| s.ranks.iter().find(|r| r.1 == i).map(|r| format!("season-end reward for {}", crate::ranked::TIERS[r.0 as usize].0))).unwrap_or_else(|| "a season challenge".into());
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(64.0), egui::Sense::hover());
            paint_reward(ui.painter(), r, it, have);
            let room = ui.available_width();
            ui.vertical(|ui| {
                ui.set_max_width(room);
                ui.label(egui::RichText::new(&it.name).font(ui_kit::display(20.0)).color(Color32::WHITE));
                ui.label(egui::RichText::new(format!("{}  ·  {at}  ·  {}", it.reward.kind(), if have { "earned" } else { "not earned yet" })).size(12.5).color(Color32::from_gray(185)));
                let how = match &it.reward {
                    Reward::Outfit { .. } => "A coat or cape colour for any fighter: everyone sees it in every match. Shown on your fighter.",
                    Reward::Dye(_) => "A dye for Varrow's wardrobe. Shown on a herald's outfit on your fighter.",
                    Reward::Finish { .. } => "A finish on every weapon you hold: everyone sees it. Shown on your fighter's blade.",
                    Reward::Emote(_) => "A new emote for Varrow Centre's emote wheel. Your fighter shows it.",
                    Reward::Title => "Words under your name on your profile card and over your head in Varrow Centre.",
                    Reward::Banner { .. } => "The top of your profile card.",
                    Reward::Nameplate { .. } => "Behind your name on your profile card and in Varrow Centre.",
                    Reward::Pet { .. } => "Follows you round Varrow Centre. Shown beside your fighter.",
                };
                ui.label(egui::RichText::new(how).size(12.0).color(Color32::from_gray(165)));
                if have {
                    ui.horizontal(|ui| {
                        let wearable = !matches!(it.reward, Reward::Dye(_) | Reward::Emote(_));
                        let worn = [&self.season.equip.outfit, &self.season.equip.finish, &self.season.equip.title, &self.season.equip.banner, &self.season.equip.nameplate, &self.season.equip.pet].iter().any(|e| e.as_deref() == Some(it.key.as_str()));
                        if wearable && ui.button(if worn { "Take off" } else { "Wear" }).clicked() {
                            self.season.wear(season::reg(), &it.key);
                            self.season_wear();
                            self.rebuild_showcase();
                            self.season_ui.preview_on = false;
                        }
                        let shown = self.season.equip.showcase.iter().any(|e| e.as_deref() == Some(it.key.as_str()));
                        if ui.button(if shown { "Take off the card" } else { "Show on my card" }).clicked() {
                            self.season.showcase(&it.key);
                        }
                    });
                }
            });
        });
        // a card's preview for the card's own rewards
        if matches!(it.reward, Reward::Title | Reward::Banner { .. } | Reward::Nameplate { .. }) {
            let mut c = self.my_card();
            match it.reward {
                Reward::Title => c.title = i as u16,
                Reward::Banner { .. } => c.banner = i as u16,
                _ => c.nameplate = i as u16,
            }
            let w = ui.available_width().min(400.0);
            let (r, _) = ui.allocate_exact_size(Vec2::new(w, w * 300.0 / 520.0), egui::Sense::hover());
            paint_card(ui.painter(), r, &c, self.my_border());
        }
        // season-end rewards and prestige
        ui.label(egui::RichText::new(format!("After tier 50: prestige every {} XP, each a new border for your card. Season-end rewards go by your peak Ranked 1v1 rank (best this season: {}).", s.prestige_xp, if self.season.peak == 0 { "unranked".to_string() } else { crate::ranked::TIERS[(self.season.peak - 1).min(7) as usize].0.to_string() })).size(11.5).color(Color32::from_gray(160)));
        self.season_ui.sel = Some(i);
    }

    fn challenges_ui(&mut self, ui: &mut egui::Ui, s: &'static Season, now: u64) {
        let (daily_in, weekly_in) = season::resets_in(now);
        let day = season::day_now();
        let can_reroll = self.season.reroll_day != day;
        let row = |ui: &mut egui::Ui, text: &str, prog: u32, goal: u32, done: bool, xp: u32| {
            egui::Frame::new().fill(Color32::from_rgba_unmultiplied(255, 255, 255, if done { 16 } else { 8 })).corner_radius(5.0).inner_margin(6.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(text).size(14.0).color(if done { Color32::from_rgb(150, 225, 160) } else { Color32::WHITE }));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("+{xp} XP")).size(12.0).color(ACCENT));
                    });
                });
                if done {
                    ui.label(egui::RichText::new("Done").size(11.5).color(Color32::from_rgb(150, 225, 160)));
                } else {
                    ui.add(egui::ProgressBar::new(prog as f32 / goal.max(1) as f32).desired_height(8.0).text(egui::RichText::new(format!("{prog} / {goal}")).size(10.0)));
                }
            });
        };
        if self.ach_gated() {
            ui.label(egui::RichText::new("This session used admin commands: challenges and the track don't move until the game is restarted.").size(12.5).color(Color32::from_rgb(255, 150, 120)));
        }
        let h = (ui.ctx().content_rect().height() - 420.0).max(200.0);
        let mut reroll = None;
        egui::ScrollArea::vertical().max_height(h).show(ui, |ui| {
            ui_kit::section(ui, &format!("Daily  ·  new in {}", season::span(daily_in)));
            for (k, sl) in self.season.daily.clone().iter().enumerate() {
                let (w, goal) = season::DAILY[sl.idx as usize % season::DAILY.len()];
                row(ui, &season::describe(w, goal), sl.prog, goal, sl.done, season::DAILY_XP);
                if can_reroll && !sl.done && ui.small_button("Reroll (one free a day)").clicked() {
                    reroll = Some(k);
                }
            }
            ui_kit::section(ui, &format!("Weekly  ·  new in {}", season::span(weekly_in)));
            for sl in &self.season.weekly {
                let (w, goal) = season::WEEKLY[sl.idx as usize % season::WEEKLY.len()];
                row(ui, &season::describe(w, goal), sl.prog, goal, sl.done, season::WEEKLY_XP);
            }
            ui_kit::section(ui, "Season challenges");
            for sl in &self.season.sprog {
                if let Some(c) = s.challenges.get(sl.idx as usize) {
                    let text = match c.reward.and_then(|i| season::reg().items.get(i)) {
                        Some(it) => format!("{}  ·  and {}: {}", c.text, it.reward.kind().to_lowercase(), it.name),
                        None => c.text.clone(),
                    };
                    row(ui, &text, sl.prog, c.goal, sl.done, c.xp);
                }
            }
        });
        if let Some(k) = reroll {
            self.season.reroll(k, day);
        }
    }

    /// The Profile page: your card and what's on it, and the match history.
    pub(crate) fn profile_page(&mut self, ui: &mut egui::Ui) -> Option<Page> {
        let mut next = None;
        ui.heading("Profile");
        ui_kit::tabs(ui, &mut self.season_ui.profile_tab, &["Card", "Match history"]);
        if self.season_ui.profile_tab == 0 {
            let card = self.my_card();
            let w = ui.available_width().min(540.0);
            let (r, _) = ui.allocate_exact_size(Vec2::new(w, w * 300.0 / 520.0), egui::Sense::hover());
            paint_card(ui.painter(), r, &card, self.my_border());
            ui.label(egui::RichText::new("Others see this card when they inspect you in Varrow Centre (right-click a player, then View profile card).").size(12.0).color(Color32::from_gray(170)));
            // what's on it: picked from what the seasons gave
            let reg = season::reg();
            let owned: Vec<(usize, &Item)> = reg.items.iter().enumerate().filter(|(_, it)| self.season.owned.contains(&it.key)).collect();
            let pick = |ui: &mut egui::Ui, label: &str, want: fn(&Reward) -> bool, cur: Option<String>| -> Option<Option<String>> {
                let mut out = None;
                let name = cur.as_deref().and_then(|k| reg.item(k)).map_or("None".to_string(), |(_, it)| it.name.clone());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(label).size(13.5));
                    egui::ComboBox::from_id_salt(("profile", label)).selected_text(name).width(240.0).show_ui(ui, |ui| {
                        if ui.selectable_label(cur.is_none(), "None").clicked() {
                            out = Some(None);
                        }
                        for (_, it) in owned.iter().filter(|(_, it)| want(&it.reward)) {
                            if ui.selectable_label(cur.as_deref() == Some(it.key.as_str()), &it.name).clicked() {
                                out = Some(Some(it.key.clone()));
                            }
                        }
                    });
                });
                out
            };
            if let Some(v) = pick(ui, "Banner", |r| matches!(r, Reward::Banner { .. }), self.season.equip.banner.clone()) {
                self.season.equip.banner = v;
                self.season.dirty = true;
            }
            if let Some(v) = pick(ui, "Title", |r| matches!(r, Reward::Title), self.season.equip.title.clone()) {
                self.season.equip.title = v;
                self.season.dirty = true;
            }
            if let Some(v) = pick(ui, "Nameplate", |r| matches!(r, Reward::Nameplate { .. }), self.season.equip.nameplate.clone()) {
                self.season.equip.nameplate = v;
                self.season.dirty = true;
            }
            for k in 0..3 {
                if let Some(v) = pick(ui, &format!("Showcase {}", k + 1), |_| true, self.season.equip.showcase[k].clone()) {
                    self.season.equip.showcase[k] = v;
                    self.season.dirty = true;
                }
            }
            if owned.is_empty() {
                ui.label(egui::RichText::new("Season rewards you earn can go on your card: play to climb the season's track.").size(12.0).color(Color32::from_gray(170)));
            }
        } else {
            let h = (ui.ctx().content_rect().height() - 360.0).max(200.0);
            if self.season.history.is_empty() {
                ui.label("No matches yet: your last 40 show here.");
            }
            egui::ScrollArea::vertical().max_height(h).show(ui, |ui| {
                egui::Grid::new("history").num_columns(5).striped(true).spacing([14.0, 5.0]).show(ui, |ui| {
                    for t in ["When", "Match", "Result", "K / D", "XP"] {
                        ui.label(egui::RichText::new(t).size(12.0).color(Color32::from_gray(150)));
                    }
                    ui.end_row();
                    let now = season::secs_now();
                    for m in &self.season.history {
                        let ago = now.saturating_sub(m.when);
                        ui.label(egui::RichText::new(if ago < 3600 { format!("{}m ago", ago / 60) } else if ago < 86_400 { format!("{}h ago", ago / 3600) } else { format!("{}d ago", ago / 86_400) }).size(12.5));
                        ui.label(egui::RichText::new(format!("{}{} · {}", if m.ranked { "Ranked " } else { "" }, m.mode, m.map)).size(12.5));
                        ui.label(egui::RichText::new(if m.won { "Win" } else { "Loss" }).size(12.5).color(if m.won { Color32::from_rgb(150, 225, 160) } else { Color32::from_rgb(235, 140, 120) }));
                        ui.label(egui::RichText::new(format!("{} / {}", m.kills, m.deaths)).size(12.5));
                        ui.label(egui::RichText::new(format!("+{}", m.xp)).size(12.5).color(ACCENT));
                        ui.end_row();
                    }
                });
            });
        }
        if Self::big_button(ui, "Back") {
            next = Some(Page::Main);
        }
        next
    }

    /// The patch notes page (patch_notes.txt).
    pub(crate) fn patch_notes_page(&mut self, ui: &mut egui::Ui) -> Option<Page> {
        let mut next = None;
        let text = self.season_ui.patch.get_or_insert_with(load_patch_notes).clone();
        ui.heading("Patch notes");
        let h = (ui.ctx().content_rect().height() - 330.0).max(200.0);
        egui::ScrollArea::vertical().max_height(h).show(ui, |ui| {
            for line in text.lines() {
                let l = line.trim_end();
                if l.starts_with("# ") || l.is_empty() && false {
                    continue;
                } else if let Some(t) = l.strip_prefix("## ") {
                    ui_kit::section(ui, t);
                } else if let Some(t) = l.strip_prefix("- ") {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new("•").color(ACCENT));
                        ui.label(egui::RichText::new(t).size(13.5));
                    });
                } else if !l.starts_with('#') && !l.is_empty() {
                    ui.label(egui::RichText::new(l).size(13.5));
                }
            }
        });
        if Self::big_button(ui, "Back") {
            next = Some(Page::Main);
        }
        next
    }

    /// The menu's fighter shows the reward picked on the track (and goes back to normal after).
    pub(crate) fn season_preview(&mut self, page: Page) {
        let show = page == Page::Season && self.season_ui.tab == 0 && !self.season_ui.start_open;
        if !show {
            if self.season_ui.preview_on {
                self.season_ui.preview_on = false;
                self.rebuild_showcase();
            }
            return;
        }
        let reg = season::reg();
        let Some(it) = self.season_ui.sel.and_then(|i| reg.items.get(i).map(|it| (i, it))) else { return };
        self.season_ui.preview_on = true;
        let t = self.demo.time;
        let base = self.settings.loadout;
        let f = &mut self.demo.f[0];
        f.loadout.outfit = base.outfit;
        f.loadout.finish = base.finish;
        f.hub.look = crate::hub::Outfit::default();
        let (i, item) = it;
        match &item.reward {
            Reward::Outfit { .. } => f.loadout.outfit = season::outfit_number(i).unwrap_or(base.outfit),
            Reward::Finish { .. } => f.loadout.finish = season::finish_number(i).unwrap_or(0),
            Reward::Pet { .. } => f.hub.look.pet = season::pet_value(i).unwrap_or(0),
            Reward::Dye(k) => {
                let mut o = crate::hub::Outfit::default().with_set(crate::hub::LOOKS[8].set);
                for d in o.dyes.iter_mut() {
                    *d = [*k, *k, crate::gear::DEFAULT_DYES[2]];
                }
                f.hub.look = o;
            }
            Reward::Emote(e) => {
                let len = e.length().unwrap_or(3.0);
                if f.hub.emote != *e as u8 || t - f.hub.emote_t > len + 0.9 {
                    f.hub.emote = *e as u8;
                    f.hub.emote_t = t;
                }
                return;
            }
            _ => {}
        }
        f.hub.emote = 0;
    }

    /// Over everything: the season start screen (in the menus) and a player's card (in the hub).
    pub(crate) fn season_overlays(&mut self, ctx: &egui::Context, rect: Rect) {
        if self.season_ui.start_open && matches!(self.screen, Screen::Menu(_)) {
            self.start_screen(ctx, rect);
        }
        if self.season_ui.view.is_none() {
            self.season_ui.view_frames = 0;
        }
        if let Some(slot) = self.season_ui.view {
            self.season_ui.view_frames += 1;
            let Screen::Play { game, .. } = &self.screen else {
                self.season_ui.view = None;
                return;
            };
            let card = game.hub.as_ref().and_then(|h| h.cards.get(&slot)).and_then(|b| Card::from_bytes(b)).or_else(|| (slot == game.local).then(|| self.my_card()));
            let name = game.f.get(slot).map_or(String::new(), |f| f.name.clone());
            let card = card.unwrap_or(Card { name, banner: season::NONE, title: season::NONE, nameplate: season::NONE, showcase: [season::NONE; 3], ..Default::default() });
            let reg = season::reg();
            let border = reg.season(card.season as u32).filter(|_| card.prestige > 0).map(|s| s.prestige[(card.prestige as usize - 1).min(s.prestige.len() - 1)]);
            let w = (rect.width() * 0.42).clamp(420.0, 620.0);
            let r = Rect::from_center_size(rect.center(), Vec2::new(w, w * 300.0 / 520.0));
            let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("card-view")));
            p.rect_filled(rect, 0.0, Color32::from_black_alpha(120));
            paint_card(&p, r, &card, border);
            p.text(Pos2::new(r.center().x, r.bottom() + 22.0), Align2::CENTER_CENTER, "Esc or right-click to close", FontId::proportional(14.0), Color32::from_gray(200));
            if self.season_ui.view_frames > 2 && ctx.input(|i| i.key_pressed(Key::Escape) || i.pointer.secondary_clicked() || i.pointer.primary_clicked()) {
                self.season_ui.view = None;
                self.esc_eaten = true;
            }
        }
    }

    /// The season start screen ("what's new"): once per season.
    fn start_screen(&mut self, ctx: &egui::Context, rect: Rect) {
        let Some(s) = self.season_now() else {
            self.season_ui.start_open = false;
            return;
        };
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("season-start-bg")));
        ui_kit::vgrad(&p, rect, Color32::from_rgba_unmultiplied(20, 4, 8, 242), Color32::from_rgba_unmultiplied(6, 4, 10, 250));
        // the red moon
        let mc = Pos2::new(rect.right() - rect.width() * 0.2, rect.top() + rect.height() * 0.3);
        let mr = rect.height() * 0.15;
        let t = self.wall_t;
        for k in (1..8).rev() {
            p.circle_filled(mc, mr * (1.0 + k as f32 * 0.12), c32a(s.colour, (7.0 + 2.0 * (t * 0.8).sin()) as u8 + 4));
        }
        p.circle_filled(mc, mr, c32(s.colour));
        p.circle_filled(mc + Vec2::new(mr * 0.18, -mr * 0.12), mr * 0.85, c32a([255, 120, 100], 40));
        for (dx, dy, cr) in [(-0.3f32, -0.2f32, 0.16f32), (0.25, 0.3, 0.12), (0.05, -0.45, 0.08), (-0.4, 0.35, 0.1), (0.42, -0.1, 0.07)] {
            p.circle_filled(mc + Vec2::new(dx, dy) * mr, mr * cr, c32a([90, 10, 18], 90));
        }
        // the Regent's silhouette on the vault's roof below it
        let ground = rect.bottom() - rect.height() * 0.16;
        ui_kit::vgrad(&p, Rect::from_min_max(Pos2::new(rect.left(), ground), rect.max), Color32::from_rgb(10, 4, 8), Color32::from_rgb(4, 2, 4));
        // the vault roof's parapet
        for k in 0..14 {
            let x = rect.left() + rect.width() * (0.42 + k as f32 * 0.045);
            p.rect_filled(Rect::from_min_size(Pos2::new(x, ground - 14.0), Vec2::new(rect.width() * 0.03, 14.0)), 0.0, Color32::from_rgb(10, 4, 8));
        }
        let fx = mc.x - mr * 0.2;
        let body = vec![Pos2::new(fx - 18.0, ground), Pos2::new(fx - 6.0, ground - 120.0), Pos2::new(fx + 6.0, ground - 120.0), Pos2::new(fx + 30.0, ground)];
        p.add(egui::Shape::convex_polygon(body, Color32::from_rgb(8, 4, 8), Stroke::NONE));
        p.circle_filled(Pos2::new(fx, ground - 132.0), 13.0, Color32::from_rgb(8, 4, 8));
        p.line_segment([Pos2::new(fx + 8.0, ground - 100.0), Pos2::new(fx + 70.0, ground - 190.0)], Stroke::new(4.0, Color32::from_rgb(8, 4, 8)));
        let mut close = false;
        let mut notes = false;
        egui::Area::new(egui::Id::unique("season-start")).fixed_pos(Pos2::new(rect.left() + 70.0, rect.top() + 70.0)).order(egui::Order::Tooltip).show(ctx, |ui| {
            ui.set_width((rect.width() * 0.48).clamp(380.0, 640.0));
            ui.label(egui::RichText::new(format!("SEASON {}", s.id)).font(ui_kit::display(22.0)).color(c32(s.colour)));
            ui.label(egui::RichText::new(s.name.to_uppercase()).font(ui_kit::display(64.0)).color(Color32::WHITE));
            ui.label(egui::RichText::new(format!("{} to {}  ·  50 free tiers  ·  ends in {}", season::date_of(s.start), season::date_of(s.end()), season::span((s.end() as u64 * 86_400).saturating_sub(season::secs_now())))).size(15.0).color(Color32::from_rgb(255, 200, 170)));
            ui.add_space(10.0);
            for para in s.theme.lines() {
                ui.label(egui::RichText::new(para).size(15.5).color(Color32::from_rgb(235, 225, 220)));
                ui.add_space(4.0);
            }
            ui.add_space(8.0);
            ui_kit::section(ui, "What's new");
            let pn = self.season_ui.patch.get_or_insert_with(load_patch_notes).clone();
            for l in pn.lines().filter_map(|l| l.strip_prefix("- ")).take(4) {
                let short = if l.len() > 150 { format!("{}...", &l[..l[..147].rfind(' ').unwrap_or(147)]) } else { l.to_string() };
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("•").color(ACCENT));
                    ui.label(egui::RichText::new(short).size(13.5).color(Color32::from_gray(215)));
                });
            }
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let b = egui::Button::new(egui::RichText::new("ENTER THE SEASON").font(ui_kit::display(20.0)).color(Color32::from_rgb(25, 10, 10))).fill(c32(s.colour)).min_size(Vec2::new(240.0, 46.0));
                if ui.add(b).clicked() {
                    close = true;
                }
                if ui.add(egui::Button::new(egui::RichText::new("Patch notes").size(15.0)).min_size(Vec2::new(130.0, 46.0))).clicked() {
                    close = true;
                    notes = true;
                }
            });
        });
        if close || ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.season_ui.start_open = false;
            self.season.seen = s.id;
            self.season.dirty = true;
            self.esc_eaten = true;
            UI_SOUND.with(|u| u.set(Some(Sfx::UiConfirm)));
            self.screen = Screen::Menu(if notes { Page::PatchNotes } else { Page::Season });
            self.season_ui.scroll_once = true;
        }
    }

    /// Picture scenes (DUEL_SEASON_SCENE=track | challenges | profile | start | preview:<tier>).
    pub(crate) fn season_scene(&mut self, scene: &str) {
        let reg = season::reg();
        let day = season::day_now();
        let Some(s) = reg.current(day) else { return };
        let mut p = season::Progress::new(424_242);
        let d = if s.active(day) { day } else { s.start + 3 };
        p.roll(reg, d);
        p.add_xp(reg, d, s.tier_xp * 23 + s.tier_xp * 2 / 5);
        let sable = season::Ctx { body: 1, duel: true, ranked: false };
        for _ in 0..9 {
            p.feed(reg, d, CareerEv::Parry, sable, 0, false);
        }
        for _ in 0..6 {
            p.feed(reg, d, CareerEv::Kill { head: true, finisher: 0 }, sable, 0, false);
        }
        p.feed(reg, d, CareerEv::MatchWon, sable, 0, false);
        p.boss_down(reg, d, 1, false);
        p.notes.clear();
        p.equip.banner = Some("s1.t4".into());
        p.equip.title = Some("s1.t14".into());
        p.equip.nameplate = Some("s1.t2".into());
        p.equip.showcase = [Some("s1.t7".into()), Some("s1.t16".into()), Some("s1.t13".into())];
        p.equip.finish = Some("s1.t10".into());
        let now = season::secs_now();
        let rows = [("Duel", "Sunfall Keep", true, 7, 3, 340, true), ("Free-for-All", "The Undercroft", false, 12, 9, 290, false), ("Duel", "Old Marble Court", true, 6, 2, 310, true), ("Team Deathmatch", "Varrow Bridgeworks", true, 15, 8, 360, false), ("Capture the Flag", "Varrow Bridgeworks", false, 4, 6, 220, false)];
        for (k, r) in rows.iter().enumerate().rev() {
            p.played(season::Played { when: now - k as u64 * 5400 - 600, mode: r.0.into(), map: r.1.into(), won: r.2, kills: r.3, deaths: r.4, xp: r.5, ranked: r.6 });
        }
        p.fresh.clear();
        p.dirty = false;
        p.publish(reg);
        self.season = p;
        self.season_ui.start_open = false;
        self.season_wear();
        match scene {
            "track" => {
                self.screen = Screen::Menu(Page::Season);
                self.season_ui.sel = s.tiers[6];
                self.season_ui.scroll_once = true;
            }
            sc if sc.starts_with("preview:") => {
                let t: usize = sc[8..].parse().unwrap_or(7);
                self.screen = Screen::Menu(Page::Season);
                self.season_ui.sel = s.tiers.get(t.clamp(1, 50) - 1).copied().flatten();
                self.season_ui.scroll_once = true;
            }
            "challenges" => {
                self.screen = Screen::Menu(Page::Season);
                self.season_ui.tab = 1;
            }
            "profile" => self.screen = Screen::Menu(Page::Profile),
            "history" => {
                self.screen = Screen::Menu(Page::Profile);
                self.season_ui.profile_tab = 1;
            }
            "start" => {
                self.screen = Screen::Menu(Page::Main);
                self.season_ui.start_open = true;
            }
            _ => {}
        }
    }
}
