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

/// The runtime of examples/channels.nib at -m32 -O2 with `LLRM_VERIFY=1`: the
/// layout made again kept the table of frequencies of the merged body, and the
/// verifier stopped `@N$BRES` ("block 0x2 runs 1.000 times, its predecessors
/// 0.383"), so channels did not build in the behavioural tier.
#[test]
fn test_the_layout_made_again_after_a_tail_merge_leaves_a_table_that_verifies() {
    let root = env!("CARGO_MANIFEST_DIR");
    let scratch = tempfile::tempdir().unwrap();
    let nib = |args: &[&str]| {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
            .current_dir(scratch.path())
            .env("LLRM_VERIFY", "1")
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    };
    nib(&[&format!("{root}/examples/channels.nib"), "-m32", "-O2", "--procedure-segments", "-o", "program.obj"]);
    let runtime = format!("{root}/crates/frontends/nib/src/runtime/runtime.nib");
    nib(&[
        &runtime,
        "-m32",
        "-O2",
        "--procedure-segments",
        "-Wno-target-width",
        "--used-by",
        "program.obj",
        "-o",
        "runtime.obj",
    ]);
}
