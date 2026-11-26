//! The Steam features beyond lobbies and relayed play, behind one interface so the game never
//! asks whether Steam is there or which App ID it runs under:
//!
//! - achievements and lifetime stats (crate::achievements),
//! - Steam Cloud saves (crate::cloud),
//! - rich presence lines (crate::presence),
//! - the friends list, invites and the party lobby (crate::party),
//! - Steam Input: the pad in hand and the action sets (crate::glyphs).
//!
//! [`RealSteam`] talks to the Steam client. Under the shared test App ID 480 it switches off what
//! needs the game's own Steamworks setup (achievements, stats, cloud, localized presence) and says
//! so in [`Features`]; the game keeps all of that on this computer and shows it the same way.
//! [`NoSteam`] is used when Steam isn't running, and [`FakeSteam`] is an in-memory Steam for the
//! tests and the screenshot scenes (several fakes can share one world to test parties).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

/// Which features are live through Steam right now (the rest are kept on this computer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Features {
    pub steam: bool,
    pub achievements: bool,
    pub stats: bool,
    pub cloud: bool,
    /// Friends see the localized lines (`steam_display`); off, they see the plain status.
    pub presence_tokens: bool,
    pub friends: bool,
    /// Steam Input is reading the controller (action sets, glyphs from its type).
    pub input: bool,
}

/// The controller family in hand: which glyphs and button names to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PadKind {
    Xbox,
    PlayStation,
    Switch,
    Deck,
}

/// Steam Input action sets (steam/steam_input_manifest.vdf).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionSet {
    Menu,
    Combat,
    Hub,
    Spectate,
}

impl ActionSet {
    pub const ALL: [ActionSet; 4] = [ActionSet::Menu, ActionSet::Combat, ActionSet::Hub, ActionSet::Spectate];
    /// Its name in the action manifest.
    pub fn key(self) -> &'static str {
        match self {
            ActionSet::Menu => "menu_controls",
            ActionSet::Combat => "combat_controls",
            ActionSet::Hub => "hub_controls",
            ActionSet::Spectate => "spectate_controls",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            ActionSet::Menu => "Menus",
            ActionSet::Combat => "Combat",
            ActionSet::Hub => "Varrow Centre",
            ActionSet::Spectate => "Spectating",
        }
    }
}

/// One Steam friend for the friends panel.
#[derive(Clone, Debug, PartialEq)]
pub struct FriendRow {
    pub id: u64,
    pub name: String,
    pub online: bool,
    /// Playing this game.
    pub in_game: bool,
    /// Their presence line (what they're doing in the game), or the Steam state.
    pub status: String,
    /// Their "Join game" connect string when they can be joined.
    pub connect: Option<String>,
}

/// Something Steam reported back (party lobbies; achievements stored).
#[derive(Clone, Debug, PartialEq)]
pub enum ApiEvent {
    PartyCreated(u64),
    PartyEntered(u64),
    PartyFailed(String),
    /// A member joined or left the party, or its data changed.
    PartyChanged(u64),
}

/// A controller read through Steam Input, already turned into the game's buttons (crate::pad).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SteamPad {
    pub lx: f32,
    pub ly: f32,
    pub rx: f32,
    pub ry: f32,
    pub lt: f32,
    pub rt: f32,
    /// crate::pad::btn bits held.
    pub held: u32,
}

/// Everything the game asks of Steam beyond lobbies (see the module notes).
pub trait SteamApi {
    fn features(&self) -> Features;
    fn app_id(&self) -> u32;
    fn me(&self) -> u64 {
        0
    }
    // ---- achievements and stats
    /// Unlock (true when Steam took it).
    fn ach_set(&mut self, _id: &str) -> bool {
        false
    }
    fn ach_get(&self, _id: &str) -> Option<bool> {
        None
    }
    #[cfg_attr(not(test), allow(dead_code))]
    fn ach_clear(&mut self, _id: &str) {}
    fn stat_set(&mut self, _id: &str, _v: i32) -> bool {
        false
    }
    #[cfg_attr(not(test), allow(dead_code))]
    fn stat_get(&self, _id: &str) -> Option<i32> {
        None
    }
    /// Send what's been set (and show Steam's own unlock pop-up).
    fn store(&mut self) -> bool {
        false
    }
    // ---- Steam Cloud
    fn cloud_read(&self, _name: &str) -> Option<Vec<u8>> {
        None
    }
    fn cloud_write(&mut self, _name: &str, _data: &[u8]) -> bool {
        false
    }
    /// When the cloud copy was written (seconds since 1970), None when there isn't one.
    fn cloud_time(&self, _name: &str) -> Option<i64> {
        None
    }
    // ---- rich presence
    fn presence(&mut self, _key: &str, _value: Option<&str>) {}
    #[cfg_attr(not(test), allow(dead_code))]
    fn presence_clear(&mut self) {}
    // ---- friends and the party
    fn friends(&self) -> Vec<FriendRow> {
        Vec::new()
    }
    /// Send a game invite carrying a connect string (it starts the game for them if need be).
    fn invite(&mut self, _friend: u64, _connect: &str) -> bool {
        false
    }
    /// Steam's own invite window for a connect string (overlay).
    fn invite_overlay(&mut self, _connect: &str) {}
    fn party_create(&mut self) {}
    fn party_join(&mut self, _id: u64) {}
    fn party_leave(&mut self, _id: u64) {}
    fn party_set(&mut self, _id: u64, _key: &str, _value: &str) {}
    fn party_get(&self, _id: u64, _key: &str) -> Option<String> {
        None
    }
    fn party_members(&self, _id: u64) -> Vec<u64> {
        Vec::new()
    }
    fn party_owner(&self, _id: u64) -> u64 {
        0
    }
    fn name_of(&self, _id: u64) -> String {
        String::new()
    }
    fn poll(&mut self) -> Vec<ApiEvent> {
        Vec::new()
    }
    // ---- Steam Input
    /// The controller Steam Input sees, and its state in this action set (None: Steam Input isn't
    /// reading a controller, so the game reads it itself).
    fn pad(&mut self, _set: ActionSet) -> Option<(PadKind, SteamPad)> {
        None
    }
    /// Steam's binding panel for the controller (Steam Input only).
    fn binding_panel(&mut self) -> bool {
        false
    }
}

// ================================================================================ no Steam

/// Steam isn't running: everything stays on this computer.
pub struct NoSteam;

impl SteamApi for NoSteam {
    fn features(&self) -> Features {
        Features::default()
    }
    fn app_id(&self) -> u32 {
        super::steam::app_id()
    }
}

// ================================================================================ real Steam

/// The Steam client (see the module notes for what's on under App ID 480).
pub struct RealSteam {
    client: steamworks::Client,
    id: u32,
    feats: Features,
    events: std::sync::Arc<std::sync::Mutex<Vec<ApiEvent>>>,
    _callbacks: Vec<steamworks::CallbackHandle>,
    /// Steam Input: the action set handles, the action handles by name, and whether to read it.
    sets: HashMap<ActionSet, u64>,
    digital: Vec<(&'static str, u64, u32)>,
    analog: Vec<(&'static str, u64)>,
    active_set: Option<(u64, u64)>,
    /// Presence values last sent (Steam is only told about changes).
    sent: HashMap<String, String>,
}

impl RealSteam {
    pub fn new(client: steamworks::Client) -> RealSteam {
        let id = super::steam::app_id();
        let real = super::steam::real_app(id);
        let events: std::sync::Arc<std::sync::Mutex<Vec<ApiEvent>>> = Default::default();
        let mut callbacks = Vec::new();
        let ev = events.clone();
        callbacks.push(client.register_callback(move |c: steamworks::LobbyChatUpdate| {
            if let Ok(mut e) = ev.lock() {
                e.push(ApiEvent::PartyChanged(c.lobby.raw()));
            }
        }));
        let ev = events.clone();
        callbacks.push(client.register_callback(move |c: steamworks::LobbyDataUpdate| {
            if let Ok(mut e) = ev.lock() {
                e.push(ApiEvent::PartyChanged(c.lobby.raw()));
            }
        }));
        // Steam Input: our action manifest beside the game (a development copy; a released game's
        // manifest comes from its Steamworks settings), then the handles
        let input = client.input();
        let mut input_on = input.init(false);
        if input_on {
            if let Some(m) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("steam").join("steam_input_manifest.vdf"))).filter(|p| p.is_file()) {
                input.set_input_action_manifest_file_path(&m.to_string_lossy());
            }
        }
        let mut sets = HashMap::new();
        let mut digital = Vec::new();
        let mut analog = Vec::new();
        if input_on {
            input.run_frame();
            for s in ActionSet::ALL {
                let h = input.get_action_set_handle(s.key());
                if h != 0 {
                    sets.insert(s, h);
                }
            }
            for (name, bit) in crate::glyphs::DIGITAL_ACTIONS {
                let h = input.get_digital_action_handle(name);
                if h != 0 {
                    digital.push((*name, h, *bit));
                }
            }
            for name in crate::glyphs::ANALOG_ACTIONS {
                let h = input.get_analog_action_handle(name);
                if h != 0 {
                    analog.push((*name, h));
                }
            }
            // no manifest loaded: leave the controller to the game's own reading
            input_on = !sets.is_empty();
        }
        let cloud = real && client.remote_storage().is_cloud_enabled_for_account() && client.remote_storage().is_cloud_enabled_for_app();
        let feats = Features { steam: true, achievements: real, stats: real, cloud, presence_tokens: real, friends: true, input: input_on };
        RealSteam { client, id, feats, events, _callbacks: callbacks, sets, digital, analog, active_set: None, sent: HashMap::new() }
    }
}

impl SteamApi for RealSteam {
    fn features(&self) -> Features {
        self.feats
    }
    fn app_id(&self) -> u32 {
        self.id
    }
    fn me(&self) -> u64 {
        self.client.user().steam_id().raw()
    }
    fn ach_set(&mut self, id: &str) -> bool {
        self.feats.achievements && self.client.user_stats().achievement(id).set().is_ok()
    }
    fn ach_get(&self, id: &str) -> Option<bool> {
        if !self.feats.achievements {
            return None;
        }
        self.client.user_stats().achievement(id).get().ok()
    }
    fn ach_clear(&mut self, id: &str) {
        if self.feats.achievements {
            let _ = self.client.user_stats().achievement(id).clear();
        }
    }
    fn stat_set(&mut self, id: &str, v: i32) -> bool {
        self.feats.stats && self.client.user_stats().set_stat_i32(id, v).is_ok()
    }
    fn stat_get(&self, id: &str) -> Option<i32> {
        if !self.feats.stats {
            return None;
        }
        self.client.user_stats().get_stat_i32(id).ok()
    }
    fn store(&mut self) -> bool {
        (self.feats.stats || self.feats.achievements) && self.client.user_stats().store_stats().is_ok()
    }
    fn cloud_read(&self, name: &str) -> Option<Vec<u8>> {
        use std::io::Read;
        if !self.feats.cloud {
            return None;
        }
        let f = self.client.remote_storage().file(name);
        if !f.exists() {
            return None;
        }
        let mut out = Vec::new();
        f.read().read_to_end(&mut out).ok()?;
        Some(out)
    }
    fn cloud_write(&mut self, name: &str, data: &[u8]) -> bool {
        use std::io::Write;
        if !self.feats.cloud {
            return false;
        }
        let mut w = self.client.remote_storage().file(name).write();
        w.write_all(data).is_ok()
    }
    fn cloud_time(&self, name: &str) -> Option<i64> {
        if !self.feats.cloud {
            return None;
        }
        let f = self.client.remote_storage().file(name);
        f.exists().then(|| f.timestamp())
    }
    fn presence(&mut self, key: &str, value: Option<&str>) {
        let v = value.unwrap_or("").to_string();
        if self.sent.get(key) == Some(&v) {
            return;
        }
        self.sent.insert(key.to_string(), v);
        self.client.friends().set_rich_presence(key, value);
    }
    fn presence_clear(&mut self) {
        self.sent.clear();
        self.client.friends().clear_rich_presence();
    }
    fn friends(&self) -> Vec<FriendRow> {
        use steamworks::{FriendFlags, FriendState};
        let f = self.client.friends();
        let mut out: Vec<FriendRow> = f
            .get_friends(FriendFlags::IMMEDIATE)
            .into_iter()
            .map(|fr| {
                let state = fr.state();
                let online = !matches!(state, FriendState::Offline | FriendState::Invisible);
                let in_game = fr.game_played().is_some_and(|g| g.game.app_id().0 == self.id);
                let line = if in_game { fr.rich_presence("status") } else { None };
                let status = match (line, state) {
                    (Some(s), _) if !s.is_empty() => s,
                    (_, _) if in_game => "Playing Duel Rush".into(),
                    (_, FriendState::Offline | FriendState::Invisible) => "Offline".into(),
                    (_, FriendState::Away | FriendState::Snooze) => "Away".into(),
                    (_, FriendState::Busy) => "Busy".into(),
                    _ => "Online".into(),
                };
                let connect = if in_game { fr.rich_presence("connect").filter(|c| !c.is_empty()) } else { None };
                FriendRow { id: fr.id().raw(), name: fr.name(), online, in_game, status, connect }
            })
            .collect();
        crate::party::sort_friends(&mut out);
        out
    }
    fn invite(&mut self, friend: u64, connect: &str) -> bool {
        self.client.friends().get_friend(steamworks::SteamId::from_raw(friend)).invite_user_to_game(connect);
        true
    }
    fn invite_overlay(&mut self, connect: &str) {
        self.client.friends().activate_invite_dialog_connect_string(connect);
    }
    fn party_create(&mut self) {
        let ev = self.events.clone();
        self.client.matchmaking().create_lobby(steamworks::LobbyType::FriendsOnly, crate::party::MAX as u32, move |r| {
            if let Ok(mut e) = ev.lock() {
                e.push(match r {
                    Ok(l) => ApiEvent::PartyCreated(l.raw()),
                    Err(err) => ApiEvent::PartyFailed(err.to_string()),
                });
            }
        });
    }
    fn party_join(&mut self, id: u64) {
        let ev = self.events.clone();
        self.client.matchmaking().join_lobby(steamworks::LobbyId::from_raw(id), move |r| {
            if let Ok(mut e) = ev.lock() {
                e.push(match r {
                    Ok(l) => ApiEvent::PartyEntered(l.raw()),
                    Err(()) => ApiEvent::PartyFailed("that party is full or closed".into()),
                });
            }
        });
    }
    fn party_leave(&mut self, id: u64) {
        self.client.matchmaking().leave_lobby(steamworks::LobbyId::from_raw(id));
    }
    fn party_set(&mut self, id: u64, key: &str, value: &str) {
        self.client.matchmaking().set_lobby_data(steamworks::LobbyId::from_raw(id), key, value);
    }
    fn party_get(&self, id: u64, key: &str) -> Option<String> {
        self.client.matchmaking().lobby_data(steamworks::LobbyId::from_raw(id), key).filter(|v| !v.is_empty())
    }
    fn party_members(&self, id: u64) -> Vec<u64> {
        self.client.matchmaking().lobby_members(steamworks::LobbyId::from_raw(id)).iter().map(|m| m.raw()).collect()
    }
    fn party_owner(&self, id: u64) -> u64 {
        self.client.matchmaking().lobby_owner(steamworks::LobbyId::from_raw(id)).raw()
    }
    fn name_of(&self, id: u64) -> String {
        self.client.friends().get_friend(steamworks::SteamId::from_raw(id)).name()
    }
    fn poll(&mut self) -> Vec<ApiEvent> {
        std::mem::take(&mut *self.events.lock().unwrap_or_else(|e| e.into_inner()))
    }
    fn pad(&mut self, set: ActionSet) -> Option<(PadKind, SteamPad)> {
        if !self.feats.input {
            return None;
        }
        let input = self.client.input();
        input.run_frame();
        let h = *input.get_connected_controllers().first()?;
        if h == 0 {
            return None;
        }
        if let Some(&sh) = self.sets.get(&set) {
            if self.active_set != Some((h, sh)) {
                input.activate_action_set_handle(h, sh);
                self.active_set = Some((h, sh));
            }
        }
        let kind = crate::glyphs::kind_of_input(input.get_input_type_for_handle(h));
        let digital: Vec<(&str, bool, u32)> = self.digital.iter().map(|&(n, ah, bit)| {
            let d = input.get_digital_action_data(h, ah);
            (n, d.bActive && d.bState, bit)
        }).collect();
        let analog: Vec<(&str, f32, f32)> = self.analog.iter().map(|&(n, ah)| {
            let d = input.get_analog_action_data(h, ah);
            (n, if d.bActive { d.x } else { 0.0 }, if d.bActive { d.y } else { 0.0 })
        }).collect();
        Some((kind, crate::glyphs::pad_from_actions(&digital, &analog)))
    }
    fn binding_panel(&mut self) -> bool {
        if !self.feats.input {
            return false;
        }
        let input = self.client.input();
        match input.get_connected_controllers().first() {
            Some(&h) if h != 0 => input.show_binding_panel(h),
            _ => false,
        }
    }
}

// ================================================================================ fake Steam

/// The shared state of a fake Steam: lobbies (parties) and what each fake account has sent.
#[derive(Default, Debug)]
pub struct FakeWorld {
    pub next_lobby: u64,
    /// Lobby id → (owner, members, data).
    pub lobbies: BTreeMap<u64, (u64, Vec<u64>, BTreeMap<String, String>)>,
    /// (from, to, connect) invites sent.
    pub invites: Vec<(u64, u64, String)>,
    /// Names of the fake accounts.
    pub names: BTreeMap<u64, String>,
}

/// An in-memory Steam account for tests and screenshot scenes. Its features can be switched to
/// act as the real game App ID or as 480.
pub struct FakeSteam {
    pub id: u64,
    pub app: u32,
    pub feats: Features,
    pub world: Rc<RefCell<FakeWorld>>,
    pub achievements: BTreeMap<String, bool>,
    pub stats: BTreeMap<String, i32>,
    pub stored: u32,
    /// The cloud: name → (bytes, timestamp).
    pub cloud: BTreeMap<String, (Vec<u8>, i64)>,
    /// The clock for cloud writes (seconds since 1970).
    pub now: i64,
    pub rich: BTreeMap<String, String>,
    pub friend_rows: Vec<FriendRow>,
    pub events: Vec<ApiEvent>,
    pub pad_state: Option<(PadKind, SteamPad)>,
    pub sets_seen: Vec<ActionSet>,
}

impl FakeSteam {
    /// A fake account with the game's own App ID (every feature on).
    pub fn new(id: u64) -> FakeSteam {
        FakeSteam::in_world(id, Rc::new(RefCell::new(FakeWorld { next_lobby: 9000, ..Default::default() })))
    }
    /// Another account in the same fake Steam (to test parties and invites between them).
    pub fn in_world(id: u64, world: Rc<RefCell<FakeWorld>>) -> FakeSteam {
        world.borrow_mut().names.entry(id).or_insert_with(|| format!("Player {id}"));
        FakeSteam {
            id,
            app: 3_210_990,
            feats: Features { steam: true, achievements: true, stats: true, cloud: true, presence_tokens: true, friends: true, input: false },
            world,
            achievements: BTreeMap::new(),
            stats: BTreeMap::new(),
            stored: 0,
            cloud: BTreeMap::new(),
            now: 1_800_000_000,
            rich: BTreeMap::new(),
            friend_rows: Vec::new(),
            events: Vec::new(),
            pad_state: None,
            sets_seen: Vec::new(),
        }
    }
    /// Act as the shared test App ID: only what works under 480 stays on.
    pub fn as_test_app(mut self) -> FakeSteam {
        self.app = super::steam::TEST_APP;
        self.feats = Features { steam: true, achievements: false, stats: false, cloud: false, presence_tokens: false, friends: true, input: false };
        self
    }
    /// A friends list for pictures and tests.
    pub fn sample_friends() -> Vec<FriendRow> {
        let row = |id: u64, name: &str, online: bool, in_game: bool, status: &str, connect: Option<&str>| FriendRow { id, name: name.into(), online, in_game, status: status.into(), connect: connect.map(str::to_string) };
        let mut v = vec![
            row(11, "Mireille", true, true, "Ranked 1v1 - Gold Writ - 3-2 on Sunfall Keep", Some("+connect_lobby 77001 +watch")),
            row(12, "Odo Brandt", true, true, "In the Varrow Centre", Some("+connect_lobby 77002")),
            row(13, "Kestrel", true, true, "Raiding the Ashen Vault - room 4/5", None),
            row(14, "Wren Halloway", true, false, "Online", None),
            row(15, "Taddeo", true, false, "Away", None),
            row(16, "Juniper Vos", false, false, "Offline", None),
            row(17, "Ash & Quill", false, false, "Offline", None),
        ];
        crate::party::sort_friends(&mut v);
        v
    }
}

impl SteamApi for FakeSteam {
    fn features(&self) -> Features {
        self.feats
    }
    fn app_id(&self) -> u32 {
        self.app
    }
    fn me(&self) -> u64 {
        self.id
    }
    fn ach_set(&mut self, id: &str) -> bool {
        if !self.feats.achievements {
            return false;
        }
        self.achievements.insert(id.to_string(), true);
        true
    }
    fn ach_get(&self, id: &str) -> Option<bool> {
        if !self.feats.achievements {
            return None;
        }
        Some(self.achievements.get(id).copied().unwrap_or(false))
    }
    fn ach_clear(&mut self, id: &str) {
        self.achievements.remove(id);
    }
    fn stat_set(&mut self, id: &str, v: i32) -> bool {
        if !self.feats.stats {
            return false;
        }
        self.stats.insert(id.to_string(), v);
        true
    }
    fn stat_get(&self, id: &str) -> Option<i32> {
        if !self.feats.stats {
            return None;
        }
        Some(self.stats.get(id).copied().unwrap_or(0))
    }
    fn store(&mut self) -> bool {
        if !(self.feats.stats || self.feats.achievements) {
            return false;
        }
        self.stored += 1;
        true
    }
    fn cloud_read(&self, name: &str) -> Option<Vec<u8>> {
        if !self.feats.cloud {
            return None;
        }
        self.cloud.get(name).map(|c| c.0.clone())
    }
    fn cloud_write(&mut self, name: &str, data: &[u8]) -> bool {
        if !self.feats.cloud {
            return false;
        }
        self.cloud.insert(name.to_string(), (data.to_vec(), self.now));
        true
    }
    fn cloud_time(&self, name: &str) -> Option<i64> {
        if !self.feats.cloud {
            return None;
        }
        self.cloud.get(name).map(|c| c.1)
    }
    fn presence(&mut self, key: &str, value: Option<&str>) {
        match value {
            Some(v) if !v.is_empty() => {
                self.rich.insert(key.to_string(), v.to_string());
            }
            _ => {
                self.rich.remove(key);
            }
        }
    }
    fn presence_clear(&mut self) {
        self.rich.clear();
    }
    fn friends(&self) -> Vec<FriendRow> {
        self.friend_rows.clone()
    }
    fn invite(&mut self, friend: u64, connect: &str) -> bool {
        self.world.borrow_mut().invites.push((self.id, friend, connect.to_string()));
        true
    }
    fn party_create(&mut self) {
        let mut w = self.world.borrow_mut();
        w.next_lobby += 1;
        let id = w.next_lobby;
        w.lobbies.insert(id, (self.id, vec![self.id], BTreeMap::new()));
        self.events.push(ApiEvent::PartyCreated(id));
    }
    fn party_join(&mut self, id: u64) {
        let mut w = self.world.borrow_mut();
        match w.lobbies.get_mut(&id) {
            Some(l) if l.1.len() < crate::party::MAX => {
                if !l.1.contains(&self.id) {
                    l.1.push(self.id);
                }
                self.events.push(ApiEvent::PartyEntered(id));
            }
            _ => self.events.push(ApiEvent::PartyFailed("that party is full or closed".into())),
        }
    }
    fn party_leave(&mut self, id: u64) {
        let mut w = self.world.borrow_mut();
        let gone = if let Some(l) = w.lobbies.get_mut(&id) {
            l.1.retain(|&m| m != self.id);
            // the owner left: the next member owns it (as Steam does)
            if l.0 == self.id {
                l.0 = l.1.first().copied().unwrap_or(0);
            }
            l.1.is_empty()
        } else {
            false
        };
        if gone {
            w.lobbies.remove(&id);
        }
    }
    fn party_set(&mut self, id: u64, key: &str, value: &str) {
        if let Some(l) = self.world.borrow_mut().lobbies.get_mut(&id) {
            // (only the owner may set lobby data, as on Steam)
            if l.0 == self.id {
                l.2.insert(key.to_string(), value.to_string());
            }
        }
    }
    fn party_get(&self, id: u64, key: &str) -> Option<String> {
        self.world.borrow().lobbies.get(&id).and_then(|l| l.2.get(key).cloned()).filter(|v| !v.is_empty())
    }
    fn party_members(&self, id: u64) -> Vec<u64> {
        self.world.borrow().lobbies.get(&id).map(|l| l.1.clone()).unwrap_or_default()
    }
    fn party_owner(&self, id: u64) -> u64 {
        self.world.borrow().lobbies.get(&id).map_or(0, |l| l.0)
    }
    fn name_of(&self, id: u64) -> String {
        self.world.borrow().names.get(&id).cloned().unwrap_or_else(|| format!("Player {id}"))
    }
    fn poll(&mut self) -> Vec<ApiEvent> {
        std::mem::take(&mut self.events)
    }
    fn pad(&mut self, set: ActionSet) -> Option<(PadKind, SteamPad)> {
        if self.pad_state.is_some() && self.sets_seen.last() != Some(&set) {
            self.sets_seen.push(set);
        }
        self.pad_state
    }
}

/// The Steam layer to use: the real client when Steam is running, a fake one for picture scenes
/// (DUEL_FAKE_STEAM=1, or `480` to act as the test App ID), otherwise none.
pub fn open(client: Option<&steamworks::Client>) -> Box<dyn SteamApi> {
    if let Ok(v) = std::env::var("DUEL_FAKE_STEAM") {
        let mut f = FakeSteam::new(76_561_198_000_000_001);
        f.friend_rows = FakeSteam::sample_friends();
        if v.trim() == "480" {
            f = f.as_test_app();
            f.friend_rows = FakeSteam::sample_friends();
        }
        return Box::new(f);
    }
    match client {
        Some(c) => Box::new(RealSteam::new(c.clone())),
        None => Box::new(NoSteam),
    }
}
