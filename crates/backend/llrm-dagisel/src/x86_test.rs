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
//! Numbered deviation: **SD311** the 64-bit shifted-mask rewrites (`SHR`/`SHL`
//! plus `TEST64rr`) are not translated; a 64-bit compare falls to the table.
//! Owner w10. Exit: the x86-64 spike. Review 2026-12-10.
//!
//! # Used by
//! [`crate::x86_select::select_node`].
//!
//! [case]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L6508

use crate::dag::SDValue;
use crate::generated::ids::{isd, mvt, target_opcode, x86};
use crate::x86_addr::{AddrDag, vt_bits};
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
