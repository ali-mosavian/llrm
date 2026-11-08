//! At -Os an inline does not grow the module.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// jpeg_fixed -Os: `put32`, a 4-trip store loop, was copied into each of its
/// eight callers (the pin's `OptSizeThreshold` 50 allows growth), 176 B over
/// gcc, which keeps it a function.
#[test]
fn a_store_loop_called_from_six_sites_is_not_copied_into_each_site_at_os() {
    let asm = driver::assembly(include_str!("data/store_loop_sites.ll"), Platform::Linux, Level::Os).expect("lowers");
    let calls = asm.lines().filter(|line| line.trim().starts_with("bl ") && line.contains("put32")).count();
    let copies = asm.lines().filter(|line| line.trim().starts_with("strb ")).count();
    assert!(calls >= 1 && copies <= 2, "{calls} calls, {copies} copies of the loop: {asm}");
}
