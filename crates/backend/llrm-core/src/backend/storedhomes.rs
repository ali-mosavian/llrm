//! The cells the optimizer proved hold a phi's value (`!llrm.home`,
//! `LirBody.homes`), as the body now stands: the one fact both spillers ask. A
//! spilled value read from its cell needs no store of its own and no cell of
//! its own.
//!
//! The proof is the optimizer's, over MIR. What is checked here is what a later
//! pass may have changed: nothing writes the cell while the value lives, and
//! every definition of the value is a plain copy, which spilling removes.

use std::collections::BTreeSet;

use crate::backend::allocate::live;
use crate::backend::overlap;
use crate::backend::spiller::_may_write;
use crate::model::ir::{Loc, Mem, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::IndexMap;

/// Whether `cell` and `other` are certainly different bytes: both fixed
/// addresses (no register in either) that `overlap` tells apart. The machine
/// cannot tell two pointers or two elements of one array apart, nor a pointer
/// from a global (`overlap`'s rule: that is MIR's to say), so anything else may
/// overlap.
pub(crate) fn apart(
    cell: &Mem,
    other: &Mem,
) -> bool {
    let fixed = |one: &Mem| one.base.is_none() && one.index.is_none() && one.selector.is_none() && one.addr.is_some();
    fixed(cell) && fixed(other) && !overlap::may_overlap(cell.addr, cell.width, other.addr, other.width)
}

/// Whether `one` stores to bytes of `cell` by a fixed address: the same global,
/// the same frame cell.
fn writes_there(
    one: &Insn,
    cell: &Mem,
) -> bool {
    let fixed = |one: &Mem| one.base.is_none() && one.index.is_none() && one.selector.is_none() && one.addr.is_some();
    fixed(cell)
        && one.what.as_ref().is_some_and(|what| {
            what.dests.iter().any(|place| matches!(place, Loc::Mem(other) if fixed(other) && !apart(cell, other)))
        })
}

/// Whether `one` may write a byte of `cell`, where nothing else vouches for
/// what a pointer reaches: the spiller's answer for the frame, and for any
/// other memory a store that is not certainly elsewhere. (The spiller's alone
/// said no to a store to the very global a load read: nothing in it but frame
/// cells was asked.)
pub fn may_write(
    one: &Insn,
    cell: &Mem,
    sealed: bool,
) -> bool {
    _may_write(one, cell, sealed)
        || one
            .what
            .as_ref()
            .is_some_and(|what| what.dests.iter().any(|place| matches!(place, Loc::Mem(other) if !apart(cell, other))))
}

/// Whether `one` only copies one value to another.
fn copies(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| matches!(what.op, Operation::Move | Operation::FloatLoad)
                && matches!(
                    (what.dests.as_slice(), what.sources.as_slice()),
                    ([Loc::Held(_)], [Loc::Held(_)])
                )
                && one.clobbers.is_empty()
                && one.delivers.is_empty()
                && one.requires.is_empty(),
        )
}

/// Whether `one` stores `value` to `cell`: it leaves in the cell what it holds.
fn stores(
    one: &Insn,
    value: u32,
    cell: &Mem,
) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| matches!(what.op, Operation::Move | Operation::FloatStore)
                && matches!(what.dests.as_slice(), [Loc::Mem(dest)] if dest == cell)
                && matches!(
                    what.sources.as_slice(),
                    [Loc::Held(held)] if held.value == value
                ),
        )
}

/// Each value `wanted` accepts that the body's homes name and that still holds:
/// its cell.
pub fn held(
    body: &LirBody,
    wanted: &dyn Fn(u32, &Mem) -> bool,
) -> IndexMap<u32, Mem> {
    let mut found: IndexMap<u32, Mem> = body
        .homes
        .iter()
        .filter(|(value, cell)| wanted(**value, cell))
        .map(|(value, cell)| (*value, cell.clone()))
        .collect();
    if found.is_empty() {
        return found;
    }
    // Every definition is a copy.
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        if !copies(one) {
            for value in &one.defines {
                found.shift_remove(value);
            }
        }
    }
    let (_, live_out) = live(body);
    for block in &body.blocks {
        let mut alive: BTreeSet<u32> =
            live_out[&block.at].iter().copied().filter(|value| found.contains_key(value)).collect();
        for one in block.insns.iter().rev() {
            // What a pointer reaches is the optimizer's proof; what is checked
            // is a write to the cell itself.
            let disturbed: Vec<u32> = alive
                .iter()
                .copied()
                .filter(|value| {
                    (_may_write(one, &found[value], body.sealed_arguments) || writes_there(one, &found[value]))
                        && !stores(one, *value, &found[value])
                })
                .collect();
            for value in disturbed {
                found.shift_remove(&value);
                alive.remove(&value);
            }
            for value in &one.defines {
                alive.remove(value);
            }
            alive.extend(one.uses.iter().copied().filter(|value| found.contains_key(value)));
        }
    }
    found
}
