//! `WebAssemblyCFGSort`: the blocks in an order where every loop is contiguous,
//! every block comes after its predecessors (back edges apart) and a branch to
//! a block that is not next can be expressed as a `br` out of a `block` or up
//! to a `loop`.
//!
//! # How it works
//! Blocks are taken in order. Of the successors that become ready (all forward
//! predecessors placed), the lowest-numbered ones are preferred; a block the
//! open loop does not dominate waits until the loop is done. When the block
//! placed has a different layout successor than before, its terminators are
//! updated (`updateTerminator`).
//!
//! A region is a loop or an exception (`sort_region`); an EH pad sorts ahead of
//! every other block that is ready, so its exception follows the code it
//! guards.
//!
//! # See also
//! `WebAssemblyCFGSort.cpp` (`sortBlocks`), `WebAssemblySortRegion.cpp`, pin
//! d1106deb71cc.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use llrm_mir::module::BlockId;

use crate::branch::update_terminator;
use crate::cfg::Cfg;
use crate::exception_info::Exceptions;
use crate::mir::Function;
use crate::sort_region::{Region, Regions};

/// An open region, the blocks of it still to place, and the blocks that wait
/// for it.
struct Entry {
    region: Region,
    header: usize,
    blocks_left: usize,
    deferred: Vec<usize>,
}

/// `maybeUpdateTerminator`: only a block all of whose terminators are branches
/// changes.
fn maybe_update_terminator(
    f: &mut Function,
    block: usize,
    next: Option<usize>,
) {
    if !crate::branch::terminators(&f.blocks[block]).iter().all(|inst| inst.is_branch()) {
        return;
    }
    // The block that followed it before the sort: the one numbered after it.
    let original = (block + 1 < f.blocks.len()).then_some(block + 1);
    let succs = f.succs[block].clone();
    update_terminator(&mut f.blocks[block], &succs, next, original);
}

/// Whether anything changed.
pub fn cfg_sort(f: &mut Function) -> bool {
    let n = f.blocks.len();
    let cfg = Cfg::of(f);
    let (tree, loops) = cfg.analyses();
    let exceptions = Exceptions::of(f, &cfg, &tree);
    let regions = Regions { loops: &loops, exceptions: &exceptions };
    let block_id = |block: usize| BlockId(block as u32);
    let pad: Vec<bool> = (0..n).map(|block| f.is_eh_pad(block)).collect();

    let mut preds_left = vec![0_usize; n];
    for (block, left) in preds_left.iter_mut().enumerate() {
        let mut count = cfg.predecessors_of(block).count();
        if let Some(one) = loops.get_loop_for(block_id(block)).filter(|one| one.get_header() == block_id(block)) {
            count -= cfg.predecessors_of(block).filter(|&pred| one.contains(block_id(pred))).count();
        }
        *left = count;
    }

    // `Preferred` takes an EH pad before any other block, then the lowest
    // number; `Ready` an EH pad first, then the highest.
    let mut preferred: BinaryHeap<(bool, Reverse<usize>)> = BinaryHeap::new();
    let mut ready: BinaryHeap<(bool, usize)> = BinaryHeap::new();
    let mut entries: Vec<Entry> = Vec::new();
    let mut order = Vec::with_capacity(n);
    let mut current = 0;
    loop {
        order.push(current);
        let region = regions.region_for(current);
        if let Some(region) = region {
            if regions.header(region) == current {
                entries.push(Entry {
                    region,
                    header: current,
                    blocks_left: regions.num_blocks(region),
                    deferred: Vec::new(),
                });
            }
            for entry in &mut entries {
                if regions.contains(entry.region, current) {
                    entry.blocks_left -= 1;
                    if entry.blocks_left == 0 {
                        ready.extend(entry.deferred.iter().map(|&block| (pad[block], block)));
                    }
                }
            }
            while entries.last().is_some_and(|entry| entry.blocks_left == 0) {
                entries.pop();
            }
        }
        for &succ in &f.succs[current] {
            if let Some(succ_loop) = loops.get_loop_for(block_id(succ)) {
                if succ_loop.get_header() == block_id(succ) && succ_loop.contains(block_id(current)) {
                    continue;
                }
            }
            preds_left[succ] -= 1;
            if preds_left[succ] == 0 {
                preferred.push((pad[succ], Reverse(succ)));
            }
        }
        let mut next = None;
        while let Some((_, Reverse(candidate))) = preferred.pop() {
            if let Some(open) = entries.last_mut() {
                if !tree.dominates(block_id(open.header), block_id(candidate)) {
                    open.deferred.push(candidate);
                    continue;
                }
            }
            let inside_and_after_header = match region {
                Some(region) => !regions.contains(region, candidate) || regions.header(region) < candidate,
                None => true,
            };
            if candidate < current && !pad[candidate] && inside_and_after_header {
                ready.push((pad[candidate], candidate));
                continue;
            }
            next = Some(candidate);
            break;
        }
        if next.is_none() {
            let Some((_, top)) = ready.pop() else {
                maybe_update_terminator(f, current, None);
                break;
            };
            let mut candidate = top;
            loop {
                match entries.last_mut() {
                    Some(open) if !tree.dominates(block_id(open.header), block_id(candidate)) => {
                        open.deferred.push(candidate);
                        candidate = ready.pop().expect("a ready block remains while some wait on the loop").1;
                    }
                    _ => break,
                }
            }
            next = Some(candidate);
        }
        let next = next.expect("a next block");
        maybe_update_terminator(f, current, Some(next));
        current = next;
    }
    assert!(entries.is_empty(), "active sort region list not finished");
    let changed = order.iter().enumerate().any(|(new, &old)| new != old);
    f.apply_order(&order);
    changed
}
