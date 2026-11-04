//! `TailDuplicator`, the pass after register allocation
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/TailDuplicator.cpp#L274)):
//! a block of at most two instructions that cannot fall through is copied
//! into each predecessor that jumps to it unconditionally, so a join of
//! returns (`common.ret`) costs no branch and no local.
//!
//! DEVIATE 1: only the general case (`tailDuplicate`); a block of one `br` is
//! `forward_branch_only_blocks`'s, and a function with a personality is left
//! alone (try scopes name their blocks). Exit: the shared tail duplicator over
//! the neutral machine function (S4), no later than 2026-11-20.
//!
//! # See also
//! `TailDuplicator::{tailDuplicateBlocks, shouldTailDuplicate,
//! canTailDuplicate, tailDuplicate}`, pin d1106deb71cc.

use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{EH_LABEL, Function, IMPLICIT_DEF, Inst};

/// `-tail-dup-size`.
const TAIL_DUP_SIZE: usize = 2;

/// `MachineBasicBlock::canFallThrough`.
fn can_fall_through(
    f: &Function,
    block: usize,
) -> bool {
    f.blocks[block].last().is_none_or(|last| !desc(last.opcode).is_some_and(|d| d.has(flag::BARRIER)))
}

/// `MI.isMetaInstruction()`.
fn is_meta(inst: &Inst) -> bool {
    matches!(inst.opcode, EH_LABEL | IMPLICIT_DEF)
}

/// `shouldTailDuplicate`, after register allocation.
fn should_tail_duplicate(
    f: &Function,
    tail: usize,
    optimize_for_size: bool,
) -> bool {
    if can_fall_through(f, tail) || f.succs[tail].contains(&tail) {
        return false;
    }
    let max = if optimize_for_size { 1 } else { TAIL_DUP_SIZE };
    let mut count = 0;
    for inst in &f.blocks[tail] {
        if desc(inst.opcode).is_some_and(|d| d.has(flag::NOT_DUPLICABLE) || d.has(flag::CONVERGENT)) {
            return false;
        }
        if !is_meta(inst) {
            count += 1;
        }
        if count > max {
            return false;
        }
    }
    true
}

/// Whether the block is only a `br`: `forward_branch_only_blocks` has it.
fn is_simple(
    f: &Function,
    tail: usize,
) -> bool {
    use crate::dagisel::generated::ids::opcode as op;
    f.succs[tail].len() == 1 && f.blocks[tail].iter().filter(|i| !is_meta(i)).all(|i| i.opcode == op::BR)
}

/// `canTailDuplicate`: `pred` has `tail` as its only successor, by an
/// unconditional branch or by falling into it.
fn can_tail_duplicate(
    f: &Function,
    tail: usize,
    pred: usize,
) -> bool {
    use crate::dagisel::generated::ids::opcode as op;
    if f.succs[pred].iter().any(|&s| s != tail) || f.is_eh_pad(tail) {
        return false;
    }
    match f.blocks[pred].last() {
        Some(last) if last.opcode == op::BR => true,
        _ => can_fall_through(f, pred),
    }
}

/// Whether anything changed. The caller removes the blocks left without a
/// predecessor.
pub fn tail_duplicate(
    f: &mut Function,
    optimize_for_size: bool,
) -> bool {
    use crate::dagisel::generated::ids::opcode as op;
    if f.personality {
        return false;
    }
    let mut changed = false;
    for tail in 1..f.blocks.len() {
        if is_simple(f, tail) || !should_tail_duplicate(f, tail, optimize_for_size) {
            continue;
        }
        let mut preds: Vec<usize> = Vec::new();
        for (from, to) in f.succs.iter().enumerate() {
            if to.contains(&tail) && !preds.contains(&from) {
                preds.push(from);
            }
        }
        let mut duplicated = 0;
        for &pred in &preds {
            if pred == tail || !can_tail_duplicate(f, tail, pred) {
                continue;
            }
            // Not into a predecessor that falls through into it.
            if pred + 1 == tail && can_fall_through(f, pred) {
                continue;
            }
            if f.blocks[pred].last().is_some_and(|last| last.opcode == op::BR) {
                f.blocks[pred].pop();
            }
            let copy = f.blocks[tail].clone();
            f.blocks[pred].extend(copy);
            f.succs[pred] = f.succs[tail].clone();
            duplicated += 1;
            changed = true;
        }
        // All but the block before it: its contents move there.
        let prev = tail - 1;
        let remaining = preds.len() - duplicated;
        if remaining == 1
            && preds.contains(&prev)
            && f.succs[prev] == [tail]
            && can_fall_through(f, prev)
            && f.succs.iter().enumerate().filter(|(_, to)| to.contains(&tail)).count() == 1
        {
            if f.blocks[prev].last().is_some_and(|last| last.opcode == op::BR) {
                f.blocks[prev].pop();
            }
            let moved = std::mem::take(&mut f.blocks[tail]);
            f.blocks[prev].extend(moved);
            f.succs[prev] = std::mem::take(&mut f.succs[tail]);
            changed = true;
        }
    }
    changed
}
