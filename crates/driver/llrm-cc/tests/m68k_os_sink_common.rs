//! gcc's `-fcrossjumping` merges the identical tails of the copies of an
//! inlined function at size; the pin's `SimplifyCFG` sinks in its late slot
//! only. QuickrBasic `synth.c` `voice()` (20 copies of a 15-store tail) was
//! 29,812 B at m68k against gcc's 26,330 B.

use std::path::Path;
use std::process::Command;

#[test]
fn the_write_back_of_four_inlined_copies_is_stored_once_at_os() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m68k-os-sink-common-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "m68k-68020")
        .args(["-Os", "-w", "-ffreestanding", "-S", "-o"])
        .arg(dir.join("a.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_os_sink_common.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("a.s")).unwrap();
    let stores =
        text.lines().filter(|line| line.trim_start().starts_with("move.l\t%d") && line.contains(", (")).count();
    assert_eq!(stores, 3, "each copy keeps its own three stores\n{text}");
}
