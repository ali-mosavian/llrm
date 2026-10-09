//! A conditional branch taken to the block laid out next, followed by a jump, is one opposite branch.

use std::process::Command;

/// bench/hanoi at -O2 had `jne L0_11 / jmp L0_86 / L0_11:` at every inlined level (3601 jumps run of 99200 instructions).
#[test]
fn test_no_branch_jumps_over_a_jump_to_the_next_block() {
    let source = format!("{}/bench/hanoi/hanoi.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(scratch.path()).args(["-O2", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", &source]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    for window in lines.windows(3) {
        let (branch, jump, label) = (window[0], window[1], window[2]);
        let taken = branch.split_whitespace().nth(1).unwrap_or("");
        assert!(!(branch.starts_with('j') && !branch.starts_with("jmp") && jump.starts_with("jmp ") && label == format!("{taken}:")), "{branch} / {jump} / {label}");
    }
}
