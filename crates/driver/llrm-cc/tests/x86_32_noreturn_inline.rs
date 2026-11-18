//! x86-32 -Os: a `noreturn` static inline callee takes its struct by value.
//! It was never inlined (no `ret`, so not copyable): the struct was built in a
//! temporary and copied to the argument slot with `rep movs`, and `raise_it`
//! stayed a function (QuickrBasic `fault_error`: 66-77 bytes against 50).
//! Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_noreturn_callee_with_a_struct_argument_is_inlined_without_a_copy() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("noreturn-inline-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-std=c23", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/noreturn_byval.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    assert!(!text.contains("raise_it proc") && !text.contains("rep movs"), "{text}");
}
