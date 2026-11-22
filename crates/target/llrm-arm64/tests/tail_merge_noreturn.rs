//! `TailMergeBlocks` takes the blocks with no successor first, whatever they
//! end in: four arms that each call `ok` and then the noreturn `syn` share the
//! two calls (the pin: 3 `bl`). Only blocks that ended in a return or a jump
//! were candidates, so QB control.c's `ctrl_stmt` wrote the same `mov x0, x19;
//! bl synerr` on every error path (4,276 B against the pin's 4,120 on the same
//! IR).

use std::process::Command;

fn calls(
    ir: &str,
    level: &str,
) -> usize {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, ir).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", level, "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    assert!(asm.contains("t:"), "premise: lowered:\n{asm}");
    asm.lines().filter(|line| line.trim_start().starts_with("bl ")).count()
}

#[test]
fn arms_that_end_in_the_same_noreturn_call_share_their_calls() {
    let ir = include_str!("data/tail_merge_noreturn.ll");
    assert!(calls(ir, "-Os") <= 3, "-Os");
    assert!(calls(ir, "-O2") <= 3, "-O2");
}
