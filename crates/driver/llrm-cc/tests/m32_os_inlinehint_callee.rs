//! x86-m32 -Os: a callee declared `inline` is inlined where the pin's
//! `updateThreshold` (325 for `inlinehint` at an optsize caller) passes its
//! cost. The Os size guard refused, so each wrapper tail-called (`jmp step`)
//! the out-of-line switch (QuickrBasic `operands.c`: 8,428 B against clang's
//! 7,227 B). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn an_inline_callee_whose_switch_folds_is_inlined_at_os() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32-os-inlinehint-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-march=i486", "-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_os_inlinehint_callee.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    let calls: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| (line.starts_with("call ") || line.starts_with("jmp ")) && line.contains("step"))
        .collect();
    assert!(calls.is_empty(), "wrappers still reach `step`: {calls:?}\n{text}");
}
