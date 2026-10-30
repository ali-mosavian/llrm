//! Finds the memory cells that provably still hold a phi's value, so a spilled
//! value read from one needs no store and no slot of its own.
//!
//! # How it works
//! MIR proves, for a phi, that a memory cell already holds its value
//! (`!llrm.home`, carried into the body as `LirBody.homes`). A spilled value
//! read from such a cell needs no store and no slot of its own. This module
//! re-checks the proof against the machine code, since later phases may have
//! changed it:
//!
//! ```text
//!  held(body, wanted):
//!    homes the body names, filtered by the caller's `wanted`
//!      drop a value with a definition that is not a plain copy
//!        (a spilled value's definitions are removed by spilling)
//!      walk each block backward with the values live there
//!        drop a value when an instruction may write its cell
//!        and is not itself a store of that value to the cell
//! ```
//!
//! `may_write` is the spiller's own test for the frame, plus any store to other
//! memory that is not certainly elsewhere. `apart` says two cells are certainly
//! different bytes: both are fixed addresses that `overlap` separates. A
//! pointer, or two elements of an array, are never apart; what a pointer
//! reaches is the optimizer's proof, not checked here.
//!
//! # Example
//! No honest example: `held`, `may_write` and `apart` answer queries and print
//! no LIR of their own. Read the spiller's `ISEL_DUMP=1` output (see
//! [`crate::backend::lirtext`]) to see a spilled phi read from its home cell.
//!
//! # Used by
//! [`crate::backend::spiller`] and [`crate::backend::floatassign`] (`held`);
//! [`crate::backend::floatfold`] (`may_write`); [`crate::backend::postrasink`]
//! (`apart`).

use std::collections::BTreeSet;

use crate::backend::bodyfacts::_may_write;
use crate::backend::liverows::live_rows_by;
use crate::backend::overlap;
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
    // Only these values are looked at, where they occur: every instruction of
    // the body was walked, and the liveness of every value solved, afresh
    // for each spill round to read the few homes in it.
    let asked: BTreeSet<u32> = found.keys().copied().collect();
    // Every definition is a copy.
    let reads = crate::backend::postings::following(body, |postings| {
        let mut reads: BTreeSet<usize> = BTreeSet::new();
        for &value in &asked {
            if postings.defs(value).iter().any(|at| !copies(&body.blocks[at.0 as usize].insns[at.1 as usize])) {
                found.shift_remove(&value);
            }
            reads.extend(postings.uses(value).iter().map(|at| at.0 as usize));
        }
        reads
    });
    let live_out = live_rows_by(body, |value| asked.contains(&value));
    for (position, block) in body.blocks.iter().enumerate() {
        let mut alive: BTreeSet<u32> = live_out.leaving(block.at).filter(|value| found.contains_key(value)).collect();
        // Nothing of these values is live at the exit or read here: no
        // instruction changes anything.
        if alive.is_empty() && !reads.contains(&position) {
            continue;
        }
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
