use llrm_6502::alloc::Mos;
use llrm_spike_mf::regalloc::Target;

/// A copy was priced 1 (the trait default) against operand costs in bytes plus
/// clocks, so the join of `x + 1` with its argument cell (a saved `ldx rc2`,
/// 2 bytes and 3 clocks) was declined for any extra cost above 1, and the step
/// went through `a` as `clc; adc #1`. `wrap` in cmos.rs reads it.
#[test]
fn a_copy_costs_what_loading_a_cell_costs_in_the_timing_table() {
    assert_eq!(Mos::<true>::copy_cost(), 5);
    assert_eq!(Mos::<false>::copy_cost(), 5);
}
