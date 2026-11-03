//! A signed divide or remainder of sign-extended narrow values is made at the
//! narrow width where the target says so: `srem i32` of two words called
//! `__modsi3` (recpow at 6502 -O1, +434 B).

use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

fn word_divisor(dag_hint: &mut SelectionDAG) -> SDValue {
    let y = dag_hint.get_register(EVT::I16, 6);
    dag_hint.get_node(ISD::SignExtend, EVT::I32, &[y])
}

/// The divisor has to live in the same DAG as the dividend, so each case
/// builds both.
fn case(
    narrow: Option<EVT>,
    op: ISD,
    dividend: EVT,
    divisor_constant: Option<u128>,
) -> (SelectionDAG, SDValue) {
    case_with(narrow, false, op, dividend, divisor_constant)
}

fn case_with(
    narrow: Option<EVT>,
    srem_defined: bool,
    op: ISD,
    dividend: EVT,
    divisor_constant: Option<u128>,
) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    let mut dag = SelectionDAG::new(DagConfig {
        narrow_divrem: narrow,
        narrow_srem_overflow_defined: srem_defined,
        ..DagConfig::default()
    });
    let x = dag.get_register(dividend, 5);
    let x = dag.get_node(ISD::SignExtend, EVT::I32, &[x]);
    let y = match divisor_constant {
        Some(c) => dag.get_constant(c, EVT::I32),
        None => word_divisor(&mut dag),
    };
    let divide = dag.get_node(op, EVT::I32, &[x, y]);
    dag.set_root(divide);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let root = dag.get_root();
    (dag, root)
}

/// `srem i32` of two sign-extended words is a word `srem`, sign extended, where
/// the target's word remainder is defined for -32768 % -1. Elsewhere (an x86
/// `idiv` traps there) the width is bumped as the pin's `narrowSDivOrSRem`
/// does, and only a divisor constant other than -1 narrows it.
#[test]
fn a_remainder_of_two_sign_extended_words_is_made_at_the_word() {
    let (dag, root) = case_with(Some(EVT::I16), true, ISD::SRem, EVT::I16, None);
    assert_eq!(dag.opcode(root), ISD::SignExtend, "{:?}", dag.opcode(root));
    let inner = dag.operand(root, 0);
    assert_eq!((dag.opcode(inner), dag.value_type(inner)), (ISD::SRem, EVT::I16));
    let (dag, root) = case(None, ISD::SRem, EVT::I16, None);
    assert_eq!(dag.opcode(root), ISD::SRem, "without the target's say-so");
    let (dag, root) = case_with(Some(EVT::I16), false, ISD::SRem, EVT::I16, None);
    assert_eq!(dag.opcode(root), ISD::SRem, "a word remainder that traps at the minimum");
    let (dag, root) = case_with(Some(EVT::I16), false, ISD::SRem, EVT::I16, Some(1000));
    assert_eq!(dag.opcode(root), ISD::SignExtend, "over a constant other than -1");
}

/// -32768 / -1 is 32768, which a word quotient cannot hold: two sign-extended
/// words stay a long divide. A byte dividend has a spare sign bit, and a
/// divisor constant other than -1 cannot be the overflow case.
#[test]
fn a_divide_narrows_only_where_the_word_quotient_cannot_overflow() {
    let (dag, root) = case(Some(EVT::I16), ISD::SDiv, EVT::I16, None);
    assert_eq!(dag.opcode(root), ISD::SDiv, "two words");
    let (dag, root) = case(Some(EVT::I16), ISD::SDiv, EVT::I8, None);
    assert_eq!(dag.opcode(root), ISD::SignExtend, "a byte over a word");
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::SDiv);
    let (dag, root) = case(Some(EVT::I16), ISD::SDiv, EVT::I16, Some(7));
    assert_eq!(dag.opcode(root), ISD::SignExtend, "a word over 7");
}

/// `udiv i32` of two zero-extended words is a word `udiv`, zero extended:
/// `ONE / (d >> 18) + 1` was `__udivsi3` at 6502 -O1 (+312 B on nbody_fixed)
/// once the pin's O1 had no correlated-propagation to narrow it.
#[test]
fn an_unsigned_divide_of_zero_extended_words_is_made_at_the_word() {
    let build = |narrow: Option<EVT>, op: ISD| {
        let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
        let mut dag = SelectionDAG::new(DagConfig { narrow_divrem: narrow, ..DagConfig::default() });
        let x = dag.get_register(EVT::I16, 5);
        let x = dag.get_node(ISD::ZeroExtend, EVT::I32, &[x]);
        let y = dag.get_register(EVT::I16, 6);
        let y = dag.get_node(ISD::ZeroExtend, EVT::I32, &[y]);
        let divide = dag.get_node(op, EVT::I32, &[x, y]);
        dag.set_root(divide);
        DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
        let root = dag.get_root();
        (dag, root)
    };
    for op in [ISD::UDiv, ISD::URem] {
        let (dag, root) = build(Some(EVT::I16), op);
        assert_eq!(dag.opcode(root), ISD::ZeroExtend, "{op:?}");
        assert_eq!(dag.opcode(dag.operand(root, 0)), op);
        let (dag, root) = build(None, op);
        assert_eq!(dag.opcode(root), op, "without the target's say-so");
    }
}
