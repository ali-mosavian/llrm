//! Complex patterns of the pin's `AArch64ISelDAGToDAG` on a small DAG, where no
//! program reaches them.

use llrm_arm64::dag::addr;
use llrm_dagisel::dag::{Dag, SDValue};
use llrm_dagisel::generated::ids::{isd, mvt, target_opcode};
use llrm_dagisel::matcher::{Opcode, Vt};
use llrm_dagisel::testing::TestDag;

fn value(
    dag: &mut TestDag,
    opcode: Opcode,
    vt: Vt,
    operands: &[SDValue],
) -> SDValue {
    SDValue::new(dag.node(opcode, &[vt], operands), 0)
}

/// `isDef32` of the pin lists `TargetOpcode::EXTRACT_SUBREG`, which no machine
/// node's `getOpcode()` equals, so a machine `EXTRACT_SUBREG` is a 32-bit
/// definition and `zext` of it is not an extended-register operand (the free
/// `mov w, w` serves). Ours refused it as not a definition, folded it, and so
/// differed from the pin.
#[test]
fn a_zero_extend_of_a_machine_extract_subreg_is_not_an_extended_register() {
    let mut dag = TestDag::new();
    let wide = dag.input(2, mvt::i64);
    let index = dag.make_constant(1, mvt::i32);
    let narrow = SDValue::new(dag.machine_node(target_opcode::EXTRACT_SUBREG, &[mvt::i32], &[wide, index]), 0);
    let extended = value(&mut dag, isd::ZERO_EXTEND, mvt::i64, &[narrow]);
    assert!(addr::arith_extended_register(&mut dag, true, extended).is_none());
}

/// The control: a register argument (`CopyFromReg`) is not a 32-bit definition,
/// so its `zext` folds as `uxtw`.
#[test]
fn a_zero_extend_of_a_register_argument_is_an_extended_register() {
    let mut dag = TestDag::new();
    let argument = dag.input(1, mvt::i32);
    let extended = value(&mut dag, isd::ZERO_EXTEND, mvt::i64, &[argument]);
    assert!(addr::arith_extended_register(&mut dag, true, extended).is_some());
}

/// A constant offset no scaled immediate and no single `add` takes is the index
/// register of the register-offset form (`SelectAddrModeXRO` builds
/// `MOVi64imm`): `[base, x]`, no shift. Ours returned nothing for any constant.
#[test]
fn a_wide_constant_offset_is_the_index_register() {
    let mut dag = TestDag::new();
    let base = dag.input(1, mvt::i64);
    let offset = dag.make_constant(70000, mvt::i64);
    let address = value(&mut dag, isd::ADD, mvt::i64, &[base, offset]);
    let found = addr::addr_mode_xro(&mut dag, false, address, 4).expect("the register-offset form");
    assert_eq!((found[0], found[1]), (base, offset));
}

/// The offsets the pin leaves to the immediate forms and to `add`: a scaled
/// 12-bit one, a 12-bit one, its negative, a 12-bit one shifted by 12 that no
/// single `movz` makes.
#[test]
fn an_offset_an_immediate_form_takes_is_not_the_index_register() {
    for offset in [8, 4104, 0xfff, -8, 0x45000] {
        let mut dag = TestDag::new();
        let base = dag.input(1, mvt::i64);
        let constant = dag.make_constant(offset, mvt::i64);
        let address = value(&mut dag, isd::ADD, mvt::i64, &[base, constant]);
        assert!(addr::addr_mode_xro(&mut dag, false, address, 4).is_none(), "{offset}");
    }
}

/// `0x7000` is one `movz`, so `isPreferredADD` leaves it to the register form.
#[test]
fn an_offset_one_movz_makes_is_the_index_register() {
    let mut dag = TestDag::new();
    let base = dag.input(1, mvt::i64);
    let offset = dag.make_constant(0x7000, mvt::i64);
    let address = value(&mut dag, isd::ADD, mvt::i64, &[base, offset]);
    assert!(addr::addr_mode_xro(&mut dag, false, address, 4).is_some());
}
