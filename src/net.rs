//! UDP lobby for up to 8 players. The host is the hub: every player sends its own fighter and
//! events to the host, and the host sends everyone the whole match (players, scores, fighters and
//! everybody else's events). Each player simulates its own fighter; hits are applied by the victim;
//! the host keeps the scores.

use crate::game::*;
use crate::math::*;
use std::collections::VecDeque;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

const MAGIC: [u8; 4] = *b"DRA6";
const K_HELLO: u8 = 1;
const K_STATE: u8 = 2;
const K_HOST: u8 = 3;
const K_BYE: u8 = 4;
const K_FULL: u8 = 5;
/// "I'm a different version": the packet is just the 4-byte mark and this kind.
const K_VERSION: u8 = 6;
pub const DEFAULT_PORT: u16 = 27960;
const EVENT_WINDOW: f32 = 0.5;
const MAX_EVENTS: usize = 48;
/// Largest packet we send. Bigger ones get split up on the internet, and a single lost piece
/// loses the whole packet (some routers drop the pieces outright).
pub const MAX_PACKET: usize = 1200;

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
    pub loadout: [u8; 4],
}

#[cfg(test)]
impl FState {
    /// A living, running fighter at `pos` (for tests).
    pub fn test_state(pos: V3, vel: V3) -> FState {
        FState { pos, vel, yaw: 0.0, pitch: 0.0, hp: MAX_HP, ap: 0.0, weapon: 1, flags: 1 | 2, flags2: 0, slash_t: 0.0, slash_idx: 0, hurt_t: 0.0, recoil: 0.0, wall_n: V3::ZERO, loadout: Loadout::default().to_bytes() }
    }
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
    w.u8(if f.spawn_prot > 0.0 { 1 } else { 0 } | if f.stagger_t > 0.0 { 2 } else { 0 } | if f.sliding > 0.0 { 4 } else { 0 });
    w.f(f.slash_t);
    w.u8(f.slash_idx);
    w.f(f.hurt_t);
    w.f(f.recoil);
    w.v(f.wall.map_or(V3::ZERO, |wr| wr.normal));
    for b in f.loadout.to_bytes() {
        w.u8(b);
    }
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
        loadout: [r.u8()?, r.u8()?, r.u8()?, r.u8()?],
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
        Ev::Hit { victim, dmg, knock, stun, head, w: wp, o, dir } => {
            w.u8(2);
            w.u8(*victim);
            w.f(*dmg);
            w.v(*knock);
            w.f(*stun);
            w.u8(*head as u8);
            w.u8(*wp);
            w.v(*o);
            w.v(*dir);
        }
        Ev::Grenade { p, v, kind } => {
            w.u8(3);
            w.v(*p);
            w.v(*v);
            w.u8(*kind);
        }
        Ev::Died { victim, killer, w: wp } => {
            w.u8(4);
            w.u8(*victim);
            w.u8(*killer);
            w.u8(*wp);
        }
        Ev::Chat { text } => {
            w.u8(5);
            let b = text.as_bytes();
            let n = b.len().min(200);
            w.u8(n as u8);
            w.0.extend_from_slice(&b[..n]);
        }
        Ev::Item { kind } => {
            w.u8(6);
            w.u8(*kind);
        }
        Ev::Ready => w.u8(7),
        Ev::Vote => w.u8(8),
    }
}

fn read_ev(r: &mut R) -> Option<Ev> {
    Some(match r.u8()? {
        1 => Ev::Shot { o: r.v()?, e: r.v()?, w: r.u8()?, spark: r.u8()? },
        2 => Ev::Hit { victim: r.u8()?, dmg: r.f()?.clamp(0.0, 200.0), knock: clamp_v(r.v()?, 30.0), stun: r.f()?.clamp(0.0, 2.0), head: r.u8()? == 1, w: r.u8()?, o: r.v()?, dir: r.v()? },
        3 => Ev::Grenade { p: r.v()?, v: clamp_v(r.v()?, 40.0), kind: r.u8()?.min(1) },
        4 => Ev::Died { victim: r.u8()?, killer: r.u8()?, w: r.u8()? },
        5 => {
            let n = r.u8()? as usize;
            Ev::Chat { text: String::from_utf8_lossy(r.take(n)?).chars().take(100).collect() }
        }
        6 => Ev::Item { kind: r.u8()?.min(1) },
        7 => Ev::Ready,
        8 => Ev::Vote,
        _ => return None,
    })
}

fn clamp_v(v: V3, max: f32) -> V3 {
    let l = v.len();
    if l > max { v * (max / l) } else { v }
}

pub struct RosterEntry {
    pub slot: usize,
    pub name: String,
    pub team: u8,
    pub kills: i32,
    pub deaths: i32,
    pub ping: u16,
    /// Share of this player's packets the host never got (percent).
    pub loss: u8,
    pub ready: bool,
    pub vote: bool,
    pub spectate: bool,
    pub wins: i32,
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
    pub round: u32,
    pub round_end: f32,
    pub duel: [u8; 2],
    pub flags: [Flag; 2],
    /// What the host thinks of you (it keeps everyone's health).
    pub you_hp: f32,
    pub you_ap: f32,
    pub you_alive: bool,
    pub roster: Vec<RosterEntry>,
    pub fighters: Vec<(usize, FState)>,
    pub events: Vec<(usize, Ev)>,
    /// The host's clock when it sent this.
    pub sent: f32,
    /// An older update that arrived after a newer one: only its events count.
    pub stale: bool,
}

pub enum Incoming {
    /// (host) a new player came in
    Join { slot: usize, name: String },
    /// (host) a player left or went quiet
    Leave { slot: usize },
    /// (host) a player's fighter and events
    PeerState { slot: usize, st: FState, events: Vec<Ev>, sent: f32, stale: bool },
    /// (player) the host's update
    Host(HostMsg),
    /// (player) the host closed the lobby
    HostLeft,
    /// (player) the lobby is full
    Full,
    /// (player) the host runs a different version of the game
    WrongVersion,
}

/// Network test: pretend the connection is slow and drops packets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NetSim {
    /// Extra round-trip time in milliseconds (half each way).
    pub lag_ms: f32,
    /// Share of packets dropped each way (0..1).
    pub loss: f32,
}

/// Packets from one sender: which arrived, which went missing.
#[derive(Clone, Copy)]
struct Link {
    last_seq: u32,
    recv: u32,
    lost: u32,
    /// Share lost over the last couple of seconds (0..1).
    loss: f32,
    since: Instant,
}

impl Link {
    fn new() -> Link {
        Link { last_seq: 0, recv: 0, lost: 0, loss: 0.0, since: Instant::now() }
    }
    /// Count a packet. False when it is older than one already seen (it came late).
    fn see(&mut self, seq: u32) -> bool {
        let fresh = seq > self.last_seq;
        if fresh {
            if self.last_seq > 0 {
                self.lost += (seq - self.last_seq - 1).min(120);
            }
            self.last_seq = seq;
        } else {
            self.lost = self.lost.saturating_sub(1); // late, not lost
        }
        self.recv += 1;
        if self.since.elapsed().as_secs_f32() >= 2.0 {
            let total = self.recv + self.lost;
            self.loss = if total > 0 { self.lost as f32 / total as f32 } else { 0.0 };
            self.recv = 0;
            self.lost = 0;
            self.since = Instant::now();
        }
        fresh
    }
}

/// Traffic over the last second.
#[derive(Clone, Copy, Default, Debug)]
pub struct Rates {
    pub pkts_in: f32,
    pub pkts_out: f32,
    pub kb_in: f32,
    pub kb_out: f32,
}

struct Peer {
    addr: SocketAddr,
    slot: usize,
    last_recv: Instant,
    ping: f32,
    link: Link,
    seq_out: u32,
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
    start: Instant,
    echo: f32,
    /// Network test (extra lag and packet loss), when switched on.
    pub sim: Option<NetSim>,
    out_q: Vec<(Instant, Vec<u8>, SocketAddr)>,
    in_q: Vec<(Instant, Vec<u8>, SocketAddr)>,
    seq: u32,
    rng: u64,
    host_link: Link,
    counts: [usize; 4],
    rate_since: Instant,
    pub rates: Rates,
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
            start: Instant::now(),
            echo: 0.0,
            sim: None,
            out_q: Vec::new(),
            in_q: Vec::new(),
            seq: 0,
            rng: 0x2545F4914F6CDD1D ^ (port as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ (std::process::id() as u64) << 20,
            host_link: Link::new(),
            counts: [0; 4],
            rate_since: Instant::now(),
            rates: Rates::default(),
        }
    }

    /// (slot, ping in ms) for every player connected to this host.
    pub fn pings(&self) -> Vec<(usize, u16)> {
        self.peers.iter().map(|p| (p.slot, (p.ping * 1000.0).round().min(9999.0) as u16)).collect()
    }

    /// (slot, share of their packets lost) for every player connected to this host.
    pub fn losses(&self) -> Vec<(usize, f32)> {
        self.peers.iter().map(|p| (p.slot, p.link.loss)).collect()
    }

    /// (player) share of the host's packets that never arrived.
    pub fn down_loss(&self) -> f32 {
        self.host_link.loss
    }

    pub fn player_count(&self) -> usize {
        self.peers.len() + 1
    }

    /// Where we're connected (player) or listening (host).
    pub fn address(&self) -> String {
        match self.host_addr {
            Some(a) => a.to_string(),
            None => format!("port {}", self.port),
        }
    }

    fn free_slot(&self) -> Option<usize> {
        (1..MAX_PLAYERS).find(|s| !self.peers.iter().any(|p| p.slot == *s))
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Network test: half the extra lag each way, with a little jitter (so packets sometimes
    /// arrive out of order, as they do on the internet).
    fn sim_delay(&mut self, sim: NetSim) -> Duration {
        let half = sim.lag_ms.max(0.0) * 0.5;
        let jitter = (self.rand() * 2.0 - 1.0) * (half * 0.15 + 2.0);
        Duration::from_secs_f32((half + jitter).max(0.0) / 1000.0)
    }

    /// Start a packet: kind, a number that grows with every packet to that address, and our clock.
    fn head(&mut self, kind: u8, to: SocketAddr) -> W {
        let seq = match self.peers.iter_mut().find(|p| p.addr == to) {
            Some(p) => {
                p.seq_out += 1;
                p.seq_out
            }
            None => {
                self.seq += 1;
                self.seq
            }
        };
        let mut w = W::head(kind);
        w.u32(seq);
        w.f(self.start.elapsed().as_secs_f32());
        w
    }

    fn send_to(&mut self, data: Vec<u8>, to: SocketAddr) {
        self.counts[2] += 1;
        self.counts[3] += data.len();
        if let Some(sim) = self.sim {
            if self.rand() < sim.loss {
                return; // dropped on the way
            }
            let d = self.sim_delay(sim);
            self.out_q.push((Instant::now() + d, data, to));
            return;
        }
        let _ = self.sock.send_to(&data, to);
    }

    /// Send what the network test held back once its time is up.
    fn flush(&mut self) {
        let now = Instant::now();
        let mut i = 0;
        while i < self.out_q.len() {
            if self.out_q[i].0 <= now {
                let (_, d, to) = self.out_q.swap_remove(i);
                let _ = self.sock.send_to(&d, to);
            } else {
                i += 1;
            }
        }
    }

    /// Read everything waiting on the socket.
    pub fn poll(&mut self) -> Vec<Incoming> {
        let mut out = Vec::new();
        let mut buf = [0u8; 8192];
        let now = Instant::now();
        loop {
            let (n, from) = match self.sock.recv_from(&mut buf) {
                Ok(x) => x,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break, // e.g. "connection reset" on Windows when a peer isn't up yet
            };
            let data = buf[..n].to_vec();
            match self.sim {
                Some(sim) => {
                    if self.rand() < sim.loss {
                        continue;
                    }
                    let d = self.sim_delay(sim);
                    self.in_q.push((now + d, data, from));
                }
                None => self.in_q.push((now, data, from)),
            }
        }
        // handle what has "arrived", oldest first
        self.in_q.sort_by_key(|e| e.0);
        let due = self.in_q.iter().take_while(|e| e.0 <= now).count();
        let ready: Vec<_> = self.in_q.drain(..due).collect();
        for (_, data, from) in ready {
            self.handle(&data, from, &mut out);
        }
        self.flush();
        if self.rate_since.elapsed().as_secs_f32() >= 1.0 {
            let t = self.rate_since.elapsed().as_secs_f32();
            self.rates = Rates {
                pkts_in: self.counts[0] as f32 / t,
                kb_in: self.counts[1] as f32 / t / 1024.0,
                pkts_out: self.counts[2] as f32 / t,
                kb_out: self.counts[3] as f32 / t / 1024.0,
            };
            self.counts = [0; 4];
            self.rate_since = Instant::now();
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

    fn handle(&mut self, data: &[u8], from: SocketAddr, out: &mut Vec<Incoming>) {
        self.counts[0] += 1;
        self.counts[1] += data.len();
        if data.len() >= 4 && data[..3] == MAGIC[..3] && data[3] != MAGIC[3] {
            // the same game, another version: they can't understand each other, so say so
            if self.is_host {
                let reply = vec![MAGIC[0], MAGIC[1], MAGIC[2], MAGIC[3], K_VERSION];
                let _ = self.sock.send_to(&reply, from);
            } else if Some(from) == self.host_addr && data.len() >= 5 && data[4] == K_VERSION {
                out.push(Incoming::WrongVersion);
            }
            return;
        }
        if data.len() < 13 || data[..4] != MAGIC {
            return;
        }
        let kind = data[4];
        let mut r = R { b: data, i: 5 };
        let (Some(seq), Some(sent)) = (r.u32(), r.f()) else { return };
        if self.is_host {
            self.host_packet(kind, &mut r, from, seq, sent, out);
        } else if Some(from) == self.host_addr {
            self.last_recv = Instant::now();
            let fresh = self.host_link.see(seq);
            match kind {
                K_HOST => {
                    if fresh {
                        self.echo = sent;
                    }
                    if let Some(m) = self.parse_host(&mut r, sent, !fresh) {
                        out.push(Incoming::Host(m));
                    }
                }
                K_BYE => out.push(Incoming::HostLeft),
                K_FULL => out.push(Incoming::Full),
                _ => {}
            }
        }
    }

    fn host_packet(&mut self, kind: u8, r: &mut R, from: SocketAddr, seq: u32, sent: f32, out: &mut Vec<Incoming>) {
        let idx = self.peers.iter().position(|p| p.addr == from);
        match kind {
            K_HELLO => {
                let name = r.s().unwrap_or_default();
                if let Some(i) = idx {
                    self.peers[i].last_recv = Instant::now();
                    self.peers[i].link.see(seq);
                    return;
                }
                let Some(slot) = self.free_slot() else {
                    let w = self.head(K_FULL, from);
                    self.send_to(w.0, from);
                    return;
                };
                let mut link = Link::new();
                link.see(seq);
                self.peers.push(Peer { addr: from, slot, last_recv: Instant::now(), ping: 0.05, link, seq_out: 0 });
                self.last_in[slot] = 0;
                self.out.retain(|e| e.0 as usize != slot);
                out.push(Incoming::Join { slot, name });
            }
            K_STATE => {
                let Some(i) = idx else { return };
                self.peers[i].last_recv = Instant::now();
                let fresh = self.peers[i].link.see(seq);
                let slot = self.peers[i].slot;
                let Some(echo) = r.f() else { return };
                if echo > 0.0 && fresh {
                    let rtt = (self.start.elapsed().as_secs_f32() - echo).clamp(0.0, 2.0);
                    self.peers[i].ping = self.peers[i].ping * 0.85 + rtt * 0.15;
                }
                let Some(st) = read_fstate(r) else { return };
                let mut pairs = Vec::new();
                if let Some(n) = r.u8() {
                    for _ in 0..n {
                        let (Some(id), Some(ev)) = (r.u32(), read_ev(r)) else { break };
                        pairs.push((id, ev));
                    }
                }
                pairs.sort_by_key(|p| p.0);
                let mut fresh_evs = Vec::new();
                for (id, ev) in pairs {
                    if id > self.last_in[slot] {
                        self.last_in[slot] = id;
                        // pass on what others need to see; hit claims stay here until checked
                        if matches!(ev, Ev::Shot { .. } | Ev::Grenade { .. } | Ev::Chat { .. }) {
                            self.out.push_back((slot as u8, id, Instant::now(), ev.clone()));
                        }
                        fresh_evs.push(ev);
                    }
                }
                out.push(Incoming::PeerState { slot, st, events: fresh_evs, sent, stale: !fresh });
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

    fn parse_host(&mut self, r: &mut R, sent: f32, stale: bool) -> Option<HostMsg> {
        let you = (r.u8()? as usize).min(MAX_PLAYERS - 1);
        let you_hp = r.f()?;
        let you_ap = r.f()?;
        let you_alive = r.u8()? == 1;
        let mode = Mode::from_u8(r.u8()?);
        let phase = Phase::from_u8(r.u8()?);
        let timer = r.f()?;
        let target = r.u16()?;
        let team_score = [r.u16()?, r.u16()?];
        let number = r.u32()?;
        let round = r.u32()?;
        let round_end = r.f()?;
        let duel = [r.u8()?, r.u8()?];
        let mut flags = [Flag { state: 0, carrier: 255, pos: V3::ZERO, timer: 0.0 }; 2];
        for fl in flags.iter_mut() {
            fl.state = r.u8()?.min(2);
            fl.carrier = r.u8()?;
            fl.pos = r.v()?;
            fl.timer = r.f()?;
        }
        let n = r.u8()? as usize;
        let mut roster = Vec::new();
        for _ in 0..n.min(MAX_PLAYERS) {
            let slot = (r.u8()? as usize).min(MAX_PLAYERS - 1);
            let name = r.s()?;
            let team = r.u8()?;
            let kills = r.i16()? as i32;
            let deaths = r.i16()? as i32;
            let ping = r.u16()?;
            let loss = r.u8()?;
            let bits = r.u8()?;
            let wins = r.i16()? as i32;
            roster.push(RosterEntry { slot, name, team, kills, deaths, ping, loss, ready: bits & 1 != 0, vote: bits & 2 != 0, spectate: bits & 4 != 0, wins });
        }
        if !stale {
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
        }
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
        Some(HostMsg { you, mode, phase, timer, target, team_score, number, round, round_end, duel, flags, you_hp, you_ap, you_alive, roster, fighters, events, sent, stale })
    }

    pub fn send_hello(&mut self, name: &str, dt: f32) {
        self.hello_acc += dt;
        self.flush();
        if self.hello_acc < 0.25 {
            return;
        }
        self.hello_acc = 0.0;
        if let Some(h) = self.host_addr {
            let mut w = self.head(K_HELLO, h);
            w.s(name);
            self.send_to(w.0, h);
        }
    }

    pub fn send_bye(&mut self) {
        // goodbyes skip the network test: the window is closing
        self.sim = None;
        let targets: Vec<SocketAddr> = if self.is_host { self.peers.iter().map(|p| p.addr).collect() } else { self.host_addr.into_iter().collect() };
        for _ in 0..3 {
            for &to in &targets {
                let w = self.head(K_BYE, to);
                self.send_to(w.0, to);
            }
        }
        let now = Instant::now();
        for e in self.out_q.iter_mut() {
            e.0 = now;
        }
        self.flush();
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
        self.flush();
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
            let targets: Vec<(SocketAddr, usize)> = self.peers.iter().map(|p| (p.addr, p.slot)).collect();
            let mut packets = Vec::new();
            for (addr, slot) in targets {
                let mut w = self.head(K_HOST, addr);
                w.u8(slot as u8);
                let you = &g.f[slot];
                w.f(you.hp);
                w.f(you.ap);
                w.u8(you.alive as u8);
                w.u8(g.mt.mode as u8);
                w.u8(g.mt.phase as u8);
                w.f(g.mt.timer);
                w.u16(g.mt.target);
                w.u16(g.mt.team_score[0]);
                w.u16(g.mt.team_score[1]);
                w.u32(g.mt.number);
                w.u32(g.mt.round);
                w.f(g.mt.round_end);
                w.u8(g.mt.duel[0]);
                w.u8(g.mt.duel[1]);
                for fl in &g.mt.flags {
                    w.u8(fl.state);
                    w.u8(fl.carrier);
                    w.v(fl.pos);
                    w.f(fl.timer);
                }
                let present = g.present();
                w.u8(present.len() as u8);
                for &s in &present {
                    let f = &g.f[s];
                    w.u8(s as u8);
                    w.s(&f.name);
                    w.u8(f.team);
                    w.i16(f.kills.clamp(-999, 9999) as i16);
                    w.i16(f.deaths.clamp(0, 9999) as i16);
                    w.u16(f.ping_ms);
                    let loss = self.peers.iter().find(|p| p.slot == s).map_or(0.0, |p| p.link.loss);
                    w.u8((loss * 100.0).round().min(100.0) as u8);
                    w.u8((f.ready as u8) | ((f.vote as u8) << 1) | ((f.spectate as u8) << 2));
                    w.i16(f.wins.clamp(-999, 9999) as i16);
                }
                let others: Vec<usize> = present.iter().copied().filter(|&s| s != slot).collect();
                w.u8(others.len() as u8);
                for s in others {
                    w.u8(s as u8);
                    write_fstate(&mut w, &g.f[s]);
                }
                // as many events as fit (the important ones first; the rest go in the next update)
                let evs = self.pick_events(Some(slot));
                let count_at = w.0.len();
                w.u8(0);
                let mut n = 0u8;
                for (origin, id, _, ev) in evs {
                    let mut e = W(Vec::with_capacity(48));
                    e.u8(*origin);
                    e.u32(*id);
                    write_ev(&mut e, ev);
                    if w.0.len() + e.0.len() > MAX_PACKET {
                        break;
                    }
                    w.0.extend_from_slice(&e.0);
                    n += 1;
                }
                w.0[count_at] = n;
                packets.push((w.0, addr));
            }
            for (d, a) in packets {
                self.send_to(d, a);
            }
        } else if let Some(h) = self.host_addr {
            let mut w = self.head(K_STATE, h);
            w.f(self.echo);
            write_fstate(&mut w, &g.f[g.local]);
            let evs = self.pick_events(None);
            let count_at = w.0.len();
            w.u8(0);
            let mut n = 0u8;
            for (_, id, _, ev) in evs {
                let mut e = W(Vec::with_capacity(48));
                e.u32(*id);
                write_ev(&mut e, ev);
                if w.0.len() + e.0.len() > MAX_PACKET {
                    break;
                }
                w.0.extend_from_slice(&e.0);
                n += 1;
            }
            w.0[count_at] = n;
            self.send_to(w.0, h);
        }
    }
}
/// Copy a received state onto a fighter that lives on another computer.
/// On the host (uthority), health and life stay the host's: a player can only come back to life once its respawn time is up.
/// `sent` is the sender's clock when it sent this and `now` our game clock: together they let the
/// fighter be played back smoothly a fixed moment behind (see `Game::tick`).
pub fn apply_remote(f: &mut Fighter, s: &FState, authority: bool, sent: f32, now: f32) {
    f.net_pos = s.pos;
    f.net_age = 0.0;
    // how far our clock is ahead of the sender's, following the quickest packets
    let o = now - sent;
    let off = match f.clock_off {
        None => o,
        Some(c) if o < c => c + (o - c) * 0.5,
        Some(c) => c + (o - c) * 0.01,
    };
    f.clock_off = Some(off);
    // a jumpy connection gets a little more room, changed slowly so playback stays even
    let late = (o - off).clamp(0.0, 0.3);
    f.lateness = f.lateness * 0.97 + late * 0.03;
    let want = (INTERP_DELAY * 0.8 + f.lateness * 2.5).clamp(INTERP_DELAY, MAX_INTERP);
    f.interp += (want - f.interp) * 0.02;
    if (f.pos - s.pos).len() > 6.0 {
        f.pos = s.pos;
        f.prev_pos = s.pos;
        f.snaps.clear();
    }
    if f.snaps.back().map_or(true, |b| sent > b.t) {
        f.snaps.push_back(Snap { t: sent, pos: s.pos, yaw: s.yaw, pitch: s.pitch, slash_t: s.slash_t, slash_idx: s.slash_idx });
        while f.snaps.len() > 48 {
            f.snaps.pop_front();
        }
    }
    if f.snaps.len() <= 1 {
        f.yaw = s.yaw;
        f.pitch = s.pitch;
    }
    f.vel = s.vel;
    f.weapon = Weapon::from_u8(s.weapon);
    let lo = Loadout::from_bytes(s.loadout);
    if lo != f.loadout {
        f.loadout = lo;
        f.items = f.items.min(2);
    }
    let reported = s.flags & 1 != 0;
    let alive = if authority {
        if f.alive {
            true
        } else if reported && f.respawn_t <= 0.5 {
            // back from the dead: fresh health and items
            f.hp = MAX_HP;
            f.ap = MAX_AP;
            f.items = 2;
            true
        } else {
            false
        }
    } else {
        f.hp = s.hp;
        f.ap = s.ap;
        reported
    };
    if f.alive && !alive {
        f.death_t = 0.0;
    }
    if !f.alive && alive {
        // came back: jump straight to the new spot
        f.pos = s.pos;
        f.prev_pos = s.pos;
        f.snaps.clear();
        f.snaps.push_back(Snap { t: sent, pos: s.pos, yaw: s.yaw, pitch: s.pitch, slash_t: 0.0, slash_idx: 0 });
    }
    f.alive = alive;
    f.on_ground = s.flags & 2 != 0;
    f.guard = s.flags & 4 != 0;
    f.wall = if s.flags & 8 != 0 { Some(WallRun { normal: s.wall_n, dir: V3::ZERO, t: 0.0, vertical: s.flags & 16 != 0, speed: 10.0 }) } else { None };
    f.dash_t = if s.flags & 32 != 0 { 0.1 } else { 0.0 };
    f.reload_t = if s.flags & 64 != 0 { 0.5 } else { 0.0 };
    f.stun_t = if s.flags & 128 != 0 { 0.1 } else { 0.0 };
    f.spawn_prot = if s.flags2 & 1 != 0 { 0.2 } else { 0.0 };
    f.stagger_t = if s.flags2 & 2 != 0 { 0.2 } else { 0.0 };
    f.sliding = if s.flags2 & 4 != 0 { 0.1 } else { 0.0 };
    if f.snaps.len() <= 1 {
        f.slash_t = s.slash_t;
        f.slash_idx = s.slash_idx;
    }
    f.hurt_t = f.hurt_t.max(s.hurt_t);
    f.recoil = s.recoil;
}

/// The computer's address on the local network (no packets are sent to find it).
pub fn local_ip() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

/// Ask a public "what's my IP" service for this computer's internet address (plain HTTP, no
/// details about the player are sent). Tries a few services; blocks for a few seconds at most.
pub fn fetch_public_ip() -> Result<String, String> {
    use std::io::{Read, Write};
    use std::net::{IpAddr, TcpStream};
    let mut last = String::from("no service answered");
    for host in ["api.ipify.org", "icanhazip.com", "ifconfig.me"] {
        let addr = match (host, 80).to_socket_addrs().ok().and_then(|mut a| a.find(|x| x.is_ipv4())) {
            Some(a) => a,
            None => {
                last = format!("couldn't look up {host}");
                continue;
            }
        };
        let mut s = match TcpStream::connect_timeout(&addr, Duration::from_secs(4)) {
            Ok(s) => s,
            Err(e) => {
                last = e.to_string();
                continue;
            }
        };
        let _ = s.set_read_timeout(Some(Duration::from_secs(4)));
        let _ = s.set_write_timeout(Some(Duration::from_secs(4)));
        let req = format!("GET / HTTP/1.1\r\nHost: {host}\r\nUser-Agent: curl/8.5.0\r\nAccept: text/plain\r\nConnection: close\r\n\r\n");
        if s.write_all(req.as_bytes()).is_err() {
            continue;
        }
        let mut body = Vec::new();
        let _ = s.take(16 * 1024).read_to_end(&mut body);
        let text = String::from_utf8_lossy(&body);
        let Some((head, rest)) = text.split_once("\r\n\r\n") else { continue };
        if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
            last = format!("{host}: {}", head.lines().next().unwrap_or(""));
            continue;
        }
        // the address is the body, or one line of it (chunked replies)
        if let Some(ip) = rest.lines().map(str::trim).find_map(|l| l.parse::<IpAddr>().ok()) {
            if !ip.is_loopback() && !ip.is_unspecified() {
                return Ok(ip.to_string());
            }
        }
    }
    Err(last)
}
#[cfg(test)]
mod public_ip_tests {
    /// Goes online, so it only runs when asked: cargo test -- --ignored public_ip
    #[test]
    #[ignore]
    fn public_ip_lookup_works() {
        let ip = super::fetch_public_ip().expect("an address");
        assert!(ip.parse::<std::net::IpAddr>().is_ok());
    }
}

#[cfg(test)]
mod packet_size_tests {
    use super::*;

    /// A full lobby in the middle of a firefight still sends packets that fit the internet's size.
    #[test]
    fn host_packets_fit_under_the_limit() {
        let mut g = Game::new(Kind::Host, 0, Mode::Ctf, 5);
        for s in 0..MAX_PLAYERS {
            g.add_player(s, "LongPlayerName16", s == 0, None, (s % 2) as u8);
        }
        let mut n = Net::host(0).expect("socket");
        // hold everything back so it can be measured
        n.sim = Some(NetSim { lag_ms: 600_000.0, loss: 0.0 });
        for s in 1..MAX_PLAYERS {
            let addr: SocketAddr = format!("127.0.0.1:{}", 40000 + s).parse().unwrap();
            n.peers.push(Peer { addr, slot: s, last_recv: Instant::now(), ping: 0.05, link: Link::new(), seq_out: 0 });
        }
        for k in 0..60u32 {
            let o = v3(k as f32, 1.0, 0.0);
            n.out.push_back(((k % 8) as u8, k + 1, Instant::now(), Ev::Shot { o, e: o + v3(0.0, 0.0, 20.0), w: 3, spark: 1 }));
            n.out.push_back(((k % 8) as u8, k + 1000, Instant::now(), Ev::Chat { text: "a fairly long chat line to fill the packet up".into() }));
        }
        n.send(&g, 1.0);
        assert_eq!(n.out_q.len(), MAX_PLAYERS - 1, "one update per player");
        for (_, data, _) in &n.out_q {
            assert!(data.len() <= MAX_PACKET, "packet of {} bytes", data.len());
            assert!(data.len() > 600, "still carries the match ({} bytes)", data.len());
        }
    }
}
