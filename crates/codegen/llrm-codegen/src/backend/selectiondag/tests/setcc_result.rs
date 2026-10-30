use super::super::legalize_types::TypeLegalizer;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn load(
    dag: &mut SelectionDAG,
    vt: EVT,
    slot: i32,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: vt,
        align: 1,
        volatile: false,
        base: MemBase::Stack(slot),
        ext: None,
        space: 0,
        ordering: None,
    };
    dag.get_load(vt, chain, ptr, info)
}

/// `select (icmp slt a, b), x, y`: the compare is an `i1`, which no
/// register holds. 73 of 160 functions of the shadow corpus stopped
/// here ("SetCC: promote result"). After: an `i8` compare (the target's
/// result type) zero-extended-in-register into the select's condition,
/// and nothing illegal reachable.
#[test]
fn an_i1_compare_feeding_a_select_is_promoted_to_i8() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (a, b, x, y) = (
        load(&mut dag, EVT::I32, 1),
        load(&mut dag, EVT::I32, 2),
        load(&mut dag, EVT::I32, 3),
        load(&mut dag, EVT::I32, 4),
    );
    let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetLt);
    let pick = dag.get_select(EVT::I32, test, x, y);
    dag.set_root(pick);
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
    assert!(!legalizer.all_legal());
    assert_eq!(legalizer.run(), Ok(true));
    assert!(legalizer.all_legal());
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Select);
    let cond = dag.operand(root, 0);
    // `zext i8 (and (anyext (setcc i8)), 1)`: the `i1` was promoted to the
    // setcc's own `i8`.
    assert_eq!(dag.value_type(cond), EVT::I8);
    let setcc = (0..dag.node_count() as u32)
        .map(|node| SDValue { node, res: 0 })
        .find(|&v| dag.opcode(v) == ISD::SetCC && dag.value_type(v) == EVT::I8);
    assert!(setcc.is_some(), "an i8 setcc exists");
}

/// A compare consumed as an `i1` and widened: `zext i32 (setcc i1)` is the
/// promoted compare masked to one bit.
#[test]
fn a_zero_extended_i1_compare_is_masked_to_one_bit() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (a, b) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2));
    let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetEq);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I32, &[test]);
    dag.set_root(wide);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::And, EVT::I32));
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(1));
}
