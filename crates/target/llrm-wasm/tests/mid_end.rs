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

const SUM: &str = "define i32 @f(i32 %x, i32 %y) {
entry:
  %s = add i32 %x, %y
  ret i32 %s
}
";

/// The function ended in an explicit `return` before its `end`, as `-O2` kept
/// the pin's `WebAssemblyPeephole` out: one byte more per function than
/// clang's (fib: 157 B against 153 B over four functions). Executed
/// instructions are the same either way: `wasm-interp --trace` counts the
/// `return` or the `end` once.
#[test]
fn the_final_return_falls_through_at_o2_and_not_at_o0() {
    let text = |level| assembly(SUM, Width::W32, Options { level, ..Options::default() }).unwrap();
    assert!(text(Level::O0).contains("\treturn"), "{}", text(Level::O0));
    assert!(text(Level::O2).contains("# fallthrough-return"), "{}", text(Level::O2));
    assert!(!text(Level::O2).contains("\treturn"), "{}", text(Level::O2));
}
