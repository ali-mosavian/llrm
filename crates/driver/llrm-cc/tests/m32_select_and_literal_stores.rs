//! m32 (i486, no cmov): a select of two constants is a branch (SELB), and equal
//! literal stores in code that runs once share a register (ISS1). Both were the
//! pin's: `setcc` arithmetic and `mov dword [m],0` stores, 2026-11-08.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    // one directory per call: the tests run in parallel and shared `d.s` raced
    static CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32sel-{level}-{}-{call}", std::process::id()));
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
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_select_and_literal_stores.c"))
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
fn test_a_select_of_two_constants_is_a_branch_without_cmov() {
    for level in ["-O2", "-O3"] {
        let asm = compiled(level);
        let g = function(&asm, "g");
        assert!(!g.iter().any(|line| line.starts_with("set")), "{level}: setcc arithmetic:\n{}", g.join("\n"));
        assert!(g.iter().any(|line| line.starts_with('j')), "{level}: no branch:\n{}", g.join("\n"));
    }
}

#[test]
fn test_a_select_of_two_constants_run_once_keeps_setcc() {
    for level in ["-O2", "-O3"] {
        let asm = compiled(level);
        let h = function(&asm, "h");
        assert!(h.iter().any(|line| line.starts_with("set")), "{level}: no setcc:\n{}", h.join("\n"));
    }
}

#[test]
fn test_equal_literal_stores_share_a_register_outside_loops() {
    for level in ["-O2", "-O3"] {
        let asm = compiled(level);
        let f = function(&asm, "f");
        let zeros = f.iter().filter(|line| line.starts_with("mov dword ptr [") && line.ends_with(", 0")).count();
        assert_eq!(zeros, 0, "{level}: immediate zero stores kept:\n{}", f.join("\n"));
        assert!(f.iter().any(|line| line.starts_with("xor ")), "{level}: no xor:\n{}", f.join("\n"));
    }
}

#[test]
fn test_o1_keeps_literal_stores_immediate() {
    // sharing costs a clock a call: queens +512 and lru +3 clocks at -O1 when
    // it did
    let asm = compiled("-O1");
    let f = function(&asm, "f");
    let zeros = f.iter().filter(|line| line.starts_with("mov dword ptr [") && line.ends_with(", 0")).count();
    assert_eq!(zeros, 4, "-O1 shared the zero stores:\n{}", f.join("\n"));
}
