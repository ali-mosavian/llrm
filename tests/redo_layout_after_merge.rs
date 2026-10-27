//! The layout is made again after the tail merge changes the blocks
//! (`MachineBlockPlacement::runOnMachineFunction`).

mod common;

/// bench/queens at -Os m32: the merged `mov ebp, edi` left the loop's latch as
/// `je Exit / jmp Top`, a branch not taken and a jump every trip; laid out
/// again it is `jne Top` and the exit falls through.
#[test]
fn test_a_loop_latch_after_a_tail_merge_is_one_branch_back() {
    let source = format!("{}/bench/queens/queens.c", env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-Os", "-m32", "-mabi=sysv", "-march=i486", "-S", "-o", "a.s", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let defined = |label: &str, before: usize| lines[..before].iter().any(|line| *line == format!("{label}:"));
    for (at, window) in lines.windows(2).enumerate() {
        let (branch, jump) = (window[0], window[1]);
        let back = jump.strip_prefix("jmp ").is_some_and(|target| defined(target, at));
        assert!(!(branch.starts_with('j') && !branch.starts_with("jmp") && back), "{branch} / {jump}");
    }
}
