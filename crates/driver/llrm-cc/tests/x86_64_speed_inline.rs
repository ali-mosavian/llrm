//! x86-64 -O2/-O3: a speed caller's inline is the pin's threshold, not a price
//! in bytes (bytes are not counted at O1-O3, ruling 2026-11-14). At 312
//! milliclocks a byte `loop_fusion` kept `clear` a function called from every
//! nest. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(
    source: &str,
    level: &str,
) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("si-{}-{call}-{level}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/run/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

/// `loop_fusion.c`: the static `clear`, called from every nest, is inlined (22
/// calls and a second procedure before, 11 calls and `main` alone now).
#[test]
fn a_static_helper_called_from_every_nest_is_inlined_at_speed() {
    for level in ["-O2", "-O3"] {
        let text = assembly("loop_fusion.c", level);
        assert!(!text.contains("clear proc near"), "{level}: `clear` is still a procedure");
        assert!(!text.contains("call clear"), "{level}: `clear` is still called");
    }
}
