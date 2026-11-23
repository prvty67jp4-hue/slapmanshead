//! Bug reports: everything a playtester should send lives in one folder they can find,
//! `Bug reports` beside the save files (on Windows `%APPDATA%\DuelRush\Bug reports`, on Linux and
//! the Steam Deck `~/.local/share/DuelRush/Bug reports`):
//!
//! - `hang_report.txt`: freezes the watchdog caught (see `watchdog`),
//! - `crash-<time>.txt`: a crash, with what went wrong, where, and what the game looked like,
//! - `bug-report-<time>.txt`: written by the Report a bug button (version, system, graphics,
//!   settings and the game's state right then).
//!
//! The button opens the folder with the newest report picked out.

use std::path::PathBuf;

/// The folder's name inside the save folder.
pub const FOLDER: &str = "Bug reports";
/// Crash and button reports kept (the oldest go first).
const KEEP: usize = 40;

pub fn dir() -> Option<PathBuf> {
    crate::platform::data_file(FOLDER)
}

/// The time now (UTC) as `2026-11-23_02-41-05`, for file names and report headers.
pub fn stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02}_{:02}-{:02}-{:02}", rem / 3600, (rem % 3600) / 60, rem % 60)
}

/// Days since 1970-01-01 to a calendar date (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The lines every report starts with: the build, the system and how long the game had run.
pub fn header(kind: &str) -> String {
    format!(
        "=== Duel Rush {kind} ===\ntime (UTC): {}\nversion: {}\nsystem: {} ({} {})\nmemory in use: {}\n",
        { let s = stamp(); format!("{} {}", &s[..10], s[11..].replace('-', ":")) },
        crate::VERSION,
        crate::platform::system().name(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        crate::platform::memory_mb().map_or("unknown".to_string(), |m| format!("{m:.0} MB")),
    )
}

/// Write a report into the folder as `<prefix>-<time>.txt`; returns where it went.
pub fn write(prefix: &str, text: &str) -> Option<PathBuf> {
    let d = dir()?;
    std::fs::create_dir_all(&d).ok()?;
    let mut path = d.join(format!("{prefix}-{}.txt", stamp()));
    let mut n = 2;
    while path.exists() {
        path = d.join(format!("{prefix}-{}-{n}.txt", stamp()));
        n += 1;
    }
    std::fs::write(&path, text).ok()?;
    prune(&d);
    Some(path)
}

/// Keep the folder small: only the newest crash and button reports.
fn prune(d: &std::path::Path) {
    let Ok(rd) = std::fs::read_dir(d) else { return };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("crash-") || n.starts_with("bug-report-")))
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .collect();
    if files.len() <= KEEP {
        return;
    }
    files.sort();
    for (_, p) in &files[..files.len() - KEEP] {
        let _ = std::fs::remove_file(p);
    }
}

/// From now on a crash writes `crash-<time>.txt` (what went wrong, where, the call stack and the
/// game's last state) before the game closes.
pub fn install_crash_log() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let what = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".to_string());
        let place = info.location().map_or("unknown".to_string(), |l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let stack = std::backtrace::Backtrace::force_capture();
        let text = format!(
            "{}\nThe game crashed.\nwhat: {what}\nwhere: {place}\nthread: {thread}\n\nwhat the game looked like:\n{}\n\ncall stack:\n{stack}\n",
            header("crash report"),
            crate::watchdog::last_snapshot(),
        );
        let _ = write("crash", &text);
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_and_reports_land_in_the_folder() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_415), (2025, 11, 23));
        let s = stamp();
        assert_eq!(s.len(), 19, "{s}");
        // (DUEL_DATA_DIR points the save folder elsewhere for the test)
        let tmp = std::env::temp_dir().join(format!("duel_reports_test_{}", std::process::id()));
        // SAFETY: tests touching this variable run in this one test only
        unsafe { std::env::set_var("DUEL_DATA_DIR", &tmp) };
        let p = write("bug-report", &header("bug report")).expect("written");
        assert!(p.starts_with(tmp.join(FOLDER)), "{p:?}");
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains(crate::VERSION) && text.contains("system:"), "{text}");
        for _ in 0..(KEEP + 5) {
            write("crash", "x");
        }
        let n = std::fs::read_dir(tmp.join(FOLDER)).unwrap().count();
        assert!(n <= KEEP + 1, "old reports are cleared out: {n}");
        unsafe { std::env::remove_var("DUEL_DATA_DIR") };
        let _ = std::fs::remove_dir_all(&tmp);
    }
}