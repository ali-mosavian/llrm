//! gcc's `-fcrossjumping` merges the identical tails of the copies of an
//! inlined function at size. The pin's `SimplifyCFG` sinks common code in its
//! late slot only, by when each copy's write-back reads its own phis:
//! QuickrBasic `synth.c` `voice()` (20 copies of a 15-store tail) was 31,808 B
//! at m64 against gcc's 27,346 B. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86_64-os-sink-common-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_os_sink_common.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

#[test]
fn the_write_back_of_four_inlined_copies_is_stored_once_at_os() {
    let text = listing();
    let stores = text.lines().filter(|line| line.contains("mov    DWORD PTR [r") && line.contains("],e")).count();
    assert_eq!(stores, 3, "each copy keeps its own three stores\n{text}");
}
