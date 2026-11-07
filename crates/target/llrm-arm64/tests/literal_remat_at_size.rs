//! At -Os a constant that stays read is made again where it is the zero
//! register's move.

use llrm_arm64::lir_view::Arm64;
use llrm_codegen::model::ir::{Held, Imm, Loc, Operation, Semantics};
use llrm_codegen::model::lir::Insn;
use llrm_lir::registers::RegId;
use llrm_spike_mf::tolir::literal_remat_wins;

fn literal(name: &str) -> Insn {
    let what = Semantics {
        name: Some(name.into()),
        dests: vec![Loc::Held(Held { value: 1, width: 8, offset: 0 })],
        sources: vec![Loc::Imm(Imm { value: 4961950020807581558, width: 8, address: None })],
        ..Semantics::new(Operation::Move)
    };
    Insn::new(0, None, Some(what), vec![1], vec![])
}

/// jpeg_single -Os copied a zero through x20 (`orr x20,xzr,xzr; orr
/// x24,xzr,x20; orr x10,xzr,x20`; 61 reg-reg moves against the
/// pin's 19) because C10 kept every constant that stays read as a copy; the pin
/// makes the zero again. `mul_constants` -Os grew 124 B when every constant was
/// made again, so another constant stays a copy.
#[test]
fn a_zero_that_stays_read_is_made_again_at_size_but_another_constant_is_not() {
    let (from, to) = (Vec::<RegId>::new(), Vec::<RegId>::new());
    assert!(literal_remat_wins::<Arm64>(&literal("Zero"), &from, &to, true, false));
    assert!(!literal_remat_wins::<Arm64>(&literal("Imm"), &from, &to, true, false));
}
