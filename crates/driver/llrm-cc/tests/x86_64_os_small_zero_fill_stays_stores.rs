//! x86-64 -Os: three bytes of zero (an initializer's padding) are a word and a
//! byte store, 10 B, not a `rep stos` fill, 15 B. The stores were priced 8 B
//! each whatever their width, so the fill won (13 B against 10 in QuickrBasic's
//! `sugg_probe`). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_three_byte_zero_fill_is_two_stores() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("small-zero-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/memset_small_pad.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    assert!(!text.contains("rep stos"), "a 3-byte clear is a string fill:\n{text}");
}
