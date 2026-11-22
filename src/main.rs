#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod anim;
mod audio;
mod bot;
mod career;
mod character;
mod chars;
mod env;
mod game;
mod gpu;
mod math;
mod net;
mod pad;
mod platform;
mod render;
mod watchdog;
mod world;

use eframe::egui::{self, Align2, Color32, FontId, Key, Pos2, Rect, Stroke, Vec2, ViewportCommand};
use game::*;
use math::*;
use net::{Incoming, Net};
use platform::{firewall, steam};
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
    /// Which map to play (Practice and hosting): world::MapId as a number.
    map: u8,
    /// Hosting: fill the lobby with bots up to this many fighters (0 = no bots).
    fill_bots: usize,
    /// Best Gauntlet score on this computer.
    gauntlet_best: u32,
    bots: usize,
    port: String,
    join_addr: String,
    volume: f32,
    music_volume: f32,
    /// Host through Steam when it's running (friends join from Steam, no port forwarding), and
    /// whether the lobby is listed publicly or only for friends.
    use_steam: bool,
    steam_public: bool,
    /// Music on at all, and separately in the menus and in matches.
    music_on: bool,
    menu_music: bool,
    match_music: bool,
    loadout: Loadout,
    binds: [Bind; N_ACTIONS],
    /// Graphics preset (index into gpu::Quality::PRESETS).
    quality: usize,
    fullscreen: bool,
    /// Frames per second limit (0 = no limit).
    fps_cap: u32,
    vsync: bool,
    /// 0 = changes with the weapon, then cross, dot, circle, cross + dot, T.
    cross_style: u8,
    cross_color: [u8; 3],
    cross_size: f32,
    /// The crosshair's dot turns red over an enemy.
    cross_red: bool,
    /// Servers joined before, newest first.
    recent: Vec<RecentServer>,
    /// Network test: pretend the connection has extra lag and loses packets.
    net_test: bool,
    net_lag: f32,
    net_loss: f32,
    /// Controller: look speed, invert up/down, aim assist (the aim slows over an enemy).
    pad_sens: f32,
    pad_invert: bool,
    aim_assist: bool,
    /// Text and HUD size (index into UI_SCALES): bigger for a TV across the room or a handheld.
    ui_scale: usize,
    /// Draw the 3D picture a little smaller when frames run late, to hold the frame rate.
    dyn_res: bool,
    /// Graphics effects (each preset sets them; each can be switched on its own).
    post: gpu::Fx,
}

/// Interface sizes: (name, zoom).
const UI_SCALES: [(&str, f32); 3] = [("Normal", 1.0), ("Couch / TV", 1.3), ("Big TV", 1.6)];

#[derive(Clone, Debug, PartialEq)]
struct RecentServer {
    addr: String,
    host: String,
    mode: String,
    /// When it was last joined (seconds since 1970).
    when: u64,
}

const MAX_RECENT: usize = 6;

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn ago(when: u64) -> String {
    let d = now_secs().saturating_sub(when);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86_399 => format!("{} h ago", d / 3600),
        _ => format!("{} days ago", d / 86_400),
    }
}

// ---------------------------------------------------------------------- controls

/// A key, mouse button or modifier an action is bound to.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Bind {
    Key(Key),
    Mouse(u8),
    Shift,
    Ctrl,
    Alt,
    Unbound,
}

const N_ACTIONS: usize = 20;
const A_FWD: usize = 0;
const A_BACK: usize = 1;
const A_LEFT: usize = 2;
const A_RIGHT: usize = 3;
const A_JUMP: usize = 4;
const A_DASH: usize = 5;
const A_DASH2: usize = 6;
const A_FIRE: usize = 7;
const A_GUARD: usize = 8;
const A_RELOAD: usize = 9;
const A_ITEM: usize = 10;
const A_MELEE: usize = 11;
const A_GUN1: usize = 12;
const A_GUN2: usize = 13;
const A_SCORES: usize = 14;
const A_CHAT: usize = 15;
const A_SLIDE: usize = 16;
const A_MUSIC: usize = 17;
const A_KICK: usize = 18;
const A_FINISH: usize = 19;

/// (saved id, what it does)
const ACTIONS: [(&str, &str); N_ACTIONS] = [
    ("fwd", "Move forward"),
    ("back", "Move back"),
    ("left", "Move left"),
    ("right", "Move right"),
    ("jump", "Jump / wall-run / wall-jump"),
    ("dash", "Dash"),
    ("dash2", "Dash (second key)"),
    ("fire", "Shoot / slash"),
    ("guard", "Guard (sword)"),
    ("reload", "Reload"),
    ("item", "Use item"),
    ("melee", "Melee weapon"),
    ("gun1", "Gun 1"),
    ("gun2", "Gun 2"),
    ("scores", "Player list (hold)"),
    ("chat", "Chat"),
    ("slide", "Slide (while running)"),
    ("music", "Music on / off"),
    ("kick", "Kick"),
    ("finish", "Finisher"),
];

fn default_binds() -> [Bind; N_ACTIONS] {
    use Bind::*;
    [
        Key(egui::Key::W),
        Key(egui::Key::S),
        Key(egui::Key::A),
        Key(egui::Key::D),
        Key(egui::Key::Space),
        Shift,
        Key(egui::Key::Q),
        Mouse(0),
        Mouse(1),
        Key(egui::Key::R),
        Key(egui::Key::G),
        Key(egui::Key::Num1),
        Key(egui::Key::Num2),
        Key(egui::Key::Num3),
        Key(egui::Key::Tab),
        Key(egui::Key::T),
        Key(egui::Key::C),
        Key(egui::Key::M),
        Key(egui::Key::F),
        Key(egui::Key::E),
    ]
}

fn mouse_button(n: u8) -> egui::PointerButton {
    match n {
        0 => egui::PointerButton::Primary,
        1 => egui::PointerButton::Secondary,
        2 => egui::PointerButton::Middle,
        3 => egui::PointerButton::Extra1,
        _ => egui::PointerButton::Extra2,
    }
}

impl Bind {
    fn name(self) -> String {
        match self {
            Bind::Key(k) => k.name().to_string(),
            Bind::Mouse(0) => "Left click".into(),
            Bind::Mouse(1) => "Right click".into(),
            Bind::Mouse(2) => "Middle click".into(),
            Bind::Mouse(n) => format!("Mouse {}", n + 1),
            Bind::Shift => "Shift".into(),
            Bind::Ctrl => "Ctrl".into(),
            Bind::Alt => "Alt".into(),
            Bind::Unbound => "-".into(),
        }
    }
    /// The saved form.
    fn id(self) -> String {
        match self {
            Bind::Key(k) => format!("key:{}", k.name()),
            Bind::Mouse(n) => format!("mouse:{n}"),
            Bind::Shift => "shift".into(),
            Bind::Ctrl => "ctrl".into(),
            Bind::Alt => "alt".into(),
            Bind::Unbound => "none".into(),
        }
    }
    fn parse(s: &str) -> Option<Bind> {
        let s = s.trim();
        if let Some(k) = s.strip_prefix("key:") {
            return Key::from_name(k).map(Bind::Key);
        }
        if let Some(n) = s.strip_prefix("mouse:") {
            return n.parse::<u8>().ok().filter(|n| *n < 5).map(Bind::Mouse);
        }
        match s {
            "shift" => Some(Bind::Shift),
            "ctrl" => Some(Bind::Ctrl),
            "alt" => Some(Bind::Alt),
            "none" => Some(Bind::Unbound),
            _ => None,
        }
    }
    fn down(self, i: &egui::InputState) -> bool {
        match self {
            Bind::Key(k) => i.key_down(k),
            Bind::Mouse(n) => i.pointer.button_down(mouse_button(n)),
            Bind::Shift => i.modifiers.shift,
            Bind::Ctrl => i.modifiers.ctrl,
            Bind::Alt => i.modifiers.alt,
            Bind::Unbound => false,
        }
    }
    /// Pressed this frame (`prev` = the modifiers held last frame).
    fn tapped(self, i: &egui::InputState, prev: egui::Modifiers) -> bool {
        match self {
            Bind::Key(k) => i.events.iter().any(|e| matches!(e, egui::Event::Key { key, pressed: true, repeat: false, .. } if *key == k)),
            Bind::Mouse(n) => i.pointer.button_pressed(mouse_button(n)),
            Bind::Shift => i.modifiers.shift && !prev.shift,
            Bind::Ctrl => i.modifiers.ctrl && !prev.ctrl,
            Bind::Alt => i.modifiers.alt && !prev.alt,
            Bind::Unbound => false,
        }
    }
}

const CROSS_STYLES: [&str; 6] = ["Auto (real spread)", "Cross", "Dot", "Circle", "Cross + dot", "T-shape"];
const FPS_CAPS: [u32; 7] = [0, 30, 60, 120, 144, 165, 240];

impl Default for Settings {
    fn default() -> Self {
        Settings {
            name: "Player".into(),
            sens: 1.0,
            fov: 74.0,
            bot_level: 1,
            rounds: 15,
            mode: 0,
            map: 0,
            fill_bots: 0,
            gauntlet_best: 0,
            bots: 3,
            port: net::DEFAULT_PORT.to_string(),
            join_addr: String::new(),
            volume: 0.7,
            music_volume: 0.3,
            music_on: true,
            menu_music: true,
            match_music: true,
            use_steam: true,
            steam_public: true,
            loadout: Loadout::default(),
            binds: default_binds(),
            quality: 2,
            fullscreen: false,
            fps_cap: 0,
            vsync: true,
            cross_style: 0,
            cross_color: [255, 255, 255],
            cross_size: 0.8,
            cross_red: true,
            recent: Vec::new(),
            net_test: false,
            net_lag: 100.0,
            net_loss: 5.0,
            pad_sens: 1.0,
            pad_invert: false,
            aim_assist: true,
            ui_scale: 0,
            dyn_res: false,
            post: gpu::Quality::PRESETS[2].1.post,
        }
    }
}

/// Saved tuning presets live in a folder next to the settings.
fn preset_dir() -> Option<PathBuf> {
    Some(settings_path()?.parent()?.join("presets"))
}

/// "Apply as new defaults" writes here; it's read when the game starts.
fn tuning_defaults_path() -> Option<PathBuf> {
    Some(settings_path()?.parent()?.join("tuning_defaults.txt"))
}

fn load_tuning_defaults() -> Tuning {
    tuning_defaults_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| Tuning::from_text(&t)).unwrap_or_default()
}

/// A preset name made safe to use as a file name.
fn preset_file_name(name: &str) -> String {
    let s: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')).take(40).collect();
    let s = s.trim().to_string();
    if s.is_empty() { "preset".into() } else { s }
}

/// The saved presets' names, sorted.
fn preset_names() -> Vec<String> {
    let Some(dir) = preset_dir() else { return Vec::new() };
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().to_string())).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// A preset file: the tuning lines plus the field of view.
fn preset_text(t: &Tuning, fov: f32) -> String {
    format!("{}fov={}\n", t.to_text(), fov)
}

fn preset_fov(text: &str) -> Option<f32> {
    text.lines().find_map(|l| l.strip_prefix("fov=")).and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| v.is_finite()).map(|v| v.clamp(60.0, 110.0))
}

fn settings_path() -> Option<PathBuf> {
    platform::data_file("settings.txt")
}

impl Settings {
    fn load() -> Settings {
        let mut s = Settings::default();
        let mut aim_v = 0u32;
        let mut saved = false;
        let mut fx_keys: Vec<(String, bool)> = Vec::new();
        if let Some(p) = settings_path() {
            if let Ok(text) = std::fs::read_to_string(p) {
                saved = true;
                for line in text.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        let v = v.trim();
                        match k.trim() {
                            "name" if !v.is_empty() => s.name = v.chars().take(16).collect(),
                            "sens" => s.sens = v.parse().unwrap_or(s.sens),
                            "fov" => s.fov = v.parse().unwrap_or(s.fov),
                            "bot" => s.bot_level = v.parse().unwrap_or(s.bot_level).min(3),
                            "rounds" => s.rounds = v.parse().unwrap_or(s.rounds).clamp(1, 50),
                            "mode" => s.mode = v.parse().unwrap_or(0u8).min(5),
                            "map" => s.map = v.parse().unwrap_or(0u8).min(2),
                            "fill_bots" => s.fill_bots = v.parse().unwrap_or(0usize).min(8),
                            "gauntlet_best" => s.gauntlet_best = v.parse().unwrap_or(0u32),
                            "bots" => s.bots = v.parse().unwrap_or(3usize).clamp(1, 7),
                            "port" => s.port = v.to_string(),
                            "join" => s.join_addr = v.to_string(),
                            "volume" => s.volume = v.parse().unwrap_or(s.volume),
                            "music_volume" => s.music_volume = v.parse().unwrap_or(0.3f32).clamp(0.0, 1.0),
                            "music_on" => s.music_on = v != "0",
                            "use_steam" => s.use_steam = v != "0",
                            "steam_public" => s.steam_public = v != "0",
                            "menu_music" => s.menu_music = v != "0",
                            "match_music" => s.match_music = v != "0",
                            "loadout" => {
                                let b: Vec<u8> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                                if b.len() == 4 {
                                    s.loadout = Loadout::from_bytes([b[0], b[1], b[2], b[3]]);
                                }
                            }
                            "quality" => s.quality = v.parse().unwrap_or(2usize).min(gpu::Quality::PRESETS.len() - 1),
                            "fullscreen" => s.fullscreen = v == "1",
                            "fps_cap" => s.fps_cap = v.parse().unwrap_or(0u32).min(1000),
                            "vsync" => s.vsync = v != "0",
                            "cross_style" => s.cross_style = v.parse().unwrap_or(0u8).min(CROSS_STYLES.len() as u8 - 1),
                            "net_test" => s.net_test = v == "1",
                            "net_lag" => s.net_lag = v.parse().unwrap_or(100.0f32).clamp(0.0, 500.0),
                            "net_loss" => s.net_loss = v.parse().unwrap_or(5.0f32).clamp(0.0, 30.0),
                            "cross_size" => s.cross_size = v.parse().unwrap_or(0.8f32).clamp(0.5, 2.0),
                            "cross_red" => s.cross_red = v != "0",
                            "pad_sens" => s.pad_sens = v.parse().unwrap_or(1.0f32).clamp(0.2, 3.0),
                            "pad_invert" => s.pad_invert = v == "1",
                            "aim_assist" => s.aim_assist = v != "0",
                            "ui_scale" => s.ui_scale = v.parse().unwrap_or(0usize).min(UI_SCALES.len() - 1),
                            "dyn_res" => s.dyn_res = v == "1",
                            k if k.starts_with("fx_") => fx_keys.push((k.to_string(), v != "0")),
                            "aim_v" => aim_v = v.parse().unwrap_or(0),
                            "cross_color" => {
                                let b: Vec<u8> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                                if b.len() == 3 {
                                    s.cross_color = [b[0], b[1], b[2]];
                                }
                            }
                            "recent" => {
                                let parts: Vec<&str> = v.split('|').collect();
                                if parts.len() == 4 && !parts[0].is_empty() && s.recent.len() < MAX_RECENT {
                                    s.recent.push(RecentServer { addr: parts[0].into(), host: parts[1].into(), mode: parts[2].into(), when: parts[3].parse().unwrap_or(0) });
                                }
                            }
                            k if k.starts_with("bind.") => {
                                let id = &k[5..];
                                if let (Some(a), Some(b)) = (ACTIONS.iter().position(|(n, _)| *n == id), Bind::parse(v)) {
                                    s.binds[a] = b;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        // settings saved before the aiming update: closer default view, smaller crosshair
        if saved && aim_v < 2 {
            if (s.fov - 80.0).abs() < 0.01 {
                s.fov = 74.0;
            }
            if (s.cross_size - 1.0).abs() < 0.01 {
                s.cross_size = 0.8;
            }
        }
        // effects: the preset's own, then whatever was switched by hand
        s.post = gpu::Quality::PRESETS[s.quality.min(gpu::Quality::PRESETS.len() - 1)].1.post;
        for (k, on) in fx_keys {
            let f = &mut s.post;
            match k.as_str() {
                "fx_soft_shadows" => f.soft_shadows = on,
                "fx_ssao" => f.ssao = on,
                "fx_bloom" => f.bloom = on,
                "fx_rays" => f.rays = on,
                "fx_grading" => f.grading = on,
                "fx_vignette" => f.vignette = on,
                _ => {}
            }
        }
        // the first start on a Steam Deck (or another handheld / TV device): big text, full screen,
        // the Deck graphics preset, a steady 60 fps
        if !saved && platform::system().couch() {
            s.couch_defaults();
        }
        // settings saved before the playtest update: quieter music by default
        if saved && aim_v < 3 && (s.music_volume - 0.5).abs() < 0.01 {
            s.music_volume = 0.3;
        }
        s
    }
    /// Settings for a handheld or a TV: big text, full screen, the Steam Deck graphics preset,
    /// 60 fps held by dynamic resolution.
    fn couch_defaults(&mut self) {
        self.ui_scale = 1;
        self.fullscreen = true;
        self.quality = gpu::Quality::DECK;
        self.post = gpu::Quality::PRESETS[gpu::Quality::DECK].1.post;
        self.fps_cap = 60;
        self.vsync = true;
        self.dyn_res = true;
    }

    fn save(&self) {
        if let Some(p) = settings_path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let mut text = format!(
                "name={}\nsens={}\nfov={}\nbot={}\nrounds={}\nmode={}\nmap={}\nfill_bots={}\ngauntlet_best={}\nbots={}\nport={}\njoin={}\nvolume={}\nloadout={},{},{},{}\n",
                self.name, self.sens, self.fov, self.bot_level, self.rounds, self.mode, self.map, self.fill_bots, self.gauntlet_best, self.bots, self.port, self.join_addr, self.volume,
                self.loadout.melee as u8, self.loadout.gun1 as u8, self.loadout.gun2 as u8, self.loadout.to_bytes()[3]
            );
            let [r, g, b] = self.cross_color;
            text += &format!(
                "quality={}\nfullscreen={}\nfps_cap={}\nvsync={}\ncross_style={}\ncross_size={}\ncross_color={r},{g},{b}\nnet_test={}\nnet_lag={}\nnet_loss={}\nmusic_volume={}\ncross_red={}\nmusic_on={}\nmenu_music={}\nmatch_music={}\nuse_steam={}\nsteam_public={}\naim_v=3\n",
                self.quality, self.fullscreen as u8, self.fps_cap, self.vsync as u8, self.cross_style, self.cross_size, self.net_test as u8, self.net_lag, self.net_loss, self.music_volume, self.cross_red as u8, self.music_on as u8, self.menu_music as u8, self.match_music as u8, self.use_steam as u8, self.steam_public as u8
            );
            text += &format!("pad_sens={}\npad_invert={}\naim_assist={}\nui_scale={}\ndyn_res={}\n", self.pad_sens, self.pad_invert as u8, self.aim_assist as u8, self.ui_scale, self.dyn_res as u8);
            let f = self.post;
            text += &format!(
                "fx_soft_shadows={}\nfx_ssao={}\nfx_bloom={}\nfx_rays={}\nfx_grading={}\nfx_vignette={}\n",
                f.soft_shadows as u8, f.ssao as u8, f.bloom as u8, f.rays as u8, f.grading as u8, f.vignette as u8
            );
            for (k, (id, _)) in ACTIONS.iter().enumerate() {
                text += &format!("bind.{id}={}\n", self.binds[k].id());
            }
            let clean = |s: &str| s.replace(['|', '\n', '\r'], " ");
            for r in &self.recent {
                text += &format!("recent={}|{}|{}|{}\n", clean(&r.addr), clean(&r.host), clean(&r.mode), r.when);
            }
            let _ = platform::save_file(&p, &text);
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
    /// Steam lobby browser.
    Browse,
    /// Career stats, finishers included.
    Stats,
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
    item: bool,
    kick: bool,
    finish: bool,
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
    chat_open: bool,
    chat_text: String,
    chat_send: Option<String>,
    vote_request: bool,
    ready_request: Option<Loadout>,
    test_chat_done: bool,
    paused: bool,
    captured: bool,
    prev_mods: egui::Modifiers,
    /// Waiting for a key for this action (and the frame it started).
    rebinding: Option<(usize, u32)>,
    settings_tab: u8,
    pause_settings: bool,
    icon_tex: Option<egui::TextureHandle>,
    frame_start: Option<std::time::Instant>,
    shown_fullscreen: Option<bool>,
    fps: f32,
    applied_vsync: Option<bool>,
    show_debug: bool,
    /// This computer's internet address, looked up once (None while asking).
    public_ip: Arc<Mutex<Option<Result<String, String>>>>,
    ip_lookup_started: bool,
    ip_announced: bool,
    /// Windows Firewall: what the last check found (None while checking), and the fix's result.
    fw_status: Arc<Mutex<Option<firewall::FwStatus>>>,
    fw_started: bool,
    fw_note: Arc<Mutex<Option<String>>>,
    disconnected: Option<String>,
    demo_t: f32,
    audio: Option<audio::Audio>,
    lan_ip: Option<String>,
    demo_cam: (V3, V3),
    autoplay: bool,
    last_tap: Option<(Key, f64)>,
    cam_dist: std::cell::Cell<f32>,
    frame_dt: f32,
    fov_kick: std::cell::Cell<f32>,
    /// The crosshair's drawn spread (pixels), eased toward the real one.
    cross_gap: std::cell::Cell<f32>,
    /// Aim-down-sights zoom (1 = none), eased.
    ads_zoom: std::cell::Cell<f32>,
    wall_t: f32,
    train_step: i32,
    /// Recoil still to settle back out of the aim.
    kick_back: f32,
    /// "Next track" presses (steps through the tracks), and a short note on screen.
    track_skip: usize,
    music_note: Option<(&'static str, f32)>,
    /// Steam, when it's running (None = direct IP only).
    steam: Option<steam::Steam>,
    browse: steam::BrowserFilter,
    /// A Steam lobby to join as soon as we can (from "Join game" or the command line).
    pending_lobby: Option<u64>,
    steam_tick: f32,
    browse_asked: bool,
    /// Test switch DUEL_STEAM_SELFTEST: step and clock of the live Steam check.
    selftest: u8,
    selftest_t: f32,
    /// Developer tuning panel (F4): the live values, whether it's open, preset name, last message.
    tune: Tuning,
    show_tuning: bool,
    preset_name: String,
    tune_msg: Option<String>,
    /// Where the smoothed camera is (camera smoothing).
    cam_smooth_pos: std::cell::Cell<Option<V3>>,
    /// Spring follow speed, the wall-run roll and how wide the speed FOV is (smoothed).
    cam_spring_vel: std::cell::Cell<V3>,
    /// How far the drawn camera sits from the plain one (for shots to follow the crosshair).
    cam_view_offset: std::cell::Cell<V3>,
    /// Career stats on this computer.
    career: career::Career,
    cam_roll: std::cell::Cell<f32>,
    fov_speed: std::cell::Cell<f32>,
    /// Notices when the game loop stalls and writes a hang report.
    watchdog: watchdog::Watchdog,
    /// Sounds played last frame (for a hang report).
    last_sounds: Vec<String>,
    /// Controllers: the hardware, this frame's state, menu moves, and whether the controller is
    /// what's being played with (prompts show its buttons then).
    pads: pad::Pads,
    pad: pad::PadState,
    pad_nav: pad::NavRepeat,
    pad_active: bool,
    /// Held controller movement and buttons for the next game input.
    pad_held: PadHeld,
    /// Quick chat open (D-pad down), with the line picked, and whether the stick is still pushed.
    quick_chat: Option<usize>,
    qc_stick_out: bool,
    /// LB / RB in a menu with tabs: move this many tabs.
    pad_tab: i32,
    /// Dynamic resolution (holds the frame rate).
    dynres: gpu::DynRes,
}

/// Controller movement and held buttons, merged into the game input.
#[derive(Clone, Copy, Default)]
struct PadHeld {
    fwd: f32,
    right: f32,
    fire: bool,
    guard: bool,
    crouch: bool,
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
            chat_open: false,
            chat_text: String::new(),
            chat_send: None,
            vote_request: false,
            ready_request: None,
            test_chat_done: false,
            paused: false,
            captured: false,
            prev_mods: egui::Modifiers::NONE,
            rebinding: None,
            settings_tab: 0,
            pause_settings: false,
            icon_tex: None,
            frame_start: None,
            shown_fullscreen: None,
            fps: 60.0,
            applied_vsync: None,
            show_debug: false,
            public_ip: Arc::new(Mutex::new(None)),
            ip_lookup_started: false,
            ip_announced: false,
            fw_status: Arc::new(Mutex::new(None)),
            fw_started: false,
            fw_note: Arc::new(Mutex::new(None)),
            disconnected: None,
            demo_t: 0.0,
            audio,
            lan_ip: net::local_ip(),
            demo_cam: (V3::ZERO, V3::ZERO),
            autoplay: false,
            last_tap: None,
            cam_dist: std::cell::Cell::new(5.0),
            frame_dt: 0.016,
            fov_kick: std::cell::Cell::new(0.0),
            cross_gap: std::cell::Cell::new(0.0),
            ads_zoom: std::cell::Cell::new(1.0),
            wall_t: 0.0,
            train_step: 0,
            kick_back: 0.0,
            track_skip: 0,
            music_note: None,
            steam: None,
            browse: steam::BrowserFilter::default(),
            pending_lobby: steam::lobby_from_args(&std::env::args().collect::<Vec<_>>()),
            steam_tick: 0.0,
            browse_asked: false,
            selftest: 0,
            selftest_t: 0.0,
            tune: load_tuning_defaults(),
            show_tuning: false,
            preset_name: "my preset".into(),
            tune_msg: None,
            cam_smooth_pos: std::cell::Cell::new(None),
            cam_spring_vel: std::cell::Cell::new(V3::ZERO),
            cam_view_offset: std::cell::Cell::new(V3::ZERO),
            career: career::Career::load(),
            cam_roll: std::cell::Cell::new(0.0),
            fov_speed: std::cell::Cell::new(0.0),
            watchdog: watchdog::Watchdog::start(std::env::var_os("DUEL_HANG_REPORT").map(PathBuf::from).or_else(watchdog::report_path)),
            last_sounds: Vec::new(),
            pads: pad::Pads::new(),
            pad: pad::PadState::default(),
            pad_nav: pad::NavRepeat::default(),
            pad_active: false,
            pad_held: PadHeld::default(),
            quick_chat: None,
            qc_stick_out: false,
            pad_tab: 0,
            dynres: gpu::DynRes::default(),
        };
        // Steam (when it's running); tests stay on direct IP
        if (!app.autoplay && app.screenshot.is_none()) || std::env::var_os("DUEL_STEAM_PROBE").is_some() {
            app.steam = steam::Steam::start();
            if std::env::var_os("DUEL_STEAM_PROBE").is_some() {
                eprintln!("steam: {}", match &app.steam { Some(s) => format!("running as {}", s.name), None => "not available, using direct IP".into() });
            }
            if std::env::var_os("DUEL_STEAM_SELFTEST").is_some() {
                if let Some(st) = app.steam.as_mut() {
                    st.create_lobby(true);
                    app.selftest = 1;
                    eprintln!("selftest: creating a public lobby");
                }
            }
        }
        // test switches: DUEL_QUALITY=0..3, DUEL_PAGE=settings:<tab> | join | main, DUEL_RECENT=1 (sample list)
        if let Some(q) = std::env::var("DUEL_QUALITY").ok().and_then(|v| v.parse::<usize>().ok()) {
            app.settings.quality = q.min(gpu::Quality::PRESETS.len() - 1);
            app.settings.post = gpu::Quality::PRESETS[app.settings.quality].1.post;
        }
        if let Some(x) = std::env::var("DUEL_CROSS").ok().and_then(|v| v.parse::<u8>().ok()) {
            app.settings.cross_style = x.min(CROSS_STYLES.len() as u8 - 1);
            app.settings.cross_color = [80, 255, 120];
        }
        // DUEL_NETSIM=100,5 switches the network test on; DUEL_DEBUG shows the F3 overlay
        if let Ok(v) = std::env::var("DUEL_NETSIM") {
            let mut it = v.split(',').filter_map(|x| x.trim().parse::<f32>().ok());
            app.settings.net_test = true;
            app.settings.net_lag = it.next().unwrap_or(100.0);
            app.settings.net_loss = it.next().unwrap_or(5.0);
        }
        app.show_debug = std::env::var_os("DUEL_DEBUG").is_some();
        // test switch: DUEL_TUNING opens the tuning panel (and puts a dummy 15 m ahead)
        app.show_tuning = std::env::var_os("DUEL_TUNING").is_some();
        if std::env::var_os("DUEL_RECENT").is_some() {
            let t = now_secs();
            app.settings.recent = vec![
                RecentServer { addr: "192.168.1.20".into(), host: "Mika".into(), mode: "Free-for-All".into(), when: t - 300 },
                RecentServer { addr: "100.64.3.7:27960".into(), host: "Dario".into(), mode: "Capture the Flag".into(), when: t - 86_400 * 2 },
            ];
        }
        if let Ok(pg) = std::env::var("DUEL_PAGE") {
            if let Some(tab) = pg.strip_prefix("settings:") {
                app.settings_tab = tab.parse().unwrap_or(0);
                app.screen = Screen::Menu(Page::Settings);
            } else if pg == "join" {
                app.screen = Screen::Menu(Page::Join);
            } else if pg == "host" {
                app.screen = Screen::Menu(Page::Host);
            }
        }
        // hidden test mode: a bot plays the local fighter (used to photograph gameplay)
        if std::env::var_os("DUEL_AUTOPLAY").is_some() {
            let nb: usize = std::env::var("DUEL_BOTS").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
            let mode = match std::env::var("DUEL_MODE").ok().and_then(|v| v.parse::<u8>().ok()) {
                Some(m) => Mode::from_u8(m),
                None if std::env::var_os("DUEL_TDM").is_some() => Mode::Tdm,
                None => Mode::Ffa,
            };
            let kills: u16 = std::env::var("DUEL_KILLS").ok().and_then(|v| v.parse().ok()).unwrap_or(15);
            let mut g = if std::env::var_os("DUEL_GAUNTLET").is_some() { Game::gauntlet("Tester", 0) } else { Game::offline("Tester", nb, 0.6, mode, kills) };
            if let Some(m) = std::env::var("DUEL_MAP").ok().and_then(|v| v.parse::<u8>().ok()) {
                g.set_map(MapId::from_u8(m));
            }
            if std::env::var_os("DUEL_HUMAN").is_none() {
                g.f[0].is_bot = true;
                g.f[0].ready = true;
                g.bots[0] = Some(bot::Bot::new(0.8, 5));
            } else if std::env::var_os("DUEL_HUMAN_READY").is_some() {
                // test switch: a hands-off player who skips the loadout screen
                g.f[0].ready = true;
            }
            app.autoplay = true;
            app.start_game(g, None);
        }
        // hidden test mode: the training room at lesson N (a bot hits the dummies unless DUEL_HUMAN)
        if let Ok(n) = std::env::var("DUEL_TRAIN") {
            let mut g = Game::training("Tester");
            g.train_skip(n.parse().unwrap_or(0));
            if std::env::var_os("DUEL_HUMAN").is_none() {
                g.f[0].is_bot = true;
                g.bots[0] = Some(bot::Bot::new(0.9, 5));
            }
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

    fn start_game(&mut self, mut game: Game, net: Option<Net>) {
        let l = game.local;
        if game.train.is_none() {
            game.my_loadout = self.settings.loadout;
        }
        if game.f[l].present && !game.f[l].is_bot && game.train.is_none() {
            game.f[l].loadout = self.settings.loadout;
            game.f[l].equip();
        }
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
        let via_steam = self.settings.use_steam && self.steam.is_some();
        let mut n = match Net::host(port) {
            Ok(n) => n,
            // the port is taken, but Steam needs no port: any free one will do
            Err(_) if via_steam => Net::host(0)?,
            Err(e) => return Err(e),
        };
        if via_steam {
            if let Some(st) = self.steam.as_mut() {
                n.steam = Some(st.client.clone());
                st.map_name = MapId::from_u8(self.settings.map).name().to_string();
                st.create_lobby(self.settings.steam_public);
            }
        }
        let mode = Mode::from_u8(self.settings.mode);
        let mut g = Game::new(Kind::Host, 0, mode, self.settings.rounds as u16);
        g.set_map(MapId::from_u8(self.settings.map));
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
        g.say(format!("Lobby open - same Wi-Fi: {addr}"));
        if via_steam {
            g.say("Opening a Steam lobby: friends can join from Steam, no port forwarding needed".into());
        }
        self.lookup_public_ip();
        self.check_firewall();
        self.ip_announced = false;
        self.start_game(g, Some(n));
        Ok(())
    }

    fn leave_to_menu(&mut self) {
        if let Screen::Play { net: Some(net), .. } | Screen::Lobby { net, .. } = &mut self.screen {
            net.send_bye();
        }
        if let Some(st) = self.steam.as_mut() {
            st.leave();
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
        } else if platform::mouse_hold() == platform::MouseHold::Locked {
            egui::CursorGrab::Locked
        } else {
            egui::CursorGrab::Confined
        };
        ctx.send_viewport_cmd(ViewportCommand::CursorGrab(grab));
    }
}

impl App {
    /// While a control is waiting for a key: take the next key, mouse button or modifier.
    fn capture_rebind(&mut self, ctx: &egui::Context) {
        let Some((a, started)) = self.rebinding else { return };
        if self.frames <= started + 1 {
            return;
        }
        let prev = self.prev_mods;
        let got: Option<Option<Bind>> = ctx.input(|i| {
            for e in &i.events {
                if let egui::Event::Key { key, pressed: true, repeat: false, .. } = e {
                    return Some(if *key == Key::Escape { None } else { Some(Bind::Key(*key)) });
                }
            }
            for n in 0..5u8 {
                if i.pointer.button_pressed(mouse_button(n)) {
                    return Some(Some(Bind::Mouse(n)));
                }
            }
            if i.modifiers.shift && !prev.shift {
                return Some(Some(Bind::Shift));
            }
            if i.modifiers.ctrl && !prev.ctrl {
                return Some(Some(Bind::Ctrl));
            }
            if i.modifiers.alt && !prev.alt {
                return Some(Some(Bind::Alt));
            }
            None
        });
        if let Some(choice) = got {
            if let Some(b) = choice {
                assign_bind(&mut self.settings.binds, a, b);
                self.settings.save();
            }
            self.rebinding = None;
        }
    }

    /// Count what happened for the career stats (and the XP it earns).
    fn career_frame(&mut self) {
        let Screen::Play { game, .. } = &mut self.screen else { return };
        if game.kind == Kind::Demo {
            game.career.clear();
            return;
        }
        let evs = std::mem::take(&mut game.career);
        if evs.is_empty() {
            return;
        }
        let level = self.career.level();
        for ev in evs {
            let xp = self.career.apply(ev);
            if let CareerEv::Kill { finisher, .. } = ev {
                if finisher > 0 {
                    game.say(format!("+{xp} XP (finisher)"));
                }
            }
        }
        if self.career.level() > level {
            game.say(format!("Level up! You are level {}", self.career.level()));
        }
        self.career.save();
    }

    /// Gauntlet: when a run ends, keep the best score and send it to the Steam leaderboard.
    fn gauntlet_frame(&mut self) {
        let Screen::Play { game, .. } = &mut self.screen else { return };
        let Some(gt) = game.gauntlet.as_mut() else { return };
        if !gt.over || gt.submitted {
            return;
        }
        gt.submitted = true;
        let score = gt.score;
        if score > self.settings.gauntlet_best {
            self.settings.gauntlet_best = score;
            self.settings.save();
        }
        if let Some(st) = self.steam.as_ref() {
            st.submit_gauntlet(score as i32);
        }
    }

    /// Steam each frame: its callbacks, joins from the friends list, our lobby's details.
    fn steam_frame(&mut self, dt: f32) {
        let Some(st) = self.steam.as_mut() else { return };
        let events = st.update();
        for ev in events {
            match ev {
                steam::SteamEvent::Created(Ok(id)) => {
                    let (mode, players) = match &self.screen {
                        Screen::Play { game, net } => (game.mt.mode as u8, net.as_ref().map_or(1, |n| n.player_count())),
                        _ => (self.settings.mode, 1),
                    };
                    if let Some(st) = self.steam.as_mut() {
                        st.lobby_created(id, mode, players);
                    }
                    if let Screen::Play { game, .. } = &mut self.screen {
                        game.say("Steam lobby open: friends can join from their friends list or Find Games".into());
                    }
                }
                steam::SteamEvent::Created(Err(e)) => {
                    if let Screen::Play { game, .. } = &mut self.screen {
                        game.say(format!("Couldn't open a Steam lobby ({e}); friends can still join by IP"));
                    }
                }
                steam::SteamEvent::JoinLobby(id) => self.pending_lobby = Some(id),
                steam::SteamEvent::Entered(Ok(id)) => {
                    let host = self.steam.as_mut().and_then(|s| s.entered(id));
                    let client = self.steam.as_ref().map(|s| s.client.clone());
                    match (host, client) {
                        (Some(h), Some(cl)) => match Net::join_steam(cl, h) {
                            Ok(n) => {
                                self.menu_msg = None;
                                self.screen = Screen::Lobby { net: n, waiting_since: 0.0 };
                            }
                            Err(e) => self.menu_msg = Some(e),
                        },
                        _ => self.menu_msg = Some("That Steam lobby has no host any more.".into()),
                    }
                }
                steam::SteamEvent::Entered(Err(())) => {
                    self.menu_msg = Some("Couldn't join that Steam lobby: it may be full or closed.".into());
                    self.screen = Screen::Menu(Page::Browse);
                }
                steam::SteamEvent::LobbyList(_) => {}
                steam::SteamEvent::Board { rank, top } => {
                    if let Screen::Play { game, .. } = &mut self.screen {
                        if let Some(gt) = game.gauntlet.as_mut() {
                            gt.rank = rank;
                            gt.board = top;
                        }
                    }
                }
            }
        }
        // a friend clicked "Join game": leave what we're doing and go
        if let Some(id) = self.pending_lobby.take() {
            if matches!(self.screen, Screen::Play { .. } | Screen::Lobby { .. }) {
                self.leave_to_menu();
            }
            if let Some(st) = self.steam.as_mut() {
                st.join_lobby(id);
                self.menu_msg = Some("Joining through Steam...".into());
            }
        }
        if self.selftest > 0 {
            self.steam_selftest(dt);
        }
        // keep our lobby's mode and head count current for the browser
        self.steam_tick -= dt;
        if self.steam_tick <= 0.0 {
            self.steam_tick = 1.0;
            if let (Some(st), Screen::Play { game, net: Some(n) }) = (self.steam.as_ref(), &self.screen) {
                if n.is_host {
                    st.update_lobby(game.mt.mode as u8, n.player_count());
                }
            }
        }
    }

    /// Developer tuning panel (F4): live sliders, presets, defaults and measuring dummies.
    fn tuning_ui(&mut self, ctx: &egui::Context, rect: Rect) {
        enum Act {
            Spawn(f32),
            Remove,
            Save,
            Load(String),
            Reset,
            Defaults,
        }
        let mut act: Option<Act> = None;
        let online = matches!(&self.screen, Screen::Play { net: Some(_), .. });
        let mut open = true;
        egui::Window::new("Tuning (F4)")
            .open(&mut open)
            .default_pos(Pos2::new(rect.right() - 420.0, rect.top() + 70.0))
            .default_width(390.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("Changes apply instantly. They only affect this computer.").size(13.0).color(Color32::from_gray(180)));
                if online {
                    ui.label(egui::RichText::new("Online: your movement changes only for you (others keep theirs).").size(13.0).color(Color32::from_rgb(255, 190, 110)));
                }
                egui::ScrollArea::vertical().max_height((rect.height() - 330.0).max(160.0)).show(ui, |ui| {
                    let mut group = "";
                    for f in TUNE_FIELDS.iter() {
                        if f.group != group {
                            group = f.group;
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(group).strong().color(ACCENT));
                        }
                        let mut v = (f.get)(&self.tune);
                        let step = if f.max - f.min <= 1.0 { 0.005 } else { 0.05 };
                        let r = ui.add(egui::Slider::new(&mut v, f.min..=f.max).step_by(step as f64).text(format!("{} ({})", f.label, f.unit)));
                        if r.changed() {
                            (f.set)(&mut self.tune, v);
                        }
                        if r.double_clicked() {
                            (f.set)(&mut self.tune, (f.get)(&Tuning::default()));
                        }
                        if f.key == "cam_smooth" {
                            ui.add(egui::Slider::new(&mut self.settings.fov, 60.0..=110.0).text("Field of view (°)"));
                        }
                    }
                });
                ui.label(egui::RichText::new("Double-click a slider to put it back to the game's default.").size(12.0).color(Color32::from_gray(160)));
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Preset");
                    ui.add(egui::TextEdit::singleline(&mut self.preset_name).desired_width(150.0));
                    if ui.button("Save Preset").clicked() {
                        act = Some(Act::Save);
                    }
                });
                ui.horizontal(|ui| {
                    let names = preset_names();
                    egui::ComboBox::from_id_salt("presets").selected_text("Load Preset...").show_ui(ui, |ui| {
                        if names.is_empty() {
                            ui.label("(no presets saved yet)");
                        }
                        for n in names {
                            if ui.selectable_label(false, &n).clicked() {
                                act = Some(Act::Load(n));
                            }
                        }
                    });
                    if ui.button("Reset").on_hover_text("Back to the game's built-in values").clicked() {
                        act = Some(Act::Reset);
                    }
                    if ui.button("Apply as new defaults").on_hover_text("Start with these values every time").clicked() {
                        act = Some(Act::Defaults);
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Spawn dummy at");
                    for d in [5.0f32, 15.0, 30.0] {
                        if ui.button(format!("{d:.0} m")).clicked() {
                            act = Some(Act::Spawn(d));
                        }
                    }
                    if ui.button("Remove").clicked() {
                        act = Some(Act::Remove);
                    }
                });
                if let Some(m) = &self.tune_msg {
                    ui.label(egui::RichText::new(m).size(13.0).color(Color32::from_rgb(170, 230, 170)));
                }
            });
        if !open {
            self.show_tuning = false;
        }
        let Some(act) = act else { return };
        self.tune_msg = Some(match act {
            Act::Spawn(d) => match &mut self.screen {
                Screen::Play { game, .. } => match game.spawn_dummy(d) {
                    Ok(m) => m,
                    Err(e) => e,
                },
                _ => String::new(),
            },
            Act::Remove => match &mut self.screen {
                Screen::Play { game, .. } => format!("Removed {} dummies", game.remove_dummies()),
                _ => String::new(),
            },
            Act::Save => {
                let name = preset_file_name(&self.preset_name);
                match preset_dir() {
                    Some(dir) => {
                        let _ = std::fs::create_dir_all(&dir);
                        match std::fs::write(dir.join(format!("{name}.txt")), preset_text(&self.tune, self.settings.fov)) {
                            Ok(()) => format!("Saved preset \"{name}\""),
                            Err(e) => format!("Couldn't save: {e}"),
                        }
                    }
                    None => "Couldn't find a folder to save in".into(),
                }
            }
            Act::Load(name) => match preset_dir().and_then(|d| std::fs::read_to_string(d.join(format!("{name}.txt"))).ok()) {
                Some(text) => {
                    self.tune = Tuning::from_text(&text);
                    if let Some(fov) = preset_fov(&text) {
                        self.settings.fov = fov;
                    }
                    self.preset_name = name.clone();
                    format!("Loaded preset \"{name}\"")
                }
                None => format!("Couldn't read preset \"{name}\""),
            },
            Act::Reset => {
                self.tune = Tuning::default();
                "Back to the built-in values".into()
            }
            Act::Defaults => match tuning_defaults_path() {
                Some(p) => {
                    if let Some(dir) = p.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    self.settings.save();
                    match std::fs::write(p, preset_text(&self.tune, self.settings.fov)) {
                        Ok(()) => "These are now your defaults (used every time the game starts)".into(),
                        Err(e) => format!("Couldn't save: {e}"),
                    }
                }
                None => "Couldn't find a folder to save in".into(),
            },
        });
    }

    /// Test switch: open a real lobby, find it in the lobby list, check the relay, close it.
    fn steam_selftest(&mut self, dt: f32) {
        let Some(st) = self.steam.as_mut() else { return };
        self.selftest_t += dt;
        match self.selftest {
            1 => {
                if let Some(id) = st.lobby {
                    eprintln!("selftest: lobby {id} created; waiting for Steam to publish it");
                    self.selftest = 2;
                    self.selftest_t = 0.0;
                } else if self.selftest_t > 20.0 {
                    eprintln!("selftest: FAIL no lobby after 20 s");
                    self.selftest = 9;
                }
            }
            2 if self.selftest_t > 2.0 => {
                st.refresh(None);
                eprintln!("selftest: asking Steam for the lobby list");
                self.selftest = 3;
                self.selftest_t = 0.0;
            }
            3 if !st.refreshing => {
                let mine = st.lobby.unwrap_or(0);
                eprintln!("selftest: {} lobbies listed", st.rows.len());
                match st.rows.iter().find(|r| r.id == mine) {
                    Some(r) => eprintln!("selftest: PASS our lobby is listed: name {:?}, mode {}, map {:?}, players {}/{}", r.name, r.mode, r.map, r.players, r.max),
                    None => eprintln!("selftest: our lobby isn't in the list (Steam may hide your own lobby)"),
                }
                let relay = st.client.networking_utils().relay_network_status();
                eprintln!("selftest: relay network: {relay:?}");
                self.selftest = 4;
                self.selftest_t = 0.0;
            }
            3 if self.selftest_t > 15.0 => {
                eprintln!("selftest: FAIL no lobby list after 15 s");
                self.selftest = 4;
            }
            4 => {
                // wait for the relay network to be ready (it measures the nearest relays first)
                let relay = st.client.networking_utils().relay_network_status();
                let ready = format!("{relay:?}").contains("Current");
                if !ready && self.selftest_t < 40.0 {
                    return;
                }
                eprintln!("selftest: relay network after {:.0} s: {relay:?}{}", self.selftest_t, if ready { " (PASS: relayed connections ready)" } else { "" });
                st.leave();
                eprintln!("selftest: left the lobby, done");
                self.selftest = 9;
            }
            _ => {}
        }
    }

    /// The Steam lobby browser: filters, the list, and Join.
    fn browser_ui(&mut self, ui: &mut egui::Ui) -> Option<u64> {
        let mut join = None;
        let Some(st) = self.steam.as_mut() else {
            ui.label("Steam isn't running, so there's no lobby list. Start Steam and restart the game, or use Join a Friend with an IP address.");
            return None;
        };
        if st.rows.is_empty() && !st.refreshing && ui.ctx().input(|i| i.time) > 0.0 && !self.browse_asked {
            self.browse_asked = true;
            st.refresh(self.browse.mode);
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Mode");
            egui::ComboBox::from_id_salt("bmode")
                .selected_text(self.browse.mode.map_or("Any".to_string(), |m| Mode::from_u8(m).name().to_string()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.browse.mode, None, "Any");
                    for m in 0..6u8 {
                        ui.selectable_value(&mut self.browse.mode, Some(m), Mode::from_u8(m).name());
                    }
                });
            ui.label("Map");
            egui::ComboBox::from_id_salt("bmap").selected_text(self.browse.map.clone().unwrap_or_else(|| "Any".into())).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.browse.map, None, "Any");
                for m in MapId::ALL {
                    ui.selectable_value(&mut self.browse.map, Some(m.name().to_string()), m.name());
                }
            });
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Max ping");
            ui.add(egui::Slider::new(&mut self.browse.max_ping, 0..=400).step_by(10.0).custom_formatter(|v, _| if v <= 0.0 { "any".into() } else { format!("{v:.0} ms") }));
            ui.checkbox(&mut self.browse.hide_full, "hide full");
            ui.checkbox(&mut self.browse.hide_empty, "hide empty");
        });
        ui.horizontal(|ui| {
            if ui.button(if st.refreshing { "Refreshing..." } else { "Refresh" }).clicked() && !st.refreshing {
                st.refresh(self.browse.mode);
            }
            ui.label(egui::RichText::new(format!("{} lobbies", st.rows.len())).color(Color32::from_gray(170)));
        });
        ui.separator();
        let rows: Vec<steam::LobbyRow> = st.rows.iter().filter(|r| self.browse.keep(r)).cloned().collect();
        if rows.is_empty() {
            ui.label(egui::RichText::new(if st.refreshing { "Looking for games..." } else { "No games match. Host one, or loosen the filters." }).color(Color32::from_gray(190)));
        }
        egui::ScrollArea::vertical().max_height((ui.ctx().content_rect().height() - 470.0).clamp(120.0, 360.0)).show(ui, |ui| {
            egui::Grid::new("lobbies").num_columns(6).striped(true).spacing([12.0, 6.0]).show(ui, |ui| {
                for h in ["Lobby", "Mode", "Map", "Players", "Ping", ""] {
                    ui.label(egui::RichText::new(h).color(ACCENT));
                }
                ui.end_row();
                for r in rows {
                    ui.label(if r.name.is_empty() { "Lobby".to_string() } else { r.name.clone() });
                    ui.label(Mode::from_u8(r.mode).name());
                    ui.label(&r.map);
                    ui.label(format!("{}/{}", r.players, r.max));
                    let (pt, pc) = match r.ping {
                        Some(p) if p <= 80 => (format!("{p} ms"), Color32::from_rgb(140, 230, 140)),
                        Some(p) if p <= 160 => (format!("{p} ms"), Color32::from_rgb(255, 215, 120)),
                        Some(p) => (format!("{p} ms"), Color32::from_rgb(255, 130, 110)),
                        None => ("...".into(), Color32::from_gray(160)),
                    };
                    ui.colored_label(pc, pt);
                    if ui.add_enabled(r.players < r.max, egui::Button::new("Join")).clicked() {
                        join = Some(r.id);
                    }
                    ui.end_row();
                }
            });
        });
        join
    }

    /// Music switches: all music, menu music, match music, and skip to the next track.
    fn music_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.settings.music_on, "Music");
            ui.add_enabled_ui(self.settings.music_on, |ui| {
                ui.checkbox(&mut self.settings.menu_music, "in menus");
                ui.checkbox(&mut self.settings.match_music, "in matches");
                if ui.button("Next track").clicked() {
                    self.track_skip += 1;
                }
            });
        });
        let key = self.settings.binds[A_MUSIC].name();
        ui.label(egui::RichText::new(format!("Press {key} in a match to turn music on or off.")).size(13.0).color(Color32::from_gray(170)));
    }

    /// The network test settings, when switched on.
    fn net_sim(&self) -> Option<net::NetSim> {
        self.settings.net_test.then(|| net::NetSim { lag_ms: self.settings.net_lag, loss: self.settings.net_loss / 100.0 })
    }

    /// Look up this computer's internet address once, in the background.
    fn lookup_public_ip(&mut self) {
        if self.ip_lookup_started {
            return;
        }
        self.ip_lookup_started = true;
        if let Ok(fake) = std::env::var("DUEL_FAKE_IP") {
            // test switch: show a made-up address instead of asking the internet
            if let Ok(mut s) = self.public_ip.lock() {
                *s = Some(Ok(fake));
            }
            return;
        }
        if self.autoplay || self.screenshot.is_some() {
            return; // tests never go online
        }
        let slot = self.public_ip.clone();
        std::thread::spawn(move || {
            let r = net::fetch_public_ip();
            if let Ok(mut s) = slot.lock() {
                *s = Some(r);
            }
        });
    }

    /// Check Windows Firewall once, in the background (reads only).
    fn check_firewall(&mut self) {
        if self.fw_started {
            return;
        }
        self.fw_started = true;
        if (self.autoplay || self.screenshot.is_some()) && std::env::var_os("DUEL_FW_CHECK").is_none() {
            return; // tests don't poke at the system
        }
        let port: u16 = self.settings.port.trim().parse().unwrap_or(net::DEFAULT_PORT);
        let slot = self.fw_status.clone();
        std::thread::spawn(move || {
            let s = firewall::check(port);
            if let Ok(mut g) = slot.lock() {
                *g = Some(s);
            }
        });
    }

    /// The firewall line for hosts, with a button to fix it when friends would be blocked.
    fn firewall_ui(&mut self, ui: &mut egui::Ui) {
        if !firewall::applies() {
            return;
        }
        self.check_firewall();
        let status = self.fw_status.lock().ok().and_then(|g| *g);
        let note = self.fw_note.lock().ok().and_then(|g| g.clone());
        match status {
            None if self.fw_started && !(self.autoplay || self.screenshot.is_some()) => {
                ui.label(egui::RichText::new("Checking Windows Firewall…").size(14.0).color(Color32::from_gray(190)));
            }
            None => {}
            Some(s) => {
                let col = if s.ok() { Color32::from_rgb(140, 225, 140) } else { Color32::from_rgb(255, 170, 110) };
                ui.label(egui::RichText::new(s.text()).size(14.0).color(col));
                if matches!(s, firewall::FwStatus::Blocked | firewall::FwStatus::NoRule) {
                    if ui.button("Allow Duel Rush through Windows Firewall").on_hover_text("Windows will ask for permission. Adds an \"allow\" rule for this game and its UDP port.").clicked() {
                        let port: u16 = self.settings.port.trim().parse().unwrap_or(net::DEFAULT_PORT);
                        let (st, nt) = (self.fw_status.clone(), self.fw_note.clone());
                        if let Ok(mut g) = st.lock() {
                            *g = None;
                        }
                        std::thread::spawn(move || {
                            let r = firewall::allow(port);
                            let s = firewall::check(port);
                            if let Ok(mut g) = nt.lock() {
                                *g = Some(match r {
                                    Ok(()) => "Firewall rule added.".to_string(),
                                    Err(e) => e,
                                });
                            }
                            if let Ok(mut g) = st.lock() {
                                *g = Some(s);
                            }
                        });
                    }
                }
            }
        }
        if let Some(n) = note {
            ui.label(egui::RichText::new(n).size(13.0).color(Color32::from_gray(190)));
        }
    }

    /// The internet address line for hosts: the address, still looking, or why it failed.
    fn public_addr_text(&self) -> (Option<String>, String) {
        let port = self.settings.port.trim().to_string();
        match self.public_ip.lock().ok().and_then(|g| g.clone()) {
            Some(Ok(ip)) => (Some(format!("{ip}:{port}")), format!("{ip}:{port}")),
            Some(Err(_)) => (None, "couldn't find it (offline?)".into()),
            None if self.ip_lookup_started => (None, "finding it…".into()),
            None => (None, "-".into()),
        }
    }

    /// Keep a server in the recent list (newest first, no repeats).
    fn remember_server(&mut self, addr: String, host: String, mode: String) {
        if addr.is_empty() {
            return;
        }
        self.settings.recent.retain(|r| r.addr != addr);
        self.settings.recent.insert(0, RecentServer { addr, host, mode, when: now_secs() });
        self.settings.recent.truncate(MAX_RECENT);
        if !self.autoplay {
            self.settings.save();
        }
    }

    /// Connect to a server in one click.
    fn join_server(&mut self, addr: &str) {
        self.settings.join_addr = addr.to_string();
        match Net::join(addr) {
            Ok(n) => {
                self.settings.save();
                self.menu_msg = None;
                self.screen = Screen::Lobby { net: n, waiting_since: 0.0 };
            }
            Err(e) => {
                self.menu_msg = Some(e);
                self.screen = Screen::Menu(Page::Join);
            }
        }
    }

    /// The list of servers joined before: Join in one click, or forget one.
    /// Returns the address to join, if one was clicked.
    fn recent_list_ui(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let mut join = None;
        let mut forget = None;
        if self.settings.recent.is_empty() {
            ui.label(egui::RichText::new("Servers you join show up here, so you can hop back in with one click.").color(Color32::from_gray(190)));
            return None;
        }
        for (k, r) in self.settings.recent.iter().enumerate() {
            egui::Frame::new().fill(Color32::from_rgba_unmultiplied(255, 255, 255, 10)).corner_radius(6.0).inner_margin(8.0).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        let title = if r.host.trim().is_empty() { "Lobby".to_string() } else { format!("{}'s lobby", r.host) };
                        ui.label(egui::RichText::new(title).strong().color(Color32::from_rgb(245, 235, 225)));
                        ui.label(egui::RichText::new(&r.addr).size(13.0).color(Color32::from_gray(200)));
                        ui.label(egui::RichText::new(format!("{}  ·  {}", r.mode, ago(r.when))).size(12.0).color(Color32::from_gray(150)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✕").on_hover_text("Forget this server").clicked() {
                            forget = Some(k);
                        }
                        if ui.add(egui::Button::new(egui::RichText::new("Join").strong()).fill(Color32::from_rgb(170, 90, 40))).clicked() {
                            join = Some(r.addr.clone());
                        }
                    });
                });
            });
        }
        if let Some(k) = forget {
            self.settings.recent.remove(k);
            self.settings.save();
        }
        join
    }

    /// Every setting, in tabs. Used on the main menu and in the pause menu.
    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        // LB / RB on a controller step through the tabs
        let step = std::mem::take(&mut self.pad_tab);
        if step != 0 {
            self.settings_tab = (self.settings_tab as i32 + step).rem_euclid(4) as u8;
        }
        ui.horizontal(|ui| {
            for (k, t) in ["Game", "Controls", "Graphics", "Crosshair"].iter().enumerate() {
                ui.selectable_value(&mut self.settings_tab, k as u8, egui::RichText::new(*t).size(17.0));
            }
        });
        ui.separator();
        match self.settings_tab {
            0 => {
                ui.add(egui::Slider::new(&mut self.settings.sens, 0.2..=3.0).text("mouse sensitivity"));
                ui.add(egui::Slider::new(&mut self.settings.fov, 60.0..=110.0).text("field of view"));
                ui.add(egui::Slider::new(&mut self.settings.volume, 0.0..=1.0).text("effects volume"));
                ui.add(egui::Slider::new(&mut self.settings.music_volume, 0.0..=1.0).text("music volume"));
                self.music_ui(ui);
                if self.audio.is_none() && self.screenshot.is_none() {
                    ui.label("(no sound device found)");
                }
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Network test").size(18.0).color(ACCENT));
                ui.label(egui::RichText::new("Try online play on one PC: open the game twice, Host in one and Join 127.0.0.1 in the other. This adds fake lag and lost packets to this copy's connection. Press F3 in a match to see ping, packet loss and players.").size(13.0).color(Color32::from_gray(185)));
                ui.checkbox(&mut self.settings.net_test, "Simulate a bad connection");
                ui.add_enabled_ui(self.settings.net_test, |ui| {
                    ui.add(egui::Slider::new(&mut self.settings.net_lag, 0.0..=300.0).step_by(10.0).suffix(" ms").text("extra lag"));
                    ui.add(egui::Slider::new(&mut self.settings.net_loss, 0.0..=20.0).step_by(1.0).suffix(" %").text("packet loss"));
                });
            }
            1 => {
                ui.label(egui::RichText::new("Click a control, then press the key or mouse button you want. Esc cancels.").color(Color32::from_gray(200)));
                // two columns of controls; scrolls if the window is short
                let room = (ui.ctx().content_rect().height() - 440.0).clamp(110.0, 400.0);
                egui::ScrollArea::vertical().max_height(room).show(ui, |ui| {
                    ui.spacing_mut().button_padding = Vec2::new(8.0, 3.0);
                    egui::Grid::new("binds").num_columns(4).spacing([10.0, 5.0]).striped(true).show(ui, |ui| {
                        let half = N_ACTIONS.div_ceil(2);
                        for row in 0..half {
                            for k in [row, row + half] {
                                if k >= N_ACTIONS {
                                    continue;
                                }
                                ui.label(egui::RichText::new(ACTIONS[k].1).size(14.5));
                                let waiting = self.rebinding.map_or(false, |(a, _)| a == k);
                                let text = if waiting { egui::RichText::new("press a key…").color(ACCENT) } else { egui::RichText::new(self.settings.binds[k].name()).strong() };
                                if ui.add_sized([112.0, 26.0], egui::Button::new(text)).clicked() && !waiting {
                                    self.rebinding = Some((k, self.frames));
                                }
                            }
                            ui.end_row();
                        }
                    });
                });
                ui.label(egui::RichText::new("Always on: arrow keys move, Enter opens chat, Esc pauses, F11 fullscreen, mouse wheel swaps weapons.").size(13.0).color(Color32::from_gray(170)));
                if ui.button("Reset controls to default").clicked() {
                    self.settings.binds = default_binds();
                    self.rebinding = None;
                }
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Controller").size(18.0).color(ACCENT));
                let pad_name = self.pads.name.clone();
                ui.label(egui::RichText::new(match &pad_name {
                    Some(n) => format!("Connected: {n}"),
                    None => "No controller connected (Xbox, PlayStation, Switch Pro and Steam Deck controls all work).".to_string(),
                }).size(13.5).color(Color32::from_gray(200)));
                ui.add(egui::Slider::new(&mut self.settings.pad_sens, 0.2..=3.0).text("look speed"));
                ui.checkbox(&mut self.settings.pad_invert, "Invert look up / down");
                ui.checkbox(&mut self.settings.aim_assist, "Aim assist (the aim slows a little over an enemy)");
                egui::CollapsingHeader::new("Controller layout").show(ui, |ui| {
                    egui::Grid::new("padlayout").num_columns(2).spacing([14.0, 3.0]).striped(true).show(ui, |ui| {
                        for (what, button) in pad::LAYOUT {
                            ui.label(egui::RichText::new(what).size(14.0));
                            ui.label(egui::RichText::new(button).size(14.0).strong());
                            ui.end_row();
                        }
                    });
                });
            }
            2 => {
                ui.label("Quality preset");
                ui.horizontal(|ui| {
                    for (k, (n, q)) in gpu::Quality::PRESETS.iter().enumerate() {
                        if ui.selectable_value(&mut self.settings.quality, k, *n).clicked() {
                            // each preset brings its own set of effects
                            self.settings.post = q.post;
                        }
                    }
                });
                let q = gpu::Quality::PRESETS[self.settings.quality.min(gpu::Quality::PRESETS.len() - 1)].1;
                let what = format!(
                    "{}% resolution · edge smoothing {} · {} effects",
                    (q.scale * 100.0).round(),
                    if q.msaa > 1 { "on" } else { "off" },
                    if q.fx >= 1.0 { "full" } else if q.fx >= 0.6 { "most" } else { "fewer" }
                );
                ui.label(egui::RichText::new(what).size(13.0).color(Color32::from_gray(180)));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Interface size");
                    for (k, (n, _)) in UI_SCALES.iter().enumerate() {
                        ui.selectable_value(&mut self.settings.ui_scale, k, *n);
                    }
                });
                ui.checkbox(&mut self.settings.dyn_res, "Dynamic resolution (lowers the 3D detail for a moment to hold the frame rate)");
                ui.label(egui::RichText::new("Effects").size(16.0).color(ACCENT));
                egui::Grid::new("fx").num_columns(2).spacing([18.0, 2.0]).show(ui, |ui| {
                    let f = &mut self.settings.post;
                    ui.checkbox(&mut f.soft_shadows, "Soft shadows");
                    ui.checkbox(&mut f.ssao, "Ambient occlusion");
                    ui.end_row();
                    ui.checkbox(&mut f.bloom, "Bloom (glow)");
                    ui.checkbox(&mut f.rays, "Light shafts");
                    ui.end_row();
                    ui.checkbox(&mut f.grading, "Colour grading (map mood)");
                    ui.checkbox(&mut f.vignette, "Vignette");
                    ui.end_row();
                });
                if platform::system().couch() && ui.button("Use the recommended Steam Deck settings").clicked() {
                    self.settings.couch_defaults();
                }
                ui.checkbox(&mut self.settings.fullscreen, "Fullscreen (F11)");
                ui.checkbox(&mut self.settings.vsync, "VSync (no tearing; caps to your screen's refresh rate)");
                ui.horizontal(|ui| {
                    ui.label("FPS limit");
                    egui::ComboBox::from_id_salt("fpscap")
                        .selected_text(if self.settings.fps_cap == 0 { "Unlimited".to_string() } else { format!("{}", self.settings.fps_cap) })
                        .show_ui(ui, |ui| {
                            for c in FPS_CAPS {
                                ui.selectable_value(&mut self.settings.fps_cap, c, if c == 0 { "Unlimited".to_string() } else { format!("{c}") });
                            }
                        });
                    ui.label(egui::RichText::new(format!("now: {:.0} fps", self.fps)).color(Color32::from_gray(170)));
                });
            }
            _ => {
                ui.horizontal(|ui| {
                    ui.label("Style");
                    egui::ComboBox::from_id_salt("xstyle").selected_text(CROSS_STYLES[self.settings.cross_style as usize]).show_ui(ui, |ui| {
                        for (k, n) in CROSS_STYLES.iter().enumerate() {
                            ui.selectable_value(&mut self.settings.cross_style, k as u8, *n);
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Colour");
                    for c in [[255, 255, 255], [80, 255, 120], [0, 230, 255], [255, 230, 0], [255, 80, 200], [255, 70, 60]] {
                        let col = Color32::from_rgb(c[0], c[1], c[2]);
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::click());
                        ui.painter().rect_filled(r, 4.0, col);
                        if self.settings.cross_color == c {
                            ui.painter().rect_stroke(r.expand(2.0), 5.0, Stroke::new(2.0, ACCENT), egui::StrokeKind::Outside);
                        }
                        if resp.clicked() {
                            self.settings.cross_color = c;
                        }
                    }
                    ui.color_edit_button_srgb(&mut self.settings.cross_color);
                });
                ui.add(egui::Slider::new(&mut self.settings.cross_size, 0.5..=2.0).text("size"));
                ui.checkbox(&mut self.settings.cross_red, "Dot turns red when it's over an enemy");
                // live preview over a bit of arena-ish background
                let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 110.0), egui::Sense::hover());
                let p = ui.painter();
                p.rect_filled(r, 6.0, Color32::from_rgb(70, 60, 58));
                p.rect_filled(Rect::from_min_max(Pos2::new(r.left(), r.center().y + 10.0), r.max), 6.0, Color32::from_rgb(120, 52, 44));
                let col = Color32::from_rgb(self.settings.cross_color[0], self.settings.cross_color[1], self.settings.cross_color[2]);
                // the lines breathe in and out like they do while moving and shooting
                let gap = 4.0 + 10.0 * (0.5 + 0.5 * (self.wall_t * 2.0).sin());
                draw_crosshair(p, r.center(), self.settings.cross_style, col, col, self.settings.cross_size, gap, false);
                ui.label(egui::RichText::new("The lines always show your gun's real spread: every bullet lands inside them. Standing still and waiting a moment makes the next shot dead-on; Auto shows the shotgun's pellet cone as a ring. Right-click with a gun to zoom.").size(13.0).color(Color32::from_gray(170)));
            }
        }
    }
}

/// Give action `a` the binding `b`; whatever had `b` before takes `a`'s old binding.
fn assign_bind(binds: &mut [Bind; N_ACTIONS], a: usize, b: Bind) {
    if let Some(o) = binds.iter().position(|x| *x == b) {
        if o != a {
            binds[o] = binds[a];
        }
    }
    binds[a] = b;
}

/// How many pixels wide a spread is on screen: the camera's focal length (pixels) times the
/// spread (tangent of the cone's half-angle). Every pellet lands within this radius.
pub fn spread_px(spread: f32, focal: f32) -> f32 {
    focal * spread
}

/// The crosshair. `gap` is the spread's radius in pixels: lines start there (and the circle sits
/// there), so every shot lands inside. `size` scales the dot and lines (0.8 = 2 px dot, 6 px
/// lines). `circle` draws the shotgun's cone as a ring instead of lines.
fn draw_crosshair(p: &egui::Painter, c: Pos2, style: u8, col: Color32, dot_col: Color32, size: f32, gap: f32, circle: bool) {
    let outline = Color32::from_black_alpha(170);
    let th = (2.0 * size).max(1.0);
    let len = 7.5 * size;
    let gap = gap.max(3.0 * size);
    let arm = |dx: f32, dy: f32| {
        let a = c + Vec2::new(dx * gap, dy * gap);
        let b = c + Vec2::new(dx * (gap + len), dy * (gap + len));
        let ext = Vec2::new(dx, dy) * 1.0;
        p.line_segment([a - ext, b + ext], Stroke::new(th + 2.0, outline));
        p.line_segment([a, b], Stroke::new(th, col));
    };
    let dot = || {
        let r = 1.25 * size;
        p.circle_filled(c, r + 1.0, outline);
        p.circle_filled(c, r, dot_col);
    };
    let ring = |r: f32| {
        p.circle_stroke(c, r, Stroke::new(th + 2.0, outline));
        p.circle_stroke(c, r, Stroke::new(th * 0.8, col));
    };
    let four = || {
        for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            arm(dx, dy);
        }
    };
    match style {
        2 => dot(),
        3 => {
            ring(gap.max(8.0 * size));
            dot();
        }
        1 => four(),
        5 => {
            for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0)] {
                arm(dx, dy);
            }
            dot();
        }
        _ => {
            if circle && style == 0 {
                ring(gap);
            } else {
                four();
            }
            dot();
        }
    }
}
/// The game's icon, drawn in code: crossed blades over a dark shield with an ember ring.
fn icon_pixels(n: u32) -> (Vec<u8>, u32) {
    let mut px = vec![0u8; (n * n * 4) as usize];
    let h = n as f32 * 0.5;
    // distance from point p to the segment a-b
    let seg = |p: (f32, f32), a: (f32, f32), b: (f32, f32)| -> f32 {
        let (vx, vy) = (b.0 - a.0, b.1 - a.1);
        let t = (((p.0 - a.0) * vx + (p.1 - a.1) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
        let (dx, dy) = (p.0 - a.0 - vx * t, p.1 - a.1 - vy * t);
        (dx * dx + dy * dy).sqrt()
    };
    for y in 0..n {
        for x in 0..n {
            // 4x supersampling for smooth edges
            let mut acc = [0.0f32; 4];
            for sy in 0..2 {
                for sx in 0..2 {
                    let u = (x as f32 + 0.25 + sx as f32 * 0.5 - h) / h;
                    let v = (y as f32 + 0.25 + sy as f32 * 0.5 - h) / h;
                    let r = (u * u + v * v).sqrt();
                    let mut c = [0.0f32; 4];
                    if r < 0.97 {
                        // dark badge with a warm glow toward the bottom
                        let glow = ((v + 1.0) * 0.5).powf(2.0);
                        c = [0.08 + 0.25 * glow, 0.05 + 0.08 * glow, 0.10, 1.0];
                        if r > 0.84 {
                            c = [1.0, 0.59, 0.27, 1.0]; // ember ring
                        }
                        // two crossed blades, each with a guard and a grip
                        for s in [1.0f32, -1.0] {
                            let tip = (0.58 * s, -0.58);
                            let base = (-0.38 * s, 0.38);
                            let pommel = (-0.6 * s, 0.6);
                            let blade = seg((u, v), tip, base);
                            let w = 0.085 * (0.35 + 0.65 * ((u - tip.0).hypot(v - tip.1) / 1.4).min(1.0));
                            if blade < w {
                                let edge = if blade > w * 0.45 { 0.78 } else { 0.97 };
                                c = [edge, edge, edge + 0.02, 1.0];
                            }
                            let guard = seg((u, v), (base.0 - 0.13 * s, base.1 - 0.13), (base.0 + 0.13 * s, base.1 + 0.13));
                            if guard < 0.05 {
                                c = [1.0, 0.62, 0.25, 1.0];
                            }
                            if seg((u, v), base, pommel) < 0.05 {
                                c = [0.35, 0.18, 0.12, 1.0];
                            }
                        }
                    }
                    for k in 0..4 {
                        acc[k] += c[k] * 0.25;
                    }
                }
            }
            let i = ((y * n + x) * 4) as usize;
            for k in 0..4 {
                px[i + k] = (acc[k].clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
    }
    (px, n)
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

    // test switch: DUEL_WINDOW=800x450
    let size = std::env::var("DUEL_WINDOW")
        .ok()
        .and_then(|v| {
            let (a, b) = v.split_once('x')?;
            Some([a.trim().parse::<f32>().ok()?, b.trim().parse::<f32>().ok()?])
        })
        .unwrap_or([1280.0, 720.0]);
    let boot = Settings::load();
    let (icon, n) = icon_pixels(64);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Duel Rush")
        .with_inner_size(size)
        .with_min_inner_size([800.0, 450.0])
        .with_icon(egui::IconData { rgba: icon, width: n, height: n });
    if screenshot.is_none() && boot.fullscreen {
        viewport = viewport.with_fullscreen(true);
    }
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        if platform::test_window_behind() {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    // vsync as saved (the settings can switch it while playing)
    options.wgpu_options.surface.present_mode = if boot.vsync { eframe::wgpu::PresentMode::AutoVsync } else { eframe::wgpu::PresentMode::AutoNoVsync };
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
    /// Controllers, read before egui sees this frame's input: in menus their moves become key
    /// presses (arrows move between buttons, A presses, B goes back), so every menu works with
    /// a controller exactly as with a keyboard.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        use pad::Nav;
        let st = self.pads.poll();
        self.pad = st;
        // who's playing: the controller once it's touched, the mouse and keyboard once they are
        if st.used() {
            self.pad_active = true;
        } else if raw.events.iter().any(|e| match e {
            egui::Event::MouseMoved(d) => d.length() > 4.0,
            egui::Event::Key { pressed: true, .. } | egui::Event::PointerButton { pressed: true, .. } => true,
            _ => false,
        }) {
            self.pad_active = false;
        }
        if !st.connected {
            return;
        }
        let navs = self.pad_nav.step(&st, self.frame_dt.clamp(0.001, 0.1));
        let in_menus = !self.captured && self.quick_chat.is_none();
        let focused = ctx.memory(|m| m.focused().is_some());
        let mut keys: Vec<Key> = Vec::new();
        for n in navs {
            match n {
                // pause in a match, resume from the pause menu, back out of a menu page
                Nav::Menu => keys.push(Key::Escape),
                _ if !in_menus => {}
                // nothing selected yet: the first move selects the first button
                Nav::Up | Nav::Down | Nav::Left | Nav::Right | Nav::Press if !focused => keys.push(Key::Tab),
                Nav::Up => keys.push(Key::ArrowUp),
                Nav::Down => keys.push(Key::ArrowDown),
                Nav::Left => keys.push(Key::ArrowLeft),
                Nav::Right => keys.push(Key::ArrowRight),
                Nav::Press => keys.push(Key::Space),
                Nav::Back => keys.push(Key::Escape),
                Nav::PrevTab => self.pad_tab -= 1,
                Nav::NextTab => self.pad_tab += 1,
            }
        }
        for key in keys {
            for pressed in [true, false] {
                raw.events.push(egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE });
            }
        }
        // the right stick scrolls long pages
        if in_menus && st.ry.abs() > 0.1 {
            raw.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, st.ry * 900.0 * self.frame_dt.clamp(0.001, 0.1)),
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Wheel,
                modifiers: egui::Modifiers::NONE,
            });
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ctx.request_repaint();
        // interface size (Couch / TV makes text, menus and the HUD bigger)
        let zoom = UI_SCALES[self.settings.ui_scale.min(UI_SCALES.len() - 1)].1;
        if (ctx.zoom_factor() - zoom).abs() > 1e-3 {
            ctx.set_zoom_factor(zoom);
        }
        // the freeze watchdog: we're alive (a minimised window rests on purpose)
        self.watchdog.beat();
        self.watchdog.hold(ctx.input(|i| i.viewport().minimized.unwrap_or(false)));
        self.watchdog.stage("frame start");
        // frame-rate limit: wait out the rest of this frame's time
        if self.settings.fps_cap > 0 && self.screenshot.is_none() {
            if let Some(t0) = self.frame_start {
                let target = std::time::Duration::from_secs_f64(1.0 / self.settings.fps_cap as f64);
                let spent = t0.elapsed();
                if spent < target {
                    let left = target - spent;
                    if left > std::time::Duration::from_millis(2) {
                        std::thread::sleep(left - std::time::Duration::from_millis(1));
                    }
                    while t0.elapsed() < target {
                        std::thread::yield_now();
                    }
                }
            }
        }
        if let Some(t0) = self.frame_start {
            let dt = t0.elapsed().as_secs_f32().max(1e-4);
            self.fps += (1.0 / dt - self.fps) * 0.05;
        }
        self.frame_start = Some(std::time::Instant::now());
        // fullscreen (F11 toggles it too)
        if ctx.input(|i| i.key_pressed(Key::F3)) && self.rebinding.is_none() {
            self.show_debug = !self.show_debug;
        }
        if ctx.input(|i| i.key_pressed(Key::F4)) && self.rebinding.is_none() && matches!(self.screen, Screen::Play { .. }) {
            self.show_tuning = !self.show_tuning;
        }
        // the tuning values are live: every game uses them at once
        self.demo.tune = self.tune;
        if let Screen::Play { game, .. } = &mut self.screen {
            game.tune = self.tune;
        }
        if ctx.input(|i| i.key_pressed(Key::F11)) && self.rebinding.is_none() {
            self.settings.fullscreen = !self.settings.fullscreen;
        }
        if self.screenshot.is_none() && self.shown_fullscreen != Some(self.settings.fullscreen) {
            if self.shown_fullscreen.is_some() || self.settings.fullscreen {
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.settings.fullscreen));
            }
            self.shown_fullscreen = Some(self.settings.fullscreen);
        }
        self.capture_rebind(&ctx);
        // vsync on or off, applied right away
        if self.applied_vsync != Some(self.settings.vsync) {
            if let Some(mut sc) = frame.wgpu_surface_config() {
                sc.present_mode = if self.settings.vsync { eframe::wgpu::PresentMode::AutoVsync } else { eframe::wgpu::PresentMode::AutoNoVsync };
                frame.set_wgpu_surface_config(sc);
            }
            self.applied_vsync = Some(self.settings.vsync);
        }
        self.frames += 1;
        let rect = ui.max_rect();
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
        self.frame_dt = dt;
        self.wall_t += dt;
        self.handle_screenshot(&ctx);

        self.watchdog.stage("Steam");
        self.steam_frame(dt);
        // network + simulation
        let mut go_menu = false;
        if let Screen::Lobby { .. } = self.screen {
            self.watchdog.stage("joining a lobby");
            self.update_lobby(&ctx, dt);
        }

        let playing = matches!(self.screen, Screen::Play { .. });
        if playing {
            let focused = ctx.input(|i| i.focused);
            if !focused && !self.paused && !self.autoplay {
                self.paused = true;
            }
            let choosing = match &self.screen {
                Screen::Play { game, .. } => game.mt.phase == Phase::Loadout && !game.f[game.local].ready,
                _ => false,
            };
            let want_capture = !self.paused && !self.match_over() && self.disconnected.is_none() && !self.autoplay && !choosing && !self.show_tuning;
            self.set_capture(&ctx, want_capture);
            if want_capture && platform::mouse_hold() == platform::MouseHold::Confined {
                ctx.send_viewport_cmd(ViewportCommand::CursorPosition(rect.center()));
            }
            self.watchdog.stage("reading input");
            sample_keys(&ctx, &self.settings.binds);
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
        self.watchdog.stage("drawing the 3D picture");
        let painter = ui.painter().clone();
        let ppp = ctx.pixels_per_point();
        if self.gpu.is_none() {
            if let Some(rs) = frame.wgpu_render_state() {
                self.gpu = Some(gpu::Gpu::new(rs, &self.demo.world));
            }
        }
        let mut gpu = self.gpu.take();
        if let (Some(g3), Some(rs)) = (gpu.as_mut(), frame.wgpu_render_state()) {
            let q = gpu::Quality::PRESETS[self.settings.quality.min(gpu::Quality::PRESETS.len() - 1)].1;
            if g3.quality != q {
                g3.set_quality(rs, q);
            }
            // the effects as set, with the preset's shadow detail
            g3.fx = gpu::Fx { shadow_res: q.post.shadow_res, ..self.settings.post };
            // dynamic resolution: hold the frame rate (60, or the chosen limit) during matches
            let target = if self.settings.fps_cap > 0 { self.settings.fps_cap as f32 } else { 60.0 };
            g3.dyn_scale = if self.settings.dyn_res && playing && self.screenshot.is_none() {
                self.dynres.update(self.frame_dt, target)
            } else {
                self.dynres = gpu::DynRes::default();
                1.0
            };
        }
        let alpha = (self.acc / TICK).clamp(0.0, 1.0);
        let full_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let mut ears = None;
        match &self.screen {
            Screen::Play { game, .. } => {
                let (cam, lp) = self.play_camera(game, rect);
                ears = Some((cam.pos, cam.right, cam.fwd));
                if let (Some(g3), Some(rs)) = (gpu.as_mut(), frame.wgpu_render_state()) {
                    if let Some(id) = g3.render(rs, game, &cam, rect, ppp, alpha, Some((game.local, lp))) {
                        painter.image(id, rect, full_uv, Color32::WHITE);
                    }
                }
                self.watchdog.stage("drawing the HUD");
                self.draw_hud(&painter, rect, &cam, game);
                if let Screen::Play { net, .. } = &self.screen {
                    self.draw_net_overlay(&painter, rect, game, net.as_ref());
                }
            }
            _ => {
                let cam = self.demo_camera(rect);
                if let (Some(g3), Some(rs)) = (gpu.as_mut(), frame.wgpu_render_state()) {
                    if let Some(id) = g3.render(rs, &self.demo, &cam, rect, ppp, alpha, None) {
                        painter.image(id, rect, full_uv, Color32::WHITE);
                    }
                }
                // darken the left side for readability, fading into the arena
                let mut mesh = egui::epaint::Mesh::default();
                let fade_w = 640.0f32.min(rect.width() * 0.62);
                let stops = [(0.0, 190u8), (0.55, 150), (1.0, 0)];
                for (k, (t, a)) in stops.iter().enumerate() {
                    let x = rect.left() + fade_w * t;
                    let col = Color32::from_rgba_unmultiplied(10, 8, 20, *a);
                    mesh.colored_vertex(Pos2::new(x, rect.top()), col);
                    mesh.colored_vertex(Pos2::new(x, rect.bottom()), col);
                    if k > 0 {
                        let i = (k as u32) * 2;
                        mesh.add_triangle(i - 2, i - 1, i + 1);
                        mesh.add_triangle(i - 2, i + 1, i);
                    }
                }
                painter.add(egui::Shape::mesh(mesh));
                // soft letterbox bars top and bottom
                for (y0, y1, a0, a1) in [(rect.top(), rect.top() + 90.0, 140u8, 0u8), (rect.bottom() - 90.0, rect.bottom(), 0, 160)] {
                    let mut m = egui::epaint::Mesh::default();
                    m.colored_vertex(Pos2::new(rect.left(), y0), Color32::from_rgba_unmultiplied(0, 0, 0, a0));
                    m.colored_vertex(Pos2::new(rect.right(), y0), Color32::from_rgba_unmultiplied(0, 0, 0, a0));
                    m.colored_vertex(Pos2::new(rect.left(), y1), Color32::from_rgba_unmultiplied(0, 0, 0, a1));
                    m.colored_vertex(Pos2::new(rect.right(), y1), Color32::from_rgba_unmultiplied(0, 0, 0, a1));
                    m.add_triangle(0, 1, 3);
                    m.add_triangle(0, 3, 2);
                    painter.add(egui::Shape::mesh(m));
                }
            }
        }

        self.gpu = gpu;

        // ---- UI overlays
        self.watchdog.stage("menus and panels");
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
                if self.show_tuning {
                    self.tuning_ui(&ctx, rect);
                }
            }
        }
        if go_menu {
            self.leave_to_menu();
        }

        // play queued sounds, in 3D from the camera
        self.watchdog.stage("sound");
        let mut sounds: Vec<(Sfx, Option<V3>, f32)> = match &mut self.screen {
            Screen::Play { game, .. } => std::mem::take(&mut game.sounds),
            _ => Vec::new(),
        };
        // menu clicks: big buttons, and a softer click for anything else clicked in a panel
        let ui_sound = UI_SOUND.with(|u| u.take());
        if let Some(s) = ui_sound {
            sounds.push((s, None, 1.0));
        } else if !self.captured && ctx.input(|i| i.pointer.primary_clicked()) && ctx.is_pointer_over_egui() {
            sounds.push((Sfx::UiClick, None, 1.0));
        }
        let n_tracks = audio::TRACK_FIGHTS.len() + 1;
        let track = match &self.screen {
            _ if !self.settings.music_on => None,
            Screen::Play { .. } if !self.settings.match_music => None,
            Screen::Play { game, .. } => {
                let base = if game.train.is_some() { 0 } else { game.mt.number as usize };
                Some(audio::TRACK_FIGHTS[(base + self.track_skip) % audio::TRACK_FIGHTS.len()])
            }
            _ if !self.settings.menu_music => None,
            _ => Some((audio::TRACK_MENU + self.track_skip) % n_tracks),
        };
        if let Some(a) = &mut self.audio {
            let mute = std::env::var_os("DUEL_AUDIO_TEST").is_some();
            a.set_volume(if mute { 0.0 } else { self.settings.volume }, if mute { 0.0 } else { self.settings.music_volume });
            a.set_music(track);
            if let Some((p, r, f)) = ears {
                a.set_listener(p, r, f);
            }
            for (s, at, pitch) in sounds.iter().copied() {
                a.play(s, at, pitch);
            }
        }
        self.last_sounds = sounds.iter().map(|(s, at, pitch)| format!("{s:?} at {at:?} pitch {pitch:.2}")).collect();
        self.prev_mods = ctx.input(|i| i.modifiers);
        self.watchdog.snapshot(self.hang_snapshot(ears));
        self.watchdog.stage("between frames (the window system or the graphics driver)");
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
    /// What the game looks like right now, in words, for a hang report.
    fn hang_snapshot(&self, ears: Option<(V3, V3, V3)>) -> String {
        use std::fmt::Write as _;
        let mut s = String::new();
        let screen = match &self.screen {
            Screen::Menu(_) => "menu".to_string(),
            Screen::Lobby { .. } => "joining a lobby".to_string(),
            Screen::Play { game, net } => format!("playing ({:?}, {:?}, {:?} map, {})", game.mt.mode, game.kind, game.world.map, if net.is_some() { "online" } else { "offline" }),
        };
        let _ = writeln!(s, "screen: {screen}");
        let _ = writeln!(s, "paused: {}  mouse captured: {}  fps: {:.0}  last frame: {:.1} ms  disconnected: {:?}", self.paused, self.captured, self.fps, self.frame_dt * 1000.0, self.disconnected);
        match ears {
            Some((p, _, f)) => {
                let _ = writeln!(s, "camera: at ({:.2}, {:.2}, {:.2}) looking ({:.2}, {:.2}, {:.2})  aim yaw {:.3} pitch {:.3}", p.x, p.y, p.z, f.x, f.y, f.z, self.aim_yaw, self.aim_pitch);
            }
            None => {
                let _ = writeln!(s, "camera: menu camera");
            }
        }
        if let Screen::Play { game: g, .. } = &self.screen {
            let _ = writeln!(s, "phase: {:?}  timer: {:.1}  round: {}  match number: {}  game time: {:.2}", g.mt.phase, g.mt.timer, g.mt.round, g.mt.number, g.time);
            match &g.killcam {
                Some(k) => {
                    let _ = writeln!(s, "kill cam: victim {} killer {:?} tool {} headshot {} t {:.2} of {:.1}", k.victim, k.killer, k.tool, k.head, k.t, KILLCAM_TIME);
                }
                None => {
                    let _ = writeln!(s, "kill cam: none");
                }
            }
            let _ = writeln!(s, "slow motion left: {:.2}  hit freeze left: {:.2}  leftover time: {:.4}", g.slowmo, g.hitstop, self.acc);
            let _ = writeln!(s, "last kill: {:?}", g.last_kill);
            for (i, f) in g.f.iter().enumerate().filter(|(_, f)| f.present) {
                let _ = writeln!(
                    s,
                    "fighter {i}{}{} '{}': alive {} hp {:.0} at ({:.2}, {:.2}, {:.2}) vel ({:.1}, {:.1}, {:.1}) yaw {:.3} {:?} kills {} wins {} dash {:.2} slide {:.2} scope {:.2} reload {:.2} ground {} finisher {}",
                    if i == g.local { " (you)" } else { "" },
                    if f.is_bot { " (bot)" } else { "" },
                    f.name, f.alive, f.hp, f.pos.x, f.pos.y, f.pos.z, f.vel.x, f.vel.y, f.vel.z, f.yaw, f.weapon, f.kills, f.wins, f.dash_t, f.sliding, f.scope_t, f.reload_t, f.on_ground, f.fin
                );
            }
            let _ = writeln!(s, "banner: {:?}", g.banner.as_ref().map(|b| &b.0));
            let _ = writeln!(s, "last events:");
            for l in &g.feed {
                let line = match &l.text {
                    Some(t) => t.clone(),
                    None => format!("{} [{}{}] {}", l.killer.as_ref().map_or("-", |k| k.0.as_str()), l.tool, if l.head { ", headshot" } else { "" }, l.victim.0),
                };
                let _ = writeln!(s, "  {line}");
            }
        }
        let _ = writeln!(s, "sounds last frame: {}", if self.last_sounds.is_empty() { "none".to_string() } else { self.last_sounds.join("; ") });
        s
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        let shot_frame: u32 = std::env::var("DUEL_SHOT_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(120);
        let killcam_now = std::env::var_os("DUEL_SHOT_KILLCAM").is_some()
            && matches!(&self.screen, Screen::Play { game, .. } if game.killcam.as_ref().map_or(false, |k| k.t > 1.3));
        if (self.frames >= shot_frame || killcam_now) && !self.shot_requested {
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
        let sens = self.settings.sens * 0.0024 / self.ads_zoom.get().max(1.0);
        let b = self.settings.binds;
        let prev = self.prev_mods;
        ctx.input(|i| {
            if self.chat_open {
                return; // typing: the chat box has the keyboard
            }
            if i.key_pressed(Key::Escape) && self.rebinding.is_none() {
                toggle_pause = true;
            }
            self.show_scores = b[A_SCORES].down(i);
            if i.key_pressed(Key::F5) {
                self.start_request = true;
            }
            if b[A_MUSIC].tapped(i, prev) {
                self.settings.music_on = !self.settings.music_on;
                self.music_note = Some((if self.settings.music_on { "Music on" } else { "Music off" }, 1.5));
            }
            if i.key_pressed(Key::N) {
                self.train_step += 1;
            }
            if i.key_pressed(Key::P) {
                self.train_step -= 1;
            }
            if i.key_pressed(Key::Enter) || b[A_CHAT].tapped(i, prev) {
                self.chat_open = true;
                self.chat_text.clear();
                return;
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
            if b[A_JUMP].tapped(i, prev) {
                self.latch.jump = true;
            }
            // double-tap a direction to dodge
            let now = i.time;
            for a in [A_FWD, A_LEFT, A_BACK, A_RIGHT] {
                if let Bind::Key(k) = b[a] {
                    if b[a].tapped(i, prev) {
                        if let Some((lk, lt)) = self.last_tap {
                            if lk == k && now - lt < 0.25 {
                                self.latch.dash = true;
                            }
                        }
                        self.last_tap = Some((k, now));
                    }
                }
            }
            if b[A_DASH].tapped(i, prev) || b[A_DASH2].tapped(i, prev) {
                self.latch.dash = true;
            }
            if b[A_FIRE].tapped(i, prev) {
                self.latch.fire_pressed = true;
            }
            if b[A_RELOAD].tapped(i, prev) {
                self.latch.reload = true;
            }
            if b[A_KICK].tapped(i, prev) {
                self.latch.kick = true;
            }
            if b[A_FINISH].tapped(i, prev) {
                self.latch.finish = true;
            }
            if b[A_ITEM].tapped(i, prev) {
                self.latch.item = true;
            }
            for (k, a) in [A_MELEE, A_GUN1, A_GUN2].into_iter().enumerate() {
                if b[a].tapped(i, prev) {
                    self.latch.select = Some(k as u8);
                }
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
        self.pad_input(captured);
        if toggle_pause && !self.match_over() && self.disconnected.is_none() {
            if self.paused && self.pause_settings {
                self.pause_settings = false; // Esc steps back out of the settings first
                self.settings.save();
            } else {
                self.paused = !self.paused;
            }
        }
    }

    /// The controller in a match: sticks move and look, buttons do every action (see pad.rs for
    /// the layout). D-pad down opens quick chat.
    fn pad_input(&mut self, captured: bool) {
        use pad::btn;
        self.pad_held = PadHeld::default();
        let st = self.pad;
        if !st.connected || !captured || self.chat_open {
            self.quick_chat = None;
            return;
        }
        if st.down(btn::VIEW) {
            self.show_scores = true;
        }
        let dt = self.frame_dt.clamp(0.0, 0.1);
        let Screen::Play { game, .. } = &self.screen else { return };
        // quick chat: the d-pad or stick picks a line, A sends it, B closes
        if let Some(mut k) = self.quick_chat {
            let n = pad::QUICK_CHAT.len();
            // the stick steps once per push (back to the middle between steps)
            let stick = if st.ly > 0.6 { -1 } else if st.ly < -0.6 { 1 } else { 0 };
            let stick_step = if stick != 0 && !self.qc_stick_out { stick } else { 0 };
            self.qc_stick_out = stick != 0;
            if st.tapped(btn::UP) || stick_step < 0 {
                k = (k + n - 1) % n;
            }
            if st.tapped(btn::DOWN) || stick_step > 0 {
                k = (k + 1) % n;
            }
            self.quick_chat = Some(k);
            if st.tapped(btn::A) {
                self.chat_send = Some(pad::QUICK_CHAT[k].to_string());
                self.quick_chat = None;
            } else if st.tapped(btn::B) || st.tapped(btn::MENU) {
                self.quick_chat = None;
            }
            return;
        }
        // look: a curve on the right stick, slowed a little over an enemy (aim assist)
        let me = &game.f[game.local];
        let assist = if self.settings.aim_assist && me.alive && !me.weapon.is_melee() && game.aim_enemy(game.local, self.aim_yaw, self.aim_pitch).is_some() { 0.35 } else { 0.0 };
        let sens = self.settings.pad_sens / self.ads_zoom.get().max(1.0);
        let (dyaw, dpitch) = pad::look(st.rx, st.ry, sens, self.settings.pad_invert, assist, dt);
        self.aim_yaw = wrap_angle(self.aim_yaw + dyaw);
        self.aim_pitch = (self.aim_pitch + dpitch).clamp(-1.4, 1.4);
        self.pad_held = PadHeld { fwd: st.ly, right: st.lx, fire: st.down(btn::RT), guard: st.down(btn::LT), crouch: st.down(btn::B) };
        let cur = me.cur;
        let l = &mut self.latch;
        l.jump |= st.tapped(btn::A);
        l.dash |= st.tapped(btn::LB) || st.tapped(btn::L3);
        l.fire_pressed |= st.tapped(btn::RT);
        l.reload |= st.tapped(btn::X);
        l.kick |= st.tapped(btn::RB);
        l.finish |= st.tapped(btn::R3);
        l.item |= st.tapped(btn::UP);
        if st.down(btn::VIEW) && game.train.is_some() {
            // Training: View + d-pad left / right steps through the lessons
            if st.tapped(btn::LEFT) {
                self.train_step -= 1;
            }
            if st.tapped(btn::RIGHT) {
                self.train_step += 1;
            }
        } else {
            if st.tapped(btn::LEFT) {
                l.select = Some(0);
            }
            if st.tapped(btn::RIGHT) {
                l.select = Some(if cur == 1 { 2 } else { 1 });
            }
        }
        if st.tapped(btn::Y) {
            l.select = Some(((cur + 1) % 3) as u8);
        }
        if st.tapped(btn::DOWN) {
            self.quick_chat = Some(0);
            self.qc_stick_out = true;
        }
    }


    fn current_input(&self, ctx_keys: &KeysHeld) -> Input {
        let mut inp = Input { yaw: self.aim_yaw, pitch: self.aim_pitch, ..Default::default() };
        if !self.captured {
            return inp;
        }
        let ph = self.pad_held;
        inp.fwd = ((ctx_keys.w as i32 - ctx_keys.s as i32) as f32 + ph.fwd).clamp(-1.0, 1.0);
        inp.right = ((ctx_keys.d as i32 - ctx_keys.a as i32) as f32 + ph.right).clamp(-1.0, 1.0);
        inp.fire = ctx_keys.fire || ph.fire;
        inp.guard = ctx_keys.guard || ph.guard;
        inp.crouch = ctx_keys.crouch || ph.crouch;
        inp.jump = self.latch.jump;
        inp.dash = self.latch.dash;
        inp.fire_pressed = self.latch.fire_pressed;
        inp.reload = self.latch.reload;
        inp.kick = self.latch.kick;
        inp.finish = self.latch.finish;
        inp.item = self.latch.item;
        inp.select = self.latch.select;
        inp
    }

    fn step_play(&mut self, dt: f32) {
        if let Some((_, t)) = self.music_note.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                self.music_note = None;
            }
        }
        let keys = self.keys_snapshot();
        let mut inp = self.current_input(&keys);
        if self.chat_open {
            inp = Input { yaw: self.aim_yaw, pitch: self.aim_pitch, ..Default::default() };
        }
        let mut chat_send = self.chat_send.take();
        if self.autoplay && !self.test_chat_done {
            if let Some(text) = std::env::var("DUEL_CHAT").ok() {
                if let Screen::Play { game, .. } = &self.screen {
                    if game.time > 4.0 {
                        chat_send = Some(text);
                        self.test_chat_done = true;
                    }
                }
            }
        }
        let vote_req = std::mem::take(&mut self.vote_request);
        let ready_req = self.ready_request.take();
        let start_req = std::mem::take(&mut self.start_request);
        let autostart = self.autostart;
        let train_step = std::mem::take(&mut self.train_step);
        let sim = self.net_sim();
        let fill_bots = self.settings.fill_bots;
        let fill_skill = BOT_LEVELS[self.settings.bot_level.min(BOT_LEVELS.len() - 1)].1;
        self.watchdog.stage("Gauntlet leaderboard");
        self.gauntlet_frame();
        self.watchdog.stage("career stats (saving)");
        self.career_frame();
        self.watchdog.stage("network");
        let Screen::Play { game, net } = &mut self.screen else { return };
        if let Some(n) = net.as_mut() {
            n.sim = sim;
        }
        if train_step != 0 {
            game.train_skip(train_step);
        }

        // network receive
        if let Some(n) = net.as_mut() {
            for msg in n.poll() {
                match msg {
                    Incoming::Join { slot, name } => {
                        if game.f[slot].present && game.f[slot].is_bot {
                            // a bot was in the only free place: it makes room
                            let bn = game.f[slot].name.clone();
                            game.remove_player(slot);
                            game.say(format!("{bn} (bot) left to make room"));
                        }
                        let name = clean_name(&name, slot);
                        let team = game.balance_team();
                        game.add_player(slot, &name, false, None, team);
                        let t = if game.mt.mode.teams() { format!(" ({} team)", TEAM_NAMES[team as usize]) } else { String::new() };
                        game.say(format!("{name} joined{t}"));
                    }
                    Incoming::Leave { slot } => {
                        if game.f[slot].present {
                            let nm = game.f[slot].name.clone();
                            game.remove_player(slot);
                            game.say(format!("{nm} left"));
                        }
                    }
                    Incoming::PeerState { slot, st, events, sent, stale } => {
                        if game.f[slot].present {
                            if !stale {
                                let now = game.time;
                                net::apply_remote(&mut game.f[slot], &st, true, sent, now);
                            }
                            for ev in events {
                                game.handle_remote_event(slot, ev);
                            }
                        }
                    }
                    Incoming::Host(m) => apply_host_msg(game, m),
                    Incoming::HostLeft => {
                        // carry on with a new host if we can
                        if !net::migrate(game, n) && self.disconnected.is_none() {
                            self.disconnected = Some("The host closed the lobby.".into());
                        }
                    }
                    Incoming::Full | Incoming::WrongVersion => {}
                }
            }
            // the host went quiet: after a few seconds the best-connected player takes over
            if !n.is_host && n.last_recv.elapsed().as_secs_f32() > 3.0 && n.succession.is_some() && self.disconnected.is_none() {
                net::migrate(game, n);
            }
            if !n.is_host && n.last_recv.elapsed().as_secs_f32() > 8.0 && self.disconnected.is_none() {
                self.disconnected = Some("Lost connection to the host.".into());
            }
            if n.is_host {
                // bots keep the lobby at the chosen size, stepping aside for real players
                n.soft = (0..game.f.len()).filter(|&s| s != game.local && game.f[s].present && game.f[s].is_bot).collect();
                if fill_bots > 0 && game.mt.phase != Phase::MatchOver {
                    if let Some(msg) = game.fill_bots(fill_bots, fill_skill) {
                        game.say(msg);
                    }
                }
                for (s, ping) in n.pings() {
                    if game.f[s].present {
                        game.f[s].ping_ms = ping;
                    }
                }
                for (s, loss) in n.losses() {
                    if game.f[s].present {
                        game.f[s].loss_pct = (loss * 100.0).round() as u8;
                    }
                }
                if !self.ip_announced {
                    if let Some(Ok(ip)) = self.public_ip.lock().ok().and_then(|g| g.clone()) {
                        self.ip_announced = true;
                        game.say(format!("Internet address: {ip}:{} (needs the port forwarded)", self.settings.port.trim()));
                    }
                }
                let ready = autostart > 0 && game.mt.phase == Phase::Warmup && n.player_count() >= autostart;
                if (start_req && matches!(game.mt.phase, Phase::Warmup | Phase::MatchOver)) || ready {
                    game.start_match();
                }
            }
        } else if start_req && game.mt.phase == Phase::MatchOver {
            game.start_match();
        }

        if let Some(text) = chat_send {
            game.say_chat(&text);
        }
        if vote_req {
            game.vote_rematch();
        }
        if let Some(lo) = ready_req {
            if game.mt.phase == Phase::Fight || game.mt.phase == Phase::MatchOver {
                game.my_loadout = lo; // takes effect at the next respawn
                game.say("Loadout saved - you'll get it when you respawn".into());
            } else {
                game.set_ready(lo);
            }
        }
        if self.paused && game.kind == Kind::Offline {
            self.acc = 0.0;
            return;
        }
        if self.autoplay && std::env::var_os("DUEL_HUD_TEST").is_some() && game.time > 3.0 {
            // test switch: show every HUD effect at once
            let l = game.local;
            let me = game.f[l].pos;
            let right = yaw_right(game.f[l].yaw);
            game.f[l].cur = 2;
            game.f[l].loadout.gun2 = Weapon::Revolver;
            game.f[l].reload_t = 0.7;
            game.kill_t = 1.2;
            game.kill_name = "Vex".into();
            game.streak = 2;
            game.headshot_t = 0.5;
            game.hitmarker = 0.3;
            game.hurt_dirs = vec![(me + right * 6.0, 1.0), (me - yaw_fwd(game.f[l].yaw) * 6.0, 0.6)];
            if game.dmg_nums.is_empty() {
                let p = me + yaw_fwd(game.f[l].yaw) * 6.0 + v3(0.0, 1.8, 0.0);
                game.dmg_nums.push(DmgNum { pos: p, amount: 42, life: 0.8, head: true });
            }
            if game.chat.is_empty() {
                game.add_chat(1, "that was a clean launch, rematch after this?");
            }
            if !game.feed.iter().any(|l| l.head) {
                game.feed.push(FeedLine { killer: Some(("Vex".into(), 1)), victim: ("Ash".into(), 2), tool: "Hand Cannon", head: true, text: None, life: 6.0 });
                game.feed.push(FeedLine { killer: Some(("Tester".into(), 0)), victim: ("Kiro".into(), 3), tool: "Sniper", head: false, text: None, life: 6.0 });
            }
        }
        // test switch: a bot's gunshot as the match-winning kill on you, while you're doing
        // something (DUEL_FINAL_DOING: air, dash, slide, scope, reload); rematch after it
        if self.autoplay {
            if let Ok(gun) = std::env::var("DUEL_FINAL_SHOT") {
                let w = match gun.as_str() {
                    "revolver" => Weapon::Revolver,
                    "shotgun" => Weapon::Shotgun,
                    "smg" => Weapon::Smg,
                    "handcannon" => Weapon::HandCannon,
                    "sniper" => Weapon::Sniper,
                    _ => Weapon::Rifle,
                };
                game.stage_final_shot(w);
                if game.mt.phase == Phase::Fight {
                    let l = game.local;
                    let f = &mut game.f[l];
                    match std::env::var("DUEL_FINAL_DOING").unwrap_or_default().as_str() {
                        "air" if f.on_ground => {
                            f.vel.y = 7.0;
                            f.on_ground = false;
                        }
                        "dash" => {
                            inp.fwd = 1.0;
                            inp.dash = (game.time * 2.0).fract() < 0.05;
                        }
                        "slide" => {
                            inp.fwd = 1.0;
                            inp.crouch = true;
                        }
                        "scope" => {
                            f.loadout.gun2 = Weapon::Sniper;
                            f.cur = 2;
                            f.weapon = Weapon::Sniper;
                            f.scope_t = 1.0;
                            inp.guard = true;
                        }
                        "reload" => f.reload_t = f.reload_t.max(0.8),
                        _ => {}
                    }
                }
                if game.mt.phase == Phase::MatchOver && game.killcam.as_ref().map_or(false, |k| k.t > KILLCAM_TIME + 4.0) && !game.f[game.local].vote {
                    game.vote_rematch();
                }
            }
        }
        // game feel: a brief freeze when a sword hit lands, slow motion for the final kill
        // the last picture showed everyone this far between ticks, from this camera
        game.view_alpha = (self.acc / TICK).clamp(0.0, 1.0);
        game.view_offset = self.cam_view_offset.get();
        self.watchdog.stage("game simulation");
        let steps = game.advance(&mut self.acc, dt, &mut inp);
        if steps > 0 {
            // presses are used once
            self.latch = Latch::default();
        }
        if let Ok(page) = std::env::var("DUEL_GALLERY") {
            gallery_pose(game, &page);
        }
        if self.autoplay && self.show_tuning && std::env::var_os("DUEL_TUNING").is_some() && game.time > 1.0 && !game.f.iter().any(|f| f.present && f.name == "Dummy 15m") {
            let _ = game.spawn_dummy(15.0);
        }
        // test switch: hold a scoped sniper (to photograph the scope)
        if self.autoplay && std::env::var_os("DUEL_SCOPE_TEST").is_some() && game.time > 3.0 {
            let l = game.local;
            let f = &mut game.f[l];
            f.loadout.gun2 = Weapon::Sniper;
            f.cur = 2;
            f.weapon = Weapon::Sniper;
            f.scope_t = 1.0;
        }
        // recoil: the kick goes into the aim, and most of it settles back by itself
        let kick = std::mem::take(&mut game.aim_kick);
        if kick > 0.0 {
            self.aim_pitch = (self.aim_pitch + kick).min(1.4);
            self.kick_back += kick * 0.75;
        }
        if self.kick_back > 0.0 {
            let r = self.kick_back * (dt * 9.0).min(1.0);
            self.aim_pitch -= r;
            self.kick_back -= r;
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
        let sim = self.net_sim();
        let Screen::Lobby { net, waiting_since } = &mut self.screen else { return };
        net.sim = sim;
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
                Incoming::WrongVersion => {
                    self.menu_msg = Some("The host has a different version of Duel Rush. You both need the same build: share the newest one and try again.".into());
                    self.screen = Screen::Menu(Page::Join);
                    return;
                }
                _ => {}
            }
        }
        if first.is_none() && *waiting_since > 15.0 {
            self.menu_msg = Some("Couldn't reach the host. Check that: the address is their Internet address from their Host screen; they've opened the lobby; UDP port 27960 is forwarded on their router to their PC; Windows Firewall allows Duel Rush on their PC (the Host screen checks this); and you both have the same version of the game.".into());
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
                let lo = Loadout::random(&mut g.rng);
                g.f[you].loadout = lo;
                g.set_ready(lo);
            }
            let host = g.f[0].name.clone();
            g.say(format!("Joined {host}'s lobby"));
            let addr = self.settings.join_addr.trim().to_string();
            let mode = g.mt.mode.name().to_string();
            self.remember_server(addr, host, mode);
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
        self.cam_view_offset.set(V3::ZERO);
        if let (Some(v), true) = (test_view(), std::env::var_os("DUEL_GALLERY").is_some()) {
            return (Cam::new(v3(v[0], v[1], v[2]), v[3], v[4], v[5], rect), game.f[game.local].pos);
        }
        if std::env::var_os("DUEL_GALLERY").is_some() {
            let l = game.local;
            return (Cam::new(v3(0.0, 2.0, 5.2), 0.0, -0.08, 96.0, rect), game.f[l].pos);
        }
        let l = game.local;
        let f = &game.f[l];
        let alpha = (self.acc / TICK).clamp(0.0, 1.0);
        let body = f.prev_pos.lerp(f.pos, alpha);
        // test switch: DUEL_VIEW=x,y,z,yaw,pitch[,fov] puts the camera there (before / after pictures)
        if let Some(v) = test_view() {
            return (Cam::new(v3(v[0], v[1], v[2]), v[3], v[4], v[5], rect), body);
        }
        // the camera follows the mouse right away, not at the next game tick
        let (yaw, pitch) = if f.is_bot { (f.yaw, f.pitch) } else { (self.aim_yaw, self.aim_pitch) };
        // final kill cam: circle the fallen player in slow motion
        if let Some((pos, target)) = game.killcam_view(alpha) {
            let d = (target - pos).norm();
            return (Cam::new(pos, d.x.atan2(d.z), d.y.clamp(-1.0, 1.0).asin(), 62.0, rect), body);
        }
        // doing a finisher: a short cinematic cut-in from the side, slowly orbiting
        if f.alive && f.fin > 0 && f.fin_target < game.f.len() {
            let t = &game.f[f.fin_target];
            let tp = t.prev_pos.lerp(t.pos, alpha);
            let mid = (body + tp) * 0.5 + v3(0.0, 1.1, 0.0);
            let line = (tp - body).flat();
            let line = if line.len() > 0.05 { line.norm() } else { yaw_fwd(f.yaw) };
            let side = v3(line.z, 0.0, -line.x);
            let pr = (f.fin_t / finisher(f.fin).dur).clamp(0.0, 1.0);
            let o = 0.45 - 0.6 * pr;
            let out = (side * o.cos() - line * (o.sin() * 0.8) + v3(0.0, 0.28, 0.0)).norm();
            let mut dist = 3.3;
            if let Some((hit, _)) = game.world.raycast(mid, out, dist + 0.3) {
                dist = (hit - 0.3).max(0.9);
            }
            let pos = mid + out * dist;
            let d = (mid - pos).norm();
            return (Cam::new(pos, d.x.atan2(d.z), d.y.asin(), 56.0, rect), body);
        }
        // caught in a finisher: a low angle looking up at whoever is doing it
        if let (true, Some(a)) = (f.alive, f.held_by) {
            if a < game.f.len() {
                let at = game.f[a].prev_pos.lerp(game.f[a].pos, alpha);
                let line = (at - body).flat();
                let line = if line.len() > 0.05 { line.norm() } else { yaw_fwd(f.yaw) };
                let side = v3(line.z, 0.0, -line.x);
                let look = at + v3(0.0, 1.55, 0.0);
                let base = body + v3(0.0, 0.35, 0.0);
                let out = (-line * 1.5 + side * 0.65).norm();
                let mut dist = 1.7;
                if let Some((hit, _)) = game.world.raycast(base, out, dist + 0.2) {
                    dist = (hit - 0.2).max(0.5);
                }
                let pos = base + out * dist;
                let d = (look - pos).norm();
                return (Cam::new(pos, d.x.atan2(d.z), d.y.clamp(-1.0, 1.0).asin(), 62.0, rect), body);
            }
        }
        // watching: follow someone still fighting
        let watching = f.spectate || (!f.alive && game.mt.mode.rounds() && game.mt.phase == Phase::Fight);
        if watching {
            let pick = game.mt.duel.iter().map(|&d| d as usize).chain(0..game.f.len()).find(|&i| i < game.f.len() && i != l && game.f[i].present && game.f[i].alive && !game.f[i].spectate);
            if let Some(t) = pick {
                let tf = &game.f[t];
                let tb = tf.prev_pos.lerp(tf.pos, alpha);
                let (pivot, dir, md) = game.camera_rig(tb, tf.yaw, tf.pitch * 0.5);
                let pos = pivot - dir * md;
                return (Cam::new(pos, tf.yaw, tf.pitch * 0.5, self.settings.fov, rect), body);
            }
        }
        let (pivot, dir, max_dist) = game.camera_rig(body, yaw, pitch);
        // right mouse with a gun: aim down sights (zoom in, camera a little closer)
        // (each gun has its own zoom; the SMG has none; the sniper's 3x scope snaps up in 0.15 s)
        let ads = f.alive && f.scope_t > 0.0;
        let want_zoom = if ads { f.weapon.zoom() } else { 1.0 };
        let rate = if f.weapon == Weapon::Sniper { 35.0 } else { 14.0 };
        let zoom = self.ads_zoom.get() + (want_zoom - self.ads_zoom.get()) * (self.frame_dt * rate).min(1.0);
        self.ads_zoom.set(zoom);
        let max_dist = max_dist * (1.0 - 0.3 * (zoom - 1.0));
        // snap in at once when a wall gets in the way, ease back out smoothly
        let mut d = self.cam_dist.get();
        if max_dist < d {
            d = max_dist;
        } else {
            d += (max_dist - d) * (self.frame_dt * 5.0).min(1.0);
        }
        self.cam_dist.set(d);
        let mut pos = pivot - dir * d;
        let ideal_pos = pos;
        let tn = game.tune;
        // what the gun aims at: the camera always looks straight at it, so however the camera
        // trails, bobs or dips, the crosshair stays exactly on target (aim never lags)
        let aim_at = pos + dir * game.world.raycast(pos, dir, 300.0).map_or(300.0, |(t, _)| t.max(1.0));
        // a light spring follow: the camera trails fast movement a touch (not when zoomed)
        let tau = if zoom > 1.05 { 0.0 } else { tn.cam_smooth };
        let dt = self.frame_dt.clamp(0.0, 0.1);
        match self.cam_smooth_pos.get() {
            Some(prev) if tau > 0.001 && (prev - pos).len() < 4.0 => {
                // critically damped spring (no overshoot)
                let omega = 2.0 / tau;
                let x = omega * dt;
                let ex = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
                let change = prev - pos;
                let vel = self.cam_spring_vel.get();
                let temp = (vel + change * omega) * dt;
                self.cam_spring_vel.set((vel - temp * omega) * ex);
                let mut smooth = pos + (change + temp) * ex;
                if (smooth - pos).len() > 1.5 {
                    smooth = pos + (smooth - pos).norm() * 1.5;
                }
                pos = smooth;
            }
            _ => self.cam_spring_vel.set(V3::ZERO),
        }
        self.cam_smooth_pos.set(Some(pos));
        let speed = f.vel.flat().len();
        let run = tn.run_speed.max(1.0);
        if f.alive {
            // a small bob while running (one dip per step, a slight sway per stride)
            if f.on_ground && f.wall.is_none() && f.sliding <= 0.0 && f.dash_t <= 0.0 && tn.cam_bob > 0.0 {
                let k = (speed / run).clamp(0.0, 1.3);
                let ph = f.walk_phase;
                let r = yaw_right(yaw);
                pos += V3::UP * ((ph * 2.0).cos() * tn.cam_bob * k) + r * (ph.sin() * tn.cam_bob * 0.5 * k);
            }
            // dip on a hard landing: down quickly, back up over a third of a second
            if f.land_impact > 0.0 && f.land_t < 0.45 && tn.land_dip > 0.0 {
                let t = f.land_t;
                let curve = if t < 0.06 { t / 0.06 } else { (1.0 - (t - 0.06) / 0.38).max(0.0).powi(2) };
                pos.y -= tn.land_dip * f.land_impact * curve;
            }
        }
        // shots leave along this exact line (see Game::shot_ray)
        self.cam_view_offset.set(pos - ideal_pos);
        let to_aim = (aim_at - pos).norm();
        let (mut yaw, mut pitch) = (to_aim.x.atan2(to_aim.z), to_aim.y.clamp(-1.0, 1.0).asin());
        if game.shake > 0.0 {
            // trauma-style shake: strong hits shake much more than light ones
            let tr = game.shake.min(1.2);
            let k = tr * tr;
            let t = self.wall_t * 38.0;
            let n = |a: f32, b: f32| (t * a).sin() * 0.6 + (t * b + 1.7).sin() * 0.4;
            pos += v3(n(1.3, 2.9), n(1.7, 3.3), n(0.9, 2.3)) * (0.16 * k);
            yaw += n(1.1, 2.6) * 0.025 * k;
            pitch += n(1.5, 3.1) * 0.02 * k;
        }
        // melee contact: a quick jolt along the blow that snaps back (looks only; shots still go
        // along the crosshair line set above)
        if game.jolt_t > 0.0 {
            let k = (game.jolt_t / JOLT_TIME).clamp(0.0, 1.0);
            let age = (1.0 - k) * JOLT_TIME;
            let wobble = (age * 75.0).sin() * k * k;
            let amp = game.jolt_k * 0.06;
            pos += game.jolt_dir * (amp * k * k * 0.7 + amp * wobble * 0.35);
            pitch += wobble * 0.014 * game.jolt_k;
            yaw += game.jolt_dir.dot(yaw_right(yaw)) * wobble * 0.01 * game.jolt_k;
        }
        // FOV kick: a snap wider on dashes and wall runs...
        let want = if f.dash_t > 0.0 {
            14.0
        } else if f.wall.is_some() {
            5.0
        } else {
            0.0
        };
        let cur = self.fov_kick.get();
        let rate = if want > cur { 22.0 } else { 5.0 };
        let kick = cur + (want - cur) * (dt * rate).min(1.0);
        self.fov_kick.set(kick);
        // ...and a gradual widening the faster you go
        let fast = if f.alive { ((speed - run) / run).clamp(0.0, 1.0) } else { 0.0 };
        let cur = self.fov_speed.get();
        let wide = cur + (fast * tn.fov_speed - cur) * (dt * if fast * tn.fov_speed > cur { 3.0 } else { 2.0 }).min(1.0);
        self.fov_speed.set(wide);
        // zoom narrows the view: same centre, so the aim point doesn't move
        let fov = (self.settings.fov + kick + wide).min(150.0);
        let fov = 2.0 * ((fov.to_radians() * 0.5).tan() / zoom).atan().to_degrees();
        let mut cam = Cam::new(pos, yaw, pitch, fov, rect);
        // roll a little into a wall run (the top of the view leans towards the wall)
        let want_roll = match f.wall {
            Some(w) if f.alive && !w.vertical => yaw_right(f.yaw).dot(-w.normal).signum() * tn.cam_roll.to_radians(),
            _ => 0.0,
        };
        let roll = self.cam_roll.get() + (want_roll - self.cam_roll.get()) * (dt * 7.0).min(1.0);
        self.cam_roll.set(roll);
        if roll.abs() > 1e-4 {
            let (s, co) = roll.sin_cos();
            let (r, u) = (cam.right, cam.up);
            cam.right = r * co - u * s;
            cam.up = u * co + r * s;
        }
        (cam, body)
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
    crouch: bool,
}

thread_local! {
    /// A menu sound asked for by a button this frame.
    static UI_SOUND: std::cell::Cell<Option<Sfx>> = const { std::cell::Cell::new(None) };
    static KEYS: std::cell::RefCell<KeysHeld> = std::cell::RefCell::new(KeysHeld::default());
}

fn sample_keys(ctx: &egui::Context, b: &[Bind; N_ACTIONS]) {
    let k = ctx.input(|i| KeysHeld {
        w: b[A_FWD].down(i) || i.key_down(Key::ArrowUp),
        a: b[A_LEFT].down(i) || i.key_down(Key::ArrowLeft),
        s: b[A_BACK].down(i) || i.key_down(Key::ArrowDown),
        d: b[A_RIGHT].down(i) || i.key_down(Key::ArrowRight),
        fire: b[A_FIRE].down(i),
        guard: b[A_GUARD].down(i),
        crouch: b[A_SLIDE].down(i),
    });
    KEYS.with(|c| *c.borrow_mut() = k);
}

/// Test switch DUEL_VIEW=x,y,z,yaw,pitch[,fov]: a fixed camera (for before / after pictures).
fn test_view() -> Option<[f32; 6]> {
    let s = std::env::var("DUEL_VIEW").ok()?;
    let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    (v.len() >= 5).then(|| [v[0], v[1], v[2], v[3], v[4], v.get(5).copied().unwrap_or(78.0)])
}

/// Test switch: line fighters up, each held in one move, to check the animations.
fn gallery_pose(g: &mut Game, page: &str) {
    let page2 = page == "2";
    // melee pages: 3 = each blow at its contact frame, 4 = mid wind-up, 5 = follow-through
    let melee_at = match page {
        "3" => Some(0.0f32),
        "4" => Some(0.5),
        "5" => Some(-0.5),
        _ => None,
    };
    g.mt.phase = Phase::Fight;
    g.mt.timer = 500.0;
    let t = g.time;
    for i in 0..8usize {
        if !g.f[i].present {
            continue;
        }
        g.bots[i] = None;
        let f = &mut g.f[i];
        let p = v3(-6.65 + i as f32 * 1.9, 0.0, 12.0);
        f.pos = p;
        f.prev_pos = p;
        f.yaw = std::f32::consts::PI;
        f.pitch = 0.0;
        f.alive = true;
        f.hp = 100.0;
        f.vel = V3::ZERO;
        f.on_ground = true;
        f.wall = None;
        f.dash_t = 0.0;
        f.slash_t = 0.0;
        f.guard = false;
        f.recoil = 0.0;
        f.hurt_t = 0.0;
        f.stun_t = 0.0;
        f.reload_t = 0.0;
        f.spawn_prot = 0.0;
        f.loadout = Loadout { melee: Weapon::Katana, gun1: Weapon::Shotgun, gun2: Weapon::Revolver, item: 0, body: (i % chars::COUNT) as u8 };
        f.cur = 2;
        f.weapon = Weapon::Revolver;
        let fwd = f.fwd();
        let right = v3(fwd.z, 0.0, -fwd.x);
        if let Some(phase) = melee_at {
            // phase: 0 = the blow lands now, + = that share of the wind-up still to go, - = that
            // share of the follow-through done
            f.cur = 0;
            f.weapon = if i == 7 { Weapon::Greatsword } else { Weapon::Katana };
            f.loadout.melee = f.weapon;
            f.charge_t = 0.0;
            f.charged = false;
            f.kick_t = 0.0;
            f.fin = 0;
            let dur = f.weapon.slash_dur();
            let at = |pre: f32| dur * 0.72 + if phase >= 0.0 { pre * phase } else { dur * 0.72 * phase };
            match i {
                0..=2 => {
                    f.slash_idx = [0u8, 1, 3][i];
                    f.slash_t = at(dur * 0.28).max(0.01);
                }
                3 => {
                    // charging a heavy (held long enough: it's ready)
                    f.charge_t = 0.7;
                    f.charged = true;
                }
                4 => {
                    f.slash_idx = SLASH_HEAVY;
                    f.slash_t = at(dur * 0.53).max(0.01);
                }
                5 => {
                    f.guard = true;
                    f.guard_t = 0.5;
                }
                6 => {
                    let hit = KICK_HIT_AT;
                    f.kick_t = (KICK_TIME - hit - if phase >= 0.0 { -hit * phase } else { (KICK_TIME - hit) * -phase * 0.5 }).clamp(0.01, KICK_TIME);
                }
                _ => {
                    // a finisher (Overhead Crush)
                    let st = finisher(7);
                    f.fin = 7;
                    f.fin_target = 0;
                    f.fin_t = (st.hit - if phase >= 0.0 { st.hit * phase * 0.5 } else { -(1.0 - st.hit) * phase * 0.5 }) * st.dur;
                }
            }
            continue;
        }
        if !page2 {
            match i {
                0 => {}
                1 => {
                    f.cur = 0;
                    f.weapon = Weapon::Katana;
                    f.vel = fwd * 8.0;
                }
                2 => {
                    f.cur = 1;
                    f.weapon = Weapon::Shotgun;
                    f.vel = right * 7.0;
                }
                3 => {
                    f.on_ground = false;
                    f.vel = v3(0.0, 6.0, 0.0);
                }
                4 => {
                    f.on_ground = false;
                    f.vel = v3(0.0, -6.0, 0.0);
                }
                5 => {
                    f.on_ground = false;
                    f.wall = Some(WallRun { normal: right, dir: fwd, t: 0.2, vertical: false, speed: 10.0 });
                    f.vel = fwd * 10.0;
                }
                6 => {
                    f.dash_t = 0.1;
                    f.vel = fwd * 16.0;
                }
                _ => {
                    f.alive = false;
                    f.death_t = 2.0;
                }
            }
        } else {
            f.cur = 0;
            f.weapon = Weapon::Katana;
            match i {
                0..=2 => {
                    let dur = f.weapon.slash_dur();
                    f.slash_idx = [0u8, 1, 3][i];
                    f.slash_t = dur * (1.0 - (t * 1.3 + i as f32 * 0.21).fract()).max(0.05);
                }
                3 => f.guard = true,
                4 => {
                    f.cur = 1;
                    f.weapon = Weapon::Shotgun;
                    f.recoil = (1.0 - (t * 1.5).fract()).max(0.0);
                }
                5 => {
                    f.loadout.gun1 = Weapon::Rifle;
                    f.cur = 1;
                    f.weapon = Weapon::Rifle;
                    f.pitch = 0.3;
                }
                6 | 7 if page2 => {
                    // throwing stars mid-throw: a burst (6) and the wide fan (7)
                    f.loadout.gun1 = Weapon::Shuriken;
                    f.cur = 1;
                    f.weapon = Weapon::Shuriken;
                    f.throw_fan = i == 7;
                    let total = if f.throw_fan { FAN_THROW_TIME } else { THROW_TIME };
                    // (driven by a frame count: the clock may stand still on this screen)
                    static FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                    let n = FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) / 2;
                    f.throw_t = total * (1.0 - (n % 24) as f32 / 24.0).max(0.02);
                }
                6 => {
                    f.loadout.gun1 = Weapon::Rocket;
                    f.cur = 1;
                    f.weapon = Weapon::Rocket;
                }
                _ => f.hurt_t = 0.15,
            }
        }
    }
}

/// A name to show: what they typed, or "Player N".
fn clean_name(n: &str, slot: usize) -> String {
    let n: String = n.trim().chars().filter(|c| !c.is_control()).take(16).collect();
    if n.is_empty() { format!("Player {}", slot + 1) } else { n }
}

/// Follow the host's update: who's here, the match, everyone's fighters and events.
fn apply_host_msg(g: &mut Game, m: net::HostMsg) {
    if m.stale {
        // an older update that came in late: everything in it is out of date except its events
        for (origin, ev) in m.events {
            g.handle_remote_event(origin, ev);
        }
        return;
    }
    g.local = m.you;
    g.set_map(MapId::from_u8(m.map));
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
        g.f[s].ping_ms = e.ping;
        g.f[s].loss_pct = e.loss;
        g.f[s].wins = e.wins;
        g.f[s].spectate = e.spectate;
        if e.spectate && s == g.local {
            g.f[s].alive = false; // watching this round
            g.f[s].respawn_t = 1e9;
        }
        if s != g.local {
            g.f[s].ready = e.ready;
            g.f[s].vote = e.vote;
        }
    }
    let l = g.local;
    if m.you_alive && g.f[l].alive && (g.hp_hold <= 0.0 || m.you_hp < g.f[l].hp - 55.0) {
        // the host keeps the health; ours follows it (except just after we healed: its
        // figure is from before it heard about that)
        g.f[l].hp = m.you_hp;
        g.f[l].ap = m.you_ap;
    }
    for s in 0..g.f.len() {
        if g.f[s].present && s != g.local && !m.roster.iter().any(|e| e.slot == s) {
            g.remove_player(s);
        }
    }
    g.apply_match(m.mode, m.phase, m.timer, m.target, m.team_score, m.number, m.round, m.round_end, m.duel, m.flags);
    let now = g.time;
    for (s, st) in &m.fighters {
        if *s != g.local && g.f[*s].present {
            net::apply_remote(&mut g.f[*s], st, false, m.sent, now);
        }
    }
    for (origin, ev) in m.events {
        g.handle_remote_event(origin, ev);
    }
}

// ---------------------------------------------------------------------- HUD

/// Marks a headshot in a line of text (see `layout_pieces`).
const HS_ICON: &str = "\u{1}";
/// Marks an execution (a finisher) in the kill feed.
const EXEC_ICON: &str = "\u{2}";
const EXEC_RED: Color32 = Color32::from_rgb(235, 70, 60);
const HS_GOLD: Color32 = Color32::from_rgb(255, 205, 70);

/// A piece of a line: some text, or the headshot icon.
enum Piece {
    Text(std::sync::Arc<egui::Galley>),
    /// Width, colour, and which icon (0 headshot, 1 execution).
    Icon(f32, Color32, u8),
}

impl Piece {
    fn width(&self) -> f32 {
        match self {
            Piece::Text(g) => g.size().x,
            Piece::Icon(w, _, _) => *w,
        }
    }
}

fn layout_pieces(p: &egui::Painter, parts: Vec<(String, Color32)>, font: &FontId, size: f32, fade: impl Fn(Color32) -> Color32) -> Vec<Piece> {
    parts
        .into_iter()
        .map(|(t, col)| {
            if t == HS_ICON {
                Piece::Icon(size * 1.3, fade(col), 0)
            } else if t == EXEC_ICON {
                Piece::Icon(size * 1.3, fade(col), 1)
            } else {
                Piece::Text(p.layout_no_wrap(t, font.clone(), fade(col)))
            }
        })
        .collect()
}

fn draw_pieces(p: &egui::Painter, at: Pos2, pieces: Vec<Piece>, size: f32) {
    let mut x = at.x;
    for pc in pieces {
        let w = pc.width();
        match pc {
            Piece::Text(g) => p.galley(Pos2::new(x, at.y), g, Color32::WHITE),
            Piece::Icon(_, col, 0) => headshot_icon(p, Pos2::new(x + w * 0.55, at.y + size * 0.62), size * 0.42, col),
            Piece::Icon(_, col, _) => exec_icon(p, Pos2::new(x + w * 0.55, at.y + size * 0.62), size * 0.45, col),
        }
        x += w;
    }
}

/// Two crossed blades: an execution.
fn exec_icon(p: &egui::Painter, c: Pos2, r: f32, col: Color32) {
    let dark = Color32::from_black_alpha(col.a() / 2);
    for (a, b) in [(Vec2::new(-1.0, 1.0), Vec2::new(1.0, -1.0)), (Vec2::new(1.0, 1.0), Vec2::new(-1.0, -1.0))] {
        p.line_segment([c + a * r + Vec2::splat(1.0), c + b * r + Vec2::splat(1.0)], Stroke::new(2.4, dark));
        p.line_segment([c + a * r, c + b * r], Stroke::new(2.0, col));
        // the guard across each blade, near its handle
        let g = c + a * (r * 0.62);
        let across = Vec2::new(b.y - a.y, a.x - b.x).normalized() * (r * 0.32);
        p.line_segment([g - across, g + across], Stroke::new(1.8, col));
    }
}

/// A small target-on-a-head mark for headshot kills: a ring, four ticks and a dot.
fn headshot_icon(p: &egui::Painter, c: Pos2, r: f32, col: Color32) {
    let dark = Color32::from_black_alpha(col.a() / 2);
    p.circle_stroke(c + Vec2::splat(1.0), r, Stroke::new(2.0, dark));
    p.circle_stroke(c, r, Stroke::new(1.6, col));
    for (dx, dy) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        let d = Vec2::new(dx, dy);
        p.line_segment([c + d * (r * 0.55), c + d * (r * 1.35)], Stroke::new(1.6, col));
    }
    p.circle_filled(c, r * 0.28, col);
}

fn text_shadow(p: &egui::Painter, pos: Pos2, align: Align2, s: &str, size: f32, color: Color32) {
    let font = FontId::proportional(size);
    p.text(pos + Vec2::new(2.0, 2.0), align, s, font.clone(), Color32::from_black_alpha(170));
    p.text(pos, align, s, font, color);
}

/// Draw text from the left, shrinking it (and finally cutting it with "…") so it fits `max_w`.
fn fit_text(p: &egui::Painter, pos: Pos2, s: &str, size: f32, max_w: f32, color: Color32) {
    let mut size = size;
    let mut text = s.to_string();
    for _ in 0..6 {
        let w = p.layout_no_wrap(text.clone(), FontId::proportional(size), color).size().x;
        if w <= max_w || size < 9.0 {
            break;
        }
        size *= 0.9;
    }
    while text.chars().count() > 1 && p.layout_no_wrap(text.clone(), FontId::proportional(size), color).size().x > max_w {
        let n = text.chars().count();
        text = text.chars().take(n.saturating_sub(2)).collect::<String>() + "…";
    }
    text_shadow(p, pos, Align2::LEFT_TOP, &text, size, color);
}

/// Centred text that shrinks to fit `max_w`.
fn fit_center(p: &egui::Painter, pos: Pos2, s: &str, size: f32, max_w: f32, color: Color32) {
    let mut size = size;
    for _ in 0..10 {
        let w = p.layout_no_wrap(s.to_string(), FontId::proportional(size), color).size().x;
        if w <= max_w || size < 9.0 {
            break;
        }
        size *= 0.9;
    }
    text_shadow(p, pos, Align2::CENTER_CENTER, s, size, color);
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
    if !g.mt.mode.teams() {
        if g.mt.mode.rounds() {
            rows.sort_by(|a, b| (-a.wins, -a.kills).cmp(&(-b.wins, -b.kills)));
        } else {
            rows.sort_by(|a, b| (-a.kills, a.deaths).cmp(&(-b.kills, b.deaths)));
        }
    }
    let w = 540.0;
    let h = 70.0 + rows.len() as f32 * 28.0 + if g.mt.mode.teams() { 30.0 } else { 0.0 };
    let r = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 110.0 + h * 0.5), Vec2::new(w, h));
    p.rect_filled(r, 10.0, Color32::from_rgba_unmultiplied(10, 8, 20, 215));
    p.rect_stroke(r, 10.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 150, 70, 110)), egui::StrokeKind::Inside);
    let x0 = r.left() + 20.0;
    let mut y = r.top() + 16.0;
    text_shadow(p, Pos2::new(x0, y), Align2::LEFT_TOP, &format!("PLAYERS  ({}/8)", rows.len()), 16.0, ACCENT);
    text_shadow(p, Pos2::new(r.right() - 175.0, y), Align2::CENTER_TOP, if g.mt.mode.rounds() { "WINS" } else { "KILLS" }, 13.0, Color32::from_gray(200));
    text_shadow(p, Pos2::new(r.right() - 110.0, y), Align2::CENTER_TOP, "DEATHS", 13.0, Color32::from_gray(200));
    text_shadow(p, Pos2::new(r.right() - 45.0, y), Align2::CENTER_TOP, "PING", 13.0, Color32::from_gray(200));
    y += 34.0;
    let mut last_team = 255u8;
    for f in rows {
        if g.mt.mode.teams() && f.team != last_team {
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
        } else if f.slot == g.host_slot && matches!(g.kind, Kind::Client) {
            name.push_str("  (host)");
        }
        if g.mt.phase == Phase::Loadout && f.ready {
            name.push_str("  - ready");
        }
        if g.mt.phase == Phase::Fight && g.mt.mode == Mode::Duel {
            if g.duel_side(f.slot).is_some() {
                name.push_str("  - fighting");
            } else if f.spectate {
                name.push_str("  - waiting");
            }
        } else if g.mt.phase == Phase::Fight && f.spectate {
            name.push_str("  - next round");
        }
        if g.mt.phase == Phase::MatchOver && f.vote {
            name.push_str("  - rematch!");
        }
        p.circle_filled(Pos2::new(x0 + 6.0, y + 9.0), 6.0, player_color(f.color_idx));
        text_shadow(p, Pos2::new(x0 + 20.0, y), Align2::LEFT_TOP, &name, 16.0, Color32::WHITE);
        let score = if g.mt.mode.rounds() { f.wins } else { f.kills };
        text_shadow(p, Pos2::new(r.right() - 175.0, y), Align2::CENTER_TOP, &format!("{}", score), 16.0, Color32::WHITE);
        text_shadow(p, Pos2::new(r.right() - 110.0, y), Align2::CENTER_TOP, &format!("{}", f.deaths), 16.0, Color32::from_gray(200));
        let ping = if f.is_bot || (f.slot == g.local && g.kind == Kind::Host) || g.kind == Kind::Offline { "-".to_string() } else { format!("{}", f.ping_ms) };
        let pc = if f.ping_ms > 150 { Color32::from_rgb(255, 140, 120) } else if f.ping_ms > 80 { Color32::from_rgb(255, 220, 140) } else { Color32::from_rgb(160, 230, 160) };
        text_shadow(p, Pos2::new(r.right() - 45.0, y), Align2::CENTER_TOP, &ping, 16.0, pc);
        y += 28.0;
    }
}

impl App {
    /// F3: frame rate, ping, packet loss, players and traffic. A small tag always shows while
    /// the network test is on, so nobody forgets it.
    /// The sniper's scope: black all round a round lens, with fine crosshairs and range marks.
    fn draw_scope(&self, p: &egui::Painter, rect: Rect) {
        let c = rect.center();
        let r = rect.width().min(rect.height()) * 0.46;
        let outer = rect.size().length();
        let black = Color32::from_rgb(4, 4, 6);
        // the dark surround, as a ring from the lens edge out past the corners
        let mut mesh = egui::epaint::Mesh::default();
        let n = 96u32;
        for k in 0..=n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            let d = Vec2::new(a.cos(), a.sin());
            mesh.colored_vertex(c + d * r, black);
            mesh.colored_vertex(c + d * outer, black);
        }
        for k in 0..n {
            let i = k * 2;
            mesh.add_triangle(i, i + 1, i + 3);
            mesh.add_triangle(i, i + 3, i + 2);
        }
        p.add(egui::Shape::mesh(mesh));
        // a soft dark rim inside the lens
        for (w, a) in [(10.0, 160u8), (22.0, 70), (40.0, 25)] {
            p.circle_stroke(c, r - w * 0.5, Stroke::new(w, Color32::from_black_alpha(a)));
        }
        // crosshairs: thick at the edges, fine in the middle, with range marks
        let line = Color32::from_rgba_unmultiplied(10, 10, 10, 230);
        let thick = Stroke::new(3.0, line);
        let fine = Stroke::new(1.2, line);
        let inner = r * 0.45;
        for (dx, dy) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            let d = Vec2::new(dx, dy);
            p.line_segment([c + d * inner, c + d * r], thick);
            p.line_segment([c + d * 4.0, c + d * inner], fine);
            for m in 1..=4 {
                let at = c + d * (inner * m as f32 / 5.0);
                let side = Vec2::new(-dy, dx) * 4.0;
                p.line_segment([at - side, at + side], fine);
            }
        }
        p.circle_filled(c, 1.6, Color32::from_rgb(230, 40, 40));
    }

    /// The HUD's size: follows the window, and the Couch / TV interface size on top (the UI's
    /// zoom already shrinks the area in points, so this brings the HUD up with it).
    fn hud_scale(&self, rect: Rect) -> f32 {
        let zoom = UI_SCALES[self.settings.ui_scale.min(UI_SCALES.len() - 1)].1;
        ((rect.width() / 1280.0).min(rect.height() / 720.0) * zoom).clamp(0.62, 2.0)
    }

    fn draw_net_overlay(&self, p: &egui::Painter, rect: Rect, g: &Game, n: Option<&Net>) {
        let s = self.hud_scale(rect);
        let x = rect.left() + 14.0 * s;
        let mut y = rect.top() + 14.0 * s;
        let warn = Color32::from_rgb(255, 170, 60);
        if self.settings.net_test && n.is_some() && !self.show_debug {
            text_shadow(p, Pos2::new(x, y), Align2::LEFT_TOP, &format!("NETWORK TEST  +{:.0} ms  {:.0}% loss  (F3)", self.settings.net_lag, self.settings.net_loss), 13.0 * s, warn);
        }
        if !self.show_debug {
            return;
        }
        let mut lines: Vec<(String, Color32)> = Vec::new();
        let white = Color32::from_rgb(235, 235, 235);
        let grey = Color32::from_gray(180);
        let good = |v: f32, ok: f32, bad: f32| if v <= ok { Color32::from_rgb(140, 230, 140) } else if v <= bad { Color32::from_rgb(255, 215, 120) } else { Color32::from_rgb(255, 120, 110) };
        lines.push((format!("FPS {:.0}  ({:.1} ms)", self.fps, 1000.0 / self.fps.max(1.0)), good(-self.fps, -55.0, -30.0)));
        let players = g.present().len();
        match n {
            None => {
                lines.push(("Playing offline (no network)".into(), grey));
                lines.push((format!("Players: {players}"), white));
            }
            Some(n) => {
                let r = n.rates;
                if n.is_host {
                    lines.push((format!("Hosting on {}", n.address()), white));
                    lines.push((format!("Players connected: {} / {}", n.player_count(), MAX_PLAYERS), white));
                    for f in g.f.iter().filter(|f| f.present && f.slot != g.local) {
                        let loss = f.loss_pct as f32;
                        let col = good(f.ping_ms as f32 + loss * 20.0, 120.0, 250.0);
                        lines.push((format!("  {}  ping {} ms  loss {}%", f.name, f.ping_ms, f.loss_pct), col));
                    }
                } else {
                    let me = &g.f[g.local];
                    lines.push((format!("Connected to {}", n.address()), white));
                    lines.push((format!("Players connected: {players}"), white));
                    lines.push((format!("Ping {} ms", me.ping_ms), good(me.ping_ms as f32, 80.0, 160.0)));
                    let down = n.down_loss() * 100.0;
                    let up = me.loss_pct as f32;
                    lines.push((format!("Packet loss  down {:.1}%  up {:.0}%", down, up), good(down.max(up), 1.0, 5.0)));
                }
                lines.push((format!("In {:.0} pkt/s {:.1} KB/s · Out {:.0} pkt/s {:.1} KB/s", r.pkts_in, r.kb_in, r.pkts_out, r.kb_out), grey));
                let delays: Vec<f32> = g.f.iter().filter(|f| f.present && f.slot != g.local).map(|f| f.interp).collect();
                let avg = if delays.is_empty() { INTERP_DELAY } else { delays.iter().sum::<f32>() / delays.len() as f32 };
                lines.push((format!("Others shown {:.0} ms behind for smoothness", avg * 1000.0), grey));
                if let Some(sim) = n.sim {
                    lines.push((format!("NETWORK TEST ON: +{:.0} ms lag, {:.0}% loss", sim.lag_ms, sim.loss * 100.0), warn));
                }
            }
        }
        let fs = 13.5 * s;
        let galleys: Vec<_> = lines.into_iter().map(|(t, c)| p.layout_no_wrap(t, FontId::monospace(fs), c)).collect();
        let w = galleys.iter().map(|g| g.size().x).fold(0.0f32, f32::max) + 20.0 * s;
        let h = galleys.iter().map(|g| g.size().y + 3.0 * s).sum::<f32>() + 14.0 * s;
        let bg = Rect::from_min_size(Pos2::new(x - 6.0 * s, y - 6.0 * s), Vec2::new(w.min(rect.width() * 0.6), h));
        p.rect_filled(bg, 6.0 * s, Color32::from_black_alpha(175));
        y += 1.0 * s;
        for gal in galleys {
            let gh = gal.size().y;
            p.galley(Pos2::new(x + 4.0 * s, y), gal, Color32::WHITE);
            y += gh + 3.0 * s;
        }
    }
}

/// The training lesson card at the top of the screen. Returns the rectangle it took.
fn draw_training(p: &egui::Painter, rect: Rect, s: f32, margin: f32, t: &Training) -> Rect {
    let w = (640.0 * s).min(rect.width() - margin * 2.0);
    let pad = 14.0 * s;
    let total = LESSONS.len();
    let (title, how) = match LESSONS.get(t.lesson) {
        Some(l) => (format!("Lesson {}/{}: {}", t.lesson + 1, total, l.title), l.how.to_string()),
        None => ("Training complete!".to_string(), "Every move learned. Keep practising combos on the dummies, or press P to revisit a lesson.".to_string()),
    };
    let how_g = p.layout(how, FontId::proportional(15.0 * s), Color32::from_rgb(235, 230, 220), w - pad * 2.0);
    let h = 36.0 * s + how_g.size().y + 40.0 * s;
    let r = Rect::from_min_size(Pos2::new(rect.center().x - w * 0.5, rect.top() + margin), Vec2::new(w, h));
    p.rect_filled(r, 8.0 * s, Color32::from_black_alpha(150));
    p.rect_stroke(r, 8.0 * s, Stroke::new(1.5 * s, Color32::from_rgba_unmultiplied(255, 150, 70, 120)), egui::StrokeKind::Inside);
    // title (left) and keys (right)
    let keys = "N skip · P back";
    let keys_g = p.layout_no_wrap(keys.to_string(), FontId::proportional(12.0 * s), Color32::from_gray(170));
    let kw = keys_g.size().x;
    p.galley(Pos2::new(r.right() - pad - kw, r.top() + 12.0 * s), keys_g, Color32::WHITE);
    fit_text(p, Pos2::new(r.left() + pad, r.top() + 8.0 * s), &title, 19.0 * s, w - pad * 3.0 - kw, ACCENT);
    p.galley(Pos2::new(r.left() + pad, r.top() + 36.0 * s), how_g, Color32::WHITE);
    // progress: one pip per repetition needed, or a big tick when the lesson is done
    let py = r.bottom() - 18.0 * s;
    let (have, need) = t.progress();
    if t.done_t > 0.0 {
        text_shadow(p, Pos2::new(r.left() + pad, py), Align2::LEFT_CENTER, "✔ NICE! Next lesson...", 17.0 * s, Color32::from_rgb(120, 230, 120));
    } else if need > 0 {
        let rad = 7.0 * s;
        for k in 0..need {
            let pc = Pos2::new(r.left() + pad + rad + k as f32 * rad * 2.8, py);
            if k < have {
                p.circle_filled(pc, rad, Color32::from_rgb(120, 230, 120));
            } else {
                p.circle_stroke(pc, rad, Stroke::new(2.0 * s, Color32::from_gray(160)));
            }
        }
        let tx = r.left() + pad + need as f32 * rad * 2.8 + 6.0 * s;
        text_shadow(p, Pos2::new(tx, py), Align2::LEFT_CENTER, &format!("{have} / {need}"), 14.0 * s, Color32::from_gray(220));
    }
    // overall progress along the bottom edge
    let frac = t.lesson.min(total) as f32 / total as f32;
    let bar_r = Rect::from_min_size(Pos2::new(r.left() + 8.0 * s, r.bottom() - 5.0 * s), Vec2::new((w - 16.0 * s) * frac, 3.0 * s));
    p.rect_filled(bar_r, 1.0, ACCENT);
    r
}

impl App {
    fn draw_hud(&self, p: &egui::Painter, rect: Rect, cam: &Cam, g: &Game) {
        if test_view().is_some() {
            return; // a clean picture of the map
        }
        let me = &g.f[g.local];
        let c = rect.center();
        // everything scales with the window so nothing overlaps at small sizes (and grows with
        // the Couch / TV interface size)
        let s = self.hud_scale(rect);
        // quick chat (controller): the lines to pick from
        if let Some(k) = self.quick_chat {
            let x = rect.left() + 18.0 * s;
            let mut y = rect.center().y - pad::QUICK_CHAT.len() as f32 * 13.0 * s;
            let box_r = Rect::from_min_size(Pos2::new(x - 8.0 * s, y - 34.0 * s), Vec2::new(230.0 * s, (pad::QUICK_CHAT.len() as f32 * 26.0 + 64.0) * s));
            p.rect_filled(box_r, 6.0 * s, Color32::from_rgba_unmultiplied(10, 8, 20, 220));
            text_shadow(p, Pos2::new(x, y - 28.0 * s), Align2::LEFT_TOP, "QUICK CHAT", 15.0 * s, ACCENT);
            for (i, line) in pad::QUICK_CHAT.iter().enumerate() {
                let col = if i == k { Color32::WHITE } else { Color32::from_gray(170) };
                if i == k {
                    p.rect_filled(Rect::from_min_size(Pos2::new(x - 4.0 * s, y - 2.0 * s), Vec2::new(214.0 * s, 24.0 * s)), 4.0 * s, Color32::from_rgba_unmultiplied(255, 150, 70, 90));
                }
                text_shadow(p, Pos2::new(x + 4.0 * s, y), Align2::LEFT_TOP, line, 16.0 * s, col);
                y += 26.0 * s;
            }
            let a = pad::button_name(pad::btn::A, self.pads.name.as_deref());
            let b = pad::button_name(pad::btn::B, self.pads.name.as_deref());
            text_shadow(p, Pos2::new(x, y + 4.0 * s), Align2::LEFT_TOP, &format!("{a}: send   {b}: close"), 12.5 * s, Color32::from_gray(190));
        }
        let margin = 14.0 * s;
        if let Some(kc) = &g.killcam {
            if kc.t < KILLCAM_TIME {
                let bar_h = rect.height() * 0.11;
                p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(rect.width(), bar_h)), 0.0, Color32::BLACK);
                p.rect_filled(Rect::from_min_size(Pos2::new(rect.left(), rect.bottom() - bar_h), Vec2::new(rect.width(), bar_h)), 0.0, Color32::BLACK);
                let a = ((kc.t / 0.3).min(1.0) * 255.0) as u8;
                let title = if kc.tool == "EXECUTED" { "EXECUTION" } else { "FINAL KILL" };
                text_shadow(p, Pos2::new(c.x, rect.top() + bar_h * 0.5), Align2::CENTER_CENTER, title, 30.0 * s, Color32::from_rgba_unmultiplied(255, 150, 70, a));
                let victim = &g.f[kc.victim].name;
                let col = Color32::from_rgba_unmultiplied(240, 230, 220, a);
                let mut parts: Vec<(String, Color32)> = Vec::new();
                match kc.killer.filter(|&k| k < g.f.len()) {
                    Some(k) => {
                        parts.push((format!("{}  [{}]", g.f[k].name, kc.tool), col));
                        if kc.head {
                            parts.push((HS_ICON.into(), HS_GOLD));
                            parts.push((" HEADSHOT".into(), Color32::from_rgba_unmultiplied(255, 215, 80, a)));
                        }
                        parts.push((format!("  {victim}"), col));
                    }
                    None => parts.push((format!("{victim} went down"), col)),
                }
                let size = 20.0 * s;
                let font = FontId::proportional(size);
                let pieces = layout_pieces(p, parts, &font, size, |c| c);
                let total: f32 = pieces.iter().map(|pc| pc.width()).sum();
                let y = rect.bottom() - bar_h * 0.5 - size * 0.6;
                draw_pieces(p, Pos2::new(c.x - total * 0.5, y), pieces, size);
                return;
            }
        }

        // perfect parry: a white flash
        if g.parry_flash > 0.0 {
            let a = ((g.parry_flash / 0.3).clamp(0.0, 1.0) * 110.0) as u8;
            p.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(255, 255, 255, a));
        }
        // hurt flash vignette
        if g.hurt_flash > 0.0 {
            let a = (g.hurt_flash * 200.0) as u8;
            let col = Color32::from_rgba_unmultiplied(200, 0, 0, a / 2);
            let w = 60.0 * s;
            p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(rect.width(), w)), 0.0, col);
            p.rect_filled(Rect::from_min_size(Pos2::new(rect.left(), rect.bottom() - w), Vec2::new(rect.width(), w)), 0.0, col);
            p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(w, rect.height())), 0.0, col);
            p.rect_filled(Rect::from_min_size(Pos2::new(rect.right() - w, rect.top()), Vec2::new(w, rect.height())), 0.0, col);
        }
        if me.hp < 30.0 && me.alive {
            let a = ((g.time * 4.0).sin().abs() * 50.0) as u8;
            p.rect_stroke(rect.shrink(4.0), 0.0, Stroke::new(8.0 * s, Color32::from_rgba_unmultiplied(220, 0, 0, a)), egui::StrokeKind::Inside);
        }

        // name tags over everyone you can see
        for other in g.f.iter().filter(|o| o.present && !o.spectate && o.slot != g.local) {
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
            text_shadow(p, sp + Vec2::new(0.0, -12.0 * s), Align2::CENTER_BOTTOM, &label, 15.0 * s, col);
            let r = Rect::from_center_size(sp, Vec2::new(70.0 * s, 8.0 * s));
            bar(p, r, other.hp / MAX_HP, if enemy { Color32::from_rgb(230, 60, 60) } else { Color32::from_rgb(80, 200, 110) });
        }

        // damage numbers: pop in, drift up, bigger for bigger hits, gold for headshots
        for d in &g.dmg_nums {
            if let Some(sp) = cam.project(d.pos) {
                let a = (d.life / 0.9 * 255.0).clamp(0.0, 255.0) as u8;
                let pop = 1.0 + 0.7 * ((d.life - 0.72) / 0.18).clamp(0.0, 1.0);
                let size = (16.0 + (d.amount.min(80) as f32 / 80.0) * 12.0) * pop * s;
                let col = if d.head { Color32::from_rgba_unmultiplied(255, 215, 50, a) } else if d.amount >= 40 { Color32::from_rgba_unmultiplied(255, 150, 90, a) } else { Color32::from_rgba_unmultiplied(255, 255, 255, a) };
                text_shadow(p, sp, Align2::CENTER_CENTER, &format!("{}", d.amount), size, col);
                if d.head {
                    text_shadow(p, sp + Vec2::new(0.0, size * 0.75), Align2::CENTER_CENTER, "HEADSHOT", 11.0 * s, col);
                }
            }
        }

        // where hits are coming from: red arcs around the crosshair
        let flat_fwd = v3(cam.fwd.x, 0.0, cam.fwd.z).norm();
        let flat_right = v3(cam.right.x, 0.0, cam.right.z).norm();
        for (src, life) in &g.hurt_dirs {
            let d = (*src - me.pos).flat();
            if d.len() < 0.3 {
                continue;
            }
            let ang = d.dot(flat_right).atan2(d.dot(flat_fwd));
            let a = ((life / 1.2).clamp(0.0, 1.0) * 220.0) as u8;
            let col = Color32::from_rgba_unmultiplied(235, 30, 30, a);
            let (r0, r1) = (105.0 * s, 128.0 * s);
            let mut mesh = egui::epaint::Mesh::default();
            let steps = 10;
            for k in 0..=steps {
                let t = ang - 0.32 + 0.64 * k as f32 / steps as f32;
                let dir = Vec2::new(t.sin(), -t.cos());
                // thicker in the middle: an arrow-ish wedge
                let mid = 1.0 - ((k as f32 / steps as f32) - 0.5).abs() * 2.0;
                mesh.colored_vertex(c + dir * r0, col);
                mesh.colored_vertex(c + dir * (r0 + (r1 - r0) * (0.35 + 0.65 * mid)), col);
            }
            for k in 0..steps as u32 {
                let i = k * 2;
                mesh.add_triangle(i, i + 1, i + 3);
                mesh.add_triangle(i, i + 3, i + 2);
            }
            p.add(egui::Shape::mesh(mesh));
        }

        // crosshair: exactly as wide as the next shot's spread
        if me.alive {
            let [cr, cg, cb] = self.settings.cross_color;
            let col = Color32::from_rgba_unmultiplied(cr, cg, cb, 235);
            let on_enemy = self.settings.cross_red && !me.weapon.is_melee() && {
                let (yaw, pitch) = if me.is_bot { (me.yaw, me.pitch) } else { (self.aim_yaw, self.aim_pitch) };
                g.aim_enemy(g.local, yaw, pitch).is_some()
            };
            let dot_col = if on_enemy { Color32::from_rgb(255, 45, 45) } else { col };
            let target = spread_px(me.spread(), cam.focal);
            // open fast (fully within 50 ms, before any gun can fire again), close smoothly
            let mut gap = self.cross_gap.get();
            let dt = self.frame_dt.max(0.0);
            if target > gap {
                gap = (gap + ((target - gap) * dt * 30.0).max(dt * 2000.0)).min(target);
            } else {
                gap += (target - gap) * (dt * 12.0).min(1.0);
            }
            self.cross_gap.set(gap);
            let size = self.settings.cross_size * s;
            // the sniper's scope replaces the crosshair once it's up
            let scoped = me.weapon == Weapon::Sniper && self.ads_zoom.get() > 2.2;
            if scoped {
                self.draw_scope(p, rect);
            } else {
                draw_crosshair(p, c, self.settings.cross_style, col, dot_col, size, gap, me.weapon == Weapon::Shotgun);
            }
            // the reload ring and labels sit outside the spread
            let ring_r = (30.0 * s).max(gap + 12.0 * s);
            // reload: a ring that fills up around the crosshair
            if !me.weapon.is_melee() && me.reload_t > 0.0 {
                let total = me.weapon.reload_time().max(0.01);
                let prog = (1.0 - me.reload_t / total).clamp(0.0, 1.0);
                let r = ring_r;
                p.circle_stroke(c, r, Stroke::new(5.0 * s, Color32::from_black_alpha(120)));
                let n = 48;
                let pts: Vec<Pos2> = (0..=((n as f32 * prog) as usize))
                    .map(|k| {
                        let t = k as f32 / n as f32 * std::f32::consts::TAU;
                        c + Vec2::new(t.sin(), -t.cos()) * r
                    })
                    .collect();
                if pts.len() >= 2 {
                    p.add(egui::Shape::line(pts, Stroke::new(4.0 * s, ACCENT)));
                }
            } else if !me.weapon.is_melee() && me.mag[me.cur] == 0 {
                text_shadow(p, c + Vec2::new(0.0, ring_r + 4.0 * s), Align2::CENTER_CENTER, "R to reload", 13.0 * s, Color32::from_rgb(255, 200, 120));
            }
            // hit confirm: a small X on the crosshair, thicker for headshots, red for a kill
            if g.hitmarker > 0.0 {
                let a = ((g.hitmarker / 0.18).min(1.0) * 255.0) as u8;
                let head = g.hit_head;
                let kill = g.kill_t > 1.2;
                let hc = if kill { Color32::from_rgba_unmultiplied(255, 50, 40, a) } else { Color32::from_rgba_unmultiplied(255, 255, 255, a) };
                let pop = 1.0 + ((g.hitmarker - 0.2) / 0.15).clamp(0.0, 1.0) * 0.25;
                let r0 = (gap * 0.72 + 4.0 * s).max(5.0 * s) * pop;
                let r1 = r0 + 6.0 * s * if head || kill { 1.25 } else { 1.0 };
                let th = if head { 3.2 } else { 1.8 } * s.max(0.8);
                for (dx, dy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                    let dir = Vec2::new(dx, dy) * std::f32::consts::FRAC_1_SQRT_2;
                    p.line_segment([c + dir * (r0 - 1.0), c + dir * (r1 + 1.0)], Stroke::new(th + 2.0, Color32::from_black_alpha((a as f32 * 0.6) as u8)));
                    p.line_segment([c + dir * r0, c + dir * r1], Stroke::new(th, hc));
                }
            }
            if g.headshot_t > 0.0 {
                let a = (g.headshot_t / 0.7 * 255.0).min(255.0) as u8;
                text_shadow(p, c - Vec2::new(0.0, (50.0 * s).max(ring_r + 14.0 * s)), Align2::CENTER_CENTER, "HEADSHOT", 16.0 * s, Color32::from_rgba_unmultiplied(255, 215, 50, a));
            }
        }

        // ---- finisher prompt over an enemy you can finish
        if me.alive && me.fin == 0 {
            if let Some(t) = g.finish_target(g.local) {
                if let Some(sp) = cam.project(g.f[t].pos + v3(0.0, 2.3, 0.0)) {
                    let key = if self.pad_active { pad::button_name(pad::btn::R3, self.pads.name.as_deref()).to_string() } else { self.settings.binds[A_FINISH].name() };
                    let pulse = 0.75 + 0.25 * (g.time * 10.0).sin();
                    let text = format!("{key}  FINISH");
                    let gal = p.layout_no_wrap(text.clone(), FontId::proportional(14.0 * s), Color32::WHITE);
                    let r = Rect::from_center_size(sp, gal.size() + Vec2::new(16.0 * s, 8.0 * s));
                    p.rect_filled(r, 5.0 * s, Color32::from_rgba_unmultiplied(120, 15, 15, (200.0 * pulse) as u8));
                    p.rect_stroke(r, 5.0 * s, Stroke::new(1.5 * s, Color32::from_rgba_unmultiplied(255, 120, 100, (255.0 * pulse) as u8)), egui::StrokeKind::Outside);
                    p.galley(r.center() - gal.size() * 0.5, gal, Color32::WHITE);
                }
            }
        }

        // ---- blade combo: hits in a row (the fifth is a KO)
        let mc = g.my_combo;
        if me.alive && mc.n >= 2 && g.time - mc.last < 1.2 {
            let fade = (1.0 - (g.time - mc.last - 0.8).max(0.0) / 0.4).clamp(0.0, 1.0);
            let a = (255.0 * fade) as u8;
            let y = c.y - 46.0 * s;
            let hot = mc.n + 1 >= COMBO_KO;
            let col = if hot { Color32::from_rgba_unmultiplied(255, 120, 60, a) } else { Color32::from_rgba_unmultiplied(255, 220, 140, a) };
            text_shadow(p, Pos2::new(c.x, y), Align2::CENTER_CENTER, &format!("{} HIT COMBO", mc.n), 15.0 * s, col);
            for k in 0..COMBO_KO {
                let x = c.x + (k as f32 - (COMBO_KO as f32 - 1.0) * 0.5) * 11.0 * s;
                let filled = k < mc.n;
                let pc = if filled { col } else { Color32::from_rgba_unmultiplied(255, 255, 255, a / 4) };
                p.circle_filled(Pos2::new(x, y + 13.0 * s), 3.2 * s, pc);
            }
        }

        // ---- melee: the guard meter under the crosshair, and the heavy's charge ring
        if me.alive && me.weapon.is_melee() {
            let show = me.guard || me.guard_hp < GUARD_MAX - 0.5 || me.guard_broke > 0.0;
            if show {
                let w = 64.0 * s;
                let h = 5.0 * s;
                let top = c.y + 30.0 * s;
                let bg = Rect::from_min_size(Pos2::new(c.x - w * 0.5, top), Vec2::new(w, h));
                p.rect_filled(bg.expand(1.5 * s), 2.0 * s, Color32::from_rgba_unmultiplied(0, 0, 0, 150));
                let k = (me.guard_hp / GUARD_MAX).clamp(0.0, 1.0);
                let col = if me.guard_broke > 0.0 {
                    // broken: flashing red
                    if (g.time * 14.0).sin() > 0.0 { Color32::from_rgb(255, 60, 50) } else { Color32::from_rgb(120, 20, 20) }
                } else if k < 0.3 {
                    Color32::from_rgb(255, 110, 60)
                } else if me.guard {
                    Color32::from_rgb(150, 205, 255)
                } else {
                    Color32::from_rgb(110, 160, 210)
                };
                p.rect_filled(Rect::from_min_size(bg.min, Vec2::new(w * k, h)), 2.0 * s, col);
                if me.guard_broke > 0.0 {
                    text_shadow(p, Pos2::new(c.x, top + h + 10.0 * s), Align2::CENTER_CENTER, "GUARD BROKEN", 13.0 * s, Color32::from_rgb(255, 90, 70));
                }
            }
            if me.charge_t > 0.15 {
                let k = ((me.charge_t - 0.15) / (CHARGE_TIME - 0.15)).clamp(0.0, 1.0);
                let r = 22.0 * s;
                let col = if me.charged {
                    let pulse = 0.6 + 0.4 * (g.time * 18.0).sin();
                    Color32::from_rgba_unmultiplied(255, 240, 200, (255.0 * pulse) as u8)
                } else {
                    Color32::from_rgba_unmultiplied(255, 190, 90, 200)
                };
                let n = 40;
                let pts: Vec<Pos2> = (0..=((n as f32 * k) as usize)).map(|q| {
                    let a = -std::f32::consts::FRAC_PI_2 + q as f32 / n as f32 * std::f32::consts::TAU;
                    c + Vec2::new(a.cos(), a.sin()) * r
                }).collect();
                if pts.len() > 1 {
                    p.add(egui::Shape::line(pts, Stroke::new(if me.charged { 3.5 } else { 2.5 } * s, col)));
                }
                if me.charged {
                    p.circle_stroke(c, r + 5.0 * s + 3.0 * s * (g.time * 18.0).sin().abs(), Stroke::new(1.5 * s, col));
                }
            }
        }

        // ---- dash: a burst of motion blur at the screen edges
        let dash_k = if me.alive && me.dash_t > 0.0 { (me.dash_t / g.tune.dash_time.max(0.05)).clamp(0.0, 1.0).sqrt() * g.tune.dash_blur } else { 0.0 };
        if dash_k > 0.02 {
            let k = dash_k.min(1.5);
            // smeared edges: a soft frame that fades towards the middle
            let inner = rect.shrink2(rect.size() * (0.12 + 0.06 * k));
            let edge = Color32::from_rgba_unmultiplied(225, 232, 240, (70.0 * k).min(120.0) as u8);
            let clear = Color32::from_rgba_unmultiplied(225, 232, 240, 0);
            let mut mesh = egui::Mesh::default();
            let (o, i) = ([rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()], [inner.left_top(), inner.right_top(), inner.right_bottom(), inner.left_bottom()]);
            for side in 0..4 {
                let n = (side + 1) % 4;
                let base = mesh.vertices.len() as u32;
                mesh.colored_vertex(o[side], edge);
                mesh.colored_vertex(o[n], edge);
                mesh.colored_vertex(i[n], clear);
                mesh.colored_vertex(i[side], clear);
                mesh.add_triangle(base, base + 1, base + 2);
                mesh.add_triangle(base, base + 2, base + 3);
            }
            p.add(egui::Shape::mesh(mesh));
            // long streaks rushing past
            let half = rect.size() * 0.5;
            let reach = half.length();
            let frame = (g.time * 40.0) as u32;
            for k2 in 0..56u32 {
                let h = |salt: u32| -> f32 {
                    let mut x = k2.wrapping_mul(2246822519) ^ frame.wrapping_mul(3266489917) ^ salt.wrapping_mul(668265263);
                    x ^= x >> 15;
                    x = x.wrapping_mul(0x2c1b3c6d);
                    x ^= x >> 12;
                    (x & 0xffff) as f32 / 65535.0
                };
                let ang = (k2 as f32 + h(1)) / 56.0 * std::f32::consts::TAU;
                let d = Vec2::new(ang.cos(), ang.sin());
                let r0 = reach * (0.55 + 0.25 * h(2) - 0.12 * k.min(1.0));
                let r1 = reach * 1.05;
                let a = ((40.0 + 90.0 * h(3)) * k.min(1.0)) as u8;
                p.line_segment([c + d * r0, c + d * r1], Stroke::new((1.5 + 3.0 * h(4)) * s, Color32::from_rgba_unmultiplied(240, 246, 255, a)));
            }
        }

        // ---- movement flow: speed lines at the edges, a chain counter, perfect timing
        if me.alive {
            let flat = me.vel.flat().len();
            let flow_k = me.flow.min(FLOW_MAX) as f32 / FLOW_MAX as f32;
            let fast = ((flat - RUN_SPEED) / (RUN_SPEED * 0.8)).clamp(0.0, 1.0);
            let strength = (flow_k * 0.75 + fast * 0.45).min(1.0);
            if strength > 0.05 {
                let n = 34;
                let half = rect.size() * 0.5;
                let reach = half.length();
                let frame = (g.time * 24.0) as u32;
                for k in 0..n {
                    // a cheap hash so the streaks flicker without any state
                    let h = |salt: u32| -> f32 {
                        let mut x = (k as u32).wrapping_mul(2654435761) ^ frame.wrapping_mul(40503) ^ salt.wrapping_mul(97);
                        x ^= x >> 13;
                        x = x.wrapping_mul(0x5bd1e995);
                        x ^= x >> 15;
                        (x & 0xffff) as f32 / 65535.0
                    };
                    if h(1) > strength * 0.9 + 0.1 {
                        continue;
                    }
                    let ang = (k as f32 + h(2) * 0.6) / n as f32 * std::f32::consts::TAU;
                    let dir = Vec2::new(ang.cos(), ang.sin());
                    let r1 = reach * 1.02;
                    let r0 = reach * (0.86 - 0.16 * strength - h(3) * 0.08);
                    let a = (60.0 + 120.0 * strength * h(4)) as u8;
                    let col = Color32::from_rgba_unmultiplied(255, 245, 230, a);
                    p.line_segment([c + dir * r0, c + dir * r1], Stroke::new((1.0 + 1.5 * h(5)) * s, col));
                }
            }
            let base_y = rect.bottom() - margin - 128.0 * s;
            if me.flow > 0 {
                let pop = 1.0 + (me.flow_pop / 0.35).clamp(0.0, 1.0) * 0.35;
                let col = if me.flow >= FLOW_MAX { Color32::from_rgb(255, 190, 80) } else { Color32::from_rgb(170, 230, 255) };
                let text = format!("FLOW x{}  +{}%", me.flow, (me.flow.min(FLOW_MAX) as f32 * FLOW_STEP * 100.0).round() as i32);
                text_shadow(p, Pos2::new(c.x, base_y), Align2::CENTER_CENTER, &text, 16.0 * s * pop, col);
                // little pips
                for k in 0..FLOW_MAX {
                    let pc = Pos2::new(c.x + (k as f32 - (FLOW_MAX as f32 - 1.0) * 0.5) * 14.0 * s, base_y + 15.0 * s);
                    if k < me.flow {
                        p.circle_filled(pc, 3.5 * s, col);
                    } else {
                        p.circle_stroke(pc, 3.5 * s, Stroke::new(1.0, Color32::from_white_alpha(90)));
                    }
                }
            }
            if me.perfect_t > 0.0 {
                let a = ((me.perfect_t / 0.6).min(1.0) * 255.0) as u8;
                let rise = (0.6 - me.perfect_t) * 30.0 * s;
                text_shadow(p, Pos2::new(c.x, base_y - 24.0 * s - rise), Align2::CENTER_CENTER, "PERFECT", 15.0 * s, Color32::from_rgba_unmultiplied(255, 225, 120, a));
            }
        }
        // health back after a kill
        if g.heal_pop > 0.0 {
            let a = ((g.heal_pop / 0.5).min(1.0) * 255.0) as u8;
            let rise = (1.6 - g.heal_pop) * 14.0 * s;
            let at = Pos2::new(rect.left() + margin + 150.0 * s, rect.bottom() - margin - 104.0 * s - rise);
            text_shadow(p, at, Align2::CENTER_CENTER, &format!("+{} HP", KILL_HEAL as i32), 17.0 * s, Color32::from_rgba_unmultiplied(110, 235, 120, a));
        }

        // kill confirmation icon
        if g.kill_t > 0.0 {
            let a = (g.kill_t.min(0.5) / 0.5 * 255.0) as u8;
            let pop = 1.0 + 0.5 * ((g.kill_t - 1.4) / 0.2).clamp(0.0, 1.0);
            let ic = c + Vec2::new(0.0, 150.0 * s);
            let r = 16.0 * s * pop;
            let red = Color32::from_rgba_unmultiplied(230, 40, 40, a);
            p.circle_filled(ic, r, Color32::from_rgba_unmultiplied(20, 10, 10, a / 2));
            p.circle_stroke(ic, r, Stroke::new(2.5 * s, red));
            // crossed blades
            let k = r * 0.6;
            p.line_segment([ic + Vec2::new(-k, -k), ic + Vec2::new(k, k)], Stroke::new(3.0 * s, Color32::from_rgba_unmultiplied(255, 255, 255, a)));
            p.line_segment([ic + Vec2::new(k, -k), ic + Vec2::new(-k, k)], Stroke::new(3.0 * s, Color32::from_rgba_unmultiplied(255, 255, 255, a)));
            text_shadow(p, ic + Vec2::new(0.0, r + 12.0 * s), Align2::CENTER_CENTER, &format!("ELIMINATED {}", g.kill_name.to_uppercase()), 14.0 * s, Color32::from_rgba_unmultiplied(255, 120, 110, a));
            let streak = match g.streak {
                2 => Some("DOUBLE KILL"),
                3 => Some("TRIPLE KILL"),
                n if n >= 4 => Some("RAMPAGE"),
                _ => None,
            };
            if let Some(t) = streak {
                text_shadow(p, ic + Vec2::new(0.0, r + 33.0 * s), Align2::CENTER_CENTER, t, 18.0 * s, Color32::from_rgba_unmultiplied(255, 190, 80, a));
            }
        }

        // ---- bottom-left: health and armor
        let pw = 300.0 * s;
        let ph = 92.0 * s;
        let left = Rect::from_min_size(Pos2::new(rect.left() + margin, rect.bottom() - margin - ph), Vec2::new(pw, ph));
        p.rect_filled(left, 8.0 * s, Color32::from_black_alpha(110));
        let pad = 12.0 * s;
        text_shadow(p, left.left_top() + Vec2::new(pad, 8.0 * s), Align2::LEFT_TOP, &me.name, 16.0 * s, Color32::from_rgb(240, 230, 220));
        let num_w = 44.0 * s;
        let hp_r = Rect::from_min_size(left.left_top() + Vec2::new(pad, 36.0 * s), Vec2::new(pw - pad * 2.0 - num_w, 18.0 * s));
        bar(p, hp_r, me.hp / MAX_HP, Color32::from_rgb(220, 60, 60));
        text_shadow(p, Pos2::new(left.right() - pad, hp_r.center().y), Align2::RIGHT_CENTER, &format!("{}", me.hp.max(0.0).ceil() as i32), 18.0 * s, Color32::WHITE);
        let ap_r = Rect::from_min_size(left.left_top() + Vec2::new(pad, 64.0 * s), Vec2::new((pw - pad * 2.0 - num_w) * 0.6, 12.0 * s));
        bar(p, ap_r, me.ap / MAX_AP, Color32::from_rgb(80, 150, 240));
        text_shadow(p, Pos2::new(ap_r.right() + 8.0 * s, ap_r.center().y), Align2::LEFT_CENTER, &format!("AP {}", me.ap.max(0.0).ceil() as i32), 13.0 * s, Color32::from_rgb(170, 200, 255));

        // ---- bottom-right: weapons, in three rows that never share space
        let right = Rect::from_min_size(Pos2::new(rect.right() - margin - pw, rect.bottom() - margin - ph), Vec2::new(pw, ph));
        p.rect_filled(right, 8.0 * s, Color32::from_black_alpha(110));
        // row 1: weapon name (left) and ammo (right)
        let ammo = match me.weapon {
            w if w.is_melee() => {
                if me.guard {
                    "GUARD".to_string()
                } else {
                    "∞".to_string()
                }
            }
            w => format!("{} / {}", me.mag[me.cur], w.mag()),
        };
        let ammo_col = if !me.weapon.is_melee() && me.reload_t > 0.0 { Color32::from_gray(140) } else { Color32::WHITE };
        let ammo_g = p.layout_no_wrap(ammo, FontId::proportional(22.0 * s), ammo_col);
        let ammo_w = ammo_g.size().x;
        let row1_y = right.top() + 6.0 * s;
        p.galley(Pos2::new(right.right() - pad - ammo_w, row1_y), ammo_g, Color32::WHITE);
        let name_room = pw - pad * 3.0 - ammo_w;
        fit_text(p, Pos2::new(right.left() + pad, row1_y + 4.0 * s), me.weapon.name(), 16.0 * s, name_room, ACCENT);
        // row 2: the three slots
        let slot_gap = 6.0 * s;
        let slot_w = (pw - pad * 2.0 - slot_gap * 2.0) / 3.0;
        let slot_h = 32.0 * s;
        let row2_y = right.top() + 36.0 * s;
        for k in 0..3usize {
            let r = Rect::from_min_size(Pos2::new(right.left() + pad + k as f32 * (slot_w + slot_gap), row2_y), Vec2::new(slot_w, slot_h));
            let sel = me.cur == k;
            p.rect_filled(r, 5.0 * s, if sel { Color32::from_rgba_unmultiplied(255, 150, 70, 170) } else { Color32::from_black_alpha(120) });
            text_shadow(p, r.left_center() + Vec2::new(6.0 * s, 0.0), Align2::LEFT_CENTER, &format!("{}", k + 1), 13.0 * s, Color32::from_gray(210));
            fit_text(p, Pos2::new(r.left() + 18.0 * s, r.center().y - 7.0 * s), me.slot_weapon(k).name(), 12.0 * s, slot_w - 22.0 * s, Color32::WHITE);
        }
        // row 3: item (left) and dash (right)
        let row3_y = right.bottom() - 14.0 * s;
        // dashes: one pip each, the refilling one fills up
        let most = g.tune.dash_charges.round().max(1.0) as usize;
        let pip = 9.0 * s;
        let pgap = 4.0 * s;
        let pips_w = most as f32 * pip + (most as f32 - 1.0) * pgap;
        let dash_g = p.layout_no_wrap("Dash".to_string(), FontId::proportional(13.0 * s), Color32::from_rgb(180, 220, 200));
        let dash_w = dash_g.size().x + 6.0 * s + pips_w;
        let px0 = right.right() - pad - pips_w;
        p.galley(Pos2::new(right.right() - pad - dash_w, row3_y - dash_g.size().y * 0.5), dash_g, Color32::WHITE);
        for k in 0..most {
            let fill = (me.dash_charges - k as f32).clamp(0.0, 1.0);
            let r = Rect::from_min_size(Pos2::new(px0 + k as f32 * (pip + pgap), row3_y - pip * 0.5), Vec2::splat(pip));
            p.rect_filled(r, 2.0 * s, Color32::from_black_alpha(140));
            if fill > 0.0 {
                let col = if fill >= 1.0 { Color32::from_rgb(120, 230, 190) } else { Color32::from_rgb(70, 120, 110) };
                p.rect_filled(Rect::from_min_max(Pos2::new(r.left(), r.bottom() - pip * fill), r.max), 2.0 * s, col);
            }
        }
        let item = format!("G  {} x{}", ITEM_NAMES[me.loadout.item.min(2) as usize], me.items);
        fit_text(p, Pos2::new(right.left() + pad, row3_y - 8.0 * s), &item, 13.0 * s, pw - pad * 3.0 - dash_w, Color32::from_rgb(220, 220, 180));

        // ---- chat, above the health panel, wrapped to its own column
        let chat_w = (rect.width() * 0.42).max(pw);
        let mut cy = left.top() - 8.0 * s;
        for line in g.chat.iter().rev() {
            let a = (line.life.min(1.0) * 255.0) as u8;
            let pc = player_color(line.color);
            let mut job = egui::text::LayoutJob::default();
            job.append(&format!("{}: ", line.name), 0.0, egui::TextFormat { font_id: FontId::proportional(15.0 * s), color: Color32::from_rgba_unmultiplied(pc.r(), pc.g(), pc.b(), a), ..Default::default() });
            job.append(&line.text, 0.0, egui::TextFormat { font_id: FontId::proportional(15.0 * s), color: Color32::from_rgba_unmultiplied(240, 240, 240, a), ..Default::default() });
            job.wrap.max_width = chat_w - 16.0 * s;
            let gal = p.layout_job(job);
            let h = gal.size().y;
            cy -= h + 6.0 * s;
            if cy < rect.top() + rect.height() * 0.4 {
                break;
            }
            p.rect_filled(Rect::from_min_size(Pos2::new(left.left(), cy - 3.0 * s), Vec2::new(gal.size().x + 16.0 * s, h + 6.0 * s)), 4.0, Color32::from_black_alpha((a as f32 * 0.5) as u8));
            p.galley(Pos2::new(left.left() + 8.0 * s, cy), gal, Color32::WHITE);
        }

        // ---- top: the training lesson, or scores and timer
        let sb = if let Some(t) = &g.train {
            draw_training(p, rect, s, margin, t)
        } else {
            Rect::from_center_size(Pos2::new(c.x, rect.top() + margin + 26.0 * s), Vec2::new(420.0 * s, 52.0 * s))
        };
        if g.train.is_none() {
        p.rect_filled(sb, 8.0 * s, Color32::from_black_alpha(130));
        let red = player_color(0);
        let blue = player_color(1);
        let gauntlet_left = (0..g.f.len()).filter(|&s| s != g.local && g.f[s].present && g.f[s].alive).count();
        let tt = match g.mt.phase {
            Phase::Fight if g.gauntlet.as_ref().is_some_and(|gt| gt.break_t > 0.0) => "NEXT WAVE".into(),
            Phase::Fight if g.mt.mode == Mode::Gauntlet => format!("{gauntlet_left} LEFT"),
            Phase::Fight if g.mt.mode.rounds() && g.mt.round_end > 0.0 => "NEXT ROUND".into(),
            Phase::Fight => {
                let secs = g.mt.timer.max(0.0).ceil() as i32;
                if g.mt.mode.rounds() { format!("R{}  {}:{:02}", g.mt.round, secs / 60, secs % 60) } else { format!("{}:{:02}", secs / 60, secs % 60) }
            }
            Phase::Warmup => "WARM-UP".into(),
            Phase::Countdown => "GET READY".into(),
            Phase::Loadout => format!("LOADOUT {:.0}", g.mt.timer.max(0.0)),
            Phase::MatchOver => "OVER".into(),
        };
        text_shadow(p, sb.center() + Vec2::new(0.0, -6.0 * s), Align2::CENTER_CENTER, &tt, 18.0 * s, Color32::WHITE);
        let sub = match &g.gauntlet {
            Some(gt) => format!("Gauntlet · best {}", gt.best),
            None => format!("{} · first to {} {}", g.mt.mode.name(), g.mt.target, g.mt.mode.goal_word()),
        };
        text_shadow(p, sb.center() + Vec2::new(0.0, 15.0 * s), Align2::CENTER_CENTER, &sub, 11.0 * s, Color32::from_gray(200));
        let side_w = 120.0 * s;
        match g.mt.mode {
            Mode::Gauntlet => {
                let (wave, score) = g.gauntlet.as_ref().map_or((0, 0), |gt| (gt.wave, gt.score));
                text_shadow(p, sb.left_center() + Vec2::new(14.0 * s, -8.0 * s), Align2::LEFT_CENTER, "WAVE", 12.0 * s, Color32::from_gray(210));
                text_shadow(p, sb.left_center() + Vec2::new(14.0 * s, 10.0 * s), Align2::LEFT_CENTER, &format!("{wave}"), 19.0 * s, Color32::from_rgb(255, 190, 90));
                text_shadow(p, sb.right_center() - Vec2::new(14.0 * s, 8.0 * s), Align2::RIGHT_CENTER, "SCORE", 12.0 * s, Color32::from_gray(210));
                text_shadow(p, sb.right_center() - Vec2::new(14.0 * s, -10.0 * s), Align2::RIGHT_CENTER, &format!("{score}"), 19.0 * s, Color32::WHITE);
            }
            Mode::Tdm | Mode::Ctf => {
                text_shadow(p, sb.left_center() + Vec2::new(14.0 * s, 0.0), Align2::LEFT_CENTER, "RED", 17.0 * s, red);
                text_shadow(p, sb.right_center() - Vec2::new(14.0 * s, 0.0), Align2::RIGHT_CENTER, "BLUE", 17.0 * s, blue);
                text_shadow(p, sb.center() + Vec2::new(-95.0 * s, 0.0), Align2::CENTER_CENTER, &format!("{}", g.mt.team_score[0]), 30.0 * s, red);
                text_shadow(p, sb.center() + Vec2::new(95.0 * s, 0.0), Align2::CENTER_CENTER, &format!("{}", g.mt.team_score[1]), 30.0 * s, blue);
            }
            _ => {
                let rounds = g.mt.mode.rounds();
                let sc = |f: &Fighter| if rounds { f.wins } else { f.kills };
                let leader = g.f.iter().filter(|f| f.present).max_by_key(|f| sc(f));
                text_shadow(p, sb.left_center() + Vec2::new(14.0 * s, -8.0 * s), Align2::LEFT_CENTER, "YOU", 12.0 * s, Color32::from_gray(210));
                text_shadow(p, sb.left_center() + Vec2::new(14.0 * s, 10.0 * s), Align2::LEFT_CENTER, &format!("{} {}", sc(me), if rounds { "wins" } else { "kills" }), 17.0 * s, player_color(me.color_idx));
                if let Some(l) = leader {
                    text_shadow(p, sb.right_center() - Vec2::new(14.0 * s, 8.0 * s), Align2::RIGHT_CENTER, "LEADER", 12.0 * s, Color32::from_gray(210));
                    let lt = format!("{} {}", l.name, sc(l));
                    let gal = p.layout_no_wrap(lt.clone(), FontId::proportional(17.0 * s), Color32::WHITE);
                    let lw = gal.size().x.min(side_w);
                    fit_text(p, Pos2::new(sb.right() - 14.0 * s - lw, sb.center().y + 1.0 * s), &lt, 17.0 * s, side_w, player_color(l.color_idx));
                }
            }
        }
        }

        // training combo counter, right of the crosshair
        if let Some(t) = &g.train {
            let x = rect.right() - margin - 10.0 * s;
            let y = c.y - 60.0 * s;
            if t.combo > 0 {
                let a = ((t.combo_t / 0.4).min(1.0) * 255.0) as u8;
                let hot = (t.combo as f32 / 10.0).min(1.0);
                let col = Color32::from_rgba_unmultiplied(255, (220.0 - 140.0 * hot) as u8, (90.0 - 60.0 * hot) as u8, a);
                text_shadow(p, Pos2::new(x, y), Align2::RIGHT_CENTER, &format!("{} HIT", t.combo), (30.0 + 10.0 * hot) * s, col);
                text_shadow(p, Pos2::new(x, y + 30.0 * s), Align2::RIGHT_CENTER, "COMBO", 16.0 * s, Color32::from_rgba_unmultiplied(255, 240, 220, a));
                text_shadow(p, Pos2::new(x, y + 52.0 * s), Align2::RIGHT_CENTER, &format!("{} damage", t.combo_dmg.round() as i32), 14.0 * s, Color32::from_rgba_unmultiplied(230, 230, 230, a));
                // time left to keep it going
                let bw = 110.0 * s;
                let br = Rect::from_min_size(Pos2::new(x - bw, y + 66.0 * s), Vec2::new(bw, 5.0 * s));
                p.rect_filled(br, 2.0, Color32::from_black_alpha(140));
                p.rect_filled(Rect::from_min_size(br.min, Vec2::new(bw * (t.combo_t / 1.5).clamp(0.0, 1.0), br.height())), 2.0, col);
            }
            if t.best_combo > 0 {
                text_shadow(p, Pos2::new(x, y + 86.0 * s), Align2::RIGHT_CENTER, &format!("Best combo: {}", t.best_combo), 13.0 * s, Color32::from_gray(200));
            }
        }

        if let Some((note, t)) = self.music_note {
            let a = ((t / 0.4).min(1.0) * 255.0) as u8;
            text_shadow(p, Pos2::new(c.x, sb.bottom() + 40.0 * s), Align2::CENTER_CENTER, note, 16.0 * s, Color32::from_rgba_unmultiplied(200, 220, 255, a));
        }
        // banner and phase messages
        if let Some((text, _)) = &g.banner {
            fit_center(p, Pos2::new(c.x, rect.top() + rect.height() * 0.3), text, 46.0 * s, rect.width() - 40.0, ACCENT);
        }
        if g.mt.phase == Phase::Countdown {
            let n = g.mt.timer.ceil().max(1.0) as i32;
            text_shadow(p, Pos2::new(c.x, rect.top() + rect.height() * 0.42), Align2::CENTER_CENTER, &format!("{n}"), 64.0 * s, Color32::WHITE);
        }
        let mut under_sb = sb.bottom() + 8.0 * s;
        if g.mt.phase == Phase::Warmup {
            let players = g.present().len();
            let hint = if g.kind == Kind::Host {
                let (public, _) = self.public_addr_text();
                match public {
                    Some(a) => format!("Warm-up · {players} in the lobby · friends join {a} · F5 starts the match"),
                    None => format!("Warm-up · {players} in the lobby · press F5 to start the match"),
                }
            } else {
                format!("Warm-up · {players} in the lobby · waiting for the host to start")
            };
            fit_center(p, Pos2::new(c.x, under_sb + 10.0 * s), &hint, 16.0 * s, rect.width() - 40.0, Color32::from_rgb(255, 230, 180));
            under_sb += 26.0 * s;
            let fw = self.fw_status.lock().ok().and_then(|g| *g);
            if g.kind == Kind::Host && fw.is_some_and(|f| matches!(f, firewall::FwStatus::Blocked | firewall::FwStatus::NoRule)) {
                let warn = "Windows Firewall may block friends - press Esc and choose \"Allow Duel Rush through Windows Firewall\"";
                fit_center(p, Pos2::new(c.x, under_sb + 10.0 * s), warn, 15.0 * s, rect.width() - 40.0, Color32::from_rgb(255, 150, 100));
                under_sb += 24.0 * s;
            }
        }
        if g.mt.phase == Phase::Fight && g.mt.mode == Mode::Duel {
            let name = |d: u8| if (d as usize) < g.f.len() && g.f[d as usize].present { g.f[d as usize].name.clone() } else { "?".into() };
            let line = format!("{}  vs  {}", name(g.mt.duel[0]), name(g.mt.duel[1]));
            fit_center(p, Pos2::new(c.x, under_sb + 10.0 * s), &line, 16.0 * s, rect.width() - 40.0, Color32::from_rgb(255, 230, 180));
            under_sb += 24.0 * s;
        }
        if g.mt.mode == Mode::Ctf && matches!(g.mt.phase, Phase::Fight | Phase::Countdown) {
            let status = |t: usize| -> String {
                let fl = g.mt.flags[t];
                match fl.state {
                    1 if (fl.carrier as usize) < g.f.len() => format!("taken by {}", g.f[fl.carrier as usize].name),
                    2 => format!("dropped ({:.0}s)", fl.timer.max(0.0)),
                    _ => "at base".into(),
                }
            };
            let line = format!("Red flag: {}   ·   Blue flag: {}", status(0), status(1));
            fit_center(p, Pos2::new(c.x, under_sb + 10.0 * s), &line, 15.0 * s, rect.width() - 40.0, Color32::from_rgb(235, 225, 210));
            under_sb += 24.0 * s;
        }
        if me.spectate && g.mt.phase == Phase::Fight {
            let pos_in_line = g.queue.iter().position(|&q| q == g.local);
            let msg = match (g.mt.mode, pos_in_line) {
                (Mode::Duel, Some(0)) => "Watching - you're up next".to_string(),
                (Mode::Duel, Some(n)) => format!("Watching - {} ahead of you", n),
                (Mode::Duel, None) => "Watching - waiting for your turn".to_string(),
                _ => "Watching - you join at the next round".to_string(),
            };
            text_shadow(p, Pos2::new(c.x, rect.top() + rect.height() * 0.62), Align2::CENTER_CENTER, &msg, 20.0 * s, Color32::from_rgb(220, 230, 255));
        } else if !me.alive && g.mt.phase != Phase::MatchOver && g.mt.mode.rounds() && g.mt.phase == Phase::Fight {
            text_shadow(p, Pos2::new(c.x, rect.top() + rect.height() * 0.62), Align2::CENTER_CENTER, "You're out - watching until the round ends", 20.0 * s, Color32::from_rgb(255, 200, 200));
        } else if !me.alive && g.mt.phase != Phase::MatchOver {
            let secs = me.respawn_t.max(0.0).ceil() as i32;
            text_shadow(p, Pos2::new(c.x, rect.top() + rect.height() * 0.62), Align2::CENTER_CENTER, &format!("You're down - back in {secs}"), 20.0 * s, Color32::from_rgb(255, 200, 200));
        }

        // ---- kill feed (top right; drops below the score bar when the window is narrow)
        let font = FontId::proportional(15.0 * s);
        let mut lines = Vec::new();
        for line in &g.feed {
            let a = (line.life.min(1.0) * 255.0) as u8;
            let fade = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a);
            let mut parts: Vec<(String, Color32)> = Vec::new();
            if let Some(t) = &line.text {
                parts.push((t.clone(), Color32::from_rgb(255, 240, 220)));
            } else {
                if let Some((k, kc)) = &line.killer {
                    parts.push((k.clone(), player_color(*kc)));
                    if line.tool == "EXECUTED" {
                        parts.push(("  ".into(), Color32::from_gray(220)));
                        parts.push((EXEC_ICON.into(), EXEC_RED));
                        parts.push((" EXECUTED".into(), EXEC_RED));
                    } else {
                        parts.push((format!("  [{}]", line.tool), Color32::from_gray(220)));
                    }
                    if line.head {
                        parts.push((HS_ICON.into(), HS_GOLD));
                    }
                    parts.push(("  ".into(), Color32::from_gray(220)));
                }
                parts.push((line.victim.0.clone(), player_color(line.victim.1)));
                if line.killer.is_none() {
                    parts.push(("  went down".into(), Color32::from_gray(220)));
                }
            }
            let pieces = layout_pieces(p, parts, &font, 15.0 * s, |col| fade(col));
            let total: f32 = pieces.iter().map(|pc| pc.width()).sum();
            lines.push((pieces, total, a));
        }
        let widest = lines.iter().map(|l| l.1).fold(0.0f32, f32::max);
        let feed_left = rect.right() - margin - widest - 12.0 * s;
        let mut y = if feed_left < sb.right() + 8.0 * s { under_sb + 4.0 * s } else { rect.top() + margin };
        let row_h = 24.0 * s;
        for (galleys, total, a) in lines {
            let back = Rect::from_min_size(Pos2::new(rect.right() - margin - total - 12.0 * s, y - 3.0 * s), Vec2::new(total + 12.0 * s, row_h - 2.0 * s));
            p.rect_filled(back, 4.0, Color32::from_black_alpha((a as f32 * 0.45) as u8));
            let x = rect.right() - margin - 6.0 * s - total;
            draw_pieces(p, Pos2::new(x, y), galleys, 15.0 * s);
            y += row_h;
        }

        // player list: hold Tab (always shown in the warm-up and at the end)
        if self.show_scores || g.mt.phase == Phase::MatchOver || (g.mt.phase == Phase::Warmup && g.time < 6.0) {
            draw_scoreboard(p, rect, g);
        }
        // controls hint, only when it fits between the two bottom panels
        if self.captured && g.time < 12.0 {
            let hint = if self.pad_active {
                "Menu: pause · View: players · Y / d-pad: weapons · LB: dash · A at a wall: wall-run · D-pad down: quick chat"
            } else {
                "Esc: menu · Tab: players · 1/2/3: weapons · Shift: dash · Space at a wall: wall-run · Enter: chat"
            };
            let gal = p.layout_no_wrap(hint.to_string(), FontId::proportional(13.0 * s), Color32::from_gray(220));
            let room = right.left() - left.right() - 16.0 * s;
            if gal.size().x <= room {
                text_shadow(p, Pos2::new(c.x, rect.bottom() - margin - 8.0 * s), Align2::CENTER_BOTTOM, hint, 13.0 * s, Color32::from_gray(220));
            } else {
                fit_center(p, Pos2::new(c.x, left.top() - 14.0 * s), hint, 13.0 * s, rect.width() - 40.0, Color32::from_gray(220));
            }
        }
    }

    // ------------------------------------------------------------------ menus

    fn _unused_panel(&self) {}
}

fn loadout_picker(ui: &mut egui::Ui, lo: &mut Loadout) {
    egui::Grid::new("loadout_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.colored_label(ACCENT, "Fighter");
        ui.horizontal_wrapped(|ui| {
            for k in 0..chars::COUNT as u8 {
                ui.selectable_value(&mut lo.body, k, chars::name(k)).on_hover_text(chars::blurb(k));
            }
        });
        ui.end_row();
        ui.colored_label(ACCENT, "Melee");
        ui.horizontal(|ui| {
            for w in Weapon::MELEE {
                ui.selectable_value(&mut lo.melee, w, w.name()).on_hover_text(w.blurb());
            }
        });
        ui.end_row();
        ui.colored_label(ACCENT, "Gun 1");
        ui.horizontal_wrapped(|ui| {
            for w in Weapon::GUNS {
                ui.selectable_value(&mut lo.gun1, w, w.name()).on_hover_text(w.blurb());
            }
        });
        ui.end_row();
        ui.colored_label(ACCENT, "Gun 2");
        ui.horizontal_wrapped(|ui| {
            for w in Weapon::GUNS {
                if w != lo.gun1 {
                    ui.selectable_value(&mut lo.gun2, w, w.name()).on_hover_text(w.blurb());
                }
            }
        });
        ui.end_row();
        ui.colored_label(ACCENT, "Item");
        ui.horizontal(|ui| {
            for k in 0..3u8 {
                ui.selectable_value(&mut lo.item, k, ITEM_NAMES[k as usize]).on_hover_text(ITEM_BLURBS[k as usize]);
            }
        });
        ui.end_row();
    });
    if lo.gun2 == lo.gun1 {
        *lo = Loadout::from_bytes(lo.to_bytes());
    }
    ui.label(egui::RichText::new(format!(
        "{}: {}  ·  {}: {}  ·  {}: {}  ·  {}: {}  ·  {}: {}",
        chars::name(lo.body),
        chars::blurb(lo.body),
        lo.melee.name(),
        lo.melee.blurb(),
        lo.gun1.name(),
        lo.gun1.blurb(),
        lo.gun2.name(),
        lo.gun2.blurb(),
        ITEM_NAMES[lo.item.min(2) as usize],
        ITEM_BLURBS[lo.item.min(2) as usize]
    )).size(13.0).color(Color32::from_gray(200)));
}

fn map_picker(ui: &mut egui::Ui, map: &mut u8) {
    ui.label("Map");
    ui.horizontal_wrapped(|ui| {
        for m in MapId::ALL {
            ui.selectable_value(map, m as u8, m.name()).on_hover_text(m.blurb());
        }
    });
    let m = MapId::from_u8(*map);
    ui.label(egui::RichText::new(format!("{}: {}", m.name(), m.blurb())).size(13.0).color(Color32::from_gray(200)));
}

fn mode_picker(ui: &mut egui::Ui, mode: &mut u8) {
    ui.label("Mode");
    ui.horizontal_wrapped(|ui| {
        for m in Mode::ALL {
            ui.selectable_value(mode, m as u8, m.name()).on_hover_text(m.blurb());
        }
    });
    let m = Mode::from_u8(*mode);
    ui.label(egui::RichText::new(format!("{}: {} (the limit below counts {})", m.name(), m.blurb(), m.goal_word())).size(13.0).color(Color32::from_gray(200)));
}

/// A menu panel. When it's taller than the room below `pos` (a short window, a handheld, or the
/// Couch / TV interface size) its contents scroll, and the selected button scrolls into view.
fn panel<R>(ctx: &egui::Context, id: &str, pos: Pos2, width: f32, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let room = (ctx.content_rect().bottom() - pos.y - 52.0).max(160.0);
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
                        egui::ScrollArea::vertical()
                            .id_salt(id)
                            .max_height(room)
                            .show(ui, |ui| {
                                ui.set_width(width);
                                f(ui)
                            })
                            .inner
                    })
                    .inner
            })
            .inner
}

impl App {
    fn big_button(ui: &mut egui::Ui, text: &str) -> bool {
        let clicked = ui.add_sized([ui.available_width(), 44.0], egui::Button::new(egui::RichText::new(text).size(19.0))).clicked();
        if clicked {
            let back = matches!(text, "Back" | "Cancel" | "Leave match" | "Back to menu" | "Quit");
            UI_SOUND.with(|u| u.set(Some(if back { Sfx::UiBack } else { Sfx::UiConfirm })));
        }
        clicked
    }

    fn menu_ui(&mut self, ctx: &egui::Context, rect: Rect, page: Page) -> bool {
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::unique("title")));
        // game icon beside the title
        if self.icon_tex.is_none() {
            let (px, n) = icon_pixels(128);
            let img = egui::ColorImage::from_rgba_unmultiplied([n as usize, n as usize], &px);
            self.icon_tex = Some(ctx.load_texture("game-icon", img, egui::TextureOptions::LINEAR));
        }
        let ipos = Pos2::new(rect.left() + 44.0, rect.top() + 34.0);
        let pulse = 0.5 + 0.5 * (self.wall_t * 1.6).sin();
        p.circle_filled(ipos + Vec2::splat(36.0), 40.0 + 3.0 * pulse, Color32::from_rgba_unmultiplied(255, 140, 60, (30.0 + 30.0 * pulse) as u8));
        if let Some(t) = &self.icon_tex {
            p.image(t.id(), Rect::from_min_size(ipos, Vec2::splat(72.0)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        let tpos = Pos2::new(rect.left() + 128.0, rect.top() + 34.0);
        p.text(tpos + Vec2::new(3.0, 3.0), Align2::LEFT_TOP, TITLE, FontId::proportional(64.0), Color32::from_black_alpha(200));
        p.text(tpos, Align2::LEFT_TOP, TITLE, FontId::proportional(64.0), ACCENT);
        // a slow shine sweeping under the title
        let line_y = tpos.y + 72.0;
        let shine = (self.wall_t * 0.35).fract();
        p.line_segment([Pos2::new(tpos.x, line_y), Pos2::new(tpos.x + 330.0, line_y)], Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 150, 70, 70)));
        let sx = tpos.x + 330.0 * shine;
        p.line_segment([Pos2::new((sx - 40.0).max(tpos.x), line_y), Pos2::new((sx + 40.0).min(tpos.x + 330.0), line_y)], Stroke::new(2.5, Color32::from_rgba_unmultiplied(255, 210, 160, 200)));
        p.text(Pos2::new(rect.left() + 48.0, rect.top() + 116.0), Align2::LEFT_TOP, "blades · bullets · wall-runs   —   arena duels for up to 8", FontId::proportional(17.0), Color32::from_rgb(235, 220, 210));
        let corner = if self.pad_active {
            let n = self.pads.name.as_deref();
            format!("{}: select  ·  {}: back  ·  LB / RB: tabs  ·  {}: player list in a match", pad::button_name(pad::btn::A, n), pad::button_name(pad::btn::B, n), pad::button_name(pad::btn::VIEW, n))
        } else {
            "F11 fullscreen  ·  Settings has controls, graphics and crosshair".to_string()
        };
        p.text(Pos2::new(rect.right() - 16.0, rect.bottom() - 12.0), Align2::RIGHT_BOTTOM, corner, FontId::proportional(13.0), Color32::from_rgba_unmultiplied(230, 220, 210, 160));
        let mut next: Option<Page> = None;
        let mut action: Option<Box<dyn FnOnce(&mut App)>> = None;
        let pos = Pos2::new(rect.left() + 48.0, rect.top() + 150.0);
        let msg = self.menu_msg.clone();
        let mut quit = false;
        // Esc (or B on a controller) steps back to the main page
        if page != Page::Main && self.rebinding.is_none() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            if page == Page::Settings {
                self.settings.save();
            }
            next = Some(Page::Main);
        }
        // recent servers, top right of the main page (when there's room)
        if page == Page::Main && rect.width() >= 980.0 && !self.settings.recent.is_empty() {
            let w = 330.0;
            let mut join = None;
            panel(ctx, "recent", Pos2::new(rect.right() - w - 88.0, rect.top() + 40.0), w, |ui| {
                ui.label(egui::RichText::new("Recent servers").size(20.0).color(ACCENT));
                join = self.recent_list_ui(ui);
            });
            if let Some(a) = join {
                action = Some(Box::new(move |app: &mut App| app.join_server(&a)));
            }
        }
        let width = if page == Page::Settings { 600.0 } else { 380.0 };
        panel(ctx, "menu", pos, width, |ui| {
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
                    if Self::big_button(ui, "Practice vs Bots") {
                        next = Some(Page::Practice);
                    }
                    if Self::big_button(ui, "Training") {
                        action = Some(Box::new(|app: &mut App| {
                            let name = clean_name(&app.settings.name, 0);
                            app.start_game(Game::training(&name), None);
                        }));
                    }
                    if Self::big_button(ui, "Host a Lobby") {
                        next = Some(Page::Host);
                    }
                    if Self::big_button(ui, "Join a Friend") {
                        next = Some(Page::Join);
                    }
                    if self.steam.is_some() && Self::big_button(ui, "Find Games (Steam)") {
                        self.browse_asked = false;
                        next = Some(Page::Browse);
                    }
                    if Self::big_button(ui, &format!("Stats  ·  level {}", self.career.level())) {
                        next = Some(Page::Stats);
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
                Page::Stats => {
                    let cr = self.career.clone();
                    ui.heading(format!("Level {}", cr.level()));
                    ui.add(egui::ProgressBar::new(cr.level_progress()).text(format!("{} XP  ·  {} to the next level", cr.xp, career::XP_PER_LEVEL - cr.xp % career::XP_PER_LEVEL)));
                    ui.add_space(6.0);
                    ui.colored_label(ACCENT, "Combat");
                    let kd = if cr.deaths == 0 { cr.kills as f32 } else { cr.kills as f32 / cr.deaths as f32 };
                    ui.label(format!("Kills {}  ·  Deaths {}  ·  K/D {:.2}  ·  Headshots {}  ·  Matches {}  ·  Gauntlet waves {}", cr.kills, cr.deaths, kd, cr.headshots, cr.matches, cr.waves));
                    ui.add_space(6.0);
                    ui.colored_label(ACCENT, "Finishers");
                    let share = if cr.kills > 0 { cr.finishers as f32 * 100.0 / cr.kills as f32 } else { 0.0 };
                    ui.label(format!("Executions {}  ({share:.0}% of your kills)", cr.finishers));
                    if let Some(k) = cr.favourite_finisher() {
                        ui.label(format!("Favourite: {}", FINISHERS[k].name));
                    }
                    egui::Grid::new("finisher-stats").num_columns(4).spacing([14.0, 3.0]).show(ui, |ui| {
                        for (k, st) in FINISHERS.iter().enumerate() {
                            ui.label(st.name);
                            ui.label(egui::RichText::new(st.weapon.map_or("aerial (any blade)", |w| w.name())).color(Color32::from_gray(180)));
                            ui.label(format!("{}", cr.fin_by[k]));
                            ui.label(if cr.unlocked(k) { egui::RichText::new("unlocked").color(Color32::from_rgb(140, 220, 160)) } else { egui::RichText::new(format!("level {}", st.unlock_level)).color(Color32::from_gray(150)) });
                            ui.end_row();
                        }
                    });
                    ui.label(egui::RichText::new("Finish a weakened (under 25% health) enemy who is staggered, guard-broken or launched: press E next to them.").size(12.5).color(Color32::from_gray(190)));
                    if Self::big_button(ui, "Back") {
                        next = Some(Page::Main);
                    }
                }
                Page::Practice => {
                    ui.heading("Practice vs Bots");
                    mode_picker(ui, &mut self.settings.mode);
                    map_picker(ui, &mut self.settings.map);
                    ui.add(egui::Slider::new(&mut self.settings.bots, 1..=7).text("bots"));
                    ui.label("Bot difficulty");
                    ui.horizontal(|ui| {
                        for (k, (n, _)) in BOT_LEVELS.iter().enumerate() {
                            ui.selectable_value(&mut self.settings.bot_level, k, *n);
                        }
                    });
                    ui.add(egui::Slider::new(&mut self.settings.rounds, 1..=50).text("score to win"));
                    if Self::big_button(ui, &format!("Gauntlet (solo waves)  ·  best {}", self.settings.gauntlet_best)) {
                        action = Some(Box::new(|app: &mut App| {
                            let name = clean_name(&app.settings.name, 0);
                            let mut g = Game::gauntlet(&name, app.settings.gauntlet_best);
                            g.set_map(MapId::from_u8(app.settings.map));
                            app.settings.save();
                            app.start_game(g, None);
                        }));
                    }
                    if Self::big_button(ui, "Fight!") {
                        action = Some(Box::new(|app: &mut App| {
                            let lvl = BOT_LEVELS[app.settings.bot_level].1;
                            let name = clean_name(&app.settings.name, 0);
                            let mode = Mode::from_u8(app.settings.mode);
                            let mut g = Game::offline(&name, app.settings.bots, lvl, mode, app.settings.rounds as u16);
                            g.set_map(MapId::from_u8(app.settings.map));
                            app.settings.save();
                            app.start_game(g, None);
                        }));
                    }
                    if Self::big_button(ui, "Back") {
                        next = Some(Page::Main);
                    }
                }
                Page::Host => {
                    ui.heading("Host a Lobby");
                    match &self.steam {
                        Some(st) => {
                            ui.checkbox(&mut self.settings.use_steam, format!("Host through Steam as {} (friends join from Steam: no port forwarding)", st.name));
                            if self.settings.use_steam {
                                ui.horizontal(|ui| {
                                    ui.radio_value(&mut self.settings.steam_public, true, "Listed in Find Games");
                                    ui.radio_value(&mut self.settings.steam_public, false, "Friends only");
                                });
                            }
                        }
                        None => {
                            ui.label(egui::RichText::new("Steam isn't running: friends join by IP address (start Steam for one-click joins without port forwarding).").size(13.0).color(Color32::from_gray(185)));
                        }
                    }
                    self.lookup_public_ip();
                    let port = self.settings.port.trim().to_string();
                    let (public, public_text) = self.public_addr_text();
                    let lan = self.lan_ip.as_ref().map(|ip| format!("{ip}:{port}"));
                    egui::Grid::new("host-addrs").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
                        ui.colored_label(ACCENT, "Internet");
                        ui.label(egui::RichText::new(&public_text).strong());
                        if let Some(a) = &public {
                            if ui.small_button("Copy").clicked() {
                                ui.ctx().copy_text(a.clone());
                            }
                        }
                        ui.end_row();
                        ui.colored_label(ACCENT, "Same Wi-Fi");
                        ui.label(egui::RichText::new(lan.clone().unwrap_or_else(|| "-".into())).strong());
                        if let Some(a) = &lan {
                            if ui.small_button("Copy").clicked() {
                                ui.ctx().copy_text(a.clone());
                            }
                        }
                        ui.end_row();
                    });
                    self.firewall_ui(ui);
                    ui.label(egui::RichText::new("Friends join your Internet address (UDP port forwarded to this PC). You can't join it yourself from home: test with a friend, or 127.0.0.1 from a second copy.").size(13.5).color(Color32::from_gray(200)));
                    mode_picker(ui, &mut self.settings.mode);
                    map_picker(ui, &mut self.settings.map);
                    ui.add(egui::Slider::new(&mut self.settings.rounds, 1..=50).text("score to win"));
                    ui.add(egui::Slider::new(&mut self.settings.fill_bots, 0..=MAX_PLAYERS).text("fill with bots up to this many fighters (0 = none)"))
                        .on_hover_text("Bots keep the lobby busy; each one leaves when a real player joins.");
                    ui.horizontal(|ui| {
                        ui.label("Port (UDP)");
                        ui.add(egui::TextEdit::singleline(&mut self.settings.port).desired_width(90.0));
                    });
                    if Self::big_button(ui, "Open Lobby") {
                        action = Some(Box::new(|app: &mut App| {
                            let port: u16 = app.settings.port.trim().parse().unwrap_or(net::DEFAULT_PORT);
                            match app.start_hosting(port) {
                                Ok(()) => {
                                    app.settings.save();
                                    app.menu_msg = None;
                                }
                                Err(e) => app.menu_msg = Some(e),
                            }
                        }));
                    }
                    if Self::big_button(ui, "Back") {
                        next = Some(Page::Main);
                    }
                }
                Page::Browse => {
                    ui.heading("Find Games");
                    if let Some(id) = self.browser_ui(ui) {
                        action = Some(Box::new(move |app: &mut App| {
                            if let Some(st) = app.steam.as_mut() {
                                st.join_lobby(id);
                                app.menu_msg = Some("Joining through Steam...".into());
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
                        action = Some(Box::new(|app: &mut App| {
                            let a = app.settings.join_addr.trim().to_string();
                            app.join_server(&a);
                        }));
                    }
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Recent servers").size(18.0).color(ACCENT));
                    egui::ScrollArea::vertical().max_height(230.0).show(ui, |ui| {
                        if let Some(a) = self.recent_list_ui(ui) {
                            action = Some(Box::new(move |app: &mut App| app.join_server(&a)));
                        }
                    });
                    if Self::big_button(ui, "Back") {
                        next = Some(Page::Main);
                    }
                }
                Page::Settings => {
                    ui.heading("Settings");
                    self.settings_ui(ui);
                    if Self::big_button(ui, "Back") {
                        self.settings.save();
                        self.rebinding = None;
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
                            ("Right click", "guard with the katana (raise it just before a slash lands for a perfect parry)"),
                            ("Space", "jump"),
                            ("Space at a wall", "while in the air: run along it, or up it if you face it"),
                            ("Space on a wall", "kick off the wall"),
                            ("Shift / Q", "dash (once more in the air)"),
                            ("C while running", "slide, keeping your speed; Space jumps out of it"),
                            ("Flow", "chain moves without stopping on the ground: each adds +4% speed (up to +20%)"),
                            ("Perfect timing", "jump or dash the instant you land for an extra boost"),
                            ("Double-tap W A S D", "dodge in that direction"),
                            ("Cancel", "after a slash lands: dash, jump or guard to cut it short"),
                            ("Butterfly", "slash, right-click guard, slash, guard... in rhythm"),
                            ("Launch", "3rd slash of a combo, or dash + slash: knocks them up"),
                            ("Air combo", "slash in the air to juggle; 3rd air slash smashes down"),
                            ("1  2  3 / wheel", "melee, gun 1, gun 2 (pick them on the loadout screen)"),
                            ("R", "reload"),
                            ("G", "use your item (medkit, armor kit or grenade; 2 per life)"),
                            ("Enter / T", "chat"),
                            ("Tab", "player list and scores"),
                            ("F5", "(host) start the match from the warm-up"),
                            ("Esc", "pause menu"),
                            ("Rocket jump", "rocket launcher: look at your feet, jump and fire together"),
                            ("N / P", "(Training) next / previous lesson"),
                            ("F11", "fullscreen / windowed"),
                        ] {
                            ui.colored_label(ACCENT, k);
                            ui.label(v);
                            ui.end_row();
                        }
                    });
                    ui.label("Every key can be changed in Settings > Controls (these are the defaults).");
                    egui::CollapsingHeader::new("With a controller").default_open(self.pad_active).show(ui, |ui| {
                        egui::Grid::new("howto-pad").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                            for (what, button) in pad::LAYOUT {
                                ui.colored_label(ACCENT, button);
                                ui.label(what);
                                ui.end_row();
                            }
                            ui.colored_label(ACCENT, "View + d-pad left / right");
                            ui.label("(Training) previous / next lesson");
                            ui.end_row();
                        });
                    });
                    ui.label("New here? Pick Training on the main menu: a room with dummies that walks you through every move step by step.");
                    ui.label("The third katana hit is heavy: it breaks guards and launches. Headshots with the revolver do extra damage. Free-for-All: everyone for themselves. Team Deathmatch: Red vs Blue. First to the kill limit wins; you respawn after 3 seconds.");
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
        let Screen::Lobby { waiting_since, .. } = &self.screen else { return false };
        let t = *waiting_since;
        let mut cancel = false;
        panel(ctx, "lobby", Pos2::new(rect.left() + 48.0, rect.top() + 150.0), 380.0, |ui| {
            ui.heading("Joining…");
            ui.label(format!("Knocking on the host's door ({:.0}s)", t));
            let dots = ".".repeat(((t * 2.0) as usize % 4) + 1);
            ui.label(dots);
            if App::big_button(ui, "Cancel") {
                cancel = true;
            }
        });
        cancel
    }
    fn play_overlay_ui(&mut self, ctx: &egui::Context, rect: Rect) -> bool {
        let (kind, phase, is_auth, ready, timer, humans, votes, voted) = match &self.screen {
            Screen::Play { game, .. } => {
                let humans: Vec<&Fighter> = game.f.iter().filter(|f| f.present && !f.is_bot).collect();
                let votes = humans.iter().filter(|f| f.vote).count();
                (game.kind, game.mt.phase, game.is_authority(), game.f[game.local].ready, game.mt.timer, humans.len(), votes, game.f[game.local].vote)
            }
            _ => return false,
        };
        let mut leave = false;
        let center = Pos2::new(rect.center().x - 200.0, rect.center().y - 60.0);

        // chat box
        if self.chat_open {
            let pos = Pos2::new(rect.left() + 14.0, rect.bottom() - 150.0);
            let mut send = false;
            let mut close = false;
            egui::Area::new(egui::Id::unique("chatbox")).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
                egui::Frame::new().fill(Color32::from_rgba_unmultiplied(10, 8, 20, 220)).corner_radius(6.0).inner_margin(8.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Say:").color(ACCENT));
                        let r = ui.add(egui::TextEdit::singleline(&mut self.chat_text).char_limit(100).desired_width(360.0).hint_text("Enter to send, Esc to cancel"));
                        r.request_focus();
                        if ui.input(|i| i.key_pressed(Key::Enter)) {
                            send = true;
                        }
                        if ui.input(|i| i.key_pressed(Key::Escape)) {
                            close = true;
                        }
                    });
                });
            });
            if send {
                let text = self.chat_text.trim().to_string();
                if !text.is_empty() {
                    self.chat_send = Some(text);
                }
                self.chat_open = false;
            }
            if close {
                self.chat_open = false;
            }
        }

        // loadout screen before each match
        if phase == Phase::Loadout && !ready && self.disconnected.is_none() {
            let pos = Pos2::new(rect.center().x - 320.0, rect.top() + 110.0);
            let mut go = false;
            let mut lo = self.settings.loadout;
            panel(ctx, "loadout", pos, 600.0, |ui| {
                ui.heading("Choose your loadout");
                ui.label(format!("The match starts when everyone is ready ({:.0}s left)", timer.max(0.0)));
                ui.add_space(6.0);
                loadout_picker(ui, &mut lo);
                ui.add_space(6.0);
                if App::big_button(ui, "Ready!") {
                    go = true;
                }
            });
            if lo != self.settings.loadout {
                self.settings.loadout = lo;
            }
            if go {
                self.ready_request = Some(self.settings.loadout);
                self.settings.save();
            }
        }
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
        let killcam_running = match &self.screen {
            Screen::Play { game, .. } => game.killcam.as_ref().map_or(false, |k| k.t < KILLCAM_TIME),
            _ => false,
        };
        if phase == Phase::MatchOver && killcam_running {
            return leave;
        }
        if phase == Phase::MatchOver {
            let mut rematch = false;
            let mut vote = false;
            let run = match &self.screen {
                Screen::Play { game, .. } => game.gauntlet.clone(),
                _ => None,
            };
            let has_steam = self.steam.is_some();
            panel(ctx, "over", Pos2::new(center.x, center.y + 60.0), 360.0, |ui| {
                if let Some(gt) = &run {
                    ui.heading(format!("Score {}", gt.score));
                    ui.label(format!("Wave {}  ·  {} kills", gt.wave, gt.kills));
                    if gt.new_best {
                        ui.colored_label(Color32::from_rgb(255, 200, 90), "New best!");
                    } else {
                        ui.label(format!("Your best: {}", gt.best));
                    }
                    if !has_steam {
                        ui.label(egui::RichText::new("Start Steam to put your score on the leaderboard.").size(12.5).color(Color32::from_gray(180)));
                    } else if gt.board.is_empty() && gt.rank == 0 {
                        ui.label(egui::RichText::new("Sending to the Steam leaderboard…").size(12.5).color(Color32::from_gray(180)));
                    } else {
                        if gt.rank > 0 {
                            ui.label(format!("Steam leaderboard: #{}", gt.rank));
                        }
                        for (name, score, rank) in &gt.board {
                            ui.label(egui::RichText::new(format!("#{rank}  {name}  {score}")).size(13.0));
                        }
                    }
                    ui.add_space(6.0);
                }
                if kind == Kind::Offline {
                    if App::big_button(ui, if run.is_some() { "Try again" } else { "Rematch" }) {
                        vote = true;
                    }
                } else {
                    ui.label(format!("Rematch votes: {votes} / {humans} (more than half starts it)"));
                    if !voted && App::big_button(ui, "Vote for a rematch") {
                        vote = true;
                    }
                    if voted {
                        ui.label("You voted. Waiting for the others…");
                    }
                    if is_auth && App::big_button(ui, "Start a new match now") {
                        rematch = true;
                    }
                }
                if App::big_button(ui, "Leave") {
                    leave = true;
                }
            });
            if rematch {
                self.start_request = true;
            }
            if vote {
                self.vote_request = true;
            }
            return leave;
        }
        if self.paused && self.pause_settings {
            let mut back = false;
            panel(ctx, "pause-settings", center, 600.0, |ui| {
                ui.heading("Settings");
                self.settings_ui(ui);
                if App::big_button(ui, "Back") {
                    back = true;
                }
            });
            if back {
                self.pause_settings = false;
                self.rebinding = None;
                self.settings.save();
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
                if let Some(st) = self.steam.as_ref().filter(|s| s.lobby.is_some()) {
                    if ui.button("Invite Steam friends").clicked() {
                        st.invite_friends();
                    }
                }
                if kind == Kind::Host {
                    let (public, public_text) = self.public_addr_text();
                    ui.horizontal(|ui| {
                        ui.label(format!("Internet: {public_text}"));
                        if let Some(a) = public {
                            if ui.small_button("Copy").clicked() {
                                ui.ctx().copy_text(a);
                            }
                        }
                    });
                    self.firewall_ui(ui);
                    if let Some(ip) = &self.lan_ip {
                        let a = format!("{ip}:{}", self.settings.port.trim());
                        ui.horizontal(|ui| {
                            ui.label(format!("Same Wi-Fi: {a}"));
                            if ui.small_button("Copy").clicked() {
                                ui.ctx().copy_text(a.clone());
                            }
                        });
                    }
                    if phase == Phase::Warmup && App::big_button(ui, "Start the match") {
                        self.start_request = true;
                        self.paused = false;
                    }
                }
                ui.add(egui::Slider::new(&mut self.settings.sens, 0.2..=3.0).text("mouse sensitivity"));
                ui.add(egui::Slider::new(&mut self.settings.volume, 0.0..=1.0).text("effects volume"));
                ui.add(egui::Slider::new(&mut self.settings.music_volume, 0.0..=1.0).text("music volume"));
                self.music_ui(ui);
                if ui.button("All settings (controls, graphics, crosshair)…").clicked() {
                    self.pause_settings = true;
                }
                ui.collapsing("Loadout", |ui| {
                    loadout_picker(ui, &mut self.settings.loadout);
                    let when = if matches!(phase, Phase::Fight | Phase::MatchOver) { "Use it from my next respawn" } else { "Use this loadout" };
                    if ui.button(when).clicked() {
                        self.ready_request = Some(self.settings.loadout);
                        self.settings.save();
                    }
                });
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

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn binds_save_and_swap() {
        for b in default_binds().iter().chain([Bind::Mouse(4), Bind::Alt, Bind::Ctrl, Bind::Unbound, Bind::Key(Key::F)].iter()) {
            assert_eq!(Bind::parse(&b.id()), Some(*b), "{:?} survives saving", b);
        }
        let mut binds = default_binds();
        // giving Jump the Shift key moves Dash onto Space, so nothing is lost
        assign_bind(&mut binds, A_JUMP, Bind::Shift);
        assert_eq!(binds[A_JUMP], Bind::Shift);
        assert_eq!(binds[A_DASH], Bind::Key(Key::Space));
    }

    #[test]
    fn icon_is_drawn() {
        let (px, n) = icon_pixels(32);
        assert_eq!(px.len(), (n * n * 4) as usize);
        let opaque = px.chunks(4).filter(|c| c[3] > 200).count();
        assert!(opaque > 500, "a round badge fills most of the icon");
        assert_eq!(px[3], 0, "corners are see-through");
    }
}

#[cfg(test)]
mod aim_tests {
    use super::*;
    use crate::world::{Block, Mat, World};

    /// A shooting range: a floor and a big wall 25 m ahead.
    fn range_world() -> World {
        let wall = |a: V3, b: V3| Block { b: Aabb::new(a, b), mat: Mat::Wall };
        World {
            blocks: vec![wall(v3(-80.0, -1.0, -80.0), v3(80.0, 0.0, 80.0)), wall(v3(-80.0, 0.0, 25.0), v3(80.0, 40.0, 27.0))],
            spawn_points: vec![(V3::ZERO, 0.0)],
            half: 80.0,
            deco: Vec::new(),
            braziers: Vec::new(),
            banners: Vec::new(),
            props: Vec::new(),
            vines: Vec::new(),
            torches: Vec::new(),
            map: crate::world::MapId::Arena,
            flags: [V3::ZERO, V3::ZERO],
        }
    }

    #[test]
    fn every_shot_lands_inside_the_crosshair() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 720.0));
        let mut rng = Rng(99);
        for fov in (60..=110).step_by(5) {
            for w in [Weapon::Shotgun, Weapon::Smg, Weapon::Revolver, Weapon::Rifle, Weapon::HandCannon, Weapon::Sniper] {
                // 0: rested (sniper scoped), 1: running right after a shot, 2: standing mid-spray
                for state in 0..3 {
                    let mut g = Game::new(Kind::Offline, 0, Mode::Ffa, 5);
                    g.world = range_world();
                    g.add_player(0, "T", true, None, 0);
                    let (pos, yaw, pitch) = (v3(0.0, 0.0, 0.0), 0.12, 0.06);
                    {
                        let f = &mut g.f[0];
                        f.pos = pos;
                        f.yaw = yaw;
                        f.pitch = pitch;
                        f.weapon = w;
                        let cap = w.gun().move_cap;
                        f.scope_t = if state == 0 { 1.0 } else { 0.0 };
                        (f.since_shot, f.move_spread, f.recoil) = match state {
                            0 => (5.0, 1.0, 0.0),
                            1 => (0.05, cap, 0.8),
                            _ => (0.075, 1.0, 0.4),
                        };
                    }
                    let spread = g.f[0].spread();
                    if state == 0 && w != Weapon::Shotgun {
                        assert_eq!(spread, 0.0, "a rested first shot is dead on");
                    }
                    let (pivot, dir, dist) = g.camera_rig(pos, yaw, pitch);
                    let cam = Cam::new(pivot - dir * dist, yaw, pitch, fov as f32, rect);
                    let radius = spread_px(spread, cam.focal);
                    let (o, d) = g.shot_ray(0);
                    let mut widest = 0.0f32;
                    for _ in 0..400 {
                        let pd = pellet_dir(d, yaw_right(yaw), spread, &mut rng);
                        let (t, _) = g.world.raycast(o, pd, 300.0).expect("hits the wall");
                        let sp = cam.project(o + pd * t).expect("in front of the camera");
                        let off = (sp - cam.center).length();
                        widest = widest.max(off);
                        assert!(off <= radius + 1.0, "{w:?} fov {fov} state {state}: pellet {off:.1}px out, crosshair {radius:.1}px");
                    }
                    if spread > 0.0 {
                        assert!(widest > radius * 0.7, "{w:?} fov {fov}: crosshair ({radius:.1}px) isn't much wider than the shots ({widest:.1}px)");
                    }
                }
            }
        }
    }

    #[test]
    fn moving_costs_little_accuracy() {
        for w in [Weapon::Smg, Weapon::Revolver, Weapon::Rifle, Weapon::Shotgun] {
            let mut f = Fighter::new("T", 0, true);
            f.weapon = w;
            f.since_shot = 0.1;
            f.recoil = 0.0;
            f.move_spread = 1.0;
            let still = f.spread();
            f.move_spread = w.gun().move_cap;
            let moving = f.spread();
            let cap = if w == Weapon::Rifle { 2.0 } else { 1.5 };
            assert!(moving <= still * cap + 1e-6, "{w:?}: moving {moving} vs still {still}");
        }
    }
}
