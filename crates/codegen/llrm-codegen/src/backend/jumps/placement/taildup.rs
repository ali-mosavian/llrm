//! Tail duplication in placement: `maybeTailDuplicateBlock`,
//! `repeatedlyTailDuplicateBlock`, `canTailDuplicateUnplacedPreds`,
//! `isProfitableToTailDup` and `precomputeTriangleChains` of
//! `MachineBlockPlacement.cpp`, with the layout-mode part of
//! `TailDuplicator.cpp` (`shouldTailDuplicate`, `canTailDuplicate`,
//! `tailDuplicate`) after register allocation.

use std::sync::Arc;

use super::{Placer, TAIL_DUPLICATION_PENALTY, TRIANGLE_CHAIN_COUNT};
use crate::backend::jumps::{_real, _unowned};
use crate::model::ir::Operation;
use crate::model::lir::{self, Insn, LirBlock};
use crate::support::hash::IndexMap;

impl Placer {
    /// The instructions `block` would be copied as, and whether it may be.
    /// Not copied: a block with a phi, a call's record, a group, a symbol or
    /// bytes spread over others (a copy claims no source bytes of its own), or
    /// one of data, a barrier, a call, an indirect jump or a sibling call (its
    /// callee is named by the instruction's id, which a copy does not keep).
    fn copied(
        &self,
        block: usize,
    ) -> Option<Vec<Arc<Insn>>> {
        let made = &self.blocks[block];
        if !made.phis.is_empty()
            || made.insns.iter().any(|one| {
                one.call.is_some() || one.group.is_some() || one.symbol == Some(true) || !one.spread.is_empty()
            })
        {
            return None;
        }
        let real = _real(made);
        let plain = real
            .iter()
            .all(
                |one| one.what
                    .as_ref()
                    .is_some_and(
                        |what| !matches!(
                            what.op,
                            Operation::Barrier | Operation::Data | Operation::Call
                        )
                            && !(what.op == Operation::Return && what.name.as_deref() == Some("jmp"))
                            && !(matches!(what.op, Operation::Jump | Operation::Branch) && what.indirect),
                    ),
            );
        plain.then_some(real)
    }

    /// The next block that is still there, in the layout the function had.
    fn next_alive(
        &self,
        block: usize,
    ) -> Option<usize> {
        (block + 1..self.blocks.len()).find(|&one| !self.dead[one])
    }

    /// `TailDuplicator::shouldTailDuplicate` in layout mode, behind the
    /// placement's `shouldTailDuplicate`: a block that goes one way is never
    /// copied. The jump to the block next in the layout is no instruction,
    /// as the fall-through it stands for is none.
    pub(super) fn should_tail_duplicate(
        &self,
        block: usize,
    ) -> bool {
        // LLVM's entry has no predecessor to take a copy; this one may.
        if self.dead[block] || block == self.entry || self.succ[block].len() == 1 || self.succ[block].contains(&block) {
            return false;
        }
        let Some(real) = self.copied(block) else { return false };
        let next = self.next_alive(block).map(|one| self.at(one));
        // `jcc next; jmp other` is `jncc other` and a fall-through: the form
        // LLVM has once the branches are folded, which placement follows.
        let mut count = real.len();
        let ops: Vec<_> = real.iter().filter_map(|one| one.what.as_ref()).collect();
        if let Some(last) = ops.last()
            && last.op == Operation::Jump
            && last.target.is_some()
        {
            let branch = ops.len().checked_sub(2).map(|before| ops[before]);
            if last.target == next
                || branch.is_some_and(|branch| branch.op == Operation::Branch && branch.target == next)
            {
                count -= 1;
            }
        }
        count <= self.dup_size
    }

    fn at(
        &self,
        block: usize,
    ) -> i64 {
        self.blocks[block].at
    }

    /// `TailDuplicator::canTailDuplicate`: `pred` goes to `tail` and nowhere
    /// else, by a plain jump.
    pub(super) fn can_tail_duplicate(
        &self,
        tail: usize,
        pred: usize,
    ) -> bool {
        if pred == tail || self.succ[pred].len() != 1 {
            return false;
        }
        let real = _real(&self.blocks[pred]);
        let ends_in_jump = real
            .last()
            .and_then(|one| one.what.as_ref())
            .is_some_and(|what| what.op == Operation::Jump && !what.indirect && what.target == Some(self.at(tail)));
        ends_in_jump && real.iter().filter_map(|one| one.what.as_ref()).all(|what| what.op != Operation::Branch)
    }

    /// `canTailDuplicateUnplacedPreds`.
    pub(super) fn can_tail_duplicate_unplaced_preds(
        &self,
        block: usize,
        to: usize,
        chain: usize,
    ) -> bool {
        if !self.should_tail_duplicate(to) {
            return false;
        }
        let (mut duplicate, mut count) = (true, 0_usize);
        let successors = &self.succ[block];
        for &from in &self.pred[to] {
            if from == block || !self.allowed(from) || (self.chain_of[from] == chain && !self.succ[to].is_empty()) {
                continue;
            }
            if !self.can_tail_duplicate(to, from) {
                if successors.len() > 1 && self.has_same_successors(from, successors) {
                    continue;
                }
                duplicate = false;
                continue;
            }
            count += 1;
        }
        if count == 0 {
            return false;
        }
        if self.succ[to].is_empty() {
            return true;
        }
        count += 1;
        !(count > self.succ[to].len() || !duplicate)
    }

    /// `greaterWithBias`.
    fn greater_with_bias(
        &self,
        a: f64,
        b: f64,
    ) -> bool {
        (a - b).max(0.0) / TAIL_DUPLICATION_PENALTY >= self.freq[self.entry]
    }

    /// `isProfitableToTailDup`: whether copying `to` into its predecessors
    /// saves more taken branches than it costs, with `block` laid out before
    /// it.
    pub(super) fn profitable_to_tail_duplicate(
        &self,
        block: usize,
        to: usize,
        q_prob: f64,
        chain: usize,
    ) -> bool {
        let (successors, left) = self.viable(to, chain);
        let p_prob = self.prob(block, to);
        let (block_freq, to_freq) = (self.freq[block], self.freq[to]);
        let p = block_freq * p_prob;
        let q_out = block_freq * q_prob;
        if successors.is_empty() {
            return self.greater_with_bias(p, q_out);
        }
        let (mut best_onward, mut post) = (0.0_f64, None);
        for &onward in &successors {
            let probability = self.prob(to, onward);
            best_onward = best_onward.max(probability);
            if post.is_none() && self.postdom.dominates(onward, to) {
                post = Some(onward);
                break;
            }
        }
        let mut q_in = 0.0_f64;
        for &from in &self.pred[to] {
            if from == to || from == block || self.chain_of[from] == chain || !self.allowed(from) {
                continue;
            }
            q_in = q_in.max(self.freq[from] * self.prob(from, to));
        }
        let free = (to_freq - q_in).max(0.0);
        let Some(post) = post.filter(|&one| self.succ[to].contains(&one)) else {
            let u_prob = best_onward;
            let v_prob = (left - u_prob).max(0.0);
            let v = to_freq * v_prob;
            let base = p + v;
            let dup = q_out + q_in.min(free) * u_prob + q_in.max(free) * v_prob;
            return self.greater_with_bias(base, dup);
        };
        let u_prob = self.prob(to, post);
        let v_prob = (left - u_prob).max(0.0);
        let (u, v) = (to_freq * u_prob, to_freq * v_prob);
        if u_prob > left / 2.0 && !self.has_better_layout_predecessor(to, post, u_prob, chain) {
            return self.greater_with_bias(p + v, q_out + q_in.max(free) * v_prob + q_in.min(free) * u_prob);
        }
        self.greater_with_bias(p + u, q_out + q_in.min(free) * left + q_in.max(free) * u_prob)
    }

    /// `repeatedlyTailDuplicateBlock`: `to` copied into its predecessors, and
    /// the block last in the chain, if it took a copy, copied in turn. The
    /// block now last in the chain.
    pub(super) fn repeatedly_tail_duplicate(
        &mut self,
        to: usize,
        last: usize,
        header: usize,
        chain: usize,
    ) -> usize {
        let (removed, mut into_last) = self.maybe_tail_duplicate(to, last, chain);
        if !removed {
            return last;
        }
        let original = into_last;
        let mut keep_going = removed;
        while into_last && keep_going {
            let blocks = &self.chains[chain].blocks;
            let Some((&copy, before)) = blocks.split_last() else { break };
            let Some(&pred) = before.last() else { break };
            (keep_going, into_last) = self.maybe_tail_duplicate(copy, pred, chain);
        }
        let last = self.tail(chain);
        if original {
            self.mark_block_successors(chain, last, header);
        }
        last
    }

    /// `maybeTailDuplicateBlock` with `TailDuplicator::tailDuplicateAndUpdate`:
    /// whether `to` was removed, and whether it was copied into `last`.
    fn maybe_tail_duplicate(
        &mut self,
        to: usize,
        last: usize,
        chain: usize,
    ) -> (bool, bool) {
        if !self.should_tail_duplicate(to) {
            return (false, false);
        }
        let mut done: Vec<usize> = Vec::new();
        for from in self.pred[to].clone() {
            if !self.can_tail_duplicate(to, from) || from == last {
                continue;
            }
            self.copy_into(from, to);
            done.push(from);
        }
        // The block before it in the chain takes it whole, if it is all that
        // is left to reach it.
        if self.succ[last] == [to] && self.pred[to] == [last] && self.can_tail_duplicate(to, last) {
            self.copy_into(last, to);
            done.push(last);
        }
        if done.is_empty() {
            return (false, false);
        }
        let removed = self.pred[to].is_empty();
        if removed {
            self.remove(to);
        }
        let mut into_last = false;
        for &from in &done {
            let from_chain = self.chain_of[from];
            if from == last {
                into_last = true;
            }
            if from == last || !self.allowed(from) || from_chain == chain {
                continue;
            }
            for next in self.succ[from].clone() {
                if !self.allowed(next) {
                    continue;
                }
                let next_chain = self.chain_of[next];
                if next_chain != chain && next_chain != from_chain {
                    self.chains[next_chain].unscheduled += 1;
                }
            }
        }
        (removed, into_last)
    }

    /// `from`, which only jumps to `to`, with `to`'s instructions where the
    /// jump was, and `to`'s way on.
    fn copy_into(
        &mut self,
        from: usize,
        to: usize,
    ) {
        let copied = self.copied(to).expect("checked by shouldTailDuplicate");
        let parent = &self.blocks[from];
        let real = _real(parent);
        let jump = Arc::clone(real.last().expect("a jump"));
        let mut insns: Vec<Arc<Insn>> = parent
            .insns
            .iter()
            .map(|one| if Arc::ptr_eq(one, &jump) { lir::bytes_only(one) } else { Arc::clone(one) })
            .collect();
        insns.extend(copied.iter().map(|one| _unowned(one, &jump)));
        let onward: Vec<(i64, f64)> =
            self.succ[to].iter().enumerate().map(|(place, &next)| (self.at(next), self.chance[to][place])).collect();
        self.rerouted.push((
            self.at(from),
            self.succ[from].iter().map(|&one| self.at(one)).collect(),
            self.at(to),
            onward,
        ));
        let succ: Vec<i64> = self.succ[to].iter().map(|&one| self.at(one)).collect();
        self.blocks[from] = LirBlock { succ: succ.into(), ..parent.with_insns(insns).into_inner() }.into();
        self.pred[to].retain(|&one| one != from);
        self.edge.swap_remove(&(from, to));
        self.succ[from] = self.succ[to].clone();
        self.chance[from] = self.chance[to].clone();
        for place in 0..self.succ[from].len() {
            let next = self.succ[from][place];
            self.edge.insert((from, next), self.chance[from][place]);
            self.pred[next].push(from);
        }
    }

    /// `removeDeadBlock` and the callback placement gives it.
    fn remove(
        &mut self,
        block: usize,
    ) {
        self.dead[block] = true;
        let chain = self.chain_of[block];
        self.chains[chain].blocks.retain(|&one| one != block);
        self.work.retain(|&one| one != block);
        if let Some(place) = self.members.iter().position(|&one| one == block) {
            self.members.remove(place);
            if place < self.next_member {
                self.next_member -= 1;
            }
            self.stamp[block] = 0;
        }
        if self.exit == Some(block) {
            self.exit = None;
        }
        for next in std::mem::take(&mut self.succ[block]) {
            self.pred[next].retain(|&one| one != block);
            self.edge.swap_remove(&(block, next));
        }
        self.chance[block].clear();
    }

    /// `precomputeTriangleChains`: runs of blocks each with a post-dominating
    /// successor that may be copied, laid out along those successors.
    pub(super) fn precompute_triangle_chains(&mut self) {
        let mut chains: IndexMap<usize, Vec<usize>> = IndexMap::default();
        for block in 0..self.succ.len() {
            if self.succ[block].len() != 2 {
                continue;
            }
            let Some(post) = self.succ[block].iter().copied().find(|&one| self.postdom.dominates(one, block)) else {
                continue;
            };
            if self.prob(block, post) < 0.5 || !self.should_tail_duplicate(post) {
                continue;
            }
            if self.pred[post].iter().any(|&from| from != block && !self.can_tail_duplicate(post, from)) {
                continue;
            }
            if let Some(mut found) = chains.swap_remove(&block) {
                found.push(post);
                chains.insert(post, found);
            } else {
                chains.entry(post).or_insert_with(|| vec![block, post]);
            }
        }
        for (_, edges) in chains {
            if edges.len() - 1 < TRIANGLE_CHAIN_COUNT {
                continue;
            }
            let mut to = *edges.last().expect("two blocks");
            for &from in edges[..edges.len() - 1].iter().rev() {
                self.computed.entry(from).or_insert((to, true));
                to = from;
            }
        }
    }
}
