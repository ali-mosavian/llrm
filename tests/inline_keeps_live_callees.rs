//! A body is not bared while a call to it stands.

use std::process::Command;

/// The inliner's count of live calls did not see the calls the estimated sites
/// spliced in: `Suit.eq` was bared with two calls to it in `main`, the
/// equal-rank path of tally's `Card.eq` ran into `unreachable` and the program
/// printed "index out of bounds" (found through CVP, whose constant phi moved
/// the sizes).
#[test]
fn test_tally_is_compiled_with_no_call_to_a_bared_body() {
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
        .current_dir(scratch.path())
        .env("LLRM_VERIFY", "1")
        .arg("-m16")
        .arg("-O2")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/tally.nib"))
        .args(["-o", "t.obj"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
