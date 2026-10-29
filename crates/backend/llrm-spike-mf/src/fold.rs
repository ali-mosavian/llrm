//! Branch folding over [`MFunction`]: a block that only jumps is skipped, a
//! branch to the next block is dropped, a conditional branch over a jump is
//! reversed, and a block that cannot fall through moves next to the branch to
//! it. Translated from `BranchFolder::OptimizeBlock` and `OptimizeBranches` in
//! [BranchFolding.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp).
//!
//! ```text
//!  before                      after
//!  b0: cmp; bgt b2             b0: cmp; ble b3
//!      bra b3                  b2: sub
//!  b2: sub; bra b4             b3: ...
//!  b4: bra b1
//! ```
//!
//! Runs after allocation and phi elimination: blocks have no phis and the
//! copies the allocation made no-ops are gone ([`remove_identity_copies`]).
//!
//! DEVIATE: `TailMergeBlocks`, `HoistCommonCode`, tail calls, EH pads, jump
//! tables and debug instructions are not translated. Reason: tail merging
//! needs the live-in bookkeeping of the pin's `LiveRegs`, and the spike has no
//! other three. Exit: the neutral-LIR task, where `llrm-codegen`'s port takes
//! over.
//! DEVIATE: an empty block that is dead is removed when its last referencing
//! branch is rewritten, and block numbers for the reordering test are the
//! layout indices when the sweep began, as `RenumberBlocks` leaves them.

use std::collections::HashMap;

use crate::mfunc::{Inst, MFunction, Operand};
use crate::regalloc::{Assignment, Target};
use crate::sink::Jump;

/// `analyzeBranch`: where a block's terminators go.
#[derive(Clone, Debug)]
struct Branches {
    taken: Option<u32>,
    other: Option<u32>,
    cond: Option<Inst>,
}

fn target(inst: &Inst) -> Option<u32> {
    inst.operands
        .iter()
        .find_map(
            |operand| match operand {
                Operand::Block(id) => Some(*id),
                _ => None,
            },
        )
}

fn is_jump(inst: &Inst) -> bool {
    inst.terminator && inst.barrier && target(inst).is_some()
}

fn is_conditional(inst: &Inst) -> bool {
    inst.terminator && !inst.barrier && target(inst).is_some()
}

fn same_branch(
    one: &Inst,
    other: &Inst,
) -> bool {
    one.kind == other.kind && one.operands == other.operands
}

/// `TargetInstrInfo::analyzeBranch`; `None` for a terminator it does not know
/// (a return).
fn analyze<C>(
    function: &MFunction<C>,
    at: usize,
) -> Option<Branches> {
    let insts = &function.blocks[at].insts;
    let first = insts.iter().rposition(|inst| !inst.terminator).map_or(0, |last| last + 1);
    match &insts[first..] {
        [] => Some(Branches { taken: None, other: None, cond: None }),
        [one] if is_jump(one) => Some(Branches { taken: target(one), other: None, cond: None }),
        [one] if is_conditional(one) => Some(Branches { taken: target(one), other: None, cond: Some(one.clone()) }),
        [cond, jump] if is_conditional(cond) && is_jump(jump) => {
            Some(Branches { taken: target(cond), other: target(jump), cond: Some(cond.clone()) })
        }
        _ => None,
    }
}

/// `removeBranch`.
fn remove_branch<C>(
    function: &mut MFunction<C>,
    at: usize,
) {
    let insts = &mut function.blocks[at].insts;
    while insts.last().is_some_and(|inst| inst.terminator && target(inst).is_some()) {
        insts.pop();
    }
}

fn retarget(
    inst: &Inst,
    block: u32,
) -> Inst {
    let mut inst = inst.clone();
    for operand in &mut inst.operands {
        if matches!(operand, Operand::Block(_)) {
            *operand = Operand::Block(block);
        }
    }
    inst
}

/// `insertBranch`: `cond` to `taken` and a jump to `other`, or a jump to
/// `taken`.
fn insert_branch<C, J: Jump>(
    function: &mut MFunction<C>,
    at: usize,
    taken: u32,
    other: Option<u32>,
    cond: Option<&Inst>,
) {
    let insts = &mut function.blocks[at].insts;
    match cond {
        None => insts.push(jump_to::<J>(taken)),
        Some(cond) => {
            insts.push(retarget(cond, taken));
            if let Some(other) = other {
                insts.push(jump_to::<J>(other));
            }
        }
    }
}

fn jump_to<J: Jump>(block: u32) -> Inst {
    let mut inst = J::jump(block);
    inst.terminator = true;
    inst.barrier = true;
    inst
}

fn position<C>(
    function: &MFunction<C>,
    id: u32,
) -> usize {
    function.blocks.iter().position(|block| block.id == id).expect("a block that exists")
}

/// `canFallThrough`: control can run off the end into the next block.
fn can_fall_through<C>(
    function: &MFunction<C>,
    at: usize,
) -> bool {
    at + 1 < function.blocks.len() && !function.blocks[at].insts.last().is_some_and(|last| last.barrier)
}

/// The blocks that name `id` in a terminator, and the block before it when
/// that falls into it (`predecessors`).
fn predecessors<C>(
    function: &MFunction<C>,
    id: u32,
) -> Vec<usize> {
    let at = position(function, id);
    (0..function.blocks.len())
        .filter(|&other| {
            function.blocks[other].insts.iter().any(|inst| inst.terminator && target(inst) == Some(id))
                || (other + 1 == at && can_fall_through(function, other))
        })
        .collect()
}

/// `isSuccessor`.
fn is_successor<C>(
    function: &MFunction<C>,
    from: usize,
    id: u32,
) -> bool {
    function.blocks[from].insts.iter().any(|inst| inst.terminator && target(inst) == Some(id))
        || (can_fall_through(function, from) && function.blocks[from + 1].id == id)
}

/// `ReplaceUsesOfBlockWith`.
fn replace_uses<C>(
    function: &mut MFunction<C>,
    at: usize,
    old: u32,
    new: u32,
) {
    for inst in function.blocks[at].insts.iter_mut().rev().take_while(|inst| inst.terminator) {
        for operand in &mut inst.operands {
            if matches!(operand, Operand::Block(id) if *id == old) {
                *operand = Operand::Block(new);
            }
        }
    }
}

fn move_to<C>(
    function: &mut MFunction<C>,
    id: u32,
    index: impl FnOnce(&MFunction<C>) -> usize,
) {
    let from = position(function, id);
    let block = function.blocks.remove(from);
    let to = index(function).min(function.blocks.len());
    function.blocks.insert(to, block);
}

/// `IsBetterFallthrough`.
fn better_fallthrough<C>(
    function: &MFunction<C>,
    one: u32,
    other: u32,
) -> bool {
    let (a, b) = (position(function, one), position(function, other));
    let (Some(last_a), Some(last_b)) = (function.blocks[a].insts.last(), function.blocks[b].insts.last()) else {
        return false;
    };
    if is_successor(function, a, other) {
        return true;
    }
    if is_successor(function, b, one) {
        return false;
    }
    last_b.call && !last_a.call
}

enum Step {
    /// `goto ReoptimizeBlock`.
    Again,
    Done(bool),
}

struct Folder<'a, C, J> {
    function: &'a mut MFunction<C>,
    /// `getNumber` of each block, by id, as the sweep began.
    number: HashMap<u32, usize>,
    jump: std::marker::PhantomData<J>,
}

impl<C, J: Jump> Folder<'_, C, J> {
    /// `OptimizeBlock`.
    fn optimize_block(
        &mut self,
        id: u32,
    ) -> Step {
        let at = position(self.function, id);
        let fall_through = (at + 1 < self.function.blocks.len()).then(|| self.function.blocks[at + 1].id);
        let current = analyze(self.function, at);

        if self.function.blocks[at].insts.is_empty() {
            let preds = predecessors(self.function, id);
            if preds.is_empty() {
                return Step::Done(false);
            }
            let Some(next) = fall_through else { return Step::Done(false) };
            for pred in preds {
                replace_uses(self.function, pred, id, next);
            }
            // nothing names it now, and what fell into it falls into `next`
            self.function.blocks.remove(at);
            return Step::Done(true);
        }

        let prev = at - 1;
        let prior = analyze(self.function, prev);
        if let Some(prior) = &prior {
            if let (Some(taken), Some(other)) = (prior.taken, prior.other) {
                if taken == other {
                    remove_branch(self.function, prev);
                    if taken != id {
                        insert_branch::<C, J>(self.function, prev, taken, None, None);
                    }
                    return Step::Again;
                }
            }
            if prior.cond.is_none()
                && prior.taken.is_none()
                && predecessors(self.function, id).len() == 1
                && prior_successors(self.function, prev) == 1
                && is_successor(self.function, prev, id)
            {
                let block = self.function.blocks.remove(at);
                self.function.blocks[prev].insts.extend(block.insts);
                return Step::Done(true);
            }
            if prior.taken == Some(id) && prior.other.is_none() {
                remove_branch(self.function, prev);
                return Step::Again;
            }
            if prior.other == Some(id) {
                remove_branch(self.function, prev);
                insert_branch::<C, J>(self.function, prev, prior.taken.unwrap(), None, prior.cond.as_ref());
                return Step::Again;
            }
            if prior.taken == Some(id) {
                if let Some(reversed) = prior.cond.as_ref().and_then(|cond| J::reverse(cond)) {
                    remove_branch(self.function, prev);
                    insert_branch::<C, J>(self.function, prev, prior.other.unwrap(), None, Some(&reversed));
                    return Step::Again;
                }
            }
            if self.function.blocks[at].insts.len() == 1
                && can_fall_through(self.function, prev)
                && current.as_ref().is_some_and(|cur| {
                    cur.taken.is_some()
                        && cur.taken == prior.taken
                        && cur.cond.as_ref().zip(prior.cond.as_ref()).is_some_and(|(a, b)| same_branch(a, b))
                })
            {
                remove_branch(self.function, prev);
                return Step::Again;
            }
            if self.function.blocks[at].insts.last().is_some_and(|last| last.barrier)
                && !is_branching(self.function, at)
                && prior.cond.is_some()
                && prior.other.is_none()
                && prior.taken == fall_through
            {
                let mut transform = true;
                if at + 2 == self.function.blocks.len() && !better_fallthrough(self.function, prior.taken.unwrap(), id)
                {
                    transform = false;
                }
                if transform {
                    if let Some(reversed) = prior.cond.as_ref().and_then(|cond| J::reverse(cond)) {
                        remove_branch(self.function, prev);
                        insert_branch::<C, J>(self.function, prev, id, None, Some(&reversed));
                        move_to(self.function, id, |function| function.blocks.len());
                        return Step::Done(true);
                    }
                }
            }
        }

        if let Some(cur) = &current {
            if let (Some(taken), Some(other)) = (cur.taken, cur.other) {
                if other == id && taken != id {
                    if let Some(reversed) = cur.cond.as_ref().and_then(|cond| J::reverse(cond)) {
                        remove_branch(self.function, at);
                        insert_branch::<C, J>(self.function, at, other, Some(taken), Some(&reversed));
                        return Step::Again;
                    }
                }
            }
            if let (Some(taken), None, None) = (cur.taken, cur.other, cur.cond.as_ref()) {
                if self.function.blocks[at]
                    .insts
                    .first()
                    .is_some_and(|first| first.terminator && target(first).is_some())
                    && taken != id
                {
                    remove_branch(self.function, at);
                    let prev_falls = can_fall_through(self.function, prev);
                    if !prev_falls || prior.is_some() || !is_successor(self.function, prev, id) {
                        if prev_falls
                            && is_successor(self.function, prev, id)
                            && let Some(prior) = &prior
                            && prior.taken != Some(id)
                            && prior.other != Some(id)
                        {
                            let (new_taken, new_other) = match prior.taken {
                                None => (id, None),
                                Some(taken) => (taken, Some(id)),
                            };
                            remove_branch(self.function, prev);
                            insert_branch::<C, J>(self.function, prev, new_taken, new_other, prior.cond.as_ref());
                        }
                        let mut changed = false;
                        let mut to_self = false;
                        for pred in predecessors(self.function, id) {
                            if self.function.blocks[pred].id == id {
                                to_self = true;
                                continue;
                            }
                            changed = true;
                            replace_uses(self.function, pred, id, taken);
                            if let Some(after) = analyze(self.function, pred) {
                                if after.taken.is_some() && after.taken == after.other {
                                    remove_branch(self.function, pred);
                                    insert_branch::<C, J>(self.function, pred, after.taken.unwrap(), None, None);
                                }
                            }
                        }
                        if changed && !to_self {
                            return Step::Done(true);
                        }
                    }
                    insert_branch::<C, J>(self.function, at, taken, None, None);
                }
            }
        }

        if !can_fall_through(self.function, prev) {
            let current_falls = can_fall_through(self.function, at);
            let here = *self.number.get(&id).unwrap_or(&at);
            for pred in predecessors(self.function, id) {
                let Some(branches) = analyze(self.function, pred) else { continue };
                let pred_id = self.function.blocks[pred].id;
                let pred_number = *self.number.get(&pred_id).unwrap_or(&pred);
                if pred_id != id
                    && !can_fall_through(self.function, pred)
                    && (branches.taken == Some(id) || branches.other == Some(id))
                    && (!current_falls || current.as_ref().is_none_or(|cur| cur.taken.is_none() || cur.other.is_none()))
                    && (!current_falls || here >= pred_number)
                {
                    if current_falls {
                        let next = self.function.blocks[at + 1].id;
                        insert_branch::<C, J>(self.function, at, next, None, None);
                    }
                    move_to(self.function, id, |function| position(function, pred_id) + 1);
                    return Step::Again;
                }
            }
            if !current_falls {
                if let Some(cur) = &current {
                    // `SuccBB = CurFBB ? CurFBB : CurTBB`: the block it jumps
                    // to, else the one it branches to;
                    // trying both moved it back and forth forever
                    if let Some(succ) = cur.other.or(cur.taken) {
                        let succ_at = position(self.function, succ);
                        if succ != id
                            && succ_at > 0
                            && self.function.blocks[succ_at - 1].id != id
                            && !can_fall_through(self.function, succ_at - 1)
                        {
                            move_to(self.function, id, |function| position(function, succ));
                            return Step::Again;
                        }
                    }
                }
                if let Some(next) = fall_through
                    && analyze(self.function, prev).is_some()
                    && is_successor(self.function, prev, next)
                {
                    move_to(self.function, id, |function| function.blocks.len());
                    return Step::Done(true);
                }
            }
        }
        Step::Done(false)
    }

    /// `OptimizeBranches`.
    fn optimize_branches(&mut self) -> bool {
        self.number = self.function.blocks.iter().enumerate().map(|(at, block)| (block.id, at)).collect();
        let mut changed = false;
        let mut at = 1;
        while at < self.function.blocks.len() {
            let id = self.function.blocks[at].id;
            let next = self.function.blocks.get(at + 1).map(|block| block.id);
            loop {
                match self.optimize_block(id) {
                    Step::Again => changed = true,
                    Step::Done(done) => {
                        changed |= done;
                        break;
                    }
                }
            }
            // dead: nothing branches to it and nothing falls into it
            if let Some(now) = self.function.blocks.iter().position(|block| block.id == id) {
                if now > 0 && predecessors(self.function, id).is_empty() {
                    self.function.blocks.remove(now);
                    changed = true;
                }
            }
            at = match next.and_then(|next| self.function.blocks.iter().position(|block| block.id == next)) {
                Some(next) => next,
                None => break,
            };
        }
        changed
    }
}

/// The successors of a block that ends in no branch: only the next one.
fn prior_successors<C>(
    function: &MFunction<C>,
    at: usize,
) -> usize {
    let mut seen: Vec<u32> =
        function.blocks[at].insts.iter().filter(|inst| inst.terminator).filter_map(target).collect();
    if can_fall_through(function, at) {
        seen.push(function.blocks[at + 1].id);
    }
    seen.sort_unstable();
    seen.dedup();
    seen.len()
}

/// Whether the block ends in a jump or a conditional branch.
fn is_branching<C>(
    function: &MFunction<C>,
    at: usize,
) -> bool {
    function.blocks[at].insts.last().is_some_and(|last| last.terminator && target(last).is_some())
}

/// `BranchFolder::OptimizeFunction` without tail merging: true when anything
/// changed.
pub fn branch_fold<T: Target + Jump>(function: &mut MFunction<T::Class>) -> bool {
    let mut folder = Folder::<T::Class, T> { function, number: HashMap::new(), jump: std::marker::PhantomData };
    let mut ever = false;
    while folder.optimize_branches() {
        ever = true;
    }
    ever
}

/// Drops the copies the allocation gave one register, as the pin's copy
/// elimination after allocation does, so a block that only held them is empty.
pub fn remove_identity_copies<T: Target>(
    function: &mut MFunction<T::Class>,
    assignment: &Assignment,
) {
    use crate::mfunc::{Kind, Reg};
    let root = |reg: Reg| match reg {
        Reg::Virt(v) => T::root(assignment.0[v as usize]),
        Reg::Phys(id) => T::root(id),
    };
    for block in &mut function.blocks {
        block
            .insts
            .retain(
                |inst| {
                    if inst.kind != Kind::Copy {
                        return true;
                    }
                    let [Operand::Def(dst), Operand::Use(src)] = inst.operands[..] else { return true };
                    root(dst) != root(src)
                },
            );
    }
}

/// `MachineCopyPropagation`'s elimination of spurious copies: a copy back
/// into the register another copy filled from it, with neither written
/// since, changes nothing (`%d1 = COPY %d0; %d0 = COPY %d1`). `wide` says
/// whether a class copies the whole register; a narrower copy leaves the rest
/// of the destination as it was.
///
/// DEVIATE: only this case of the pass (no forwarding of uses), within a
/// block. Reason: it is the one the allocator leaves behind (phi and
/// two-address copies of one value). Exit: the neutral-LIR task.
pub fn remove_redundant_copies<T: Target>(
    function: &mut MFunction<T::Class>,
    assignment: &Assignment,
    wide: impl Fn(&T::Class) -> bool,
) {
    remove_redundant_copies_between::<T>(function, assignment, wide, |_| false)
}

/// [`remove_redundant_copies`] where a copy into or out of a fixed register (`whole_fixed`) counts
/// too, as an argument register of a call does.
pub fn remove_redundant_copies_between<T: Target>(
    function: &mut MFunction<T::Class>,
    assignment: &Assignment,
    wide: impl Fn(&T::Class) -> bool,
    whole_fixed: impl Fn(llrm_lir::registers::RegId) -> bool,
) {
    use crate::mfunc::{Kind, Reg};
    let classes = function.classes.clone();
    let root = |reg: Reg| match reg {
        Reg::Virt(v) => T::root(assignment.0[v as usize]),
        Reg::Phys(id) => T::root(id),
    };
    let whole = |reg: Reg| match reg {
        Reg::Virt(v) => wide(&classes[v as usize]),
        Reg::Phys(id) => whole_fixed(id),
    };
    for block in &mut function.blocks {
        // `(destination, source)` roots of the copies still in force
        let mut held: Vec<(_, _)> = Vec::new();
        let mut keep = Vec::with_capacity(block.insts.len());
        for inst in std::mem::take(&mut block.insts) {
            let copy = match inst.operands[..] {
                [Operand::Def(dst), Operand::Use(src)] if inst.kind == Kind::Copy && whole(dst) && whole(src) => {
                    Some((root(dst), root(src)))
                }
                _ => None,
            };
            if let Some((dst, src)) = copy {
                if held.contains(&(src, dst)) {
                    continue;
                }
            }
            if inst.call {
                held.clear();
            }
            let written: Vec<_> = inst
                .operands
                .iter()
                .filter_map(|o| if let Operand::Def(r) = o { Some(root(*r)) } else { None })
                .collect();
            held.retain(|(to, from)| !written.iter().any(|w| T::overlaps(*w, *to) || T::overlaps(*w, *from)));
            if let Some(pair) = copy {
                held.push(pair);
            }
            keep.push(inst);
        }
        block.insts = keep;
    }
}
