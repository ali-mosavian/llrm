//! Keeps one frame cell where a value is copied from a cell into a spill cell.
//!
//! # How it works
//! `mov r, [A] ; mov [B], r` copies the cell `A` into the spill cell `B`. When
//! `B` is written by that store alone, and `A` is not written on any path from
//! the copy to a read of `B`, every read of `B` reads what `A` holds: `B` is
//! renamed to `A` and the copy, now a store of `r` over the cell it was just
//! loaded from, is dropped. The cell `B` is named no more and the frame loses
//! its bytes.
//!
//! The rule is taken over fixed frame cells only, and not where an address of
//! the frame is taken (a pointer may then write either cell). Each cell takes
//! part in one rename.
//!
//! # Example
//! x87 `lrint` stores through a stack temporary and loads it back; the value
//! then lives across a call, and the spiller gives it a cell of its own:
//!
//! ```text
//!  before                      after
//!  fistp dword [bp-4]          fistp dword [bp-4]
//!  mov eax, [bp-4]             mov eax, [bp-4]
//!  mov [bp-8], eax
//!  call p                      call p
//!  mov eax, [bp-8]             mov eax, [bp-4]
//! ```
//!
//! From unit test `test_a_spill_copied_from_a_cell_reads_that_cell` in
//! `peephole_tests.rs`.
//!
//! # Runs
//! A sub-pass of [`crate::backend::peephole`], after `postrasink`.
//!
//! # Keeps
//! None is returned where no copy qualifies, and the frame is left as it is.
//!
//! # Deviations from the pin
//! - DEVIATE S1: the pin keeps both cells. `StackSlotColoring` shares only the
//!   cells `InlineSpiller` made, so the stack temporary of an x87 conversion
//!   and the spill of its result stay apart (`llc -O2 -mtriple=i386-code16` on
//!   the IR of `long v = (long)x; p(v); q(v);` with a clobber-all asm after `p`
//!   spills `movl %eax, -20(%ebp)` from `fistpl -24(%ebp)`). The old allocator
//!   reloaded from the first cell, and m16 `-Os` must not grow past main. Exit:
//!   none; this is a size rule.

use std::sync::Arc;

use crate::backend::postrasink::{may_read, overwrites, spill_store_of};
use crate::backend::storedhomes::apart;
use crate::model::ir::{Loc, Mem, Operation, Reg};
use crate::model::lir::{Block, Insn, LirBlock, LirBody};
use crate::objectfile::module::Space;
use crate::support::hash::{HashSet, IndexMap};

/// A cell named by its place in the frame alone.
fn fixed(cell: &Mem) -> bool {
    cell.base.is_none()
        && cell.index.is_none()
        && cell.selector.is_none()
        && cell.addr.is_some_and(|addr| addr.space == Space::Frame)
}

/// The register a plain load of a fixed frame cell fills, and the cell.
fn load_of(one: &Insn) -> Option<(Reg, Mem)> {
    if one.volatile || one.group.is_some() || !one.clobbers.is_empty() || !one.requires.is_empty() {
        return None;
    }
    let what = one.what.as_ref()?;
    match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::Move, Some("mov"), [Loc::Reg(dest)], [Loc::Mem(cell)]) if fixed(cell) => {
            Some((*dest, cell.clone()))
        }
        _ => None,
    }
}

/// Whether `one` names `cell` by anything but its exact bytes.
fn partly(
    one: &Insn,
    cell: &Mem,
) -> bool {
    let Some(what) = one.what.as_ref() else { return false };
    what.dests
        .iter()
        .chain(&what.sources)
        .any(
            |place| match place {
                Loc::Mem(other) => {
                    !apart(cell, other)
                        && (!fixed(other)
                            || other.width != cell.width
                            || other.addr.map(|addr| addr.disp) != cell.addr.map(|addr| addr.disp))
                }
                _ => false,
            },
        )
}

/// Whether a read of `b` can come after a write of `a` on a path from the
/// copy at `copy` (block, index of the load): the read would see what `a`
/// held when the copy ran only if `a` was not written since.
fn stale(
    body: &LirBody,
    copy: (i64, usize),
    a: &Mem,
    b: &Mem,
) -> bool {
    let by_at: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut seen: HashSet<(i64, bool)> = HashSet::default();
    let mut work = vec![(copy.0, copy.1 + 2, false)];
    while let Some((at, from, mut crossed)) = work.pop() {
        let block = by_at[&at];
        let mut ended = false;
        for (index, one) in block.insns.iter().enumerate().skip(from) {
            // The copy runs again: `b` is made from `a` afresh.
            if at == copy.0 && index == copy.1 {
                ended = true;
                break;
            }
            if crossed && may_read(one, b) {
                return true;
            }
            crossed |= overwrites(one, a);
        }
        if !ended {
            for next in block.succ.iter() {
                if by_at.contains_key(next) && seen.insert((*next, crossed)) {
                    work.push((*next, 0, crossed));
                }
            }
        }
    }
    false
}

/// `body` with each spill cell copied from a cell and written by nothing else
/// read from that cell, and the copy gone; none where no copy qualifies.
pub fn merged(body: &LirBody) -> Option<LirBody> {
    let addressed = body
        .blocks
        .iter()
        .flat_map(|block| &block.insns)
        .any(
            |one| one.what
                .as_ref()
                .is_some_and(
                    |what| what.dests
                        .iter()
                        .chain(&what.sources)
                        .any(
                            |place| matches!(
                                place,
                                Loc::Address(cell) if cell.addr.is_none_or(|addr| addr.space == Space::Frame)
                            ),
                        ),
                ),
        );
    if addressed || body.returns_twice {
        return None;
    }
    let mut renames: Vec<(Mem, Mem)> = Vec::new();
    let mut dropped: HashSet<(i64, usize)> = HashSet::default();
    for block in &body.blocks {
        for (index, pair) in block.insns.windows(2).enumerate() {
            let Some((loaded, a)) = load_of(&pair[0]) else { continue };
            let Some((stored, b)) = spill_store_of(&pair[1]) else { continue };
            if loaded.register != stored.register
                || a.width != b.width
                || !fixed(&b)
                || !apart(&a, &b)
                || renames.iter().any(|(from, to)| !apart(from, &a) || !apart(from, &b) || !apart(to, &b))
            {
                continue;
            }
            let others = body
                .blocks
                .iter()
                .flat_map(|one| one.insns.iter().enumerate().map(move |(at, insn)| (one.at, at, insn)));
            let alone = others
                .filter(|(at, place, _)| (*at, *place) != (block.at, index + 1))
                .all(|(_, _, insn)| !overwrites(insn, &b) && !partly(insn, &b));
            if alone && !stale(body, (block.at, index), &a, &b) {
                renames.push((b, a));
                dropped.insert((block.at, index + 1));
            }
        }
    }
    if renames.is_empty() {
        return None;
    }
    let rename = |place: &Loc| -> Loc {
        match place {
            Loc::Mem(cell) => match renames.iter().find(|(from, _)| fixed(cell) && !apart(from, cell)) {
                Some((_, to)) => Loc::Mem(Mem { addr: to.addr, offset: to.offset, ..cell.clone() }),
                None => place.clone(),
            },
            other => other.clone(),
        }
    };
    let blocks: Vec<Block> = body
        .blocks
        .iter()
        .map(|block| {
            let insns: Vec<Arc<Insn>> = block
                .insns
                .iter()
                .enumerate()
                .filter(|(index, _)| !dropped.contains(&(block.at, *index)))
                .map(|(_, one)| {
                    let Some(what) = &one.what else { return Arc::clone(one) };
                    let dests: Vec<Loc> = what.dests.iter().map(&rename).collect();
                    let sources: Vec<Loc> = what.sources.iter().map(&rename).collect();
                    if dests == what.dests && sources == what.sources {
                        return Arc::clone(one);
                    }
                    Arc::new(Insn {
                        what: Some(crate::model::ir::Semantics { dests, sources, ..what.clone() }),
                        ..(**one).clone()
                    })
                })
                .collect();
            Block::from(LirBlock { insns: insns.into(), ..block.clone().into_inner() })
        })
        .collect();
    Some(body.with_blocks(blocks))
}
