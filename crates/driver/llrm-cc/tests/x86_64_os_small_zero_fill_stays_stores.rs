//! x86-64 -Os fills priced by the encodings of their destination.
//! - Three bytes of zero through a pointer (an initializer's padding) are a
//!   word and a byte store, 10 B, not a `rep stos` fill, 14 B. The stores were
//!   priced 8 B each whatever their width, so the fill won.
//! - 33 bytes of 0x7b into a global are a `rep stosb`, 16 B: its stores are
//!   `[rip+disp32]`, 28 B. Pricing every store as `[reg+disp8]` chose the
//!   stores (+37 B in `memory_expansion`).
//!
//! Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(source: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("small-zero-{}-{source}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

#[test]
fn a_three_byte_zero_fill_is_two_stores() {
    let text = assembly("memset_small_pad.c");
    assert!(!text.contains("rep stos"), "a 3-byte clear is a string fill:\n{text}");
}

#[test]
fn a_global_pattern_fill_stays_rep_stosb() {
    let text = assembly("memset_global_pattern.c");
    assert!(text.contains("rep stosb"), "a 33-byte global fill is rip-relative stores:\n{text}");
}
