//! An over-aligned local beside a dynamic `alloca` needs the pin's base pointer
//! (DEVIATE RA1). Until it lands the compile fails by name: it must never
//! compile with the local off its alignment.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn test_an_over_aligned_local_with_an_alloca_is_refused_by_name() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32realign-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-O2", "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_realign_alloca.c"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&done.stderr);
    assert!(!done.status.success(), "compiled with the local off its alignment:\n{stderr}");
    assert!(stderr.contains("an over-aligned local with a dynamic alloca is not realigned"), "{stderr}");
}

/// `LLRM_VERIFY=1` rejected every m32 dynamic alloca: the stack pointer was
/// named `SP` (a word) at width 4 ("names 25 at width 4"), so the compile died
/// before it could reach any realign decision.
#[test]
fn test_a_dynamic_alloca_passes_the_verifier_at_m32() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32alloca-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env("LLRM_VERIFY", "1")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-O2", "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_alloca.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}
