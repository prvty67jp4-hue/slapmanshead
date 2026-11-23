//! Freeze watchdog: the game loop checks in every frame; a helper thread notices when it hasn't
//! for over 3 seconds and writes a hang report (what was on screen, the match phase, the kill
//! cam, slow motion, the camera, the last events, and which part of the frame it stopped in)
//! to `hang_report.txt` in the Bug reports folder (see `reports`). When the game comes back it notes how long the
//! stall lasted.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A stall longer than this is reported.
pub const STALL: Duration = Duration::from_secs(3);

struct Shared {
    start: Instant,
    /// Milliseconds since `start` at the last check-in.
    beat: AtomicU64,
    /// The part of the frame running now (sim, render, hud, ui, sound...).
    stage: Mutex<&'static str>,
    /// What the game looked like at the last check-in.
    snapshot: Mutex<String>,
    stop: AtomicBool,
    /// Not drawing on purpose (the window is minimised): a pause isn't a freeze.
    hold: AtomicBool,
}

pub struct Watchdog {
    shared: Arc<Shared>,
    path: Option<PathBuf>,
}

pub fn report_path() -> Option<PathBuf> {
    Some(crate::reports::dir()?.join("hang_report.txt"))
}

/// The game's last check-in picture, for a crash report (empty before the first frame).
static LAST_SNAPSHOT: Mutex<String> = Mutex::new(String::new());

pub fn last_snapshot() -> String {
    // (a crash while the snapshot is being written: say so rather than wait)
    LAST_SNAPSHOT.try_lock().map(|s| s.clone()).unwrap_or_else(|_| "(being updated when it crashed)".to_string())
}

impl Watchdog {
    /// Start watching, reporting to `path` (None: only keep count, write nothing).
    pub fn start(path: Option<PathBuf>) -> Watchdog {
        let shared = Arc::new(Shared {
            start: Instant::now(),
            beat: AtomicU64::new(0),
            stage: Mutex::new("start-up"),
            snapshot: Mutex::new(String::new()),
            stop: AtomicBool::new(false),
            hold: AtomicBool::new(false),
        });
        let s2 = shared.clone();
        let p2 = path.clone();
        let _ = std::thread::Builder::new().name("watchdog".into()).spawn(move || watch(s2, p2));
        Watchdog { shared, path }
    }

    /// The game loop is alive (call once a frame).
    pub fn beat(&self) {
        let ms = self.shared.start.elapsed().as_millis() as u64;
        self.shared.beat.store(ms, Ordering::Relaxed);
    }

    /// Which part of the frame is running now.
    pub fn stage(&self, name: &'static str) {
        if let Ok(mut s) = self.shared.stage.lock() {
            *s = name;
        }
    }

    /// What the game looks like now, for a report.
    pub fn snapshot(&self, text: String) {
        if let Ok(mut s) = LAST_SNAPSHOT.lock() {
            s.clone_from(&text);
        }
        if let Ok(mut s) = self.shared.snapshot.lock() {
            *s = text;
        }
    }

    /// The window is minimised (or otherwise not drawing on purpose): don't report the pause.
    pub fn hold(&self, on: bool) {
        self.shared.hold.store(on, Ordering::Relaxed);
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

fn watch(sh: Arc<Shared>, path: Option<PathBuf>) {
    // the stall being reported (its start, in ms), so each one is written once
    let mut reported: Option<u64> = None;
    let mut worst = 0u64;
    while !sh.stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(200));
        let now = sh.start.elapsed().as_millis() as u64;
        let beat = sh.beat.load(Ordering::Relaxed);
        let quiet = now.saturating_sub(beat);
        match reported {
            None if beat > 0 && quiet > STALL.as_millis() as u64 && !sh.hold.load(Ordering::Relaxed) => {
                reported = Some(beat);
                worst = quiet;
                let stage = sh.stage.lock().map(|s| *s).unwrap_or("?");
                let snap = sh.snapshot.lock().map(|s| s.clone()).unwrap_or_default();
                write_report(path.as_ref(), &report_text(quiet, stage, &snap));
            }
            Some(at) if beat != at => {
                // it came back
                write_report(path.as_ref(), &format!("    ...the game came back after {:.1} s\n\n", (beat.saturating_sub(at).max(worst)) as f64 / 1000.0));
                reported = None;
            }
            Some(_) => worst = worst.max(quiet),
            None => {}
        }
    }
}

pub fn report_text(quiet_ms: u64, stage: &str, snapshot: &str) -> String {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!(
        "=== Duel Rush hang report (unix time {stamp}, {}, version {}) ===\nThe game loop stopped for over {:.1} s.\nStuck in: {stage}\n{snapshot}\n",
        crate::reports::stamp(),
        crate::VERSION,
        quiet_ms as f64 / 1000.0
    )
}

fn write_report(path: Option<&PathBuf>, text: &str) {
    eprint!("{text}");
    let Some(p) = path else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // keep the file small: start over once it passes 256 KB
    let big = std::fs::metadata(p).map(|m| m.len() > 256 * 1024).unwrap_or(false);
    let file = if big { std::fs::File::create(p) } else { std::fs::OpenOptions::new().create(true).append(true).open(p) };
    if let Ok(mut f) = file {
        let _ = f.write_all(text.as_bytes());
        let _ = f.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stalled_loop_gets_a_report_and_a_live_one_does_not() {
        let path = std::env::temp_dir().join(format!("duel_hang_test_{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let wd = Watchdog::start(Some(path.clone()));
        // a lively loop: no report
        for _ in 0..20 {
            wd.stage("sim");
            wd.snapshot("screen: Play\nphase: Fight".into());
            wd.beat();
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!path.exists(), "no report while frames keep coming");
        // stuck in the kill cam
        wd.stage("render");
        wd.snapshot("screen: Play\nphase: MatchOver\nkill cam: victim 0 t 0.40".into());
        std::thread::sleep(STALL + Duration::from_millis(700));
        let text = std::fs::read_to_string(&path).expect("a hang report");
        assert!(text.contains("Stuck in: render") && text.contains("phase: MatchOver") && text.contains("kill cam"), "{text}");
        // and it says when it came back
        wd.beat();
        std::thread::sleep(Duration::from_millis(500));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("came back"), "{text}");
        drop(wd);
        let _ = std::fs::remove_file(&path);
    }
}
