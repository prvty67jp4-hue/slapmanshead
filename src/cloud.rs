//! Steam Cloud saves: the bank, the outfits (in the hub save), settings, the career, the ranked
//! record and the achievements follow the player between computers.
//!
//! At start-up each file is compared with its cloud copy: the same, nothing to do; different,
//! the newer one wins and the other is kept beside the save as `<name>.cloudbak`, so a conflict
//! never loses anything. While playing, a save that changed is uploaded about every half minute
//! and when the game closes. Under the test App ID 480 (or with Steam Cloud off) nothing is sent
//! and the saves stay on this computer.

use crate::platform::steam_api::SteamApi;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The saves that sync: (cloud name, file on this computer).
pub fn files(account: u64) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, Option<PathBuf>)> = vec![
        ("settings.txt".into(), crate::platform::data_file("settings.txt")),
        ("career.txt".into(), crate::platform::data_file("career.txt")),
        ("hub.txt".into(), crate::hub_ui::hub_save_path()),
        ("achievements.txt".into(), crate::achievements::path()),
        ("season.txt".into(), crate::season::path()),
    ];
    if let Some(p) = crate::items::bank_path(account) {
        v.push((p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), Some(p)));
    }
    if let Some(p) = crate::ranked::path(account) {
        v.push((p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), Some(p)));
    }
    v.into_iter().filter_map(|(n, p)| Some((n, p?))).filter(|(n, _)| !n.is_empty()).collect()
}

/// What happened to one save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The same here and in the cloud (or neither has it).
    Same,
    /// This computer's copy was newer (or the cloud had none): sent up. The cloud's older copy, if
    /// it differed, is kept here as the backup.
    Uploaded { backup: bool },
    /// The cloud's copy was newer: it replaced this one, which is kept as the backup.
    Downloaded { backup: bool },
    /// Steam Cloud isn't on (test App ID, or switched off).
    Off,
    Failed(String),
}

/// The backup kept beside a save when a conflict was settled.
pub fn backup_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".cloudbak");
    PathBuf::from(s)
}

fn modified(p: &Path) -> Option<i64> {
    let t = std::fs::metadata(p).ok()?.modified().ok()?;
    Some(t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64)
}

fn write_atomic(p: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut tmp = p.as_os_str().to_owned();
    tmp.push(".cloudtmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, p).or_else(|_| std::fs::write(p, bytes))
}

/// Settle one save with its cloud copy: newest wins, the other is kept as the backup.
pub fn sync_file(api: &mut dyn SteamApi, name: &str, path: &Path) -> Outcome {
    if !api.features().cloud {
        return Outcome::Off;
    }
    let local = std::fs::read(path).ok();
    let cloud = api.cloud_read(name);
    match (local, cloud) {
        (None, None) => Outcome::Same,
        (Some(l), None) => {
            if api.cloud_write(name, &l) { Outcome::Uploaded { backup: false } } else { Outcome::Failed("Steam Cloud refused the upload".into()) }
        }
        (None, Some(c)) => match write_atomic(path, &c) {
            Ok(()) => Outcome::Downloaded { backup: false },
            Err(e) => Outcome::Failed(e.to_string()),
        },
        (Some(l), Some(c)) if l == c => Outcome::Same,
        (Some(l), Some(c)) => {
            let lt = modified(path).unwrap_or(0);
            let ct = api.cloud_time(name).unwrap_or(0);
            if ct > lt {
                // the cloud is newer: keep ours aside, take theirs
                let _ = std::fs::write(backup_path(path), &l);
                match write_atomic(path, &c) {
                    Ok(()) => Outcome::Downloaded { backup: true },
                    Err(e) => Outcome::Failed(e.to_string()),
                }
            } else {
                // ours is newer: keep the cloud's aside, send ours
                let _ = std::fs::write(backup_path(path), &c);
                if api.cloud_write(name, &l) { Outcome::Uploaded { backup: true } } else { Outcome::Failed("Steam Cloud refused the upload".into()) }
            }
        }
    }
}

/// The sync's state for this run: when each save was last sent (its modified time then).
#[derive(Default, Debug)]
pub struct Cloud {
    pub sent: HashMap<String, Option<i64>>,
    pub tick: f32,
    /// What the start-up sync did, for the Settings page (a line per save that changed).
    pub report: Vec<String>,
    pub started: bool,
}

impl Cloud {
    /// The start-up sync. Returns the saves that came down from the cloud (so they're read again).
    pub fn start(&mut self, api: &mut dyn SteamApi, account: u64) -> Vec<String> {
        self.started = true;
        let mut down = Vec::new();
        if !api.features().cloud {
            return down;
        }
        for (name, path) in files(account) {
            let o = sync_file(api, &name, &path);
            match &o {
                Outcome::Downloaded { backup } => {
                    self.report.push(format!("{name}: the newer copy came from Steam Cloud{}", if *backup { " (this computer's is kept as a backup)" } else { "" }));
                    down.push(name.clone());
                }
                Outcome::Uploaded { backup: true } => self.report.push(format!("{name}: this computer's newer copy went to Steam Cloud (the cloud's is kept as a backup)")),
                Outcome::Failed(e) => self.report.push(format!("{name}: couldn't sync ({e})")),
                _ => {}
            }
            self.sent.insert(name, modified(&path));
        }
        down
    }

    /// While playing: send saves that changed (about every half minute, or now when `force`).
    pub fn upkeep(&mut self, api: &mut dyn SteamApi, account: u64, dt: f32, force: bool) -> usize {
        self.tick -= dt;
        if (!force && self.tick > 0.0) || !api.features().cloud {
            return 0;
        }
        self.tick = 30.0;
        let mut n = 0;
        for (name, path) in files(account) {
            let m = modified(&path);
            if m.is_some() && self.sent.get(&name) != Some(&m) {
                if let Ok(bytes) = std::fs::read(&path) {
                    if api.cloud_write(&name, &bytes) {
                        n += 1;
                    }
                }
                self.sent.insert(name, m);
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::steam_api::FakeSteam;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("duel_cloud_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn set_time(p: &Path, secs: i64) {
        let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
        f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64)).unwrap();
    }

    #[test]
    fn newest_wins_and_the_other_copy_is_kept() {
        let d = dir("conflict");
        let mut steam = FakeSteam::new(1);
        let save = d.join("career.txt");
        // nothing anywhere: nothing to do; only here: it goes up
        assert_eq!(sync_file(&mut steam, "career.txt", &save), Outcome::Same);
        std::fs::write(&save, "wins=3\n").unwrap();
        assert_eq!(sync_file(&mut steam, "career.txt", &save), Outcome::Uploaded { backup: false });
        assert_eq!(steam.cloud_read("career.txt").as_deref(), Some(&b"wins=3\n"[..]));
        assert_eq!(sync_file(&mut steam, "career.txt", &save), Outcome::Same, "already in step");
        // another computer played on and synced later: its copy is newer
        set_time(&save, 1_700_000_000);
        steam.now = 1_800_000_000;
        steam.cloud_write("career.txt", b"wins=9\n");
        assert_eq!(sync_file(&mut steam, "career.txt", &save), Outcome::Downloaded { backup: true });
        assert_eq!(std::fs::read_to_string(&save).unwrap(), "wins=9\n");
        assert_eq!(std::fs::read_to_string(backup_path(&save)).unwrap(), "wins=3\n", "ours kept");
        // now this computer is newer than the cloud
        std::fs::write(&save, "wins=11\n").unwrap();
        set_time(&save, 1_900_000_000);
        assert_eq!(sync_file(&mut steam, "career.txt", &save), Outcome::Uploaded { backup: true });
        assert_eq!(steam.cloud_read("career.txt").as_deref(), Some(&b"wins=11\n"[..]));
        assert_eq!(std::fs::read_to_string(backup_path(&save)).unwrap(), "wins=9\n", "the cloud's kept");
        // a new computer: the cloud fills it in
        let fresh = d.join("fresh").join("career.txt");
        assert_eq!(sync_file(&mut steam, "career.txt", &fresh), Outcome::Downloaded { backup: false });
        assert_eq!(std::fs::read_to_string(&fresh).unwrap(), "wins=11\n");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn under_the_test_app_saves_stay_here() {
        let d = dir("test_app");
        let save = d.join("settings.txt");
        std::fs::write(&save, "name=x\n").unwrap();
        let mut steam = FakeSteam::new(1).as_test_app();
        assert_eq!(sync_file(&mut steam, "settings.txt", &save), Outcome::Off);
        let mut c = Cloud::default();
        assert!(c.start(&mut steam, 0).is_empty());
        assert_eq!(c.upkeep(&mut steam, 0, 99.0, true), 0);
        assert!(steam.cloud.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn every_save_the_brief_names_syncs() {
        let names: Vec<String> = files(76_561_198_000_000_001).into_iter().map(|f| f.0).collect();
        for want in ["settings.txt", "career.txt", "hub.txt", "achievements.txt", "bank_76561198000000001.dat", "ranked_76561198000000001.dat"] {
            assert!(names.iter().any(|n| n == want), "{want} in {names:?}");
        }
    }
}
