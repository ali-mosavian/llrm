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
//! `fold_branches` is `OptimizeBlock` but tail merging, one sweep over the
//! blocks in layout order that bypasses a branch-only block or moves one: the
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
//! (S4), no later than 2026-11-27; deleting this file is part of that PR.
//!
//! # See also
//! `BranchFolding.cpp`, pin d1106deb71cc: `OptimizeBlock`, `IsBranchOnlyBlock`.

use crate::dagisel::generated::ids::opcode as op;
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{Function, Inst, Operand, VReg};

/// Whether control can fall from the end of `block` into the next one.
fn falls_through(
    f: &Function,
    block: usize,
) -> bool {
    f.blocks[block].last().is_none_or(|last| !desc(last.opcode).is_some_and(|d| d.has(flag::BARRIER)))
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

/// `MachineBasicBlock::updateTerminator`: the branches of `block` after the
/// block laid out after it changed from `previous` (what used to follow it) to
/// the next block of the function: a branch to the new fall-through goes, a
/// conditional branch to it is reversed, and a fall-through that moved gets a
/// branch. The layout is the order of `f.blocks`.
pub(crate) fn update_terminator(
    f: &mut Function,
    block: usize,
    previous: Option<usize>,
) {
    if f.succs[block].is_empty() {
        return;
    }
    let Some(end) = ending(f, block) else { return };
    let layout = (block + 1 < f.blocks.len()).then_some(block + 1);
    match (end.taken, end.other, &end.cond) {
        (Some(taken), None, None) => {
            if layout == Some(taken) {
                set_branches(f, block, None, None, None);
            }
        }
        (None, _, _) => {
            let Some(previous) = previous else { return };
            if f.succs[block].contains(&previous) && !f.is_eh_pad(previous) && layout != Some(previous) {
                set_branches(f, block, Some(previous), None, None);
            }
        }
        (Some(taken), Some(other), Some(cond)) => {
            if layout == Some(taken) {
                set_branches(f, block, Some(other), None, Some(&reversed(cond)));
            } else if layout == Some(other) {
                set_branches(f, block, Some(taken), None, Some(cond));
            }
        }
        (Some(taken), None, Some(cond)) => {
            let Some(previous) = previous else { return };
            if previous == taken {
                // The fall-through went where the branch goes: the branch goes.
                set_branches(f, block, None, None, None);
                if layout != Some(taken) {
                    set_branches(f, block, Some(taken), None, None);
                }
            } else if layout == Some(taken) {
                set_branches(f, block, Some(previous), None, Some(&reversed(cond)));
            } else if layout != Some(previous) {
                set_branches(f, block, Some(taken), Some(previous), Some(cond));
            }
        }
        (Some(_), Some(_), None) => {}
    }
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

    /// The target of `block` if it is only an unconditional branch, or empty
    /// and so falls into the block after it in the layout (`OptimizeBlock`:
    /// "if this block is empty, make everyone use its fall-through").
    fn branch_only(
        &self,
        block: usize,
    ) -> Option<usize> {
        match self.f.blocks[block].as_slice() {
            [] => {
                let next = self.layout.next[block];
                (next != Layout::NONE && !self.f.is_eh_pad(next)).then_some(next)
            }
            [only] if only.opcode == op::BR => match only.operands[..] {
                [Operand::Block(to)] if to as usize != block => Some(to as usize),
                _ => None,
            },
            _ => None,
        }
    }

    /// `BranchFolder::OptimizeBlock`, the case of a block that is only an
    /// unconditional branch (`IsBranchOnlyBlock`) or empty: every branch to it
    /// goes to its target instead, the block before it that fell into it gets
    /// an explicit branch first, and a conditional branch whose two edges now
    /// meet becomes the unconditional one. A block left with no predecessor
    /// goes. Whether the block was bypassed.
    fn forward_branch_only(
        &mut self,
        block: usize,
    ) -> bool {
        let n = self.f.blocks.len();
        if self.f.is_eh_pad(block) {
            return false;
        }
        let Some(mut to) = self.branch_only(block) else { return false };
        // Through a chain of such blocks, but not around a cycle of them.
        for _ in 0..n {
            match self.branch_only(to) {
                Some(next) if next != block => to = next,
                _ => break,
            }
        }
        if to == block {
            return false;
        }
        // The block before it that falls into it branches to it, so that the
        // loop below sees every edge.
        let before = self.layout.prev[block];
        if before != Layout::NONE && falls_through(self.f, before) {
            self.f.blocks[before].push(Inst::new(op::BR, &[Operand::Block(block as u32)]));
        }
        let from = block as u32;
        let mut changed = false;
        for at in 0..n {
            if self.dead[at] {
                continue;
            }
            let mut moved = false;
            for operand in self.f.blocks[at].iter_mut().flat_map(|inst| inst.operands.iter_mut()) {
                if *operand == Operand::Block(from) {
                    *operand = Operand::Block(to as u32);
                    moved = true;
                }
            }
            for s in self.f.succs[at].iter_mut().filter(|s| **s == block) {
                *s = to;
            }
            if !moved {
                continue;
            }
            // A conditional branch whose two edges meet is the jump.
            if let [.., cond, jump] = self.f.blocks[at].as_slice() {
                let meets = jump.opcode == op::BR
                    && matches!(cond.opcode, op::BR_IF | op::BR_UNLESS)
                    && cond.operands.first() == jump.operands.first();
                if meets {
                    let last = self.f.blocks[at].len() - 2;
                    self.f.blocks[at].remove(last);
                }
            }
            self.dedupe_succs(at);
            self.preds[block].retain(|&pred| pred != at);
            if !self.preds[to].contains(&at) {
                self.preds[to].push(at);
            }
            changed = true;
        }
        if changed && self.preds[block].is_empty() {
            for s in std::mem::take(&mut self.f.succs[block]) {
                self.preds[s].retain(|&pred| pred != block);
            }
            self.f.blocks[block].clear();
            self.layout.unlink(block);
            self.dead[block] = true;
        }
        changed
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
        // (`!CurFallsThru || !CurTBB || !CurFBB`: a branch table has neither.)
        let cur_ok = cur.is_none_or(|c| !falls || c.taken.is_none() || c.other.is_none());
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
    f.sync_preds();
    let preds = f.preds.clone();
    let mut folder = Folder { f, layout: Layout::new(n), preds, dead: vec![false; n] };
    let mut changed = false;
    // The pin repeats until nothing changes; each move lands a fall-through.
    for _ in 0..n {
        let mut round = false;
        for block in folder.layout.order().into_iter().skip(1) {
            if !folder.dead[block] {
                // One sweep, as the pin's: a block is bypassed or moved, each
                // in the layout the blocks before it left.
                let bypassed = folder.forward_branch_only(block);
                round |= bypassed || folder.optimize_block(block);
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
        folder.f.preds = std::mem::take(&mut folder.preds);
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
        let taken = if first_falls_through { 2 } else { 1 };
        let mut head = vec![
            Inst::new(op::CONST_I32, &[Operand::Def(c), Operand::Imm(0)]),
            Inst::new(op::BR_IF, &[block(taken), Operand::Use(c)]),
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
        assert!(fold_branches(&mut f));
        remove_unreachable_blocks(&mut f);
        // Both edges of bb0 meet at bb3, which is then bb0's alone: one block.
        assert_eq!(f.blocks.len(), 1);
        assert!(f.blocks[0].iter().all(|inst| !matches!(inst.opcode, op::BR_IF | op::BR | op::BR_UNLESS)));
        assert_eq!(f.blocks[0].last().map(|inst| inst.opcode), Some(op::RETURN));
    }

    /// The previous block falls into bb1: it gets `br bb1` first, as the pin's
    /// `OptimizeBlock` makes the fall-through explicit, and bb1 still goes.
    #[test]
    fn a_block_the_previous_one_falls_into_is_bypassed_too() {
        let mut f = function(true);
        assert!(fold_branches(&mut f));
        remove_unreachable_blocks(&mut f);
        assert_eq!(f.blocks.len(), 1);
        assert_eq!(f.blocks[0].last().map(|inst| inst.opcode), Some(op::RETURN));
    }

    /// A block the coalescer emptied (its copy gone) kept its predecessors:
    /// a branch to it jumps over the block it falls into (memcpy_libcall -Os,
    /// a `block`, a `br` and an `end` more than the pin).
    #[test]
    fn a_branch_to_an_empty_block_goes_to_the_block_it_falls_into() {
        let mut f = function(false);
        f.blocks[1].clear();
        f.succs[1] = vec![2];
        assert!(fold_branches(&mut f));
        remove_unreachable_blocks(&mut f);
        // bb1 falls into bb2, which is one `br bb3`: both edges meet at bb3.
        assert_eq!(f.blocks.len(), 1);
        assert_eq!(f.blocks[0].last().map(|inst| inst.opcode), Some(op::RETURN));
    }

    /// The pin's test for moving a block after a predecessor reads
    /// `!CurFallsThru || !CurTBB || !CurFBB`: a branch table has no `TBB` or
    /// `FBB`, so it passes. Ours required a branch we could analyze, and the
    /// blocks after a switch loop landed in another order (tail_calls -Os
    /// `__main_void`, known_loads_walks -O2 +32 instructions).
    fn a_branch_table_block_is_moved_after_its_predecessor_like_any_other() {
        let mut f = Function::default();
        let c = f.new_vreg(ValType::I32);
        let block = Operand::Block;
        let konst = |n: i64| Inst::new(op::CONST_I32, &[Operand::Def(c), Operand::Imm(n)]);
        let table = |targets: &[u32]| {
            let mut operands = vec![Operand::Use(c)];
            operands.extend(targets.iter().map(|&t| block(t)));
            Inst::new(op::BR_TABLE_I32, &operands)
        };
        let br = |to: u32| Inst::new(op::BR, &[block(to)]);
        // A: table. S, L: branch to R and back to A. E: empty. R: falls into
        // B, a table. Z: returns.
        f.blocks = vec![
            vec![konst(10), table(&[3, 1, 2])],
            vec![konst(11), br(4)],
            vec![konst(12), br(0)],
            vec![],
            vec![konst(13)],
            vec![table(&[6, 0])],
            vec![konst(14), Inst::new(op::RETURN, &[])],
        ];
        f.succs = vec![vec![3, 1, 2], vec![4], vec![0], vec![4], vec![5], vec![6, 0], vec![]];
        assert!(fold_branches(&mut f));
        let labels: Vec<i64> = f
            .blocks
            .iter()
            .map(|b| match b.first().map(|i| i.operands[..].to_vec()) {
                Some(o) if o.len() == 2 => {
                    if let Operand::Imm(n) = o[1] {
                        n
                    } else {
                        -1
                    }
                }
                _ => -1,
            })
            .collect();
        // The pin's order for this shape, by `OptimizeBlock` read by hand; the
        // block that returns follows the back edge.
        assert_eq!(labels, [10, 11, 13, 12, 14, -1, -1]);
    }

    /// `SplitCriticalEdge` ends with `updateTerminator`: the block that had
    /// `br_unless S; br A` with A next now has the split block after it, and
    /// A is not next any more, so the branch is `br_if A` falling into the
    /// split block. Ours kept both branches, and the block was not the shape
    /// the pin's branch folder moves after a predecessor (tail_calls -Os: the
    /// second switch loop's latch sat before the blocks the pin puts it after).
    #[test]
    fn a_split_edge_leaves_a_fall_through_conditional_branch() {
        let mut f = Function::default();
        let c = f.new_vreg(ValType::I32);
        let block = Operand::Block;
        f.blocks = vec![
            vec![
                Inst::new(op::CONST_I32, &[Operand::Def(c), Operand::Imm(0)]),
                Inst::new(op::BR_UNLESS, &[block(2), Operand::Use(c)]),
                Inst::new(op::BR, &[block(1)]),
            ],
            vec![Inst::new(op::RETURN, &[])],
            vec![Inst::new(op::RETURN, &[])],
        ];
        f.succs = vec![vec![2, 1], vec![], vec![]];
        crate::phi_elimination::split_edge(&mut f, 0, 2);
        // [P, split, A, S]: P branches to A when the condition holds and falls
        // into the split block.
        assert_eq!(f.blocks.len(), 4);
        let last = f.blocks[0].last().unwrap();
        assert_eq!((last.opcode, last.operands[0]), (op::BR_IF, block(2)), "{:?}", f.blocks[0]);
        assert_eq!(f.blocks[1].len(), 1);
        assert_eq!(f.blocks[1][0].operands[0], block(3));
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
