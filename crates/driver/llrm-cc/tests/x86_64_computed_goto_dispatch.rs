//! x86-64 -O1 to -Os: the dispatch of an
//! interpreter (`goto *tab[*pc]` after every handler) is copied into each
//! handler after allocation, as the pin does for a computed goto. Before, one
//! block held it and all twenty handlers jumped to it, with the phi copies of
//! `pc` and `sp` in front of every dispatch (QB `exec_loop`, dbl_fn +15% over
//! gcc). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

/// One directory per call and thread: two tests of this file compile at the
/// same level in parallel, and a shared `cg{level}-{pid}` let one delete the
/// other's output (`read_to_string` unwrap, a gate failure of #10774).
fn scratch_dir(level: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "cg{level}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ))
}

fn assembly(level: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = scratch_dir(level);
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

/// -Os copies the dispatch as -O1 to -O3 do: `TailDuplicatePass` repeats its
/// sweep until nothing changes, so a shared tail that the first copy turned
/// into a computed goto is copied in turn. A single sweep left 11 of the 20
/// (the pin's 20); QB `exec.c` m64 -Os 12,981 B with 32 dispatches against the
/// pin's 126 and the best compiler's 15,485.
#[test]
fn an_os_dispatch_is_one_indirect_jump_per_handler() {
    let found = jumps("-Os");
    assert!(found >= 20, "{found} indirect jumps at -Os: the dispatch is still shared (the pin has 20)");
}

/// The tail duplicator's new phis (`pc`, `sp`) went after the old ones, so the
/// copies at the dispatch came out reversed against the pin's and `pc + K` tied
/// to `K`: a `mov` of `pc` in every handler (278 instructions, the pin's 259).
#[test]
fn a_dispatch_handler_ties_the_sum_to_pc_without_a_copy() {
    for level in ["-O1", "-O2", "-O3"] {
        let count = assembly(level)
            .lines()
            .filter(|line| line.starts_with("    ") && !line.trim_start().starts_with(['d', '.']))
            .count();
        assert!(count <= 259, "{count} instructions at {level}: a handler copies pc");
    }
}

/// Two tests that compile at the same level in parallel must not share a
/// scratch directory (one `remove_dir_all`ed the other's assembly).
#[test]
fn parallel_tests_at_one_level_use_different_scratch_directories() {
    let here = scratch_dir("-O1");
    let there = std::thread::spawn(|| scratch_dir("-O1")).join().unwrap();
    assert_ne!(here, there, "two threads share {}", here.display());
}
