//! Build script: put Valve's Steam API library (redist/steam_api64.dll, the Steamworks SDK's
//! redistributable) next to the game, and link it "delay-loaded" so the game still starts and
//! falls back to direct-IP play when the library or Steam isn't there.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=redist/steam_api64.dll");
    println!("cargo:rerun-if-changed=build.rs");
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows") && target.contains("msvc") && target.contains("x86_64") {
        // load steam_api64.dll only when a Steam function is first called (we check it's there)
        println!("cargo:rustc-link-arg=/DELAYLOAD:steam_api64.dll");
        println!("cargo:rustc-link-lib=delayimp");
        copy_next_to_exe("steam_api64.dll");
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
