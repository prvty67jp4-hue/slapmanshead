//! The Varrow Hub's tests: sixteen players and nobody hurt, seats, challenges into the Court Pits
//! and back, the game tables, the Rooftop Run, the aim rooms' scores, and what goes over the
//! network (looks, emotes, seats, challenges, tables) arriving the same on every computer.
#![cfg(test)]

use crate::boards::{self, AnyMove, CMove, Level, Outcome};
use crate::game::*;
use crate::hub::*;
use crate::math::*;
use crate::varrow;

fn run(g: &mut Game, secs: f32, inp: &Input) {
    for _ in 0..(secs / TICK) as usize {
        g.tick(inp);
    }
}

fn wait(g: &mut Game, secs: f32) {
    for _ in 0..(secs / TICK) as usize {
        let i = idle(g);
        g.tick(&i);
    }
}

fn idle(g: &Game) -> Input {
    Input { yaw: g.f[g.local].yaw, ..Default::default() }
}

#[test]
fn a_hub_holds_sixteen_and_nobody_gets_hurt() {
    let _quiet = crate::heavy();
    let mut g = Game::hub("Me", 15, Kind::Offline);
    assert_eq!(g.present().len(), HUB_PLAYERS);
    assert_eq!(g.world.map, crate::world::MapId::Varrow);
    // everyone has a look, and the regulars wear a set
    assert!(g.f[1].hub.look.items.iter().any(|&i| i != 0));
    // swing and shoot at a regular standing right in front: nothing happens to them
    let near = g.f[0].pos + v3(0.0, 0.0, 1.5);
    g.f[1].respawn_at(near, 0.0);
    let mut inp = Input { yaw: 0.0, fire: true, fire_pressed: true, ..Default::default() };
    for _ in 0..200 {
        g.tick(&inp);
        inp.fire_pressed = !inp.fire_pressed;
    }
    assert!(g.f.iter().filter(|f| f.present).all(|f| f.alive && f.hp >= MAX_HP), "nobody hurt in the hub");
    assert!(!g.is_enemy(0, 1));
    // the hub never ends
    wait(&mut g, 30.0);
    assert_eq!(g.mt.phase, Phase::Fight);
}

#[test]
fn seats_are_taken_one_at_a_time_and_walking_gets_you_up() {
        let _quiet = crate::heavy();
    let mut g = Game::hub("Me", 1, Kind::Offline);
    let (p, _) = seat_pose(0).unwrap();
    g.f[0].respawn_at(v3(p.x, 0.0, p.z) + v3(0.4, 0.0, 0.0), 0.0);
    assert!(g.hub_sit_toggle());
    assert_eq!(g.f[0].hub.seat, 0);
    assert_eq!(g.f[0].hub.emote, Emote::Sit as u8);
    wait(&mut g, 1.0);
    assert_eq!(g.f[0].hub.seat, 0, "still sitting");
    // someone else can't sit there
    g.f[1].respawn_at(v3(p.x, 0.0, p.z) + v3(0.3, 0.0, 0.3), 0.0);
    assert_ne!(free_seat_near(&g, g.f[1].pos, 0.6, 1), Some(0));
    // walking off stands you up
    let walk = Input { fwd: 1.0, yaw: g.f[0].yaw + std::f32::consts::PI, ..Default::default() };
    run(&mut g, 1.0, &walk);
    assert_eq!(g.f[0].hub.seat, SEAT_NONE);
    assert_ne!(g.f[0].hub.emote, Emote::Sit as u8);
}

#[test]
fn emotes_play_and_end_when_you_move() {
        let _quiet = crate::heavy();
    let mut g = Game::hub("Me", 0, Kind::Offline);
    g.hub_emote(Emote::Wave);
    assert_eq!(g.f[0].hub.emote, Emote::Wave as u8);
    wait(&mut g, 3.0);
    assert_eq!(g.f[0].hub.emote, 0, "a wave ends by itself");
    g.hub_emote(Emote::Dance);
    wait(&mut g, 5.0);
    assert_eq!(g.f[0].hub.emote, Emote::Dance as u8, "dancing goes on");
    let walk = Input { fwd: 1.0, ..idle(&g) };
    run(&mut g, 0.5, &walk);
    assert_eq!(g.f[0].hub.emote, 0, "until you move");
}

/// The whole round trip: a challenge, a yes, into a pit, two rounds, out again.
#[test]
fn a_challenge_takes_both_into_a_pit_and_back_out() {
    let _quiet = crate::heavy();
    let mut g = Game::hub("Me", 2, Kind::Offline);
    g.hub_challenge(1, DuelKind::Duel);
    let c = g.hub.as_ref().unwrap().challenges.first().copied().expect("the challenge is up");
    assert_eq!((c.from, c.to), (0, 1));
    // (the regular says yes)
    g.hub_remote(1, HubEv::Answer { id: c.id, by: 1, accept: true });
    let h = g.hub.as_ref().unwrap();
    assert!(h.challenges.is_empty());
    let d = h.pits[0].expect("the duel is on in the west pit");
    assert_eq!(d.phase, PitPhase::Intro);
    // both are in the pit, apart, frozen until the bell
    for s in [0usize, 1] {
        let p = g.f[s].pos;
        let pit = varrow::PITS[0];
        assert!(p.x > pit.x.0 && p.x < pit.x.1 && p.z > pit.z.0 && p.z < pit.z.1, "{s} in the pit at {p:?}");
    }
    assert!(g.hub_frozen(0) && !g.is_enemy(0, 1));
    // the regular watching isn't in it, and nobody outside can be hurt
    assert!(!g.is_enemy(0, 2) && !g.is_enemy(1, 2));
    wait(&mut g, PIT_INTRO + 0.2);
    assert_eq!(g.hub.as_ref().unwrap().pits[0].unwrap().phase, PitPhase::Fight);
    assert!(g.is_enemy(0, 1) && !g.hub_frozen(0));
    // round one to us, round two to us: it's over
    for round in 0..2 {
        g.authority_hit(1, Some(0), 999.0, V3::ZERO, 0.0, false, Weapon::Katana as u8);
        wait(&mut g, 0.2);
        let d = g.hub.as_ref().unwrap().pits[0].unwrap();
        assert_eq!(d.wins, [round + 1, 0]);
        if round == 0 {
            assert_eq!(d.phase, PitPhase::Break);
            wait(&mut g, PIT_BREAK + 0.2);
            assert!(g.f[1].alive && g.f[1].hp >= MAX_HP, "the next round starts fresh");
        }
    }
    assert_eq!(g.hub.as_ref().unwrap().pits[0].unwrap().phase, PitPhase::Over);
    wait(&mut g, PIT_OVER + 0.5);
    let h = g.hub.as_ref().unwrap();
    assert!(h.pits[0].is_none(), "the pit is free again");
    assert!(h.results.iter().any(|r| r.contains("Me beat") && r.contains("2-0")), "{:?}", h.results);
    assert_eq!(h.earned.pit_wins, 1);
    for s in [0usize, 1] {
        assert_eq!(g.f[s].hub.pit, 255);
        assert!(g.f[s].pos.z < 14.0 && g.f[s].alive, "{s} back on the plaza");
    }
    assert!(!g.is_enemy(0, 1));
}

#[test]
fn a_no_is_a_no_and_open_challenges_go_on_the_board() {
        let _quiet = crate::heavy();
    let mut g = Game::hub("Me", 2, Kind::Offline);
    g.hub_challenge(1, DuelKind::Ledger);
    let c = g.hub.as_ref().unwrap().challenges[0];
    g.hub_remote(1, HubEv::Answer { id: c.id, by: 1, accept: false });
    let h = g.hub.as_ref().unwrap();
    assert!(h.challenges.is_empty() && h.pits.iter().all(|p| p.is_none()));
    assert!(h.results.iter().any(|r| r.contains("turned down")));
    // someone else can't answer a challenge meant for another
    g.hub_challenge(1, DuelKind::Duel);
    let c = g.hub.as_ref().unwrap().challenges[0];
    g.hub_remote(2, HubEv::Answer { id: c.id, by: 2, accept: true });
    assert!(g.hub.as_ref().unwrap().pits[0].is_none());
    g.hub_remote(0, HubEv::Answer { id: c.id, by: 0, accept: false });
    // an open one: anyone may take it, and a Ledger Duel gives both their gear
    g.hub_challenge(255, DuelKind::Ledger);
    let c = g.hub.as_ref().unwrap().challenges[0];
    assert_eq!(c.to, 255);
    g.hub_remote(2, HubEv::Answer { id: c.id, by: 2, accept: true });
    let d = g.hub.as_ref().unwrap().pits[0].expect("on");
    assert_eq!((d.who, d.kind), ([0, 2], DuelKind::Ledger));
    wait(&mut g, PIT_INTRO + 0.5);
    assert!(g.ledger_for(0) && g.ledger_for(2) && !g.ledger_for(1));
    assert!(g.f[0].kit.is_some() && g.f[2].kit.is_some(), "kitted out for the Ledger Duel");
}

#[test]
fn the_tables_play_by_the_rules_and_the_bot_answers() {
        let _quiet = crate::heavy();
    let mut g = Game::hub("Me", 0, Kind::Offline);
    let seat = TABLE_SEATS;
    let (p, _) = seat_pose(seat).unwrap();
    g.f[0].respawn_at(v3(p.x, varrow::DECK, p.z), 0.0);
    assert!(g.hub_sit_toggle());
    assert_eq!(g.f[0].hub.seat, seat);
    let t = &g.hub.as_ref().unwrap().tables[0];
    assert_eq!(t.seats[0], Sitter::Player(0));
    g.hub_table(0, TableAct::Bot(Level::Easy));
    assert_eq!(g.hub.as_ref().unwrap().tables[0].seats[1], Sitter::Bot(Level::Easy));
    // an illegal move does nothing; a legal one is played
    g.hub_table(0, TableAct::Move(AnyMove::Chess(CMove { from: 12, to: 36, promo: 0 })));
    assert!(g.hub.as_ref().unwrap().tables[0].last.is_empty());
    g.hub_table(0, TableAct::Move(AnyMove::Chess(CMove { from: 12, to: 28, promo: 0 })));
    assert_eq!(g.hub.as_ref().unwrap().tables[0].last, "e2-e4");
    // it isn't our move again until the bot has answered
    g.hub_table(0, TableAct::Move(AnyMove::Chess(CMove { from: 11, to: 27, promo: 0 })));
    assert_eq!(g.hub.as_ref().unwrap().tables[0].last, "e2-e4");
    for _ in 0..400 {
        let i = idle(&g);
        g.tick(&i);
        std::thread::sleep(std::time::Duration::from_millis(1));
        if g.hub.as_ref().unwrap().tables[0].board.turn() == 0 {
            break;
        }
    }
    let t = &g.hub.as_ref().unwrap().tables[0];
    assert_eq!(t.board.turn(), 0, "the bot moved");
    assert_ne!(t.last, "e2-e4");
    // getting up mid-game resigns it
    g.hub_sit_toggle();
    let t = &g.hub.as_ref().unwrap().tables[0];
    assert_eq!(t.outcome, Outcome::Win(1));
    assert_eq!(t.seats, [Sitter::Empty, Sitter::Empty]);
}

#[test]
fn winning_against_a_hard_bot_earns_the_pilgrim() {
        let _quiet = crate::heavy();
    let mut g = Game::hub("Me", 0, Kind::Offline);
    let (p, _) = seat_pose(TABLE_SEATS + 4).unwrap();
    g.f[0].respawn_at(v3(p.x, varrow::DECK, p.z), 0.0);
    g.hub_sit_toggle();
    g.hub_table(2, TableAct::Bot(Level::Hard));
    // a won position for red, one drop from four in a row
    {
        let t = &mut g.hub.as_mut().unwrap().tables[2];
        let mut c = boards::Connect4::new();
        for col in [0, 6, 1, 6, 2, 5] {
            c.play(col);
        }
        t.board = boards::Board::Connect4(c);
    }
    g.hub_table(2, TableAct::Move(AnyMove::Drop(3)));
    assert_eq!(g.hub.as_ref().unwrap().tables[2].outcome, Outcome::Win(0));
    let earned = g.hub.as_ref().unwrap().earned;
    assert!(earned.table_hard && earned.has(Unlock::TableHard));
}

#[test]
fn the_rooftop_run_counts_its_rings_in_order() {
        let _quiet = crate::heavy();
    let mut g = Game::hub("Me", 0, Kind::Offline);
    let at = |g: &mut Game, k: usize| {
        let (x, y, z) = varrow::RUN_RINGS[k];
        let p = v3(x, y - 0.9, z);
        g.f[0].pos = p;
        g.f[0].prev_pos = p;
        g.f[0].vel = V3::ZERO;
    };
    // skipping a ring: no finish
    for k in [0usize, 1, 3, 2, 6] {
        at(&mut g, k);
        let i = idle(&g);
        g.tick(&i);
        wait(&mut g, 1.0);
    }
    assert!(g.hub.as_ref().unwrap().run.last.is_none());
    // in order: a time, a best, a ghost
    let mut g = Game::hub("Me", 0, Kind::Offline);
    for k in 0..varrow::RUN_RINGS.len() {
        at(&mut g, k);
        let i = idle(&g);
        g.tick(&i);

        if k + 1 < varrow::RUN_RINGS.len() {
            wait(&mut g, 2.0);
        }
    }
    let h = g.hub.as_ref().unwrap();
    let (t, best) = h.run.last.expect("finished");
    assert!(best && (t - 12.0).abs() < 0.3, "{t}");
    assert!(h.ghost.len() > 100 && h.earned.rooftop);
    assert_eq!(h.boards[BOARD_RUN as usize][0].0, "Me");
    // the ghost runs with the next attempt
    let ghost = h.ghost.clone();
    let mut g2 = Game::hub("Me", 0, Kind::Offline);
    g2.hub.as_mut().unwrap().ghost = ghost.clone();
    at(&mut g2, 0);
    let i = idle(&g2);
    g2.tick(&i);
    wait(&mut g2, 1.0);
    let gh = g2.hub_ghosts();
    assert_eq!(gh.len(), 1);
    assert!(gh[0].ghost > 0.0);
    let saved = ghost_from_text(&ghost_to_text(&ghost));
    assert_eq!(saved.len(), ghost.len());
}

/// The course can be run: a runner with the moves anyone has (run, jump, dash) follows the
/// route (the rings, and the edges to jump from) and gets round in good time.
#[test]
fn the_rooftop_run_can_be_run() {
    let _quiet = crate::heavy();
    // (where to head, and what to do on getting within reach: 0 just pass, 1 jump, 2 jump
    // and dash in the air)
    let route: [(f32, f32, f32, u8); 15] = [
        (-17.0, 0.0, -19.0, 0),
        (-19.5, 1.2, -17.3, 0),
        (-19.5, 2.2, -15.9, 0),
        (-19.5, 3.2, -14.5, 0),
        (-21.5, 4.2, -13.5, 0),
        (-27.0, 4.2, -2.0, 0),
        (-28.0, 4.2, 7.4, 1),
        (-28.0, 5.0, 12.0, 0),
        (-28.0, 5.0, 13.0, 2),
        (-29.0, 5.0, 22.0, 0),
        (-27.6, 5.0, 21.7, 0),
        (-23.5, 4.5, 21.7, 0),
        (-14.0, 4.5, 21.8, 0),
        (0.0, 4.5, 21.6, 0),
        (0.0, 0.0, 9.5, 0),
    ];
    let mut g = Game::hub("Runner", 0, Kind::Offline);
    g.f[0].respawn_at(v3(-15.5, 0.0, -20.5), -0.6);
    let mut k = 0usize;
    let mut stuck = 0.0f32;
    let mut last = g.f[0].pos;
    let mut dash_armed = false;
    let mut ticks = 0;
    while g.hub.as_ref().unwrap().run.last.is_none() && ticks < (90.0 / TICK) as usize {
        let (x, y, z, act) = route[k.min(route.len() - 1)];
        let f = &g.f[0];
        let to = v3(x, y, z) - f.pos;
        let flat = to.flat();
        let mut inp = Input { fwd: 1.0, yaw: flat.x.atan2(flat.z), ..Default::default() };
        // up onto something within reach: jump
        if f.on_ground && to.y > 0.4 && flat.len() < 2.2 {
            inp.jump = true;
        }
        if flat.len() < 0.7 && (to.y.abs() < 1.2 || act > 0) {
            if act > 0 && f.on_ground {
                inp.jump = true;
                dash_armed = act == 2;
            }
            k += 1;
        }
        if dash_armed && !f.on_ground && f.vel.y < 2.0 {
            inp.dash = true;
            dash_armed = false;
        }
        if stuck > 0.4 && f.on_ground {
            inp.jump = true;
        }
        g.tick(&inp);
        stuck = if (g.f[0].pos - last).len() < 0.01 { stuck + TICK } else { 0.0 };
        last = g.f[0].pos;
        if k >= 5 && k <= 13 && g.f[0].pos.y < 2.0 {
            panic!("fell off heading for step {k} {:?}, at {:?}", route[k.min(route.len() - 1)], g.f[0].pos);
        }
        ticks += 1;
    }
    let h = g.hub.as_ref().unwrap();
    let (t, _) = h.run.last.unwrap_or_else(|| panic!("didn't finish: next ring {} at step {k} ({:?})", h.run.next, g.f[0].pos));
    assert!(t < 45.0, "{t}");
}
#[test]
fn every_hub_message_survives_the_wire() {
    let mut o = Outfit::default().with_set(HERALD);
    o.dyes[4] = [3, 7, 11];
    let duel = PitDuel { who: [3, 9], kind: DuelKind::Ledger, wins: [1, 2], first_to: 3, phase: PitPhase::Break, t: 1.25 };
    let all = vec![
        HubEv::Look { who: 5, look: o },
        HubEv::Emote { who: 15, emote: Emote::Laugh as u8 },
        HubEv::Sit { who: 2, seat: TABLE_SEATS + 3 },
        HubEv::Challenge { from: 1, to: 255, kind: 1 },
        HubEv::Answer { id: 513, by: 4, accept: true },
        HubEv::Challenges { list: vec![(1, 2, 3, 0), (9, 4, 255, 1)] },
        HubEv::Pit { pit: 1, duel: Some(duel) },
        HubEv::Pit { pit: 0, duel: None },
        HubEv::Table { table: 2, who: 7, act: TableAct::Move(AnyMove::Drop(4)) },
        HubEv::Table { table: 1, who: 7, act: TableAct::Move(AnyMove::Checkers(boards::DMove(vec![20, 29, 38]))) },
        HubEv::Table { table: 0, who: 7, act: TableAct::Bot(Level::Hard) },
        HubEv::TableState { table: 3, ver: 77, board: boards::Board::new(boards::GameKind::Chess).to_bytes(), seats: [1, 102], outcome: Outcome::Win(1).to_u8(), last: "Nb1-c3".into() },
        HubEv::Score { who: 3, board: 2, value: 4321 },
        HubEv::Board { board: BOARD_RUN, rows: vec![("Wren".into(), 41230), ("Odo".into(), 45001)] },
        HubEv::AimDuo { from: 1, to: 2, drill: 3 },
        HubEv::AimGo { a: 1, b: 2, drill: 4, seed: 0xdead },
        HubEv::Note { text: "Wren beat Odo 2-1".into() },
    ];
    for e in all {
        let back = crate::net::ev_round_trip(&Ev::Hub(e.clone()));
        assert_eq!(format!("{back:?}"), format!("{:?}", Some(Ev::Hub(e.clone()))), "{e:?}");
    }
    // junk is refused
    assert_eq!(HubEv::from_bytes(&[1, 2, 3]), None);
    assert_eq!(HubEv::from_bytes(&[99]), None);
}

#[test]
fn looks_emotes_and_seats_reach_the_other_computers() {
        let _quiet = crate::heavy();
    // a host and a player who joined it (the network's own part is tested in net.rs: here the
    // messages are handed across as the network would)
    let mut host = Game::hub("Host", 0, Kind::Host);
    host.add_player(1, "Guest", false, None, 0);
    let mut guest = Game::new(Kind::Client, 1, Mode::Hub, 0);
    guest.set_map(crate::world::MapId::Varrow);
    guest.hub = Some(Box::new(Hub::new()));
    guest.add_player(0, "Host", false, None, 0);
    guest.add_player(1, "Guest", true, None, 0);
    let o = Outfit::default().with_set(SMUGGLER);
    guest.hub_wear(o);
    guest.hub_emote(Emote::Salute);
    for ev in guest.out_events.drain(..).collect::<Vec<_>>() {
        let ev = crate::net::ev_round_trip(&ev).unwrap();
        host.handle_remote_event(1, ev);
    }
    assert_eq!(host.f[1].hub.look, o);
    assert_eq!(host.f[1].hub.emote, Emote::Salute as u8);
    // the host's own look, its regulars' and its tables reach the guest
    host.hub_wear(Outfit::default().with_set(PILGRIM));
    host.host_sync_all();
    for ev in host.out_events.drain(..).collect::<Vec<_>>() {
        let ev = crate::net::ev_round_trip(&ev).unwrap();
        guest.handle_remote_event(0, ev);
    }
    assert_eq!(guest.f[0].hub.look, Outfit::default().with_set(PILGRIM));
    // and the compact state the host sends for each fighter keeps their emote and seat
    host.f[1].hub.seat = 4;
    let st = crate::net::fstate_hub_round_trip(&host.f[1]).unwrap();
    assert_eq!((st.emote, st.seat), (Emote::Salute as u8, 4));
    assert!((st.pos - host.f[1].pos).len() < 1e-4);
}

#[test]
fn challenges_and_tables_go_through_the_host() {
        let _quiet = crate::heavy();
    let mut host = Game::hub("Host", 1, Kind::Host);
    host.add_player(2, "Guest", false, None, 0);
    // the guest challenges the host
    host.handle_remote_event(2, Ev::Hub(HubEv::Challenge { from: 2, to: 0, kind: 0 }));
    let c = host.hub.as_ref().unwrap().challenges[0];
    assert_eq!((c.from, c.to), (2, 0));
    assert_eq!(host.hub.as_ref().unwrap().asked.map(|a| a.id), Some(c.id), "the host is asked");
    let sent: Vec<Ev> = host.out_events.drain(..).collect();
    assert!(sent.iter().any(|e| matches!(e, Ev::Hub(HubEv::Challenges { list }) if list.len() == 1)));
    // a player can't speak for someone else: the host answers for itself only
    host.handle_remote_event(2, Ev::Hub(HubEv::Answer { id: c.id, by: 0, accept: true }));
    assert!(host.hub.as_ref().unwrap().pits[0].is_none());
    host.hub_answer(c.id, true);
    assert!(host.hub.as_ref().unwrap().pits[0].is_some());
    let sent: Vec<Ev> = host.out_events.drain(..).collect();
    assert!(sent.iter().any(|e| matches!(e, Ev::Hub(HubEv::Pit { duel: Some(_), .. }))));
    // the tables: a guest's move only counts when it's theirs to make
    let (p, _) = seat_pose(TABLE_SEATS + 2).unwrap();
    host.f[2].pos = v3(p.x, varrow::DECK, p.z);
    host.handle_remote_event(2, Ev::Hub(HubEv::Sit { who: 2, seat: TABLE_SEATS + 2 }));
    assert_eq!(host.hub.as_ref().unwrap().tables[1].seats[0], Sitter::Player(2));
    let m = boards::Checkers::new().legal()[0].clone();
    host.handle_remote_event(2, Ev::Hub(HubEv::Table { table: 1, who: 2, act: TableAct::Move(AnyMove::Checkers(m)) }));
    assert!(host.hub.as_ref().unwrap().tables[1].last.is_empty(), "no game until both sides are taken");
    host.handle_remote_event(2, Ev::Hub(HubEv::Table { table: 1, who: 2, act: TableAct::Bot(Level::Medium) }));
    let m = boards::Checkers::new().legal()[0].clone();
    host.handle_remote_event(2, Ev::Hub(HubEv::Table { table: 1, who: 2, act: TableAct::Move(AnyMove::Checkers(m)) }));
    assert!(!host.hub.as_ref().unwrap().tables[1].last.is_empty());
    let sent: Vec<Ev> = host.out_events.drain(..).collect();
    let st = sent.iter().rev().find_map(|e| match e {
        Ev::Hub(HubEv::TableState { table: 1, board, .. }) => Some(board.clone()),
        _ => None,
    });
    let b = boards::Board::from_bytes(&st.expect("the table goes out")).unwrap();
    assert_eq!(b.turn(), 1);
}

#[test]
fn outfits_keep_only_what_is_earned_and_in_its_slot() {
    let earned = Earned::default();
    let o = Outfit::default().with_set(HERALD).with_set(RIG);
    let ok = o.checked(&earned);
    assert_eq!(ok.items[Slot::Head as usize], 1, "the Rig's helm is always yours");
    assert_eq!(ok.items[Slot::Cape as usize], 0, "the herald's cape needs mastery 5");
    let rich = Earned { mastery: 5, rooftop: true, table_hard: true, pit_wins: 0, ..Default::default() };
    let all = Outfit::default().with_set(HERALD).checked(&rich);
    assert!(all.items.iter().filter(|&&i| i != 0).count() >= 7);
    // a look in the wrong slot is taken off
    let mut bad = Outfit::default();
    bad.items[Slot::Feet as usize] = 1;
    assert_eq!(bad.checked(&rich).items[Slot::Feet as usize], 0);
    // ten outfits saved and read back
    let mut s = HubSave { look: all, ..Default::default() };
    for k in 0..12 {
        s.outfits.push((format!("Fit {k}"), Outfit::default().with_set([RIG, HERALD, SMUGGLER, PILGRIM][k % 4])));
    }
    s.earned = rich;
    s.aim_best = [1, 2, 3, 4, 5];
    s.run_best = 41_230;
    let back = HubSave::from_text(&s.to_text());
    assert_eq!(back.look, s.look);
    assert_eq!(back.outfits.len(), OUTFIT_SAVES);
    assert_eq!(back.outfits[3], s.outfits[3]);
    assert_eq!((back.earned.rooftop, back.earned.table_hard), (true, true));
    assert_eq!(back.aim_best, s.aim_best);
    assert_eq!(back.run_best, 41_230);
    // every set has a piece for every slot it claims, and the new outfits have seven each
    for set in [HERALD, SMUGGLER, PILGRIM] {
        assert_eq!(LOOKS.iter().filter(|l| l.set == set).count(), 7, "{set}");
    }
}

#[test]
fn an_aim_room_score_reaches_the_gallery_board() {
    let _quiet = crate::heavy();
    let mut g = Game::hub("Me", 0, Kind::Offline);
    g.start_aim(crate::hub_aim::Drill::Flicks, 0, 7, None);
    assert_eq!(g.f[0].weapon, Weapon::Revolver);
    let stand = varrow::LANES[0].stand;
    assert!((g.f[0].pos - stand).flat().len() < 0.01);
    // shoot every target as it comes up
    for _ in 0..(120.0 / TICK) as usize {
        let tg = g.hub.as_ref().unwrap().aim.as_ref().and_then(|a| a.target);
        let mut inp = idle(&g);
        if let Some(t) = tg {
            let eye = g.shot_ray(0).0;
            let d = (t.pos - eye).norm();
            inp.yaw = d.x.atan2(d.z);
            inp.pitch = d.y.asin();
            inp.fire = true;
            inp.fire_pressed = true;
        }
        g.tick(&inp);
        if g.hub.as_ref().unwrap().aim.as_ref().is_some_and(|a| a.done) {
            break;
        }
    }
    let h = g.hub.as_ref().unwrap();
    let a = h.aim.as_ref().unwrap();
    assert!(a.done && a.hits >= 25, "{} hits", a.hits);
    assert!(h.aim_best[0] > 0 && h.boards[0].first().is_some_and(|r| r.0 == "Me"));
    assert!(h.aim_result.is_some());
}

/// Name tags in a crowd: nearest first, none overlapping another, smaller and fainter further
/// off; an emote's name shows only for its first moments.
#[test]
fn name_tags_stack_and_fade_in_a_crowd() {
    let mut g = Game::hub("You", 15, Kind::Offline);
    // everyone bunched up 4-9 m in front of the camera
    for k in 1..g.f.len() {
        let x = ((k % 5) as f32 - 2.0) * 0.45;
        let z = 4.0 + (k / 5) as f32 * 1.6;
        g.f[k].pos = v3(x, 0.0, -12.0 + z);
        g.f[k].hub.emote = 0;
    }
    g.f[0].pos = v3(0.0, 0.0, -14.0);
    g.f[3].hub.emote = Emote::Wave as u8;
    g.f[3].hub.emote_t = g.time - 0.5;
    g.f[4].hub.emote = Emote::Wave as u8;
    g.f[4].hub.emote_t = g.time - 10.0;
    let rect = eframe::egui::Rect::from_min_size(eframe::egui::Pos2::ZERO, eframe::egui::Vec2::new(1280.0, 720.0));
    let cam = crate::render::Cam::new(v3(0.0, 1.7, -16.0), 0.0, -0.05, 70.0, rect);
    let tags = crate::App::name_tags(&g, &cam, 1.0);
    assert!(tags.len() >= 6, "most of the crowd is named: {}", tags.len());
    let rects: Vec<eframe::egui::Rect> = tags.iter().map(|(p, size, _, name, _, _, _)| {
        let w = name.chars().count() as f32 * size * 0.55 + 8.0;
        eframe::egui::Rect::from_min_max(eframe::egui::Pos2::new(p.x - w / 2.0, p.y - size * 1.15), eframe::egui::Pos2::new(p.x + w / 2.0, p.y))
    }).collect();
    for i in 0..rects.len() {
        for j in i + 1..rects.len() {
            assert!(!rects[i].shrink(1.0).intersects(rects[j].shrink(1.0)), "{} and {} overlap", tags[i].3, tags[j].3);
        }
    }
    // further off: never bigger or brighter than nearer ones (the list is nearest first)
    assert!(tags.first().unwrap().1 >= tags.last().unwrap().1);
    assert!(tags.first().unwrap().2 >= tags.last().unwrap().2);
    // a fresh emote is named, a stale one isn't
    let by = |n: &str| tags.iter().find(|t| t.3 == n).map(|t| t.4.is_some());
    assert_eq!(by(&g.f[3].name.clone()), Some(true));
    assert_eq!(by(&g.f[4].name.clone()).unwrap_or(false), false);
}
