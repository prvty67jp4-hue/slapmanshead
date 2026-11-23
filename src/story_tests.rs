//! Tests for the story in matches (assets/lore.txt, see crate::lore): a 1v1's intro pair, the
//! fighters' voice lines (start, kill, finisher, death, rivals meeting) and the announcer's calls
//! (match start, round won, match won, Writ claimed); none of it in the menu's background fight
//! or training. Gameplay is untouched: these only look at what is said.

use crate::game::*;
use crate::lore::{self, Call, Cue};
use crate::math::*;

/// An offline match with `n` fighters in the given bodies, everyone ready, nobody thinking.
fn match_of(mode: Mode, bodies: &[u8], target: u16) -> Game {
    let mut g = Game::offline("A", bodies.len() - 1, 0.5, mode, target);
    for (i, &b) in bodies.iter().enumerate() {
        g.bots[i] = None;
        g.f[i].loadout.body = b;
        g.f[i].ready = true;
    }
    g.my_loadout.body = bodies[0];
    g
}

fn until(g: &mut Game, secs: f32, done: impl Fn(&Game) -> bool) {
    for _ in 0..(secs / TICK) as usize {
        if done(g) {
            return;
        }
        g.tick(&Input::default());
    }
}

fn run(g: &mut Game, secs: f32) {
    for _ in 0..(secs / TICK) as usize {
        g.tick(&Input::default());
    }
}

/// What `who` (None: the announcer) has said that's still up.
fn said(g: &Game, who: Option<usize>) -> Vec<String> {
    g.voice.iter().filter(|v| v.who == who).map(|v| v.text.clone()).collect()
}

fn lines(body: u8, cue: Cue) -> Vec<String> {
    lore::Lore::parse(lore::BUILT_IN).all(&lore::fighter_section(body), cue.key()).iter().map(|s| s.to_string()).collect()
}

fn announcer_lines(c: Call) -> Vec<String> {
    lore::Lore::parse(lore::BUILT_IN).all("announcer", c.key()).iter().map(|s| s.to_string()).collect()
}

fn kill(g: &mut Game, by: usize, who: usize, tool: u8) {
    let (a, at) = (g.f[by].chest(), g.f[who].chest());
    g.damage(by, who, LETHAL, V3::ZERO, 0.0, false, at, tool, a, v3(0.0, 0.0, 1.0));
    g.tick(&Input::default());
}

/// A duel between rivals: the intro card's pair is the two of them before the bell, then the
/// announcer rings it and both say a start line of their own.
#[test]
fn a_rival_duel_has_an_intro_and_first_words() {
    let mut g = match_of(Mode::Duel, &[0, 1], 5);
    until(&mut g, 30.0, |g| g.mt.phase == Phase::Countdown);
    assert_eq!(g.mt.phase, Phase::Countdown);
    let (a, b) = g.intro_pair().expect("a 1v1 gets an intro card");
    assert_eq!([a.min(b), a.max(b)], [0, 1]);
    // they are each other's rivals, with lines for meeting
    assert_eq!(lore::rival(0), Some(1));
    assert!(lore::vs_line(0, 1).is_some() && lore::vs_line(1, 0).is_some());
    until(&mut g, 10.0, |g| g.mt.phase == Phase::Fight);
    run(&mut g, 3.0);
    let bell = said(&g, None);
    assert!(bell.iter().any(|l| announcer_lines(Call::MatchStart).contains(l)), "the bell: {bell:?}");
    for (i, body) in [(0usize, 0u8), (1, 1)] {
        let mine = said(&g, Some(i));
        assert!(mine.iter().any(|l| lines(body, Cue::Start).contains(l)), "fighter {i} speaks first: {mine:?}");
    }
    assert!(g.voice.iter().filter(|v| v.who.is_none()).all(|v| v.subtitle), "the announcer is always subtitled");
}

/// Bigger matches have no intro card; rivals in the same match greet each other with their
/// `vs_` lines as it starts.
#[test]
fn rivals_greet_each_other_when_they_meet() {
    let mut g = match_of(Mode::Ffa, &[2, 3, 0, 1], 10);
    until(&mut g, 30.0, |g| g.mt.phase == Phase::Countdown);
    assert_eq!(g.intro_pair(), None, "four fighters: no 1v1 card");
    until(&mut g, 10.0, |g| g.mt.phase == Phase::Fight);
    run(&mut g, 3.0);
    assert!(said(&g, Some(0)).contains(&lore::vs_line(2, 3).unwrap().to_string()), "Anvil to Ilya: {:?}", g.voice);
    assert!(said(&g, Some(1)).contains(&lore::vs_line(3, 2).unwrap().to_string()), "Ilya back to Anvil");
}

/// A kill: the one who fell says a death line, the one who did it a kill line (both shown to
/// you, it's your fight); an execution's line only goes over the head (its own card shows it).
/// Taking down a Writ holder brings the announcer's call.
#[test]
fn kills_deaths_finishers_and_writs_are_voiced() {
    let mut g = match_of(Mode::Ffa, &[3, 2], 20);
    until(&mut g, 30.0, |g| g.mt.phase == Phase::Fight);
    run(&mut g, 4.0);
    g.voice.clear();
    for i in 0..2 {
        g.f[i].spawn_prot = 0.0;
    }
    g.streaks[1] = WRIT_STREAK;
    kill(&mut g, 0, 1, Weapon::Rifle as u8);
    let death = g.voice.iter().find(|v| v.who == Some(1)).expect("a death line");
    assert!(lines(2, Cue::Death).contains(&death.text) && death.subtitle);
    let k = g.voice.iter().find(|v| v.who == Some(0)).expect("a kill line");
    assert!(lines(3, Cue::Kill).contains(&k.text) && k.subtitle);
    assert!(said(&g, None).iter().any(|l| announcer_lines(Call::WritClaimed).contains(l)), "the Writ is called: {:?}", g.voice);
    // an execution: the finisher line, over the head only
    until(&mut g, 20.0, |g| g.f[1].alive && g.f[0].alive);
    assert!(g.f[1].alive, "back for another go");
    g.voice.clear();
    for i in 0..2 {
        g.f[i].spawn_prot = 0.0;
    }
    kill(&mut g, 0, 1, EXEC_TOOL);
    let fin = g.voice.iter().find(|v| v.who == Some(0)).expect("a finisher line");
    assert!(lines(3, Cue::Finisher).contains(&fin.text));
    assert!(!fin.subtitle, "the execution's card already shows it");
    // lines fade, and never pile up
    run(&mut g, VOICE_LIFE + 1.5);
    assert!(g.voice.len() <= VOICE_MAX);
    assert!(g.voice.iter().all(|v| v.t < VOICE_LIFE));
}

/// Rounds: each one won is called by the announcer; the match's end too.
#[test]
fn round_and_match_wins_are_announced() {
    let mut g = match_of(Mode::Duel, &[0, 2], 2);
    let mut match_called = false;
    for _ in 0..(90.0 / TICK) as usize {
        if g.mt.phase == Phase::MatchOver {
            break;
        }
        if g.mt.phase == Phase::Fight && g.mt.round_end <= 0.0 && g.f[1].alive && g.f[0].alive && g.f[1].spawn_prot <= 0.0 {
            kill(&mut g, 0, 1, Weapon::Rifle as u8);
        }
        g.tick(&Input::default());
        match_called |= g.voice.iter().any(|v| v.who.is_none() && announcer_lines(Call::MatchWin).contains(&v.text));
    }
    assert_eq!(g.mt.phase, Phase::MatchOver, "the match ends");
    assert!(match_called, "the match's end is called");
    // (each round's call happens once: the watch only fires as a round turns over)
    let mut g = match_of(Mode::Duel, &[0, 2], 5);
    until(&mut g, 30.0, |g| g.mt.phase == Phase::Fight);
    run(&mut g, 4.0);
    g.voice.clear();
    for i in 0..2 {
        g.f[i].spawn_prot = 0.0;
    }
    kill(&mut g, 0, 1, Weapon::Rifle as u8);
    run(&mut g, 1.0);
    let calls = g.voice.iter().filter(|v| v.who.is_none() && announcer_lines(Call::RoundWin).contains(&v.text)).count();
    assert_eq!(calls, 1, "the round's win, called once: {:?}", g.voice);
}

/// No voices in the menu's background fight or in training.
#[test]
fn the_menu_fight_and_training_stay_quiet() {
    let mut d = Game::demo();
    run(&mut d, 20.0);
    assert!(d.voice.is_empty(), "menu fight: {:?}", d.voice);
    let mut t = Game::training("A");
    run(&mut t, 5.0);
    assert!(t.voice.is_empty());
    assert_eq!(t.intro_pair(), None);
}
