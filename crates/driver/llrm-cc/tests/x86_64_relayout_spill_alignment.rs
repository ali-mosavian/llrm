//! x86-64: the frame laid again aligned a 4-byte spill slot as the 8-aligned
//! home it came from, as 8. jpeg_single -Os kept 568 bytes of frame where
//! natural alignment (the pin's `getSpillAlignment`) keeps 520. Needs
//! `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_four_byte_spill_slot_is_aligned_as_four_bytes_when_the_frame_is_laid_again() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("relayout-align-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("s.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-I"])
        .arg(root.join("runtime/shared/posix"))
        .arg("-o")
        .arg(&asm)
        .arg(root.join("bench/jpeg_single/jpeg_single.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    let body = text.split("bench_jpeg_single proc").nth(1).expect("no bench_jpeg_single");
    let frame: u32 =
        body.lines().find_map(|line| line.trim().strip_prefix("sub rsp, ")).expect("no frame").parse().unwrap();
    assert!(frame <= 520, "frame {frame} bytes: 4-byte spill slots padded to 8");
}
