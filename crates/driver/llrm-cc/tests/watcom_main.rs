//! C semantics the Watcom targets keep. Needs the pinned clang
//! (`LLRM_PIN_CLANG`) and the Open Watcom headers (`LLRM_OW`).

use std::path::Path;
use std::process::Command;

/// `main` that falls off its end returns 0 (C99): `-ffreestanding` made clang
/// leave the return value unset, so the program exited with a stack load.
#[test]
fn main_that_falls_off_its_end_returns_zero() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-main-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("m.c"), "int v;\nint main(void) { v = 2; }\n").unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
        .args(["-O2", "-w", "-S", "-o"])
        .arg(dir.join("m.s"))
        .arg(dir.join("m.c"))
        .output()
        .unwrap();
    let listing = std::fs::read_to_string(dir.join("m.s")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let body = listing.split("main_ proc").nth(1).unwrap_or_default();
    assert!(body.contains("xor eax, eax") || body.contains("mov eax, 0"), "{listing}");
}

/// `__cdecl` on a flat Watcom target is the stack convention, `_name`; an
/// unmarked function stays the register one, `name_`.
#[test]
fn cdecl_is_the_stack_convention_on_x86_32_wat() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = std::env::temp_dir().join(format!("llrm-cc-wat-cdecl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), "int __cdecl a(int x, int y) { return x + y; }\nint b(int x, int y) { return x - y; }\n").unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap_or_else(|| root.join("target/release/llrm-llc").into()))
        .args(["-O2", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(listing.contains("_a proc") && listing.contains("b_ proc"), "{listing}");
    assert!(listing.split("_a proc").nth(1).unwrap().contains("[esp+4]"), "{listing}");
}
