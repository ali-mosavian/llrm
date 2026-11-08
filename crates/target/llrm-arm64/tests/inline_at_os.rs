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

/// jpeg_fixed -Os: `tmul` and `clamp`, a multiply-divide and a compare-select
/// the unit model refuses at each site, were never tried inline because their
/// callers are over 300 instructions (the byte trial's bound); the copies are
/// smaller than the calls and the function (jpeg_fixed 3616 -> 3516 B).
#[test]
fn small_helpers_of_a_large_caller_are_tried_inline_at_os() {
    let asm = driver::assembly(include_str!("data/jpeg_fixed_os.ll"), Platform::Linux, Level::Os).expect("lowers");
    let kept: Vec<&str> = ["tmul", "clamp"].into_iter().filter(|name| asm.lines().any(|line| *line == format!("{name}:"))).collect();
    assert!(kept.is_empty(), "left as functions: {kept:?}");
}
