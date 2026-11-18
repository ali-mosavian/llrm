//! x86_32-dos -Os: a callee-saved register pushed after `sub esp, N` sits where the call's first
//! argument is stored (`[esp]`), so the call overwrote the caller's `ebx` (2026-11-13). The frame
//! was refused its pin order because the wide divide's inline bytes were taken for frame access.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn test_saved_registers_are_pushed_before_the_space_the_call_stores_into() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32ossaved-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-dos")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-Os", "-w", "-S", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_os_saved_regs_call_frame.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let sub = lines.iter().position(|line| line.starts_with("sub esp,")).expect("a stack adjustment");
    let late: Vec<&&str> =
        lines[sub..].iter().take_while(|line| !line.starts_with("call")).filter(|line| line.starts_with("push e")).collect();
    assert!(late.is_empty(), "{late:?} pushed after `sub esp`, below the call's arguments:\n{asm}");
}
