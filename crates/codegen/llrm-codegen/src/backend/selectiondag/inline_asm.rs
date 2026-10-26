//! `X86ISD::LLRM_ASM`: a Nib/QB `llrm.ia16.asm.*` block as one DAG node.
//!
//! # How it works
//! The block's name is the encoding: [`llrm_mir::intrinsics::asm`] parses its
//! bytes, input and output registers, clobbers, memory flag and fixups, so the
//! node carries the name and nothing is stated twice. Registers cross as
//! glued `CopyToReg` before the node and `CopyFromReg` after it, as for a
//! call; the clobbers and the memory flag come from the name.
//!
//! Operands, in order:
//! 1. the chain;
//! 2. `TargetExternalSymbol(name)`;
//! 3. one `TargetGlobalAddress` per fixup, in `fixups` order;
//! 4. one `Register` per input, in `inputs` order;
//! 5. the glue of the last input copy, when there is an input.
//!
//! Results: the chain and the glue.
//!
//! # Example
//! One input in `ax`, one output in `dx`:
//!
//! ```text
//! ch,glue = CopyToReg t0, $ax, a
//! ch,glue = X86ISD::LLRM_ASM ch, TargetExternalSymbol'llrm.ia16.asm.…', Register $ax, glue
//! x, ch, glue = CopyFromReg ch, $dx, glue
//! ```
//!
//! # Runs
//! Built by the builder for a call of an `llrm.ia16.asm.*` intrinsic; read by
//! the emitter (sdag-d), which writes the bytes and the fixups.
//!
//! # See also
//! LLVM's `SelectionDAGBuilder::visitInlineAsm`, the nearest analogue
//! ([SelectionDAGBuilder.cpp:9631](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L9631)).

use llrm_lir::registers::RegId;

use super::dag::{SDValue, SelectionDAG};
use super::mvt::EVT;
use super::x86isd;

/// The nodes of the block: the chain after it, and the value read from each
/// output register in order. `fixups` are the symbols' addresses, `inputs`
/// the registers and their values, `outputs` the registers and their types.
pub fn lower_asm(
    dag: &mut SelectionDAG,
    root: SDValue,
    name: &str,
    fixups: &[SDValue],
    inputs: &[(RegId, SDValue)],
    outputs: &[(RegId, EVT)],
) -> (SDValue, Vec<SDValue>) {
    let mut chain = root;
    let mut glue: Option<SDValue> = None;
    for &(reg, value) in inputs {
        let copy = dag.get_copy_to_phys_reg(chain, reg, value, glue);
        chain = SDValue { node: copy.node, res: 0 };
        glue = Some(SDValue { node: copy.node, res: 1 });
    }
    let mut ops = vec![chain, dag.get_target_external_symbol(EVT::I32, name)];
    ops.extend_from_slice(fixups);
    for &(reg, value) in inputs {
        let vt = dag.value_type(value);
        ops.push(dag.get_phys_register(vt, reg));
    }
    ops.extend(glue);
    let node = dag.get_node_multi(x86isd::LLRM_ASM, &[EVT::OTHER, EVT::GLUE], &ops);
    let mut chain = SDValue { node: node.node, res: 0 };
    let mut glue = SDValue { node: node.node, res: 1 };
    let mut results = Vec::new();
    for &(reg, vt) in outputs {
        let copy = dag.get_copy_from_phys_reg(chain, reg, vt, glue);
        chain = SDValue { node: copy.node, res: 1 };
        glue = SDValue { node: copy.node, res: 2 };
        results.push(copy);
    }
    (chain, results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::selectiondag::{DagConfig, ISD};

    /// The node's operand order is the emitter's contract: chain, symbol,
    /// fixups, input registers, glue. A reorder would hand the emitter a
    /// register where it reads a symbol.
    #[test]
    fn operands_are_chain_symbol_fixups_registers_glue() {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let (ax, dx) = (RegId::from_raw(1), RegId::from_raw(2));
        let value = dag.get_constant(7, EVT::I32);
        let fixup = dag.get_target_global_address(EVT::I32, 0, 0);
        let root = dag.entry_node();
        let (chain, results) =
            lower_asm(&mut dag, root, "llrm.ia16.asm.90", &[fixup], &[(ax, value)], &[(dx, EVT::I32)]);
        assert_eq!(results.len(), 1);
        let read = dag.operand(results[0], 2);
        let node = SDValue { node: read.node, res: 0 };
        assert_eq!(dag.opcode(node), x86isd::LLRM_ASM);
        let ops: Vec<ISD> = (0..5).map(|at| dag.opcode(dag.operand(node, at))).collect();
        assert_eq!(
            ops,
            [ISD::CopyToReg, ISD::TargetExternalSymbolNode, ISD::TargetGlobalAddress, ISD::Register, ISD::CopyToReg]
        );
        assert_eq!(chain.res, 1);
    }
}
