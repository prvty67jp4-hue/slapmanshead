//! Recorded CC0 sound effects (see assets/CREDITS.txt) played in 3D, plus streamed music.
//!
//! Every effect is decoded once at start-up into mono samples at the output device's rate. A
//! sound placed in the world gets quieter with distance and is panned left or right from where
//! the camera faces; sounds without a place (menu clicks, hit markers) play in the middle.
//! Music is decoded a little at a time on its own thread and crossfades between tracks.

use crate::game::Sfx;
use crate::math::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

macro_rules! clips {
    ($($f:literal),* $(,)?) => { &[$((include_bytes!(concat!("../assets/sfx/", $f)) as &[u8], $f)),*] };
}

type Clip = (&'static [u8], &'static str);

/// How each sound is played: its recordings, loudness and how far it carries (metres at which
/// it is half as loud).
struct Props {
    clips: &'static [Clip],
    gain: f32,
    range: f32,
    /// Played this much higher (or lower, below 1) than recorded.
    pitch: f32,
}

fn props(s: Sfx) -> Props {
    let p = |clips: &'static [Clip], gain: f32, range: f32| Props { clips, gain, range, pitch: 1.0 };
    let pp = |clips: &'static [Clip], gain: f32, range: f32, pitch: f32| Props { clips, gain, range, pitch };
    match s {
        Sfx::Shotgun => p(clips!["shotgun_1.wav", "shotgun_2.wav"], 0.95, 28.0),
        Sfx::Revolver => p(clips!["revolver_1.wav", "revolver_2.wav"], 0.85, 26.0),
        Sfx::Smg => p(clips!["smg_1.wav", "smg_2.wav"], 0.55, 22.0),
        Sfx::Rifle => p(clips!["rifle_1.wav", "rifle_2.wav"], 0.9, 34.0),
        Sfx::Rocket => p(clips!["rocket.ogg"], 0.8, 24.0),
        Sfx::Slash => p(clips!["swing_1.wav", "swing_2.wav", "swing_3.wav", "swing_4.wav"], 0.6, 9.0),
        Sfx::Clang => p(clips!["clang_1.ogg", "clang_2.ogg"], 0.75, 16.0),
        Sfx::Flesh => p(clips!["flesh_1.ogg", "flesh_2.ogg", "flesh_3.ogg"], 0.8, 12.0),
        Sfx::BulletFlesh => p(clips!["bullet_flesh_1.ogg", "bullet_flesh_2.ogg"], 0.55, 10.0),
        Sfx::ImpactStone => p(clips!["impact_stone_1.ogg", "impact_stone_2.ogg", "impact_stone_3.ogg"], 0.3, 8.0),
        Sfx::Boom => p(clips!["boom_1.ogg", "boom_2.ogg", "boom_crunch.ogg"], 1.0, 40.0),
        Sfx::Dash => p(clips!["dash_1.wav", "dash_2.wav"], 0.5, 8.0),
        Sfx::Jump => p(clips!["jump_1.ogg", "jump_2.ogg"], 0.35, 6.0),
        Sfx::Land => p(clips!["land_1.ogg", "land_2.ogg"], 0.45, 7.0),
        Sfx::StepStone => p(clips!["step_stone_1.ogg", "step_stone_2.ogg", "step_stone_3.ogg", "step_stone_4.ogg", "step_stone_5.ogg"], 0.32, 6.0),
        Sfx::StepWood => p(clips!["step_wood_1.ogg", "step_wood_2.ogg", "step_wood_3.ogg", "step_wood_4.ogg", "step_wood_5.ogg"], 0.36, 6.0),
        Sfx::WallRun => p(clips!["wall_1.ogg", "wall_2.ogg", "step_stone_2.ogg", "step_stone_4.ogg"], 0.3, 6.0),
        Sfx::Reload => p(clips!["reload.ogg"], 0.5, 6.0),
        Sfx::ReloadEnd => p(clips!["reload_end.ogg"], 0.55, 6.0),
        Sfx::SwitchBlade => p(clips!["switch_blade.ogg"], 0.45, 6.0),
        Sfx::SwitchGun => p(clips!["switch_gun.ogg"], 0.45, 6.0),
        Sfx::Throw => p(clips!["throw.wav"], 0.5, 7.0),
        Sfx::Heal => p(clips!["heal.ogg"], 0.6, 6.0),
        Sfx::Armor => p(clips!["armor.ogg"], 0.5, 6.0),
        Sfx::Hurt => p(clips!["hurt_1.ogg", "hurt_2.ogg"], 0.7, 8.0),
        Sfx::Tick => p(clips!["ui_tick.ogg"], 0.55, 0.0),
        Sfx::Hit => p(clips!["hitmark.ogg"], 0.45, 0.0),
        Sfx::Kill => p(clips!["kill.ogg"], 0.6, 0.0),
        Sfx::Headshot => p(clips!["headshot.ogg"], 0.45, 0.0),
        Sfx::UiClick => p(clips!["ui_click.ogg"], 0.5, 0.0),
        Sfx::UiConfirm => p(clips!["ui_confirm.ogg"], 0.5, 0.0),
        Sfx::UiBack => p(clips!["ui_back.ogg"], 0.5, 0.0),
        Sfx::MatchStart => p(clips!["match_start.ogg"], 0.55, 0.0),
        Sfx::MatchOver => p(clips!["match_over.ogg"], 0.55, 0.0),
        // hit feedback for the shooter: a deep thump for big hits, a metal crack when armour breaks
        Sfx::HitHeavy => p(clips!["land_1.ogg", "land_2.ogg"], 0.75, 0.0),
        Sfx::ArmorBreak => p(clips!["clang_3.ogg"], 0.6, 0.0),
        // a perfect parry: a bright ring, heard well across the arena
        Sfx::ParryPerfect => p(clips!["headshot.ogg"], 0.8, 24.0),
        // movement: a soft whoosh for perfect timing, cloth and grit for a slide
        Sfx::Perfect => p(clips!["dash_2.wav", "swing_2.wav"], 0.35, 6.0),
        Sfx::Slide => p(clips!["wall_1.ogg", "wall_2.ogg"], 0.5, 7.0),
        // the hand cannon: the magnum, deeper and heavier; the sniper: the rifle, deep and loud,
        // heard across the whole arena
        Sfx::HandCannon => pp(clips!["revolver_1.wav", "revolver_2.wav"], 1.0, 34.0, 0.8),
        Sfx::Sniper => pp(clips!["rifle_1.wav", "rifle_2.wav", "shotgun_1.wav"], 1.0, 70.0, 0.72),
        Sfx::ScopeIn => pp(clips!["switch_gun.ogg"], 0.4, 0.0, 1.4),
        // melee depth: a dull boot thump, a huge low clang, a bright ricochet ring, the blade
        // humming ready, and the ground cracking under a greatsword
        Sfx::Kick => pp(clips!["land_1.ogg", "land_2.ogg"], 0.85, 10.0, 1.35),
        Sfx::GuardBreak => pp(clips!["clang_3.ogg", "clang_1.ogg"], 1.0, 26.0, 0.62),
        Sfx::Deflect => pp(clips!["clang_2.ogg", "clang_1.ogg"], 0.9, 22.0, 1.55),
        Sfx::ChargeReady => pp(clips!["switch_blade.ogg"], 0.65, 8.0, 1.6),
        Sfx::Slam => pp(clips!["boom_crunch.ogg", "boom_2.ogg"], 0.95, 28.0, 0.8),
        // a throwing star: a short, high, thin whip through the air
        Sfx::Shuriken => pp(clips!["swing_1.wav", "swing_3.wav"], 0.5, 10.0, 1.75),
        // the pump racking (a heavy mechanical clack) and a shell thumbed in
        Sfx::Pump => pp(clips!["switch_gun.ogg", "reload_end.ogg"], 0.8, 10.0, 0.78),
        Sfx::ShellIn => pp(clips!["reload_end.ogg"], 0.5, 6.0, 1.45),
        // abilities, loud enough that enemies can react: a bright ping, a whipping line, a heavy
        // metal shield, a hollow whoosh
        Sfx::MarkPing => pp(clips!["headshot.ogg"], 0.85, 30.0, 0.62),
        Sfx::GrappleFire => pp(clips!["throw.wav", "swing_1.wav"], 0.8, 18.0, 0.68),
        Sfx::ShieldUp => pp(clips!["armor.ogg", "clang_1.ogg"], 0.9, 22.0, 0.7),
        Sfx::Vanish => pp(clips!["dash_2.wav"], 0.85, 16.0, 0.55),
    }
}

/// Music tracks: menu, then the in-match tracks.
pub const TRACK_MENU: usize = 0;
pub const TRACK_FIGHTS: [usize; 2] = [1, 2];
const TRACKS: [Clip; 3] = [
    (include_bytes!("../assets/music/menu.ogg"), "ogg"),
    (include_bytes!("../assets/music/fight.ogg"), "ogg"),
    (include_bytes!("../assets/music/battle.mp3"), "mp3"),
];

fn ext(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or("")
}

/// An open compressed file being read packet by packet.
struct Reader {
    format: Box<dyn FormatReader>,
    dec: Box<dyn Decoder>,
    track: u32,
    rate: u32,
}

impl Reader {
    fn open(bytes: &'static [u8], extension: &str) -> Option<Reader> {
        let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
        let mut hint = Hint::new();
        hint.with_extension(extension);
        let probed = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default()).ok()?;
        let format = probed.format;
        let t = format.default_track()?;
        let dec = symphonia::default::get_codecs().make(&t.codec_params, &DecoderOptions::default()).ok()?;
        let rate = t.codec_params.sample_rate?;
        let track = t.id;
        Some(Reader { format, dec, track, rate })
    }

    /// The next block of audio as stereo frames, or None at the end.
    fn next(&mut self) -> Option<Vec<[f32; 2]>> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(_) => return None,
            };
            if packet.track_id() != self.track {
                continue;
            }
            match self.dec.decode(&packet) {
                Ok(buf) => {
                    let spec = *buf.spec();
                    let ch = spec.channels.count().max(1);
                    let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
                    sb.copy_interleaved_ref(buf);
                    let s = sb.samples();
                    return Some(
                        s.chunks(ch)
                            .map(|f| if ch == 1 { [f[0], f[0]] } else { [f[0], f[1]] })
                            .collect(),
                    );
                }
                Err(SymError::DecodeError(_)) => continue,
                Err(_) => return None,
            }
        }
    }
}

/// Decode a whole short clip to mono at `out_rate`.
fn decode_clip(bytes: &'static [u8], extension: &str, out_rate: f32) -> Vec<f32> {
    let Some(mut r) = Reader::open(bytes, extension) else { return Vec::new() };
    let mut mono = Vec::new();
    while let Some(block) = r.next() {
        mono.extend(block.iter().map(|f| (f[0] + f[1]) * 0.5));
    }
    resample(&mono, r.rate as f32, out_rate)
}

// ---------------------------------------------------------------------------------- gunshots

/// How a gun's shot is built: the recording (its harsh top end softened) layered with a designed
/// crack (the supersonic snap), body (the bark of the muzzle blast), boom (a low thump that drops
/// in pitch: the punch you feel) and an outdoor echo tail.
#[derive(Clone, Copy, Debug)]
pub struct GunVoice {
    /// The recording: how loud, and where its highs are rolled off (Hz).
    rec: f32,
    rec_lp: f32,
    /// Crack: loudness and length (ms).
    crack: f32,
    crack_ms: f32,
    /// Body: loudness, band (Hz) and length (ms).
    body: f32,
    body_band: (f32, f32),
    body_ms: f32,
    /// Boom: loudness, start and end pitch (Hz) and length (ms).
    boom: f32,
    boom_hz: (f32, f32),
    boom_ms: f32,
    /// Echo tail: loudness and how long it rings (s).
    tail: f32,
    tail_s: f32,
}

pub fn gun_voice(s: Sfx) -> Option<GunVoice> {
    let v = |rec, rec_lp, crack, crack_ms, body, body_band, body_ms, boom, boom_hz, boom_ms, tail, tail_s| GunVoice { rec, rec_lp, crack, crack_ms, body, body_band, body_ms, boom, boom_hz, boom_ms, tail, tail_s };
    Some(match s {
        Sfx::Revolver => v(0.5, 5000.0, 0.8, 6.0, 0.7, (700.0, 2600.0), 90.0, 0.85, (150.0, 52.0), 170.0, 0.22, 0.9),
        Sfx::HandCannon => v(0.45, 4000.0, 0.75, 8.0, 0.75, (500.0, 2000.0), 120.0, 1.0, (120.0, 40.0), 230.0, 0.28, 1.2),
        Sfx::Rifle => v(0.5, 6000.0, 0.9, 4.0, 0.65, (900.0, 3200.0), 70.0, 0.65, (170.0, 62.0), 125.0, 0.2, 0.8),
        Sfx::Smg => v(0.55, 6000.0, 0.65, 3.0, 0.6, (1000.0, 3500.0), 45.0, 0.45, (200.0, 80.0), 70.0, 0.12, 0.45),
        // the scattergun: a big, low, rolling boom
        Sfx::Shotgun => v(0.45, 3300.0, 0.65, 8.0, 0.9, (300.0, 1500.0), 170.0, 1.25, (95.0, 32.0), 320.0, 0.32, 1.3),
        Sfx::Sniper => v(0.45, 5000.0, 1.0, 5.0, 0.7, (700.0, 2600.0), 110.0, 0.9, (140.0, 44.0), 210.0, 0.32, 1.8),
        _ => return None,
    })
}

/// A two-pole band-pass (state-variable filter) over a buffer.
fn band_pass(x: &[f32], lo: f32, hi: f32, rate: f32) -> Vec<f32> {
    let fc = (lo * hi).sqrt();
    let q = (fc / (hi - lo).max(1.0)).clamp(0.3, 4.0);
    let f = 2.0 * (std::f32::consts::PI * fc / rate).sin();
    let damp = 1.0 / q;
    let (mut low, mut band) = (0.0f32, 0.0f32);
    x.iter()
        .map(|&s| {
            let high = s - low - damp * band;
            band += f * high;
            low += f * band;
            band
        })
        .collect()
}

/// A one-pole low-pass.
fn low_pass(x: &mut [f32], hz: f32, rate: f32) {
    let a = 1.0 - (-2.0 * std::f32::consts::PI * hz / rate).exp();
    let mut y = 0.0f32;
    for s in x.iter_mut() {
        y += a * (*s - y);
        *s = y;
    }
}

/// A small outdoor reverb (four damped combs into two all-passes): the shot's echo off walls.
fn echo(x: &[f32], len_s: f32, rate: f32) -> Vec<f32> {
    let n = x.len() + (len_s * rate) as usize;
    let mut out = vec![0.0f32; n];
    let combs = [0.0297f32, 0.0371, 0.0411, 0.0437];
    // feedback so each comb falls by 60 dB over len_s
    for (k, d) in combs.iter().enumerate() {
        let delay = ((d * (1.0 + 0.35 * k as f32 * 0.0) * rate) as usize).max(1);
        let g = 10f32.powf(-3.0 * d / len_s.max(0.05));
        let mut buf = vec![0.0f32; delay];
        let mut damp = 0.0f32;
        for i in 0..n {
            let inp = x.get(i).copied().unwrap_or(0.0);
            let j = i % delay;
            let y = buf[j];
            damp += 0.45 * (y - damp); // darker with every bounce
            buf[j] = inp + damp * g;
            out[i] += y * 0.25;
        }
    }
    for (d, g) in [(0.005f32, 0.7f32), (0.0017, 0.7)] {
        let delay = ((d * rate) as usize).max(1);
        let mut buf = vec![0.0f32; delay];
        for i in 0..n {
            let j = i % delay;
            let b = buf[j];
            let y = -g * out[i] + b;
            buf[j] = out[i] + g * y;
            out[i] = y;
        }
    }
    out
}

/// Build a gunshot from its recording and the gun's voice. `seed` makes the variants differ.
pub fn design_gunshot(rec: &[f32], v: &GunVoice, rate: f32, seed: u32) -> Vec<f32> {
    let mut rng = seed.wrapping_mul(747_796_405).wrapping_add(2_891_336_453) | 1;
    let mut noise = move || {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    };
    // each variant a touch different: pitch and lengths
    let vary = 1.0 + (seed as f32 * 0.37).sin() * 0.06;
    let dry_len = (0.45 * rate) as usize;
    let n = rec.len().max(dry_len);
    let mut dry = vec![0.0f32; n];
    // the recording, softened (no fizz) and with any offset removed
    let mut r = rec.to_vec();
    low_pass(&mut r, v.rec_lp, rate);
    let mean = r.iter().take((0.01 * rate) as usize).sum::<f32>() / ((0.01 * rate) as usize).max(1) as f32;
    let peak = r.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-4);
    for (i, s) in r.iter().enumerate() {
        dry[i] += (s - mean) / peak * v.rec;
    }
    // crack: a bright snap, gone in a few milliseconds
    let crack_n = (v.crack_ms * 0.001 * rate * 4.0) as usize;
    let mut crack: Vec<f32> = (0..crack_n.min(n)).map(|_| noise()).collect();
    let hp: Vec<f32> = {
        let mut lp = crack.clone();
        low_pass(&mut lp, 1500.0, rate);
        crack.iter().zip(lp.iter()).map(|(a, b)| a - b).collect()
    };
    let tau = v.crack_ms * 0.001 * rate / 3.0;
    for (i, s) in hp.iter().enumerate() {
        dry[i] += s * (-(i as f32) / tau).exp() * v.crack;
    }
    crack.clear();
    // body: band-passed noise, a fast rise then decay
    let body_n = ((v.body_ms * vary * 0.001 * rate) as usize * 4).min(n);
    let raw: Vec<f32> = (0..body_n).map(|_| noise()).collect();
    let band = band_pass(&raw, v.body_band.0 * vary, v.body_band.1 * vary, rate);
    let bpeak = band.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-4);
    let tb = v.body_ms * vary * 0.001 * rate / 2.5;
    for (i, s) in band.iter().enumerate() {
        let attack = (i as f32 / (0.0015 * rate)).min(1.0);
        dry[i] += s / bpeak * attack * (-(i as f32) / tb).exp() * v.body;
    }
    // boom: a sine dropping in pitch, softly saturated
    let boom_n = ((v.boom_ms * 0.001 * rate) as usize * 3).min(n);
    let mut phase = 0.0f32;
    let (f0, f1) = (v.boom_hz.0 / vary, v.boom_hz.1 / vary);
    let tl = v.boom_ms * 0.001 * rate;
    for i in 0..boom_n {
        let x = i as f32 / tl;
        let f = f1 + (f0 - f1) * (-x * 3.0).exp();
        phase += 2.0 * std::f32::consts::PI * f / rate;
        let attack = (i as f32 / (0.002 * rate)).min(1.0);
        let s = (phase.sin() * 2.2).tanh() / 2.2f32.tanh();
        dry[i] += s * attack * (-x * 2.2).exp() * v.boom;
    }
    // the echo off the walls, from the first part of the shot
    let early: Vec<f32> = dry.iter().take((0.12 * rate) as usize).copied().collect();
    let wet = echo(&early, v.tail_s * vary, rate);
    let mut out = vec![0.0f32; dry.len().max(wet.len())];
    for (i, s) in dry.iter().enumerate() {
        out[i] += s;
    }
    let wpeak = wet.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-4);
    for (i, s) in wet.iter().enumerate() {
        out[i] += s / wpeak * v.tail;
    }
    // glued together: soft clipping, then levelled
    for s in out.iter_mut() {
        *s = (*s * 1.5).tanh();
    }
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-4);
    // a short fade at the very end
    let fade = (0.03 * rate) as usize;
    let len = out.len();
    for i in 0..fade.min(len) {
        out[len - 1 - i] *= i as f32 / fade as f32;
    }
    out.iter().map(|s| s / peak * 0.92).collect()
}

/// Final touches on a decoded effect: guns get their designed shot.
fn finish_clip(s: Sfx, k: usize, clip: Vec<f32>, rate: f32) -> Vec<f32> {
    match gun_voice(s) {
        Some(v) if !clip.is_empty() => design_gunshot(&clip, &v, rate, s as u32 * 16 + k as u32 + 1),
        _ => clip,
    }
}

fn resample(s: &[f32], from: f32, to: f32) -> Vec<f32> {
    if s.is_empty() || (from - to).abs() < 1.0 {
        return s.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let n = (s.len() as f64 / ratio) as usize;
    (0..n)
        .map(|i| {
            let x = i as f64 * ratio;
            let j = x as usize;
            let f = (x - j as f64) as f32;
            let a = s[j.min(s.len() - 1)];
            let b = s[(j + 1).min(s.len() - 1)];
            a + (b - a) * f
        })
        .collect()
}

struct Voice {
    data: Arc<Vec<f32>>,
    pos: f64,
    step: f64,
    gl: f32,
    gr: f32,
}

struct Mixer {
    voices: Vec<Voice>,
    volume: f32,
    /// Music: interleaved stereo at the device rate, filled by the music thread.
    music: VecDeque<f32>,
    music_volume: f32,
    /// Fades toward `music_target` (0 while switching tracks).
    music_gain: f32,
    music_target: f32,
}

pub struct Audio {
    _stream: cpal::Stream,
    mixer: Arc<Mutex<Mixer>>,
    bank: Vec<Vec<Arc<Vec<f32>>>>,
    /// Which track the music thread should play.
    want_track: Arc<Mutex<Option<usize>>>,
    listener: (V3, V3, V3),
    rng: u32,
}

/// Every sound, in the order of `Sfx`.
pub const ALL: [Sfx; 54] = [
    Sfx::Shotgun,
    Sfx::Revolver,
    Sfx::Smg,
    Sfx::Rifle,
    Sfx::Rocket,
    Sfx::Slash,
    Sfx::Clang,
    Sfx::Flesh,
    Sfx::BulletFlesh,
    Sfx::ImpactStone,
    Sfx::Boom,
    Sfx::Dash,
    Sfx::Jump,
    Sfx::Land,
    Sfx::StepStone,
    Sfx::StepWood,
    Sfx::WallRun,
    Sfx::Reload,
    Sfx::ReloadEnd,
    Sfx::SwitchBlade,
    Sfx::SwitchGun,
    Sfx::Throw,
    Sfx::Heal,
    Sfx::Armor,
    Sfx::Hurt,
    Sfx::Tick,
    Sfx::Hit,
    Sfx::Kill,
    Sfx::Headshot,
    Sfx::UiClick,
    Sfx::UiConfirm,
    Sfx::UiBack,
    Sfx::MatchStart,
    Sfx::MatchOver,
    Sfx::HitHeavy,
    Sfx::ArmorBreak,
    Sfx::ParryPerfect,
    Sfx::Perfect,
    Sfx::Slide,
    Sfx::HandCannon,
    Sfx::Sniper,
    Sfx::ScopeIn,
    Sfx::Kick,
    Sfx::GuardBreak,
    Sfx::Deflect,
    Sfx::ChargeReady,
    Sfx::Slam,
    Sfx::Shuriken,
    Sfx::Pump,
    Sfx::ShellIn,
    Sfx::MarkPing,
    Sfx::GrappleFire,
    Sfx::ShieldUp,
    Sfx::Vanish,
];

fn mix_into(m: &Arc<Mutex<Mixer>>, frames: usize, channels: usize, rate: f32, mut write: impl FnMut(usize, f32)) {
    let mut guard = match m.lock() {
        Ok(g) => g,
        Err(_) => {
            for i in 0..frames * channels {
                write(i, 0.0);
            }
            return;
        }
    };
    let mx = &mut *guard;
    let fade = 1.0 / (0.6 * rate); // about 0.6 s to fade the music in or out
    for f in 0..frames {
        let (mut l, mut r) = (0.0f32, 0.0f32);
        for v in mx.voices.iter_mut() {
            let i = v.pos as usize;
            if i + 1 < v.data.len() {
                let t = (v.pos - i as f64) as f32;
                let s = v.data[i] + (v.data[i + 1] - v.data[i]) * t;
                l += s * v.gl;
                r += s * v.gr;
            }
            v.pos += v.step;
        }
        l *= mx.volume;
        r *= mx.volume;
        // music
        if mx.music_gain < mx.music_target {
            mx.music_gain = (mx.music_gain + fade).min(mx.music_target);
        } else if mx.music_gain > mx.music_target {
            mx.music_gain = (mx.music_gain - fade).max(mx.music_target);
        }
        if mx.music.len() >= 2 {
            let ml = mx.music.pop_front().unwrap_or(0.0);
            let mr = mx.music.pop_front().unwrap_or(0.0);
            let g = mx.music_gain * mx.music_volume * 0.6;
            l += ml * g;
            r += mr * g;
        }
        let (l, r) = (l.tanh(), r.tanh());
        if channels == 1 {
            write(f, (l + r) * 0.5);
        } else {
            write(f * channels, l);
            write(f * channels + 1, r);
            for c in 2..channels {
                write(f * channels + c, 0.0);
            }
        }
    }
    mx.voices.retain(|v| (v.pos as usize) + 1 < v.data.len());
}

/// Keeps the music buffer topped up; switches tracks with a fade.
fn music_thread(mixer: Arc<Mutex<Mixer>>, want: Arc<Mutex<Option<usize>>>, rate: f32) {
    let mut current: Option<usize> = None;
    let mut reader: Option<Reader> = None;
    let mut src: VecDeque<[f32; 2]> = VecDeque::new();
    let mut pos = 0.0f64;
    loop {
        let w = want.lock().map(|g| *g).unwrap_or(None);
        if w != current {
            // fade out what's playing, then start the new track
            if let Ok(mut m) = mixer.lock() {
                m.music_target = 0.0;
            }
            for _ in 0..80 {
                let quiet = mixer.lock().map(|m| m.music_gain < 0.02 || m.music.is_empty()).unwrap_or(true);
                if quiet {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            if let Ok(mut m) = mixer.lock() {
                m.music.clear();
                m.music_target = 1.0;
            }
            current = w;
            reader = w.and_then(|t| Reader::open(TRACKS[t].0, TRACKS[t].1));
            src.clear();
            pos = 0.0;
        }
        if let Some(rd) = reader.as_mut() {
            let step = rd.rate as f64 / rate as f64;
            let mut out = Vec::new();
            let have = mixer.lock().map(|m| m.music.len()).unwrap_or(0);
            let need = (rate * 2.0 * 0.35) as usize; // keep about a third of a second ready
            if have < need {
                while out.len() < need - have {
                    // make sure there are source frames around the read position
                    while src.len() < pos as usize + 2 {
                        match rd.next() {
                            Some(b) => src.extend(b),
                            None => {
                                // loop the track from the start
                                if let Some(t) = current {
                                    if let Some(n) = Reader::open(TRACKS[t].0, TRACKS[t].1) {
                                        *rd = n;
                                    }
                                }
                                match rd.next() {
                                    Some(b) => src.extend(b),
                                    None => break,
                                }
                            }
                        }
                    }
                    if src.len() < pos as usize + 2 {
                        break;
                    }
                    let i = pos as usize;
                    let t = (pos - i as f64) as f32;
                    let (a, b) = (src[i], src[i + 1]);
                    out.push(a[0] + (b[0] - a[0]) * t);
                    out.push(a[1] + (b[1] - a[1]) * t);
                    pos += step;
                    let drop = pos as usize;
                    if drop > 4096 {
                        src.drain(..drop);
                        pos -= drop as f64;
                    }
                }
                if let Ok(mut m) = mixer.lock() {
                    m.music.extend(out);
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(12));
    }
}

impl Audio {
    pub fn new() -> Option<Audio> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let supported = device.default_output_config().ok()?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.config();
        let sr = config.sample_rate as f32;
        let channels = config.channels as usize;
        let mixer = Arc::new(Mutex::new(Mixer { voices: Vec::new(), volume: 0.7, music: VecDeque::new(), music_volume: 0.5, music_gain: 0.0, music_target: 1.0 }));
        let m2 = mixer.clone();
        let stream = match format {
            cpal::SampleFormat::F32 => device
                .build_output_stream(
                    config,
                    move |data: &mut [f32], _| {
                        let frames = data.len() / channels;
                        mix_into(&m2, frames, channels, sr, |i, s| data[i] = s);
                    },
                    |_| {},
                    None,
                )
                .ok()?,
            cpal::SampleFormat::I16 => device
                .build_output_stream(
                    config,
                    move |data: &mut [i16], _| {
                        let frames = data.len() / channels;
                        mix_into(&m2, frames, channels, sr, |i, s| data[i] = (s * 32767.0) as i16);
                    },
                    |_| {},
                    None,
                )
                .ok()?,
            _ => return None,
        };
        stream.play().ok()?;
        let bank = ALL.iter().map(|&s| props(s).clips.iter().enumerate().map(|(k, (b, name))| Arc::new(finish_clip(s, k, decode_clip(b, ext(name), sr), sr))).collect()).collect();
        let want_track = Arc::new(Mutex::new(None));
        let (m3, w3) = (mixer.clone(), want_track.clone());
        std::thread::Builder::new().name("music".into()).spawn(move || music_thread(m3, w3, sr)).ok()?;
        Some(Audio { _stream: stream, mixer, bank, want_track, listener: (V3::ZERO, v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0)), rng: 0x1234_5678 })
    }

    pub fn set_volume(&mut self, effects: f32, music: f32) {
        if let Ok(mut m) = self.mixer.lock() {
            m.volume = effects;
            m.music_volume = music;
        }
    }

    /// Where the ears are: the camera's position, its right and its forward direction.
    pub fn set_listener(&mut self, pos: V3, right: V3, fwd: V3) {
        self.listener = (pos, right, fwd);
    }

    /// Which music to play (None for silence).
    pub fn set_music(&mut self, track: Option<usize>) {
        if let Ok(mut w) = self.want_track.lock() {
            *w = track;
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Play a sound: in the world at `at` (quieter with distance, panned to its side), or in
    /// the middle when `at` is None. `pitch` 1 is as recorded; world sounds vary a little on top.
    pub fn play(&mut self, s: Sfx, at: Option<V3>, pitch: f32) {
        let pr = props(s);
        let idx = s as usize;
        let n = self.bank.get(idx).map_or(0, |v| v.len());
        if n == 0 {
            return;
        }
        let pick = ((self.rand() * n as f32) as usize).min(n - 1);
        let data = self.bank[idx][pick].clone();
        let (gl, gr) = match at {
            Some(p) if pr.range > 0.0 => match spatial(self.listener, p, pr.range) {
                Some(g) => g,
                None => return, // too far to hear
            },
            _ => (1.0, 1.0),
        };
        let pitch = pr.pitch * pitch * if at.is_some() { 1.0 + (self.rand() - 0.5) * 0.08 } else { 1.0 };
        if let Ok(mut m) = self.mixer.lock() {
            if m.voices.len() >= 40 {
                // drop the oldest so new sounds always play
                m.voices.remove(0);
            }
            m.voices.push(Voice { data, pos: 0.0, step: pitch as f64, gl: gl * pr.gain, gr: gr * pr.gain });
        }
    }
}

/// Left and right loudness for a sound at `p`: fades with distance (half as loud at `range`
/// metres) and pans by how far to the side it is. None when it's too faint to bother.
pub fn spatial(listener: (V3, V3, V3), p: V3, range: f32) -> Option<(f32, f32)> {
    let (lp, right, fwd) = listener;
    let d = p - lp;
    let dist = d.len();
    let x = (dist / range.max(0.1)).max(0.0);
    let mut att = 1.0 / (1.0 + x * x);
    if att < 0.015 {
        return None;
    }
    let dir = if dist > 1e-3 { d * (1.0 / dist) } else { fwd };
    if dir.dot(fwd) < -0.2 {
        att *= 0.82; // a little duller behind you
    }
    // close sounds (your own) stay near the middle
    let side = dir.dot(right).clamp(-1.0, 1.0) * (dist / 2.0).min(1.0) * 0.85;
    let a = (side + 1.0) * std::f32::consts::FRAC_PI_4;
    let k = std::f32::consts::SQRT_2;
    Some((a.cos() * k * att, a.sin() * k * att))
}

#[cfg(test)]
mod gunshot_tests {
    use super::*;

    #[test]
    fn designed_gunshots_punch_ring_out_and_differ() {
        let rate = 48_000.0;
        // a stand-in recording: a short noisy burst
        let mut x = 12345u32;
        let rec: Vec<f32> = (0..(0.3 * rate) as usize)
            .map(|i| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                ((x >> 8) as f32 / (1u32 << 23) as f32 - 1.0) * (-(i as f32) / 2000.0).exp()
            })
            .collect();
        for s in [Sfx::Revolver, Sfx::HandCannon, Sfx::Rifle, Sfx::Smg, Sfx::Shotgun, Sfx::Sniper] {
            let v = gun_voice(s).expect("a gun");
            let a = design_gunshot(&rec, &v, rate, 1);
            let b = design_gunshot(&rec, &v, rate, 2);
            assert!(a.iter().all(|s| s.is_finite() && s.abs() <= 1.0), "{s:?}: clean levels");
            assert!(a.len() as f32 / rate >= v.tail_s * 0.9, "{s:?}: the echo rings out");
            // the boom: plenty of low end in the first quarter second
            let mut lo = a[..(0.25 * rate) as usize].to_vec();
            low_pass(&mut lo, 150.0, rate);
            let low_energy: f32 = lo.iter().map(|s| s * s).sum::<f32>();
            assert!(low_energy > 5.0, "{s:?}: it has punch ({low_energy})");
            // the two variants aren't copies
            let diff: f32 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum();
            assert!(diff > 10.0, "{s:?}: variants differ");
            // loud at the start, quiet at the end
            let head = a[..(0.05 * rate) as usize].iter().fold(0.0f32, |m, s| m.max(s.abs()));
            let tail = a[a.len() - (0.05 * rate) as usize..].iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(head > 0.5 && tail < 0.05, "{s:?}: a shot, then silence ({head}, {tail})");
        }
        assert!(gun_voice(Sfx::Slash).is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_decodes() {
        for s in ALL {
            for (b, name) in props(s).clips {
                let pcm = decode_clip(b, ext(name), 48000.0);
                assert!(pcm.len() > 300, "{name} decodes to sound ({} samples)", pcm.len());
                assert!(pcm.iter().any(|x| x.abs() > 0.05), "{name} isn't silent");
            }
        }
        // every Sfx is in ALL, in order, and the game has room to track each one
        for (i, s) in ALL.iter().enumerate() {
            assert_eq!(*s as usize, i);
        }
        assert!(ALL.len() <= crate::game::SFX_SLOTS, "raise SFX_SLOTS");
    }

    #[test]
    fn music_opens() {
        for (b, e) in TRACKS {
            let mut r = Reader::open(b, e).expect("track opens");
            assert!(r.next().is_some());
        }
    }

    #[test]
    fn sounds_are_3d() {
        let ears = (V3::ZERO, v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0));
        let (l, r) = spatial(ears, v3(10.0, 0.0, 0.0), 20.0).unwrap();
        assert!(r > l * 3.0, "a sound to the right is louder in the right ear");
        let (l2, r2) = spatial(ears, v3(-10.0, 0.0, 0.0), 20.0).unwrap();
        assert!(l2 > r2 * 3.0, "and to the left in the left ear");
        let near = spatial(ears, v3(0.0, 0.0, 3.0), 20.0).unwrap();
        let far = spatial(ears, v3(0.0, 0.0, 30.0), 20.0).unwrap();
        assert!(near.0 > far.0 * 2.0, "closer is louder");
        assert!((near.0 - near.1).abs() < 0.01, "straight ahead is centred");
        assert!(spatial(ears, v3(0.0, 0.0, 400.0), 6.0).is_none(), "far away footsteps aren't heard");
    }
}
