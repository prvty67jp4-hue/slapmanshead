//! Tests for the readability, weapon-feel, feedback and tactics pass: surfaces (footsteps,
//! impacts, sound tails), sneaking, smokes and flashes, inspect and magazines, hit markers, the
//! death recap, pings, Team Rounds, and that a hit on what a player saw is checked at that moment.

use crate::game::*;
use crate::math::*;
use crate::world::{MapId, Surf, World};

/// A fight on the team map: you at `at`, nothing moving.
fn on_bridgeworks(mode: Mode) -> Game {
    let mut g = Game::offline("A", 1, 0.5, mode, 50);
    g.set_map(MapId::Bridgeworks);
    g.mt.phase = Phase::Fight;
    g.mt.timer = 999.0;
    for i in 0..2 {
        g.bots[i] = None;
        g.f[i].spawn_prot = 0.0;
    }
    g
}

fn put(g: &mut Game, i: usize, p: V3) {
    g.f[i].pos = p;
    g.f[i].prev_pos = p;
    g.f[i].vel = V3::ZERO;
    g.f[i].on_ground = true;
}

#[test]
fn footsteps_sound_like_the_ground() {
    let w = World::build(MapId::Bridgeworks);
    assert_eq!(step_sound(&w, v3(-24.0, 0.0, 0.0)), Sfx::StepWood, "the wooden footbridge");
    assert_eq!(step_sound(&w, v3(-15.0, -3.0, 0.0)), Sfx::StepWater, "wading in the river");
    assert_eq!(step_sound(&w, v3(21.0, 1.6, 12.0)), Sfx::StepMetal, "on the iron furnace");
    assert_eq!(step_sound(&w, v3(0.0, 0.0, -12.0)), Sfx::StepStone, "the plaza's stone");
}

/// Sneaking: slower than the speed footsteps are heard at, so it makes no steps; running does.
#[test]
fn sneaking_is_quiet_and_running_is_heard() {
    let steps = |sneak: bool| {
        let mut g = on_bridgeworks(Mode::Ffa);
        put(&mut g, 0, v3(0.0, 0.0, -12.0));
        put(&mut g, 1, v3(20.0, 0.0, -25.0));
        let mut top = 0.0f32;
        let mut n = 0;
        for _ in 0..240 {
            g.sounds.clear();
            g.tick(&Input { fwd: 1.0, sneak, ..Default::default() });
            top = top.max(g.f[0].vel.flat().len());
            n += g.sounds.iter().filter(|s| matches!(s.0, Sfx::StepStone | Sfx::StepWood | Sfx::StepMetal | Sfx::StepWater)).count();
        }
        (top, n)
    };
    let (sneak_speed, sneak_steps) = steps(true);
    let (run_speed, run_steps) = steps(false);
    assert!(sneak_speed <= SNEAK_SPEED + 0.05, "sneaking caps the speed ({sneak_speed})");
    assert_eq!(sneak_steps, 0, "and makes no footsteps");
    assert!(run_speed > RUN_SPEED * 0.9 && run_steps >= 4, "running is heard ({run_steps} steps)");
}

/// What a bullet meets decides what flies off and the mark it leaves; marks fade away.
#[test]
fn bullets_hit_stone_wood_metal_and_water_differently() {
    let w = World::build(MapId::Bridgeworks);
    let down = v3(0.0, -1.0, 0.0);
    let s = |o: V3, d: V3| w.surface_hit(o, d, 50.0).map(|h| h.2);
    assert_eq!(s(v3(-24.0, 3.0, 0.5), down), Some(Surf::Wood), "the footbridge's planks");
    assert_eq!(s(v3(-15.0, 3.0, 0.5), down), Some(Surf::Water), "the river");
    assert_eq!(s(v3(21.25, 2.4, 12.0), down), Some(Surf::Metal), "the furnace (under the workshop roof)");
    assert_eq!(s(v3(0.0, 1.0, -12.0), v3(0.0, 0.0, -1.0)), Some(Surf::Stone), "the base wall");
    let mut g = on_bridgeworks(Mode::Ffa);
    for (code, at) in [(3u8, v3(10.0, 1.0, -17.0)), (11, v3(-24.0, 0.0, 0.5)), (12, v3(21.25, 1.6, 12.0)), (13, v3(-15.0, -2.62, 0.5))] {
        let before = g.particles.len();
        g.impact_fx(at, code);
        assert!(g.particles.len() > before + 5, "code {code}: something flies off");
    }
    let holes: Vec<_> = g.decals.iter().filter(|d| d.kind == 3).collect();
    assert_eq!(holes.len(), 3, "stone, wood and metal keep a bullet hole");
    let wall = holes.iter().find(|d| d.pos.z < -16.0).unwrap();
    assert!((wall.normal - v3(0.0, 0.0, 1.0)).len() < 1e-3, "on the wall, facing out of it ({:?})", wall.normal);
    assert!(g.decals.iter().any(|d| d.kind == 4), "the water ripples");
    for _ in 0..(10.5 / TICK) as usize {
        g.tick(&Input::default());
    }
    assert!(g.decals.iter().all(|d| d.kind != 3), "bullet holes fade away");
    // dust takes the wall's own colour: the red base's brick dust is warm
    let mut g = on_bridgeworks(Mode::Ffa);
    g.impact_fx(v3(10.0, 1.0, -17.0), 3);
    assert!(g.particles.iter().any(|p| p.size > 0.15 && p.color[0] > p.color[2] + 30), "warm brick dust");
}

#[test]
fn gunshots_ring_short_indoors_and_long_outside() {
    let w = World::build(MapId::Bridgeworks);
    assert!(w.enclosed(v3(20.0, 0.0, 8.0)), "the roofed workshop hall");
    assert!(w.enclosed(v3(-20.0, 0.0, 27.0)), "the barracks");
    assert!(!w.enclosed(v3(0.0, 0.0, -11.0)), "the open plaza");
    let mut g = on_bridgeworks(Mode::Ffa);
    put(&mut g, 0, v3(0.0, 0.0, -11.0));
    g.f[0].loadout.gun1 = Weapon::Rifle;
    g.f[0].equip();
    g.tick(&Input { select: Some(1), ..Default::default() });
    for _ in 0..40 {
        g.tick(&Input::default());
    }
    g.sounds.clear();
    g.tick(&Input { fire: true, fire_pressed: true, ..Default::default() });
    assert!(g.sounds.iter().any(|s| s.0 == Sfx::TailOpen), "outside: the long tail");
    put(&mut g, 0, v3(20.0, 0.0, 8.0));
    for _ in 0..120 {
        g.tick(&Input::default());
    }
    g.sounds.clear();
    g.tick(&Input { fire: true, fire_pressed: true, ..Default::default() });
    assert!(g.sounds.iter().any(|s| s.0 == Sfx::TailRoom), "inside: the room's slap");
}

/// Each fighter's utility: two a life (or round), thrown with its key; a smoke hides what's
/// behind it (from bots too); a flash blinds those looking at it, not through walls.
#[test]
fn smokes_and_flashes() {
    for body in 0..4u8 {
        let mut g = on_bridgeworks(Mode::Ffa);
        g.f[0].loadout.body = body;
        put(&mut g, 0, v3(0.0, 0.0, -11.0));
        assert_eq!(g.f[0].util, UTIL_PER_ROUND);
        let kind = Utility::of(body).kind();
        for k in 0..3 {
            g.tick(&Input { utility: true, ..Default::default() });
            for _ in 0..30 {
                g.tick(&Input::default());
            }
            let thrown = g.grenades.iter().filter(|gr| gr.kind == kind).count() + if kind == 3 { g.smokes.len() } else { 0 };
            assert!(thrown >= (k + 1).min(2) || k >= 2, "fighter {body}: throw {k}");
        }
        assert_eq!(g.f[0].util, 0, "fighter {body}: two and no more");
        let p = g.f[0].pos;
        g.f[0].respawn_at(p, 0.0);
        assert_eq!(g.f[0].util, UTIL_PER_ROUND, "back with a new life");
    }
    assert_ne!(Utility::of(0), Utility::of(1), "two smokes and two flashes");
    // a smoke between two points hides one from the other once it's thick
    let mut g = on_bridgeworks(Mode::Ffa);
    g.smokes.push(Smoke { pos: v3(0.0, 0.0, -10.0), t: 0.0, body: 0 });
    let (a, b) = (v3(-8.0, 1.5, -10.0), v3(8.0, 1.5, -10.0));
    assert!(!g.smoke_blocks(a, b), "not yet: it's only just burst");
    for _ in 0..(1.5 / TICK) as usize {
        g.tick(&Input::default());
    }
    assert!(g.smoke_blocks(a, b), "thick: it hides them");
    assert!(!g.smoke_blocks(a, v3(-8.0, 1.5, -2.0)), "not what's beside it");
    for _ in 0..(SMOKE_LIFE / TICK) as usize {
        g.tick(&Input::default());
    }
    assert!(g.smokes.is_empty(), "and it clears");
    // flashes
    let w = World::build(MapId::Bridgeworks);
    let eye = v3(0.0, 1.6, -11.0);
    let p = v3(0.0, 1.5, -6.0);
    let facing = flash_blind(&w, &[], eye, v3(0.0, 0.0, 1.0), p);
    let away = flash_blind(&w, &[], eye, v3(0.0, 0.0, -1.0), p);
    let far = flash_blind(&w, &[], v3(0.0, 1.6, -16.0), v3(0.0, 0.0, 1.0), p);
    assert!(facing > 1.5 && away < 0.6 && far < facing, "facing {facing}, away {away}, further {far}");
    assert_eq!(flash_blind(&w, &[], v3(10.0, 1.6, -21.0), v3(0.0, 0.0, 1.0), v3(10.0, 1.5, -12.0)), 0.0, "not through the base wall");
}

#[test]
fn a_flash_blinds_the_bots_and_they_hold_fire() {
    let mut g = on_bridgeworks(Mode::Ffa);
    put(&mut g, 0, v3(0.0, 0.0, -13.0));
    put(&mut g, 1, v3(0.0, 0.0, -6.0));
    g.f[1].yaw = std::f32::consts::PI;
    g.bots[1] = Some(crate::bot::Bot::new(1.0, 3));
    g.f[1].blind_t = 2.0;
    let hp = g.f[0].hp;
    for _ in 0..(1.2 / TICK) as usize {
        g.tick(&Input::default());
    }
    assert!(g.f[0].hp >= hp, "a blinded bot doesn't hit you");
}

#[test]
fn inspect_and_magazines() {
    let mut g = on_bridgeworks(Mode::Ffa);
    put(&mut g, 0, v3(0.0, 0.0, -11.0));
    g.f[0].loadout.gun1 = Weapon::Rifle;
    g.f[0].equip();
    g.tick(&Input { select: Some(1), ..Default::default() });
    for _ in 0..40 {
        g.tick(&Input::default());
    }
    g.tick(&Input { inspect: true, ..Default::default() });
    assert!(g.f[0].inspect_t > 0.0, "inspecting");
    g.tick(&Input { fire: true, fire_pressed: true, ..Default::default() });
    assert_eq!(g.f[0].inspect_t, 0.0, "shooting puts it away");
    for _ in 0..30 {
        g.tick(&Input::default());
    }
    let before = g.casings.iter().filter(|c| c.kind == 1).count();
    g.tick(&Input { reload: true, ..Default::default() });
    g.tick(&Input::default());
    assert_eq!(g.casings.iter().filter(|c| c.kind == 1).count(), before + 1, "the empty magazine drops");
    // every gun's handling moves the hands through its reload and inspect
    for w in [Weapon::Revolver, Weapon::Shotgun, Weapon::Smg, Weapon::Rifle, Weapon::Rocket, Weapon::HandCannon, Weapon::Sniper] {
        let mut f = Fighter::new("t", 0, true);
        f.weapon = w;
        let aim = glam::Vec3::Z;
        let side = glam::Vec3::X;
        let chest = glam::Vec3::new(0.0, 1.35, 0.0);
        let idle = crate::character::gun_handling(&f, aim, side, chest);
        f.reload_t = w.reload_time() * 0.5;
        let mid = crate::character::gun_handling(&f, aim, side, chest);
        f.reload_t = 0.0;
        f.inspect_t = INSPECT_TIME * 0.5;
        let insp = crate::character::gun_handling(&f, aim, side, chest);
        assert!(idle.dir.is_none() && mid.dir.is_some() && insp.dir.is_some(), "{w:?}: reload and inspect move the gun");
        assert!(mid.left.is_some(), "{w:?}: the other hand works the reload");
    }
}

/// Hit markers: a body hit, a headshot and a kill each look and sound different.
#[test]
fn hit_markers_tell_body_head_and_kill_apart() {
    let mut g = on_bridgeworks(Mode::Ffa);
    put(&mut g, 0, v3(0.0, 0.0, -11.0));
    put(&mut g, 1, v3(0.0, 0.0, -6.0));
    let (a, d) = (g.f[0].chest(), v3(0.0, 0.0, 1.0));
    let at = g.f[1].chest();
    g.sounds.clear();
    g.damage(0, 1, 10.0, V3::ZERO, 0.0, false, at, Weapon::Rifle as u8, a, d);
    assert_eq!(g.hit_kind, 0);
    assert!(g.sounds.iter().any(|s| s.0 == Sfx::Hit));
    for _ in 0..30 {
        g.tick(&Input::default());
    }
    g.sounds.clear();
    g.damage(0, 1, 10.0, V3::ZERO, 0.0, true, at, Weapon::Rifle as u8, a, d);
    assert_eq!(g.hit_kind, 1);
    assert!(g.sounds.iter().any(|s| s.0 == Sfx::Headshot));
    for _ in 0..30 {
        g.tick(&Input::default());
    }
    g.sounds.clear();
    g.damage(0, 1, LETHAL, V3::ZERO, 0.0, false, at, Weapon::Rifle as u8, a, d);
    g.tick(&Input::default());
    assert_eq!(g.hit_kind, 2);
    assert!(g.sounds.iter().any(|s| s.0 == Sfx::Kill) && g.sounds.iter().any(|s| s.0 == Sfx::KillConfirm));
}

/// The death recap says what you dealt your killer and what they dealt you this life.
#[test]
fn the_death_recap_shows_damage_both_ways() {
    let mut g = on_bridgeworks(Mode::Ffa);
    put(&mut g, 0, v3(0.0, 0.0, -11.0));
    put(&mut g, 1, v3(0.0, 0.0, -6.0));
    let (a0, a1) = (g.f[0].chest(), g.f[1].chest());
    g.damage(0, 1, 30.0, V3::ZERO, 0.0, false, a1, Weapon::Rifle as u8, a0, v3(0.0, 0.0, 1.0));
    g.damage(1, 0, 40.0, V3::ZERO, 0.0, false, a0, Weapon::Rifle as u8, a1, v3(0.0, 0.0, -1.0));
    g.damage(1, 0, LETHAL, V3::ZERO, 0.0, false, a0, Weapon::Rifle as u8, a1, v3(0.0, 0.0, -1.0));
    g.tick(&Input::default());
    let r = g.recap.as_ref().expect("a recap");
    assert!((r.dealt - 30.0).abs() < 0.5, "you dealt 30 ({})", r.dealt);
    assert!(r.taken >= 100.0, "they dealt you everything ({})", r.taken);
    assert_eq!(g.life_dealt, [0.0; MAX_PLAYERS], "a fresh count for the next life");
}

/// Pings go to the team with their callout; the host passes them on; enemies never see them.
#[test]
fn pings_reach_the_team_and_name_the_place() {
    let mut g = Game::new(Kind::Host, 0, Mode::Rounds, 7);
    g.set_map(MapId::Bridgeworks);
    g.add_player(0, "Host", true, None, 0);
    g.add_player(1, "Mate", false, None, 0);
    g.add_player(2, "Enemy", false, None, 1);
    g.ping(0, v3(-28.0, 0.3, 10.0));
    assert!(g.out_events.iter().any(|e| matches!(e, Ev::Ping { who: 0, kind: 0, .. })), "sent to the others");
    assert!(g.feed.iter().any(|l| l.text.as_deref().is_some_and(|t| t.contains("Blue Crane"))), "named by its callout");
    g.out_events.clear();
    g.handle_remote_event(1, Ev::Ping { who: 1, kind: 1, p: v3(0.0, 0.3, 0.0) });
    assert!(g.pings.iter().any(|p| p.owner == 1), "a teammate's ping shows");
    assert!(g.out_events.iter().any(|e| matches!(e, Ev::Ping { who: 1, .. })), "and is passed on");
    g.handle_remote_event(2, Ev::Ping { who: 2, kind: 0, p: v3(0.0, 0.3, 0.0) });
    assert!(!g.pings.iter().any(|p| p.owner == 2), "an enemy's doesn't");
    g.handle_remote_event(2, Ev::Ping { who: 1, kind: 0, p: v3(5.0, 0.3, 0.0) });
    assert!(!g.pings.iter().any(|p| p.pos.x == 5.0), "nobody pings for someone else");
    for _ in 0..((PING_LIFE + 0.5) / TICK) as usize {
        g.tick(&Input::default());
    }
    assert!(g.pings.is_empty(), "pings fade");
}

/// Team Rounds: one life a round; the side left standing scores; everyone comes back (utility
/// full) for the next; the first side to the target wins. No economy, nothing to buy.
#[test]
fn team_rounds_are_won_by_the_side_left_standing() {
    assert!(Mode::Rounds.teams() && Mode::Rounds.rounds());
    assert!(Mode::ALL.contains(&Mode::Rounds));
    assert_eq!(Mode::from_u8(Mode::Rounds as u8), Mode::Rounds);
    let mut g = Game::offline("A", 3, 0.5, Mode::Rounds, 2);
    g.set_map(MapId::Bridgeworks);
    for i in 0..4 {
        g.bots[i] = None;
        g.f[i].is_bot = i != 0;
        g.f[i].ready = true;
    }
    // play until the fight is on
    for _ in 0..(30.0 / TICK) as usize {
        if g.mt.phase == Phase::Fight && g.f.iter().take(4).all(|f| f.alive) {
            break;
        }
        g.tick(&Input::default());
    }
    assert_eq!(g.mt.phase, Phase::Fight);
    let my_team = g.f[0].team;
    let enemies: Vec<usize> = (0..4).filter(|&i| g.f[i].team != my_team).collect();
    assert!(!enemies.is_empty());
    for round in 1..=2 {
        for _ in 0..(3.5 / TICK) as usize {
            g.tick(&Input::default());
        }
        g.f[0].util = 0;
        for &e in &enemies {
            if g.f[e].alive {
                let (a, at) = (g.f[0].chest(), g.f[e].chest());
                g.damage(0, e, LETHAL, V3::ZERO, 0.0, false, at, Weapon::Rifle as u8, a, v3(0.0, 0.0, 1.0));
            }
        }
        for _ in 0..30 {
            g.tick(&Input::default());
        }
        assert_eq!(g.mt.team_score[my_team as usize], round, "round {round} to the side left standing");
        assert!(enemies.iter().all(|&e| !g.f[e].alive || g.mt.round_end > 0.0), "no respawns inside a round");
        if round == 1 {
            for _ in 0..(3.5 / TICK) as usize {
                g.tick(&Input::default());
            }
            assert!(enemies.iter().all(|&e| g.f[e].alive), "everyone back for the next round");
            assert_eq!(g.f[0].util, UTIL_PER_ROUND, "with their utility");
        }
    }
    assert_eq!(g.mt.phase, Phase::MatchOver, "first to two rounds wins");
}
