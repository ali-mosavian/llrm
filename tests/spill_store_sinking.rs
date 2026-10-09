//! A spill store runs only on the path that reads the slot.

use std::process::Command;

/// hanoi's `n == 0` exit stored the slot of the accumulator before the test, a
/// slot only the loop path reads: the exit touched the frame, so no prologue
/// could be placed after it (x_hanoi at -O2 ran 19 instructions for each leaf
/// call gcc ran in 3).
#[test]
fn test_a_spill_store_ahead_of_a_branch_is_made_on_the_path_that_reads_the_slot() {
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/bench/hanoi/hanoi.c");
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .env("LLRM_CANDIDATES", "spiller")
        .args(["-O2", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let entry: Vec<&str> = text
        .lines()
        .skip_while(|line| !line.starts_with("_hanoi proc"))
        .skip_while(|line| !line.trim().starts_with("L0_0:"))
        .take_while(|line| !line.trim().starts_with("jne"))
        .collect();
    assert!(!entry.is_empty(), "no entry block: {text}");
    assert!(entry.iter().all(|line| !line.contains("[esp+")), "the entry stores a slot: {entry:?}\n{text}");
}
