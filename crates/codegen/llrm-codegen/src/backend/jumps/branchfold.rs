//! LLVM's `BranchFolder::OptimizeBranches`: each block's way out is cleaned up
//! against the layout until no block changes.
//! <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1426>
//!
//! # How it works
//! `Exit::of` is `analyzeBranch`: a block ends in a conditional branch to
//! `taken` and a jump to `other`, in a jump to `taken`, or in neither (it falls
//! through, returns, or leaves by a table). `Folder::block` is `OptimizeBlock`,
//! its rules in LLVM's order, each starting the block again if it changed it:
//!
//! 1. a block that does nothing and goes to one block is bypassed by every
//!    block that enters it (an empty block, or one that only jumps);
//! 2. a conditional whose two ways are one block is a jump;
//! 3. a jump to the block laid out next is dropped, and `jcc L; jmp M` with `L`
//!    next becomes `j!cc M`;
//! 4. `jcc Out; jmp Loop` in the block `Loop` becomes `j!cc Loop; jmp Out`
//!    (L1685);
//! 5. a block of one conditional branch identical to the branch that ends the
//!    block before it takes that branch away (L1586).
//!
//! A branch on a compare of a register the block just loaded with a constant
//! is a jump or nothing (`knownflags`, DEVIATE B1).
//!
//! A block that nothing enters is dead; its source bytes are kept by `_prune`.
//!
//! # Deviations from the pin
//! - D20: the rules that move a block (`moveAfter`, `moveBefore`) are not
//!   translated; placement has laid the blocks out, and LLVM redoes its layout
//!   after this pass changes anything.
//! - D21: the next block is the next that emits something (the explicit form
//!   leaves a block with only anchors where it was), and a block's own jump is
//!   read against it, not the previous block's against this one.
//!
//! # See also
//! `tailmerge` (the pass this one follows), `placement`.

mod knownflags;

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::backend::layout::_OPPOSITE;
use crate::model::ir::{Operation, Semantics};
use crate::model::lir::{self, Block, BlockOdds, Insn, LirBlock, LirBody};
use crate::support::hash::{IndexMap, IndexSet};

/// `analyzeBranch`: the branches a block ends in.
#[derive(Default)]
struct Exit {
    /// The conditional branch, if the block has one.
    cond: Option<Arc<Insn>>,
    /// Where the conditional branch goes, or the jump if there is no branch.
    taken: Option<i64>,
    /// Where the jump after a conditional branch goes.
    other: Option<i64>,
    /// The jump, if the block ends in one.
    jump: Option<Arc<Insn>>,
}

/// Where a direct branch or jump of kind `op` goes.
fn direct(
    one: &Insn,
    op: Operation,
) -> Option<i64> {
    one.what
        .as_ref()
        .filter(|what| what.op == op && !what.indirect && what.table.is_empty())
        .and_then(|what| what.target)
}

impl Exit {
    /// A block that leaves for more than two blocks is not analysed, as LLVM's
    /// `analyzeBranch` gives up on one with several conditional branches.
    fn of(block: &LirBlock) -> Self {
        if block.succ.len() > 2 {
            return Self::default();
        }
        let (last, before) = super::_real_tail(block);
        let Some(last) = last else { return Self::default() };
        if let Some(to) = direct(&last, Operation::Jump) {
            return match before.and_then(|cond| direct(&cond, Operation::Branch).map(|taken| (cond, taken))) {
                Some((cond, taken)) => Self { cond: Some(cond), taken: Some(taken), other: Some(to), jump: Some(last) },
                None => Self { taken: Some(to), jump: Some(last), ..Self::default() },
            };
        }
        match direct(&last, Operation::Branch) {
            Some(taken) => Self { cond: Some(last), taken: Some(taken), ..Self::default() },
            None => Self::default(),
        }
    }
}

/// A branch that reads or writes a counter (`jcxz`, `loop`), not only the
/// flags: it has no opposite, and dropping, hoisting over or merging it changes
/// the counter. LLVM's `analyzeBranch` gives up on it.
fn counted(one: &Insn) -> bool {
    one.what.as_ref().and_then(|what| what.name.as_deref()).is_some_and(|name| !_OPPOSITE.contains_key(name))
}

/// `one` going to `target`, the condition made `name` if given.
fn aimed(
    one: &Arc<Insn>,
    target: i64,
    name: Option<&str>,
) -> Arc<Insn> {
    let what = one.what.as_ref().expect("a branch has semantics");
    let name = name.map(std::sync::Arc::<str>::from).or_else(|| what.name.clone());
    Arc::new(Insn { what: Some(Semantics { name, target: Some(target), ..what.clone() }), ..(**one).clone() })
}

/// Whether control can run off the end of `block`.
fn runs_off(block: &LirBlock) -> bool {
    let last = block.insns.iter().rev().filter_map(|one| one.what.as_ref()).find(|what| what.op != Operation::Nothing);
    !last.is_some_and(|last| matches!(last.op, Operation::Jump | Operation::Return))
}

#[cfg(test)]
thread_local! {
    /// Blocks looked at by `Folder::pass`.
    pub(super) static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct Folder<'a> {
    /// The blocks in layout order; a dead block stays for `_prune`.
    blocks: Vec<Block>,
    pos: IndexMap<i64, usize>,
    /// The next and the previous live block in layout.
    next: Vec<Option<usize>>,
    prev: Vec<Option<usize>>,
    dead: Vec<bool>,
    preds: IndexMap<i64, BTreeSet<i64>>,
    odds: BlockOdds,
    entry: i64,
    /// Loop headers with a measured trip count stay where they are.
    protected: &'a BTreeSet<i64>,
    /// The register file and width the effects of an instruction are worked
    /// out in; none in a body that has no allocation (a test's).
    regs: Option<(llrm_lir::registers::Regs, u32)>,
    /// The value an instruction of the other way, hoisted away, defined or
    /// read, to the hoisted instruction's (as in `tailmerge`).
    rename: IndexMap<u32, u32>,
    /// Blocks that lost flow when a branch was decided (`knownflags`): their
    /// recorded frequencies are stale, as are those of what they lead to.
    stale: BTreeSet<i64>,
}

/// `body` with its branches folded; `protected` blocks are never bypassed.
pub fn folded(
    body: &LirBody,
    protected: &BTreeSet<i64>,
) -> LirBody {
    let mut folder = Folder::of(body, protected);
    while folder.pass() {}
    if !folder.rename.is_empty() {
        let rename: IndexMap<u32, u32> =
            folder.rename.keys().map(|one| (*one, super::_root(&folder.rename, *one))).collect();
        for block in &mut folder.blocks {
            let insns = block.insns.iter().map(|one| crate::backend::bodyfacts::_renamed(one, &rename)).collect();
            *block = block.with_insns(insns);
        }
    }
    let mut done = body.with_blocks(folder.blocks);
    done.odds = folder.odds;
    super::_prune(&mut done);
    super::_forget_moved_inflow(body, &mut done);
    if !folder.stale.is_empty()
        && let Some(kept) = done.frequencies.as_ref()
    {
        let table = kept.0.iter().filter(|(at, _)| !folder.stale.contains(at)).map(|(at, runs)| (*at, *runs)).collect();
        done.frequencies = Some(Arc::new(lir::BlockFrequencies(table)));
    }
    done
}

/// Whether the flags `block.insns[at]` sets are written again before any
/// instruction reads them. A call or return reads none (the pin's `RET` and
/// `CALL` have no EFLAGS use), and none are live into a successor.
fn flags_dead_after(
    regs: llrm_lir::registers::Regs,
    bits: u32,
    block: &LirBlock,
    at: usize,
) -> bool {
    let mut live = crate::backend::peephole::_ARITHMETIC_LANES.clone();
    for insn in &block.insns[at + 1..] {
        if insn.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Call | Operation::Return)) {
            return true;
        }
        let (read, written) = crate::backend::peephole::_flag_effect(regs, bits, insn);
        if !read.is_disjoint(&live) {
            return false;
        }
        live = live.minus(&written);
        if live.is_empty() {
            return true;
        }
    }
    true
}

/// An instruction `isSafeToMove` allows above a branch: a move or arithmetic
/// on registers and constants, with nothing else it does.
fn pure(insn: &Insn) -> bool {
    insn.what.as_ref().is_some_and(|what| {
        matches!(
            what.op,
            Operation::Move | Operation::Binary | Operation::Unary
        )
            && what.dests.iter().all(|loc| matches!(loc, crate::model::ir::Loc::Reg(_)))
            && what
                .sources
                .iter()
                .all(|loc| matches!(
                    loc,
                    crate::model::ir::Loc::Reg(_) | crate::model::ir::Loc::Imm(_)
                ))
    }) && insn.call.is_none()
        && insn.group.is_none()
        && insn.symbol.is_none()
        && insn.spread.is_empty()
        && insn.clobbers.is_empty()
        && !insn.volatile
        && !insn.frame_adjust
        && !insn.spill_reload
        && !insn.spill_store
}

impl<'a> Folder<'a> {
    fn of(
        body: &LirBody,
        protected: &'a BTreeSet<i64>,
    ) -> Self {
        let count = body.blocks.len();
        let mut preds: IndexMap<i64, BTreeSet<i64>> = IndexMap::default();
        for block in &body.blocks {
            for to in &block.succ {
                preds.entry(*to).or_default().insert(block.at);
            }
        }
        Self {
            blocks: body.blocks.clone(),
            pos: body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect(),
            next: (0..count).map(|index| (index + 1 < count).then_some(index + 1)).collect(),
            prev: (0..count).map(|index| index.checked_sub(1)).collect(),
            dead: vec![false; count],
            preds,
            odds: body.odds.clone(),
            entry: body.entry,
            protected,
            regs: body.registers.map(|regs| (regs, body.bits)),
            rename: IndexMap::default(),
            stale: BTreeSet::new(),
        }
    }

    /// `OptimizeBranches`: every live block once. Whether any changed.
    fn pass(&mut self) -> bool {
        let mut changed = false;
        let mut index = Some(0);
        while let Some(at) = index {
            index = self.next[at];
            if self.dead[at] {
                continue;
            }
            #[cfg(test)]
            VISITS.with(|one| one.set(one.get() + 1));
            changed |= self.block(at);
            let block = &self.blocks[at];
            if block.at != self.entry && self.preds.get(&block.at).is_none_or(BTreeSet::is_empty) {
                self.kill(at);
                changed = true;
            }
        }
        changed
    }

    /// `RemoveDeadBlock`: nothing enters the block, so it enters no one.
    fn kill(
        &mut self,
        index: usize,
    ) {
        let at = self.blocks[index].at;
        for to in self.blocks[index].succ.iter() {
            if let Some(entering) = self.preds.get_mut(to) {
                entering.remove(&at);
            }
        }
        self.dead[index] = true;
        let (before, after) = (self.prev[index], self.next[index]);
        if let Some(before) = before {
            self.next[before] = after;
        }
        if let Some(after) = after {
            self.prev[after] = before;
        }
    }

    /// The block laid out after `index` that emits something, which control
    /// reaches by running off the end of `index`.
    fn after(
        &self,
        index: usize,
    ) -> Option<i64> {
        let mut at = self.next[index];
        while let Some(next) = at {
            let block = &self.blocks[next];
            if !block.phis.is_empty() || block.insns.iter().any(|one| !one.is_meta()) {
                return Some(block.at);
            }
            at = self.next[next];
        }
        None
    }

    /// `OptimizeBlock`. Whether the block changed.
    fn block(
        &mut self,
        index: usize,
    ) -> bool {
        let mut changed = false;
        while self.rules(index) {
            changed = true;
        }
        changed
    }

    /// One rule of `OptimizeBlock`, the first that applies: whether one did.
    fn rules(
        &mut self,
        index: usize,
    ) -> bool {
        let block = self.blocks[index].clone();
        // A block that does nothing and goes to one block.
        if block.at != self.entry
            && !self.protected.contains(&block.at)
            && let Some(onward) = super::_passage(&block)
            && onward != block.at
            && self.preds.get(&block.at).is_some_and(|entering| entering.iter().any(|from| *from != block.at))
        {
            return self.bypass(index, onward);
        }
        if self.merged(index) {
            return true;
        }
        let exit = Exit::of(&block);
        if exit.cond.as_deref().is_some_and(counted) {
            return false;
        }
        // A branch on a compare of a constant just loaded.
        if self.constant_branch(index, &exit) {
            return true;
        }
        // A conditional whose two ways are one block.
        if let (Some(cond), Some(jump)) = (&exit.cond, &exit.jump)
            && exit.taken == exit.other
        {
            let insns = block
                .insns
                .iter()
                .map(|one| if Arc::ptr_eq(one, cond) { lir::anchor(Arc::clone(one)) } else { Arc::clone(one) })
                .collect();
            let _ = jump;
            self.set(
                index,
                LirBlock {
                    succ: exit.taken.into_iter().collect::<Vec<_>>().into(),
                    ..block.with_insns(insns).into_inner()
                },
            );
            return true;
        }
        // A conditional branch to the block it runs into is a jump.
        if let (Some(cond), None, Some(to)) = (&exit.cond, &exit.jump, exit.taken)
            && block.succ.len() == 1
        {
            let jump = Arc::new(Insn {
                what: Some(Semantics { name: Some("jmp".into()), target: Some(to), ..Semantics::new(Operation::Jump) }),
                uses: Vec::new(),
                ..(**cond).clone()
            });
            let insns = block
                .insns
                .iter()
                .map(|one| if Arc::ptr_eq(one, cond) { Arc::clone(&jump) } else { Arc::clone(one) })
                .collect();
            self.set(index, block.with_insns(insns).into_inner());
            return true;
        }
        let after = self.after(index);
        // A jump to the next block is dropped.
        if let Some(jump) = &exit.jump
            && exit.other.or(exit.taken) == after
        {
            // A decoded jump may own source bytes; retain those as an inert
            // anchor so layout can account for them. One the frontend inserted
            // owns none and goes.
            let insns = block
                .insns
                .iter()
                .filter(|one| !Arc::ptr_eq(one, jump) || !one.inserted())
                .map(|one| if Arc::ptr_eq(one, jump) { lir::anchor(Arc::clone(one)) } else { Arc::clone(one) })
                .collect();
            self.set(index, block.with_insns(insns).into_inner());
            return true;
        }
        // `jcc next; jmp M` is `j!cc M`.
        if let (Some(cond), Some(jump), Some(taken), Some(to)) = (&exit.cond, &exit.jump, exit.taken, exit.other)
            && Some(taken) == after
            && let Some(opposite) =
                cond.what.as_ref().and_then(|what| what.name.as_deref()).and_then(|name| _OPPOSITE.get(name))
        {
            let flipped = aimed(cond, to, Some(opposite));
            let insns = block
                .insns
                .iter()
                .filter(|one| !Arc::ptr_eq(one, jump))
                .map(|one| if Arc::ptr_eq(one, cond) { Arc::clone(&flipped) } else { Arc::clone(one) })
                .collect();
            self.set(index, block.with_insns(insns).into_inner());
            return true;
        }
        // `jcc Out; jmp Loop` in a block that is `Loop`: `j!cc Loop; jmp Out`,
        // so the one-block loop is a conditional branch back (L1685).
        if let (Some(cond), Some(jump), Some(taken), Some(other)) = (&exit.cond, &exit.jump, exit.taken, exit.other)
            && other == block.at
            && taken != block.at
            && let Some(opposite) =
                cond.what.as_ref().and_then(|what| what.name.as_deref()).and_then(|name| _OPPOSITE.get(name))
        {
            let (back, out) = (aimed(cond, other, Some(opposite)), aimed(jump, taken, None));
            let insns = block
                .insns
                .iter()
                .map(|one| {
                    if Arc::ptr_eq(one, cond) {
                        Arc::clone(&back)
                    } else if Arc::ptr_eq(one, jump) {
                        Arc::clone(&out)
                    } else {
                        Arc::clone(one)
                    }
                })
                .collect();
            self.set(index, block.with_insns(insns).into_inner());
            return true;
        }
        // A block of one conditional branch identical to the one that ends the
        // block it follows: the earlier branch goes (L1586).
        if let (Some(cond), None, Some(taken)) = (&exit.cond, &exit.jump, exit.taken)
            && block.insns.iter().filter(|one| !one.is_meta()).count() == 1
            && let Some(before) = self.prev[index]
        {
            let previous = self.blocks[before].clone();
            let ahead = Exit::of(&previous);
            let name = |one: &Arc<Insn>| one.what.as_ref().and_then(|what| what.name.clone());
            if let (Some(earlier), None, Some(there)) = (&ahead.cond, &ahead.jump, ahead.taken)
                && !counted(earlier)
                && there == taken
                && name(earlier) == name(cond)
                && self.after(before) == Some(block.at)
                && previous.succ.contains(&block.at)
            {
                let insns = previous
                    .insns
                    .iter()
                    .map(|one| if Arc::ptr_eq(one, earlier) { lir::anchor(Arc::clone(one)) } else { Arc::clone(one) })
                    .collect();
                self.odds.rerouted(previous.at, &previous.succ, taken, &[(block.at, 1.0)]);
                if let Some(entering) = self.preds.get_mut(&taken) {
                    entering.remove(&previous.at);
                }
                self.set(
                    before,
                    LirBlock { succ: std::iter::once(block.at).collect(), ..previous.with_insns(insns).into_inner() },
                );
                return true;
            }
        }
        self.hoist_common(index, &exit)
    }

    /// The block before runs into this one and nothing else enters it: the
    /// two are one block, the contents of this one moved to the end of the
    /// other (L1515).
    fn merged(
        &mut self,
        index: usize,
    ) -> bool {
        let block = self.blocks[index].clone();
        let Some(before) = self.prev[index] else { return false };
        let previous = self.blocks[before].clone();
        let exit = Exit::of(&previous);
        if block.at == self.entry
            || self.protected.contains(&block.at)
            || !block.phis.is_empty()
            || exit.cond.is_some()
            || exit.jump.is_some()
            || previous.succ.len() != 1
            || !previous.succ.contains(&block.at)
            || !runs_off(&previous)
            || self.after(before) != Some(block.at)
            || self.preds.get(&block.at).is_none_or(|entering| entering.len() != 1 || !entering.contains(&previous.at))
        {
            return false;
        }
        for (to, chance) in self.odds.chances(block.at, &block.succ) {
            self.odds
                .taken
                .insert((previous.at, to), (chance * BlockOdds::CERTAIN).round().min(f64::from(u32::MAX)) as u32);
        }
        for to in block.succ.iter() {
            if let Some(entering) = self.preds.get_mut(to) {
                entering.remove(&block.at);
                entering.insert(previous.at);
            }
        }
        let insns = previous.insns.iter().chain(block.insns.iter()).cloned().collect();
        self.set(before, LirBlock { succ: block.succ.clone(), ..previous.with_insns(insns).into_inner() });
        // `pass` removes it: nothing enters it and it enters no one.
        self.set(index, LirBlock { succ: Default::default(), ..block.with_insns(Vec::new()).into_inner() });
        if let Some(entering) = self.preds.get_mut(&block.at) {
            entering.clear();
        }
        true
    }

    /// `HoistCommonCodeInSuccs` (L2043): the instructions both ways of a
    /// conditional branch begin with, where this block is the only way into
    /// either, run ahead of the branch (and of the compare that sets it up)
    /// instead: the longest run that neither the compare nor the branch reads
    /// or writes (L2088-2186). Only moves and arithmetic on registers and
    /// constants, the kind allocation inserts (`isSafeToMove` is false for
    /// the rest); an instruction that sets flags needs them dead after it, and
    /// the compare in front of it to set them again.
    fn hoist_common(
        &mut self,
        index: usize,
        exit: &Exit,
    ) -> bool {
        let Some((regs, bits)) = self.regs else { return false };
        let block = self.blocks[index].clone();
        let (Some(cond), Some(taken)) = (&exit.cond, exit.taken) else { return false };
        let Some(other) = exit.other.or_else(|| self.after(index)) else { return false };
        if taken == other || taken == block.at || other == block.at {
            return false;
        }
        let alone =
            |at: i64| self.preds.get(&at).is_some_and(|entering| entering.len() == 1 && entering.contains(&block.at));
        if !alone(taken) || !alone(other) {
            return false;
        }
        let (Some(&one), Some(&two)) = (self.pos.get(&taken), self.pos.get(&other)) else { return false };
        let (first, second) = (self.blocks[one].clone(), self.blocks[two].clone());
        if !first.phis.is_empty() || !second.phis.is_empty() {
            return false;
        }
        let real = |block: &LirBlock| -> Vec<usize> {
            block.insns.iter().enumerate().filter(|(_, insn)| !insn.is_meta()).map(|(at, _)| at).collect()
        };
        let (firsts, seconds) = (real(&first), real(&second));
        let pairs = |at: usize| match (firsts.get(at), seconds.get(at)) {
            (Some(&one), Some(&two)) => {
                let (head, tail) = (&first.insns[one], &second.insns[two]);
                (head.what == tail.what && head.inserted() && tail.inserted() && pure(head)).then_some((one, two))
            }
            _ => None,
        };
        if pairs(0).is_none() {
            return false;
        }
        // From the compare that sets up the branch, or the branch, to the end.
        let last = block.insns.iter().position(|insn| Arc::ptr_eq(insn, cond)).expect("the branch is in its block");
        let mut from = last;
        if let Some(before) = block.insns[..last].iter().rposition(|insn| !insn.is_meta()) {
            if block.insns[before].what.as_ref().is_some_and(|what| what.op == Operation::Compare) {
                from = before;
            }
        }
        let flags = crate::backend::peephole::_flag_lanes(0xFFFF_FFFF);
        let (mut used, mut defined) = (crate::backend::lanes::Lanes::new(), crate::backend::lanes::Lanes::new());
        // The branches read the flags and no register; a move writes neither.
        let branches =
            |insn: &Insn| insn.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Branch | Operation::Jump));
        for insn in block.insns[from..].iter().filter(|insn| !insn.is_meta() && !branches(insn)) {
            let Some((read, write)) = crate::backend::peephole::_register_effects(regs, bits, insn, true, true) else {
                return false;
            };
            used = used.or(&read);
            defined = defined.or(&write);
        }
        // With no compare ahead of it the branch reads flags nothing may set.
        if from == last {
            used = used.or(&flags);
        }
        let mut run = Vec::new();
        while let Some((at, by)) = pairs(run.len()) {
            let head = &first.insns[at];
            let Some((reads, writes)) = crate::backend::peephole::_register_effects(regs, bits, head, true, true)
            else {
                break;
            };
            // A flag the instruction sets is the compare's to set again: only
            // the registers are `defined` against.
            let registers = writes.minus(&flags);
            let sets_flags = !writes.is_disjoint(&flags);
            if !writes.is_disjoint(&used)
                || !registers.is_disjoint(&defined)
                || !reads.is_disjoint(&defined)
                || (sets_flags
                    && !(flags_dead_after(regs, bits, &first, at) && flags_dead_after(regs, bits, &second, by)))
            {
                break;
            }
            run.push((at, by));
        }
        if run.is_empty() {
            return false;
        }
        for (at, by) in &run {
            let (kept, dropped) = (&first.insns[*at], &second.insns[*by]);
            let pairs = dropped.defines.iter().zip(&kept.defines).chain(dropped.uses.iter().zip(&kept.uses));
            for (from, to) in pairs.filter(|(from, to)| from != to) {
                let (from, to) = (super::_root(&self.rename, *from), super::_root(&self.rename, *to));
                if from != to {
                    self.rename.insert(from, to);
                }
            }
        }
        let mut insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
        for (place, (at, by)) in run.iter().enumerate() {
            // One instruction for both ways: the line they share (L2251).
            let (kept, other) = (&first.insns[*at], &second.insns[*by]);
            let line = lir::merged_line(kept.line, other.line);
            let hoisted =
                if line == kept.line { Arc::clone(kept) } else { Arc::new(Insn { line, ..(**kept).clone() }) };
            insns.insert(from + place, hoisted);
        }
        self.set(index, block.with_insns(insns).into_inner());
        for (place, leads) in
            [(one, run.iter().map(|(at, _)| *at).collect::<Vec<_>>()), (two, run.iter().map(|(_, by)| *by).collect())]
        {
            let gone = self.blocks[place].clone();
            let insns: Vec<Arc<Insn>> = gone
                .insns
                .iter()
                .enumerate()
                .filter(|(position, _)| !leads.contains(position))
                .map(|(_, insn)| Arc::clone(insn))
                .collect();
            self.set(place, gone.with_insns(insns).into_inner());
        }
        true
    }

    /// `block` stays; every block that enters `index` goes to `onward` instead.
    fn bypass(
        &mut self,
        index: usize,
        onward: i64,
    ) -> bool {
        let at = self.blocks[index].at;
        let entering: Vec<i64> = self.preds[&at].iter().copied().filter(|from| *from != at).collect();
        for from in entering {
            let place = self.pos[&from];
            // A block that runs into this one gets the jump it ran on by.
            if self.after(place) == Some(at) && runs_off(&self.blocks[place]) {
                self.made_explicit(place, at);
            }
            self.retarget(place, at, onward);
        }
        true
    }

    /// The block at `index`, which runs off its end into `to`, jumps there.
    fn made_explicit(
        &mut self,
        index: usize,
        to: i64,
    ) {
        let block = self.blocks[index].clone();
        if block.insns.last().is_none() {
            return;
        }
        let at = block.insns.last().map_or(block.at, |last| last.at);
        let jump = Insn::new(
            at,
            Some((at, at)),
            Some(Semantics { name: Some("jmp".into()), target: Some(to), ..Semantics::new(Operation::Jump) }),
            Vec::new(),
            Vec::new(),
        );
        let insns = block.insns.iter().cloned().chain([Arc::new(jump)]).collect();
        self.set(index, block.with_insns(insns).into_inner());
    }

    /// The branches of the block at `index` that go to `old` go to `new`.
    fn retarget(
        &mut self,
        index: usize,
        old: i64,
        new: i64,
    ) {
        let block = self.blocks[index].clone();
        self.odds.rerouted(block.at, &block.succ, old, &[(new, 1.0)]);
        let moved = |at: i64| if at == old { new } else { at };
        let insns = block
            .insns
            .iter()
            .map(|one| match one.what.as_ref() {
                Some(what)
                    if matches!(what.op, Operation::Branch | Operation::Jump)
                        && (what.target == Some(old) || what.table.contains(&old)) =>
                {
                    let (target, table) = (what.target.map(moved), what.table.iter().copied().map(moved).collect());
                    Arc::new(Insn { what: Some(Semantics { target, table, ..what.clone() }), ..(**one).clone() })
                }
                _ => Arc::clone(one),
            })
            .collect();
        let succ: IndexSet<i64> = block.succ.iter().copied().map(moved).collect();
        self.preds.entry(old).or_default().remove(&block.at);
        self.preds.entry(new).or_default().insert(block.at);
        self.set(index, LirBlock { succ: succ.into_iter().collect(), ..block.with_insns(insns).into_inner() });
    }

    fn set(
        &mut self,
        index: usize,
        block: LirBlock,
    ) {
        self.blocks[index] = block.into();
    }
}
