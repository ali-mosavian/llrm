//! `ISD::SMUL_LOHI` / `ISD::UMUL_LOHI` selection ([L6085][case]): the
//! one-operand `MUL`/`IMUL`, with an operand in `EAX` and the 64-bit product
//! left in `EDX:EAX`.
//!
//! # How it works
//! A translation of the pin's case for `i32` without `MULX` (no 486 or P5 has
//! BMI2). Either operand folds from memory, the multiply being commutative; the
//! product halves that are used are copied out of `EAX` and `EDX`.
//!
//! `orderRegForMul` ([L5280][order]) swaps the operands when the second one
//! already sits in the implicit register, so no copy is needed; the live-in
//! physical register of a virtual `CopyFromReg` comes from
//! [`AddrDag::live_in_physical`].
//!
//! [order]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L5280
//!
//! # Used by
//! [`crate::x86_select::select_node`].
//!
//! [case]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L6085

use llrm_dagisel::addr::AddrDag;
use llrm_dagisel::dag::SDValue;
use llrm_dagisel::generated::ids::{isd, mvt, x86};

use crate::x86_divrem::copy_results;
use crate::x86_hooks::{Cpu, X86Hooks};

/// Whether the constant `c` has at least as many significant bits as `other`
/// can have (its width less its known leading zeros): the 486's multiply clocks
/// are 10 plus the bits of the r/m operand (Intel 240440-002, Table 10.1 note
/// 3).
fn constant_is_wider<D: AddrDag>(
    dag: &D,
    c: SDValue,
    other: SDValue,
    signed: bool,
    flat: bool,
) -> bool {
    let Some(value) = dag.constant(c) else { return true };
    let width = llrm_dagisel::addr::vt_bits(dag.value_type(other));
    magic_is_wider(value, width, signed, flat, dag.known_zero_bits(other))
}

/// [`constant_is_wider`] over the facts: the constant, the other operand's
/// width and the bits known zero in it (low bits first). The caller with the
/// DAG's own known bits is `BuildUDIV`/`BuildSDIV` (DEVIATE SD444), where the
/// dividend still has its length.
pub fn magic_is_wider(
    value: i64,
    width: u32,
    signed: bool,
    flat: bool,
    known_zero: u64,
) -> bool {
    let narrow = if width >= 64 { value } else { value & ((1i64 << width) - 1) };
    let bits = if signed {
        let shift = 64 - width.min(64);
        ((narrow << shift) >> shift).unsigned_abs().checked_ilog2().map_or(0, |n| n + 1)
    } else {
        (narrow as u64).checked_ilog2().map_or(0, |n| n + 1)
    };
    let zeros = (known_zero << (64 - width.min(64))).leading_ones();
    // A signed multiplier of unknown sign has `width - 1` magnitude bits: the
    // sign is not one the early-out counts (a 31-bit `x / 5` magic against an
    // unknown `i32` was a tie it lost: the magic stayed the r/m operand, 41
    // clocks, where the dividend is the one priced).
    // An unsigned variable of unknown length is as likely short as long (a
    // loop counter against `x / 100`'s 31-bit magic: gcc puts the magic in
    // EAX, the pin the counter, and the 486 read 41 clocks where the counter
    // costs 17), so a constant one bit short of it goes to EAX too: DEVIATE
    // SD444 widened to the unsigned case, flat targets only (m16 unmeasured).
    let sign = u32::from(zeros == 0 && (signed || flat));
    let longest = width - zeros.min(width);
    // DEVIATE SD444-HALF (owner x86-m32, exit 2026-11-24), flat targets: a variable whose
    // length is not known is priced at half its width, the mean of a log-uniform length,
    // so a constant of that many bits goes to EAX. One bit short of the full width left
    // `x % 7`'s 30-bit magic the r/m operand, 40 clocks where the variable costs 10 + its
    // own bits: recmany -O2 187010 -> 156108 clk, recsum -32%, x_rle -21%, x_loop_interchange
    // -10%. A wrong guess costs at most the constant's bits less the variable's.
    if flat {
        return bits >= longest.div_ceil(2);
    }
    bits >= longest.saturating_sub(sign)
}

/// `orderRegForMul`: swap so the operand already in `lo` is the implicit one.
fn order_reg_for_mul<D: AddrDag>(
    dag: &D,
    n0: &mut SDValue,
    n1: &mut SDValue,
    lo: u16,
) {
    let phys = |v: SDValue| {
        if dag.opcode(v.node) != isd::CopyFromReg {
            return None;
        }
        let reg = dag.operand(v.node, 1).node;
        dag.live_in_physical(reg).or_else(|| dag.register_of(reg))
    };
    if phys(*n1) == Some(lo) && phys(*n0) != Some(lo) {
        std::mem::swap(n0, n1);
    }
}

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

    if folded.is_none() {
        order_reg_for_mul(dag, &mut n0, &mut n1, lo);
    }

    // DEVIATE SD444 (owner w2, exit 2026-11-17): where the multiply ends early
    // on the r/m operand (10 + the bits of that operand), a constant with more
    // significant bits than the other operand can have (a division's
    // full-width magic) goes to `EAX`, so the variable is the r/m operand whose
    // length is read. A narrower constant stays the r/m operand, as in the pin.
    // Stays while the 486 row (`mul` 43 clocks against 19 for a short dividend;
    // +4M clocks on 7 frames/huge rows) holds; the exit is a re-measure
    // against the pin's order on the 486 rows.
    if folded.is_none()
        && hooks.options.multiply_early_out
        && dag.opcode(n1.node) == isd::Constant
        && dag.opcode(n0.node) != isd::Constant
        && constant_is_wider(dag, n1, n0, is_signed, hooks.options.address_mode != llrm_dagisel::addr::AddrMode::Real16)
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

    if folded.is_none() {
        order_reg_for_mul(dag, &mut n0, &mut n1, lo);
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

#[cfg(test)]
mod tests {
    use super::magic_is_wider;

    /// `x % 7`'s 30-bit magic against an unsigned `x` of unknown length stayed the r/m operand:
    /// `mul` read 10 + 30 clocks on the 486 where the variable is the shorter. recmany -O2
    /// 187010 clk. A short constant stays the r/m operand; m16 keeps its rule.
    #[test]
    fn a_magic_of_half_the_width_or_more_goes_to_eax_against_a_variable_of_unknown_length() {
        assert!(magic_is_wider(613_566_757, 32, false, true, 0), "30 bits against 32");
        assert!(magic_is_wider(1 << 15, 32, false, true, 0), "16 bits is half of 32");
        assert!(!magic_is_wider((1 << 15) - 1, 32, false, true, 0), "15 bits stays the r/m operand");
        assert!(!magic_is_wider(10, 32, false, true, 0), "a short constant stays the r/m operand");
        assert!(!magic_is_wider(613_566_757, 32, false, false, 0), "real mode keeps its rule");
    }
}
