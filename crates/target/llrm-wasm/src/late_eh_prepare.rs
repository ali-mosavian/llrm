//! `WebAssemblyLateEHPrepare`: what a function with EH pads needs once the
//! CFG is final: pads nothing reaches go, each pad starts with a `catch_all`,
//! and the stack pointer is written back after it.
//!
//! # How it works
//! A pad is a block that starts with an `EH_LABEL`. One that has no
//! predecessor is deleted with the blocks only it reached. Every other pad
//! gets `CATCH_ALL_LEGACY` after its label (QB's `catch ptr null` is a
//! catch-all; a typed `catch` is not translated), then a `global.set
//! __stack_pointer` of the register the prologue kept, since the unwinder
//! leaves the global wherever the callee stopped.
//!
//! DEVIATE W18: no `hoistCatches`, `catchret` and `cleanupret`
//! replacement, `THROW_REF`, or `try_table` (the new proposal): none of them
//! is built. Reason: the legacy proposal and landing pads only. Exit: a
//! frontend with typed catches or funclet pads.
//!
//! # See also
//! `WebAssemblyLateEHPrepare.cpp` (`removeUnreachableEHPads` L140,
//! `addCatchAlls` L214, `restoreStackPointer` L395), pin d1106deb71cc.

use crate::dagisel::generated::ids::opcode as op;
use crate::dagisel::post_ids::CATCH_ALL_LEGACY;
use crate::mir::{EH_LABEL, Function, Inst, Operand, PHI, ValType};

/// Whether anything changed.
pub fn late_eh_prepare(f: &mut Function) -> bool {
    if !f.personality {
        return false;
    }
    let removed = remove_unreachable_eh_pads(f);
    let pads: Vec<usize> = (0..f.blocks.len()).filter(|&block| f.is_eh_pad(block)).collect();
    for &pad in &pads {
        // After the phis and the label.
        let at = f.blocks[pad].iter().take_while(|inst| matches!(inst.opcode, PHI | EH_LABEL)).count();
        f.blocks[pad].insert(at, Inst::new(CATCH_ALL_LEGACY, &[]));
        if let Some(sp) = f.sp {
            let (global_set, symbol) = match f.vreg_type(sp) {
                ValType::I64 => (op::GLOBAL_SET_I64, f.symbol("__stack_pointer", 0)),
                _ => (op::GLOBAL_SET_I32, f.symbol("__stack_pointer", 0)),
            };
            f.blocks[pad].insert(at + 1, Inst::new(global_set, &[Operand::Sym(symbol), Operand::Use(sp)]));
        }
    }
    removed || !pads.is_empty()
}

/// `removeUnreachableEHPads`: a pad with no predecessor, and every block that
/// is left with none after it, are deleted.
fn remove_unreachable_eh_pads(f: &mut Function) -> bool {
    let mut preds = vec![0usize; f.blocks.len()];
    for succs in &f.succs {
        for &to in succs {
            preds[to] += 1;
        }
    }
    let mut work: Vec<usize> = (0..f.blocks.len()).filter(|&block| f.is_eh_pad(block) && preds[block] == 0).collect();
    if work.is_empty() {
        return false;
    }
    let mut gone = vec![false; f.blocks.len()];
    while let Some(block) = work.pop() {
        if gone[block] || preds[block] != 0 {
            continue;
        }
        gone[block] = true;
        for &to in &f.succs[block] {
            preds[to] -= 1;
            work.push(to);
        }
    }
    let kept: Vec<usize> = (0..f.blocks.len()).filter(|&block| !gone[block]).collect();
    f.apply_order(&kept);
    true
}
