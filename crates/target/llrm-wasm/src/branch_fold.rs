//! `BranchFolder::OptimizeBlock`, the case of a block that is only an
//! unconditional branch
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/BranchFolding.cpp#L1705)):
//! every branch to it goes to its target instead, the block before it that
//! fell into it gets an explicit branch first, a conditional branch whose two
//! edges now meet becomes the unconditional one, and the block, left with no
//! predecessor, goes with `UnreachableMachineBlockElim`. The block
//! `SplitPHIEdges` makes for a PHI copy is such a block once the coalescer has
//! removed the copy.
//!
//! DEVIATE: only this case of `OptimizeBlock` (no tail merging, no branch
//! reversal, no moving of blocks), and not in a function with a personality
//! (try scopes name their blocks). Exit: the shared branch folder over the
//! neutral machine function (S4).
//!
//! # See also
//! `BranchFolding.cpp`, pin d1106deb71cc: `OptimizeBlock`, `IsBranchOnlyBlock`.

use crate::dagisel::generated::ids::opcode as op;
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{Function, Inst, Operand};

/// The target of `block` if it is only an unconditional branch.
fn branch_only(
    f: &Function,
    block: usize,
) -> Option<usize> {
    match f.blocks[block].as_slice() {
        [only] if only.opcode == op::BR => match only.operands[..] {
            [Operand::Block(to)] if to as usize != block => Some(to as usize),
            _ => None,
        },
        _ => None,
    }
}

/// Whether control can fall from the end of `block` into the next one.
fn falls_through(
    f: &Function,
    block: usize,
) -> bool {
    f.blocks[block].last().is_none_or(|last| !desc(last.opcode).is_some_and(|d| d.has(flag::BARRIER)))
}

/// Whether anything changed.
pub fn forward_branch_only_blocks(f: &mut Function) -> bool {
    if f.personality {
        return false;
    }
    let mut changed = false;
    for block in 1..f.blocks.len() {
        let Some(mut to) = branch_only(f, block) else { continue };
        if f.is_eh_pad(block) {
            continue;
        }
        // Through a chain of such blocks, but not around a cycle of them.
        for _ in 0..f.blocks.len() {
            match branch_only(f, to) {
                Some(next) if next != block => to = next,
                _ => break,
            }
        }
        if to == block {
            continue;
        }
        // The block before it that falls into it branches to it, so that the
        // loop below sees every edge.
        let before = block - 1;
        if falls_through(f, before) {
            f.blocks[before].push(Inst::new(op::BR, &[Operand::Block(block as u32)]));
        }
        let from = block as u32;
        for at in 0..f.blocks.len() {
            let mut moved = false;
            for operand in f.blocks[at].iter_mut().flat_map(|inst| inst.operands.iter_mut()) {
                if *operand == Operand::Block(from) {
                    *operand = Operand::Block(to as u32);
                    moved = true;
                }
            }
            for s in f.succs[at].iter_mut().filter(|s| **s == block) {
                *s = to;
            }
            // A conditional branch whose two edges meet is the jump.
            if moved && let [.., cond, jump] = f.blocks[at].as_slice() {
                let meets = jump.opcode == op::BR
                    && matches!(cond.opcode, op::BR_IF | op::BR_UNLESS)
                    && cond.operands.first() == jump.operands.first();
                if meets {
                    let last = f.blocks[at].len() - 2;
                    f.blocks[at].remove(last);
                }
            }
            changed |= moved;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::ValType;
    use crate::unreachable_blocks::remove_unreachable_blocks;

    /// bb0 branches to bb1 or bb2, bb1 and bb2 are one `br` to bb3.
    /// `first_falls_through` leaves bb0 without its `br bb2`.
    fn function(first_falls_through: bool) -> Function {
        let mut f = Function::default();
        let c = f.new_vreg(ValType::I32);
        let block = Operand::Block;
        let mut head = vec![
            Inst::new(op::CONST_I32, &[Operand::Def(c), Operand::Imm(0)]),
            Inst::new(op::BR_IF, &[block(1), Operand::Use(c)]),
        ];
        if !first_falls_through {
            head.push(Inst::new(op::BR, &[block(2)]));
        }
        f.blocks = vec![
            head,
            vec![Inst::new(op::BR, &[block(3)])],
            vec![Inst::new(op::BR, &[block(3)])],
            vec![Inst::new(op::RETURN, &[])],
        ];
        f.succs = vec![vec![1, 2], vec![3], vec![3], vec![]];
        f
    }

    /// The block `SplitPHIEdges` made stayed a block of one `br` after the
    /// coalescer emptied it: a `block`/`end_block` pair and a `br` for a jump
    /// to where the branch could go.
    #[test]
    fn a_branch_to_a_block_of_one_br_goes_to_its_target_and_the_block_goes() {
        let mut f = function(false);
        assert!(forward_branch_only_blocks(&mut f));
        // Both edges of bb0 meet at bb3: the conditional branch is dropped.
        assert_eq!(f.blocks[0].last(), Some(&Inst::new(op::BR, &[Operand::Block(3)])));
        assert!(f.blocks[0].iter().all(|inst| inst.opcode != op::BR_IF));
        remove_unreachable_blocks(&mut f);
        assert_eq!(f.blocks.len(), 2);
    }

    /// The previous block falls into bb2: it gets `br bb2` first, as the pin's
    /// `OptimizeBlock` makes the fall-through explicit, and bb2 still goes.
    #[test]
    fn a_block_the_previous_one_falls_into_is_bypassed_too() {
        let mut f = function(true);
        assert!(forward_branch_only_blocks(&mut f));
        assert_eq!(f.blocks[0].last(), Some(&Inst::new(op::BR, &[Operand::Block(3)])));
        remove_unreachable_blocks(&mut f);
        assert_eq!(f.blocks.len(), 2);
    }
}
