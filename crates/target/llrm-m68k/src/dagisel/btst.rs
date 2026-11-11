//! `btst #k,Dn`: the immediate form of the bit test.
//!
//! DEVIATE M68K-BTST-IMM (owner m68k-lsr-addressing, exit none): the pin's
//! `BTST32di` pattern (`MxBITPatR`, `M68kInstrBits.td`) emits no
//! `EmitConvertToTarget`, so its constant bit number reaches the printer as a
//! register and `llc` stops with "Unknown immediate kind" (seen on
//! `(x >> 5) & 1 != 0`). A bit number built as a `TargetConstant` matches the
//! register pattern `BTST32dd` untouched; [`immediate_bit_numbers`] morphs
//! that node into `BTST32di`, as `X86DAGToDAGISel::PostprocessISelDAG` rewrites
//! selected nodes.

use llrm_codegen::backend::selectiondag::SDValue as Value;
use llrm_codegen::backend::selectiondag::SelectionDAG;
use llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_dagisel::dag::{Dag, SDValue};

use super::generated::ids::opcode::{BTST32dd, BTST32di};
use super::short::reachable_nodes;

/// After the selector: each `BTST32dd` whose bit number is a `TargetConstant`
/// becomes `BTST32di`, in place.
pub fn immediate_bit_numbers(dag: &mut SelectionDAG) {
    let nodes = reachable_nodes(dag);
    let mut adapter = DagisAdapter::new(dag);
    for node in nodes {
        let data = adapter.dag.node(Value { node, res: 0 });
        if data.opcode != ISD::Machine(u32::from(BTST32dd)) {
            continue;
        }
        if adapter.dag.node(data.ops[1]).opcode != ISD::TargetConstant {
            continue;
        }
        let operands: Vec<_> = (0..adapter.num_operands(node)).map(|index| adapter.operand(node, index)).collect();
        let vts: Vec<_> =
            (0..adapter.num_values(node)).map(|res| adapter.value_type(SDValue { node, res: res as u8 })).collect();
        let (morphed, _) = adapter.morph_node_to(node, BTST32di, &vts, &operands);
        if morphed != node {
            adapter.replace_node(node, morphed);
        }
    }
}
