//! A long operation with a small immediate takes the immediate from a register.
//!
//! # How it works
//! `and.l #15,%d2` is 6 bytes and 16 clocks on the 68000; `moveq #15,%d3` and
//! `and.l %d3,%d2` are 4 and 12. [`widen`] runs after [`super::short::narrow`]
//! and morphs a selected `ADD`, `SUB`, `AND`, `OR` or `CMP` of a long
//! immediate in `-128..=127` into the register form, the immediate in a
//! `MOVI32ri` that `ExpandMOVI` makes a `moveq`. The register form sets the
//! condition codes as the immediate form does. The immediates `narrow`
//! shortens (`addq`, `subq`, `tst`) are already smaller and stay.
//!
//! DEVIATE: the pin keeps the immediate form (its M68k states no immediate
//! cost). Reason: best of gcc and clang is the goal, and gcc keeps such a
//! constant in a register (`moveq #9,%d2; cmp.l %d0,%d2`). Exit: 2026-11-20.

use llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::{SDValue as Value, SelectionDAG};
use llrm_dagisel::dag::{Dag, SDValue};
use llrm_support::hash::HashSet;

use super::generated::m68k_m68000::MACHINE_OPCODES;

/// `(immediate row, register row, operand holding the immediate)`: the
/// operand counts the node's operands, not the results.
const ROWS: [(&str, &str, usize); 5] = [
    ("ADD32di", "ADD32dd", 1),
    ("SUB32di", "SUB32dd", 1),
    ("AND32di", "AND32dd", 1),
    ("OR32di", "OR32dd", 1),
    ("CMP32di", "CMP32dr", 0),
];

fn code(name: &str) -> Option<u16> {
    MACHINE_OPCODES.iter().find(|(_, one)| *one == name).map(|(code, _)| *code)
}

/// After the selector and `narrow`: each long operation with a small
/// immediate takes it from a register.
pub fn widen(dag: &mut SelectionDAG) {
    let mut adapter = DagisAdapter::new(dag);
    let mut seen = HashSet::default();
    let mut work = vec![adapter.dag.get_root().node];
    while let Some(node) = work.pop() {
        if seen.insert(node) {
            work.extend(adapter.dag.node(Value { node, res: 0 }).ops.iter().map(|op| op.node));
        }
    }
    let mut nodes: Vec<u32> = seen.into_iter().collect();
    nodes.sort_unstable();
    let Some(load) = code("MOVI32ri") else { return };
    for node in nodes {
        widen_node(&mut adapter, node, load);
    }
}

fn widen_node(
    adapter: &mut DagisAdapter<'_>,
    node: u32,
    load: u16,
) {
    let ISD::Machine(opcode) = adapter.dag.node(Value { node, res: 0 }).opcode else { return };
    let Some(&(_, register, at)) = ROWS.iter().find(|(imm, _, _)| code(imm).is_some_and(|c| u32::from(c) == opcode))
    else {
        return;
    };
    let Some(register) = code(register) else { return };
    let mut operands: Vec<_> = (0..adapter.num_operands(node)).map(|index| adapter.operand(node, index)).collect();
    let Some(&immediate) = operands.get(at) else { return };
    let Some(value) = adapter.constant(immediate) else { return };
    if !(-128..=127).contains(&value) {
        return;
    }
    let vt = adapter.value_type(immediate);
    let constant = adapter.target_constant(value, vt);
    let loaded = adapter.machine_node(load, &[vt], &[constant]);
    operands[at] = SDValue { node: loaded, res: 0 };
    let vts: Vec<_> =
        (0..adapter.num_values(node)).map(|res| adapter.value_type(SDValue { node, res: res as u8 })).collect();
    let (morphed, _) = adapter.morph_node_to(node, register, &vts, &operands);
    if morphed != node {
        adapter.replace_node(node, morphed);
    }
}
