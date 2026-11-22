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
}

fn props(s: Sfx) -> Props {
    let p = |clips: &'static [Clip], gain: f32, range: f32| Props { clips, gain, range };
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
pub const ALL: [Sfx; 39] = [
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
        let bank = ALL.iter().map(|&s| props(s).clips.iter().map(|(b, name)| Arc::new(decode_clip(b, ext(name), sr))).collect()).collect();
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
        let pitch = pitch * if at.is_some() { 1.0 + (self.rand() - 0.5) * 0.08 } else { 1.0 };
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
        // every Sfx is in ALL, in order
        for (i, s) in ALL.iter().enumerate() {
            assert_eq!(*s as usize, i);
        }
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
