//! `foldSubToUSubSat`
//! ([L4100](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4100)):
//! `umax(a, b) - b`, `a - umin(a, b)` and the truncated
//! `sub(a, trunc(umin(zext(a), b)))` stayed a max (or min) and a subtract: on
//! arm64 `text_write`'s clamped difference was a compare, a select and a
//! subtract twice over (`SUBS CSEL SUBS SUBS CSEL SUBS`) where the pin has
//! `SUBS CSEL`.

use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

fn combined(build: impl FnOnce(&mut SelectionDAG, SDValue, SDValue) -> SDValue) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let (a, b) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let root = build(&mut dag, a, b);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    (dag, root)
}

#[test]
fn a_max_minus_one_of_its_operands_is_a_saturating_subtract() {
    for swapped in [false, true] {
        let (dag, root) = combined(|dag, a, b| {
            let max = if swapped {
                dag.get_node(ISD::UMax, EVT::I32, &[b, a])
            } else {
                dag.get_node(ISD::UMax, EVT::I32, &[a, b])
            };
            dag.get_node(ISD::Sub, EVT::I32, &[max, b])
        });
        assert_eq!(dag.opcode(root), ISD::USubSat, "swapped {swapped}");
        assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Register);
        assert_eq!(dag.opcode(dag.operand(root, 1)), ISD::Register);
    }
}

#[test]
fn an_operand_minus_the_min_with_it_is_a_saturating_subtract() {
    for swapped in [false, true] {
        let (dag, root) = combined(|dag, a, b| {
            let min = if swapped {
                dag.get_node(ISD::UMin, EVT::I32, &[b, a])
            } else {
                dag.get_node(ISD::UMin, EVT::I32, &[a, b])
            };
            dag.get_node(ISD::Sub, EVT::I32, &[a, min])
        });
        assert_eq!(dag.opcode(root), ISD::USubSat, "swapped {swapped}");
    }
}

/// `sub i16 a, trunc(umin(zext a, b))` is `usubsat(a, trunc(umin(b, 0xFFFF)))`.
#[test]
fn a_difference_with_the_clamped_wide_operand_is_a_truncated_saturating_subtract() {
    let mut dag = dag();
    let a = dag.get_register(EVT::I16, 5);
    let b = dag.get_register(EVT::I32, 6);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I32, &[a]);
    let min = dag.get_node(ISD::UMin, EVT::I32, &[wide, b]);
    let narrow = dag.get_node(ISD::Truncate, EVT::I16, &[min]);
    let root = dag.get_node(ISD::Sub, EVT::I16, &[a, narrow]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::USubSat);
    assert_eq!(dag.value_type(root), EVT::I16);
    let clamp = dag.operand(root, 1);
    assert_eq!(dag.opcode(clamp), ISD::Truncate);
    assert_eq!(dag.opcode(dag.operand(clamp, 0)), ISD::UMin);
}

/// `umax(a, b) - c` is not a saturating subtract.
#[test]
fn a_difference_with_another_value_stays() {
    let (dag, root) = combined(|dag, a, b| {
        let c = dag.get_register(EVT::I32, 7);
        let max = dag.get_node(ISD::UMax, EVT::I32, &[a, b]);
        dag.get_node(ISD::Sub, EVT::I32, &[max, c])
    });
    assert_eq!(dag.opcode(root), ISD::Sub);
}

/// A max with another use stays: the subtract would not remove it.
#[test]
fn a_max_with_another_use_stays() {
    let (dag, root) = combined(|dag, a, b| {
        let max = dag.get_node(ISD::UMax, EVT::I32, &[a, b]);
        let diff = dag.get_node(ISD::Sub, EVT::I32, &[max, b]);
        dag.get_node(ISD::Add, EVT::I32, &[diff, max])
    });
    assert_eq!(dag.opcode(root), ISD::Add);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Sub);
}

/// `trunc (umax(a, b) - b)` of zero-extended 16-bit values: the `sub` is a
/// saturating subtract before the truncate reads it (`visitTRUNCATE`, L17910,
/// asks too; the `sub` is visited first).
#[test]
fn a_truncated_max_difference_of_wide_values_is_a_truncated_saturating_subtract() {
    let mut dag = dag();
    let (a16, b16) = (dag.get_register(EVT::I16, 5), dag.get_register(EVT::I16, 6));
    let a = dag.get_node(ISD::ZeroExtend, EVT::I32, &[a16]);
    let b = dag.get_node(ISD::ZeroExtend, EVT::I32, &[b16]);
    let max = dag.get_node(ISD::UMax, EVT::I32, &[a, b]);
    let diff = dag.get_node(ISD::Sub, EVT::I32, &[max, b]);
    let root = dag.get_node(ISD::Truncate, EVT::I16, &[diff]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let inner = if dag.opcode(root) == ISD::Truncate { dag.operand(root, 0) } else { root };
    assert_eq!(dag.opcode(inner), ISD::USubSat);
}
