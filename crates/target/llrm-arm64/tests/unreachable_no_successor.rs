//! An IR `unreachable` ends a block that goes nowhere.

/// QuickrBasic a64 -O2: a block ending in `unreachable` selects to nothing,
/// and the block after it was taken for its successor. A value defined past
/// the `unreachable` was then read on a path that never defines it:
/// `hoistspill.rs:100`, `spliteditor.rs:981` and `joinvals.rs:347` panicked
/// (8 of 99 files refused), and `LLRM_ARM64_VERIFY` named the register.
#[test]
fn a_block_ending_in_unreachable_does_not_fall_into_the_next() {
    let cases = [
        include_str!("data/unreachable_hoistspill.ll"),
        include_str!("data/unreachable_spliteditor.ll"),
        include_str!("data/unreachable_joinvals.ll"),
    ];
    for (number, text) in cases.into_iter().enumerate() {
        assert!(text.contains("unreachable"), "premise: case {number} has an unreachable block");
        let dir = tempfile::tempdir().unwrap();
        let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
        std::fs::write(&input, text).unwrap();
        let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
            .args(["-S", "-O2", "-o"])
            .arg(&output)
            .arg(&input)
            .env("LLRM_ARM64_VERIFY", "1")
            .output()
            .unwrap();
        assert!(said.status.success(), "case {number}: {}", String::from_utf8_lossy(&said.stderr));
    }
}
