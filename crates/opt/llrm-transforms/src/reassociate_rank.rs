//! Reassociate's ranks: how late in the function each value is computed.
//!
//! Translation of `ReassociatePass::BuildRankMap` and `getRank`
//! ([Reassociate.cpp:209](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L209),
//! [:233](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L233))
//! at d1106deb71cc.
//!
//! MIRROR: constants and globals are rank 0, arguments 3, 4, ..., each
//! reachable block (reverse post-order) takes `n << 16` and its instructions
//! that cannot move take that plus a count, and any other instruction is one
//! more than the highest rank among its operands, a `not`, `neg` or `fneg` not
//! counting. The walk of operands is an explicit stack, as upstream's, and
//! stops at the rank of the instruction's own block.
//!
//! # How it works
//! [`Ranks::build`] numbers arguments and pinned instructions once.
//! [`Ranks::of`] computes the rest on demand and memoizes it, so a value's rank
//! is worked out once per function: O(n) over the function.
//!
//! # Used by
//! `crate::reassociate`.
//!
//! # Example
//! `%a = add i32 %x, 1` with `%x` the first argument (rank 3) is rank 4.

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::valuetracking::{Query, pattern};

/// The rank of every value and block seen so far.
pub struct Ranks {
    blocks: IdMap<BlockId, u32>,
    values: IdMap<ValueId, u32>,
    /// Turns of the operand walk, for the slope test.
    steps: u64,
    /// Calls that missed the memo and built a query and a stack.
    walks: u64,
}

/// One suspended `getRank` call: the instruction, the operand it waits on and
/// the highest rank of the operands before it.
struct Frame {
    value: ValueId,
    next: usize,
    rank: u32,
}

impl Ranks {
    pub fn empty() -> Self {
        Self { blocks: IdMap::new(), values: IdMap::new(), steps: 0, walks: 0 }
    }

    #[cfg(test)]
    pub fn steps(&self) -> u64 {
        self.steps
    }

    #[cfg(test)]
    pub fn walks(&self) -> u64 {
        self.walks
    }

    /// `ValueRankMap.contains(V)`: `V` has an entry, as every value `getRank`
    /// has looked at does (upstream's `operator[]` inserts a zero).
    pub fn knows(
        &self,
        value: ValueId,
    ) -> bool {
        self.values.contains_key(&value)
    }

    /// `ValueRankMap.erase(V)`.
    pub fn forget(
        &mut self,
        value: ValueId,
    ) {
        self.values.remove(&value);
    }

    /// `BuildRankMap` over `order`, the reachable blocks in reverse post-order;
    /// `pinned` says which instructions cannot move
    /// (`mayHaveNonDefUseDependency`).
    pub fn build(
        function: &Function,
        order: &[BlockId],
        pinned: impl Fn(InstId) -> bool,
    ) -> Self {
        let mut ranks = Self::empty();
        let mut rank = 2_u32;
        for &parameter in function.parameters() {
            rank += 1;
            ranks.values.insert(parameter, rank);
        }
        for &block in order {
            rank += 1;
            let mut block_rank = rank << 16;
            ranks.blocks.insert(block, block_rank);
            // Every instruction that cannot move counts, one with no value
            // too, so that the ranks in the block differ.
            for inst in function.iter_block(block) {
                if pinned(inst) {
                    block_rank += 1;
                    if let Some(result) = function.instruction(inst).result {
                        ranks.values.insert(result, block_rank);
                    }
                }
            }
        }
        ranks
    }

    /// `getRank(V)`.
    pub fn of(
        &mut self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        v: Operand,
    ) -> u32 {
        let Operand::Value(root) = v else { return 0 };
        if let Some(&known) = self.values.get(&root)
            && known != 0
        {
            self.steps += 1;
            return known;
        }
        self.walks += 1;
        let query = Query::bare(context, function, layout);
        let mut stack = vec![Frame { value: root, next: 0, rank: 0 }];
        loop {
            self.steps += 1;
            let frame = stack.last().expect("a frame");
            let rank = match function.value(frame.value).def {
                ValueDef::Argument(_) => *self.values.get_or_insert_with(frame.value, || 0),
                ValueDef::Instruction(inst) => {
                    let known = *self.values.get_or_insert_with(frame.value, || 0);
                    if known != 0 {
                        known
                    } else {
                        let instruction = function.instruction(inst);
                        let block_rank = function.parent(inst).and_then(|b| self.blocks.get(&b)).copied().unwrap_or(0);
                        if frame.next == instruction.operands.len() || frame.rank == block_rank {
                            let own = instruction.result.expect("a value");
                            let operand = Operand::Value(own);
                            let transparent = pattern::not(&query, operand).is_some()
                                || pattern::neg(&query, operand).is_some()
                                || pattern::fneg(&query, operand).is_some();
                            let rank = frame.rank + u32::from(!transparent);
                            self.values.insert(frame.value, rank);
                            rank
                        } else {
                            let next = instruction.operands[frame.next];
                            match next {
                                Operand::Value(value) => stack.push(Frame { value, next: 0, rank: 0 }),
                                _ => stack.last_mut().expect("a frame").next += 1,
                            }
                            continue;
                        }
                    }
                }
            };
            stack.pop();
            let Some(parent) = stack.last_mut() else { return rank };
            parent.rank = parent.rank.max(rank);
            parent.next += 1;
        }
    }
}

#[cfg(test)]
#[path = "reassociate_rank_tests.rs"]
mod tests;
