//! Drops a spill store no later read of its slot can see. `unread_spill_stores`
//! drops the stores to a slot nothing reads; a slot several values share is
//! read, yet a store to it can still be overwritten on every path before the
//! next read:
//!
//! ```text
//! L0:  mov [bp-6],dx      ; dead: L1 stores [bp-6] before it reads it
//! L1:  mov [bp-6],si
//!      mov ax,[bp-6]
//! ```
//!
//! Backward liveness of the slots over the successors; a plain store kills its
//! slot, any other operand naming it reads it. Only a slot the allocator owns
//! (`spill_homes`) whose every access is the same cell is judged: a slot
//! another cell overlaps, or that a pointer can reach, stays.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::backend::peephole::{frame_home, plain_spill_store, spill_homes};
use crate::model::ir::Loc;
use crate::model::lir::{self, Insn, LirBody};
use crate::support::hash::IndexMap;

/// `body` without the spill stores whose slot is overwritten on every path
/// before it is read.
pub fn dead_spill_stores(body: &LirBody) -> LirBody {
    let slots = judged(body);
    if slots.is_empty() {
        return body.clone();
    }
    let bit: IndexMap<i64, usize> = slots.iter().enumerate().map(|(at, home)| (*home, at)).collect();
    let words = bit.len().div_ceil(64);
    let succ: IndexMap<i64, &Vec<i64>> = body.blocks.iter().map(|block| (block.at, &*block.succ)).collect();
    let mut live_in: IndexMap<i64, Vec<u64>> = body.blocks.iter().map(|block| (block.at, vec![0; words])).collect();
    let after = |live_in: &IndexMap<i64, Vec<u64>>, at: i64| {
        let mut out = vec![0u64; words];
        for to in succ[&at] {
            if let Some(more) = live_in.get(to) {
                out.iter_mut().zip(more).for_each(|(word, other)| *word |= other);
            }
        }
        out
    };
    loop {
        let mut changed = false;
        for block in body.blocks.iter().rev() {
            let mut live = after(&live_in, block.at);
            walk(&block.insns, &bit, &mut live, &mut |_| {});
            if live != live_in[&block.at] {
                live_in.insert(block.at, live);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut dead: BTreeSet<*const Insn> = BTreeSet::new();
    for block in &body.blocks {
        let mut live = after(&live_in, block.at);
        let list: Vec<&Arc<Insn>> = block.insns.iter().collect();
        walk(&block.insns, &bit, &mut live, &mut |at| {
            dead.insert(Arc::as_ptr(list[at]));
        });
    }
    if dead.is_empty() {
        return body.clone();
    }
    body.with_blocks(
        body.blocks
            .iter()
            .map(|block| {
                block.with_insns(lir::without(
                    &block.insns,
                    |one| dead.contains(&Arc::as_ptr(one)),
                    None::<fn(&Arc<Insn>) -> Arc<Insn>>,
                ))
            })
            .collect(),
    )
}

/// The slots to judge: the allocator's own, each reached by one cell only.
fn judged(body: &LirBody) -> Vec<i64> {
    let mut cells: IndexMap<i64, BTreeSet<(i64, u32)>> = IndexMap::default();
    let mut ranges: Vec<(i64, i64)> = Vec::new();
    let owned = spill_homes(body);
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        let Some(what) = one.what.as_ref() else { continue };
        for place in what.dests.iter().chain(&what.sources) {
            let Loc::Mem(cell) = place else { continue };
            let Some(addr) = cell.addr.filter(|addr| addr.space == crate::objectfile::module::Space::Frame) else {
                continue;
            };
            ranges.push((addr.disp, addr.disp + i64::from(cell.width)));
            if let Some(home) = frame_home(place).filter(|home| owned.contains(home)) {
                cells.entry(home).or_default().insert((addr.disp, cell.width));
            }
        }
    }
    cells
        .into_iter()
        .filter(|(_, seen)| {
            let mut one = seen.iter();
            let (Some((disp, width)), None) = (one.next(), one.next()) else { return false };
            let (low, high) = (*disp, *disp + i64::from(*width));
            ranges.iter().filter(|(start, end)| *start < high && *end > low).all(|range| *range == (low, high))
        })
        .map(|(home, _)| home)
        .collect()
}

/// `live` before `insns`, given it after them; `dead` is told the index of
/// each store to a slot not live after it.
fn walk(
    insns: &[Arc<Insn>],
    bit: &IndexMap<i64, usize>,
    live: &mut [u64],
    dead: &mut dyn FnMut(usize),
) {
    for (at, one) in insns.iter().enumerate().rev() {
        let Some(what) = one.what.as_ref() else { continue };
        let set = |live: &mut [u64], on: bool, id: usize| {
            if on {
                live[id / 64] |= 1 << (id % 64);
            } else {
                live[id / 64] &= !(1 << (id % 64));
            }
        };
        let store = plain_spill_store(one).and_then(|home| bit.get(&home).copied());
        if let Some(id) = store {
            if live[id / 64] >> (id % 64) & 1 == 0 {
                dead(at);
            }
            set(live, false, id);
        }
        for (place_at, place) in what.dests.iter().chain(&what.sources).enumerate() {
            if store.is_some() && place_at == 0 {
                continue;
            }
            if let Some(id) = frame_home(place).and_then(|home| bit.get(&home).copied()) {
                set(live, true, id);
            }
        }
    }
}
