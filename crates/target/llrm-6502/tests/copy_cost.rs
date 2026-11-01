use llrm_6502::alloc::Mos;
use llrm_spike_mf::regalloc::Target;

/// A copy was priced 1 (the trait default) against operand costs in bytes
/// plus clocks (`tax` is 1 byte and 2 clocks), so a join that saved a
/// 3-unit copy was declined for any extra cost above 1.
#[test]
fn a_copy_costs_what_a_transfer_costs_in_the_timing_table() {
    assert_eq!(Mos::<true>::copy_cost(), 3);
    assert_eq!(Mos::<false>::copy_cost(), 3);
}
