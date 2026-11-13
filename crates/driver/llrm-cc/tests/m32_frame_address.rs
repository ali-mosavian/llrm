//! m32: `__builtin_frame_address(0)` was a call to the symbol
//! `llvm.frameaddress.p0` and the assembler panicked ("no entry found for key",
//! assemble.rs:401), 2026-11-09. Until the intrinsic is lowered it is refused
//! by name, never a panic or a call to a symbol.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn test_frame_address_is_lowered_or_refused_by_name_never_a_panic() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32frameaddr-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-O1", "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_frame_address.c"))
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&done.stderr);
    assert!(!said.contains("panicked"), "{said}");
    if done.status.success() {
        let asm = std::fs::read_to_string(dir.join("d.s")).unwrap();
        assert!(!asm.contains("llvm.frameaddress"), "a call to the intrinsic's name:\n{asm}");
    } else {
        assert!(said.contains("llvm.frameaddress"), "a refusal that does not name the intrinsic: {said}");
    }
}
