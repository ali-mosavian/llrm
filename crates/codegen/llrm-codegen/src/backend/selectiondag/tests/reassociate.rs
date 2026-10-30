//! `reassociateOps` for the other commutative operations: `(op (op x, c), y)`
//! left the constant in the inner node, so no `lea`/immediate form met it and
//! `(op (op x, c1), c2)` of `xor` and `mul` stayed two operations.

use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

const OPS: [ISD; 4] = [ISD::And, ISD::Or, ISD::Xor, ISD::Mul];

fn combined(build: impl FnOnce(&mut SelectionDAG, SDValue, SDValue) -> SDValue) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let root = build(&mut dag, x, y);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    (dag, root)
}

/// `(op (op x, 12), y)` stayed that: the constant sat in the inner node
/// instead of the outer one the matcher folds.
#[test]
fn a_constant_moves_to_the_outer_node() {
    for op in OPS {
        let (dag, root) = combined(|dag, x, y| {
            let c = dag.get_constant(12, EVT::I32);
            let inner = dag.get_node(op, EVT::I32, &[x, c]);
            dag.get_node(op, EVT::I32, &[inner, y])
        });
        assert_eq!(dag.opcode(root), op, "{op:?}");
        assert_eq!(dag.constant(dag.operand(root, 1)), Some(12), "{op:?}: {}", dag.dump());
        let inner = dag.operand(root, 0);
        assert_eq!(dag.opcode(inner), op, "{op:?}");
        assert!(dag.constant(dag.operand(inner, 1)).is_none(), "{op:?}: {}", dag.dump());
    }
}

/// `(xor (xor x, 3), 5)` is `x ^ 6`; `(mul (mul x, 3), 5)` is `x * 15`.
#[test]
fn two_constants_are_one() {
    for (op, a, b, both) in
        [(ISD::Xor, 3, 5, 6), (ISD::Mul, 3, 5, 15), (ISD::And, 0xF0, 0x3C, 0x30), (ISD::Or, 1, 4, 5)]
    {
        let (dag, root) = combined(|dag, x, _| {
            let (a, b) = (dag.get_constant(a, EVT::I32), dag.get_constant(b, EVT::I32));
            let inner = dag.get_node(op, EVT::I32, &[x, a]);
            dag.get_node(op, EVT::I32, &[inner, b])
        });
        assert_eq!(dag.opcode(root), op, "{op:?}");
        assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Register, "{op:?}: {}", dag.dump());
        assert_eq!(dag.constant(dag.operand(root, 1)), Some(both), "{op:?}");
    }
}

/// `x+1` stored and `(x+1)+1` stored kept the chain (matrix at m16: the
/// matcher folded the nested constants into `[bx+si+5]` with a register
/// holding 1, +224 bytes): an `add` of constants is `x + 2` even when a store
/// reads it (SD210 refused any `add` with a memory user).
#[test]
fn a_chain_of_stored_increments_is_x_plus_k() {
    use crate::backend::selectiondag::{MemBase, MemInfo};
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let ptr = dag.get_register(EVT::I32, 7);
    let one = dag.get_constant(1, EVT::I32);
    let a = dag.get_node(ISD::Add, EVT::I32, &[x, one]);
    let b = dag.get_node(ISD::Add, EVT::I32, &[a, one]);
    let info = MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Stack(0), ext: None, space: 0, ordering: None };
    let first = dag.get_store(dag.entry_node(), a, ptr, info.clone());
    let second = dag.get_store(first, b, ptr, info);
    dag.set_root(second);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let value = dag.operand(root, 1);
    assert_eq!((dag.opcode(value), dag.operand(value, 0), dag.constant(dag.operand(value, 1))), (ISD::Add, x, Some(2)));
}
