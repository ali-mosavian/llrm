//! Reassociate's pair map: how often two operands are combined by the same
//! operation, so that the most popular pair is formed first and CSE finds it.
//!
//! Translation of `BuildPairMap` and the choice of the best pair in
//! `ReassociateExpression`
//! ([Reassociate.cpp:2669](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L2669),
//! [:2543](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L2543))
//! at d1106deb71cc.
//!
//! MIRROR: only the root of each tree is counted; a tree of more than
//! `GLOBAL_REASSOCIATE_LIMIT` leaves is skipped, and so is the choice for an
//! expression of more; with `reassociate-use-cse-local` (on) the reordering
//! stays among the leaves of the first block seen.
//!
//! # How it works
//! A hash map from (operation, operand, operand) to a count: one pass over the
//! function, at most 45 pairs per tree (the limit squared over two), so O(n).
//! The choice scans the pairs of one tree, also bounded by the limit.
//!
//! DEVIATE: a pair is ordered by value id, not by address; ids are never
//! reused, so a key cannot go stale (`PairMapValue::isValid`).
//!
//! # Used by
//! `crate::reassociate`, `crate::reassociate_inst`.
//!
//! # Example
//! In `a*b*c` and `a*b*d`, the pair `a*b` counts 2: `a*b` is formed first in
//! both and the second is a common subexpression.

use llrm_mir::module::{BlockId, Operand};
use llrm_mir::opcode::BinaryOp;
use llrm_support::hash::HashMap;

use crate::reassociate::{Reassociator, ValueEntry};

/// `GlobalReassociateLimit`: arbitrary, but it prevents quadratic behavior.
pub const GLOBAL_REASSOCIATE_LIMIT: usize = 10;

type Key = (u8, u64, u64);

/// A total order on operands, for the canonical order of a pair.
fn order_of(v: Operand) -> u64 {
    match v {
        Operand::Value(value) => u64::from(value.0) << 2,
        Operand::Constant(constant) => (constant.index() as u64) << 2 | 1,
        Operand::Block(block) => (u64::from(block.0) << 2) | 2,
    }
}

fn key(
    op: BinaryOp,
    a: Operand,
    b: Operand,
) -> Key {
    let (a, b) = (order_of(a), order_of(b));
    (op as u8, a.min(b), a.max(b))
}

#[derive(Default)]
pub struct Pairs {
    scores: HashMap<Key, u32>,
}

impl Pairs {
    /// `BuildPairMap` over `order`, the reachable blocks in reverse post-order.
    pub(crate) fn build(
        r: &Reassociator,
        order: &[BlockId],
    ) -> Self {
        let mut pairs = Self::default();
        for &block in order {
            for inst in r.function.iter_block(block) {
                if !r.is_associative(inst) {
                    continue;
                }
                let op = r.binop(inst).expect("a binary operator");
                // Ignore nodes that are not the root of a tree.
                let value = r.result(inst);
                if r.has_one_use(value)
                    && r.only_user(value)
                        .is_some_and(|user| r.function.instruction(user).opcode == r.function.instruction(inst).opcode)
                {
                    continue;
                }
                // Collect all operands of a single reassociable expression.
                let mut worklist = vec![r.operand(inst, 0), r.operand(inst, 1)];
                let mut ops: Vec<Operand> = Vec::new();
                while ops.len() <= GLOBAL_REASSOCIATE_LIMIT
                    && let Some(operand) = worklist.pop()
                {
                    let node = r
                        .inst_of(operand)
                        .filter(
                            |&n| r.function.instruction(n).opcode == r.function.instruction(inst).opcode
                                && r.has_one_use(operand),
                        );
                    let Some(node) = node else {
                        ops.push(operand);
                        continue;
                    };
                    // Paranoid about self-referencing expressions in
                    // unreachable code.
                    for index in 0..2 {
                        if r.operand(node, index) != operand {
                            worklist.push(r.operand(node, index));
                        }
                    }
                }
                // Skip extremely long expressions.
                if ops.len() > GLOBAL_REASSOCIATE_LIMIT {
                    continue;
                }
                // All pairwise combinations of operands, each once per tree.
                let mut seen: Vec<Key> = Vec::new();
                for i in 0..ops.len().saturating_sub(1) {
                    for j in i + 1..ops.len() {
                        let k = key(op, ops[i], ops[j]);
                        if seen.contains(&k) {
                            continue;
                        }
                        seen.push(k);
                        *pairs.scores.entry(k).or_insert(0) += 1;
                    }
                }
            }
        }
        pairs
    }

    fn score(
        &self,
        op: BinaryOp,
        a: Operand,
        b: Operand,
    ) -> u32 {
        self.scores.get(&key(op, a, b)).copied().unwrap_or(0)
    }
}

impl Reassociator<'_> {
    /// Move the pair of `ops` with the highest count to the back of the list,
    /// so that it is formed first and CSE finds it: `a*b*c*d*e` with `c*e` the
    /// most popular pair is `(((c*e)*d)*b)*a`.
    pub fn move_most_popular_pair_to_back(
        &self,
        root: llrm_mir::module::InstId,
        ops: &mut Vec<ValueEntry>,
    ) {
        let op = self.binop(root).expect("a binary operator");
        let mut limit_idx = 0;
        // With the CSE-driven heuristic two values would be put at the start of
        // the expression though they may live very late in the CFG. With the
        // CSE-local one, reordering is limited to the values of the first block
        // seen, so no dependence is formed from an unrelated part of the CFG
        // (one that would become loop dependent).
        let entry = self.function.entry();
        let mut first_seen: Option<Option<BlockId>> = None;
        for i in (0..ops.len() - 1).rev() {
            // A value that is no instruction is free of the CFG: as if in the
            // entry block, so that they stay together.
            let seen = match self.inst_of(ops[i].op) {
                Some(inst) => self.function.parent(inst),
                None => entry,
            };
            match first_seen {
                None => first_seen = Some(seen),
                Some(first) if first != seen => {
                    // In another block: the last value of the same one is next.
                    limit_idx = i + 1;
                    break;
                }
                Some(_) => {}
            }
        }
        let mut max = 1;
        let mut best_rank = 0;
        let mut best: Option<(usize, usize)> = None;
        for i in (limit_idx + 1..ops.len()).rev() {
            for j in (limit_idx..i).rev() {
                let score = self.pairs.score(op, ops[i].op, ops[j].op);
                let max_rank = ops[i].rank.max(ops[j].rank);
                // Of equal counts, the pair of the lowest rank: the one with
                // both operands first in the topological order.
                if score > max || (score == max && max_rank < best_rank) {
                    best = Some((j, i));
                    max = score;
                    best_rank = max_rank;
                }
            }
        }
        if max > 1
            && let Some((first, second)) = best
        {
            let (op0, op1) = (ops[first], ops[second]);
            ops.remove(second);
            ops.remove(first);
            ops.push(op0);
            ops.push(op1);
        }
    }
}
