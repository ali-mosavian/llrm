//! x86-64 -O1 to -O3 (-Os keeps it one block, TL4): the dispatch of an
//! interpreter (`goto *tab[*pc]` after every handler) is copied into each
//! handler after allocation, as the pin does for a computed goto. Before, one
//! block held it and all twenty handlers jumped to it, with the phi copies of
//! `pc` and `sp` in front of every dispatch (QB `exec_loop`, dbl_fn +15% over
//! gcc). Needs `LLRM_LLC`.

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

fn jumps(level: &str) -> usize {
    let text = assembly(level);
    text.lines().filter(|line| line.trim_start().starts_with("jmp") && line.contains('[')).count()
}

/// Twenty handlers and the entry: one indirect jump each, not one shared one.
/// -O2 left 11 (the tail merge nests; placement refused an indirect jump).
#[test]
fn a_computed_goto_dispatch_is_one_indirect_jump_per_handler() {
    for level in ["-O1", "-O2", "-O3"] {
        let found = jumps(level);
        assert!(found >= 20, "{found} indirect jumps at {level}: the dispatch is still shared");
    }
}

/// DEVIATE TL4 (`skip_computed_goto_copies_at_size`): at -Os the dispatch stays
/// one block, the shape before the copies (QB exec.c is over the best of gcc
/// and clang in bytes when copied).
#[test]
fn an_os_dispatch_stays_one_block() {
    let found = jumps("-Os");
    assert!(found <= 4, "{found} indirect jumps at -Os: the dispatch was copied into the handlers (bytes first)");
}
