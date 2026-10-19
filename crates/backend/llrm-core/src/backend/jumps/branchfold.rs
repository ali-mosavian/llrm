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
//!    next becomes `j!cc M`.
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

/// `one` going to `target`, the condition made `name` if given.
fn aimed(
    one: &Arc<Insn>,
    target: i64,
    name: Option<&str>,
) -> Arc<Insn> {
    let what = one.what.as_ref().expect("a branch has semantics");
    let name = name.map(str::to_owned).or_else(|| what.name.clone());
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
}

/// `body` with its branches folded; `protected` blocks are never bypassed.
pub fn folded(
    body: &LirBody,
    protected: &BTreeSet<i64>,
) -> LirBody {
    let mut folder = Folder::of(body, protected);
    while folder.pass() {}
    let mut done = body.with_blocks(folder.blocks);
    done.odds = folder.odds;
    super::_prune(&mut done);
    done
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
        let exit = Exit::of(&block);
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
                what: Some(Semantics {
                    name: Some("jmp".to_owned()),
                    target: Some(to),
                    ..Semantics::new(Operation::Jump)
                }),
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
        false
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
            Some(Semantics { name: Some("jmp".to_owned()), target: Some(to), ..Semantics::new(Operation::Jump) }),
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
