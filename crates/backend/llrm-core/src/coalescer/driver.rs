//! `RegisterCoalescer::run`: the order copies are tried in, and trying again
//! until nothing joins. Translated from RegisterCoalescer.cpp
//! (`joinAllIntervals`, `copyCoalesceInMBB`, `copyCoalesceWorkList`,
//! `coalesceLocals`, `compareMBBPriority`, `isSplitEdge`, `isLocalCopy`) at
//! d1106deb71cc.
//!
//! # How it works
//! Blocks are taken deepest loop first, then the split edges (a block that is
//! only copies and a jump, between one predecessor and one successor), then the
//! better connected, then by number. In each block the copies are collected top
//! down; a copy of a value that lives in one block goes to the local list, the
//! others to the work list, and the work list's copies are tried at once. When
//! the loop depth drops, the local list is tried (`JoinGlobalCopies`, which x86
//! enables). A copy that is `Deferred` stays for the next round; rounds repeat
//! while one makes progress, since a join can make another possible.
//!
//! DEVIATE C7 (no number): the terminal rule (`applyTerminalRule`) is not
//! translated: `-terminal-rule` is off by default at the pin. `ToBeUpdated`,
//! `lateLiveIntervalUpdate`, `InflateRegs` and the debug-info maps belong to
//! parts this coalescer lacks (DEVIATE C4, C1) or to debug info.
//!
//! # Example
//! `v2 := mov v1; v3 := mov v2; read v3`: both copies are tried in the one
//! block, both join, and one value is left.
//!
//! # Used by
//! Nothing yet.

use std::cmp::Ordering;

use super::pair::{CoalescerEnv, full_copy};
use super::register_coalescer::{JoinResult, State, join_copy};
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::slotindex::InstrKey;
use crate::model::ir::Operation;
use crate::support::hash::HashSet;

/// `MBBPriorityInfo`.
#[derive(Clone, Debug)]
pub struct MbbPriority {
    pub block: usize,
    pub depth: u32,
    pub is_split: bool,
    /// Predecessors and successors.
    pub connectivity: usize,
}

/// `compareMBBPriority`: deeper loops first, then split edges, then the better
/// connected, then the block number.
pub fn compare_mbb_priority(
    lhs: &MbbPriority,
    rhs: &MbbPriority,
) -> Ordering {
    rhs.depth
        .cmp(&lhs.depth)
        .then_with(|| rhs.is_split.cmp(&lhs.is_split))
        .then_with(|| rhs.connectivity.cmp(&lhs.connectivity))
        .then_with(|| lhs.block.cmp(&rhs.block))
}

/// `isSplitEdge`: a block to vacate to eliminate branches: one predecessor, one
/// successor, nothing but copies and an unconditional jump.
fn is_split_edge(
    lis: &LiveIntervals,
    block: usize,
) -> bool {
    if lis.cfg.preds[block].len() != 1 || lis.cfg.succs[block].len() != 1 {
        return false;
    }
    lis.block_insns(block)
        .iter()
        .all(|one| full_copy(one).is_some() || one.what.as_ref().is_some_and(|what| what.op == Operation::Jump))
}

/// `isLocalCopy`: the copy defines or uses a value that lives in one block.
fn is_local_copy(
    lis: &mut LiveIntervals,
    copy: InstrKey,
) -> bool {
    let Some((dst, src)) = full_copy(lis.instr(copy)) else { return false };
    let (src, dst) = (lis.interval_rc(src.value), lis.interval_rc(dst.value));
    lis.interval_is_in_one_mbb(&src).is_some() || lis.interval_is_in_one_mbb(&dst).is_some()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Stats {
    pub joined: usize,
    /// Copies still in the list when no round made progress.
    pub left: usize,
}

struct Coalescer<'a> {
    lis: &'a mut LiveIntervals,
    env: &'a dyn CoalescerEnv,
    state: State,
    work: Vec<Option<InstrKey>>,
    local: Vec<Option<InstrKey>>,
    joined: usize,
}

#[derive(Clone, Copy)]
enum List {
    Work,
    Local,
}

impl Coalescer<'_> {
    fn list(
        &mut self,
        which: List,
    ) -> &mut Vec<Option<InstrKey>> {
        match which {
            List::Work => &mut self.work,
            List::Local => &mut self.local,
        }
    }

    /// `copyCoalesceWorkList`: the copies of `list` from `from`; whether any
    /// joined.
    fn copy_coalesce_work_list(
        &mut self,
        which: List,
        from: usize,
    ) -> bool {
        let mut progress = false;
        let mut current_erased: HashSet<InstrKey> = HashSet::default();
        for at in from..self.list(which).len() {
            let Some(copy) = self.list(which)[at] else { continue };
            let copy = self.state.current(copy);
            // Skip instructions that have already been erased, for example by
            // dead code elimination.
            if self.state.erased.contains(&copy) || current_erased.contains(&copy) {
                self.list(which)[at] = None;
                continue;
            }
            let result = join_copy(self.lis, self.env, &mut self.state, &mut current_erased, copy);
            if result == JoinResult::Joined {
                progress = true;
                self.joined += 1;
            }
            if result != JoinResult::Deferred {
                self.list(which)[at] = None;
            }
        }
        // Clear instructions not recorded in `erased` but erased.
        if !current_erased.is_empty() {
            for slot in self.work.iter_mut().chain(self.local.iter_mut()) {
                if slot.is_some_and(|copy| current_erased.contains(&self.state.current(copy))) {
                    *slot = None;
                }
            }
        }
        progress
    }

    /// `copyCoalesceInMBB`.
    fn copy_coalesce_in_mbb(
        &mut self,
        block: usize,
    ) {
        // Collect all copies in MBB. Don't start coalescing anything yet, it
        // might invalidate the iterator. Top-down, to propagate coalescing
        // forward.
        let prev_size = self.work.len();
        let copies: Vec<InstrKey> =
            self.lis.block_insns(block).iter().filter(|one| full_copy(one).is_some()).map(|one| one.id()).collect();
        for copy in copies {
            if is_local_copy(self.lis, copy) {
                self.local.push(Some(copy));
            } else {
                self.work.push(Some(copy));
            }
        }
        // Try coalescing the collected copies immediately, and remove the
        // nulls. This prevents the work list from getting too large since most
        // copies are joinable on the first attempt.
        if self.copy_coalesce_work_list(List::Work, prev_size) {
            let mut at = prev_size;
            while at < self.work.len() {
                if self.work[at].is_none() {
                    self.work.remove(at);
                } else {
                    at += 1;
                }
            }
        }
    }

    /// `coalesceLocals`.
    fn coalesce_locals(&mut self) {
        self.copy_coalesce_work_list(List::Local, 0);
        let local = std::mem::take(&mut self.local);
        self.work.extend(local.into_iter().filter(Option::is_some));
    }
}

/// `joinAllIntervals`: tries every copy of the function, `depth[b]` being the
/// loop depth of block `b`.
pub fn run(
    lis: &mut LiveIntervals,
    env: &dyn CoalescerEnv,
    depth: &[u32],
) -> Stats {
    let mut order: Vec<MbbPriority> = (0..lis.cfg.len())
        .map(|block| MbbPriority {
            block,
            depth: depth[block],
            is_split: is_split_edge(lis, block),
            connectivity: lis.cfg.preds[block].len() + lis.cfg.succs[block].len(),
        })
        .collect();
    order.sort_by(compare_mbb_priority);
    let mut coalescer = Coalescer { lis, env, state: State::new(), work: Vec::new(), local: Vec::new(), joined: 0 };
    // Coalesce intervals in MBB priority order.
    let mut curr_depth = u32::MAX;
    for info in &order {
        // Try coalescing the collected local copies for deeper loops.
        if info.depth < curr_depth {
            coalescer.coalesce_locals();
            curr_depth = info.depth;
        }
        coalescer.copy_coalesce_in_mbb(info.block);
    }
    coalescer.coalesce_locals();
    // Joining intervals can allow other intervals to be joined. Iteratively
    // join until we make no progress.
    while coalescer.copy_coalesce_work_list(List::Work, 0) {}
    let left = coalescer.work.iter().flatten().count();
    Stats { joined: coalescer.joined, left }
}
