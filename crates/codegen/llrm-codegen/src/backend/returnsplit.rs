//! Gives a return that reloads a spilled constant its own block on the edges
//! that stored the constant, where that lets the frame be set up later.
//!
//! # How it works
//! A return block `mov r, [cell]; ret` shared by an early path and the body
//! keeps the early path in the frame: the path stores its constant to the cell
//! (`mov [cell], 1; cmp; je ret`) and the shared return reloads it. For each
//! edge into the block whose source ends in such a store, with nothing between
//! the store and the end that touches memory or a call, the edge goes to a copy
//! `mov r, imm; ret` and the store goes when no path from the source reads
//! the cell before writing it. The edges are kept only when shrink wrapping
//! then sets the frame up after the source (`shrinkwrap::wrapped`): a copy of a
//! return whose epilogue still runs is a size cost with nothing skipped.
//!
//! # Example
//! `place` of `bench/queens/queens.c` at m16 -O2 (abridged; the left column is
//! `LLRM_NO_RETSPLIT=1`):
//!
//! ```text
//!  without                           with
//!  mov word ptr [bp-6], 1            cmp dx, cx
//!  cmp dx, cx                        je early
//!  je ret                            ...
//!  ...                               early:
//!  ret: mov ax, [bp-6]                 mov ax, 1
//!       ...                            ret
//! ```
//!
//! The store of 1 and the reload are gone, and the frame is set up after the
//! test.
//!
//! # Runs
//! In `assemble`, ahead of `jumps::duplicated_returns`, at -O1 and above.
//!
//! # Deviations from the pin
//! - DEVIATE RS1: no counterpart. The pin gets the early path from
//!   `ShrinkWrap::tryToSplitRestore` and tail duplication after PEI, and its
//!   constant stays in `eax` through the phi lowering instead of a cell. Exit:
//!   the allocator keeps the constant out of a cell.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::backend::{jumps, masm};
use crate::model::ir::{Imm, Loc, Operation, Semantics};
use crate::model::lir::{Block, Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;

/// `procedure` with the returns split that let shrink wrapping skip the frame.
pub fn split(procedure: masm::Procedure) -> masm::Procedure {
    if procedure.size || crate::support::env_flag!("LLRM_NO_RETSPLIT") {
        return procedure;
    }
    let Some(body) = split_body(&procedure) else { return procedure };
    masm::Procedure { body, ..procedure }
}

/// Where the frame is set up in `body`, if it is set up later than the entry.
fn save_of(
    procedure: &masm::Procedure,
    body: &LirBody,
) -> Option<i64> {
    [true, false]
        .into_iter()
        .find_map(
            |frame| {
                let wrap = masm::wrap_of(procedure, body, frame, false)?;
                wrap.opens.keys().next().copied()
            },
        )
}

/// An edge to split: the block it leaves, the store to drop, the constant.
struct Edge {
    from: i64,
    store: usize,
    value: i64,
}

fn split_body(procedure: &masm::Procedure) -> Option<LirBody> {
    let body = &procedure.body;
    if body.blocks.len() < 3 {
        return None;
    }
    let by_at: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let predecessors = jumps::_predecessors(&body.blocks);
    let base = save_of(procedure, body);
    if base.is_some_and(|at| at != body.entry) {
        return None;
    }
    for tail in &body.blocks {
        let Some((load, cell)) = reload_return(tail) else { continue };
        let parents = predecessors.get(&tail.at)?;
        if parents.len() < 2 || tail.at == body.entry {
            continue;
        }
        let live = cell_live_in(body, &cell);
        let mut edges = Vec::new();
        for &at in parents {
            let Some(parent) = by_at.get(&at) else { continue };
            if let Some(edge) = stored_constant(parent, tail.at, &cell, &live) {
                edges.push(edge);
            }
        }
        if edges.is_empty() {
            continue;
        }
        let all = rebuilt(body, tail, &load, &edges);
        let Some(save) = save_of(procedure, &all) else { continue };
        if save == body.entry {
            continue;
        }
        // Only the edges from before the frame; one the frame reaches keeps its
        // epilogue, and a copy of it is bytes.
        let after = reached(&all, save);
        let early: Vec<Edge> = edges.into_iter().filter(|edge| !after.contains(&edge.from)).collect();
        if early.is_empty() {
            continue;
        }
        let made = rebuilt(body, tail, &load, &early);
        if save_of(procedure, &made).is_some_and(|at| at != body.entry) {
            return Some(made);
        }
    }
    None
}

/// The load and its cell, where `block` is `mov r, [cell]; ret` and no more.
fn reload_return(block: &LirBlock) -> Option<(Arc<Insn>, crate::model::ir::Mem)> {
    let real = jumps::_real(block);
    let [load, ret] = real.as_slice() else { return None };
    if !block.phis.is_empty()
        || !block.succ.is_empty()
        || ret.what.as_ref()?.op != Operation::Return
        || ret.what.as_ref()?.sources.len() > 0 && !matches!(ret.what.as_ref()?.sources.first(), Some(Loc::Imm(_)))
        || !load.uses.is_empty()
        || load.group.is_some()
        || load.symbol == Some(true)
    {
        return None;
    }
    let what = load.what.as_ref()?;
    let (Some(Loc::Reg(_)), Some(Loc::Mem(cell))) = (what.dests.first(), what.sources.first()) else { return None };
    (what.op == Operation::Move && what.dests.len() == 1 && what.sources.len() == 1 && cell.in_frame())
        .then(|| (Arc::clone(load), cell.clone()))
}

/// The edge from `parent` into `tail` when `parent` ends with `mov [cell], imm`
/// that the path to `tail` is the only reader of.
fn stored_constant(
    parent: &LirBlock,
    tail: i64,
    cell: &crate::model::ir::Mem,
    live: &IndexMap<i64, bool>,
) -> Option<Edge> {
    let real: Vec<(usize, &Arc<Insn>)> = parent.insns.iter().enumerate().filter(|(_, one)| !one.is_meta()).collect();
    let (last, _) = real.last()?;
    let exit = parent.insns[*last].what.as_ref()?;
    if !matches!(exit.op, Operation::Branch | Operation::Jump) || exit.target != Some(tail) {
        return None;
    }
    let ending = parent.insns[*last].as_ref();
    if !ending.inserted()
        || ending.symbol == Some(true)
        || ending.group.is_some()
        || !ending.spread.is_empty()
        || exit.indirect
    {
        return None;
    }
    for &(index, one) in real.iter().rev().skip(1) {
        let what = one.what.as_ref()?;
        if let (Operation::Move, [Loc::Mem(dest)], [Loc::Imm(imm)]) =
            (what.op, what.dests.as_slice(), what.sources.as_slice())
            && dest.same_place(cell)
            && one.spill_store
        {
            // Dead once the edge leaves: no other way out reads it first.
            let others = parent.succ.iter().filter(|&&next| next != tail);
            return others
                .into_iter()
                .all(|next| !live.get(next).copied().unwrap_or(true))
                .then_some(
                    Edge {
                        from: parent.at,
                        store: index,
                        value: imm.value,
                    },
                );
        }
        if what.op == Operation::Compare {
            continue;
        }
        let touches =
            what.dests.iter().chain(&what.sources).any(|place| matches!(place, Loc::Mem(_) | Loc::Address(_)));
        if touches
            || matches!(
                what.op,
                Operation::Call | Operation::Barrier | Operation::Data | Operation::Push | Operation::Pop
            )
        {
            return None;
        }
    }
    None
}

/// Per block, whether `cell` is read before it is written from the start of the
/// block.
fn cell_live_in(
    body: &LirBody,
    cell: &crate::model::ir::Mem,
) -> IndexMap<i64, bool> {
    let mut live: IndexMap<i64, bool> = body.blocks.iter().map(|block| (block.at, false)).collect();
    loop {
        let mut changed = false;
        for block in body.blocks.iter().rev() {
            let mut now = block.succ.iter().any(|next| live.get(next).copied().unwrap_or(true));
            for one in block.insns.iter().rev() {
                let Some(what) = &one.what else { continue };
                let writes = what.op == Operation::Move
                    && matches!(
                        what.dests.as_slice(),
                        [Loc::Mem(dest)] if dest.same_place(cell)
                    );
                let reads = what.sources.iter().any(|place| overlaps(place, cell))
                    || (!writes && what.dests.iter().any(|place| overlaps(place, cell)))
                    || matches!(
                        what.op,
                        Operation::Call | Operation::Escape | Operation::Barrier
                    );
                if writes {
                    now = false;
                }
                if reads {
                    now = true;
                }
            }
            if live[&block.at] != now {
                live.insert(block.at, now);
                changed = true;
            }
        }
        if !changed {
            return live;
        }
    }
}

fn overlaps(
    place: &Loc,
    cell: &crate::model::ir::Mem,
) -> bool {
    match place {
        Loc::Mem(other) if other.in_frame() => match (other.addr, cell.addr) {
            (Some(a), Some(b)) => a.disp < b.disp + cell.width as i64 && b.disp < a.disp + other.width as i64,
            _ => true,
        },
        Loc::Address(address) => address.in_frame(),
        _ => false,
    }
}

fn reached(
    body: &LirBody,
    from: i64,
) -> BTreeSet<i64> {
    let by_at: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let (mut seen, mut work) = (BTreeSet::from([from]), vec![from]);
    while let Some(at) = work.pop() {
        for &next in by_at.get(&at).map(|block| block.succ.iter()).into_iter().flatten() {
            if seen.insert(next) {
                work.push(next);
            }
        }
    }
    seen
}

/// `body` with each edge sent to a block of its own, after `tail`.
fn rebuilt(
    body: &LirBody,
    tail: &LirBlock,
    load: &Arc<Insn>,
    edges: &[Edge],
) -> LirBody {
    let mut next = body.blocks.iter().map(|block| block.at).max().unwrap_or(0) + 1;
    let ret = tail.insns.iter().rev().find(|one| !one.is_meta()).expect("a return");
    let mut made: Vec<Block> = Vec::new();
    let mut extra: Vec<Block> = Vec::new();
    for block in &body.blocks {
        let Some(edge) = edges.iter().find(|edge| edge.from == block.at) else {
            made.push(block.clone());
            continue;
        };
        let at = next;
        next += 1;
        let anchor = block.insns.last().map_or(block.at, |last| last.at);
        let insns: Vec<Arc<Insn>> = block
            .insns
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != edge.store)
            .map(|(_, one)| {
                let what = one.what.as_ref();
                match what {
                    Some(what)
                        if matches!(what.op, Operation::Branch | Operation::Jump) && what.target == Some(tail.at) =>
                    {
                        Arc::new(Insn { what: Some(Semantics { target: Some(at), ..what.clone() }), ..(**one).clone() })
                    }
                    _ => Arc::clone(one),
                }
            })
            .collect();
        let width = load
            .what
            .as_ref()
            .and_then(
                |what| match what.dests.first() {
                    Some(Loc::Reg(reg)) => Some(reg.width),
                    _ => None,
                },
            );
        let what = load.what.as_ref().expect("a load");
        let value = Loc::Imm(Imm { value: edge.value, width: width.unwrap_or(2), address: None });
        let mov = Arc::new(Insn {
            at: anchor,
            covers: Some((anchor, anchor)),
            what: Some(Semantics { sources: vec![value], ..what.clone() }),
            spill_reload: false,
            ..(**load).clone()
        });
        let ret = Arc::new(Insn { at: anchor, covers: Some((anchor, anchor)), spread: Vec::new(), ..(**ret).clone() });
        let mut changed = LirBlock {
            succ: block.succ.iter().map(|&one| if one == tail.at { at } else { one }).collect(),
            ..block.with_insns(insns).into_inner()
        };
        changed.succ = changed.succ.iter().copied().collect();
        made.push(changed.into());
        extra.push(LirBlock::new(at, vec![mov, ret]).into());
    }
    // After the tail, which no block falls out of: a block that fell into the
    // tail must not fall into a copy.
    let place = made.iter().position(|block| block.at == tail.at).map_or(made.len(), |index| index + 1);
    made.splice(place..place, extra);
    body.with_blocks(made)
}
