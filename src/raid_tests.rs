//! The Ashen Vault's tests: going in from the gate (bots filling a solo party), scaling by party
//! size and Sealed Writ, revives and wipes, every one of the Ashblood Regent's mechanics (his four
//! moves' shapes, the blood waves' gaps, the four spikes blocked or hidden from, the jab chain and
//! its stagger, the ash-blood pools), the loot from a boss's chest, the raid's snapshot over the
//! network, a player joining from another computer and coming back after a drop, and bots
//! clearing the whole vault on normal.
#![cfg(test)]

use crate::game::*;
use crate::math::*;
use crate::raid::{self, Cause, FoeKind, Pool, RaidMsg, Stage, Tele, TeleKind, Wave, MV_JAB, NO_MOVE, RM_JABS, RM_SPIKES, RM_STAGGER, RM_WAVES};
use crate::vault::{self, ROOMS, VY};

fn tick(g: &mut Game, secs: f32, mut f: impl FnMut(&mut Game) -> Input) {
    for _ in 0..(secs / TICK).round() as usize {
        let i = f(g);
        g.tick(&i);
    }
}

fn idle(g: &mut Game) -> Input {
    Input { yaw: g.f[g.local].yaw, ..Default::default() }
}

/// A hub with `regulars`, me in the Vault Gate's circle, gone in with `bots` bots.
fn in_vault(regulars: usize, bots: usize, hard: bool) -> Game {
    let mut g = Game::hub("Me", regulars, Kind::Offline);
    let (x, z, _) = vault::GATE;
    g.f[0].respawn_at(v3(x, 0.0, z), 0.0);
    if hard {
        if let Some(h) = g.hub.as_mut() {
            h.earned.raid.cleared = true;
        }
    }
    raid::gate_start(&mut g, 0, hard, bots);
    assert!(g.raid.is_some(), "the vault starts");
    g
}

/// Straight into a room's fight (everyone placed at its entry).
fn to_room(g: &mut Game, room: usize) {
    let r = g.raid.as_mut().unwrap();
    r.room = room;
    r.setup_room();
    r.stage = Stage::Fight;
    r.stage_t = 0.0;
    r.place = r.place.wrapping_add(1);
    tick(g, 0.1, idle);
}

fn regent_idx(g: &Game) -> usize {
    g.raid.as_ref().unwrap().foes.iter().position(|f| f.kind == FoeKind::Regent).unwrap()
}

/// Put me somewhere (and the boss somewhere), facing it.
fn stand(g: &mut Game, me: V3, boss: V3) {
    let k = regent_idx(g);
    let r = g.raid.as_mut().unwrap();
    r.foes[k].pos = boss;
    r.foes[k].shown = boss;
    let d = (boss - me).flat();
    g.f[0].respawn_at(me, d.x.atan2(d.z));
}

#[test]
fn a_solo_player_goes_in_with_bots_and_lands_in_the_first_room() {
    let _q = crate::heavy();
    let mut g = in_vault(3, 0, false);
    let r = g.raid.as_ref().unwrap();
    // alone: a bot comes along so there are at least two
    assert_eq!(r.members.len(), 2);
    assert!(r.members.iter().any(|m| m.bot));
    assert_eq!((r.room, r.stage), (0, Stage::Intro));
    tick(&mut g, 0.2, idle);
    assert!(vault::in_vault(g.f[0].pos) && ROOMS[0].contains(g.f[0].pos, 1.0), "placed in the Warden's hall: {:?}", g.f[0].pos);
    // three bots: a full party of four
    let g4 = in_vault(5, 3, false);
    assert_eq!(g4.raid.as_ref().unwrap().members.len(), 4);
    // a second party can't go in while one is inside
    let mut g2 = in_vault(5, 1, false);
    let before = g2.raid.as_ref().unwrap().members.len();
    raid::gate_start(&mut g2, 0, false, 3);
    assert_eq!(g2.raid.as_ref().unwrap().members.len(), before);
}

#[test]
fn health_scales_with_the_party_and_sealed_writ_is_harder() {
    let mk = |n: usize, hard: bool| {
        let members = (0..n).map(|k| raid::Member { slot: k as u8, name: format!("p{k}"), bot: false, downed: false, revive: 0.0, reviver: 255, threat: 0.0, dealt: 0.0, connected: true, holding: false, calm: 0.0 }).collect();
        let mut r = raid::Raid::new(members, hard, 7);
        r.room = 3;
        r.setup_room();
        r
    };
    let hp = |n, h| mk(n, h).boss().unwrap().max;
    assert!((hp(2, false) / hp(4, false) - raid::scale(2)).abs() < 1e-3);
    assert!((hp(3, false) / hp(4, false) - raid::scale(3)).abs() < 1e-3);
    assert!((hp(4, true) / hp(4, false) - 1.3).abs() < 1e-3);
    assert_eq!(mk(4, false).revives_left, 3);
    assert_eq!(mk(4, true).revives_left, 2);
}

#[test]
fn holding_e_on_a_downed_ally_for_three_seconds_revives_them() {
    let _q = crate::heavy();
    let mut g = in_vault(3, 1, false);
    to_room(&mut g, 3);
    let bot = g.raid.as_ref().unwrap().members.iter().find(|m| m.bot).unwrap().slot as usize;
    raid::hurt(&mut g, bot, 999.0, Cause::Jab);
    assert!(g.raid.as_ref().unwrap().is_downed(bot));
    let at = g.f[bot].pos + v3(0.8, 0.0, 0.0);
    g.f[0].respawn_at(at, 0.0);
    // not long enough: still down
    tick(&mut g, 1.5, |g| {
        g.f[0].hp = MAX_HP;
        g.raid_use(true, false);
        idle(g)
    });
    assert!(g.raid.as_ref().unwrap().is_downed(bot));
    tick(&mut g, 1.8, |g| {
        g.f[0].hp = MAX_HP;
        g.raid_use(true, false);
        idle(g)
    });
    let r = g.raid.as_ref().unwrap();
    assert!(!r.is_downed(bot), "revived after 3 s");
    assert_eq!(r.revives_left, 2);
}

#[test]
fn a_full_wipe_restarts_the_room() {
    let _q = crate::heavy();
    let mut g = in_vault(3, 1, false);
    to_room(&mut g, 3);
    let k = regent_idx(&g);
    g.raid.as_mut().unwrap().foes[k].hp *= 0.5;
    let slots: Vec<usize> = g.raid.as_ref().unwrap().members.iter().map(|m| m.slot as usize).collect();
    for s in slots {
        raid::hurt(&mut g, s, 999.0, Cause::Arc);
    }
    tick(&mut g, 0.2, idle);
    assert_eq!(g.raid.as_ref().unwrap().stage, Stage::Wipe);
    tick(&mut g, 5.0, idle);
    let r = g.raid.as_ref().unwrap();
    assert_eq!((r.room, r.wipes), (3, 1));
    assert!(r.members.iter().all(|m| !m.downed));
    let b = r.boss().unwrap();
    assert!((b.hp - b.max).abs() < 1.0, "the boss is whole again");
    assert_eq!(r.recap.len(), 2, "the recap names both falls");
}

#[test]
fn the_regents_four_moves_each_have_one_safe_answer() {
    let at = v3(25.0, VY, 1.0);
    let fwd = v3(0.0, 0.0, 1.0);
    let side = v3(1.0, 0.0, 0.0);
    let t = |kind, pos, r, len| Tele { kind, pos, dir: 0.0, r, len, t: 0.0, total: 1.0, dmg: 0.0, cause: Cause::Jab, hit: 0 };
    // the red flash jab: in front is hit, stepping back out of reach is safe
    let jab = t(TeleKind::Cone, at, 4.2, 0.55);
    assert!(jab.covers(at + fwd * 2.5));
    assert!(!jab.covers(at + fwd * 5.0));
    // the blood ring: everywhere round him except right up against him
    let ring = t(TeleKind::Donut, at, 9.0, 0.0);
    assert!(ring.covers(at + fwd * 4.0) && ring.covers(at - side * 6.0));
    assert!(!ring.covers(at + fwd * 1.0), "hugging him is safe");
    // the lance thrust: a line in front and behind; a sidestep is safe
    let thrust = t(TeleKind::Line, at - fwd * 9.0, 1.1, 18.0);
    assert!(thrust.covers(at + fwd * 3.0) && thrust.covers(at - fwd * 3.0));
    assert!(!thrust.covers(at + side * 2.5));
    // the sweeping arc: all round him except directly behind
    let arc = t(TeleKind::Arc, at, 5.8, 0.6);
    assert!(arc.covers(at + fwd * 2.0) && arc.covers(at + side * 2.0) && arc.covers(at - side * 2.0));
    assert!(!arc.covers(at - fwd * 2.0), "behind him is safe");
}

#[test]
fn blood_waves_pass_through_their_gaps_only() {
    let w = Wave { axis: 0, pos: 20.0, gaps: [2.0, 7.0], ngaps: 2, hit: 0 };
    assert!(w.catches(v3(20.0, VY, -4.0)));
    assert!(!w.catches(v3(20.0, VY, 2.3)), "in a gap");
    assert!(!w.catches(v3(20.0, VY, 7.0)));
    assert!(!w.catches(v3(22.0, VY, -4.0)), "not where the wall is");
    // a step to the side in the wall's path (moving diagonally) leaves the gap
    assert!(w.catches(v3(20.0, VY, 2.0 + raid::GAP_HALF + 0.2)));
}

#[test]
fn the_giant_spikes_one_shot_unless_blocked_on_the_cue_or_hidden_from() {
    let _q = crate::heavy();
    let spikes = |g: &mut Game, t: f32| {
        let r = g.raid.as_mut().unwrap();
        r.regent.mode = RM_SPIKES;
        r.regent.vanished = false;
        r.regent.spikes = 1;
        r.regent.spike_t = t;
    };
    let centre = v3(vault::ROOF_CENTRE.0, VY, vault::ROOF_CENTRE.1);
    // standing in the open, doing nothing: down
    let mut g = in_vault(3, 1, false);
    to_room(&mut g, 3);
    stand(&mut g, centre + v3(0.0, 0.0, -4.0), centre);
    spikes(&mut g, 0.3);
    tick(&mut g, 0.5, idle);
    let r = g.raid.as_ref().unwrap();
    assert!(r.is_downed(0), "the spike lands");
    assert_eq!(r.recap.last().map(|x| x.1), Some(Cause::Spike));
    // blade out, guard raised on the cue: blocked
    let mut g = in_vault(3, 1, false);
    to_room(&mut g, 3);
    stand(&mut g, centre + v3(0.0, 0.0, -4.0), centre);
    tick(&mut g, 0.5, |g| Input { yaw: g.f[0].yaw, select: Some(0), ..Default::default() });
    spikes(&mut g, 0.6);
    tick(&mut g, 0.8, |g| {
        let t = g.raid.as_ref().unwrap().regent.spike_t;
        Input { yaw: g.f[0].yaw, guard: t < 0.4, ..Default::default() }
    });
    assert!(!g.raid.as_ref().unwrap().is_downed(0), "blocked on the cue");
    // guard held up far too early: the window has passed
    let mut g = in_vault(3, 1, false);
    to_room(&mut g, 3);
    stand(&mut g, centre + v3(0.0, 0.0, -4.0), centre);
    tick(&mut g, 0.3, |g| Input { yaw: g.f[0].yaw, select: Some(0), ..Default::default() });
    spikes(&mut g, 1.6);
    tick(&mut g, 1.9, |g| Input { yaw: g.f[0].yaw, guard: true, ..Default::default() });
    assert!(g.raid.as_ref().unwrap().is_downed(0), "a guard raised 1.6 s early doesn't count");
    // behind a chimney (from him): safe
    let mut g = in_vault(3, 1, false);
    to_room(&mut g, 3);
    let (cx, cz) = vault::CHIMNEYS[0];
    let ch = v3(cx, VY, cz);
    let behind = ch + (ch - centre).flat().norm() * (vault::CHIMNEY_HALF + 0.5);
    stand(&mut g, behind, centre);
    spikes(&mut g, 0.3);
    tick(&mut g, 0.5, idle);
    assert!(!g.raid.as_ref().unwrap().is_downed(0), "the chimney covers you");
}

#[test]
fn parrying_the_whole_jab_chain_staggers_him_and_a_miss_sends_the_waves_back() {
    let _q = crate::heavy();
    let centre = v3(vault::ROOF_CENTRE.0, VY, vault::ROOF_CENTRE.1);
    let chain = |g: &mut Game| {
        let r = g.raid.as_mut().unwrap();
        r.phase = 2;
        r.regent.mode = RM_JABS;
        r.regent.vanished = false;
        r.regent.chain_left = 5;
        r.regent.chain_ok = true;
        r.regent.mark = 0;
        r.regent.mv = NO_MOVE;
        r.regent.t = 0.3;
    };
    let mut g = in_vault(3, 1, false);
    to_room(&mut g, 3);
    stand(&mut g, centre + v3(0.0, 0.0, -2.0), centre);
    tick(&mut g, 0.3, |g| Input { yaw: g.f[0].yaw, select: Some(0), ..Default::default() });
    chain(&mut g);
    let mut jabs = 0;
    let mut seen = false;
    tick(&mut g, 6.0, |g| {
        let rg = g.raid.as_ref().unwrap().regent;
        if rg.mv == MV_JAB && !seen {
            jabs += 1;
        }
        seen = rg.mv == MV_JAB;
        g.f[0].hp = MAX_HP;
        Input { yaw: g.f[0].yaw, guard: rg.mv != NO_MOVE && rg.wind < 0.3, ..Default::default() }
    });
    assert_eq!(jabs, 5);
    assert_eq!(g.raid.as_ref().unwrap().regent.mode, RM_STAGGER, "every jab parried: staggered");
    // staggered, he takes double
    let k = regent_idx(&g);
    let before = g.raid.as_ref().unwrap().foes[k].hp;
    raid::hit_foe(&mut g, 0, k, 100.0);
    assert!((before - g.raid.as_ref().unwrap().foes[k].hp - 200.0).abs() < 0.5);
    // one jab let through: back to the waves
    let mut g = in_vault(3, 1, false);
    to_room(&mut g, 3);
    stand(&mut g, centre + v3(0.0, 0.0, -2.0), centre);
    chain(&mut g);
    tick(&mut g, 6.0, |g| {
        g.f[0].hp = MAX_HP;
        idle(g)
    });
    assert_eq!(g.raid.as_ref().unwrap().regent.mode, RM_WAVES);
}

#[test]
fn the_ash_blood_pools_switch_off_your_guard_and_abilities() {
    let _q = crate::heavy();
    let mut g = in_vault(3, 1, false);
    to_room(&mut g, 3);
    let p = g.f[0].pos;
    assert!(!raid::in_ash(&g, 0));
    g.raid.as_mut().unwrap().pools.push(Pool { kind: 3, pos: v3(p.x, VY, p.z), r: 1.6, max: 1.6, life: 30.0 });
    assert!(raid::in_ash(&g, 0));
    let inp = g.hub_input(0, Input { guard: true, ability: true, utility: true, ..Default::default() });
    assert!(!inp.guard && !inp.ability && !inp.utility);
    // and it drains you
    let hp = g.f[0].hp;
    tick(&mut g, 1.0, idle);
    assert!(g.f[0].hp < hp || g.raid.as_ref().unwrap().is_downed(0), "the pool drains");
}

#[test]
fn a_beaten_boss_leaves_a_chest_with_a_piece_from_its_table() {
    let _q = crate::heavy();
    let mut g = in_vault(3, 1, false);
    to_room(&mut g, 3);
    let k = regent_idx(&g);
    raid::hit_foe(&mut g, 0, k, 1e9);
    tick(&mut g, 0.2, idle);
    let r = g.raid.as_ref().unwrap();
    assert_eq!(r.stage, Stage::Cleared);
    let c = r.chest.as_ref().expect("a chest");
    assert!(!c.items.is_empty());
    let (common, rare) = raid::DROPS[3];
    for (item, _) in &r.mine {
        assert!(common.contains(item) || rare.contains(item));
    }
    let e = g.hub.as_ref().unwrap().earned.raid;
    assert!(r.mine.iter().all(|(i, _)| e.has(*i)), "kept in the hub save");
    // opening it (E near it)
    let cp = c.pos;
    g.f[0].respawn_at(cp + v3(0.0, 0.0, 1.0), 0.0);
    g.raid_use(false, true);
    assert!(g.raid.as_ref().unwrap().chest.as_ref().unwrap().open);
    // the bot gets nothing (loot is for players)
    let bot = g.raid.as_ref().unwrap().members.iter().find(|m| m.bot).unwrap().slot;
    assert!(g.raid.as_ref().unwrap().chest.as_ref().unwrap().items.iter().all(|x| x.0 != bot));
}

#[test]
fn the_raid_snapshot_round_trips() {
    let _q = crate::heavy();
    let mut g = in_vault(3, 2, true);
    to_room(&mut g, 3);
    {
        let r = g.raid.as_mut().unwrap();
        r.regent.mode = RM_WAVES;
        r.regent.vanished = true;
        r.regent.waves_left = 3;
        r.regent.mark = 2;
        r.waves.push(Wave { axis: 2, pos: -3.5, gaps: [18.0, 30.0], ngaps: 2, hit: 0 });
        r.pools.push(Pool { kind: 3, pos: v3(20.0, VY, 0.0), r: 1.2, max: 1.6, life: 20.0 });
        r.foes[0].hp = 12345.0;
    }
    let r = g.raid.as_ref().unwrap();
    let b = raid::encode(r);
    assert!(b.len() < 1100, "a snapshot fits one packet: {}", b.len());
    let d = raid::decode(&b, None).expect("decodes");
    assert_eq!((d.room, d.stage, d.hard, d.members.len()), (r.room, r.stage, r.hard, r.members.len()));
    assert_eq!((d.regent.mode, d.regent.vanished, d.regent.waves_left, d.regent.mark), (RM_WAVES, true, 3, 2));
    assert_eq!(d.waves.len(), 1);
    assert!((d.waves[0].pos + 3.5).abs() < 0.05 && (d.waves[0].gaps[1] - 30.0).abs() < 0.05 && d.waves[0].ngaps == 2);
    assert_eq!(d.pools.len(), 1);
    let (a, b) = (d.foes[0].hp / d.foes[0].max, r.foes[0].hp / r.foes[0].max);
    assert!((a - b).abs() < 0.001, "the boss's health share: {a} vs {b}");
}

/// Hand every message one way, as the network would.
fn carry(from: &mut Game, from_slot: usize, to: &mut Game) {
    for ev in from.out_events.drain(..).collect::<Vec<_>>() {
        if let Some(ev) = crate::net::ev_round_trip(&ev) {
            to.handle_remote_event(from_slot, ev);
        }
    }
}

#[test]
fn a_player_on_another_computer_joins_at_the_gate_hits_the_boss_and_can_come_back() {
    let _q = crate::heavy();
    let mut host = Game::hub("Host", 1, Kind::Host);
    host.add_player(2, "Guest", false, None, 0);
    let mut guest = Game::new(Kind::Client, 2, Mode::Hub, 0);
    guest.set_map(crate::world::MapId::Varrow);
    guest.hub = Some(Box::new(crate::hub::Hub::new()));
    guest.add_player(0, "Host", false, None, 0);
    guest.add_player(1, "Regular", false, None, 0);
    guest.add_player(2, "Guest", true, None, 0);
    let (x, z, _) = vault::GATE;
    host.f[0].respawn_at(v3(x - 0.5, 0.0, z), 0.0);
    host.f[2].respawn_at(v3(x + 0.5, 0.0, z), 0.0);
    // the guest presses E in the circle and asks for no bots: both of them go in
    raid::raid_msg(&mut guest, &RaidMsg::Gate { hard: false, bots: 0 });
    carry(&mut guest, 2, &mut host);
    let r = host.raid.as_ref().expect("the host starts the vault");
    assert_eq!(r.members.len(), 2);
    assert!(host.in_raid(0) && host.in_raid(2));
    // its snapshot reaches the guest
    carry(&mut host, 0, &mut guest);
    assert!(guest.raid.is_some() && guest.in_raid(2));
    // the guest hits the boss: the host takes it off
    {
        let r = host.raid.as_mut().unwrap();
        r.stage = Stage::Fight;
    }
    let hp0 = host.raid.as_ref().unwrap().foes[0].hp;
    guest.raid.as_mut().unwrap().stage = Stage::Fight;
    raid::hit_foe(&mut guest, 2, 0, 500.0);
    carry(&mut guest, 2, &mut host);
    assert!((hp0 - host.raid.as_ref().unwrap().foes[0].hp - 500.0).abs() < 1.0);
    // the guest drops: marked as gone, the raid carries on; back in: put back in
    host.f[2].present = false;
    raid::tick(&mut host, TICK);
    assert!(!host.raid.as_ref().unwrap().member(2).unwrap().connected);
    host.f[2].present = true;
    let place = host.raid.as_ref().unwrap().place;
    raid::tick(&mut host, TICK);
    let r = host.raid.as_ref().unwrap();
    assert!(r.member(2).unwrap().connected && r.place != place, "back in the running raid, placed with the party");
}

/// Solo with three bots (me played by the same bot brain): the whole vault on normal.
#[test]
fn bots_clear_the_vault_on_normal() {
    let _q = crate::heavy();
    let mut g = in_vault(3, 3, false);
    let mut rooms = Vec::new();
    let limit: f32 = std::env::var("RAID_SIM_MIN").ok().and_then(|v| v.parse().ok()).unwrap_or(55.0) * 60.0;
    let mut t = 0.0;
    let mut wipes_seen = 0;
    let mut seen_recap = 0;
    let mut last_report = 0.0;
    while t < limit {
        let inp = crate::raid_bots::input(&g, 0, TICK);
        g.tick(&inp);
        t += TICK;
        let Some(r) = g.raid.as_ref() else {
            eprintln!("the raid ended at {t:.0} s; me at {:?} in raid {}", g.f[0].pos, g.in_raid(0));
            break;
        };
        if t - last_report > 10.0 {
            last_report = t;
            let b = r.boss().map_or(0.0, |b| b.hp / b.max);
            let hp: Vec<String> = r.members.iter().map(|m| {
                let f = &g.f[m.slot as usize];
                format!("{}{:.0} @({:.1},{:.1},{:.1})", if m.downed { "x" } else { "" }, f.hp, f.pos.x, f.pos.y, f.pos.z)
            }).collect();
            eprintln!("t {t:.0} room {} stage {:?} boss {:.2} party {:?} flood {:.2} plates {:?}", r.room, r.stage, b, hp, r.flood, r.plates);
        }
        if r.recap.len() != seen_recap {
            seen_recap = r.recap.len();
            if let Some(x) = r.recap.last() {
                let b = r.boss().map_or(V3::ZERO, |b| b.pos);
                let p = g.f[x.0 as usize].pos;
                eprintln!("  down at {t:.1}: slot {} {} at ({:.1},{:.1}) boss ({:.1},{:.1})", x.0, x.1.name(), p.x, p.z, b.x, b.z);
            }
        }
        if r.wipes != wipes_seen {
            wipes_seen = r.wipes;
            let causes: Vec<&str> = r.recap.iter().rev().take(r.members.len()).map(|x| x.1.name()).collect();
            eprintln!("wipe {} at {t:.0} s in room {}: {:?}", r.wipes, r.room, causes);
        }
        if r.stage == Stage::Cleared && rooms.last() != Some(&r.room) {
            rooms.push(r.room);
            eprintln!("room {} cleared at {:.0} s (wipes so far {})", r.room, t, r.wipes);
        }
        if r.stage == Stage::Done {
            eprintln!("vault cleared at {:.0} s, total {:.0} s, {} wipes", t, r.total_time, r.wipes);
            rooms.push(4);
            break;
        }
        if r.stage == Stage::Cleared {
            // walk on into the next room (the bots follow whoever goes first)
            let next = r.room + 1;
            if next < 5 && (t * 10.0) as u32 % 50 == 0 {
                let p = ROOMS[next].entry_pos();
                g.f[0].respawn_at(p, 0.0);
            }
        }
    }
    let (room, wipes) = g.raid.as_ref().map_or((5, 0), |r| (r.room, r.wipes));
    assert_eq!(rooms, vec![0, 1, 2, 3, 4], "bots clear every room (stuck in room {room} after {wipes} wipes)");
}

/// Each hall's boss is introduced once in a raid (a camera sweep and its name card), not again
/// after a wipe; a boss met before can be skipped, one never met can't; the Magistrate is drawn
/// from its own model now (not as a fighter) and is huge; each boss and hall lights itself.
#[test]
fn boss_introductions_and_lights() {
    use crate::raid_ui::{intro_cam, intro_step, Intro, INTRO_TIME};
    let mut g = in_vault(3, 3, false);
    let mut it = Intro::default();
    assert_eq!(g.raid.as_ref().unwrap().stage, Stage::Intro);
    // never met: it plays, Space doesn't skip it, the camera sweeps in, and it's remembered
    let mut met = None;
    let mut cams = Vec::new();
    for k in 0..30 {
        if let Some(m) = intro_step(&mut it, &g, 0.1, 0, k == 5) {
            met = Some(m);
        }
        if let Some((pos, look, _)) = intro_cam(&it, &g) {
            cams.push((pos, look));
        }
    }
    assert!(it.on && !it.skipped && !it.skippable, "the first introduction plays through");
    assert_eq!(met, Some(0), "the Warden is remembered as met");
    let boss = g.raid.as_ref().unwrap().foes[0].pos;
    let (first, last) = (cams[0], cams[cams.len() - 1]);
    assert!((last.0 - boss).flat().len() < (first.0 - boss).flat().len(), "the camera closes in");
    assert!(last.0.y < first.0.y, "and comes down to look up at it");
    assert!(ROOMS[0].contains(last.0, 0.0), "inside its hall");
    for _ in 0..((INTRO_TIME + 1.0) / 0.1) as usize {
        intro_step(&mut it, &g, 0.1, 1, false);
    }
    assert!(!it.on, "it ends");
    // a wipe sends the hall back to its Intro stage: no second introduction
    intro_step(&mut it, &g, 0.1, 1, false);
    assert!(!it.on, "once a raid");
    // the next hall, its boss met before: Space skips it
    to_room(&mut g, 1);
    g.raid.as_mut().unwrap().stage = Stage::Intro;
    intro_step(&mut it, &g, 0.1, 0b10, false);
    assert!(it.on && it.skippable);
    intro_step(&mut it, &g, 0.1, 0b10, true);
    intro_step(&mut it, &g, 0.1, 0b10, false);
    assert!(!it.on && intro_cam(&it, &g).is_none(), "skipped");
    // the Magistrate: its own model, not a fighter; huge
    to_room(&mut g, 4);
    assert!(raid::fighters(&g).is_empty(), "no stand-in fighter for the Magistrate");
    assert!(FoeKind::Magistrate.size().1 > 3.0);
    // lights: the boss's own and the hall's cold windows
    let lights = raid::lights(&g);
    assert!(lights.len() >= 3, "{} lights in the pit", lights.len());
    to_room(&mut g, 0);
    let lights = raid::lights(&g);
    assert!(lights.iter().any(|l| l.2[2] > l.2[0] * 1.5), "a cold window light");
    assert!(lights.iter().any(|l| l.2[0] > l.2[2] * 2.0), "the Warden's warm fire");
}
