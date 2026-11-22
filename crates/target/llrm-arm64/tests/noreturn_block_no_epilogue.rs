//! A block that ends in a call that never returns needs no epilogue.

/// QuickrBasic operands.c -Os: every `oper_*` ends its cold path in a noreturn
/// `fault_status` call. The frame was taken down and a `ret` made after it,
/// 8 B in each of 65 functions (9,988 B against the pin's 9,464 B).
/// `PrologEpilogInserter::calculateSaveRestoreBlocks` inserts no epilogue in a
/// restore block that has no successor and is no return.
#[test]
fn a_block_ending_in_a_noreturn_call_has_no_epilogue() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/noreturn_block_no_epilogue.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    let call = asm.lines().position(|line| line.trim() == "bl fault").expect("the call to fault");
    let after: Vec<&str> =
        asm.lines().skip(call + 1).map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('.')).collect();
    assert!(after.is_empty(), "code after a call that never returns: {after:?}\n{asm}");
}
