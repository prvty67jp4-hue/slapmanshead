//! UDP lobby for up to 8 players. The host is the hub: every player sends its own fighter and
//! events to the host, and the host sends everyone the whole match (players, scores, fighters and
//! everybody else's events). Each player simulates its own fighter; hits are applied by the victim;
//! the host keeps the scores.

use crate::game::*;
use crate::math::*;
use std::collections::VecDeque;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Instant;

const MAGIC: [u8; 4] = *b"DRA3";
const K_HELLO: u8 = 1;
const K_STATE: u8 = 2;
const K_HOST: u8 = 3;
const K_BYE: u8 = 4;
const K_FULL: u8 = 5;
pub const DEFAULT_PORT: u16 = 27960;
const EVENT_WINDOW: f32 = 0.5;
const MAX_EVENTS: usize = 48;

struct W(Vec<u8>);
impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i16(&mut self, v: i16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
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
        let n = b.len().min(32);
        self.u8(n as u8);
        self.0.extend_from_slice(&b[..n]);
    }
    fn head(kind: u8) -> W {
        let mut w = W(Vec::with_capacity(1200));
        w.0.extend_from_slice(&MAGIC);
        w.u8(kind);
        w
    }
}

struct R<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.i + n > self.b.len() {
            return None;
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|s| u16::from_le_bytes([s[0], s[1]]))
    }
    fn i16(&mut self) -> Option<i16> {
        self.take(2).map(|s| i16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn f(&mut self) -> Option<f32> {
        let v = self.take(4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]))?;
        if v.is_finite() { Some(v) } else { Some(0.0) }
    }
    fn v(&mut self) -> Option<V3> {
        Some(v3(self.f()?, self.f()?, self.f()?))
    }
    fn s(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        let b = self.take(n)?;
        Some(String::from_utf8_lossy(b).to_string())
    }
}

/// One fighter's body as sent over the network.
pub struct FState {
    pub pos: V3,
    pub vel: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub hp: f32,
    pub ap: f32,
    pub weapon: u8,
    pub flags: u8,
    pub flags2: u8,
    pub slash_t: f32,
    pub slash_idx: u8,
    pub hurt_t: f32,
    pub recoil: f32,
    pub wall_n: V3,
}

fn write_fstate(w: &mut W, f: &Fighter) {
    w.v(f.pos);
    w.v(f.vel);
    w.f(f.yaw);
    w.f(f.pitch);
    w.f(f.hp);
    w.f(f.ap);
    w.u8(f.weapon as u8);
    w.u8(f.flags());
    w.u8(if f.spawn_prot > 0.0 { 1 } else { 0 });
    w.f(f.slash_t);
    w.u8(f.slash_idx);
    w.f(f.hurt_t);
    w.f(f.recoil);
    w.v(f.wall.map_or(V3::ZERO, |wr| wr.normal));
}

fn read_fstate(r: &mut R) -> Option<FState> {
    Some(FState {
        pos: r.v()?,
        vel: r.v()?,
        yaw: r.f()?,
        pitch: r.f()?,
        hp: r.f()?,
        ap: r.f()?,
        weapon: r.u8()?,
        flags: r.u8()?,
        flags2: r.u8()?,
        slash_t: r.f()?,
        slash_idx: r.u8()?,
        hurt_t: r.f()?,
        recoil: r.f()?,
        wall_n: r.v()?,
    })
}

fn write_ev(w: &mut W, ev: &Ev) {
    match ev {
        Ev::Shot { o, e, w: wp, spark } => {
            w.u8(1);
            w.v(*o);
            w.v(*e);
            w.u8(*wp);
            w.u8(*spark);
        }
        Ev::Hit { victim, dmg, knock, stun, head, w: wp } => {
            w.u8(2);
            w.u8(*victim);
            w.f(*dmg);
            w.v(*knock);
            w.f(*stun);
            w.u8(*head as u8);
            w.u8(*wp);
        }
        Ev::Grenade { p, v } => {
            w.u8(3);
            w.v(*p);
            w.v(*v);
        }
        Ev::Died { killer, w: wp } => {
            w.u8(4);
            w.u8(*killer);
            w.u8(*wp);
        }
    }
}

fn read_ev(r: &mut R) -> Option<Ev> {
    Some(match r.u8()? {
        1 => Ev::Shot { o: r.v()?, e: r.v()?, w: r.u8()?, spark: r.u8()? },
        2 => Ev::Hit { victim: r.u8()?, dmg: r.f()?.clamp(0.0, 200.0), knock: r.v()?, stun: r.f()?.clamp(0.0, 2.0), head: r.u8()? == 1, w: r.u8()? },
        3 => Ev::Grenade { p: r.v()?, v: r.v()? },
        4 => Ev::Died { killer: r.u8()?, w: r.u8()? },
        _ => return None,
    })
}

pub struct RosterEntry {
    pub slot: usize,
    pub name: String,
    pub team: u8,
    pub kills: i32,
    pub deaths: i32,
}

/// Everything the host tells a player each update.
pub struct HostMsg {
    pub you: usize,
    pub mode: Mode,
    pub phase: Phase,
    pub timer: f32,
    pub target: u16,
    pub team_score: [u16; 2],
    pub number: u32,
    pub roster: Vec<RosterEntry>,
    pub fighters: Vec<(usize, FState)>,
    pub events: Vec<(usize, Ev)>,
}

pub enum Incoming {
    /// (host) a new player came in
    Join { slot: usize, name: String },
    /// (host) a player left or went quiet
    Leave { slot: usize },
    /// (host) a player's fighter and events
    PeerState { slot: usize, st: FState, events: Vec<Ev> },
    /// (player) the host's update
    Host(HostMsg),
    /// (player) the host closed the lobby
    HostLeft,
    /// (player) the lobby is full
    Full,
}

struct Peer {
    addr: SocketAddr,
    slot: usize,
    last_recv: Instant,
}

pub struct Net {
    sock: UdpSocket,
    pub is_host: bool,
    host_addr: Option<SocketAddr>,
    peers: Vec<Peer>,
    out: VecDeque<(u8, u32, Instant, Ev)>,
    next_id: u32,
    last_in: [u32; MAX_PLAYERS],
    roster_names: Vec<String>,
    send_acc: f32,
    hello_acc: f32,
    pub port: u16,
    pub last_recv: Instant,
    pub my_slot: usize,
}

impl Net {
    pub fn host(port: u16) -> Result<Net, String> {
        let ip = std::env::var("DUEL_BIND").unwrap_or_else(|_| "0.0.0.0".to_string());
        let sock = UdpSocket::bind((ip.as_str(), port)).map_err(|e| format!("Couldn't open port {port}: {e}"))?;
        sock.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(Net::make(sock, true, None, port))
    }

    pub fn join(addr: &str) -> Result<Net, String> {
        let addr = addr.trim();
        if addr.is_empty() {
            return Err("Type your friend's address first.".into());
        }
        let full = if addr.starts_with('[') || (addr.contains(':') && addr.matches(':').count() == 1) { addr.to_string() } else { format!("{addr}:{DEFAULT_PORT}") };
        let peer = full
            .to_socket_addrs()
            .map_err(|e| format!("Couldn't find {full}: {e}"))?
            .next()
            .ok_or_else(|| format!("Couldn't find {full}"))?;
        let bind = if peer.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
        let sock = UdpSocket::bind(bind).map_err(|e| e.to_string())?;
        sock.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(Net::make(sock, false, Some(peer), peer.port()))
    }

    fn make(sock: UdpSocket, is_host: bool, host_addr: Option<SocketAddr>, port: u16) -> Net {
        Net {
            sock,
            is_host,
            host_addr,
            peers: Vec::new(),
            out: VecDeque::new(),
            next_id: 1,
            last_in: [0; MAX_PLAYERS],
            roster_names: vec![String::new(); MAX_PLAYERS],
            send_acc: 0.0,
            hello_acc: 1.0,
            port,
            last_recv: Instant::now(),
            my_slot: 0,
        }
    }

    pub fn player_count(&self) -> usize {
        self.peers.len() + 1
    }

    fn free_slot(&self) -> Option<usize> {
        (1..MAX_PLAYERS).find(|s| !self.peers.iter().any(|p| p.slot == *s))
    }

    fn send_to(&self, data: &[u8], to: SocketAddr) {
        let _ = self.sock.send_to(data, to);
    }

    /// Read everything waiting on the socket.
    pub fn poll(&mut self) -> Vec<Incoming> {
        let mut out = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let (n, from) = match self.sock.recv_from(&mut buf) {
                Ok(x) => x,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break, // e.g. "connection reset" on Windows when a peer isn't up yet
            };
            let data = &buf[..n];
            if data.len() < 5 || data[..4] != MAGIC {
                continue;
            }
            let kind = data[4];
            let mut r = R { b: data, i: 5 };
            if self.is_host {
                self.host_packet(kind, &mut r, from, &mut out);
            } else if Some(from) == self.host_addr {
                self.last_recv = Instant::now();
                match kind {
                    K_HOST => {
                        if let Some(m) = self.parse_host(&mut r) {
                            out.push(Incoming::Host(m));
                        }
                    }
                    K_BYE => out.push(Incoming::HostLeft),
                    K_FULL => out.push(Incoming::Full),
                    _ => {}
                }
            }
        }
        if self.is_host {
            let mut gone = Vec::new();
            self.peers.retain(|p| {
                let quiet = p.last_recv.elapsed().as_secs_f32() > 6.0;
                if quiet {
                    gone.push(p.slot);
                }
                !quiet
            });
            for slot in gone {
                out.push(Incoming::Leave { slot });
            }
        }
        out
    }

    fn host_packet(&mut self, kind: u8, r: &mut R, from: SocketAddr, out: &mut Vec<Incoming>) {
        let idx = self.peers.iter().position(|p| p.addr == from);
        match kind {
            K_HELLO => {
                let name = r.s().unwrap_or_default();
                if let Some(i) = idx {
                    self.peers[i].last_recv = Instant::now();
                    return;
                }
                let Some(slot) = self.free_slot() else {
                    let w = W::head(K_FULL);
                    self.send_to(&w.0, from);
                    return;
                };
                self.peers.push(Peer { addr: from, slot, last_recv: Instant::now() });
                self.last_in[slot] = 0;
                self.out.retain(|e| e.0 as usize != slot);
                out.push(Incoming::Join { slot, name });
            }
            K_STATE => {
                let Some(i) = idx else { return };
                self.peers[i].last_recv = Instant::now();
                let slot = self.peers[i].slot;
                let Some(st) = read_fstate(r) else { return };
                let mut pairs = Vec::new();
                if let Some(n) = r.u8() {
                    for _ in 0..n {
                        let (Some(id), Some(ev)) = (r.u32(), read_ev(r)) else { break };
                        pairs.push((id, ev));
                    }
                }
                pairs.sort_by_key(|p| p.0);
                let mut fresh = Vec::new();
                for (id, ev) in pairs {
                    if id > self.last_in[slot] {
                        self.last_in[slot] = id;
                        // pass it on to everyone else
                        self.out.push_back((slot as u8, id, Instant::now(), ev.clone()));
                        fresh.push(ev);
                    }
                }
                out.push(Incoming::PeerState { slot, st, events: fresh });
            }
            K_BYE => {
                if let Some(i) = idx {
                    let slot = self.peers.remove(i).slot;
                    out.push(Incoming::Leave { slot });
                }
            }
            _ => {}
        }
    }

    fn parse_host(&mut self, r: &mut R) -> Option<HostMsg> {
        let you = (r.u8()? as usize).min(MAX_PLAYERS - 1);
        let mode = Mode::from_u8(r.u8()?);
        let phase = Phase::from_u8(r.u8()?);
        let timer = r.f()?;
        let target = r.u16()?;
        let team_score = [r.u16()?, r.u16()?];
        let number = r.u32()?;
        let n = r.u8()? as usize;
        let mut roster = Vec::new();
        for _ in 0..n.min(MAX_PLAYERS) {
            let slot = (r.u8()? as usize).min(MAX_PLAYERS - 1);
            let name = r.s()?;
            let team = r.u8()?;
            let kills = r.i16()? as i32;
            let deaths = r.i16()? as i32;
            roster.push(RosterEntry { slot, name, team, kills, deaths });
        }
        // a slot taken by someone new starts its event count again
        for e in &roster {
            if self.roster_names[e.slot] != e.name {
                self.roster_names[e.slot] = e.name.clone();
                self.last_in[e.slot] = 0;
            }
        }
        for s in 0..MAX_PLAYERS {
            if !roster.iter().any(|e| e.slot == s) {
                self.roster_names[s].clear();
            }
        }
        self.my_slot = you;
        let nf = r.u8()? as usize;
        let mut fighters = Vec::new();
        for _ in 0..nf.min(MAX_PLAYERS) {
            let slot = (r.u8()? as usize).min(MAX_PLAYERS - 1);
            let st = read_fstate(r)?;
            fighters.push((slot, st));
        }
        let ne = r.u8()? as usize;
        let mut evs = Vec::new();
        for _ in 0..ne {
            let origin = (r.u8()? as usize).min(MAX_PLAYERS - 1);
            let id = r.u32()?;
            let ev = read_ev(r)?;
            evs.push((origin, id, ev));
        }
        evs.sort_by_key(|e| (e.0, e.1));
        let mut events = Vec::new();
        for (origin, id, ev) in evs {
            if origin != you && id > self.last_in[origin] {
                self.last_in[origin] = id;
                events.push((origin, ev));
            }
        }
        Some(HostMsg { you, mode, phase, timer, target, team_score, number, roster, fighters, events })
    }

    pub fn send_hello(&mut self, name: &str, dt: f32) {
        self.hello_acc += dt;
        if self.hello_acc < 0.25 {
            return;
        }
        self.hello_acc = 0.0;
        let mut w = W::head(K_HELLO);
        w.s(name);
        if let Some(h) = self.host_addr {
            self.send_to(&w.0, h);
        }
    }

    pub fn send_bye(&mut self) {
        let w = W::head(K_BYE);
        for _ in 0..3 {
            if self.is_host {
                for p in &self.peers {
                    self.send_to(&w.0, p.addr);
                }
            } else if let Some(h) = self.host_addr {
                self.send_to(&w.0, h);
            }
        }
    }

    /// Queue one of our own events.
    pub fn queue(&mut self, ev: Ev) {
        let id = self.next_id;
        self.next_id += 1;
        self.out.push_back((self.my_slot as u8, id, Instant::now(), ev));
    }

    /// Recent events for someone (not their own), the important ones first.
    fn pick_events(&self, except: Option<usize>) -> Vec<&(u8, u32, Instant, Ev)> {
        let mut v: Vec<_> = self.out.iter().filter(|e| except.map_or(true, |x| e.0 as usize != x)).collect();
        v.sort_by_key(|e| (matches!(e.3, Ev::Shot { .. }), std::cmp::Reverse(e.1)));
        v.truncate(MAX_EVENTS);
        v
    }

    /// Send updates (about 60 a second).
    pub fn send(&mut self, g: &Game, dt: f32) {
        self.send_acc += dt;
        if self.send_acc < 1.0 / 60.0 {
            return;
        }
        self.send_acc = 0.0;
        while let Some((_, _, t, _)) = self.out.front() {
            if t.elapsed().as_secs_f32() > EVENT_WINDOW {
                self.out.pop_front();
            } else {
                break;
            }
        }
        if self.is_host {
            for p in &self.peers {
                let mut w = W::head(K_HOST);
                w.u8(p.slot as u8);
                w.u8(g.mt.mode as u8);
                w.u8(g.mt.phase as u8);
                w.f(g.mt.timer);
                w.u16(g.mt.target);
                w.u16(g.mt.team_score[0]);
                w.u16(g.mt.team_score[1]);
                w.u32(g.mt.number);
                let present = g.present();
                w.u8(present.len() as u8);
                for &s in &present {
                    let f = &g.f[s];
                    w.u8(s as u8);
                    w.s(&f.name);
                    w.u8(f.team);
                    w.i16(f.kills.clamp(-999, 9999) as i16);
                    w.i16(f.deaths.clamp(0, 9999) as i16);
                }
                let others: Vec<usize> = present.iter().copied().filter(|&s| s != p.slot).collect();
                w.u8(others.len() as u8);
                for s in others {
                    w.u8(s as u8);
                    write_fstate(&mut w, &g.f[s]);
                }
                let evs = self.pick_events(Some(p.slot));
                w.u8(evs.len() as u8);
                for (origin, id, _, ev) in evs {
                    w.u8(*origin);
                    w.u32(*id);
                    write_ev(&mut w, ev);
                }
                self.send_to(&w.0, p.addr);
            }
        } else if let Some(h) = self.host_addr {
            let mut w = W::head(K_STATE);
            write_fstate(&mut w, &g.f[g.local]);
            let evs = self.pick_events(None);
            w.u8(evs.len() as u8);
            for (_, id, _, ev) in evs {
                w.u32(*id);
                write_ev(&mut w, ev);
            }
            self.send_to(&w.0, h);
        }
    }
}

/// Copy a received state onto a fighter that lives on another computer.
pub fn apply_remote(f: &mut Fighter, s: &FState) {
    f.net_pos = s.pos;
    f.net_age = 0.0;
    if (f.pos - s.pos).len() > 6.0 {
        f.pos = s.pos;
        f.prev_pos = s.pos;
    }
    f.vel = s.vel;
    f.yaw = s.yaw;
    f.pitch = s.pitch;
    f.hp = s.hp;
    f.ap = s.ap;
    f.weapon = Weapon::from_u8(s.weapon);
    let alive = s.flags & 1 != 0;
    if f.alive && !alive {
        f.death_t = 0.0;
    }
    if !f.alive && alive {
        // came back: jump straight to the new spot
        f.pos = s.pos;
        f.prev_pos = s.pos;
    }
    f.alive = alive;
    f.on_ground = s.flags & 2 != 0;
    f.guard = s.flags & 4 != 0;
    f.wall = if s.flags & 8 != 0 { Some(WallRun { normal: s.wall_n, dir: V3::ZERO, t: 0.0, vertical: s.flags & 16 != 0 }) } else { None };
    f.dash_t = if s.flags & 32 != 0 { 0.1 } else { 0.0 };
    f.reload_t = if s.flags & 64 != 0 { 0.5 } else { 0.0 };
    f.stun_t = if s.flags & 128 != 0 { 0.1 } else { 0.0 };
    f.spawn_prot = if s.flags2 & 1 != 0 { 0.2 } else { 0.0 };
    f.slash_t = s.slash_t;
    f.slash_idx = s.slash_idx;
    f.hurt_t = f.hurt_t.max(s.hurt_t);
    f.recoil = s.recoil;
}

/// The computer's address on the local network (no packets are sent to find it).
pub fn local_ip() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}
