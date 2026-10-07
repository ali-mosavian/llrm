//! A copy the loop runs is joined where the register pressure that made Briggs and George refuse it costs less than the
//! copy: #792.

use std::process::Command;

/// bench/nbody_fixed at -Os on the 486: the four loop-carried values and their copies on the self-pair path were left
/// in registers rotated one step apart (Briggs and George refused every join: the pair loop holds more values than the
/// target has registers), and both edges were 4-cycles of `xchg`, three each: six `xchg` and 5683857 clocks in the
/// vsgcc harness against 5300004 before the lea chains of #789 moved the colouring. A join whose copies run more
/// than the cheapest value it could force out costs is taken.
#[test]
fn test_a_loop_carried_value_and_its_copy_share_a_register_where_the_copy_runs_more_than_a_spill() {
    let scratch = tempfile::tempdir().unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/nbody_fixed/nbody_fixed.c");
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(scratch.path()).args(["-m32", "-Os", "-march=i486", "-S", "-o", "a.s"]).arg(&source).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let exchanges = listing.lines().filter(|line| line.trim_start().starts_with("xchg ")).count();
    assert!(exchanges <= 2, "{exchanges} xchg\n{listing}");
}
