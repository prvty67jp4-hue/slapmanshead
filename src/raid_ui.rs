//! The Ashen Vault on screen (crate::raid has the rules): the Vault Gate's panel (go in, Sealed
//! Writ once you've cleared it, bots to fill the party), the leaderboard by the gate, leaving, and
//! the raid's HUD: the boss bars, the room and total clocks, revives left, the party, what the
//! marked player must do, the spikes' cue, the chest and its loot, the wipe and the results with
//! the death recap.

use super::*;
use crate::raid::{self, Cause, FoeKind, LootKind, Stage, LOOT, NO_MOVE};
use crate::vault::{self, ROOMS};

fn flat(a: V3, b: V3) -> f32 {
    (a - b).flat().len()
}

/// Whether the local player stands in the Vault Gate's circle.
pub fn at_gate(g: &Game) -> bool {
    let (x, z, r) = vault::GATE;
    let p = g.f[g.local].pos;
    g.hub.is_some() && !g.in_raid(g.local) && flat(p, v3(x, 0.0, z)) < r && p.y > -5.0
}

pub fn at_board(g: &Game) -> bool {
    let (x, _, z) = vault::GATE_BOARD;
    !g.in_raid(g.local) && flat(g.f[g.local].pos, v3(x, 0.0, z)) < 3.0
}

/// What E does here (None: nothing of the vault's).
pub fn prompt(g: &Game) -> Option<String> {
    let me = g.local;
    if let Some(r) = g.my_raid() {
        if r.is_downed(me) {
            return Some("You're down: an ally can hold E on you  ·  Esc leave the vault".into());
        }
        let p = g.f[me].pos;
        if let Some(m) = r.members.iter().find(|m| m.downed && (m.slot as usize) < g.f.len() && flat(g.f[m.slot as usize].pos, p) < raid::REVIVE_RANGE) {
            return Some(format!("Hold E to revive {}  ·  {} revives left", m.name, r.revives_left));
        }
        if let Some(c) = r.chest.as_ref().filter(|c| !c.open && flat(c.pos, p) < 3.0) {
            return Some(if c.rare { "E open the chest (something glows inside)".into() } else { "E open the chest".into() });
        }
        return Some("Esc leave the vault".into());
    }
    if at_gate(g) {
        let n = g.f.iter().filter(|f| f.present && !f.is_bot && flat(f.pos, v3(vault::GATE.0, 0.0, vault::GATE.1)) < vault::GATE.2).count();
        return Some(if g.raid.is_some() { "THE VAULT GATE  ·  a party is inside".into() } else { format!("THE VAULT GATE  ·  E to go in ({n} in the circle)") });
    }
    if at_board(g) {
        return Some("E the vault's leaderboard".into());
    }
    None
}

/// The gate's panel.
pub fn gate_panel(ui: &mut egui::Ui, g: &mut Game, bots: &mut usize, close: &mut bool) {
    let me = g.local;
    ui.label(egui::RichText::new("THE ASHEN VAULT").size(22.0).color(ACCENT).strong());
    ui.label("Five rooms under the city, a boss in each: the Warden of Wax, the Twin Assessors, the Chained Ledger, Lord Vesper Kael on the roof, and the Ashen Magistrate. 2 to 4 of you; everyone standing in the circle goes in together. A wipe starts the room again; three revives a room (hold E on a downed ally).");
    ui.add_space(6.0);
    let (x, z, r) = vault::GATE;
    let party: Vec<String> = g.f.iter().filter(|f| f.present && !f.is_bot && flat(f.pos, v3(x, 0.0, z)) < r).map(|f| f.name.clone()).collect();
    ui.label(egui::RichText::new(format!("In the circle: {}", party.join(", "))).strong());
    let room = 4usize.saturating_sub(party.len().min(4));
    *bots = (*bots).min(room);
    if party.len() == 1 && *bots == 0 {
        *bots = 1;
    }
    ui.horizontal(|ui| {
        ui.label("Bots to fill the party:");
        for k in 0..=room {
            if ui.selectable_label(*bots == k, format!("{k}")).clicked() && !(party.len() == 1 && k == 0) {
                *bots = k;
            }
        }
    });
    let earned = g.hub.as_ref().map(|h| h.earned.raid).unwrap_or_default();
    let busy = g.raid.is_some();
    ui.add_space(6.0);
    let mut go: Option<bool> = None;
    ui.horizontal(|ui| {
        if ui.add_enabled(!busy, egui::Button::new(egui::RichText::new("Enter the Vault").strong())).clicked() {
            go = Some(false);
        }
        let writ = egui::Button::new(egui::RichText::new("SEALED WRIT (hard)").color(Color32::from_rgb(255, 120, 90)));
        let resp = ui.add_enabled(!busy && earned.cleared, writ);
        if resp.clicked() {
            go = Some(true);
        }
        if !earned.cleared {
            resp.on_disabled_hover_text("Clear the vault once to unseal the writ");
        }
    });
    if !earned.cleared {
        ui.label(egui::RichText::new("Sealed Writ: clear the vault once to unlock it (harder moves, fewer revives, an extra move per boss).").small().color(Color32::from_gray(170)));
    }
    if busy {
        ui.label(egui::RichText::new("A party is in the vault right now.").color(Color32::from_gray(170)));
    }
    if let Some(hard) = go {
        if g.is_authority() {
            raid::gate_start(g, me, hard, *bots);
        } else {
            raid::raid_msg(g, &raid::RaidMsg::Gate { hard, bots: *bots as u8 });
        }
        *close = true;
    }
}

/// The leaderboard by the gate.
pub fn board_panel(ui: &mut egui::Ui, g: &Game) {
    let Some(h) = g.hub.as_ref() else { return };
    ui.label(egui::RichText::new("THE VAULT'S FASTEST").size(22.0).color(ACCENT).strong());
    for k in 0..3usize {
        ui.label(egui::RichText::new(format!("Parties of {}", k + 2)).strong());
        let rows = &h.boards[(crate::hub::BOARD_RAID as usize) + k];
        if rows.is_empty() {
            ui.label(egui::RichText::new("no clears yet").small().color(Color32::from_gray(160)));
        }
        for (n, (who, ms)) in rows.iter().take(5).enumerate() {
            ui.label(format!("{}. {}  {}", n + 1, raid::fmt_time(*ms as f32 / 1000.0), who));
        }
        let best = h.earned.raid.best[k];
        if best > 0 {
            ui.label(egui::RichText::new(format!("your best: {}", raid::fmt_time(best as f32 / 1000.0))).small().color(ACCENT));
        }
    }
    ui.add_space(6.0);
    let e = h.earned.raid;
    let have = (0..LOOT.len() as u8).filter(|&l| e.has(l)).count();
    ui.label(egui::RichText::new(format!("Your vault finds: {have} of {}", LOOT.len())).strong());
    for (k, l) in LOOT.iter().enumerate() {
        if e.has(k as u8) {
            let c = if l.rare { Color32::from_rgb(200, 140, 255) } else { Color32::from_rgb(230, 220, 200) };
            ui.label(egui::RichText::new(format!("· {}", l.name)).small().color(c));
        }
    }
}

/// What the marked player has to do for the move coming.
fn regent_hint(r: &raid::Raid) -> Option<&'static str> {
    let rg = r.regent;
    match (rg.mode, rg.mv) {
        (raid::RM_JABS, _) => Some("JAB CHAIN  ·  PARRY EVERY JAB (or dodge)"),
        (raid::RM_STEPS, _) => Some("HE'S COMING  ·  when the footsteps stop, run 3 m out of his path"),
        (raid::RM_SPELL, _) => Some("HIS SPELL  ·  BLOCK IT"),
        (_, raid::MV_JAB) => Some("RED FLASH JAB  ·  STEP BACK"),
        (_, raid::MV_RING) => Some("BLOOD RING  ·  DASH INTO HIM"),
        (_, raid::MV_THRUST) => Some("LANCE THRUST  ·  SIDESTEP LEFT OR RIGHT"),
        (_, raid::MV_ARC) => Some("SWEEPING ARC  ·  GET BEHIND HIM"),
        (_, raid::MV_CHARGE) => Some("CHARGE  ·  OUT OF HIS PATH"),
        _ => None,
    }
}

fn bar(p: &egui::Painter, r: Rect, k: f32, col: Color32) {
    p.rect_filled(r, 3.0, Color32::from_black_alpha(170));
    p.rect_filled(Rect::from_min_size(r.min, Vec2::new(r.width() * k.clamp(0.0, 1.0), r.height())), 3.0, col);
    p.rect_stroke(r, 3.0, Stroke::new(1.0, Color32::from_white_alpha(60)), egui::StrokeKind::Inside);
}

/// The raid's HUD.
pub fn hud(p: &egui::Painter, rect: Rect, g: &Game) {
    let Some(r) = g.my_raid() else { return };
    let s = (rect.height() / 900.0).clamp(0.75, 1.6);
    let me = g.local;
    let mid = rect.center().x;
    let white = Color32::from_rgb(240, 235, 225);
    // ---- the boss bars
    let bosses: Vec<(usize, &raid::Foe)> = r.foes.iter().enumerate().filter(|(_, f)| !matches!(f.kind, FoeKind::Thrall | FoeKind::Rift)).collect();
    let mut y = rect.top() + 26.0 * s;
    if matches!(r.stage, Stage::Fight | Stage::Intro | Stage::Wipe) {
        for (k, f) in &bosses {
            let w = 520.0 * s;
            let br = Rect::from_min_size(Pos2::new(mid - w * 0.5, y + 20.0 * s), Vec2::new(w, 14.0 * s));
            let col = if f.kind == FoeKind::Regent { Color32::from_rgb(200, 24, 36) } else { Color32::from_rgb(220, 120, 40) };
            text_shadow(p, Pos2::new(mid, y + 8.0 * s), Align2::CENTER_CENTER, f.kind.name(), 17.0 * s, white);
            bar(p, br, f.hp / f.max.max(1.0), if f.alive { col } else { Color32::from_gray(70) });
            let tag = match raid::vulnerable(r, *k) {
                Err(t) if !t.is_empty() && f.alive => Some(t.to_string()),
                _ if f.kind == FoeKind::Regent && r.regent.mode == raid::RM_STAGGER => Some(format!("STAGGERED  ·  x2 DAMAGE  {:.0}s", r.regent.t.max(0.0))),
                _ => None,
            };
            if let Some(t) = tag {
                text_shadow(p, Pos2::new(br.right() + 8.0 * s, br.center().y), Align2::LEFT_CENTER, &t, 13.0 * s, Color32::from_rgb(255, 220, 160));
            }
            text_shadow(p, br.center(), Align2::CENTER_CENTER, &format!("{:.0}%", f.hp / f.max.max(1.0) * 100.0), 11.0 * s, white);
            y += 44.0 * s;
        }
    }
    // ---- the room, the clocks, revives
    let room = ROOMS[r.room.min(4)];
    let line = format!(
        "{}{}  ·  room {}/5  ·  {}  ·  total {}  ·  revives {}",
        room.name,
        if r.hard { "  ·  SEALED WRIT" } else { "" },
        r.room + 1,
        raid::fmt_time(r.room_time),
        raid::fmt_time(r.total_time),
        r.revives_left
    );
    text_shadow(p, Pos2::new(mid, y + 6.0 * s), Align2::CENTER_CENTER, &line, 14.0 * s, Color32::from_rgb(220, 205, 185));
    // ---- the party
    let x0 = rect.left() + 22.0 * s;
    let mut py = rect.top() + rect.height() * 0.42;
    for m in &r.members {
        let sl = m.slot as usize;
        let hp = g.f.get(sl).map_or(0.0, |f| f.hp / MAX_HP);
        let name = format!("{}{}{}", m.name, if m.bot { " (bot)" } else { "" }, if !m.connected { " (reconnecting)" } else { "" });
        let col = if m.downed { Color32::from_rgb(140, 200, 255) } else if sl == me { ACCENT } else { white };
        text_shadow(p, Pos2::new(x0, py), Align2::LEFT_CENTER, &name, 14.0 * s, col);
        let br = Rect::from_min_size(Pos2::new(x0, py + 10.0 * s), Vec2::new(170.0 * s, 7.0 * s));
        if m.downed {
            bar(p, br, m.revive / raid::REVIVE_TIME, Color32::from_rgb(120, 200, 255));
            text_shadow(p, Pos2::new(br.right() + 6.0 * s, br.center().y), Align2::LEFT_CENTER, "DOWN", 11.0 * s, Color32::from_rgb(140, 200, 255));
        } else {
            bar(p, br, hp, Color32::from_rgb(90, 200, 110));
        }
        if r.regent.mark == m.slot && r.room == 3 && (r.regent.mv != NO_MOVE || matches!(r.regent.mode, raid::RM_JABS | raid::RM_STEPS | raid::RM_SPELL)) {
            text_shadow(p, Pos2::new(x0 - 14.0 * s, py), Align2::CENTER_CENTER, "◆", 14.0 * s, Color32::from_rgb(255, 40, 50));
        }
        py += 34.0 * s;
    }
    // ---- what you must do now
    let mut big: Option<(String, Color32)> = None;
    if r.stage == Stage::Fight && r.room == 3 {
        let rg = r.regent;
        if rg.mode == raid::RM_SPIKES && rg.spikes & (1 << me) != 0 {
            let cue = rg.spike_t <= 0.9;
            big = Some((
                if cue { "BLOCK NOW!".into() } else { format!("GIANT SPIKE ON YOU  ·  blade out, block on the flash (or hide behind a chimney)  {:.1}", rg.spike_t.max(0.0)) },
                if cue { Color32::WHITE } else { Color32::from_rgb(255, 70, 70) },
            ));
        } else if rg.mark as usize == me {
            if let Some(h) = regent_hint(r) {
                let extra = if rg.mode == raid::RM_JABS { format!("  ·  {} left", rg.chain_left) } else { String::new() };
                big = Some((format!("MARKED  ·  {h}{extra}"), Color32::from_rgb(255, 60, 60)));
            }
        }
        if rg.mode == raid::RM_WAVES && big.is_none() {
            big = Some(("BLOOD WAVES  ·  slip through the gaps moving straight".into(), Color32::from_rgb(255, 120, 120)));
        }
        if raid::in_ash(g, me) {
            text_shadow(p, Pos2::new(mid, rect.center().y + 120.0 * s), Align2::CENTER_CENTER, "IN THE ASH  ·  no guard, no abilities: GET OUT", 18.0 * s, Color32::from_rgb(255, 90, 90));
        }
    }
    if let Some((c, t, mark)) = r.call {
        let (word, col) = match c {
            raid::Call::Spread => ("SPREAD", Color32::from_rgb(255, 200, 80)),
            raid::Call::Stack => ("STACK", Color32::from_rgb(120, 220, 255)),
        };
        let who = r.member(mark as usize).map(|m| m.name.as_str()).unwrap_or("");
        text_shadow(p, Pos2::new(mid, rect.center().y - 150.0 * s), Align2::CENTER_CENTER, word, 54.0 * s, col);
        text_shadow(p, Pos2::new(mid, rect.center().y - 108.0 * s), Align2::CENTER_CENTER, &format!("{who}  ·  {:.1}", t.max(0.0)), 16.0 * s, col);
    }
    if let Some((t, col)) = big {
        text_shadow(p, Pos2::new(mid, rect.center().y - 70.0 * s), Align2::CENTER_CENTER, &t, 24.0 * s, col);
    }
    // ---- notes
    for (k, (t, life)) in r.notes.iter().rev().take(4).enumerate() {
        let a = (life / 0.6).clamp(0.0, 1.0);
        text_shadow(p, Pos2::new(mid, rect.top() + rect.height() * 0.30 + k as f32 * 24.0 * s), Align2::CENTER_CENTER, t, 17.0 * s, Color32::from_rgba_unmultiplied(255, 230, 190, (a * 255.0) as u8));
    }
    // ---- the chest's loot
    if let Some(c) = r.chest.as_ref().filter(|c| c.open) {
        let mine = &r.mine;
        let h = (70.0 + 26.0 * mine.len() as f32) * s;
        let box_r = Rect::from_center_size(Pos2::new(mid, rect.bottom() - 260.0 * s), Vec2::new(420.0 * s, h));
        let rare = c.rare && mine.iter().any(|m| m.1);
        p.rect_filled(box_r, 10.0, Color32::from_rgba_unmultiplied(18, 10, 24, 230));
        p.rect_stroke(box_r, 10.0, Stroke::new(2.0, if rare { Color32::from_rgb(190, 110, 255) } else { ACCENT }), egui::StrokeKind::Inside);
        p.text(box_r.center_top() + Vec2::new(0.0, 22.0 * s), Align2::CENTER_CENTER, if rare { "A RARE FIND!" } else { "THE CHEST" }, FontId::proportional(20.0 * s), if rare { Color32::from_rgb(210, 150, 255) } else { ACCENT });
        if mine.is_empty() {
            p.text(box_r.center() + Vec2::new(0.0, 12.0 * s), Align2::CENTER_CENTER, "(nothing for you this time)", FontId::proportional(14.0 * s), Color32::from_gray(170));
        }
        for (k, (item, rr)) in mine.iter().enumerate() {
            let l = LOOT.get(*item as usize);
            let name = l.map_or("?", |l| l.name);
            let kind = match l.map(|l| l.kind) {
                Some(LootKind::Look(_)) => "wardrobe",
                Some(LootKind::Dye(_)) => "dye",
                Some(LootKind::Pet) => "pet",
                Some(LootKind::Title) => "title",
                None => "",
            };
            let col = if *rr { Color32::from_rgb(210, 150, 255) } else { white };
            p.text(box_r.left_top() + Vec2::new(24.0 * s, (54.0 + 26.0 * k as f32) * s), Align2::LEFT_CENTER, format!("{name}  ({kind})"), FontId::proportional(16.0 * s), col);
        }
    }
    // ---- the wipe, the results
    match r.stage {
        Stage::Wipe => {
            text_shadow(p, Pos2::new(mid, rect.center().y - 40.0 * s), Align2::CENTER_CENTER, "THE PARTY FALLS", 44.0 * s, Color32::from_rgb(230, 70, 60));
            recap(p, Pos2::new(mid, rect.center().y + 10.0 * s), r, s, 5);
        }
        Stage::Done => {
            let w = 520.0 * s;
            let rr = Rect::from_center_size(Pos2::new(mid, rect.center().y + 10.0 * s), Vec2::new(w, 330.0 * s));
            p.rect_filled(rr, 10.0, Color32::from_rgba_unmultiplied(16, 10, 10, 225));
            p.rect_stroke(rr, 10.0, Stroke::new(2.0, ACCENT), egui::StrokeKind::Inside);
            let mut yy = rr.top() + 26.0 * s;
            p.text(Pos2::new(mid, yy), Align2::CENTER_CENTER, if r.hard { "THE VAULT IS BROKEN  ·  SEALED WRIT" } else { "THE VAULT IS BROKEN" }, FontId::proportional(22.0 * s), ACCENT);
            yy += 30.0 * s;
            p.text(Pos2::new(mid, yy), Align2::CENTER_CENTER, format!("{} of you  ·  total {}  ·  {} wipes", r.members.len(), raid::fmt_time(r.result_ms as f32 / 1000.0), r.wipes), FontId::proportional(16.0 * s), white);
            yy += 28.0 * s;
            for (k, rm) in ROOMS.iter().enumerate() {
                p.text(Pos2::new(rr.left() + 40.0 * s, yy), Align2::LEFT_CENTER, rm.name, FontId::proportional(14.0 * s), Color32::from_gray(200));
                p.text(Pos2::new(rr.right() - 40.0 * s, yy), Align2::RIGHT_CENTER, raid::fmt_time(r.splits[k]), FontId::proportional(14.0 * s), white);
                yy += 20.0 * s;
            }
            yy += 8.0 * s;
            p.text(Pos2::new(mid, yy), Align2::CENTER_CENTER, "Damage dealt", FontId::proportional(14.0 * s), ACCENT);
            yy += 18.0 * s;
            for m in &r.members {
                p.text(Pos2::new(mid, yy), Align2::CENTER_CENTER, format!("{}  {:.0}", m.name, m.dealt), FontId::proportional(13.0 * s), white);
                yy += 17.0 * s;
            }
            recap(p, Pos2::new(mid, rr.bottom() + 20.0 * s), r, s, 4);
        }
        _ => {}
    }
}

/// The death recap: who went down to what, newest first.
fn recap(p: &egui::Painter, at: Pos2, r: &raid::Raid, s: f32, n: usize) {
    if r.recap.is_empty() {
        return;
    }
    text_shadow(p, at, Align2::CENTER_CENTER, "DEATH RECAP", 14.0 * s, Color32::from_rgb(230, 120, 100));
    for (k, (slot, cause, room)) in r.recap.iter().rev().take(n).enumerate() {
        let who = r.member(*slot as usize).map(|m| m.name.as_str()).unwrap_or("someone");
        let c: Cause = *cause;
        let t = format!("{who}: {}  ({})", c.name(), ROOMS[(*room as usize).min(4)].name);
        text_shadow(p, at + Vec2::new(0.0, (18.0 + 17.0 * k as f32) * s), Align2::CENTER_CENTER, &t, 13.0 * s, Color32::from_rgb(230, 210, 200));
    }
}

// ================================================================== a boss's introduction

/// A hall's boss introduced as its fight begins: the camera sweeps in from high by the door to
/// look up at the boss while its name card shows, between black bars. It plays the first time a
/// hall begins in a raid (not after a wipe); a boss met before (saved in Settings) can be skipped
/// with Space.
#[derive(Clone, Copy, Debug, Default)]
pub struct Intro {
    pub room: u8,
    pub on: bool,
    pub t: f32,
    pub skippable: bool,
    pub skipped: bool,
    /// Halls introduced in this raid (one bit each).
    pub shown: u8,
}

/// How long an introduction runs (the hall's Intro stage).
pub const INTRO_TIME: f32 = 3.5;

fn ease(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Move the introduction on by a frame. `seen` says which halls' bosses were met before; returns
/// a hall to remember as met (once most of its introduction has played).
pub fn intro_step(it: &mut Intro, g: &Game, dt: f32, seen: u8, skip: bool) -> Option<u8> {
    let Some(r) = g.my_raid() else {
        *it = Intro::default();
        return None;
    };
    let room = r.room.min(7) as u8;
    if r.stage == Stage::Intro && it.shown & (1 << room) == 0 && !it.on {
        *it = Intro { room, on: true, t: 0.0, skippable: seen & (1 << room) != 0, skipped: false, shown: it.shown | (1 << room) };
    }
    let mut met = None;
    if it.on {
        it.t += dt;
        if skip && it.skippable && it.t > 0.15 {
            it.skipped = true;
        }
        if it.t > 1.5 && seen & (1 << it.room) == 0 {
            met = Some(it.room);
        }
        if it.skipped || it.t >= INTRO_TIME + 0.5 || room != it.room {
            it.on = false;
        }
    }
    met
}

/// What the introduction's camera sees: (position, the point it looks at, field of view).
pub fn intro_cam(it: &Intro, g: &Game) -> Option<(V3, V3, f32)> {
    if !it.on || it.skipped {
        return None;
    }
    let r = g.my_raid()?;
    let b = r.boss()?;
    let pos_of = |f: &raid::Foe| if g.is_authority() { f.pos } else { f.shown };
    // (the twins: frame them both)
    let twins: Vec<V3> = r.foes.iter().filter(|f| matches!(f.kind, FoeKind::Tally | FoeKind::Seal)).map(pos_of).collect();
    let focus = if twins.len() == 2 { (twins[0] + twins[1]) * 0.5 } else { pos_of(b) };
    let (head, size) = match b.kind {
        FoeKind::Warden => (4.3, 1.0),
        FoeKind::Tally | FoeKind::Seal => (2.6, 1.3),
        FoeKind::Ledger => (3.3, 0.95),
        FoeKind::Magistrate => (3.2, 0.9),
        _ => (2.0, 0.65),
    };
    let k = ease(0.0, INTRO_TIME, it.t);
    // (the twins: from square in front of the pair, so both are in the picture)
    let a = if twins.len() == 2 {
        // (out towards the middle of the court, where there's room to stand back)
        let c = ROOMS[(it.room as usize).min(ROOMS.len() - 1)].centre();
        let mut d = (v3(c.x, focus.y, c.z) - focus).flat();
        if d.len() < 1.0 {
            d = yaw_fwd(b.yaw);
        }
        d.x.atan2(d.z) + 0.45 - 0.45 * k
    } else {
        b.yaw + 1.25 - 0.95 * k
    };
    let dist = (13.0 - 6.2 * k) * size;
    let h = (7.0 - 5.4 * k) * size.max(0.8);
    let rm = ROOMS[(it.room as usize).min(ROOMS.len() - 1)];
    let mut pos = focus + yaw_fwd(a) * dist;
    pos.x = pos.x.clamp(rm.x.0 + 0.8, rm.x.1 - 0.8);
    pos.z = pos.z.clamp(rm.z.0 + 0.8, rm.z.1 - 0.8);
    pos.y = focus.y + h;
    let look = focus + V3::UP * (head * (0.5 + 0.45 * k));
    Some((pos, look, 62.0 - 10.0 * k))
}

/// Each hall's boss: its name and a line under it.
fn intro_title(room: u8) -> (&'static str, &'static str) {
    match room {
        0 => ("THE WARDEN OF WAX", "Keeper of the Candle Halls"),
        1 => ("THE TWIN ASSESSORS", "Tally, the Gilded  ·  Seal, the Grey"),
        2 => ("THE CHAINED LEDGER", "Every debt, written in ink"),
        3 => ("LORD VESPER KAEL", "The Ashblood Regent"),
        _ => ("THE ASHEN MAGISTRATE", "Judge of the Last Appeal"),
    }
}

/// The name card over the introduction: black bars, the hall's name, the boss's name large with a
/// rule under it and its line, and (for a boss met before) how to skip.
pub fn intro_card(p: &egui::Painter, rect: Rect, it: &Intro) {
    if !it.on || it.skipped {
        return;
    }
    let s = (rect.height() / 900.0).clamp(0.75, 1.6);
    let t = it.t;
    let bars = ease(0.0, 0.35, t) * (1.0 - ease(INTRO_TIME, INTRO_TIME + 0.45, t));
    let bh = rect.height() * 0.11 * bars;
    p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(rect.width(), bh)), 0.0, Color32::BLACK);
    p.rect_filled(Rect::from_min_size(Pos2::new(rect.left(), rect.bottom() - bh), Vec2::new(rect.width(), bh)), 0.0, Color32::BLACK);
    let a = ease(0.45, 1.0, t) * (1.0 - ease(INTRO_TIME - 0.35, INTRO_TIME + 0.2, t));
    if a <= 0.01 {
        return;
    }
    let (name, line) = intro_title(it.room);
    let hall = ROOMS[(it.room as usize).min(ROOMS.len() - 1)].name;
    let al = |c: [u8; 3], k: f32| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], (255.0 * k * a) as u8);
    let x = rect.left() + rect.width() * 0.08;
    let y = rect.bottom() - rect.height() * 0.11 - 150.0 * s;
    let slide = (1.0 - ease(0.45, 1.2, t)) * 40.0 * s;
    text_shadow(p, Pos2::new(x + slide, y), Align2::LEFT_CENTER, &hall.to_uppercase(), 15.0 * s, al([220, 190, 150], 0.9));
    text_shadow(p, Pos2::new(x + slide * 0.6, y + 46.0 * s), Align2::LEFT_CENTER, name, 52.0 * s, al([255, 236, 210], 1.0));
    let w = 520.0 * s * ease(0.6, 1.4, t);
    p.rect_filled(Rect::from_min_size(Pos2::new(x, y + 80.0 * s), Vec2::new(w, 2.5 * s)), 0.0, al([230, 120, 50], 0.95));
    text_shadow(p, Pos2::new(x + slide * 0.3, y + 104.0 * s), Align2::LEFT_CENTER, line, 20.0 * s, al([230, 215, 195], 0.9));
    if it.skippable {
        text_shadow(p, Pos2::new(rect.right() - 30.0 * s, rect.bottom() - bh * 0.5), Align2::RIGHT_CENTER, "SPACE  skip", 14.0 * s, al([200, 200, 200], 0.8));
    }
}

// ================================================================== pictures (DUEL_GALLERY=raid:...)

/// Test switch DUEL_GALLERY=raid:<page> (with DUEL_HUB=raid): a vault mechanic held in the middle
/// of its telegraph for a picture, run after every game step. Pages: gate, board (the hub's side:
/// panels opened by hub_scene), warden, twins, ledger, jab, ring, thrust, arc, waves, spikes,
/// pools, magistrate, chest.
pub fn stage(g: &mut Game, page: &str) {
    use crate::raid::{Pool, Shot, Tele, TeleKind, Wave};
    use crate::vault::VY;
    let me = g.local;
    if page == "gate" || page == "board" {
        let (gx, gz, _) = vault::GATE;
        if page == "gate" {
            for (k, s) in [me, 1, 2].into_iter().enumerate() {
                if s < g.f.len() {
                    let p = v3(gx - 1.0 + k as f32 * 1.0, 0.0, gz + 0.4 * k as f32);
                    g.f[s].pos = p;
                    g.f[s].prev_pos = p;
                    g.f[s].vel = V3::ZERO;
                    g.f[s].yaw = std::f32::consts::PI;
                }
            }
        } else {
            let (bx, _, bz) = vault::GATE_BOARD;
            let p = v3(bx + 1.6, 0.0, bz);
            g.f[me].pos = p;
            g.f[me].prev_pos = p;
            g.f[me].vel = V3::ZERO;
        }
        return;
    }
    // into the vault once, with three bots
    if g.raid.is_none() {
        let (gx, gz, _) = vault::GATE;
        g.f[me].respawn_at(v3(gx, 0.0, gz), 0.0);
        raid::gate_start(g, me, false, 3);
    }
    // (pages: <boss>[_<variant>], and intro_<boss> for a hall's introduction)
    let base = page.trim_start_matches("intro_");
    let room = match base.split('_').next().unwrap_or("") {
        "warden" => 0,
        "twins" => 1,
        "ledger" => 2,
        "magistrate" => 4,
        _ => 3,
    };
    let first = g.raid.as_ref().is_some_and(|r| r.room != room);
    if first {
        let r = g.raid.as_mut().unwrap();
        r.room = room;
        r.setup_room();
        r.place = r.place.wrapping_add(1);
    }
    if page == "magistrate_gavel" || page == "magistrate_fall" {
        raid::set_floor(g, false);
    }
    let slots: Vec<usize> = g.raid.as_ref().map_or(Vec::new(), |r| r.members.iter().map(|m| m.slot as usize).collect());
    let time = g.time;
    // (a dying page: the boss stands half a second, then falls)
    thread_local!(static START: std::cell::Cell<f32> = const { std::cell::Cell::new(-1.0) });
    let start = START.with(|s| {
        if s.get() < 0.0 || s.get() > time {
            s.set(time);
        }
        s.get()
    });
    let fallen = page.ends_with("_fall") && time - start > 0.5;
    let Some(r) = g.raid.as_mut() else { return };
    if page.starts_with("intro_") {
        // the hall's fight about to begin: its introduction plays (App::boss_intro)
        r.stage = Stage::Intro;
        r.stage_t = 3.0;
        r.teles.clear();
        return;
    }
    r.stage = Stage::Fight;
    r.stage_t = 0.0;
    r.cd = [99.0; 8];
    r.revives_left = 3;
    for m in &mut r.members {
        m.downed = false;
    }
    r.teles.clear();
    let tele = |kind, pos: V3, dir: f32, rad: f32, len: f32, k: f32, cause| Tele { kind, pos: v3(pos.x, VY, pos.z), dir, r: rad, len, t: 1.0 - k, total: 1.0, dmg: 0.0, cause, hit: 0 };
    let yaw_to = |a: V3, b: V3| (b.x - a.x).atan2(b.z - a.z);
    // where everyone stands (x, z), and the boss
    let mut spots: Vec<(f32, f32)> = Vec::new();
    match base {
        "warden" | "warden_slam" | "warden_idle" | "warden_fall" => {
            let c = ROOMS[0].centre();
            let b = v3(c.x + 1.0, VY, c.z + 1.0);
            r.foes[0].pos = b;
            r.foes[0].shown = b;
            r.foes[0].hp = r.foes[0].max * 0.62;
            r.foes[0].alive = !fallen;
            spots = vec![(c.x - 2.5, c.z - 2.0), (vault::PLATES[0].0, vault::PLATES[0].1), (vault::PLATES[1].0, vault::PLATES[1].1), (vault::PLATES[2].0, vault::PLATES[2].1)];
            let tank = v3(spots[0].0, VY, spots[0].1);
            r.foes[0].yaw = yaw_to(b, tank);
            match base {
                "warden" => {
                    r.foes[0].state = 1;
                    r.foes[0].t = 1.3 * (1.0 - 0.55);
                    r.teles.push(tele(TeleKind::Cone, b, r.foes[0].yaw, 6.0, 1.0, 0.55, Cause::Cleave));
                }
                "warden_slam" => {
                    r.foes[0].state = 2;
                    r.foes[0].t = 1.7 * (1.0 - 0.62);
                    r.teles.push(tele(TeleKind::Circle, b, 0.0, 4.6, 0.0, 0.62, Cause::Slam));
                    spots[0] = (c.x - 4.5, c.z - 3.5);
                }
                _ => {
                    r.foes[0].state = 0;
                    r.foes[0].t = 0.0;
                }
            }
            r.plates = [1.0, 1.0, 0.7, 0.0];
            r.flood = 0.3;
            if r.pools.is_empty() {
                r.pools.push(Pool { kind: 0, pos: v3(c.x - 4.0, VY, c.z + 4.0), r: 2.4, max: 3.2, life: 99.0 });
            }
        }
        "twins" | "twins_apart" | "twins_fall" => {
            // (close: bound, their chain glowing; apart: Tally throws, Seal stamps)
            let apart = base != "twins";
            r.foes[0].pos = if apart { v3(-6.0, VY, -25.0) } else { v3(-3.0, VY, -25.0) };
            r.foes[1].pos = if apart { v3(5.0, VY, -27.0) } else { v3(2.5, VY, -26.5) };
            for f in r.foes.iter_mut() {
                f.hp = f.max * 0.7;
                f.shown = f.pos;
                f.yaw = yaw_to(f.pos, v3(0.0, VY, -21.0));
            }
            r.foes[0].state = 1;
            r.foes[0].t = 0.5 * (1.0 - 0.2);
            r.foes[1].state = if apart { 1 } else { 0 };
            r.foes[1].t = 1.0 * (1.0 - 0.5);
            if base == "twins_fall" {
                r.foes[1].alive = !fallen;
                r.foes[1].state = 0;
            }
            spots = vec![(-1.0, -31.0), (-6.0, -19.0), (6.0, -30.0), (2.0, -20.0)];
            r.teles.push(tele(TeleKind::Circle, v3(6.0, VY, -30.0), 0.0, 2.6, 0.0, 0.6, Cause::Seal));
            r.teles.push(tele(TeleKind::Circle, v3(2.0, VY, -20.0), 0.0, 2.6, 0.0, 0.6, Cause::Seal));
            r.shots.clear();
            for c in -2..=2 {
                let a = 2.9 + c as f32 * 0.16;
                let d = ((time * 0.8).fract()) * 6.0 + 1.0;
                r.shots.push(Shot { kind: 0, pos: v3(-5.0, VY + 1.8, -25.0) + v3(a.sin(), -0.03, a.cos()) * d, vel: v3(a.sin(), 0.0, a.cos()) * 15.0, dmg: 0.0, life: 9.0 });
            }
        }
        "ledger" | "ledger_storm" | "ledger_fall" => {
            let c = ROOMS[2].centre();
            r.foes.retain(|f| f.kind == FoeKind::Ledger);
            r.foes[0].hp = r.foes[0].max * 0.55;
            r.foes[0].alive = !fallen;
            r.foes[0].state = if base == "ledger" { 1 } else { 0 };
            r.foes[0].t = 1.5 * (1.0 - 0.62);
            for k in 0..3 {
                let mut t = r.foes[0];
                t.kind = FoeKind::Thrall;
                t.pos = v3(c.x - 7.0 + k as f32 * 7.0, VY, c.z + if k == 1 { 7.5 } else { -6.0 });
                t.hp = 200.0;
                t.max = 260.0;
                t.yaw = yaw_to(t.pos, c);
                r.foes.push(t);
            }
            spots = vec![(c.x - 4.0, c.z - 5.0), (c.x - 1.0, c.z - 6.0), (c.x + 5.0, c.z + 3.0), (c.x + 6.0, c.z - 2.0)];
            if slots.len() >= 2 {
                r.tethers = vec![(slots[0] as u8, slots[1] as u8, 9.0, 0.0)];
            }
            let tgt = v3(spots[2].0, VY, spots[2].1);
            match base {
                "ledger" => r.teles.push(tele(TeleKind::Line, c, yaw_to(c, tgt), 1.3, 20.0, 0.5, Cause::InkBeam)),
                "ledger_storm" => r.teles.push(tele(TeleKind::Circle, c, 0.0, 6.0, 0.0, 0.55, Cause::PageStorm)),
                _ => {}
            }
        }
        "magistrate_gavel" | "magistrate_fall" => {
            let k = r.foes.iter().position(|f| f.kind == FoeKind::Magistrate).unwrap_or(0);
            r.foes[k].hp = r.foes[k].max * 0.8;
            r.phase = 1;
            r.floor_down = false;
            r.pools.clear();
            let c = ROOMS[4].centre();
            let b = v3(c.x, VY, c.z);
            r.foes[k].pos = b;
            r.foes[k].shown = b;
            r.foes[k].alive = !fallen;
            spots = vec![(c.x - 3.0, c.z - 6.0), (c.x + 5.0, c.z - 4.0), (c.x - 6.0, c.z + 2.0), (c.x + 4.0, c.z + 5.0)];
            let tgt = v3(spots[0].0, VY, spots[0].1);
            r.foes[k].yaw = yaw_to(b, tgt);
            if base == "magistrate_gavel" {
                r.foes[k].state = 1;
                r.foes[k].t = 1.5 * (1.0 - 0.6);
                r.teles.push(tele(TeleKind::Circle, tgt, 0.0, 3.6, 0.0, 0.6, Cause::Gavel));
            } else {
                r.foes[k].state = 0;
            }
        }
        "magistrate" => {
            let k = r.foes.iter().position(|f| f.kind == FoeKind::Magistrate).unwrap_or(0);
            r.foes[k].hp = r.foes[k].max * 0.55;
            r.phase = 2;
            r.floor_down = true;
            let (x0, z0) = vault::ISLANDS[0];
            r.foes[k].pos = v3(x0, VY, z0);
            spots = vec![(vault::ISLANDS[4].0, vault::ISLANDS[4].1), (vault::ISLANDS[1].0, vault::ISLANDS[1].1), (vault::ISLANDS[2].0, vault::ISLANDS[2].1), (vault::ISLANDS[3].0 + 1.0, vault::ISLANDS[3].1)];
            let tgt = v3(spots[1].0, VY, spots[1].1);
            r.foes[k].yaw = yaw_to(r.foes[k].pos, tgt);
            r.foes[k].shown = r.foes[k].pos;
            r.foes[k].state = 2;
            r.foes[k].t = 1.15 * (1.0 - 0.6);
            r.teles.push(tele(TeleKind::Line, r.foes[k].pos, r.foes[k].yaw, 0.75, 30.0, 0.6, Cause::Verdict));
            r.condemned = Some((3, 1.5));
            if r.pools.is_empty() {
                let (x, z) = vault::ISLANDS[2];
                r.pools.push(Pool { kind: 2, pos: v3(x, VY, z), r: vault::ISLAND_HALF + 0.6, max: vault::ISLAND_HALF + 0.6, life: 99.0 });
            }
            spots[2] = (vault::ISLANDS[4].0 + 1.5, vault::ISLANDS[4].1 + 1.0);
        }
        _ => {
            // the Regent's roof
            let k = r.foes.iter().position(|f| f.kind == FoeKind::Regent).unwrap_or(0);
            let (cx, cz) = vault::ROOF_CENTRE;
            let b = v3(cx, VY, cz + 3.0);
            r.foes[k].pos = b;
            r.foes[k].shown = b;
            r.foes[k].hp = r.foes[k].max * if matches!(page, "pools" | "chest") { 0.3 } else if matches!(page, "waves" | "spikes") { 0.6 } else { 0.85 };
            r.foes[k].yaw = std::f32::consts::PI;
            r.regent.vanished = false;
            r.regent.mark = me as u8;
            r.regent.mv = crate::raid::NO_MOVE;
            r.regent.mode = crate::raid::RM_MOVES;
            r.regent.t = 99.0;
            r.waves.clear();
            r.phase = 1;
            let fwd = v3(0.0, 0.0, -1.0);
            spots = vec![(cx, cz - 0.5), (cx - 3.5, cz + 6.0), (cx + 4.0, cz + 5.0), (cx + 2.5, cz - 1.5)];
            let mv = |r: &mut crate::raid::Raid, m: u8, state: u8| {
                r.regent.mv = m;
                r.regent.wind = 0.5;
                r.foes[k].state = state;
                r.foes[k].t = 0.5;
            };
            match page {
                "jab" => {
                    mv(r, raid::MV_JAB, 1);
                    r.teles.push(tele(TeleKind::Cone, b, std::f32::consts::PI, 4.2, 0.55, 0.55, Cause::Jab));
                }
                "ring" => {
                    mv(r, raid::MV_RING, 2);
                    r.teles.push(tele(TeleKind::Donut, b, std::f32::consts::PI, 9.0, 0.0, 0.55, Cause::BloodRing));
                    spots[0] = (cx + 0.4, cz + 1.4);
                }
                "thrust" => {
                    mv(r, raid::MV_THRUST, 3);
                    r.teles.push(tele(TeleKind::Line, b - fwd * 9.0, std::f32::consts::PI, 1.1, 18.0, 0.55, Cause::Thrust));
                    spots[0] = (cx + 2.6, cz + 0.5);
                }
                "arc" => {
                    mv(r, raid::MV_ARC, 4);
                    r.teles.push(tele(TeleKind::Arc, b, std::f32::consts::PI, 5.8, 0.6, 0.55, Cause::Arc));
                    spots[0] = (cx - 0.3, cz + 5.0);
                }
                "waves" => {
                    r.phase = 2;
                    r.regent.mode = crate::raid::RM_WAVES;
                    r.regent.vanished = true;
                    r.regent.waves_left = 3;
                    let roof = ROOMS[3];
                    let along = roof.z.0 + 4.0 + (time * 1.5).fract() * 0.01;
                    r.waves = vec![Wave { axis: 2, pos: along, gaps: [21.0, 29.5], ngaps: 2, hit: 0 }, Wave { axis: 1, pos: roof.x.1 - 3.0, gaps: [4.0, 0.0], ngaps: 1, hit: 0 }];
                    spots = vec![(29.5, -1.0), (21.0, 0.5), (21.2, 3.5), (25.0, 6.0)];
                }
                "spikes" => {
                    r.phase = 2;
                    r.regent.mode = crate::raid::RM_SPIKES;
                    r.regent.spikes = slots.iter().fold(0u16, |m, s| m | 1 << s);
                    r.regent.spike_t = 1.6;
                    r.foes[k].pos = v3(cx, VY, cz);
                    r.foes[k].state = 5;
                    spots = vec![(cx - 1.5, cz - 4.5), (cx - 5.5, cz + 2.0), (cx + 5.0, cz + 2.5), (vault::CHIMNEYS[3].0 + 1.4, vault::CHIMNEYS[3].1 + 1.4)];
                }
                "pools" => {
                    r.phase = 3;
                    if r.pools.is_empty() {
                        for (dx, dz) in [(-4.0f32, -2.0f32), (3.5, -3.5), (5.5, 4.0), (-5.0, 5.0), (0.5, -6.0), (-1.5, 2.0)] {
                            r.pools.push(Pool { kind: 3, pos: v3(cx + dx, VY, cz + dz), r: 1.6, max: 1.6, life: 999.0 });
                        }
                    }
                    for p in r.pools.iter_mut() {
                        p.life = 999.0;
                        p.r = 1.6;
                    }
                    r.teles.push(Tele { kind: TeleKind::Circle, pos: v3(cx - 2.0, VY, cz - 4.0), dir: 0.0, r: 1.6, len: 0.0, t: 0.06, total: 0.9, dmg: 0.0, cause: Cause::AshPool, hit: 0 });
                    mv(r, raid::MV_ARC, 4);
                    r.teles.push(tele(TeleKind::Arc, b, std::f32::consts::PI, 5.8, 0.6, 0.35, Cause::Arc));
                    spots = vec![(cx - 0.2, cz + 5.5), (cx - 5.5, cz - 1.0), (cx + 3.5, cz - 0.5), (cx + 1.0, cz - 3.0)];
                }
                "chest" => {
                    r.foes[k].alive = false;
                    r.stage = Stage::Cleared;
                    if r.chest.is_none() {
                        r.chest = Some(raid::Chest { pos: v3(cx, VY, cz), rare: true, open: true, items: vec![(me as u8, 25), (me as u8, 21)] });
                        r.mine = vec![(25, false), (21, true)];
                    }
                    spots = vec![(cx - 0.5, cz - 2.2), (cx - 2.5, cz + 1.0), (cx + 2.5, cz + 0.5), (cx + 1.5, cz - 2.5)];
                }
                _ => {}
            }
        }
    }
    // everyone held where they stand, facing the boss, well
    let boss = r.boss().map(|b| b.pos).unwrap_or(ROOMS[room].centre());
    for (k, s) in slots.iter().enumerate() {
        if let Some(&(x, z)) = spots.get(k) {
            let p = v3(x, VY, z);
            let f = &mut g.f[*s];
            f.pos = p;
            f.prev_pos = p;
            f.net_pos = p;
            f.vel = V3::ZERO;
            f.hp = MAX_HP * if k == 0 { 0.8 } else { 0.6 + 0.1 * k as f32 };
            f.yaw = yaw_to(p, boss);
            f.on_ground = true;
        }
    }
}
