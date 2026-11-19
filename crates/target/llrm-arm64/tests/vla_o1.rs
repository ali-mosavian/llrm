//! A variable-length array at -O1 once panicked in a debug build.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// release/next 4c9d3eea71, `llrm-arm64-llc -O1 tests/darwin/vla.ll`:
/// "attempt to subtract with overflow" at `promote.rs` (`span.high - span.low`
/// of a dynamic alloca's unbounded range), rc=101. A release build wrapped.
#[test]
fn a_variable_length_array_compiles_at_o1() {
    let source = include_str!("darwin/vla.ll");
    driver::assembly(source, Platform::Darwin, Level::O1).expect("lowers");
}
