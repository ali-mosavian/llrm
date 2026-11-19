//! Identical exported functions are one body at `-Os` on m32 (DEVIATE MERGEFUNC-OS): the second is a `jmp` to the first.
//! sfm16 `__gtsf2` was a 16-byte copy of `__gesf2`; gcc folds them (`-fipa-icf`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    static CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32merge-{level}-{}-{call}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args([level, "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_mergefunc.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

fn function<'a>(
    asm: &'a str,
    name: &str,
) -> Vec<&'a str> {
    let start = asm.lines().position(|line| line.trim() == format!("{name} proc near")).expect("the function");
    asm.lines().skip(start).take_while(|line| line.trim() != format!("{name} endp")).map(str::trim).collect()
}

#[test]
fn test_at_os_the_second_of_two_identical_functions_jumps_to_the_first() {
    let asm = compiled("-Os");
    let twin = function(&asm, "twin_b");
    assert!(twin.iter().any(|line| *line == "jmp twin_a"), "twin_b is a jmp to twin_a:\n{}", twin.join("\n"));
    let other = function(&asm, "other");
    assert!(!other.iter().any(|line| line.starts_with("jmp twin")), "a different body stays a body:\n{}", other.join("\n"));
}

#[test]
fn test_at_o2_identical_functions_stay_two_bodies() {
    let asm = compiled("-O2");
    let twin = function(&asm, "twin_b");
    assert!(!twin.iter().any(|line| line.starts_with("jmp twin_a")), "no merge at -O2:\n{}", twin.join("\n"));
}
