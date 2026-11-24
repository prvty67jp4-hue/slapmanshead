//! Tests for Ledger Duels (crate::gear, Mode::Ledger): gear switches, specials, food, the rules,
//! how gear changes damage, the duel online (rules and gear reach the other side, a special's hit
//! is checked by the host, a player dropping out mid-duel), and the balance simulation behind
//! BALANCE.md (`cargo test --release ledger_balance -- --ignored --nocapture`).

use crate::duel_bench::{Duelist, Style};
use crate::game::*;
use crate::gear::{self, presets, GearLoadout, Kit, Rules, Slot, Spec};
use crate::math::*;
use crate::net::{apply_remote, Incoming, Net};

/// A Ledger Duel on a flat floor: you (slot 0) and a still dummy (slot 1) `dist` apart, facing
/// each other, the fight on.
fn duel(dist: f32) -> Game {
    let mut g = Game::offline("A", 1, 0.5, Mode::Ledger, 3);
    g.world = crate::game::lag_comp_tests::flat_world();
    g.bots[1] = None;
    g.mt.phase = Phase::Fight;
    g.mt.timer = 999.0;
    g.mt.duel = [0, 1];
    for i in 0..2 {
        let f = &mut g.f[i];
        f.alive = true;
        f.spawn_prot = 0.0;
        f.is_bot = false;
        f.hp = 100.0;
        f.ap = 0.0;
        f.on_ground = true;
        f.vel = V3::ZERO;
    }
    g.f[0].pos = v3(0.0, 0.0, 0.0);
    g.f[1].pos = v3(0.0, 0.0, dist);
    g.f[0].yaw = 0.0;
    g.f[1].yaw = std::f32::consts::PI;
    for i in 0..2 {
        g.f[i].prev_pos = g.f[i].pos;
    }
    g
}

fn kit(g: &mut Game, i: usize, k: usize) {
    g.give_kit(i, &presets()[k]);
}

/// Your input, facing the dummy (you never turn in these tests).
fn i0() -> Input {
    Input::default()
}

fn idle(g: &Game, i: usize) -> Input {
    Input { yaw: g.f[i].yaw, ..Default::default() }
}

fn run(g: &mut Game, secs: f32, inp: Input) {
    for _ in 0..(secs / TICK).round() as usize {
        g.ext_input = vec![Some(inp), Some(idle(g, 1))];
        g.tick(&inp);
    }
}

/// A swap takes its commit time (the weapon changes only once it's done, no attacking
/// meanwhile), shows on the fighter for the other side, and the rule can forbid it.
#[test]
fn gear_switches_take_their_time() {
    let mut g = duel(2.0);
    kit(&mut g, 0, 1); // Bastion Wall: sabre and buckler, cleaver in pack slot 1
    kit(&mut g, 1, 4);
    assert_eq!(g.f[0].weapon, Weapon::Katana);
    let k = g.f[0].kit.as_ref().unwrap().inv.iter().position(|&id| id == 25).unwrap();
    let t = gear::swap_time(gear::item(25).unwrap(), Some(gear::Set::Bastion));
    run(&mut g, TICK, Input { inv: Some(k as u8), ..i0() });
    assert!(g.f[0].kit.as_ref().unwrap().swap.is_some(), "the swap starts");
    // mid-swap: still the sabre, and an attack does nothing
    run(&mut g, t * 0.5, Input { fire_pressed: true, ..i0() });
    assert_eq!(g.f[0].weapon, Weapon::Katana);
    assert!(!g.f[0].slashing(), "no attacking mid-swap");
    assert!(g.f[0].kit.as_ref().unwrap().bits() & gear::B_SWAP != 0, "others see the swap");
    run(&mut g, t * 0.6 + TICK * 2.0, i0());
    assert_eq!(g.f[0].weapon, Weapon::Greatsword, "the cleaver is in hand");
    let kt = g.f[0].kit.as_ref().unwrap();
    assert_eq!(kt.equip[Slot::Off as usize], 0, "two hands: the buckler went into the pack");
    assert!(kt.inv.contains(&24) && kt.inv.contains(&35));
    // no switching
    let mut g = duel(2.0);
    g.mt.rules = Rules { no_switch: true, ..Default::default() };
    kit(&mut g, 0, 1);
    run(&mut g, 1.0, Input { inv: Some(0), ..i0() });
    assert_eq!(g.f[0].weapon, Weapon::Katana, "the rule holds");
    assert!(g.f[0].kit.as_ref().unwrap().note.is_some(), "and says why");
}

/// Swing until the dummy is hit (or time runs out): what it lost.
fn swing_hit(g: &mut Game, spec: bool) -> f32 {
    let hp0 = g.f[1].hp;
    let mut first = true;
    for _ in 0..(2.0 / TICK) as usize {
        let inp = Input { fire_pressed: !spec && first, spec: spec && first, ..idle(g, 0) };
        first = false;
        g.ext_input = vec![Some(inp), Some(idle(g, 1))];
        g.tick(&inp);
        if g.f[1].hp < hp0 {
            break;
        }
    }
    hp0 - g.f[1].hp
}

/// A special costs its share, winds up where the other side can see it (no attacking during
/// it), then the blow lands much harder; a crushing special smashes a raised guard; and with
/// specials off it never starts.
#[test]
fn specials_wind_up_and_hit_hard() {
    let mut g = duel(2.2);
    kit(&mut g, 0, 0); // Gravewarden Headsman: the cleaver
    kit(&mut g, 1, 4);
    let plain = swing_hit(&mut g, false);
    assert!(plain > 5.0, "a plain cleaver blow lands ({plain})");
    // fresh dummy, then the special
    let mut g = duel(2.2);
    kit(&mut g, 0, 0);
    kit(&mut g, 1, 4);
    let inp = Input { spec: true, ..idle(&g, 0) };
    g.ext_input = vec![Some(inp), Some(idle(&g, 1))];
    g.tick(&inp);
    {
        let k = g.f[0].kit.as_ref().unwrap();
        assert_eq!(k.spec.round(), 50.0, "Headsman's Drop costs half the bar");
        assert!(k.wind.is_some() && k.bits() & gear::B_WIND != 0, "the wind-up shows");
    }
    run(&mut g, 0.25, Input { fire_pressed: true, ..i0() });
    assert!(!g.f[0].slashing(), "no other attack during the wind-up");
    let spec = {
        let hp0 = 100.0;
        for _ in 0..(1.5 / TICK) as usize {
            let i0 = idle(&g, 0);
            g.ext_input = vec![Some(i0), Some(idle(&g, 1))];
            g.tick(&i0);
            if g.f[1].hp < hp0 {
                break;
            }
        }
        hp0 - g.f[1].hp
    };
    assert!(spec > plain * 1.6, "the special hits far harder: {spec} vs {plain}");
    assert!(spec <= Spec::HeadsmansDrop.info().cap + 0.01);
    // into a raised guard: the guard breaks
    let mut g = duel(2.2);
    kit(&mut g, 0, 0);
    kit(&mut g, 1, 4);
    g.f[1].guard = true;
    g.f[1].guard_t = 1.0;
    let guard_in = Input { guard: true, ..idle(&g, 1) };
    let mut inp = Input { spec: true, ..idle(&g, 0) };
    for _ in 0..(1.6 / TICK) as usize {
        g.ext_input = vec![Some(inp), Some(guard_in)];
        g.tick(&inp);
        inp.spec = false;
        if g.f[1].guard_broke > 0.0 || g.f[1].hp < 100.0 {
            break;
        }
    }
    assert!(g.f[1].guard_broke > 0.0 || g.f[1].guard_hp <= 0.0, "Headsman's Drop smashes the guard");
    // specials off
    let mut g = duel(2.2);
    g.mt.rules = Rules { no_specs: true, ..Default::default() };
    kit(&mut g, 0, 0);
    run(&mut g, TICK, Input { spec: true, ..i0() });
    assert!(g.f[0].kit.as_ref().unwrap().wind.is_none());
    assert_eq!(g.f[0].kit.as_ref().unwrap().spec, 100.0);
}

/// Eating heals and costs time (no attacking while eating); a bite can follow at once; no food
/// in a no-food duel.
#[test]
fn food_heals_but_takes_time() {
    let mut g = duel(2.2);
    kit(&mut g, 0, 1); // Bastion Wall: a bread and two cakes in the pack
    kit(&mut g, 1, 4);
    let slot = |g: &Game, id: u8| g.f[0].kit.as_ref().unwrap().inv.iter().position(|&x| x == id).unwrap() as u8;
    g.f[0].hp = 40.0;
    let bread = slot(&g, 44);
    run(&mut g, TICK, Input { inv: Some(bread), ..i0() });
    assert!((g.f[0].hp - 56.0).abs() < 0.5, "bread: +16 ({})", g.f[0].hp);
    let lock = g.f[0].kit.as_ref().unwrap().eat_t;
    let cake = slot(&g, 46);
    run(&mut g, TICK, Input { inv: Some(cake), ..i0() });
    assert!((g.f[0].hp - 66.0).abs() < 0.5, "a combo cake: +10 ({})", g.f[0].hp);
    assert!(g.f[0].kit.as_ref().unwrap().eat_t > lock, "for a little extra time");
    run(&mut g, 0.3, Input { fire_pressed: true, ..i0() });
    assert!(!g.f[0].slashing(), "no attacking while eating");
    run(&mut g, 1.2, i0());
    run(&mut g, TICK * 2.0, Input { fire_pressed: true, ..i0() });
    assert!(g.f[0].slashing(), "done eating: the blade swings again");
    let mut g = duel(2.2);
    g.mt.rules = Rules { no_food: true, ..Default::default() };
    kit(&mut g, 0, 1);
    g.f[0].hp = 40.0;
    let bread = g.f[0].kit.as_ref().unwrap().inv.iter().position(|&x| x == 44).unwrap() as u8;
    run(&mut g, TICK, Input { inv: Some(bread), ..i0() });
    assert_eq!(g.f[0].hp, 40.0, "no food in this duel");
}

/// The rules: no movement holds you in place; no guns puts a blade in hand; best of five needs
/// three; abilities, utility and medkits are off in every Ledger Duel.
#[test]
fn rules_shape_the_duel() {
    let mut g = duel(6.0);
    g.mt.rules = Rules { no_move: true, ..Default::default() };
    kit(&mut g, 0, 2);
    let p0 = g.f[0].pos;
    run(&mut g, 0.8, Input { fwd: 1.0, right: 1.0, jump: true, dash: true, ..i0() });
    assert!((g.f[0].pos - p0).flat().len() < 0.05, "no movement");
    let mut g = duel(6.0);
    g.mt.rules = Rules { no_guns: true, ..Default::default() };
    kit(&mut g, 0, 3); // the Longsight rifle
    assert!(g.f[0].weapon.is_melee(), "a blade instead of the rifle");
    assert_eq!(Rules { best_of: 5, ..Default::default() }.target(), 3);
    assert!(!g.abilities_on(), "no fighter abilities in a Ledger Duel");
    run(&mut g, 0.2, Input { utility: true, item: true, ability: true, ..i0() });
    assert_eq!(g.f[0].util, 0);
    assert_eq!(g.f[0].items, 0);
    assert_eq!(g.f[0].ap, 0.0, "gear instead of armour");
    // every other mode is as it was
    let f = Game::offline("A", 1, 0.5, Mode::Ffa, 10);
    assert!(f.abilities_on() && !f.ledger_on());
}

/// How gear changes a hit: defence takes its share, a full Gravewarden set hits harder the lower
/// its wearer's health, hits are capped, and the spread is never more than 6%.
#[test]
fn gear_changes_damage_within_limits() {
    let mut g = duel(3.0);
    kit(&mut g, 0, 4); // Court Duellist
    kit(&mut g, 1, 1); // Bastion Wall: heavy defence
    let soft = {
        let mut h = duel(3.0);
        kit(&mut h, 0, 4);
        h.give_kit(1, &GearLoadout { name: "Bare".into(), equip: [0, 0, 0, 0, 0, 24, 0, 0], inv: [0; 12], dyes: gear::DEFAULT_DYES });
        h.ledger_scale(Some(0), 1, 30.0, Weapon::Katana as u8, 0)
    };
    let hard = g.ledger_scale(Some(0), 1, 30.0, Weapon::Katana as u8, 0);
    assert!(hard < soft * 0.75, "Bastion takes a good share off: {hard} vs {soft}");
    // a sniper's body shot is capped outside a special
    assert!(g.ledger_scale(Some(0), 1, LETHAL, Weapon::Sniper as u8, 0) <= gear::NORMAL_MAX);
    // an execution still finishes
    assert_eq!(g.ledger_scale(Some(0), 1, LETHAL, EXEC_TOOL, 0), LETHAL);
    // Gravewarden: deadlier when hurt
    let mut g = duel(3.0);
    kit(&mut g, 0, 0);
    kit(&mut g, 1, 4);
    let full = g.ledger_scale(Some(0), 1, 30.0, Weapon::Greatsword as u8, 0);
    g.f[0].hp = 20.0;
    let low = g.ledger_scale(Some(0), 1, 30.0, Weapon::Greatsword as u8, 0);
    assert!(low > full * 1.2, "{low} vs {full}");
    // the spread
    let mut seen = Vec::new();
    for k in 0..200 {
        g.time = k as f32 * 0.37;
        seen.push(g.ledger_scale(Some(0), 1, 20.0, Weapon::Katana as u8, 0));
    }
    let (lo, hi) = seen.iter().fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    assert!(hi / lo < 1.13 && hi > lo, "a small spread: {lo}..{hi}");
    // a special the attacker's weapon doesn't have doesn't count
    let forged = g.ledger_scale(Some(0), 1, 20.0, Weapon::Greatsword as u8, Spec::SealBreaker as u8);
    g.time = 0.0;
    let _ = forged;
    let own = g.ledger_scale(Some(0), 1, 20.0, Weapon::Greatsword as u8, Spec::HeadsmansDrop as u8);
    let none = g.ledger_scale(Some(0), 1, 20.0, Weapon::Greatsword as u8, 0);
    let fake = g.ledger_scale(Some(0), 1, 20.0, Weapon::Greatsword as u8, Spec::SealBreaker as u8);
    assert!(own > none * 1.8 && (fake - none).abs() < 1e-4, "{own} {none} {fake}");
}

/// The network carries a hit's special and a meal, and a fighter's gear, bar and tells.
#[test]
fn ledger_packets_round_trip() {
    let ev = Ev::Hit { victim: 1, dmg: 30.0, knock: V3::ZERO, stun: 0.2, head: false, w: 2, o: V3::ZERO, dir: V3::ZERO, guard: 0.0, vt: 1.5, spec: Spec::HeadsmansDrop as u8 };
    let back = crate::net::ev_round_trip(&ev).expect("decodes");
    assert!(matches!(back, Ev::Hit { spec, .. } if spec == Spec::HeadsmansDrop as u8));
    assert!(matches!(crate::net::ev_round_trip(&Ev::Eat { item: 45 }), Some(Ev::Eat { item: 45 })));
    let mut f = Fighter::new("A", 0, true);
    f.alive = true;
    let mut k = Kit::new(&presets()[0]);
    k.spec = 62.0;
    k.wind = Some((Spec::HeadsmansDrop, 0.3));
    f.kit = Some(Box::new(k));
    let st = crate::net::fstate_round_trip(&f).expect("decodes");
    assert_eq!(st.gear, presets()[0].equip);
    assert_eq!(st.spec, 62);
    assert!(st.gbits & gear::B_WIND != 0);
    let mut copy = Fighter::new("A", 0, false);
    apply_remote(&mut copy, &st, false, 1.0, 1.0);
    let ck = copy.kit.as_ref().expect("a copy of their gear");
    assert!(ck.remote && ck.wind.is_some() && ck.equip == presets()[0].equip);
    // out of a Ledger Duel: no gear sent, none kept
    let plain = crate::net::fstate_round_trip(&Fighter::new("B", 1, true)).unwrap();
    apply_remote(&mut copy, &plain, false, 2.0, 2.0);
    assert!(copy.kit.is_none());
}

/// Online: the host's rules and mode reach the player; each side sees the other's gear; a
/// player's special hit is checked against their weapon by the host; and when the player drops
/// out mid-duel the host's game carries on (a newcomer can take the empty side).
#[test]
fn a_ledger_duel_online_and_a_dropout() {
    let mut hn = Net::host(0).expect("host");
    let port = hn.local_port();
    let mut hg = Game::new(Kind::Host, 0, Mode::Ledger, 2);
    hg.mt.rules = Rules { no_food: true, best_of: 5, ..Default::default() };
    hg.mt.target = hg.mt.rules.target();
    hg.add_player(0, "Host", true, None, 0);
    hg.my_kit = presets()[0].clone();
    hg.mt.phase = Phase::Fight;
    hg.mt.timer = 300.0;
    let mut cn = Net::join(&format!("127.0.0.1:{port}")).unwrap();
    let mut cg: Option<Game> = None;
    let dt = 1.0 / 60.0;
    let pump = |hn: &mut Net, hg: &mut Game, cn: Option<&mut Net>, cg: &mut Option<Game>| {
        for m in hn.poll() {
            match m {
                Incoming::Join { slot, name } => hg.add_player(slot, &name, false, None, 0),
                Incoming::Leave { slot } => hg.remove_player(slot),
                Incoming::PeerState { slot, st, sent, events, .. } if hg.f[slot].present => {
                    let now = hg.time;
                    apply_remote(&mut hg.f[slot], &st, true, sent, now);
                    for e in events {
                        hg.handle_remote_event(slot, e);
                    }
                }
                _ => {}
            }
        }
        hg.tick(&Input { yaw: hg.f[0].yaw, ..Default::default() });
        hn.send(hg, dt);
        if let Some(cn) = cn {
            if cg.is_none() {
                cn.send_hello("Guest", dt);
            }
            for m in cn.poll() {
                if let Incoming::Host(msg) = m {
                    let g = cg.get_or_insert_with(|| {
                        let mut g = Game::new(Kind::Client, msg.you, msg.mode, msg.target);
                        g.my_kit = presets()[3].clone();
                        g
                    });
                    crate::apply_host_msg(g, msg);
                }
            }
            if let Some(g) = cg.as_mut() {
                let l = g.local;
                g.tick(&Input { yaw: g.f[l].yaw, ..Default::default() });
                cn.send(g, dt);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(3));
    };
    for _ in 0..240 {
        pump(&mut hn, &mut hg, Some(&mut cn), &mut cg);
    }
    let c = cg.as_ref().expect("the player joined");
    assert_eq!(c.mt.mode, Mode::Ledger);
    assert_eq!(c.mt.rules, hg.mt.rules, "the challenger's rules reach the player");
    assert!(c.f[0].kit.as_ref().is_some_and(|k| k.remote && k.equip == presets()[0].equip), "the player sees the host's gear");
    let s = c.local;
    assert!(hg.f[s].kit.as_ref().is_some_and(|k| k.remote && k.equip[Slot::Main as usize] == 29), "the host sees the player's rifle");
    // the player says they hit the host with Seal Breaker (their rifle's own special): it counts;
    // a cleaver's special from a rifle doesn't
    let hit = |spec: Spec| Ev::Hit { victim: 0, dmg: 34.0, knock: V3::ZERO, stun: 0.0, head: false, w: Weapon::Rifle as u8, o: hg.f[s].eye(), dir: (hg.f[0].chest() - hg.f[s].eye()).norm(), guard: 0.0, vt: 0.0, spec: spec as u8 };
    let ok = hit(Spec::SealBreaker);
    let bad = hit(Spec::HeadsmansDrop);
    let hp0 = hg.f[0].hp;
    hg.f[0].spawn_prot = 0.0;
    hg.f[s].spawn_prot = 0.0;
    let expect_ok = hg.ledger_scale(Some(s), 0, 34.0, Weapon::Rifle as u8, Spec::SealBreaker as u8);
    let expect_bad = hg.ledger_scale(Some(s), 0, 34.0, Weapon::Rifle as u8, 0);
    assert!(expect_ok > expect_bad * 1.4, "{expect_ok} vs {expect_bad}");
    hg.handle_remote_event(s, ok);
    let took = hp0 - hg.f[0].hp;
    if took > 0.0 {
        assert!(took > expect_bad * 1.3, "the special counted: {took}");
        let hp1 = hg.f[0].hp;
        hg.handle_remote_event(s, bad);
        let took2 = hp1 - hg.f[0].hp;
        assert!(took2 < took * 0.8, "a special the weapon doesn't have doesn't count: {took2} vs {took}");
    }
    // the player drops out mid-duel
    cn.send_bye();
    drop(cn);
    for _ in 0..60 {
        pump(&mut hn, &mut hg, None, &mut cg);
    }
    assert!(!hg.f[s].present, "the player is gone");
    assert!(hg.f[0].present && hg.f[0].kit.is_some(), "the host's duel carries on");
    assert!(matches!(hg.mt.phase, Phase::Fight | Phase::MatchOver), "{:?}", hg.mt.phase);
    // a newcomer can take the empty side
    let mut n2 = Net::join(&format!("127.0.0.1:{port}")).unwrap();
    let mut g2: Option<Game> = None;
    for _ in 0..200 {
        pump(&mut hn, &mut hg, Some(&mut n2), &mut g2);
    }
    assert!(g2.is_some(), "someone new joins the Court");
}

// ---------------------------------------------------------------- the balance simulation

/// A duel longer than this is a draw.
const LIMIT: f32 = 75.0;

/// One duel between presets `a` and `b` (which side each starts on alternates with the seed):
/// 1 if `a` wins, 0 if `b` does, 0.5 for a draw; and how long it took.
pub fn sim_duel(a: usize, b: usize, seed: u64) -> (f32, f32) {
    let mut g = duel(1.0);
    let mut rng = Rng(seed.wrapping_mul(2862933555777941757).wrapping_add(3037000493) | 1);
    let dist = rng.range(7.0, 15.0);
    let side = rng.range(-3.0, 3.0);
    let (ia, ib) = if seed % 2 == 0 { (0, 1) } else { (1, 0) };
    g.f[0].pos = v3(side, 0.0, -dist * 0.5);
    g.f[1].pos = v3(-side, 0.0, dist * 0.5);
    for i in 0..2 {
        let d = g.f[1 - i].pos - g.f[i].pos;
        g.f[i].yaw = d.x.atan2(d.z);
        g.f[i].prev_pos = g.f[i].pos;
        g.f[i].is_bot = true;
    }
    let p = presets();
    g.give_kit(ia, &p[a]);
    g.give_kit(ib, &p[b]);
    let mut bots = [Duelist::new(Style::Precise, seed * 7 + 1), Duelist::new(Style::Precise, seed * 7 + 2)];
    let mut t = 0.0;
    while t < LIMIT && g.f[0].alive && g.f[1].alive {
        let i0 = bots[0].think(&g, 0, 1, TICK);
        let i1 = bots[1].think(&g, 1, 0, TICK);
        g.ext_input = vec![Some(i0), Some(i1)];
        g.tick(&i0);
        t += TICK;
    }
    let (aa, ba) = (g.f[ia].alive, g.f[ib].alive);
    let score = if aa && !ba {
        1.0
    } else if ba && !aa {
        0.0
    } else {
        0.5
    };
    (score, t)
}

/// Every preset against every other, `per_pair` duels each way round; returns each preset's
/// score against the field and the full table (row a, column b: a's score against b).
pub fn sim_field(per_pair: u32) -> (Vec<f32>, Vec<Vec<f32>>, u32, f32) {
    let n = presets().len();
    let jobs: Vec<(usize, usize, u64)> = (0..n).flat_map(|a| (0..n).filter(move |&b| b != a).flat_map(move |b| (0..per_pair).map(move |k| (a, b, 5000 + (a * 97 + b * 13) as u64 * 1000 + k as u64)))).collect();
    let total = jobs.len() as u32;
    let jobs = std::sync::Arc::new(std::sync::Mutex::new(jobs));
    let table = std::sync::Arc::new(std::sync::Mutex::new(vec![vec![(0.0f32, 0u32); n]; n]));
    let secs = std::sync::Arc::new(std::sync::Mutex::new(0.0f32));
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(4).max(1);
    let hs: Vec<_> = (0..threads)
        .map(|_| {
            let (jobs, table, secs) = (jobs.clone(), table.clone(), secs.clone());
            std::thread::Builder::new()
                .stack_size(16 << 20)
                .spawn(move || loop {
                    let Some((a, b, seed)) = jobs.lock().unwrap().pop() else { break };
                    let (s, t) = sim_duel(a, b, seed);
                    let mut tb = table.lock().unwrap();
                    tb[a][b].0 += s;
                    tb[a][b].1 += 1;
                    tb[b][a].0 += 1.0 - s;
                    tb[b][a].1 += 1;
                    *secs.lock().unwrap() += t;
                })
                .unwrap()
        })
        .collect();
    for h in hs {
        h.join().unwrap();
    }
    let tb = table.lock().unwrap().clone();
    let rates: Vec<Vec<f32>> = tb.iter().map(|row| row.iter().map(|&(s, c)| if c == 0 { 0.5 } else { s / c as f32 }).collect()).collect();
    let field: Vec<f32> = tb.iter().map(|row| {
        let (s, c) = row.iter().fold((0.0, 0u32), |(a, b), &(s, c)| (a + s, b + c));
        s / c.max(1) as f32
    }).collect();
    let avg = *secs.lock().unwrap() / total.max(1) as f32;
    (field, rates, total, avg)
}

/// The BALANCE.md section for Ledger Duels.
pub fn sim_report(field: &[f32], rates: &[Vec<f32>], total: u32, avg: f32) -> String {
    let p = presets();
    let mut s = String::from("## Ledger Duels: loadouts against the field\n\n");
    s.push_str("Made by `cargo test --release ledger_balance -- --ignored --nocapture` (src/ledger_tests.rs).\n\n");
    s.push_str(&format!(
        "{total} bot duels on a flat floor, every preset loadout against every other the same number of times (each side starting on each end, 7-15 m apart). \
Both bots are the precise duelling bot from the weapon benchmark (same aim, reactions and movement) with the Ledger layer on top: they eat when hurt and not pressed, \
follow a meal with a bite, drink a tonic early, throw a special when it can finish or is worth it, swap to a weapon whose special can finish, and swap between blade and gun by range. \
A draw (both standing after {LIMIT:.0} s) counts as half a win. Average duel: {avg:.1} s.\n\n"
    ));
    s.push_str("| Loadout | Set | Main hand | Win rate vs the field |\n|---|---|---|---|\n");
    for (k, l) in p.iter().enumerate() {
        let set = gear::full_set(&l.equip).map_or("-", |x| x.name());
        s.push_str(&format!("| {} | {} | {} | **{:.1}%** |\n", l.name, set, l.main().map_or("-", |m| m.name), field[k] * 100.0));
    }
    // by set: the loadouts wearing all four pieces of it
    s.push_str("\nBy set (the loadouts wearing it):\n\n| Set | Win rate vs the field |\n|---|---|\n");
    let mut sets: Vec<(String, Vec<f32>)> = Vec::new();
    for (k, l) in p.iter().enumerate() {
        let name = gear::full_set(&l.equip).map_or("No set".to_string(), |x| x.name().to_string());
        match sets.iter_mut().find(|e| e.0 == name) {
            Some(e) => e.1.push(field[k]),
            None => sets.push((name, vec![field[k]])),
        }
    }
    for (name, v) in &sets {
        s.push_str(&format!("| {name} | {:.1}% |\n", v.iter().sum::<f32>() / v.len() as f32 * 100.0));
    }
    let worst = field.iter().cloned().fold(0.0f32, f32::max);
    s.push_str(&format!("\nTarget: no loadout above ~55% against the field. Result: the highest is {:.1}% - {}.\n\n", worst * 100.0, if worst <= 0.555 { "met" } else { "NOT met" }));
    s.push_str("Head to head (row's win rate against the column):\n\n| |");
    for l in &p {
        s.push_str(&format!(" {} |", short(&l.name)));
    }
    s.push_str("\n|---|");
    for _ in &p {
        s.push_str("---|");
    }
    s.push('\n');
    for (a, l) in p.iter().enumerate() {
        s.push_str(&format!("| {} |", short(&l.name)));
        for b in 0..p.len() {
            if a == b {
                s.push_str(" - |");
            } else {
                s.push_str(&format!(" {:.0}% |", rates[a][b] * 100.0));
            }
        }
        s.push('\n');
    }
    s.push_str("\nNo staking or wagering of any kind exists in Ledger Duels: the honour ledger only records wins, losses and streaks per loadout.\n");
    s
}

fn short(name: &str) -> &str {
    name.split(' ').next().unwrap_or(name)
}

/// The full balance run (about 2,000 duels): writes its section of BALANCE.md.
#[test]
#[ignore]
fn ledger_balance() {
    let per: u32 = std::env::var("LEDGER_PER_PAIR").ok().and_then(|v| v.parse().ok()).unwrap_or(36);
    let t0 = std::time::Instant::now();
    let (field, rates, total, avg) = sim_field(per);
    let rep = sim_report(&field, &rates, total, avg);
    println!("{rep}\n({:.0} s)", t0.elapsed().as_secs_f32());
    if total >= 2000 {
        crate::duel_bench::write_section("ledger", &rep);
    }
    for (k, r) in field.iter().enumerate() {
        assert!(*r <= 0.555, "{} wins {:.1}% against the field", presets()[k].name, r * 100.0);
    }
}

/// A quick version for every test run: a few duels per pairing, nothing runs away.
#[test]
fn no_loadout_runs_away_with_it() {
    let (field, _, total, _) = sim_field(2);
    assert!(total >= 100);
    for (k, r) in field.iter().enumerate() {
        assert!(*r <= 0.75, "{} wins {:.0}% in the quick run", presets()[k].name, r * 100.0);
    }
}

// ---------------------------------------------------------------- the Bridgewright's Rig

fn rigger() -> usize {
    presets().iter().position(|p| p.name == "Bridgewright Rigger").expect("the Rig's preset")
}

/// Haul: half the bar, a 0.4 s wind-up the other side sees, then the hook flies: whoever is in
/// its line within 8 m is yanked in and staggered.
#[test]
fn haul_yanks_them_in_and_staggers_them() {
    let mut g = duel(7.0);
    kit(&mut g, 0, rigger());
    kit(&mut g, 1, 4);
    assert_eq!(g.f[0].kit.as_ref().unwrap().main().unwrap().id, gear::HOOK);
    run(&mut g, TICK, Input { spec: true, ..i0() });
    {
        let k = g.f[0].kit.as_ref().unwrap();
        assert_eq!(k.spec.round(), 50.0, "Haul costs half the bar");
        assert!(matches!(k.wind, Some((Spec::Haul, _))) && k.bits() & gear::B_WIND != 0, "the chain rattles: everyone sees it");
    }
    let mut stunned = false;
    let mut closest = 99.0f32;
    for _ in 0..(1.2 / TICK) as usize {
        let i = i0();
        g.ext_input = vec![Some(i), Some(idle(&g, 1))];
        g.tick(&i);
        stunned |= g.f[1].stun_t > 0.0;
        closest = closest.min((g.f[1].pos - g.f[0].pos).flat().len());
    }
    assert!(stunned, "hauled in: staggered");
    assert!(closest < 3.0, "yanked in from 7 m to {closest:.2} m");
    assert!(g.f[1].hp < 100.0, "the hook bites a little");
}

/// Haul can be dodged (step out of its line during the wind-up) or parried (the thrower reels).
#[test]
fn haul_can_be_dodged_or_parried() {
    // dodged: they step aside while the chain rattles
    let mut g = duel(7.0);
    kit(&mut g, 0, rigger());
    kit(&mut g, 1, 4);
    run(&mut g, TICK, Input { spec: true, ..i0() });
    g.f[1].pos.x += 2.5;
    g.f[1].prev_pos = g.f[1].pos;
    run(&mut g, 1.0, i0());
    assert!((g.f[1].pos - g.f[0].pos).flat().len() > 6.0, "the hook misses");
    assert!(g.f[1].stun_t <= 0.0);
    // parried: the guard comes up just as it lands
    let mut g = duel(5.0);
    kit(&mut g, 0, rigger());
    kit(&mut g, 1, 4);
    run(&mut g, TICK, Input { spec: true, ..i0() });
    run(&mut g, gear::HAUL_WINDUP - 0.1, i0());
    let mut thrower_reeled = false;
    for _ in 0..(0.4 / TICK) as usize {
        let i = i0();
        let guard = Input { guard: true, ..idle(&g, 1) };
        g.ext_input = vec![Some(i), Some(guard)];
        g.tick(&i);
        thrower_reeled |= g.f[0].stun_t > 0.0;
    }
    assert!(thrower_reeled, "parried: the thrower reels");
    assert!((g.f[1].pos - g.f[0].pos).flat().len() > 4.0, "and nobody is pulled");
}

/// Change of Tools: swaps 30% quicker with all four pieces; a blade hit and a gun hit within
/// 3 s give +10 bar, at most once every 6 s; fewer than four pieces: none of it.
#[test]
fn change_of_tools_rewards_switching() {
    let r = Rules::default();
    let cleaver = gear::item(25).unwrap();
    assert!((gear::swap_time(cleaver, Some(gear::Set::Bridgewright)) - gear::swap_time(cleaver, None) * 0.7).abs() < 1e-5);
    let mut k = Kit::new(&presets()[rigger()]);
    assert_eq!(k.stats(&r).set, Some(gear::Set::Bridgewright));
    k.spec = 40.0;
    assert!(!k.landed(true, 1.0, &r), "one blade hit: nothing yet");
    assert!(k.landed(false, 2.5, &r), "then a gun hit within 3 s: +10");
    assert_eq!(k.spec, 50.0);
    assert!(!k.landed(true, 3.0, &r), "not again within 6 s");
    k.tick(6.1, &r);
    k.spec = 50.0;
    assert!(k.landed(false, 9.5, &r) || k.landed(true, 9.6, &r), "after the cooldown it pays again");
    assert!(k.charm_glows(), "the charm glows teal while the bonus is on");
    // three pieces: no set, no bonus, a plain swap time
    let mut three = presets()[rigger()].clone();
    three.equip[Slot::Hands as usize] = 20;
    let mut k3 = Kit::new(&three);
    assert_eq!(k3.stats(&r).set, None);
    assert!(!k3.landed(true, 1.0, &r) && !k3.landed(false, 1.5, &r));
    assert!(!k3.charm_glows());
    // and in the game: a blade hit then a gun hit pays
    let mut g = duel(2.0);
    kit(&mut g, 0, rigger());
    kit(&mut g, 1, 4);
    g.f[0].kit.as_mut().unwrap().spec = 20.0;
    let (a, b) = (g.f[0].chest(), g.f[1].chest());
    g.damage(0, 1, 10.0, V3::ZERO, 0.0, false, b, Weapon::Katana as u8, a, v3(0.0, 0.0, 1.0));
    g.damage(0, 1, 10.0, V3::ZERO, 0.0, false, b, Weapon::Revolver as u8, a, v3(0.0, 0.0, 1.0));
    assert!((g.f[0].kit.as_ref().unwrap().spec - 30.0).abs() < 0.5, "+10 for the change of tools");
}

/// The off-hand pistol: R fires it while the hook (a one-handed blade) is in hand; four rounds,
/// then a reload.
#[test]
fn the_snub_fires_from_the_off_hand() {
    let mut g = duel(6.0);
    // (shooting from the eye, as bots do: the player's camera line passes over a dummy this close)
    g.mt.rules = Rules { no_specs: true, ..Default::default() };
    g.f[0].is_bot = true;
    kit(&mut g, 0, rigger());
    kit(&mut g, 1, 4);
    assert!(g.f[0].weapon.is_melee() && g.f[0].kit.as_ref().unwrap().snub_ready());
    let hp0 = g.f[1].hp;
    for _ in 0..gear::SNUB_ROUNDS {
        run(&mut g, TICK, Input { reload: true, ..i0() });
        run(&mut g, gear::SNUB_GAP + 0.02, i0());
    }
    assert!(g.f[1].hp < hp0, "the snub hits ({} left)", g.f[1].hp);
    let k = g.f[0].kit.as_ref().unwrap();
    assert_eq!(k.snub, 0);
    assert!(k.snub_reload > 0.0 && !k.snub_ready(), "empty: reloading");
    run(&mut g, gear::SNUB_RELOAD + 0.1, i0());
    assert_eq!(g.f[0].kit.as_ref().unwrap().snub, gear::SNUB_ROUNDS);
}

/// Dyes: saved with the loadout, sent with the fighter.
#[test]
fn dyes_are_kept_and_sent() {
    let mut l = presets()[rigger()].clone();
    assert_eq!(l.dyes, gear::DEFAULT_DYES, "oiled brown, brass, teal");
    l.dyes = [5, 9, 7];
    assert_eq!(GearLoadout::from_text(&l.to_text()).unwrap().dyes, [5, 9, 7]);
    let mut f = Fighter::new("A", 0, true);
    f.kit = Some(Box::new(Kit::new(&l)));
    let st = crate::net::fstate_round_trip(&f).unwrap();
    assert_eq!(st.dyes, [5, 9, 7]);
    let mut copy = Fighter::new("A", 0, false);
    apply_remote(&mut copy, &st, false, 1.0, 1.0);
    assert_eq!(copy.kit.as_ref().unwrap().dyes, [5, 9, 7]);
}
