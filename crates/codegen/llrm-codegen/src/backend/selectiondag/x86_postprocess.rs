//! `X86DAGToDAGISel::PostprocessISelDAG`: `TEST r, r` of an `AND r1, r2` whose
//! result nothing else reads becomes `TEST r1, r2`.
//!
//! The table selects `(X86cmp x, 0)` as `TEST x, x` and an `and` as `AND`
//! ("Avoid selecting these and instead use a test+and", X86InstrArithmetic.td);
//! this step joins them, so the `and` result needs no register of its own.
//!
//! `AND*rm` + `TEST*rr` becomes `TEST*mr` with the `and`'s address, chain and
//! memory operands.
//!
//! DEVIATE: the vector `KORTEST` and `SUBREG_TO_REG` cases are not translated:
//! no vector types (SD13). Review 2026-11-22.

use llrm_dagisel::dag::{Dag, SDValue as PinValue};
use llrm_dagisel::generated::ids::mvt;

use super::dag::SelectionDAG;
use super::dagisel_adapter::DagisAdapter;
use super::isd::ISD;

/// (`TESTrr`, `ANDrr`) of the four widths.
const PAIRS: [(u32, u32); 4] = [(5025, 949), (5010, 834), (5015, 875), (5020, 916)];
/// (`TESTrr`, `ANDrm`, `TESTmr`) of the four widths.
const MEMORY: [(u32, u32, u32); 4] = [(5025, 944, 5023), (5010, 829, 5008), (5015, 870, 5013), (5020, 911, 5018)];

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
        let and_opcode = adapter.dag.nodes()[first.node as usize].opcode;
        if let Some(&(_, _, test_code)) = MEMORY.iter().find(|(test, and, _)| *test == code && ISD::Machine(*and) == and_opcode)
        {
            // (reg, base, scale, index, disp, segment, chain) -> (base, scale,
            // index, disp, segment, reg, chain); the flags are result 1.
            if !adapter.has_n_uses_of_value(PinValue { node: first.node, res: 1 }, 0) {
                continue;
            }
            let and = first.node;
            let o: Vec<PinValue> = (0..7).map(|i| adapter.operand(and, i)).collect();
            let operands = [o[1], o[2], o[3], o[4], o[5], o[0], o[6]];
            let test = adapter.machine_node(test_code as u16, &[mvt::i32, mvt::Other], &operands);
            if let Some(mems) = adapter.mem_refs.get(&and).cloned() {
                adapter.set_node_mem_refs(test, &mems);
            }
            adapter.replace_uses(PinValue { node: and, res: 2 }, PinValue { node: test, res: 1 });
            adapter.replace_uses(PinValue { node, res: 0 }, PinValue { node: test, res: 0 });
            continue;
        }
        if and_opcode != ISD::Machine(and_code) {
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
