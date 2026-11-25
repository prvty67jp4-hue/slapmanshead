//! Tests for roles, admin commands, the bank and gifts:
//! - roles: a host can't make himself admin, the owners file decides on each computer, and only
//!   the owner can promote;
//! - every :: command, and who may use them;
//! - a session marked by ::max, ::god or ::speed stays off the boards;
//! - the bank file catches a hand edit and falls back to the last good copy;
//! - gifts both ways: what moves, a "no thanks", and items made new only by a proven admin.
#![cfg(test)]

use crate::commands::{self, Direct};
use crate::game::*;
use crate::hub::*;
use crate::items::{self, ItemKey, ItemStore};
use crate::math::*;
use crate::roles::{Role, Roster, Session, Trust};

const OWNER: u64 = 76561198000000001;
const HOST_ID: u64 = 76561198000000002;
const GUEST_ID: u64 = 76561198000000003;

fn roster() -> Roster {
    Roster { owners: vec![OWNER], admins: vec![] }
}

/// An offline hub where this computer's player is the given Steam ID (and the shipped list).
fn hub_as(id: u64, regulars: usize) -> Game {
    let mut g = Game::hub("Me", regulars, Kind::Offline);
    g.admin = Session { roster: roster(), my_id: id, started: true, ..Default::default() };
    g.set_verified_id(0, id);
    g
}

/// A host (slot 0) and a guest (slot 1), each with its own copy of the owners list, each knowing
/// the other's Steam ID the way the connection proves it.
fn pair(host_id: u64, guest_id: u64) -> (Game, Game) {
    let mut host = Game::hub("Host", 0, Kind::Host);
    host.add_player(1, "Guest", false, None, 0);
    host.admin = Session { roster: roster(), my_id: host_id, started: true, ..Default::default() };
    host.set_verified_id(0, host_id);
    host.set_verified_id(1, guest_id);
    let mut guest = Game::new(Kind::Client, 1, Mode::Hub, 0);
    guest.set_map(crate::world::MapId::Varrow);
    guest.hub = Some(Box::new(Hub::new()));
    guest.add_player(0, "Host", false, None, 0);
    guest.add_player(1, "Guest", true, None, 0);
    guest.admin = Session { roster: roster(), my_id: guest_id, started: true, ..Default::default() };
    guest.set_verified_id(1, guest_id);
    guest.set_verified_id(0, host_id);
    (host, guest)
}

/// Hand every event one way, as the network would.
fn carry(from: &mut Game, from_slot: usize, to: &mut Game) {
    for ev in from.out_events.drain(..).collect::<Vec<_>>() {
        if let Some(ev) = crate::net::ev_round_trip(&ev) {
            to.handle_remote_event(from_slot, ev);
        }
    }
}

/// Deliver the messages a computer sends straight over Steam (Steam names their sender).
fn deliver_direct(from: &mut Game, to: &mut Game, to_id: u64) {
    let sender = from.admin.my_id;
    for (id, bytes) in std::mem::take(&mut from.admin.direct_out) {
        if id == to_id {
            commands::on_direct(to, sender, &bytes);
        }
    }
}

fn coat() -> ItemKey {
    ItemKey::Look(84)
}

fn count(g: &Game, k: ItemKey) -> u32 {
    g.hub.as_ref().unwrap().bank.count(k)
}

#[test]
fn a_host_cannot_make_himself_admin() {
    let _q = crate::heavy();
    // the host edits his own owners file to name himself
    let (mut host, mut guest) = pair(HOST_ID, GUEST_ID);
    host.admin.roster.owners.push(HOST_ID);
    assert_eq!(host.role_of(0), Role::Owner, "on his own computer he is");
    // the guest checks its own list, against the host's ID its connection proves
    assert_eq!(guest.role_of(0), Role::Player);
    // his ::give to the guest goes straight over Steam: the guest refuses it
    host.say_chat("::give Guest the regent's long coat");
    assert!(!host.admin.direct_out.is_empty());
    deliver_direct(&mut host, &mut guest, GUEST_ID);
    assert_eq!(count(&guest, coat()), 0, "nothing made from a non-admin");
    assert!(guest.admin.log.iter().any(|l| l.contains("refused")), "and it's logged");
    // nor can he promote anyone, or reset them
    commands::on_direct(&mut guest, HOST_ID, &Direct::Roster { id: HOST_ID, on: true }.to_bytes());
    commands::on_direct(&mut guest, HOST_ID, &Direct::Reset.to_bytes());
    assert!(guest.admin.roster.admins.is_empty() && !guest.admin.reset_req);
    // the host can't claim the owner's ID for himself (his own is proven) ...
    guest.set_reported_ids(&[(0, OWNER)]);
    assert_eq!(guest.admin.ids[0], HOST_ID);
    assert_eq!(guest.role_of(0), Role::Player);
    // ... nor pin it on someone made up: a reported ID counts only if Steam lists them in the lobby
    guest.add_player(3, "FakeOwner", false, None, 0);
    guest.set_reported_ids(&[(3, OWNER)]);
    assert_eq!(guest.admin.trust[3], Trust::Reported);
    assert_eq!(guest.role_of(3), Role::Player);
    guest.admin.lobby = vec![HOST_ID, GUEST_ID, OWNER];
    assert_eq!(guest.role_of(3), Role::Owner, "the real owner in the lobby counts for session effects");
    // the real owner's message (Steam proves who sent it) is applied
    commands::on_direct(&mut guest, OWNER, &Direct::Grant { item: coat(), n: 1 }.to_bytes());
    assert_eq!(count(&guest, coat()), 1);
    // and the owner promotes someone: everyone's own list takes it
    commands::on_direct(&mut guest, OWNER, &Direct::Roster { id: HOST_ID, on: true }.to_bytes());
    assert_eq!(guest.admin.roster.role(HOST_ID), Role::Admin);
    commands::on_direct(&mut guest, OWNER, &Direct::Roster { id: HOST_ID, on: false }.to_bytes());
    assert_eq!(guest.admin.roster.role(HOST_ID), Role::Player);
}

#[test]
fn a_players_command_runs_on_the_host_only_if_the_host_sees_them_as_admin() {
    let _q = crate::heavy();
    // an ordinary guest: their own computer refuses first; a forged one is refused by the host
    let (mut host, mut guest) = pair(HOST_ID, GUEST_ID);
    guest.say_chat("::announce hello");
    assert!(guest.out_events.is_empty(), "not even sent");
    guest.hub_send(HubEv::Cmd { who: 1, text: "announce hello".into() });
    carry(&mut guest, 1, &mut host);
    assert!(host.admin.announce.is_none());
    assert!(host.admin.log.iter().any(|l| l.contains("refused by the host")));
    // the owner as a guest: the host checks the ID on the connection, and carries it out
    let (mut host, mut guest) = pair(HOST_ID, OWNER);
    guest.say_chat("::announce Raid at eight!");
    carry(&mut guest, 1, &mut host);
    assert_eq!(host.admin.announce.as_ref().map(|a| a.0.as_str()), Some("Raid at eight!"));
    carry(&mut host, 0, &mut guest);
    assert_eq!(guest.admin.announce.as_ref().map(|a| a.0.as_str()), Some("Raid at eight!"), "the banner reaches everyone");
    // a player can't send the banner themselves
    let (mut host, mut guest) = pair(HOST_ID, GUEST_ID);
    guest.hub_send(HubEv::Announce { text: "fake".into() });
    carry(&mut guest, 1, &mut host);
    assert!(host.admin.announce.is_none());
}

#[test]
fn without_steam_a_development_build_makes_you_owner_on_your_own_computer_only() {
    let s = Session::start(0);
    assert_eq!(s.dev_owner, crate::roles::dev_build());
    let mut g = Game::hub("Me", 2, Kind::Offline);
    g.admin = Session { roster: roster(), dev_owner: true, started: true, ..Default::default() };
    assert_eq!(g.role_of(0), Role::Owner);
    assert_eq!(g.role_of(1), Role::Player, "nobody else");
    // a released build without Steam: nobody is
    g.admin.dev_owner = false;
    assert_eq!(g.role_of(0), Role::Player);
    g.say_chat("::god");
    assert!(!g.admin.is_god(0));
}

#[test]
fn every_command_does_its_job() {
    let _q = crate::heavy();
    let mut g = hub_as(OWNER, 3);
    let say = |g: &mut Game, s: &str| g.say_chat(s);
    // ::help
    say(&mut g, "::help");
    assert!(g.admin.show_help);
    assert_eq!(commands::help_lines(Role::Admin).len(), commands::COMMANDS.len() - 2);
    // ::god: no damage (and the session is marked)
    say(&mut g, "::god");
    assert!(g.admin.is_god(0) && g.admin.gated);
    let hp = g.f[0].hp;
    g.apply_hit(0, Some(1), 50.0, V3::ZERO, 0.0, false, 0, None);
    assert_eq!(g.f[0].hp, hp);
    say(&mut g, "::god");
    assert!(!g.admin.is_god(0));
    // ::speed
    say(&mut g, "::speed 2");
    assert_eq!(g.admin.speed[0], 2.0);
    say(&mut g, "::speed 99");
    assert_eq!(g.admin.speed[0], 4.0, "kept sane");
    say(&mut g, "::speed 1");
    // ::give to yourself, with an amount, by name and by code
    say(&mut g, "::give me red moon dye 3");
    assert_eq!(count(&g, ItemKey::Dye(26)), 3);
    say(&mut g, "::give Me L84");
    assert_eq!(count(&g, coat()), 1);
    // ::giveall (you're one of everyone; the regulars have no bank)
    say(&mut g, "::giveall ashblood glaive");
    assert_eq!(count(&g, ItemKey::Look(94)), 1);
    // ::max
    say(&mut g, "::max");
    assert!(g.admin.max_req);
    for k in items::catalogue() {
        assert!(g.hub.as_ref().unwrap().bank.owns(k), "{} owned after ::max", k.name());
    }
    let e = g.hub.as_ref().unwrap().earned;
    assert!(e.rooftop && e.table_hard && e.mastery >= 99 && e.raid.cleared_hard);
    // ::tp to a player, ::bring one here
    let target = g.f[2].pos;
    let name2 = g.f[2].name.clone();
    say(&mut g, &format!("::tp {name2}"));
    assert!((g.f[0].pos - target).flat().len() < 2.0);
    g.f[0].pos = v3(5.0, 0.0, 5.0);
    let name1 = g.f[1].name.clone();
    say(&mut g, &format!("::bring {}", &name1[..3]));
    assert!((g.f[1].pos - g.f[0].pos).flat().len() < 2.0, "brought by part of the name");
    // ::mute and ::unmute
    say(&mut g, &format!("::mute {name1} 5"));
    assert!(!commands::may_chat(&g, 1));
    say(&mut g, &format!("::unmute {name1}"));
    assert!(commands::may_chat(&g, 1));
    // ::spawn bot 2
    let before = g.f.iter().filter(|f| f.present).count();
    say(&mut g, "::spawn bot 2");
    assert_eq!(g.f.iter().filter(|f| f.present).count(), before + 2);
    // ::kick (a regular goes)
    let name3 = g.f[3].name.clone();
    say(&mut g, &format!("::kick {name3} too loud"));
    assert!(!g.f[3].present);
    // ::time and ::weather
    say(&mut g, "::time night");
    assert_eq!(g.admin.time_of_day, 2);
    say(&mut g, "::weather fog");
    assert_eq!(g.admin.weather, 2);
    say(&mut g, "::weather sunny");
    assert_eq!(g.admin.weather, 2, "an unknown one changes nothing");
    // ::announce
    say(&mut g, "::announce Welcome to Varrow!");
    assert_eq!(g.admin.announce.as_ref().map(|a| a.0.as_str()), Some("Welcome to Varrow!"));
    // ::item search
    g.chat.clear();
    say(&mut g, "::item search vault warden");
    assert!(g.chat.iter().any(|c| c.text.contains("Vault Warden's Helm")), "{:?}", g.chat.iter().map(|c| &c.text).collect::<Vec<_>>());
    // ::reset asks to confirm first
    say(&mut g, "::reset me");
    assert!(!g.admin.reset_req && g.admin.pending_reset.is_some());
    say(&mut g, "::reset me confirm");
    assert!(g.admin.reset_req);
    // ::admin and ::unadmin (the owner): this list, and a message to each player
    g.add_player(5, "Friend", false, None, 0);
    g.set_verified_id(5, GUEST_ID);
    say(&mut g, "::admin Friend");
    assert_eq!(g.admin.roster.role(GUEST_ID), Role::Admin);
    assert!(g.admin.direct_out.iter().any(|(to, b)| *to == GUEST_ID && Direct::from_bytes(b) == Some(Direct::Roster { id: GUEST_ID, on: true })));
    assert_eq!(g.role_of(5), Role::Admin);
    say(&mut g, "::unadmin Friend");
    assert_eq!(g.role_of(5), Role::Player);
    // an unknown command, and everything went in the log
    say(&mut g, "::fly");
    assert!(g.admin.log.len() >= 20);
    assert!(g.admin.log.iter().any(|l| l.contains("::fly") && l.contains("unknown")));
    // nothing typed as a command ever went out as chat
    assert!(!g.chat.iter().any(|c| c.text.starts_with("::")));
}

#[test]
fn admins_can_use_commands_but_not_the_owners_and_players_none() {
    let _q = crate::heavy();
    let mut g = hub_as(GUEST_ID, 2);
    g.say_chat("::god");
    assert!(!g.admin.is_god(0), "a player can't");
    assert!(g.admin.log.iter().any(|l| l.contains("refused")));
    g.admin.roster.admins.push(GUEST_ID);
    g.say_chat("::god");
    assert!(g.admin.is_god(0), "an admin can");
    g.add_player(5, "Friend", false, None, 0);
    g.set_verified_id(5, HOST_ID);
    g.say_chat("::admin Friend");
    assert_eq!(g.admin.roster.role(HOST_ID), Role::Player, "only the owner promotes");
}

#[test]
fn tab_completes_commands_players_and_items() {
    let mut g = hub_as(OWNER, 3);
    assert_eq!(commands::complete(&g, "::ann").as_deref(), Some("::announce "));
    assert_eq!(commands::complete(&g, "::gi").as_deref(), Some("::give"), "give and giveall share it");
    let name = g.f[1].name.clone();
    let done = commands::complete(&g, &format!("::tp {}", &name[..2].to_lowercase())).unwrap();
    assert!(done.starts_with(&format!("::tp {name}")), "{done}");
    assert_eq!(commands::complete(&g, "::give me the regent").as_deref(), Some("::give me The Regent's Long Coat"));
    assert_eq!(commands::complete(&g, "::time ni").as_deref(), Some("::time night"));
    assert!(!commands::suggestions(&g, "::").is_empty());
    // a player sees none of it
    g.admin.my_id = GUEST_ID;
    assert_eq!(commands::complete(&g, "::ann"), None);
}

#[test]
fn a_session_that_used_admin_commands_counts_for_no_records() {
    let _q = crate::heavy();
    let mut g = hub_as(OWNER, 1);
    assert!(g.counts_for_records(0));
    // a score goes on the board before
    g.hub_send(HubEv::Score { who: 0, board: BOARD_RUN, value: 41000 });
    assert_eq!(g.hub.as_ref().unwrap().boards[BOARD_RUN as usize].len(), 1);
    // ::speed (not 1) marks the session: from then on no board takes it
    g.say_chat("::speed 1.5");
    assert!(g.admin.gated && !g.counts_for_records(0));
    g.hub_send(HubEv::Score { who: 0, board: BOARD_RUN, value: 1000 });
    assert_eq!(g.hub.as_ref().unwrap().boards[BOARD_RUN as usize][0].1, 41000, "the faster run isn't posted");
    // a guest's marked session: the host knows, and doesn't post theirs either
    let (mut host, mut guest) = pair(HOST_ID, OWNER);
    guest.say_chat("::god");
    carry(&mut guest, 1, &mut host);
    assert!(!host.counts_for_records(1));
    guest.hub_send(HubEv::Score { who: 1, board: BOARD_RUN, value: 1000 });
    carry(&mut guest, 1, &mut host);
    assert!(host.hub.as_ref().unwrap().boards[BOARD_RUN as usize].is_empty());
    // ::give to someone else doesn't mark you
    let mut g2 = hub_as(OWNER, 1);
    g2.add_player(5, "Friend", false, None, 0);
    g2.set_verified_id(5, GUEST_ID);
    g2.say_chat("::give Friend red moon dye");
    assert!(!g2.admin.gated);
}

#[test]
fn a_hand_edited_bank_file_is_caught_and_the_last_good_copy_used() {
    let dir = std::env::temp_dir().join(format!("duel_bank_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("bank_1.dat");
    let mut b = items::LocalBank::default();
    b.grant(coat(), 1, "test");
    items::save_at(&b, &p, 1);
    // a second save keeps the first as .bak
    b.grant(ItemKey::Dye(26), 1, "test");
    items::save_at(&b, &p, 1);
    let (good, note) = items::load_at(&p, 1);
    assert!(note.is_none());
    assert_eq!(good.count(ItemKey::Dye(26)), 1);
    // someone gives themselves ten coats in a text editor
    let t = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, t.replace("L84:1", "L84:10")).unwrap();
    let (back, note) = items::load_at(&p, 1);
    assert!(note.is_some_and(|n| n.contains("changed outside the game")));
    assert_eq!(back.count(coat()), 1, "the last good copy, not the edit");
    assert!(p.with_extension("tampered").exists(), "the edited file is set aside");
    // a copy of someone else's bank doesn't load either
    items::save_at(&b, &p, 2);
    let (other, note) = items::load_at(&p, 1);
    assert!(note.is_some());
    assert_eq!(other.count(ItemKey::Dye(26)), 0, "the .bak is account 1's from before the edit, or nothing");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn gifts_move_both_ways_and_a_no_thanks_gives_it_back() {
    let _q = crate::heavy();
    let (mut host, mut guest) = pair(HOST_ID, GUEST_ID);
    host.hub.as_mut().unwrap().bank.grant(coat(), 2, "test");
    // the host gives the guest a coat
    crate::gifts::offer(&mut host, 1, coat(), 1, false).unwrap();
    assert_eq!(count(&host, coat()), 1, "set aside at once");
    carry(&mut host, 0, &mut guest);
    let inc = guest.hub.as_ref().unwrap().gifts.incoming.expect("the guest is asked");
    assert_eq!((inc.item, inc.n, inc.from), (coat(), 1, 0));
    crate::gifts::answer(&mut guest, true);
    carry(&mut guest, 1, &mut host);
    carry(&mut host, 0, &mut guest);
    assert_eq!(count(&guest, coat()), 1, "the guest has it");
    assert_eq!(count(&host, coat()), 1, "and the host one fewer");
    assert!(host.hub.as_ref().unwrap().bank.history.iter().any(|h| h.contains("gave 1 The Regent's Long Coat to Guest")));
    assert!(guest.hub.as_ref().unwrap().bank.history.iter().any(|h| h.contains("gift from Host")));
    assert!(guest.hub.as_ref().unwrap().earned.look_owned(look(84).unwrap()), "and can wear it");
    // the guest gives it back
    crate::gifts::offer(&mut guest, 0, coat(), 1, false).unwrap();
    carry(&mut guest, 1, &mut host);
    assert!(host.hub.as_ref().unwrap().gifts.incoming.is_some());
    crate::gifts::answer(&mut host, true);
    carry(&mut host, 0, &mut guest);
    assert_eq!(count(&host, coat()), 2);
    assert_eq!(count(&guest, coat()), 0);
    assert!(!guest.hub.as_ref().unwrap().earned.look_owned(look(84).unwrap()), "gone from the wardrobe");
    // a "no thanks": it comes back
    crate::gifts::offer(&mut host, 1, coat(), 1, false).unwrap();
    carry(&mut host, 0, &mut guest);
    crate::gifts::answer(&mut guest, false);
    carry(&mut guest, 1, &mut host);
    carry(&mut host, 0, &mut guest);
    assert_eq!(count(&host, coat()), 2, "back in the bank");
    assert_eq!(count(&guest, coat()), 0);
    // what you don't have, or a starter, can't go
    assert!(crate::gifts::offer(&mut guest, 0, coat(), 1, false).is_err());
    assert!(crate::gifts::offer(&mut host, 1, ItemKey::Look(1), 1, false).is_err());
}

#[test]
fn only_a_proven_admin_can_gift_something_made_new() {
    let _q = crate::heavy();
    // a player asking the host to make a gift new: the host turns it down
    let (mut host, mut guest) = pair(HOST_ID, GUEST_ID);
    guest.hub_send(HubEv::GiftOffer { gid: 77, from: 1, to: 0, item: coat().to_bytes(), n: 1, create: true });
    carry(&mut guest, 1, &mut host);
    assert_eq!(count(&host, coat()), 0);
    carry(&mut host, 0, &mut guest);
    // the owner (as host) makes one new for the guest: it counts with the owner's own proof
    let (mut host, mut guest) = pair(OWNER, GUEST_ID);
    crate::gifts::offer(&mut host, 1, coat(), 1, true).unwrap();
    deliver_direct(&mut host, &mut guest, GUEST_ID);
    carry(&mut host, 0, &mut guest);
    crate::gifts::answer(&mut guest, true);
    carry(&mut guest, 1, &mut host);
    carry(&mut host, 0, &mut guest);
    assert_eq!(count(&guest, coat()), 1, "made new, proven");
    // a host (not admin) faking a made-new gift: no proof from an admin, ignored
    let (mut host, mut guest) = pair(HOST_ID, GUEST_ID);
    host.admin.roster.owners.push(HOST_ID); // his own edited list
    crate::gifts::offer(&mut host, 1, coat(), 1, true).unwrap();
    deliver_direct(&mut host, &mut guest, GUEST_ID);
    carry(&mut host, 0, &mut guest);
    crate::gifts::answer(&mut guest, true);
    carry(&mut guest, 1, &mut host);
    carry(&mut host, 0, &mut guest);
    assert_eq!(count(&guest, coat()), 0, "the guest's own list says he's no admin");
}

#[test]
fn the_bank_tabs_carry_and_ids_go_over_the_network() {
    let mut b = items::LocalBank::default();
    b.grant(coat(), 1, "t");
    b.grant(ItemKey::Dye(26), 3, "t");
    let t = b.add_tab("Vault").unwrap();
    b.file(coat(), Some(t), None);
    b.file(ItemKey::Dye(26), Some(t), Some(0));
    assert_eq!(b.tabs[t].items, vec![ItemKey::Dye(26), coat()]);
    assert!(b.carry(ItemKey::Dye(26)));
    assert_eq!(b.count(ItemKey::Dye(26)), 3, "carried still counts");
    b.swap_carried(0, 27);
    assert_eq!(b.carried[27], Some(ItemKey::Dye(26)));
    assert!(b.deposit(27));
    assert_eq!(b.items[&ItemKey::Dye(26)], 3);
    for _ in 0..items::MAX_TABS {
        b.add_tab("x");
    }
    assert_eq!(b.tabs.len(), items::MAX_TABS);
    // the new events survive the trip
    for e in [
        HubEv::Ids { list: vec![(0, HOST_ID), (3, 0)] },
        HubEv::Cmd { who: 2, text: "announce Raid at eight".into() },
        HubEv::GiftOffer { gid: 9, from: 1, to: 2, item: coat().to_bytes(), n: 3, create: false },
        HubEv::GiftDone { gid: 9, from: 1, to: 2, item: coat().to_bytes(), n: 3, ok: false, why: "they said no thanks".into() },
        HubEv::Move { who: 2, pos: v3(1.0, 2.0, 3.0) },
        HubEv::World { time: 2, weather: 1 },
    ] {
        assert_eq!(HubEv::from_bytes(&e.to_bytes()), Some(e));
    }
    for d in [Direct::Grant { item: coat(), n: 2 }, Direct::Reset, Direct::Roster { id: OWNER, on: true }, Direct::GiftSeal { gid: 5 }] {
        assert_eq!(Direct::from_bytes(&d.to_bytes()), Some(d));
    }
    assert_eq!(Direct::from_bytes(b"nope"), None);
}
