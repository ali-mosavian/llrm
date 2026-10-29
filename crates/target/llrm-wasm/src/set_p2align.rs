//! `WebAssemblySetP2AlignOperands`: the `p2align` operand of a load or store
//! becomes the log2 of its memory operand's alignment, at most the natural
//! one. Selection leaves it 0.
//!
//! # See also
//! `WebAssemblySetP2AlignOperands.cpp`, pin d1106deb71cc.

use crate::asm_strings::find;
use crate::mir::{Function, Operand};

/// `WebAssembly::GetDefaultP2Align`: the log2 of the access size the
/// mnemonic names, as the printer needs it.
pub fn default_p2align(opcode: u16) -> Option<u64> {
    let asm = find(opcode)?;
    if !asm.names.contains(&"p2align") {
        return None;
    }
    let (ty, op) = asm.text.split_once('.')?;
    Some(if op.contains("8") {
        0
    } else if op.contains("16") {
        1
    } else if op.contains("32") && ty == "i64" {
        2
    } else if ty == "i32" || ty == "f32" {
        2
    } else {
        3
    })
}

/// Whether anything changed.
pub fn set_p2align_operands(f: &mut Function) -> bool {
    let mut changed = false;
    for inst in f.blocks.iter_mut().flatten() {
        let Some(natural) = default_p2align(inst.opcode) else { continue };
        let mem = inst.mem.expect("a load or store has one memory operand");
        assert_eq!(mem.size, 1 << natural, "the default p2align is the access size");
        let at = inst.operands.iter().position(|o| matches!(o, Operand::Imm(_))).expect("a p2align operand");
        assert_eq!(inst.operands[at], Operand::Imm(0), "selection sets p2align to 0");
        inst.operands[at] = Operand::Imm(u64::from(mem.align.trailing_zeros()).min(natural) as i64);
        changed = true;
    }
    changed
}
