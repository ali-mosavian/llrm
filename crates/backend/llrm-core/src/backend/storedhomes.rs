//! The cells the optimizer proved hold a phi's value (`!llrm.home`, `LirBody.homes`), as the body now stands: the
//! one fact both spillers ask. A spilled value read from its cell needs no store of its own and no cell of its own.
//!
//! The proof is the optimizer's, over MIR. What is checked here is what a later pass may have changed: nothing
//! writes the cell while the value lives, and every definition of the value is a plain copy, which spilling removes.

use std::collections::BTreeSet;

use crate::backend::allocate::live;
use crate::backend::spiller::_may_write;
use crate::model::ir::{Loc, Mem, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::IndexMap;

/// Whether `one` only copies one value to another.
fn copies(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| {
        matches!(what.op, Operation::Move | Operation::FloatLoad)
            && matches!((what.dests.as_slice(), what.sources.as_slice()), ([Loc::Held(_)], [Loc::Held(_)]))
            && one.clobbers.is_empty()
            && one.delivers.is_empty()
            && one.requires.is_empty()
    })
}

/// Whether `one` stores `value` to `cell`: it leaves in the cell what it holds.
fn stores(one: &Insn, value: u32, cell: &Mem) -> bool {
    one.what.as_ref().is_some_and(|what| {
        matches!(what.op, Operation::Move | Operation::FloatStore)
            && matches!(what.dests.as_slice(), [Loc::Mem(dest)] if dest == cell)
            && matches!(what.sources.as_slice(), [Loc::Held(held)] if held.value == value)
    })
}

/// Each value `wanted` accepts that the body's homes name and that still holds: its cell.
pub fn held(body: &LirBody, wanted: &dyn Fn(u32, &Mem) -> bool) -> IndexMap<u32, Mem> {
    let mut found: IndexMap<u32, Mem> = body.homes.iter().filter(|(value, cell)| wanted(**value, cell)).map(|(value, cell)| (*value, cell.clone())).collect();
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
        let mut alive: BTreeSet<u32> = live_out[&block.at].iter().copied().filter(|value| found.contains_key(value)).collect();
        for one in block.insns.iter().rev() {
            let disturbed: Vec<u32> = alive.iter().copied().filter(|value| _may_write(one, &found[value], body.sealed_arguments) && !stores(one, *value, &found[value])).collect();
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
