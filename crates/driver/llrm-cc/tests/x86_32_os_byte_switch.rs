//! `switch (n->kind)` on a `uint8_t` with cases of 128 and up: CodeGenPrepare
//! widened the switch to `i32`, so each case was `cmp eax, imm32` (6 B) beside
//! a `movzx`, where gcc compares the byte, `cmp al, imm8` (2 B). QuickrBasic's
//! control.c was 187 B larger at -Os. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing(level: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-byte-switch-{level}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", level, "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_os_byte_switch.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{level}: {}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

#[test]
fn a_byte_switch_at_os_compares_the_byte() {
    let text = listing("-Os");
    assert!(text.contains("cmp al, "), "no byte compare:\n{text}");
    assert!(!text.contains("cmp eax, "), "a widened compare:\n{text}");
}
