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
use crate::x86_hooks::{Cpu, X86Hooks};

/// Selects `node`; false when its type is one the pin's case has no row for:
/// `i32`, and `i64` at 64 bits.
pub fn select_mul_lohi<D: AddrDag>(
    dag: &mut D,
    hooks: &X86Hooks,
    node: u32,
) -> bool {
    let nvt = dag.value_type(SDValue::new(node, 0));
    let wide = nvt == mvt::i64 && hooks.options.cpu == Cpu::X64;
    if nvt != mvt::i32 && !wide {
        return false;
    }
    let is_signed = dag.opcode(node) == isd::SMUL_LOHI;
    let (reg_op, mem_op) = match (is_signed, wide) {
        (true, false) => (x86::IMUL32r, x86::IMUL32m),
        (false, false) => (x86::MUL32r, x86::MUL32m),
        (true, true) => (x86::IMUL64r, x86::IMUL64m),
        (false, true) => (x86::MUL64r, x86::MUL64m),
    };
    let (lo, hi) = if wide { (x86::RAX, x86::RDX) } else { (x86::EAX, x86::EDX) };
    let (mut n0, mut n1) = (dag.operand(node, 0), dag.operand(node, 1));

    let mut folded = hooks.try_fold_load(dag, node, node, n1);
    if folded.is_none() {
        folded = hooks.try_fold_load(dag, node, node, n0);
        if folded.is_some() {
            std::mem::swap(&mut n0, &mut n1);
        }
    }

    // DEVIATE SD444: where the multiply ends early on the r/m operand, a
    // constant (a division's full-width magic) goes to `EAX`, so the other
    // operand is the one whose length is read. The pin keeps operand 0 in
    // `EAX`.
    if folded.is_none()
        && hooks.options.multiply_early_out
        && dag.opcode(n1.node) == isd::Constant
        && dag.opcode(n0.node) != isd::Constant
    {
        std::mem::swap(&mut n0, &mut n1);
    }

    let entry = dag.entry_node();
    let copy = dag.copy_to_reg(entry, lo, n0, None);
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
    copy_results(dag, node, nvt, lo, hi, in_glue);
    true
}

/// `X86ISD::UMUL`, and `X86ISD::SMUL` at `i8` ([L5998][case]): the
/// one-operand `MUL`/`IMUL` with an operand in `AL`/`AX`/`EAX`; the value is
/// the low half and the flags are the multiply's `EFLAGS`. The other widths of
/// `SMUL` are patterns. False for a type the case has no row for.
///
/// [case]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L5998
pub fn select_mul_flags<D: AddrDag>(
    dag: &mut D,
    hooks: &X86Hooks,
    node: u32,
    signed: bool,
) -> bool {
    let nvt = dag.value_type(SDValue::new(node, 0));
    let (lo, reg_op, mem_op) = match (nvt, signed) {
        (mvt::i8, true) => (x86::AL, x86::IMUL8r, x86::IMUL8m),
        (mvt::i8, false) => (x86::AL, x86::MUL8r, x86::MUL8m),
        (mvt::i16, false) => (x86::AX, x86::MUL16r, x86::MUL16m),
        (mvt::i32, false) => (x86::EAX, x86::MUL32r, x86::MUL32m),
        (mvt::i64, false) if hooks.options.cpu == Cpu::X64 => (x86::RAX, x86::MUL64r, x86::MUL64m),
        _ => return false,
    };
    let (mut n0, mut n1) = (dag.operand(node, 0), dag.operand(node, 1));

    let mut folded = hooks.try_fold_load(dag, node, node, n1);
    if folded.is_none() {
        folded = hooks.try_fold_load(dag, node, node, n0);
        if folded.is_some() {
            std::mem::swap(&mut n0, &mut n1);
        }
    }

    let entry = dag.entry_node();
    let copy = dag.copy_to_reg(entry, lo, n0, None);
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
    for (result, reg, vt) in [(0u8, lo, nvt), (1, x86::EFLAGS, mvt::i32)] {
        let value = SDValue::new(node, result);
        if dag.users(value).is_empty() {
            continue;
        }
        let entry = dag.entry_node();
        let from = dag.copy_from_reg(entry, reg, vt, in_glue);
        in_glue = SDValue::new(from, 2);
        dag.replace_uses(value, SDValue::new(from, 0));
    }
    dag.remove_dead_node(node);
    true
}
