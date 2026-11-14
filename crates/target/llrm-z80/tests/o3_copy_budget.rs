//! -O3 takes -O2's copy budget on the z80: the smallest code at every level.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

fn calls(level: Level) -> usize {
    let ir = include_str!("kernels/ir/nested_call_loops.ll");
    let asm = text(ir, "main", 0, &Request::new(level)).expect("compiles");
    asm.lines().filter(|line| line.trim_start().starts_with("call ")).count()
}

/// long_shift_by_a_call_result -O3 fully unrolled its 2 x 4 nested loops under LLVM's -O3 copy budget (200% of the
/// target's): 16 calls and 568 B against 431 B at -O2 and 457 B for SDCC. -O3 may not copy more calls than -O2.
#[test]
fn o3_copies_no_more_calls_than_o2_where_the_wider_copy_budget_unrolls_a_nest() {
    let (o2, o3) = (calls(Level::O2), calls(Level::O3));
    assert!(o3 <= o2, "O3 {o3} calls, O2 {o2}");
}
