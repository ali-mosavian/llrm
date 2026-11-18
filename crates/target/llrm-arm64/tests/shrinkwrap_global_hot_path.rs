//! A path that only reads globals makes no frame (`ShrinkWrap`).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// jpeg_single -O3 `getbit`: the hot path returns after one global load and
/// store, the cold one calls `read`. The frame was made at the entry (14
/// instructions a call, clang 11): a global access was taken for a stack access
/// (`IsKnownNonStackPtr`), and a page shared with a `MOVaddr` kept the address
/// in a callee-saved register from the entry.
#[test]
fn a_hot_path_that_only_touches_globals_makes_no_frame() {
    let asm = driver::assembly(include_str!("data/shrinkwrap_global_hot_path.ll"), Platform::Linux, Level::O3)
        .expect("lowers");
    let body: Vec<&str> = asm
        .lines()
        .skip_while(|line| *line != "getbit:")
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.starts_with('.'))
        .collect();
    let branch = body.iter().position(|line| line.starts_with("cbnz")).expect("the test of the bit count");
    assert!(
        body[..branch].iter().all(|line| !line.starts_with("stp") && !line.starts_with("str x30")),
        "the frame is made before the branch: {body:#?}"
    );
}
