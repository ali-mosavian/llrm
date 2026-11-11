//! llvm-mos's `isZExtFree` holds for every widening of an integer, so its InlineCost prices a
//! `zext` 0. The 6502 priced it per new byte in every cost kind: `frame_address_stack_args` -Os
//! read `kernel` into `line` at 65 against llvm-mos's 50 (three zexts of 5), `terms` at -10
//! against -20 (two).

use llrm_mir::tti::{CastKind, Cost, CostKind, Ty};

#[test]
fn a_zero_extension_is_free_to_the_passes_and_costs_its_bytes_to_the_size_models() {
    let tti = llrm_6502::driver::machine().tti().expect("a Tti");
    let zext = |kind| tti.cast_cost(CastKind::ZExt, Ty::Int(32), Ty::Int(16), kind);
    assert_eq!(zext(CostKind::SizeAndLatency), Cost::FREE, "InlineCost's kind");
    assert_ne!(zext(CostKind::CodeSize), Cost::FREE, "each widened byte is a `lda #0; sta`");
    assert_ne!(
        tti.cast_cost(CastKind::SExt, Ty::Int(32), Ty::Int(16), CostKind::SizeAndLatency),
        Cost::FREE,
        "a sign extension is a branch and a fill"
    );
}
