//! Who may run the admin commands: Owner, Admin, Player.
//!
//! - The owners and admins are Steam IDs in `owners.txt` in the save folder. On first start it's
//!   copied from `assets/owners.txt`, which ships with the game. You can edit it by hand.
//! - Every computer checks roles against its own copy, using identities it can verify itself.
//!   Nothing the host says about roles counts.
//!   - Your own ID comes from Steam on this computer.
//!   - A host sees each player's Steam ID from the connection, which Steam authenticates.
//!   - A player sees the host's Steam ID from its own connection to the host.
//!   - Other players' IDs are only "reported" by the host. They count for session effects
//!     (moving, muting, the banner) only when Steam's own lobby list shows that ID is in the
//!     lobby.
//! - Anything that changes a player's saved progress or items (::give, ::giveall, ::reset, an
//!   admin's gift made from nothing, ::admin) goes straight from the issuer to the target over
//!   Steam's authenticated messages (crate::commands). The receiver applies it only if its own
//!   owners file gives that sender the right role. So a host can't make himself admin by editing
//!   his file, and can't forge commands from someone who is.
//! - A development build (not a numbered release) played without Steam treats you as the
//!   Owner on your own computer, so the commands work when testing locally. Nobody else sees
//!   you as one.

use crate::game::*;

/// The owners list that ships with the game (the first start copies it to the save folder).
pub const DEFAULT_OWNERS: &str = include_str!("../assets/owners.txt");

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Player = 0,
    Admin = 1,
    Owner = 2,
}

impl Role {
    pub fn name(self) -> &'static str {
        ["Player", "Admin", "Owner"][self as usize]
    }
}

/// The owners and admins (Steam IDs).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Roster {
    pub owners: Vec<u64>,
    pub admins: Vec<u64>,
}

impl Roster {
    pub fn parse(t: &str) -> Roster {
        let mut r = Roster::default();
        for line in t.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let mut it = line.split_whitespace();
            match (it.next(), it.next().and_then(|v| v.parse::<u64>().ok())) {
                (Some("owner"), Some(id)) if !r.owners.contains(&id) => r.owners.push(id),
                (Some("admin"), Some(id)) if !r.admins.contains(&id) => r.admins.push(id),
                _ => {}
            }
        }
        r
    }
    pub fn to_text(&self) -> String {
        let mut s = String::from("# Duel Rush owners and admins (edit me). One line each: \"owner <SteamID64>\" or \"admin <SteamID64>\".\n# Owners promote admins in game with ::admin <player> (and ::unadmin).\n");
        for id in &self.owners {
            s += &format!("owner {id}\n");
        }
        for id in &self.admins {
            s += &format!("admin {id}\n");
        }
        s
    }
    fn path() -> Option<std::path::PathBuf> {
        crate::platform::data_file("owners.txt")
    }
    /// This computer's list (written from the shipped one the first time).
    pub fn load() -> Roster {
        let Some(p) = Roster::path() else { return Roster::parse(DEFAULT_OWNERS) };
        match std::fs::read_to_string(&p) {
            Ok(t) => Roster::parse(&t),
            Err(_) => {
                let r = Roster::parse(DEFAULT_OWNERS);
                if !cfg!(test) {
                    let _ = crate::platform::save_file(&p, &r.to_text());
                }
                r
            }
        }
    }
    pub fn save(&self) {
        if cfg!(test) {
            return;
        }
        if let Some(p) = Roster::path() {
            let _ = crate::platform::save_file(&p, &self.to_text());
        }
    }
    pub fn role(&self, id: u64) -> Role {
        if id == 0 {
            Role::Player
        } else if self.owners.contains(&id) {
            Role::Owner
        } else if self.admins.contains(&id) {
            Role::Admin
        } else {
            Role::Player
        }
    }
    pub fn set_admin(&mut self, id: u64, on: bool) -> bool {
        let had = self.admins.contains(&id);
        if on && !had && !self.owners.contains(&id) {
            self.admins.push(id);
            true
        } else if !on && had {
            self.admins.retain(|&a| a != id);
            true
        } else {
            false
        }
    }
}

/// How sure this computer is of a slot's Steam ID.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Trust {
    #[default]
    None,
    /// The host said so (not proof).
    Reported,
    /// From the connection itself, or our own Steam.
    Verified,
}

/// Times of day and weather an admin can set (the hub's sky).
pub const TIMES: [&str; 3] = ["day", "dusk", "night"];
pub const WEATHERS: [&str; 3] = ["clear", "rain", "fog"];

/// This session's admin state on this computer.
#[derive(Clone, Debug)]
pub struct Session {
    pub roster: Roster,
    /// Each slot's Steam ID (0 unknown) and how it's known.
    pub ids: [u64; MAX_PLAYERS],
    pub trust: [Trust; MAX_PLAYERS],
    /// Steam's own list of the lobby's members (filled by the menu side from Steam).
    pub lobby: Vec<u64>,
    /// Our own Steam ID (0: Steam isn't running).
    pub my_id: u64,
    /// The development-build fallback: we're the Owner on this computer.
    pub dev_owner: bool,
    /// Slots that take no damage; movement speed multipliers; muted until (game time).
    pub god: u32,
    pub speed: [f32; MAX_PLAYERS],
    pub muted: [f32; MAX_PLAYERS],
    /// This computer's session has had stats or unlocks changed (::max, ::god, ::give to
    /// yourself, ::speed): it counts for no leaderboard, record, achievement or career.
    pub gated: bool,
    /// (host) the players whose sessions are marked so.
    pub gated_slots: u32,
    /// The hub's sky: time of day and weather (TIMES, WEATHERS).
    pub time_of_day: u8,
    pub weather: u8,
    /// A banner for everyone (text, seconds left).
    pub announce: Option<(String, f32)>,
    /// A ::reset waiting for its confirm (target slot, seconds left).
    pub pending_reset: Option<(usize, f32)>,
    /// We were removed by an admin (the reason).
    pub kicked: Option<String>,
    /// Lines for the command log file (the menu side appends them), and replies for our chat.
    pub log: Vec<String>,
    pub replies: Vec<String>,
    /// Messages to send straight to a Steam user (their ID, the bytes), and the session's
    /// commands that wait for the host.
    pub direct_out: Vec<(u64, Vec<u8>)>,
    /// What this computer's player should do now that the menu side handles: ::max (unlock
    /// everything, mastery too), a ::reset of our progress, items given to us.
    pub max_req: bool,
    pub reset_req: bool,
    /// (host) players removed: told this frame, dropped from the session the next.
    pub kicked_slots: Vec<usize>,
    pub drop_slots: Vec<usize>,
    /// (host) when the Steam IDs go out again.
    pub ids_t: f32,
    /// Set up for this game (the menu side fills in our Steam ID and the list once).
    pub started: bool,
    /// The ::help list is open on screen.
    pub show_help: bool,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            roster: Roster::default(),
            ids: [0; MAX_PLAYERS],
            trust: [Trust::None; MAX_PLAYERS],
            lobby: Vec::new(),
            my_id: 0,
            dev_owner: false,
            god: 0,
            speed: [1.0; MAX_PLAYERS],
            muted: [0.0; MAX_PLAYERS],
            gated: false,
            gated_slots: 0,
            time_of_day: 0,
            weather: 0,
            announce: None,
            pending_reset: None,
            kicked: None,
            log: Vec::new(),
            replies: Vec::new(),
            direct_out: Vec::new(),
            max_req: false,
            reset_req: false,
            kicked_slots: Vec::new(),
            drop_slots: Vec::new(),
            ids_t: 0.0,
            started: false,
            show_help: false,
        }
    }
}

/// Whether this is a development build (made anywhere but the release builder).
pub fn dev_build() -> bool {
    env!("DUEL_BUILD_NUMBER") == "0"
}

impl Session {
    /// For a new game: this computer's list, and the fallback when there's no Steam.
    pub fn start(my_id: u64) -> Session {
        Session { roster: Roster::load(), my_id, dev_owner: my_id == 0 && dev_build(), started: true, ..Default::default() }
    }
    pub fn is_god(&self, slot: usize) -> bool {
        slot < 32 && self.god & (1 << slot) != 0
    }
    pub fn set_god(&mut self, slot: usize, on: bool) {
        if slot < 32 {
            if on {
                self.god |= 1 << slot;
            } else {
                self.god &= !(1 << slot);
            }
        }
    }
    /// Note something for the log file.
    pub fn log_line(&mut self, who: &str, id: u64, line: &str, outcome: &str) {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        self.log.push(format!("{t}\t{who}\t{id}\t{line}\t{outcome}"));
    }
}

impl Game {
    /// A slot's role as this computer sees it.
    pub fn role_of(&self, slot: usize) -> Role {
        let s = &self.admin;
        if slot == self.local {
            if s.my_id != 0 {
                return s.roster.role(s.my_id);
            }
            return if s.dev_owner { Role::Owner } else { Role::Player };
        }
        if slot >= MAX_PLAYERS || (slot < self.f.len() && self.f[slot].is_bot) {
            return Role::Player;
        }
        let id = s.ids[slot];
        match s.trust[slot] {
            Trust::Verified => s.roster.role(id),
            Trust::Reported if s.lobby.contains(&id) => s.roster.role(id),
            _ => Role::Player,
        }
    }

    /// A Steam user's role on this computer (for a message that came straight from them, its
    /// sender proven by Steam).
    pub fn role_of_id(&self, id: u64) -> Role {
        if id != 0 && id == self.admin.my_id {
            return self.role_of(self.local);
        }
        self.admin.roster.role(id)
    }

    /// The slot a Steam ID is in (as far as we know).
    pub fn slot_of_id(&self, id: u64) -> Option<usize> {
        if id == 0 {
            return None;
        }
        (0..self.f.len().min(MAX_PLAYERS)).find(|&s| self.f[s].present && self.admin.ids[s] == id)
    }

    /// Record a slot's Steam ID as known for sure (the connection, or our own Steam).
    pub fn set_verified_id(&mut self, slot: usize, id: u64) {
        if slot < MAX_PLAYERS && id != 0 {
            self.admin.ids[slot] = id;
            self.admin.trust[slot] = Trust::Verified;
        }
    }

    /// The IDs the host reported (never over one it knows for sure).
    pub fn set_reported_ids(&mut self, list: &[(u8, u64)]) {
        for &(s, id) in list {
            let s = s as usize;
            if s < MAX_PLAYERS && s != self.local && self.admin.trust[s] != Trust::Verified {
                self.admin.ids[s] = id;
                self.admin.trust[s] = if id == 0 { Trust::None } else { Trust::Reported };
            }
        }
    }

    /// An answer to an admin command, in the chat (only on this computer).
    pub fn admin_line(&mut self, text: String) {
        self.chat.push(ChatLine { name: "Admin".into(), color: 1, text, life: 14.0, role: 0 });
        if self.chat.len() > 8 {
            self.chat.remove(0);
        }
    }

    /// Mark this session as not counting for records (changed stats or unlocks).
    pub fn gate_session(&mut self, why: &str) {
        if !self.admin.gated {
            self.admin.gated = true;
            self.admin.replies.push(format!("This session no longer counts for records, leaderboards or the career ({why})."));
            let me = self.local as u8;
            if self.kind == Kind::Client {
                self.out_events.push(Ev::Hub(crate::hub::HubEv::Gated { who: me }));
            } else if (me as usize) < 32 {
                self.admin.gated_slots |= 1 << me;
            }
        }
    }

    /// Whether a slot's results may go on boards and records.
    pub fn counts_for_records(&self, slot: usize) -> bool {
        if slot == self.local {
            return !self.admin.gated;
        }
        slot >= 32 || self.admin.gated_slots & (1 << slot) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_owners_file_reads_owners_and_admins() {
        let r = Roster::parse("# c\nowner 111\nadmin 222 # promoted\nadmin x\nowner 111\n");
        assert_eq!(r.owners, vec![111]);
        assert_eq!(r.admins, vec![222]);
        assert_eq!((r.role(111), r.role(222), r.role(333), r.role(0)), (Role::Owner, Role::Admin, Role::Player, Role::Player));
        assert_eq!(Roster::parse(&r.to_text()), r);
        // the shipped list has an owner
        assert!(!Roster::parse(DEFAULT_OWNERS).owners.is_empty());
    }
}
