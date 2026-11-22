//! Steam: lobbies (host one, find them in a browser, join from the friends list), rich presence
//! for "Join game", and relayed messages so nobody has to forward ports. Uses the Steamworks
//! dev App ID 480 until the game has its own. When Steam isn't running (or its library is
//! missing) `Steam::start` returns None and the game keeps to direct-IP play.

use crate::net;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use steamworks::networking_types::{NetworkingIdentity, SendFlags};
use steamworks::{CallbackHandle, Client, DistanceFilter, LobbyId, LobbyKey, LobbyType, SteamId, StringFilter, StringFilterKind};

/// Steamworks' public test app ("Spacewar"), until we have our own App ID.
pub const APP_ID: u32 = 480;
/// Lobby data that marks our lobbies among everyone else testing with App ID 480.
const GAME_TAG: &str = "duelrush";

/// Something Steam told us, handled on the next frame.
#[derive(Debug)]
pub enum SteamEvent {
    Created(Result<u64, String>),
    Entered(Result<u64, ()>),
    /// A friend clicked "Join game" (or accepted an invite): join this lobby.
    JoinLobby(u64),
    LobbyList(Vec<u64>),
}

/// One row of the lobby browser.
#[derive(Clone, Debug)]
pub struct LobbyRow {
    pub id: u64,
    pub host: u64,
    pub name: String,
    pub mode: u8,
    pub map: String,
    pub players: usize,
    pub max: usize,
    /// Round trip to the host through Steam, once measured.
    pub ping: Option<u32>,
}

pub struct Steam {
    pub client: Client,
    pub me: u64,
    pub name: String,
    /// The map our hosted lobby plays (shown in the browser).
    pub map_name: String,
    /// The lobby we host or are in.
    pub lobby: Option<u64>,
    pub hosting: bool,
    events: Arc<Mutex<Vec<SteamEvent>>>,
    _callbacks: Vec<CallbackHandle>,
    pub rows: Vec<LobbyRow>,
    pub refreshing: bool,
    pings_sent: HashMap<u64, Instant>,
    pings: HashMap<u64, u32>,
}

/// Is the Steam library next to the game? (It's loaded only when first used; calling into it
/// without it would crash, so check first.)
fn library_present() -> bool {
    if !cfg!(windows) {
        return true;
    }
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("steam_api64.dll").is_file())).unwrap_or(false)
}

/// "+connect_lobby <id>" from the command line (Steam starts the game this way from "Join game").
pub fn lobby_from_args(args: &[String]) -> Option<u64> {
    let i = args.iter().position(|a| a == "+connect_lobby")?;
    args.get(i + 1)?.parse().ok()
}

/// The same from a rich-presence connect string.
fn lobby_from_connect(s: &str) -> Option<u64> {
    let parts: Vec<String> = s.split_whitespace().map(str::to_string).collect();
    lobby_from_args(&parts)
}

impl Steam {
    /// Connect to the running Steam client, or None (then the game uses direct IP only).
    pub fn start() -> Option<Steam> {
        if std::env::var_os("DUEL_NO_STEAM").is_some() || !library_present() {
            return None;
        }
        let client = std::panic::catch_unwind(|| Client::init_app(APP_ID)).ok()?.ok()?;
        let events: Arc<Mutex<Vec<SteamEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let mut callbacks = Vec::new();
        // a friend chose "Join game" on us, or we accepted a lobby invite
        let ev = events.clone();
        callbacks.push(client.register_callback(move |j: steamworks::GameLobbyJoinRequested| {
            if let Ok(mut e) = ev.lock() {
                e.push(SteamEvent::JoinLobby(j.lobby_steam_id.raw()));
            }
        }));
        let ev = events.clone();
        callbacks.push(client.register_callback(move |j: steamworks::GameRichPresenceJoinRequested| {
            if let Some(id) = lobby_from_connect(&j.connect) {
                if let Ok(mut e) = ev.lock() {
                    e.push(SteamEvent::JoinLobby(id));
                }
            }
        }));
        // anyone messaging us is a player joining (or a lobby browser measuring ping): accept
        client.networking_messages().session_request_callback(|req| {
            req.accept();
        });
        // relayed connections: start finding the nearest relays now
        client.networking_utils().init_relay_network_access();
        let me = client.user().steam_id().raw();
        let name = client.friends().name();
        Some(Steam { client, me, name, map_name: "Arena".into(), lobby: None, hosting: false, events, _callbacks: callbacks, rows: Vec::new(), refreshing: false, pings_sent: HashMap::new(), pings: HashMap::new() })
    }

    /// Run Steam's callbacks, answer ping probes, and hand back what happened.
    pub fn update(&mut self) -> Vec<SteamEvent> {
        self.client.run_callbacks();
        // ping probes: answer "P", time our "P"s when the "Q" comes back
        let msgs = self.client.networking_messages().receive_messages_on_channel(net::STEAM_PING_CHANNEL, 64);
        for m in msgs {
            let Some(from) = m.identity_peer().steam_id() else { continue };
            let data = m.data();
            match data.first() {
                Some(b'P') => {
                    let mut reply = data.to_vec();
                    reply[0] = b'Q';
                    let _ = self.client.networking_messages().send_message_to_user(NetworkingIdentity::new_steam_id(from), SendFlags::UNRELIABLE_NO_NAGLE, &reply, net::STEAM_PING_CHANNEL);
                }
                Some(b'Q') => {
                    if let Some(t) = self.pings_sent.remove(&from.raw()) {
                        self.pings.insert(from.raw(), t.elapsed().as_millis().min(9999) as u32);
                    }
                }
                _ => {}
            }
        }
        for r in self.rows.iter_mut() {
            if let Some(p) = self.pings.get(&r.host) {
                r.ping = Some(*p);
            }
        }
        let mut out = std::mem::take(&mut *self.events.lock().unwrap_or_else(|e| e.into_inner()));
        for e in &out {
            if let SteamEvent::LobbyList(ids) = e {
                self.refreshing = false;
                self.read_rows(ids);
            }
        }
        out.retain(|e| !matches!(e, SteamEvent::LobbyList(_)));
        out
    }

    /// Open a lobby for our hosted game (public, or friends only).
    pub fn create_lobby(&mut self, public: bool) {
        let ev = self.events.clone();
        let ty = if public { LobbyType::Public } else { LobbyType::FriendsOnly };
        self.client.matchmaking().create_lobby(ty, crate::game::MAX_PLAYERS as u32, move |r| {
            if let Ok(mut e) = ev.lock() {
                e.push(SteamEvent::Created(r.map(|l| l.raw()).map_err(|e| e.to_string())));
            }
        });
    }

    /// Our lobby is up: describe it for the browser and let friends join from Steam.
    pub fn lobby_created(&mut self, id: u64, mode: u8, players: usize) {
        self.lobby = Some(id);
        self.hosting = true;
        let mm = self.client.matchmaking();
        let l = LobbyId::from_raw(id);
        mm.set_lobby_data(l, "game", GAME_TAG);
        mm.set_lobby_data(l, "ver", net::PROTOCOL);
        mm.set_lobby_data(l, "name", &format!("{}'s lobby", self.name));
        mm.set_lobby_data(l, "host", &self.me.to_string());
        mm.set_lobby_data(l, "map", &self.map_name);
        self.update_lobby(mode, players);
        self.set_presence(id);
    }

    /// Keep the lobby's mode and head count current (shown in the browser).
    pub fn update_lobby(&self, mode: u8, players: usize) {
        if let (Some(id), true) = (self.lobby, self.hosting) {
            let mm = self.client.matchmaking();
            let l = LobbyId::from_raw(id);
            mm.set_lobby_data(l, "mode", &mode.to_string());
            mm.set_lobby_data(l, "players", &players.to_string());
        }
    }

    fn set_presence(&self, lobby: u64) {
        let f = self.client.friends();
        f.set_rich_presence("connect", Some(&format!("+connect_lobby {lobby}")));
        f.set_rich_presence("status", Some("In a Duel Rush lobby"));
    }

    /// Join a lobby (from the browser, the friends list or an invite).
    pub fn join_lobby(&mut self, id: u64) {
        let ev = self.events.clone();
        self.client.matchmaking().join_lobby(LobbyId::from_raw(id), move |r| {
            if let Ok(mut e) = ev.lock() {
                e.push(SteamEvent::Entered(r.map(|l| l.raw())));
            }
        });
    }

    /// We're in the lobby: who hosts the game?
    pub fn entered(&mut self, id: u64) -> Option<u64> {
        self.lobby = Some(id);
        self.hosting = false;
        self.set_presence(id);
        let mm = self.client.matchmaking();
        let l = LobbyId::from_raw(id);
        let host = mm.lobby_data(l, "host").and_then(|h| h.parse().ok()).unwrap_or_else(|| mm.lobby_owner(l).raw());
        (host != 0 && host != self.me).then_some(host)
    }

    /// Leave the lobby (when leaving the match).
    pub fn leave(&mut self) {
        if let Some(id) = self.lobby.take() {
            self.client.matchmaking().leave_lobby(LobbyId::from_raw(id));
        }
        self.hosting = false;
        self.client.friends().clear_rich_presence();
    }

    /// Open Steam's invite window for our lobby.
    pub fn invite_friends(&self) {
        if let Some(id) = self.lobby {
            self.client.friends().activate_invite_dialog(LobbyId::from_raw(id));
        }
    }

    /// Ask Steam for open lobbies of this game (and this version), optionally of one mode.
    pub fn refresh(&mut self, mode: Option<u8>) {
        self.refreshing = true;
        let mm = self.client.matchmaking();
        mm.add_request_lobby_list_string_filter(StringFilter(LobbyKey::new("game"), GAME_TAG, StringFilterKind::Equal));
        mm.add_request_lobby_list_string_filter(StringFilter(LobbyKey::new("ver"), net::PROTOCOL, StringFilterKind::Equal));
        let mode_text = mode.map(|m| m.to_string());
        if let Some(m) = &mode_text {
            mm.add_request_lobby_list_string_filter(StringFilter(LobbyKey::new("mode"), m, StringFilterKind::Equal));
        }
        mm.set_request_lobby_list_distance_filter(DistanceFilter::Worldwide);
        mm.set_request_lobby_list_result_count_filter(50);
        let ev = self.events.clone();
        mm.request_lobby_list(move |r| {
            if let Ok(mut e) = ev.lock() {
                e.push(SteamEvent::LobbyList(r.map(|v| v.into_iter().map(|l| l.raw()).collect()).unwrap_or_default()));
            }
        });
    }

    /// Fill the browser from a lobby list, and send each host a ping probe.
    fn read_rows(&mut self, ids: &[u64]) {
        let mm = self.client.matchmaking();
        self.rows = ids
            .iter()
            .map(|&id| {
                let l = LobbyId::from_raw(id);
                let data = |k: &str| mm.lobby_data(l, k).unwrap_or_default();
                let host: u64 = data("host").parse().unwrap_or(0);
                LobbyRow {
                    id,
                    host,
                    name: data("name"),
                    mode: data("mode").parse().unwrap_or(0),
                    map: data("map"),
                    players: data("players").parse().unwrap_or_else(|_| mm.lobby_member_count(l)),
                    max: mm.lobby_member_limit(l).unwrap_or(crate::game::MAX_PLAYERS),
                    ping: self.pings.get(&host).copied(),
                }
            })
            .collect();
        for r in self.rows.clone() {
            if r.host != 0 && r.host != self.me {
                self.pings_sent.insert(r.host, Instant::now());
                let probe = [b'P', 0, 0, 0];
                let _ = self.client.networking_messages().send_message_to_user(
                    NetworkingIdentity::new_steam_id(SteamId::from_raw(r.host)),
                    SendFlags::UNRELIABLE_NO_NAGLE,
                    &probe,
                    net::STEAM_PING_CHANNEL,
                );
            }
        }
    }
}

/// What the browser shows, after its filters.
pub struct BrowserFilter {
    /// None = any mode.
    pub mode: Option<u8>,
    /// None = any map.
    pub map: Option<String>,
    /// Hide lobbies slower than this (ms); 0 = any.
    pub max_ping: u32,
    pub hide_full: bool,
    pub hide_empty: bool,
}

impl Default for BrowserFilter {
    fn default() -> Self {
        BrowserFilter { mode: None, map: None, max_ping: 0, hide_full: true, hide_empty: false }
    }
}

impl BrowserFilter {
    pub fn keep(&self, r: &LobbyRow) -> bool {
        if self.mode.is_some_and(|m| m != r.mode) {
            return false;
        }
        if self.map.as_ref().is_some_and(|m| !r.map.eq_ignore_ascii_case(m)) {
            return false;
        }
        if self.max_ping > 0 && r.ping.is_some_and(|p| p > self.max_ping) {
            return false;
        }
        if self.hide_full && r.players >= r.max {
            return false;
        }
        if self.hide_empty && r.players == 0 {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_game_arguments_are_read() {
        let args: Vec<String> = ["duel_arena.exe", "+connect_lobby", "109775241024323456"].iter().map(|s| s.to_string()).collect();
        assert_eq!(lobby_from_args(&args), Some(109775241024323456));
        assert_eq!(lobby_from_connect("+connect_lobby 42"), Some(42));
        assert_eq!(lobby_from_args(&["duel_arena.exe".to_string()]), None);
    }

    #[test]
    fn browser_filters() {
        let row = |mode, players, ping| LobbyRow { id: 1, host: 2, name: "x".into(), mode, map: "Arena".into(), players, max: 8, ping };
        let f = BrowserFilter { mode: Some(1), map: None, max_ping: 120, hide_full: true, hide_empty: true };
        assert!(f.keep(&row(1, 3, Some(60))));
        assert!(!f.keep(&row(0, 3, Some(60))), "other mode");
        assert!(!f.keep(&row(1, 8, Some(60))), "full");
        assert!(!f.keep(&row(1, 0, Some(60))), "empty");
        assert!(!f.keep(&row(1, 3, Some(300))), "too slow");
        assert!(f.keep(&row(1, 3, None)), "ping not measured yet: still shown");
        let any_map = BrowserFilter { map: Some("Temple".into()), ..BrowserFilter::default() };
        assert!(!any_map.keep(&row(1, 3, None)), "other map");
    }

    #[test]
    fn falls_back_when_steam_is_not_there() {
        // no Steam client on the test machine (or DUEL_NO_STEAM): None, never a crash
        // SAFETY: only this test reads or writes this variable.
        unsafe { std::env::set_var("DUEL_NO_STEAM", "1") };
        assert!(Steam::start().is_none());
    }
}
