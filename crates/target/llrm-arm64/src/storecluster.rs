//! A store is moved down to the store of the neighbouring address that follows
//! it, so that the two are adjacent for `pairs` (the pin's MachineScheduler
//! clusters such stores: `AArch64InstrInfo::shouldClusterMemOps`).
//!
//! # How it works
//! Machine form, virtual registers, before allocation. A store to `[base, #imm]`
//! looks a few instructions ahead for a store of the same size to the next or
//! previous slot from the same base. It passes instructions that neither
//! redefine the base or the value it stores, nor touch memory it does not
//! commute with, and is placed right before the second. Moved before
//! allocation, it costs nothing in registers the way moving it afterwards
//! would: the value lives to the second store, instead of the allocator
//! reusing its register for the value computed between them.
//!
//! ```text
//!   x = a*3 ; str x,[p] ; y = a*5 ; str y,[p,#8]
//!   x = a*3 ; y = a*5 ; str x,[p] ; str y,[p,#8]      -> stp x, y, [p]
//! ```

use crate::mc::inst::*;
use crate::mc::visit::Reg;
use crate::mfunc::{MFunc, Op};
use crate::pairs::{access, commutes, leaves_the_block, pair_of, touch, touch_of};

/// How many instructions ahead a store looks.
const WINDOW: usize = 12;

fn cluster(mut ops: Vec<Op>) -> Vec<Op> {
    let mut i = 0;
    while i < ops.len() {
        let Op::I(inst) = &ops[i] else {
            i += 1;
            continue;
        };
        let Some(first) = access(inst).filter(|first| !first.load) else {
            i += 1;
            continue;
        };
        let (float, number) = crate::pairs::register(&first.mem);
        let source = if float { Reg::Fpr(Vreg(number)) } else { Reg::Gpr(Gpr(number)) };
        let base = Reg::Gpr(first.base);
        for j in i + 1..(i + 1 + WINDOW).min(ops.len()) {
            if let Op::I(other) = &ops[j]
                && let Some(second) = access(other).filter(|second| !second.load)
                && pair_of(&first, &second).is_some()
            {
                if j > i + 1 {
                    let moved = ops.remove(i);
                    ops.insert(j - 1, moved);
                }
                break;
            }
            let (defs, _) = ops[j].defs_and_uses();
            let passable = match &ops[j] {
                Op::I(between) => {
                    !leaves_the_block(between)
                        && (!crate::pairs::is_memory(between)
                            || touch(between).is_some_and(|other| commutes(&touch_of(&first), &other)))
                }
                Op::Copy { .. } | Op::Imm { .. } | Op::FrameAddress { .. } => true,
                Op::Call { .. } | Op::Return { .. } => false,
            };
            if !passable || defs.contains(&base) || defs.contains(&source) {
                break;
            }
        }
        i += 1;
    }
    ops
}

pub fn run(function: &mut MFunc) {
    for block in &mut function.blocks {
        block.ops = cluster(std::mem::take(&mut block.ops));
    }
}
