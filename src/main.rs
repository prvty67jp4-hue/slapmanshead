#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod bot;
mod game;
mod gpu;
mod math;
mod net;
mod render;
mod world;

use eframe::egui::{self, Align2, Color32, FontId, Key, Pos2, Rect, Stroke, Vec2, ViewportCommand};
use game::*;
use math::*;
use net::{Incoming, Net};
use render::Cam;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const TITLE: &str = "DUEL RUSH";
const ACCENT: Color32 = Color32::from_rgb(255, 150, 70);

#[derive(Clone)]
struct Settings {
    name: String,
    sens: f32,
    fov: f32,
    bot_level: usize,
    rounds: u8,
    mode: u8,
    bots: usize,
    port: String,
    join_addr: String,
    volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            name: "Player".into(),
            sens: 1.0,
            fov: 80.0,
            bot_level: 1,
            rounds: 15,
            mode: 0,
            bots: 3,
            port: net::DEFAULT_PORT.to_string(),
            join_addr: String::new(),
            volume: 0.7,
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(base).join("DuelRush").join("settings.txt"))
}

impl Settings {
    fn load() -> Settings {
        let mut s = Settings::default();
        if let Some(p) = settings_path() {
            if let Ok(text) = std::fs::read_to_string(p) {
                for line in text.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        let v = v.trim();
                        match k.trim() {
                            "name" if !v.is_empty() => s.name = v.chars().take(16).collect(),
                            "sens" => s.sens = v.parse().unwrap_or(s.sens),
                            "fov" => s.fov = v.parse().unwrap_or(s.fov),
                            "bot" => s.bot_level = v.parse().unwrap_or(s.bot_level).min(3),
                            "rounds" => s.rounds = v.parse().unwrap_or(s.rounds).clamp(1, 50),
                            "mode" => s.mode = v.parse().unwrap_or(0u8).min(1),
                            "bots" => s.bots = v.parse().unwrap_or(3usize).clamp(1, 7),
                            "port" => s.port = v.to_string(),
                            "join" => s.join_addr = v.to_string(),
                            "volume" => s.volume = v.parse().unwrap_or(s.volume),
                            _ => {}
                        }
                    }
                }
            }
        }
        s
    }
    fn save(&self) {
        if let Some(p) = settings_path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let text = format!(
                "name={}\nsens={}\nfov={}\nbot={}\nrounds={}\nmode={}\nbots={}\nport={}\njoin={}\nvolume={}\n",
                self.name, self.sens, self.fov, self.bot_level, self.rounds, self.mode, self.bots, self.port, self.join_addr, self.volume
            );
            let _ = std::fs::write(p, text);
        }
    }
}

const BOT_LEVELS: [(&str, f32); 4] = [("Easy", 0.2), ("Normal", 0.5), ("Hard", 0.8), ("Brutal", 1.0)];

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Main,
    Practice,
    Host,
    Join,
    Settings,
    HowTo,
}

enum Screen {
    Menu(Page),
    Lobby { net: Net, waiting_since: f32 },
    Play { game: Game, net: Option<Net> },
}

#[derive(Default)]
struct Latch {
    jump: bool,
    dash: bool,
    fire_pressed: bool,
    reload: bool,
    grenade: bool,
    select: Option<u8>,
}

struct App {
    screen: Screen,
    demo: Game,
    gpu: Option<gpu::Gpu>,
    settings: Settings,
    menu_msg: Option<String>,
    screenshot: Option<PathBuf>,
    frames: u32,
    shot_slot: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
    shot_requested: bool,
    acc: f32,
    latch: Latch,
    aim_yaw: f32,
    aim_pitch: f32,
    local_alive: bool,
    start_request: bool,
    autostart: usize,
    show_scores: bool,
    paused: bool,
    captured: bool,
    prev_shift: bool,
    disconnected: Option<String>,
    demo_t: f32,
    audio: Option<audio::Audio>,
    lan_ip: Option<String>,
    demo_cam: (V3, V3),
    autoplay: bool,
    last_tap: Option<(Key, f64)>,
    cam_dist: std::cell::Cell<f32>,
    frame_dt: f32,
}

impl App {
    fn new(screenshot: Option<PathBuf>) -> App {
        let demo = Game::demo();
        let settings = Settings::load();
        let audio_test = std::env::var_os("DUEL_AUDIO_TEST").is_some();
        let audio = if screenshot.is_none() || audio_test { audio::Audio::new() } else { None };
        if audio_test {
            eprintln!("audio device: {}", if audio.is_some() { "ok" } else { "none" });
        }
        let mut app = App {
            screen: Screen::Menu(Page::Main),
            demo,
            gpu: None,
            settings,
            menu_msg: None,
            screenshot,
            frames: 0,
            shot_slot: Arc::new(Mutex::new(None)),
            shot_requested: false,
            acc: 0.0,
            latch: Latch::default(),
            aim_yaw: 0.0,
            aim_pitch: 0.0,
            local_alive: true,
            start_request: false,
            autostart: 0,
            show_scores: false,
            paused: false,
            captured: false,
            prev_shift: false,
            disconnected: None,
            demo_t: 0.0,
            audio,
            lan_ip: net::local_ip(),
            demo_cam: (V3::ZERO, V3::ZERO),
            autoplay: false,
            last_tap: None,
            cam_dist: std::cell::Cell::new(5.0),
            frame_dt: 0.016,
        };
        // hidden test mode: a bot plays the local fighter (used to photograph gameplay)
        if std::env::var_os("DUEL_AUTOPLAY").is_some() {
            let nb: usize = std::env::var("DUEL_BOTS").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
            let mode = if std::env::var_os("DUEL_TDM").is_some() { Mode::Tdm } else { Mode::Ffa };
            let mut g = Game::offline("Tester", nb, 0.6, mode, 15);
            g.f[0].is_bot = true;
            g.bots[0] = Some(bot::Bot::new(0.8, 5));
            app.autoplay = true;
            app.start_game(g, None);
        }
        // hidden network test modes (a bot plays for you)
        if let Some(port) = std::env::var_os("DUEL_AUTOHOST") {
            let port: u16 = port.to_string_lossy().parse().unwrap_or(net::DEFAULT_PORT);
            app.settings.name = "HostBot".into();
            app.autoplay = true;
            app.autostart = std::env::var("DUEL_AUTOSTART").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
            let mode = if std::env::var_os("DUEL_TDM").is_some() { 1 } else { 0 };
            app.settings.mode = mode;
            let _ = app.start_hosting(port);
        }
        if let Some(addr) = std::env::var_os("DUEL_AUTOJOIN") {
            app.settings.name = std::env::var("DUEL_NAME").unwrap_or_else(|_| "JoinBot".into());
            if let Ok(n) = Net::join(&addr.to_string_lossy()) {
                app.autoplay = true;
                app.screen = Screen::Lobby { net: n, waiting_since: 0.0 };
            }
        }
        app
    }

    fn start_game(&mut self, game: Game, net: Option<Net>) {
        let l = game.local;
        self.aim_yaw = game.f[l].yaw;
        self.aim_pitch = 0.0;
        self.local_alive = game.f[l].alive;
        self.acc = 0.0;
        self.latch = Latch::default();
        self.paused = false;
        self.disconnected = None;
        self.screen = Screen::Play { game, net };
    }

    /// Open a lobby: you're in the arena at once (warm-up); friends drop in as they join.
    fn start_hosting(&mut self, port: u16) -> Result<(), String> {
        let n = Net::host(port)?;
        let mode = Mode::from_u8(self.settings.mode);
        let mut g = Game::new(Kind::Host, 0, mode, self.settings.rounds as u16);
        let name = clean_name(&self.settings.name, 0);
        g.add_player(0, &name, true, None, 0);
        if self.autoplay {
            g.f[0].is_bot = true;
            g.bots[0] = Some(bot::Bot::new(0.8, 11));
        }
        let addr = match &self.lan_ip {
            Some(ip) => format!("{ip}:{port}"),
            None => format!("port {port}"),
        };
        g.say(format!("Lobby open on {addr} - friends: Join a Friend"));
        self.start_game(g, Some(n));
        Ok(())
    }

    fn leave_to_menu(&mut self) {
        if let Screen::Play { net: Some(net), .. } | Screen::Lobby { net, .. } = &mut self.screen {
            net.send_bye();
        }
        self.screen = Screen::Menu(Page::Main);
        self.paused = false;
        self.disconnected = None;
    }

    fn set_capture(&mut self, ctx: &egui::Context, on: bool) {
        if on == self.captured {
            return;
        }
        self.captured = on;
        ctx.send_viewport_cmd(ViewportCommand::CursorVisible(!on));
        let grab = if !on {
            egui::CursorGrab::None
        } else if cfg!(target_os = "macos") {
            egui::CursorGrab::Locked
        } else {
            egui::CursorGrab::Confined
        };
        ctx.send_viewport_cmd(ViewportCommand::CursorGrab(grab));
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--screenshot" && i + 1 < args.len() {
            screenshot = Some(PathBuf::from(&args[i + 1]));
            i += 1;
        }
        i += 1;
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Duel Rush")
        .with_inner_size([1280.0, 720.0])
        .with_min_inner_size([800.0, 450.0]);
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        use eframe::winit::platform::macos::EventLoopBuilderExtMacOS;
        options.event_loop_builder = Some(Box::new(|b| {
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    eframe::run_native(
        "Duel Rush",
        options,
        Box::new(move |cc| {
            let mut style = (*cc.egui_ctx.global_style()).clone();
            style.visuals = egui::Visuals::dark();
            style.spacing.button_padding = Vec2::new(14.0, 8.0);
            style.text_styles.insert(egui::TextStyle::Button, FontId::proportional(18.0));
            style.text_styles.insert(egui::TextStyle::Body, FontId::proportional(16.0));
            style.text_styles.insert(egui::TextStyle::Heading, FontId::proportional(26.0));
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            cc.egui_ctx.set_style_of(egui::Theme::Dark, style);
            Ok(Box::new(App::new(screenshot)))
        }),
    )
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ctx.request_repaint();
        self.frames += 1;
        let rect = ui.max_rect();
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
        self.frame_dt = dt;
        self.handle_screenshot(&ctx);

        // network + simulation
        let mut go_menu = false;
        if let Screen::Lobby { .. } = self.screen {
            self.update_lobby(&ctx, dt);
        }

        let playing = matches!(self.screen, Screen::Play { .. });
        if playing {
            let focused = ctx.input(|i| i.focused);
            if !focused && !self.paused && !self.autoplay {
                self.paused = true;
            }
            let want_capture = !self.paused && !self.match_over() && self.disconnected.is_none() && !self.autoplay;
            self.set_capture(&ctx, want_capture);
            if want_capture && !cfg!(target_os = "macos") {
                ctx.send_viewport_cmd(ViewportCommand::CursorPosition(rect.center()));
            }
            sample_keys(&ctx);
            self.read_play_input(&ctx);
            self.step_play(dt);
        } else {
            self.set_capture(&ctx, false);
            // background demo match
            self.demo_t += dt;
            let mut a = self.acc + dt;
            let mut n = 0;
            while a >= TICK && n < 12 {
                self.demo.tick(&Input::default());
                a -= TICK;
                n += 1;
            }
            self.acc = a.min(TICK);
            self.demo.sounds.clear();
            self.update_demo_cam(dt);
        }

        // ---- drawing
        let painter = ui.painter().clone();
        let ppp = ctx.pixels_per_point();
        if self.gpu.is_none() {
            if let Some(rs) = frame.wgpu_render_state() {
                self.gpu = Some(gpu::Gpu::new(rs, &self.demo.world));
            }
        }
        let mut gpu = self.gpu.take();
        let alpha = (self.acc / TICK).clamp(0.0, 1.0);
        let full_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        match &self.screen {
            Screen::Play { game, .. } => {
                let (cam, lp) = self.play_camera(game, rect);
                if let (Some(g3), Some(rs)) = (gpu.as_mut(), frame.wgpu_render_state()) {
                    if let Some(id) = g3.render(rs, game, &cam, rect, ppp, alpha, Some((game.local, lp))) {
                        painter.image(id, rect, full_uv, Color32::WHITE);
                    }
                }
                self.draw_hud(&painter, rect, &cam, game);
            }
            _ => {
                let cam = self.demo_camera(rect);
                if let (Some(g3), Some(rs)) = (gpu.as_mut(), frame.wgpu_render_state()) {
                    if let Some(id) = g3.render(rs, &self.demo, &cam, rect, ppp, alpha, None) {
                        painter.image(id, rect, full_uv, Color32::WHITE);
                    }
                }
                // darken for readability
                painter.rect_filled(Rect::from_min_max(rect.min, Pos2::new(rect.left() + 520.0, rect.bottom())), 0.0, Color32::from_rgba_unmultiplied(10, 8, 20, 150));
            }
        }

        self.gpu = gpu;

        // ---- UI overlays
        match &self.screen {
            Screen::Menu(page) => {
                let page = *page;
                go_menu |= self.menu_ui(&ctx, rect, page);
            }
            Screen::Lobby { .. } => {
                go_menu |= self.lobby_ui(&ctx, rect);
            }
            Screen::Play { .. } => {
                go_menu |= self.play_overlay_ui(&ctx, rect);
            }
        }
        if go_menu {
            self.leave_to_menu();
        }

        // play queued sounds
        let sounds: Vec<Sfx> = match &mut self.screen {
            Screen::Play { game, .. } => std::mem::take(&mut game.sounds),
            _ => Vec::new(),
        };
        if let Some(a) = &mut self.audio {
            a.set_volume(if std::env::var_os("DUEL_AUDIO_TEST").is_some() { 0.0 } else { self.settings.volume });
            for s in sounds {
                a.play(s);
            }
        }
    }

    fn on_exit(&mut self) {
        if let Screen::Play { net: Some(net), .. } | Screen::Lobby { net, .. } = &mut self.screen {
            net.send_bye();
        }
        if !self.autoplay {
            self.settings.save();
        }
    }
}

impl App {
    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        let shot_frame: u32 = std::env::var("DUEL_SHOT_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(120);
        if self.frames >= shot_frame && !self.shot_requested {
            self.shot_requested = true;
            let slot = self.shot_slot.clone();
            ctx.request_screenshot(move |img| {
                if let Ok(mut s) = slot.lock() {
                    *s = Some(img);
                }
            });
        }
        let got = self.shot_slot.lock().ok().and_then(|mut s| s.take());
        if let Some(img) = got {
            let [w, h] = img.size;
            let mut bytes = Vec::with_capacity(w * h * 4);
            for c in &img.pixels {
                let [r, g, b, _] = c.to_array();
                bytes.extend_from_slice(&[r, g, b, 255]);
            }
            if let Some(dir) = path.parent() {
                if !dir.as_os_str().is_empty() {
                    let _ = std::fs::create_dir_all(dir);
                }
            }
            if let Err(e) = image::save_buffer(&path, &bytes, w as u32, h as u32, image::ColorType::Rgba8) {
                eprintln!("could not save screenshot: {e}");
            }
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if self.frames > shot_frame + 1500 {
            eprintln!("screenshot timed out");
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    fn match_over(&self) -> bool {
        match &self.screen {
            Screen::Play { game, .. } => game.mt.phase == Phase::MatchOver,
            _ => false,
        }
    }

    fn read_play_input(&mut self, ctx: &egui::Context) {
        let Screen::Play { .. } = &self.screen else { return };
        let mut toggle_pause = false;
        let captured = self.captured;
        let sens = self.settings.sens * 0.0024;
        ctx.input(|i| {
            if i.key_pressed(Key::Escape) {
                toggle_pause = true;
            }
            self.show_scores = i.key_down(Key::Tab);
            if i.key_pressed(Key::Enter) {
                self.start_request = true;
            }
            if !captured {
                return;
            }
            let mut d = Vec2::ZERO;
            for e in &i.events {
                if let egui::Event::MouseMoved(m) = e {
                    d += *m;
                }
            }
            self.aim_yaw = wrap_angle(self.aim_yaw + d.x * sens);
            self.aim_pitch = (self.aim_pitch - d.y * sens).clamp(-1.4, 1.4);
            let tap = |k: Key| i.events.iter().any(|e| matches!(e, egui::Event::Key { key, pressed: true, repeat: false, .. } if *key == k));
            if tap(Key::Space) {
                self.latch.jump = true;
            }
            // double-tap a direction to dodge
            let now = i.time;
            for k in [Key::W, Key::A, Key::S, Key::D] {
                if tap(k) {
                    if let Some((lk, lt)) = self.last_tap {
                        if lk == k && now - lt < 0.25 {
                            self.latch.dash = true;
                        }
                    }
                    self.last_tap = Some((k, now));
                }
            }
            let shift = i.modifiers.shift;
            if shift && !self.prev_shift {
                self.latch.dash = true;
            }
            self.prev_shift = shift;
            if tap(Key::Q) || tap(Key::E) {
                self.latch.dash = true;
            }
            if i.pointer.primary_pressed() {
                self.latch.fire_pressed = true;
            }
            if tap(Key::R) {
                self.latch.reload = true;
            }
            if tap(Key::G) || tap(Key::F) {
                self.latch.grenade = true;
            }
            if i.key_pressed(Key::Num1) {
                self.latch.select = Some(0);
            }
            if i.key_pressed(Key::Num2) {
                self.latch.select = Some(1);
            }
            if i.key_pressed(Key::Num3) {
                self.latch.select = Some(2);
            }
        });
        // scroll wheel cycles weapons
        let scroll = ctx.input(|i| {
            let mut s = 0.0;
            for e in &i.events {
                if let egui::Event::MouseWheel { delta, .. } = e {
                    s += delta.y;
                }
            }
            s * 100.0
        });
        if captured && scroll.abs() > 0.5 {
            if let Screen::Play { game, .. } = &self.screen {
                let cur = game.f[game.local].weapon as i32;
                let next = (cur + if scroll < 0.0 { 1 } else { 2 }).rem_euclid(3);
                self.latch.select = Some(next as u8);
            }
        }
        if toggle_pause && !self.match_over() && self.disconnected.is_none() {
            self.paused = !self.paused;
        }
    }

    fn current_input(&self, ctx_keys: &KeysHeld) -> Input {
        let mut inp = Input { yaw: self.aim_yaw, pitch: self.aim_pitch, ..Default::default() };
        if !self.captured {
            return inp;
        }
        inp.fwd = (ctx_keys.w as i32 - ctx_keys.s as i32) as f32;
        inp.right = (ctx_keys.d as i32 - ctx_keys.a as i32) as f32;
        inp.fire = ctx_keys.fire;
        inp.guard = ctx_keys.guard;
        inp.jump = self.latch.jump;
        inp.dash = self.latch.dash;
        inp.fire_pressed = self.latch.fire_pressed;
        inp.reload = self.latch.reload;
        inp.grenade = self.latch.grenade;
        inp.select = self.latch.select;
        inp
    }

    fn step_play(&mut self, dt: f32) {
        let keys = self.keys_snapshot();
        let mut inp = self.current_input(&keys);
        let start_req = std::mem::take(&mut self.start_request);
        let autostart = self.autostart;
        let Screen::Play { game, net } = &mut self.screen else { return };

        // network receive
        if let Some(n) = net.as_mut() {
            for msg in n.poll() {
                match msg {
                    Incoming::Join { slot, name } => {
                        let name = clean_name(&name, slot);
                        let team = game.balance_team();
                        game.add_player(slot, &name, false, None, team);
                        let t = if game.mt.mode == Mode::Tdm { format!(" ({} team)", TEAM_NAMES[team as usize]) } else { String::new() };
                        game.say(format!("{name} joined{t}"));
                    }
                    Incoming::Leave { slot } => {
                        if game.f[slot].present {
                            let nm = game.f[slot].name.clone();
                            game.remove_player(slot);
                            game.say(format!("{nm} left"));
                        }
                    }
                    Incoming::PeerState { slot, st, events } => {
                        if game.f[slot].present {
                            net::apply_remote(&mut game.f[slot], &st);
                            for ev in events {
                                game.handle_remote_event(slot, ev);
                            }
                        }
                    }
                    Incoming::Host(m) => apply_host_msg(game, m),
                    Incoming::HostLeft => {
                        self.disconnected = Some("The host closed the lobby.".into());
                    }
                    Incoming::Full => {}
                }
            }
            if !n.is_host && n.last_recv.elapsed().as_secs_f32() > 8.0 && self.disconnected.is_none() {
                self.disconnected = Some("Lost connection to the host.".into());
            }
            if n.is_host {
                let ready = autostart > 0 && game.mt.phase == Phase::Warmup && n.player_count() >= autostart;
                if (start_req && matches!(game.mt.phase, Phase::Warmup | Phase::MatchOver)) || ready {
                    game.start_match();
                }
            }
        } else if start_req && game.mt.phase == Phase::MatchOver {
            game.start_match();
        }

        if self.paused && game.kind == Kind::Offline {
            self.acc = 0.0;
            return;
        }
        self.acc += dt;
        let mut steps = 0;
        while self.acc >= TICK && steps < 12 {
            game.tick(&inp);
            // edges are used once
            inp.jump = false;
            inp.dash = false;
            inp.fire_pressed = false;
            inp.reload = false;
            inp.grenade = false;
            inp.select = None;
            self.latch = Latch::default();
            self.acc -= TICK;
            steps += 1;
        }
        if steps == 12 {
            self.acc = 0.0;
        }
        // after a respawn, look where the new body faces
        let l = game.local;
        if game.f[l].alive && !self.local_alive {
            self.aim_yaw = game.f[l].yaw;
            self.aim_pitch = 0.0;
        }
        self.local_alive = game.f[l].alive;
        if let Some(n) = net.as_mut() {
            for ev in game.out_events.drain(..) {
                n.queue(ev);
            }
            n.send(game, dt);
        } else {
            game.out_events.clear();
        }
    }

    fn keys_snapshot(&self) -> KeysHeld {
        KEYS.with(|k| *k.borrow())
    }

    /// Joining: say hello until the host answers, then step into its match.
    fn update_lobby(&mut self, ctx: &egui::Context, dt: f32) {
        let name = clean_name(&self.settings.name, 1);
        let Screen::Lobby { net, waiting_since } = &mut self.screen else { return };
        *waiting_since += dt;
        net.send_hello(&name, dt);
        let mut first: Option<net::HostMsg> = None;
        for m in net.poll() {
            match m {
                Incoming::Host(h) => {
                    first = Some(h);
                    break;
                }
                Incoming::Full => {
                    self.menu_msg = Some("That lobby is full (8 players).".into());
                    self.screen = Screen::Menu(Page::Join);
                    return;
                }
                _ => {}
            }
        }
        if first.is_none() && *waiting_since > 15.0 {
            self.menu_msg = Some("Couldn't reach the host. Check the address, that they pressed Host, and their firewall / port forwarding.".into());
            self.screen = Screen::Menu(Page::Join);
            return;
        }
        if let Some(h) = first {
            let Screen::Lobby { net, .. } = std::mem::replace(&mut self.screen, Screen::Menu(Page::Main)) else { return };
            let mut g = Game::new(Kind::Client, h.you, h.mode, h.target);
            g.mt.number = h.number;
            let you = h.you;
            apply_host_msg(&mut g, h);
            if self.autoplay {
                g.f[you].is_bot = true;
                g.bots[you] = Some(bot::Bot::new(0.8, 11 + you as u64));
            }
            let host = g.f[0].name.clone();
            g.say(format!("Joined {host}'s lobby"));
            self.start_game(g, Some(net));
            ctx.request_repaint();
        }
    }
    fn demo_target(&self) -> (V3, V3) {
        let a = self.demo.f[0].pos;
        let b = match self.demo.nearest_enemy(0) {
            Some(j) => self.demo.f[j].pos,
            None => self.demo.f[1].pos,
        };
        let mid = (a + b) * 0.5 + v3(0.0, 1.0, 0.0);
        let sep = (b - a).flat().len();
        let axis = if sep > 0.1 { (b - a).flat().norm() } else { v3(1.0, 0.0, 0.0) };
        let swing = (self.demo_t * 0.13).sin() * 0.7;
        let perp = v3(-axis.z, 0.0, axis.x);
        let side = (perp * swing.cos() + axis * swing.sin()).norm();
        let dist = (sep * 0.5 / 0.42).clamp(15.0, 21.0);
        let mut pos = mid + side * dist + v3(0.0, 6.0, 0.0);
        let to = pos - mid;
        let l = to.len();
        if let Some((t, _)) = self.demo.world.raycast(mid, to * (1.0 / l), l) {
            pos = mid + to * ((t - 0.6).max(2.0) / l);
        }
        // stay inside the arena
        pos.x = pos.x.clamp(-22.0, 22.0);
        pos.z = pos.z.clamp(-22.0, 22.0);
        (pos, mid)
    }

    fn update_demo_cam(&mut self, dt: f32) {
        let (pos, look) = self.demo_target();
        if self.demo_cam.0 == V3::ZERO {
            self.demo_cam = (pos, look);
        }
        let k = (dt * 1.2).min(1.0);
        self.demo_cam.0 = self.demo_cam.0.lerp(pos, k);
        self.demo_cam.1 = self.demo_cam.1.lerp(look, (dt * 3.0).min(1.0));
    }

    fn demo_camera(&self, rect: Rect) -> Cam {
        let (pos, look) = self.demo_cam;
        let d = (look - pos).norm();
        let yaw = d.x.atan2(d.z);
        let pitch = d.y.asin();
        // aim a little left so the duel shows to the right of the menu
        Cam::new(pos, yaw - 0.27, pitch, 80.0, rect)
    }

    fn play_camera(&self, game: &Game, rect: Rect) -> (Cam, V3) {
        let l = game.local;
        let f = &game.f[l];
        let alpha = (self.acc / TICK).clamp(0.0, 1.0);
        let body = f.prev_pos.lerp(f.pos, alpha);
        // the camera follows the mouse right away, not at the next game tick
        let (yaw, pitch) = if f.is_bot { (f.yaw, f.pitch) } else { (self.aim_yaw, self.aim_pitch) };
        let (pivot, dir, max_dist) = game.camera_rig(body, yaw, pitch);
        // snap in at once when a wall gets in the way, ease back out smoothly
        let mut d = self.cam_dist.get();
        if max_dist < d {
            d = max_dist;
        } else {
            d += (max_dist - d) * (self.frame_dt * 5.0).min(1.0);
        }
        self.cam_dist.set(d);
        let mut pos = pivot - dir * d;
        if game.shake > 0.0 {
            let s = game.shake * 0.12;
            let t = game.time * 60.0;
            pos += v3((t * 1.3).sin() * s, (t * 1.7).cos() * s, (t * 0.9).sin() * s);
        }
        (Cam::new(pos, yaw, pitch, self.settings.fov, rect), body)
    }
}

// ---------------------------------------------------------------------- held keys

#[derive(Clone, Copy, Default)]
struct KeysHeld {
    w: bool,
    a: bool,
    s: bool,
    d: bool,
    fire: bool,
    guard: bool,
}

thread_local! {
    static KEYS: std::cell::RefCell<KeysHeld> = std::cell::RefCell::new(KeysHeld::default());
}

fn sample_keys(ctx: &egui::Context) {
    let k = ctx.input(|i| KeysHeld {
        w: i.key_down(Key::W) || i.key_down(Key::ArrowUp),
        a: i.key_down(Key::A) || i.key_down(Key::ArrowLeft),
        s: i.key_down(Key::S) || i.key_down(Key::ArrowDown),
        d: i.key_down(Key::D) || i.key_down(Key::ArrowRight),
        fire: i.pointer.primary_down(),
        guard: i.pointer.secondary_down() || i.key_down(Key::C),
    });
    KEYS.with(|c| *c.borrow_mut() = k);
}

/// A name to show: what they typed, or "Player N".
fn clean_name(n: &str, slot: usize) -> String {
    let n: String = n.trim().chars().filter(|c| !c.is_control()).take(16).collect();
    if n.is_empty() { format!("Player {}", slot + 1) } else { n }
}

/// Follow the host's update: who's here, the match, everyone's fighters and events.
fn apply_host_msg(g: &mut Game, m: net::HostMsg) {
    g.local = m.you;
    for e in &m.roster {
        let s = e.slot;
        if !g.f[s].present || g.f[s].name != e.name {
            let owned = s == g.local;
            let keep_bot = owned && g.f[s].is_bot;
            g.mt.mode = m.mode;
            g.add_player(s, &e.name, owned, None, e.team);
            if keep_bot {
                g.f[s].is_bot = true;
                g.bots[s] = Some(bot::Bot::new(0.8, 11 + s as u64));
            }
        }
        g.f[s].team = e.team;
        g.f[s].kills = e.kills;
        g.f[s].deaths = e.deaths;
    }
    for s in 0..g.f.len() {
        if g.f[s].present && s != g.local && !m.roster.iter().any(|e| e.slot == s) {
            g.remove_player(s);
        }
    }
    g.apply_match(m.mode, m.phase, m.timer, m.target, m.team_score, m.number);
    for (s, st) in &m.fighters {
        if *s != g.local && g.f[*s].present {
            net::apply_remote(&mut g.f[*s], st);
        }
    }
    for (origin, ev) in m.events {
        g.handle_remote_event(origin, ev);
    }
}

// ---------------------------------------------------------------------- HUD

fn text_shadow(p: &egui::Painter, pos: Pos2, align: Align2, s: &str, size: f32, color: Color32) {
    let font = FontId::proportional(size);
    p.text(pos + Vec2::new(2.0, 2.0), align, s, font.clone(), Color32::from_black_alpha(170));
    p.text(pos, align, s, font, color);
}

fn bar(p: &egui::Painter, r: Rect, frac: f32, col: Color32) {
    p.rect_filled(r, 3.0, Color32::from_black_alpha(140));
    let mut f = r.shrink(2.0);
    f.set_width(f.width() * frac.clamp(0.0, 1.0));
    p.rect_filled(f, 2.0, col);
}

fn player_color(idx: u8) -> Color32 {
    let c = gpu::PALETTE[(idx as usize) % gpu::PALETTE.len()];
    // brighter than the coat so it reads on screen
    let l = |v: u8| ((v as f32) * 0.55 + 110.0).min(255.0) as u8;
    Color32::from_rgb(l(c[0]), l(c[1]), l(c[2]))
}

fn draw_scoreboard(p: &egui::Painter, rect: Rect, g: &Game) {
    let mut rows: Vec<&Fighter> = g.f.iter().filter(|f| f.present).collect();
    rows.sort_by(|a, b| (a.team, -a.kills, a.deaths).cmp(&(b.team, -b.kills, b.deaths)));
    if g.mt.mode == Mode::Ffa {
        rows.sort_by(|a, b| (-a.kills, a.deaths).cmp(&(-b.kills, b.deaths)));
    }
    let w = 460.0;
    let h = 70.0 + rows.len() as f32 * 28.0 + if g.mt.mode == Mode::Tdm { 30.0 } else { 0.0 };
    let r = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 110.0 + h * 0.5), Vec2::new(w, h));
    p.rect_filled(r, 10.0, Color32::from_rgba_unmultiplied(10, 8, 20, 215));
    p.rect_stroke(r, 10.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 150, 70, 110)), egui::StrokeKind::Inside);
    let x0 = r.left() + 20.0;
    let mut y = r.top() + 16.0;
    text_shadow(p, Pos2::new(x0, y), Align2::LEFT_TOP, &format!("PLAYERS  ({}/8)", rows.len()), 16.0, ACCENT);
    text_shadow(p, Pos2::new(r.right() - 110.0, y), Align2::CENTER_TOP, "KILLS", 13.0, Color32::from_gray(200));
    text_shadow(p, Pos2::new(r.right() - 45.0, y), Align2::CENTER_TOP, "DEATHS", 13.0, Color32::from_gray(200));
    y += 34.0;
    let mut last_team = 255u8;
    for f in rows {
        if g.mt.mode == Mode::Tdm && f.team != last_team {
            last_team = f.team;
            let t = (f.team as usize).min(1);
            text_shadow(p, Pos2::new(x0, y), Align2::LEFT_TOP, &format!("{} TEAM  —  {}", TEAM_NAMES[t].to_uppercase(), g.mt.team_score[t]), 14.0, player_color(t as u8));
            y += 24.0;
        }
        if f.slot == g.local {
            p.rect_filled(Rect::from_min_size(Pos2::new(r.left() + 8.0, y - 3.0), Vec2::new(w - 16.0, 25.0)), 4.0, Color32::from_rgba_unmultiplied(255, 150, 70, 40));
        }
        let mut name = f.name.clone();
        if f.slot == g.local {
            name.push_str("  (you)");
        } else if f.is_bot {
            name.push_str("  (bot)");
        } else if f.slot == 0 && matches!(g.kind, Kind::Client) {
            name.push_str("  (host)");
        }
        p.circle_filled(Pos2::new(x0 + 6.0, y + 9.0), 6.0, player_color(f.color_idx));
        text_shadow(p, Pos2::new(x0 + 20.0, y), Align2::LEFT_TOP, &name, 16.0, Color32::WHITE);
        text_shadow(p, Pos2::new(r.right() - 110.0, y), Align2::CENTER_TOP, &format!("{}", f.kills), 16.0, Color32::WHITE);
        text_shadow(p, Pos2::new(r.right() - 45.0, y), Align2::CENTER_TOP, &format!("{}", f.deaths), 16.0, Color32::from_gray(200));
        y += 28.0;
    }
}

impl App {
    fn draw_hud(&self, p: &egui::Painter, rect: Rect, cam: &Cam, g: &Game) {
        let me = &g.f[g.local];
        let c = rect.center();

        // hurt flash vignette
        if g.hurt_flash > 0.0 {
            let a = (g.hurt_flash * 200.0) as u8;
            let col = Color32::from_rgba_unmultiplied(200, 0, 0, a / 2);
            let w = 60.0;
            p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(rect.width(), w)), 0.0, col);
            p.rect_filled(Rect::from_min_size(Pos2::new(rect.left(), rect.bottom() - w), Vec2::new(rect.width(), w)), 0.0, col);
            p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(w, rect.height())), 0.0, col);
            p.rect_filled(Rect::from_min_size(Pos2::new(rect.right() - w, rect.top()), Vec2::new(w, rect.height())), 0.0, col);
        }
        if me.hp < 30.0 && me.alive {
            let a = ((g.time * 4.0).sin().abs() * 50.0) as u8;
            p.rect_stroke(rect.shrink(4.0), 0.0, Stroke::new(8.0, Color32::from_rgba_unmultiplied(220, 0, 0, a)), egui::StrokeKind::Inside);
        }

        // name tags over everyone you can see
        for other in g.f.iter().filter(|o| o.present && o.slot != g.local) {
            if !(other.alive || other.death_t < 2.0) {
                continue;
            }
            let head = other.pos + v3(0.0, 2.15, 0.0);
            let Some(sp) = cam.project(head) else { continue };
            if !rect.contains(sp) || !g.world.line_clear(cam.pos, head) {
                continue;
            }
            let enemy = g.is_enemy(g.local, other.slot);
            let col = player_color(other.color_idx);
            let label = if enemy { other.name.clone() } else { format!("{} (ally)", other.name) };
            text_shadow(p, sp + Vec2::new(0.0, -12.0), Align2::CENTER_BOTTOM, &label, 15.0, col);
            let r = Rect::from_center_size(sp, Vec2::new(70.0, 8.0));
            bar(p, r, other.hp / MAX_HP, if enemy { Color32::from_rgb(230, 60, 60) } else { Color32::from_rgb(80, 200, 110) });
        }        // damage numbers
        for d in &g.dmg_nums {
            if let Some(sp) = cam.project(d.pos) {
                let a = (d.life / 0.9 * 255.0).clamp(0.0, 255.0) as u8;
                let col = if d.head { Color32::from_rgba_unmultiplied(255, 220, 60, a) } else { Color32::from_rgba_unmultiplied(255, 255, 255, a) };
                text_shadow(p, sp, Align2::CENTER_CENTER, &format!("{}", d.amount), if d.head { 24.0 } else { 18.0 }, col);
            }
        }

        // crosshair
        if me.alive {
            let col = Color32::from_rgba_unmultiplied(255, 255, 255, 220);
            let st = Stroke::new(2.0, col);
            match me.weapon {
                Weapon::Sword => {
                    p.circle_stroke(c, 10.0, st);
                    p.circle_filled(c, 2.0, col);
                }
                Weapon::Shotgun => {
                    let r = 26.0 + me.recoil * 10.0;
                    p.circle_stroke(c, r, Stroke::new(1.5, col));
                    p.circle_filled(c, 2.0, col);
                }
                Weapon::Revolver => {
                    let moving = me.vel.flat().len() > 1.0 || !me.on_ground;
                    let gap = 5.0 + if moving { 7.0 } else { 0.0 } + me.recoil * 8.0;
                    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                        let a = c + Vec2::new(dx * gap, dy * gap);
                        let b = c + Vec2::new(dx * (gap + 9.0), dy * (gap + 9.0));
                        p.line_segment([a + Vec2::new(1.0, 1.0), b + Vec2::new(1.0, 1.0)], Stroke::new(2.0, Color32::from_black_alpha(150)));
                        p.line_segment([a, b], st);
                    }
                }
            }
            if g.hitmarker > 0.0 {
                let a = (g.hitmarker / 0.35 * 255.0).min(255.0) as u8;
                let hc = Color32::from_rgba_unmultiplied(255, 80, 60, a);
                for (dx, dy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                    p.line_segment([c + Vec2::new(dx * 7.0, dy * 7.0), c + Vec2::new(dx * 15.0, dy * 15.0)], Stroke::new(2.5, hc));
                }
            }
        }

        // health / armor
        let base = Pos2::new(rect.left() + 28.0, rect.bottom() - 30.0);
        let panel = Rect::from_min_max(Pos2::new(base.x - 14.0, base.y - 78.0), Pos2::new(base.x + 300.0, base.y + 14.0));
        p.rect_filled(panel, 8.0, Color32::from_black_alpha(110));
        text_shadow(p, Pos2::new(base.x, base.y - 70.0), Align2::LEFT_TOP, &me.name, 16.0, Color32::from_rgb(240, 230, 220));
        let hp_r = Rect::from_min_size(Pos2::new(base.x, base.y - 44.0), Vec2::new(230.0, 18.0));
        bar(p, hp_r, me.hp / MAX_HP, Color32::from_rgb(220, 60, 60));
        text_shadow(p, Pos2::new(hp_r.right() + 10.0, hp_r.center().y), Align2::LEFT_CENTER, &format!("{}", me.hp.max(0.0).ceil() as i32), 18.0, Color32::WHITE);
        let ap_r = Rect::from_min_size(Pos2::new(base.x, base.y - 20.0), Vec2::new(230.0 * 0.6, 12.0));
        bar(p, ap_r, me.ap / MAX_AP, Color32::from_rgb(80, 150, 240));
        text_shadow(p, Pos2::new(ap_r.right() + 10.0, ap_r.center().y), Align2::LEFT_CENTER, &format!("AP {}", me.ap.max(0.0).ceil() as i32), 14.0, Color32::from_rgb(170, 200, 255));

        // weapons
        let wb = Pos2::new(rect.right() - 28.0, rect.bottom() - 30.0);
        let panel = Rect::from_min_max(Pos2::new(wb.x - 300.0, wb.y - 78.0), Pos2::new(wb.x + 14.0, wb.y + 14.0));
        p.rect_filled(panel, 8.0, Color32::from_black_alpha(110));
        let ammo = match me.weapon {
            Weapon::Sword => {
                if me.guard {
                    "GUARDING".to_string()
                } else {
                    "∞".to_string()
                }
            }
            w => {
                if me.reload_t > 0.0 {
                    "RELOADING".to_string()
                } else {
                    format!("{} / {}", me.mag[w as usize], w.mag())
                }
            }
        };
        text_shadow(p, Pos2::new(wb.x, wb.y - 70.0), Align2::RIGHT_TOP, me.weapon.name(), 16.0, ACCENT);
        text_shadow(p, Pos2::new(wb.x, wb.y - 48.0), Align2::RIGHT_TOP, &ammo, 26.0, Color32::WHITE);
        for k in 0..3u8 {
            let r = Rect::from_min_size(Pos2::new(wb.x - 286.0 + k as f32 * 40.0, wb.y - 64.0), Vec2::new(34.0, 34.0));
            let sel = me.weapon as u8 == k;
            p.rect_filled(r, 5.0, if sel { Color32::from_rgba_unmultiplied(255, 150, 70, 170) } else { Color32::from_black_alpha(120) });
            text_shadow(p, r.center(), Align2::CENTER_CENTER, &format!("{}", k + 1), 16.0, Color32::WHITE);
        }
        let gtxt = format!("Grenades {}", me.grenades);
        text_shadow(p, Pos2::new(wb.x - 286.0, wb.y - 10.0), Align2::LEFT_CENTER, &gtxt, 14.0, Color32::from_rgb(220, 220, 180));
        let mut dash = "Dash ready".to_string();
        if me.dash_cd > 0.0 {
            dash = "Dash ...".into();
        }
        text_shadow(p, Pos2::new(wb.x - 160.0, wb.y - 10.0), Align2::LEFT_CENTER, &dash, 14.0, Color32::from_rgb(180, 220, 200));

        // scores + timer
        let top = Pos2::new(c.x, rect.top() + 18.0);
        let sb = Rect::from_center_size(top + Vec2::new(0.0, 22.0), Vec2::new(420.0, 52.0));
        p.rect_filled(sb, 8.0, Color32::from_black_alpha(130));
        let red = player_color(0);
        let blue = player_color(1);
        let tt = match g.mt.phase {
            Phase::Fight => {
                let s = g.mt.timer.max(0.0).ceil() as i32;
                format!("{}:{:02}", s / 60, s % 60)
            }
            Phase::Warmup => "WARM-UP".into(),
            Phase::Countdown => "GET READY".into(),
            Phase::MatchOver => "OVER".into(),
        };
        text_shadow(p, sb.center() + Vec2::new(0.0, -6.0), Align2::CENTER_CENTER, &tt, 18.0, Color32::WHITE);
        text_shadow(p, sb.center() + Vec2::new(0.0, 15.0), Align2::CENTER_CENTER, &format!("{} · first to {}", g.mt.mode.name(), g.mt.target), 11.0, Color32::from_gray(200));
        match g.mt.mode {
            Mode::Tdm => {
                text_shadow(p, sb.left_center() + Vec2::new(14.0, 0.0), Align2::LEFT_CENTER, "RED", 17.0, red);
                text_shadow(p, sb.right_center() - Vec2::new(14.0, 0.0), Align2::RIGHT_CENTER, "BLUE", 17.0, blue);
                text_shadow(p, sb.center() + Vec2::new(-95.0, 0.0), Align2::CENTER_CENTER, &format!("{}", g.mt.team_score[0]), 30.0, red);
                text_shadow(p, sb.center() + Vec2::new(95.0, 0.0), Align2::CENTER_CENTER, &format!("{}", g.mt.team_score[1]), 30.0, blue);
            }
            Mode::Ffa => {
                let leader = g.f.iter().filter(|f| f.present).max_by_key(|f| f.kills);
                text_shadow(p, sb.left_center() + Vec2::new(14.0, -8.0), Align2::LEFT_CENTER, "YOU", 12.0, Color32::from_gray(210));
                text_shadow(p, sb.left_center() + Vec2::new(14.0, 10.0), Align2::LEFT_CENTER, &format!("{} kills", me.kills), 17.0, player_color(me.color_idx));
                if let Some(l) = leader {
                    text_shadow(p, sb.right_center() - Vec2::new(14.0, 8.0), Align2::RIGHT_CENTER, "LEADER", 12.0, Color32::from_gray(210));
                    text_shadow(p, sb.right_center() - Vec2::new(14.0, -10.0), Align2::RIGHT_CENTER, &format!("{} {}", l.name, l.kills), 17.0, player_color(l.color_idx));
                }
            }
        }

        // banner
        if let Some((text, _)) = &g.banner {
            text_shadow(p, Pos2::new(c.x, rect.top() + rect.height() * 0.3), Align2::CENTER_CENTER, text, 46.0, ACCENT);
        }
        if g.mt.phase == Phase::Countdown {
            let n = g.mt.timer.ceil().max(1.0) as i32;
            text_shadow(p, Pos2::new(c.x, rect.top() + rect.height() * 0.42), Align2::CENTER_CENTER, &format!("{n}"), 64.0, Color32::WHITE);
        }
        if g.mt.phase == Phase::Warmup {
            let players = g.present().len();
            let hint = if g.kind == Kind::Host {
                format!("Warm-up · {players} in the lobby · press Enter to start the match")
            } else {
                format!("Warm-up · {players} in the lobby · waiting for the host to start")
            };
            text_shadow(p, Pos2::new(c.x, sb.bottom() + 18.0), Align2::CENTER_TOP, &hint, 16.0, Color32::from_rgb(255, 230, 180));
        }
        if !me.alive && g.mt.phase != Phase::MatchOver {
            let s = me.respawn_t.max(0.0).ceil() as i32;
            text_shadow(p, Pos2::new(c.x, rect.top() + rect.height() * 0.62), Align2::CENTER_CENTER, &format!("You're down - back in {s}"), 20.0, Color32::from_rgb(255, 200, 200));
        }

        // kill feed
        let mut y = rect.top() + 18.0;
        for line in &g.feed {
            let a = (line.life.min(1.0) * 255.0) as u8;
            let fade = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a);
            let font = FontId::proportional(15.0);
            let mut parts: Vec<(String, Color32)> = Vec::new();
            if let Some(t) = &line.text {
                parts.push((t.clone(), Color32::from_rgb(255, 240, 220)));
            } else {
                if let Some((k, kc)) = &line.killer {
                    parts.push((k.clone(), player_color(*kc)));
                    parts.push((format!("  [{}]  ", line.tool), Color32::from_gray(220)));
                } else {
                    parts.push(("✝ ".into(), Color32::from_gray(220)));
                }
                parts.push((line.victim.0.clone(), player_color(line.victim.1)));
            }
            let galleys: Vec<_> = parts.into_iter().map(|(s, col)| p.layout_no_wrap(s, font.clone(), fade(col))).collect();
            let total: f32 = galleys.iter().map(|g| g.size().x).sum();
            let back = Rect::from_min_size(Pos2::new(rect.right() - 24.0 - total, y - 3.0), Vec2::new(total + 12.0, 22.0));
            p.rect_filled(back, 4.0, Color32::from_black_alpha((a as f32 * 0.45) as u8));
            let mut x = rect.right() - 18.0 - total;
            for gl in galleys {
                let w = gl.size().x;
                p.galley(Pos2::new(x, y), gl, Color32::WHITE);
                x += w;
            }
            y += 26.0;
        }

        // player list: hold Tab (always shown in the warm-up and at the end)
        if self.show_scores || g.mt.phase == Phase::MatchOver || (g.mt.phase == Phase::Warmup && g.time < 6.0) {
            draw_scoreboard(p, rect, g);
        }        if self.captured && g.time < 12.0 {
            text_shadow(p, Pos2::new(c.x, rect.bottom() - 18.0), Align2::CENTER_BOTTOM, "Esc: menu   ·   Tab: players   ·   1/2/3 or wheel: weapons   ·   Shift: dash   ·   Space at a wall: wall-run", 14.0, Color32::from_gray(220));
        }
    }

    // ------------------------------------------------------------------ menus

    fn _unused_panel(&self) {}
}

fn panel<R>(ctx: &egui::Context, id: &str, pos: Pos2, width: f32, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        egui::Area::new(egui::Id::unique(id))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(14, 12, 26, 225))
                    .corner_radius(10.0)
                    .inner_margin(20.0)
                    .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 150, 70, 90)))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        f(ui)
                    })
                    .inner
            })
            .inner
}

impl App {
    fn big_button(ui: &mut egui::Ui, text: &str) -> bool {
        ui.add_sized([ui.available_width(), 44.0], egui::Button::new(egui::RichText::new(text).size(19.0))).clicked()
    }

    fn menu_ui(&mut self, ctx: &egui::Context, rect: Rect, page: Page) -> bool {
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::unique("title")));
        let tpos = Pos2::new(rect.left() + 48.0, rect.top() + 40.0);
        p.text(tpos + Vec2::new(3.0, 3.0), Align2::LEFT_TOP, TITLE, FontId::proportional(64.0), Color32::from_black_alpha(200));
        p.text(tpos, Align2::LEFT_TOP, TITLE, FontId::proportional(64.0), ACCENT);
        p.text(tpos + Vec2::new(4.0, 74.0), Align2::LEFT_TOP, "blades · bullets · wall-runs   —   1v1 arena", FontId::proportional(17.0), Color32::from_rgb(235, 220, 210));
        let mut next: Option<Page> = None;
        let mut action: Option<Box<dyn FnOnce(&mut App)>> = None;
        let pos = Pos2::new(rect.left() + 48.0, rect.top() + 150.0);
        let msg = self.menu_msg.clone();
        let mut quit = false;
        panel(ctx, "menu", pos, 380.0, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            if let Some(m) = &msg {
                ui.colored_label(Color32::from_rgb(255, 180, 120), m);
                ui.add_space(4.0);
            }
            match page {
                Page::Main => {
                    ui.horizontal(|ui| {
                        ui.label("Your name");
                        ui.add(egui::TextEdit::singleline(&mut self.settings.name).char_limit(16).desired_width(220.0));
                    });
                    ui.add_space(6.0);
                    if Self::big_button(ui, "Practice vs Bot") {
                        next = Some(Page::Practice);
                    }
                    if Self::big_button(ui, "Host a Match") {
                        next = Some(Page::Host);
                    }
                    if Self::big_button(ui, "Join a Friend") {
                        next = Some(Page::Join);
                    }
                    if Self::big_button(ui, "How to Play") {
                        next = Some(Page::HowTo);
                    }
                    if Self::big_button(ui, "Settings") {
                        next = Some(Page::Settings);
                    }
                    if Self::big_button(ui, "Quit") {
                        quit = true;
                    }
                }
                Page::Practice => {
                    ui.heading("Practice vs Bot");
                    ui.label("Bot difficulty");
                    ui.horizontal(|ui| {
                        for (k, (n, _)) in BOT_LEVELS.iter().enumerate() {
                            ui.selectable_value(&mut self.settings.bot_level, k, *n);
                        }
                    });
                    ui.add(egui::Slider::new(&mut self.settings.rounds, 1..=15).text("rounds to win"));
                    if Self::big_button(ui, "Fight!") {
                        action = Some(Box::new(|app: &mut App| {
                            let lvl = BOT_LEVELS[app.settings.bot_level].1;
                            let name = if app.settings.name.trim().is_empty() { "Player".to_string() } else { app.settings.name.clone() };
                            let g = Game::new(Kind::Offline, 0, [&name, "Bot"], lvl, app.settings.rounds);
                            app.settings.save();
                            app.start_game(g, None);
                        }));
                    }
                    if Self::big_button(ui, "Back") {
                        next = Some(Page::Main);
                    }
                }
                Page::Host => {
                    ui.heading("Host a Match");
                    ui.label("Your friend joins using your address. On the same Wi-Fi / network use the local address. Over the internet, forward the UDP port on your router (or use a virtual LAN app like Tailscale, ZeroTier or Radmin VPN and share that address).");
                    ui.horizontal(|ui| {
                        ui.label("Port (UDP)");
                        ui.add(egui::TextEdit::singleline(&mut self.settings.port).desired_width(90.0));
                    });
                    ui.add(egui::Slider::new(&mut self.settings.rounds, 1..=15).text("rounds to win"));
                    if Self::big_button(ui, "Start Hosting") {
                        action = Some(Box::new(|app: &mut App| {
                            let port: u16 = app.settings.port.trim().parse().unwrap_or(net::DEFAULT_PORT);
                            match Net::host(port) {
                                Ok(n) => {
                                    app.settings.save();
                                    app.menu_msg = None;
                                    app.screen = Screen::Lobby { net: n, waiting_since: 0.0 };
                                }
                                Err(e) => app.menu_msg = Some(e),
                            }
                        }));
                    }
                    if Self::big_button(ui, "Back") {
                        next = Some(Page::Main);
                    }
                }
                Page::Join => {
                    ui.heading("Join a Friend");
                    ui.label("Type the address your friend sees on their Host screen, for example 192.168.1.20 or 192.168.1.20:27960");
                    ui.add(egui::TextEdit::singleline(&mut self.settings.join_addr).hint_text("address[:port]").desired_width(f32::INFINITY));
                    if Self::big_button(ui, "Connect") {
                        action = Some(Box::new(|app: &mut App| match Net::join(&app.settings.join_addr) {
                            Ok(n) => {
                                app.settings.save();
                                app.menu_msg = None;
                                app.screen = Screen::Lobby { net: n, waiting_since: 0.0 };
                            }
                            Err(e) => app.menu_msg = Some(e),
                        }));
                    }
                    if Self::big_button(ui, "Back") {
                        next = Some(Page::Main);
                    }
                }
                Page::Settings => {
                    ui.heading("Settings");
                    ui.add(egui::Slider::new(&mut self.settings.sens, 0.2..=3.0).text("mouse sensitivity"));
                    ui.add(egui::Slider::new(&mut self.settings.fov, 60.0..=110.0).text("field of view"));
                    ui.add(egui::Slider::new(&mut self.settings.volume, 0.0..=1.0).text("volume"));
                    if self.audio.is_none() && self.screenshot.is_none() {
                        ui.label("(no sound device found)");
                    }
                    if Self::big_button(ui, "Back") {
                        self.settings.save();
                        next = Some(Page::Main);
                    }
                }
                Page::HowTo => {
                    ui.heading("How to Play");
                    egui::Grid::new("keys").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                        for (k, v) in [
                            ("W A S D", "move"),
                            ("Mouse", "aim"),
                            ("Left click", "shoot / slash (click again for a 3-hit combo)"),
                            ("Right click", "guard with the katana (blocks bullets & slashes)"),
                            ("Space", "jump"),
                            ("Space at a wall", "while in the air: run along it, or up it if you face it"),
                            ("Space on a wall", "kick off the wall"),
                            ("Shift / Q / E", "dash (once more in the air)"),
                            ("Double-tap W A S D", "dodge in that direction"),
                            ("Cancel", "after a slash lands: dash, jump or guard to cut it short"),
                            ("Butterfly", "slash, right-click guard, slash, guard... in rhythm"),
                            ("Launch", "3rd slash of a combo, or dash + slash: knocks them up"),
                            ("Air combo", "slash in the air to juggle; 3rd air slash smashes down"),
                            ("1  2  3 / wheel", "katana, scattergun, revolver"),
                            ("R", "reload"),
                            ("G / F", "throw a grenade (2 per round)"),
                            ("Esc", "pause menu"),
                        ] {
                            ui.colored_label(ACCENT, k);
                            ui.label(v);
                            ui.end_row();
                        }
                    });
                    ui.label("The third katana hit is heavy: it breaks guards and launches. Headshots with the revolver do extra damage. Win rounds by taking your rival down.");
                    if Self::big_button(ui, "Back") {
                        next = Some(Page::Main);
                    }
                }
            }
        });
        if let Some(n) = next {
            self.menu_msg = None;
            self.screen = Screen::Menu(n);
        }
        if let Some(a) = action {
            a(self);
        }
        if quit {
            self.settings.save();
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        false
    }

    fn lobby_ui(&mut self, ctx: &egui::Context, rect: Rect) -> bool {
        let Screen::Lobby { net, waiting_since } = &self.screen else { return false };
        let is_host = net.is_host;
        let port = net.port;
        let t = *waiting_since;
        let mut cancel = false;
        let ip = if is_host { self.lan_ip.clone() } else { None };
        panel(ctx, "lobby", Pos2::new(rect.left() + 48.0, rect.top() + 150.0), 380.0, |ui| {
            if is_host {
                ui.heading("Waiting for your friend…");
                ui.label("Tell them to press Join a Friend and type:");
                let addr = match &ip {
                    Some(ip) => format!("{ip}:{port}"),
                    None => format!("<your address>:{port}"),
                };
                ui.label(egui::RichText::new(&addr).size(26.0).color(ACCENT));
                if ui.button("Copy address").clicked() {
                    ui.ctx().copy_text(addr.clone());
                }
                ui.label("That's your address on this network. For friends over the internet, forward this UDP port on your router and give them your public address, or use a virtual LAN app.");
                ui.label("If Windows asks about the firewall, allow access.");
            } else {
                ui.heading("Connecting…");
                ui.label(format!("{:.0}s", t));
            }
            let dots = ".".repeat(((t * 2.0) as usize % 4) + 1);
            ui.label(dots);
            if App::big_button(ui, "Cancel") {
                cancel = true;
            }
        });
        cancel
    }

    fn play_overlay_ui(&mut self, ctx: &egui::Context, rect: Rect) -> bool {
        let (kind, phase, is_auth) = match &self.screen {
            Screen::Play { game, .. } => (game.kind, game.round.phase, game.is_authority()),
            _ => return false,
        };
        let mut leave = false;
        let center = Pos2::new(rect.center().x - 200.0, rect.center().y - 60.0);
        if let Some(msg) = self.disconnected.clone() {
            panel(ctx, "disc", center, 360.0, |ui| {
                ui.heading("Match ended");
                ui.label(msg);
                if App::big_button(ui, "Back to menu") {
                    leave = true;
                }
            });
            return leave;
        }
        if phase == Phase::MatchOver {
            let mut rematch = false;
            panel(ctx, "over", Pos2::new(center.x, center.y + 60.0), 360.0, |ui| {
                if is_auth {
                    if App::big_button(ui, "Rematch") {
                        rematch = true;
                    }
                } else {
                    ui.label("Waiting for the host to start a rematch…");
                }
                if App::big_button(ui, "Leave") {
                    leave = true;
                }
            });
            if rematch {
                if let Screen::Play { game, .. } = &mut self.screen {
                    game.rematch();
                }
            }
            return leave;
        }
        if self.paused {
            let mut resume = false;
            panel(ctx, "pause", center, 360.0, |ui| {
                ui.heading("Paused");
                if kind != Kind::Offline {
                    ui.label("The match keeps going while this menu is open!");
                }
                ui.add(egui::Slider::new(&mut self.settings.sens, 0.2..=3.0).text("mouse sensitivity"));
                ui.add(egui::Slider::new(&mut self.settings.fov, 60.0..=110.0).text("field of view"));
                ui.add(egui::Slider::new(&mut self.settings.volume, 0.0..=1.0).text("volume"));
                if App::big_button(ui, "Resume") {
                    resume = true;
                }
                if App::big_button(ui, "Leave match") {
                    leave = true;
                }
            });
            if resume {
                self.paused = false;
                self.settings.save();
            }
        }
        leave
    }
}

// The offline match pauses the simulation while the pause menu is open.
impl App {
    #[allow(dead_code)]
    fn sim_paused(&self) -> bool {
        match &self.screen {
            Screen::Play { game, .. } => self.paused && game.kind == Kind::Offline,
            _ => false,
        }
    }
}
