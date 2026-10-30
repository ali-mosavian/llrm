use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::{X86Lowering, cond, x86isd};
use super::*;

fn lowered(
    op: ISD,
    existing: Option<CondCode>,
) -> (SelectionDAG, SDValue, [SDValue; 2]) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    // A user of an existing compare keeps it in the DAG (and reachable).
    let other = existing.map(|cc| dag.get_setcc(EVT::I8, x, y, cc));
    let best = dag.get_node(op, EVT::I32, &[x, y]);
    let root = match other {
        Some(test) => {
            let wide = dag.get_node(ISD::ZeroExtend, EVT::I32, &[test]);
            dag.get_node(ISD::Add, EVT::I32, &[best, wide])
        }
        None => best,
    };
    dag.set_root(root);
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false }).run().unwrap();
    let root = dag.get_root();
    (dag, root, [x, y])
}

/// `max(x, y)` (signed or not) stopped the legalizer: "SMax: convert node to
/// libcall" (3 functions of the m32 shadow). It is a compare and a select:
/// `x > y ? x : y`.
#[test]
fn a_min_or_max_is_a_compare_and_a_conditional_move() {
    for (op, code) in [(ISD::SMax, cond::G), (ISD::SMin, cond::L), (ISD::UMax, cond::A), (ISD::UMin, cond::B)] {
        let (dag, root, [x, y]) = lowered(op, None);
        assert_eq!(dag.opcode(root), x86isd::CMOV, "{op:?}");
        assert_eq!((dag.operand(root, 0), dag.operand(root, 1)), (y, x), "{op:?}: x when the compare holds");
        assert_eq!(dag.constant(dag.operand(root, 2)), Some(code), "{op:?}");
    }
}

/// With `y < x` already in the DAG the select reuses it with its arms
/// swapped: `(y < x) ? x : y`... as the pin's `buildMinMax` does.
#[test]
fn an_existing_swapped_compare_is_reused() {
    let (dag, root, [x, y]) = lowered(ISD::SMax, Some(CondCode::SetLt));
    let cmov = dag.operand(root, 0);
    assert_eq!(dag.opcode(cmov), x86isd::CMOV);
    // `x < y ? y : x`: the arms are y when the compare (L) holds.
    assert_eq!((dag.operand(cmov, 0), dag.operand(cmov, 1)), (x, y));
    assert_eq!(dag.constant(dag.operand(cmov, 2)), Some(cond::L));
}
