//! The m68k machine states `TargetLowering` facts to CodeGenPrepare. Without
//! them the mid-end skips every step that reads one.

use llrm_m68k::driver::machine;
use llrm_mir::lowering::CgpStep;

#[test]
fn the_machine_answers_the_register_types_of_the_pin() {
    let lowering = machine().lowering().expect("m68k states its lowering facts");
    for bits in [8, 16, 32] {
        assert!(lowering.is_type_legal(bits), "i{bits} is a register type");
    }
    for bits in [1, 64] {
        assert!(!lowering.is_type_legal(bits), "i{bits} is not");
    }
}

#[test]
fn overflow_math_stays_out_until_the_carry_is_folded_into_the_branch() {
    let lowering = machine().lowering().expect("lowering");
    assert!(!lowering.runs_cgp_step(CgpStep::OverflowMath));
}
