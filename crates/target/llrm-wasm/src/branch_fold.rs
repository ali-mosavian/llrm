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
//! `fold_branches` is the rest of `OptimizeBlock` but tail merging: the
//! block before it merges, loses a branch to the block or reverses one, and
//! the block moves after a predecessor, before a successor, or to the end, so
//! that a fall-through happens.
//!
//! `HoistCommonCode` moves the instructions two single-predecessor successors
//! start with into the block that branches to them.
//!
//! DEVIATE 1: tail merging is `tail_merge`'s, no `OptimizeBranches` outside
//! these cases, and not in a function with a personality (try scopes name their
//! blocks). Exit: the shared branch folder over the neutral machine function
//! (S4), no later than 2026-11-20; deleting this file is part of that PR.
//!
//! # See also
//! `BranchFolding.cpp`, pin d1106deb71cc: `OptimizeBlock`, `IsBranchOnlyBlock`.

use crate::dagisel::generated::ids::opcode as op;
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{Function, Inst, Operand, VReg};

/// The target of `block` if it is only an unconditional branch, or empty and
/// so falls into the next block (`OptimizeBlock`: "if this block is empty, make
/// everyone use its fall-through").
fn branch_only(
    f: &Function,
    block: usize,
) -> Option<usize> {
    match f.blocks[block].as_slice() {
        [] => (block + 1 < f.blocks.len() && !f.is_eh_pad(block + 1)).then_some(block + 1),
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

/// The explicit branches ending a block, as `TargetInstrInfo::analyzeBranch`
/// reports them.
#[derive(Clone)]
pub(crate) struct Ending {
    /// `TBB`: the target of the conditional branch, or of the only branch.
    pub(crate) taken: Option<usize>,
    /// `FBB`: the target of the unconditional branch after a conditional one.
    pub(crate) other: Option<usize>,
    /// The condition, a `BR_IF` or `BR_UNLESS` with its target left out.
    pub(crate) cond: Option<Inst>,
}

/// `None` when the block ends in a branch table: `analyzeBranch` fails.
pub(crate) fn ending(
    f: &Function,
    block: usize,
) -> Option<Ending> {
    let target = |inst: &Inst| match inst.operands[..] {
        [Operand::Block(to), ..] => Some(to as usize),
        _ => None,
    };
    let is_cond = |inst: &Inst| matches!(inst.opcode, op::BR_IF | op::BR_UNLESS);
    match f.blocks[block].as_slice() {
        [.., cond, jump] if jump.opcode == op::BR && is_cond(cond) => {
            Some(Ending { taken: target(cond), other: target(jump), cond: Some(cond.clone()) })
        }
        [.., jump] if jump.opcode == op::BR => Some(Ending { taken: target(jump), other: None, cond: None }),
        [.., cond] if is_cond(cond) => Some(Ending { taken: target(cond), other: None, cond: Some(cond.clone()) }),
        [.., last] if last.opcode == op::BR_TABLE_I32 => None,
        _ => Some(Ending { taken: None, other: None, cond: None }),
    }
}

/// `removeBranch` then `insertBranch`.
pub(crate) fn set_branches(
    f: &mut Function,
    block: usize,
    taken: Option<usize>,
    other: Option<usize>,
    cond: Option<&Inst>,
) {
    let insts = &mut f.blocks[block];
    if insts.last().is_some_and(|i| i.opcode == op::BR) {
        insts.pop();
    }
    if insts.last().is_some_and(|i| matches!(i.opcode, op::BR_IF | op::BR_UNLESS)) {
        insts.pop();
    }
    let Some(taken) = taken else { return };
    match cond {
        Some(cond) => {
            let mut cond = cond.clone();
            cond.operands[0] = Operand::Block(taken as u32);
            insts.push(cond);
            if let Some(other) = other {
                insts.push(Inst::new(op::BR, &[Operand::Block(other as u32)]));
            }
        }
        None => insts.push(Inst::new(op::BR, &[Operand::Block(taken as u32)])),
    }
}

/// `reverseBranchCondition`.
pub(crate) fn reversed(cond: &Inst) -> Inst {
    let mut cond = cond.clone();
    cond.opcode = if cond.opcode == op::BR_IF { op::BR_UNLESS } else { op::BR_IF };
    cond
}

/// The layout as a list of blocks that can be moved in constant time.
struct Layout {
    prev: Vec<usize>,
    next: Vec<usize>,
    tail: usize,
}

impl Layout {
    const NONE: usize = usize::MAX;

    fn new(n: usize) -> Layout {
        Layout {
            prev: (0..n).map(|b| b.checked_sub(1).unwrap_or(Self::NONE)).collect(),
            next: (0..n).map(|b| if b + 1 < n { b + 1 } else { Self::NONE }).collect(),
            tail: n - 1,
        }
    }

    fn unlink(
        &mut self,
        b: usize,
    ) {
        let (p, n) = (self.prev[b], self.next[b]);
        if p != Self::NONE {
            self.next[p] = n;
        }
        if n != Self::NONE {
            self.prev[n] = p;
        } else {
            self.tail = p;
        }
    }

    fn insert_after(
        &mut self,
        b: usize,
        at: usize,
    ) {
        let n = self.next[at];
        self.prev[b] = at;
        self.next[b] = n;
        self.next[at] = b;
        if n != Self::NONE {
            self.prev[n] = b;
        } else {
            self.tail = b;
        }
    }

    /// `MachineBasicBlock::moveAfter`.
    fn move_after(
        &mut self,
        b: usize,
        at: usize,
    ) {
        self.unlink(b);
        self.insert_after(b, at);
    }

    /// `MachineBasicBlock::moveBefore`; the entry block stays first.
    fn move_before(
        &mut self,
        b: usize,
        at: usize,
    ) {
        let before = self.prev[at];
        self.unlink(b);
        self.insert_after(b, before);
    }

    fn order(&self) -> Vec<usize> {
        std::iter::successors(Some(0), |&b| Some(self.next[b]).filter(|&x| x != Self::NONE)).collect()
    }
}

struct Folder<'a> {
    f: &'a mut Function,
    layout: Layout,
    preds: Vec<Vec<usize>>,
    /// Merged into the block before: left with no code and no edges.
    dead: Vec<bool>,
}

impl Folder<'_> {
    /// `MachineBasicBlock::canFallThrough`: a branch to the next block counts.
    fn can_fall_through(
        &self,
        block: usize,
    ) -> bool {
        let next = self.layout.next[block];
        let Some(end) = ending(self.f, block) else { return false };
        match (end.taken, end.other, end.cond.is_some()) {
            (None, ..) => falls_through(self.f, block),
            (Some(t), None, false) => t == next,
            (Some(t), Some(o), _) => t == next || o == next,
            (Some(_), None, true) => true,
        }
    }

    fn dedupe_succs(
        &mut self,
        block: usize,
    ) {
        let mut seen = Vec::new();
        self.f
            .succs[block]
            .retain(
                |s| {
                    let new = !seen.contains(s);
                    seen.push(*s);
                    new
                },
            );
    }

    /// `BranchFolder::OptimizeBlock`, without tail merging.
    fn optimize_block(
        &mut self,
        block: usize,
    ) -> bool {
        let mut changed = false;
        loop {
            let prev = self.layout.prev[block];
            if prev == Layout::NONE || self.f.is_eh_pad(block) {
                return changed;
            }
            let cur = ending(self.f, block);
            if let Some(p) = ending(self.f, prev) {
                // Both edges of the block before go to one block.
                if p.taken.is_some() && p.taken == p.other {
                    set_branches(self.f, prev, p.taken.filter(|&t| t != block), None, None);
                    self.dedupe_succs(prev);
                    changed = true;
                    continue;
                }
                // The block before falls into this one only: they are one.
                if p.taken.is_none()
                    && self.preds[block] == [prev]
                    && self.f.succs[prev] == [block]
                    && falls_through(self.f, prev)
                {
                    let code = std::mem::take(&mut self.f.blocks[block]);
                    self.f.blocks[prev].extend(code);
                    self.f.succs[prev] = std::mem::take(&mut self.f.succs[block]);
                    for &s in &self.f.succs[prev] {
                        for pred in &mut self.preds[s] {
                            if *pred == block {
                                *pred = prev;
                            }
                        }
                    }
                    self.preds[block].clear();
                    self.layout.unlink(block);
                    self.dead[block] = true;
                    return true;
                }
                // A branch to the next block is the fall-through.
                if p.taken == Some(block) && p.other.is_none() {
                    set_branches(self.f, prev, None, None, None);
                    changed = true;
                    continue;
                }
                if p.other == Some(block) {
                    set_branches(self.f, prev, p.taken, None, p.cond.as_ref());
                    changed = true;
                    continue;
                }
                // `br_if block; br other` falls through to the block instead.
                if p.taken == Some(block)
                    && let (Some(other), Some(cond)) = (p.other, &p.cond)
                {
                    set_branches(self.f, prev, Some(other), None, Some(&reversed(cond)));
                    changed = true;
                    continue;
                }
                // A block that returns moves to the end when the block before
                // would then fall through to what it branches to.
                let after = self.layout.next[block];
                if self.f.succs[block].is_empty()
                    && p.cond.is_some()
                    && p.other.is_none()
                    && after != Layout::NONE
                    && p.taken == Some(after)
                    && !self.can_fall_through(block)
                    && (after != self.layout.tail || self.better_fall_through(p.taken.unwrap(), block))
                    && let Some(cond) = &p.cond
                {
                    set_branches(self.f, prev, Some(block), None, Some(&reversed(cond)));
                    let tail = self.layout.tail;
                    self.layout.move_after(block, tail);
                    return true;
                }
            }
            // A loop of one block branches back on the true edge.
            if let Some(c) = &cur
                && let (Some(taken), Some(other), Some(cond)) = (c.taken, c.other, &c.cond)
                && other == block
                && taken != block
            {
                set_branches(self.f, block, Some(other), Some(taken), Some(&reversed(cond)));
                changed = true;
                continue;
            }
            return self.move_block(block, prev, cur.as_ref()) || changed;
        }
    }

    /// `BranchFolder::HoistCommonCodeInSuccs`: the instructions both
    /// successors of a conditional branch start with move up into the block.
    fn hoist_common_code(
        &mut self,
        block: usize,
    ) -> bool {
        let Some(end) = ending(self.f, block) else { return false };
        let (Some(taken), Some(cond)) = (end.taken, end.cond.as_ref()) else { return false };
        let _ = cond;
        let other = match end.other {
            Some(other) => other,
            None => match self.f.succs[block].iter().copied().find(|&s| s != taken) {
                Some(other) => other,
                None => return false,
            },
        };
        if taken == other || self.preds[taken].len() > 1 || self.preds[other].len() > 1 {
            return false;
        }
        if self.f.is_eh_pad(taken) || self.f.is_eh_pad(other) {
            return false;
        }
        // The first terminator is the conditional branch; what it reads must
        // not be defined by what moves.
        let terminators = if end.other.is_some() { 2 } else { 1 };
        let mut loc = self.f.blocks[block].len() - terminators;
        let mut uses: Vec<VReg> = self.f.blocks[block][loc]
            .operands
            .iter()
            .filter_map(|o| if let Operand::Use(r) = o { Some(*r) } else { None })
            .collect();
        if !uses.is_empty() && loc > 0 {
            let prev = &self.f.blocks[block][loc - 1];
            if prev.defs().any(|d| uses.contains(&d)) {
                let mut store = true;
                if !crate::machine_sink::is_safe_to_move(prev, &mut store) {
                    return false;
                }
                for o in prev.operands.clone() {
                    match o {
                        Operand::Use(r) => uses.push(r),
                        Operand::Def(r) => uses.retain(|u| *u != r),
                        _ => {}
                    }
                }
                loc -= 1;
            }
        }
        let mut count = 0;
        while count < self.f.blocks[taken].len() && count < self.f.blocks[other].len() {
            let (a, b) = (&self.f.blocks[taken][count], &self.f.blocks[other][count]);
            if a.opcode != b.opcode || a.operands != b.operands || a.is_terminator() {
                break;
            }
            let mut store = true;
            if !crate::machine_sink::is_safe_to_move(a, &mut store) {
                break;
            }
            if a.defs().any(|d| uses.contains(&d)) {
                break;
            }
            count += 1;
        }
        if count == 0 {
            return false;
        }
        let moved: Vec<Inst> = self.f.blocks[taken].drain(..count).collect();
        self.f.blocks[other].drain(..count);
        for (at, inst) in moved.into_iter().enumerate() {
            self.f.blocks[block].insert(loc + at, inst);
        }
        true
    }

    /// `IsBetterFallthrough(first, second)`.
    fn better_fall_through(
        &self,
        first: usize,
        second: usize,
    ) -> bool {
        let is_call =
            |b: usize| self.f.blocks[b].last().is_some_and(|i| desc(i.opcode).is_some_and(|d| d.has(flag::CALL)));
        if self.f.blocks[first].is_empty() || self.f.blocks[second].is_empty() {
            return false;
        }
        if self.f.succs[first].contains(&second) {
            return true;
        }
        if self.f.succs[second].contains(&first) {
            return false;
        }
        is_call(second) && !is_call(first)
    }

    /// The part of `OptimizeBlock` that moves a block to where a
    /// fall-through happens.
    fn move_block(
        &mut self,
        block: usize,
        prev: usize,
        cur: Option<&Ending>,
    ) -> bool {
        if self.can_fall_through(prev) {
            return false;
        }
        let falls = self.can_fall_through(block);
        // After a predecessor that does not fall through.
        let cur_ok = cur.is_some_and(|c| !falls || c.taken.is_none() || c.other.is_none());
        if cur_ok {
            for i in 0..self.preds[block].len() {
                let pred = self.preds[block][i];
                let Some(p) = ending(self.f, pred) else { continue };
                if pred != block
                    && !self.can_fall_through(pred)
                    && (p.taken == Some(block) || p.other == Some(block))
                    && (!falls || block >= pred)
                {
                    if falls {
                        let next = self.layout.next[block];
                        if self.f.blocks[block].last().is_none_or(|l| l.opcode != op::BR) {
                            self.f.blocks[block].push(Inst::new(op::BR, &[Operand::Block(next as u32)]));
                        }
                    }
                    self.layout.move_after(block, pred);
                    return true;
                }
            }
        }
        if falls {
            return false;
        }
        // Before a successor that nothing falls into.
        if let Some(c) = cur {
            for succ in [c.other, c.taken].into_iter().flatten() {
                let before = self.layout.prev[succ];
                if succ != block && before != block && before != Layout::NONE && !self.can_fall_through(before) {
                    self.layout.move_before(block, succ);
                    return true;
                }
            }
        }
        // To the end, when the block before would fall through to the next.
        let next = self.layout.next[block];
        if next != Layout::NONE
            && !self.f.is_eh_pad(next)
            && ending(self.f, prev).is_some()
            && self.f.succs[prev].contains(&next)
        {
            let tail = self.layout.tail;
            self.layout.move_after(block, tail);
            return true;
        }
        false
    }
}

/// `BranchFolder::OptimizeBranches` (DEVIATE 1): the layout and the branches
/// change. Whether anything did.
pub fn fold_branches(f: &mut Function) -> bool {
    let n = f.blocks.len();
    if f.personality || n < 3 {
        return false;
    }
    let mut preds = vec![Vec::new(); n];
    for (from, succs) in f.succs.iter().enumerate() {
        for &to in succs {
            if !preds[to].contains(&from) {
                preds[to].push(from);
            }
        }
    }
    let mut folder = Folder { f, layout: Layout::new(n), preds, dead: vec![false; n] };
    let mut changed = false;
    // The pin repeats until nothing changes; each move lands a fall-through.
    for _ in 0..n {
        let mut round = false;
        for block in folder.layout.order().into_iter().skip(1) {
            if !folder.dead[block] {
                round |= folder.optimize_block(block);
            }
        }
        for block in folder.layout.order() {
            if !folder.dead[block] {
                round |= folder.hoist_common_code(block);
            }
        }
        changed |= round;
        if !round {
            break;
        }
    }
    if changed {
        let mut order = folder.layout.order();
        order.extend((0..n).filter(|&b| folder.dead[b]));
        folder.f.apply_order(&order);
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

    /// A block the coalescer emptied (its copy gone) kept its predecessors:
    /// a branch to it jumps over the block it falls into (memcpy_libcall -Os,
    /// a `block`, a `br` and an `end` more than the pin).
    #[test]
    fn a_branch_to_an_empty_block_goes_to_the_block_it_falls_into() {
        let mut f = function(false);
        f.blocks[1].clear();
        f.succs[1] = vec![2];
        assert!(forward_branch_only_blocks(&mut f));
        // bb1 falls into bb2, which is one `br bb3`: both edges meet at bb3.
        assert_eq!(f.blocks[0].last(), Some(&Inst::new(op::BR, &[Operand::Block(3)])));
        assert!(f.blocks[0].iter().all(|inst| inst.opcode != op::BR_IF));
    }

    /// Both arms of a branch began with the same constant, which the register
    /// coalescer had given one register: each arm computed it, the pin runs it
    /// once above the branch (fpbench O2 1417 B against the pin's 1334 B).
    #[test]
    fn what_both_arms_start_with_moves_above_the_branch() {
        let mut f = Function::default();
        let (c, d) = (f.new_vreg(ValType::I32), f.new_vreg(ValType::I32));
        let seven = || Inst::new(op::CONST_I32, &[Operand::Def(d), Operand::Imm(7)]);
        f.blocks = vec![
            vec![
                Inst::new(op::CONST_I32, &[Operand::Def(c), Operand::Imm(0)]),
                Inst::new(op::BR_IF, &[Operand::Block(1), Operand::Use(c)]),
                Inst::new(op::BR, &[Operand::Block(2)]),
            ],
            vec![seven(), Inst::new(op::RETURN, &[Operand::Use(d)])],
            vec![seven(), Inst::new(op::RETURN, &[Operand::Use(d)])],
        ];
        f.succs = vec![vec![1, 2], vec![], vec![]];
        assert!(fold_branches(&mut f));
        let sevens =
            |b: &[Inst]| b.iter().filter(|i| i.opcode == op::CONST_I32 && i.operands[1] == Operand::Imm(7)).count();
        assert_eq!(sevens(&f.blocks[0]), 1);
        assert_eq!(f.blocks.iter().map(|b| sevens(b)).sum::<usize>(), 1);
    }
}
