//! Build script: put Valve's Steam API library (redist/steam_api64.dll, the Steamworks SDK's
//! redistributable) next to the game, and link it "delay-loaded" so the game still starts and
//! falls back to direct-IP play when the library or Steam isn't there.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=redist/steam_api64.dll");
    println!("cargo:rerun-if-changed=build.rs");
    // a build stamp for the version on the main menu: when this build was made (UTC). It's
    // remade whenever the game's code or assets change, so every playtest build has its own.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rustc-env=DUEL_BUILD={}", build_stamp());
    // the release build number: GitHub Actions' run number (0 for a build made anywhere else, a
    // development build, which never updates itself). DUEL_RELEASE_BUILD overrides it: a "Mac
    // only" rebuild of an existing release takes that release's number, so it matches the others.
    println!("cargo:rerun-if-env-changed=GITHUB_RUN_NUMBER");
    println!("cargo:rerun-if-env-changed=DUEL_RELEASE_BUILD");
    let run: u32 = ["DUEL_RELEASE_BUILD", "GITHUB_RUN_NUMBER"].iter().find_map(|k| std::env::var(k).ok().and_then(|v| v.trim().parse().ok()).filter(|&n: &u32| n > 0)).unwrap_or(0);
    println!("cargo:rustc-env=DUEL_BUILD_NUMBER={run}");
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let text = if run > 0 { format!("v{version} build {run}") } else { format!("v{version} dev build ({})", build_stamp()) };
    println!("cargo:rustc-env=DUEL_VERSION_TEXT={text}");
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows") && target.contains("msvc") && target.contains("x86_64") {
        // load steam_api64.dll only when a Steam function is first called (we check it's there)
        println!("cargo:rustc-link-arg=/DELAYLOAD:steam_api64.dll");
        println!("cargo:rustc-link-lib=delayimp");
        copy_next_to_exe("steam_api64.dll");
    }
    if target.contains("linux") {
        // Linux and the Steam Deck: look for libsteam_api.so next to the game, wherever it's put
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
    }
}

/// OUT_DIR sits somewhere under target/<profile>/build/ (the exact depth depends on the Cargo
/// version); the program lands in target/<profile>, the parent of that `build` folder.
fn copy_next_to_exe(name: &str) {
    let src = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("redist").join(name);
    let Ok(out) = std::env::var("OUT_DIR") else { return };
    let out = PathBuf::from(out);
    let Some(build_dir) = out.ancestors().find(|a| a.file_name().is_some_and(|n| n == "build")) else { return };
    let Some(profile_dir) = build_dir.parent() else { return };
    for dir in [profile_dir.to_path_buf(), profile_dir.join("deps")] {
        if dir.is_dir() {
            let _ = std::fs::copy(&src, dir.join(name));
        }
    }
}

/// "YYMMDD.HHMM" in UTC, from the clock (no date library needed).
fn build_stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    format!("{:02}{:02}{:02}.{:02}{:02}", y % 100, m, d, rem / 3600, (rem % 3600) / 60)
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