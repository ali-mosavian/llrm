use super::super::legalize_types::TypeLegalizer;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn counted(op: ISD) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let mut dword = |slot: i32| {
        let info =
            MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Stack(slot), ext: None, space: 0, ordering: None };
        dag.get_load(EVT::I32, chain, ptr, info)
    };
    let (lo, hi) = (dword(1), dword(2));
    let x = dag.get_node(ISD::BuildPair, EVT::I64, &[lo, hi]);
    let count = dag.get_node(op, EVT::I64, &[x]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[count]);
    dag.set_root(low);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let root = dag.get_root();
    (dag, root)
}

/// `ctpop i64` at 32 bits: the count of each half, added (an `i64` ctpop
/// stopped sdag-c's bit counts).
#[test]
fn an_i64_ctpop_is_the_sum_of_the_halves_counts() {
    let (dag, root) = counted(ISD::CtPop);
    assert_eq!(dag.opcode(root), ISD::Add);
    for half in [dag.operand(root, 0), dag.operand(root, 1)] {
        assert_eq!((dag.opcode(half), dag.value_type(half)), (ISD::CtPop, EVT::I32));
    }
    assert_ne!(dag.operand(root, 0), dag.operand(root, 1), "low half and high half");
}

/// `ctlz i64`: the high half's count when it is not zero, else the low half's
/// plus 32.
#[test]
fn an_i64_ctlz_selects_between_the_halves() {
    let (dag, root) = counted(ISD::CtLz);
    assert_eq!(dag.opcode(root), ISD::Select);
    let test = dag.operand(root, 0);
    assert_eq!(dag.opcode(test), ISD::SetCC);
    assert_eq!(
        dag.node(dag.operand(test, 2)).data,
        NodeData::CondCode(CondCode::SetNe),
        "the half that is counted first is not zero"
    );
    let (taken, other) = (dag.operand(root, 1), dag.operand(root, 2));
    assert_eq!(dag.opcode(taken), ISD::CtLzZeroPoison, "the high half's count");
    assert_eq!(dag.opcode(other), ISD::Add);
    assert_eq!(dag.constant(dag.operand(other, 1)), Some(32));
}

/// `cttz i64`: the low half's count when it is not zero, else the high half's
/// plus 32.
#[test]
fn an_i64_cttz_selects_between_the_halves() {
    let (dag, root) = counted(ISD::CtTz);
    assert_eq!(dag.opcode(root), ISD::Select);
    let test = dag.operand(root, 0);
    assert_eq!(dag.opcode(test), ISD::SetCC);
    assert_eq!(
        dag.node(dag.operand(test, 2)).data,
        NodeData::CondCode(CondCode::SetNe),
        "the half that is counted first is not zero"
    );
    let (taken, other) = (dag.operand(root, 1), dag.operand(root, 2));
    assert_eq!(dag.opcode(taken), ISD::CtTzZeroPoison, "the low half's count");
    assert_eq!(dag.opcode(other), ISD::Add);
    assert_eq!(dag.constant(dag.operand(other, 1)), Some(32));
}
