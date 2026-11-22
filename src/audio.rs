//! Procedurally made sound effects played through the default output device.

use crate::game::Sfx;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

struct Voice {
    data: Arc<Vec<f32>>,
    pos: usize,
    gain: f32,
}

struct Mixer {
    voices: Vec<Voice>,
    volume: f32,
}

pub struct Audio {
    _stream: cpal::Stream,
    mixer: Arc<Mutex<Mixer>>,
    bank: Vec<(Sfx, Arc<Vec<f32>>)>,
}

struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn synth(s: Sfx, sr: f32) -> Vec<f32> {
    let mut n = Noise(0x1234567 ^ (s as u32 + 1).wrapping_mul(2654435761));
    let len = |secs: f32| (secs * sr) as usize;
    let tau = std::f32::consts::TAU;
    let mut out;
    match s {
        Sfx::Revolver | Sfx::Shotgun => {
            let heavy = s == Sfx::Shotgun;
            let dur = if heavy { 0.5 } else { 0.32 };
            out = vec![0.0; len(dur)];
            let mut lp = 0.0;
            for (i, o) in out.iter_mut().enumerate() {
                let t = i as f32 / sr;
                let env = (-t * if heavy { 9.0 } else { 15.0 }).exp();
                let a = if heavy { 0.25 } else { 0.4 };
                lp += a * (n.next() - lp);
                let crack = if t < 0.012 { n.next() * (1.0 - t / 0.012) } else { 0.0 };
                let thump = (tau * (if heavy { 70.0 } else { 110.0 }) * t * (1.0 - t)).sin() * (-t * 18.0).exp();
                *o = (lp * env * 1.6 + crack * 0.8 + thump * 0.9) * 0.6;
            }
        }
        Sfx::Slash | Sfx::Dash => {
            let dur = if s == Sfx::Slash { 0.24 } else { 0.18 };
            out = vec![0.0; len(dur)];
            let mut lp = 0.0;
            let mut hp_prev = 0.0;
            for (i, o) in out.iter_mut().enumerate() {
                let t = i as f32 / sr;
                let p = t / dur;
                let env = (p * std::f32::consts::PI).sin().powf(1.5);
                let cut = 0.05 + 0.25 * (p * std::f32::consts::PI).sin();
                lp += cut * (n.next() - lp);
                let hp = lp - hp_prev;
                hp_prev = lp;
                *o = (lp * 0.7 + hp * 0.6) * env * if s == Sfx::Slash { 0.9 } else { 0.5 };
            }
        }
        Sfx::Clang => {
            out = vec![0.0; len(0.6)];
            for (i, o) in out.iter_mut().enumerate() {
                let t = i as f32 / sr;
                let env = (-t * 7.0).exp();
                let v = (tau * 1250.0 * t).sin() * 0.5 + (tau * 1873.0 * t).sin() * 0.35 + (tau * 2710.0 * t).sin() * 0.25 + (tau * 3960.0 * t).sin() * 0.15;
                let hit = if t < 0.01 { n.next() * 0.6 } else { 0.0 };
                *o = (v * env + hit) * 0.35;
            }
        }
        Sfx::Hit => {
            out = vec![0.0; len(0.09)];
            for (i, o) in out.iter_mut().enumerate() {
                let t = i as f32 / sr;
                let env = (-t * 45.0).exp();
                *o = ((tau * 1500.0 * t).sin() * 0.5 + n.next() * 0.2) * env * 0.5;
            }
        }
        Sfx::Boom => {
            out = vec![0.0; len(1.3)];
            let mut lp = 0.0;
            for (i, o) in out.iter_mut().enumerate() {
                let t = i as f32 / sr;
                let env = (-t * 3.2).exp();
                lp += 0.06 * (n.next() - lp);
                let low = (tau * 48.0 * t).sin() * (-t * 5.0).exp();
                let crack = if t < 0.03 { n.next() * (1.0 - t / 0.03) } else { 0.0 };
                *o = (lp * 3.5 * env + low * 0.9 + crack * 0.7) * 0.6;
            }
        }
        Sfx::Jump => {
            out = vec![0.0; len(0.1)];
            let mut lp = 0.0;
            for (i, o) in out.iter_mut().enumerate() {
                let t = i as f32 / sr;
                lp += 0.1 * (n.next() - lp);
                *o = lp * (-t * 35.0).exp() * 0.6;
            }
        }
        Sfx::Reload => {
            out = vec![0.0; len(0.35)];
            for (i, o) in out.iter_mut().enumerate() {
                let t = i as f32 / sr;
                let c1 = if t < 0.03 { (-t * 120.0).exp() } else { 0.0 };
                let t2 = t - 0.22;
                let c2 = if t2 > 0.0 && t2 < 0.03 { (-t2 * 120.0).exp() } else { 0.0 };
                *o = (n.next() * 0.5 + (tau * 2200.0 * t).sin() * 0.5) * (c1 + c2) * 0.4;
            }
        }
        Sfx::Hurt => {
            out = vec![0.0; len(0.2)];
            let mut lp = 0.0;
            for (i, o) in out.iter_mut().enumerate() {
                let t = i as f32 / sr;
                lp += 0.08 * (n.next() - lp);
                let thud = (tau * 90.0 * t).sin() * (-t * 20.0).exp();
                *o = (lp * 2.0 * (-t * 18.0).exp() + thud) * 0.5;
            }
        }
        Sfx::Tick => {
            out = vec![0.0; len(0.25)];
            for (i, o) in out.iter_mut().enumerate() {
                let t = i as f32 / sr;
                *o = ((tau * 880.0 * t).sin() + (tau * 1320.0 * t).sin() * 0.5) * (-t * 10.0).exp() * 0.3;
            }
        }
    }
    out
}

const ALL: [Sfx; 11] = [Sfx::Revolver, Sfx::Shotgun, Sfx::Slash, Sfx::Clang, Sfx::Hit, Sfx::Boom, Sfx::Dash, Sfx::Jump, Sfx::Reload, Sfx::Hurt, Sfx::Tick];

fn mix_into(m: &Arc<Mutex<Mixer>>, frames: usize, channels: usize, mut write: impl FnMut(usize, f32)) {
    let mut guard = match m.lock() {
        Ok(g) => g,
        Err(_) => {
            for i in 0..frames * channels {
                write(i, 0.0);
            }
            return;
        }
    };
    let vol = guard.volume;
    for f in 0..frames {
        let mut s = 0.0;
        for v in guard.voices.iter_mut() {
            if v.pos < v.data.len() {
                s += v.data[v.pos] * v.gain;
                v.pos += 1;
            }
        }
        let s = (s * vol).tanh();
        for c in 0..channels {
            write(f * channels + c, s);
        }
    }
    guard.voices.retain(|v| v.pos < v.data.len());
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
        let mixer = Arc::new(Mutex::new(Mixer { voices: Vec::new(), volume: 0.7 }));
        let m2 = mixer.clone();
        let stream = match format {
            cpal::SampleFormat::F32 => device
                .build_output_stream(
                    config,
                    move |data: &mut [f32], _| {
                        let frames = data.len() / channels;
                        mix_into(&m2, frames, channels, |i, s| data[i] = s);
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
                        mix_into(&m2, frames, channels, |i, s| data[i] = (s * 32767.0) as i16);
                    },
                    |_| {},
                    None,
                )
                .ok()?,
            _ => return None,
        };
        stream.play().ok()?;
        let bank = ALL.iter().map(|&s| (s, Arc::new(synth(s, sr)))).collect();
        Some(Audio { _stream: stream, mixer, bank })
    }

    pub fn set_volume(&mut self, v: f32) {
        if let Ok(mut m) = self.mixer.lock() {
            m.volume = v;
        }
    }

    pub fn play(&mut self, s: Sfx) {
        let Some((_, data)) = self.bank.iter().find(|(k, _)| *k == s) else { return };
        if let Ok(mut m) = self.mixer.lock() {
            if m.voices.len() < 24 {
                m.voices.push(Voice { data: data.clone(), pos: 0, gain: 1.0 });
            }
        }
    }
}
