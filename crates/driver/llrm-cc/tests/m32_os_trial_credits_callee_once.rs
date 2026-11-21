//! x86_32-linux -Os: the SZ2 trial credited a callee that goes with its last
//! calls once per site, so three copies of `justify` (three memset/memmove
//! calls) read 48 B smaller and were kept: +110 B in QB `file.c` `set`
//! (2026-11-19). One copy must remain: three memory calls, not nine.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn test_a_callee_of_three_sites_is_credited_once_by_the_os_trial() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32oscredit-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-Os", "-w", "-S", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_os_trial_credits_callee_once.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let copies = asm
        .lines()
        .filter(|line| line.contains("call") && (line.contains("memset") || line.contains("memmove")))
        .count();
    assert!(copies <= 3, "{copies} memset/memmove calls: `justify` was inlined at every site:\n{asm}");
}
