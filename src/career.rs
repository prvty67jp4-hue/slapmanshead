//! Career stats kept on this computer: what you've done across all your matches, the XP it
//! earned and your level. Finisher styles unlock by level (all at level 1 for now: they are
//! meant to become cosmetics).

use crate::game::{CareerEv, FINISHERS};
use std::path::PathBuf;

/// XP for each thing you do.
pub const XP_KILL: u32 = 10;
pub const XP_HEADSHOT: u32 = 5;
/// On top of the kill.
pub const XP_FINISHER: u32 = 50;
pub const XP_WAVE: u32 = 20;
pub const XP_MATCH: u32 = 25;
/// XP from one level to the next.
pub const XP_PER_LEVEL: u32 = 500;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Career {
    pub kills: u32,
    pub deaths: u32,
    pub headshots: u32,
    pub finishers: u32,
    /// Finishers done with each style (see game::FINISHERS).
    pub fin_by: [u32; FINISHERS.len()],
    pub matches: u32,
    pub waves: u32,
    pub xp: u32,
}

fn path() -> Option<PathBuf> {
    crate::platform::data_file("career.txt")
}

impl Career {
    pub fn load() -> Career {
        path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| Career::from_text(&t)).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(p) = path() {
            let _ = crate::platform::save_file(&p, &self.to_text());
        }
    }

    pub fn to_text(&self) -> String {
        let by: Vec<String> = self.fin_by.iter().map(|n| n.to_string()).collect();
        format!(
            "kills={}\ndeaths={}\nheadshots={}\nfinishers={}\nfin_by={}\nmatches={}\nwaves={}\nxp={}\n",
            self.kills,
            self.deaths,
            self.headshots,
            self.finishers,
            by.join(","),
            self.matches,
            self.waves,
            self.xp
        )
    }

    pub fn from_text(t: &str) -> Career {
        let mut c = Career::default();
        for line in t.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let n = || v.trim().parse::<u32>().unwrap_or(0);
            match k.trim() {
                "kills" => c.kills = n(),
                "deaths" => c.deaths = n(),
                "headshots" => c.headshots = n(),
                "finishers" => c.finishers = n(),
                "matches" => c.matches = n(),
                "waves" => c.waves = n(),
                "xp" => c.xp = n(),
                "fin_by" => {
                    for (slot, x) in c.fin_by.iter_mut().zip(v.split(',')) {
                        *slot = x.trim().parse().unwrap_or(0);
                    }
                }
                _ => {}
            }
        }
        c
    }

    /// Count one thing; returns the XP it earned.
    pub fn apply(&mut self, ev: CareerEv) -> u32 {
        let xp = match ev {
            CareerEv::Kill { head, finisher } => {
                self.kills += 1;
                let mut xp = XP_KILL;
                if head {
                    self.headshots += 1;
                    xp += XP_HEADSHOT;
                }
                if finisher > 0 {
                    self.finishers += 1;
                    if let Some(n) = self.fin_by.get_mut(finisher as usize - 1) {
                        *n += 1;
                    }
                    xp += XP_FINISHER;
                }
                xp
            }
            CareerEv::Death => {
                self.deaths += 1;
                0
            }
            CareerEv::WaveCleared => {
                self.waves += 1;
                XP_WAVE
            }
            CareerEv::MatchEnd => {
                self.matches += 1;
                XP_MATCH
            }
        };
        self.xp += xp;
        xp
    }

    pub fn level(&self) -> u32 {
        1 + self.xp / XP_PER_LEVEL
    }

    /// Share of the way to the next level.
    pub fn level_progress(&self) -> f32 {
        (self.xp % XP_PER_LEVEL) as f32 / XP_PER_LEVEL as f32
    }

    /// The finisher style used most (if any yet).
    pub fn favourite_finisher(&self) -> Option<usize> {
        let (k, n) = self.fin_by.iter().enumerate().max_by_key(|(_, n)| **n)?;
        (*n > 0).then_some(k)
    }

    pub fn unlocked(&self, style: usize) -> bool {
        FINISHERS.get(style).is_some_and(|s| self.level() >= s.unlock_level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_earn_xp_and_survive_saving() {
        let mut c = Career::default();
        assert_eq!(c.apply(CareerEv::Kill { head: true, finisher: 0 }), XP_KILL + XP_HEADSHOT);
        assert_eq!(c.apply(CareerEv::Kill { head: false, finisher: 4 }), XP_KILL + XP_FINISHER);
        c.apply(CareerEv::Death);
        c.apply(CareerEv::MatchEnd);
        assert_eq!((c.kills, c.headshots, c.finishers, c.deaths, c.matches), (2, 1, 1, 1, 1));
        assert_eq!(c.fin_by[3], 1);
        assert_eq!(c.favourite_finisher(), Some(3));
        assert_eq!(Career::from_text(&c.to_text()), c);
        c.xp = XP_PER_LEVEL * 2 + XP_PER_LEVEL / 2;
        assert_eq!(c.level(), 3);
        assert!((c.level_progress() - 0.5).abs() < 1e-6);
        assert!((0..FINISHERS.len()).all(|k| c.unlocked(k)));
    }
}
