//! Walks from an instruction, as LLVM's iterators do:
//! `std::next(I->getIterator())` to `BB->end()`. They read `next_in_block`, so
//! they hold no copy of the block.

use crate::module::{BlockId, Function, InstId};

impl Function {
    /// The instructions that follow `inst` in its block, in order.
    pub fn iter_after(
        &self,
        inst: InstId,
    ) -> impl Iterator<Item = InstId> + '_ {
        std::iter::successors(self.next_in_block(inst), |&one| self.next_in_block(one))
    }

    /// `BasicBlock::rbegin` to `rend`: the instructions last first, by the
    /// links.
    pub fn iter_block_rev(
        &self,
        id: BlockId,
    ) -> impl Iterator<Item = InstId> + '_ {
        std::iter::successors(self.block(id).last(), |&one| self.prev_in_block(one))
    }

    /// `inst` and what follows it in its block: `inst->getIterator()` to
    /// `end()`.
    pub fn iter_from(
        &self,
        inst: InstId,
    ) -> impl Iterator<Item = InstId> + '_ {
        std::iter::successors(Some(inst), |&one| self.next_in_block(one))
    }

    /// `first` up to but not including `last`, both in one block, `first`
    /// not after `last`: `make_range(first->getIterator(),
    /// last->getIterator())`.
    pub fn iter_range(
        &self,
        first: InstId,
        last: InstId,
    ) -> impl Iterator<Item = InstId> + '_ {
        self.iter_from(first).take_while(move |&one| one != last)
    }
}
