//! At -Os the pass rotates a loop only when the copied exit test folds in the
//! preheader; the trial (DEVIATE ROTATION-HOT) keeps a rotation that executes
//! less for a few bytes.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn sieve_instructions(trial: bool) -> usize {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/sieve_os.ll")).unwrap();
    let mut llc = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    llc.args(["-S", "-Os", "-o"]).arg(&output).arg(&input);
    if !trial {
        llc.env("LLRM_ARM64_NO_TRIAL", "1");
    }
    let said = llc.output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    let insts = asm.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
    assert!(insts >= 40, "premise: the function is lowered:\n{asm}");
    insts
}

/// sieve -Os: the outer loop's unfoldable `i*i <= n` test copied into the
/// preheader is one instruction (4 B) over the unrotated loop, so the pass
/// leaves it (51 instructions). Since #9699 the placer copies that test into
/// the latch (TailDupPlacement), the same instruction: 52 with and without the
/// trial, so the trial's rotation (below) costs no more bytes than the pass alone.
#[test]
fn a_loop_whose_copied_exit_test_does_not_fold_is_not_rotated_by_the_pass_at_os() {
    assert_eq!(sieve_instructions(false), 52);
}

/// DEVIATE ROTATION-HOT: the same loop rotated runs 6.8% fewer instructions
/// (19087 -> 17788, the harness count) for those 4 B, and the trial keeps it.
/// Before, sieve -Os was 200 B and 19087 instructions against clang's 17944.
#[test]
fn the_trial_keeps_the_rotation_that_executes_less_for_four_bytes_at_os() {
    assert_eq!(sieve_instructions(true), 52);
}

/// rectwo -Os: `n - 1`, invariant in the loop that calls `paths` again, was
/// hoisted into the entry block (LICM with speculation, the loop being
/// unrotated); it lived in x19 across the call, so the frame was built before
/// the `n == 0` test and every base-case call (most of 970,000 instructions
/// against clang's 778,000) paid the prologue and epilogue.
#[test]
fn the_base_case_of_a_recursive_loop_returns_before_the_frame_is_built_at_os() {
    let asm = driver::assembly(include_str!("data/frame_late_os.ll"), Platform::Linux, Level::Os).expect("lowers");
    let body: Vec<&str> = asm
        .lines()
        .skip_while(|line| *line != "paths:")
        .skip(1)
        .take_while(|line| !line.ends_with(':') || line.starts_with(".L"))
        .collect();
    let first_frame = body.iter().position(|line| line.trim().starts_with("stp x29, x30"));
    let first_test = body.iter().position(|line| line.trim().starts_with("cbz"));
    assert!(first_frame.is_some() && first_test.is_some(), "premise: a framed function:\n{asm}");
    assert!(first_test < first_frame, "the frame is built before the n == 0 test:\n{asm}");
}
