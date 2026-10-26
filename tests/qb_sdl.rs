//! The sdl device platform (runtime/qb/dev/sdl) builds against SDL2 and runs
//! under SDL's dummy video and audio drivers: tests/qbhost/sdl_smoke.c prints,
//! draws, types and sounds on a worker thread and checks the frame the SDL
//! thread presents, the keys made of SDL events, the tone in the audio
//! callback, and that closing the window ends the program.
//!
//! SDL2 comes from `SDL2_CONFIG`, `sdl2-config` on the path, or
//! tools/build-sdl2.sh's private build, which the test makes (once, about two
//! minutes) when there is none: a machine without SDL2 fails here by name, it
//! does not skip.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn sdl2_config() -> Option<PathBuf> {
    let from_env = std::env::var_os("SDL2_CONFIG").map(PathBuf::from);
    let private = std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache/llrm/sdl2/bin/sdl2-config"));
    from_env
        .into_iter()
        .chain(private)
        .chain([PathBuf::from("sdl2-config")])
        .find(|path| Command::new(path).arg("--version").output().is_ok_and(|done| done.status.success()))
}

fn flags(
    config: &Path,
    which: &str,
) -> Vec<String> {
    let done = Command::new(config).arg(which).output().expect("sdl2-config runs");
    String::from_utf8_lossy(&done.stdout).split_whitespace().map(str::to_owned).collect()
}

#[test]
fn the_sdl_platform_presents_what_the_program_draws() {
    smoke(None);
}

/// ThreadSanitizer reported data races between the program and the SDL thread
/// (mode, palette and cursor state, the audio tone) and failed the first frame
/// check: the smoke now passes clean.
#[test]
fn the_sdl_platform_is_race_free_under_thread_sanitizer() {
    smoke(Some("thread"));
}

fn smoke(sanitize: Option<&str>) {
    let config = sdl2_config().unwrap_or_else(|| {
        let built = Command::new(root().join("tools/build-sdl2.sh")).output().expect("tools/build-sdl2.sh runs");
        assert!(
            built.status.success(),
            "no SDL2, and tools/build-sdl2.sh failed: {}",
            String::from_utf8_lossy(&built.stderr)
        );
        PathBuf::from(String::from_utf8_lossy(&built.stdout).trim())
    });
    let qb = root().join("runtime/qb");
    let out = std::env::temp_dir().join(format!("sdl_smoke_{}_{}", sanitize.unwrap_or("plain"), std::process::id()));
    let mut build = Command::new("cc");
    build.args(["-std=gnu11", "-w", "-g", "-pthread", "-D__cdecl="]);
    if let Some(kind) = sanitize {
        build.arg(format!("-fsanitize={kind}"));
    }
    build.args(flags(&config, "--cflags"));
    for include in ["posix", ".", "dev", "dev/sdl", "dev/host", "../shared/posix"] {
        build.arg("-I").arg(qb.join(include));
    }
    let runtime = [
        "console", "cntext", "cngfx", "gfx", "gline", "gcirc", "gpaint", "gputget", "gdraw", "play", "inkey", "sleep",
        "timer", "datetime", "events", "screen", "varptr", "fout", "i8out", "bigint", "ifout", "fin", "rtinit",
        "rtterm", "error", "nhstutil", "stcore", "nheap", "strcase", "string",
    ];
    for module in runtime {
        build.arg(qb.join(format!("{module}.c")));
    }
    for module in ["dev", "host", "input", "audio", "tone", "scale", "present", "sync"] {
        build.arg(qb.join(format!("dev/sdl/{module}.c")));
    }
    for module in ["video", "vgadata", "tick"] {
        build.arg(qb.join(format!("dev/host/{module}.c")));
    }
    for source in [
        "runtime/shared/posix/os.c",
        "runtime/qb/posix/startup.c",
        "runtime/qb/posix/block.c",
        "runtime/qb/posix/ad.c",
        "runtime/qb/posix/mathops.c",
        "runtime/qb/posix/paintq.c",
        "tests/qbhost/hostmodule.c",
        "tests/qbhost/sdl_smoke.c",
    ] {
        build.arg(root().join(source));
    }
    build.args(flags(&config, "--libs"));
    if let Some(prefix) = config.parent().and_then(Path::parent).filter(|prefix| prefix.join("lib").is_dir()) {
        build.arg(format!("-Wl,-rpath,{}", prefix.join("lib").display()));
    }
    build.arg("-lm").arg("-o").arg(&out);
    let built = build.output().expect("cc runs");
    if sanitize.is_some() && String::from_utf8_lossy(&built.stderr).contains("tsan") {
        eprintln!("skipped: this compiler has no thread sanitizer runtime");
        return;
    }
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let done = Command::new(&out)
        .env("TSAN_OPTIONS", "halt_on_error=1")
        .env("SDL_VIDEODRIVER", "dummy")
        .env("SDL_AUDIODRIVER", "dummy")
        .output()
        .expect("the program runs");
    std::fs::remove_file(&out).ok();
    let printed = String::from_utf8_lossy(&done.stdout).into_owned();
    // A kernel with high ASLR entropy leaves the sanitizer no shadow memory: it
    // never ran the program.
    if sanitize.is_some() && String::from_utf8_lossy(&done.stderr).contains("ThreadSanitizer can not mmap") {
        eprintln!("skipped: ThreadSanitizer cannot map its shadow memory on this kernel (vm.mmap_rnd_bits)");
        return;
    }
    assert!(done.status.success(), "{printed}{}", String::from_utf8_lossy(&done.stderr));
    assert_eq!(printed.trim(), "ok");
}

/// On macOS (case-insensitive) `#include "sdl.h"` found SDL2's own `SDL.h`
/// first, so none of this platform's declarations reached sdl_smoke.c and it
/// failed with implicit declarations. No header here may differ from an
/// SDL2 header only by case.
#[test]
fn no_header_collides_with_sdl2_case_insensitively() {
    let dir = root().join("runtime/qb/dev/sdl");
    for entry in std::fs::read_dir(dir).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().to_lowercase();
        assert_ne!(
            name, "sdl.h",
            "runtime/qb/dev/sdl/{name} collides with SDL2's SDL.h on case-insensitive file systems"
        );
    }
}

/// The window used to stretch to any size with SDL's logical size, so a resized
/// window showed uneven pixels. `sdl_fit` gives the largest 4:3 picture that
/// fits, centred.
#[test]
fn the_picture_fills_the_window_at_four_to_three() {
    let qb = root().join("runtime/qb");
    let out = std::env::temp_dir().join(format!("scale_unit_{}", std::process::id()));
    let built = Command::new("cc")
        .args(["-std=gnu11", "-Wall", "-I"])
        .arg(qb.join("dev/sdl"))
        .arg(root().join("tests/qbhost/scale_unit.c"))
        .arg(qb.join("dev/sdl/scale.c"))
        .arg("-o")
        .arg(&out)
        .output()
        .expect("cc runs");
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let done = Command::new(&out).output().expect("the test runs");
    std::fs::remove_file(&out).ok();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stdout));
}
