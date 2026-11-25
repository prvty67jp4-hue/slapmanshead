//! Replays: every match is recorded, small (deflated), into the game's folder (`replays/`), and
//! can be watched again from the Replays menu.
//!
//! A recording keeps, twenty times a second, everyone as the network describes them (the same
//! compact state a player receives: crate::net::encode_fstate), the shots in the air, the damage
//! numbers and the kill feed: enough to draw the match again from any angle. A match played on
//! this computer against bots also keeps how it was set up and every tick's input, so it can be
//! re-simulated exactly (`resimulate`): the game's own random numbers are seeded and it uses no
//! clock, so the same start and the same inputs give the same match, down to its last bit (the
//! file holds a hash of the end to check it).
//!
//! The viewer (`Viewer`) puts a recorded moment into an ordinary Game and lets the renderer draw
//! it: play and pause, 0.25x to 4x, scrubbing on the timeline, a free camera, the view behind any
//! player or through their eyes, and "clip the last 30 seconds" (a numbered PNG sequence at 30
//! frames a second, ready for a video editor).

use crate::game::*;
use crate::math::*;
use crate::net;
use crate::world::MapId;
use std::io::{Read, Write};
use std::path::PathBuf;

/// A recorded frame every this many seconds.
pub const RATE: f32 = 1.0 / 20.0;
const MAGIC: &[u8; 4] = b"DRRP";
const VERSION: u16 = 1;
/// A recording stops adding frames after an hour.
const MAX_FRAMES: u32 = 20 * 60 * 60;
/// Replays kept on this computer (the oldest go).
pub const KEEP: usize = 60;
/// The clip button: how far back, and the picture rate of the sequence.
pub const CLIP_SECS: f32 = 30.0;
pub const CLIP_FPS: f32 = 30.0;

// ---------------------------------------------------------------- bytes

struct Wr(Vec<u8>);
impl Wr {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i16(&mut self, v: i32) {
        self.0.extend_from_slice(&(v.clamp(-32000, 32000) as i16).to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn v(&mut self, v: V3) {
        self.f(v.x);
        self.f(v.y);
        self.f(v.z);
    }
    fn s(&mut self, s: &str) {
        let b = s.as_bytes();
        let n = b.len().min(60);
        self.u8(n as u8);
        self.0.extend_from_slice(&b[..n]);
    }
}

struct Rd<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.i..self.i + n)?;
        self.i += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|s| u16::from_le_bytes([s[0], s[1]]))
    }
    fn i16(&mut self) -> Option<i32> {
        self.take(2).map(|s| i16::from_le_bytes([s[0], s[1]]) as i32)
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn u64(&mut self) -> Option<u64> {
        let s = self.take(8)?;
        Some(u64::from_le_bytes(s.try_into().ok()?))
    }
    fn f(&mut self) -> Option<f32> {
        let v = self.take(4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]))?;
        Some(if v.is_finite() { v } else { 0.0 })
    }
    fn v(&mut self) -> Option<V3> {
        Some(v3(self.f()?, self.f()?, self.f()?))
    }
    fn s(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        Some(String::from_utf8_lossy(self.take(n)?).to_string())
    }
}

// ---------------------------------------------------------------- what a file says

/// One fighter in a recorded match.
#[derive(Clone, Debug, PartialEq)]
pub struct FighterInfo {
    pub slot: u8,
    pub name: String,
    pub color: u8,
    pub team: u8,
    pub bot: bool,
    pub wins: i32,
    pub kills: i32,
    pub deaths: i32,
    pub rank: u8,
}

/// A recording's summary (read without unpacking the rest): for the match history.
#[derive(Clone, Debug, PartialEq)]
pub struct Info {
    /// When it was played (seconds since 1970).
    pub date: u64,
    pub map: u8,
    pub mode: u8,
    pub target: u16,
    pub ranked: bool,
    /// The slot this computer played (or watched from).
    pub local: u8,
    pub duration: f32,
    pub fighters: Vec<FighterInfo>,
    pub frames: u32,
    /// It can be re-simulated exactly (a match played here against bots).
    pub resim: bool,
    pub won: bool,
}

impl Info {
    pub fn mode(&self) -> Mode {
        Mode::from_u8(self.mode)
    }
    pub fn map_name(&self) -> &'static str {
        MapId::from_u8(self.map).name()
    }
    /// The score: "A 3 - 1 B" for a duel (round wins), else the top two by kills.
    pub fn score_line(&self) -> String {
        let mut f = self.fighters.clone();
        let rounds = self.mode().rounds();
        f.sort_by_key(|x| std::cmp::Reverse(if rounds { x.wins } else { x.kills }));
        match f.as_slice() {
            [a, b, ..] => {
                let (x, y) = if rounds { (a.wins, b.wins) } else { (a.kills, b.kills) };
                format!("{} {x} - {y} {}", a.name, b.name)
            }
            [a] => format!("{} {}", a.name, if rounds { a.wins } else { a.kills }),
            [] => String::new(),
        }
    }
    pub fn date_text(&self) -> String {
        let days = self.date / 86_400;
        let secs = self.date % 86_400;
        let (y, m, d) = civil(days as i64);
        format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", secs / 3600, secs / 60 % 60)
    }
    fn write(&self, w: &mut Wr) {
        w.u64(self.date);
        w.u8(self.map);
        w.u8(self.mode);
        w.u16(self.target);
        w.u8(self.ranked as u8 | (self.resim as u8) << 1 | (self.won as u8) << 2);
        w.u8(self.local);
        w.f(self.duration);
        w.u32(self.frames);
        w.u8(self.fighters.len() as u8);
        for f in &self.fighters {
            w.u8(f.slot);
            w.s(&f.name);
            w.u8(f.color);
            w.u8(f.team);
            w.u8(f.bot as u8);
            w.i16(f.wins);
            w.i16(f.kills);
            w.i16(f.deaths);
            w.u8(f.rank);
        }
    }
    fn read(r: &mut Rd) -> Option<Info> {
        let date = r.u64()?;
        let map = r.u8()?.min(MapId::Varrow as u8);
        let mode = r.u8()?;
        let target = r.u16()?;
        let bits = r.u8()?;
        let local = r.u8()?;
        let duration = r.f()?;
        let frames = r.u32()?;
        let n = r.u8()? as usize;
        let mut fighters = Vec::new();
        for _ in 0..n.min(MAX_PLAYERS) {
            fighters.push(FighterInfo { slot: r.u8()?.min(MAX_PLAYERS as u8 - 1), name: r.s()?, color: r.u8()?, team: r.u8()?, bot: r.u8()? != 0, wins: r.i16()?, kills: r.i16()?, deaths: r.i16()?, rank: r.u8()? });
        }
        Some(Info { date, map, mode, target, ranked: bits & 1 != 0, local, duration, fighters, frames, resim: bits & 2 != 0, won: bits & 4 != 0 })
    }
}

/// Days since 1970 as a calendar date (Howard Hinnant's algorithm).
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// How a match against bots on this computer was set up: enough to start it again exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    pub name: String,
    pub bots: u8,
    pub skill: f32,
    pub mode: u8,
    pub target: u16,
    pub map: u8,
    pub seed: u64,
    pub loadout: [u8; 4],
    pub classic: bool,
    /// Executions the player hadn't unlocked (they decide what a finisher does).
    pub fin_locks: u32,
}

impl Setup {
    /// The match as it was set up (before its first tick).
    pub fn game(&self) -> Game {
        let mut g = Game::offline(&self.name, self.bots as usize, self.skill, Mode::from_u8(self.mode), self.target);
        g.set_map(MapId::from_u8(self.map));
        g.mt.classic = self.classic;
        let lo = Loadout::from_bytes(self.loadout);
        g.my_loadout = lo;
        g.f[0].loadout = lo;
        g.f[0].equip();
        g.fin_locks = self.fin_locks;
        g.rng = Rng(self.seed);
        g
    }
    fn write(&self, w: &mut Wr) {
        w.s(&self.name);
        w.u8(self.bots);
        w.f(self.skill);
        w.u8(self.mode);
        w.u16(self.target);
        w.u8(self.map);
        w.u64(self.seed);
        for b in self.loadout {
            w.u8(b);
        }
        w.u8(self.classic as u8);
        w.u32(self.fin_locks);
    }
    fn read(r: &mut Rd) -> Option<Setup> {
        Some(Setup { name: r.s()?, bots: r.u8()?.min(15), skill: r.f()?, mode: r.u8()?, target: r.u16()?, map: r.u8()?.min(MapId::Varrow as u8), seed: r.u64()?, loadout: [r.u8()?, r.u8()?, r.u8()?, r.u8()?], classic: r.u8()? != 0, fin_locks: r.u32()? })
    }
}

fn put_input(w: &mut Wr, i: &Input) {
    w.f(i.fwd);
    w.f(i.right);
    let bits = [i.jump, i.dash, i.fire, i.fire_pressed, i.guard, i.reload, i.item, i.crouch, i.kick, i.finish, i.ability, i.sneak, i.utility, i.inspect, i.spec];
    let mut b = 0u16;
    for (k, on) in bits.iter().enumerate() {
        if *on {
            b |= 1 << k;
        }
    }
    w.u16(b);
    w.u8(i.select.unwrap_or(255));
    w.u8(i.inv.unwrap_or(255));
    w.f(i.yaw);
    w.f(i.pitch);
}

fn get_input(r: &mut Rd) -> Option<Input> {
    let fwd = r.f()?;
    let right = r.f()?;
    let b = r.u16()?;
    let on = |k: u16| b & (1 << k) != 0;
    let select = r.u8()?;
    let inv = r.u8()?;
    Some(Input {
        fwd,
        right,
        jump: on(0),
        dash: on(1),
        fire: on(2),
        fire_pressed: on(3),
        guard: on(4),
        reload: on(5),
        item: on(6),
        crouch: on(7),
        kick: on(8),
        finish: on(9),
        ability: on(10),
        sneak: on(11),
        utility: on(12),
        inspect: on(13),
        spec: on(14),
        select: (select != 255).then_some(select),
        inv: (inv != 255).then_some(inv),
        yaw: r.f()?,
        pitch: r.f()?,
    })
}

/// A hash of everything that decides how a match goes from here (re-simulation must match it).
pub fn state_hash(g: &Game) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: &[u8]| {
        for x in b {
            h ^= *x as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    };
    eat(&g.rng.0.to_le_bytes());
    eat(&g.time.to_bits().to_le_bytes());
    eat(&[g.mt.phase as u8]);
    eat(&g.mt.round.to_le_bytes());
    for f in &g.f {
        if !f.present {
            continue;
        }
        for x in [f.pos.x, f.pos.y, f.pos.z, f.vel.x, f.vel.y, f.vel.z, f.yaw, f.pitch, f.hp, f.ap] {
            eat(&x.to_bits().to_le_bytes());
        }
        eat(&[f.alive as u8, f.weapon as u8]);
        eat(&f.kills.to_le_bytes());
        eat(&f.wins.to_le_bytes());
    }
    h
}

// ---------------------------------------------------------------- recording

/// Records the match being played (lives in Game::replay_rec).
#[derive(Clone, Debug)]
pub struct Recorder {
    pub setup: Option<Setup>,
    inputs: Vec<u8>,
    ticks: u32,
    /// What the player did between ticks (readying up with a loadout): (tick, loadout).
    commands: Vec<(u32, [u8; 4])>,
    /// A ranked match (marked in the history).
    pub ranked: bool,
    /// Seconds since the match ended (it's written a moment later, to keep the final blow).
    over_t: f32,
    frames: Vec<u8>,
    nframes: u32,
    acc: f32,
    first: Option<f32>,
    last_t: f32,
    pub date: u64,
    /// The finished file, once the match is over (main.rs saves it).
    pub finished: Option<Vec<u8>>,
}

impl Recorder {
    pub fn new(setup: Option<Setup>) -> Recorder {
        let date = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        Recorder { setup, inputs: Vec::new(), ticks: 0, commands: Vec::new(), ranked: false, over_t: 0.0, frames: Vec::new(), nframes: 0, acc: RATE, first: None, last_t: 0.0, date, finished: None }
    }

    /// Every tick, before it runs: the local input (kept only for a match that can be re-run).
    pub fn input(&mut self, i: &Input) {
        if self.setup.is_some() && self.finished.is_none() {
            let mut w = Wr(std::mem::take(&mut self.inputs));
            put_input(&mut w, i);
            self.inputs = w.0;
            self.ticks += 1;
        }
    }

    /// The player readied up with this loadout (between ticks).
    pub fn ready(&mut self, lo: Loadout) {
        if self.setup.is_some() && self.finished.is_none() {
            self.commands.push((self.ticks, lo.to_bytes()));
        }
    }

    /// Every tick, after it ran: a frame now and then while the match is on; the file once it
    /// has been over for two seconds.
    pub fn after_tick(&mut self, g: &Game, dt: f32) {
        if self.finished.is_none() && g.mt.phase == Phase::MatchOver && self.nframes > 0 {
            self.over_t += dt;
            if self.over_t >= 2.0 {
                self.finish(g);
                return;
            }
        }
        if self.finished.is_some() || !matches!(g.mt.phase, Phase::Countdown | Phase::Fight | Phase::MatchOver) || self.nframes >= MAX_FRAMES {
            return;
        }
        self.acc += dt;
        if self.acc < RATE {
            return;
        }
        self.acc = 0.0;
        self.first.get_or_insert(g.time);
        self.last_t = g.time;
        self.nframes += 1;
        let mut w = Wr(std::mem::take(&mut self.frames));
        w.f(g.time);
        w.u8(g.mt.phase as u8);
        w.u16(g.mt.round.min(65535) as u16);
        w.u16(g.mt.team_score[0]);
        w.u16(g.mt.team_score[1]);
        let present: Vec<usize> = (0..g.f.len().min(16)).filter(|&s| g.f[s].present).collect();
        let mask = present.iter().fold(0u16, |m, &s| m | 1 << s);
        w.u16(mask);
        for &s in &present {
            let f = &g.f[s];
            w.u8(f.alive as u8 | (f.spectate as u8) << 1);
            w.i16(f.kills);
            w.i16(f.deaths);
            w.i16(f.wins);
            w.f(f.death_t);
            net::encode_fstate(&mut w.0, f);
        }
        let tr: Vec<&Tracer> = g.tracers.iter().rev().take(32).collect();
        w.u8(tr.len() as u8);
        for t in tr {
            w.v(t.a);
            w.v(t.b);
            w.f(t.life);
            w.f(t.max_life);
            w.f(t.width);
            for c in t.color {
                w.u8(c);
            }
        }
        let nums: Vec<&DmgNum> = g.dmg_nums.iter().chain(g.taken_nums.iter()).rev().take(16).collect();
        w.u8(nums.len() as u8);
        for d in nums {
            w.v(d.pos);
            w.i16(d.amount);
            w.i16(d.armour);
            w.u8(d.head as u8 | (d.confirmed as u8) << 1);
            w.f(d.life);
        }
        // the kill feed's new lines
        let fresh: Vec<&FeedLine> = g.feed.iter().filter(|l| l.life > 6.0 - RATE - TICK * 1.5 && l.text.is_none()).collect();
        w.u8(fresh.len() as u8);
        for l in fresh {
            match &l.killer {
                Some((n, c)) => {
                    w.u8(1);
                    w.s(n);
                    w.u8(*c);
                }
                None => w.u8(0),
            }
            w.s(&l.victim.0);
            w.u8(l.victim.1);
            w.s(l.tool);
            w.u8(l.head as u8);
        }
        self.frames = w.0;
    }

    /// The match is over: the whole file.
    pub fn finish(&mut self, g: &Game) -> Vec<u8> {
        let ranked = self.ranked;
        let fighters = (0..g.f.len().min(16))
            .filter(|&s| g.f[s].present)
            .map(|s| {
                let f = &g.f[s];
                FighterInfo { slot: s as u8, name: f.name.clone(), color: f.color_idx, team: f.team, bot: f.is_bot, wins: f.wins, kills: f.kills, deaths: f.deaths, rank: f.rank }
            })
            .collect();
        let info = Info {
            date: self.date,
            map: g.world.map as u8,
            mode: g.mt.mode as u8,
            target: g.mt.target,
            ranked,
            local: g.local.min(255) as u8,
            duration: (self.last_t - self.first.unwrap_or(self.last_t)).max(0.0),
            fighters,
            frames: self.nframes,
            resim: self.setup.is_some(),
            won: g.local_won(),
        };
        let mut head = Wr(Vec::new());
        info.write(&mut head);
        let mut body = Wr(Vec::new());
        match &self.setup {
            Some(s) => {
                body.u8(1);
                s.write(&mut body);
                body.u64(state_hash(g));
                body.u32(self.ticks);
                body.0.extend_from_slice(&self.inputs);
                body.u16(self.commands.len().min(65535) as u16);
                for (t, lo) in &self.commands {
                    body.u32(*t);
                    for b in lo {
                        body.u8(*b);
                    }
                }
            }
            None => body.u8(0),
        }
        body.u32(self.nframes);
        body.0.extend_from_slice(&self.frames);
        let mut out = Vec::with_capacity(body.0.len() / 4 + 64);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(head.0.len() as u32).to_le_bytes());
        out.extend_from_slice(&head.0);
        let mut z = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        let _ = z.write_all(&body.0);
        out.extend_from_slice(&z.finish().unwrap_or_default());
        self.finished = Some(out.clone());
        out
    }
}

// ---------------------------------------------------------------- reading

#[derive(Clone, Debug)]
pub struct FrameFighter {
    pub slot: usize,
    pub alive: bool,
    pub spectate: bool,
    pub kills: i32,
    pub deaths: i32,
    pub wins: i32,
    pub death_t: f32,
    pub st: net::FState,
}

#[derive(Clone, Debug)]
pub struct FeedEv {
    pub killer: Option<(String, u8)>,
    pub victim: (String, u8),
    pub tool: &'static str,
    pub head: bool,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub t: f32,
    pub phase: Phase,
    pub round: u32,
    pub team_score: [u16; 2],
    pub fighters: Vec<FrameFighter>,
    pub tracers: Vec<Tracer>,
    pub nums: Vec<DmgNum>,
    pub feed: Vec<FeedEv>,
}

#[derive(Clone, Debug)]
pub struct Replay {
    pub info: Info,
    pub setup: Option<Setup>,
    pub end_hash: u64,
    pub inputs: Vec<Input>,
    pub commands: Vec<(u32, [u8; 4])>,
    pub frames: Vec<Frame>,
}

/// A tool's name back as the game's own (the kill feed keeps them as &'static str).
fn tool_named(s: &str) -> &'static str {
    (0..=40u8).map(kill_tool).find(|t| *t == s).unwrap_or("?")
}

/// Only the summary at the start of a file.
pub fn read_info(bytes: &[u8]) -> Option<Info> {
    if bytes.get(..4)? != MAGIC || u16::from_le_bytes([*bytes.get(4)?, *bytes.get(5)?]) != VERSION {
        return None;
    }
    let n = u32::from_le_bytes(bytes.get(6..10)?.try_into().ok()?) as usize;
    Info::read(&mut Rd { b: bytes.get(10..10 + n)?, i: 0 })
}

pub fn decode(bytes: &[u8]) -> Result<Replay, String> {
    let info = read_info(bytes).ok_or("not a replay of this version")?;
    let n = u32::from_le_bytes(bytes[6..10].try_into().map_err(|_| "short")?) as usize;
    let mut body = Vec::new();
    flate2::read::DeflateDecoder::new(&bytes[10 + n..]).read_to_end(&mut body).map_err(|e| e.to_string())?;
    let mut r = Rd { b: &body, i: 0 };
    let bad = || "the replay is damaged".to_string();
    let (mut setup, mut end_hash, mut inputs, mut commands) = (None, 0, Vec::new(), Vec::new());
    if r.u8().ok_or_else(bad)? == 1 {
        setup = Some(Setup::read(&mut r).ok_or_else(bad)?);
        end_hash = r.u64().ok_or_else(bad)?;
        let ticks = r.u32().ok_or_else(bad)?;
        for _ in 0..ticks {
            inputs.push(get_input(&mut r).ok_or_else(bad)?);
        }
        for _ in 0..r.u16().ok_or_else(bad)? {
            let t = r.u32().ok_or_else(bad)?;
            commands.push((t, [r.u8().ok_or_else(bad)?, r.u8().ok_or_else(bad)?, r.u8().ok_or_else(bad)?, r.u8().ok_or_else(bad)?]));
        }
    }
    let nf = r.u32().ok_or_else(bad)?;
    let mut frames = Vec::with_capacity(nf as usize);
    for _ in 0..nf {
        let f = read_frame(&mut r).ok_or_else(bad)?;
        frames.push(f);
    }
    Ok(Replay { info, setup, end_hash, inputs, commands, frames })
}

fn read_frame(r: &mut Rd) -> Option<Frame> {
    let t = r.f()?;
    let phase = Phase::from_u8(r.u8()?);
    let round = r.u16()? as u32;
    let team_score = [r.u16()?, r.u16()?];
    let mask = r.u16()?;
    let mut fighters = Vec::new();
    for s in 0..16usize {
        if mask & (1 << s) == 0 {
            continue;
        }
        let bits = r.u8()?;
        let (kills, deaths, wins) = (r.i16()?, r.i16()?, r.i16()?);
        let death_t = r.f()?;
        let mut at = r.i;
        let st = net::decode_fstate(r.b, &mut at)?;
        r.i = at;
        fighters.push(FrameFighter { slot: s, alive: bits & 1 != 0, spectate: bits & 2 != 0, kills, deaths, wins, death_t, st });
    }
    let mut tracers = Vec::new();
    for _ in 0..r.u8()? {
        let (a, b, life, max_life, width) = (r.v()?, r.v()?, r.f()?, r.f()?, r.f()?);
        let color = [r.u8()?, r.u8()?, r.u8()?];
        tracers.push(Tracer { a, b, life, max_life, width, color });
    }
    let mut nums = Vec::new();
    for _ in 0..r.u8()? {
        let (pos, amount, armour, bits, life) = (r.v()?, r.i16()?, r.i16()?, r.u8()?, r.f()?);
        nums.push(DmgNum { pos, amount, life, head: bits & 1 != 0, victim: 255, confirmed: bits & 2 != 0, armour, exact: amount as f32, exact_ap: 0.0, conf_t: 0.0 });
    }
    let mut feed = Vec::new();
    for _ in 0..r.u8()? {
        let killer = if r.u8()? == 1 { Some((r.s()?, r.u8()?)) } else { None };
        let victim = (r.s()?, r.u8()?);
        let tool = tool_named(&r.s()?);
        let head = r.u8()? != 0;
        feed.push(FeedEv { killer, victim, tool, head });
    }
    Some(Frame { t, phase, round, team_score, fighters, tracers, nums, feed })
}

/// Play a recorded match against bots again from its start with its inputs: the game it ends in
/// (its hash must equal the recording's `end_hash`).
pub fn resimulate(rep: &Replay) -> Option<Game> {
    let s = rep.setup.as_ref()?;
    let mut g = s.game();
    let mut next = 0;
    for (k, i) in rep.inputs.iter().enumerate() {
        while let Some((t, lo)) = rep.commands.get(next).filter(|c| c.0 as usize <= k) {
            let _ = t;
            g.set_ready(Loadout::from_bytes(*lo));
            next += 1;
        }
        g.tick(i);
    }
    Some(g)
}

/// Does the recording re-simulate exactly?
pub fn verify(rep: &Replay) -> Option<bool> {
    resimulate(rep).map(|g| state_hash(&g) == rep.end_hash)
}

// ---------------------------------------------------------------- files

pub fn folder() -> Option<PathBuf> {
    crate::platform::data_file("replays")
}

/// Save a finished recording (keeping the newest KEEP). Returns its path.
pub fn save(bytes: &[u8], info_mode: &str) -> Option<PathBuf> {
    let dir = folder()?;
    std::fs::create_dir_all(&dir).ok()?;
    let info = read_info(bytes)?;
    let name = format!("{}_{}_{}.drr", info.date, info_mode.to_lowercase().replace(' ', "-"), info.map);
    let p = dir.join(name);
    std::fs::write(&p, bytes).ok()?;
    let mut all = list();
    if all.len() > KEEP {
        all.sort_by_key(|(_, i)| i.date);
        for (old, _) in all.iter().take(all.len() - KEEP) {
            let _ = std::fs::remove_file(old);
        }
    }
    Some(p)
}

/// The replays on this computer, newest first: (file, summary).
pub fn list() -> Vec<(PathBuf, Info)> {
    let Some(dir) = folder() else { return Vec::new() };
    let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<(PathBuf, Info)> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "drr"))
        .filter_map(|p| {
            // (the summary is in the first few hundred bytes)
            let mut f = std::fs::File::open(&p).ok()?;
            let mut head = vec![0u8; 4096];
            let n = f.read(&mut head).ok()?;
            read_info(&head[..n]).map(|i| (p, i))
        })
        .collect();
    out.sort_by_key(|(_, i)| std::cmp::Reverse(i.date));
    out
}

// ---------------------------------------------------------------- watching

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewCam {
    /// Behind a player, as they played.
    Follow,
    /// Through a player's eyes.
    Eyes,
    /// Flying free (WASD and the mouse).
    Free,
}

/// A clip being written: a picture per step from `from` to `to`.
#[derive(Clone, Debug)]
pub struct Clip {
    pub dir: PathBuf,
    pub from: f32,
    pub to: f32,
    pub frame: u32,
    /// A picture was asked for and hasn't arrived yet.
    pub waiting: bool,
}

pub const SPEEDS: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];

pub struct Viewer {
    pub rep: Replay,
    pub path: Option<PathBuf>,
    pub game: Game,
    /// Seconds from the recording's start.
    pub t: f32,
    pub playing: bool,
    pub speed: f32,
    pub cam: ViewCam,
    pub who: usize,
    pub free_pos: V3,
    pub free_yaw: f32,
    pub free_pitch: f32,
    pub clip: Option<Clip>,
    pub note: Option<(String, f32)>,
    feed_upto: usize,
}

impl Viewer {
    pub fn new(rep: Replay, path: Option<PathBuf>) -> Viewer {
        let info = rep.info.clone();
        let mut game = Game::new(Kind::Demo, 0, info.mode(), info.target);
        game.set_map(MapId::from_u8(info.map));
        for fi in &info.fighters {
            let s = fi.slot as usize;
            if s < game.f.len() {
                game.add_player(s, &fi.name, false, None, fi.team);
                game.f[s].color_idx = fi.color;
                game.f[s].rank = fi.rank;
                game.f[s].is_bot = false;
            }
        }
        let who = (info.local as usize).min(MAX_PLAYERS - 1);
        let who = if info.fighters.iter().any(|f| f.slot as usize == who) { who } else { info.fighters.first().map_or(0, |f| f.slot as usize) };
        let mut v = Viewer { rep, path, game, t: 0.0, playing: true, speed: 1.0, cam: ViewCam::Follow, who, free_pos: V3::ZERO, free_yaw: 0.0, free_pitch: -0.3, clip: None, note: None, feed_upto: 0 };
        v.apply();
        let f = &v.game.f[v.who];
        v.free_pos = f.pos + v3(0.0, 4.0, 0.0) - yaw_fwd(f.yaw) * 5.0;
        v.free_yaw = f.yaw;
        v
    }

    pub fn duration(&self) -> f32 {
        match (self.rep.frames.first(), self.rep.frames.last()) {
            (Some(a), Some(b)) => (b.t - a.t).max(0.0),
            _ => 0.0,
        }
    }

    pub fn seek(&mut self, t: f32) {
        self.t = t.clamp(0.0, self.duration());
        self.feed_upto = 0;
        self.game.feed.clear();
        self.apply();
    }

    /// Advance with the real time passed (at the chosen speed), and show the moment.
    pub fn step(&mut self, dt: f32) {
        if let Some(c) = &self.clip {
            // a clip: one picture per step, whatever the speed
            if !c.waiting {
                self.t = (c.from + c.frame as f32 / CLIP_FPS).min(c.to);
            }
        } else if self.playing {
            self.t += dt * self.speed;
            if self.t >= self.duration() {
                self.t = self.duration();
                self.playing = false;
            }
        }
        if let Some((_, l)) = self.note.as_mut() {
            *l -= dt;
        }
        if self.note.as_ref().is_some_and(|n| n.1 <= 0.0) {
            self.note = None;
        }
        self.apply();
    }

    /// Start a clip of the last CLIP_SECS seconds up to now, written into `dir`.
    pub fn start_clip(&mut self, dir: PathBuf) {
        let to = self.t.max(1.0);
        let from = (to - CLIP_SECS).max(0.0);
        let _ = std::fs::create_dir_all(&dir);
        self.clip = Some(Clip { dir, from, to, frame: 0, waiting: false });
        self.playing = false;
    }

    /// The frames around `t` and how far between them.
    fn around(&self, t: f32) -> Option<(usize, usize, f32)> {
        let fr = &self.rep.frames;
        let first = fr.first()?.t;
        let at = first + t;
        let k = fr.partition_point(|f| f.t <= at);
        let b = k.min(fr.len() - 1);
        let a = k.saturating_sub(1).min(b);
        let w = if b > a { ((at - fr[a].t) / (fr[b].t - fr[a].t).max(1e-4)).clamp(0.0, 1.0) } else { 0.0 };
        Some((a, b, w))
    }

    /// Put the moment `t` into the game, for drawing.
    pub fn apply(&mut self) {
        let Some((a, b, w)) = self.around(self.t) else { return };
        let (fa, fb) = (&self.rep.frames[a], &self.rep.frames[b]);
        let g = &mut self.game;
        g.time = fa.t + (fb.t - fa.t) * w;
        g.mt.phase = fa.phase;
        g.mt.round = fa.round;
        g.mt.team_score = fa.team_score;
        for s in 0..g.f.len() {
            let Some(x) = fa.fighters.iter().find(|x| x.slot == s) else {
                if g.f[s].present && !self.rep.info.fighters.iter().any(|fi| fi.slot as usize == s) {
                    g.f[s].present = false;
                }
                continue;
            };
            let f = &mut g.f[s];
            f.present = true;
            net::apply_remote(f, &x.st, false, fa.t, fa.t);
            f.snaps.clear();
            let y = fb.fighters.iter().find(|y| y.slot == s);
            let (mut pos, mut yaw, mut pitch) = (x.st.pos, x.st.yaw, x.st.pitch);
            if let Some(y) = y.filter(|y| (y.st.pos - x.st.pos).len() < 3.0) {
                pos = pos.lerp(y.st.pos, w);
                yaw += wrap_angle(y.st.yaw - yaw) * w;
                pitch += (y.st.pitch - pitch) * w;
            }
            f.pos = pos;
            f.prev_pos = pos;
            f.net_pos = pos;
            f.yaw = yaw;
            f.pitch = pitch;
            f.alive = x.alive;
            f.spectate = x.spectate;
            f.death_t = x.death_t + (fb.t - fa.t) * w;
            f.kills = x.kills;
            f.deaths = x.deaths;
            f.wins = x.wins;
            f.hp = x.st.hp;
        }
        let near = if w < 0.5 { fa } else { fb };
        g.tracers = near.tracers.clone();
        g.dmg_nums = near.nums.clone();
        // the kill feed: lines from the last six seconds
        g.feed.clear();
        let now = g.time;
        for fr in self.rep.frames[..=a].iter().rev().take_while(|fr| now - fr.t < 6.0) {
            for l in &fr.feed {
                g.feed.insert(0, FeedLine { killer: l.killer.clone(), victim: l.victim.clone(), tool: l.tool, head: l.head, text: None, life: 6.0 - (now - fr.t) });
            }
        }
        g.feed.truncate(6);
        // first person hides the one you look through (as your own body is hidden)
        g.local = self.who.min(g.f.len() - 1);
        let _ = self.feed_upto;
    }

    /// The other players to switch between (in slot order), and the next / previous one.
    pub fn cycle(&mut self, step: i32) {
        let slots: Vec<usize> = self.rep.info.fighters.iter().map(|f| f.slot as usize).collect();
        if slots.is_empty() {
            return;
        }
        let k = slots.iter().position(|&s| s == self.who).unwrap_or(0) as i32;
        let n = slots.len() as i32;
        self.who = slots[((k + step).rem_euclid(n)) as usize];
        self.apply();
    }

    /// Where the camera is: (position, the point it looks at, field of view).
    pub fn camera(&self) -> (V3, V3, f32) {
        let f = &self.game.f[self.who.min(self.game.f.len() - 1)];
        match self.cam {
            ViewCam::Free => (self.free_pos, self.free_pos + look_dir(self.free_yaw, self.free_pitch), 74.0),
            ViewCam::Eyes => {
                let eye = f.pos + v3(0.0, 1.62, 0.0);
                (eye, eye + look_dir(f.yaw, f.pitch), 82.0)
            }
            ViewCam::Follow => {
                let (pivot, dir, dist) = self.game.camera_rig(f.pos, f.yaw, f.pitch * 0.6);
                let pos = pivot - dir * dist;
                (pos, pos + dir, 74.0)
            }
        }
    }

    /// Fly the free camera.
    pub fn fly(&mut self, fwd: f32, right: f32, up: f32, dyaw: f32, dpitch: f32, dt: f32) {
        self.free_yaw += dyaw;
        self.free_pitch = (self.free_pitch + dpitch).clamp(-1.5, 1.5);
        let d = look_dir(self.free_yaw, self.free_pitch);
        let r = yaw_right(self.free_yaw);
        self.free_pos += (d * fwd + r * right + V3::UP * up) * (9.0 * dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(k: u32) -> Input {
        // a player who runs about, turns, jumps, dashes and shoots now and then
        let t = k as f32 * TICK;
        Input {
            fwd: if (k / 90) % 3 == 0 { 1.0 } else { 0.4 },
            right: (t * 0.7).sin(),
            jump: k % 97 == 0,
            dash: k % 151 == 0,
            fire: (k / 40) % 2 == 0,
            fire_pressed: k % 40 == 0,
            yaw: t * 0.9,
            pitch: (t * 0.3).sin() * 0.2,
            ..Default::default()
        }
    }

    fn setup() -> Setup {
        Setup { name: "Rec".into(), bots: 1, skill: 0.6, mode: Mode::Duel as u8, target: 3, map: MapId::Arena as u8, seed: 0x1234_5678_9abc, loadout: Loadout::default().to_bytes(), classic: false, fin_locks: 0 }
    }

    /// Play a match against a bot with a script, recording it.
    fn record(secs: f32) -> (Vec<u8>, u64) {
        let s = setup();
        let mut g = s.game();
        g.replay_rec = Some(Box::new(Recorder::new(Some(s.clone()))));
        g.set_ready(Loadout::default());
        let ticks = (secs / TICK) as u32;
        for k in 0..ticks {
            g.tick(&script(k));
        }
        let hash = state_hash(&g);
        let mut rec = g.replay_rec.take().unwrap();
        (rec.finish(&g), hash)
    }

    /// A replay re-simulates exactly: the same start and inputs give the very same match.
    #[test]
    fn a_replay_resimulates_exactly() {
        let (bytes, hash) = record(25.0);
        let rep = decode(&bytes).expect("decodes");
        assert!(rep.info.resim && rep.setup == Some(setup()));
        assert_eq!(rep.end_hash, hash);
        assert!(rep.inputs.len() > 1400, "every tick's input: {}", rep.inputs.len());
        assert_eq!(verify(&rep), Some(true), "the re-simulated match ends in exactly the same state");
        // and it really is about the inputs: change one and it differs
        let mut bent = rep.clone();
        // (in the fight: running the other way for a couple of seconds)
        for i in bent.inputs.iter_mut().skip(700).take(120) {
            i.fwd = -1.0;
            i.right = 1.0;
        }
        assert_eq!(verify(&bent), Some(false), "other inputs, another match");
    }

    /// Small files, a frame twenty times a second, readable summaries, and the viewer shows any
    /// moment (scrubbing back and forth gives the same picture).
    #[test]
    fn recordings_are_small_and_watchable() {
        let (bytes, _) = record(30.0);
        let info = read_info(&bytes).expect("summary");
        assert_eq!(info.mode(), Mode::Duel);
        assert_eq!(info.fighters.len(), 2);
        assert!(info.frames > 300, "{} frames", info.frames);
        let per_min = bytes.len() as f32 / (info.duration.max(1.0) / 60.0);
        assert!(per_min < 400_000.0, "about {:.0} KB a minute", per_min / 1000.0);
        assert!(!info.score_line().is_empty() && info.date_text().len() == 16);
        let rep = decode(&bytes).unwrap();
        let mut v = Viewer::new(rep, None);
        assert!(v.duration() > 10.0);
        v.seek(8.0);
        let a: Vec<V3> = v.game.f.iter().filter(|f| f.present).map(|f| f.pos).collect();
        v.seek(2.0);
        v.seek(8.0);
        let b: Vec<V3> = v.game.f.iter().filter(|f| f.present).map(|f| f.pos).collect();
        assert_eq!(a, b, "scrubbing lands on the same moment");
        // playing at 4x moves four times as far through it
        v.seek(1.0);
        v.speed = 4.0;
        v.playing = true;
        v.step(0.5);
        assert!((v.t - 3.0).abs() < 1e-3);
        // the cameras: behind, through the eyes, free
        for cam in [ViewCam::Follow, ViewCam::Eyes, ViewCam::Free] {
            v.cam = cam;
            let (p, l, _) = v.camera();
            assert!(p.x.is_finite() && (l - p).len() > 0.1);
        }
        v.cycle(1);
        assert_ne!(v.who, info.local as usize);
        // the clip covers the last 30 s up to now, a picture each 1/30 s
        v.seek(20.0);
        v.start_clip(std::env::temp_dir().join("duel_clip_test"));
        let c = v.clip.as_ref().unwrap();
        assert!((c.to - c.from - 20.0).abs() < 1e-3 || (c.to - c.from - CLIP_SECS).abs() < 1e-3);
    }

    #[test]
    fn calendar_dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(19_723), (2024, 1, 1));
        assert_eq!(civil(20_516), (2026, 3, 4));
    }
}
