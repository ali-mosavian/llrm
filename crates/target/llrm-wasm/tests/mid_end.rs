//! `-O<n>` runs the shared mid-end before the wasm backend.

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

const SLOT: &str = "define i32 @f(i32 %x) {
entry:
  %p = alloca i32
  store i32 %x, ptr %p
  %v = load i32, ptr %p
  ret i32 %v
}
";

/// `llrm-wasm-llc` ignored `-O2`: every local stayed in a stack slot, so the
/// bench programs ran 4x the instructions of clang's and the same at every
/// level.
#[test]
fn a_stack_slot_is_a_local_at_o2_and_not_at_o0() {
    let text = |level| assembly(SLOT, Width::W32, Options { level, ..Options::default() }).unwrap();
    assert!(text(Level::O0).contains("i32.store"));
    assert!(!text(Level::O2).contains("i32.store"), "{}", text(Level::O2));
}
