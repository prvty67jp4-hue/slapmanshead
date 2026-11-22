//! Everything that differs between Windows, Mac, Linux / Steam Deck (and, later, consoles) lives
//! here, so the rest of the game never asks which system it runs on:
//!
//! - where saves go (settings, career, presets, hang reports),
//! - Steam (lobbies, relayed networking, leaderboards) in [`steam`],
//! - the Windows Firewall check in [`firewall`],
//! - how the mouse is held during play,
//! - which device this is (a Steam Deck gets settings that hold 60 frames a second).
//!
//! A console port adds its own branch to each function here (and its own online service beside
//! `steam`); nothing else should need a `cfg(target_os)`.

pub mod firewall;
pub mod steam;

use std::path::PathBuf;

/// The game's folder name inside the system's save location.
const FOLDER: &str = "DuelRush";

/// Which system the game is running on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    Windows,
    Mac,
    Linux,
    /// Linux on Valve's handheld: a 1280x800 screen, a built-in controller, a modest GPU.
    SteamDeck,
}

impl System {
    pub fn name(self) -> &'static str {
        match self {
            System::Windows => "Windows",
            System::Mac => "Mac",
            System::Linux => "Linux",
            System::SteamDeck => "Steam Deck",
        }
    }

    /// A handheld or TV device: controller first, big text, a 60 fps budget.
    pub fn couch(self) -> bool {
        self == System::SteamDeck
    }
}

/// The system we're running on (`DUEL_DEVICE=deck` pretends to be a Steam Deck, for testing).
pub fn system() -> System {
    if std::env::var("DUEL_DEVICE").is_ok_and(|v| v.eq_ignore_ascii_case("deck")) {
        return System::SteamDeck;
    }
    if cfg!(windows) {
        System::Windows
    } else if cfg!(target_os = "macos") {
        System::Mac
    } else if is_steam_deck() {
        System::SteamDeck
    } else {
        System::Linux
    }
}

/// Steam sets `SteamDeck=1` for games on the Deck; its board names are Jupiter (LCD) and
/// Galileo (OLED).
fn is_steam_deck() -> bool {
    if std::env::var("SteamDeck").is_ok_and(|v| v == "1") {
        return true;
    }
    if cfg!(target_os = "linux") {
        if let Ok(name) = std::fs::read_to_string("/sys/devices/virtual/dmi/id/product_name") {
            let n = name.trim();
            return n == "Jupiter" || n == "Galileo";
        }
    }
    false
}

/// Where the game keeps its files:
/// - Windows: `%APPDATA%\DuelRush`
/// - Mac: `~/Library/Application Support/DuelRush`
/// - Linux and Steam Deck: `$XDG_DATA_HOME/DuelRush` (normally `~/.local/share/DuelRush`)
///
/// Saves from older builds in `~/DuelRush` keep being used where they already are.
pub fn data_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("DUEL_DATA_DIR") {
        return Some(PathBuf::from(dir));
    }
    if cfg!(windows) {
        let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("USERPROFILE"))?;
        return Some(PathBuf::from(base).join(FOLDER));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let legacy = home.join(FOLDER);
    if legacy.is_dir() {
        return Some(legacy);
    }
    if cfg!(target_os = "macos") {
        return Some(home.join("Library").join("Application Support").join(FOLDER));
    }
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(".local").join("share"));
    Some(base.join(FOLDER))
}

/// A file in the game's folder (settings.txt, career.txt, hang_report.txt...).
pub fn data_file(name: &str) -> Option<PathBuf> {
    Some(data_dir()?.join(name))
}

/// Write a save file, making its folder first. Writes a temporary file and swaps it in, so a
/// crash or power loss (a handheld's battery running out) never leaves a half-written save.
pub fn save_file(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).or_else(|_| std::fs::write(path, text))
}

/// How to hold the mouse during play: macOS can lock it in place, elsewhere it's kept inside
/// the window (and put back in the middle each frame).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseHold {
    Locked,
    Confined,
}

pub fn mouse_hold() -> MouseHold {
    if cfg!(target_os = "macos") { MouseHold::Locked } else { MouseHold::Confined }
}

/// Test screenshots open their window behind everything else where the system allows it.
pub fn test_window_behind() -> bool {
    cfg!(windows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_go_in_one_folder_and_survive_rewrites() {
        let dir = std::env::temp_dir().join(format!("duel_platform_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let f = dir.join("nested").join("settings.txt");
        save_file(&f, "a=1\n").unwrap();
        save_file(&f, "a=2\n").unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "a=2\n");
        assert!(!f.with_extension("tmp").exists(), "no temporary file left behind");
        let _ = std::fs::remove_dir_all(&dir);
        let d = data_dir().expect("a save folder");
        assert!(d.ends_with(FOLDER) || std::env::var_os("DUEL_DATA_DIR").is_some());
        assert_eq!(data_file("career.txt").unwrap(), d.join("career.txt"));
        assert!(!system().name().is_empty());
    }
}
