//! `X86ISD::CMP` of an `and` with zero ([L6508][case]): the compare becomes the
//! narrowest `TEST` that holds the mask, from memory when the `and` reads a
//! load.
//!
//! # How it works
//! `(X86cmp (and $op, $imm), 0)` is `test $op, $imm`; when the mask fits in a
//! byte (a word at minsize, a dword when the operand is wider), the test reads
//! that many bytes only. It does not depend on which way the branch tests the
//! flag. A mask bit that is the sign of the narrower width changes `SF`, so it
//! needs a user that does not read the sign.
//!
//! A 64-bit compare against a mask that is one run of ones, where only the
//! zero flag is read, becomes a shift and a `TEST` of the shifted value (or of
//! its 8, 16 or 32 low bits), which drops a `movabs` or a 32-bit immediate.
//!
//! # Used by
//! [`crate::x86_select::select_node`].
//!
//! [case]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L6508

use crate::addr::{AddrDag, vt_bits};
use crate::dag::SDValue;
use crate::generated::ids::{isd, mvt, target_opcode, x86};
use crate::x86_hooks::X86Hooks;

/// Selects `node`; false when the pin's case leaves it to the table.
pub fn select_cmp_of_and<D: AddrDag>(
    dag: &mut D,
    hooks: &X86Hooks,
    node: u32,
) -> bool {
    let (mut n0, n1) = (dag.operand(node, 0), dag.operand(node, 1));
    if dag.constant(n1) != Some(0) {
        return false;
    }
    let cmp_vt = dag.value_type(n0);
    let cmp_bits = match cmp_vt {
        mvt::i8 => 8,
        mvt::i16 => 16,
        mvt::i32 => 32,
        mvt::i64 => 64,
        _ => return false,
    };
    // Look past a truncate the compare alone reads.
    if dag.opcode(n0.node) == isd::TRUNCATE && dag.node_has_one_use(n0.node) {
        n0 = dag.operand(n0.node, 0);
    }
    let n0_vt = dag.value_type(n0);
    if dag.opcode(n0.node) != isd::AND || !dag.node_has_one_use(n0.node) || n0_vt == mvt::i8 {
        return false;
    }
    let Some(mask) = dag.constant(dag.operand(n0.node, 1)) else {
        return false;
    };
    // The truncate may have hidden bits the compare does not see.
    let mask = mask as u64 & (u64::MAX >> (64 - cmp_bits));

    let flags = SDValue::new(node, 0);
    if (cmp_vt == mvt::i64 || cmp_vt == mvt::i32)
        && !(-128..=127).contains(&(mask as i64))
        && is_shifted_mask_64(mask)
        && hooks.only_uses_zero_flag(dag, flags)
        && select_shifted_mask(dag, node, n0.node, mask, cmp_vt)
    {
        return true;
    }
    let no_sign = |dag: &D| hooks.has_no_sign_flag_uses(dag, flags);
    let (vt, sub_reg, reg_opcode, mem_opcode) = if mask <= 0xFF
        && (mask & 0x80 == 0 || cmp_vt == mvt::i8 || no_sign(dag))
    {
        (mvt::i8, x86::sub_8bit, x86::TEST8ri, x86::TEST8mi)
    } else if hooks.options.min_size && mask <= 0xFFFF && (mask & 0x8000 == 0 || cmp_vt == mvt::i16 || no_sign(dag)) {
        // A word test saves one byte and may stall the decoders; only minsize
        // wants it.
        (mvt::i16, x86::sub_16bit, x86::TEST16ri, x86::TEST16mi)
    } else if mask <= u64::from(u32::MAX)
        && n0_vt != mvt::i16
        && ((mask & 0x8000_0000 == 0 && (cmp_vt != mvt::i16 || mask & 0x8000 == 0))
            || cmp_vt == mvt::i32
            || no_sign(dag))
    {
        (mvt::i32, x86::sub_32bit, x86::TEST32ri, x86::TEST32mi)
    } else {
        return false;
    };

    let imm = dag.target_constant(mask as i64, vt);
    let mut reg = dag.operand(n0.node, 0);
    let new = match hooks.try_fold_load(dag, node, n0.node, reg) {
        Some(a) => {
            // A volatile or atomic load keeps its width.
            if !dag.is_simple(reg.node) && dag.memory_store_size(reg.node) * 8 != u64::from(vt_bits(vt)) {
                return false;
            }
            let chain = dag.operand(reg.node, 0);
            let ops = [a.base, a.scale, a.index, a.disp, a.segment, imm, chain];
            let new = dag.machine_node(mem_opcode, &[mvt::i32, mvt::Other], &ops);
            dag.replace_uses(SDValue::new(reg.node, 1), SDValue::new(new, 1));
            if let Some(mems) = dag.mem_operands(reg.node) {
                dag.set_node_mem_refs(new, &mems);
            }
            new
        }
        None => {
            if n0_vt != vt {
                let index = dag.target_constant(i64::from(sub_reg), mvt::i32);
                reg = SDValue::new(dag.machine_node(target_opcode::EXTRACT_SUBREG, &[vt], &[reg, index]), 0);
            }
            dag.machine_node(reg_opcode, &[mvt::i32], &[reg, imm])
        }
    };
    dag.replace_uses(flags, SDValue::new(new, 0));
    dag.remove_dead_node(node);
    true
}

/// `APInt::isShiftedMask`: one run of ones.
fn is_shifted_mask_64(mask: u64) -> bool {
    let filled = mask.wrapping_sub(1) | mask;
    mask != 0 && filled.wrapping_add(1) & filled == 0
}

/// The `CmpVT == MVT::i64` block of the pin's `X86ISD::CMP` case: `and` is
/// `(and x, mask)` with `mask` a shifted mask that is not an `imm8`.
///
/// DEVIATE TS1 (owner greedy-subreg, exit 2026-11-14): the same two arms for an
/// `i32` compare. The pin stops at `i64` because only there does the mask need
/// a `movabs`; at `i32` a `test r32, imm32` is 6 bytes where the `shl`/`shr`/
/// `add` is 2 or 3, and the pin's own `SavesBytes` rule (one use of `x`) is the
/// guard. After `foldICmpShlConstant` (#7837) `(x << k) == 0` reaches the
/// selector as `(x & mask) == 0` and costs 3 bytes per site against the old
/// `shl` (f32 trunc/cmp, f64 add_sub at m32/m64).
fn select_shifted_mask<D: AddrDag>(
    dag: &mut D,
    node: u32,
    and: u32,
    mask: u64,
    cmp_vt: crate::matcher::Vt,
) -> bool {
    let wide = cmp_vt == mvt::i64;
    let (leading, trailing) = (mask.leading_zeros() - if wide { 0 } else { 32 }, mask.trailing_zeros());
    let x = dag.operand(and, 0);
    let fits_i32 = !wide || i32::try_from(mask as i64).is_ok();
    // Profitable if it drops a movabs or shrinks a 32-bit immediate to 8 bits
    // without an extra register move.
    let saves_bytes = !fits_i32 || dag.node_has_one_use(x.node);
    let flags = SDValue::new(node, 0);
    let shift = |dag: &mut D, opcode: u16, amount: u32| {
        let amount = dag.target_constant(i64::from(amount), cmp_vt);
        SDValue::new(dag.machine_node(opcode, &[cmp_vt, mvt::i32], &[x, amount]), 0)
    };
    let (shr, shl, add, test_rr) = if wide {
        (x86::SHR64ri, x86::SHL64ri, x86::ADD64rr, x86::TEST64rr)
    } else {
        (x86::SHR32ri, x86::SHL32ri, x86::ADD32rr, x86::TEST32rr)
    };
    let (shifted, test, sub) = if leading == 0 && saves_bytes {
        // The mask covers the sign bit: a SHR and the flags of the test.
        (shift(dag, shr, trailing), test_rr, None)
    } else if trailing == 0 && saves_bytes {
        if leading == 1 {
            // Doubling is the same length as the shift, sets ZF identically
            // and the peephole folds the test into it.
            let sum = dag.machine_node(add, &[cmp_vt, mvt::i32], &[x, x]);
            let sum = SDValue::new(sum, 0);
            let test = dag.machine_node(test_rr, &[mvt::i32], &[sum, sum]);
            dag.replace_uses(flags, SDValue::new(test, 0));
            dag.remove_dead_node(node);
            return true;
        }
        (shift(dag, shl, leading), test_rr, None)
    } else if wide && dag.node_has_one_use(dag.operand(and, 1).node) && !fits_i32 {
        // The run reaches into the high half and is 8, 16 or 32 bits wide.
        match 64 - leading - trailing {
            8 => (shift(dag, x86::SHR64ri, trailing), x86::TEST8rr, Some((x86::sub_8bit, mvt::i8))),
            16 => (shift(dag, x86::SHR64ri, trailing), x86::TEST16rr, Some((x86::sub_16bit, mvt::i16))),
            32 => (shift(dag, x86::SHR64ri, trailing), x86::TEST32rr, Some((x86::sub_32bit, mvt::i32))),
            _ => return false,
        }
    } else {
        return false;
    };
    let value = match sub {
        Some((sub_reg, vt)) => {
            let index = dag.target_constant(i64::from(sub_reg), mvt::i32);
            SDValue::new(dag.machine_node(target_opcode::EXTRACT_SUBREG, &[vt], &[shifted, index]), 0)
        }
        None => shifted,
    };
    let test = dag.machine_node(test, &[mvt::i32], &[value, value]);
    dag.replace_uses(flags, SDValue::new(test, 0));
    dag.remove_dead_node(node);
    true
}
