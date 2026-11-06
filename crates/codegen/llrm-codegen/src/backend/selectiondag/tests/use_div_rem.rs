//! `DAGCombiner::useDivRem`: a divide and a remainder of the same operands are
//! one `DIVREM` where the target has it custom (a type it holds in no
//! register) or a library call for it. A 64-bit divide and remainder on a
//! 32-bit target were two library calls.

use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;
use crate::backend::selectiondag::actions::Action;

fn combined(
    divrem_named: bool,
    divrem_custom: bool,
    ops: [ISD; 2],
) -> SelectionDAG {
    let mut tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    for divrem in [ISD::UDivRem, ISD::SDivRem] {
        if divrem_custom {
            tli.set_operation_action(divrem, MVT::I64, Action::Custom);
        }
        if divrem_named {
            tli.set_libcall_name(divrem, MVT::I64, "__divmoddi4");
        }
    }
    let mut dag = SelectionDAG::new(DagConfig::default());
    let (x, y) = (dag.get_register(EVT::I64, 5), dag.get_register(EVT::I64, 6));
    let quotient = dag.get_node(ops[0], EVT::I64, &[x, y]);
    let remainder = dag.get_node(ops[1], EVT::I64, &[x, y]);
    let both = dag.get_node(ISD::Add, EVT::I64, &[quotient, remainder]);
    dag.set_root(both);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    dag
}

fn live(
    dag: &SelectionDAG,
    op: ISD,
) -> usize {
    dag.node_ids().filter(|&id| dag.opcode(SDValue { node: id, res: 0 }) == op && dag.has_users(SDValue { node: id, res: 0 })).count()
}

/// A custom `DIVREM` of a type the target holds in no register, with a routine
/// named for it, takes the pair: one two-result node, no divide, no remainder.
#[test]
fn a_divide_and_a_remainder_of_a_long_long_are_one_divrem_where_the_target_has_the_call() {
    for (ops, divrem) in [([ISD::UDiv, ISD::URem], ISD::UDivRem), ([ISD::SDiv, ISD::SRem], ISD::SDivRem)] {
        let dag = combined(true, true, ops);
        assert_eq!(live(&dag, divrem), 1, "{ops:?}");
        assert_eq!(live(&dag, ops[0]) + live(&dag, ops[1]), 0, "{ops:?}");
    }
}

/// A remainder first, a divide second: the same node.
#[test]
fn the_order_of_the_pair_does_not_matter() {
    let dag = combined(true, true, [ISD::URem, ISD::UDiv]);
    assert_eq!(live(&dag, ISD::UDivRem), 1);
    assert_eq!(live(&dag, ISD::UDiv) + live(&dag, ISD::URem), 0);
}

/// Without the routine, or without the custom action on the illegal type,
/// the pair stays two operations (every target but one that asks for it).
#[test]
fn a_target_with_neither_keeps_the_pair() {
    for (named, custom) in [(false, true), (true, false), (false, false)] {
        let dag = combined(named, custom, [ISD::UDiv, ISD::URem]);
        assert_eq!(live(&dag, ISD::UDivRem), 0, "named {named} custom {custom}");
        assert_eq!(live(&dag, ISD::UDiv) + live(&dag, ISD::URem), 2, "named {named} custom {custom}");
    }
}
