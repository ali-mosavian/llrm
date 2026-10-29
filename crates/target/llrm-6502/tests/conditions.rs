//! The 6502 lowering of conditions on hand-built DAGs. Expected nodes are worked out by hand from
//! the flags a `CMP a, b` leaves: Z set when equal, C set when `a >= b` unsigned (SBC convention:
//! no borrow), N the top bit of `a - b`.

use llrm_6502::dagisel::lowering::{MosLowering, cond, mosisd, target_lowering};
use llrm_codegen::backend::selectiondag::isd::{CondCode, ISD};
use llrm_codegen::backend::selectiondag::legalize_dag::DagLegalizer;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{DagConfig, SDValue, SelectionDAG};

fn legalize(dag: &mut SelectionDAG) -> SDValue {
    let tli = target_lowering();
    DagLegalizer::new(dag, &tli, &mut MosLowering).run().expect("legalizes");
    dag.get_root()
}

fn inputs(dag: &mut SelectionDAG) -> [SDValue; 2] {
    [1, 2].map(|n| dag.get_register(EVT::I8, n))
}

/// `brcond (setne a, b)`: branch on Z clear after `cmp a, b`.
#[test]
fn branch_on_not_equal_is_bne_on_cmp() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let [a, b] = inputs(&mut dag);
    let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetNe);
    let dest = dag.get_basic_block(0);
    let entry = dag.entry_node();
    let br = dag.get_node(ISD::BrCond, EVT::OTHER, &[entry, test, dest]);
    dag.set_root(br);

    let root = legalize(&mut dag);

    assert_eq!(dag.opcode(root), mosisd::BRCOND);
    assert_eq!(dag.constant(dag.operand(root, 2)), Some(cond::NE));
    let cmp = dag.operand(root, 3);
    assert_eq!((dag.opcode(cmp), dag.operand(cmp, 0), dag.operand(cmp, 1)), (mosisd::CMP, a, b));
}

/// `a <u b` is "no carry" (CC) on `cmp a, b`; `a >u b` is `b <u a`: the same code, operands swapped.
#[test]
fn unsigned_less_is_carry_clear_and_greater_swaps_the_operands() {
    for (cc, swapped) in [(CondCode::SetULt, false), (CondCode::SetUGt, true)] {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let [a, b] = inputs(&mut dag);
        let test = dag.get_setcc(EVT::I1, a, b, cc);
        dag.set_root(test);

        let root = legalize(&mut dag);

        assert_eq!((dag.opcode(root), dag.constant(dag.operand(root, 0))), (mosisd::SETCC, Some(cond::CC)));
        let cmp = dag.operand(root, 1);
        let want = if swapped { [b, a] } else { [a, b] };
        assert_eq!([dag.operand(cmp, 0), dag.operand(cmp, 1)], want, "{cc:?}");
    }
}

/// A signed compare against zero is the sign of the byte: `a <s 0` is "minus" (MI) on `cmp a, 0`.
#[test]
fn signed_less_than_zero_is_minus() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let [a, _] = inputs(&mut dag);
    let zero = dag.get_constant(0, EVT::I8);
    let test = dag.get_setcc(EVT::I1, a, zero, CondCode::SetLt);
    dag.set_root(test);

    let root = legalize(&mut dag);

    assert_eq!(dag.constant(dag.operand(root, 0)), Some(cond::MI));
    let cmp = dag.operand(root, 1);
    assert_eq!((dag.operand(cmp, 0), dag.is_null_constant(dag.operand(cmp, 1))), (a, true));
}

/// Any other signed compare flips both sign bits and compares unsigned: `a <s 5` is
/// `(a ^ 0x80) <u (5 ^ 0x80)`, the constant folded to 0x85.
#[test]
fn signed_less_than_a_constant_flips_the_sign_bits() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let [a, _] = inputs(&mut dag);
    let five = dag.get_constant(5, EVT::I8);
    let test = dag.get_setcc(EVT::I1, a, five, CondCode::SetLt);
    dag.set_root(test);

    let root = legalize(&mut dag);

    assert_eq!(dag.constant(dag.operand(root, 0)), Some(cond::CC));
    let cmp = dag.operand(root, 1);
    let flipped = dag.operand(cmp, 0);
    assert_eq!((dag.opcode(flipped), dag.operand(flipped, 0)), (ISD::Xor, a));
    assert_eq!(dag.constant(dag.operand(cmp, 1)), Some(0x85));
}

/// `select (setuge a, b), x, y` is `CMOV y, x, CS, (cmp a, b)`: the false value first.
#[test]
fn select_on_unsigned_greater_equal_is_cmov_on_carry_set() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let [a, b] = inputs(&mut dag);
    let [x, y] = [3, 4].map(|n| dag.get_register(EVT::I8, n));
    let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetUGe);
    let select = dag.get_node(ISD::Select, EVT::I8, &[test, x, y]);
    dag.set_root(select);

    let root = legalize(&mut dag);

    assert_eq!(dag.opcode(root), mosisd::CMOV);
    assert_eq!([dag.operand(root, 0), dag.operand(root, 1)], [y, x]);
    assert_eq!(dag.constant(dag.operand(root, 2)), Some(cond::CS));
}

fn words(dag: &mut SelectionDAG) -> [SDValue; 2] {
    [1, 2].map(|n| dag.get_register(EVT::I16, n))
}

/// A word `<u` compares like a byte: C clear after `cmp a, b`, on the word operands.
#[test]
fn a_word_unsigned_less_is_carry_clear_on_a_word_cmp() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let [a, b] = words(&mut dag);
    let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetULt);
    dag.set_root(test);

    let root = legalize(&mut dag);

    assert_eq!(dag.constant(dag.operand(root, 0)), Some(cond::CC));
    let cmp = dag.operand(root, 1);
    assert_eq!((dag.opcode(cmp), dag.operand(cmp, 0), dag.operand(cmp, 1)), (mosisd::CMP, a, b));
}

/// `cmp`'s Z covers only the high byte of a word, so a word `==` is `CMPZ`, which compares both.
#[test]
fn a_word_equality_is_cmpz() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let [a, b] = words(&mut dag);
    let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetEq);
    dag.set_root(test);

    let root = legalize(&mut dag);

    assert_eq!(dag.constant(dag.operand(root, 0)), Some(cond::EQ));
    assert_eq!(dag.opcode(dag.operand(root, 1)), mosisd::CMPZ);
}

/// A signed word compare against a constant flips bit 15: `a <s 5` is `(a ^ 0x8000) <u 0x8005`.
#[test]
fn a_signed_word_compare_flips_bit_15() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let [a, _] = words(&mut dag);
    let five = dag.get_constant(5, EVT::I16);
    let test = dag.get_setcc(EVT::I1, a, five, CondCode::SetLt);
    dag.set_root(test);

    let root = legalize(&mut dag);

    let cmp = dag.operand(root, 1);
    let flipped = dag.operand(cmp, 0);
    assert_eq!(dag.opcode(flipped), ISD::Xor);
    assert_eq!(dag.constant(dag.operand(flipped, 1)), Some(0x8000));
    assert_eq!(dag.constant(dag.operand(cmp, 1)), Some(0x8005));
}

/// Shifts stay shifts for the selector, by a constant (an instruction per bit) or by a register (a
/// runtime call where emitted). Before the pseudos the variable one was refused by the legalizer.
#[test]
fn constant_and_variable_shifts_are_kept() {
    for by_register in [false, true] {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let [a, n] = inputs(&mut dag);
        let amount = if by_register { n } else { dag.get_constant(3, EVT::I8) };
        let shift = dag.get_node(ISD::Shl, EVT::I8, &[a, amount]);
        dag.set_root(shift);
        let root = legalize(&mut dag);
        assert_eq!(dag.opcode(root), ISD::Shl, "by_register = {by_register}");
    }
}
