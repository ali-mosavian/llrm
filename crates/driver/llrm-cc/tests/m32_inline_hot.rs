//! m32 -O2 (INL-HOT): a call the pin's threshold alone refuses at a hot site is inlined where the
//! trial's clocks pay for the bytes, and kept where they do not. `mix` (cost 630 over 337, 31x a
//! loop trip, 709 B for 682 clocks) stayed a call: x_sf_add's `add_bits` did the same and ran
//! 52162 clocks against 41300 inlined (2026-11-08).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn test_o2_inlines_a_hot_call_the_threshold_refused_when_the_trial_pays() {
    hot_call_is_inlined_at("-O2");
}

/// The same at -O3: x_sf_div -O3 137429 -> 120942 clk (+657 B), x_sf_add/sub/mul/i2d 10198-12441 clk
/// each (+528..551 B) once the trial runs there; a 121-program A/B changed nothing else.
#[test]
fn test_o3_inlines_a_hot_call_the_threshold_refused_when_the_trial_pays() {
    hot_call_is_inlined_at("-O3");
}

fn hot_call_is_inlined_at(level: &str) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("inlinehot{}-{}", level, std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args([level, "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/inline_hot.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let calls: Vec<&str> = asm.lines().map(str::trim).filter(|line| line.starts_with("call")).collect();
    assert!(
        !calls.iter().any(|line| line.ends_with(" mix")),
        "a hot call whose trial pays stayed a call:\n{}",
        calls.join("\n")
    );
    assert!(
        calls.iter().any(|line| line.ends_with(" mixbig")),
        "a 12 KB copy for 600 clocks was kept:\n{}",
        calls.join("\n")
    );
}
