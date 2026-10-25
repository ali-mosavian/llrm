//! `ISD::SDIVREM` / `ISD::UDIVREM` selection ([L6214][case]): the dividend in
//! `AX`/`EAX`, its high half cleared or sign-extended into `DX`/`EDX`,
//! `DIV`/`IDIV`, then the quotient and remainder copied out of their registers.
//!
//! # How it works
//! A translation of the pin's case for `i16`, `i32` and, at 64 bits, `i64`. The
//! divisor folds from memory when `tryFoldLoad` allows. A signed divide whose
//! dividend has a zero sign bit clears the high half like an unsigned one.
//!
//! Numbered deviation: **SD302** `i8` (`AL`/`AH` through `MOVZX16rr8`) is not
//! translated: no target produces it, so the node stays unselected rather
//! than guessed. Owner sdag-c. Exit: x86-64 spike.
//! Review 2026-12-10.
//!
//! # Used by
//! [`crate::x86_select::select_node`].
//!
//! [case]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L6214

use crate::dag::SDValue;
use crate::generated::ids::{isd, mvt, target_opcode, x86};
use crate::matcher::Vt;
use crate::x86_addr::{AddrDag, known_zero};
use crate::x86_hooks::{Cpu, X86Hooks};

/// Selects `node`; false when its type is one SD302 leaves out.
pub fn select_divrem<D: AddrDag>(
    dag: &mut D,
    hooks: &X86Hooks,
    node: u32,
) -> bool {
    let is_signed = dag.opcode(node) == isd::SDIVREM;
    let nvt = dag.value_type(SDValue::new(node, 0));
    let (reg_op, mem_op, lo, hi, sext) = match (nvt, is_signed) {
        (mvt::i16, false) => (x86::DIV16r, x86::DIV16m, x86::AX, x86::DX, x86::CWD),
        (mvt::i16, true) => (x86::IDIV16r, x86::IDIV16m, x86::AX, x86::DX, x86::CWD),
        (mvt::i32, false) => (x86::DIV32r, x86::DIV32m, x86::EAX, x86::EDX, x86::CDQ),
        (mvt::i32, true) => (x86::IDIV32r, x86::IDIV32m, x86::EAX, x86::EDX, x86::CDQ),
        (mvt::i64, false) if hooks.options.cpu == Cpu::X64 => (x86::DIV64r, x86::DIV64m, x86::RAX, x86::RDX, x86::CQO),
        (mvt::i64, true) if hooks.options.cpu == Cpu::X64 => (x86::IDIV64r, x86::IDIV64m, x86::RAX, x86::RDX, x86::CQO),
        _ => return false,
    };
    let n0 = dag.operand(node, 0);
    let n1 = dag.operand(node, 1);

    let folded = hooks.try_fold_load(dag, node, node, n1);
    let sign_bit = 1u64
        << (if nvt == mvt::i16 {
            15
        } else if nvt == mvt::i64 {
            63
        } else {
            31
        });
    let sign_bit_is_zero = known_zero(dag, n0, 0) & sign_bit != 0;

    let entry = dag.entry_node();
    let copy = dag.copy_to_reg(entry, lo, n0, None);
    let mut in_glue = SDValue::new(copy, 1);
    if is_signed && !sign_bit_is_zero {
        in_glue = SDValue::new(dag.machine_node(sext, &[mvt::Glue], &[in_glue]), 0);
    } else {
        let mut clr = SDValue::new(dag.machine_node(x86::MOV32r0, &[mvt::i32, mvt::i32], &[]), 0);
        if nvt == mvt::i16 {
            let sub = dag.target_constant(i64::from(x86::sub_16bit), mvt::i32);
            clr = SDValue::new(dag.machine_node(target_opcode::EXTRACT_SUBREG, &[mvt::i16], &[clr, sub]), 0);
        } else if nvt == mvt::i64 {
            let zero = dag.target_constant(0, mvt::i64);
            let sub = dag.target_constant(i64::from(x86::sub_32bit), mvt::i32);
            clr = SDValue::new(dag.machine_node(target_opcode::SUBREG_TO_REG, &[mvt::i64], &[zero, clr, sub]), 0);
        }
        let entry = dag.entry_node();
        let copy = dag.copy_to_reg(entry, hi, clr, Some(in_glue));
        in_glue = SDValue::new(copy, 1);
    }

    if let Some(a) = folded {
        let chain = dag.operand(n1.node, 0);
        let ops = [a.base, a.scale, a.index, a.disp, a.segment, chain, in_glue];
        let div = dag.machine_node(mem_op, &[mvt::Other, mvt::Glue], &ops);
        in_glue = SDValue::new(div, 1);
        dag.replace_uses(SDValue::new(n1.node, 1), SDValue::new(div, 0));
        if let Some(mems) = dag.mem_operands(n1.node) {
            dag.set_node_mem_refs(div, &mems);
        }
    } else {
        in_glue = SDValue::new(dag.machine_node(reg_op, &[mvt::Glue], &[n1, in_glue]), 0);
    }

    copy_results(dag, node, nvt, lo, hi, in_glue);
    true
}

/// Copies the low and high results of `node` out of `lo` and `hi`, the ones
/// that are used, then deletes `node`.
pub(crate) fn copy_results<D: AddrDag>(
    dag: &mut D,
    node: u32,
    nvt: Vt,
    lo: u16,
    hi: u16,
    mut in_glue: SDValue,
) {
    for (result, reg) in [(0u8, lo), (1, hi)] {
        let value = SDValue::new(node, result);
        if dag.users(value).is_empty() {
            continue;
        }
        let entry = dag.entry_node();
        let from = dag.copy_from_reg(entry, reg, nvt, in_glue);
        in_glue = SDValue::new(from, 2);
        dag.replace_uses(value, SDValue::new(from, 0));
    }
    dag.remove_dead_node(node);
}
