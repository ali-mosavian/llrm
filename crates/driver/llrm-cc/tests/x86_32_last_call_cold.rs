//! x86-32 -Os: a local function called once, from a cold arm, is inlined: gcc's
//! `-finline-functions-called-once`. The pin drops the `LastCallToStatic`
//! bonus at a cold site, so `big` stayed a function (QuickrBasic `xstmt`:
//! 804 bytes, control.c 7813 against gcc's 6948). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_function_called_once_from_a_cold_arm_is_inlined_at_size() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("last-call-cold-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", "-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/last_call_cold.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    assert!(!text.contains("big proc"), "{text}");
}
