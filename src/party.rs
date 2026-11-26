//! Parties: up to four friends who play together. A party is a friends-only Steam lobby of its
//! own beside the match lobby. Its leader (the lobby's owner) queues and joins, and the others
//! follow: when the leader enters a match lobby (Quick Play, a hosted game, Varrow Centre, a
//! friend's game) the party's `follow` data names it and everyone else joins it. A ranked 1v1
//! has one free place, so the rest of the party joins it to watch.
//!
//! Invites carry `+join_party <id>`; a friend accepts from the in-game friends panel, from
//! Steam's own chat invite or by starting the game from it.

use crate::platform::steam_api::{ApiEvent, FriendRow, SteamApi};

/// Most people in a party.
pub const MAX: usize = 4;

/// The connect string that invites into a party.
pub fn party_connect(id: u64) -> String {
    format!("+join_party {id}")
}

/// The connect string that invites into a match lobby (`watch` for one only to watch).
pub fn lobby_connect(id: u64, watch: bool) -> String {
    if watch { format!("+connect_lobby {id} +watch") } else { format!("+connect_lobby {id}") }
}

/// The friends panel's order: playing this game, then online, then offline, each by name.
pub fn sort_friends(v: &mut [FriendRow]) {
    v.sort_by(|a, b| (!a.in_game, !a.online, a.name.to_lowercase()).cmp(&(!b.in_game, !b.online, b.name.to_lowercase())));
}

/// What the party wants the game to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartyAct {
    /// Follow the leader into this match lobby (to watch when it's ranked).
    Join { lobby: u64, watch: bool },
    /// A line for the player.
    Note(String),
}

/// This player's party (none when `id` is 0).
#[derive(Clone, Debug, Default)]
pub struct Party {
    pub id: u64,
    pub leader: u64,
    pub members: Vec<(u64, String)>,
    /// Being made (invites wait for it).
    pub creating: bool,
    pub waiting_invites: Vec<u64>,
    /// The leader's match lobby already followed (members), or announced (leader).
    pub followed: u64,
    pub announced: (u64, bool),
}

impl Party {
    pub fn active(&self) -> bool {
        self.id != 0
    }
    pub fn leading(&self, me: u64) -> bool {
        self.active() && self.leader == me
    }

    /// Invite a friend into the party (made first when there's none).
    pub fn invite(&mut self, api: &mut dyn SteamApi, friend: u64) -> PartyAct {
        if self.active() {
            if self.members.len() >= MAX {
                return PartyAct::Note(format!("The party is full ({MAX})."));
            }
            api.invite(friend, &party_connect(self.id));
            PartyAct::Note(format!("Invited {} to your party.", api.name_of(friend)))
        } else {
            if !self.waiting_invites.contains(&friend) {
                self.waiting_invites.push(friend);
            }
            if !self.creating {
                self.creating = true;
                api.party_create();
            }
            PartyAct::Note(format!("Making a party and inviting {}...", api.name_of(friend)))
        }
    }

    /// Accept an invite.
    pub fn join(&mut self, api: &mut dyn SteamApi, id: u64) {
        if self.active() && self.id != id {
            self.leave(api);
        }
        api.party_join(id);
    }

    pub fn leave(&mut self, api: &mut dyn SteamApi) {
        if self.active() {
            api.party_leave(self.id);
        }
        *self = Party::default();
    }

    /// Steam's answers about the party lobby.
    pub fn on_event(&mut self, api: &mut dyn SteamApi, ev: &ApiEvent) -> Option<PartyAct> {
        match ev {
            ApiEvent::PartyCreated(id) => {
                self.creating = false;
                self.id = *id;
                self.leader = api.me();
                api.party_set(*id, "game", "duelrush");
                api.party_set(*id, "leader", &api.me().to_string());
                for f in std::mem::take(&mut self.waiting_invites) {
                    api.invite(f, &party_connect(*id));
                }
                self.refresh(api);
                Some(PartyAct::Note("Party made: you lead it. Friends you invite follow you into your matches.".into()))
            }
            ApiEvent::PartyEntered(id) => {
                self.id = *id;
                self.refresh(api);
                let leader = api.name_of(self.leader);
                Some(PartyAct::Note(format!("You joined {leader}'s party: you'll follow them into their matches.")))
            }
            ApiEvent::PartyFailed(why) => {
                self.creating = false;
                self.waiting_invites.clear();
                Some(PartyAct::Note(format!("Couldn't make or join the party ({why}).")))
            }
            ApiEvent::PartyChanged(id) if *id == self.id => {
                self.refresh(api);
                None
            }
            ApiEvent::PartyChanged(_) => None,
        }
    }

    /// Who's in it now (and who leads: the lobby's owner, so it passes on when the leader leaves).
    pub fn refresh(&mut self, api: &dyn SteamApi) {
        if !self.active() {
            return;
        }
        self.leader = api.party_owner(self.id);
        self.members = api.party_members(self.id).into_iter().map(|m| (m, api.name_of(m))).collect();
    }

    /// The leader: where we are now (our match lobby, 0 for none) so the party follows.
    pub fn leader_at(&mut self, api: &mut dyn SteamApi, lobby: u64, ranked: bool) {
        if !self.leading(api.me()) || self.announced == (lobby, ranked) {
            return;
        }
        self.announced = (lobby, ranked);
        api.party_set(self.id, "follow", &if lobby == 0 { String::new() } else { lobby.to_string() });
        api.party_set(self.id, "watch", if ranked { "1" } else { "0" });
    }

    /// The others: follow the leader into a lobby they've just entered (`current`: our own
    /// match lobby, if any). Checked about once a second.
    pub fn follow(&mut self, api: &dyn SteamApi, current: u64) -> Option<PartyAct> {
        if !self.active() || self.leading(api.me()) {
            return None;
        }
        let target: u64 = api.party_get(self.id, "follow").and_then(|v| v.parse().ok()).unwrap_or(0);
        if target == 0 || target == self.followed {
            return None;
        }
        self.followed = target;
        if target == current {
            return None;
        }
        let watch = api.party_get(self.id, "watch").as_deref() == Some("1");
        Some(PartyAct::Join { lobby: target, watch })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::steam::{parse_connect, Connect};
    use crate::platform::steam_api::FakeSteam;

    fn pump(p: &mut Party, api: &mut FakeSteam) -> Vec<PartyAct> {
        let evs = api.poll();
        evs.iter().filter_map(|e| p.on_event(api, e)).collect()
    }

    /// Two friends on a fake Steam: the leader invites, the friend accepts the invite's connect
    /// string, and when the leader queues into a match the friend follows (to watch a ranked one).
    #[test]
    fn the_party_leader_queues_for_everyone() {
        let mut lead = FakeSteam::new(1);
        let mut friend = FakeSteam::in_world(2, lead.world.clone());
        let (mut pl, mut pf) = (Party::default(), Party::default());
        // inviting with no party makes one, then sends the invite
        assert!(matches!(pl.invite(&mut lead, 2), PartyAct::Note(_)));
        assert!(pl.creating && !pl.active());
        pump(&mut pl, &mut lead);
        assert!(pl.active() && pl.leading(1));
        let (from, to, connect) = lead.world.borrow().invites.last().cloned().expect("an invite went out");
        assert_eq!((from, to), (1, 2));
        // the friend gets it (from the panel, Steam's chat, or a fresh start of the game)
        let Some(Connect::Party(id)) = parse_connect(&connect) else { panic!("a party invite: {connect}") };
        pf.join(&mut friend, id);
        pump(&mut pf, &mut friend);
        assert_eq!(pf.id, pl.id);
        assert_eq!(pf.leader, 1, "the owner leads");
        pl.refresh(&lead);
        assert_eq!(pl.members.len(), 2);
        // nothing to follow yet
        assert_eq!(pf.follow(&friend, 0), None);
        // the leader finds a Quick Play game: the party follows into it
        pl.leader_at(&mut lead, 555, false);
        assert_eq!(pf.follow(&friend, 0), Some(PartyAct::Join { lobby: 555, watch: false }));
        assert_eq!(pf.follow(&friend, 555), None, "followed once");
        // the leader queues ranked: only one place, so the party watches
        pl.leader_at(&mut lead, 777, true);
        assert_eq!(pf.follow(&friend, 555), Some(PartyAct::Join { lobby: 777, watch: true }));
        // a member can't steer the party
        pf.leader_at(&mut friend, 999, false);
        assert_eq!(friend.party_get(pf.id, "follow").as_deref(), Some("777"));
        // the leader leaves: the friend leads now
        pl.leave(&mut lead);
        pf.refresh(&friend);
        assert!(pf.leading(2));
        assert_eq!(pf.members.len(), 1);
    }

    #[test]
    fn invites_into_a_full_party_are_refused_and_lobbies_carry_watch() {
        let mut lead = FakeSteam::new(1);
        let mut p = Party::default();
        p.invite(&mut lead, 2);
        pump(&mut p, &mut lead);
        for k in 3..6u64 {
            let mut f = FakeSteam::in_world(k, lead.world.clone());
            let mut pk = Party::default();
            pk.join(&mut f, p.id);
            pump(&mut pk, &mut f);
        }
        p.refresh(&lead);
        assert_eq!(p.members.len(), MAX);
        assert!(matches!(p.invite(&mut lead, 9), PartyAct::Note(n) if n.contains("full")));
        assert_eq!(parse_connect(&lobby_connect(42, true)), Some(Connect::Watch(42)));
        assert_eq!(parse_connect(&lobby_connect(42, false)), Some(Connect::Lobby(42)));
        let mut rows = FakeSteam::sample_friends();
        sort_friends(&mut rows);
        assert!(rows[0].in_game && !rows.last().unwrap().online, "playing first, offline last");
    }
}
