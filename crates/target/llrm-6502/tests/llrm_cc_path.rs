//! The tests that run `llrm-cc` looked for `<worktree>/target/release/llrm-cc`
//! whatever `CARGO_TARGET_DIR` and profile built them: a stale binary there ran
//! for the code under test, and a fresh build elsewhere was ignored.

mod common;

use std::path::Path;

#[test]
fn llrm_cc_is_the_one_beside_the_test_executable_not_a_fixed_path_in_the_tree() {
    let exe = Path::new("/scratch/x-target/fast/deps/driver-0123abcd");
    assert_eq!(common::pipeline::llrm_cc_beside(exe), Path::new("/scratch/x-target/fast/llrm-cc"));
    let stale = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/release/llrm-cc");
    assert_ne!(common::pipeline::llrm_cc_beside(exe), stale);
}
