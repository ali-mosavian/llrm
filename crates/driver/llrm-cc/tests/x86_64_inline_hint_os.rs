//! x86-64 -Os: a `static inline` helper the size guard held a call, where the
//! pin's `updateThreshold` raises its threshold to 325 (`inlinehint`). QB
//! `operands.c`: `hot_operand` stayed a call in `hot_int`, so `hot_int` took
//! its argument's address (no SROA), stayed a call in 40 handlers of ~100 B
//! each, and the object was 16,542 B against clang's 10,823. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ih-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-fno-pic", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/x86_64_inline_hint_os.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

/// Twenty handlers over `hot_int` and `operand`, both `static inline`: no call
/// to either is left (before: 20 calls to `hot_int`, 904 instructions).
#[test]
fn an_inline_hinted_helper_is_inlined_at_os_where_the_pin_inlines_it() {
    let text = assembly();
    let calls: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("call") && (line.contains("hot_int") || line.contains("operand")))
        .collect();
    assert!(calls.is_empty(), "{} calls left:\n{text}", calls.len());
}
