//! `PostRAMachineSinking`: a copy into a callee-saved register moves to the
//! one successor that reads it.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// The argument was copied into `w19` in the entry block, ahead of the test,
/// so the frame (three stores and three loads) was made even for the call
/// that returns at once; the pin tests first and makes the frame after
/// (recpow/rectwo: the leaf call cost 14 instructions, the pin's 5).
#[test]
fn the_early_exit_runs_before_the_frame() {
    let asm = driver::assembly(include_str!("data/post_ra_sink.ll"), Platform::Linux, Level::O2).expect("lowers");
    let start = asm.find("paths:").expect("the function is there");
    let body = &asm[start..];
    let test = body.find("cbz w").expect("the test");
    let frame = body.find("stp x29, x30").expect("the frame");
    assert!(test < frame, "{body}");
}

/// A block that is one jump (the edge out of a branch whose target has other
/// predecessors) is no place for a copy: the pin has no such block, and the
/// copy sunk into ours cost a `b` back (switch -Os: +4 B).
#[test]
fn a_copy_is_not_sunk_into_an_edge_block() {
    let asm =
        driver::assembly(include_str!("data/post_ra_sink_edge.ll"), Platform::Linux, Level::Os).expect("lowers");
    let jumps = asm.lines().filter(|line| line.starts_with("\tb ")).count();
    assert_eq!(jumps, 7, "{asm}");
}
