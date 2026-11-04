//! `EarlyTailDuplicate`, the pass before the machine SSA optimizations
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/TailDuplicator.cpp#L274)):
//! a block of one `br` is dropped from the path of each predecessor, which
//! branches to its successor instead. The IR's loop preheader, kept apart by
//! `CodeGenPrepare`, is such a block: it costs the loop a branch and the
//! predecessor an `i32.const; return` shape the pin does not have.
//!
//! DEVIATE 1: only `duplicateSimpleBB`; a block with instructions is
//! `tail_duplicate`'s, after register allocation, which has no PHIs to
//! rewrite. Exit: the shared tail duplicator over the neutral machine function
//! (S4), no later than 2026-11-20.
//!
//! # See also
//! `TailDuplicator::{tailDuplicateBlocks, isSimpleBB, shouldTailDuplicate,
//! duplicateSimpleBB, updateSuccessorsPHIs}`, pin d1106deb71cc.

use crate::branch_fold::{ending, set_branches};
use crate::dagisel::generated::ids::opcode as op;
use crate::mir::{Function, Operand, PHI};

/// `isSimpleBB` and `shouldTailDuplicate` for a block that does not fall
/// through: it holds one unconditional branch.
fn is_simple(
    f: &Function,
    tail: usize,
    has_preds: bool,
) -> bool {
    has_preds
        && f.succs[tail].len() == 1
        && f.succs[tail][0] != tail
        && matches!(f.blocks[tail].as_slice(), [only] if only.opcode == op::BR)
}

/// `bothUsedInPHI`: `pred` and the tail share a successor that starts with a
/// PHI, which would need two entries for `pred`.
fn both_used_in_phi(
    f: &Function,
    pred: usize,
    tail: usize,
) -> bool {
    f.succs[pred].iter().any(|s| f.succs[tail].contains(s) && f.blocks[*s].first().is_some_and(|i| i.opcode == PHI))
}

/// `replaceSuccessor`, or `removeSuccessor` when `pred` already reaches `to`.
fn replace_successor(
    f: &mut Function,
    pred: usize,
    from: usize,
    to: usize,
) {
    let at = f.succs[pred].iter().position(|&s| s == from).expect("an edge to the tail");
    if !f.succs[pred].contains(&to) {
        f.succs[pred][at] = to;
        return;
    }
    f.succs[pred].remove(at);
    if let Some(probs) = f.edge_probs.get_mut(pred).filter(|p| p.len() > at) {
        probs.remove(at);
        let sum: u64 = probs.iter().map(|&p| u64::from(p)).sum();
        if sum != 0 {
            for p in probs.iter_mut() {
                *p = ((u64::from(*p) << 31) / sum) as u32;
            }
        }
    }
}

/// `updateSuccessorsPHIs`: each PHI of `succ` takes the tail's value from
/// every block that now jumps to `succ` directly.
fn update_phis(
    f: &mut Function,
    succ: usize,
    tail: usize,
    dead: bool,
    preds: &[usize],
) {
    for phi in f.blocks[succ].iter_mut().take_while(|inst| inst.opcode == PHI) {
        let at = (1..phi.operands.len())
            .step_by(2)
            .find(|&i| phi.operands[i + 1] == Operand::Block(tail as u32))
            .expect("a PHI entry for the tail");
        let value = phi.operands[at];
        if dead {
            phi.operands.drain(at..at + 2);
        }
        for &pred in preds {
            phi.operands.push(value);
            phi.operands.push(Operand::Block(pred as u32));
        }
    }
}

/// Whether anything changed. The caller removes the blocks left without a
/// predecessor.
pub fn early_tail_duplicate(f: &mut Function) -> bool {
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); f.blocks.len()];
    for (from, to) in f.succs.iter().enumerate() {
        for &to in to {
            if !preds[to].contains(&from) {
                preds[to].push(from);
            }
        }
    }
    let mut changed = false;
    for tail in 1..f.blocks.len() {
        if !is_simple(f, tail, !preds[tail].is_empty()) {
            continue;
        }
        let target = f.succs[tail][0];
        let mut duplicated = Vec::new();
        for pred in preds[tail].clone() {
            if f.succs[pred].iter().any(|&s| f.is_eh_pad(s)) || both_used_in_phi(f, pred, tail) {
                continue;
            }
            let Some(end) = ending(f, pred) else { continue };
            let next = (pred + 1 < f.blocks.len()).then_some(pred + 1);
            let (mut taken, mut other, mut cond) = (end.taken, end.other, end.cond);
            if cond.is_none() {
                other = taken;
            }
            taken = taken.or(next);
            other = other.or(next);
            let redirect = |b: Option<usize>| if b == Some(tail) { Some(target) } else { b };
            (taken, other) = (redirect(taken), redirect(other));
            if taken == other {
                cond = None;
                other = None;
            }
            if other == next {
                other = None;
            }
            if taken == next && other.is_none() {
                taken = None;
            }
            set_branches(f, pred, taken, other, cond.as_ref());
            replace_successor(f, pred, tail, target);
            preds[tail].retain(|&p| p != pred);
            if !preds[target].contains(&pred) {
                preds[target].push(pred);
            }
            duplicated.push(pred);
        }
        if duplicated.is_empty() {
            continue;
        }
        let dead = preds[tail].is_empty();
        update_phis(f, target, tail, dead, &duplicated);
        if dead {
            f.blocks[tail].clear();
            f.succs[tail].clear();
            if let Some(probs) = f.edge_probs.get_mut(tail) {
                probs.clear();
            }
            preds[target].retain(|&p| p != tail);
        }
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{Inst, ValType};

    /// bb0 branches to bb1 or bb3; bb1 is a lone `br bb2`; bb2 has a PHI
    /// reading bb1 and bb3.
    fn function() -> (Function, u32) {
        let mut f = Function::default();
        let (a, b, c) = (f.new_vreg(ValType::I32), f.new_vreg(ValType::I32), f.new_vreg(ValType::I32));
        f.blocks = vec![
            vec![Inst::new(op::BR_IF, &[Operand::Block(1), Operand::Use(a)]), Inst::new(op::BR, &[Operand::Block(3)])],
            vec![Inst::new(op::BR, &[Operand::Block(2)])],
            vec![Inst::new(
                PHI,
                &[Operand::Def(c), Operand::Use(a), Operand::Block(1), Operand::Use(b), Operand::Block(3)],
            )],
            vec![Inst::new(op::BR, &[Operand::Block(2)])],
        ];
        f.succs = vec![vec![1, 3], vec![2], vec![], vec![2]];
        (f, a)
    }

    /// A loop preheader that only branches stayed a block of its own, and
    /// the PHI after it kept reading it.
    #[test]
    fn a_predecessor_branches_past_a_block_of_one_branch() {
        let (mut f, a) = function();
        assert!(early_tail_duplicate(&mut f));
        assert_eq!(f.succs[0], [2, 3]);
        assert_eq!(f.blocks[0][0].operands, [Operand::Block(2), Operand::Use(a)]);
        assert!(f.blocks[1].is_empty() && f.succs[1].is_empty());
        let pairs: Vec<_> = f.blocks[2][0].operands[1..].chunks(2).map(|p| p[1]).collect();
        assert_eq!(pairs, [Operand::Block(3), Operand::Block(0)]);
    }
}
