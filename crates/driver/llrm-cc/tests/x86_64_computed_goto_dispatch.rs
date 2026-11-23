//! x86-64 -O2 and -Os: the dispatch of an interpreter (`goto *tab[*pc]` after
//! every handler) is copied into each handler after allocation, as the pin does
//! for a computed goto. Before, one block held it and all twenty handlers
//! jumped to it, with the phi copies of `pc` and `sp` in front of every
//! dispatch (QB `exec_loop`, dbl_fn +15% over gcc). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(level: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cg{level}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/x86_64_computed_goto_dispatch.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

/// Twenty handlers and the entry: one indirect jump each, not one shared one.
/// At -O2 placement finished what the pass left (11 jumps); at -Os, where the
/// tail merge nests (pairs), the pass itself left 11 (QB exec_loop arrays
/// +22%).
fn jumps(level: &str) {
    let text = assembly(level);
    let jumps = text.lines().filter(|line| line.trim_start().starts_with("jmp") && line.contains('[')).count();
    assert!(jumps >= 20, "{jumps} indirect jumps at {level} in:\n{text}");
}

#[test]
fn a_computed_goto_dispatch_is_one_indirect_jump_per_handler() {
    jumps("-O2");
}

#[test]
fn a_computed_goto_dispatch_is_one_indirect_jump_per_handler_at_os() {
    jumps("-Os");
}
