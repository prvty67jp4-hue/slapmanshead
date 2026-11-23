//! The auto-updater. Every release on GitHub (the `latest` release of the game's public repo,
//! made by `.github/workflows/build.yml`) carries a `version.json`: the build number, the
//! version, a summary of what changed, and for each platform the download's file name and its
//! SHA-256. On start (in the background, never holding up the menu) the game reads it; if the
//! build is newer it offers the update. Updating downloads this platform's zip (with progress),
//! checks its SHA-256 (a mismatch is refused), swaps the files and restarts:
//!
//! - Windows: the running program is renamed `.old` (Windows allows that), the new program and
//!   `steam_api64.dll` go in its place, the new one starts, and the `.old` files are deleted on
//!   the next start.
//! - Mac: the app bundle's contents are replaced (its `libsteam_api.dylib` kept), the same way.
//! - Linux / Steam Deck: the update is offered as a download (the releases page opens).
//!
//! Anything that goes wrong puts the old files back, so the current version keeps working, and
//! says "Update failed - download manually". A development build (one not made by the release
//! workflow, with no build number) never updates itself.

use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The game's public repository on GitHub.
pub const REPO: &str = "prvty67jp4-hue/slapmanshead";
/// This build's number (GitHub's run number; 0 for a development build).
pub const BUILD: u32 = parse_u32(env!("DUEL_BUILD_NUMBER"));

const fn parse_u32(s: &str) -> u32 {
    let b = s.as_bytes();
    let mut i = 0;
    let mut n: u32 = 0;
    while i < b.len() {
        let d = b[i];
        if d < b'0' || d > b'9' {
            return 0;
        }
        n = n * 10 + (d - b'0') as u32;
        i += 1;
    }
    n
}

/// This build's number, or a pretend one for testing the updater (DUEL_FAKE_BUILD).
pub fn local_build() -> u32 {
    std::env::var("DUEL_FAKE_BUILD").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(BUILD)
}

/// Where the latest release's files are.
pub fn manifest_url() -> String {
    std::env::var("DUEL_UPDATE_URL").unwrap_or_else(|_| format!("https://github.com/{REPO}/releases/download/latest/version.json"))
}
pub fn file_url(name: &str) -> String {
    let base = manifest_url();
    match base.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{name}"),
        None => name.to_string(),
    }
}
/// The page to download it by hand.
pub fn releases_page() -> String {
    format!("https://github.com/{REPO}/releases/latest")
}

/// Can two builds play together by build number alone? The same build, or a development build on
/// either side (a developer testing). Only used for copies from before the network protocol
/// number (see can_play_together).
pub fn builds_compatible(a: u32, b: u32) -> bool {
    a == 0 || b == 0 || a == b
}

/// The network protocol this copy speaks, or a pretend one for testing (DUEL_FAKE_PROTO).
pub fn local_proto() -> u32 {
    std::env::var("DUEL_FAKE_PROTO").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(crate::net::NET_PROTOCOL)
}

/// Can this copy (`my_build`, `my_proto`) play with another? When the other says its network
/// protocol, that decides: the same protocol plays together whatever the build numbers (a Mac
/// download a few builds behind still joins). A copy from before the protocol number (`their_proto`
/// None) has to be the same build.
pub fn can_play_together(my_build: u32, my_proto: u32, their_build: u32, their_proto: Option<u32>) -> bool {
    match their_proto {
        Some(p) => p == my_proto,
        None => builds_compatible(my_build, their_build),
    }
}

/// What to tell a player who can't join (`host`'s build and protocol, as it said).
pub fn mismatch_text(my_build: u32, my_proto: u32, host_build: u32, host_proto: Option<u32>) -> String {
    let newer = match host_proto {
        Some(p) if p != my_proto => p > my_proto,
        _ => host_build > my_build,
    };
    let what = if newer { "update first (Settings > Updates, or the downloads page)" } else { "the host needs to update first" };
    match host_proto {
        Some(p) => format!("Your game (build {my_build}, network version {my_proto}) can't play with the host's (build {host_build}, network version {p}): the network version has to match - {what}."),
        None => format!("Your build ({my_build}) doesn't match the host's ({host_build}) - {what}."),
    }
}

/// Is `remote` an update for `local`? Only a newer build, and never for a development build.
pub fn is_newer(remote: u32, local: u32) -> bool {
    local > 0 && remote > local
}

/// This platform's key in version.json.
pub fn platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

/// One platform's download.
#[derive(Clone, Debug, PartialEq)]
pub struct Download {
    pub file: String,
    pub sha256: String,
    /// The build this file is, when it isn't the release's own (a Mac zip kept from an earlier
    /// build because that run's Mac build didn't finish).
    pub build: Option<u32>,
    /// The network protocol that file speaks (None: not said, or not known for an old zip).
    pub protocol: Option<u32>,
}

/// What version.json says.
#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    pub build: u32,
    pub version: String,
    pub notes: String,
    pub files: Vec<(String, Download)>,
    /// The network protocol the release's own build speaks (None in releases from before it).
    pub protocol: Option<u32>,
}

/// A platform's name as players know it.
pub fn platform_name(p: &str) -> &str {
    match p {
        "windows" => "Windows",
        "mac" => "Mac",
        "linux" => "Linux / Steam Deck",
        other => other,
    }
}

impl Manifest {
    pub fn download(&self, platform: &str) -> Option<&Download> {
        self.files.iter().find(|(p, _)| p == platform).map(|(_, d)| d)
    }
    /// The build platform would get (its own file's build when the release kept an older one).
    pub fn build_for(&self, platform: &str) -> u32 {
        self.download(platform).and_then(|d| d.build).unwrap_or(self.build)
    }

    /// The network protocol platform's download speaks: its own when it says, the release's when
    /// it's the release's own build, otherwise unknown.
    pub fn protocol_for(&self, platform: &str) -> Option<u32> {
        let d = self.download(platform)?;
        d.protocol.or(if self.build_for(platform) == self.build { self.protocol } else { None })
    }

    /// The downloads that are older than the release (a build that couldn't be made in this run
    /// keeps the last one): (platform, its build).
    pub fn behind(&self) -> Vec<(&str, u32)> {
        self.files.iter().map(|(p, _)| (p.as_str(), self.build_for(p))).filter(|&(_, b)| b < self.build).collect()
    }

    /// Can a download that's behind still play with the release's build? Only when both say the
    /// same network protocol (an old download that doesn't say can't: it needs the same build).
    pub fn plays_with_release(&self, platform: &str) -> bool {
        match (self.protocol_for(platform), self.protocol) {
            (Some(a), Some(b)) => a == b,
            _ => self.build_for(platform) == self.build,
        }
    }

    /// What to tell a player about downloads that are behind, from where they stand (`me`, their
    /// platform): None when every download is the release's build.
    pub fn behind_note(&self, me: &str) -> Option<String> {
        let notes: Vec<String> = self
            .behind()
            .into_iter()
            .map(|(p, b)| {
                let name = platform_name(p);
                let plays = self.plays_with_release(p);
                if p == me {
                    let tail = if plays {
                        "It still plays with them: the network version is the same.".to_string()
                    } else {
                        format!("Until it's rebuilt it can't play with build {} (a different network version).", self.build)
                    };
                    format!("The newest {name} download is build {b}; the other platforms are on build {} (GitHub couldn't make the {name} build yet). {tail}", self.build)
                } else {
                    let tail = if plays {
                        format!("{name} players can still play with you: the network version is the same.")
                    } else {
                        format!("{name} players can't join build {} until the {name} download is rebuilt.", self.build)
                    };
                    format!("The {name} download is behind: it's still build {b} (this release is build {}). {tail}", self.build)
                }
            })
            .collect();
        (!notes.is_empty()).then(|| notes.join(" "))
    }
}

/// Read version.json.
pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("version.json isn't valid: {e}"))?;
    let build = v.get("build").and_then(|b| b.as_u64()).filter(|&b| b > 0 && b <= u32::MAX as u64).ok_or("version.json has no build number")? as u32;
    let version = v.get("version").and_then(|s| s.as_str()).unwrap_or("").to_string();
    let notes = v.get("notes").and_then(|s| s.as_str()).unwrap_or("").chars().take(2000).collect();
    let mut files = Vec::new();
    if let Some(map) = v.get("files").and_then(|f| f.as_object()) {
        for (platform, d) in map {
            let file = d.get("file").and_then(|s| s.as_str()).unwrap_or("");
            let sha = d.get("sha256").and_then(|s| s.as_str()).unwrap_or("").to_ascii_lowercase();
            // a plain file name (no paths) and a full SHA-256
            if file.is_empty() || file.contains(['/', '\\']) || file.contains("..") || sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
                continue;
            }
            let fb = d.get("build").and_then(|b| b.as_u64()).filter(|&b| b > 0 && b <= u32::MAX as u64).map(|b| b as u32);
            let fp = d.get("protocol").and_then(|b| b.as_u64()).filter(|&b| b <= u32::MAX as u64).map(|b| b as u32);
            files.push((platform.clone(), Download { file: file.to_string(), sha256: sha, build: fb, protocol: fp }));
        }
    }
    let protocol = v.get("protocol").and_then(|b| b.as_u64()).filter(|&b| b <= u32::MAX as u64).map(|b| b as u32);
    Ok(Manifest { build, version, notes, files, protocol })
}

/// The SHA-256 of some bytes, as lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex(&h.finalize())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Refuse a download whose SHA-256 isn't the one the release published.
pub fn check_hash(actual: &str, expected: &str) -> Result<(), String> {
    if !expected.is_empty() && actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!("the download is damaged or not the published file (its SHA-256 is {}..., expected {}...)", &actual[..actual.len().min(12)], &expected[..expected.len().min(12)]))
    }
}

/// The files in a zip (folders left out), with their unix modes.
pub fn read_zip(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>, Option<u32>)>, String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("the download isn't a zip: {e}"))?;
    let mut out = Vec::new();
    let mut total = 0usize;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().replace('\\', "/");
        if name.contains("..") || name.starts_with('/') {
            return Err(format!("the zip has an unsafe path: {name}"));
        }
        let mut data = Vec::with_capacity(f.size() as usize);
        f.read_to_end(&mut data).map_err(|e| e.to_string())?;
        total += data.len();
        if total > 1 << 30 {
            return Err("the zip is far bigger than the game".into());
        }
        out.push((name, data, f.unix_mode()));
    }
    Ok(out)
}

/// One file to put in place.
#[derive(Debug)]
pub struct Swap {
    pub target: PathBuf,
    pub data: Vec<u8>,
    pub executable: bool,
}

/// Windows: the new program goes where the running one is (whatever it's called), the Steam
/// library beside it.
pub fn plan_windows(exe: &Path, entries: &[(String, Vec<u8>, Option<u32>)]) -> Result<Vec<Swap>, String> {
    let dir = exe.parent().ok_or("the game's folder is unknown")?;
    let mut out = Vec::new();
    for (name, data, _) in entries {
        let base = name.rsplit('/').next().unwrap_or(name);
        if base.eq_ignore_ascii_case("Duel Rush.exe") {
            out.push(Swap { target: exe.to_path_buf(), data: data.clone(), executable: true });
        } else if base.eq_ignore_ascii_case("steam_api64.dll") {
            out.push(Swap { target: dir.join("steam_api64.dll"), data: data.clone(), executable: false });
        }
    }
    if !out.iter().any(|s| s.target == exe) {
        return Err("the download has no Duel Rush.exe".into());
    }
    Ok(out)
}

/// Mac: everything inside the bundle's Contents is replaced, except the Steam library it has.
pub fn plan_mac(exe: &Path, entries: &[(String, Vec<u8>, Option<u32>)]) -> Result<Vec<Swap>, String> {
    // .../Duel Rush.app/Contents/MacOS/duel_arena
    let contents = exe.parent().and_then(|p| p.parent()).ok_or("the app bundle is unknown")?;
    let mut out = Vec::new();
    for (name, data, mode) in entries {
        let Some((_, rest)) = name.split_once("Contents/") else { continue };
        if rest.is_empty() || rest.ends_with("libsteam_api.dylib") && contents.join(rest).exists() {
            continue;
        }
        let exec = mode.is_some_and(|m| m & 0o111 != 0) || rest.starts_with("MacOS/") && !rest.ends_with(".dylib");
        out.push(Swap { target: contents.join(rest), data: data.clone(), executable: exec });
    }
    if !out.iter().any(|s| s.target.starts_with(contents.join("MacOS")) && s.executable) {
        return Err("the download has no Mac program".into());
    }
    Ok(out)
}

fn old_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".old");
    PathBuf::from(s)
}

/// Put the new files in place: each existing file is renamed `.old` first (a running program
/// can be renamed, not overwritten), the new one written beside it and moved in. If any step
/// fails, everything done so far is undone, so the current version keeps working.
pub fn swap_files(swaps: &[Swap]) -> Result<(), String> {
    let mut done: Vec<(&Swap, bool)> = Vec::new();
    let result = (|| {
        for s in swaps {
            if let Some(dir) = s.target.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("can't make {}: {e}", dir.display()))?;
            }
            let old = old_path(&s.target);
            let _ = std::fs::remove_file(&old);
            let had = s.target.exists();
            if had {
                std::fs::rename(&s.target, &old).map_err(|e| format!("can't move {} aside: {e}", s.target.display()))?;
            }
            done.push((s, had));
            let mut tmp = s.target.as_os_str().to_owned();
            tmp.push(".new");
            let tmp = PathBuf::from(tmp);
            {
                let mut f = std::fs::File::create(&tmp).map_err(|e| format!("can't write {}: {e}", tmp.display()))?;
                f.write_all(&s.data).and_then(|_| f.flush()).map_err(|e| format!("can't write {}: {e}", tmp.display()))?;
            }
            #[cfg(unix)]
            if s.executable {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
            }
            std::fs::rename(&tmp, &s.target).map_err(|e| format!("can't put {} in place: {e}", s.target.display()))?;
        }
        Ok(())
    })();
    if result.is_err() {
        for (s, had) in done.iter().rev() {
            let _ = std::fs::remove_file(&s.target);
            if *had {
                let _ = std::fs::rename(old_path(&s.target), &s.target);
            }
        }
    }
    result
}

/// Delete the `.old` files a previous update left (on the next start, once they're free).
pub fn cleanup_old() {
    let Ok(exe) = std::env::current_exe() else { return };
    let mut dirs = vec![];
    if let Some(d) = exe.parent() {
        dirs.push(d.to_path_buf());
        if cfg!(target_os = "macos") {
            if let Some(c) = d.parent() {
                dirs.push(c.to_path_buf());
            }
        }
    }
    for d in dirs {
        remove_old_in(&d);
    }
}

pub fn remove_old_in(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "old") && p.is_file() {
            let _ = std::fs::remove_file(&p);
        }
    }
}

// ---------------------------------------------------------------- the background worker

/// What the updater is doing.
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Idle,
    Checking { manual: bool },
    UpToDate,
    /// A development build: it doesn't update itself.
    Dev,
    /// A newer build is out.
    Available(Manifest),
    Downloading { build: u32, got: u64, total: u64 },
    Installing { build: u32 },
    /// The new files are in place: start the new program (from a menu, never mid-match).
    Restart { build: u32 },
    Failed(String),
    /// The check itself failed (no internet...): only shown when asked for.
    CheckFailed(String),
}

#[derive(Clone)]
pub struct Updater {
    pub state: Arc<Mutex<UpdaterState>>,
}

pub struct UpdaterState {
    pub phase: Phase,
    /// "Later" was pressed for this build.
    pub dismissed: Option<u32>,
    /// The last version.json read (to say when a platform's download is behind).
    pub latest: Option<Manifest>,
}

impl Updater {
    pub fn new() -> Updater {
        Updater { state: Arc::new(Mutex::new(UpdaterState { phase: Phase::Idle, dismissed: None, latest: None })) }
    }

    /// The last version.json read, if any.
    pub fn latest(&self) -> Option<Manifest> {
        self.state.lock().ok().and_then(|s| s.latest.clone())
    }

    /// What the last check found about downloads that are behind, for this platform's player.
    pub fn behind_note(&self) -> Option<String> {
        self.latest().and_then(|m| m.behind_note(platform()))
    }

    pub fn phase(&self) -> Phase {
        self.state.lock().map(|s| s.phase.clone()).unwrap_or(Phase::Idle)
    }

    fn set(&self, p: Phase) {
        if let Ok(mut s) = self.state.lock() {
            s.phase = p;
        }
    }

    pub fn dismiss(&self, build: u32) {
        if let Ok(mut s) = self.state.lock() {
            s.dismissed = Some(build);
        }
    }

    pub fn dismissed(&self) -> Option<u32> {
        self.state.lock().ok().and_then(|s| s.dismissed)
    }

    pub fn busy(&self) -> bool {
        matches!(self.phase(), Phase::Checking { .. } | Phase::Downloading { .. } | Phase::Installing { .. })
    }

    /// Look for an update in the background.
    pub fn check(&self, manual: bool) {
        if self.busy() {
            return;
        }
        if local_build() == 0 {
            self.set(Phase::Dev);
            return;
        }
        self.set(Phase::Checking { manual });
        let me = self.clone();
        let _ = std::thread::Builder::new().name("update check".into()).spawn(move || {
            let got = fetch(&manifest_url(), None).and_then(|b| parse_manifest(&String::from_utf8_lossy(&b)));
            if let (Ok(m), Ok(mut s)) = (&got, me.state.lock()) {
                s.latest = Some(m.clone());
            }
            let p = match got {
                Ok(m) if is_newer(m.build_for(platform()), local_build()) => Phase::Available(m),
                Ok(_) => Phase::UpToDate,
                Err(e) => Phase::CheckFailed(e),
            };
            me.set(p);
        });
    }

    /// Download, check and install `m` in the background; on success the phase is Restart.
    pub fn update(&self, m: Manifest) {
        if self.busy() {
            return;
        }
        let me = self.clone();
        self.set(Phase::Downloading { build: m.build_for(platform()), got: 0, total: 0 });
        let _ = std::thread::Builder::new().name("update download".into()).spawn(move || {
            let r = me.download_and_install(&m);
            me.set(match r {
                Ok(()) => Phase::Restart { build: m.build_for(platform()) },
                Err(e) => Phase::Failed(e),
            });
        });
    }

    fn download_and_install(&self, m: &Manifest) -> Result<(), String> {
        let d = m.download(platform()).ok_or_else(|| format!("this build has no download for {}", platform()))?;
        if platform() == "linux" {
            return Err("updating in place isn't supported on Linux yet".into());
        }
        let build = m.build_for(platform());
        let me = self.clone();
        let bytes = fetch(&file_url(&d.file), Some(&move |got, total| me.set(Phase::Downloading { build, got, total })))?;
        check_hash(&sha256_hex(&bytes), &d.sha256)?;
        self.set(Phase::Installing { build });
        let entries = read_zip(&bytes)?;
        let exe = std::env::current_exe().map_err(|e| format!("can't find the game's program: {e}"))?;
        let swaps = if cfg!(windows) { plan_windows(&exe, &entries)? } else { plan_mac(&exe, &entries)? };
        swap_files(&swaps)
    }
}

/// Download a file over HTTPS (redirects followed), reporting progress.
fn fetch(url: &str, progress: Option<&dyn Fn(u64, u64)>) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(30))
        .https_only(!url.starts_with("http://127.0.0.1"))
        .build();
    let resp = agent.get(url).set("User-Agent", "DuelRush-updater").call().map_err(|e| match e {
        ureq::Error::Status(code, _) => format!("the server answered {code}"),
        other => format!("couldn't reach the update server ({other})"),
    })?;
    let total: u64 = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut r = resp.into_reader().take(1 << 30);
    let mut out = Vec::with_capacity(total as usize);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = r.read(&mut buf).map_err(|e| format!("the download broke off ({e})"))?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
        if let Some(p) = progress {
            p(out.len() as u64, total);
        }
    }
    if total > 0 && out.len() as u64 != total {
        return Err("the download stopped early".into());
    }
    Ok(out)
}

/// Start the new program (with the same arguments) and let this one close.
pub fn relaunch() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::Command::new(&exe).args(&args).spawn().map_err(|e| format!("couldn't start the new version: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_compare() {
        assert!(is_newer(58, 57));
        assert!(!is_newer(57, 57));
        assert!(!is_newer(56, 57), "never back to an older build");
        assert!(!is_newer(99, 0), "a development build doesn't update itself");
        assert!(builds_compatible(57, 57));
        assert!(!builds_compatible(57, 58));
        assert!(builds_compatible(0, 58) && builds_compatible(57, 0), "a developer's build can join anyone");
        assert_eq!(parse_u32("123"), 123);
        assert_eq!(parse_u32(""), 0);
        assert_eq!(parse_u32("12a"), 0);
    }

    #[test]
    fn manifest_reads_and_rejects_bad_entries() {
        let sha = "a".repeat(64);
        let text = format!(
            r#"{{"build": 61, "version": "0.2.0", "notes": "Better swords", "files": {{
                "windows": {{"file": "DuelRush-Windows.zip", "sha256": "{sha}"}},
                "mac": {{"file": "../evil.zip", "sha256": "{sha}"}},
                "linux": {{"file": "DuelRush-Linux.tar.gz", "sha256": "short"}} }} }}"#
        );
        let m = parse_manifest(&text).unwrap();
        assert_eq!(m.build, 61);
        assert_eq!(m.notes, "Better swords");
        assert_eq!(m.download("windows").unwrap().file, "DuelRush-Windows.zip");
        assert!(m.download("mac").is_none(), "paths in file names are refused");
        assert!(m.download("linux").is_none(), "a hash that isn't SHA-256 is refused");
        assert!(parse_manifest(r#"{"version": "1"}"#).is_err(), "no build number");
        assert!(parse_manifest("not json").is_err());
        assert!(file_url("DuelRush-Windows.zip").ends_with("/releases/download/latest/DuelRush-Windows.zip"));
    }

    /// When a release keeps an older Mac zip (that run's Mac build didn't finish), the Mac is
    /// told that zip's own build, so it isn't offered the same build again as an update.
    #[test]
    fn a_kept_older_file_carries_its_own_build() {
        let sha = "b".repeat(64);
        let text = format!(
            r#"{{"build": 20, "files": {{
                "windows": {{"file": "DuelRush-Windows.zip", "sha256": "{sha}"}},
                "mac": {{"file": "DuelRush-Mac.zip", "sha256": "{sha}", "build": 18}} }} }}"#
        );
        let m = parse_manifest(&text).unwrap();
        assert_eq!(m.build_for("windows"), 20);
        assert_eq!(m.build_for("mac"), 18);
        assert_eq!(m.build_for("linux"), 20, "no file of its own: the release's build");
        assert!(!is_newer(m.build_for("mac"), 18), "a Mac on build 18 is up to date");
    }

    /// Who can play together: the network protocol decides, not the build number. A Mac zip a few
    /// builds behind plays with Windows if the protocol is unchanged; a different protocol never
    /// does; a copy from before the protocol number has to be the same build.
    #[test]
    fn the_network_protocol_decides_who_plays_together() {
        // same protocol, different builds (Mac #21 behind Windows #23): they play
        assert!(can_play_together(23, 11, 21, Some(11)));
        assert!(can_play_together(21, 11, 23, Some(11)));
        // same build number but a different protocol (can't really happen, but the protocol wins)
        assert!(!can_play_together(23, 11, 23, Some(12)));
        // different protocols: no, whichever side is newer
        assert!(!can_play_together(23, 12, 21, Some(11)));
        assert!(!can_play_together(21, 11, 23, Some(12)));
        // a copy from before the protocol number (it doesn't say): the same build only
        assert!(can_play_together(23, 11, 23, None));
        assert!(!can_play_together(23, 11, 18, None), "the old Mac build #18 can't join #23");
        assert!(can_play_together(0, 11, 18, None), "a developer's build still joins anyone old");
        // a developer's build with the same protocol joins a release, and the other way round
        assert!(can_play_together(0, 11, 23, Some(11)) && can_play_together(23, 11, 0, Some(11)));
        // what the player is told
        let t = mismatch_text(21, 11, 23, Some(12));
        assert!(t.contains("network version 12") && t.contains("update first"), "{t}");
        let t = mismatch_text(23, 12, 21, Some(11));
        assert!(t.contains("the host needs to update first"), "{t}");
        let t = mismatch_text(18, 11, 23, None);
        assert!(t.contains("Your build (18)") && t.contains("update first"), "{t}");
        assert_eq!(local_proto(), crate::net::NET_PROTOCOL);
    }

    fn release(mac: &str, top_protocol: &str) -> Manifest {
        let sha = "c".repeat(64);
        parse_manifest(&format!(
            r#"{{"build": 23, {top_protocol} "files": {{
                "windows": {{"file": "DuelRush-Windows.zip", "sha256": "{sha}"}},
                "linux": {{"file": "DuelRush-Linux.tar.gz", "sha256": "{sha}"}},
                "mac": {{"file": "DuelRush-Mac.zip", "sha256": "{sha}" {mac}}} }} }}"#
        ))
        .unwrap()
    }

    /// version.json says when the Mac download is behind and whether it still plays with the
    /// release; the game says so to Mac and Windows players alike.
    #[test]
    fn a_mac_download_behind_is_said_clearly() {
        // all on the release's build: nothing to say
        let m = release("", r#""protocol": 11,"#);
        assert!(m.behind().is_empty() && m.behind_note("windows").is_none() && m.behind_note("mac").is_none());
        assert_eq!(m.protocol_for("mac"), Some(11));
        // the Mac zip kept from build 21, same protocol: behind, but it plays
        let m = release(r#", "build": 21, "protocol": 11"#, r#""protocol": 11,"#);
        assert_eq!(m.behind(), vec![("mac", 21)]);
        assert!(m.plays_with_release("mac"));
        let w = m.behind_note("windows").unwrap();
        assert!(w.contains("Mac download is behind") && w.contains("build 21") && w.contains("can still play"), "{w}");
        let mac = m.behind_note("mac").unwrap();
        assert!(mac.contains("newest Mac download is build 21") && mac.contains("still plays"), "{mac}");
        // a different protocol: it can't
        let m = release(r#", "build": 21, "protocol": 10"#, r#""protocol": 11,"#);
        assert!(!m.plays_with_release("mac"));
        assert!(m.behind_note("windows").unwrap().contains("can't join build 23"));
        assert!(m.behind_note("mac").unwrap().contains("can't play with build 23"));
        // today's case: the Mac zip is build 18, from before the protocol number: it can't
        let m = release(r#", "build": 18"#, "");
        assert_eq!(m.protocol_for("mac"), None);
        assert!(!m.plays_with_release("mac"));
        assert!(m.behind_note("linux").unwrap().contains("still build 18"));
        // and a Mac on 18 isn't offered 18 again
        assert!(!is_newer(m.build_for("mac"), 18));
    }

    #[test]
    fn hashes_are_checked() {
        // the SHA-256 of "abc" (FIPS 180-2 test vector)
        let h = sha256_hex(b"abc");
        assert_eq!(h, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert!(check_hash(&h, &h.to_uppercase()).is_ok());
        assert!(check_hash(&h, &sha256_hex(b"abd")).is_err(), "a different file is refused");
        assert!(check_hash(&h, "").is_err(), "no published hash, no update");
    }

    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).unix_permissions(0o755);
            for (name, data) in files {
                w.start_file(*name, opts).unwrap();
                w.write_all(data).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("duel_update_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn windows_swap_puts_new_files_in_and_keeps_old_until_next_start() {
        let d = temp_dir("win");
        let exe = d.join("Duel Rush.exe");
        std::fs::write(&exe, b"old exe").unwrap();
        std::fs::write(d.join("steam_api64.dll"), b"old dll").unwrap();
        let z = zip_of(&[("Duel Rush/Duel Rush.exe", b"new exe"), ("Duel Rush/steam_api64.dll", b"new dll"), ("Duel Rush/readme.txt", b"x")]);
        let entries = read_zip(&z).unwrap();
        let plan = plan_windows(&exe, &entries).unwrap();
        assert_eq!(plan.len(), 2, "only the program and the Steam library");
        swap_files(&plan).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new exe");
        assert_eq!(std::fs::read(d.join("steam_api64.dll")).unwrap(), b"new dll");
        assert_eq!(std::fs::read(d.join("Duel Rush.exe.old")).unwrap(), b"old exe", "the running program moved aside");
        // the next start clears the old files
        remove_old_in(&d);
        assert!(!d.join("Duel Rush.exe.old").exists() && !d.join("steam_api64.dll.old").exists());
        assert!(exe.exists());
        // a zip without the program is refused
        let bad = read_zip(&zip_of(&[("Duel Rush/steam_api64.dll", b"x")])).unwrap();
        assert!(plan_windows(&exe, &bad).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_failed_swap_puts_everything_back() {
        let d = temp_dir("rollback");
        let exe = d.join("game.exe");
        std::fs::write(&exe, b"old exe").unwrap();
        // the second file can't be written: its folder would have to be inside a plain file
        std::fs::write(d.join("afile"), b"x").unwrap();
        let blocked = d.join("afile").join("x.dll");
        let swaps = vec![
            Swap { target: exe.clone(), data: b"new exe".to_vec(), executable: true },
            Swap { target: blocked.clone(), data: b"x".to_vec(), executable: false },
        ];
        assert!(swap_files(&swaps).is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"old exe", "the current version still works");
        assert!(!d.join("game.exe.old").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn mac_swap_replaces_the_bundle_but_keeps_the_steam_library() {
        let d = temp_dir("mac");
        let contents = d.join("Duel Rush.app").join("Contents");
        let macos = contents.join("MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        let exe = macos.join("duel_arena");
        std::fs::write(&exe, b"old").unwrap();
        std::fs::write(macos.join("libsteam_api.dylib"), b"steam").unwrap();
        std::fs::write(contents.join("Info.plist"), b"old plist").unwrap();
        let z = zip_of(&[
            ("Duel Rush.app/Contents/MacOS/duel_arena", b"new"),
            ("Duel Rush.app/Contents/MacOS/libsteam_api.dylib", b"other steam"),
            ("Duel Rush.app/Contents/Info.plist", b"new plist"),
            ("Duel Rush.app/Contents/_CodeSignature/CodeResources", b"sig"),
        ]);
        let plan = plan_mac(&exe, &read_zip(&z).unwrap()).unwrap();
        swap_files(&plan).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        assert_eq!(std::fs::read(macos.join("libsteam_api.dylib")).unwrap(), b"steam", "the Steam library is kept");
        assert_eq!(std::fs::read(contents.join("Info.plist")).unwrap(), b"new plist");
        assert!(contents.join("_CodeSignature").join("CodeResources").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn unsafe_zips_are_refused() {
        let z = zip_of(&[("../outside.exe", b"x")]);
        assert!(read_zip(&z).is_err());
        assert!(read_zip(b"not a zip").is_err());
    }
}
