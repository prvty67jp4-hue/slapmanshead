//! UDP lobby for up to 8 players. The host is the hub: every player sends its own fighter and
//! events to the host, and the host sends everyone the whole match (players, scores, fighters and
//! everybody else's events). Each player simulates its own fighter; hits are applied by the victim;
//! the host keeps the scores.

use crate::game::*;
use crate::math::*;
use std::collections::VecDeque;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

const MAGIC: [u8; 4] = *b"DRB2";
/// The network protocol number: two copies of the game can play together when theirs match,
/// whatever their build numbers (so a Mac download that's a few builds behind still plays with
/// Windows and Linux as long as nothing both sides must agree on has changed since).
///
/// Bump it (and PROTOCOL below with it) whenever the netcode changes (packets, events, what a
/// fighter's state carries) or anything both sides must agree on does: map layouts, the hit
/// rules and the game's tuning. The `match_fingerprint` test notices most such changes and fails
/// until it's bumped.
pub const NET_PROTOCOL: u32 = 16;
/// The same as text (Steam lobbies only list games that speak the same one).
pub const PROTOCOL: &str = "DRA16";
/// Steam message channels: game packets, and lobby-browser ping probes.
pub const STEAM_GAME_CHANNEL: u32 = 0;
pub const STEAM_PING_CHANNEL: u32 = 1;
/// Admin messages straight between players (crate::commands::Direct).
pub const STEAM_ADMIN_CHANNEL: u32 = 2;

/// Steam players have no IP address here; each gets a made-up one built from their Steam ID
/// (in a private IPv6 range), so the rest of the netcode treats them like anyone else.
pub fn steam_addr(id: u64) -> SocketAddr {
    let ip = std::net::Ipv6Addr::new(0xfd53, 0x7465, 0x616d, 0, (id >> 48) as u16, (id >> 32) as u16, (id >> 16) as u16, id as u16);
    SocketAddr::new(std::net::IpAddr::V6(ip), 1)
}

/// The Steam ID behind a made-up address, if it is one.
pub fn addr_steam(a: &SocketAddr) -> Option<u64> {
    match a {
        SocketAddr::V6(v6) if v6.port() == 1 => {
            let s = v6.ip().segments();
            if s[0] == 0xfd53 && s[1] == 0x7465 && s[2] == 0x616d && s[3] == 0 {
                Some((s[4] as u64) << 48 | (s[5] as u64) << 32 | (s[6] as u64) << 16 | s[7] as u64)
            } else {
                None
            }
        }
        _ => None,
    }
}
const K_HELLO: u8 = 1;
const K_STATE: u8 = 2;
const K_HOST: u8 = 3;
const K_BYE: u8 = 4;
const K_FULL: u8 = 5;
/// (host) who's connected and how well: the list everyone needs to pick a new host.
const K_PEERS: u8 = 6;
/// (player) "I'm back, in my old slot" to a player who took over as host.
const K_REJOIN: u8 = 7;
/// "I'm a different version": the packet is just the 4-byte mark and this kind.
const K_VERSION: u8 = 8;
/// (host) "we're on different builds": the host's build number follows.
const K_BUILD: u8 = 9;
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
#[derive(Clone, Debug)]
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
    /// 1 kicking, 2 heavy charged, 4 charging, 8 dash-through, 16 greatsword plunge.
    pub flags3: u8,
    /// 1 Iron Wall up, 2 invisible (Afterimage), 4 on a grapple line, 8 a rocket locking in,
    /// 16 a grenade winding up.
    pub flags4: u8,
    /// Ability charge (hundredths; shown on enemies' nameplates up close).
    pub ab: u8,
    /// Execution being played (number + 1; 0 = none; see crate::exec).
    pub fin: u8,
    pub slash_t: f32,
    pub slash_idx: u8,
    pub hurt_t: f32,
    pub recoil: f32,
    pub wall_n: V3,
    pub loadout: [u8; 4],
    /// Ledger Duels: the gear worn (crate::gear item ids; all 0 outside them), the special bar
    /// and what they're doing (crate::gear::B_* bits).
    pub gear: [u8; crate::gear::SLOTS],
    pub spec: u8,
    pub gbits: u8,
    pub dyes: [u8; 3],
    /// The Varrow Hub: their emote and seat (crate::hub).
    pub emote: u8,
    pub seat: u16,
}

#[cfg(test)]
impl FState {
    /// A living, running fighter at `pos` (for tests).
    pub fn test_state(pos: V3, vel: V3) -> FState {
        FState { pos, vel, yaw: 0.0, pitch: 0.0, hp: MAX_HP, ap: 0.0, weapon: 1, flags: 1 | 2, flags2: 0, flags3: 0, flags4: 0, ab: 0, fin: 0, slash_t: 0.0, slash_idx: 0, hurt_t: 0.0, recoil: 0.0, wall_n: V3::ZERO, loadout: Loadout::default().to_bytes(), gear: [0; crate::gear::SLOTS], spec: 0, gbits: 0, dyes: crate::gear::DEFAULT_DYES, emote: 0, seat: crate::hub::SEAT_NONE }
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
    w.u8(if f.spawn_prot > 0.0 { 1 } else { 0 } | if f.stagger_t > 0.0 { 2 } else { 0 } | if f.sliding > 0.0 { 4 } else { 0 } | if f.scope_t > 0.0 { 8 } else { 0 });
    w.u8(if f.kick_t > 0.0 { 1 } else { 0 } | if f.charged { 2 } else { 0 } | if f.charge_t > 0.2 { 4 } else { 0 } | if f.through_t > 0.0 { 8 } else { 0 } | if f.slam { 16 } else { 0 } | if f.throw_t > 0.0 { 32 } else { 0 } | if f.throw_fan { 64 } else { 0 });
    w.u8(if f.shield_t > 0.0 { 1 } else { 0 } | if f.invis_t > 0.0 { 2 } else { 0 } | if f.grapple.is_some() { 4 } else { 0 } | if f.lock_t > 0.0 { 8 } else { 0 } | if f.nade_wind > 0.0 { 16 } else { 0 });
    w.u8((f.ab_charge * 100.0).round().clamp(0.0, 255.0) as u8);
    w.u8(f.fin);
    w.f(f.slash_t);
    w.u8(f.slash_idx);
    w.f(f.hurt_t);
    w.f(f.recoil);
    w.v(f.wall.map_or(V3::ZERO, |wr| wr.normal));
    for b in f.loadout.to_bytes() {
        w.u8(b);
    }
    match &f.kit {
        Some(k) => {
            for &id in &k.equip {
                w.u8(id);
            }
            w.u8(k.spec.round().clamp(0.0, 100.0) as u8);
            w.u8(k.bits());
            for d in k.dyes {
                w.u8(d);
            }
        }
        None => {
            for _ in 0..crate::gear::SLOTS + 5 {
                w.u8(0);
            }
        }
    }
    w.u8(f.hub.emote);
    w.u16(f.hub.seat);
}

/// (the hub) A fighter as the host sends them to everyone: in full for a pit duellist, else
/// just what the picture needs (31 bytes instead of about 100, so sixteen fit in a packet).
fn write_fstate_hub(w: &mut W, f: &Fighter, room: bool) {
    if f.hub.pit != 255 && room {
        w.u8(0);
        write_fstate(w, f);
        return;
    }
    // (a duellist sent short, when the packet is full: their health isn't in it)
    w.u8(if f.hub.pit != 255 { 2 } else { 1 });
    w.v(f.pos);
    for c in [f.vel.x, f.vel.y, f.vel.z] {
        w.i16((c * 100.0).round().clamp(-32000.0, 32000.0) as i16);
    }
    w.u16((wrap_angle(f.yaw).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * 65535.0).round() as u16);
    w.i16((f.pitch.clamp(-1.6, 1.6) * 10000.0).round() as i16);
    w.u8(f.weapon as u8);
    w.u8(f.flags());
    w.u8(if f.sliding > 0.0 { 4 } else { 0 });
    let n = f.wall.map_or(V3::ZERO, |wr| wr.normal);
    for c in [n.x, n.y, n.z] {
        w.u8(((c.clamp(-1.0, 1.0) * 127.0).round() as i8) as u8);
    }
    for b in f.loadout.to_bytes() {
        w.u8(b);
    }
    w.u8(f.hub.emote);
    w.u16(f.hub.seat);
}

fn read_fstate_hub(r: &mut R) -> Option<FState> {
    let kind = r.u8()?;
    if kind == 0 {
        return read_fstate(r);
    }
    let pos = r.v()?;
    let vel = v3(r.i16()? as f32 / 100.0, r.i16()? as f32 / 100.0, r.i16()? as f32 / 100.0);
    let yaw = r.u16()? as f32 / 65535.0 * std::f32::consts::TAU;
    let pitch = r.i16()? as f32 / 10000.0;
    let weapon = r.u8()?;
    let flags = r.u8()?;
    let flags2 = r.u8()?;
    let wall_n = v3(r.u8()? as i8 as f32 / 127.0, r.u8()? as i8 as f32 / 127.0, r.u8()? as i8 as f32 / 127.0);
    let loadout = [r.u8()?, r.u8()?, r.u8()?, r.u8()?];
    let emote = r.u8()?.min(8);
    let seat = r.u16()?;
    let hp = if kind == 2 { -1.0 } else { MAX_HP };
    Some(FState { pos, vel, yaw, pitch, hp, ap: 0.0, weapon, flags, flags2, flags3: 0, flags4: 0, ab: 0, fin: 0, slash_t: 0.0, slash_idx: 0, hurt_t: 0.0, recoil: 0.0, wall_n, loadout, gear: [0; crate::gear::SLOTS], spec: 0, gbits: 0, dyes: crate::gear::DEFAULT_DYES, emote, seat })
}

/// A fighter's state as the network writes it (replays keep it the same way).
pub fn encode_fstate(out: &mut Vec<u8>, f: &Fighter) {
    let mut w = W(std::mem::take(out));
    write_fstate(&mut w, f);
    *out = w.0;
}

/// A fighter's state read back from  at t (moved past it).
pub fn decode_fstate(b: &[u8], at: &mut usize) -> Option<FState> {
    let mut r = R { b, i: *at };
    let s = read_fstate(&mut r)?;
    *at = r.i;
    Some(s)
}

/// (tests) An event written and read back.
#[cfg(test)]
pub fn ev_round_trip(ev: &Ev) -> Option<Ev> {
    let mut w = W(Vec::new());
    write_ev(&mut w, ev);
    read_ev(&mut R { b: &w.0, i: 0 })
}

/// (tests) A fighter's state written and read back.
#[cfg(test)]
pub fn fstate_round_trip(f: &Fighter) -> Option<FState> {
    let mut w = W(Vec::new());
    write_fstate(&mut w, f);
    read_fstate(&mut R { b: &w.0, i: 0 })
}

/// (tests) A fighter's state as the hub's host sends it, read back.
#[cfg(test)]
pub fn fstate_hub_round_trip(f: &Fighter) -> Option<FState> {
    let mut w = W(Vec::new());
    write_fstate_hub(&mut w, f, true);
    read_fstate_hub(&mut R { b: &w.0, i: 0 })
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
        flags3: r.u8()?,
        flags4: r.u8()?,
        ab: r.u8()?,
        fin: r.u8()?.min(crate::exec::COUNT as u8),
        slash_t: r.f()?,
        slash_idx: r.u8()?,
        hurt_t: r.f()?,
        recoil: r.f()?,
        wall_n: r.v()?,
        loadout: [r.u8()?, r.u8()?, r.u8()?, r.u8()?],
        gear: {
            let mut g = [0u8; crate::gear::SLOTS];
            for x in g.iter_mut() {
                *x = r.u8()?;
            }
            g
        },
        spec: r.u8()?.min(100),
        gbits: r.u8()?,
        dyes: {
            let n = crate::gear::DYES.len() as u8 - 1;
            [r.u8()?.min(n), r.u8()?.min(n), r.u8()?.min(n)]
        },
        emote: r.u8()?.min(8),
        seat: r.u16()?,
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
        Ev::Hit { victim, dmg, knock, stun, head, w: wp, o, dir, guard, vt, spec } => {
            w.u8(2);
            w.u8(*victim);
            w.f(*dmg);
            w.v(*knock);
            w.f(*stun);
            w.u8(*head as u8);
            w.u8(*wp);
            w.v(*o);
            w.v(*dir);
            w.f(*guard);
            w.f(*vt);
            w.u8(*spec);
        }
        Ev::Eat { item } => {
            w.u8(16);
            w.u8(*item);
        }
        Ev::Deflect { by, dmg } => {
            w.u8(9);
            w.u8(*by);
            w.f(*dmg);
        }
        Ev::Grenade { p, v, kind } => {
            w.u8(3);
            w.v(*p);
            w.v(*v);
            w.u8(*kind);
        }
        Ev::Died { victim, killer, w: wp, head } => {
            w.u8(4);
            w.u8(*victim);
            w.u8(*killer);
            w.u8(*wp);
            w.u8(*head as u8);
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
        Ev::MapVote { map } => {
            w.u8(13);
            w.u8(*map);
        }
        Ev::Ability { who, kind, p, d } => {
            w.u8(11);
            w.u8(*who);
            w.u8(*kind);
            w.v(*p);
            w.v(*d);
        }
        Ev::Confirm { attacker, victim, hp, ap, head } => {
            w.u8(12);
            w.u8(*attacker);
            w.u8(*victim);
            w.f(*hp);
            w.f(*ap);
            w.u8(*head as u8);
        }
        Ev::ExecAsk { target, open } => {
            w.u8(10);
            w.u8(*target);
            w.u8(*open);
        }
        Ev::Ping { who, kind, p } => {
            w.u8(15);
            w.u8(*who);
            w.u8(*kind);
            w.v(*p);
        }
        Ev::Exec { who, target, kind } => {
            w.u8(14);
            w.u8(*who);
            w.u8(*target);
            w.u8(*kind);
        }
        Ev::Hub(h) => {
            let b = h.to_bytes();
            w.u8(17);
            w.u16(b.len().min(1000) as u16);
            w.0.extend_from_slice(&b[..b.len().min(1000)]);
        }
    }
}

fn read_ev(r: &mut R) -> Option<Ev> {
    Some(match r.u8()? {
        1 => Ev::Shot { o: r.v()?, e: r.v()?, w: r.u8()?, spark: r.u8()? },
        2 => Ev::Hit { victim: r.u8()?, dmg: r.f()?.clamp(0.0, LETHAL), knock: clamp_v(r.v()?, 30.0), stun: r.f()?.clamp(0.0, 2.0), head: r.u8()? == 1, w: r.u8()?, o: r.v()?, dir: r.v()?, guard: r.f()?.clamp(0.0, 100.0), vt: r.f()?, spec: r.u8()? },
        16 => Ev::Eat { item: r.u8()? },
        3 => Ev::Grenade { p: r.v()?, v: clamp_v(r.v()?, 70.0), kind: r.u8()?.min(4) },
        4 => Ev::Died { victim: r.u8()?, killer: r.u8()?, w: r.u8()?, head: r.u8()? == 1 },
        5 => {
            let n = r.u8()? as usize;
            Ev::Chat { text: String::from_utf8_lossy(r.take(n)?).chars().take(100).collect() }
        }
        6 => Ev::Item { kind: r.u8()?.min(1) },
        7 => Ev::Ready,
        8 => Ev::Vote,
        13 => Ev::MapVote { map: r.u8()?.min(crate::world::MapId::ALL.len() as u8 - 1) },
        9 => Ev::Deflect { by: r.u8()?, dmg: r.f()?.clamp(0.0, 60.0) },
        11 => Ev::Ability { who: r.u8()?.min(MAX_PLAYERS as u8 - 1), kind: r.u8()?.min(3), p: r.v()?, d: clamp_v(r.v()?, 40.0) },
        12 => Ev::Confirm { attacker: r.u8()?.min(MAX_PLAYERS as u8 - 1), victim: r.u8()?.min(MAX_PLAYERS as u8 - 1), hp: r.f()?.clamp(0.0, 999.0), ap: r.f()?.clamp(0.0, 999.0), head: r.u8()? != 0 },
        10 => Ev::ExecAsk { target: r.u8()?.min(MAX_PLAYERS as u8 - 1), open: r.u8()? & 0b1111 },
        15 => Ev::Ping { who: r.u8()?.min(MAX_PLAYERS as u8 - 1), kind: r.u8()?.min(crate::game::PINGS.len() as u8 - 1), p: clamp_v(r.v()?, 500.0) },
        14 => Ev::Exec { who: r.u8()?.min(MAX_PLAYERS as u8 - 1), target: r.u8()?.min(MAX_PLAYERS as u8 - 1), kind: r.u8()?.clamp(1, crate::exec::COUNT as u8) },
        17 => {
            let n = r.u16()? as usize;
            Ev::Hub(crate::hub::HubEv::from_bytes(r.take(n)?)?)
        }
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
    /// Their vote for the next map (255: none).
    pub map_vote: u8,
    /// Their ranked 1v1 rank (crate::ranked::Rank as a byte; 0 unranked) and hidden rating.
    pub rank: u8,
    pub rating: u16,
    pub rd: u16,
}

/// Everything the host tells a player each update.
pub struct HostMsg {
    pub you: usize,
    pub mode: Mode,
    /// A Classic lobby (no abilities).
    pub classic: bool,
    /// Which map (crate::world::MapId as a number).
    pub map: u8,
    pub phase: Phase,
    pub timer: f32,
    pub target: u16,
    pub team_score: [u16; 2],
    pub number: u32,
    pub round: u32,
    pub round_end: f32,
    pub duel: [u8; 2],
    pub flags: [Flag; 2],
    /// Ledger Duels: the challenger's (the host's) rules.
    pub rules: crate::gear::Rules,
    /// What the host thinks of you (it keeps everyone's health).
    pub you_hp: f32,
    pub you_ap: f32,
    pub you_alive: bool,
    pub roster: Vec<RosterEntry>,
    /// Whether the player list came with this update (in the hub it comes every third one).
    pub roster_full: bool,
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
    /// (host) someone came in to watch: a spectator, with no player's place (their slot is above
    /// the lobby's size and never fights)
    Watch { slot: usize, name: String },
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
    /// (player) the host can't play with this copy: its build number, and its network protocol
    /// (None from hosts made before the protocol number existed)
    WrongBuild { host: u32, proto: Option<u32> },
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

/// One player as the host sees them (for picking a new host).
#[derive(Clone, Debug, PartialEq)]
pub struct PeerInfo {
    pub slot: usize,
    pub ping: u16,
    pub addr: SocketAddr,
}

/// What a player knows to carry on if the host goes: the match's id, the host's slot, and every
/// player's address and connection.
#[derive(Clone, Debug, PartialEq)]
pub struct Succession {
    pub session: u32,
    pub host_slot: usize,
    pub peers: Vec<PeerInfo>,
}

impl Succession {
    /// Who takes over: the player with the best connection to the host (lowest ping, then the
    /// lowest slot), so every player picks the same one without asking.
    pub fn successor(&self) -> Option<usize> {
        self.peers.iter().filter(|p| p.slot != self.host_slot).min_by_key(|p| (p.ping, p.slot)).map(|p| p.slot)
    }
}

fn write_addr(w: &mut W, a: &SocketAddr) {
    match a {
        SocketAddr::V4(v) => {
            w.u8(4);
            w.0.extend_from_slice(&v.ip().octets());
        }
        SocketAddr::V6(v) => {
            w.u8(6);
            w.0.extend_from_slice(&v.ip().octets());
        }
    }
    w.u16(a.port());
}

fn read_addr(r: &mut R) -> Option<SocketAddr> {
    let ip: std::net::IpAddr = match r.u8()? {
        4 => {
            let b = r.take(4)?;
            std::net::Ipv4Addr::new(b[0], b[1], b[2], b[3]).into()
        }
        6 => {
            let b: [u8; 16] = r.take(16)?.try_into().ok()?;
            std::net::Ipv6Addr::from(b).into()
        }
        _ => return None,
    };
    Some(SocketAddr::new(ip, r.u16()?))
}

struct Peer {
    addr: SocketAddr,
    slot: usize,
    /// Only watching (a spectator), and their rank as they say (rank byte, rating, deviation).
    watch: bool,
    rank: (u8, u16, u16),
    last_recv: Instant,
    ping: f32,
    link: Link,
    seq_out: u32,
}

pub struct Net {
    sock: UdpSocket,
    pub is_host: bool,
    /// The build number this side says it is (this build's; tests set others).
    pub build: u32,
    /// The network protocol this side speaks (NET_PROTOCOL; tests set others).
    pub proto: u32,
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
    /// Steam (when running): players reached through Steam's relay by Steam ID.
    pub steam: Option<steamworks::Client>,
    counts: [usize; 4],
    rate_since: Instant,
    pub rates: Rates,
    /// (host) this match's id; players rejoining a new host show it.
    pub session: u32,
    /// (player) the latest list from the host, to carry on without it.
    pub succession: Option<Succession>,
    peers_acc: f32,
    /// (player) keep saying "I'm back" to a new host for this long.
    rejoin_left: f32,
    /// (player) the host already said goodbye (it says it several times).
    bye_seen: bool,
    /// (host) slots our bots are in: a joining player gets a free slot first, else a bot's.
    pub soft: Vec<usize>,
    /// (host) how many players this game takes (a match: 8; the hub: 16).
    pub cap: usize,
    roster_tick: u32,
    /// (host) addresses removed by an admin, kept out until then.
    banned: Vec<(SocketAddr, Instant)>,
    /// (player) join only to watch (a spectator), and the rank we tell the host.
    pub watch: bool,
    pub my_rank: (u8, u16, u16),
}

impl Net {
    /// The port this computer listens on (tests host on a free one).
    #[cfg(test)]
    pub fn local_port(&self) -> u16 {
        self.sock.local_addr().map(|a| a.port()).unwrap_or(0)
    }

    /// (host) The Steam ID a player is connected from (Steam vouches for it), if through Steam.
    pub fn peer_steam(&self, slot: usize) -> Option<u64> {
        self.peers.iter().find(|p| p.slot == slot).and_then(|p| addr_steam(&p.addr))
    }

    /// (player) The host's Steam ID, from our own connection to it.
    pub fn host_steam(&self) -> Option<u64> {
        self.host_addr.as_ref().and_then(addr_steam)
    }

    /// (host) Remove a player (an admin's ::kick): they're kept out for ten minutes.
    pub fn drop_slot(&mut self, slot: usize) {
        if let Some(i) = self.peers.iter().position(|p| p.slot == slot) {
            let p = self.peers.remove(i);
            self.banned.push((p.addr, Instant::now() + Duration::from_secs(600)));
            self.out.retain(|e| e.0 as usize != slot);
        }
    }

    pub fn host(port: u16) -> Result<Net, String> {
        let ip = std::env::var("DUEL_BIND").unwrap_or_else(|_| "0.0.0.0".to_string());
        let sock = UdpSocket::bind((ip.as_str(), port)).map_err(|e| format!("Couldn't open port {port}: {e}"))?;
        sock.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(Net::make(sock, true, None, port))
    }

    /// Join a friend's game through Steam (relayed: nobody needs to forward ports).
    pub fn join_steam(client: steamworks::Client, host: u64) -> Result<Net, String> {
        let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
        sock.set_nonblocking(true).map_err(|e| e.to_string())?;
        let mut n = Net::make(sock, false, Some(steam_addr(host)), DEFAULT_PORT);
        n.steam = Some(client);
        Ok(n)
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
            build: crate::update::local_build(),
            proto: crate::update::local_proto(),
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
            steam: None,
            counts: [0; 4],
            rate_since: Instant::now(),
            rates: Rates::default(),
            session: 0,
            succession: None,
            peers_acc: 0.0,
            rejoin_left: 0.0,
            bye_seen: false,
            soft: Vec::new(),
            cap: MATCH_PLAYERS,
            roster_tick: 0,
            banned: Vec::new(),
            watch: false,
            my_rank: (0, 1500, 350),
        }
        .seeded()
    }

    fn seeded(mut self) -> Net {
        // a match id nobody else will have
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        self.rng ^= t;
        self.session = ((self.rng >> 16) as u32) | 1;
        self
    }

    /// (player) The host is gone: if we have the list, the best-connected player takes over.
    /// When that's us, we start hosting on our own socket with everyone's last known address;
    /// otherwise we start talking to them. Returns (old host slot, new host slot).
    pub fn take_over(&mut self) -> Option<(usize, usize)> {
        if self.is_host {
            return None;
        }
        let s = self.succession.clone()?;
        let next = s.successor()?;
        if next == self.my_slot {
            self.is_host = true;
            self.host_addr = None;
            self.session = s.session;
            self.peers = s
                .peers
                .iter()
                .filter(|p| p.slot != self.my_slot && p.slot != s.host_slot)
                .map(|p| Peer { addr: p.addr, slot: p.slot, watch: false, rank: (0, 1500, 350), last_recv: Instant::now(), ping: p.ping as f32 / 1000.0, link: Link::new(), seq_out: 0 })
                .collect();
        } else {
            let addr = s.peers.iter().find(|p| p.slot == next)?.addr;
            self.host_addr = Some(addr);
            self.session = s.session;
            self.host_link = Link::new();
            self.rejoin_left = 5.0;
        }
        self.last_recv = Instant::now();
        self.succession = None;
        self.echo = 0.0;
        self.bye_seen = false;
        Some((s.host_slot, next))
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
            Some(a) if addr_steam(&a).is_some() => "the host through Steam (relayed)".into(),
            Some(a) => a.to_string(),
            None if self.steam.is_some() => format!("port {} and Steam", self.port),
            None => format!("port {}", self.port),
        }
    }

    /// Send raw bytes to someone: Steam players through Steam, everyone else by UDP.
    fn raw_send(&self, data: &[u8], to: SocketAddr) {
        if let Some(id) = addr_steam(&to) {
            if let Some(cl) = &self.steam {
                let who = steamworks::networking_types::NetworkingIdentity::new_steam_id(steamworks::SteamId::from_raw(id));
                let _ = cl.networking_messages().send_message_to_user(who, steamworks::networking_types::SendFlags::UNRELIABLE_NO_NAGLE, data, STEAM_GAME_CHANNEL);
            }
            return;
        }
        let _ = self.sock.send_to(data, to);
    }

    fn free_slot(&self) -> Option<usize> {
        // (after a takeover the host isn't in slot 0, and slot 0 may be free again)
        let open = |s: &usize| *s != self.my_slot && !self.peers.iter().any(|p| p.slot == *s);
        let cap = self.cap.clamp(2, MAX_PLAYERS);
        (0..cap).find(|s| open(s) && !self.soft.contains(s)).or_else(|| (0..cap).find(open))
    }

    /// (host) A place for a spectator: above the lobby's size, so they never take a player's.
    fn watch_slot(&self) -> Option<usize> {
        let open = |s: &usize| *s != self.my_slot && !self.peers.iter().any(|p| p.slot == *s);
        let cap = self.cap.clamp(2, MAX_PLAYERS);
        (cap..MAX_PLAYERS).rev().find(open)
    }

    /// (host) What a player told us of their rank (rank byte, rating, deviation).
    pub fn peer_rank(&self, slot: usize) -> Option<(u8, u16, u16)> {
        self.peers.iter().find(|p| p.slot == slot && !p.watch).map(|p| p.rank)
    }

    /// (host) How many are watching.
    pub fn watchers(&self) -> usize {
        self.peers.iter().filter(|p| p.watch).count()
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
    /// The clock snapshots are stamped with (seconds since this connection opened).
    pub fn clock(&self) -> f32 {
        self.start.elapsed().as_secs_f32()
    }

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
        self.raw_send(&data, to);
    }

    /// Send what the network test held back once its time is up.
    fn flush(&mut self) {
        let now = Instant::now();
        let mut i = 0;
        while i < self.out_q.len() {
            if self.out_q[i].0 <= now {
                let (_, d, to) = self.out_q.swap_remove(i);
                self.raw_send(&d, to);
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
        // and whatever came through Steam
        let steam_in: Vec<(Vec<u8>, SocketAddr)> = match &self.steam {
            Some(cl) => cl
                .networking_messages()
                .receive_messages_on_channel(STEAM_GAME_CHANNEL, 128)
                .into_iter()
                .filter_map(|m| {
                    let id = m.identity_peer().steam_id()?.raw();
                    Some((m.data().to_vec(), steam_addr(id)))
                })
                .collect(),
            None => Vec::new(),
        };
        for (data, from) in steam_in {
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
                self.raw_send(&reply, from);
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
                K_BYE => {
                    if !self.bye_seen {
                        self.bye_seen = true;
                        out.push(Incoming::HostLeft);
                    }
                }
                K_FULL => out.push(Incoming::Full),
                K_BUILD => {
                    if let Some(host) = r.u32() {
                        out.push(Incoming::WrongBuild { host, proto: r.u32() });
                    }
                }
                K_PEERS => {
                    if let Some(s) = parse_peers(&mut r) {
                        self.succession = Some(s);
                    }
                }
                _ => {}
            }
        }
    }

    fn host_packet(&mut self, kind: u8, r: &mut R, from: SocketAddr, seq: u32, sent: f32, out: &mut Vec<Incoming>) {
        let idx = self.peers.iter().position(|p| p.addr == from);
        match kind {
            K_HELLO => {
                // (removed by an admin: kept out for a while)
                self.banned.retain(|b| b.1 > Instant::now());
                if self.banned.iter().any(|b| b.0 == from) {
                    let w = self.head(K_FULL, from);
                    self.send_to(w.0, from);
                    return;
                }
                let name = r.s().unwrap_or_default();
                // copies that speak different network protocols can't play together (players from
                // before the protocol number: different builds can't): say which we are instead
                let build = r.u32().unwrap_or(0);
                let proto = r.u32();
                // (protocol 16) watching only, and their rank
                let watch = r.u8().unwrap_or(0) == 1;
                let rank = (r.u8().unwrap_or(0), r.u16().unwrap_or(1500), r.u16().unwrap_or(350));
                if !crate::update::can_play_together(self.build, self.proto, build, proto) {
                    let mut w = self.head(K_BUILD, from);
                    w.u32(self.build);
                    w.u32(self.proto);
                    self.send_to(w.0, from);
                    return;
                }
                if let Some(i) = idx {
                    self.peers[i].last_recv = Instant::now();
                    self.peers[i].link.see(seq);
                    self.peers[i].rank = rank;
                    return;
                }
                let slot = if watch { self.watch_slot() } else { self.free_slot() };
                let Some(slot) = slot else {
                    let w = self.head(K_FULL, from);
                    self.send_to(w.0, from);
                    return;
                };
                let mut link = Link::new();
                link.see(seq);
                self.peers.push(Peer { addr: from, slot, watch, rank, last_recv: Instant::now(), ping: 0.05, link, seq_out: 0 });
                self.last_in[slot] = 0;
                self.out.retain(|e| e.0 as usize != slot);
                out.push(if watch { Incoming::Watch { slot, name } } else { Incoming::Join { slot, name } });
            }
            K_STATE => {
                let Some(i) = idx else { return };
                self.peers[i].last_recv = Instant::now();
                let fresh = self.peers[i].link.see(seq);
                let slot = self.peers[i].slot;
                // (a spectator's own fighter means nothing: only their clock matters)
                let watching = self.peers[i].watch;
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
                if watching {
                    return;
                }
                let mut fresh_evs = Vec::new();
                for (id, ev) in pairs {
                    if id > self.last_in[slot] {
                        self.last_in[slot] = id;
                        // pass on what others need to see; hit claims stay here until checked
                        let relay = match &ev {
                            Ev::Shot { .. } | Ev::Grenade { .. } | Ev::Chat { .. } => true,
                            // a player's look and emote go straight on (named as theirs)
                            Ev::Hub(h) => h.relayed(),
                            _ => false,
                        };
                        if relay {
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
            K_REJOIN => {
                let (Some(session), Some(slot), Some(name)) = (r.u32(), r.u8(), r.s()) else { return };
                let slot = slot as usize;
                if session != self.session || slot >= MAX_PLAYERS || slot == self.my_slot {
                    return;
                }
                match self.peers.iter_mut().find(|p| p.slot == slot) {
                    // their address as we see it (it may differ from what the old host saw)
                    Some(p) => {
                        p.addr = from;
                        p.last_recv = Instant::now();
                    }
                    None => {
                        self.peers.retain(|p| p.addr != from);
                        let mut link = Link::new();
                        link.see(seq);
                        self.peers.push(Peer { addr: from, slot, watch: false, rank: (0, 1500, 350), last_recv: Instant::now(), ping: 0.05, link, seq_out: 0 });
                        out.push(Incoming::Join { slot, name });
                    }
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
        let mode_byte = r.u8()?;
        let classic = mode_byte & 0x80 != 0;
        let mode = Mode::from_u8(mode_byte & 0x7f);
        let map = r.u8()?.min(crate::world::MapId::Varrow as u8);
        let phase = Phase::from_u8(r.u8()?);
        let timer = r.f()?;
        let target = r.u16()?;
        let team_score = [r.u16()?, r.u16()?];
        let number = r.u32()?;
        let round = r.u32()?;
        let round_end = r.f()?;
        let duel = [r.u8()?, r.u8()?];
        let rules = {
            let bits = r.u8()?;
            crate::gear::Rules::from_bits(bits, r.u8()?)
        };
        let roster_full = r.u8()? != 0;
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
            let map_vote = r.u8()?;
            let rank = r.u8()?;
            let (rating, rd) = if mode == Mode::Hub { (1500, 350) } else { (r.u16()?, r.u16()?) };
            roster.push(RosterEntry { slot, name, team, kills, deaths, ping, loss, ready: bits & 1 != 0, vote: bits & 2 != 0, spectate: bits & 4 != 0, wins, map_vote, rank, rating, rd });
        }
        if !stale && roster_full {
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
            let st = if mode == Mode::Hub { read_fstate_hub(r)? } else { read_fstate(r)? };
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
        Some(HostMsg { you, mode, classic, map, phase, timer, target, team_score, number, round, round_end, duel, flags, rules, you_hp, you_ap, you_alive, roster, roster_full, fighters, events, sent, stale })
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
            w.u32(self.build);
            w.u32(self.proto);
            w.u8(self.watch as u8);
            w.u8(self.my_rank.0);
            w.u16(self.my_rank.1);
            w.u16(self.my_rank.2);
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
            // once a second, everyone gets the list they'd need to carry on without us
            self.peers_acc += 1.0 / 60.0;
            if self.peers_acc >= 1.0 && !self.peers.is_empty() {
                self.peers_acc = 0.0;
                let list: Vec<PeerInfo> = self.peers.iter().map(|p| PeerInfo { slot: p.slot, ping: (p.ping * 1000.0).round().min(9999.0) as u16, addr: p.addr }).collect();
                let to: Vec<SocketAddr> = self.peers.iter().map(|p| p.addr).collect();
                for a in to {
                    let mut w = self.head(K_PEERS, a);
                    w.u32(self.session);
                    w.u8(self.my_slot as u8);
                    w.u8(list.len() as u8);
                    for p in &list {
                        w.u8(p.slot as u8);
                        w.u16(p.ping);
                        write_addr(&mut w, &p.addr);
                    }
                    self.send_to(w.0, a);
                }
            }
            let targets: Vec<(SocketAddr, usize)> = self.peers.iter().map(|p| (p.addr, p.slot)).collect();
            let mut packets = Vec::new();
            for (addr, slot) in targets {
                let mut w = self.head(K_HOST, addr);
                w.u8(slot as u8);
                let you = &g.f[slot];
                w.f(you.hp);
                w.f(you.ap);
                w.u8(you.alive as u8);
                w.u8(g.mt.mode as u8 | if g.mt.classic { 0x80 } else { 0 });
                w.u8(g.world.map as u8);
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
                w.u8(g.mt.rules.bits());
                w.u8(g.mt.rules.best_of);
                // (the hub: the player list goes in every third update, to leave room for 16)
                let hub = g.mt.mode == Mode::Hub;
                let with_roster = !hub || self.roster_tick % 3 == 0;
                w.u8(with_roster as u8);
                for fl in &g.mt.flags {
                    w.u8(fl.state);
                    w.u8(fl.carrier);
                    w.v(fl.pos);
                    w.f(fl.timer);
                }
                let present = g.present();
                let listed: &[usize] = if with_roster { &present } else { &[] };
                w.u8(listed.len() as u8);
                for &s in listed {
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
                    w.u8(f.map_vote);
                    // (the hub shows only the badge; a match needs the rating for ranked)
                    w.u8(f.rank);
                    if !hub {
                        w.u16(f.rating);
                        w.u16(f.rating_rd);
                    }
                }
                let others: Vec<usize> = present.iter().copied().filter(|&s| s != slot).collect();
                w.u8(others.len() as u8);
                let total = others.len();
                for (k, s) in others.into_iter().enumerate() {
                    w.u8(s as u8);
                    if hub {
                        // in full when it fits with a short one for everyone still to come
                        let room = w.0.len() + 110 + (total - k - 1) * 38 + 8 <= MAX_PACKET;
                        write_fstate_hub(&mut w, &g.f[s], room);
                    } else {
                        write_fstate(&mut w, &g.f[s]);
                    }
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
            self.roster_tick = self.roster_tick.wrapping_add(1);
        } else if let Some(h) = self.host_addr {
            if self.rejoin_left > 0.0 {
                // a new host: tell them who we are (several times: the first may not get through
                // until their side has sent us something)
                self.rejoin_left -= 1.0 / 60.0;
                let mut w = self.head(K_REJOIN, h);
                w.u32(self.session);
                w.u8(self.my_slot as u8);
                w.s(&self.roster_names[self.my_slot.min(MAX_PLAYERS - 1)].clone());
                self.send_to(w.0, h);
            }
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
fn parse_peers(r: &mut R) -> Option<Succession> {
    let session = r.u32()?;
    let host_slot = r.u8()? as usize;
    let n = r.u8()? as usize;
    let mut peers = Vec::new();
    for _ in 0..n.min(MAX_PLAYERS) {
        let slot = (r.u8()? as usize).min(MAX_PLAYERS - 1);
        let ping = r.u16()?;
        let addr = read_addr(r)?;
        peers.push(PeerInfo { slot, ping, addr });
    }
    Some(Succession { session, host_slot: host_slot.min(MAX_PLAYERS - 1), peers })
}

/// The host is gone: carry the match on. The best-connected player becomes the host (their game
/// turns into the host's, with everyone's scores, health and the round as they were; the old
/// host's bots go with it), the others reconnect to them. False when there's nobody to take
/// over (then the match is over for us).
pub fn migrate(g: &mut Game, n: &mut Net) -> bool {
    let humans: Vec<usize> = n.succession.as_ref().map(|s| s.peers.iter().map(|p| p.slot).collect()).unwrap_or_default();
    let Some((old, next)) = n.take_over() else { return false };
    if next == g.local {
        g.become_host(old, &humans);
    } else {
        g.follow_new_host(old, next);
    }
    true
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
    // the hub: their emote and seat as they say
    if s.emote != f.hub.emote {
        f.hub.emote = s.emote;
        f.hub.emote_t = now;
    }
    f.hub.seat = s.seat;
    let lo = Loadout::from_bytes(s.loadout);
    if lo != f.loadout {
        f.loadout = lo;
        f.items = f.items.min(2);
    }
    // Ledger Duels: their gear as they say it is (what's worn, their bar, what they're doing)
    if s.gear.iter().any(|&g| g != 0) {
        match f.kit.as_mut() {
            Some(k) if k.remote => {
                k.set_remote(s.gear, s.spec, s.gbits);
                k.dyes = s.dyes;
            }
            _ => {
                let mut k = crate::gear::Kit::remote(s.gear, s.spec, s.gbits);
                k.dyes = s.dyes;
                f.kit = Some(Box::new(k));
            }
        }
    } else if f.kit.as_ref().is_some_and(|k| k.remote) {
        f.kit = None;
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
        // (a duellist sent short in the hub says nothing about health)
        if s.hp >= 0.0 {
            f.hp = s.hp;
            f.ap = s.ap;
        }
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
    f.scope_t = if s.flags2 & 8 != 0 { f.scope_t.max(SCOPE_IN) } else { 0.0 };
    // the kick plays out here from its start; the rest are on/off
    if s.flags3 & 1 != 0 {
        if f.kick_t <= 0.0 {
            f.kick_t = crate::game::KICK_TIME;
        }
    } else {
        f.kick_t = 0.0;
    }
    f.charged = s.flags3 & 2 != 0;
    f.charge_t = if s.flags3 & 4 != 0 { f.charge_t.max(0.25) } else { 0.0 };
    f.through_t = if s.flags3 & 8 != 0 { 0.1 } else { 0.0 };
    f.slam = s.flags3 & 16 != 0;
    // abilities: the shield and invisibility as reported, the grapple line until it lets go
    f.shield_t = if s.flags4 & 1 != 0 { f.shield_t.max(0.15) } else { 0.0 };
    f.invis_t = if s.flags4 & 2 != 0 { f.invis_t.max(0.15) } else { 0.0 };
    f.lock_t = if s.flags4 & 8 != 0 { f.lock_t.max(0.05) } else { 0.0 };
    f.nade_wind = if s.flags4 & 16 != 0 { f.nade_wind.max(0.05) } else { 0.0 };
    f.ab_charge = s.ab as f32 / 100.0;
    if s.flags4 & 4 == 0 {
        f.grapple = None;
    }
    // a finisher plays out here from its start
    if s.fin != f.fin {
        f.fin = s.fin;
        f.fin_t = 0.0;
    }
    // a throw plays out here from its start
    if s.flags3 & 32 != 0 {
        if f.throw_t <= 0.0 {
            f.throw_fan = s.flags3 & 64 != 0;
            f.throw_t = if f.throw_fan { crate::game::FAN_THROW_TIME } else { crate::game::THROW_TIME };
        }
    }
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

    /// A full lobby in the middle of a firefight still sends packets that fit the internet's size:
    /// a match of 8, and a hub of 16 (with two duels on in the pits, sent in full).
    #[test]
    fn host_packets_fit_under_the_limit() {
        for (mode, players) in [(Mode::Ctf, MATCH_PLAYERS), (Mode::Hub, MAX_PLAYERS)] {
            host_packets_fit(mode, players);
        }
    }

    fn host_packets_fit(mode: Mode, players: usize) {
        let mut g = Game::new(Kind::Host, 0, mode, 5);
        for s in 0..players {
            g.add_player(s, "LongPlayerName16", s == 0, None, (s % 2) as u8);
        }
        if mode == Mode::Hub {
            for s in 1..5 {
                g.f[s].hub.pit = (s / 3) as u8;
            }
        }
        let mut n = Net::host(0).expect("socket");
        n.cap = players;
        // hold everything back so it can be measured
        n.sim = Some(NetSim { lag_ms: 600_000.0, loss: 0.0 });
        for s in 1..players {
            let addr: SocketAddr = format!("127.0.0.1:{}", 40000 + s).parse().unwrap();
            n.peers.push(Peer { addr, slot: s, watch: false, rank: (0, 1500, 350), last_recv: Instant::now(), ping: 0.05, link: Link::new(), seq_out: 0 });
        }
        for k in 0..60u32 {
            let o = v3(k as f32, 1.0, 0.0);
            n.out.push_back(((k % 8) as u8, k + 1, Instant::now(), Ev::Shot { o, e: o + v3(0.0, 0.0, 20.0), w: 3, spark: 1 }));
            n.out.push_back(((k % 8) as u8, k + 1000, Instant::now(), Ev::Chat { text: "a fairly long chat line to fill the packet up".into() }));
        }
        // (three updates: the hub sends its player list in every third)
        for _ in 0..3 {
            n.send(&g, 1.0);
        }
        assert_eq!(n.out_q.len(), (players - 1) * 3, "one update per player each time");
        for (_, data, _) in &n.out_q {
            assert!(data.len() <= MAX_PACKET, "{mode:?}: packet of {} bytes", data.len());
            assert!(data.len() > 600, "{mode:?}: still carries the match ({} bytes)", data.len());
        }
    }
}

#[cfg(test)]
mod steam_addr_tests {
    use super::*;

    #[test]
    fn steam_ids_round_trip_through_addresses() {
        for id in [76561197960287930u64, 1, u64::MAX, 0x0110_0001_0000_0000] {
            let a = steam_addr(id);
            assert_eq!(addr_steam(&a), Some(id));
        }
        // real addresses aren't mistaken for Steam players
        assert_eq!(addr_steam(&"192.168.1.20:27960".parse().unwrap()), None);
        assert_eq!(addr_steam(&"[::1]:1".parse().unwrap()), None);
    }
}

#[cfg(test)]
mod melee_net_tests {
    use super::*;

    fn round_trip(ev: Ev) -> Ev {
        let mut w = W(Vec::new());
        write_ev(&mut w, &ev);
        let mut r = R { b: &w.0, i: 0 };
        read_ev(&mut r).expect("decodes")
    }

    #[test]
    fn guard_drain_and_deflects_cross_the_network() {
        match round_trip(Ev::Hit { victim: 3, dmg: 0.0, knock: v3(1.0, 0.0, 0.0), stun: 0.5, head: false, w: crate::game::KICK_TOOL, o: V3::ZERO, dir: V3::ZERO, guard: 31.0, vt: 0.0 , spec: 0 }) {
            Ev::Hit { victim, guard, w, stun, .. } => assert!(victim == 3 && (guard - 31.0).abs() < 1e-3 && w == crate::game::KICK_TOOL && (stun - 0.5).abs() < 1e-3),
            e => panic!("{e:?}"),
        }
        match round_trip(Ev::Deflect { by: 2, dmg: 18.0 }) {
            Ev::Deflect { by, dmg } => assert!(by == 2 && (dmg - 18.0).abs() < 1e-3),
            e => panic!("{e:?}"),
        }
        // the kick, the charge and the plunge show on other screens
        let mut f = Fighter::new("x", 1, true);
        f.alive = true;
        f.kick_t = 0.2;
        f.charged = true;
        f.charge_t = 0.6;
        f.slam = true;
        let mut w = W(Vec::new());
        write_fstate(&mut w, &f);
        let s = read_fstate(&mut R { b: &w.0, i: 0 }).expect("decodes");
        assert_eq!(s.flags3 & (1 | 2 | 4 | 16), 1 | 2 | 4 | 16);
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    use crate::game::{Kind, Mode, Phase};

    #[test]
    fn successor_is_the_best_connected_player() {
        let a = |p: u16| SocketAddr::from(([127, 0, 0, 1], p));
        let s = Succession { session: 7, host_slot: 0, peers: vec![PeerInfo { slot: 1, ping: 80, addr: a(1) }, PeerInfo { slot: 2, ping: 25, addr: a(2) }, PeerInfo { slot: 3, ping: 25, addr: a(3) }] };
        assert_eq!(s.successor(), Some(2), "lowest ping, then lowest slot");
        // the list survives the trip
        let mut w = W(Vec::new());
        w.u32(s.session);
        w.u8(0);
        w.u8(3);
        for p in &s.peers {
            w.u8(p.slot as u8);
            w.u16(p.ping);
            write_addr(&mut w, &p.addr);
        }
        assert_eq!(parse_peers(&mut R { b: &w.0, i: 0 }), Some(s));
    }

    /// A player whose game speaks another network protocol is told the host's build instead of
    /// joining; the same build joins.
    #[test]
    fn different_builds_are_told_instead_of_joining() {
        let mut h = Net::host(0).expect("host");
        h.build = 57;
        let port = h.sock.local_addr().unwrap().port();
        let mut c = Net::join(&format!("127.0.0.1:{port}")).unwrap();
        c.build = 58;
        c.proto = h.proto + 1;
        let (mut joined, mut told) = (false, None);
        for _ in 0..200 {
            c.send_hello("Bo", 0.3);
            for m in h.poll() {
                joined |= matches!(m, Incoming::Join { .. });
            }
            for m in c.poll() {
                if let Incoming::WrongBuild { host, .. } = m {
                    told = Some(host);
                }
            }
            if told.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        assert_eq!(told, Some(57), "the player hears the host's build");
        assert!(!joined, "and doesn't join");
        // the same build joins
        let mut c2 = Net::join(&format!("127.0.0.1:{port}")).unwrap();
        c2.build = 57;
        for _ in 0..200 {
            c2.send_hello("Ann", 0.3);
            for m in h.poll() {
                joined |= matches!(m, Incoming::Join { .. });
            }
            if joined {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        assert!(joined, "the same build joins");
    }

    /// The join check goes by the network protocol: a player a few builds behind (the Mac zip
    /// that GitHub couldn't rebuild) joins when the protocol is the same; a different protocol is
    /// told the host's build and protocol instead; a player from before the protocol number (its
    /// hello has no protocol) must be the same build.
    #[test]
    fn the_protocol_not_the_build_decides_who_joins() {
        let mut h = Net::host(0).expect("host");
        h.build = 23;
        h.proto = 11;
        let port = h.sock.local_addr().unwrap().port();
        let try_join = |h: &mut Net, c: &mut Net| -> (bool, Option<(u32, Option<u32>)>) {
            let (mut joined, mut told) = (false, None);
            for _ in 0..200 {
                c.send_hello("Mac", 0.3);
                for m in h.poll() {
                    joined |= matches!(m, Incoming::Join { .. });
                }
                for m in c.poll() {
                    if let Incoming::WrongBuild { host, proto } = m {
                        told = Some((host, proto));
                    }
                }
                if joined || told.is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(3));
            }
            (joined, told)
        };
        // an older build that speaks the same protocol joins
        let mut mac = Net::join(&format!("127.0.0.1:{port}")).unwrap();
        mac.build = 21;
        mac.proto = 11;
        let (joined, told) = try_join(&mut h, &mut mac);
        assert!(joined && told.is_none(), "same protocol, older build: joins");
        // a different protocol is told the host's build and protocol
        let mut old = Net::join(&format!("127.0.0.1:{port}")).unwrap();
        old.build = 22;
        old.proto = 10;
        let (joined, told) = try_join(&mut h, &mut old);
        assert!(!joined, "a different protocol doesn't join");
        assert_eq!(told, Some((23, Some(11))));
        // a hello from before the protocol number (name and build only), on another build
        let pre = Net::join(&format!("127.0.0.1:{port}")).unwrap();
        let mut told_old = false;
        let mut joined_old = false;
        for _ in 0..200 {
            let mut w = W(Vec::new());
            w.0.extend_from_slice(&MAGIC);
            w.u8(K_HELLO);
            w.u32(1);
            w.f(0.0);
            w.s("Old");
            w.u32(18);
            pre.sock.send_to(&w.0, format!("127.0.0.1:{port}")).unwrap();
            for m in h.poll() {
                joined_old |= matches!(m, Incoming::Join { .. });
            }
            let mut buf = [0u8; 64];
            pre.sock.set_nonblocking(true).ok();
            while let Ok((n, _)) = pre.sock.recv_from(&mut buf) {
                told_old |= n > 4 && buf[4] == K_BUILD;
            }
            if told_old {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        assert!(told_old && !joined_old, "an old build #18 without a protocol is turned away");
    }

    /// The protocol's text (Steam lobbies) and number agree.
    #[test]
    fn the_protocol_text_and_number_agree() {
        assert_eq!(PROTOCOL, format!("DRA{NET_PROTOCOL}"));
    }

    /// A fingerprint of what both sides of a match must agree on: how a fighter's state and an
    /// event are written, every map's layout and the game's tuning. When it changes, builds with
    /// the same NET_PROTOCOL would no longer agree: bump NET_PROTOCOL (and PROTOCOL), then
    /// record the new fingerprint here, for that protocol.
    #[test]
    fn match_fingerprint() {
        let mut bytes = Vec::new();
        let mut w = W(Vec::new());
        write_fstate(&mut w, &Fighter::new("x", 0, true));
        write_ev(&mut w, &Ev::Ping { who: 1, kind: 2, p: v3(1.0, 2.0, 3.0) });
        bytes.extend_from_slice(&w.0);
        // (to the millimetre, so the maths library of each system can't change it)
        let mm = |x: f32| (x * 1000.0).round() as i64;
        for m in crate::world::MapId::ALL {
            for b in crate::world::World::build(m).blocks {
                let (a, c) = (b.b.min, b.b.max);
                bytes.extend_from_slice(format!("{} {} {} {} {} {} {:?};", mm(a.x), mm(a.y), mm(a.z), mm(c.x), mm(c.y), mm(c.z), b.mat).as_bytes());
            }
        }
        bytes.extend_from_slice(format!("{:?}", Tuning::default()).as_bytes());
        // FNV-1a
        let mut hsh: u64 = 0xcbf2_9ce4_8422_2325;
        for b in bytes {
            hsh = (hsh ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
        let recorded: (u32, u64) = (16, 10911351392787391105);
        assert_eq!((NET_PROTOCOL, hsh), recorded, "what both sides of a match must agree on changed: bump NET_PROTOCOL and PROTOCOL, then record the new value here");
    }

    /// A host and two players on this computer; the host leaves mid-match and the game goes on.
    #[test]
    fn the_match_survives_the_host_leaving() {
        let h = Net::host(0).expect("host");
        let port = h.sock.local_addr().unwrap().port();
        let mut hg = Game::new(Kind::Host, 0, Mode::Ffa, 50);
        hg.add_player(0, "Host", true, None, 0);
        hg.mt.phase = Phase::Fight;
        hg.mt.timer = 300.0;
        let mut nets = [Net::join(&format!("127.0.0.1:{port}")).unwrap(), Net::join(&format!("127.0.0.1:{port}")).unwrap()];
        let names = ["Ann", "Bo"];
        let mut games: [Option<Game>; 2] = [None, None];
        let dt = 1.0 / 60.0;
        let step = |h: Option<&mut (Net, Game)>, nets: &mut [Net; 2], games: &mut [Option<Game>; 2], hosts_seen: &mut [u32; 2]| {
            if let Some((hn, hg)) = h {
                for m in hn.poll() {
                    match m {
                        Incoming::Join { slot, name } => hg.add_player(slot, &name, false, None, 0),
                        Incoming::PeerState { slot, st, sent, .. } if hg.f[slot].present => {
                            let now = hg.time;
                            apply_remote(&mut hg.f[slot], &st, true, sent, now);
                        }
                        _ => {}
                    }
                }
                hn.send(hg, dt);
            }
            for k in 0..2 {
                let n = &mut nets[k];
                if games[k].is_none() {
                    n.send_hello(names[k], dt);
                }
                for m in n.poll() {
                    match m {
                        Incoming::Host(msg) => {
                            hosts_seen[k] += 1;
                            let g = games[k].get_or_insert_with(|| Game::new(Kind::Client, msg.you, msg.mode, msg.target));
                            crate::apply_host_msg(g, msg);
                        }
                        Incoming::HostLeft => {
                            if let Some(g) = games[k].as_mut() {
                                assert!(migrate(g, n), "someone takes over");
                            }
                        }
                        Incoming::Join { slot, name } => {
                            if let Some(g) = games[k].as_mut() {
                                if !g.f[slot].present {
                                    g.add_player(slot, &name, false, None, 0);
                                }
                            }
                        }
                        Incoming::PeerState { slot, st, sent, .. } => {
                            if let Some(g) = games[k].as_mut() {
                                if g.f[slot].present {
                                    let now = g.time;
                                    apply_remote(&mut g.f[slot], &st, true, sent, now);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(g) = games[k].as_ref() {
                    n.send(g, dt);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(3));
        };
        let mut seen = [0u32; 2];
        let mut host = Some((h, hg));
        for frame in 0..200 {
            if frame == 60 {
                // the host keeps score: give the players something to keep
                let (_, hg) = host.as_mut().unwrap();
                for s in 1..3 {
                    if hg.f[s].present {
                        hg.f[s].kills = s as i32 * 2;
                        hg.f[s].hp = 40.0 + s as f32;
                    }
                }
            }
            step(host.as_mut(), &mut nets, &mut games, &mut seen);
        }
        assert!(games.iter().all(|g| g.is_some()), "both players joined");
        assert!(nets.iter().all(|n| n.succession.is_some()), "both know who would take over");
        // the host leaves
        let (mut hn, _) = host.take().unwrap();
        hn.send_bye();
        drop(hn);
        let before = seen;
        for _ in 0..150 {
            step(None, &mut nets, &mut games, &mut seen);
        }
        let hosts: Vec<usize> = (0..2).filter(|&k| nets[k].is_host).collect();
        assert_eq!(hosts.len(), 1, "exactly one player hosts now");
        let (nh, other) = (hosts[0], 1 - hosts[0]);
        assert_eq!(games[nh].as_ref().unwrap().kind, Kind::Host);
        assert!(seen[other] > before[other] + 20, "the other player hears from the new host: {} -> {}", before[other], seen[other]);
        for k in 0..2 {
            let g = games[k].as_ref().unwrap();
            assert!(!g.f[0].present, "the old host is gone");
            assert_eq!(g.banner.as_ref().map(|b| b.0.as_str()), Some("Host has been usurped"));
            for s in 1..3 {
                assert_eq!(g.f[s].kills, s as i32 * 2, "scores kept (slot {s} on player {k})");
            }
        }
        // health carries on from where the old host left it
        let g = games[nh].as_ref().unwrap();
        for s in 1..3 {
            assert!((g.f[s].hp - (40.0 + s as f32)).abs() < 0.01, "health kept: {}", g.f[s].hp);
        }
    }
}

#[cfg(test)]
mod bot_slot_tests {
    use super::*;

    #[test]
    fn players_get_a_free_slot_before_a_bots() {
        let mut n = Net::host(0).expect("host");
        n.soft = vec![1, 2, 3];
        assert_eq!(n.free_slot(), Some(4), "a free slot first");
        // everything else taken: a bot's slot it is
        n.soft = (1..MAX_PLAYERS).collect();
        assert_eq!(n.free_slot(), Some(1));
    }
}

#[cfg(test)]
mod finisher_net_tests {
    use super::*;

    #[test]
    fn finishers_cross_the_network() {
        let mut w = W(Vec::new());
        write_ev(&mut w, &Ev::Ability { who: 2, kind: 1, p: crate::math::v3(1.0, 2.0, 3.0), d: crate::math::v3(0.0, 0.0, 99.0) });
        match read_ev(&mut R { b: &w.0, i: 0 }) {
            Some(Ev::Ability { who: 2, kind: 1, p, d }) => assert!(p.z == 3.0 && d.z <= 40.0, "an ability crosses the network (its throw clamped)"),
            other => panic!("{other:?}"),
        }
        // an execution: asked for by a player, decided by the host
        let mut w = W(Vec::new());
        write_ev(&mut w, &Ev::ExecAsk { target: 3, open: 0b1011 });
        match read_ev(&mut R { b: &w.0, i: 0 }) {
            Some(Ev::ExecAsk { target: 3, open: 0b1011 }) => {}
            e => panic!("{e:?}"),
        }
        let mut w = W(Vec::new());
        write_ev(&mut w, &Ev::Exec { who: 2, target: 3, kind: 14 });
        match read_ev(&mut R { b: &w.0, i: 0 }) {
            Some(Ev::Exec { who: 2, target: 3, kind: 14 }) => {}
            e => panic!("{e:?}"),
        }
        // nonsense is clamped: no execution past the last one, no slot past the last
        let mut w = W(Vec::new());
        write_ev(&mut w, &Ev::Exec { who: 200, target: 9, kind: 99 });
        match read_ev(&mut R { b: &w.0, i: 0 }) {
            Some(Ev::Exec { who, target, kind }) => assert!((who as usize) < MAX_PLAYERS && (target as usize) < MAX_PLAYERS && kind as usize == crate::exec::COUNT),
            e => panic!("{e:?}"),
        }
        let mut f = Fighter::new("x", 1, true);
        f.alive = true;
        f.fin = 7;
        let mut w = W(Vec::new());
        write_fstate(&mut w, &f);
        assert_eq!(read_fstate(&mut R { b: &w.0, i: 0 }).unwrap().fin, 7);
    }
}
#[cfg(test)]
mod spectator_tests {
    use super::*;

    /// A spectator joins a 1v1 (two players' places) with no player's place of their own: they
    /// get a slot above the lobby's size, never become a fighter on the host, see both duellists
    /// where the host has them (with their ranks), and what they send about themselves is ignored.
    #[test]
    fn spectators_watch_in_sync_without_a_slot() {
        let mut hn = Net::host(0).expect("host");
        hn.cap = 2;
        let port = hn.sock.local_addr().unwrap().port();
        let mut hg = Game::new(Kind::Host, 0, Mode::Duel, 3);
        hg.add_player(0, "Host", true, None, 0);
        hg.f[0].rank = 13;
        hg.mt.phase = Phase::Fight;
        hg.mt.timer = 300.0;
        let mut pn = Net::join(&format!("127.0.0.1:{port}")).unwrap();
        pn.my_rank = (10, 1480, 90);
        let mut sn = Net::join(&format!("127.0.0.1:{port}")).unwrap();
        sn.watch = true;
        let (mut pg, mut sg): (Option<Game>, Option<Game>) = (None, None);
        let mut watch_slot = None;
        let mut spec_states = 0;
        let dt = 1.0 / 60.0;
        for frame in 0..240 {
            // the host moves its fighter about; the player stands still
            hg.f[0].pos = v3((frame as f32 * 0.05).sin() * 4.0, 0.0, 2.0);
            for m in hn.poll() {
                match m {
                    Incoming::Join { slot, name } => {
                        hg.add_player(slot, &name, false, None, 0);
                    }
                    Incoming::Watch { slot, .. } => watch_slot = Some(slot),
                    Incoming::PeerState { slot, st, sent, .. } => {
                        if Some(slot) == watch_slot {
                            spec_states += 1;
                        }
                        if hg.f[slot].present {
                            let now = hg.time;
                            apply_remote(&mut hg.f[slot], &st, true, sent, now);
                        }
                    }
                    _ => {}
                }
            }
            for s in 0..hg.f.len() {
                if let Some(r) = hn.peer_rank(s) {
                    hg.f[s].rank = r.0;
                    hg.f[s].rating = r.1;
                }
            }
            hn.send(&hg, dt);
            for (n, g, name) in [(&mut pn, &mut pg, "Pat"), (&mut sn, &mut sg, "Viewer")] {
                if g.is_none() {
                    n.send_hello(name, dt);
                }
                for m in n.poll() {
                    if let Incoming::Host(msg) = m {
                        let gg = g.get_or_insert_with(|| Game::new(Kind::Client, msg.you, msg.mode, msg.target));
                        crate::apply_host_msg(gg, msg);
                    }
                }
                if let Some(gg) = g.as_ref() {
                    n.send(gg, dt);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        let ws = watch_slot.expect("the spectator was let in to watch");
        assert!(ws >= 2, "a spectator never takes a player's place (slot {ws})");
        assert!(!hg.f[ws].present, "they're no fighter on the host");
        assert_eq!(hg.present().len(), 2, "the duel is still one against one");
        assert_eq!(spec_states, 0, "what a spectator sends about themselves is ignored");
        assert_eq!(hn.watchers(), 1);
        let sg = sg.expect("the spectator follows the match");
        assert_eq!(sg.local, ws);
        assert!(!sg.f[ws].present, "watching, with no fighter of their own");
        let players: Vec<usize> = (0..sg.f.len()).filter(|&s| sg.f[s].present).collect();
        assert_eq!(players.len(), 2, "both duellists are seen: {players:?}");
        assert!((sg.f[0].net_pos - hg.f[0].pos).len() < 0.6, "in sync with the host: {:?} vs {:?}", sg.f[0].net_pos, hg.f[0].pos);
        let p = players.into_iter().find(|&s| s != 0).unwrap();
        assert_eq!(sg.f[p].rank, 10, "the player's rank reaches the spectator");
        assert_eq!(sg.f[0].rank, 13, "and the host's");
        assert_eq!(sg.mt.mode, Mode::Duel);
        let pg = pg.expect("the player joined");
        assert!(pg.local < 2, "the player has a player's place");
    }
}
