//! A combine run that never ends is an error, not an arena that grows until
//! the machine is out of memory.

use super::super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::*;

/// A target combine that answers every `add` with a new `add`: two combines
/// that undo each other, as hoisting a truncate and narrowing it back did for
/// z80 (`or i8 (trunc a), (trunc b)`, 10 GB).
struct Churn(u128);

impl CombineHooks for Churn {
    fn has_target_combine(
        &self,
        op: ISD,
    ) -> bool {
        op == ISD::Add
    }

    fn perform_dag_combine(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
        _level: CombineLevel,
    ) -> Option<SDValue> {
        if dag.opcode(node) != ISD::Add {
            return None;
        }
        self.0 += 1;
        let next = dag.get_constant(self.0, EVT::I32);
        Some(dag.get_node(ISD::Add, EVT::I32, &[dag.operand(node, 0), next]))
    }
}

#[test]
fn a_combine_that_makes_a_node_per_visit_for_ever_is_stopped_with_an_error() {
    let mut dag = super::dag();
    let x = dag.get_register(EVT::I32, 5);
    let one = dag.get_constant(1, EVT::I32);
    let root = dag.get_node(ISD::Add, EVT::I32, &[x, one]);
    dag.set_root(root);
    let stopped = DagCombiner::new(&mut dag, &mut Churn(1), CombineLevel::BeforeLegalizeTypes).run_bounded();
    let runaway = stopped.expect_err("the run ended");
    assert_eq!(runaway.opcode, Some(ISD::Add), "{runaway}");
    assert!(runaway.nodes > 1 << 17, "{runaway}");
}
