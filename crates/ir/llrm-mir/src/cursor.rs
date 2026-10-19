//! Walks from an instruction, as LLVM's iterators do: `std::next(I->getIterator())`
//! to `BB->end()`. They read `next_in_block`, so they hold no copy of the block.

use crate::module::{Function, InstId};

impl Function {
    /// The instructions that follow `inst` in its block, in order.
    pub fn iter_after(
        &self,
        inst: InstId,
    ) -> impl Iterator<Item = InstId> + '_ {
        std::iter::successors(self.next_in_block(inst), |&one| self.next_in_block(one))
    }
}
