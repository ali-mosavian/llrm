//! `visitCTLZ`/`visitCTTZ`/`visitCTPOP`: a count of a value with a known set
//! bit stayed the zero-safe form (a `cmov` after `bsr`), and a population
//! count of a harmless shift stayed a count of the shifted value.

use super::super::dag_combiner::{CombineLevel, DagCombiner};
use super::super::x86_lowering::X86Lowering;
use super::*;

fn combined(build: impl FnOnce(&mut SelectionDAG, SDValue) -> SDValue) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let root = build(&mut dag, x);
    dag.set_root(root);
    let mut hooks = X86Lowering { cmov: true, sse2: false, min_size: false, i16_mul_chains: false, profile: None };
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    (dag, root)
}

fn constant(
    dag: &mut SelectionDAG,
    n: u128,
) -> SDValue {
    dag.get_constant(n, EVT::I32)
}

/// `ctlz (x | 1)` and `cttz (x | 1)` kept their zero case.
#[test]
fn a_count_of_a_value_with_a_set_bit_is_the_zero_poison_form() {
    for (op, poison) in [(ISD::CtLz, ISD::CtLzZeroPoison), (ISD::CtTz, ISD::CtTzZeroPoison)] {
        let (dag, root) = combined(|dag, x| {
            let one = constant(dag, 1);
            let or = dag.get_node(ISD::Or, EVT::I32, &[x, one]);
            dag.get_node(op, EVT::I32, &[or])
        });
        assert_eq!(dag.opcode(root), poison, "{op:?}");
    }
    let (dag, root) = combined(|dag, x| dag.get_node(ISD::CtLz, EVT::I32, &[x]));
    assert_eq!(dag.opcode(root), ISD::CtLz, "an unknown value keeps its zero case");
}

/// `ctpop (srl (shl x, 8), 8)` over a value whose low byte is clear counted
/// the shifted value.
#[test]
fn a_population_count_skips_a_shift_that_moves_no_set_bit() {
    let (dag, root) = combined(|dag, x| {
        let (hi, eight) = (constant(dag, 0xFF00), constant(dag, 8));
        let masked = dag.get_node(ISD::And, EVT::I32, &[x, hi]);
        let shifted = dag.get_node(ISD::Srl, EVT::I32, &[masked, eight]);
        dag.get_node(ISD::CtPop, EVT::I32, &[shifted])
    });
    assert_eq!(dag.opcode(root), ISD::CtPop);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::And);
    let (dag, root) = combined(|dag, x| {
        let nine = constant(dag, 9);
        let shifted = dag.get_node(ISD::Srl, EVT::I32, &[x, nine]);
        dag.get_node(ISD::CtPop, EVT::I32, &[shifted])
    });
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Srl, "an active bit is lost");
}
