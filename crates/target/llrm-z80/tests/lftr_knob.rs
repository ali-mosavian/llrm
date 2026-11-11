//! DEVIATE LFTR1: following the pin's step-one counter rule grew z80 `variadic` by 30 B at -O2 and
//! 51 B at -Os; the knob must stay on here.

use llrm_mir::tti::Tti;

#[test]
fn lftr_counts_any_constant_stride() {
    assert!(llrm_z80::tti::Z80Tti::new().lftr_any_stride());
}
