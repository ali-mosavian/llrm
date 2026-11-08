//! `X86DAGToDAGISel::PostprocessISelDAG`: `TEST r, r` of an `AND r1, r2` whose
//! result nothing else reads becomes `TEST r1, r2`.
//!
//! The table selects `(X86cmp x, 0)` as `TEST x, x` and an `and` as `AND`
//! ("Avoid selecting these and instead use a test+and", X86InstrArithmetic.td);
//! this step joins them, so the `and` result needs no register of its own.
//!
//! DEVIATE: the memory forms (`AND*rm` + `TEST*rr` -> `TEST*mr`) are not
//! translated; the generated tables have no `TEST*mr` row. Exit: the table
//! rows. Owner w9. The vector `KORTEST` and `SUBREG_TO_REG` cases are not
//! translated: no vector types (SD13). Review 2026-11-22.

use llrm_dagisel::dag::{Dag, SDValue as PinValue};
use llrm_dagisel::generated::ids::mvt;

use super::dag::SelectionDAG;
use super::dagisel_adapter::DagisAdapter;
use super::isd::ISD;

/// (`TESTrr`, `ANDrr`) of the four widths.
const PAIRS: [(u32, u32); 4] = [(5025, 949), (5010, 834), (5015, 875), (5020, 916)];

pub fn postprocess(dag: &mut SelectionDAG) {
    let mut adapter = DagisAdapter::new(dag);
    for node in (0..adapter.dag.nodes().len() as u32).rev() {
        let ISD::Machine(code) = adapter.dag.nodes()[node as usize].opcode else { continue };
        let Some(&(_, and_code)) = PAIRS.iter().find(|(test, _)| *test == code) else { continue };
        if adapter.is_dead(node) || !adapter.has_live_users(node) {
            continue;
        }
        let (first, second) = (adapter.operand(node, 0), adapter.operand(node, 1));
        if first != second || !adapter.has_n_uses_of_value(first, 2) {
            continue;
        }
        if adapter.dag.nodes()[first.node as usize].opcode != ISD::Machine(and_code) {
            continue;
        }
        // The `and`'s flags are read by nothing: only the test's matter.
        if !adapter.has_n_uses_of_value(PinValue { node: first.node, res: 1 }, 0) {
            continue;
        }
        let operands = [adapter.operand(first.node, 0), adapter.operand(first.node, 1)];
        let test = adapter.machine_node(code as u16, &[mvt::i32], &operands);
        adapter.replace_uses(PinValue { node, res: 0 }, PinValue { node: test, res: 0 });
    }
}
