//! `PeepholeOptimizer`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PeepholeOptimizer.cpp)),
//! as far as WebAssembly reaches it: the copies.
//!
//! # How it works
//! Forward through each block, for every `COPY`:
//! 1. `optimizeCoalescableCopy`: a copy of a value another copy made reads that
//!    copy's source (`ValueTracker::getNextSourceFromCopy`, one hop: every wasm
//!    source is in the copy's register class, so `shouldRewriteCopySrc` stops
//!    at the first).
//! 2. `foldRedundantCopy`: a second copy of a register `CopySrcMIs` has seen in
//!    this block goes, and its result is the first copy's.
//!
//! The pin's other rewrites ask the target: `analyzeCompare`, `optimizeSelect`,
//! `optimizeCondBranch`, `isCoalescableExtInstr`, `foldImmediate`,
//! `getIncrementValue` and the load folds are all false or absent for
//! WebAssembly, and no wasm register has a sub-register.
//!
//! ```text
//! %5 = COPY %6                %5 = COPY %6
//! %4 = COPY %6      ==>       (uses of %4 and %3 read %5)
//! %3 = COPY %6
//! ```
//!
//! DEVIATE PO1: a copy of a PHI's result is not traced through the PHI's
//! incoming values (`getNextSourceFromPHI`, `RewritePHILimit`). Exit: none
//! measured; the corpus needs no PHI source.
//!
//! # Runs
//! After `machine_sink`, before the second `dead_machine_instruction_elim`; the
//! MIR is in SSA form.

use std::collections::HashMap;

use crate::mir::{Function, Operand, VReg, is_copy};

/// A copy's destination and source.
fn copy_regs(inst: &crate::mir::Inst) -> Option<(VReg, VReg)> {
    if !is_copy(inst.opcode) {
        return None;
    }
    match inst.operands[..] {
        [Operand::Def(dst), Operand::Use(src)] => Some((dst, src)),
        _ => None,
    }
}

/// Whether anything changed.
pub fn peephole_opt(f: &mut Function) -> bool {
    // `MRI->getVRegDef`: where each register's only definition is.
    let mut defs: HashMap<VReg, (usize, usize)> = HashMap::new();
    for (block, insts) in f.blocks.iter().enumerate() {
        for (at, inst) in insts.iter().enumerate() {
            for reg in inst.defs() {
                defs.insert(reg, (block, at));
            }
        }
    }
    let mut changed = false;
    let mut erased: Vec<Vec<bool>> = f.blocks.iter().map(|insts| vec![false; insts.len()]).collect();
    let mut replaced: HashMap<VReg, VReg> = HashMap::new();
    for block in 0..f.blocks.len() {
        // `CopySrcMIs`: the first copy of each register in this block.
        let mut copy_srcs: HashMap<VReg, VReg> = HashMap::new();
        for at in 0..f.blocks[block].len() {
            let Some((dst, src)) = copy_regs(&f.blocks[block][at]) else { continue };
            // `SP32` is a physical register here, which the pin's copy rewrites
            // leave alone; it has more than one definition.
            if Some(dst) == f.sp || Some(src) == f.sp {
                continue;
            }
            let src = replaced.get(&src).copied().unwrap_or(src);
            // `optimizeCoalescableCopy`.
            if let Some(&(b, i)) = defs.get(&src)
                && !erased[b][i]
                && let Some((_, next)) = copy_regs(&f.blocks[b][i])
            {
                f.blocks[block][at].operands[1] = Operand::Use(next);
                changed = true;
                continue;
            }
            // `foldRedundantCopy`.
            match copy_srcs.get(&src) {
                None => {
                    copy_srcs.insert(src, dst);
                }
                Some(&first) if f.vreg_type(first) == f.vreg_type(dst) => {
                    replaced.insert(dst, first);
                    erased[block][at] = true;
                    changed = true;
                }
                Some(_) => {}
            }
        }
    }
    if !changed {
        return false;
    }
    // `replaceRegWith`, for every copy the fold removed.
    let follow = |mut reg: VReg| {
        while let Some(&next) = replaced.get(&reg) {
            reg = next;
        }
        reg
    };
    for (block, insts) in f.blocks.iter_mut().enumerate() {
        let mut at = 0;
        insts.retain(|_| {
            at += 1;
            !erased[block][at - 1]
        });
        for inst in insts.iter_mut() {
            for operand in inst.operands.iter_mut() {
                if let Operand::Use(reg) = operand {
                    *reg = follow(*reg);
                }
            }
        }
    }
    true
}
