//! `ISD::SMUL_LOHI` / `ISD::UMUL_LOHI` selection ([L6085][case]): the
//! one-operand `MUL`/`IMUL`, with an operand in `EAX` and the 64-bit product
//! left in `EDX:EAX`.
//!
//! # How it works
//! A translation of the pin's case for `i32` without `MULX` (no 486 or P5 has
//! BMI2). Either operand folds from memory, the multiply being commutative; the
//! product halves that are used are copied out of `EAX` and `EDX`.
//!
//! Numbered deviation: **SD303** `orderRegForMul` (swap so the operand already
//! in `EAX` is the implicit one) is skipped: it reads the live-in physical
//! register of a virtual `CopyFromReg`, which the DAG does not carry. At worst
//! one extra copy. Owner sdag-c. Exit: live-in map in the adapter. Review
//! 2026-12-10.
//!
//! # Used by
//! [`crate::x86_select::select_node`].
//!
//! [case]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L6085

use crate::dag::SDValue;
use crate::generated::ids::{isd, mvt, x86};
use crate::x86_addr::AddrDag;
use crate::x86_divrem::copy_results;
use crate::x86_hooks::X86Hooks;

/// Selects `node`; false when its type is one the pin's case has no row for at
/// 32 bits.
pub fn select_mul_lohi<D: AddrDag>(
    dag: &mut D,
    hooks: &X86Hooks,
    node: u32,
) -> bool {
    let nvt = dag.value_type(SDValue::new(node, 0));
    if nvt != mvt::i32 {
        return false;
    }
    let is_signed = dag.opcode(node) == isd::SMUL_LOHI;
    let (reg_op, mem_op) = if is_signed { (x86::IMUL32r, x86::IMUL32m) } else { (x86::MUL32r, x86::MUL32m) };
    let (mut n0, mut n1) = (dag.operand(node, 0), dag.operand(node, 1));

    let mut folded = hooks.try_fold_load(dag, node, node, n1);
    if folded.is_none() {
        folded = hooks.try_fold_load(dag, node, node, n0);
        if folded.is_some() {
            std::mem::swap(&mut n0, &mut n1);
        }
    }

    let entry = dag.entry_node();
    let copy = dag.copy_to_reg(entry, x86::EAX, n0, None);
    let mut in_glue = SDValue::new(copy, 1);

    if let Some(a) = folded {
        let chain = dag.operand(n1.node, 0);
        let ops = [a.base, a.scale, a.index, a.disp, a.segment, chain, in_glue];
        let mul = dag.machine_node(mem_op, &[mvt::Other, mvt::Glue], &ops);
        in_glue = SDValue::new(mul, 1);
        dag.replace_uses(SDValue::new(n1.node, 1), SDValue::new(mul, 0));
        if let Some(mems) = dag.mem_operands(n1.node) {
            dag.set_node_mem_refs(mul, &mems);
        }
    } else {
        in_glue = SDValue::new(dag.machine_node(reg_op, &[mvt::Glue], &[n1, in_glue]), 0);
    }
    copy_results(dag, node, nvt, x86::EAX, x86::EDX, in_glue);
    true
}
