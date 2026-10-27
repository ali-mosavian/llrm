use super::super::legalize_types::TypeLegalizer;
use super::*;
use crate::backend::selectiondag::x86_lowering::x86isd;

fn legalized(build: impl FnOnce(&mut SelectionDAG, SDValue, SDValue) -> SDValue) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let wide = |dag: &mut SelectionDAG, at| {
        let (lo, hi) = (dag.get_register(EVT::I32, at), dag.get_register(EVT::I32, at + 1));
        dag.get_node(ISD::BuildPair, EVT::I64, &[lo, hi])
    };
    let (a, b) = (wide(&mut dag, 7), wide(&mut dag, 9));
    let root = build(&mut dag, a, b);
    dag.set_root(root);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let root = dag.get_root();
    (dag, root)
}

/// `a == b` on an `i64` read only the high words' compare once the
/// type legalizer let a full `setcc` through: both halves must reach it.
#[test]
fn wide_equality_compares_both_halves() {
    let (dag, root) = legalized(|dag, a, b| dag.get_setcc(EVT::I8, a, b, CondCode::SetEq));
    let or = dag.operand(root, 0);
    assert_eq!(dag.opcode(or), ISD::Or);
    assert_eq!((dag.opcode(dag.operand(or, 0)), dag.opcode(dag.operand(or, 1))), (ISD::Xor, ISD::Xor));
}

/// `a < b` (signed) is the high words' signed compare fed the borrow of the low
/// words' unsigned subtract (`SETCCCARRY`, `cmp; sbb`), where it was a select
/// between two compares: the low borrow must be unsigned, or `0 < 0x80000000`
/// style low halves compare wrong.
#[test]
fn wide_signed_less_is_the_high_compare_with_the_low_borrow() {
    let (dag, root) = legalized(|dag, a, b| dag.get_setcc(EVT::I8, a, b, CondCode::SetLt));
    assert_eq!(dag.opcode(root), ISD::SetCCCarry);
    let borrow = dag.operand(root, 2);
    assert_eq!((dag.opcode(borrow), borrow.res), (ISD::USubO, 1));
    assert_eq!(dag.node(dag.operand(root, 3)).data, NodeData::CondCode(CondCode::SetLt));
}

/// `a > b` is `b < a`: the operands swap.
#[test]
fn wide_greater_swaps_the_operands_of_the_carry_compare() {
    let (dag, root) = legalized(|dag, a, b| dag.get_setcc(EVT::I8, a, b, CondCode::SetUGt));
    assert_eq!(dag.opcode(root), ISD::SetCCCarry);
    assert_eq!(dag.node(dag.operand(root, 3)).data, NodeData::CondCode(CondCode::SetULt));
}

/// A select of `i64`s is one select per half with the same condition.
#[test]
fn wide_select_selects_each_half() {
    let (dag, root) = legalized(|dag, a, b| {
        let cond = dag.get_register(EVT::I8, 9);
        let sel = dag.get_select(EVT::I64, cond, a, b);
        let one = dag.get_constant(1, EVT::I32);
        dag.get_node(ISD::ExtractElement, EVT::I32, &[sel, one])
    });
    assert_eq!(dag.opcode(root), ISD::Select);
}

/// A bitcast of an `i32` to a far pointer: its low word is the offset.
#[test]
fn wide_bitcast_from_a_word_splits_low_and_high() {
    let tli = TargetLowering::empty(TargetLoweringInfo::x86_16());
    let mut dag = dag();
    let word = dag.get_register(EVT::I32, 7);
    let far = dag.get_node(ISD::BitCast, MVT::Far.into(), &[word]);
    let offset = dag.get_node(ISD::Truncate, EVT::I16, &[far]);
    dag.set_root(offset);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.operand(root, 0)), (ISD::Truncate, word));
}

/// A huge pointer plus a dword carries into the selector: offset low word
/// of the dword sum, selector stepped by the sum's high word times the DOS
/// stride (shift 12). Without the expansion the node stayed unexpanded and
/// the function was not selected.
#[test]
fn huge_add_steps_the_selector_by_the_carry() {
    let tli = TargetLowering::empty(TargetLoweringInfo::x86_16());
    let mut dag = dag();
    let (offset, selector) = (dag.get_register(EVT::I16, 7), dag.get_register(EVT::I16, 8));
    let far = dag.get_node(ISD::BuildPair, MVT::Far.into(), &[offset, selector]);
    let delta = dag.get_register(EVT::I32, 9);
    let moved = dag.get_node(x86isd::HUGE_ADD, MVT::Far.into(), &[far, delta]);
    let pair = dag.get_node(ISD::BitCast, EVT::I32, &[moved]);
    dag.set_root(pair);
    TypeLegalizer::new(&mut dag, &tli).with_target(&llrm_x86_m16::M16).run().unwrap();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::BuildPair);
    let (low, high) = (dag.operand(root, 0), dag.operand(root, 1));
    let sum = dag.operand(low, 0);
    assert_eq!((dag.opcode(low), dag.opcode(sum)), (ISD::Truncate, ISD::Add));
    assert_eq!((dag.opcode(high), dag.operand(high, 0)), (ISD::Add, selector));
    let shl = dag.operand(dag.operand(high, 1), 0);
    assert_eq!((dag.opcode(shl), dag.constant(dag.operand(shl, 1))), (ISD::Shl, Some(12)));
    assert_eq!(dag.opcode(dag.operand(shl, 0)), ISD::Sra);
}

/// A `poison` or `undef` far pointer (a null-ish default of a far argument)
/// stopped the type legalizer: "Poison: expand result" (c/fsgs_call.c of the
/// m32 run). It is a poison of each half.
#[test]
fn an_undef_or_poison_wide_value_expands_to_undef_halves() {
    for (make, op) in [
        (SelectionDAG::get_undef as fn(&mut SelectionDAG, EVT) -> SDValue, ISD::Undef),
        (SelectionDAG::get_poison, ISD::Poison),
    ] {
        for (tli, wide, half) in [
            (TargetLowering::empty(TargetLoweringInfo::x86_16()), EVT::from(MVT::Far), EVT::I16),
            (TargetLowering::x86_32(Features::default(), MVT::I32), EVT::I64, EVT::I32),
        ] {
            let mut dag = dag();
            let value = make(&mut dag, wide);
            let low = dag.get_node(ISD::Truncate, half, &[value]);
            dag.set_root(low);
            let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
            legalizer.run().unwrap();
            assert!(legalizer.all_legal(), "{op:?} {wide:?}");
        }
    }
}
